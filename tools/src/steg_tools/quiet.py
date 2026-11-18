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

Several `just check`/`just test` runs on one machine (several worktrees, several
workers) make timing-sensitive tests fail for no reason of their own (TASK-1.132), so
each heavy cargo step - build, test, clippy - takes one of a small number of
machine-wide slots before it runs (`STEG_CHECK_SLOTS`, default below); a cheap step
(fmt, deny, the uv-run Python tools) never needs one. `STEG_CHECK_SLOTS_DIR` points at
the lock directory the slots live in (default below); `STEG_CHECK_SLOT_WAIT_S` bounds
how long a waiter will wait before failing the step outright (default: unbounded).

Usage: steg-quiet <label> -- <command> [args...]
"""

import fcntl
import os
import re
import signal
import subprocess
import sys
import threading
import time
from collections.abc import Iterator, Mapping, Sequence
from contextlib import contextmanager
from pathlib import Path

LOG_DIR = Path(".quiet")
TAIL_LINES = 60

# Revised on review from the plan's own 600s: 300s was set from this task's own
# recorded step timings alone (132.2s the slowest affected-crate selection seen in CI,
# 109.2s the slowest single-package step, 92s a cold full-workspace build with no cache
# at all on this machine) - but every one of those is a `just check` step. One recipe
# that goes through this same `steg-quiet` and is NOT part of `just check`,
# `test-openpty-race-control`, carries its own internal 600s safety net
# (`CONTROL_SAFETY_NET`, tests/openpty_race.rs) before it gives up with its own clear
# message; a bound here below that would have `steg-quiet` kill it first and replace
# that specific message with a bare "TIMED OUT" - and this task's own recorded run of
# that recipe took 214.2s, 71% of 300s, under what was, on the day, an otherwise ordinary
# load. No CI run sampled in this task's own history ever hit a cold Zig/ghostty-vt build
# either (every sampled `build (steg-kernel)` step was under 6s - the tarball-seeded
# cache TASK-1.34 built stays warm), so there is no measured number for the one case the
# plan named by name. 900s clears the 600s internal bound above with real headroom,
# clears every measured `just check`/`just build`/`just test-all` step several times
# over, and still leaves half of CI's own 30-minute job limit free - a false TIMED OUT on
# a slow runner costs more than the extra minutes a looser bound gives up on a real hang,
# so this errs toward the looser number rather than the tightest one the measurements
# alone would have supported.
DEFAULT_TIMEOUT_S = 900.0

# The cargo subcommands a slot limits - the ones that actually compile or run code and
# contend for CPU, disk and the target directory. `cargo fmt --check` and `cargo deny
# check licenses` are metadata-only and never reach this set.
_HEAVY_CARGO_SUBCOMMANDS = frozenset({"build", "test", "clippy"})

DEFAULT_SLOT_COUNT = 2


def heavy_cargo_command(command: Sequence[str]) -> bool:
    """Say whether `command` is a cargo build/test/clippy invocation - the only steps a slot limits."""
    return len(command) >= 2 and command[0] == "cargo" and command[1] in _HEAVY_CARGO_SUBCOMMANDS


def effective_slot_count(env: Mapping[str, str]) -> int | None:
    """How many heavy steps may run machine-wide at once, or `None` for no limit.

    `STEG_CHECK_SLOTS` wins when set: `0` means no limit, used verbatim everywhere
    including CI if set there on purpose. Unset, CI (GitHub Actions always sets `CI`)
    defaults to no limit - its own concurrency group already runs at most one check
    job per ref, so the limiter would only add lock-directory assumptions on an
    ephemeral runner for no benefit. Unset and not CI defaults to `DEFAULT_SLOT_COUNT`.

    Raises:
        ValueError: `STEG_CHECK_SLOTS` is set to a negative number.
    """
    raw = env.get("STEG_CHECK_SLOTS")
    if raw is not None:
        count = int(raw)
        if count < 0:
            raise ValueError(f"STEG_CHECK_SLOTS must not be negative, got {raw!r}")
        return None if count == 0 else count
    if env.get("CI"):
        return None
    return DEFAULT_SLOT_COUNT


def lock_directory(env: Mapping[str, str]) -> Path:
    """Where the slot lock files live - per-user, per-machine, outside every worktree.

    `STEG_CHECK_SLOTS_DIR` overrides everything (also how tests point this at a
    temporary directory); otherwise `$XDG_CACHE_HOME/steg/check-slots` if set, else
    `~/.cache/steg/check-slots`. Never keyed off `$TMPDIR`: it can differ between
    sandboxed shells on the same machine, which would silently defeat the limiter
    while appearing to work.
    """
    override = env.get("STEG_CHECK_SLOTS_DIR")
    if override:
        return Path(override)
    cache_home = env.get("XDG_CACHE_HOME")
    base = Path(cache_home) if cache_home else Path.home() / ".cache"
    return base / "steg" / "check-slots"


def wait_bound_seconds(env: Mapping[str, str]) -> float | None:
    """How long a waiter may wait for a slot before failing, from `STEG_CHECK_SLOT_WAIT_S`, or `None` for no bound."""
    raw = env.get("STEG_CHECK_SLOT_WAIT_S")
    return float(raw) if raw is not None else None


def wait_line(label: str, total: int, wait_bound: float | None) -> str:
    """The one line printed when every one of `total` slots is busy.

    Names what it waits for and whether it has been told a bound, so a worker that
    only sees this much (the rest of its output cut off by some other timeout) can
    tell "still waiting" from "broken" - and says plainly that the step's own
    STEG_QUIET_TIMEOUT_S bound has not started yet, since the wait happens before it.
    """
    if wait_bound is None:
        reason = "waiting for a slot to free with no bound (STEG_CHECK_SLOT_WAIT_S is unset)"
    else:
        reason = f"waiting for a slot to free, told it may wait up to {wait_bound:.0f}s (STEG_CHECK_SLOT_WAIT_S)"
    return (
        f"wait    {label} ({total} of {total} slots busy, STEG_CHECK_SLOTS={total}) - {reason}; "
        "this step's own STEG_QUIET_TIMEOUT_S bound has not started yet"
    )


def _try_each_without_blocking(fds: Sequence[int]) -> int | None:
    """Try every fd in `fds` for an exclusive lock without blocking; return the first that succeeds."""
    for fd in fds:
        try:
            fcntl.flock(fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
            return fd
        except BlockingIOError:
            continue
    return None


def _race_all_slots(paths: Sequence[Path], wait_bound: float | None, lock_dir: Path) -> int:
    """Block on every slot file in `paths` at once; return the fd of whichever frees first.

    One daemon thread per slot, each opening its own fd and making a single blocking
    `flock` call on it - a real kernel wait, not polling. The first thread to acquire
    its lock wins, decided under `decision` so only one ever does; any thread that
    acquires its own lock afterwards releases it again at once, so a loser can never
    hold a slot hostage or starve another waiter. Every thread is a daemon: whichever
    are still blocked when this process exits are torn down with it, their locks
    dropped by the kernel - nothing is left behind for a loser, win or lose.

    Raises:
        TimeoutError: `wait_bound` is set and no slot freed within it.
    """
    decision = threading.Lock()
    won = threading.Event()
    winner: list[int] = []

    def race_one(path: Path) -> None:
        fd = os.open(path, os.O_CREAT | os.O_RDWR)
        fcntl.flock(fd, fcntl.LOCK_EX)
        with decision:
            if winner:
                fcntl.flock(fd, fcntl.LOCK_UN)
                os.close(fd)
            else:
                winner.append(fd)
                won.set()

    for path in paths:
        threading.Thread(target=race_one, args=(path,), daemon=True).start()
    if not won.wait(timeout=wait_bound):
        raise TimeoutError(
            f"gave up waiting {wait_bound:.0f}s for a check slot in {lock_dir} - "
            "set STEG_CHECK_SLOT_WAIT_S or STEG_CHECK_SLOTS to change this"
        )
    return winner[0]


@contextmanager
def held_slot(lock_dir: Path, count: int, label: str, wait_bound: float | None) -> Iterator[None]:
    """Hold one of `count` machine-wide slots, under `lock_dir`, for the life of this block.

    The fast path is a single non-blocking pass over the `count` slot files; if one is
    free, it is taken at once, no thread spawned. Only when every slot is busy does
    this print `wait_line` and fall back to `_race_all_slots`, so a waiter gets
    whichever slot frees first, never a fixed one. Releases automatically on return,
    on an exception, or on this process being killed: the slot's fd is held only by
    this process (never inherited by a child it spawns - PEP 446's default
    close-on-exec, reinforced by `subprocess.Popen`'s own default `close_fds=True`), so
    the kernel drops the flock the instant this process's last reference to that fd
    closes, whether that is this context manager's own `finally` or the process dying
    outright. There is no lock, pid, or state file recording who holds a slot, so a
    killed run leaves nothing behind to clean up.

    Raises:
        TimeoutError: `wait_bound` is set and no slot freed within it.
    """
    lock_dir.mkdir(parents=True, exist_ok=True)
    paths = [lock_dir / f"slot-{i}.lock" for i in range(count)]
    fds = [os.open(path, os.O_CREAT | os.O_RDWR) for path in paths]
    held_fd = _try_each_without_blocking(fds)
    if held_fd is None:
        print(wait_line(label, count, wait_bound))
        sys.stdout.flush()
        try:
            held_fd = _race_all_slots(paths, wait_bound, lock_dir)
        finally:
            for fd in fds:
                os.close(fd)
    else:
        for fd in fds:
            if fd != held_fd:
                os.close(fd)
    try:
        yield
    finally:
        fcntl.flock(held_fd, fcntl.LOCK_UN)
        os.close(held_fd)


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


def _execute(label: str, command: list[str], bound: float) -> int:
    """Run `command` bounded by `bound` seconds, keep its output in a log, and report."""
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


def main() -> int:
    """Run the command, taking a machine-wide slot first if it is a heavy cargo step."""
    try:
        label, command = split_arguments(sys.argv[1:])
    except ValueError as error:
        print(error, file=sys.stderr)
        return 2
    bound = timeout_seconds()
    slot_count = effective_slot_count(os.environ)
    if slot_count is not None and heavy_cargo_command(command):
        lock_dir = lock_directory(os.environ)
        wait_bound = wait_bound_seconds(os.environ)
        try:
            with held_slot(lock_dir, slot_count, label, wait_bound):
                return _execute(label, command, bound)
        except TimeoutError as error:
            print(f"FAILED  {label} ({error})")
            return 1
    return _execute(label, command, bound)
