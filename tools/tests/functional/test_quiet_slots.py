"""The machine-wide slot limiter (TASK-1.132): real `steg-quiet` processes, real flock.

Every subprocess here gets a minimal, explicit environment - never `os.environ` passed
through - so a test can never print a credential it inherited by accident, and every
wait on a subprocess is bounded (`subprocess.Popen.communicate(timeout=...)`), never a
bare wait. A fake `cargo` on each subprocess's own `PATH` stands in for the real
compiler: it is invoked exactly as `heavy_cargo_command` expects (`cargo <subcommand>
...`), so the detection in `quiet.py` is exercised unchanged, while its own duration is
picked by the test instead of however long a real build takes.

Timings (reviewed after a round of findings on a loaded machine - several other
workers build here at the same time as these tests run, and this machine has been
seen failing other tasks' own timing-sensitive tests under that load):

- `ORDER_SLACK_S` bounds an inequality that only ever gets MORE lenient by widening it
  (a "must come at/after" check) - safe to make generous, since widening it can never
  hide the opposite kind of bug, only avoid a false failure from ordinary scheduling
  delay under load.
- `CONCURRENT_SLACK_S` bounds how far apart two starts meant to happen "together" may
  be. This one is deliberately kept smaller than any `duration` it is compared against
  in the same test: widening it without also widening the gap it is meant to catch
  would let a real serialisation bug hide behind the slack itself.
- `HARNESS_BOUND_S` is this test file's own patience (`communicate(timeout=...)`,
  `_wait_for`'s deadline) - generous and separate from the above, since a slow but
  eventually-correct run under load should never fail the test harness itself, only
  the correctness assertions should ever catch a real regression.
- Every `duration_s` chosen is comfortably larger than whatever slack or bound it is
  compared against, so the signal (a real bug) stays well clear of the noise (ordinary
  scheduling delay).
"""

import os
import signal
import subprocess
import sys
import time
from pathlib import Path

from steg_tools.quiet import heavy_cargo_command

STEG_QUIET = str(Path(sys.executable).parent / "steg-quiet")

ORDER_SLACK_S = 2.0
CONCURRENT_SLACK_S = 1.0
HARNESS_BOUND_S = 30.0


def _fake_cargo(bin_dir: Path) -> None:
    """Write a `cargo` onto `bin_dir` that times itself instead of compiling anything.

    argv: `<subcommand> <duration> <start file> <end file> <pid file>` - the pid file
    is written first and unconditionally, so a test can always find and kill this
    process by pid afterwards, win or lose.
    """
    script = bin_dir / "cargo"
    script.write_text(
        f"#!{sys.executable}\n"
        "import os, sys, time, pathlib\n"
        "_, _subcommand, duration_s, start_file, end_file, pid_file = sys.argv\n"
        "pathlib.Path(pid_file).write_text(str(os.getpid()))\n"
        "pathlib.Path(start_file).write_text(repr(time.time()))\n"
        "time.sleep(float(duration_s))\n"
        "pathlib.Path(end_file).write_text(repr(time.time()))\n"
    )
    script.chmod(0o755)


def _env(bin_dir: Path | None, **overrides: str) -> dict[str, str]:
    """A minimal, explicit environment - `PATH` only to resolve the fake `cargo`, plus the variables under test."""
    env = {"PATH": str(bin_dir) if bin_dir is not None else ""}
    env.update(overrides)
    return env


def _spawn_heavy(
    *,
    tmp_path: Path,
    bin_dir: Path,
    label: str,
    duration_s: float,
    lock_dir: Path,
    slots: str,
    wait_bound: str | None = None,
) -> tuple[subprocess.Popen[str], Path, Path, Path]:
    """Start one `steg-quiet <label> -- cargo test ...` subprocess; return it and its start/end/pid files."""
    start_file, end_file, pid_file = (tmp_path / f"{label}.start", tmp_path / f"{label}.end", tmp_path / f"{label}.pid")
    env = _env(bin_dir, STEG_CHECK_SLOTS=slots, STEG_CHECK_SLOTS_DIR=str(lock_dir))
    if wait_bound is not None:
        env["STEG_CHECK_SLOT_WAIT_S"] = wait_bound
    command = [
        STEG_QUIET,
        label,
        "--",
        "cargo",
        "test",
        str(duration_s),
        str(start_file),
        str(end_file),
        str(pid_file),
    ]
    proc = subprocess.Popen(command, env=env, cwd=str(tmp_path), stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
    return proc, start_file, end_file, pid_file


def _wait_for(path: Path, deadline_s: float = HARNESS_BOUND_S) -> None:
    """Poll briefly, bounded, for `path` to appear - test glue only, not the mechanism this file proves."""
    started = time.monotonic()
    while not path.exists():
        if time.monotonic() - started > deadline_s:
            raise TimeoutError(f"{path} never appeared within {deadline_s}s")
        time.sleep(0.02)


def _read_timestamp(path: Path) -> float:
    return float(path.read_text())


def _kill_if_running(pid_file: Path) -> None:
    """Defensive cleanup: a test that killed a holder mid-step must not leave its own child behind."""
    if not pid_file.exists():
        return
    try:
        pid = int(pid_file.read_text())
    except ValueError:
        return
    try:
        os.kill(pid, signal.SIGKILL)
    except ProcessLookupError:
        pass


def test_two_heavy_steps_with_one_slot_do_not_overlap(tmp_path: Path) -> None:
    bin_dir, lock_dir = tmp_path / "bin", tmp_path / "slots"
    bin_dir.mkdir()
    _fake_cargo(bin_dir)
    duration = 3.0  # well clear of ORDER_SLACK_S below

    proc_a, start_a, end_a, _ = _spawn_heavy(
        tmp_path=tmp_path, bin_dir=bin_dir, label="a", duration_s=duration, lock_dir=lock_dir, slots="1"
    )
    proc_b, start_b, end_b, _ = _spawn_heavy(
        tmp_path=tmp_path, bin_dir=bin_dir, label="b", duration_s=duration, lock_dir=lock_dir, slots="1"
    )
    assert proc_a.communicate(timeout=HARNESS_BOUND_S)[0] is not None
    assert proc_b.communicate(timeout=HARNESS_BOUND_S)[0] is not None

    first_start, first_end = _read_timestamp(start_a), _read_timestamp(end_a)
    second_start, second_end = _read_timestamp(start_b), _read_timestamp(end_b)
    if second_start < first_start:
        first_start, first_end, second_start, second_end = second_start, second_end, first_start, first_end
    assert second_start >= first_end - ORDER_SLACK_S, "the second run must not start until the first has finished"


def test_the_same_scenario_with_no_limit_overlaps_instead(tmp_path: Path) -> None:
    """The required control for the test above: with the limiter off, the two runs overlap."""
    bin_dir, lock_dir = tmp_path / "bin", tmp_path / "slots"
    bin_dir.mkdir()
    _fake_cargo(bin_dir)
    duration = 3.0

    proc_a, start_a, end_a, _ = _spawn_heavy(
        tmp_path=tmp_path, bin_dir=bin_dir, label="a", duration_s=duration, lock_dir=lock_dir, slots="0"
    )
    proc_b, start_b, end_b, _ = _spawn_heavy(
        tmp_path=tmp_path, bin_dir=bin_dir, label="b", duration_s=duration, lock_dir=lock_dir, slots="0"
    )
    assert proc_a.communicate(timeout=HARNESS_BOUND_S)[0] is not None
    assert proc_b.communicate(timeout=HARNESS_BOUND_S)[0] is not None

    first_start, first_end = _read_timestamp(start_a), _read_timestamp(end_a)
    second_start, second_end = _read_timestamp(start_b), _read_timestamp(end_b)
    if second_start < first_start:
        first_start, first_end, second_start, second_end = second_start, second_end, first_start, first_end
    assert second_start < first_end - ORDER_SLACK_S, "with no limit the two runs should overlap, not serialise"
    assert not lock_dir.exists(), "STEG_CHECK_SLOTS=0 must never touch the lock directory"


def test_a_sigkilled_holder_frees_its_slot_at_once(tmp_path: Path) -> None:
    bin_dir, lock_dir = tmp_path / "bin", tmp_path / "slots"
    bin_dir.mkdir()
    _fake_cargo(bin_dir)
    holder_duration = 8.0  # the holder is never meant to finish naturally - it is killed

    holder, holder_start, _, holder_pid = _spawn_heavy(
        tmp_path=tmp_path, bin_dir=bin_dir, label="holder", duration_s=holder_duration, lock_dir=lock_dir, slots="1"
    )
    waiter_pid = tmp_path / "waiter.pid"
    try:
        _wait_for(holder_start)  # the holder has acquired the one slot and is now sleeping
        os.kill(holder.pid, signal.SIGKILL)  # kill steg-quiet itself, not its fake-cargo child

        started = time.monotonic()
        waiter, waiter_start, waiter_end, waiter_pid = _spawn_heavy(
            tmp_path=tmp_path, bin_dir=bin_dir, label="waiter", duration_s=0.2, lock_dir=lock_dir, slots="1"
        )
        assert waiter.communicate(timeout=HARNESS_BOUND_S)[0] is not None
        elapsed = time.monotonic() - started
        assert elapsed < holder_duration - ORDER_SLACK_S, (
            "a slot held by a killed process must free at once, not after the full duration it was holding for"
        )
        assert _read_timestamp(waiter_end) > _read_timestamp(waiter_start)
    finally:
        _kill_if_running(holder_pid)
        _kill_if_running(waiter_pid)
        try:
            holder.wait(timeout=HARNESS_BOUND_S)
        except subprocess.TimeoutExpired:
            pass


def test_the_variable_sets_how_many_run_at_once(tmp_path: Path) -> None:
    bin_dir, lock_dir = tmp_path / "bin", tmp_path / "slots"
    bin_dir.mkdir()
    _fake_cargo(bin_dir)
    duration = 3.0  # clear of CONCURRENT_SLACK_S, so a real serialisation bug can't hide behind the slack

    procs = [
        _spawn_heavy(tmp_path=tmp_path, bin_dir=bin_dir, label=f"p{i}", duration_s=duration, lock_dir=lock_dir, slots="2") for i in range(3)
    ]
    for proc, _, _, _ in procs:
        assert proc.communicate(timeout=HARNESS_BOUND_S)[0] is not None

    spans = sorted((_read_timestamp(start), _read_timestamp(end)) for _, start, end, _ in procs)
    first, second, third = spans
    assert second[0] - first[0] <= CONCURRENT_SLACK_S, "two of the three should start together - the limit is 2"
    assert third[0] >= min(first[1], second[1]) - ORDER_SLACK_S, "the third must wait for one of the first two to free a slot"
    makespan = max(end for _, end in spans) - min(start for start, _ in spans)
    assert makespan < duration * 2 + ORDER_SLACK_S, "three steps with two slots must not fully serialise to three steps' worth of time"


def test_a_waiter_gets_whichever_slot_frees_first_not_a_fixed_one(tmp_path: Path) -> None:
    """The fix for the convoy review found: a waiter must not be stuck behind one committed slot."""
    bin_dir, lock_dir = tmp_path / "bin", tmp_path / "slots"
    bin_dir.mkdir()
    _fake_cargo(bin_dir)
    long_duration, short_duration = 8.0, 2.0

    long_holder, long_start, long_end, _ = _spawn_heavy(
        tmp_path=tmp_path, bin_dir=bin_dir, label="long", duration_s=long_duration, lock_dir=lock_dir, slots="2"
    )
    _wait_for(long_start)  # claims the first slot alone, so which slot is "long"'s is deterministic

    short_holder, short_start, short_end, _ = _spawn_heavy(
        tmp_path=tmp_path, bin_dir=bin_dir, label="short", duration_s=short_duration, lock_dir=lock_dir, slots="2"
    )
    _wait_for(short_start)  # both slots are now busy before the waiter is started

    waiter, waiter_start, _waiter_end, _ = _spawn_heavy(
        tmp_path=tmp_path, bin_dir=bin_dir, label="waiter", duration_s=0.1, lock_dir=lock_dir, slots="2"
    )
    waiter_output, _ = waiter.communicate(timeout=HARNESS_BOUND_S)
    assert waiter_output is not None
    assert short_holder.communicate(timeout=HARNESS_BOUND_S)[0] is not None
    assert long_holder.communicate(timeout=HARNESS_BOUND_S)[0] is not None

    # Finding #3 (review of TASK-1.132): the wait line itself, not just its formatter, must be printed.
    assert "wait    waiter" in waiter_output
    assert "STEG_CHECK_SLOTS=2" in waiter_output

    waiter_started_at = _read_timestamp(waiter_start)
    short_ended_at = _read_timestamp(short_end)
    long_ended_at = _read_timestamp(long_end)
    assert waiter_started_at <= short_ended_at + ORDER_SLACK_S, "the waiter should acquire the slot that freed quickly"
    assert waiter_started_at < long_ended_at - ORDER_SLACK_S, "the waiter must not have been stuck behind the long holder"


def test_no_limit_never_creates_the_lock_directory(tmp_path: Path) -> None:
    bin_dir, lock_dir = tmp_path / "bin", tmp_path / "slots"
    bin_dir.mkdir()
    _fake_cargo(bin_dir)

    proc, _, end_file, _ = _spawn_heavy(tmp_path=tmp_path, bin_dir=bin_dir, label="a", duration_s=0.1, lock_dir=lock_dir, slots="0")
    assert proc.communicate(timeout=HARNESS_BOUND_S)[0] is not None
    assert end_file.exists()
    assert not lock_dir.exists()


def test_a_cheap_step_is_never_slowed_by_a_full_set_of_heavy_slots(tmp_path: Path) -> None:
    bin_dir, lock_dir = tmp_path / "bin", tmp_path / "slots"
    bin_dir.mkdir()
    _fake_cargo(bin_dir)
    holder_duration = 8.0

    holder, holder_start, _, holder_pid = _spawn_heavy(
        tmp_path=tmp_path, bin_dir=bin_dir, label="holder", duration_s=holder_duration, lock_dir=lock_dir, slots="1"
    )
    try:
        _wait_for(holder_start)  # the single slot is now held for the next 8s

        assert not heavy_cargo_command([sys.executable, "-c", "print('ok')"])
        cheap_command = [STEG_QUIET, "cheap", "--", sys.executable, "-c", "print('ok')"]
        cheap_env = _env(None, STEG_CHECK_SLOTS="1", STEG_CHECK_SLOTS_DIR=str(lock_dir))

        started = time.monotonic()
        cheap = subprocess.run(
            cheap_command, check=False, env=cheap_env, cwd=str(tmp_path), capture_output=True, text=True, timeout=HARNESS_BOUND_S
        )
        elapsed = time.monotonic() - started

        assert cheap.returncode == 0
        assert elapsed < holder_duration - ORDER_SLACK_S, "a cheap step must never queue behind a full set of heavy slots"
    finally:
        _kill_if_running(holder_pid)
        try:
            holder.wait(timeout=HARNESS_BOUND_S)
        except subprocess.TimeoutExpired:
            pass


def test_a_bounded_wait_fails_cleanly_without_running_its_command(tmp_path: Path) -> None:
    """The plan's own step 7: STEG_CHECK_SLOT_WAIT_S expiring fails the step, rather than running it unslotted."""
    bin_dir, lock_dir = tmp_path / "bin", tmp_path / "slots"
    bin_dir.mkdir()
    _fake_cargo(bin_dir)
    holder_duration, bound = 8.0, 2.0

    holder, holder_start, _, holder_pid = _spawn_heavy(
        tmp_path=tmp_path, bin_dir=bin_dir, label="holder", duration_s=holder_duration, lock_dir=lock_dir, slots="1"
    )
    try:
        _wait_for(holder_start)  # the one slot is now held for the next 8s

        started = time.monotonic()
        waiter, waiter_start, _waiter_end, _ = _spawn_heavy(
            tmp_path=tmp_path,
            bin_dir=bin_dir,
            label="waiter",
            duration_s=0.1,
            lock_dir=lock_dir,
            slots="1",
            wait_bound=str(bound),
        )
        output, _ = waiter.communicate(timeout=HARNESS_BOUND_S)
        elapsed = time.monotonic() - started

        assert waiter.returncode != 0, "a wait that crosses its own bound must fail the step"
        assert elapsed < holder_duration - ORDER_SLACK_S, "the refusal must come close to its own bound, not the holder's"
        assert f"gave up waiting {bound:.0f}s" in output
        assert str(lock_dir) in output
        assert "STEG_CHECK_SLOT_WAIT_S" in output
        assert "STEG_CHECK_SLOTS" in output
        assert not waiter_start.exists(), "the step must never have run its own command once its wait timed out"
    finally:
        _kill_if_running(holder_pid)
        try:
            holder.wait(timeout=HARNESS_BOUND_S)
        except subprocess.TimeoutExpired:
            pass


def test_a_non_numeric_slot_count_fails_cleanly_not_with_a_traceback(tmp_path: Path) -> None:
    bin_dir, lock_dir = tmp_path / "bin", tmp_path / "slots"
    bin_dir.mkdir()
    _fake_cargo(bin_dir)
    command = [
        STEG_QUIET,
        "a",
        "--",
        "cargo",
        "test",
        "0.1",
        str(tmp_path / "s"),
        str(tmp_path / "e"),
        str(tmp_path / "p"),
    ]
    env = _env(bin_dir, STEG_CHECK_SLOTS="not-a-number", STEG_CHECK_SLOTS_DIR=str(lock_dir))

    result = subprocess.run(command, check=False, env=env, cwd=str(tmp_path), capture_output=True, text=True, timeout=HARNESS_BOUND_S)

    assert result.returncode != 0
    assert "Traceback" not in result.stdout
    assert "STEG_CHECK_SLOTS" in result.stdout
    assert "not-a-number" in result.stdout
