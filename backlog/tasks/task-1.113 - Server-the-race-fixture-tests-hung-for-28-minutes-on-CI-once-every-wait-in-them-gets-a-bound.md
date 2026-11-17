---
id: TASK-1.113
title: >-
  Server: the race-fixture tests hung for 28 minutes on CI once; every wait in
  them gets a bound
status: In Progress
assignee:
  - '@bruk.w9'
created_date: '2026-11-16T05:42:00Z'
updated_date: '2026-11-17T01:27:00Z'
labels:
  - size-2
milestone: m-3
dependencies: []
parent_task_id: TASK-1
project: bruk-io-steg
asdlc_accepted_from: DRAFT-2
asdlc_branch: task-1.113-server-the-race-fixture-tests-hung-for
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
On 2026-11-16 pull request 55's check run 43802834909 (attempt 1, commit 51c1659, a change to crates/ui and tools only) printed 'ok test (steg-server) (12.3s)' at 04:16:46 UTC and nothing more until the job's 30-minute limit cancelled it at 04:44:21. The next step is the steg-server tests with the race-fixture feature (crates/server/tests/race.rs: several real processes racing for the arbiter's socket). Attempt 2 on the same commit passed that step in 10.6s. No earlier occurrence in the last 100 runs. A test that can wait for ever breaks the team's rule that a wait is bounded, costs 30 CI minutes when it happens, and left a worker idle for 80 minutes behind it. It may share a cause with the flakes recorded on TASK-1.82 (the arbiter's stale-socket test; the sdk's local socket reporting the address in use). Rests on doc-8 (arbitration, R1 to R3) and the team's testing rules.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The step that hung is identified from the evidence, or the notes say plainly that it could not be reproduced and what was tried
- [ ] #2 Every wait in the tests run under the race-fixture feature (on a child process, a socket, a thread or a file) has a bound, and a test that passes its bound fails with a message naming what it waited for and kills and reaps the processes it started
- [ ] #3 A test proves a bound works: a fixture process made to hang fails its test within the bound and leaves no process behind, with a recorded negative control
- [ ] #4 just check cannot spend its whole CI time limit on one hung test: the suite fails within minutes when a test hangs
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
## Findings (settle AC #1 and the orchestrator's question #1)

Read `crates/server/src/outbound.rs:267-298` (`run_writer`'s wait loop) and `:253-256`
(`close`): every mutation of `Queues.closed`/the queues happens under the same `Mutex`
the waiter re-checks before calling `Condvar::wait`, and `wait` is only ever called
while already holding that lock - a `close()`/`enqueue_*` landing between the check and
the wait cannot be missed. Read `crates/server/src/connection.rs:274-322`
(`ConnectionCleanup::drop`): `outbound.close()` runs on every path out of
`run_post_handshake` - normal return, early return, or a panic unwinding through a
request handler - because it is a local's `Drop`, which Rust runs on unwind too. So no
production path ever abandons a connection without calling `close()`: there is no
"writer thread that never ends" in the product. This is a test bug, not a product bug.

The sighted stack (TASK-1.113's notes) named
`outbound::tests::a_broken_write_closes_the_queue_so_the_readers_next_enqueue_fails`
blocked in `Condvar::wait`. Read as written, that test's own thread can only reach
`wait` if `write_all` on a `UnixStream` whose peer was just `drop`ped *succeeds* instead
of failing (`outbound.rs:477-490`) - probed 800,000/800,000 failures on this machine,
serial and under 16-way concurrent load (scratch probe, not committed), so that exact
mechanism could not be forced to reproduce here. A demonstrably real, timing-independent
mechanism exists two tests over: `control_is_drained_before_any_data_...` (:342-360),
`data_streams_are_drained_round_robin_...` (:363-387), `a_stream_with_nothing_queued_...`
(:390-407), and `a_genuine_flood_on_a_data_stream_...` (:581-637) each spawn a writer
thread and call `outbound.close()`/`writer.join()` only *after* assertions that can
panic - one of them (`a_genuine_flood_...`, line 621) has a hard 500ms latency
assertion, exactly the kind of thing heavy CI/local load (already the suspected cause of
other flakes on TASK-1.82) can trip. If any of those assertions panics, the spawned
writer thread leaks, permanently parked in `Condvar::wait` with nothing left to ever
notify it - the identical symptom, reproducible by construction, no OS timing needed.
Both candidates get the same fix below; I am not picking one as "the" cause since the
fix (bound the wait, guarantee cleanup on every exit path) covers both regardless of
which actually fired on CI.

AC #2's literal scope ("every wait in the tests run under the race-fixture feature") is
larger than size-2: `cargo test -p steg-server --features race-fixture` (the step that
hung, `justfile:135`) runs the *whole* crate - every `#[cfg(test)]` unit test plus every
file in `crates/server/tests/`, not only `race.rs`. The sweep (grep for `.join()`,
`.recv()`, `.wait()`, `read_exact`/`read_line`) found unbounded waits across
`tests/{shutdown,idle_stop,program,events,routing,handshake,runtime,real_pty}.rs`,
`crates/steg/tests/{idle_stop,server_start,login_environment}.rs`, and `#[cfg(test)]`
blocks in `src/{arbiter,bind,connection,dispatch/*,env,idle,listen,login_env,pane,paths,
program_param,pty,runtime,signal}.rs` - roughly 60 sites, most never reported hanging.
Per the standing preference to split: this task fixes the file that actually hung
(`outbound.rs`), the two files the task's own title and TASK-1.82's notes name
(`race.rs`, `openpty_race.rs`), and ships a bound that is independent of the Rust-level
sweep's completeness (below). A follow-up task takes the rest, reusing the shapes built
here. Flagging this for the engineer to rule on rather than narrowing AC #2 myself.

## Files that change

- `crates/server/src/lib.rs`: add `#[cfg(test)] mod test_support;`.
- `crates/server/src/test_support.rs` (new): `run_bounded(what, bound, body)` - runs
  `body` on a thread, `mpsc::Receiver::recv_timeout(bound)` on the result, panics naming
  `what` and `bound` on timeout; `ClosingWriterGuard` - owns the `Arc<Outbound>` and the
  writer's `JoinHandle`, and on `Drop` calls `outbound.close()` then joins with a bound,
  logging (not panicking - this can run mid-unwind) if the join itself ever times out.
  Mirrors `connection.rs:274-322`'s own "a local's `Drop` cleans up every exit path"
  idiom, moved into test code for the same reason.
- `crates/server/src/outbound.rs`: the four spawn-then-assert-then-cleanup tests use
  `ClosingWriterGuard` instead of a bare `outbound.close(); writer.join().unwrap();` at
  the end, so cleanup runs on a panicking path too. A `bounded_pair()` helper wraps
  `UnixStream::pair()` and sets a read timeout on the client half (closes the unbounded
  `read_exact`/`read_line` reads throughout this file); every test switches to it.
  `a_broken_write_closes_the_queue_...` wraps its direct `run_writer` call in
  `run_bounded` too (defense in depth, independent of which mechanism caused the
  sighting). New test `a_panicking_assertion_does_not_leak_the_writer_thread`:
  constructs a `ClosingWriterGuard` inside `std::panic::catch_unwind`, panics before any
  assertion runs, then proves (via a short `run_bounded` join on a handle obtained before
  the panic) that the thread is already gone - the demonstrated reproduction of the leak
  this task is fixing. New test `run_bounded_fails_fast_on_a_body_that_never_returns`:
  `#[should_panic(expected = "timed out")]`, calls `run_bounded` with a closure that
  waits forever on a fresh, never-closed `Outbound`'s own condvar directly, with a short
  (e.g. 200ms) bound - the negative control AC #3 asks for, for the thread case.
- `crates/server/tests/support/mod.rs` (new): the same `run_bounded` shape (duplicated,
  not shared from `src/` - integration tests are separate compilation units and cannot
  import a crate-private module; stated plainly rather than hidden), plus
  `wait_child_bounded(label, child, bound) -> ExitStatus`: `child.wait()` inside
  `run_bounded`; on timeout, SIGKILLs the child and does one more short bounded wait
  before giving up with a message naming the child and the bound.
- `crates/server/tests/race.rs`: `mod support;`. `ReapOnDrop::drop` (:54-59) and the
  explicit `kill()`/`wait()` pairs (:135-146, 176-180) use `wait_child_bounded` instead
  of a bare `child.wait()`. `wait_until_accepting`'s poll (:75-86) is left as is and
  called out in a comment: it retries an external, cross-process readiness condition
  with no in-process event to subscribe to, and already has an explicit deadline - not
  the same risk as a bare `join()`/`wait()`.
- `crates/server/tests/openpty_race.rs`: `mod support;`. `wait_timeout_or_kill` (:367+)
  is replaced by a call to the shared `wait_child_bounded`, removing its 5ms sleep-poll
  (`.claude/rules/testing-rust.md`: "no test sleeps to wait for something; it waits on
  the event") - the exact replacement TASK-1.82's notes already named this task for.
- `tools/src/steg_tools/quiet.py`: bounds the whole step, not only the Rust-level waits
  this task's sweep could have missed. `main` switches from `subprocess.run` to a
  `Popen(..., start_new_session=True)` plus `communicate(timeout=bound)`; on
  `TimeoutExpired`, `os.killpg(os.getpgid(proc.pid), signal.SIGKILL)` (the direct child
  alone would leave a hung test binary's own children behind), drains the already
  -captured output, and reports through a new `timed_out_report(label, bound, seconds,
  output, log)` (same one-line-plus-tail shape as `failed_report`, "TIMED OUT" in
  place of "FAILED", naming the bound). The bound is `STEG_QUIET_TIMEOUT_S`
  (env, default 600 - checked against `.quiet/*.log` timings from a recent `just check`
  run so the default clears every legitimate step, including the Zig/ghostty-vt build,
  with real headroom, and sits far under CI's 30-minute job limit).
- `tools/tests/unit/test_quiet.py`: a unit test for `timed_out_report`'s formatting,
  beside the existing `failed_report` test.
- `tools/tests/functional/test_quiet_timeout.py` (new): runs real `steg-quiet` as a
  subprocess against a command that outlives a very short `STEG_QUIET_TIMEOUT_S`
  (a Python one-liner that sleeps and then writes a sentinel file), asserts `steg-quiet`
  itself returns within a small bound, reports "TIMED OUT", and that the sentinel file
  is never written - proving the process group was actually killed, not just the
  immediate child abandoned.

## Order of work

1. `tools/src/steg_tools/quiet.py` + its two test files (the AC #4 backstop) - smallest,
   most independent piece, and the one that makes every later step safe to run even if
   something in it still hangs.
2. `crates/server/src/test_support.rs` + `lib.rs`'s one-line addition.
3. `crates/server/src/outbound.rs`: `bounded_pair()`, `ClosingWriterGuard` wired into the
   four existing tests, the `a_broken_write_...` test wrapped in `run_bounded`, then the
   two new tests (the leak reproduction and the negative control).
4. `crates/server/tests/support/mod.rs`, then `race.rs` and `openpty_race.rs` wired to
   it.
5. Record checks (Proof, below); write the findings above into this task's notes; draft
   the follow-up task for the rest of the sweep and link it.

## Risks

- `ClosingWriterGuard`'s `Drop` runs during an active panic if the guard itself is being
  unwound past; it must never panic itself (a second panic during unwind aborts the
  process) - its timeout path only logs. Mitigation: the new leak-reproduction test
  exercises exactly this path.
- `os.killpg` needs the child's own process group to actually contain what hung
  (`start_new_session=True` at spawn, not added after) - get this wrong and the backstop
  silently keeps the old behaviour. Mitigation: the functional test's sentinel-file
  check proves the kill really reached the process, not just the immediate `cargo`.
- The chosen `STEG_QUIET_TIMEOUT_S` default could be too tight for a slow CI runner and
  turn a slow-but-fine step into a false failure. Mitigation: set it from this task's own
  own recorded `just check`/`just test-all` timings with real headroom, not a guess, and
  say in the notes what the slowest observed step was.
- Converting `wait_timeout_or_kill` changes `openpty_race.rs`'s own already-passing
  control test's timing characteristics. Mitigation: re-run
  `test-openpty-race-control` after the change and record it.
- The 800,000-trial probe does not prove the EPIPE-timing mechanism is impossible, only
  that it didn't reproduce here; the plan does not rely on that probe to justify the fix
  - both candidate mechanisms get the same bound regardless.

## Proof

- `asdlc record-check -- cargo test -p steg-server --features race-fixture outbound::tests`
  (AC #1, #2 for the sighted file): the new leak-reproduction and negative-control tests
  pass; a described-in-notes-only manual check (never raw output pasted) that reverting
  `ClosingWriterGuard` back to the bare `close()`/`join()` makes the leak test fail.
- `asdlc record-check -- cargo test -p steg-server --features race-fixture` and
  `--features pty-race-fixture` (AC #2 for `race.rs`/`openpty_race.rs`), plus
  `just test-openpty-race-control` recorded again.
- `asdlc record-check -- just test-tools` (the new `quiet.py` unit + functional tests,
  AC #4).
- `asdlc record-check -- just check` at the end, full run.
- Note on the task naming the follow-up task created for the rest of the sweep (AC #2's
  literal, whole-crate reading), and asking the engineer to confirm the split rather than
  silently narrowing AC #2.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Sighting with a stack, 2026-11-16 by @bruk.o: on this machine, in TASK-1.121's worktree, 'just check' hung for 29 minutes in the step 'test (steg-server, race-fixture)'. A sample of the test binary showed one test thread still alive: outbound::tests::a_broken_write_closes_the_queue_so_the_readers_next_enqueue_fails. Its stack: the test calls outbound::run_writer on a UnixStream, which sits in Condvar::wait on the Queues lock (pthread_cond_wait), never woken. So the test calls run_writer on its own thread and relies on a write failing to end it; when the queue is empty at that moment the writer waits for work with nothing left to wake it. This is likely the same hang as the 28 minutes on CI, and it is in the unit tests built with the race-fixture feature, not in the race-fixture integration tests. Fix belongs here: the test must not be able to block forever (enqueue before running the writer, or run the writer on a thread joined with a bound), and check whether run_writer itself can miss a wake-up when the queue is closed while it waits.
<!-- SECTION:NOTES:END -->

## Comments

<!-- COMMENTS:BEGIN -->
created: 2026-11-16T13:14:00Z

---

2026-11-16, @bruk.o: a second kind of hung test, for the same lesson: TASK-1.98's new accept-backoff tests joined a reader thread with no bound and hung for 30 minutes on CI (run 43866214322) and three times on the worker's machine. Fixed in that task; when this task bounds the race tests, consider a suite-level guard so that no single test binary can run past a few minutes in just check (the fourth criterion here).

---

created: 2026-11-17T01:27:00Z

---

2026-11-16, @bruk.o, plan check (fbc7088082a8): not yet for approval; the finding needs one more step. (1) The second mechanism cannot be the hang: a leaked parked thread does not keep a Rust test binary alive, the process exits when the harness returns; and the sampled stack was the test's own thread under the harness, inside the direct run_writer call. So the write to the stream whose peer was dropped did succeed. (2) A mechanism that fits and that the 800,000-trial probe could not show: UnixStream::pair on macOS makes the pair and sets close-on-exec afterwards, not atomically. Other tests in the same binary fork and exec programs (pty and pane tests). A fork between the pair being made and drop(client) leaves a child holding the client end, so the peer is not gone, the write succeeds, and run_writer waits forever on an empty queue. Test this: repeat the probe while other threads spawn short-lived processes. (3) If confirmed, bounding that test only turns a hang into an intermittent failure. The test should not depend on a closed peer at all: run_writer is generic over Write, so give it a sink that always fails. (4) If confirmed it is also a product question, to note as a follow-up and not fix here: sockets the server accepts are likewise marked close-on-exec after the fact on macOS, so a pane's program started in that window could inherit a client's connection. Keep the rest: the step bound in steg-quiet, the helper, race.rs and openpty_race.rs, and the split of the wider sweep.

---
<!-- COMMENTS:END -->
