---
id: TASK-1.84
title: >-
  Server: pass a typed command's start and end from a pane's terminal to the
  kernel
status: In Progress
assignee:
  - '@bruk.w1'
created_date: '2026-11-15T21:43:00Z'
updated_date: '2026-11-16T00:36:00Z'
labels:
  - size-2
milestone: m-4
dependencies:
  - TASK-1.79
  - TASK-1.76
  - TASK-1.83
parent_task_id: TASK-1
project: bruk-io-steg
asdlc_accepted_from: DRAFT-3
asdlc_branch: task-1.84-server-pass-a-typed-command-s-start-and
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
<!-- SECTION:NOTES:END -->

## Comments

<!-- COMMENTS:BEGIN -->
created: 2026-11-16T00:33:00Z

---

Split on 2026-11-15 by @bruk.o under the engineer's standing decision to split by default: the worker's plan found two changes. This task keeps passing the marks to the kernel. Reporting the display status over the API moved to a new draft with the list and show methods nobody owned (tab show and tab list are still answered NotImplementedYet, and reading a status needs the clock passed into dispatch, which TASK-1.45 is rewriting now). Criterion 1 reworded to match; criteria 2 and 3 unchanged.

---
<!-- COMMENTS:END -->
