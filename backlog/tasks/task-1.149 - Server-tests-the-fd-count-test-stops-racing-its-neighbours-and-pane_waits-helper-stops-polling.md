---
id: TASK-1.149
title: >-
  Server tests: the fd-count test stops racing its neighbours, and pane_wait's
  helper stops polling
status: In Progress
assignee:
  - '@bruk.w2'
created_date: '2026-11-22T04:12:00Z'
updated_date: '2026-11-22T20:04:00Z'
labels:
  - size-2
milestone: m-3
dependencies: []
parent_task_id: TASK-1
project: bruk-io-steg
asdlc_accepted_from: DRAFT-1
asdlc_branch: task-1.149-server-tests-the-fd-count-test-stops
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Split out of TASK-1.137 by its plan (22 November 2026). Two sighted problems that are not missing bounds. (1) shutdown.rs: every wait in the fd-count test is already bounded; it fails because it counts the process's /dev/fd entries while the file's six other tests run concurrently. TASK-1.137's plan proposes the FLOOD_TEST_LOCK pattern from attach.rs, taken by all seven tests; the alternative is to change what the test measures. (2) pane_wait.rs: the helper polls with a 20 ms sleep (review nit on TASK-1.140); the file's own pane.wait for exit replaces it with no extra protocol setup, and cannot hang on a pane that has already ended. Touches only those two test files.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 crates/server/tests/shutdown.rs many_short_connections_do_not_grow_the_tracked_set no longer counts file descriptors while the file's other tests run: a recorded control shows it failing when the serialisation is removed and a neighbour holds descriptors open
- [ ] #2 crates/server/tests/pane_wait.rs wait_for_program_status waits on the pane's exit through pane wait, with no sleep-and-poll loop left in the file
- [ ] #3 just check passes
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
## Files that change
- `crates/server/tests/shutdown.rs`: add `use std::sync::Mutex;` (alongside the existing `mpsc`), a new file-scoped `static SHUTDOWN_TESTS_LOCK: Mutex<()> = Mutex::new(());` next to `open_fd_count`, and, as the first statement of each of the file's seven `#[test]` fns, `let _test_lock = SHUTDOWN_TESTS_LOCK.lock().unwrap_or_else(std::sync::PoisonError::into_inner);` - first, not merely present, so Rust's reverse-declaration-order `Drop` keeps the lock held until every socket, server thread and `ScopedTempDir` that test created has already been torn down. No change to what `many_short_connections_do_not_grow_the_tracked_set` measures or to any assertion.
- `crates/server/tests/pane_wait.rs`: rename `wait_for_program_status(stream, connection, pane, expected: &str)` to `wait_for_exit(stream, connection, pane)`, dropping `expected` (both call sites only ever pass `"ended"`); its body stops polling `pane.show` and instead sends one `pane.wait` call (`wait_params(pane, Some(20))`, the same helper and bound `pane_environment.rs`'s own `pane.wait` wait already uses) and asserts the reply is `Response::Ok`. Update both call sites (currently `wait_for_program_status(&mut stream, &mut connection, &pane, "ended");`) to `wait_for_exit(&mut stream, &mut connection, &pane);`. No other function in the file changes; `read_reply_for`'s own deadline loop is a blocking read with a timeout, not a sleep-and-poll, and `start_server`'s own 5ms accept-poll (lines 69-76) is untouched - see the open question below.

## Order of work
1. `pane_wait.rs`: do the rename and rewrite first - it is the smaller, more mechanical change and gives an early signal. Verified already (read `register_wait` in `crates/server/src/runtime/wait.rs:242`): its already-ended branch (`if let Some(ProgramStatus::Ended { exit_code }) = program_status_of(...)`) only reads state and replies; it does not consume or mutate anything, so calling `pane.wait` twice on an already-ended pane (once inside the new helper, once again by the test's own explicit call right after) answers `Ok` both times. Build and run narrowly (`env ZIG=/Users/brukhabtu/.local/share/mise/installs/zig/0.17.1/bin/zig cargo test -p steg-server --test pane_wait`) before touching `shutdown.rs`.
2. `shutdown.rs`: add the `Mutex` import, the static, and the first-statement guard to all seven tests in one pass (missing it on even one leaves that test still racing the other six). Run narrowly (`env ZIG=... cargo test -p steg-server --test shutdown`) at the harness's default parallelism - not `--test-threads=1`, which would hide the very contention this task removes.
3. Run both negative controls (below), each by hand, reverted after, at the final head (after the last commit).
4. `just check`.

## Risks
- **No cross-process guard needed, and none is added.** `/dev/fd` is this process's own descriptor table; `many_short_connections_do_not_grow_the_tracked_set` only ever raced its six *in-process* siblings (cargo's own thread-per-test parallelism within one binary), never the other worktree or the other three `steg-server` feature-variant binaries `just check` runs alongside it - those are separate processes with separate `/dev/fd` tables. Copying `attach.rs`'s `flood_lock_path`/`cross_process_flood_guard` here would guard against a collision that cannot happen and is deliberately left out.
- **The two attach.rs contention sightings in this task's notes are out of scope and should split**, not fold in:
  - `an_unresponsive_panes_own_commands_do_not_stall_the_connection_or_another_pane` (the "unresponsive-pane" sighting) already takes *both* `FLOOD_TEST_LOCK` (in-process) and `cross_process_flood_guard` (cross-worktree) - confirmed at `attach.rs:1499-1502`. It cannot be an instance of "missing the FLOOD_TEST_LOCK pattern"; it already has it and still flaked once under heavy machine load. Its cause is a different, unestablished mechanism (resource starvation under load, or a bound that needs widening) - the task's own notes say exactly that: "cause not established."
  - `a_declared_size_reaches_the_pty_and_is_reported_on_every_attached_stream` (the pane-size sighting) takes *neither* lock (confirmed: no `_flood_guard` appears before its own `#[test]` at `attach.rs:1043`), so unlike the test above it was never guarded at all - a different starting point again.
  - Two different tests, two different starting points (one fully guarded, one not), neither traced to a missing lock: this is not "the FLOOD_TEST_LOCK pattern, taken by one more test." Recommend a new investigation task against `attach.rs`, milestone `m-3`, scoped to establishing the actual cause before picking a fix - default: file it now as a follow-up, cause-finding only, no fix assumed.
- **Serialising all seven `shutdown.rs` tests removes their intra-file parallelism.** All seven are already individually fast (bounded waits, no multi-second sleeps outside the deliberately-slow trickle test), so the wall-clock cost of running them one at a time in this one file is small; accepted, not mitigated further.
- **The negative controls' temporary code (the barriers, the scratch neighbour test, the deleted call, the lock removal) must not reach the committed diff** - the same discipline `pane_wait.rs`'s own existing negative-control comments already describe ("not committed as a toggle"; "reverted before this test was committed"). Each control step below says exactly what is added and that it is reverted before the next step.
- **Before any failing run is recorded**, it is run once, unrecorded, to confirm it prints nothing from this machine's real environment (the fd assertion prints two integers; the `pane_wait` failure prints only this test's scoped socket path and an error code) - per this team's safety rule.

## Open question for the engineer
AC #2 reads "no sleep-and-poll loop left in the file," and `pane_wait.rs::start_server` (lines 69-76) keeps a 5ms accept-poll after this change - it is not touched because TASK-1.140's own review explicitly excluded it ("verbatim precedent already used in pane_capture.rs/pane_send.rs... not a departure this diff introduces") and TASK-1.137's plan excluded it again. Recommended default: leave it and treat the AC as met by the `wait_for_program_status`/`wait_for_exit` rewrite specifically, per that precedent - but flag this now so the engineer can overrule before approval rather than after review.

## Proof
Mechanism 1 - `pane.wait` replaces polling (AC #2), a control traced to the mechanism rather than to a compile error:
- Unrecorded first, then recorded failing: temporarily delete the `wait_for_exit(&mut stream, &mut connection, &pane);` call from `timeout_zero_on_an_already_ended_pane_answers_ok_with_the_real_exit_code` only (the `pane_wait.rs:388` call site, *not* the other one - see why below). Without that precondition wait, the pane is still mid-`sh -c "exit 5"` when `pane.wait --timeout 0` is sent, so `register_wait`'s `Some(0)` branch answers `TimedOut`, not `Ok` with exit code 5: `env ZIG=/Users/brukhabtu/.local/share/mise/installs/zig/0.17.1/bin/zig cargo test -p steg-server --test pane_wait timeout_zero_on_an_already_ended_pane_answers_ok_with_the_real_exit_code` fails on that assertion.
  Why not the other call site (`wait_on_an_already_ended_pane_with_no_timeout_answers_at_once_with_the_real_exit_code`): deleting its precondition wait leaves a `pane.wait` with `None`, which just registers and still answers `Ok(7)` once the program later exits on its own - that test would likely still pass, proving nothing. Stated here so a reviewer does not ask for it separately.
- Restore the call; recorded passing: `asdlc record-check TASK-1.149 --in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.149 --as bruk.w2 -- env ZIG=/Users/brukhabtu/.local/share/mise/installs/zig/0.17.1/bin/zig cargo test -p steg-server --test pane_wait`.
- AC #2's "no sleep-and-poll loop left in the file" (for `wait_for_exit` itself) checked by inspection too: `wait_for_exit`'s body contains no `thread::sleep`.

Mechanism 2 - the lock serialises `shutdown.rs`'s seven tests (AC #1), a control built on mutual exclusion and two synchronisation barriers rather than any sleep or timing assumption:
- At the final head (lock in place, after the last commit), temporarily add one extra `#[test] fn scratch_task_1_149_neighbour()` to `shutdown.rs` that opens 40 throwaway file descriptors (`std::fs::File::open("/dev/null")`, comfortably over the assertion's own `+ 10` slack).
- **Failing run**: in both `many_short_connections_do_not_grow_the_tracked_set` and the scratch neighbour, temporarily remove the `SHUTDOWN_TESTS_LOCK` guard, and add two shared 2-party barriers, `static BARRIER_OPEN`/`static BARRIER_CLOSE: std::sync::OnceLock<std::sync::Barrier>`. Neighbour: open the 40 files, wait at `BARRIER_OPEN`, wait at `BARRIER_CLOSE`, then drop them. fd-count test: capture `before`, wait at `BARRIER_OPEN`, run its 200-round connect loop plus the trigger connection, capture `after`, wait at `BARRIER_CLOSE`. This guarantees the neighbour's 40 descriptors are open for the fd-count test's entire measurement window by construction, not by a timed sleep that a slow machine could outlast. Unrecorded first to confirm it prints only the two integers, then recorded failing: `env ZIG=/Users/brukhabtu/.local/share/mise/installs/zig/0.17.1/bin/zig cargo test -p steg-server --test shutdown`.
- **Passing run**: restore `SHUTDOWN_TESTS_LOCK` in both the real test and the scratch neighbour, and delete both barriers entirely - with both tests taking the same mutex, a barrier wait between them could never complete (one side holds the lock the other needs to even reach its own wait), which is exactly the mutual exclusion this fix relies on, not an oversight. Record, running the whole file so the other five real tests are concurrent too (only adding descriptor pressure in the failing run, and fully serialised in this one): `asdlc record-check TASK-1.149 --in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.149 --as bruk.w2 -- env ZIG=/Users/brukhabtu/.local/share/mise/installs/zig/0.17.1/bin/zig cargo test -p steg-server --test shutdown`.
- Delete `scratch_task_1_149_neighbour`, `BARRIER_OPEN` and `BARRIER_CLOSE` before the final commit; none of the three appear in it.

AC #3:
- `asdlc record-check TASK-1.149 --in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.149 --as bruk.w2 -- just check`, recorded passing at the final head.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Sighting for whoever plans this, 22 November 2026 (not in this task's scope unless the plan takes it): crates/server/tests/attach.rs a_declared_size_reaches_the_pty_and_is_reported_on_every_attached_stream failed once in a local just check on TASK-1.150's branch while two other builds ran on the machine; it passed alone and in the next full run. Reported by @bruk.w5 as contention, cause not established.

Second contention sighting in crates/server/tests/attach.rs, 22 November 2026: its unresponsive-pane test failed once in a local just check on TASK-1.146's branch while other builds ran; it passed alone and on a full re-run. Reported by @bruk.w2; cause not established. With the pane-size test above, that is two different attach.rs tests failing under load in one day.

Plan checked against the task's criteria by @bruk.o (22 Nov 2026): ok to go to the engineer. Lock taken first in all seven shutdown.rs tests; no cross-process guard, since the fd count is per process; the helper becomes one pane.wait for exit. The two attach.rs sightings are split out as a draft (cause-finding, no fix assumed). For the build: the passing half of the shutdown control must be a run that completes and passes within a bound, not an argument that it cannot overlap. Open for the engineer: criterion 2 says no sleep-and-poll loop is left in pane_wait.rs, and start_server's 5 ms accept-poll stays (the same shape in pane_capture.rs and pane_send.rs, excluded by TASK-1.140's review and TASK-1.137's plan); default is to leave it and say so.
<!-- SECTION:NOTES:END -->
