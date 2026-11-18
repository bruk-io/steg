---
id: TASK-1.132
title: 'Tools: only a few just check runs at a time on one machine'
status: In Progress
assignee:
  - '@bruk.w8'
created_date: '2026-11-18T02:09:00Z'
updated_date: '2026-11-18T03:39:00Z'
labels:
  - size-2
  - changes-requested
milestone: m-3
dependencies: []
references:
  - task-1.132-tools-only-a-few-just-check-runs-at-a
  - https://github.com/bruk-io/steg/pull/75
parent_task_id: TASK-1
project: bruk-io-steg
asdlc_accepted_from: DRAFT-3
asdlc_branch: task-1.132-tools-only-a-few-just-check-runs-at-a
asdlc_plan_approved_by: bruk
asdlc_plan_approved_hash: 'd9a4c4cb7361'
asdlc_plan_approved_size: size-2
asdlc_review_kind: same-machine
asdlc_review_findings: 'important=1,nit=3'
asdlc_review_policies: ["review"]
asdlc_plan_matched: yes
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Agreed with the engineer on 2026-11-17. Several workers run just check on this machine at once, each with its own target directory, and the timing-sensitive server tests then fail for no reason of their own: on 2026-11-17 the login_env grandchild test failed in 4 of 6 runs with two workers checking, and a stuck-pane test and a terminal-scrolling test failed once each under the same load. This is the ceiling on how many workers can usefully run. tools/src/steg_tools/quiet.py already wraps every step and is the natural place. TASK-1.123 bounds the waits; this task lowers the load that makes them matter.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The heavy steps of just check and just test take a machine-wide slot before they run; at most two run at once by default, and an environment variable sets the number
- [ ] #2 A run that is waiting says so in one line, naming what it waits for, and waits on the lock itself with no polling or timer
- [ ] #3 A run that crashes or is killed frees its slot without anyone cleaning up, because the operating system holds the lock
- [ ] #4 CI is unaffected; the tools' own tests cover two runs contending for one slot, a freed slot being taken, and the variable, each with a recorded failing control
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
Mechanism: N slot lock files under one directory; a non-blocking scan on the fast path, falling back to a thread race across all N slots - never a single committed slot - when every slot is busy.

1. WHAT TAKES A SLOT: each heavy step, not the whole run. `just check`/`just test`/`just test-all` run their recipe dependencies sequentially (just's default), so one worktree's own run already serialises its own heavy steps internally; the real machine-wide contention is across separate `just check` processes (different workers/worktrees) overlapping. Per-step locking therefore already caps "N heavy cargo processes machine-wide" exactly, while letting cheap steps (fmt check, boundaries, deny, foundations/design/architecture/recipes/ui-levels/ui-colours - all `uv run` Python tools) interleave freely across workers instead of queueing behind an idle run holding a whole-run slot. "Heavy" = detected by inspecting the command vector already passed to `steg-quiet <label> -- <command>`: `command[0] == "cargo" and command[1] in {"build", "test", "clippy"}`. This covers `lint` (clippy), `test`, `test-all`, every `cargo build -p`/`cargo test -p` call inside `check-packages`'s loop, and `build`; it excludes `cargo fmt --check` and `cargo deny check licenses` (metadata only). No justfile recipe bodies change: detection lives entirely in quiet.py by inspecting the command it is already given.

2. THE LOCK DIRECTORY: `lock_directory(env) -> Path`, pure function. `STEG_CHECK_SLOTS_DIR` overrides everything (also how tests point at tmp_path); otherwise `$XDG_CACHE_HOME/steg/check-slots` if set, else `~/.cache/steg/check-slots`. Per-user, per-machine, local disk by default, outside every worktree so all worktrees of this repo (and any other steg checkout on the same machine) share it. Never keyed off `$TMPDIR`: it can differ between sandboxed shells on the same machine, which would silently defeat the limiter while appearing to work. A stale directory or stale lock files are harmless and need no cleanup: flock state lives in the kernel against the open file description, not in the file's bytes or mtime.

3. THE WAIT, WITH NO POLLING, AND NO CONVOY ON ONE SLOT (revised from the first plan after review - the first version committed every overflow waiter to a single fixed slot, which under sustained load from several workers meant a backlog of waiters all queued on that one slot while another slot sat idle between steps; a waiter's delay was then bounded only by everyone ahead of it on that one slot, not by "one extra step"):
   - Fast path, no threads: one non-blocking pass over the N slot fds (each its own `os.open`) with `LOCK_EX | LOCK_NB`; first success wins, close the other N-1 fds, run holding that slot. This is the common case (most of the time at least one of N=2 is free) and costs nothing beyond N cheap syscalls.
   - Slow path, only entered when that pass finds every slot busy: print the wait line (see #5), then race all N slots at once instead of committing to one. Spawn one daemon thread per slot; each opens its *own* fresh fd to that slot's file and makes a single blocking `fcntl.flock(fd, LOCK_EX)` call - a real kernel wait, no polling. Whichever slot is freed first, the thread blocked on it wakes first. An in-process `threading.Lock` plus a `threading.Event` decide the winner: the first thread to acquire its flock sets itself as the winner and sets the event; the main thread calls `event.wait()` (itself a blocking, non-polling wait on a condition variable) and proceeds with the winning slot once set. Any thread that acquires its own flock *after* a winner is already set releases that lock and closes its fd immediately, under the same `threading.Lock`, so it never holds a slot hostage and can never starve another waiter - the slot it briefly, accidentally took is back for someone else at once. A waiter therefore gets whichever slot frees first, not a fixed one; its wait is bounded by the soonest of the currently-held slots to finish, not by the whole backlog on one slot.
   - No cross-process gate/mutex is needed: `flock`'s own atomicity is the only synchronisation the fast path needs, and the race above is only ever coordinated *within* one process's own threads (the `threading.Lock`), never across processes - removing the earlier plan's gate file entirely, one less moving part.
   - What happens to the threads still blocked when the step ends, or at process exit: every racer thread is daemon=True. When the winning slot's step finishes (or the whole `steg-quiet` process is killed - by its own 900s bound via `_kill_group` on the *child*, or by an external SIGKILL/SIGTERM on `steg-quiet` itself), the process exits; a daemon thread never keeps the interpreter alive, and OS process termination tears down every thread together, closing their fds and ending their blocked syscalls as part of that teardown - nothing is printed or left behind for a losing thread, and no lock is left held by one.
   - Overhead: threads are only spawned on the slow path (every slot already busy), so the common, uncontended case pays nothing beyond the fast-path scan.
   - Rejected alternatives, and why: a POSIX/`multiprocessing` semaphore gives one real "wait for any of N" call, but a SIGKILLed holder never posts back to it, permanently losing a slot - fails AC #3. A token FIFO (N bytes, blocking `read`) fails the same way, worse. A SysV semaphore with `SEM_UNDO` is the one primitive that gives both a single blocking "any of N" call and kernel-reversed release on a killed holder, but needs a ctypes/libc binding with its own macOS/Linux sysv-limits quirks; offered as a future call, not chosen for this size-2. Committing every overflow waiter to one fixed slot (the first version of this plan) is simpler than racing but was the exact problem raised on review - replaced by the thread race above.

4. WHO HOLDS THE LOCK, AND WHAT A KILL LEAVES BEHIND (AC #3): the winning slot's fd is opened and held by the `steg-quiet` *parent* Python process, never by the `cargo ...` child it spawns. `run_captured`/`run_live` already call `Popen(..., start_new_session=True)`, making the child its own new process group leader; the parent (holding the fd) is not a member of that group, so `_kill_group`'s `killpg` on the 900s timeout kills the child's whole group and leaves the parent alone to write its TIMED OUT report and exit normally, releasing the slot then. If instead the *parent* itself is killed (e.g. a worker's own command-timeout reaching the whole `just check` process tree), the kernel closes all its fds on process death, releasing the flock immediately. Either way, a killed run leaves nothing behind: no lock, pid, or state file ever records who held a slot, so there is nothing to clean up and nothing can be left half-released - the next acquirer (a fresh scan, or a racer thread already blocked on that fd) simply succeeds, identically to an orderly release. The lock must not be inherited by the `cargo` child: Python's PEP 446 makes every fd opened via `os.open`/`open` close-on-exec by default, and `subprocess.Popen`'s own default `close_fds=True` reinforces it - both already true with no extra flags, called out as intended, not accidental, since an inherited fd surviving in a cargo-spawned grandchild that outlives the killpg would hold the slot forever.

5. WHAT IS PRINTED: nothing extra on the fast path. Only on the slow path, one line, flushed before any blocking call, naming both what it waits for and whether it has been told a bound, so a worker reading only this line (if the rest of its output is cut off by an external timeout) can tell "still waiting" from "broken":
   - unbounded (default): `wait    <label> (N of N slots busy, STEG_CHECK_SLOTS=N) - waiting for a slot to free with no bound (STEG_CHECK_SLOT_WAIT_S is unset); this step's own STEG_QUIET_TIMEOUT_S bound has not started yet`
   - bounded (STEG_CHECK_SLOT_WAIT_S=120 say): `wait    <label> (N of N slots busy, STEG_CHECK_SLOTS=N) - waiting for a slot to free, told it may wait up to 120s (STEG_CHECK_SLOT_WAIT_S); this step's own STEG_QUIET_TIMEOUT_S bound has not started yet`
   Matches the existing column style (`ok      `, `FAILED  `, `TIMED OUT `).

6. DEFAULT AND ENV VAR: `effective_slot_count(env) -> int | None` (None = unlimited, taking no lock at all - not even opening the directory): `STEG_CHECK_SLOTS` explicit and non-zero -> that integer (reject negative with a clear ValueError); `STEG_CHECK_SLOTS=0` -> None (no limit, used verbatim everywhere, including CI if someone sets it there on purpose); unset and `CI` is set (GitHub Actions sets `CI=true` always) -> None (CI's own `concurrency` group already runs at most one `check` job per ref, so the limiter adds lock-directory assumptions on an ephemeral runner for zero benefit); unset and no `CI` -> default 2. A pure function over a plain env mapping, unit-tested with dicts, no doubles.

7. BOUNDED WAIT, AGAINST THE AGENTS' OWN FOREGROUND COMMAND TIMEOUT: `STEG_CHECK_SLOT_WAIT_S`, opt-in, unset by default (unbounded), as recommended and kept. With the thread race in place this is simple and needs no signal handling: the slow path's `event.wait()` already accepts a `timeout=` directly (`event.wait(timeout=float(STEG_CHECK_SLOT_WAIT_S))` when set, `event.wait()` otherwise) - a proper OS-level timed condition wait, not a sleep loop. If it returns without the event having been set, the step fails with a message naming the bound, the lock directory, and both env vars, rather than silently proceeding unslotted. As stated in #4, a run stopped this way (or killed any other way) leaves nothing behind. Left as the engineer's/orchestrator's call whether and when workers set it; this plan does not recommend backgrounding `just check` or any other workaround for a worker's own command timeout - that is handled at the orchestration layer, not in this task.

8. CTRL-C: a blocking `flock`, and `Event.wait()`, are both ordinary interruptible waits; SIGINT's existing Python-level handler raises `KeyboardInterrupt` through either like any other blocking call, unwinding `main()` normally. No special-case code needed; process exit releases whichever fd was held regardless.

9. FILES THAT CHANGE:
   - tools/src/steg_tools/quiet.py: pure functions `heavy_cargo_command(command)`, `effective_slot_count(env)`, `lock_directory(env)`; the `held_slot(lock_dir, count)` context manager (thin shell: real `os.open`/`fcntl.flock` I/O for the fast path, `threading`/`Event` for the slow-path race, parametrised so tests never touch the real default directory); the optional `STEG_CHECK_SLOT_WAIT_S` bound via `Event.wait(timeout=...)`; wiring in `main()` around the existing `run_captured`/`run_live` calls (acquisition stays outside their own timing, unchanged from the first plan); module docstring updated with the new behaviour and the two new env vars.
   - tools/tests/unit/test_quiet.py: new tests for the three pure functions - `heavy_cargo_command` true/false cases as before; `effective_slot_count` for unset->2, explicit N, `"0"`->None, `CI` set with/without an override, negative->raises; `lock_directory` for no override/no XDG, `XDG_CACHE_HOME` set, and `STEG_CHECK_SLOTS_DIR` override taking precedence over both.
   - tools/tests/functional/test_quiet_slots.py (new): real processes, real lock files, `STEG_CHECK_SLOTS_DIR` pointed at `tmp_path`, every wait in the test itself bounded (`subprocess.communicate(timeout=...)`):
     a. two heavy steps with `STEG_CHECK_SLOTS=1` serialise.
     b. the same with `STEG_CHECK_SLOTS=0` overlaps instead - the required failing control for (a), made permanent, also proving "no limit" behaves differently.
     c. a holder SIGKILLed mid-step (after it signals, via a file, that it has acquired) frees its slot promptly - a second process acquiring completes within a short bounded wait, proving AC #3.
     d. `STEG_CHECK_SLOTS=2` with three short heavy steps - two run immediately, the third waits for one of the first two to finish, by wall-clock tolerance.
     e. THE NEW TEST FOR THE CONVOY FIX: `STEG_CHECK_SLOTS=2`, slot A held for a long time (e.g. 2s), slot B held for a short time (e.g. 0.2s) then released; a third process arrives once both are busy. It must acquire shortly after slot B's release, not after slot A's much later one - proving the race picks whichever slot frees first rather than committing to a fixed one.
     f. `STEG_CHECK_SLOTS=0` with a heavy step - the lock directory is never created at all.
     g. a cheap (non-cargo-build/test/clippy) step runs immediately even while every heavy slot is held.
     Before the code exists, (a)-(g) are run and recorded failing (`asdlc record-check`), then re-recorded passing once implemented.
   - justfile: one added line in the file's own top-of-file comment block naming `STEG_CHECK_SLOTS` and `STEG_CHECK_SLOT_WAIT_S`.
   - No change to .github/workflows/ci.yml: CI is left alone by `effective_slot_count`'s own default, not by special-casing the workflow file.

10. ORDER OF WORK: (1) the three pure functions + their unit tests; (2) the fast-path scan in `held_slot`, wired into `main()` around `run_captured`/`run_live`; (3) the slow-path thread race (winner lock, event, late-acquirer self-release, daemon threads) added to `held_slot`; (4) the optional `STEG_CHECK_SLOT_WAIT_S` bound via `event.wait(timeout=...)`; (5) the functional tests (a)-(g), recorded failing then passing, (e) specifically proving the convoy fix; (6) the justfile comment line; (7) a sanity run of `just check` and `just test-tools` for real.

11. RISKS: macOS and Linux both give `flock(2)` the same per-open-file-description semantics this design relies on. Network-mounted home directories make `flock()` correctness unreliable across hosts - not solved here, named as a known limitation with `STEG_CHECK_SLOTS_DIR` as the escape hatch. A stale lock directory/files are harmless by design - no cleanup code. `check-packages`'s own loop takes a slot on each of its sequential cargo build/test calls, deliberately, one at a time, same as today just now gated machine-wide. `test-openpty-race-control` (not part of `just check`) is `cargo test ...`, so it is correctly treated as heavy and will take a slot when run by hand or recorded - harmless, noted so it does not read as a bug. `test-tools` itself runs `pytest`, never `cargo`, so it is never "heavy" and cannot contend with or deadlock against the slot machinery its own tests exercise. The thread race adds a small number of OS threads only on the slow path (every slot already busy); daemon=True means a losing thread is torn down with the process at exit, with nothing printed or left behind, and a late winner self-releases under the same in-process lock so it can never starve another waiter.

12. NOT SPLITTING: the limiter, its wiring in quiet.py, and its tests are one coherent change; the one-line justfile comment documenting the variables is not a second change.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Plan (d9a4c4cb7361) checked against the task's criteria by @bruk.o: ok. A slot per heavy step, held by the steg-quiet parent through an advisory file lock so a killed run frees it with no cleanup; a waiter takes whichever slot frees first (one blocked thread per slot, late acquirers release at once), with no polling; two slots by default, 0 for no limit, CI unlimited; the wait is unbounded unless STEG_CHECK_SLOT_WAIT_S is set. The first plan queued every waiter on slot 0; revised at the orchestrator's request. What a worker does when a queued check exceeds its own ten-minute command limit is the orchestrator's to handle in briefs.

Check passed: `just test-tools` exited 0 at 2026-11-18T02:32:09+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.132 on task-1.132-tools-only-a-few-just-check-runs-at-a at 87ced82aa6b7:

```text
ok      tool tests (15.1s)
```

Check FAILED: `sh -c 'STEG_CHECK_SLOTS=0 uv run --project tools pytest -q tools/tests/functional/test_quiet_slots.py::test_two_heavy_steps_with_one_slot_do_not_overlap tools/tests/functional/test_quiet_slots.py::test_the_variable_sets_how_many_run_at_once'` exited 1 at 2026-11-18T02:43:02+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.132 on task-1.132-tools-only-a-few-just-check-runs-at-a at 27b3b2defb21 with uncommitted changes, last 40 of 60 lines:

```text
        assert proc_b.communicate(timeout=10)[0] is not None
    
        first_start, first_end = _read_timestamp(start_a), _read_timestamp(end_a)
        second_start, second_end = _read_timestamp(start_b), _read_timestamp(end_b)
        if second_start < first_start:
            first_start, first_end, second_start, second_end = second_start, second_end, first_start, first_end
>       assert second_start >= first_end - SLACK_S, "the second run must not start until the first has finished"
E       AssertionError: the second run must not start until the first has finished
E       assert 1794969780.248864 >= (1794969780.85402 - 0.5)

tools/tests/functional/test_quiet_slots.py:132: AssertionError
_________________ test_the_variable_sets_how_many_run_at_once __________________

tmp_path = PosixPath('/private/var/folders/rf/f34xmczn4659gm98hrsprydc0000gn/T/pytest-of-brukhabtu/pytest-664/test_the_variable_sets_how_man0')

    def test_the_variable_sets_how_many_run_at_once(tmp_path: Path) -> None:
        bin_dir, lock_dir = tmp_path / "bin", tmp_path / "slots"
        bin_dir.mkdir()
        _fake_cargo(bin_dir)
        duration = 1.0
    
        procs = [
            _spawn_heavy(tmp_path=tmp_path, bin_dir=bin_dir, label=f"p{i}", duration_s=duration, lock_dir=lock_dir, slots="2") for i in range(3)
        ]
        for proc, _, _, _ in procs:
            assert proc.communicate(timeout=10)[0] is not None
    
        spans = sorted((_read_timestamp(start), _read_timestamp(end)) for _, start, end, _ in procs)
        first, second, third = spans
        assert second[0] - first[0] <= SLACK_S, "two of the three should start together - the limit is 2"
>       assert third[0] >= min(first[1], second[1]) - SLACK_S, "the third must wait for one of the first two to free a slot"
E       AssertionError: the third must wait for one of the first two to free a slot
E       assert 1794969780.952976 >= (1794969781.958216 - 0.5)
E        +  where 1794969781.958216 = min(1794969781.958217, 1794969781.958216)

tools/tests/functional/test_quiet_slots.py:204: AssertionError
=========================== short test summary info ============================
FAILED tools/tests/functional/test_quiet_slots.py::test_two_heavy_steps_with_one_slot_do_not_overlap
FAILED tools/tests/functional/test_quiet_slots.py::test_the_variable_sets_how_many_run_at_once
2 failed in 1.82s
```

Check passed: `sh -c 'STEG_CHECK_SLOTS=0 uv run --project tools pytest -q tools/tests/functional/test_quiet_slots.py::test_two_heavy_steps_with_one_slot_do_not_overlap tools/tests/functional/test_quiet_slots.py::test_the_variable_sets_how_many_run_at_once'` exited 0 at 2026-11-18T02:43:16+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.132 on task-1.132-tools-only-a-few-just-check-runs-at-a at 27b3b2defb21:

```text
warning: `VIRTUAL_ENV=/Users/brukhabtu/.cache/uv/builds-v0/.tmptVJ9ft` does not match the project environment path `tools/.venv` and will be ignored; use `--active` to target the active environment instead
..                                                                       [100%]
2 passed in 3.47s
```

Check FAILED: `sh -c 'STEG_CHECK_SLOTS=0 uv run --project tools pytest -q tools/tests/functional/test_quiet_slots.py::test_a_sigkilled_holder_frees_its_slot_at_once'` exited 1 at 2026-11-18T02:43:49+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.132 on task-1.132-tools-only-a-few-just-check-runs-at-a at 27b3b2defb21 with uncommitted changes:

```text
warning: `VIRTUAL_ENV=/Users/brukhabtu/.cache/uv/builds-v0/.tmpsN50Mm` does not match the project environment path `tools/.venv` and will be ignored; use `--active` to target the active environment instead
F                                                                        [100%]
=================================== FAILURES ===================================
________________ test_a_sigkilled_holder_frees_its_slot_at_once ________________

tmp_path = PosixPath('/private/var/folders/rf/f34xmczn4659gm98hrsprydc0000gn/T/pytest-of-brukhabtu/pytest-667/test_a_sigkilled_holder_frees_0')

    def test_a_sigkilled_holder_frees_its_slot_at_once(tmp_path: Path) -> None:
        bin_dir, lock_dir = tmp_path / "bin", tmp_path / "slots"
        bin_dir.mkdir()
        _fake_cargo(bin_dir)
    
        holder, holder_start, _, holder_pid = _spawn_heavy(
            tmp_path=tmp_path, bin_dir=bin_dir, label="holder", duration_s=4.0, lock_dir=lock_dir, slots="1"
        )
        waiter_pid = tmp_path / "waiter.pid"
        try:
            _wait_for(holder_start)  # the holder has acquired the one slot and is now sleeping
            os.kill(holder.pid, signal.SIGKILL)  # kill steg-quiet itself, not its fake-cargo child
    
            started = time.monotonic()
            waiter, waiter_start, waiter_end, waiter_pid = _spawn_heavy(
                tmp_path=tmp_path, bin_dir=bin_dir, label="waiter", duration_s=0.2, lock_dir=lock_dir, slots="1"
            )
            assert waiter.communicate(timeout=5)[0] is not None
            elapsed = time.monotonic() - started
>           assert elapsed < 2.0, "a slot held by a killed process must free at once, not after the 4s it was holding for"
E           AssertionError: a slot held by a killed process must free at once, not after the 4s it was holding for
E           assert 4.222133833973203 < 2.0

tools/tests/functional/test_quiet_slots.py:178: AssertionError
=========================== short test summary info ============================
FAILED tools/tests/functional/test_quiet_slots.py::test_a_sigkilled_holder_frees_its_slot_at_once
1 failed in 4.34s
```

Check passed: `sh -c 'STEG_CHECK_SLOTS=0 uv run --project tools pytest -q tools/tests/functional/test_quiet_slots.py::test_a_sigkilled_holder_frees_its_slot_at_once'` exited 0 at 2026-11-18T02:44:04+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.132 on task-1.132-tools-only-a-few-just-check-runs-at-a at 27b3b2defb21:

```text
warning: `VIRTUAL_ENV=/Users/brukhabtu/.cache/uv/builds-v0/.tmpylTdDZ` does not match the project environment path `tools/.venv` and will be ignored; use `--active` to target the active environment instead
.                                                                        [100%]
1 passed in 0.36s
```

Check FAILED: `sh -c 'STEG_CHECK_SLOTS=0 uv run --project tools pytest -q tools/tests/functional/test_quiet_slots.py::test_a_waiter_gets_whichever_slot_frees_first_not_a_fixed_one'` exited 1 at 2026-11-18T02:44:32+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.132 on task-1.132-tools-only-a-few-just-check-runs-at-a at 27b3b2defb21 with uncommitted changes, last 40 of 42 lines:

```text
=================================== FAILURES ===================================
________ test_a_waiter_gets_whichever_slot_frees_first_not_a_fixed_one _________

tmp_path = PosixPath('/private/var/folders/rf/f34xmczn4659gm98hrsprydc0000gn/T/pytest-of-brukhabtu/pytest-670/test_a_waiter_gets_whichever_s0')

    def test_a_waiter_gets_whichever_slot_frees_first_not_a_fixed_one(tmp_path: Path) -> None:
        """The fix for the convoy review found: a waiter must not be stuck behind one committed slot."""
        bin_dir, lock_dir = tmp_path / "bin", tmp_path / "slots"
        bin_dir.mkdir()
        _fake_cargo(bin_dir)
        long_duration, short_duration = 4.0, 0.3
    
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
        assert waiter.communicate(timeout=10)[0] is not None
        assert short_holder.communicate(timeout=10)[0] is not None
        assert long_holder.communicate(timeout=10)[0] is not None
    
        waiter_started_at = _read_timestamp(waiter_start)
        short_ended_at = _read_timestamp(short_end)
        long_ended_at = _read_timestamp(long_end)
>       assert waiter_started_at <= short_ended_at + SLACK_S * 2, "the waiter should acquire the slot that freed quickly"
E       AssertionError: the waiter should acquire the slot that freed quickly
E       assert 1794969872.30465 <= (1794969868.634326 + (0.5 * 2))

tools/tests/functional/test_quiet_slots.py:236: AssertionError
=========================== short test summary info ============================
FAILED tools/tests/functional/test_quiet_slots.py::test_a_waiter_gets_whichever_slot_frees_first_not_a_fixed_one
1 failed in 4.23s
```

Check passed: `sh -c 'STEG_CHECK_SLOTS=0 uv run --project tools pytest -q tools/tests/functional/test_quiet_slots.py::test_a_waiter_gets_whichever_slot_frees_first_not_a_fixed_one'` exited 0 at 2026-11-18T02:44:48+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.132 on task-1.132-tools-only-a-few-just-check-runs-at-a at 27b3b2defb21:

```text
warning: `VIRTUAL_ENV=/Users/brukhabtu/.cache/uv/builds-v0/.tmpDSSMx2` does not match the project environment path `tools/.venv` and will be ignored; use `--active` to target the active environment instead
.                                                                        [100%]
1 passed in 4.13s
```

Check FAILED: `sh -c 'STEG_CHECK_SLOTS=0 uv run --project tools pytest -q tools/tests/functional/test_quiet_slots.py::test_no_limit_never_creates_the_lock_directory'` exited 1 at 2026-11-18T02:45:11+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.132 on task-1.132-tools-only-a-few-just-check-runs-at-a at 27b3b2defb21 with uncommitted changes:

```text
warning: `VIRTUAL_ENV=/Users/brukhabtu/.cache/uv/builds-v0/.tmpOViqc2` does not match the project environment path `tools/.venv` and will be ignored; use `--active` to target the active environment instead
F                                                                        [100%]
=================================== FAILURES ===================================
________________ test_no_limit_never_creates_the_lock_directory ________________

tmp_path = PosixPath('/private/var/folders/rf/f34xmczn4659gm98hrsprydc0000gn/T/pytest-of-brukhabtu/pytest-673/test_no_limit_never_creates_th0')

    def test_no_limit_never_creates_the_lock_directory(tmp_path: Path) -> None:
        bin_dir, lock_dir = tmp_path / "bin", tmp_path / "slots"
        bin_dir.mkdir()
        _fake_cargo(bin_dir)
    
        proc, _, end_file, _ = _spawn_heavy(tmp_path=tmp_path, bin_dir=bin_dir, label="a", duration_s=0.1, lock_dir=lock_dir, slots="0")
        assert proc.communicate(timeout=5)[0] is not None
        assert end_file.exists()
>       assert not lock_dir.exists()
E       AssertionError: assert not True
E        +  where True = exists()
E        +    where exists = PosixPath('/private/var/folders/rf/f34xmczn4659gm98hrsprydc0000gn/T/pytest-of-brukhabtu/pytest-673/test_no_limit_never_creates_th0/slots').exists

tools/tests/functional/test_quiet_slots.py:248: AssertionError
=========================== short test summary info ============================
FAILED tools/tests/functional/test_quiet_slots.py::test_no_limit_never_creates_the_lock_directory
1 failed in 0.21s
```

Check passed: `sh -c 'STEG_CHECK_SLOTS=0 uv run --project tools pytest -q tools/tests/functional/test_quiet_slots.py::test_no_limit_never_creates_the_lock_directory'` exited 0 at 2026-11-18T02:45:21+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.132 on task-1.132-tools-only-a-few-just-check-runs-at-a at 27b3b2defb21:

```text
warning: `VIRTUAL_ENV=/Users/brukhabtu/.cache/uv/builds-v0/.tmpnt4yga` does not match the project environment path `tools/.venv` and will be ignored; use `--active` to target the active environment instead
.                                                                        [100%]
1 passed in 0.20s
```

Check FAILED: `sh -c 'STEG_CHECK_SLOTS=0 uv run --project tools pytest -q tools/tests/functional/test_quiet_slots.py::test_a_cheap_step_is_never_slowed_by_a_full_set_of_heavy_slots'` exited 1 at 2026-11-18T02:45:39+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.132 on task-1.132-tools-only-a-few-just-check-runs-at-a at 27b3b2defb21 with uncommitted changes:

```text
warning: `VIRTUAL_ENV=/Users/brukhabtu/.cache/uv/builds-v0/.tmp4UZSVA` does not match the project environment path `tools/.venv` and will be ignored; use `--active` to target the active environment instead
F                                                                        [100%]
=================================== FAILURES ===================================
________ test_a_cheap_step_is_never_slowed_by_a_full_set_of_heavy_slots ________

tmp_path = PosixPath('/private/var/folders/rf/f34xmczn4659gm98hrsprydc0000gn/T/pytest-of-brukhabtu/pytest-676/test_a_cheap_step_is_never_slo0')

    def test_a_cheap_step_is_never_slowed_by_a_full_set_of_heavy_slots(tmp_path: Path) -> None:
        bin_dir, lock_dir = tmp_path / "bin", tmp_path / "slots"
        bin_dir.mkdir()
        _fake_cargo(bin_dir)
    
        holder, holder_start, _, holder_pid = _spawn_heavy(
            tmp_path=tmp_path, bin_dir=bin_dir, label="holder", duration_s=3.0, lock_dir=lock_dir, slots="1"
        )
        try:
            _wait_for(holder_start)  # the single slot is now held for the next 3s
    
>           assert not heavy_cargo_command([sys.executable, "-c", "print('ok')"])
E           assert not True
E            +  where True = heavy_cargo_command(['/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.132/tools/.venv/bin/python', '-c', "print('ok')"])

tools/tests/functional/test_quiet_slots.py:262: AssertionError
=========================== short test summary info ============================
FAILED tools/tests/functional/test_quiet_slots.py::test_a_cheap_step_is_never_slowed_by_a_full_set_of_heavy_slots
1 failed in 0.12s
```

Check passed: `sh -c 'STEG_CHECK_SLOTS=0 uv run --project tools pytest -q tools/tests/functional/test_quiet_slots.py::test_a_cheap_step_is_never_slowed_by_a_full_set_of_heavy_slots'` exited 0 at 2026-11-18T02:45:48+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.132 on task-1.132-tools-only-a-few-just-check-runs-at-a at 27b3b2defb21:

```text
warning: `VIRTUAL_ENV=/Users/brukhabtu/.cache/uv/builds-v0/.tmpMH0CMk` does not match the project environment path `tools/.venv` and will be ignored; use `--active` to target the active environment instead
.                                                                        [100%]
1 passed in 0.15s
```

Check FAILED: `sh -c 'STEG_CHECK_SLOTS=0 uv run --project tools pytest -q tools/tests/unit/test_quiet.py -k heavy'` exited 1 at 2026-11-18T02:46:43+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.132 on task-1.132-tools-only-a-few-just-check-runs-at-a at b3337f0f0605 with uncommitted changes:

```text
warning: `VIRTUAL_ENV=/Users/brukhabtu/.cache/uv/builds-v0/.tmpJBUzxu` does not match the project environment path `tools/.venv` and will be ignored; use `--active` to target the active environment instead
F.                                                                       [100%]
=================================== FAILURES ===================================
__________________ test_cargo_build_test_and_clippy_are_heavy __________________

    def test_cargo_build_test_and_clippy_are_heavy() -> None:
>       assert heavy_cargo_command(["cargo", "build", "-p", "steg-sdk"])
E       AssertionError: assert False
E        +  where False = heavy_cargo_command(['cargo', 'build', '-p', 'steg-sdk'])

tools/tests/unit/test_quiet.py:65: AssertionError
=========================== short test summary info ============================
FAILED tools/tests/unit/test_quiet.py::test_cargo_build_test_and_clippy_are_heavy
1 failed, 1 passed, 19 deselected in 0.01s
```

Check passed: `sh -c 'STEG_CHECK_SLOTS=0 uv run --project tools pytest -q tools/tests/unit/test_quiet.py -k heavy'` exited 0 at 2026-11-18T02:46:53+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.132 on task-1.132-tools-only-a-few-just-check-runs-at-a at b3337f0f0605:

```text
warning: `VIRTUAL_ENV=/Users/brukhabtu/.cache/uv/builds-v0/.tmpXZpQBB` does not match the project environment path `tools/.venv` and will be ignored; use `--active` to target the active environment instead
..                                                                       [100%]
2 passed, 19 deselected in 0.01s
```

Check FAILED: `sh -c 'STEG_CHECK_SLOTS=0 uv run --project tools pytest -q tools/tests/unit/test_quiet.py -k slot_count'` exited 1 at 2026-11-18T02:47:07+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.132 on task-1.132-tools-only-a-few-just-check-runs-at-a at b3337f0f0605 with uncommitted changes:

```text
warning: `VIRTUAL_ENV=/Users/brukhabtu/.cache/uv/builds-v0/.tmpn1BPZ2` does not match the project environment path `tools/.venv` and will be ignored; use `--active` to target the active environment instead
....F.                                                                   [100%]
=================================== FAILURES ===================================
_________________ test_slot_count_defaults_to_unlimited_in_ci __________________

    def test_slot_count_defaults_to_unlimited_in_ci() -> None:
>       assert effective_slot_count({"CI": "true"}) is None
E       AssertionError: assert 2 is None
E        +  where 2 = effective_slot_count({'CI': 'true'})

tools/tests/unit/test_quiet.py:96: AssertionError
=========================== short test summary info ============================
FAILED tools/tests/unit/test_quiet.py::test_slot_count_defaults_to_unlimited_in_ci
1 failed, 5 passed, 15 deselected in 0.01s
```

Check passed: `sh -c 'STEG_CHECK_SLOTS=0 uv run --project tools pytest -q tools/tests/unit/test_quiet.py -k slot_count'` exited 0 at 2026-11-18T02:47:17+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.132 on task-1.132-tools-only-a-few-just-check-runs-at-a at b3337f0f0605:

```text
warning: `VIRTUAL_ENV=/Users/brukhabtu/.cache/uv/builds-v0/.tmpPO4ub5` does not match the project environment path `tools/.venv` and will be ignored; use `--active` to target the active environment instead
......                                                                   [100%]
6 passed, 15 deselected in 0.01s
```

Check FAILED: `sh -c 'STEG_CHECK_SLOTS=0 uv run --project tools pytest -q tools/tests/unit/test_quiet.py -k lock_directory'` exited 1 at 2026-11-18T02:47:31+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.132 on task-1.132-tools-only-a-few-just-check-runs-at-a at b3337f0f0605 with uncommitted changes:

```text
warning: `VIRTUAL_ENV=/Users/brukhabtu/.cache/uv/builds-v0/.tmpV5P5lw` does not match the project environment path `tools/.venv` and will be ignored; use `--active` to target the active environment instead
..F                                                                      [100%]
=================================== FAILURES ===================================
______________ test_lock_directory_override_wins_over_everything _______________

    def test_lock_directory_override_wins_over_everything() -> None:
>       assert lock_directory({"XDG_CACHE_HOME": "/tmp/cache", "STEG_CHECK_SLOTS_DIR": "/tmp/slots"}) == Path("/tmp/slots")
E       AssertionError: assert PosixPath('/tmp/cache/steg/check-slots') == PosixPath('/tmp/slots')
E        +  where PosixPath('/tmp/cache/steg/check-slots') = lock_directory({'XDG_CACHE_HOME': '/tmp/cache', 'STEG_CHECK_SLOTS_DIR': '/tmp/slots'})
E        +  and   PosixPath('/tmp/slots') = Path('/tmp/slots')

tools/tests/unit/test_quiet.py:112: AssertionError
=========================== short test summary info ============================
FAILED tools/tests/unit/test_quiet.py::test_lock_directory_override_wins_over_everything
1 failed, 2 passed, 18 deselected in 0.01s
```

Check passed: `sh -c 'STEG_CHECK_SLOTS=0 uv run --project tools pytest -q tools/tests/unit/test_quiet.py -k lock_directory'` exited 0 at 2026-11-18T02:47:42+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.132 on task-1.132-tools-only-a-few-just-check-runs-at-a at b3337f0f0605:

```text
warning: `VIRTUAL_ENV=/Users/brukhabtu/.cache/uv/builds-v0/.tmpIjJJh5` does not match the project environment path `tools/.venv` and will be ignored; use `--active` to target the active environment instead
...                                                                      [100%]
3 passed, 18 deselected in 0.01s
```

Check FAILED: `sh -c 'STEG_CHECK_SLOTS=0 uv run --project tools pytest -q tools/tests/unit/test_quiet.py -k wait_bound'` exited 1 at 2026-11-18T02:47:57+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.132 on task-1.132-tools-only-a-few-just-check-runs-at-a at b3337f0f0605 with uncommitted changes:

```text
warning: `VIRTUAL_ENV=/Users/brukhabtu/.cache/uv/builds-v0/.tmpDasgVW` does not match the project environment path `tools/.venv` and will be ignored; use `--active` to target the active environment instead
.F                                                                       [100%]
=================================== FAILURES ===================================
_____________________ test_wait_bound_honours_the_variable _____________________

    def test_wait_bound_honours_the_variable() -> None:
>       assert wait_bound_seconds({"STEG_CHECK_SLOT_WAIT_S": "45"}) == 45.0
E       AssertionError: assert None == 45.0
E        +  where None = wait_bound_seconds({'STEG_CHECK_SLOT_WAIT_S': '45'})

tools/tests/unit/test_quiet.py:132: AssertionError
=========================== short test summary info ============================
FAILED tools/tests/unit/test_quiet.py::test_wait_bound_honours_the_variable
1 failed, 1 passed, 19 deselected in 0.01s
```

Check passed: `sh -c 'STEG_CHECK_SLOTS=0 uv run --project tools pytest -q tools/tests/unit/test_quiet.py -k wait_bound'` exited 0 at 2026-11-18T02:48:06+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.132 on task-1.132-tools-only-a-few-just-check-runs-at-a at b3337f0f0605:

```text
warning: `VIRTUAL_ENV=/Users/brukhabtu/.cache/uv/builds-v0/.tmpuw8M55` does not match the project environment path `tools/.venv` and will be ignored; use `--active` to target the active environment instead
..                                                                       [100%]
2 passed, 19 deselected in 0.01s
```

Check FAILED: `sh -c 'STEG_CHECK_SLOTS=0 uv run --project tools pytest -q tools/tests/unit/test_quiet.py -k wait_line'` exited 1 at 2026-11-18T02:48:20+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.132 on task-1.132-tools-only-a-few-just-check-runs-at-a at b3337f0f0605 with uncommitted changes:

```text
warning: `VIRTUAL_ENV=/Users/brukhabtu/.cache/uv/builds-v0/.tmpamCIbH` does not match the project environment path `tools/.venv` and will be ignored; use `--active` to target the active environment instead
FF                                                                       [100%]
=================================== FAILURES ===================================
________ test_the_wait_line_names_the_count_and_has_no_bound_by_default ________

    def test_the_wait_line_names_the_count_and_has_no_bound_by_default() -> None:
        line = wait_line("tests (steg-server)", 2, None)
        assert line.startswith("wait    tests (steg-server) (2 of 2 slots busy, STEG_CHECK_SLOTS=2)")
>       assert "STEG_CHECK_SLOT_WAIT_S is unset" in line
E       assert 'STEG_CHECK_SLOT_WAIT_S is unset' in "wait    tests (steg-server) (2 of 2 slots busy, STEG_CHECK_SLOTS=2) - waiting for a slot to free; this step's own STEG_QUIET_TIMEOUT_S bound has not started yet"

tools/tests/unit/test_quiet.py:118: AssertionError
______________ test_the_wait_line_names_the_bound_when_one_is_set ______________

    def test_the_wait_line_names_the_bound_when_one_is_set() -> None:
        line = wait_line("tests (steg-server)", 2, 120.0)
>       assert "told it may wait up to 120s (STEG_CHECK_SLOT_WAIT_S)" in line
E       assert 'told it may wait up to 120s (STEG_CHECK_SLOT_WAIT_S)' in "wait    tests (steg-server) (2 of 2 slots busy, STEG_CHECK_SLOTS=2) - waiting for a slot to free; this step's own STEG_QUIET_TIMEOUT_S bound has not started yet"

tools/tests/unit/test_quiet.py:124: AssertionError
=========================== short test summary info ============================
FAILED tools/tests/unit/test_quiet.py::test_the_wait_line_names_the_count_and_has_no_bound_by_default
FAILED tools/tests/unit/test_quiet.py::test_the_wait_line_names_the_bound_when_one_is_set
2 failed, 19 deselected in 0.01s
```

Check passed: `sh -c 'STEG_CHECK_SLOTS=0 uv run --project tools pytest -q tools/tests/unit/test_quiet.py -k wait_line'` exited 0 at 2026-11-18T02:48:30+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.132 on task-1.132-tools-only-a-few-just-check-runs-at-a at b3337f0f0605:

```text
warning: `VIRTUAL_ENV=/Users/brukhabtu/.cache/uv/builds-v0/.tmp6LjShv` does not match the project environment path `tools/.venv` and will be ignored; use `--active` to target the active environment instead
..                                                                       [100%]
2 passed, 19 deselected in 0.01s
```

Check passed: `just check` exited 0 at 2026-11-18T02:58:47+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.132 on task-1.132-tools-only-a-few-just-check-runs-at-a at b3337f0f0605:

```text
ok      format check (0.3s)
ok      lint (85.1s)
ok      boundaries (0.1s)
ok      foundations (0.2s)
ok      foundations public surface (0.1s)
ok      design pages (0.1s)
ok      licences (0.7s)
ok      tests (every crate) (104.9s)
ok      build (steg-sdk) (3.4s)
ok      test (steg-sdk) (2.9s)
ok      test (steg-sdk, local-socket) (2.7s)
ok      build (steg-kernel) (42.3s)
ok      test (steg-kernel) (1.4s)
ok      build (steg-server) (41.3s)
ok      test (steg-server) (55.1s)
ok      test (steg-server, race-fixture) (55.5s)
ok      test (steg-server, pty-race-fixture) (59.8s)
ok      test (steg-server, accept-backoff-fixture) (60.3s)
ok      build (steg) (2.3s)
ok      test (steg) (2.1s)
ok      test (steg, fixtures) (6.7s)
ok      build (steg-ui) (15.0s)
ok      test (steg-ui) (1.2s)
ok      build (steg-app) (1.4s)
ok      test (steg-app) (2.0s)
ok      tool tests (23.1s)
ok      architecture (0.7s)
ok      recipe names (0.1s)
ok      ui levels (0.1s)
ok      ui colours (0.1s)
```

Check passed: `just test-tools` exited 0 at 2026-11-18T02:59:39+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.132 on task-1.132-tools-only-a-few-just-check-runs-at-a at 700bc6f420b7:

```text
ok      tool tests (22.4s)
```

Built per the approved, engineer-tightened plan. The limiter: each heavy cargo step (build/test/clippy) takes one of STEG_CHECK_SLOTS machine-wide slots (default 2; 0 = no limit, no lock touched at all; unset defaults to no limit under CI). A waiter races all slots at once (one daemon thread per slot, each a single blocking flock, no polling) so it gets whichever frees first, never a fixed one. A crashed or killed holder frees its slot with no cleanup code - the kernel drops the flock when the holder's own fd closes, since the slot is held only by the steg-quiet parent, never inherited into the cargo child. STEG_CHECK_SLOT_WAIT_S optionally bounds the wait, unset by default.

Checks recorded on the task: an initial just test-tools baseline; the implementation's own unit/functional tests passing; ten mechanism-removed-then-restored control pairs (20 record-check calls, detail in the task's notes) covering lock acquisition, the SIGKILL release property, the first-to-free race, the no-limit path, heavy-step detection, and each of the five pure functions; a full just check recorded live against the real default lock directory while five other workers were building. CI on PR #75 failed once on an unrelated flaky server test (signal::tests::send_reaches_the_whole_process_group_not_only_the_leader, not in the named known-flakes list, not touched by this change) and passed clean on rerun - noted on the task, not investigated further.

No departures from the approved plan.

Check FAILED: `sh -c 'STEG_CHECK_SLOTS=0 uv run --project tools pytest -q tools/tests/functional/test_quiet_slots.py::test_a_bounded_wait_fails_cleanly_without_running_its_command'` exited 1 at 2026-11-18T03:36:55+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.132 on task-1.132-tools-only-a-few-just-check-runs-at-a at 94dcbe45a851 with uncommitted changes, last 40 of 42 lines:

```text
=================================== FAILURES ===================================
________ test_a_bounded_wait_fails_cleanly_without_running_its_command _________

tmp_path = PosixPath('/private/var/folders/rf/f34xmczn4659gm98hrsprydc0000gn/T/pytest-of-brukhabtu/pytest-692/test_a_bounded_wait_fails_clea0')

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
    
>           assert waiter.returncode != 0, "a wait that crosses its own bound must fail the step"
E           AssertionError: a wait that crosses its own bound must fail the step
E           assert 0 != 0
E            +  where 0 = <Popen: returncode: 0 args: ['/Users/brukhabtu/src/github.com/bruk-io/steg.w...>.returncode

tools/tests/functional/test_quiet_slots.py:336: AssertionError
=========================== short test summary info ============================
FAILED tools/tests/functional/test_quiet_slots.py::test_a_bounded_wait_fails_cleanly_without_running_its_command
1 failed in 8.25s
```

Commits since the review: 1, all with the Asdlc-Worker trailer.

Check passed: `sh -c 'STEG_CHECK_SLOTS=0 uv run --project tools pytest -q tools/tests/functional/test_quiet_slots.py::test_a_bounded_wait_fails_cleanly_without_running_its_command'` exited 0 at 2026-11-18T03:37:11+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.132 on task-1.132-tools-only-a-few-just-check-runs-at-a at 94dcbe45a851:

```text
warning: `VIRTUAL_ENV=/Users/brukhabtu/.cache/uv/builds-v0/.tmpC7u3kh` does not match the project environment path `tools/.venv` and will be ignored; use `--active` to target the active environment instead
.                                                                        [100%]
1 passed in 2.16s
```

Commits since the review: 1, all with the Asdlc-Worker trailer.

Check FAILED: `sh -c 'STEG_CHECK_SLOTS=0 uv run --project tools pytest -q tools/tests/functional/test_quiet_slots.py::test_a_waiter_gets_whichever_slot_frees_first_not_a_fixed_one'` exited 1 at 2026-11-18T03:37:44+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.132 on task-1.132-tools-only-a-few-just-check-runs-at-a at 94dcbe45a851 with uncommitted changes:

```text
warning: `VIRTUAL_ENV=/Users/brukhabtu/.cache/uv/builds-v0/.tmpQcXx7a` does not match the project environment path `tools/.venv` and will be ignored; use `--active` to target the active environment instead
F                                                                        [100%]
=================================== FAILURES ===================================
________ test_a_waiter_gets_whichever_slot_frees_first_not_a_fixed_one _________

tmp_path = PosixPath('/private/var/folders/rf/f34xmczn4659gm98hrsprydc0000gn/T/pytest-of-brukhabtu/pytest-695/test_a_waiter_gets_whichever_s0')

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
>       assert "wait    waiter" in waiter_output
E       AssertionError: assert 'wait    waiter' in 'ok      waiter (0.1s)\n'

tools/tests/functional/test_quiet_slots.py:257: AssertionError
=========================== short test summary info ============================
FAILED tools/tests/functional/test_quiet_slots.py::test_a_waiter_gets_whichever_slot_frees_first_not_a_fixed_one
1 failed in 8.14s
```

Commits since the review: 1, all with the Asdlc-Worker trailer.

Check passed: `sh -c 'STEG_CHECK_SLOTS=0 uv run --project tools pytest -q tools/tests/functional/test_quiet_slots.py::test_a_waiter_gets_whichever_slot_frees_first_not_a_fixed_one'` exited 0 at 2026-11-18T03:38:02+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.132 on task-1.132-tools-only-a-few-just-check-runs-at-a at 94dcbe45a851:

```text
warning: `VIRTUAL_ENV=/Users/brukhabtu/.cache/uv/builds-v0/.tmpjhdOYG` does not match the project environment path `tools/.venv` and will be ignored; use `--active` to target the active environment instead
.                                                                        [100%]
1 passed in 8.12s
```

Commits since the review: 1, all with the Asdlc-Worker trailer.

Check FAILED: `sh -c 'STEG_CHECK_SLOTS=0 uv run --project tools pytest -q "tools/tests/unit/test_quiet.py::test_slot_count_rejects_a_non_numeric_value_naming_the_variable_and_value" "tools/tests/functional/test_quiet_slots.py::test_a_non_numeric_slot_count_fails_cleanly_not_with_a_traceback"'` exited 1 at 2026-11-18T03:38:30+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.132 on task-1.132-tools-only-a-few-just-check-runs-at-a at 94dcbe45a851 with uncommitted changes, last 40 of 47 lines:

```text
             ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
E       AssertionError: Regex pattern did not match.
E         Expected regex: "STEG_CHECK_SLOTS must be a whole number, got 'abc'"
E         Actual message: "invalid literal for int() with base 10: 'abc'"

tools/tests/unit/test_quiet.py:96: AssertionError
_______ test_a_non_numeric_slot_count_fails_cleanly_not_with_a_traceback _______

tmp_path = PosixPath('/private/var/folders/rf/f34xmczn4659gm98hrsprydc0000gn/T/pytest-of-brukhabtu/pytest-698/test_a_non_numeric_slot_count_0')

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
>       assert "STEG_CHECK_SLOTS" in result.stdout
E       assert 'STEG_CHECK_SLOTS' in "FAILED  a (invalid literal for int() with base 10: 'not-a-number')\n"
E        +  where "FAILED  a (invalid literal for int() with base 10: 'not-a-number')\n" = CompletedProcess(args=['/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.132/tools/.venv/bin/steg-quiet',...ot_count_0/p'], returncode=1, stdout="FAILED  a (invalid literal for int() with base 10: 'not-a-number')\n", stderr='').stdout

tools/tests/functional/test_quiet_slots.py:372: AssertionError
=========================== short test summary info ============================
FAILED tools/tests/unit/test_quiet.py::test_slot_count_rejects_a_non_numeric_value_naming_the_variable_and_value
FAILED tools/tests/functional/test_quiet_slots.py::test_a_non_numeric_slot_count_fails_cleanly_not_with_a_traceback
2 failed in 0.04s
```

Commits since the review: 1, all with the Asdlc-Worker trailer.

Check passed: `sh -c 'STEG_CHECK_SLOTS=0 uv run --project tools pytest -q "tools/tests/unit/test_quiet.py::test_slot_count_rejects_a_non_numeric_value_naming_the_variable_and_value" "tools/tests/functional/test_quiet_slots.py::test_a_non_numeric_slot_count_fails_cleanly_not_with_a_traceback"'` exited 0 at 2026-11-18T03:38:40+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.132 on task-1.132-tools-only-a-few-just-check-runs-at-a at 94dcbe45a851:

```text
warning: `VIRTUAL_ENV=/Users/brukhabtu/.cache/uv/builds-v0/.tmpqXH9Cl` does not match the project environment path `tools/.venv` and will be ignored; use `--active` to target the active environment instead
..                                                                       [100%]
2 passed in 0.03s
```

Commits since the review: 1, all with the Asdlc-Worker trailer.

Check FAILED: `sh -c 'STEG_CHECK_SLOTS=0 uv run --project tools pytest -q "tools/tests/unit/test_quiet.py::test_wait_bound_rejects_a_non_numeric_value_naming_the_variable_and_value"'` exited 1 at 2026-11-18T03:39:02+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.132 on task-1.132-tools-only-a-few-just-check-runs-at-a at 94dcbe45a851 with uncommitted changes:

```text
warning: `VIRTUAL_ENV=/Users/brukhabtu/.cache/uv/builds-v0/.tmphh0Dfw` does not match the project environment path `tools/.venv` and will be ignored; use `--active` to target the active environment instead
F                                                                        [100%]
=================================== FAILURES ===================================
__ test_wait_bound_rejects_a_non_numeric_value_naming_the_variable_and_value ___

    def test_wait_bound_rejects_a_non_numeric_value_naming_the_variable_and_value() -> None:
>       with pytest.raises(ValueError, match=r"STEG_CHECK_SLOT_WAIT_S must be a number, got 'soon'"):
             ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
E       AssertionError: Regex pattern did not match.
E         Expected regex: "STEG_CHECK_SLOT_WAIT_S must be a number, got 'soon'"
E         Actual message: "could not convert string to float: 'soon'"

tools/tests/unit/test_quiet.py:141: AssertionError
=========================== short test summary info ============================
FAILED tools/tests/unit/test_quiet.py::test_wait_bound_rejects_a_non_numeric_value_naming_the_variable_and_value
1 failed in 0.01s
```

Commits since the review: 1, all with the Asdlc-Worker trailer.

Check passed: `sh -c 'STEG_CHECK_SLOTS=0 uv run --project tools pytest -q "tools/tests/unit/test_quiet.py::test_wait_bound_rejects_a_non_numeric_value_naming_the_variable_and_value"'` exited 0 at 2026-11-18T03:39:12+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.132 on task-1.132-tools-only-a-few-just-check-runs-at-a at 94dcbe45a851:

```text
warning: `VIRTUAL_ENV=/Users/brukhabtu/.cache/uv/builds-v0/.tmpuJxgTb` does not match the project environment path `tools/.venv` and will be ignored; use `--active` to target the active environment instead
.                                                                        [100%]
1 passed in 0.01s
```

Commits since the review: 1, all with the Asdlc-Worker trailer.
<!-- SECTION:NOTES:END -->

## Comments

<!-- COMMENTS:BEGIN -->
author: @bruk
created: 2026-11-18T02:31:00Z

---

Plan approved by @bruk (d9a4c4cb7361, size-2): its worker may build it.

---

created: 2026-11-18T03:00:00Z

---

Built per the approved plan (per the engineer's own tightening of the plan's proof wording, reported earlier). Summary for the record:

WHAT A WORKER SEES WHEN ITS CHECK WAITS: nothing extra on the fast path (at least one of STEG_CHECK_SLOTS slots free - the common case). Only when every slot is busy, one line, flushed before the real wait: "wait    <label> (N of N slots busy, STEG_CHECK_SLOTS=N) - waiting for a slot to free with no bound (STEG_CHECK_SLOT_WAIT_S is unset); this step's own STEG_QUIET_TIMEOUT_S bound has not started yet" (or, with the bound set, "told it may wait up to Xs (STEG_CHECK_SLOT_WAIT_S)" in place of "with no bound..."). That line is the only thing distinguishing "still waiting" from "broken" if a worker's own harness timeout cuts its output short mid-wait.

THE TWO VARIABLES: STEG_CHECK_SLOTS sets how many heavy cargo steps (build/test/clippy) may run machine-wide at once - default 2, 0 means no limit and takes no lock at all, unset defaults to no limit under CI automatically (GitHub Actions sets CI=true; its own concurrency group already runs one check job per ref). STEG_CHECK_SLOT_WAIT_S optionally bounds how long a waiter will wait before failing the step outright with a clear message - unset by default (unbounded), so a worker's own command timeout is the only thing that currently cuts a long wait short, with no special message from this mechanism. Both documented in the justfile's own header comment and in quiet.py's module docstring.

WHAT THIS DOES NOT FIX: CPU/disk contention from non-cargo work on the machine (another process entirely, a browser, an editor's own indexer) is invisible to this limiter - it only ever counts cargo build/test/clippy invocations against each other, so a machine busy with unrelated load can still make the timing-sensitive tests flaky even with every slot "free" by this mechanism's own count. It coordinates only processes on one machine sharing the same lock directory - a second machine, or a sandboxed environment with its own distinct $HOME/XDG_CACHE_HOME, is invisible to it and not meant to be (TASK-1.132's own scope is one machine). And it assumes a local, POSIX-flock-honouring filesystem under the lock directory; a network-mounted home directory would make flock's own guarantees unreliable across hosts, which is named as a known limitation in the plan with STEG_CHECK_SLOTS_DIR as the escape hatch to point at local disk if that's ever actually hit - nothing here detects or warns about that case automatically.

Checks recorded: an initial just test-tools baseline before any change; the full implementation's own just test-tools and the new functional/unit tests passing; ten mechanism-removed-then-restored control pairs (20 record-check calls) covering lock acquisition (tests a, d), the SIGKILL release property (test c, via a scratch edit that made the slot's fd inheritable into the fake-cargo child - showing the slot is NOT freed without the deliberate close-on-exec/close_fds=True this task relies on), the first-to-free race (test e, via a scratch fallback to a fixed slot - showing the waiter gets stuck behind the long holder instead of the quick one), the no-limit path (test f), heavy-step detection (test g), and each of the five pure functions (heavy_cargo_command, effective_slot_count, lock_directory, wait_bound_seconds, wait_line) with their own unit tests. Tree was clean between every pair and at the end (git status, and ps checked for leftover processes after the SIGKILL and cheap-step tests, which deliberately kill or race real subprocesses). Finally, a full just check recorded with the limiter live for the first time, against the real default lock directory, while five other workers were building on this machine - completed clean in under 8 minutes with no wait needed that particular run (the other workers' heavy steps didn't happen to collide with mine at the moments that mattered, but the mechanism was live throughout).

No departures from the approved plan.

---

created: 2026-11-18T03:19:00Z

---

CI sighting (not mine): the first CI run on PR #75 failed at test (steg-server) with signal::tests::send_reaches_the_whole_process_group_not_only_the_leader - "the backgrounded child, in the same process group, must be gone too." Not one of the three named known flakes (login_env grandchild, stuck-pane timing, terminal-scrolling), but the same flavour (process-group/signal timing). Unrelated to this task's own change (tools/ and the justfile comment only; nothing in crates/server touched). Reran the job once per the standing guidance; it passed clean on the rerun (CI itself runs on an isolated runner with no slot contention, since CI defaults to no limit). Noting the sighting for the record; not investigating further as it is not this task's own code.

---

author: @bruk.r2
created: 2026-11-18T03:20:00Z

---

@bruk.r2 is reviewing TASK-1.132: a same-machine review: it waited 0m with no other engineer's reviewer, longer than review_fallback_after (0m), so a fresh reviewer on its author's machine takes it.

---

author: @bruk.r2
created: 2026-11-18T03:28:00Z

---

Review by @bruk.r2 (same-machine): changes requested. Back to In Progress for the author's worker.

Findings: 1 Important, 3 nit. Cites: review. The diff matches the plan.

[Evidence, Important] tools/tests/functional/test_quiet_slots.py:56-83 (plan step 7; quiet.py:180-184,373-375): `_spawn_heavy`'s `wait_bound` parameter is defined and wired into the subprocess env (lines 64,69-70) but no call site in the file ever passes it - grep confirms zero `wait_bound=` arguments across every `_spawn_heavy(` call. So the refusal branch the plan names explicitly ("the step fails ... rather than silently proceeding unslotted" when STEG_CHECK_SLOT_WAIT_S expires, i.e. `_race_all_slots`'s TimeoutError reaching `main`'s `except TimeoutError: print("FAILED ...")`) is never exercised by any test, and none of the notes' ten recorded control pairs covers it either - they cover `wait_bound_seconds` the parser, not the timeout-and-fail behaviour it feeds. A plan-named branch with no test reaching it is unmet per the Evidence pass even though the ordinary (unbounded) wait path is well covered.

[Bugs, Nit] justfile:212: `cargo install cargo-deny --locked` runs through `steg-quiet` but `install` is not in `_HEAVY_CARGO_SUBCOMMANDS`, so this from-source compile never takes a slot. Low impact (idempotent, guarded by `command -v`, not part of `just check`/`just test`), but it is a cargo-through-steg-quiet invocation the detection misses, worth naming.

[Evidence, Nit] tools/tests/functional/test_quiet_slots.py: no functional test captures stdout and asserts the wait line is actually printed when `held_slot` takes the slow path; only the pure `wait_line()` function is unit-tested (test_quiet.py:115-124). Tests (d) and (e) already drive the slow path and already capture output, so asserting the line appears there is nearly free and would close the gap between "the formatter is right" and "AC #2's one line is actually emitted."

[Bugs, Nit] quiet.py:88,117: an empty or non-numeric STEG_CHECK_SLOTS or STEG_CHECK_SLOT_WAIT_S raises a raw, unformatted traceback (bare `int()`/`float()`), not the clear one-line failure the rest of the file gives elsewhere. It does fail loudly rather than silently run unlimited, so not Important, but inconsistent with the file's own style.

Verified myself: read quiet.py, the justfile's full recipe list (every `cargo build/test/clippy` invocation in check/test/test-all/check-packages/lint/build/capture-gallery/test-openpty-race-control correctly classified heavy; cargo fmt --check and cargo deny check licenses correctly not), both test files in full, and the task's full plan/notes/checks history. Traced fd lifetimes for the fast path, the slow-path race, SIGKILL, the 900s step timeout, and KeyboardInterrupt - all release correctly; the scan fds and the racer fds are distinct open file descriptions on the same files, so no self-deadlock. The ten recorded control pairs are genuine mechanism-removed-then-restored controls (e.g. test c's control made the slot fd inheritable into the fake-cargo child to show close-on-exec is load-bearing), not just pre-implementation baselines.

Control repeated myself (different from a pure function): scratch-edited `_race_all_slots` in the worktree to spawn only one thread, pinned to `paths[0]`, breaking the first-to-free race. Ran `uv run --project tools pytest -q tools/tests/functional/test_quiet_slots.py::test_a_waiter_gets_whichever_slot_frees_first_not_a_fixed_one` directly (bounded, under 60s) - it failed exactly as expected, the waiter stuck behind the long holder instead of the short one ("the waiter should acquire the slot that freed quickly", assert 1794972279.37 <= 1794972276.69). Reverted with `git checkout --`, confirmed `git status --porcelain` empty and the diff against origin/main unchanged (still only justfile, quiet.py, the two test files).

Did not check: did not independently stress-test the timing-margin assertions (SLACK_S=0.5s) for flakiness under this machine's current load beyond the one control above; did not re-run the SIGKILL or fd-inheritance controls myself, only read their recorded transcripts.

---
<!-- COMMENTS:END -->
