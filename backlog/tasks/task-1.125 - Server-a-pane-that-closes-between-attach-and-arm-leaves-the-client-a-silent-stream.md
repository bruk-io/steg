---
id: TASK-1.125
title: >-
  Server: a pane that closes between attach and arm leaves the client a silent
  stream
status: In Progress
assignee:
  - '@bruk.w4'
created_date: '2026-11-17T02:40:00Z'
updated_date: '2026-11-17T03:55:00Z'
labels:
  - size-2
milestone: m-4
dependencies:
  - TASK-1.47
parent_task_id: TASK-1
project: bruk-io-steg
asdlc_accepted_from: DRAFT-3
asdlc_branch: task-1.125-server-a-pane-that-closes-between-attach
asdlc_plan_approved_by: bruk
asdlc_plan_approved_hash: 'b734cfaf5683'
asdlc_plan_approved_size: size-2
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found by TASK-1.47's worker on 2026-11-16 after its second review had passed, so it could not go into that task. pane.attach replies with a snapshot, then the connection arms the attachment in a second step. If the pane closes between the two, arm_attachment does nothing and says nothing: the client has been told it is attached, the stream stays open, and no update or close ever arrives on it. The client can still learn of the closure from tree events and detach, so nothing leaks past the connection, but an attached client that does not watch events waits forever. The gap between the two steps existed before TASK-1.47's lock fix and is a few instructions wide in the normal case; a busy pane widens it. The worker has a fix and a test ready as a patch in the session scratchpad (arm_attachment returns whether it armed; on false the connection closes the stream at once and records nothing). decision-2 section 3; doc-14.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 When a pane has closed by the time its attachment is armed, the client's stream is closed at once and the connection keeps no record of it; a later pane.detach for that target answers as for a pane that was never attached
- [ ] #2 A test builds that state deterministically at the runtime level and proves arm reports it, with a recorded failing control and passing run
- [ ] #3 A test at the socket level shows the client sees its stream close when the pane closes during a held-up attach, with a recorded control; or the notes say why that cannot be made deterministic and what was tried
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
## Files that change
- `crates/server/src/runtime.rs`: `Runtime::arm_attachment` returns `bool` (whether it actually armed a live attachment); a new unit test, `arm_attachment_answers_false_once_the_pane_has_already_closed`, in `mod tests`. Taken verbatim from the durable patch `task-1.47-arm-returns-armed.patch` (applies cleanly to this worktree's own `f91a8f27` base, re-checked).
- `crates/server/src/connection.rs`: the `"pane.attach"` success arm in `handle_request` checks `arm_attachment`'s return. `true`: unchanged, records `pane_streams` as today. `false`: closes both sides of the stream (`outbound.close_stream`/`connection.close_stream`) and never records it in `pane_streams`, matching AC #1 exactly. Same patch.
- `crates/server/tests/attach.rs`: a new test for AC #3, built with `new_pane_running`/`attach`/`detach` already in the file - see "Order of work" and the open question below for what it can actually prove today.
- No sdk or kernel file changes; no change to `dispatch/pane.rs`'s close path.

## A second patch with no other home
`task-1.47-stronger-lock-test.patch` (same folder) rewrites `attach_pane_on_a_busy_panes_own_worker_does_not_stall_an_unrelated_requests_lock` in `runtime.rs` - a stronger version of TASK-1.47's own already-merged lock-contention test (bigger flood, a `done_rx` check confirming the stuck attach is still unanswered, the measurement itself moved onto its own thread with a bounded `recv_timeout` since a plain `Mutex::lock` has no timeout of its own). It touches only an existing test's body, not production code, and has nothing to do with this task's own three criteria. Applying it here is the only place it has to land, since TASK-1.47 closed before the worker finished strengthening it. Flagged as a separate, addable-or-strikeable step for the engineer: apply it as its own commit, distinct from the AC-driven work above, or strike it from this task's own scope and give it a different home. I recommend keeping it here (same file, no risk to anything else, and the version on main is otherwise weaker than it should be) but it is the engineer's call, not mine.

## Order of work
1. Apply `task-1.47-arm-returns-armed.patch` to this worktree (`git apply`). Build, run `cargo test -p steg-server --lib arm_attachment_answers_false_once_the_pane_has_already_closed`. This alone delivers AC #1 and AC #2 together - the patch's own test already has a recorded failing control (reverting `arm_attachment` to report armed unconditionally) and a passing run, done once already against this same patch; redo both through `asdlc record-check` here, since a control run elsewhere does not count for this task.
2. Decide, with the engineer, on the stronger-lock-test patch (see above) - apply it as its own commit if accepted, or note in this task that it was struck and where it goes instead.
3. Read the worker's exit path again in this worktree (`pane.rs`'s `PaneCommand::Exited` arm) to confirm the answer below still holds, then write AC #3's test - the exact shape depends on whether TASK-1.114 (`pane.send`) has merged to main by this point; see the open question and the two branches under it.
4. `just check`, recorded. Push, `gh pr checks --watch` to green. Finish, naming in the notes: which of the two test shapes under AC #3 was used (or the honest account if neither could be built), and that the lock-test patch was applied/struck per the engineer's own call.

## Answered during planning
**Does closing the stream today also happen when a pane ends while already armed (not racing anything)?** No. Read `pane.rs`'s `PaneCommand::Exited` arm (`worker_loop`): it sends `PaneEvent::Exited` (which becomes `TreeEvent::PaneExited`, reaching only an `event.watch` subscriber) and, if nothing is `awaiting_exit`, does not even break the worker's own loop - it never touches `attachments` at all, armed or not. A client attached to a pane that exits normally learns nothing from the stream itself either, today, regardless of this race. This is a real, pre-existing gap, but a different and larger one than this task's three criteria name (it would mean notifying every live attachment on every pane exit, not just the one minted during a lost race) - out of this size-2 task's own scope; worth its own follow-up if wanted, not folded in here.

**Can AC #3's test observe "the client sees its stream close" directly?** No, and this is worth settling plainly: `steg_sdk::wire::Incoming` has no "stream closed" frame at all (`Hello`, `Request`, `Reply`, `Event`, `UnknownEvent`, `StreamControl`, `Data`, `UnknownStream`, `Malformed` - read in full). The fix closes both sides *silently* - sending nothing - which is correct (decision-2 section 3 treats a frame for a stream the server never meant to send, `UnknownStream`, as the one invariant a correct server never produces; manufacturing one on purpose to signal this would violate the same invariant it is a proof of elsewhere). So "the client sees its stream close" can only mean its client-observable *consequence*: a later `pane.detach` for that same target, from the same connection, answers exactly as it would for a pane never attached at all (`BadRequest`, the existing `detaching_a_pane_this_connection_never_attached_to_is_a_bad_request` shape) - that is what AC #3's test proves, not a frame arriving.

## Open question for the engineer, not settled here
**Making AC #3's test deterministic needs a busy worker, and nothing on main today can create one from a socket-level test.** The runtime-level test (AC #2, already built) widens the attach-to-arm window by flooding the pane's own stdin directly through `PaneHandle::send_input` - unreachable from `crates/server/tests/attach.rs`, which only has the wire API `steg-server` actually serves today. `pane.send` is TASK-1.114's own method, still `NotImplementedYet` on this worktree's `main` (confirmed: `dispatch/pane.rs`'s own `"send" | "capture" | "wait" => Err(not_implemented_yet(...))`). No other implemented method creates sustained backpressure on a worker (`pane.resize`/`pane.show` are fast, no blocking write). Without it, the race window at the socket level is a few CPU instructions wide (the task's own description says so) with no way to widen it deterministically - a real reproduction would depend on OS scheduling luck, not something to record as a control.

Two branches, depending on TASK-1.114's own state when this is built:
- **If TASK-1.114 has merged by then:** build the socket-level test with two connections - one floods the target pane's own stdin via `pane.send` (same shape as the runtime-level test's flood, now over the wire) then the *other* connection sends `pane.attach` for that same pane (queuing behind the flood, blocking that connection's own server-side thread); while blocked, the first connection sends `pane.close --force`; once the stuck attach finally answers and the server-side arm fails, the second connection calls `pane.detach` for the same target and asserts the `BadRequest` answer above. Recorded failing (revert to the pre-patch `arm_attachment`) then passing, same as every other control on this task.
- **If it has not:** the notes say exactly this - what was tried, why `pane.send`'s absence makes the window undeterministic today, and that the test above is the one to add once TASK-1.114 lands (a one-line follow-up, not a redesign).

I am not picking one now because it depends on timing outside this task's own control; I will act on whichever is true when I build, and say which in the finish notes.

## Risks
- **TASK-1.114 touches the same `"pane.attach"` match arm in `connection.rs`.** Their branch (checked directly) still has the pre-fix, unchecked `runtime.arm_attachment(...)` call and adds its own `stream_panes.borrow_mut().insert(stream, pane)` line right after it, inside what becomes this task's own `if armed { ... }` branch. Whichever of the two tasks merges second resolves a real but small conflict: `stream_panes.insert(...)` belongs inside the `if armed` branch (nothing to key by a stream that is about to be closed unarmed), not the `else`. Named here so neither worker is surprised by it; not something to pre-emptively fix in either task.
- **The stronger-lock-test patch, applied here, touches a test TASK-1.114 does not touch but does run** (`cargo test -p steg-server` covers all of `runtime.rs`'s own module) - low risk, no shared lines, just worth the engineer's own sign-off per the section above.
- **Sizing:** this is still size-2 as labelled - the arm-fix patch is already built and verified elsewhere, and the socket-level test (whichever branch applies) reuses existing helpers in `attach.rs` rather than inventing new infrastructure.

## Proof
- `cargo test -p steg-server --lib arm_attachment_answers_false_once_the_pane_has_already_closed` - recorded failing (control: report armed unconditionally) then passing. AC #1, AC #2.
- Whichever AC #3 test is built - recorded failing then passing, or the honest account in the finish notes with what was tried. AC #3.
- `just check` - recorded, green, before finishing.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Plan checked against the task's criteria and decision-2 by @bruk.o (plan b734cfaf5683): the plan does what it says, but it shows the task as accepted does not fix what the client sees. The wire has no way to tell a client its stream has ended: a stream's control frames carry screen updates and nothing else. So after this fix the server's own records are right and a later pane.detach answers correctly, but a client that was told it is attached still hears nothing. The plan also found the same silence in the ordinary case: a pane whose program ends while a client is attached and armed sends nothing on the stream; only tree events report it. Put to the engineer: either approve this as the small server-side tidy it is and draft the real fix separately (an update that says the pane has ended, sent to every attachment, which the window needs for a program that has ended, TASK-1.61), or widen this task to that.

Check FAILED: `cargo test -p steg-server --lib arm_attachment_answers_false_once_the_pane_has_already_closed` exited 101 at 2026-11-17T03:55:43+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.125 on task-1.125-server-a-pane-that-closes-between-attach at d41ba8a5d63d with uncommitted changes:

```text
   Compiling steg-server v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.125/crates/server)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.90s
     Running unittests src/lib.rs (target/debug/deps/steg_server-6d2f1d51fdd2e018)

running 1 test
test runtime::tests::arm_attachment_answers_false_once_the_pane_has_already_closed ... FAILED

failures:

---- runtime::tests::arm_attachment_answers_false_once_the_pane_has_already_closed stdout ----

thread 'runtime::tests::arm_attachment_answers_false_once_the_pane_has_already_closed' (50117161) panicked at crates/server/src/runtime.rs:2956:9:
arm_attachment must answer false once the pane it was meant for has already closed, not arm a worker that no longer has anywhere to send anything
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    runtime::tests::arm_attachment_answers_false_once_the_pane_has_already_closed

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 191 filtered out; finished in 0.02s

error: test failed, to rerun pass `-p steg-server --lib`
```

Check passed: `cargo test -p steg-server --lib arm_attachment_answers_false_once_the_pane_has_already_closed` exited 0 at 2026-11-17T03:55:57+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.125 on task-1.125-server-a-pane-that-closes-between-attach at d41ba8a5d63d with uncommitted changes:

```text
   Compiling steg-server v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.125/crates/server)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.90s
     Running unittests src/lib.rs (target/debug/deps/steg_server-6d2f1d51fdd2e018)

running 1 test
test runtime::tests::arm_attachment_answers_false_once_the_pane_has_already_closed ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 191 filtered out; finished in 0.02s
```
<!-- SECTION:NOTES:END -->

## Comments

<!-- COMMENTS:BEGIN -->
author: @bruk
created: 2026-11-17T03:51:00Z

---

Plan approved by @bruk (b734cfaf5683, size-2): its worker may build it.

---
<!-- COMMENTS:END -->
