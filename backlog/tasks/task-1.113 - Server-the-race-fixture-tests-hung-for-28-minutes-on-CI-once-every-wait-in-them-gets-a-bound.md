---
id: TASK-1.113
title: >-
  Server: the race-fixture tests hung for 28 minutes on CI once; every wait in
  them gets a bound
status: In Review
assignee:
  - '@bruk.w9'
created_date: '2026-11-16T05:42:00Z'
updated_date: '2026-11-17T06:22:00Z'
labels:
  - size-2
  - review-passed
milestone: m-3
dependencies: []
references:
  - task-1.113-server-the-race-fixture-tests-hung-for
  - https://github.com/bruk-io/steg/pull/69
parent_task_id: TASK-1
project: bruk-io-steg
asdlc_accepted_from: DRAFT-2
asdlc_branch: task-1.113-server-the-race-fixture-tests-hung-for
asdlc_plan_approved_by: bruk
asdlc_plan_approved_hash: '2eb24f415089'
asdlc_plan_approved_size: size-2
asdlc_review_kind: same-machine
asdlc_review_findings: 'important=0,nit=0'
asdlc_plan_matched: yes
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
On 2026-11-16 pull request 55's check run 43802834909 (attempt 1, commit 51c1659, a change to crates/ui and tools only) printed 'ok test (steg-server) (12.3s)' at 04:16:46 UTC and nothing more until the job's 30-minute limit cancelled it at 04:44:21. The next step is the steg-server tests with the race-fixture feature (crates/server/tests/race.rs: several real processes racing for the arbiter's socket). Attempt 2 on the same commit passed that step in 10.6s. No earlier occurrence in the last 100 runs. A test that can wait for ever breaks the team's rule that a wait is bounded, costs 30 CI minutes when it happens, and left a worker idle for 80 minutes behind it. It may share a cause with the flakes recorded on TASK-1.82 (the arbiter's stale-socket test; the sdk's local socket reporting the address in use). Rests on doc-8 (arbitration, R1 to R3) and the team's testing rules.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The step that hung is identified from the evidence, or the notes say plainly that it could not be reproduced and what was tried
- [ ] #2 A test proves a bound works: a fixture process made to hang fails its test within the bound and leaves no process behind, with a recorded negative control
- [ ] #3 just check cannot spend its whole CI time limit on one hung test: the suite fails within minutes when a test hangs
- [ ] #4 Every wait in the test that hung (outbound.rs), in race.rs and in openpty_race.rs has a bound, and a test that passes its bound fails with a message naming what it waited for and kills and reaps the processes it started; the remaining waits found by the sweep are TASK-1.123's
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
## Findings (settle AC #1 and the orchestrator's question #1 - revised after team-lead review)

Read `crates/server/src/outbound.rs:267-298` (`run_writer`'s wait loop) and `:253-256`
(`close`): every mutation of `Queues.closed`/the queues happens under the same `Mutex`
the waiter re-checks before calling `Condvar::wait`, and `wait` is only ever called
while already holding that lock - a `close()`/`enqueue_*` landing between the check and
the wait cannot be missed. Read `crates/server/src/connection.rs:274-322`
(`ConnectionCleanup::drop`): `outbound.close()` runs on every path out of
`run_post_handshake` - normal return, early return, or a panic unwind - because it is a
local's `Drop`. So no production path ever abandons a connection without calling
`close()`: there is no "writer thread that never ends" in the product.

My first pass at the test-side cause (a panicking assertion elsewhere in this file
leaking a background writer thread) was wrong, as team-lead's review caught: a parked
non-main thread does not keep the test binary alive. The harness collects each
dispatched test's own result on its own thread and the process exits once that
collection is done; a thread some other, already-failed test spawned and never joined is
abandoned at process exit, with no effect on the next test or the run's own completion.
The sampled stack was the hanging test's *own* thread, inside its own direct,
synchronous `run_writer` call - consistent with only
`a_broken_write_closes_the_queue_so_the_readers_next_enqueue_fails`, the one test in this
file that calls `run_writer` without a `thread::spawn` in between.

Confirmed instead, per team-lead's direction: on macOS, `UnixStream::pair()` cannot set
`CLOEXEC` atomically. Read the toolchain's own std source
(`sys/net/connection/socket/unix.rs:111-144`, `Socket::new_pair`): the atomic
`SOCK_CLOEXEC`-on-`socketpair()` path is gated to Linux-like targets only
(`target_os = "linux"/"freebsd"/"netbsd"/...`, not `apple`); on macOS it calls
`socketpair()` plain, then sets `CLOEXEC` on each fd with two separate, later `fcntl`
calls (`:134-140`). A `fork`+`exec` landing in that gap inherits both fds regardless of
what `CLOEXEC` is set to a moment later in the parent - inheritance happens at `fork`,
`CLOEXEC` only matters at `exec`. `UnixListener::accept()` has the identical shape
(`:243-273`): atomic `accept4(..., SOCK_CLOEXEC)` on Linux-like targets, plain `accept()`
plus a later `fcntl` on macOS.

`outbound.rs`'s own unit tests run in the same compiled test binary, and by default the
same process, as every other `#[cfg(test)]` module in this crate - several of which fork
real processes from their own test threads, concurrently, under libtest's default
parallelism: `signal.rs:111-144` spawns `sh` children directly, `login_env.rs:209-228`
and `:632-667` spawn a real shell and a background `sleep ... &`, `pty.rs`'s
`RealLauncher` opens a pty and spawns a program through `portable_pty`, `pane.rs:952`
spawns `kill`. Any of these landing in the `socketpair()`-to-`set_cloexec()` gap of
`a_broken_write_closes_the_queue_...`'s own `UnixStream::pair()` call would have the
forked child inherit the `client` end; `drop(client)` in the test then closes only the
parent's own reference, the kernel still sees an open reference held by the forked
child, and the next `write_all` on `server` succeeds instead of failing.

Reproduced directly (scratch probe,
`/private/tmp/.../scratchpad/task-1.113-sockprobe/probe3.rs`, not committed, run in the
foreground, every spawned child explicitly `wait()`ed): with no concurrent forking, 0
successes in 800,000+ trials (serial and 16-way concurrent, both already run before
team-lead's review and recorded in the earlier plan revision). With 8 threads
continuously forking+exec'ing short-lived `sh -c true` children (the same shape
`signal.rs`'s own tests use) while 8 other threads repeated
`pair(); drop(client); write_all(server)`: **406,116 of 1,600,000 writes succeeded**
(roughly 1 in 4, once real concurrent forking is happening) - confirming the mechanism
directly, not just by reading the std source. `run_writer` then loops back to an empty,
unclosed queue and calls `Condvar::wait`, which nothing in that test will ever notify
again. This is the real cause: a test-side assumption (dropping one half of a freshly
made pair makes the other half's very next write fail at once) that macOS's own
non-atomic `CLOEXEC` makes unreliable whenever another thread in the same process forks
at the wrong moment - which this crate's own test suite does, routinely, by design.

Checked the rest of `outbound.rs`'s own tests for the same assumption: none of the
others depend on it - `closing_drains_what_is_already_queued_then_the_writer_returns`
calls `outbound.close()` explicitly before running the writer, never drops a peer to end
it, and the four spawn-then-join tests all end their writer via `outbound.close()` too,
not a dropped socket. Checked the other `drop(client)`/`drop(stream)` sites across
`crates/server/tests/{shutdown,idle_stop,routing}.rs`: all of them wait on an event with
seconds of slack (`recv_timeout(Duration::from_secs(5))` or similar) for the *server's*
own read loop to see the disconnect, not a bare, zero-slack "the very next local call
must fail" - a stray inherited fd from this same race would not survive seconds, so
these are not at the same risk and need no change.

**Product-adjacent finding, named but not fixed here** (team-lead's point 4): the same
non-atomic `accept()` applies to every connection `listen.rs` accepts on macOS. Grepped
`crates/server/src/*.rs` for `close_fds`/`CLOEXEC`/`set_cloexec`/`FD_CLOEXEC`: none of
this crate's own code does anything to scrub inherited descriptors before a pane's
program execs. `pty.rs`'s `RealLauncher` hands the command to `portable_pty`
(`CommandBuilder`/`PtyPair::slave.spawn_command`, not `std::process::Command` directly
for real panes), so whatever protection exists against a freshly accepted connection fd
being inherited by a concurrently-spawned pane's program lives entirely inside
`portable_pty`'s own spawn path, which I have not read. Whether that crate's spawn
already closes everything but its own wired-up pty fds, or shares the same gap, is
unverified - naming it for a follow-up task to check, not building anything for it here.

AC #2's literal scope is still larger than size-2 for the reasons already found (the
sweep across `tests/*.rs` and `#[cfg(test)]` `src/*.rs` blocks, ~60 sites) - unchanged
by this revision, see the "Files that change"/split note below.

## Files that change

- `crates/server/src/outbound.rs`: `a_broken_write_closes_the_queue_so_the_readers_next_enqueue_fails`
  no longer uses a real `UnixStream` at all. A tiny test-only `Write` type
  (`AlwaysFailsToWrite` or similar), whose `write`/`write_all` unconditionally returns
  `Err(io::Error::new(ErrorKind::BrokenPipe, ...))`, replaces the dropped-peer
  `UnixStream::pair()` - `run_writer` is already generic over `W: Write`, so this needs
  no change to the production code. This removes the dependence on `CLOEXEC`/fork timing
  entirely, rather than merely bounding around it; a bound alone would only have turned
  the hang into an intermittent ~1-in-4-when-contended failure, per the 406,116/1,600,000
  reproduction above. The call is still wrapped in `run_bounded` (below) as a second,
  independent guard for AC #2's literal "every wait has a bound", not as the fix for this
  test's own flakiness. `enqueue_data`/`enqueue_control` cap tests and the four
  spawn-then-assert-then-`close()` tests are unaffected (they don't depend on a dropped
  peer); those four gain a `ClosingWriterGuard` (below) purely as cleanup hygiene - stated
  as hygiene, not as a fix for any hang, per team-lead's review.
- `crates/server/src/lib.rs`: add `#[cfg(test)] mod test_support;`.
- `crates/server/src/test_support.rs` (new): `run_bounded(what, bound, body)` - runs
  `body` on a thread, `mpsc::Receiver::recv_timeout(bound)` on the result, panics naming
  `what` and `bound` on timeout; `ClosingWriterGuard` - owns the `Arc<Outbound>` and the
  writer's `JoinHandle`, and on `Drop` calls `outbound.close()` then joins with a bound,
  logging (never panicking - this can run mid-unwind) if the join itself ever times out.
  Mirrors `connection.rs:274-322`'s own "a local's `Drop` cleans up every exit path"
  idiom, moved into test code as the same hygiene, for the same reason - not presented as
  explaining the sighted hang.
- `crates/server/tests/support/mod.rs` (new): the same `run_bounded` shape (duplicated,
  not shared from `src/` - integration tests are separate compilation units and cannot
  import a crate-private module; stated plainly rather than hidden), plus
  `wait_child_bounded(label, child, bound) -> ExitStatus`: `child.wait()` inside
  `run_bounded`; on timeout, SIGKILLs the child and does one more short bounded wait
  before giving up with a message naming the child and the bound.
- `crates/server/tests/race.rs`: `mod support;`. `ReapOnDrop::drop` (:54-59) and the
  explicit `kill()`/`wait()` pairs (:135-146, 176-180) use `wait_child_bounded` instead
  of a bare `child.wait()`. `wait_until_accepting`'s poll (:75-86) is left as is, with a
  comment: it retries an external, cross-process readiness condition with no in-process
  event to subscribe to, and already has an explicit deadline.
- `crates/server/tests/openpty_race.rs`: `mod support;`. `wait_timeout_or_kill` (:367+)
  is replaced by a call to the shared `wait_child_bounded`, removing its 5ms sleep-poll
  (`.claude/rules/testing-rust.md`: "no test sleeps to wait for something; it waits on
  the event") - the exact replacement TASK-1.82's notes already named this task for.
- `tools/src/steg_tools/quiet.py`: bounds the whole step, independent of Rust-level
  coverage. `main` switches from `subprocess.run` to `Popen(..., start_new_session=True)`
  plus `communicate(timeout=bound)`; on `TimeoutExpired`,
  `os.killpg(os.getpgid(proc.pid), signal.SIGKILL)` (the direct child alone would leave a
  hung test binary's own children behind), drains the already-captured output, and
  reports through a new `timed_out_report(label, bound, seconds, output, log)` (same
  one-line-plus-tail shape as `failed_report`, "TIMED OUT" in place of "FAILED", naming
  the bound). The bound is `STEG_QUIET_TIMEOUT_S` (env, default 600 - checked against
  `.quiet/*.log` timings from a recent `just check` run so the default clears every
  legitimate step, including the Zig/ghostty-vt build, with real headroom, far under
  CI's 30-minute job limit).
- `tools/tests/unit/test_quiet.py`: a unit test for `timed_out_report`'s formatting,
  beside the existing `failed_report` test.
- `tools/tests/functional/test_quiet_timeout.py` (new): runs real `steg-quiet` as a
  subprocess against a command that outlives a very short `STEG_QUIET_TIMEOUT_S` (a
  Python one-liner that sleeps and then writes a sentinel file), asserts `steg-quiet`
  itself returns within a small bound, reports "TIMED OUT", and that the sentinel file is
  never written - proving the process group was actually killed, not just the immediate
  child abandoned.

## Order of work

1. `tools/src/steg_tools/quiet.py` + its two test files (the AC #4 backstop) - smallest,
   most independent piece, and the one that makes every later step safe to run even if
   something in it still hangs.
2. `crates/server/src/test_support.rs` + `lib.rs`'s one-line addition.
3. `crates/server/src/outbound.rs`: the `AlwaysFailsToWrite` sink and the rewritten
   `a_broken_write_closes_the_queue_...` test (deterministic fix), `run_bounded` wrapped
   around its call, `ClosingWriterGuard` wired into the four spawn-then-assert tests as
   hygiene, then `run_bounded_fails_fast_on_a_body_that_never_returns` as the generic
   negative control for the bound mechanism itself.
4. `crates/server/tests/support/mod.rs`, then `race.rs` and `openpty_race.rs` wired to
   it.
5. Record checks (Proof, below); write the findings above into this task's notes
   (including the product-adjacent `accept()`/`portable_pty` question, named for a
   follow-up); draft the follow-up task for the rest of the sweep and link it.

## Risks

- `ClosingWriterGuard`'s `Drop` runs during an active panic if the guard itself is being
  unwound past; it must never panic itself (a second panic during unwind aborts the
  process) - its timeout path only logs.
- `os.killpg` needs the child's own process group to actually contain what hung
  (`start_new_session=True` at spawn, not added after) - get this wrong and the backstop
  silently keeps the old behaviour. Mitigation: the functional test's sentinel-file check
  proves the kill really reached the process, not just the immediate `cargo`.
- The chosen `STEG_QUIET_TIMEOUT_S` default could be too tight for a slow CI runner and
  turn a slow-but-fine step into a false failure. Mitigation: set it from this task's own
  recorded `just check`/`just test-all` timings with real headroom, not a guess, and say
  in the notes what the slowest observed step was.
- Converting `wait_timeout_or_kill` changes `openpty_race.rs`'s own already-passing
  control test's timing characteristics. Mitigation: re-run
  `test-openpty-race-control` after the change and record it.
- The same `CLOEXEC` race could, in principle, affect any other test that drops one half
  of a freshly made pair and expects the other half's very next call to fail with no
  slack - checked and found none in this crate today (see Findings), but a future test
  written the same way would reintroduce it; worth a line in `testing-rust.md` or the
  follow-up task, not built here.

## Proof

- `asdlc record-check -- cargo test -p steg-server --features race-fixture outbound::tests`
  (AC #1, #2 for the sighted file): the rewritten test passes deterministically (no
  socket, no fork timing involved); `run_bounded_fails_fast_on_a_body_that_never_returns`
  passes as the negative control for the bound mechanism (AC #3).
- `asdlc record-check -- cargo test -p steg-server --features race-fixture` and
  `--features pty-race-fixture` (AC #2 for `race.rs`/`openpty_race.rs`), plus
  `just test-openpty-race-control` recorded again.
- `asdlc record-check -- just test-tools` (the new `quiet.py` unit + functional tests,
  AC #4).
- `asdlc record-check -- just check` at the end, full run.
- Note on the task naming the follow-up task created for the rest of the sweep (AC #2's
  literal, whole-crate reading) and the unverified `portable_pty`/`accept()` question,
  asking the engineer to confirm the split rather than silently narrowing AC #2.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Sighting with a stack, 2026-11-16 by @bruk.o: on this machine, in TASK-1.121's worktree, 'just check' hung for 29 minutes in the step 'test (steg-server, race-fixture)'. A sample of the test binary showed one test thread still alive: outbound::tests::a_broken_write_closes_the_queue_so_the_readers_next_enqueue_fails. Its stack: the test calls outbound::run_writer on a UnixStream, which sits in Condvar::wait on the Queues lock (pthread_cond_wait), never woken. So the test calls run_writer on its own thread and relies on a write failing to end it; when the queue is empty at that moment the writer waits for work with nothing left to wake it. This is likely the same hang as the 28 minutes on CI, and it is in the unit tests built with the race-fixture feature, not in the race-fixture integration tests. Fix belongs here: the test must not be able to block forever (enqueue before running the writer, or run the writer on a thread joined with a bound), and check whether run_writer itself can miss a wake-up when the queue is closed while it waits.

Plan checked against the task's criteria and the testing rules by @bruk.o (plan 2eb24f415089): ok. The cause is confirmed, not guessed: on macOS a socket pair is marked close-on-exec after it is made, other unit tests in the same binary fork, and a child that inherits the dropped end keeps the peer open, so the write succeeds and run_writer waits forever (probe: about one write in four succeeded with concurrent forks, none without). The test gets a sink that always fails, so it no longer depends on a closed peer. Kept: a bound on every step in steg-quiet with a process-group kill, the helpers, race.rs and openpty_race.rs. Split, for the engineer to accept: the wider sweep and the product question of a pane's program inheriting a connection are drafted as separate tasks; criterion 2 here is to be narrowed to the files this task names once the engineer agrees.

Check passed: `just test-tools` exited 0 at 2026-11-17T03:49:23+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.113 on task-1.113-server-the-race-fixture-tests-hung-for at 5d22d48902f2 with uncommitted changes:

```text
ok      tool tests (14.0s)
```

Check passed: `cargo test -p steg-server --lib` exited 0 at 2026-11-17T03:53:34+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.113 on task-1.113-server-the-race-fixture-tests-hung-for at 5d22d48902f2 with uncommitted changes, last 40 of 206 lines:

```text
test runtime::tests::runtime_new_does_not_panic_and_starts_empty ... ok
test runtime::tests::a_shell_pane_not_only_a_command_pane_gets_the_same_composed_environment ... ok
test runtime::tests::disconnect_client_reapplies_the_next_latest_declared_size ... ok
test signal::tests::send_reaches_a_real_process_with_the_named_signal ... ok
kill: 23502: No such process
test runtime::stop_tests::stop_ends_a_program_producing_output ... ok
test test_support::tests::a_body_that_returns_promptly_is_not_delayed ... ok
kill: 23698: No such process
test signal::tests::send_reaches_the_whole_process_group_not_only_the_leader ... ok
test runtime::tests::steg_vars_and_term_always_win_over_a_conflicting_value_from_the_login_shells_own_base_env ... ok
test runtime::tests::pane_split_falls_back_to_home_when_the_inherited_live_cwd_no_longer_exists ... ok
test runtime::tests::pane_split_inherits_the_live_cwd_of_the_pane_it_split_from_not_its_started_in_cwd ... ok
test runtime::tests::a_typed_command_still_running_stays_running_until_told_otherwise ... ok
test pane::attach_tests::an_attachment_gets_nothing_before_it_is_armed_even_if_output_happens_first ... ok
test login_env::tests::capture_login_environment_falls_back_on_a_shell_that_never_finishes_in_time ... ok
test pane::attach_tests::attaching_returns_the_panes_current_screen_and_updates_follow_it ... ok
test runtime::tests::an_unfiltered_subscriber_receives_nothing ... ok
test pane::ending_tests::await_exit_is_not_delayed_by_a_backlog_on_the_workers_own_queue ... ok
kill: 23505: No such process
test runtime::stop_tests::stop_ends_a_program_that_traps_the_polite_signal ... ok
test test_support::tests::a_body_that_never_returns_fails_fast_naming_what_it_waited_for - should panic ... ok
test runtime::tests::the_general_sizing_step_reaches_every_pane_through_split_close_and_swap ... ok
test pane::ending_tests::signal_hangup_reaches_a_backgrounded_grandchild_too ... ok
test idle::tests::the_checker_blocks_with_no_timeout_while_ineligible_and_waits_exactly_once_while_eligible ... ok
test runtime::tests::a_mark_then_the_shells_own_real_exit_cascades_with_no_panic ... ok
test runtime::tests::a_shell_with_no_marks_leaves_every_status_field_unchanged_until_one_arrives ... ok
test login_env::tests::capture_login_environment_falls_back_on_a_shell_that_exits_non_zero ... ok
test login_env::tests::capture_login_environment_prints_noise_before_the_marker_without_corrupting_the_dump ... ok
test pane::ending_tests::signal_kill_ends_a_program_that_traps_the_polite_signal ... ok
test runtime::tests::a_typed_command_that_fails_is_failed_with_its_exit_code ... ok
test runtime::tests::a_typed_command_that_succeeds_is_running_then_finished_and_the_tab_shows_it ... ok
test runtime::tests::a_typed_commands_status_reaches_pane_show_and_tab_show_through_runtime_handle ... ok
test runtime::tests::a_command_panes_own_program_printing_marks_does_not_change_its_status ... ok
kill: 23788: No such process
test login_env::tests::capture_login_environment_kills_a_backgrounded_grandchild_that_held_the_pipe_open ... ok
kill: 23500: No such process
test pane::tests::a_forward_failing_once_the_worker_is_gone_still_reaps_a_program_that_ends_on_its_own ... ok
test runtime::tests::attach_pane_on_a_busy_panes_own_worker_does_not_stall_an_unrelated_requests_lock ... ok

test result: ok. 193 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.08s
```

Check passed: `cargo test -p steg-server --features race-fixture` exited 0 at 2026-11-17T04:03:24+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.113 on task-1.113-server-the-race-fixture-tests-hung-for at 5d22d48902f2 with uncommitted changes, last 40 of 501 lines:

```text
kill: 52971: No such process
kill: 52972: No such process
test tab_close_with_force_ends_every_program_in_scope_within_one_grace_window ... ok
kill: 52858: No such process
test a_session_being_forced_is_unreachable_by_every_method_that_can_name_it ... ok
kill: 52866: No such process
test a_tab_being_forced_is_unreachable_by_every_method_that_can_name_it ... ok
kill: 52891: No such process
test a_pane_being_forced_is_unreachable_by_every_method_that_can_name_it ... ok
kill: 52958: No such process
test a_pane_being_forced_is_unreachable_by_another_request_meanwhile ... ok

test result: ok. 30 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 5.09s

     Running tests/shutdown.rs (target/debug/deps/shutdown-c6275789232d00c3)

running 8 tests
test stopping_closes_the_listener_removes_the_socket_and_releases_the_lock ... ok
test stop_on_a_genuinely_idle_server_returns_from_run_within_a_bound ... ok
test stop_called_twice_concurrently_on_an_idle_server_is_harmless ... ok
test a_stalled_handshake_is_closed_and_run_returns_promptly_on_stop ... ok
test a_completed_idle_connection_is_closed_and_run_returns_promptly_on_stop ... ok
test many_short_connections_do_not_grow_the_tracked_set ... ok
test stopping_while_a_trickling_handshake_is_in_progress_still_returns_within_a_bound ... ok
test stop_racing_a_client_connecting_still_returns_within_a_bound ... ok

test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.13s

     Running tests/socket_mode.rs (target/debug/deps/socket_mode-5b738c682df06af0)

running 1 test
test the_socket_is_0600_even_under_a_permissive_umask ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests steg_server

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

Check passed: `cargo test -p steg-server --features pty-race-fixture` exited 0 at 2026-11-17T04:04:16+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.113 on task-1.113-server-the-race-fixture-tests-hung-for at 5d22d48902f2 with uncommitted changes, last 40 of 500 lines:

```text
kill: 57058: No such process
kill: 57060: No such process
test tab_close_with_force_ends_every_program_in_scope_within_one_grace_window ... ok
kill: 56962: No such process
test a_pane_being_forced_is_unreachable_by_another_request_meanwhile ... ok
kill: 56973: No such process
kill: 56975: No such process
test a_tab_being_forced_is_unreachable_by_every_method_that_can_name_it ... ok
test a_session_being_forced_is_unreachable_by_every_method_that_can_name_it ... ok
kill: 57029: No such process
test a_pane_being_forced_is_unreachable_by_every_method_that_can_name_it ... ok

test result: ok. 30 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 5.10s

     Running tests/shutdown.rs (target/debug/deps/shutdown-ea9d4bbecb02e2c9)

running 8 tests
test a_stalled_handshake_is_closed_and_run_returns_promptly_on_stop ... ok
test stop_on_a_genuinely_idle_server_returns_from_run_within_a_bound ... ok
test stop_called_twice_concurrently_on_an_idle_server_is_harmless ... ok
test a_completed_idle_connection_is_closed_and_run_returns_promptly_on_stop ... ok
test stopping_closes_the_listener_removes_the_socket_and_releases_the_lock ... ok
test many_short_connections_do_not_grow_the_tracked_set ... ok
test stopping_while_a_trickling_handshake_is_in_progress_still_returns_within_a_bound ... ok
test stop_racing_a_client_connecting_still_returns_within_a_bound ... ok

test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.14s

     Running tests/socket_mode.rs (target/debug/deps/socket_mode-4e1e3a14870d2373)

running 1 test
test the_socket_is_0600_even_under_a_permissive_umask ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests steg_server

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

Check FAILED: `just test-openpty-race-control` exited 101 at 2026-11-17T04:08:00+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.113 on task-1.113-server-the-race-fixture-tests-hung-for at 5d22d48902f2 with uncommitted changes:

```text
FAILED  openpty race control (exit 101, 214.2s)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.09s
     Running tests/openpty_race.rs (target/debug/deps/openpty_race-7feda493ce844436)

running 1 test
test cross_process_opens_with_no_retry_the_permanent_control_eventually_fails has been running for over 60 seconds

thread 'cross_process_opens_with_no_retry_the_permanent_control_eventually_fails' (50293704) panicked at crates/server/tests/openpty_race.rs:408:5:
expected at least one failure within 100000 real cross-process opens with retry forced off (max_open_attempts = 1) - reaching the bound with none seen means the race was not reproduced on this run (saw 100000 opens, 0 failures); see the task's own notes before weakening or deleting this test
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace
test cross_process_opens_with_no_retry_the_permanent_control_eventually_fails ... FAILED

failures:

failures:
    cross_process_opens_with_no_retry_the_permanent_control_eventually_fails

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 1 filtered out; finished in 214.05s

error: test failed, to rerun pass `-p steg-server --test openpty_race`
full output: .quiet/openpty-race-control.log
error: recipe `test-openpty-race-control` failed on line 149 with exit code 101
```

Check FAILED: `just check` exited 1 at 2026-11-17T04:08:32+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.113 on task-1.113-server-the-race-fixture-tests-hung-for at 5d22d48902f2 with uncommitted changes:

```text
FAILED  format check (exit 1, 0.2s)
Diff in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.113/crates/server/tests/openpty_race.rs:340:
         // reader thread has already returned in the overwhelming
         // majority of cases - bounded all the same, rather than a bare
         // `join()` that could block indefinitely if it has not.
[31m-        run_bounded(&format!("reader thread {worker} to end"), STOP_TIMEOUT, move || {
(B[m[31m-            let _ = handle.join();
(B[m[31m-        });
(B[m[32m+        run_bounded(
(B[m[32m+            &format!("reader thread {worker} to end"),
(B[m[32m+            STOP_TIMEOUT,
(B[m[32m+            move || {
(B[m[32m+                let _ = handle.join();
(B[m[32m+            },
(B[m[32m+        );
(B[m     }
 
     let total_opens: u64 = done.iter().map(|d| d.map_or(0, |(t, _)| t)).sum();
full output: .quiet/format-check.log
error: recipe `check-fmt` failed on line 157 with exit code 1
```

Check FAILED: `just check` exited 101 at 2026-11-17T04:09:30+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.113 on task-1.113-server-the-race-fixture-tests-hung-for at 5d22d48902f2 with uncommitted changes, last 40 of 65 lines:

```text
    Checking resvg v0.45.1
    Checking icu_properties v2.4.0
    Checking icu_normalizer v2.4.0
    Checking half v2.7.1
   Compiling toml v0.8.23
   Compiling naga v25.0.1
    Checking tiff v0.11.3
    Checking exr v1.74.3
   Compiling cbindgen v0.28.0
    Checking idna_adapter v1.2.2
    Checking idna v1.1.0
    Checking url v2.5.8
    Checking zed-reqwest v0.12.15-zed
    Checking ravif v0.13.0
    Checking gpui_http_client v0.2.2
    Checking image v0.25.10
    Checking blade-graphics v0.7.1
    Checking blade-util v0.3.0
   Compiling gpui v0.2.2
    Checking steg-ui v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.113/crates/ui)
    Checking steg-app v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.113/crates/app)
   Compiling ghostty-vt v0.8.0+ghostty.1.4.0-main.83fb956 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.113/third_party/ghostty-vt-patched)
    Checking steg-kernel v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.113/crates/kernel)
    Checking steg-server v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.113/crates/server)
    Checking steg v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.113/crates/steg)
error: `allow` attribute without specifying a reason
  --> crates/server/tests/support/mod.rs:28:1
   |
28 | #[allow(dead_code)]
   | ^^^^^^^^^^^^^^^^^^^
   |
   = help: try adding a reason at the end with `, reason = ".."`
   = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.98.0/index.html#allow_attributes_without_reason
   = note: requested on the command line with `-D clippy::allow-attributes-without-reason`

error: could not compile `steg-server` (test "race") due to 1 previous error
warning: build failed, waiting for other jobs to finish...
error: could not compile `steg-server` (test "openpty_race") due to 1 previous error
full output: .quiet/lint.log
error: recipe `lint` failed with exit code 101
```

Check passed: `just check` exited 0 at 2026-11-17T04:15:20+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.113 on task-1.113-server-the-race-fixture-tests-hung-for at 5d22d48902f2 with uncommitted changes:

```text
ok      format check (0.2s)
ok      lint (29.6s)
ok      boundaries (0.2s)
ok      foundations (0.2s)
ok      foundations public surface (0.1s)
ok      design pages (0.1s)
ok      licences (0.7s)
ok      tests (every crate) (31.1s)
ok      build (steg-sdk) (3.3s)
ok      test (steg-sdk) (2.4s)
ok      test (steg-sdk, local-socket) (2.8s)
ok      build (steg-kernel) (40.7s)
ok      test (steg-kernel) (1.5s)
ok      build (steg-server) (28.7s)
ok      test (steg-server) (27.1s)
ok      test (steg-server, race-fixture) (27.9s)
ok      test (steg-server, pty-race-fixture) (33.5s)
ok      test (steg-server, accept-backoff-fixture) (29.4s)
ok      build (steg) (2.3s)
ok      test (steg) (2.3s)
ok      test (steg, fixtures) (7.0s)
ok      build (steg-ui) (14.5s)
ok      test (steg-ui) (1.4s)
ok      build (steg-app) (1.6s)
ok      test (steg-app) (0.8s)
ok      tool tests (13.6s)
ok      architecture (0.7s)
ok      recipe names (0.1s)
ok      ui levels (0.1s)
ok      ui colours (0.1s)
```

Check passed: `just check` exited 0 at 2026-11-17T04:19:24+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.113 on task-1.113-server-the-race-fixture-tests-hung-for at dc5ac181fb37:

```text
ok      format check (0.2s)
ok      lint (1.3s)
ok      boundaries (0.1s)
ok      foundations (0.1s)
ok      foundations public surface (0.0s)
ok      design pages (0.0s)
ok      licences (0.7s)
ok      tests (every crate) (29.9s)
ok      build (steg-sdk) (0.1s)
ok      test (steg-sdk) (0.3s)
ok      test (steg-sdk, local-socket) (0.4s)
ok      build (steg-kernel) (0.1s)
ok      test (steg-kernel) (0.2s)
ok      build (steg-server) (0.7s)
ok      test (steg-server) (25.8s)
ok      test (steg-server, race-fixture) (28.2s)
ok      test (steg-server, pty-race-fixture) (33.6s)
ok      test (steg-server, accept-backoff-fixture) (29.1s)
ok      build (steg) (0.7s)
ok      test (steg) (2.5s)
ok      test (steg, fixtures) (8.0s)
ok      build (steg-ui) (0.3s)
ok      test (steg-ui) (0.3s)
ok      build (steg-app) (0.3s)
ok      test (steg-app) (0.2s)
ok      tool tests (13.5s)
ok      architecture (0.7s)
ok      recipe names (0.0s)
ok      ui levels (0.0s)
ok      ui colours (0.1s)
```

Built per the approved, revised plan. Root cause (AC #1): confirmed via the toolchain's own std source (sys/net/connection/socket/unix.rs) that UnixStream::pair/accept cannot set CLOEXEC atomically on macOS; a fork from another thread of the same test binary (signal.rs, login_env.rs, pty.rs, pane.rs all fork real processes) can inherit the dropped client fd before close() lands, keeping the write from failing. Reproduced directly (scratch probe, not committed): 0/800,000+ failures with nothing else forking, ~1 in 4 successes with 8 threads forking concurrently. connection.rs's own ConnectionCleanup::drop confirms close() always runs on every production exit path, so this is a test-only defect, not a product bug.

Fix: a_broken_write_closes_the_queue_so_the_readers_next_enqueue_fails now uses a sink that always fails (AlwaysFailsToWrite) instead of a dropped-peer socket, removing the dependency on fork timing entirely; also wrapped in the new run_bounded helper as a second, independent guard. Added crates/server/src/test_support.rs (run_bounded, ClosingWriterGuard - the latter gives the four spawn-then-assert outbound.rs tests panic-safe cleanup, mirroring connection.rs's own Drop idiom; stated as hygiene, not a fix for the hang). Added crates/server/tests/support/mod.rs (same run_bounded shape plus wait_child_bounded, a watchdog-thread design, not a sleep-poll) and wired it into race.rs (the kill()-then-wait() sites) and openpty_race.rs (replacing wait_timeout_or_kill's 5ms poll, per TASK-1.82's own forward reference, and bounding the reader-thread join). ReapOnDrop's own Drop impls in both files are left as plain kill()+wait(): wait_child_bounded panics on timeout, and a panic inside Drop during an unwind would abort the process rather than report one clean failure - documented inline.

steg-quiet (tools/src/steg_tools/quiet.py) now bounds every just check step independently of Rust-level coverage: Popen with start_new_session=True, killpg on a per-step timeout (STEG_QUIET_TIMEOUT_S, default 300s). Default set from real recorded timings: 132.2s was the slowest step seen across ~15 sampled CI runs, 109.2s the slowest single-package step, 92s a cold full-workspace build with no cache at all on this machine; no CI run sampled ever hit a cold Zig/ghostty-vt build (cache stayed warm every time), so 300s clears every observed case with real headroom and still leaves CI's 30-minute job limit room to report several timeouts rather than exhaust it on one.

Checks recorded via asdlc record-check: cargo test -p steg-server --lib (193 passed); cargo test -p steg-server --features race-fixture (full crate); cargo test -p steg-server --features pty-race-fixture (full crate); just test-openpty-race-control (re-run per the plan's own risk - one run missed the reproduction at 0/100,000, a second run reproduced in 0.8s, both within the control's own documented variance, nothing weakened); just test-tools (new quiet.py unit + functional tests, including a real grandchild-process-and-sentinel-file proof that killpg reaches the whole group, not only the direct child); just check, full workspace, twice (once before, once after rebasing onto origin/main's latest board commits).

Two negative controls run by hand, not committed, not posted as raw output (description only, per the safety rule): reverting outbound.rs's close()-on-broken-write made the rewritten test fail with a clear assertion mismatch, confirming it still tests what it claims; reverting run_bounded's own timeout (rx.recv() instead of recv_timeout) made its own negative-control test hang indefinitely (confirmed by wrapping it in an external 8s timeout, which fired), confirming the bound mechanism itself is load-bearing. Both restored and re-verified passing before any check was recorded.

Scope: criterion 2 was narrowed to outbound.rs, race.rs and openpty_race.rs per the engineer's split (TASK-1.123 for the wider sweep, TASK-1.124 for the accept()/portable_pty product question) before this build started. ps checked clean of leftover processes after every run. PR #69, CI green (6m21s, gh pr checks --watch).

Check passed: `cargo test -p steg-server --lib` exited 0 at 2026-11-17T04:51:16+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.113 on task-1.113-server-the-race-fixture-tests-hung-for at dc5ac181fb37 with uncommitted changes, last 40 of 207 lines:

```text
test runtime::tests::apply_size_changed_resizes_only_the_panes_the_change_names ... ok
test runtime::tests::a_shell_pane_not_only_a_command_pane_gets_the_same_composed_environment ... ok
test signal::tests::send_reaches_a_real_process_with_the_named_signal ... ok
test runtime::tests::disconnect_client_reapplies_the_next_latest_declared_size ... ok
kill: 51262: No such process
kill: 51093: No such process
test runtime::stop_tests::stop_ends_a_program_producing_output ... ok
test signal::tests::send_reaches_the_whole_process_group_not_only_the_leader ... ok
test test_support::tests::a_body_that_returns_promptly_is_not_delayed ... ok
test runtime::tests::steg_vars_and_term_always_win_over_a_conflicting_value_from_the_login_shells_own_base_env ... ok
test runtime::tests::a_typed_command_still_running_stays_running_until_told_otherwise ... ok
test runtime::tests::pane_split_falls_back_to_home_when_the_inherited_live_cwd_no_longer_exists ... ok
test runtime::tests::pane_split_inherits_the_live_cwd_of_the_pane_it_split_from_not_its_started_in_cwd ... ok
test pane::attach_tests::an_attachment_gets_nothing_before_it_is_armed_even_if_output_happens_first ... ok
test login_env::tests::capture_login_environment_falls_back_on_a_shell_that_never_finishes_in_time ... ok
test pane::attach_tests::attaching_returns_the_panes_current_screen_and_updates_follow_it ... ok
test pane::ending_tests::await_exit_is_not_delayed_by_a_backlog_on_the_workers_own_queue ... ok
test runtime::tests::an_unfiltered_subscriber_receives_nothing ... ok
kill: 51095: No such process
test runtime::stop_tests::stop_ends_a_program_that_traps_the_polite_signal ... ok
test test_support::tests::a_body_that_never_returns_fails_fast_naming_what_it_waited_for - should panic ... ok
test runtime::tests::the_general_sizing_step_reaches_every_pane_through_split_close_and_swap ... ok
test pane::ending_tests::signal_hangup_reaches_a_backgrounded_grandchild_too ... ok
test idle::tests::the_checker_blocks_with_no_timeout_while_ineligible_and_waits_exactly_once_while_eligible ... ok
test runtime::tests::a_mark_then_the_shells_own_real_exit_cascades_with_no_panic ... ok
test runtime::tests::a_shell_with_no_marks_leaves_every_status_field_unchanged_until_one_arrives ... ok
test pane::ending_tests::signal_kill_ends_a_program_that_traps_the_polite_signal ... ok
test login_env::tests::capture_login_environment_falls_back_on_a_shell_that_exits_non_zero ... ok
test runtime::tests::a_typed_command_that_fails_is_failed_with_its_exit_code ... ok
test runtime::tests::a_typed_command_that_succeeds_is_running_then_finished_and_the_tab_shows_it ... ok
test runtime::tests::a_typed_commands_status_reaches_pane_show_and_tab_show_through_runtime_handle ... ok
test login_env::tests::capture_login_environment_prints_noise_before_the_marker_without_corrupting_the_dump ... ok
test runtime::tests::a_command_panes_own_program_printing_marks_does_not_change_its_status ... ok
kill: 51380: No such process
test login_env::tests::capture_login_environment_kills_a_backgrounded_grandchild_that_held_the_pipe_open ... ok
kill: 51088: No such process
test pane::tests::a_forward_failing_once_the_worker_is_gone_still_reaps_a_program_that_ends_on_its_own ... ok
test runtime::tests::attach_pane_on_a_busy_panes_own_worker_does_not_stall_an_unrelated_requests_lock ... ok

test result: ok. 193 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.09s
```

Commits since the review: 0.

Check passed: `cargo test -p steg-server --features race-fixture` exited 0 at 2026-11-17T04:51:51+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.113 on task-1.113-server-the-race-fixture-tests-hung-for at dc5ac181fb37 with uncommitted changes, last 40 of 502 lines:

```text
kill: 53066: No such process
kill: 53067: No such process
test tab_close_with_force_ends_every_program_in_scope_within_one_grace_window ... ok
kill: 52956: No such process
kill: 52954: No such process
test a_pane_being_forced_is_unreachable_by_another_request_meanwhile ... ok
test a_tab_being_forced_is_unreachable_by_every_method_that_can_name_it ... ok
kill: 52971: No such process
kill: 52968: No such process
test a_session_being_forced_is_unreachable_by_every_method_that_can_name_it ... ok
test a_pane_being_forced_is_unreachable_by_every_method_that_can_name_it ... ok

test result: ok. 30 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 5.08s

     Running tests/shutdown.rs (target/debug/deps/shutdown-c6275789232d00c3)

running 8 tests
test stop_called_twice_concurrently_on_an_idle_server_is_harmless ... ok
test a_stalled_handshake_is_closed_and_run_returns_promptly_on_stop ... ok
test a_completed_idle_connection_is_closed_and_run_returns_promptly_on_stop ... ok
test stop_on_a_genuinely_idle_server_returns_from_run_within_a_bound ... ok
test stopping_closes_the_listener_removes_the_socket_and_releases_the_lock ... ok
test stopping_while_a_trickling_handshake_is_in_progress_still_returns_within_a_bound ... ok
test many_short_connections_do_not_grow_the_tracked_set ... ok
test stop_racing_a_client_connecting_still_returns_within_a_bound ... ok

test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.14s

     Running tests/socket_mode.rs (target/debug/deps/socket_mode-5b738c682df06af0)

running 1 test
test the_socket_is_0600_even_under_a_permissive_umask ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests steg_server

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

Commits since the review: 0.

Check passed: `cargo test -p steg-server --features pty-race-fixture` exited 0 at 2026-11-17T04:52:34+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.113 on task-1.113-server-the-race-fixture-tests-hung-for at dc5ac181fb37 with uncommitted changes, last 40 of 501 lines:

```text
kill: 54908: No such process
kill: 54910: No such process
test tab_close_with_force_ends_every_program_in_scope_within_one_grace_window ... ok
kill: 54783: No such process
test a_session_being_forced_is_unreachable_by_every_method_that_can_name_it ... ok
kill: 54803: No such process
test a_pane_being_forced_is_unreachable_by_another_request_meanwhile ... ok
kill: 54857: No such process
test a_pane_being_forced_is_unreachable_by_every_method_that_can_name_it ... ok
kill: 54859: No such process
test a_tab_being_forced_is_unreachable_by_every_method_that_can_name_it ... ok

test result: ok. 30 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 5.09s

     Running tests/shutdown.rs (target/debug/deps/shutdown-ea9d4bbecb02e2c9)

running 8 tests
test stopping_closes_the_listener_removes_the_socket_and_releases_the_lock ... ok
test a_stalled_handshake_is_closed_and_run_returns_promptly_on_stop ... ok
test stop_called_twice_concurrently_on_an_idle_server_is_harmless ... ok
test stop_on_a_genuinely_idle_server_returns_from_run_within_a_bound ... ok
test a_completed_idle_connection_is_closed_and_run_returns_promptly_on_stop ... ok
test stopping_while_a_trickling_handshake_is_in_progress_still_returns_within_a_bound ... ok
test many_short_connections_do_not_grow_the_tracked_set ... ok
test stop_racing_a_client_connecting_still_returns_within_a_bound ... ok

test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.13s

     Running tests/socket_mode.rs (target/debug/deps/socket_mode-4e1e3a14870d2373)

running 1 test
test the_socket_is_0600_even_under_a_permissive_umask ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests steg_server

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

Commits since the review: 0.

Check passed: `just test-tools` exited 0 at 2026-11-17T04:52:55+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.113 on task-1.113-server-the-race-fixture-tests-hung-for at dc5ac181fb37 with uncommitted changes:

```text
ok      tool tests (13.7s)
```

Commits since the review: 0.

Check FAILED: `just test-openpty-race-control` exited 101 at 2026-11-17T04:56:47+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.113 on task-1.113-server-the-race-fixture-tests-hung-for at dc5ac181fb37 with uncommitted changes:

```text
FAILED  openpty race control (exit 101, 221.9s)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.08s
     Running tests/openpty_race.rs (target/debug/deps/openpty_race-7feda493ce844436)

running 1 test
test cross_process_opens_with_no_retry_the_permanent_control_eventually_fails has been running for over 60 seconds

thread 'cross_process_opens_with_no_retry_the_permanent_control_eventually_fails' (50960016) panicked at crates/server/tests/openpty_race.rs:412:5:
expected at least one failure within 100000 real cross-process opens with retry forced off (max_open_attempts = 1) - reaching the bound with none seen means the race was not reproduced on this run (saw 100000 opens, 0 failures); see the task's own notes before weakening or deleting this test
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace
test cross_process_opens_with_no_retry_the_permanent_control_eventually_fails ... FAILED

failures:

failures:
    cross_process_opens_with_no_retry_the_permanent_control_eventually_fails

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 2 filtered out; finished in 221.82s

error: test failed, to rerun pass `-p steg-server --test openpty_race`
full output: .quiet/openpty-race-control.log
error: recipe `test-openpty-race-control` failed on line 149 with exit code 101
```

Commits since the review: 0.

Check passed: `just test-openpty-race-control` exited 0 at 2026-11-17T04:57:00+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.113 on task-1.113-server-the-race-fixture-tests-hung-for at dc5ac181fb37 with uncommitted changes:

```text
ok      openpty race control (0.7s)
```

Commits since the review: 0.

Check passed: `just check` exited 0 at 2026-11-17T05:01:18+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.113 on task-1.113-server-the-race-fixture-tests-hung-for at dc5ac181fb37 with uncommitted changes:

```text
ok      format check (0.2s)
ok      lint (30.1s)
ok      boundaries (0.1s)
ok      foundations (0.1s)
ok      foundations public surface (0.0s)
ok      design pages (0.1s)
ok      licences (0.8s)
ok      tests (every crate) (36.9s)
ok      build (steg-sdk) (0.1s)
ok      test (steg-sdk) (1.1s)
ok      test (steg-sdk, local-socket) (0.9s)
ok      build (steg-kernel) (0.1s)
ok      test (steg-kernel) (0.4s)
ok      build (steg-server) (29.0s)
ok      test (steg-server) (27.9s)
ok      test (steg-server, race-fixture) (27.7s)
ok      test (steg-server, pty-race-fixture) (34.5s)
ok      test (steg-server, accept-backoff-fixture) (30.2s)
ok      build (steg) (0.9s)
ok      test (steg) (3.0s)
ok      test (steg, fixtures) (9.1s)
ok      build (steg-ui) (0.3s)
ok      test (steg-ui) (0.8s)
ok      build (steg-app) (0.3s)
ok      test (steg-app) (0.4s)
ok      tool tests (13.5s)
ok      architecture (0.7s)
ok      recipe names (0.1s)
ok      ui levels (0.1s)
ok      ui colours (0.1s)
```

Commits since the review: 0.

Check FAILED: `cargo test -p steg-server --lib outbound::tests::a_broken_write_closes_the_queue_so_the_readers_next_enqueue_fails` exited 101 at 2026-11-17T05:03:28+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.113 on task-1.113-server-the-race-fixture-tests-hung-for at 11f5e0ce3676 with uncommitted changes:

```text
   Compiling ghostty-vt-sys v0.8.0+ghostty.1.4.0-main.83fb956
   Compiling ghostty-vt v0.8.0+ghostty.1.4.0-main.83fb956 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.113/third_party/ghostty-vt-patched)
   Compiling steg-kernel v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.113/crates/kernel)
   Compiling steg-server v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.113/crates/server)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 29.57s
     Running unittests src/lib.rs (target/debug/deps/steg_server-6d2f1d51fdd2e018)

running 1 test
test outbound::tests::a_broken_write_closes_the_queue_so_the_readers_next_enqueue_fails ... FAILED

failures:

---- outbound::tests::a_broken_write_closes_the_queue_so_the_readers_next_enqueue_fails stdout ----

thread 'outbound::tests::a_broken_write_closes_the_queue_so_the_readers_next_enqueue_fails' (51098358) panicked at crates/server/src/outbound.rs:490:9:
assertion `left == right` failed: the writer's own broken-write exit must close the queue, the same as an explicit Outbound::close() would
  left: Ok(())
 right: Err(Closed)
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    outbound::tests::a_broken_write_closes_the_queue_so_the_readers_next_enqueue_fails

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 192 filtered out; finished in 0.00s

error: test failed, to rerun pass `-p steg-server --lib`
```

Commits since the review: 26, 25 without the Asdlc-Worker trailer.

Check passed: `cargo test -p steg-server --lib outbound::tests::a_broken_write_closes_the_queue_so_the_readers_next_enqueue_fails` exited 0 at 2026-11-17T05:03:40+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.113 on task-1.113-server-the-race-fixture-tests-hung-for at 11f5e0ce3676:

```text
   Compiling steg-server v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.113/crates/server)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.67s
     Running unittests src/lib.rs (target/debug/deps/steg_server-6d2f1d51fdd2e018)

running 1 test
test outbound::tests::a_broken_write_closes_the_queue_so_the_readers_next_enqueue_fails ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 192 filtered out; finished in 0.00s
```

Commits since the review: 26, 25 without the Asdlc-Worker trailer.

Check FAILED: `uv run --quiet --project tools steg-quiet 'run_bounded timeout control (mechanism removed)' -- cargo test -p steg-server --lib test_support::tests::a_body_that_never_returns_fails_fast_naming_what_it_waited_for -- --exact` exited 1 at 2026-11-17T05:04:03+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.113 on task-1.113-server-the-race-fixture-tests-hung-for at 11f5e0ce3676 with uncommitted changes:

```text
TIMED OUT run_bounded timeout control (mechanism removed) (bound 5s, ran 5.0s)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.09s
     Running unittests src/lib.rs (target/debug/deps/steg_server-6d2f1d51fdd2e018)

running 1 test
full output: .quiet/run-bounded-timeout-control-mechanism-removed.log
```

Commits since the review: 26, 25 without the Asdlc-Worker trailer.

Check passed: `cargo test -p steg-server --lib test_support::tests::a_body_that_never_returns_fails_fast_naming_what_it_waited_for -- --exact` exited 0 at 2026-11-17T05:04:17+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.113 on task-1.113-server-the-race-fixture-tests-hung-for at 11f5e0ce3676:

```text
   Compiling steg-server v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.113/crates/server)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.71s
     Running unittests src/lib.rs (target/debug/deps/steg_server-6d2f1d51fdd2e018)

running 1 test
test test_support::tests::a_body_that_never_returns_fails_fast_naming_what_it_waited_for - should panic ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 192 filtered out; finished in 0.21s
```

Commits since the review: 26, 25 without the Asdlc-Worker trailer.

Check FAILED: `uv run --quiet --project tools pytest -q tools/tests/functional/test_quiet_timeout.py` exited 1 at 2026-11-17T05:04:48+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.113 on task-1.113-server-the-race-fixture-tests-hung-for at 11f5e0ce3676 with uncommitted changes:

```text
FF.                                                                      [100%]
=================================== FAILURES ===================================
_________ test_a_captured_step_past_its_bound_is_killed_group_and_all __________

tmp_path = PosixPath('/private/var/folders/rf/f34xmczn4659gm98hrsprydc0000gn/T/pytest-of-brukhabtu/pytest-623/test_a_captured_step_past_its_0')

    def test_a_captured_step_past_its_bound_is_killed_group_and_all(tmp_path: Path) -> None:
        sentinel = tmp_path / "sentinel"
        command = [sys.executable, "-c", _OUTLIVES_THE_BOUND_WITH_A_GRANDCHILD, str(sentinel)]
    
        code, output, seconds = run_captured(command, BOUND_S)
    
        assert code is None
        assert output == ""
>       assert seconds < 2.0, f"a killed step should return close to its {BOUND_S}s bound, took {seconds}s"
E       AssertionError: a killed step should return close to its 0.2s bound, took 10.036291625001468s
E       assert 10.036291625001468 < 2.0

tools/tests/functional/test_quiet_timeout.py:37: AssertionError
___________ test_a_live_step_past_its_bound_is_killed_group_and_all ____________

tmp_path = PosixPath('/private/var/folders/rf/f34xmczn4659gm98hrsprydc0000gn/T/pytest-of-brukhabtu/pytest-623/test_a_live_step_past_its_boun0')

    def test_a_live_step_past_its_bound_is_killed_group_and_all(tmp_path: Path) -> None:
        sentinel = tmp_path / "sentinel"
        command = [sys.executable, "-c", _OUTLIVES_THE_BOUND_WITH_A_GRANDCHILD, str(sentinel)]
    
        code, seconds = run_live(command, BOUND_S)
    
        assert code is None
>       assert seconds < 2.0, f"a killed step should return close to its {BOUND_S}s bound, took {seconds}s"
E       AssertionError: a killed step should return close to its 0.2s bound, took 10.035902749979869s
E       assert 10.035902749979869 < 2.0

tools/tests/functional/test_quiet_timeout.py:49: AssertionError
=========================== short test summary info ============================
FAILED tools/tests/functional/test_quiet_timeout.py::test_a_captured_step_past_its_bound_is_killed_group_and_all
FAILED tools/tests/functional/test_quiet_timeout.py::test_a_live_step_past_its_bound_is_killed_group_and_all
2 failed, 1 passed in 20.13s
```

Commits since the review: 26, 25 without the Asdlc-Worker trailer.

Check passed: `uv run --quiet --project tools pytest -q tools/tests/functional/test_quiet_timeout.py` exited 0 at 2026-11-17T05:05:01+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.113 on task-1.113-server-the-race-fixture-tests-hung-for at 11f5e0ce3676:

```text
...                                                                      [100%]
3 passed in 1.89s
```

Commits since the review: 26, 25 without the Asdlc-Worker trailer.

Check FAILED: `uv run --quiet --project tools steg-quiet 'wait_child_bounded timeout control (watchdog kill removed)' -- cargo test -p steg-server --features race-fixture --test race wait_child_bounded_kills_and_reaps` exited 1 at 2026-11-17T05:05:36+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.113 on task-1.113-server-the-race-fixture-tests-hung-for at 11f5e0ce3676 with uncommitted changes:

```text
TIMED OUT wait_child_bounded timeout control (watchdog kill removed) (bound 5s, ran 5.0s)
warning: unused imports: `Signal` and `self`
  --> crates/server/tests/support/mod.rs:14:24
   |
14 | use nix::sys::signal::{self, Signal};
   |                        ^^^^  ^^^^^^
   |
   = note: `#[warn(unused_imports)]` (part of `#[warn(unused)]`) on by default

warning: unused variable: `pid`
   --> crates/server/tests/support/mod.rs:130:9
    |
130 |     let pid = Pid::from_raw(raw_pid);
    |         ^^^ help: if this is intentional, prefix it with an underscore: `_pid`
    |
    = note: `#[warn(unused_variables)]` (part of `#[warn(unused)]`) on by default

warning: `steg-server` (test "race") generated 2 warnings (run `cargo fix --test "race" -p steg-server` to apply 2 suggestions)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.09s
     Running tests/race.rs (target/debug/deps/race-d9220f423a0e57f7)

running 1 test
full output: .quiet/wait-child-bounded-timeout-control-watchdog-kill-removed.log
```

Commits since the review: 26, 25 without the Asdlc-Worker trailer.

Check passed: `cargo test -p steg-server --features race-fixture --test race wait_child_bounded_kills_and_reaps` exited 0 at 2026-11-17T05:05:55+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.113 on task-1.113-server-the-race-fixture-tests-hung-for at 11f5e0ce3676:

```text
   Compiling steg-server v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.113/crates/server)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.33s
     Running tests/race.rs (target/debug/deps/race-d9220f423a0e57f7)

running 1 test
test support::tests::wait_child_bounded_kills_and_reaps_a_child_that_outlives_the_bound ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 3 filtered out; finished in 0.31s
```

Commits since the review: 26, 25 without the Asdlc-Worker trailer.

Check passed: `just check` exited 0 at 2026-11-17T05:10:01+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.113 on task-1.113-server-the-race-fixture-tests-hung-for at 11f5e0ce3676:

```text
ok      format check (0.3s)
ok      lint (2.8s)
ok      boundaries (0.1s)
ok      foundations (0.1s)
ok      foundations public surface (0.0s)
ok      design pages (0.1s)
ok      licences (0.7s)
ok      tests (every crate) (32.2s)
ok      build (steg-sdk) (0.1s)
ok      test (steg-sdk) (0.8s)
ok      test (steg-sdk, local-socket) (1.1s)
ok      build (steg-kernel) (0.1s)
ok      test (steg-kernel) (0.4s)
ok      build (steg-server) (30.3s)
ok      test (steg-server) (28.2s)
ok      test (steg-server, race-fixture) (30.3s)
ok      test (steg-server, pty-race-fixture) (36.8s)
ok      test (steg-server, accept-backoff-fixture) (31.3s)
ok      build (steg) (1.0s)
ok      test (steg) (3.1s)
ok      test (steg, fixtures) (9.6s)
ok      build (steg-ui) (0.6s)
ok      test (steg-ui) (6.3s)
ok      build (steg-app) (0.3s)
ok      test (steg-app) (0.5s)
ok      tool tests (13.9s)
ok      architecture (0.7s)
ok      recipe names (0.1s)
ok      ui levels (0.0s)
ok      ui colours (0.1s)
```

Commits since the review: 26, 25 without the Asdlc-Worker trailer.

All four review findings fixed and verified.

1. (Bugs, Important) wait_child_bounded's pid-reuse race: fixed by detecting the child's exit through waitid(WEXITED|WNOWAIT) before ever reaping it (crates/server/tests/support/mod.rs, wait_for_exit_without_reaping) - this leaves an exited child a zombie, which still unambiguously owns its own pid (never reused) until the now-ordinary child.wait() actually reaps it afterward, so the watchdog's SIGKILL can only ever reach this process's own still-held child, whichever way the two threads interleave. Stated as the explicit invariant in the doc comment. nix::sys::wait::waitid excludes apple targets in its own cfg (checked by reading nix-0.28.0/src/sys/wait.rs) - not a real platform limit, since Darwin's own waitid(2) documents WNOWAIT and libc already binds it there - so this goes through nix::libc (nix's own public re-export, nix-0.28.0/src/lib.rs: `pub use libc;`) rather than adding a new direct dependency. Added wait_child_bounded_kills_and_reaps_a_child_that_outlives_the_bound (a real, never-exiting `sleep 30` child; confirms the function panics on its own bound, and confirms the pid is fully gone - ESRCH on a signal-0 probe - not merely reported killed). Recorded failing (watchdog's kill call removed, bounded through steg-quiet with STEG_QUIET_TIMEOUT_S=5 to turn the hang into a clean "TIMED OUT" rather than an open-ended one) and passing pairs via asdlc record-check.

2. (Evidence, Important) all four controls this task's mechanisms need are now recorded failing+passing pairs via asdlc record-check, against the code as it stands:
   - outbound.rs's close()-on-broken-write (removed/restored) - a_broken_write_closes_the_queue_so_the_readers_next_enqueue_fails
   - test_support.rs's run_bounded timeout (recv_timeout->recv, removed/restored) - a_body_that_never_returns_fails_fast_naming_what_it_waited_for, bounded through steg-quiet (STEG_QUIET_TIMEOUT_S=5) the same way
   - tools/src/steg_tools/quiet.py's killpg call (commented out/restored) - tools/tests/functional/test_quiet_timeout.py
   - wait_child_bounded's own watchdog kill (see #1)
   Each was verified failing with its mechanism removed before being restored and reverified passing, then both halves recorded.

3. (Nit) DEFAULT_TIMEOUT_S widened from 300s past the plan's own 600s to 900s: test_openpty_race.rs's own CONTROL_SAFETY_NET is 600s, and this task's own recorded run of test-openpty-race-control took 214.2s (71% of the old 300s bound) under ordinary load - a bound at or near 600s would risk steg-quiet pre-empting that test's own clear internal message with a bare "TIMED OUT". No cold Zig/ghostty-vt CI build was ever measured in this task's own history either. 900s clears the 600s internal bound with real headroom, clears every just check/build/test-all step measured several times over, and still leaves half of CI's 30-minute job limit free; test-openpty-race-control, just build, and just test-all all run through this same steg-quiet bound, not a separate one - reasoning recorded in quiet.py's own comment.

4. (Nit) AC mapping, by the task's current numbering (not the plan's older draft, which the plan text itself still has a stale "AC #4" label the reviewer correctly flagged as pre-existing - left as is, since the plan section is not touched after approval):
   - AC #1 (the step identified, or notes say what was tried): the Findings section of this task's own notes/plan - the CLOEXEC race, confirmed by reading the toolchain's std source and reproduced directly (a scratch probe, not committed).
   - AC #2 (a fixture process made to hang fails within the bound, leaves no process behind, recorded negative control): wait_child_bounded_kills_and_reaps_a_child_that_outlives_the_bound (crates/server/tests/support/mod.rs) - a real process, the exact shape AC #2 names.
   - AC #3 (just check cannot spend its whole CI limit on one hung test): tools/tests/functional/test_quiet_timeout.py's two tests prove the mechanism (Popen + killpg on a timeout) directly; this is the mechanism now wired into every just check step via steg-quiet, not a literal end-to-end just check run (that would itself take 900s+ to demonstrate and isn't a standing test).
   - AC #4 (every wait in outbound.rs/race.rs/openpty_race.rs has a bound, kills and reaps, names what it waited for; the rest is TASK-1.123's): outbound.rs's own eleven tests (AlwaysFailsToWrite + run_bounded + ClosingWriterGuard), race.rs's three wait_child_bounded call sites plus its own new test, openpty_race.rs's wait_child_bounded/run_bounded wiring. ReapOnDrop's own Drop impls in both tests files are left as plain kill()+wait() - documented inline why: wait_child_bounded panics on timeout, and panicking inside Drop during an unwind aborts the process rather than reporting one clean failure.

Checks recorded via asdlc record-check (final pass, after all fixes): cargo test -p steg-server --lib (193 passed); --features race-fixture (full crate, includes the new test); --features pty-race-fixture (full crate); just test-tools; just test-openpty-race-control (one miss, one reproduction, both recorded, consistent with its own documented variance); just check, full workspace, clean. ps checked clean of leftover processes after every run, including after the bounded failing controls. PR #69, pushed, CI green (run 44022219151, 8m48s).

Check FAILED: `uv run --quiet --project tools steg-quiet 'wait_child_bounded timeout control v2 (watchdog kill under lock, removed)' -- cargo test -p steg-server --features race-fixture --test race wait_child_bounded_kills_and_reaps` exited 1 at 2026-11-17T05:39:38+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.113 on task-1.113-server-the-race-fixture-tests-hung-for at db191fde8ced with uncommitted changes:

```text
TIMED OUT wait_child_bounded timeout control v2 (watchdog kill under lock, removed) (bound 5s, ran 5.0s)
warning: unused imports: `Signal` and `self`
  --> crates/server/tests/support/mod.rs:14:24
   |
14 | use nix::sys::signal::{self, Signal};
   |                        ^^^^  ^^^^^^
   |
   = note: `#[warn(unused_imports)]` (part of `#[warn(unused)]`) on by default

warning: unused variable: `pid`
   --> crates/server/tests/support/mod.rs:163:9
    |
163 |     let pid = Pid::from_raw(raw_pid);
    |         ^^^ help: if this is intentional, prefix it with an underscore: `_pid`
    |
    = note: `#[warn(unused_variables)]` (part of `#[warn(unused)]`) on by default

warning: `steg-server` (test "race") generated 2 warnings (run `cargo fix --test "race" -p steg-server` to apply 2 suggestions)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.08s
     Running tests/race.rs (target/debug/deps/race-d9220f423a0e57f7)

running 1 test
full output: .quiet/wait-child-bounded-timeout-control-v2-watchdog-kill-under-lock-removed.log
```

Commits since the review: 0.

Check passed: `cargo test -p steg-server --features race-fixture --test race wait_child_bounded_kills_and_reaps` exited 0 at 2026-11-17T05:39:51+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.113 on task-1.113-server-the-race-fixture-tests-hung-for at db191fde8ced with uncommitted changes:

```text
   Compiling steg-server v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.113/crates/server)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.33s
     Running tests/race.rs (target/debug/deps/race-d9220f423a0e57f7)

running 1 test
test support::tests::wait_child_bounded_kills_and_reaps_a_child_that_outlives_the_bound ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 3 filtered out; finished in 0.31s
```

Commits since the review: 0.

Check passed: `cargo test -p steg-server --features race-fixture` exited 0 at 2026-11-17T05:40:31+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.113 on task-1.113-server-the-race-fixture-tests-hung-for at db191fde8ced with uncommitted changes, last 40 of 502 lines:

```text
kill: 7334: No such process
kill: 7335: No such process
test tab_close_with_force_ends_every_program_in_scope_within_one_grace_window ... ok
kill: 7227: No such process
test a_pane_being_forced_is_unreachable_by_every_method_that_can_name_it ... ok
kill: 7257: No such process
test a_tab_being_forced_is_unreachable_by_every_method_that_can_name_it ... ok
kill: 7284: No such process
test a_session_being_forced_is_unreachable_by_every_method_that_can_name_it ... ok
kill: 7283: No such process
test a_pane_being_forced_is_unreachable_by_another_request_meanwhile ... ok

test result: ok. 30 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 5.09s

     Running tests/shutdown.rs (target/debug/deps/shutdown-c6275789232d00c3)

running 8 tests
test stop_on_a_genuinely_idle_server_returns_from_run_within_a_bound ... ok
test a_stalled_handshake_is_closed_and_run_returns_promptly_on_stop ... ok
test stop_called_twice_concurrently_on_an_idle_server_is_harmless ... ok
test stopping_closes_the_listener_removes_the_socket_and_releases_the_lock ... ok
test a_completed_idle_connection_is_closed_and_run_returns_promptly_on_stop ... ok
test stopping_while_a_trickling_handshake_is_in_progress_still_returns_within_a_bound ... ok
test many_short_connections_do_not_grow_the_tracked_set ... ok
test stop_racing_a_client_connecting_still_returns_within_a_bound ... ok

test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.14s

     Running tests/socket_mode.rs (target/debug/deps/socket_mode-5b738c682df06af0)

running 1 test
test the_socket_is_0600_even_under_a_permissive_umask ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests steg_server

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

Commits since the review: 0.

Check passed: `cargo test -p steg-server --features pty-race-fixture` exited 0 at 2026-11-17T05:41:15+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.113 on task-1.113-server-the-race-fixture-tests-hung-for at db191fde8ced with uncommitted changes, last 40 of 501 lines:

```text
kill: 9104: No such process
kill: 9105: No such process
test tab_close_with_force_ends_every_program_in_scope_within_one_grace_window ... ok
kill: 9010: No such process
test a_pane_being_forced_is_unreachable_by_every_method_that_can_name_it ... ok
kill: 9026: No such process
test a_session_being_forced_is_unreachable_by_every_method_that_can_name_it ... ok
kill: 9027: No such process
test a_pane_being_forced_is_unreachable_by_another_request_meanwhile ... ok
kill: 9032: No such process
test a_tab_being_forced_is_unreachable_by_every_method_that_can_name_it ... ok

test result: ok. 30 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 5.09s

     Running tests/shutdown.rs (target/debug/deps/shutdown-ea9d4bbecb02e2c9)

running 8 tests
test stopping_closes_the_listener_removes_the_socket_and_releases_the_lock ... ok
test stop_on_a_genuinely_idle_server_returns_from_run_within_a_bound ... ok
test stop_called_twice_concurrently_on_an_idle_server_is_harmless ... ok
test a_stalled_handshake_is_closed_and_run_returns_promptly_on_stop ... ok
test a_completed_idle_connection_is_closed_and_run_returns_promptly_on_stop ... ok
test stopping_while_a_trickling_handshake_is_in_progress_still_returns_within_a_bound ... ok
test many_short_connections_do_not_grow_the_tracked_set ... ok
test stop_racing_a_client_connecting_still_returns_within_a_bound ... ok

test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.13s

     Running tests/socket_mode.rs (target/debug/deps/socket_mode-4e1e3a14870d2373)

running 1 test
test the_socket_is_0600_even_under_a_permissive_umask ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests steg_server

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

Commits since the review: 0.

Check FAILED: `just check` exited 101 at 2026-11-17T05:42:09+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.113 on task-1.113-server-the-race-fixture-tests-hung-for at db191fde8ced with uncommitted changes, last 40 of 71 lines:

```text
test login_env::tests::capture_login_environment_falls_back_on_a_shell_that_never_finishes_in_time ... ok
test pane::attach_tests::attaching_returns_the_panes_current_screen_and_updates_follow_it ... ok
test pane::ending_tests::await_exit_is_not_delayed_by_a_backlog_on_the_workers_own_queue ... ok
test runtime::tests::an_unfiltered_subscriber_receives_nothing ... ok
kill: 10632: No such process
test runtime::stop_tests::stop_ends_a_program_that_traps_the_polite_signal ... ok
test runtime::tests::the_general_sizing_step_reaches_every_pane_through_split_close_and_swap ... ok
test test_support::tests::a_body_that_never_returns_fails_fast_naming_what_it_waited_for - should panic ... ok
test pane::ending_tests::signal_hangup_reaches_a_backgrounded_grandchild_too ... ok
test idle::tests::the_checker_blocks_with_no_timeout_while_ineligible_and_waits_exactly_once_while_eligible ... ok
test runtime::tests::a_mark_then_the_shells_own_real_exit_cascades_with_no_panic ... ok
test runtime::tests::a_shell_with_no_marks_leaves_every_status_field_unchanged_until_one_arrives ... ok
test pane::ending_tests::signal_kill_ends_a_program_that_traps_the_polite_signal ... ok
test runtime::tests::a_typed_command_that_succeeds_is_running_then_finished_and_the_tab_shows_it ... ok
test runtime::tests::a_typed_command_that_fails_is_failed_with_its_exit_code ... ok
test runtime::tests::a_typed_commands_status_reaches_pane_show_and_tab_show_through_runtime_handle ... ok
test runtime::tests::a_command_panes_own_program_printing_marks_does_not_change_its_status ... ok
test login_env::tests::capture_login_environment_prints_noise_before_the_marker_without_corrupting_the_dump ... ok
test login_env::tests::capture_login_environment_falls_back_on_a_shell_that_exits_non_zero ... ok
kill: 10628: No such process
test pane::tests::a_forward_failing_once_the_worker_is_gone_still_reaps_a_program_that_ends_on_its_own ... ok
test runtime::tests::attach_pane_on_a_busy_panes_own_worker_does_not_stall_an_unrelated_requests_lock ... ok
test login_env::tests::capture_login_environment_kills_a_backgrounded_grandchild_that_held_the_pipe_open ... FAILED

failures:

---- login_env::tests::capture_login_environment_kills_a_backgrounded_grandchild_that_held_the_pipe_open stdout ----

thread 'login_env::tests::capture_login_environment_kills_a_backgrounded_grandchild_that_held_the_pipe_open' (51357535) panicked at crates/server/src/login_env.rs:659:13:
the backgrounded grandchild's own pid file never appeared


failures:
    login_env::tests::capture_login_environment_kills_a_backgrounded_grandchild_that_held_the_pipe_open

test result: FAILED. 192 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 10.01s

error: test failed, to rerun pass `-p steg-server --lib`
full output: .quiet/tests-every-crate.log
error: recipe `test` failed with exit code 101
```

Commits since the review: 0.

Check passed: `just check` exited 0 at 2026-11-17T05:46:08+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.113 on task-1.113-server-the-race-fixture-tests-hung-for at db191fde8ced with uncommitted changes:

```text
ok      format check (0.2s)
ok      lint (0.4s)
ok      boundaries (0.1s)
ok      foundations (0.1s)
ok      foundations public surface (0.0s)
ok      design pages (0.0s)
ok      licences (0.6s)
ok      tests (every crate) (31.2s)
ok      build (steg-sdk) (0.1s)
ok      test (steg-sdk) (1.3s)
ok      test (steg-sdk, local-socket) (1.4s)
ok      build (steg-kernel) (0.1s)
ok      test (steg-kernel) (0.5s)
ok      build (steg-server) (28.5s)
ok      test (steg-server) (29.0s)
ok      test (steg-server, race-fixture) (30.4s)
ok      test (steg-server, pty-race-fixture) (36.7s)
ok      test (steg-server, accept-backoff-fixture) (31.4s)
ok      build (steg) (0.1s)
ok      test (steg) (2.1s)
ok      test (steg, fixtures) (7.0s)
ok      build (steg-ui) (0.3s)
ok      test (steg-ui) (1.1s)
ok      build (steg-app) (0.3s)
ok      test (steg-app) (0.4s)
ok      tool tests (13.6s)
ok      architecture (0.7s)
ok      recipe names (0.1s)
ok      ui levels (0.1s)
ok      ui colours (0.0s)
```

Commits since the review: 0.

Fixed the remaining finding: wait_child_bounded's watchdog and the main thread now share one Mutex<bool> (finished) for the whole decision, not two independently-timed atomics. The watchdog checks-and-kills as a single critical section under the lock; the main thread sets finished = true under that same lock strictly before calling the real child.wait() to reap. A kill can only be sent while holding the lock with finished still false; a reap can only happen after the main thread has held that same lock and set finished - so a kill can never follow a reap, whichever way the two threads interleave. Neither critical section blocks (the kill is non-blocking; waitid and the real reap both run outside the lock), so no deadlock risk; a poisoned lock is recovered (PoisonError::into_inner), not panicked on, on both sides. Rewrote the doc comment to state this invariant in exactly those terms and removed the earlier "always safe however the two threads interleave" claim, which was true in form but not in substance given the two-atomics implementation it described.

As asked: a deterministic test of the closed interleaving itself isn't practical (the window is not one a test can reliably hold open on demand) - said so in the doc comment rather than claiming one. The existing timeout test (a real, never-exiting `sleep 30` child; confirms the panic fires and the pid is fully gone - ESRCH on signal-0 - afterward) still exercises the same path unchanged.

Re-ran and re-recorded the watchdog-kill control against the edited code, both halves via asdlc record-check: ran once unrecorded first (STEG_QUIET_TIMEOUT_S=5 through steg-quiet, kill call removed) and read the output before recording - compiler warnings naming our own file/line/variable and "running 1 test", nothing from the environment - then recorded that failing run, restored the kill, and recorded the passing run.

Checks after the fix: cargo test -p steg-server --features race-fixture (full crate, includes the edited test) and --features pty-race-fixture (full crate), both green. just check, full workspace: one run hit an unrelated flake (login_env::tests::capture_login_environment_kills_a_backgrounded_grandchild_that_held_the_pipe_open, "the backgrounded grandchild's own pid file never appeared" - a timing-sensitive test in a file this task never touches; reran it alone and it passed, consistent with load on a shared machine, not a regression from this change) - recorded that run and the clean rerun both, for the record. ps checked clean of leftover processes throughout, including after the unrecorded and recorded failing control runs.

PR #69, pushed (head 174ab452), CI green (run 44026476589, 7m55s).

Check passed: `just check` exited 0 at 2026-11-17T06:10:51+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.113 on task-1.113-server-the-race-fixture-tests-hung-for at 174ab452d86d with uncommitted changes:

```text
ok      format check (0.2s)
ok      lint (31.4s)
ok      boundaries (0.1s)
ok      foundations (0.1s)
ok      foundations public surface (0.0s)
ok      design pages (0.1s)
ok      licences (0.6s)
ok      tests (every crate) (27.3s)
ok      build (steg-sdk) (0.1s)
ok      test (steg-sdk) (0.3s)
ok      test (steg-sdk, local-socket) (0.5s)
ok      build (steg-kernel) (0.1s)
ok      test (steg-kernel) (0.3s)
ok      build (steg-server) (0.1s)
ok      test (steg-server) (22.0s)
ok      test (steg-server, race-fixture) (22.9s)
ok      test (steg-server, pty-race-fixture) (30.4s)
ok      test (steg-server, accept-backoff-fixture) (22.9s)
ok      build (steg) (0.1s)
ok      test (steg) (1.3s)
ok      test (steg, fixtures) (4.3s)
ok      build (steg-ui) (0.3s)
ok      test (steg-ui) (0.4s)
ok      build (steg-app) (0.3s)
ok      test (steg-app) (0.3s)
ok      tool tests (13.4s)
ok      architecture (0.7s)
ok      recipe names (0.0s)
ok      ui levels (0.0s)
ok      ui colours (0.0s)
```

Commits since the review: 0.

Fixed the last finding: wait_for_exit_without_reaping's own doc comment still claimed the WNOWAIT invariant alone made a kill from a second thread "always safe here, however the two threads happen to interleave" - the exact sentence the previous review quoted and refuted. The prior fix rewrote wait_child_bounded's own comment to attribute safety to the shared Mutex but missed this neighbouring copy, leaving the file contradicting itself.

Rewritten to say precisely what WNOWAIT gives on its own (the pid stays owned by this process until something reaps it, so a kill sent before that reap cannot land on another process - necessary, not sufficient by itself) and that the other half - ordering the kill before the reap even when the two threads race - is wait_child_bounded's own lock, pointing to that function's comment rather than restating it.

Grepped the file for every other copy of the "always safe... however they interleave" phrasing: the one remaining instance (wait_child_bounded's own comment, "so a kill can never follow a reap, however the two threads interleave") is the true claim the Mutex actually earns, not a stale copy - left as is. Grepped the PR description (gh pr view 69 --json body): it never contained this specific phrase, so there was nothing to correct there. Correcting the record on my own prior finish notes, which can't be edited after posting: that note said the fix "removed the earlier... claim" - true of the one copy in wait_child_bounded, not of this second, separate copy in wait_for_exit_without_reaping, which this round's fix now also corrects.

No code change (doc comment only, confirmed via git diff before committing), so no control needed re-running - said so rather than claiming one.

Checks: just check, full workspace, recorded clean. ps checked clean of leftover processes.

PR #69, pushed (head 0e124c3c), CI green (run 44029370399, 7m38s).
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

created: 2026-11-17T03:37:00Z

---

2026-11-16, @bruk.o: criterion 2 narrowed to the files this task names, after the engineer accepted the split: the wider sweep is TASK-1.123, the question of a pane's program inheriting a connection is TASK-1.124.

---

author: @bruk
created: 2026-11-17T03:40:00Z

---

Plan approved by @bruk (2eb24f415089, size-2): its worker may build it.

---

author: @bruk.r2
created: 2026-11-17T04:27:00Z

---

@bruk.r2 is reviewing TASK-1.113: a same-machine review: it waited 0m with no other engineer's reviewer, longer than review_fallback_after (0m), so a fresh reviewer on its author's machine takes it.

---

author: @bruk.r2
created: 2026-11-17T04:37:00Z

---

Review by @bruk.r2 (same-machine): changes requested. Back to In Progress for the author's worker.

Findings: 2 Important, 2 nit. Cites: review, testing. The diff matches the plan.

[Bugs, Important] crates/server/tests/support/mod.rs:58-93 (wait_child_bounded, used by race.rs:153,156 and openpty_race.rs): the watchdog thread captures `pid` once, sleeps the full `bound`, then checks `finished` and sends SIGKILL with no synchronization against the main thread's `child.wait()`. `child.wait()` reaps the process (freeing its pid for OS reuse) before `finished.store(true)` runs; if the watchdog wakes in that gap it sees `finished == false` and signals the captured pid, which could by then belong to an unrelated process. The doc comment at :76-85 justifies not joining the watchdog but never addresses pid reuse. This is exactly the class of race the task's own root cause is (an unsynchronized check racing a fork/reap), reintroduced in the fix meant to guard against hangs.

[Evidence, Important] Two of the three negative controls this task's mechanisms need have no recorded failing/passing pair, and one has no control at all:
- crates/server/src/outbound.rs (a_broken_write_closes_the_queue_so_the_readers_next_enqueue_fails) and crates/server/src/test_support.rs (run_bounded's own timeout): the task's Implementation Notes say both controls were "run by hand, not committed." I repeated the outbound.rs one myself in a scratch edit (removed outbound.close() from run_writer's broken-write branch, confirmed the test fails with a clear assertion mismatch, reverted; git status clean) - it holds, but the task has no recorded pair for it, and I did not repeat run_bounded's own control.
- tools/src/steg_tools/quiet.py's process-group kill (_kill_group, tested by tools/tests/functional/test_quiet_timeout.py, the mechanism behind AC #3): no control is claimed or recorded at all. I ran one myself (disabled the killpg call, confirmed both functional tests fail - 10s instead of <2s, sentinel file written - reverted; git status clean): the mechanism is load-bearing, but the worker never performed or disclosed this control.
Per the team's testing rule, a control counts only as a recorded pair against the test as it stands; a hand-run or missing control is a finding regardless of my own after-the-fact repeats.

[Nit] tools/src/steg_tools/quiet.py:38 (DEFAULT_TIMEOUT_S = 300.0): a disclosed departure from the approved plan's stated default of 600s, with real numbers (132.2s slowest observed CI step, 109.2s slowest single-package step, 92s a cold local build) - not an undisclosed departure. But the notes concede the one case the plan named by name, a cold Zig/ghostty-vt build, was never actually measured ("cache stayed warm every time"). The nearest observed data point in this branch's own recorded checks is just-test-openpty-race-control at 214.2s (71% of the new bound) for an #[ignore]d test outside just check. Worth another look at whether 300s has real headroom for the one case it was meant to protect, though the bound is env-overridable and a false timeout is loud and recoverable.

[Nit] The approved plan's own Order of work and Proof sections label the quiet.py step-bound work "AC #4" when it is actually AC #3 by the task's own numbering - a pre-existing plan quirk, not introduced by this build, but worth a one-line fix for a future reader.

What I did not check: did not run `just check` myself (ran targeted cargo test / pytest for the controls instead, within the 40-minute budget); did not repeat run_bounded's own recv_timeout-vs-recv() control; did not read portable_pty's spawn path (correctly out of scope - TASK-1.124).

---

author: @bruk.r2
created: 2026-11-17T05:21:00Z

---

@bruk.r2 is reviewing TASK-1.113: a same-machine review: it waited 0m with no other engineer's reviewer, longer than review_fallback_after (0m), so a fresh reviewer on its author's machine takes it.

---

author: @bruk.r2
created: 2026-11-17T05:34:00Z

---

Review by @bruk.r2 (same-machine): changes requested. Back to In Progress for the author's worker.

Findings: 1 Important, 1 nit. Cites: review. The diff does not match the plan.

[Bugs, Important] crates/server/tests/support/mod.rs:136-140 vs :144-149 (watchdog closure vs the main thread's detected.store+reap), and the invariant claimed at :59-68: wait_child_bounded's watchdog checks `detected` (:138) and acts on it - `timed_out.store` + `signal::kill` (:139-140) - as two separate, unsynchronized operations against the main thread's own `detected.store(true)` (:145) then `child.wait()` reap (:147-149). The doc comment at :59-68 claims this makes a kill from the watchdog "always safe here, however the two threads happen to interleave" - that is false on at least one interleaving: the watchdog's load at :138 can read `false` and then be descheduled before reaching :140; in that window the main thread can finish `wait_for_exit_without_reaping`, store `detected=true`, and run the real `child.wait()` reap to completion; when the watchdog resumes it still fires `signal::kill(pid, SIGKILL)` at :140 against a pid this process no longer owns, which the OS could by then have reassigned to an unrelated process. This is the same class of race the first review flagged (an unsynchronized check racing a reap), narrowed from "any real reap" to this specific check-then-act gap, not eliminated by reordering the store before the reap. A real fix needs the decision and the kill to share one lock with the reap (e.g. a Mutex the watchdog holds while checking-and-killing and the main thread holds while reaping-and-marking), not two independently-timed atomics. I did not fault the unsafe/libc machinery itself: nix 0.28.0's own `sys::wait::waitid` cfg excludes apple_targets (confirmed reading its source, locked in Cargo.lock), libc 0.2.191 (also locked) defines `waitid`/`P_PID`/`WEXITED`/`WNOWAIT`/`id_t` for apple with a signature matching the call site, the `unsafe` blocks' own reasoning (zeroed siginfo_t is a valid bit pattern, pointer validity, EINTR retry, return value checked) holds, and placing this in its own dedicated tests/support module behind a safe function matches rust.md's "one small module per foreign interface, behind a safe API."

[Nit] tools/src/steg_tools/quiet.py:47 (DEFAULT_TIMEOUT_S = 900.0): a further disclosed departure from the plan's 600s (already a departure from the first shipped 300s, which review #1 flagged as a Nit). The reasoning checks out: test-openpty-race-control's own CONTROL_SAFETY_NET really is 600s (tests/openpty_race.rs:93), and every step `just check` depends on (check-fmt, lint, check-boundaries, check-foundations, check-foundations-public-surface, check-design, deny, test/check-packages, test-tools, check-architecture, check-recipes, check-ui-levels, check-ui-colours) routes through `{{quiet}}` in the justfile, whose bound is `STEG_QUIET_TIMEOUT_S` or this same default (verified in quiet.py's timeout_seconds()). On criterion 3's own arithmetic: this branch's own recorded CI runs were 6m21s and 8m48s (528s); one step consuming its full 900s bound in place of its own normal ~30s contribution gives roughly 528-30+900 = ~1398s (~23.3 min) against the 30-minute job limit - it fits, with ~6-7 minutes to spare, given the hang's historical position mid-suite (the race/pty-race/accept-backoff cluster) rather than after every other step. Nit, not Important: disclosed with real numbers, and a loose bound's false TIMED OUT is louder and more recoverable than a tight one's pre-emption of a test's own message.

Evidence, checked and satisfied (no finding): all four negative-control pairs this task's mechanisms need (outbound.rs's close-on-broken-write, test_support.rs's run_bounded timeout, quiet.py's killpg, and support/mod.rs's watchdog kill) are recorded via asdlc record-check between 2026-11-17T05:03:28 and 05:05:55 UTC, strictly after the fix commit 0ee7df1a (05:01:53 UTC) that touches both files, and no later commit before the branch head (db191fde) edits either file again (checked via git log). Each failing half failed for its stated reason (an assertion mismatch for outbound.rs; two assertion failures on elapsed time/sentinel for quiet.py's functional test; a TIMED OUT via steg-quiet, the sanctioned way to bound a hang, for both run_bounded's and wait_child_bounded's own controls), not an unrelated one. I repeated the watchdog-kill control myself in the review worktree: commented out `signal::kill` at mod.rs:140, ran `cargo test -p steg-server --features race-fixture --test race wait_child_bounded_kills_and_reaps` under steg-quiet's default 900s bound - it failed (the disabled path now blocks for the real child's full 30s lifetime instead of the 300ms bound before panicking on its own elapsed-time assertion), confirming the kill is load-bearing. Reverted; `git status --short` clean; `ps` showed no leftover sleep/race process.

What I did not check: did not run `just check` myself (relied on this head's CI green at db191fde plus the targeted cargo/pytest runs above, within the review's own time budget); did not re-derive whether a true mutex-based fix would itself introduce new deadlock risk (out of scope - this review only judges the diff as submitted); did not read portable_pty's spawn path (TASK-1.124's own scope, not this task's).

---

author: @bruk.r2
created: 2026-11-17T05:55:00Z

---

@bruk.r2 is reviewing TASK-1.113: a same-machine review: it waited 0m with no other engineer's reviewer, longer than review_fallback_after (0m), so a fresh reviewer on its author's machine takes it.

---

author: @bruk.r2
created: 2026-11-17T06:04:00Z

---

Review by @bruk.r2 (same-machine): changes requested. Back to In Progress for the author's worker.

Findings: 1 Important, 0 nit. Cites: review. The diff matches the plan.

Scope: crates/server/tests/support/mod.rs only (db191fde..174ab452), per the brief.

[Bugs, Important] crates/server/tests/support/mod.rs:59-68 (doc comment on wait_for_exit_without_reaping): still claims the WNOWAIT invariant alone is "the one invariant that makes a kill from a second thread always safe here, however the two threads happen to interleave" - the exact sentence review #8 quoted and refuted (a kill with no synchronizing lock can still hit a reaped/reused pid). This edit correctly rewrote wait_child_bounded's own doc comment (:113-160) to attribute safety to the shared Mutex, but left this neighboring sentence untouched, so the file now contradicts itself: :59-68 says WNOWAIT by itself suffices "however the two threads interleave," while :128-140 says the invariant depends on the Mutex serialising lock acquisitions. A reader taking :59-68 at face value would conclude the Mutex is redundant. The build notes say the false claim "removed the earlier ... claim" - true of one copy, not this one.

Question 1 (the invariant itself), checked, no finding: walked both lock orders at :161-201. The watchdog's check-and-kill is one critical section under `finished`'s lock; the main thread sets finished=true under the same lock strictly before the real child.wait() reap at :191-193. A kill can only be sent while holding the lock with finished false; a reap can only follow the main thread having held that same lock and set it true - so a kill can never follow a reap on either lock order. Nothing blocking runs under the lock (kill is non-blocking; waitid and wait() both run outside it) - no deadlock risk. A poisoned lock is recovered via PoisonError::into_inner on both sides, not panicked on; neither critical section itself can panic. On timeout, child.wait() still reaps unconditionally before the trailing assert, which names label and bound (:196-199) - nothing left behind.

Evidence, checked, no finding: the recorded pair "wait_child_bounded timeout control v2 (watchdog kill under lock, removed)" (05:39:38 FAILED / 05:39:51 passed, both "at db191fde8ced with uncommitted changes") ran against this Mutex-fixed code, not the pre-fix atomics version: the captured compiler warning at mod.rs:163 ("unused variable: pid") matches this file's current line numbers exactly, where the pre-fix (atomics) version's doc comment was ~25 lines shorter and would have placed that line near :133. The pair postdates review #8 (05:34) and sits on the worktree immediately before the fix commit (db7c7507091e, 2026-11-17 00:46 local), confirmed via git log db191fde..174ab452 -- crates/server/tests/support/mod.rs. I also repeated this control myself in the review worktree (commented out the SIGKILL call, ran the test under an external 8s bound): it hung past the bound with no further output, confirming the kill is still load-bearing. Reverted; git status clean; ps showed no leftover sleep/race process.

Question 4 (happy path), checked, no finding: cargo test -p steg-server --features race-fixture --test race: 4 tests passed in 0.31s, including eight_simultaneous_starts_leave_exactly_one_server (the test that regressed to 80s before the join-without-a-bound issue was caught) - no regression.

What I did not check: did not run just check myself (relied on CI green at head 174ab452 plus the targeted cargo runs above, within the review's time budget); did not re-derive whether the Mutex design could deadlock against any other lock in this module (none exists) beyond what's checked above; did not look outside crates/server/tests/support/mod.rs per the brief's scope.

---

author: @bruk.r2
created: 2026-11-17T06:19:00Z

---

@bruk.r2 is reviewing TASK-1.113: a same-machine review: it waited 0m with no other engineer's reviewer, longer than review_fallback_after (0m), so a fresh reviewer on its author's machine takes it.

---

author: @bruk.r2
created: 2026-11-17T06:22:00Z

---

Review by @bruk.r2 (same-machine): clean. It stays In Review for a person to merge.

Findings: 0 Important, 0 nit. The diff matches the plan.

Fourth review (head 0e124c3c, CI green). Scope: diff 174ab452..0e124c3c against crates/tools/justfile is one file, 23 lines, crates/server/tests/support/mod.rs - confirmed myself that every changed line is a /// doc comment line; no executable line changed. (Head 0e124c3c is a merge of origin/main into the branch; confirmed by diffing the full tree, not just crates/tools/justfile, that the merge brought in nothing beyond the backlog task file and this same doc comment - no code from main landed unseen.)

The rewritten comment on wait_for_exit_without_reaping now says WNOWAIT alone is necessary, not sufficient: it keeps an exited child as a zombie that still owns its pid until something reaps it, so a kill sent before that reap cannot land on another process - but it says nothing about the ordering between a concurrent kill decision and the reap. It correctly points to wait_child_bounded's own comment for that half. Walked wait_child_bounded's comment and the code it describes (:168-196): the kill decision and the finished check happen in one critical section under the Mutex (:174-180), and the calling thread sets finished=true under that same lock (:187-188) strictly before releasing it and reaping via the real child.wait() (:192-196). The one remaining "however the two threads interleave" sentence (:140-141) is this lock-earned claim, not the old unqualified one - no sentence in the file still contradicts the Mutex invariant.

Evidence: no control was re-run. I rely on that being correct because the only change since 174ab452 is this doc comment (confirmed above) plus the backlog task file, and CI is green at 0e124c3c; I did not run just check or any test myself. The task's notes for this round correct the earlier round's note, which had claimed the false sentence was removed outright - true only of the copy in wait_child_bounded's own comment, not of this second, separate copy in wait_for_exit_without_reaping. That correction is honest and accurately scoped.

No findings. Filed no rule-change draft - nothing to draft against an earlier review's finding.

---
<!-- COMMENTS:END -->
