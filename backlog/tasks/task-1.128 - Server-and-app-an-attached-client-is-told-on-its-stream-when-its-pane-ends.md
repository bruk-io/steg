---
id: TASK-1.128
title: 'Server and app: an attached client is told on its stream when its pane ends'
status: In Progress
assignee:
  - '@bruk.w4'
created_date: '2026-11-17T03:51:00Z'
updated_date: '2026-11-17T23:31:00Z'
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
asdlc_plan_approved_by: bruk
asdlc_plan_approved_hash: 'ffc63839f742'
asdlc_plan_approved_size: size-3
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
- [ ] #4 Tests at the socket level cover a program that exits by itself, a forced close, and two clients attached to one pane, each with a recorded failing control and passing run; every wait has a bound
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
## Scope (after the split)

Server and sdk only now: the `Ended` wire message, the pane worker's
reason-tracking and flush, the TASK-1.125 race fix, a safe stream half-
close in `Outbound`, and every socket-level test (AC #1, #2, #3, #4 - the
window's own reaction moved to DRAFT-1, which depends on this task and on
TASK-1.121). This task's own diff touches one line in the app crate for
the sole reason that adding an enum variant anywhere in the workspace
obligates fixing every match it breaks, everywhere, including crates this
task does not otherwise change - see "Every exhaustive match this breaks"
below; nothing else in `crates/app` is this task's job.

## Design

### Two reasons, carried by one new sdk type

`EndReason { ProgramExited { exit_code: i32 }, Closed }`, and `ScreenUpdate
::Ended { reason: EndReason }`. `ProgramExited` when the program ended
with nothing waiting for it; `Closed` (no exit code) whenever the pane's
own worker is ending because `Runtime::close_and_dispatch`/`force_end`
asked it to - a forced close's own real exit code (SIGHUP/SIGKILL) is not
carried under `Closed`: once the pane is gone from the tree, the signal
that ended its program is an implementation detail, not the fact worth
telling a client. Named as an open question for the engineer below, not
decided unilaterally.

### Classifying the reason correctly - the bug an advisor review found,
and the fix

The first draft of this plan classified "closed vs. exited" by whether
`PaneCommand::AwaitExit`'s own `awaiting_exit: Option<Sender<()>>` was set
when `Exited` arrived. That is wrong, and would have flaked rather than
failed cleanly: `force_end` (runtime.rs:1966) signals via `RunGate`
directly (pass 1) and waits via `RunGate` directly (pass 2) - neither
touches the worker's own command queue - and `settle_worker`'s own
`AwaitExit` is pass **5**, sent long after the signal. The relay thread
(`spawn_relay`) marks `RunGate` dead *and* sends `PaneCommand::Exited` as
two separate steps; for an ordinary forced close, `Exited` is typically
already processed by the worker *before* `AwaitExit` ever arrives, so
`awaiting_exit.is_some()` is `false` at exactly the moment that matters -
a forced close would have been read as a spontaneous exit, and the
relative order of pass 2 completing versus the relay's own `Exited` send
is itself racy on top of that.

**Fix:** a new command, `PaneCommand::MarkClosing` (no payload, no reply -
the same non-blocking, fire-and-forget shape `PaneHandle::signal_hangup`'s
own command-queue siblings already use), sent by a new pass 0 in `force_
end` - *before* pass 1's `signal_hangup` loop, over the worker's own
command queue (not `RunGate`). The worker's handler: `if ended.is_none()
{ ended = Some(EndReason::Closed); }` on a new worker-local `let mut
ended: Option<EndReason> = None;` (never overwrites an already-known, more
specific reason - see "attach to an already-ended pane" below). This is
strictly ordered ahead of the matching `Exited` by construction: nothing
can make the relay thread observe the program's death and enqueue `Exited`
before the signal that kills it is even sent, and the signal is not sent
until pass 1, which cannot start until pass 0's own plain, synchronous
loop (one channel send per handle) has already returned. `force_end`
serves both `close_and_dispatch` (forced and plain closes alike - the
comment at runtime.rs:1130-1139 already notes it runs for every popped
handle regardless of whether its program was running) and server stop
(`Runtime::stop`, runtime.rs:892, confirmed by reading both call sites) -
one new pass fixes the reason for both, with nothing to change at either
call site. The worker's own `Exited` arm becomes: `let reason = ended.
unwrap_or(EndReason::ProgramExited { exit_code }); ended = Some(reason);`
then `flush_ended(..., reason)` - a spontaneous exit with no `MarkClosing`
ever sent keeps its own, more specific reason.

### Attach to an already-ended pane is not a race, it is the ordinary way
of looking at one - the second bug an advisor review found

A pane's own program ending does not close the pane (R97 keeps it and its
scrollback around). A client calling `pane.attach` on such a pane, at any
later time, gets a real last-held snapshot back (`PaneCommand::Attach`'s
own reply is always honest, unchanged by this task either way) and then
arms it. The first draft's `flush_ended` cleared every attachment outright
the moment the pane ended, so a later `Arm` found nothing - the client's
stream opens and then simply never carries anything, the exact symptom
TASK-1.125 exists to prevent, reopened in a new place.

**Fix:** `flush_ended` only removes the attachments it can actually
deliver to right now (`outbound.is_some()`); every attachment still
`outbound: None` is left in the `Vec` untouched, and `ended` persists.
`PaneCommand::Arm`'s handler gains one new branch: if `ended` is `Some
(reason)`, it does not register the `Outbound` for ticking at all - it
builds `Ended`'s frame for that attachment's own `stream` (read from the
entry `Attach` already pushed, never cleared - exactly why it must be left
there) on the spot, calls `outbound.end_stream(...)`, and removes that one
entry. `PaneCommand::Attach` itself needs no change: its reply is already
correct regardless of `ended`. `PaneCommand::Resume` (test-only today - no
production API calls it) also resets `ended = None`, so a future restart
is not permanently poisoned by this flag; added now, while the comment
explaining why is cheap to write, the same "close the gap now" reasoning
this file's own comments already use elsewhere (e.g. `RunGate`'s
`generation`). The `Shutdown` arm also calls `flush_ended(..., ended.
unwrap_or(EndReason::Closed))` before breaking - not dead code: `PaneHandle
::shutdown` can end the worker while a program is still running and
attachments are still armed (its own doc: this "does not reliably end a
still-running program" - the loop simply breaks), and `pane.rs`'s own unit
tests already call it that way, even though no production caller does
today. (The `AwaitExit` arm's immediate-reply branch does **not** get an
equivalent call: `current.is_none()` there is only ever reached after a
prior `Exited` has already run and already flushed every armed
attachment, so a second call there would have nothing left to send - an
earlier draft of this plan claimed it as a defensive no-op that "closes a
gap"; it does not, and is not added.)

### `Outbound::close_stream` drops a queue, synchronously, on call

`close_stream` (crates/server/src/outbound.rs:179-196) drops whatever is
still queued for a stream the instant it is called, under its own lock -
calling it right after `enqueue_data` races the writer thread for whether
the just-queued `Ended` frame is ever actually written. `Outbound` gets a
new primitive, `end_stream`, that enqueues the final frame (clearing a
full queue rather than dropping the new frame - decision-2 section 6
already treats a backlogged queue's own content as disposable, and the
terminal message cannot be) and marks the stream for teardown only once
the writer thread has actually drained it, never before.

### The fifth way a pane ends: the server stops

`Runtime::stop` (runtime.rs:878-892, called by `listen::run` once every
connection thread has already been joined) also calls `force_end`, so
`MarkClosing`'s fix covers its reason too. What it does not fix: by the
time this runs, every connection's own `Outbound` has already been told
to stop (its own teardown already ran), so `end_stream` will reliably
return `Err(EnqueueDataError::Closed)` here and the structured `Ended`
frame is not delivered - the client learns the pane is gone only from the
ordinary socket EOF it already has to handle regardless. Accepted, not
fixed further (named as an open question below): there is no connection
left to deliver a structured message over reliably once its own teardown
has already started.

### Every exhaustive match this task's new `ScreenUpdate::Ended` variant
breaks, found by `grep -rn "ScreenUpdate::" crates` and checked one by
one - `wildcard_enum_match_arm = "deny"` (`Cargo.toml:30`) means every one
of these needs a real, named arm, never a `_`:

- `crates/kernel/src/terminal.rs:1648-1653`/`1657-1662` (`expect_full`/
  `expect_delta`, test-only helpers that unwrap a value the kernel's own
  `changes_since` constructed): add `ScreenUpdate::Ended { .. } => panic!
  ("expected Full/Delta, got Ended")` to each - the kernel itself can never
  produce this variant (it is injected only by the server's own `flush_
  ended`, a layer above the kernel), so a real exit code's exact wording
  does not matter, just that these two helpers keep being exhaustive.
- `crates/server/src/pane.rs:1176-1179` (`run_tick`'s own `new_seq` match,
  **production code**, over the value `terminal.changes_since` itself
  returned - the kernel's own enum, never `Ended` in practice, for the
  same reason as the kernel helpers above): add `ScreenUpdate::Ended { ..
  } => unreachable!("the kernel's own changes_since never produces Ended -
  only flush_ended injects it, after this match already ran")`.
- `crates/server/src/pane.rs:2201-2206`, `2233-2239`, `2268-2274`,
  `2328-2340` (four `run_tick` unit tests' own `unexpected @ (Variant |
  Variant)` catch-alls): add `ScreenUpdate::Ended` to each alternation -
  `run_tick` itself also never produces `Ended` (same reasoning), these
  are purely to keep each match exhaustive, one-line additions.
- `crates/server/src/pane.rs:2413-2426` and the identical, separately-
  defined `crates/server/tests/attach.rs:464-477` (`apply_update`, a test
  helper simulating a real attached client applying each update kind):
  add `ScreenUpdate::Ended { .. } => {}` to both, with a one-line doc
  update ("`Ended` is not applied here - a caller that cares about it
  checks for it before deciding whether there is still a screen update to
  apply at all") - this one *is* reachable by this task's own new tests,
  so it is a real no-op, not an `unreachable!()`.
- `crates/app/src/model.rs` (`Model::apply`, the one app-crate site, per
  the engineer's own instruction to keep this task's touch there to
  exactly one arm, since TASK-1.121 is editing this file concurrently):
  add `ScreenUpdate::Ended { .. } => {}` with a comment naming the follow-
  up by what it is, not by its current draft id (draft ids are reused once
  promoted - see the board's own convention) - e.g. "// a no-op for now;
  the app's own reaction to a pane ending is a separate, later task". This
  placeholder happens to already match AC #4's eventual "keeps the last
  screen" (nothing is mutated), which is a bonus, not the reason it is
  chosen - it is chosen because this task has no mandate to decide the
  window's own behaviour.
- Confirmed by the same grep: `crates/steg` (the CLI) and `crates/ui`
  reference `ScreenUpdate` nowhere - nothing to touch there.

### The literal wire payload

Externally tagged (serde's own default, unchanged from every other
variant):

```json
{"Ended":{"reason":{"ProgramExited":{"exit_code":3}}}}
{"Ended":{"reason":"Closed"}}
```

### AC #3's "a client built before this change that meets the final form
does not crash" - proven at the sdk level, not the app's

The guarantee is a property of `ScreenUpdate`'s own derived `Deserialize`
(serde returns `Err` on an unknown variant tag, never panics) - true for
*any* caller of `serde_json::from_slice::<ScreenUpdate>`, not specific to
how the app's own `drain` happens to handle a decode error today. Proven
directly in `crates/sdk/src/screen.rs`'s own test module: decode a raw
JSON object naming a made-up, non-existent variant (not literally `Ended`
- a fictitious one, so this proves the general forward-compatibility
property, not just today's one addition) and assert `Err`, never a panic.
No production code change; this is the better of the two options named by
the engineer, chosen because it is true and because it does not reach into
`crates/app` at all, which this task otherwise has no reason to touch
beyond the one placeholder arm above.

## Files that change

- `crates/sdk/src/screen.rs`: add `EndReason`/`ScreenUpdate::Ended`;
  round-trip tests for both new shapes and for the literal JSON above
  (asserting the exact tag shapes, the same way `pane_attached_round_
  trips_and_carries_both_fields_in_one_object` asserts a concrete shape,
  not just a round trip); the decode-robustness test for AC #3.
- `crates/sdk/src/lib.rs`: export `EndReason` alongside `ScreenUpdate`.
- `crates/sdk/src/description/methods.rs`: reword `pane.attach`'s own
  `details`/`follows.item.summary` to mention the terminal update - no
  shape change; `every_nested_reply_and_stream_item_names_a_real_sdk_type`
  stays green untouched (the name `"ScreenUpdate"` does not change).
- `crates/server/src/outbound.rs`: `Queues::closing: HashSet<u32>` and
  `Outbound::end_stream(&self, stream: u32, bytes: Vec<u8>) -> Result<(),
  EnqueueDataError>` - clears `stream`'s own existing queue, pushes `bytes`
  unconditionally (bypassing `DATA_CAP`; refuses only on `EnqueueDataError
  ::Closed`/`NotOpen`, the same as `enqueue_data`), marks it `closing`.
  `Queues::pop_next`/`spawn_writer`'s own loop: right after popping a data
  frame, checks whether that stream's own queue is now empty and `closing`
  ; if so, removes it from `data`/`order`/`closing`, under the same lock,
  the same tear-down `close_stream` already does. New unit tests: the
  final frame survives a full queue; a stream with nothing else queued
  tears down at once; a stream with more still queued behind the final
  frame is not torn down until the writer actually drains it (a real two-
  thread proof).
- `crates/server/src/pane.rs`: worker-local `ended: Option<EndReason>`;
  `flush_ended` (runs one last `run_tick` first - `PtyBytes` only *arms* a
  future tick deadline via `maybe_arm_tick`, never runs one, so AC #1's
  "after any screen update still owed" needs this or the program's last
  output is lost the moment attachments are retained-away - then sends
  `Ended` to every armed attachment via `end_stream` and removes it,
  leaving every unarmed one alone); new `PaneCommand::MarkClosing` and
  `PaneHandle::mark_closing(&self)`; the `Exited`/`Arm`/`Resume`/
  `Shutdown` handler changes above; the six exhaustive-match fixes listed
  above that land in this file. New unit tests: a program that exits on
  its own sends exactly one `Ended{ProgramExited}` per attachment, with
  the real exit code, after its last real content; `MarkClosing`'s own
  flag-precedence logic (`if ended.is_none()`) keeps a more specific,
  already-known reason rather than overwriting it - this proves the
  worker's own logic, not `force_end`'s pass ordering, which is only a
  real property at the socket level (see `attach.rs`'s forced-close test
  below for that); attaching to a pane that already ended (`ended` already
  `Some` before `Attach`/`Arm` ever run) gets `Ended` on arm, immediately,
  with no tick involved; `flush_ended` under a full queue still delivers
  it.
- `crates/server/src/runtime.rs`: `force_end`'s new pass 0 (`for (_, handle
  ) in &handles { handle.mark_closing(); }`), ahead of today's pass 1.
- `crates/server/src/connection.rs`: `finish_attach`'s `armed == false`
  branch (the TASK-1.125 race - the pane's *worker itself* was already
  gone by the time `arm_attachment` ran, a different case from the "pane
  ended, worker still alive" one above, and still fixed at this layer,
  which is the only one with both `Outbound` and `Connection` in hand)
  builds `ScreenUpdate::Ended { reason: EndReason::Closed }`, encodes one
  frame, and calls `outbound.end_stream(stream, bytes)` in place of
  today's bare `outbound.close_stream(stream)` - `connection.close_stream
  (stream)` stays, unchanged, right after. Existing unit test for this
  function updated to assert the frame is sent before the close.
- `crates/server/tests/attach.rs`: the `apply_update` fix above, plus new
  socket-level tests, reusing `start_server`/`connect_and_handshake`/`new_
  pane_running`/`attach`/`next_update`/`screen_text` and the `pane.close`/
  `force: true` call shape the forced-close tests already use:
  - a program that exits by itself (`sh -c "echo done; exit 3"`): the
    attached client's last `next_update` is `Ended{ProgramExited{exit_code
    : 3}}`, then a bounded quiet-window read (the shape `detach_ends_the_
    stream_even_though_the_program_keeps_producing_output` already uses)
    sees nothing more;
  - `pane.close --force` on a pane with one attached, still-running
    client: the client's last update is `Ended{Closed}` - this is the test
    that actually proves `MarkClosing`'s own ordering (`force_end`'s pass 0
    before pass 1), recorded failing against the pre-`MarkClosing` code
    first;
  - two clients attached to one pane (the shape `two_connections_attached_
    to_one_pane_...` already sets up): both get `Ended` for the same
    program exit, independently;
  - attaching to a pane whose program already ended, well after the fact
    (a plain second `pane.attach`, no race, no flood): the new client's
    own stream carries exactly one `Ended` frame and nothing else - this
    proves the second bug's fix;
  - the TASK-1.125 race itself, reusing `a_pane_that_closes_between_a_
    queued_attach_and_its_arm_leaves_nothing_to_detach`'s own flood/force
    -close setup: connection two's own stream carries one `Ended{Closed}`
    frame before it goes quiet, replacing today's assertion that nothing
    arrives at all.
  Every wait bounded, per this file's own existing convention - no
  unbounded `recv()`/blocking read added anywhere.

## Open questions for the engineer (not decided by this plan)

- `EndReason::Closed` carries no exit code even for a forced close whose
  program did have one. Recommend keeping it this way (above).
- `PROTOCOL_VERSION` (`crates/sdk/src/wire/handshake.rs:25`) stays `1`.
  Recommend this, following TASK-1.114's own precedent (it added `Screen.
  size` without bumping it either), though decision-2 section 7's own
  wording ("matched exactly") is strict enough to read either way.
- Server stop delivers `Ended` only best-effort (named above) - recommend
  accepting this, since there is no connection left to deliver reliably
  over once its own teardown has started.

## For the contract-change PR (quoted here so the code and the record
agree; this plan's own notes on `record-check` will quote this same
wording as what the build was checked against)

The literal JSON above, plus this sentence, proposed for wherever the
engineer judges it belongs - doc-14 itself defers to decision-2 section 3
for the frame format ("decided and measured in decision-2 and are not
restated here"), so the natural home for this wording is decision-2's own
"An update is..." paragraph in section 3, with doc-14 left to a short
cross-reference if the engineer wants one there too:

> An update's third form is `Ended`: the pane has ended and nothing more
> will ever follow on this stream. It carries one of two reasons -
> `ProgramExited`, with the program's own exit code, when nothing closed
> the pane; or `Closed`, with no exit code, whenever the pane itself was
> closed. `Ended` is sent once, after any update still owed, and the
> stream is then closed on both sides; no frame ever arrives on it
> afterward.

decision-2 section 3's ordering-rule box ("queue the attach result first,
register the attachment's queue second") should also gain the symmetric
rule this task adds: "queue the last frame before tearing the stream
down, never the reverse" - as an addendum, via its own PR, per the team's
own decision-records-by-pull-request convention, not edited by this plan.

## Order of work

1. `screen.rs`/`lib.rs`: `EndReason`/`ScreenUpdate::Ended`, their round-
   trip tests, the literal-JSON assertions, and AC #3's decode-robustness
   test. Compiles and tests green with no other file touched yet.
2. `outbound.rs`: `Queues::closing`/`end_stream`, with its own three unit
   tests (full queue, immediate teardown, deferred teardown proven with a
   real second thread). No caller yet.
3. The six exhaustive-match fixes (kernel's two test helpers, `run_tick`'s
   production match, its four unit tests' catch-alls, `apply_update` in
   both `pane.rs` and `attach.rs`) and the one `model.rs` placeholder arm -
   done together, as the mechanical "make it compile with the new variant
   existing" step, before any of this task's own real behaviour lands.
4. `pane.rs`: `ended`, `flush_ended`, `MarkClosing`, the `Exited`/`Arm`/
   `Resume`/`Shutdown` changes, with their own unit tests.
5. `runtime.rs`: `force_end`'s new pass 0.
6. `connection.rs`: `finish_attach`'s fix; update its own existing test.
7. `attach.rs`: the five new socket-level tests, each recorded failing
   first against the pre-step-4/5/6 code, then passing.
8. `methods.rs`: reword `pane.attach`'s own description text.

## Risks

- **`Outbound::end_stream`'s own deferred-teardown race has no existing
  precedent in this codebase to copy.** Mitigated by writing its three
  unit tests before wiring any caller to it (step 2 before 4/6), with the
  deferred-teardown test using a real, independently-driven writer
  thread, not a single-threaded call sequence that could pass by
  accident.
- **`MarkClosing`'s own ordering guarantee rests on `force_end`'s pass 0
  being a plain, synchronous loop that fully returns before pass 1 starts
  sending real OS signals** - true today by construction, but a future
  refactor that parallelises passes 0 and 1 (the way passes 2-6 already
  use `thread::scope`, for throughput) would silently reopen the first
  bug. Mitigated by a doc comment on `mark_closing`'s own call site naming
  this dependency explicitly.
- **Six separate call sites across two crates need the same one-line fix
  for a single new enum variant.** `wildcard_enum_match_arm = "deny"`
  means the compiler itself catches every one that is missed - this risk
  is already closed by the toolchain, not by review.
- **A pane that never had any attachment at all when it ends** costs this
  change nothing extra (`flush_ended` iterates an empty `Vec`) - checked
  by reasoning, not singled out as its own test.

## Proof

- `just check` and `just test` (server, sdk crates and their dependents,
  per this diff - including `crates/app` for the one compile fix).
- Each acceptance criterion:
  - #1: `attach.rs`'s new program-exit and forced-close tests, each with
    its own failing-before/passing-after control - the forced-close one
    specifically proves the `MarkClosing` ordering fix.
  - #2: `attach.rs`'s updated TASK-1.125 race test, same failing/passing
    control.
  - #3: `screen.rs`'s round-trip, literal-JSON, and decode-robustness
    tests; `methods.rs`'s unchanged `every_nested_reply_and_stream_item_
    names_a_real_sdk_type`.
  - #4: all five `attach.rs` tests bounded (no `recv()`/blocking read
    without a timeout anywhere added), each recorded via `asdlc record-
    check`.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Split by @bruk.o on 2026-11-17, as the plan recommended: this task keeps the wire message and the server side (former AC #1, #2, #3, #5). Former AC #4 (the bare window's reaction) moved to a draft task, 'App: the bare window shows that its program has ended, and stops taking keys', which depends on this task and TASK-1.121. This task still has to leave the app compiling: a minimal placeholder arm for the new update, replaced by that task.

Plan (ffc63839f742) checked against doc-14 and decision-2 by @bruk.o: ok. It builds only the server/sdk half after the split; the final update's shape and ordering match the task description; proof maps to the four criteria, with the forced-close and attach-after-end socket tests covering the two bugs the first draft had. The contract text (doc-14 bullet, decision-2 section 3 addendum by its own PR) is the orchestrator's to make, timed with the build. Three open questions go to the engineer with the plan: Closed carries no exit code; PROTOCOL_VERSION stays 1; server stop is best-effort.

Check FAILED: `just check` exited 101 at 2026-11-17T22:54:26+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.128 on task-1.128-server-and-app-an-attached-client-is at d7a34f46b4ee:

```text
ok      format check (0.3s)
ok      lint (83.4s)
ok      boundaries (0.2s)
ok      foundations (0.2s)
ok      foundations public surface (0.1s)
ok      design pages (0.1s)
ok      licences (0.7s)
skip    tests: this change touches no crate
ok      build (steg-sdk) (3.3s)
ok      test (steg-sdk) (2.3s)
ok      test (steg-sdk, local-socket) (2.7s)
ok      build (steg-kernel) (41.1s)
FAILED  test (steg-kernel) (exit 101, 0.4s)
   Compiling steg-sdk v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.128/crates/sdk)
error[E0432]: unresolved import `screen::EndReason`
  --> crates/sdk/src/lib.rs:49:37
   |
49 |     CellStyle, Cursor, CursorShape, EndReason, Page, PaneAttached, Rgb, Row, RowId, Run, Screen,
   |                                     ^^^^^^^^^ no `EndReason` in `screen`

For more information about this error, try `rustc --explain E0432`.
error: could not compile `steg-sdk` (lib) due to 1 previous error
full output: .quiet/test-steg-kernel.log
error: recipe `check-packages` failed with exit code 101
```

Check FAILED: `just check` exited 1 at 2026-11-17T23:27:55+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.128 on task-1.128-server-and-app-an-attached-client-is at d7a34f46b4ee with uncommitted changes, last 40 of 55 lines:

```text
(B[m[32m+        assert!(
(B[m[32m+            queued > 0,
(B[m[32m+            "this test needs a real, non-empty backlog first"
(B[m[32m+        );
(B[m 
         let mut attachments = vec![Attachment {
             id: AttachmentId(0),
Diff in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.128/crates/server/src/pane.rs:2958:
                 Ok(Some(frame)) => {
                     let update: ScreenUpdate = serde_json::from_slice(&frame.payload)
                         .expect("every frame here is a real ScreenUpdate");
[31m-                    if matches!(update, ScreenUpdate::Ended { reason: EndReason::Closed }) {
(B[m[32m+                    if matches!(
(B[m[32m+                        update,
(B[m[32m+                        ScreenUpdate::Ended {
(B[m[32m+                            reason: EndReason::Closed
(B[m[32m+                        }
(B[m[32m+                    ) {
(B[m                         saw_ended = true;
                         break;
                     }
Diff in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.128/crates/server/tests/attach.rs:2009:
         Duration::from_secs(5),
     );
     let expected = EndReason::ProgramExited { exit_code: 4 };
[31m-    assert_eq!(reason_a, expected, "connection A must see the real exit code");
(B[m[31m-    assert_eq!(reason_b, expected, "connection B must see the real exit code too");
(B[m[32m+    assert_eq!(
(B[m[32m+        reason_a, expected,
(B[m[32m+        "connection A must see the real exit code"
(B[m[32m+    );
(B[m[32m+    assert_eq!(
(B[m[32m+        reason_b, expected,
(B[m[32m+        "connection B must see the real exit code too"
(B[m[32m+    );
(B[m 
     assert_nothing_more_arrives_on(
         &mut stream_a,
full output: .quiet/format-check.log
error: recipe `check-fmt` failed on line 157 with exit code 1
```

Check FAILED: `just check` exited 101 at 2026-11-17T23:28:12+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.128 on task-1.128-server-and-app-an-attached-client-is at d7a34f46b4ee with uncommitted changes:

```text
ok      format check (0.3s)
FAILED  lint (exit 101, 4.4s)
    Checking steg-sdk v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.128/crates/sdk)
    Checking steg-kernel v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.128/crates/kernel)
    Checking steg-app v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.128/crates/app)
    Checking steg-server v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.128/crates/server)
    Checking steg v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.128/crates/steg)
error: value assigned to `saw_ended` is never read
    --> crates/server/src/pane.rs:2958:29
     |
2958 |         let mut saw_ended = false;
     |                             ^^^^^ this value is reassigned later and never used
...
2970 |                         saw_ended = true;
     |                         ---------------- `saw_ended` is overwritten here before the previous value is read
     |
     = note: `-D unused-assignments` implied by `-D warnings`
     = help: to override `-D warnings` add `#[allow(unused_assignments)]`

error: could not compile `steg-server` (lib test) due to 1 previous error
warning: build failed, waiting for other jobs to finish...
error: doc list item without indentation
    --> crates/server/tests/attach.rs:1988:5
     |
1988 | /// the one that happened to ask first.
     |     ^
     |
     = help: if this is supposed to be its own paragraph, add a blank line
     = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.98.0/index.html#doc_lazy_continuation
     = note: `-D clippy::doc-lazy-continuation` implied by `-D warnings`
     = help: to override `-D warnings` add `#[allow(clippy::doc_lazy_continuation)]`
help: indent this line
     |
1988 | ///   the one that happened to ask first.
     |     ++

error: could not compile `steg-server` (test "attach") due to 1 previous error
full output: .quiet/lint.log
error: recipe `lint` failed with exit code 101
```

Check FAILED: `just check` exited 101 at 2026-11-17T23:31:36+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.128 on task-1.128-server-and-app-an-attached-client-is at d7a34f46b4ee with uncommitted changes, last 40 of 79 lines:

```text
test pane::ending_tests::await_exit_is_not_delayed_by_a_backlog_on_the_workers_own_queue ... ok
test runtime::tests::an_unfiltered_subscriber_receives_nothing ... ok
kill: 87422: No such process
test runtime::stop_tests::stop_ends_a_program_that_traps_the_polite_signal ... ok
test test_support::tests::a_body_that_never_returns_fails_fast_naming_what_it_waited_for - should panic ... ok
test runtime::tests::the_general_sizing_step_reaches_every_pane_through_split_close_and_swap ... ok
test pane::ending_tests::signal_hangup_reaches_a_backgrounded_grandchild_too ... ok
test idle::tests::the_checker_blocks_with_no_timeout_while_ineligible_and_waits_exactly_once_while_eligible ... ok
test runtime::tests::a_mark_then_the_shells_own_real_exit_cascades_with_no_panic ... ok
test runtime::tests::a_shell_with_no_marks_leaves_every_status_field_unchanged_until_one_arrives ... ok
test pane::ending_tests::signal_kill_ends_a_program_that_traps_the_polite_signal ... ok
test login_env::tests::capture_login_environment_falls_back_on_a_shell_that_exits_non_zero ... ok
test login_env::tests::capture_login_environment_prints_noise_before_the_marker_without_corrupting_the_dump ... ok
test runtime::tests::a_typed_command_that_fails_is_failed_with_its_exit_code ... ok
test runtime::tests::a_typed_command_that_succeeds_is_running_then_finished_and_the_tab_shows_it ... ok
test runtime::tests::a_typed_commands_status_reaches_pane_show_and_tab_show_through_runtime_handle ... ok
test runtime::tests::a_command_panes_own_program_printing_marks_does_not_change_its_status ... ok
test outbound::tests::end_stream_still_delivers_the_final_frame_when_the_queue_was_already_full ... ok
kill: 87724: No such process
test login_env::tests::capture_login_environment_kills_a_backgrounded_grandchild_that_held_the_pipe_open ... ok
kill: 87414: No such process
test pane::tests::a_forward_failing_once_the_worker_is_gone_still_reaps_a_program_that_ends_on_its_own ... ok

failures:

---- pane::attach_tests::flush_ended_still_delivers_its_frame_when_the_attachments_own_queue_is_already_full stdout ----

thread 'pane::attach_tests::flush_ended_still_delivers_its_frame_when_the_attachments_own_queue_is_already_full' (54842990) panicked at crates/server/src/pane.rs:2979:62:
called `Result::unwrap()` on an `Err` value: Os { code: 22, kind: InvalidInput, message: "Invalid argument" }
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    pane::attach_tests::flush_ended_still_delivers_its_frame_when_the_attachments_own_queue_is_already_full

test result: FAILED. 202 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.05s

error: test failed, to rerun pass `-p steg-server --lib`
full output: .quiet/test-steg-server-race-fixture.log
error: recipe `check-packages` failed with exit code 101
```
<!-- SECTION:NOTES:END -->

## Comments

<!-- COMMENTS:BEGIN -->
author: @bruk
created: 2026-11-17T22:51:00Z

---

Plan approved by @bruk (ffc63839f742, size-3): its worker may build it.

---
<!-- COMMENTS:END -->
