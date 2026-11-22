---
id: TASK-1.149
title: >-
  Server tests: the fd-count test stops racing its neighbours, and pane_wait's
  helper stops polling
status: In Progress
assignee:
  - '@bruk.w2'
created_date: '2026-11-22T04:12:00Z'
updated_date: '2026-11-22T20:01:00Z'
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
- `crates/server/tests/pane_wait.rs`: rename `wait_for_program_status(stream, connection, pane, expected: &str)` to `wait_for_exit(stream, connection, pane)`, dropping `expected` (both call sites only ever pass `"ended"`); its body stops polling `pane.show` and instead sends one `pane.wait` call (`wait_params(pane, Some(20))`, the same helper and bound `pane_environment.rs`'s own `pane.wait` wait already uses) and asserts the reply is `Response::Ok`. Update both call sites (currently `wait_for_program_status(&mut stream, &mut connection, &pane, "ended");`, at the two tests that call it) to `wait_for_exit(&mut stream, &mut connection, &pane);`. No other function in the file changes; `read_reply_for`'s own deadline loop is a blocking read with a timeout, not a sleep-and-poll, and is unaffected.

## Order of work
1. `pane_wait.rs`: do the rename and rewrite first - it is the smaller, more mechanical change and gives an early signal. Verified already (read `register_wait` in `crates/server/src/runtime/wait.rs:242`): its already-ended branch (`if let Some(ProgramStatus::Ended { exit_code }) = program_status_of(...)`) only reads state and replies; it does not consume or mutate anything, so calling `pane.wait` twice on an already-ended pane (once inside the new helper, once again by the test's own explicit call right after) answers `Ok` both times. Build and run narrowly (`env ZIG=/Users/brukhabtu/.local/share/mise/installs/zig/0.17.1/bin/zig cargo test -p steg-server --test pane_wait`) before touching `shutdown.rs`.
2. `shutdown.rs`: add the `Mutex` import, the static, and the first-statement guard to all seven tests in one pass (missing it on even one leaves that test still racing the other six). Run narrowly (`env ZIG=... cargo test -p steg-server --test shutdown`) at the harness's default parallelism - not `--test-threads=1`, which would hide the very contention this task removes.
3. Run both negative controls (below), each by hand, reverted after, at the final head (after the last commit).
4. `just check`.

## Risks
- **No cross-process guard needed, and none is added.** `/dev/fd` is this process's own descriptor table; `many_short_connections_do_not_grow_the_tracked_set` only ever raced its six *in-process* siblings (cargo's own thread-per-test parallelism within one binary), never the other worktree or the other three `steg-server` feature-variant binaries `just check` runs alongside it - those are separate processes with separate `/dev/fd` tables. Copying `attach.rs`'s `flood_lock_path`/`cross_process_flood_guard` here would guard against a collision that cannot happen and is deliberately left out.
- **The two attach.rs contention sightings in this task's notes are out of scope and should split**, not fold in:
  - `an_unresponsive_panes_own_commands_do_not_stall_the_connection_or_another_pane` (the "unresponsive-pane" sighting) already takes *both* `FLOOD_TEST_LOCK` (in-process) and `cross_process_flood_guard` (cross-worktree) - confirmed at `attach.rs:1499-1502`. It cannot be an instance of "missing the FLOOD_TEST_LOCK pattern"; it already has it and still flaked once under heavy machine load. Its cause is a different, unestablished mechanism (resource starvation under load, or a bound that needs widening) - the asdlc notes say exactly that: "cause not established."
  - `a_declared_size_reaches_the_pty_and_is_reported_on_every_attached_stream` (the pane-size sighting) takes *neither* lock (confirmed: no `_flood_guard` appears before its own `#[test]` at `attach.rs:1043`), so unlike the test above it was never guarded at all - a different starting point again.
  - Two different tests, two different starting points (one fully guarded, one not), neither traced to a missing lock: this is not "the FLOOD_TEST_LOCK pattern, taken by one more test." Recommend a new investigation task against `attach.rs`, scoped to establishing the actual cause before picking a fix - default: file it now as a follow-up intent/task, milestone `m-3`, cause-finding only (no fix assumed).
- **Serialising all seven `shutdown.rs` tests removes their intra-file parallelism.** All seven are already individually fast (bounded waits, no multi-second sleeps outside the deliberately-slow trickle test), so the wall-clock cost of running them one at a time in this one file is small; accepted, not mitigated further.
- **The negative controls' temporary code (the barrier, the scratch neighbour test, the lock removal) must not reach the committed diff** - the same discipline `pane_wait.rs`'s own existing negative-control comments already describe ("not committed as a toggle"; "reverted before this test was committed"). Each control's steps below say exactly what is added and that it is reverted before the next step.
- **Before any failing run is recorded**, it is run once, unrecorded, to confirm it prints nothing from this machine's real environment (the fd assertion prints two integers; the `pane_wait` tests' own failure messages print only this test's scoped socket paths) - per this team's safety rule.

## Proof
Mechanism 1 - `pane.wait` replaces polling (AC #2):
- Unrecorded first, then recorded failing: with `wait_for_exit`'s body reverted to the old poll-with-sleep shape removed entirely (stub it to `panic!("removed for control")` or similar, so the two call sites cannot silently pass some other way), `env ZIG=/Users/brukhabtu/.local/share/mise/installs/zig/0.17.1/bin/zig cargo test -p steg-server --test pane_wait wait_on_an_already_ended_pane_with_no_timeout_answers_at_once_with_the_real_exit_code` fails to compile/run.
- Restore the real body; recorded passing: `asdlc record-check TASK-1.149 --in <worktree> --as bruk.w2 -- env ZIG=/Users/brukhabtu/.local/share/mise/installs/zig/0.17.1/bin/zig cargo test -p steg-server --test pane_wait`.
- This is a compile-time/behavioural control rather than a race, so a single recorded pass is the honest proof here; AC #2's own wording ("no sleep-and-poll loop left in the file") is additionally checked by inspection - `grep -n "thread::sleep" crates/server/tests/pane_wait.rs` after the change should show nothing inside `wait_for_exit` (the file's other, unrelated `start_server` 5ms accept-poll is pre-existing and out of this task's scope, same as TASK-1.140's own review nit already noted).

Mechanism 2 - the lock serialises `shutdown.rs`'s seven tests (AC #1), a deterministic control built on mutual exclusion itself rather than timing luck:
- At the final head (lock in place, after the last commit), temporarily add one extra `#[test] fn scratch_task_1_149_neighbour()` to `shutdown.rs` that opens 40 throwaway file descriptors (`std::fs::File::open("/dev/null")`, comfortably over the assertion's own `+ 10` slack) and holds them for a fixed `std::thread::sleep(Duration::from_secs(5))` before dropping them.
- **Failing run**: in both `many_short_connections_do_not_grow_the_tracked_set` and the new scratch neighbour, temporarily remove the `SHUTDOWN_TESTS_LOCK` guard, and add a shared `static CONTROL_BARRIER: std::sync::OnceLock<std::sync::Barrier> = std::sync::OnceLock::new();` (2 parties) that both wait on - the fd-count test right after capturing `before`, the neighbour right before opening its 40 files - so the neighbour's descriptors are deterministically open throughout the fd-count test's whole connect loop, not merely likely to overlap. Unrecorded first to confirm it prints only the two integers, then recorded failing: `env ZIG=/Users/brukhabtu/.local/share/mise/installs/zig/0.17.1/bin/zig cargo test -p steg-server --test shutdown many_short_connections_do_not_grow_the_tracked_set scratch_task_1_149_neighbour`.
- **Passing run**: restore `SHUTDOWN_TESTS_LOCK` in both the real test and the scratch neighbour, and delete the barrier entirely - with both tests taking the same mutex, a barrier wait between them could never complete (one side holds the lock the other needs to even reach its own wait), which is exactly the mutual exclusion this fix relies on, not an oversight. Record: `asdlc record-check TASK-1.149 --in <worktree> --as bruk.w2 -- env ZIG=/Users/brukhabtu/.local/share/mise/installs/zig/0.17.1/bin/zig cargo test -p steg-server --test shutdown many_short_connections_do_not_grow_the_tracked_set scratch_task_1_149_neighbour`.
- Delete `scratch_task_1_149_neighbour` and `CONTROL_BARRIER` before the final commit; they appear in neither.

AC #3:
- `asdlc record-check TASK-1.149 --in <worktree> --as bruk.w2 -- just check` (run from the worktree's own justfile/working directory), recorded passing at the final head.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Sighting for whoever plans this, 22 November 2026 (not in this task's scope unless the plan takes it): crates/server/tests/attach.rs a_declared_size_reaches_the_pty_and_is_reported_on_every_attached_stream failed once in a local just check on TASK-1.150's branch while two other builds ran on the machine; it passed alone and in the next full run. Reported by @bruk.w5 as contention, cause not established.

Second contention sighting in crates/server/tests/attach.rs, 22 November 2026: its unresponsive-pane test failed once in a local just check on TASK-1.146's branch while other builds ran; it passed alone and on a full re-run. Reported by @bruk.w2; cause not established. With the pane-size test above, that is two different attach.rs tests failing under load in one day.
<!-- SECTION:NOTES:END -->
