"""Run a command and say one line when it passes.

Output costs tokens, so a passing step prints its label and how long it took, and
nothing else. A failing step prints the end of its output and the path of the full
log. Set STEG_VERBOSE=1 to see everything as it happens.

Usage: steg-quiet <label> -- <command> [args...]
"""

import os
import re
import subprocess
import sys
import time
from pathlib import Path

LOG_DIR = Path(".quiet")
TAIL_LINES = 60


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


def split_arguments(argv: list[str]) -> tuple[str, list[str]]:
    """Split `<label> -- <command...>`.

    Raises:
        ValueError: The arguments are not a label, `--`, and a command.
    """
    if len(argv) < 3 or argv[1] != "--":
        raise ValueError("usage: steg-quiet <label> -- <command> [args...]")
    return argv[0], argv[2:]


def main() -> int:
    """Run the command, keep its output in a log, and report."""
    try:
        label, command = split_arguments(sys.argv[1:])
    except ValueError as error:
        print(error, file=sys.stderr)
        return 2
    started = time.monotonic()
    if os.environ.get("STEG_VERBOSE"):
        code = subprocess.run(command).returncode
        print(passed_line(label, time.monotonic() - started) if code == 0 else f"FAILED  {label} (exit {code})")
        return code
    done = subprocess.run(command, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, errors="replace")
    seconds = time.monotonic() - started
    LOG_DIR.mkdir(exist_ok=True)
    log = LOG_DIR / f"{slug(label)}.log"
    log.write_text(done.stdout)
    if done.returncode == 0:
        print(passed_line(label, seconds))
    else:
        print(failed_report(label, done.returncode, seconds, done.stdout, log))
    return done.returncode

