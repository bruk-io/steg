---
id: TASK-1.84
title: >-
  Server: pass a typed command's start and end from a pane's terminal to the
  kernel
status: In Review
assignee:
  - '@bruk.w1'
created_date: '2026-11-15T21:43:00Z'
updated_date: '2026-11-16T01:28:00Z'
labels:
  - size-2
milestone: m-4
dependencies:
  - TASK-1.79
  - TASK-1.76
  - TASK-1.83
references:
  - task-1.84-server-pass-a-typed-command-s-start-and
  - https://github.com/bruk-io/steg/pull/46
parent_task_id: TASK-1
project: bruk-io-steg
asdlc_accepted_from: DRAFT-3
asdlc_branch: task-1.84-server-pass-a-typed-command-s-start-and
asdlc_plan_approved_by: bruk
asdlc_plan_approved_hash: '33f0386d0f53'
asdlc_plan_approved_size: size-2
asdlc_review_kind: same-machine
asdlc_review_findings: 'important=1,nit=1'
asdlc_review_policies: ["review"]
asdlc_plan_matched: no
asdlc_reviewer: bruk.r2
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Piece three of the split of TASK-1.79 (2026-11-15). Depends on the kernel piece being accepted and built as well; add that dependency when the drafts have ids. doc-11 R50, R82; doc-16 (the server is where I/O meets the kernel).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 When a pane's terminal reports a prompt-marked command starting or ending, the server passes it to the kernel with a clock reading as a Moment, through the same route a program's exit already takes (TASK-1.76's Runtime::apply_event), in an order in which a mark is never applied after that pane's own exit; the kernel's status for the pane and the tab's shown status reflect it, read through the kernel. Reporting that status over the API (pane show, tab show, tab list) is the follow-up task's
- [ ] #2 Shown end to end with a real shell in a real pty that emits the marks: a command that succeeds, one that fails, and one still running; and a shell that emits none, where nothing changes
- [ ] #3 No child is left unreaped and no thread running by anything this adds, and the tests stop the runtime they start
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
This was two reviewable changes; the split is now the board's own (criterion 1 ends at the kernel, reporting it over the API is TASK-1.89). §1-§4 below build the kernel wiring criteria 1-3 ask for, unchanged from the last round. §5 records why the wire-level half stays out, now as a settled fact rather than a question. §6 is rewritten per review: real negative controls (a scratch edit that removes the mechanism, confirmed to fail, then restored - not two tests compared to each other), two new ordering tests, and an explicit `Tree::tab_shown_status` assertion for criterion 1's own wording.

## 1. Where marks are drained, and why that ordering is safe

Drained inside `crates/server/src/pane.rs`'s `worker_loop`, in the `PaneCommand::PtyBytes(bytes)` arm, immediately after `terminal.write(&bytes)` - and the same way after `terminal.resize(...)` in the `Resize` arm, since `Terminal::resize` also calls `drain_events` internally and can queue a mark exactly as `write` does (mirrors the kernel's own stated reason for draining after every event-producing call, `terminal.rs`'s doc on `drain_events`). One small helper, `fn drain_command_marks(terminal: &mut Terminal, pane: PaneId, events: &Sender<PaneEvent>)`, called from both arms, turns each `CommandMark::OutputStarted`/`CommandMark::Ended { exit_code }` `take_command_marks()` returns into a `PaneEvent::CommandOutputStarted { pane }` / `PaneEvent::CommandEnded { pane, exit_code }` and sends it on the same `events: Sender<PaneEvent>` `PaneEvent::Exited` already uses - not a new channel, and not a new `PaneCommand::TakeMarks(reply)` polled from `Runtime` (the shape `Cwd` uses, and the rejected alternative): that would create a second path with no ordering relationship to `Exited` at all, which is exactly the race criteria 1/3 rule out.

The ordering proof is the channel, not a lock: `worker_loop`'s own single thread is the one sender of every `PaneEvent` for a pane (both the marks this task adds and the `Exited` TASK-1.76 already sends), and `spawn_relay` only ever enqueues `PtyBytes` while reading to EOF and sends `Exited` strictly after, once `child.wait()` returns - so the `Exited` for a pane's last run is always the last `PaneCommand` that run's relay thread submits, and the worker drains any mark from a `PtyBytes` arm before it ever reaches a later `Exited` arm. mpsc is FIFO per sender, so the drain thread on the other end (`Runtime`'s own, in `runtime.rs`) receives and applies them in that same order. No change to locking, to `spawn_relay`, or to how `Exited` itself is produced. §6's new ordering tests exercise this directly, not just by argument.

One line on reachability: the drain-after-resize call is not exercised by any test this plan adds. `Terminal::resize` can only ever queue `Event::PtyWrite` (an in-band size report ack) through `drain_events`'s own match - it has no path to `Event::SemanticPrompt`, which only ever comes from VT-parsing a program's *written* bytes, never from a resize. It is still right to add: the same reasoning `terminal.rs` already states for `write` ("every engine call that can queue an event drains afterwards rather than leaving it to whichever write happens to come next") applies verbatim to `resize`, at zero cost, and a test asserting `take_command_marks()` stays empty after a resize would prove nothing no present-day resize already doesn't (every existing kernel-level resize test already incidentally confirms this by never finding a mark). Named here as unreachable-but-right, not hidden.

## 2. New `PaneEvent` variants (`crates/server/src/pane.rs`)

```rust
pub enum PaneEvent {
    Exited { pane: PaneId, exit_code: i32 },
    CommandOutputStarted { pane: PaneId },
    CommandEnded { pane: PaneId, exit_code: Option<i32> },
}
```
Per-mark variants, not `Vec<CommandMark>` on one variant: `PaneEvent` derives `Copy` today, which a `Vec` would break, and the fields read consistently against the existing `Exited { pane, exit_code }`. Import `steg_kernel::CommandMark` alongside the existing `steg_kernel::Terminal` import.

## 3. Applying them (`crates/server/src/runtime.rs`, `apply_event_locked`)

Two new match arms, same shape as `Exited`'s, each computing `now` the same way `Exited` already does (`Moment::new(Instant::now().duration_since(state.clock_origin))` - one clock read per event, in the one place that already does this, not stamped earlier in the worker thread, which would need `clock_origin` duplicated into the worker and a second clock reader for no benefit, since the drain is not a queue that backs up in practice):

- `CommandOutputStarted { pane }` -> `state.tree.note_command_output_started(pane)`. `Ok(Started)` and `Ok(NotAShellPane)` both need nothing further today - no client event stream exists yet (R52 is M2, TASK-1.46's own gap), so there is nothing to forward either outcome to; say so in the arm, not just in this plan.
- `CommandEnded { pane, exit_code }` -> `state.tree.note_command_ended(pane, exit_code, now)`. `Ok(Ended(_))`, `Ok(EndedWithNoExitCode)` and `Ok(Ignored)` likewise need nothing further today, same reason.
- `Err(Error::PaneNotFound(_))` on either -> **do nothing to `state.panes`.** This is the one place this plan must not copy the `Exited` arm's shape: `Exited`'s `PaneNotFound` arm removes the stale `PaneHandle` because an exit means the handle is pointless. A mark's `PaneNotFound` means the opposite - the tree lost the pane (a forced close racing the mark) while the program the `PaneHandle` still owns may well be running. Removing it would drop the `Sender` clone inside the handle, which nothing else holds, ending the worker loop and leaking the relay thread and an unkilled child - worse than the orphan being guarded against, and a direct criterion 3 violation. So: match it, do nothing, move on. §6's new "mark after close" test proves this directly.
- `Err(other)` -> `unreachable!`, with a message naming that `Tree::note_command_output_started`/`note_command_ended` document only `Error::PaneNotFound` as a possible error (`tree.rs`'s own doc comments on both), not the `ProgramNotStarted` reasoning `Exited`'s own `unreachable!` gives - a different method, a different documented error set, so copying that message would misstate why the arm is unreachable.

## 4. Doc fix this task's own addition falsifies

`crates/kernel/src/terminal.rs`'s `command_marks` field doc (the paragraph starting "Unbounded until a caller exists") says "nothing in this task calls `take_command_marks`, so in the repository's shipped state after this task merges, this grows for the whole life of any pane." This task is that caller. Update the paragraph to say `crates/server/src/pane.rs`'s `worker_loop` now drains it after every `write`/`resize`, so in practice it holds at most the marks from one pty read or one resize's own in-band report before the next drain - named, not left stale (`.claude/rules/code-design.md`: what the code says must be true).

## 5. What this task does not build (now TASK-1.89, settled on the board)

Criterion 1's own wording ends at the kernel: "the kernel's status for the pane and the tab's shown status reflect it, read through the kernel" - no wire clause left in it to satisfy here. Reporting that status over the API (`pane.show`, `tab.show`, `tab.list`) is TASK-1.89, with the second-field recommendation (`PaneRecord.display_status: Option<steg_sdk::Status>`, alongside `status`, never replacing its running/ended meaning - the same reasoning `crates/kernel/src/pane.rs`'s own doc gives for keeping `program_status`/`status` apart) carried onto it rather than decided here. This task still touches none of `crates/server/src/dispatch/pane.rs`'s `show()`, `crates/server/src/dispatch/tab.rs`, `steg_sdk::PaneRecord`, or `dispatch::dispatch`'s signature (`crates/server/src/dispatch/mod.rs`, TASK-1.45's file right now) - avoiding that overlap is still true, just no longer a decision this plan is asking for.

## 6. Proof (criteria 1-3)

New tests in `crates/server/src/runtime.rs`'s `#[cfg(test)] mod tests` (not `tests/`: reading `Pane::status(now)`/`Tree::tab_shown_status` needs a `Moment` built from the private `State::clock_origin`, which only in-crate tests can reach) - each drives a real `Runtime` with a real pty program that prints real OSC 133 sequences directly (`sh -c 'printf "\033]133;C\007"; ...; printf "\033]133;D;0\007"'` or similar) - no shell integration exists yet (that is TASK-1.85's own job), so the test program is its own stand-in for a shell that already has it. Every wait is bounded, polling `Pane::status(now)` (never a bare `recv`/`sleep` loop with no bound), the same shape this file's existing tests already use, and every test calls `runtime.stop()` (or lets `Drop` run it) before finishing, proving criterion 3 - stated once, in a shared comment: this change adds no new thread and no new process; marks ride the existing `events` channel and the existing drain thread `Runtime::new` already starts.

### The four shapes (criterion 2)

- **A command that succeeds**: emits `C`, then `D;0`. Asserts `Pane::status(now)` becomes `Running` after the `C`, then `Finished` after the `D`, and (criterion 1's own wording) `Tree::tab_shown_status(tab, now)` returns `Some(pane)` once `Finished` - this is the one test that carries the tab-level assertion, since it is the simplest single-pane case to read it against.
- **A command that fails**: emits `C`, then `D;3` (or similar). Asserts `Failed { exit_code: 3 }`.
- **A command still running**: emits `C` only, no `D` yet. Asserts `Running` while waiting, then sends the `D` itself (reusing the same pane) and asserts the status changes away from `Running` - this second half is what makes the first assertion not vacuous (see the controls below).
- **A shell that emits no marks at all**: a plain program with no OSC 133 output. Asserts `Pane::program_status()`, `Pane::status(now)` and `Pane::next_status_deadline(now)` are all unchanged (program status stays `Running`, display status stays `None`, deadline stays `None`) across a bounded wait.

### Negative controls (real ones: remove the mechanism, confirm the test fails, restore it)

Each of these is run once by hand while writing the tests, not left in the suite, and named on the task's notes when recorded:

- **(a) Drain removed.** Comment out the call to `drain_command_marks` after `terminal.write` (leave the one after `resize` alone, since no test exercises it per §1's own note). Confirm: the succeeds, fails, and still-running tests all fail - every one of them stays at whatever `Pane::status` was before the first mark (`None`, since none of these panes' shells have started a command yet), because nothing ever reaches `apply_event_locked` to change it. Restore the call.
- **(b) `CommandEnded` arm made a no-op.** Temporarily replace its body with nothing (keep `CommandOutputStarted`'s arm as-is). Confirm: the succeeds test fails (status stays `Running`, never reaches `Finished`) and the fails test fails the same way (never reaches `Failed`), while the still-running test still passes up to its first assertion (it only depends on `CommandOutputStarted` landing) - which is exactly why the still-running test's own second half (sending the `D` and asserting the change) is in scope: without it, this one test could not tell "the arm works" from "the arm was never reached", the gap a control is for. Restore the arm.
- **(c) No-marks test's own control.** Named, not assumed: within that same test, after the bounded wait where nothing changed, send one real `C` mark into the same pane (reusing the harness's own send-bytes path) and assert `Pane::status(now)` does become `Some(Running)` - proving the preceding "nothing changed" assertions were reading a mechanism that can change, not one that is simply never wired to anything in this test's own path.

### Two orderings criterion 1 argues but did not yet have a test for

- **A typed command running when the shell itself exits** (mark then a real exit, same pane, in that order): the test program emits `C` and then the process itself ends (for example `exit 7` right after the `printf`), so the real channel carries `CommandOutputStarted` then `Exited` for the same pane, not a hand-built sequence. Asserts: the pane's own `closes_on_exit()` cascade still runs exactly as `apply_event_closes_a_shell_pane_through_the_same_cascade_as_a_manual_close` already proves for a plain exit (`tree.sessions()` empty), no panic, and the `PaneHandle` is gone from `state.panes` - proving the mark does not leave anything behind that the following `Exited` has to clean up around.
- **A mark for a pane the tree no longer holds** (the `PaneNotFound` arm, §3's own trap): same shape as the existing `apply_event_on_a_pane_the_tree_no_longer_holds_removes_the_stale_handle_without_panicking` test - a `PaneHandle` for a real, still-running program, inserted into `state.panes` under an id the tree was never given (standing in for a forced close that has already happened, the way that existing test's own comment already reads it, until TASK-1.78's own forced-close lands). Sends `PaneEvent::CommandOutputStarted`/`CommandEnded` for that id directly through `apply_event_locked` and asserts: no panic, and - unlike that sibling test - the handle is *still in* `state.panes` afterward, because §3 says a mark must not remove it. The test itself then calls the handle's own `.stop()` to end the real process it started, standing in for the same cleanup path that existing sibling test's own `Exited`-driven removal gives a real orphan once its program actually exits on its own, or that `Runtime::stop` gives every pane when the whole server stops - either real path ends the process this synthetic one only has to tidy up by hand.
- **A command pane whose own program happens to print OSC 133** (not a shell pane): starts a `Program::Command` pane whose program prints `C` then exits 0. Asserts `Pane::status(now)` is `Running` throughout and becomes `Finished` only once the program actually exits - driven by `record_exit`, never by the mark - matching the kernel's own `note_command_output_started_on_a_command_pane_is_ignored`/`note_command_ended_on_a_command_pane_is_ignored` (already proven at the `Pane`/`Tree` level by TASK-1.83) end to end through a real `Runtime`, so the `NotAShellPane`/`Ignored` arms in `apply_event_locked` are proven not just to compile but to leave a command pane's own status exactly alone.

## Risks / overlap

- **TASK-1.78** (`pane.rs`, `pty.rs`, `runtime.rs`; a rewritten `Runtime::stop` and the close paths) - real overlap, different regions of the same files. This plan touches only: two new `PaneEvent` variants, one new helper function, two call sites of that helper (`PtyBytes`/`Resize` arms), and two new match arms in `apply_event_locked`. It does not touch `stop`, `Stop`, `shutdown`, or `spawn_relay` at all - whichever of the two tasks merges second rebases past the other's additions, not a structural conflict.
- **TASK-1.45** (`dispatch/mod.rs`, `listen.rs`, `connection.rs`, `lookup.rs`, `ServerConfig`) - no overlap: `dispatch::dispatch`'s signature is left untouched.
- **TASK-1.77** (`crates/steg`, one detach function) - no overlap.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Carried from TASK-1.83's planning (2026-11-15): pane.show's wire field 'status' is built today from the program's status (running or ended), in crates/server/src/dispatch/pane.rs, not from the display status the kernel now keeps (running, finished, failed, with its ending rules). This task is where the server first has a clock reading to ask the kernel for the display status, so its plan says what pane show and the tab's shown status report over the API, against doc-2 and doc-11, and whether the field's meaning changes or a second field is needed; name any description change for the engineer.

Plan checked against doc-11 (R50, R82, R83), decision-7, doc-15 and doc-16 by @bruk.o: ok. Marks are drained on the pane's own worker after each write to its terminal and sent as events on the channel the pane's exit already uses, so one sender orders a mark before that pane's exit; the runtime applies each under its lock with one clock reading as a Moment; a mark for a pane the tree no longer holds does nothing and leaves the handle to the path that closes the pane. Nothing is reported over the API here (TASK-1.89). Proof is with real programs in real ptys printing the marks, with three negative controls that remove the mechanism. One point for the build, not the plan: the drain after a resize cannot be reached (a resize queues no mark) and so cannot be tested; leave it out, since any mark is drained by the next write.

Check passed: `just check` exited 0 at 2026-11-16T00:41:52+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.84 on task-1.84-server-pass-a-typed-command-s-start-and at 75387af8c49a:

```text
ok      format check (0.2s)
ok      lint (83.1s)
ok      boundaries (0.2s)
ok      licences (0.7s)
skip    tests: this change touches no crate
ok      build (steg-sdk) (3.0s)
ok      test (steg-sdk) (1.7s)
ok      test (steg-sdk, local-socket) (2.0s)
ok      build (steg-kernel) (41.0s)
ok      test (steg-kernel) (1.4s)
ok      build (steg-server) (42.5s)
ok      test (steg-server) (6.2s)
ok      test (steg-server, race-fixture) (6.5s)
ok      build (steg) (0.2s)
ok      test (steg) (0.3s)
ok      build (steg-app) (18.5s)
ok      test (steg-app) (0.7s)
ok      tool tests (1.7s)
ok      architecture (1.0s)
ok      recipe names (0.1s)
```

Check passed: `sh -c 'cargo check -p steg-server --tests 2>&1 | tail -30'` exited 0 at 2026-11-16T00:47:54+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.84 on task-1.84-server-pass-a-typed-command-s-start-and at 75387af8c49a with uncommitted changes:

```text
    Checking steg-server v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.84/crates/server)
error[E0004]: non-exhaustive patterns: `PaneEvent::CommandOutputStarted { .. }` and `PaneEvent::CommandEnded { .. }` not covered
   --> crates/server/src/runtime.rs:543:11
    |
543 |     match event {
    |           ^^^^^ patterns `PaneEvent::CommandOutputStarted { .. }` and `PaneEvent::CommandEnded { .. }` not covered
    |
note: `PaneEvent` defined here
   --> crates/server/src/pane.rs:113:10
    |
113 | pub enum PaneEvent {
    |          ^^^^^^^^^
...
123 |     CommandOutputStarted {
    |     -------------------- not covered
...
130 |     CommandEnded {
    |     ------------ not covered
    = note: the matched value is of type `PaneEvent`
help: ensure that all possible cases are being handled by adding a match arm with a wildcard pattern, a match arm with multiple or-patterns as shown, or multiple match arms
    |
571 ~         },
572 +         PaneEvent::CommandOutputStarted { .. } | PaneEvent::CommandEnded { .. } => todo!()
    |

For more information about this error, try `rustc --explain E0004`.
error: could not compile `steg-server` (lib) due to 1 previous error
warning: build failed, waiting for other jobs to finish...
error: could not compile `steg-server` (lib test) due to 1 previous error
```

Check FAILED: `cargo check -p steg-server --tests` exited 101 at 2026-11-16T00:48:03+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.84 on task-1.84-server-pass-a-typed-command-s-start-and at 75387af8c49a with uncommitted changes:

```text
    Checking steg-server v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.84/crates/server)
error[E0004]: non-exhaustive patterns: `PaneEvent::CommandOutputStarted { .. }` and `PaneEvent::CommandEnded { .. }` not covered
   --> crates/server/src/runtime.rs:543:11
    |
543 |     match event {
    |           ^^^^^ patterns `PaneEvent::CommandOutputStarted { .. }` and `PaneEvent::CommandEnded { .. }` not covered
    |
note: `PaneEvent` defined here
   --> crates/server/src/pane.rs:113:10
    |
113 | pub enum PaneEvent {
    |          ^^^^^^^^^
...
123 |     CommandOutputStarted {
    |     -------------------- not covered
...
130 |     CommandEnded {
    |     ------------ not covered
    = note: the matched value is of type `PaneEvent`
help: ensure that all possible cases are being handled by adding a match arm with a wildcard pattern, a match arm with multiple or-patterns as shown, or multiple match arms
    |
571 ~         },
572 +         PaneEvent::CommandOutputStarted { .. } | PaneEvent::CommandEnded { .. } => todo!()
    |

For more information about this error, try `rustc --explain E0004`.
error: could not compile `steg-server` (lib) due to 1 previous error
warning: build failed, waiting for other jobs to finish...
error: could not compile `steg-server` (lib test) due to 1 previous error
```

Check passed: `cargo test -p steg-server --lib runtime::tests` exited 0 at 2026-11-16T00:49:42+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.84 on task-1.84-server-pass-a-typed-command-s-start-and at 75387af8c49a with uncommitted changes:

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.08s
     Running unittests src/lib.rs (target/debug/deps/steg_server-75c81fd969602d6a)

running 18 tests
test runtime::tests::apply_event_records_an_exit_and_leaves_a_command_pane_in_place ... ok
test runtime::tests::apply_event_closes_a_shell_pane_through_the_same_cascade_as_a_manual_close ... ok
test runtime::tests::login_shell_from_process_env_never_panics_and_is_never_empty ... ok
test runtime::tests::runtime_new_does_not_panic_and_starts_empty ... ok
test runtime::tests::a_mark_for_a_pane_the_tree_no_longer_holds_does_nothing_and_does_not_panic ... ok
test runtime::tests::apply_event_on_a_pane_the_tree_no_longer_holds_removes_the_stale_handle_without_panicking ... ok
test runtime::tests::a_real_exit_event_drives_the_same_cascade_apply_event_runs ... ok
test runtime::tests::apply_size_changed_lets_a_pane_go_under_the_floor_when_the_kernel_says_so ... ok
test runtime::tests::apply_size_changed_resizes_only_the_panes_the_change_names ... ok
test runtime::tests::pane_split_falls_back_to_home_when_the_inherited_live_cwd_no_longer_exists ... ok
test runtime::tests::pane_split_inherits_the_live_cwd_of_the_pane_it_split_from_not_its_started_in_cwd ... ok
test runtime::tests::a_typed_command_still_running_stays_running_until_told_otherwise ... ok
test runtime::tests::the_general_sizing_step_reaches_every_pane_through_split_close_and_swap ... ok
test runtime::tests::a_command_panes_own_program_printing_marks_does_not_change_its_status ... ok
test runtime::tests::a_mark_then_the_shells_own_real_exit_cascades_with_no_panic ... ok
test runtime::tests::a_shell_with_no_marks_leaves_every_status_field_unchanged_until_one_arrives ... ok
test runtime::tests::a_typed_command_that_fails_is_failed_with_its_exit_code ... ok
test runtime::tests::a_typed_command_that_succeeds_is_running_then_finished_and_the_tab_shows_it ... ok

test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 72 filtered out; finished in 0.55s
```

Check FAILED: `cargo test -p steg-server --lib a_typed_command` exited 101 at 2026-11-16T00:50:50+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.84 on task-1.84-server-pass-a-typed-command-s-start-and at 75387af8c49a with uncommitted changes, last 40 of 44 lines:

```text
    |    ^^^^^^^^^^^^^^^^^^^
    |
    = note: `#[warn(dead_code)]` (part of `#[warn(unused)]`) on by default

warning: `steg-server` (lib test) generated 1 warning
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.09s
     Running unittests src/lib.rs (target/debug/deps/steg_server-75c81fd969602d6a)

running 3 tests
test runtime::tests::a_typed_command_that_succeeds_is_running_then_finished_and_the_tab_shows_it ... FAILED
test runtime::tests::a_typed_command_still_running_stays_running_until_told_otherwise ... FAILED
test runtime::tests::a_typed_command_that_fails_is_failed_with_its_exit_code ... FAILED

failures:

---- runtime::tests::a_typed_command_that_succeeds_is_running_then_finished_and_the_tab_shows_it stdout ----

thread 'runtime::tests::a_typed_command_that_succeeds_is_running_then_finished_and_the_tab_shows_it' (16262527) panicked at crates/server/src/runtime.rs:1560:13:
pane 00000002 never reached status Some(Running); last seen None
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace

---- runtime::tests::a_typed_command_still_running_stays_running_until_told_otherwise stdout ----

thread 'runtime::tests::a_typed_command_still_running_stays_running_until_told_otherwise' (16262525) panicked at crates/server/src/runtime.rs:1560:13:
pane 00000002 never reached status Some(Running); last seen None

---- runtime::tests::a_typed_command_that_fails_is_failed_with_its_exit_code stdout ----

thread 'runtime::tests::a_typed_command_that_fails_is_failed_with_its_exit_code' (16262526) panicked at crates/server/src/runtime.rs:1560:13:
pane 00000002 never reached status Some(Running); last seen None


failures:
    runtime::tests::a_typed_command_still_running_stays_running_until_told_otherwise
    runtime::tests::a_typed_command_that_fails_is_failed_with_its_exit_code
    runtime::tests::a_typed_command_that_succeeds_is_running_then_finished_and_the_tab_shows_it

test result: FAILED. 0 passed; 3 failed; 0 ignored; 0 measured; 87 filtered out; finished in 20.02s

error: test failed, to rerun pass `-p steg-server --lib`
```

Check FAILED: `cargo test -p steg-server --lib a_typed_command` exited 101 at 2026-11-16T00:52:18+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.84 on task-1.84-server-pass-a-typed-command-s-start-and at 75387af8c49a with uncommitted changes:

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.08s
     Running unittests src/lib.rs (target/debug/deps/steg_server-75c81fd969602d6a)

running 3 tests
test runtime::tests::a_typed_command_that_succeeds_is_running_then_finished_and_the_tab_shows_it ... FAILED
test runtime::tests::a_typed_command_that_fails_is_failed_with_its_exit_code ... FAILED
test runtime::tests::a_typed_command_still_running_stays_running_until_told_otherwise ... FAILED

failures:

---- runtime::tests::a_typed_command_that_succeeds_is_running_then_finished_and_the_tab_shows_it stdout ----

thread 'runtime::tests::a_typed_command_that_succeeds_is_running_then_finished_and_the_tab_shows_it' (16275770) panicked at crates/server/src/runtime.rs:1565:13:
pane 00000002 never reached status Some(Finished); last seen Some(Running)

---- runtime::tests::a_typed_command_that_fails_is_failed_with_its_exit_code stdout ----

thread 'runtime::tests::a_typed_command_that_fails_is_failed_with_its_exit_code' (16275769) panicked at crates/server/src/runtime.rs:1565:13:
pane 00000002 never reached status Some(Failed { exit_code: 3 }); last seen Some(Running)
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace

---- runtime::tests::a_typed_command_still_running_stays_running_until_told_otherwise stdout ----

thread 'runtime::tests::a_typed_command_still_running_stays_running_until_told_otherwise' (16275768) panicked at crates/server/src/runtime.rs:1565:13:
pane 00000002 never reached status Some(Finished); last seen Some(Running)


failures:
    runtime::tests::a_typed_command_still_running_stays_running_until_told_otherwise
    runtime::tests::a_typed_command_that_fails_is_failed_with_its_exit_code
    runtime::tests::a_typed_command_that_succeeds_is_running_then_finished_and_the_tab_shows_it

test result: FAILED. 0 passed; 3 failed; 0 ignored; 0 measured; 87 filtered out; finished in 20.05s

error: test failed, to rerun pass `-p steg-server --lib`
```

Check FAILED: `cargo test -p steg-server --lib a_shell_with_no_marks` exited 101 at 2026-11-16T00:53:29+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.84 on task-1.84-server-pass-a-typed-command-s-start-and at 75387af8c49a with uncommitted changes:

```text
warning: function `drain_command_marks` is never used
   --> crates/server/src/pane.rs:509:4
    |
509 | fn drain_command_marks(terminal: &mut Terminal, pane: PaneId, events: &Sender<PaneEvent>) {
    |    ^^^^^^^^^^^^^^^^^^^
    |
    = note: `#[warn(dead_code)]` (part of `#[warn(unused)]`) on by default

warning: `steg-server` (lib test) generated 1 warning
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.08s
     Running unittests src/lib.rs (target/debug/deps/steg_server-75c81fd969602d6a)

running 1 test
test runtime::tests::a_shell_with_no_marks_leaves_every_status_field_unchanged_until_one_arrives ... FAILED

failures:

---- runtime::tests::a_shell_with_no_marks_leaves_every_status_field_unchanged_until_one_arrives stdout ----

thread 'runtime::tests::a_shell_with_no_marks_leaves_every_status_field_unchanged_until_one_arrives' (16292229) panicked at crates/server/src/runtime.rs:1560:13:
pane 00000002 never reached status Some(Running); last seen None
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    runtime::tests::a_shell_with_no_marks_leaves_every_status_field_unchanged_until_one_arrives

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 89 filtered out; finished in 20.35s

error: test failed, to rerun pass `-p steg-server --lib`
```

Check FAILED: `just check` exited 1 at 2026-11-16T00:53:46+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.84 on task-1.84-server-pass-a-typed-command-s-start-and at 75387af8c49a with uncommitted changes:

```text
FAILED  format check (exit 1, 0.2s)
Diff in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.84/crates/server/src/runtime.rs:16:
 use std::thread::{self, JoinHandle};
 use std::time::Instant;
 
[31m-use steg_kernel::{CommandEnded, CommandOutputStarted, ExitOutcome, Moment, Settings, SizeChanged, Tree};
(B[m[32m+use steg_kernel::{
(B[m[32m+    CommandEnded, CommandOutputStarted, ExitOutcome, Moment, Settings, SizeChanged, Tree,
(B[m[32m+};
(B[m use steg_sdk::{ApiError, Description, ErrorCode, PaneId, Program, Request, Response, validate};
 
 use crate::dispatch::{self, Context};
Diff in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.84/crates/server/src/runtime.rs:1708:
             Program::Shell {
                 cwd: "/tmp".to_string(),
             },
[31m-            vec!["sh", "-c", "read line; printf '\\033]133;C\\007'; sleep 100"],
(B[m[32m+            vec![
(B[m[32m+                "sh",
(B[m[32m+                "-c",
(B[m[32m+                "read line; printf '\\033]133;C\\007'; sleep 100",
(B[m[32m+            ],
(B[m         );
 
         // Bounded, not a race against nothing: every field R82/R83 govern
Diff in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.84/crates/server/src/runtime.rs:1834:
     #[test]
     fn a_command_panes_own_program_printing_marks_does_not_change_its_status() {
         let runtime = Runtime::new(std::sync::Arc::new(RealLauncher), login_for_marks());
[31m-        let script =
(B[m[31m-            "printf '\\033]133;C\\007'; sleep 0.3; printf '\\033]133;D;0\\007'; exit 0";
(B[m[32m+        let script = "printf '\\033]133;C\\007'; sleep 0.3; printf '\\033]133;D;0\\007'; exit 0";
(B[m         let (_, _, pane) = start_real_pane(
             &runtime,
             Program::Command {
full output: .quiet/format-check.log
error: recipe `check-fmt` failed on line 134 with exit code 1
```

Check passed: `just check` exited 0 at 2026-11-16T00:54:50+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.84 on task-1.84-server-pass-a-typed-command-s-start-and at 75387af8c49a with uncommitted changes:

```text
ok      format check (0.2s)
ok      lint (0.8s)
ok      boundaries (0.1s)
ok      licences (0.6s)
ok      tests (-p steg -p steg-kernel -p steg-server) (35.8s)
ok      build (steg-sdk) (0.1s)
ok      test (steg-sdk) (0.3s)
ok      test (steg-sdk, local-socket) (0.4s)
ok      build (steg-kernel) (0.4s)
ok      test (steg-kernel) (0.9s)
ok      build (steg-server) (0.1s)
ok      test (steg-server) (3.8s)
ok      test (steg-server, race-fixture) (7.1s)
ok      build (steg) (0.2s)
ok      test (steg) (0.1s)
ok      build (steg-app) (0.3s)
ok      test (steg-app) (0.3s)
ok      tool tests (1.0s)
ok      architecture (0.7s)
ok      recipe names (0.0s)
```

Check passed: `just check` exited 0 at 2026-11-16T00:56:01+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.84 on task-1.84-server-pass-a-typed-command-s-start-and at c916142e98f7:

```text
ok      format check (0.2s)
ok      lint (0.8s)
ok      boundaries (0.1s)
ok      licences (0.7s)
ok      tests (-p steg -p steg-kernel -p steg-server) (7.3s)
ok      build (steg-sdk) (0.1s)
ok      test (steg-sdk) (0.2s)
ok      test (steg-sdk, local-socket) (0.3s)
ok      build (steg-kernel) (0.4s)
ok      test (steg-kernel) (0.9s)
ok      build (steg-server) (0.1s)
ok      test (steg-server) (3.9s)
ok      test (steg-server, race-fixture) (6.9s)
ok      build (steg) (0.2s)
ok      test (steg) (0.1s)
ok      build (steg-app) (0.3s)
ok      test (steg-app) (0.3s)
ok      tool tests (0.9s)
ok      architecture (1.0s)
ok      recipe names (0.0s)
```

Check passed: `just check` exited 0 at 2026-11-16T00:59:59+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.84 on task-1.84-server-pass-a-typed-command-s-start-and at 7f6ab3ae5a6e:

```text
ok      format check (0.2s)
ok      lint (0.4s)
ok      boundaries (0.1s)
ok      licences (0.7s)
ok      tests (-p steg -p steg-kernel -p steg-server) (4.0s)
ok      build (steg-sdk) (0.1s)
ok      test (steg-sdk) (0.2s)
ok      test (steg-sdk, local-socket) (0.4s)
ok      build (steg-kernel) (0.1s)
ok      test (steg-kernel) (0.2s)
ok      build (steg-server) (0.3s)
ok      test (steg-server) (3.9s)
ok      test (steg-server, race-fixture) (4.1s)
ok      build (steg) (0.1s)
ok      test (steg) (0.1s)
ok      build (steg-app) (0.3s)
ok      test (steg-app) (0.3s)
ok      tool tests (1.1s)
ok      architecture (1.1s)
ok      recipe names (0.0s)
```

Built as planned (approved digest 33f0386d0f53), with the one departure the plan check named: no drain after Terminal::resize - a resize can only ever queue Event::PtyWrite, never Event::SemanticPrompt, so there is nothing for it to drain and nothing to test; the kernel's terminal.rs doc fix says "drained after every write" to match.

Coverage, clause by clause:

#1 (kernel wiring, read through the kernel - the wire half is TASK-1.89's): crates/server/src/pane.rs's worker_loop drains Terminal::take_command_marks after every write and sends PaneEvent::CommandOutputStarted/CommandEnded on the same `events` channel Exited already uses - one sender per pane, so mpsc's FIFO order is the proof a mark can never be applied after that pane's own exit, not a lock. crates/server/src/runtime.rs's apply_event_locked applies them through Tree::note_command_output_started/note_command_ended with `now` computed the same way Exited's arm already does. A mark's PaneNotFound is a deliberate no-op (does not remove the PaneHandle, unlike Exited's own arm) - proven by a_mark_for_a_pane_the_tree_no_longer_holds_does_nothing_and_does_not_panic. Tree::tab_shown_status is asserted directly in a_typed_command_that_succeeds_is_running_then_finished_and_the_tab_shows_it.

#2 (shown end to end, real shell, real pty): a_typed_command_that_succeeds_is_running_then_finished_and_the_tab_shows_it, a_typed_command_that_fails_is_failed_with_its_exit_code, a_typed_command_still_running_stays_running_until_told_otherwise, a_shell_with_no_marks_leaves_every_status_field_unchanged_until_one_arrives (all four in crates/server/src/runtime.rs, each driving a real Runtime and a real pty program printing real OSC 133 directly - no shell integration exists yet, TASK-1.85's own job). Two more beyond the four named shapes, for the orderings criterion 1 argues: a_mark_then_the_shells_own_real_exit_cascades_with_no_panic (mark then the pane's own real exit, same channel, same pane) and a_command_panes_own_program_printing_marks_does_not_change_its_status (R97: a command pane's own program always wins over a mark).

#3 (no leak): every test above drives a real Runtime and lets Drop (or the test's own explicit handle.stop()) end what it started; none of this change adds a new thread or process of its own - marks ride the existing events channel and drain thread.

Checks recorded on this task: a baseline `just check` (exit 0) before any change; a failing `cargo check -p steg-server --tests` (exit 101, non-exhaustive match) before apply_event_locked's new arms existed; a passing `cargo test -p steg-server --lib runtime::tests` (exit 0, 18 tests) once they did; three negative controls, each applied, confirmed failing, and restored - (a) the drain call after terminal.write removed (the three mark tests fail, exit 101), (b) the CommandEnded arm made a no-op (succeeds/fails fail, and still-running's own second half - sending the end mark itself and watching the status move - fails too, while its first half still passes; this is what makes that one test able to tell CommandOutputStarted's own arm from CommandEnded's), (c) the no-marks test's own built-in control (feeding one real mark at the end) also fails under (a)'s removed drain, confirmed separately; a final `just check` (exit 0) at the merged, pushed head.

TASK-1.78 (forced close) was still In Progress, not merged, at every check above and again just before pushing - the mark-for-a-pane-already-gone test's own comment ("until TASK-1.78 lands") still matches the repository's current state; nothing to re-read against a merge that hasn't happened yet. origin/main moved twice while this was in flight (once before the first push - rebased cleanly, no code touched; once after - merged, board-only files, no code touched); `just check` was re-run and recorded clean at the final head before the final push, and CI is green on that same head (7f6ab3a).

Check FAILED: `cargo test -p steg-server --lib a_command_panes_own_program` exited 101 at 2026-11-16T01:21:36+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.84 on task-1.84-server-pass-a-typed-command-s-start-and at 7f6ab3ae5a6e with uncommitted changes:

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.09s
     Running unittests src/lib.rs (target/debug/deps/steg_server-75c81fd969602d6a)

running 1 test
test runtime::tests::a_command_panes_own_program_printing_marks_does_not_change_its_status ... FAILED

failures:

---- runtime::tests::a_command_panes_own_program_printing_marks_does_not_change_its_status stdout ----

thread 'runtime::tests::a_command_panes_own_program_printing_marks_does_not_change_its_status' (16955249) panicked at crates/server/src/runtime.rs:1910:9:
assertion `left == right` failed
  left: Some(Failed { exit_code: 3 })
 right: Some(Running)
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    runtime::tests::a_command_panes_own_program_printing_marks_does_not_change_its_status

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 89 filtered out; finished in 0.43s

error: test failed, to rerun pass `-p steg-server --lib`
```

Commits since the review: 0.

Check FAILED: `cargo test -p steg-server --lib a_command_panes_own_program` exited 101 at 2026-11-16T01:22:37+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.84 on task-1.84-server-pass-a-typed-command-s-start-and at 7f6ab3ae5a6e with uncommitted changes:

```text
warning: function `drain_command_marks` is never used
   --> crates/server/src/pane.rs:509:4
    |
509 | fn drain_command_marks(terminal: &mut Terminal, pane: PaneId, events: &Sender<PaneEvent>) {
    |    ^^^^^^^^^^^^^^^^^^^
    |
    = note: `#[warn(dead_code)]` (part of `#[warn(unused)]`) on by default

warning: `steg-server` (lib test) generated 1 warning
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.08s
     Running unittests src/lib.rs (target/debug/deps/steg_server-75c81fd969602d6a)

running 1 test
test runtime::tests::a_command_panes_own_program_printing_marks_does_not_change_its_status ... FAILED

failures:

---- runtime::tests::a_command_panes_own_program_printing_marks_does_not_change_its_status stdout ----

thread 'runtime::tests::a_command_panes_own_program_printing_marks_does_not_change_its_status' (16973480) panicked at crates/server/src/runtime.rs:1562:13:
pane 00000005 never reached status Some(Failed { exit_code: 3 }); last seen None
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    runtime::tests::a_command_panes_own_program_printing_marks_does_not_change_its_status

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 89 filtered out; finished in 20.02s

error: test failed, to rerun pass `-p steg-server --lib`
```

Commits since the review: 0.

Check passed: `just check` exited 0 at 2026-11-16T01:24:04+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.84 on task-1.84-server-pass-a-typed-command-s-start-and at 7cbed63ed702 with uncommitted changes:

```text
ok      format check (0.2s)
ok      lint (1.2s)
ok      boundaries (0.1s)
ok      licences (0.7s)
ok      tests (-p steg -p steg-kernel -p steg-server) (36.7s)
ok      build (steg-sdk) (0.1s)
ok      test (steg-sdk) (0.3s)
ok      test (steg-sdk, local-socket) (0.4s)
ok      build (steg-kernel) (0.4s)
ok      test (steg-kernel) (0.8s)
ok      build (steg-server) (0.1s)
ok      test (steg-server) (3.9s)
ok      test (steg-server, race-fixture) (7.0s)
ok      build (steg) (0.2s)
ok      test (steg) (0.1s)
ok      build (steg-app) (0.3s)
ok      test (steg-app) (0.3s)
ok      tool tests (1.2s)
ok      architecture (1.0s)
ok      recipe names (0.0s)
```

Commits since the review: 48, 48 without the Asdlc-Worker trailer.

Review findings fixed (review by @bruk.r2, 2026-11-16):

Important: a_command_panes_own_program_printing_marks_does_not_change_its_status could not fail - its D mark and its real exit both read as Finished, and the D landed outside the one window the test ever checked Running inside, so a wrongly-applied mark and a mark that never arrived were both invisible to it. Fixed as the finding's own second option: the D mark now carries exit code 3, differing from the real exit's own 0, so a wrongly-applied mark is observable as Failed{exit_code:3} - a value the real exit path alone can never produce. Before asserting anything about the command pane, the test now proves the marks had time to land by running the identical script in a sibling shell pane and waiting for it to read that same Failed{exit_code:3} first, over the one drain thread every pane's marks share (picked over an in-crate applied-event counter: this reuses the real channel and drain thread as they already are, with no test-only instrumentation added to production types). Two negative controls run, confirmed failing, and restored, each recorded with asdlc record-check: the kernel's command-pane guard bypassed in both Pane::note_command_output_started and note_command_ended (crates/kernel/src/pane.rs) - the test fails, reading Failed{exit_code:3} where it must read Running; and the mark drain removed again in crates/server/src/pane.rs - the test fails, the shell-pane control itself timing out waiting for Failed, so the test no longer passes when nothing arrives either.

Nit: reworded the mark-for-a-pane-already-gone test's own comment (crates/server/src/runtime.rs) to describe what the test stands for - a pane whose own real program may still be running after its tree entry is gone - rather than naming TASK-1.78 as the task whose merge would make the wording stale.

Checks recorded on this task since the review: a passing run of the fixed test alone (exit 0), five repeats with no flakiness; the bypass-the-guard control (exit 101, confirmed failing, restored); the drain-removed control (exit 101, confirmed failing, restored); the full runtime::tests suite (18 tests, exit 0) after restoring both; a full `just check` (exit 0) at the merged head.

origin/main moved while this was in flight - fetched and merged (git merge --no-edit, branch already pushed): every incoming commit was board-only (backlog/ task and draft updates, review and check records); no crates/ file touched. TASK-1.78 (forced close) was still In Progress at merge time and remains so - the reworded comment on the gone-pane test no longer names it, so nothing here depends on when it lands. `just check` was re-recorded clean at the merged, pushed head, and CI is green on that same head (c495468).
<!-- SECTION:NOTES:END -->

## Comments

<!-- COMMENTS:BEGIN -->
created: 2026-11-16T00:33:00Z

---

Split on 2026-11-15 by @bruk.o under the engineer's standing decision to split by default: the worker's plan found two changes. This task keeps passing the marks to the kernel. Reporting the display status over the API moved to a new draft with the list and show methods nobody owned (tab show and tab list are still answered NotImplementedYet, and reading a status needs the clock passed into dispatch, which TASK-1.45 is rewriting now). Criterion 1 reworded to match; criteria 2 and 3 unchanged.

---

author: @bruk
created: 2026-11-16T00:37:00Z

---

Plan approved by @bruk (33f0386d0f53, size-2): its worker may build it.

---

author: @bruk.r2
created: 2026-11-16T01:03:00Z

---

@bruk.r2 is reviewing TASK-1.84: a same-machine review: it waited 0m with no other engineer's reviewer, longer than review_fallback_after (0m), so a fresh reviewer on its author's machine takes it.

---

author: @bruk.r2
created: 2026-11-16T01:16:00Z

---

Review by @bruk.r2 (same-machine): changes requested. Back to In Progress for the author's worker.

Findings: 1 Important, 1 nit. Cites: review. The diff does not match the plan.

Head reviewed: 7f6ab3ae5a6e9b8766f2eab12dd457284dcb1ac7 (matches the task's final recorded `just check` and the pull request's green CI check run). Independently reproduced: `just check` exit 0 on this head in a fresh worktree; the 18 runtime::tests pass; the 6 new mark tests pass 20/20 on repeated runs with default parallelism (no flakiness observed); negative control (a) (removing the `drain_command_marks` call after `terminal.write`) reproduced in a scratch edit, reverted afterward - it reproduces the three recorded failures (`a_typed_command_that_succeeds...`, `a_typed_command_that_fails...`, `a_typed_command_still_running...`, plus `a_mark_then_the_shells_own_real_exit...` and `a_shell_with_no_marks...`, all failing "never reached status Some(Running); last seen None").

[Evidence, Important] crates/server/src/runtime.rs, `a_command_panes_own_program_printing_marks_does_not_change_its_status` (named in plan §6 as the proof that `apply_event_locked`'s `NotAShellPane`/`Ignored` arms are reached end to end, not just compiled): under my repeated negative control (a) - `drain_command_marks` removed entirely, so no mark of any kind ever reaches `apply_event_locked` - this is the only one of the six new mark tests that still passes. The command pane's initial status is already `Running` from `start_program`, independent of any mark; the `C` mark fires near t=0 inside the 150ms assertion window but even applied as a shell pane's `Started` it sets the same `Running` value, so there is nothing to observe; the `D;0` mark fires at 0.3s, outside that window, and the script's own `exit 0` right behind it drives `Finished` through the ordinary `Exited`/`record_exit` path regardless of whether the mark was drained, ignored, or never delivered. The final `wait_for_pane_status(Finished)` therefore goes green off the real exit either way. The test cannot tell "the mark was correctly ignored" apart from "the mark mechanism does nothing, or was never wired up at all" - exactly the gap a negative control is for, and it fails that control.

[Compliance, Nit] crates/server/src/runtime.rs, `a_mark_for_a_pane_the_tree_no_longer_holds_does_nothing_and_does_not_panic`'s own comment ("standing in for a forced close that has already happened ... until TASK-1.78 lands") will read as stale once TASK-1.78 merges, since the scenario becomes real rather than synthetic; the `PaneNotFound` arm's behaviour itself is generic (keyed only on whether the tree still holds the pane) and I found nothing in TASK-1.78's planned files that would change its meaning.

Other brief items checked clean: `events.send` appears only twice in pane.rs, both inside `worker_loop` (the `Exited` arm and `drain_command_marks`, called only from the `PtyBytes` arm) - the worker is the sole sender of every `PaneEvent` for a pane on every path; a restart only ever targets a command pane (R97: a shell pane closes rather than restarting), and the kernel already treats marks on a command pane as `NotAShellPane`/`Ignored`, so no old-run mark can be misapplied to a new run even putting the channel-ordering argument aside. `apply_event_locked`'s two new arms match the plan exactly, including `now` computed once per event the same way the `Exited` arm already does, and the `Err(other) => unreachable!` arms are genuinely unreachable - `Tree::note_command_output_started`/`note_command_ended` only ever construct `Err(Error::PaneNotFound(id))`, confirmed by reading `tree.rs`. Kernel diff is confined to the one doc comment on `terminal.rs`; no other kernel file touched, no `unsafe`, no new dependency. The one departure from the plan (no drain after `Terminal::resize`) was already named and accepted at plan-check by @bruk.o, so `--plan-matched no` reflects that departure as data, not a new finding.

---

author: @bruk.r2
created: 2026-11-16T01:28:00Z

---

@bruk.r2 is reviewing TASK-1.84: a same-machine review: it waited 0m with no other engineer's reviewer, longer than review_fallback_after (0m), so a fresh reviewer on its author's machine takes it.

---
<!-- COMMENTS:END -->
