---
id: TASK-1.128
title: 'Server and app: an attached client is told on its stream when its pane ends'
status: In Progress
assignee:
  - '@bruk.w4'
created_date: '2026-11-17T03:51:00Z'
updated_date: '2026-11-17T22:39:00Z'
labels:
  - size-3
milestone: m-4
dependencies:
  - TASK-1.125
  - TASK-1.114
  - TASK-1.117
parent_task_id: TASK-1
project: bruk-io-steg
asdlc_accepted_from: DRAFT-2
asdlc_branch: task-1.128-server-and-app-an-attached-client-is
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found by TASK-1.125's plan on 2026-11-16, and seen by the engineer the same day: typing exit in the bare window ends the shell and the window is never told. A stream's control frames carry screen updates and nothing else, so the wire has no way to say a stream has ended. When a pane's program ends while a client is attached, the pane's worker reports it as a tree event only; nothing is sent on the attachment's stream, and the client keeps a stream that will never carry another update. TASK-1.125 makes the server's own records right in the narrow case where a pane closes between attach and arm, but the client still hears nothing there either. This task gives a screen update a final form that says the pane has ended, with how it ended where the server knows it (the program exited with a status, or the pane was closed), sent once to every attachment and followed by the stream being closed on both sides. The bare window shows that its program has ended and stops taking keys; the drawn states of a program that has ended, and what the window offers next, stay with TASK-1.61. decision-2 section 3 and doc-14 need the new final update written in; doc-9; doc-10.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 When a pane's program ends or the pane is closed, every client attached to it receives one final update on its stream saying so, with the exit status when there is one, after any screen update still owed and before the stream is closed; no frame arrives on that stream afterwards
- [ ] #2 The same final update is sent when a pane has closed between attach and arm, in place of TASK-1.125's silent close
- [ ] #3 The sdk's typed update carries the final form, the API description and doc-14 describe it, and the drift tests cover it; a client built before this change that meets the final form does not crash
- [ ] #4 The bare window, on the final update, keeps the last screen, shows in words that the program has ended and with what status, and sends no further input; running the app again makes a new tab as it does today
- [ ] #5 Tests at the socket level cover a program that exits by itself, a forced close, and two clients attached to one pane, each with a recorded failing control and passing run; every wait has a bound
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
## Recommendation first: split this into two tasks

This task is "server and app" by title, and the two halves have almost no
shared risk: the wire message and its server-side delivery are heavily
testable at the socket level with no judgement calls; the app's own
reaction is a product/UI decision (an open question below) tangled with
TASK-1.121, landing in the same files, at the same time. Recommending the
default split (per the team's own convention): **Task A** (server+sdk: the
`Ended` wire message, the pane worker's reason-tracking and flush, the
TASK-1.125 race fix, a safe stream half-close in `Outbound`, and every
socket-level test - AC #1, #2, #3, #5) and **Task B** (app: the bare
window's reaction, after Task A lands and after TASK-1.121 is visible to
build against - AC #4). The plan below is written so either task can be
lifted out of it unchanged; it is recorded whole on this task until the
orchestrator actually splits it.

## Design decided by this plan (revised after an advisor review found two
real bugs in the first draft - both from the same root cause: the pane
worker cannot tell *why* it is ending just from which command arrived)

### Bug 1 (now fixed): a forced close was classified as a program exit

`force_end` (runtime.rs:1966) signals via `RunGate` directly (pass 1) and
waits via `RunGate` directly (pass 2) - neither touches the worker's own
command queue. `settle_worker`'s `AwaitExit` is pass **5**, sent long
after the signal. The relay thread (`spawn_relay`) marks `RunGate` dead
*and* sends `PaneCommand::Exited` as two separate steps - for an ordinary
forced close, `Exited` is typically already processed by the worker
*before* `AwaitExit` ever arrives, so `awaiting_exit.is_some()` is `false`
at exactly the moment that matters, and the first draft's classification
(`awaiting_exit.is_some() == closed`) silently read a forced close as a
spontaneous exit - worse, the relative order of pass 2 completing versus
the relay's own `Exited` send is itself racy, so this would have flaked
rather than failed cleanly.

**Fix:** a new command, `PaneCommand::MarkClosing`, sent by a new pass 0
in `force_end` - *before* pass 1's `signal_hangup` loop, over the worker's
own command queue (not `RunGate`). The worker's handler: `if ended.is_none
() { ended = Some(EndReason::Closed); }` (never overwrites an already-
known, more specific reason - see "attach to an already-ended pane"
below). This is strictly ordered ahead of the matching `Exited` by
construction, not by luck: nothing can make the relay thread observe the
program's death and enqueue `Exited` before the signal that kills it is
even sent, and the signal is not sent until pass 1, which cannot start
until pass 0's loop (plain, synchronous, one channel send per handle) has
already returned. `force_end` serves both `close_and_dispatch` (forced
and plain closes alike - the comment at runtime.rs:1130-1139 already notes
`force_end` runs for every popped handle regardless of whether its
program was running) and server stop (the brief's fifth way a pane ends),
so one new pass fixes the reason for all of them, with nothing to change
at either call site. The worker's own `Exited` arm becomes: `let reason =
ended.unwrap_or(EndReason::ProgramExited { exit_code }); ended = Some
(reason);` then `flush_ended(..., reason)` - a spontaneous exit with no
`MarkClosing` ever sent keeps its own, more specific reason.

### Bug 2 (now fixed): an attachment armed after the pane already ended
got nothing, ever - and this is not a narrow race, it is the ordinary
shape of "attach to an already-ended pane"

`pane.attach` on a pane whose program already ended is not refused (R97
keeps the pane and its scrollback around for exactly this) - a client
doing that *at any later time*, not just inside a tight race window, gets
a real last-held snapshot back from `PaneCommand::Attach` (unchanged -
that reply is always honest regardless of `ended`), and then calls `pane.
attach`'s own second step, arm. The first draft's `flush_ended` cleared
`attachments` outright the moment the pane ended, so by the time that
later `Arm` arrived there was nothing left to find - the client's stream
opens and then simply never carries anything, the exact symptom TASK-1.125
exists to prevent, reopened in a new place by this task's own fix for it.

**Fix:** `flush_ended` only removes the attachments it can actually
deliver to right now (`outbound.is_some()`) - every attachment still
`outbound: None` (not yet armed) is left in the `Vec` untouched, and the
worker-local `ended: Option<EndReason>` set alongside it persists. The
`PaneCommand::Arm` handler gains the one new branch this needs: if `ended`
is `Some(reason)`, it does not register the `Outbound` for later ticking
at all - it builds `Ended`'s own frame on the spot, calls `outbound.end_
stream(attachment.stream, bytes)` directly, and removes that one
attachment from the `Vec` (its own stream id, read from the entry the
earlier `Attach` already pushed - never cleared, which is exactly why
`flush_ended` must leave it there). `PaneCommand::Attach` itself needs no
change at all: it already replies with a real snapshot unconditionally,
which is the right behaviour whether or not `ended` is set. `PaneCommand::
Resume` (test-only today, pane.rs:976-998 - no production API calls it
yet) also resets `ended = None`, so a future restart is not permanently
poisoned by this flag; added now, while the comment explaining why is
cheap to write, rather than left as a gap for whichever task wires a
restart API to rediscover.

### The fifth way a pane ends: the server stops

`force_end` is also what server stop uses (runtime.rs's own doc on
`force_end`, "and ... dispatch's own close-with---force wrapper" - server
stop is the other caller). `MarkClosing`'s fix above covers its reason
the same way. What it does *not* fix, and the brief asked this to be said
rather than assumed: on server stop, every connection is also being torn
down at roughly the same time (`ConnectionCleanup`/the listener closing),
so `Outbound::end_stream` can legitimately return `Err(EnqueueDataError::
Closed)` if that connection's own writer has already been told to stop
first - in that case the structured `Ended` frame is not delivered, and
the client learns the pane is gone only from the ordinary socket EOF/
`Incoming::Disconnected` path it already has to handle regardless. This is
accepted, not fixed further: there is no connection left to deliver a
structured message over reliably once its own teardown has started, and
AC #1's guarantee is about panes ending while their attachments' own
connections stay up, which server stop is not.

### `PROTOCOL_VERSION`: no bump, following this project's own precedent

decision-2 section 7 says one integer covers "the framing, the control
envelope and the terminal data representation," matched exactly between
client and server. `PROTOCOL_VERSION` (`crates/sdk/src/wire/handshake.rs:
25`) has been `1` since it was introduced and has never changed since,
including through TASK-1.114's own addition of `Screen.size` - an
equally real shape change to the same "terminal data representation" this
task's new `ScreenUpdate::Ended` variant extends. Followed, not silently
ignored: this plan does not bump it, on the same reasoning TASK-1.114
already set as precedent, and says so explicitly for the engineer to
confirm or overrule, since decision-2's own wording ("matched exactly")
is strict enough that a reviewer could reasonably ask.

### `Outbound::close_stream` drops a queue, synchronously, on call

(Unchanged from the first draft, confirmed sound by the advisor review.)
`close_stream` (`crates/server/src/outbound.rs:179-196`) drops whatever is
still queued for a stream the instant it is called, under its own lock -
calling it right after `enqueue_data` races the writer thread for whether
the just-queued `Ended` frame is ever actually written. `Outbound` gets a
new primitive, `end_stream`, that enqueues the final frame (bypassing
`DATA_CAP` - decision-2 section 6 already treats a backlogged queue's own
content as disposable, and the terminal message cannot be) and marks the
stream for teardown only once the writer thread has actually drained it,
never before.

### The literal wire payload, for the contract-change request below

Externally tagged (serde's default, unchanged from every other variant
here):

```json
{"Ended":{"reason":{"ProgramExited":{"exit_code":3}}}}
{"Ended":{"reason":"Closed"}}
```

(`EndReason::Closed` is a unit variant, serialised as a bare string by
serde's own default; `ProgramExited` carries a field, serialised as the
usual `{"Variant":{...}}`.)

## Files that change

### Task A - wire and server

- `crates/sdk/src/screen.rs`: add `EndReason { ProgramExited { exit_code:
  i32 }, Closed }` and `ScreenUpdate::Ended { reason: EndReason }`; round-
  trip tests for both new shapes and for the literal JSON above (asserting
  the exact tag shapes, the same way `pane_attached_round_trips_and_
  carries_both_fields_in_one_object` asserts a concrete shape rather than
  only round-tripping).
- `crates/sdk/src/lib.rs`: export `EndReason` alongside `ScreenUpdate` in
  the `pub use screen::{...}` list.
- `crates/sdk/src/description/methods.rs`: reword `pane.attach`'s own
  `details`/`follows.item.summary` to mention the terminal update - no
  shape change; `every_nested_reply_and_stream_item_names_a_real_sdk_type`
  stays green untouched (the name `"ScreenUpdate"` does not change).
- `crates/server/src/outbound.rs`: add `Queues::closing: HashSet<u32>` and
  `Outbound::end_stream(&self, stream: u32, bytes: Vec<u8>) -> Result<(),
  EnqueueDataError>` - clears `stream`'s own existing queue, pushes `bytes`
  unconditionally (bypassing `DATA_CAP`; refuses only on `EnqueueDataError
  ::Closed`/`NotOpen`, the same as `enqueue_data`), and marks it `closing`.
  `Queues::pop_next` (or its caller in `spawn_writer`) checks, right after
  popping a data frame, whether that stream's own queue is now empty and
  it is marked `closing`; if so, removes it from `data`/`order`/`closing`
  there, under the same lock, the same tear-down `close_stream` already
  does. New unit tests: the final frame survives a full queue; a stream
  with nothing else queued tears down at once; a stream with more still
  queued behind the final frame is not torn down until the writer actually
  drains it (a real two-thread proof, not a single-threaded call sequence,
  or this is exactly the race this change exists to close).
- `crates/server/src/pane.rs`:
  - New worker-local state in `worker_loop`: `let mut ended: Option<
    EndReason> = None;` alongside `attachments`/`tracked_seq`.
  - New free function (beside `run_tick`): `fn flush_ended(terminal: &mut
    Terminal, tracked_seq: &mut u64, attachments: &mut Vec<Attachment>,
    reason: EndReason)` - runs one last `run_tick` (so AC #1's "after any
    screen update still owed" is not left to a timer that will never fire
    again - `PtyBytes` only *arms* a future tick, `maybe_arm_tick`, it does
    not run one), serialises `ScreenUpdate::Ended { reason }` once, and
    `attachments.retain_mut`s: every attachment with `outbound.is_some()`
    gets the frame via `outbound.end_stream(...)` and is removed; every
    attachment still `outbound: None` (not yet armed) is left exactly as
    it is, for the `Arm` handler below to find later.
  - New `PaneCommand::MarkClosing` (no payload, no reply - the same non-
    blocking, no-stall shape `PaneHandle::signal_hangup`'s own *command*
    -queue siblings already use for a fire-and-forget worker message) and
    `PaneHandle::mark_closing(&self)`, sent by `force_end`'s new pass 0.
    Handler: `if ended.is_none() { ended = Some(EndReason::Closed); }`.
  - `worker_loop`'s `PaneCommand::Exited` arm: before sending `PaneEvent::
    Exited`, set `next_tick = None`, compute `let reason = ended.unwrap_or
    (EndReason::ProgramExited { exit_code }); ended = Some(reason);`, then
    `flush_ended(&mut terminal, &mut tracked_seq, &mut attachments, reason
    )`.
  - `PaneCommand::Arm` handler: if `ended` is `Some(reason)`, do not set
    `attachment.outbound`/call `maybe_arm_tick` at all - find the
    attachment by `id` (it is still present; see `flush_ended`'s own doc
    for why), remove it, build `Ended`'s frame for its own `stream`, call
    `outbound.end_stream(...)`. Unchanged (the existing body) when `ended`
    is `None`.
  - `PaneCommand::Resume` handler: also sets `ended = None` (test-only
    reachable today - no production caller - added for the same "close
    the gap now, cheaply" reasoning the file's own existing comments use
    elsewhere, e.g. `RunGate`'s `generation`).
  - The `AwaitExit` arm's immediate-reply branch (nothing running) and the
    `Shutdown` arm: each also calls `flush_ended(..., ended.unwrap_or
    (EndReason::Closed))` before replying/breaking - defensive, since both
    are reached today only after an exit already flushed and left only
    not-yet-armed attachments behind (if any), but closes the gap if a
    later caller ever reaches either differently.
  - New unit tests, same style as the existing `fn next_update`/`fn apply_
    update` test helpers in this file's own `#[cfg(test)] mod tests`: a
    program that exits on its own sends exactly one `Ended{ProgramExited}`
    per attachment, with the real exit code, after its last real content;
    `MarkClosing` sent before a program's own natural exit still reports
    `Closed`, not `ProgramExited`, proving the ordering, not just the
    single-threaded logic; attaching to a pane that already ended (`ended`
    already `Some` before `Attach`/`Arm` ever run) gets `Ended` on arm,
    immediately, with no tick involved; `flush_ended` under a full queue
    still delivers it.
- `crates/server/src/runtime.rs`: `force_end`'s new pass 0 (`for (_, handle
  ) in &handles { handle.mark_closing(); }`), ahead of today's pass 1.
- `crates/server/src/connection.rs`: `finish_attach`'s `armed == false`
  branch (the TASK-1.125 race - the pane's *worker itself* was already gone
  by the time `arm_attachment` ran, a different case from this task's own
  "pane ended but worker still alive" one above, and still fixed at the
  connection layer, which is the only layer that still has both the
  `Outbound` and the `Connection` in hand there) builds `ScreenUpdate::
  Ended { reason: EndReason::Closed }`, encodes one frame for `stream`, and
  calls `outbound.end_stream(stream, bytes)` in place of today's bare
  `outbound.close_stream(stream)` - `connection.close_stream(stream)` stays,
  unchanged, right after. Existing unit test for this function (TASK-1.125's
  own) updated to assert the frame is sent before the close, not just that
  `armed == false` closes without panicking.
- `crates/server/tests/attach.rs`: new socket-level tests, reusing `start_
  server`/`connect_and_handshake`/`new_pane_running`/`attach`/`next_update`
  /`apply_update`/`screen_text` and the `pane.close`/`force: true` call
  shape the forced-close tests already use:
  - a program that exits by itself (`sh -c "echo done; exit 3"`): the
    attached client's last `next_update` is `Ended{ProgramExited{exit_code
    : 3}}`, and a bounded quiet-window read afterward (the same shape
    `detach_ends_the_stream_even_though_the_program_keeps_producing_output`
    already uses) sees nothing more, decoded as `UnknownStream` on this
    connection's own now-closed side;
  - `pane.close --force` on a pane with one attached, still-running client:
    the client's last update is `Ended{Closed}` - this is the test that
    pins down bug 1 above; recorded failing against the first draft's own
    `awaiting_exit`-based classification before `MarkClosing` exists, to
    prove it actually catches the bug, not just that the final code passes;
  - two clients attached to one pane (the shape `two_connections_attached_
    to_one_pane_...` already sets up): both get `Ended` for the same
    program exit, independently, before either stream goes quiet;
  - attaching to a pane whose program has already ended, well after the
    fact (no race, no flood - a plain second `pane.attach` once the first
    client has already seen `Ended`): the new client's own stream carries
    exactly one `Ended` frame and nothing else - this is the test that
    pins down bug 2 above, and is distinct from the next one;
  - the TASK-1.125 race itself, reusing `a_pane_that_closes_between_a_
    queued_attach_and_its_arm_leaves_nothing_to_detach`'s own flood/force
    -close setup: connection two's own stream carries one `Ended{Closed}`
    frame before it goes quiet, replacing today's assertion that nothing
    at all arrives.
  Every wait bounded, per this file's own existing convention (`reply_
  within`, bounded quiet-window polling) - no unbounded `recv()`/blocking
  read added anywhere.

### Task B - the bare window

- `crates/app/src/client.rs`: `ClientEvent` gains `Ended(steg_sdk::
  EndReason)` beside `Update`/`Disconnected`; `drain`'s `StreamControl` arm
  matches `ScreenUpdate::Ended { reason }` into `ClientEvent::Ended(reason)`
  rather than folding it into `Update`, and also calls `connection.close_
  stream(stream)` (the client's own half of "closed on both sides"). New
  test mirroring `a_control_frame_on_the_open_stream_arrives_as_an_update`,
  for `Ended`.
- `crates/app/src/client.rs` (AC #3's "does not crash"): a test that feeds
  a raw JSON object shaped like a *future* unknown `ScreenUpdate` variant
  (not `Ended` - a made-up one, so this proves the general case, not just
  today's addition) on the open stream and asserts `ClientEvent::
  Disconnected` results, never a panic - the concrete proof that this app's
  own decode-error handling in `drain` (`Err(_) => return false`) already
  satisfies AC #3 by construction; no production code change needed for
  this one, it only needed to be pinned down with a test.
- `crates/app/src/model.rs` or a new small, plain-data module beside it (no
  `gpui::Window`/`Context`, matching this file's own stated reason for
  being testable without a window): a pure function, e.g. `fn describe_end
  (reason: &steg_sdk::EndReason) -> String`, returning the exact text
  ("the program ended, status 3" / "the pane was closed") - pulled out so
  AC #4's own wording is unit-tested directly, not only through a live run.
- `crates/app/src/main.rs`: `AttachedState` gains `ended: Option<steg_sdk::
  EndReason>`; `handle_client_event`'s `ClientEvent::Ended` arm sets it and
  notifies, *not* `Disconnected` (the window stays `Attached`, keeping the
  model/last screen exactly as it is - AC #4's "keeps the last screen");
  `Render`'s `Phase::Attached` arm appends one plain text line below the
  terminal, built from `describe_end`, when `ended.is_some()` - reusing
  `placeholder`'s own colour tokens, not a new design-system component
  (TASK-1.61's job, explicitly out of scope per this task's own
  description).
- Key handling: **open question for the engineer, not decided here** -
  TASK-1.121 (typing, landing in `main.rs`/possibly `terminal_element.rs`,
  in progress on another worker, not yet in this worktree) is what adds
  the only path bytes could reach the server on today's bare window. This
  task cannot name the exact guard until that lands. Default, smallest
  behaviour if nothing else is decided: once `ended` is `Some`, the key
  handler short-circuits before calling whatever TASK-1.121 adds to send
  input - a one-line guard at the top of that handler, not a redesign.
  Alternatives: disable the window's own focus handle entirely (loses
  scroll/copy, if either exists by then); let keys reach the terminal
  harmlessly (wrong per AC #4's "sends no further input" - the server-side
  stream is already torn down by Task A's own fix, so a key sent here
  writes to a stream nothing will ever read, silently, which is worse than
  visibly refusing it).
- `running the app again makes a new tab`: already true, unchanged -
  `startup.rs`'s `resolve_pane` already checks `pane.list`'s own `program_
  status != "running"` and opens a fresh tab in that case, independent of
  this task's wire change (confirmed by reading `resolve_pane` directly; no
  file in this bullet needs to change for AC #4's last clause).
- AC #4's own proof, decided rather than hedged: `describe_end`'s own unit
  tests (exact wording, both reasons) plus `client.rs`'s `Ended` test are
  the automated proof; `main.rs` has no `#[cfg(test)]` module today and
  this plan does not add one just for a render assertion - the live run
  below is AC #4's visual proof, not a fallback for a missing automated
  one.
- Live run (AC #5's own "proof" half, for this part only): with the real
  app and server, start a shell pane, run `exit 3` in it, and screenshot
  the window showing the last screen plus the new text line, and the same
  for `steg pane close --force` on the attached pane from a second
  terminal.

## Contract-change request (for the orchestrator, not done by this plan)

- `doc-14` ("Spec: wire protocol"): the "An update is..." paragraph in
  section "The wire format, in enough detail to build from" needs a third
  bullet for the final form - its two reasons, the literal JSON shown
  above, and "sent once, after any update still owed, before the stream
  closes on both sides."
- `decision-2` section 3: the ordering rule box ("queue the attach result
  first, register the attachment's queue second") should gain the
  symmetric rule this task adds ("queue the last frame before tearing the
  stream down, never the reverse") - recorded as an addendum, not a
  rewrite of an accepted decision, per the team's own decision-records-
  by-pull-request convention.
- Whether `PROTOCOL_VERSION` really should stay unbumped given decision-2
  section 7's "matched exactly" wording - this plan follows TASK-1.114's
  own precedent (no bump) but flags it rather than deciding it silently,
  since section 7's own wording is strict enough to read either way.
- None of these three are edited by this plan; flagging for the engineer
  to fold in, likely in the same PR that adds `Outbound::end_stream`.

## Order of work

1. `screen.rs`/`lib.rs`: add `EndReason`/`ScreenUpdate::Ended` and their
   round-trip tests, including the literal-JSON assertions. Compiles and
   tests green with no other file touched yet.
2. `outbound.rs`: add `Queues::closing` and `Outbound::end_stream`, with
   its own three unit tests (full queue, immediate teardown, deferred
   teardown proven with a real second thread). No caller yet.
3. `pane.rs`: `ended`, `flush_ended`, `MarkClosing`, the `Exited`/`Arm`/
   `Resume`/`AwaitExit`/`Shutdown` changes, with unit tests covering both
   bugs the advisor found directly (ordering of `MarkClosing` before a
   real exit; attach-after-end).
4. `runtime.rs`: `force_end`'s new pass 0.
5. `connection.rs`: `finish_attach`'s `armed == false` branch calls `end_
   stream` instead of `close_stream`; update its own existing unit test.
6. `attach.rs`: the five new socket-level tests (program exit, forced
   close, two clients, attach-to-an-already-ended-pane, the TASK-1.125
   race), each recorded failing first against the pre-step-3/4/5 code,
   then passing.
7. `methods.rs`: reword `pane.attach`'s own description text. No test
   change expected.
8. (Task B, once Task A is merged and TASK-1.121's own landed diff is
   readable) `client.rs`: `ClientEvent::Ended`, the drain change, the two
   new tests.
9. `describe_end` (new, small, plain-data) with its own unit tests.
10. `main.rs`: `ended` state, the render line, the key guard (exact shape
    decided against TASK-1.121's real code, not this plan's guess).
11. Live run against the real app and server; screenshots attached to the
    PR per the team's own UI-PR convention.

## Risks

- **`Outbound::end_stream`'s own deferred-teardown race has no existing
  precedent in this codebase to copy.** Mitigated by writing its three
  unit tests before wiring any caller to it (step 2 before 3/5), with the
  deferred-teardown test using a real, independently-driven writer thread,
  not a single-threaded call sequence that could pass by accident.
- **`MarkClosing`'s own ordering guarantee rests on `force_end`'s pass 0
  being a plain, synchronous loop that fully returns before pass 1 starts
  sending real OS signals** - true today by construction (`force_end`'s
  own code, read directly), but a future refactor that parallelises passes
  0 and 1 (the same way passes 2-6 already use `thread::scope`, for
  throughput) would silently reopen bug 1. Mitigated by a doc comment on
  `mark_closing`'s own call site naming this dependency explicitly, the
  same way `flush_ended`'s doc names why unarmed attachments must survive
  it.
- **TASK-1.121 lands in the same three app files at close to the same
  time.** Mitigated by Task B starting only after both TASK-1.121 and this
  task's own Task A are merged, and by keeping Task B's own diff to the
  bullets above, nothing wider.
- **A pane that never had any attachment at all when it ends** costs this
  change nothing extra (`flush_ended` iterates an empty `Vec`) - checked by
  reasoning, not singled out as its own test, since it is the same code
  path as every other case with zero iterations.
- **Server stop's own best-effort delivery** (named above, not fixed
  further) is a real, accepted gap against AC #1's literal wording for
  that one case - flagged for the engineer to confirm is acceptable, not
  silently narrowed.

## Proof

- `just check` and `just test` (server, sdk, app crates and their
  dependents, per this diff).
- Each acceptance criterion:
  - #1: `attach.rs`'s new program-exit and forced-close tests, each with
    its own failing-before/passing-after control (step 6) - the forced-
    close one specifically pins down the advisor's bug 1.
  - #2: `attach.rs`'s updated TASK-1.125 race test, same failing/passing
    control.
  - #3: `screen.rs`'s round-trip and literal-JSON tests, `methods.rs`'s
    unchanged `every_nested_reply_and_stream_item_names_a_real_sdk_type`,
    and `client.rs`'s decode-robustness test.
  - #4: `describe_end`'s unit tests, `client.rs`'s `Ended` test, and the
    live run (screenshotted).
  - #5: all five `attach.rs` tests bounded (no `recv()`/blocking read
    without a timeout anywhere added), each recorded via `asdlc record-
    check`.

## Corrections after a second advisor review (fix these three before building; the plan above is otherwise confirmed sound)

1. **The "defensive" `flush_ended` call on the `AwaitExit` arm's
   immediate-reply branch is dead code as written - drop it.** `current.
   is_none()` there is only ever reached *after* a `PaneCommand::Exited`
   has already run (`current` only ever becomes `None` inside that arm's
   own `current.take()`), and that earlier `Exited` already ran `flush_
   ended` - every armed attachment is already gone and every unarmed one
   is deliberately left for `Arm` to handle. Calling `flush_ended` again
   here has nothing left to iterate that `outbound.is_some()`, ever. Do
   not add this call.
2. **Keep the `Shutdown` arm's own `flush_ended` call - it is not dead,
   unlike the one above.** `PaneHandle::shutdown` can be called while a
   program is still running and attachments are still armed (its own doc:
   ending the worker this way "does not reliably end a still-running
   program" - the loop simply breaks). No production caller does this
   today, but `pane.rs`'s own unit tests do (`handle.shutdown()`, three
   call sites in `#[cfg(test)]`), so this is a real, exercised path, not
   a hypothetical one, and the flush there has real work to do.
3. **Reword the planned pane.rs unit test** ("`MarkClosing` sent before a
   program's own natural exit still reports `Closed`... proving the
   ordering") - a single-threaded unit test proves the worker's own `if
   ended.is_none()` precedence logic, not `force_end`'s pass-0-before-
   pass-1 ordering, which is only a real property at the socket level.
   The actual ordering proof is `attach.rs`'s forced-close test (step 6);
   the pane.rs test is a narrower, useful check of the flag logic alone,
   and should be described as that.
<!-- SECTION:PLAN:END -->
