---
id: TASK-1.84
title: >-
  Server: pass a typed command's start and end from a pane's terminal to the
  kernel
status: In Progress
assignee:
  - '@bruk.w1'
created_date: '2026-11-15T21:43:00Z'
updated_date: '2026-11-16T00:32:00Z'
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
- [ ] #1 When a pane's terminal reports a prompt-marked command starting or ending, the server passes it to the kernel with a clock reading as a Moment, through the same route a program's exit already takes (TASK-1.76's Runtime::apply_event), and pane show and the tab's shown status reflect it
- [ ] #2 Shown end to end with a real shell in a real pty that emits the marks: a command that succeeds, one that fails, and one still running; and a shell that emits none, where nothing changes
- [ ] #3 No child is left unreaped and no thread running by anything this adds, and the tests stop the runtime they start
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
This is two reviewable changes. §1-§4 below build the kernel wiring AC #1 and #2 ask for. §5 is the part of AC #1 this plan does NOT build - the wire-level half ("pane show and the tab's shown status reflect it") - named with why, for the engineer to decide: amend AC #1 to drop that clause into a follow-up task, or accept this task as partial against it. Nothing here widens silently to cover it.

## 1. Where marks are drained, and why that ordering is safe

Drained inside `crates/server/src/pane.rs`'s `worker_loop`, in the `PaneCommand::PtyBytes(bytes)` arm, immediately after `terminal.write(&bytes)` - and the same way after `terminal.resize(...)` in the `Resize` arm, since `Terminal::resize` also calls `drain_events` internally and can queue a mark exactly as `write` does (mirrors the kernel's own stated reason for draining after every event-producing call, `terminal.rs`'s doc on `drain_events`). One small helper, `fn drain_command_marks(terminal: &mut Terminal, pane: PaneId, events: &Sender<PaneEvent>)`, called from both arms, turns each `CommandMark::OutputStarted`/`CommandMark::Ended { exit_code }` `take_command_marks()` returns into a `PaneEvent::CommandOutputStarted { pane }` / `PaneEvent::CommandEnded { pane, exit_code }` and sends it on the same `events: Sender<PaneEvent>` `PaneEvent::Exited` already uses - not a new channel, and not a new `PaneCommand::TakeMarks(reply)` polled from `Runtime` (the shape `Cwd` uses, and the rejected alternative): that would create a second path with no ordering relationship to `Exited` at all, which is exactly the race AC #3/#1 rule out.

The ordering proof is the channel, not a lock: `worker_loop`'s own single thread is the one sender of every `PaneEvent` for a pane (both the marks this task adds and the `Exited` TASK-1.76 already sends), and `spawn_relay` only ever enqueues `PtyBytes` while reading to EOF and sends `Exited` strictly after, once `child.wait()` returns - so the `Exited` for a pane's last run is always the last `PaneCommand` that run's relay thread submits, and the worker drains any mark from a `PtyBytes` arm before it ever reaches a later `Exited` arm. mpsc is FIFO per sender, so the drain thread on the other end (`Runtime`'s own, in `runtime.rs`) receives and applies them in that same order. No change to locking, to `spawn_relay`, or to how `Exited` itself is produced.

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
- `Err(Error::PaneNotFound(_))` on either -> **do nothing to `state.panes`.** This is the one place this plan must not copy the `Exited` arm's shape: `Exited`'s `PaneNotFound` arm removes the stale `PaneHandle` because an exit means the handle is pointless. A mark's `PaneNotFound` means the opposite - the tree lost the pane (a forced close racing the mark) while the program the `PaneHandle` still owns may well be running. Removing it would drop the `Sender` clone inside the handle, which nothing else holds, ending the worker loop and leaking the relay thread and an unkilled child - worse than the orphan being guarded against, and a direct AC #3 violation. So: match it, do nothing, move on.
- `Err(other)` -> `unreachable!`, with a message naming that `Tree::note_command_output_started`/`note_command_ended` document only `Error::PaneNotFound` as a possible error (`tree.rs`'s own doc comments on both), not the `ProgramNotStarted` reasoning `Exited`'s own `unreachable!` gives - a different method, a different documented error set, so copying that message would misstate why the arm is unreachable.

## 4. Doc fix this task's own addition falsifies

`crates/kernel/src/terminal.rs`'s `command_marks` field doc (the paragraph starting "Unbounded until a caller exists") says "nothing in this task calls `take_command_marks`, so in the repository's shipped state after this task merges, this grows for the whole life of any pane." This task is that caller. Update the paragraph to say `crates/server/src/pane.rs`'s `worker_loop` now drains it after every `write`/`resize`, so in practice it holds at most the marks from one pty read or one resize's own in-band report before the next drain - named, not left stale (`.claude/rules/code-design.md`: what the code says must be true).

## 5. What this plan does NOT build, and why

AC #1 says pane show and the tab's shown status "reflect it." This plan does not touch `crates/server/src/dispatch/pane.rs`'s `show()`, `crates/server/src/dispatch/tab.rs` (`tab.show`/`tab.list` are both `not_implemented_yet` in full today - no task has built them at all yet, so wiring one field into a method that doesn't exist is its own, larger task), or `steg_sdk::PaneRecord`/`description/methods.rs`, for three independent reasons, any one of which is enough on its own:

- `dispatch::dispatch`'s signature (`crates/server/src/dispatch/mod.rs`) is explicitly a pure function today ("no socket, no I/O in its signature" - its own module doc) and carries no clock reading. Reading `Pane::status(now)` from inside `pane::show` needs a real `now: Moment`, which only `Runtime`'s `State::clock_origin` can produce - so surfacing it means adding a `now: Moment` parameter to `dispatch::dispatch` itself and both its call sites in `Runtime::handle`. That file is TASK-1.45's right now (frames, streams, requests routed to the API) - touching its central signature while that task is actively rewriting the same file is exactly the overlap the brief asked this plan to name and avoid, not take on silently.
- Changing what `pane.show`'s `status` field means, or adding a field, is a wire/API-description change (decision-1, doc-16 R101: the sdk is steg's public interface, defined once) - the task's own implementation notes already route this to the engineer by name ("whether the field's meaning changes or a second field is needed; name any description change for the engineer"), not to this plan to decide unilaterally.
- Half of AC #1's wire clause - "the tab's shown status" - cannot be built at all without first building `tab.show`/`tab.list`, which no task has done.

Recommendation for that follow-up task, named but not built here: add a *second* field to `PaneRecord` (e.g. `display_status: Option<steg_sdk::Status>`), not change `status`'s existing meaning. `crates/kernel/src/pane.rs`'s own doc comment on why `program_status`/`status` are two separate fields, kept deliberately apart, is the same argument for the wire: `status` (running/ended) and a new `display_status` (running/finished/failed, `steg_sdk::Status` - already the exact type `Pane::status()` returns, no new sdk type needed) should stay two fields on the wire too, not one field whose meaning silently changes under existing callers of doc-2's contract.

What *is* already true at the kernel level, and does not need the wire to prove it: `Tree::tab_shown_status` (built in TASK-1.41) and `Pane::status` already rank and report a typed command's running/finished/failed state exactly like a command pane's own program - proven by the existing kernel test suite (`tab.rs`'s `shown_status_ranks_a_running_typed_command_exactly_like_a_running_program` and siblings) plus this task's own end-to-end proof in §6 below, which checks `Pane::status(now)` through a real `Runtime`, not the wire. AC #1's "reflect it" is satisfied at that level; satisfying it over the wire is §5's named gap.

## 6. Proof (AC #2, #3)

New tests in `crates/server/src/runtime.rs`'s `#[cfg(test)] mod tests` (not `tests/`: reading `Pane::status(now)` needs a `Moment` built from the private `State::clock_origin`, which only in-crate tests can reach) - each drives a real `Runtime` with a real pty program that prints real OSC 133 sequences directly (`sh -c 'printf "\033]133;C\007"; ...; printf "\033]133;D;0\007"'` or similar) - no shell integration exists yet (that is TASK-1.85's own job), so the test program is its own stand-in for a shell that already has it. Every wait is bounded, polling `Pane::status(now)` (never a bare `recv`/`sleep` loop with no bound), the same shape this file's existing tests already use, and every test calls `runtime.stop()` (or lets `Drop` run it) before finishing, proving AC #3 - explicitly stated in each test or in a shared comment: this change adds no new thread and no new process; marks ride the existing `events` channel and the existing drain thread `Runtime::new` already starts.

- **A command that succeeds**: emits `C`, then `D;0`. Asserts `Pane::status(now)` becomes `Running` after the `C`, then `Finished` after the `D`. Negative control: a sibling case asserts the same shell shape with `D;1` instead ends up `Failed`, not `Finished` - so a test that hardcoded `Finished` regardless of the exit code could not pass both.
- **A command that fails**: emits `C`, then `D;3` (or similar). Asserts `Failed { exit_code: 3 }`. Its own negative control is the succeeds case above, run the other way: `D;0` there must *not* read as `Failed`.
- **A command still running**: emits `C` only, no `D` yet. Asserts `Running` while waiting. Negative control: the test then sends the `D` itself (reusing the same pane) and asserts the status changes away from `Running` - proving the first assertion was not vacuously true because nothing in this path could ever change it.
- **A shell that emits no marks at all**: a plain program with no OSC 133 output. Asserts `Pane::status(now)` stays `None` throughout (R82/R83's own "a shell idling at its own prompt" baseline - `crate::pane`'s own doc on this). Its negative control is the three tests above: the same harness, same assertion shape, does detect a change when marks are present, so "nothing changed" here is not a test that could never have failed.

## Risks / overlap

- **TASK-1.78** (`pane.rs`, `pty.rs`, `runtime.rs`; a rewritten `Runtime::stop` and the close paths) - real overlap, different regions of the same files. This plan touches only: two new `PaneEvent` variants, one new helper function, two call sites of that helper (`PtyBytes`/`Resize` arms), and two new match arms in `apply_event_locked`. It does not touch `stop`, `Stop`, `shutdown`, or `spawn_relay` at all - whichever of the two tasks merges second rebases past the other's additions, not a structural conflict.
- **TASK-1.45** (`dispatch/mod.rs`, `listen.rs`, `connection.rs`, `lookup.rs`, `ServerConfig`) - no overlap, because of §5's own decision: `dispatch::dispatch`'s signature is left untouched.
- **TASK-1.77** (`crates/steg`, one detach function) - no overlap.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Carried from TASK-1.83's planning (2026-11-15): pane.show's wire field 'status' is built today from the program's status (running or ended), in crates/server/src/dispatch/pane.rs, not from the display status the kernel now keeps (running, finished, failed, with its ending rules). This task is where the server first has a clock reading to ask the kernel for the display status, so its plan says what pane show and the tab's shown status report over the API, against doc-2 and doc-11, and whether the field's meaning changes or a second field is needed; name any description change for the engineer.
<!-- SECTION:NOTES:END -->
