---
id: TASK-1.128
title: 'Server and app: an attached client is told on its stream when its pane ends'
status: In Progress
assignee:
  - '@bruk.w4'
created_date: '2026-11-17T03:51:00Z'
updated_date: '2026-11-17T22:30:00Z'
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
`Ended` wire message, the pane worker's flush-on-exit/close, the
TASK-1.125 race fix, a safe stream half-close in `Outbound`, and every
socket-level test) and **Task B** (app: the bare window's reaction, after
Task A lands and after TASK-1.121 is visible to build against). The plan
below is written so either task can be lifted out of it unchanged; it is
recorded whole on this task until the orchestrator actually splits it.

## Design decided by this plan (not in the task's own wording)

- Two reasons, not one with an optional field: `EndReason::ProgramExited
  { exit_code }` when the program ended on its own with nothing waiting
  for it, and `EndReason::Closed` (no exit code) whenever the pane's own
  worker is ending because `Runtime::close_and_dispatch`/`force_end` asked
  it to - `crates/server/src/pane.rs`'s own `awaiting_exit: Option<Sender
  <()>>` is already exactly that signal (only `force_end`'s own `settle_
  worker` ever sets it) and needs no new state. A forced close's own real
  exit code (SIGHUP/SIGKILL) is not carried under `Closed`: once the pane
  is gone from the tree, the signal that ended its program is an
  implementation detail, not the fact worth telling a client. Flag this
  for the engineer if they want the code kept anyway.
- `Outbound::close_stream` (`crates/server/src/outbound.rs:179-196`)
  **drops whatever is still queued for a stream**, synchronously, under
  its own lock - calling it right after `enqueue_data` races the writer
  thread for whether the just-queued `Ended` frame is ever actually
  written, exactly the kind of bug decision-2 section 3's own "two rules
  ... learned the hard way" names. Nothing in this crate already does
  enqueue-then-close back to back with something worth keeping in the
  queue (`pane.detach`/today's `finish_attach` close with nothing pending)
  - this task is the first caller that needs "deliver this one last frame,
  *then* tear the stream down," so `Outbound` gets a new primitive for it
  rather than two existing calls stitched together.
- A flooding attachment's queue can be at `DATA_CAP` (64) at the exact
  moment the pane ends. The new primitive does not drop the terminal
  message in that case: it clears whatever was already queued for that
  stream (decision-2 section 6 already treats a backlogged queue's own
  content as disposable catch-up data) and queues only `Ended`, bypassing
  the cap - the one guarantee AC #1 needs ("every client ... receives one
  final update") cannot be best-effort.
- The pane worker (`pane.rs`) has no reference to a connection's own
  `Connection`/`pane_streams`/`stream_panes` (different thread, not
  shared) - only to the `Arc<Outbound>` each armed `Attachment` already
  holds. So the worker's own half of this fix only ever closes the
  *outbound* (write) side. The connection's own read-side bookkeeping is
  left to go stale until that connection's next `pane.detach` (a no-op
  against an already-cleared worker attachment, same as today) or its own
  teardown (`ConnectionCleanup::drop`, unchanged) - both already tolerate
  a stale entry (`PaneHandle::detach`/`Runtime::forget_attachment`'s own
  docs: "a no-op if `id` is not currently registered"). A client that
  (incorrectly) sends input after `Ended` reaches
  `PaneHandle::send_input`, documented as "a no-op, not an error, if no
  program is currently running" - it does not end the connection the way
  a frame on a truly unregistered stream would. `connection.rs`'s own
  `finish_attach` (the TASK-1.125 race) *does* have the `Connection`
  object in hand and keeps closing both sides there, unchanged from today.
- Before building the `Ended` payload, the worker's `Exited` arm runs one
  more `run_tick` synchronously (clearing any armed `next_tick` first) -
  AC #1's "after any screen update still owed" is not automatic: `PtyBytes`
  only *arms* a future tick deadline (`maybe_arm_tick`), it does not run
  one, so the program's last output could otherwise be dropped on the
  floor the moment attachments are cleared.

## Files that change

### Task A - wire and server

- `crates/sdk/src/screen.rs`: add `EndReason { ProgramExited { exit_code:
  i32 }, Closed }` and `ScreenUpdate::Ended { reason: EndReason }`; a
  `round_trips` test for each new shape, matching the existing `screen_
  update_full_round_trips`/`_delta_round_trips` pattern.
- `crates/sdk/src/lib.rs`: export `EndReason` alongside `ScreenUpdate` in
  the `pub use screen::{...}` list.
- `crates/sdk/src/description/methods.rs`: reword `pane.attach`'s own
  `details`/`follows.item.summary` to mention the terminal update - no
  shape change, `every_nested_reply_and_stream_item_names_a_real_sdk_type`
  stays green untouched (the name `"ScreenUpdate"` does not change).
- `crates/server/src/outbound.rs`: add `Queues::closing: HashSet<u32>` and
  `Outbound::end_stream(&self, stream: u32, bytes: Vec<u8>) -> Result<(),
  EnqueueDataError>` - clears `stream`'s own existing queue, pushes
  `bytes` unconditionally (bypassing `DATA_CAP`; refuses only on
  `EnqueueDataError::Closed`/`NotOpen`, the same as `enqueue_data`), and
  marks it `closing`. `Queues::pop_next` (or its caller in `spawn_writer`)
  checks, right after popping a data frame, whether that stream's own
  queue is now empty and it is marked `closing`; if so, removes it from
  `data`/`order`/`closing` there, under the same lock, the same tear-down
  `close_stream` already does. New unit tests: the final frame survives a
  full queue; a stream with nothing else queued tears down at once; a
  stream with more still queued behind the final frame is not torn down
  until the writer actually drains it (a real two-thread proof, not a
  single-threaded call sequence, or this is exactly the race this change
  exists to close).
- `crates/server/src/pane.rs`:
  - New free function (beside `run_tick`): `fn flush_ended(terminal: &mut
    Terminal, tracked_seq: &mut u64, attachments: &mut Vec<Attachment>,
    reason: EndReason)` - runs one last `run_tick`, serialises `ScreenUpdate
    ::Ended { reason }` once, and for every attachment with `outbound.
    is_some()` calls `outbound.end_stream(attachment.stream, bytes.clone
    ())`; then `attachments.clear()`.
  - `worker_loop`'s `PaneCommand::Exited` arm: before sending `PaneEvent::
    Exited` (ordering does not matter between these two - different
    channels, different consumers - but after is simplest to read), set
    `next_tick = None` and call `flush_ended(..., if awaiting_exit.is_some
    () { EndReason::Closed } else { EndReason::ProgramExited { exit_code
    } })`.
  - The `AwaitExit` arm's immediate-reply branch (nothing running) and the
    `Shutdown` arm: each also calls `flush_ended(..., EndReason::Closed)`
    before replying/breaking - defensive, since both are reached today
    only after an exit already flushed and cleared `attachments`, but
    cheap on an empty `Vec` and closes the gap if a later caller ever
    reaches either with live attachments still held.
  - New unit tests, same style as the existing `fn next_update`/`fn apply_
    update` test helpers in this file's own `#[cfg(test)] mod tests`: a
    program that exits on its own sends exactly one `Ended{ProgramExited}`
    per attachment, with the real exit code, after its last real content;
    `flush_ended` under a full queue still delivers it (drives `Outbound`
    directly, no real pty needed).
- `crates/server/src/connection.rs`: `finish_attach`'s `armed == false`
  branch (the TASK-1.125 race) builds `ScreenUpdate::Ended { reason:
  EndReason::Closed }`, encodes one frame for `stream`, and calls `outbound
  .end_stream(stream, bytes)` in place of today's bare `outbound.close_
  stream(stream)` - `connection.close_stream(stream)` stays, unchanged,
  right after (this call site keeps a `&mut Connection`, unlike the
  worker). Existing unit test for this function (TASK-1.125's own) updated
  to assert the frame is sent before the close, not just that `armed ==
  false` closes without panicking.
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
    the client's last update is `Ended{Closed}`, no exit code asserted;
  - two clients attached to one pane (the shape `two_connections_attached_
    to_one_pane_...` already sets up): both get `Ended` for the same
    program exit, independently, before either stream goes quiet;
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
  EndReason)` beside `Update`/`Disconnected`; `drain`'s `StreamControl`
  arm matches `ScreenUpdate::Ended { reason }` into `ClientEvent::Ended
  (reason)` rather than folding it into `Update`, and also calls `connection
  .close_stream(stream)` (the client's own half of "closed on both sides").
  New test mirroring `a_control_frame_on_the_open_stream_arrives_as_an_
  update`, for `Ended`.
- `crates/app/src/client.rs` (AC #3's "does not crash"): a test that feeds
  a raw JSON object shaped like a *future* unknown `ScreenUpdate` variant
  (not `Ended` - a made-up one, so this proves the general case, not just
  today's addition) on the open stream and asserts `ClientEvent::
  Disconnected` results, never a panic - the concrete proof that this
  app's own decode-error handling in `drain` (`Err(_) => return false`)
  already satisfies AC #3 by construction; no production code changes
  needed for this one, it only needed to be pinned down with a test.
- `crates/app/src/main.rs`: `AttachedState` (or a new field alongside it)
  gains `ended: Option<steg_sdk::EndReason>`; `handle_client_event`'s
  `ClientEvent::Ended` arm sets it and notifies, *not* `Disconnected` (the
  window stays `Attached`, keeping the model/last screen exactly as it
  is - AC #4's "keeps the last screen"); `Render`'s `Phase::Attached` arm
  appends one plain text line below the terminal when `ended.is_some()`
  ("the program ended, status 3" / "the pane was closed"), reusing
  `placeholder`'s own colour tokens, not a new design-system component
  (that is TASK-1.61's job, explicitly out of scope per this task's own
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
  harmlessly (wrong per AC #4's "sends no further input", since the
  stream is closed server-side already - a key sent here is written to a
  stream no server-side worker will ever read, silently, which is worse
  than visibly refusing it).
- `running the app again makes a new tab`: already true, unchanged -
  `startup.rs`'s `resolve_pane` already checks `pane.list`'s own `program_
  status != "running"` and opens a fresh tab in that case, independent of
  this task's wire change (confirmed by reading `resolve_pane` directly;
  no file in this bullet needs to change for AC #4's last clause).
- Live run (AC #5's own "proof" half, for this part only): with the real
  app and server, start a shell pane, run `exit 3` in it, and screenshot
  the window showing the last screen plus the new text line, and the same
  for `steg pane close --force` on the attached pane from a second
  terminal.

## Contract-change request (for the orchestrator, not done by this plan)

- `doc-14` ("Spec: wire protocol"): the "An update is..." paragraph in
  section "The wire format, in enough detail to build from" needs a third
  bullet for the final form (`Ended`, its two reasons, and "sent once,
  before the stream closes on both sides").
- `decision-2` section 3: the ordering rule box ("queue the attach result
  first, register the attachment's queue second") should gain the
  symmetric rule this task adds ("queue the last frame before tearing the
  stream down, never the reverse") - recorded as an addendum, not a
  rewrite of an accepted decision, per the team's own decision-records-
  by-pull-request convention.
- Neither is edited by this plan; flagging for the engineer to fold in,
  likely in the same PR that adds `Outbound::end_stream`.

## Order of work

1. `screen.rs`/`lib.rs`: add `EndReason`/`ScreenUpdate::Ended` and their
   round-trip tests. Compiles and tests green with no other file touched
   yet - the smallest possible first step with its own feedback loop.
2. `outbound.rs`: add `Queues::closing` and `Outbound::end_stream`, with
   its own three unit tests (full queue, immediate teardown, deferred
   teardown proven with a real second thread). No caller yet.
3. `pane.rs`: `flush_ended`, wired into the three `worker_loop` arms, with
   `run_tick`'s own `next_tick` cleared first. Unit tests against a real
   pty program that exits on its own (this file's own existing harness).
4. `connection.rs`: `finish_attach`'s `armed == false` branch calls `end_
   stream` instead of `close_stream`; update its own existing unit test.
5. `attach.rs`: the four new socket-level tests (program exit, forced
   close, two clients, the TASK-1.125 race), each recorded failing first
   against the pre-step-3/4 code (a repo stash or a deliberately reverted
   call), then passing.
6. `methods.rs`: reword `pane.attach`'s own description text. No test
   change expected (`every_nested_reply_and_stream_item_names_a_real_sdk_
   type` only checks the name, unaffected).
7. (Task B, once Task A is merged and TASK-1.121's own landed diff is
   readable) `client.rs`: `ClientEvent::Ended`, the drain change, the two
   new tests.
8. `main.rs`: `ended` state, the render line, the key guard (exact shape
   decided against TASK-1.121's real code, not this plan's guess).
9. Live run against the real app and server; screenshots attached to the
   PR per the team's own UI-PR convention.

## Risks

- **The `Outbound::end_stream` race is the one piece of this plan with no
  existing precedent to copy** - everything else in this task reuses a
  shape the codebase already has (the worker's own `flush`-then-`clear`
  mirrors `run_tick`'s own per-tick loop; the connection-side fix mirrors
  today's `finish_attach` almost line for line). Mitigated by writing
  `end_stream`'s own three unit tests *before* wiring any caller to it
  (step 2 before 3/4), and by the deferred-teardown test using a real,
  independently-driven writer thread, not a single-threaded call sequence
  that could pass by accident.
- **TASK-1.121 lands in the same three app files at close to the same
  time.** Mitigated by Task B starting only after both TASK-1.121 is
  merged and this task's own Task A is merged, and by keeping Task B's
  own diff to the three bullets above, nothing wider.
- **Reason classification (`awaiting_exit.is_some()` meaning "this is a
  close, not a spontaneous exit") depends on exactly one production
  caller setting it (`force_end`'s `settle_worker`).** If a later task
  adds a second caller of `PaneCommand::AwaitExit` for a different reason,
  this classification silently goes wrong. Mitigated by the doc comment on
  `flush_ended`'s own call site naming this dependency explicitly, so a
  future change finds it by reading, not by a bug report.
- **A pane that never had any attachment at all when it ends** costs this
  change nothing extra (`flush_ended` iterates an empty `Vec`) - checked
  by reasoning, not singled out as its own test, since it is the same code
  path as every other case with zero iterations.

## Proof

- `just check` and `just test` (server, sdk, app crates and their
  dependents, per this diff).
- Each acceptance criterion:
  - #1: `attach.rs`'s new program-exit test, with its own failing-before
    /passing-after control (step 5).
  - #2: `attach.rs`'s updated TASK-1.125 race test, same failing/passing
    control.
  - #3: `screen.rs`'s two new round-trip tests, `methods.rs`'s unchanged
    `every_nested_reply_and_stream_item_names_a_real_sdk_type`, and `client
    .rs`'s new decode-robustness test.
  - #4: `main.rs`'s render test (if the app crate's own test harness covers
    rendered text today - checked at build time; otherwise the live run
    below stands in, screenshotted) plus the live run.
  - #5: all four `attach.rs` tests bounded (no `recv()`/blocking read
    without a timeout anywhere added), each recorded via `asdlc record-
    check`.
<!-- SECTION:PLAN:END -->
