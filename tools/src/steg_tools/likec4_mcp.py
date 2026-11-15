"""Query LikeC4's MCP server over stdio, the same way `.mcp.json` runs it (doc-16 R108).

A thin client: just enough of the Model Context Protocol to call `read-project-summary`
and read back the project's title, view ids and element ids. `read_project_summary` is
the only function that touches a process or a pipe; parsing its response is pure.
"""

from __future__ import annotations

import json
import queue
import subprocess
import threading
from collections.abc import Mapping, Sequence
from dataclasses import dataclass
from pathlib import Path

#: The LikeC4 version `.mcp.json` pins the server to, kept in step by hand with the
#: `likec4@` version the justfile's model-validating recipe passes to `npx`.
PINNED_LIKEC4_VERSION = "1.61.0"


@dataclass(frozen=True)
class ProjectSummary:
    """The parts of a `read-project-summary` response this client cares about."""

    title: str
    view_ids: frozenset[str]
    element_ids: frozenset[str]


def _initialize_request() -> Mapping[str, object]:
    return {
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2025-06-18",
            "capabilities": {},
            "clientInfo": {"name": "steg-tools-likec4-mcp-smoke", "version": "0.1.0"},
        },
    }


def _initialized_notification() -> Mapping[str, object]:
    return {"jsonrpc": "2.0", "method": "notifications/initialized"}


def _read_project_summary_request() -> Mapping[str, object]:
    return {
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/call",
        "params": {"name": "read-project-summary", "arguments": {}},
    }


def parse_project_summary(responses: Sequence[Mapping[str, object]]) -> ProjectSummary:
    """Pull the title, view ids and element ids out of two JSON-RPC responses.

    `responses` is whatever came back for `_initialize_request` and
    `_read_project_summary_request`, in either order.

    Raises:
        LookupError: there is no response with id 2, or its result is not the
            `read-project-summary` tool's structured content.
    """
    by_id = {response.get("id"): response for response in responses}
    summary_response = by_id.get(2)
    if summary_response is None:
        raise LookupError(f"no response to the read-project-summary call (id 2): {responses!r}")
    result = summary_response.get("result")
    if not isinstance(result, Mapping) or result.get("isError"):
        raise LookupError(f"read-project-summary did not return a result: {summary_response!r}")
    content = result["structuredContent"]
    return ProjectSummary(
        title=content["title"],
        view_ids=frozenset(view["id"] for view in content["views"]),
        element_ids=frozenset(element["id"] for element in content["elements"]),
    )


def read_project_summary(likec4_path: Path, *, likec4_version: str = PINNED_LIKEC4_VERSION, timeout: float = 30.0) -> ProjectSummary:
    """Start `likec4 mcp --stdio` on `likec4_path` and read back its project summary.

    This runs the exact command `.mcp.json` configures (`npx --yes likec4@<version> mcp
    --stdio <path>`), speaks the stdio JSON-RPC transport directly, and tears the
    process down again - it is the real gateway, not a stand-in for it.

    Raises:
        TimeoutError: the server didn't answer within `timeout` seconds.
        LookupError: see `parse_project_summary`.
    """
    process = subprocess.Popen(
        ["npx", "--yes", f"likec4@{likec4_version}", "mcp", "--stdio", str(likec4_path)],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    assert process.stdin is not None
    assert process.stdout is not None

    # A plain `readline()` would block forever if the server never answers, and closing
    # stdin to force EOF races the server's own processing (confirmed: it answers in well
    # under a second normally, but closing stdin immediately after writing cuts the
    # in-flight tool call off before it replies). A background reader plus a bounded
    # `queue.get` gives a real timeout without either risk.
    lines: queue.Queue[str] = queue.Queue()

    def _drain_stdout() -> None:
        assert process.stdout is not None
        for line in process.stdout:
            lines.put(line)

    reader = threading.Thread(target=_drain_stdout, daemon=True)
    reader.start()

    try:
        for message in (_initialize_request(), _initialized_notification(), _read_project_summary_request()):
            process.stdin.write(json.dumps(message) + "\n")
        process.stdin.flush()

        responses = []
        for _ in range(2):  # one reply per request; the notification gets none
            try:
                line = lines.get(timeout=timeout)
            except queue.Empty:
                raise TimeoutError(f"likec4 mcp did not answer within {timeout}s") from None
            responses.append(json.loads(line))
        return parse_project_summary(responses)
    finally:
        process.stdin.close()
        try:
            process.wait(timeout=5)
        except subprocess.TimeoutExpired:
            process.kill()
