"""Run a command and say one line when it passes.

Output costs tokens, so a passing step prints its label and how long it took, and
nothing else. A failing step prints the end of its output and the path of the full
log. Set STEG_VERBOSE=1 to see everything as it happens.

A step that hangs - a test with an unbound wait, TASK-1.113's own lesson - must not
spend `just check`'s whole CI time budget on its own: every step is bounded by
`STEG_QUIET_TIMEOUT_S` (default below), and a step that crosses it is killed, process
group and all (`start_new_session=True` at spawn, `killpg` on timeout - the direct
child alone would leave a hung test binary's own spawned processes behind), and
reported as a failure rather than left to run out CI's own 30-minute job limit.

Usage: steg-quiet <label> -- <command> [args...]
"""

import os
import re
import signal
import subprocess
import sys
import time
from pathlib import Path

LOG_DIR = Path(".quiet")
TAIL_LINES = 60

# The slowest step recorded while building TASK-1.113: 132.2s for the affected-crate
# selection in a real CI run (run 44004157135), and 109.2s for the slowest single
# package step (accept-backoff-fixture, same run); a cold build of the whole workspace
# on a developer machine with no cache at all (this task's own worktree, nothing in
# `target/` yet) took 92s. No CI run sampled in this task's own history ever hit a cold
# Zig/ghostty-vt build (every sampled `build (steg-kernel)` step was under 6s - the
# tarball-seeded cache TASK-1.34 built stays warm). 300s clears every one of those with
# real headroom and still leaves room, inside CI's 30-minute job limit, to report
# several such timeouts rather than exhaust it on one.
DEFAULT_TIMEOUT_S = 300.0


def slug(label: str) -> str:
    """Turn a label into a file name."""
    return re.sub(r"[^a-z0-9]+", "-", label.lower()).strip("-") or "step"


def tail(text: str, limit: int) -> str:
    """Keep the last `limit` lines, saying how many were left out."""
    lines = text.splitlines()
    if len(lines) <= limit:
        return "\n".join(lines)
    return "\n".join([f"... {len(lines) - limit} earlier lines left out ...", *lines[-limit:]])


def passed_line(label: str, seconds: float) -> str:
    """Report a passing step in one line."""
    return f"ok      {label} ({seconds:.1f}s)"


def failed_report(label: str, code: int, seconds: float, output: str, log: Path, limit: int = TAIL_LINES) -> str:
    """Report a failing step: what failed, the end of its output, and where the rest is."""
    return "\n".join([f"FAILED  {label} (exit {code}, {seconds:.1f}s)", tail(output, limit), f"full output: {log}"])


def timed_out_report(label: str, bound: float, seconds: float, output: str, log: Path, limit: int = TAIL_LINES) -> str:
    """Report a step killed for crossing its bound: what it was, the end of its output so far, and where the rest is."""
    return "\n".join(
        [
            f"TIMED OUT {label} (bound {bound:.0f}s, ran {seconds:.1f}s)",
            tail(output, limit),
            f"full output: {log}",
        ]
    )


def split_arguments(argv: list[str]) -> tuple[str, list[str]]:
    """Split `<label> -- <command...>`.

    Raises:
        ValueError: The arguments are not a label, `--`, and a command.
    """
    if len(argv) < 3 or argv[1] != "--":
        raise ValueError("usage: steg-quiet <label> -- <command> [args...]")
    return argv[0], argv[2:]


def timeout_seconds() -> float:
    """The bound a step gets, from `STEG_QUIET_TIMEOUT_S` or the default."""
    return float(os.environ.get("STEG_QUIET_TIMEOUT_S", DEFAULT_TIMEOUT_S))


def _kill_group(proc: subprocess.Popen[str] | subprocess.Popen[bytes]) -> None:
    """Kill the whole process group a bounded step started, not only `proc` itself.

    `start_new_session=True` at spawn made `proc.pid` its own process group leader, so
    this reaches a hung test binary's own spawned processes too - the direct child
    alone would leave them running. Already-exited is not an error here: the group may
    have torn itself down between the timeout firing and this call.
    """
    try:
        os.killpg(os.getpgid(proc.pid), signal.SIGKILL)
    except ProcessLookupError:
        pass


def run_captured(command: list[str], bound: float) -> tuple[int | None, str, float]:
    """Run `command` with its output captured, bounded by `bound` seconds.

    Returns the exit code (`None` on a timeout), the combined stdout/stderr collected
    so far, and how long it ran.
    """
    started = time.monotonic()
    proc = subprocess.Popen(
        command,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
        errors="replace",
        start_new_session=True,
    )
    try:
        output, _ = proc.communicate(timeout=bound)
        return proc.returncode, output, time.monotonic() - started
    except subprocess.TimeoutExpired:
        _kill_group(proc)
        # The group is already dead, so this drains whatever is left (plus whatever
        # was already buffered before the timeout) promptly - not a second bound.
        output, _ = proc.communicate()
        return None, output, time.monotonic() - started


def run_live(command: list[str], bound: float) -> tuple[int | None, float]:
    """Run `command` with its real stdout/stderr (a verbose run), bounded by `bound` seconds.

    Returns the exit code (`None` on a timeout) and how long it ran.
    """
    started = time.monotonic()
    proc = subprocess.Popen(command, start_new_session=True)
    try:
        return proc.wait(timeout=bound), time.monotonic() - started
    except subprocess.TimeoutExpired:
        _kill_group(proc)
        proc.wait()
        return None, time.monotonic() - started


def main() -> int:
    """Run the command, keep its output in a log, and report."""
    try:
        label, command = split_arguments(sys.argv[1:])
    except ValueError as error:
        print(error, file=sys.stderr)
        return 2
    bound = timeout_seconds()
    if os.environ.get("STEG_VERBOSE"):
        code, seconds = run_live(command, bound)
        if code is None:
            print(f"TIMED OUT {label} (bound {bound:.0f}s, ran {seconds:.1f}s)")
            return 1
        print(passed_line(label, seconds) if code == 0 else f"FAILED  {label} (exit {code})")
        return code
    code, output, seconds = run_captured(command, bound)
    LOG_DIR.mkdir(exist_ok=True)
    log = LOG_DIR / f"{slug(label)}.log"
    log.write_text(output)
    if code is None:
        print(timed_out_report(label, bound, seconds, output, log))
        return 1
    if code == 0:
        print(passed_line(label, seconds))
    else:
        print(failed_report(label, code, seconds, output, log))
    return code
