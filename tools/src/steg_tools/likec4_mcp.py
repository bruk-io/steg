"""Query LikeC4's MCP server over stdio, the same way `.mcp.json` runs it (doc-16 R108).

A thin client: just enough of the Model Context Protocol to call `read-project-summary`
and read back the project's title, view ids and element ids. `summary_from_command` is
the only function that touches a process or a pipe (`read_project_summary` builds the
real `npx` command and hands it to that); parsing a response is pure.
"""

import json
import queue
import subprocess
import threading
from collections.abc import Iterable, Iterator, Mapping, Sequence
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


def take_replies(messages: Iterable[Mapping[str, object]], count: int) -> list[Mapping[str, object]]:
    """Collect the first `count` JSON-RPC replies out of `messages`, in order.

    A message with no "id" is a notification, not a reply to anything this client sent
    (the server emits `notifications/...` on its own schedule); it is skipped and does
    not count toward `count`.
    """
    replies: list[Mapping[str, object]] = []
    for message in messages:
        if "id" in message:
            replies.append(message)
            if len(replies) == count:
                break
    return replies


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


def read_project_summary(
    likec4_path: Path,
    *,
    likec4_version: str = PINNED_LIKEC4_VERSION,
    startup_timeout: float = 60.0,
    timeout: float = 5.0,
) -> ProjectSummary:
    """Start `likec4 mcp --stdio` on `likec4_path` and read back its project summary.

    This runs the exact command `.mcp.json` configures (`npx --yes likec4@<version> mcp
    --stdio <path>`) through `summary_from_command`, which speaks the stdio JSON-RPC
    transport directly and tears the process down again - it is the real gateway, not a
    stand-in for it.

    Raises:
        TimeoutError: see `summary_from_command`.
        LookupError: see `parse_project_summary`.
    """
    return summary_from_command(
        ["npx", "--yes", f"likec4@{likec4_version}", "mcp", "--stdio", str(likec4_path)],
        startup_timeout=startup_timeout,
        timeout=timeout,
    )


def summary_from_command(
    argv: Sequence[str],
    *,
    startup_timeout: float = 60.0,
    timeout: float = 5.0,
) -> ProjectSummary:
    """Start `argv` as a child process speaking the stdio JSON-RPC transport, and read
    back its project summary.

    `read_project_summary` is the real caller, with the exact `npx` command `.mcp.json`
    configures; `tests/functional/test_likec4_mcp_phased_timeout.py` is the other one, with a fake
    child in place of a real likec4 server, to drive the two bounds below
    deterministically (`tests/adapter/test_likec4_mcp_smoke.py` is the one test that
    runs a real likec4 server, and proves this function against it).

    The wait for the *first* line out of the process is bounded by `startup_timeout`,
    not `timeout`: for the real `npx` command, that line is the only one that can be
    delayed by `npx` resolving `likec4@<version>` against the npm registry, which the
    root `.npmrc` (`prefer-offline=true`) and CI's own warmed cache (TASK-1.97) keep
    off the network in the common case, but which still has to tolerate a genuine cache
    miss - measured, before that fix, at 9.7-13.4s cold, locally, and up to 70s when
    the registry itself was unreachable and npm's own retry ladder ran out (that 70s
    case is the one the fix actually removes: the same scenario with `.npmrc` present
    takes well under a second, TASK-1.97's notes). Every line after the first is the
    actual JSON-RPC exchange with a server already running, measured well under a
    second even under load, so `timeout` stays tight and a stall there is reported as
    what it is - the server, not npm - rather than folded into one generic "did not
    answer" that could mean either.

    Raises:
        TimeoutError: the child didn't answer its first message within
            `startup_timeout` seconds, or a later one within `timeout` seconds.
        LookupError: see `parse_project_summary`.
    """
    process = subprocess.Popen(
        argv,
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    assert process.stdin is not None
    assert process.stdout is not None
    assert process.stderr is not None

    # A plain `readline()` would block forever if the server never answers, and closing
    # stdin to force EOF races the server's own processing (confirmed: it answers in well
    # under a second normally, but closing stdin immediately after writing cuts the
    # in-flight tool call off before it replies). A background reader plus a bounded
    # `queue.get` gives a real timeout without either risk.
    lines: queue.Queue[str] = queue.Queue()
    stderr_lines: list[str] = []

    def _drain_stdout() -> None:
        assert process.stdout is not None
        for line in process.stdout:
            lines.put(line)

    def _drain_stderr() -> None:
        # Nothing reads the server's stderr otherwise, so a failure would say only
        # "it didn't answer" with no hint why. Collecting it here means a later
        # failure can report it.
        assert process.stderr is not None
        stderr_lines.extend(process.stderr)

    threading.Thread(target=_drain_stdout, daemon=True).start()
    threading.Thread(target=_drain_stderr, daemon=True).start()

    def _received_messages() -> Iterator[Mapping[str, object]]:
        first = True
        while True:
            bound = startup_timeout if first else timeout
            try:
                line = lines.get(timeout=bound)
            except queue.Empty:
                phase = "to start" if first else "mid-exchange"
                raise TimeoutError(f"likec4 mcp did not answer ({phase}) within {bound}s") from None
            first = False
            yield json.loads(line)

    try:
        for message in (_initialize_request(), _initialized_notification(), _read_project_summary_request()):
            process.stdin.write(json.dumps(message) + "\n")
        process.stdin.flush()

        responses = take_replies(_received_messages(), count=2)
        return parse_project_summary(responses)
    except Exception as exc:
        stderr_text = "".join(stderr_lines).strip()
        if stderr_text:
            exc.add_note(f"likec4 mcp stderr:\n{stderr_text}")
        raise
    finally:
        process.stdin.close()
        try:
            process.wait(timeout=5)
        except subprocess.TimeoutExpired:
            process.kill()
