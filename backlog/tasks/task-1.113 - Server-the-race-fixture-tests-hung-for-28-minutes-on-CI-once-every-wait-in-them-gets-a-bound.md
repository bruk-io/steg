---
id: TASK-1.113
title: >-
  Server: the race-fixture tests hung for 28 minutes on CI once; every wait in
  them gets a bound
status: In Progress
assignee:
  - '@bruk.w9'
created_date: '2026-11-16T05:42:00Z'
updated_date: '2026-11-17T04:19:00Z'
labels:
  - size-2
milestone: m-3
dependencies: []
parent_task_id: TASK-1
project: bruk-io-steg
asdlc_accepted_from: DRAFT-2
asdlc_branch: task-1.113-server-the-race-fixture-tests-hung-for
asdlc_plan_approved_by: bruk
asdlc_plan_approved_hash: '2eb24f415089'
asdlc_plan_approved_size: size-2
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
<!-- COMMENTS:END -->
