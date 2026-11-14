#!/usr/bin/env python3
# /// script
# requires-python = ">=3.11"
# dependencies = []
# ///
"""Speak the protocol from Python, with no Rust and no terminal emulator.

This is gate 1 of TASK-1.2's decision rule: plugins are separate Python processes on the same
public API (R42, R75), so a protocol they cannot speak is out whatever else it does. Nothing here
is imported from steg and nothing is generated: the framing is four bytes of length, the control
messages are JSON, and the screen arrives as rows of text runs. The dependency list above is
empty on purpose - that it needs no third-party package at all is part of the finding.

Usage: ./client.py --socket PATH [--program PROG] [--seconds N]
"""

from __future__ import annotations

import argparse
import json
import socket
import struct
import sys
import time
from typing import Any

MAGIC = "steg"
PROTOCOL = 1
CONTROL_STREAM = 0
KIND_HELLO, KIND_CONTROL, KIND_DATA = 0, 1, 2
HEADER = struct.Struct(">IB")


class Link:
    """Hold one connection to a steg server and the bytes not yet framed."""

    def __init__(self, path: str) -> None:
        self.sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        self.sock.connect(path)
        self.buf = bytearray()
        self.next_id = 1

    def send(self, stream: int, kind: int, payload: bytes) -> None:
        """Write one frame: a 4-byte big-endian length, the header, then the payload."""
        body = HEADER.pack(stream, kind) + payload
        self.sock.sendall(struct.pack(">I", len(body)) + body)

    def send_json(self, stream: int, kind: int, value: Any) -> None:
        """Write one frame whose payload is JSON."""
        self.send(stream, kind, json.dumps(value).encode())

    def read_frame(self, timeout: float | None = None) -> tuple[int, int, bytes] | None:
        """Return the next whole frame, or None when the deadline passes or the stream ends."""
        deadline = None if timeout is None else time.monotonic() + timeout
        while True:
            if len(self.buf) >= 4:
                (length,) = struct.unpack(">I", self.buf[:4])
                if len(self.buf) >= 4 + length:
                    body = bytes(self.buf[4 : 4 + length])
                    del self.buf[: 4 + length]
                    stream, kind = HEADER.unpack(body[:5])
                    return stream, kind, body[5:]
            if deadline is not None:
                left = deadline - time.monotonic()
                if left <= 0:
                    return None
                self.sock.settimeout(left)
            try:
                chunk = self.sock.recv(65536)
            except TimeoutError:
                return None
            if not chunk:
                return None
            self.buf.extend(chunk)

    def hello(self) -> dict[str, Any]:
        """Do the handshake, and fail loudly with the server's own message if it refuses."""
        self.send_json(
            CONTROL_STREAM,
            KIND_HELLO,
            {"steg": MAGIC, "protocol": PROTOCOL, "client": "client.py/0.0.0"},
        )
        frame = self.read_frame(timeout=5)
        if frame is None:
            raise SystemExit("the server closed the connection without answering the hello")
        _stream, kind, payload = frame
        if kind != KIND_HELLO:
            raise SystemExit(f"the server's first frame is kind {kind}, not a hello")
        answer = json.loads(payload)
        if "error" in answer:
            raise SystemExit(answer["message"])
        return answer

    def call(self, method: str, params: dict[str, Any] | None = None) -> Any:
        """Make one control call and wait for the answer with the same id."""
        request_id = self.next_id
        self.next_id += 1
        self.send_json(
            CONTROL_STREAM,
            KIND_CONTROL,
            {"type": "request", "id": request_id, "method": method, "params": params or {}},
        )
        while True:
            frame = self.read_frame(timeout=30)
            if frame is None:
                raise SystemExit(f"{method}: no answer")
            stream, kind, payload = frame
            if kind != KIND_CONTROL or stream != CONTROL_STREAM:
                continue
            message = json.loads(payload)
            if message.get("id") != request_id:
                continue
            if message["type"] == "error":
                raise SystemExit(f"{method}: {message['code']}: {message['message']}")
            return message["result"]


def row_text(row: dict[str, Any]) -> str:
    """Join a row's runs into plain text, as a capture shows it."""
    return "".join(run["text"] for run in row["runs"]).rstrip()


def render(scrollback: list[dict[str, Any]], screen: list[dict[str, Any]]) -> str:
    """Render scrollback and screen the way the server's own capture does."""
    return "".join(f"{row_text(row)}\n" for row in [*scrollback, *screen])


def apply_delta(
    screen: list[dict[str, Any]],
    scrollback: list[dict[str, Any]],
    delta: dict[str, Any],
) -> None:
    """Apply one delta in place: dropped rows, appended rows, then the changed screen rows."""
    for _ in range(delta["scrollback_dropped"]):
        if scrollback:
            scrollback.pop(0)
    scrollback.extend(delta["scrollback_appended"])
    for change in delta["rows"]:
        index = change["index"]
        while len(screen) <= index:
            screen.append({"runs": []})
        screen[index] = change["row"]


def main() -> int:
    """Attach to a pane from Python and check the screen against the server's own."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--socket", required=True)
    parser.add_argument("--program", default=None, help="run this program in a new pane")
    parser.add_argument("--lines", type=int, default=3000)
    parser.add_argument("--seconds", type=float, default=2.0)
    args = parser.parse_args()

    link = Link(args.socket)
    server = link.hello()
    print(f"handshake server={server['server']} protocol={server['protocol']} api={server['api']}")

    described = link.call("describe")
    methods = [method["name"] for method in described["methods"]]
    print(f"describe  protocol={described['protocol']} api={described['api']}")
    print(f"describe  {len(methods)} methods: {', '.join(methods)}")

    if args.program:
        opened = link.call(
            "pane.open",
            {
                "program": args.program,
                "args": ["emit", "--lines", str(args.lines), "--interval-us", "400"],
                "cols": 80,
                "rows": 24,
            },
        )
        pane = opened["pane"]
    else:
        pane = link.call("pane.list")["panes"][0]["pane"]
    print(f"pane      {pane}")

    attached = link.call("pane.attach", {"pane": pane, "mode": "deltas", "cols": 80, "rows": 24})
    snapshot = attached["snapshot"]
    stream = attached["stream"]
    screen = list(snapshot["screen"])
    scrollback = list(snapshot["scrollback"])
    print(
        f"attach    attachment={attached['attachment']} stream={stream} seq={snapshot['seq']} "
        f"{snapshot['cols']}x{snapshot['rows']} scrollback_rows={len(scrollback)}"
    )

    # Input travels as a data frame on the attachment's own stream, so a plugin can type into a
    # pane without a second connection.
    link.send(stream, KIND_DATA, b"")

    deltas = resyncs = 0
    deadline = time.monotonic() + args.seconds
    while time.monotonic() < deadline:
        frame = link.read_frame(timeout=deadline - time.monotonic())
        if frame is None:
            break
        got_stream, kind, payload = frame
        if got_stream != stream or kind != KIND_CONTROL:
            continue
        update = json.loads(payload)
        if update["update"] == "delta":
            apply_delta(screen, scrollback, update)
            deltas += 1
        else:
            snapshot = update["snapshot"]
            screen = list(snapshot["screen"])
            scrollback = list(snapshot["scrollback"])
            resyncs += 1
    print(f"updates   deltas={deltas} resyncs={resyncs} rows_held={len(scrollback) + len(screen)}")

    # The gate: a Python client with no emulator holds the same screen as the server.
    mine = render(scrollback[-10_000:], screen)
    theirs = link.call("pane.capture", {"pane": pane, "scrollback": True})["text"]
    if mine == theirs:
        print(f"agree     {len(mine)} bytes, {mine.count(chr(10))} lines, byte for byte")
        return 0
    print("differ    the Python client and the server disagree", file=sys.stderr)
    for index, (left, right) in enumerate(zip(mine.splitlines(), theirs.splitlines()), start=1):
        if left != right:
            print(f"          first at line {index}: {left!r} against {right!r}", file=sys.stderr)
            break
    else:
        print(
            f"          {len(mine.splitlines())} lines against {len(theirs.splitlines())}",
            file=sys.stderr,
        )
    return 1


if __name__ == "__main__":
    raise SystemExit(main())
