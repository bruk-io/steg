---
id: TASK-1.128
title: 'Server and app: an attached client is told on its stream when its pane ends'
status: In Review
assignee:
  - '@bruk.w4'
created_date: '2026-11-17T03:51:00Z'
updated_date: '2026-11-18T01:21:00Z'
labels:
  - size-3
  - review-passed
milestone: m-4
dependencies:
  - TASK-1.125
  - TASK-1.114
  - TASK-1.117
references:
  - task-1.128-server-and-app-an-attached-client-is
  - https://github.com/bruk-io/steg/pull/73
parent_task_id: TASK-1
project: bruk-io-steg
asdlc_accepted_from: DRAFT-2
asdlc_branch: task-1.128-server-and-app-an-attached-client-is
asdlc_plan_approved_by: bruk
asdlc_plan_approved_hash: 'ffc63839f742'
asdlc_plan_approved_size: size-3
asdlc_review_kind: same-machine
asdlc_review_findings: 'important=0,nit=0'
asdlc_plan_matched: yes
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

Check passed: `just check` exited 0 at 2026-11-17T23:42:56+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.128 on task-1.128-server-and-app-an-attached-client-is at d7a34f46b4ee with uncommitted changes:

```text
ok      format check (0.3s)
ok      lint (1.4s)
ok      boundaries (0.1s)
ok      foundations (0.1s)
ok      foundations public surface (0.0s)
ok      design pages (0.0s)
ok      licences (0.6s)
ok      tests (-p steg -p steg-app -p steg-kernel -p steg-sdk -p steg-server) (57.5s)
ok      build (steg-sdk) (0.1s)
ok      test (steg-sdk) (0.3s)
ok      test (steg-sdk, local-socket) (0.4s)
ok      build (steg-kernel) (0.1s)
ok      test (steg-kernel) (0.2s)
ok      build (steg-server) (29.4s)
ok      test (steg-server) (54.9s)
ok      test (steg-server, race-fixture) (56.3s)
ok      test (steg-server, pty-race-fixture) (61.2s)
ok      test (steg-server, accept-backoff-fixture) (59.1s)
ok      build (steg) (2.3s)
ok      test (steg) (2.6s)
ok      test (steg, fixtures) (7.9s)
ok      build (steg-ui) (14.7s)
ok      test (steg-ui) (1.4s)
ok      build (steg-app) (1.3s)
ok      test (steg-app) (1.4s)
ok      tool tests (14.3s)
ok      architecture (0.7s)
ok      recipe names (0.1s)
ok      ui levels (0.1s)
ok      ui colours (0.1s)
```

Check FAILED: `just check` exited 101 at 2026-11-17T23:50:19+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.128 on task-1.128-server-and-app-an-attached-client-is at a9b0d46ddae8, last 40 of 71 lines:

```text
test pane::attach_tests::attaching_returns_the_panes_current_screen_and_updates_follow_it ... ok
test pane::attach_tests::a_program_that_exits_on_its_own_sends_ended_with_its_real_exit_code_after_its_last_content ... ok
test pane::attach_tests::mark_closing_sent_before_a_programs_own_exit_is_the_reason_ended_reports ... ok
kill: 49174: No such process
test runtime::stop_tests::stop_ends_a_program_that_traps_the_polite_signal ... ok
test pane::ending_tests::await_exit_is_not_delayed_by_a_backlog_on_the_workers_own_queue ... ok
test runtime::tests::an_unfiltered_subscriber_receives_nothing ... ok
test runtime::tests::the_general_sizing_step_reaches_every_pane_through_split_close_and_swap ... ok
test test_support::tests::a_body_that_never_returns_fails_fast_naming_what_it_waited_for - should panic ... ok
test pane::ending_tests::signal_hangup_reaches_a_backgrounded_grandchild_too ... ok
test idle::tests::the_checker_blocks_with_no_timeout_while_ineligible_and_waits_exactly_once_while_eligible ... ok
test runtime::tests::a_mark_then_the_shells_own_real_exit_cascades_with_no_panic ... ok
test runtime::tests::a_shell_with_no_marks_leaves_every_status_field_unchanged_until_one_arrives ... ok
test pane::ending_tests::signal_kill_ends_a_program_that_traps_the_polite_signal ... ok
test runtime::tests::a_typed_command_that_fails_is_failed_with_its_exit_code ... ok
test runtime::tests::a_typed_command_that_succeeds_is_running_then_finished_and_the_tab_shows_it ... ok
test runtime::tests::a_typed_commands_status_reaches_pane_show_and_tab_show_through_runtime_handle ... ok
test runtime::tests::a_command_panes_own_program_printing_marks_does_not_change_its_status ... ok
kill: 49197: No such process
test pane::tests::a_forward_failing_once_the_worker_is_gone_still_reaps_a_program_that_ends_on_its_own ... ok
test login_env::tests::capture_login_environment_falls_back_on_a_shell_that_exits_non_zero ... ok
test login_env::tests::capture_login_environment_prints_noise_before_the_marker_without_corrupting_the_dump ... ok
test login_env::tests::capture_login_environment_kills_a_backgrounded_grandchild_that_held_the_pipe_open ... FAILED

failures:

---- login_env::tests::capture_login_environment_kills_a_backgrounded_grandchild_that_held_the_pipe_open stdout ----

thread 'login_env::tests::capture_login_environment_kills_a_backgrounded_grandchild_that_held_the_pipe_open' (55146981) panicked at crates/server/src/login_env.rs:659:13:
the backgrounded grandchild's own pid file never appeared


failures:
    login_env::tests::capture_login_environment_kills_a_backgrounded_grandchild_that_held_the_pipe_open

test result: FAILED. 202 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 10.01s

error: test failed, to rerun pass `-p steg-server --lib`
full output: .quiet/tests-p-steg-p-steg-app-p-steg-kernel-p-steg-sdk-p-steg-server.log
error: recipe `test` failed with exit code 101
```

Check FAILED: `just check` exited 101 at 2026-11-18T00:01:06+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.128 on task-1.128-server-and-app-an-attached-client-is at a9b0d46ddae8, last 40 of 81 lines:

```text
test pane::ending_tests::await_exit_is_not_delayed_by_a_backlog_on_the_workers_own_queue ... ok
test runtime::tests::an_unfiltered_subscriber_receives_nothing ... ok
kill: 3961: No such process
test runtime::stop_tests::stop_ends_a_program_that_traps_the_polite_signal ... ok
test test_support::tests::a_body_that_never_returns_fails_fast_naming_what_it_waited_for - should panic ... ok
test runtime::tests::the_general_sizing_step_reaches_every_pane_through_split_close_and_swap ... ok
test pane::ending_tests::signal_hangup_reaches_a_backgrounded_grandchild_too ... ok
test idle::tests::the_checker_blocks_with_no_timeout_while_ineligible_and_waits_exactly_once_while_eligible ... ok
test runtime::tests::a_mark_then_the_shells_own_real_exit_cascades_with_no_panic ... ok
test runtime::tests::a_shell_with_no_marks_leaves_every_status_field_unchanged_until_one_arrives ... ok
test pane::ending_tests::signal_kill_ends_a_program_that_traps_the_polite_signal ... ok
test runtime::tests::a_typed_command_that_succeeds_is_running_then_finished_and_the_tab_shows_it ... ok
test runtime::tests::a_typed_command_that_fails_is_failed_with_its_exit_code ... ok
test runtime::tests::a_typed_commands_status_reaches_pane_show_and_tab_show_through_runtime_handle ... ok
test runtime::tests::a_command_panes_own_program_printing_marks_does_not_change_its_status ... ok
test login_env::tests::capture_login_environment_falls_back_on_a_shell_that_exits_non_zero ... ok
test outbound::tests::end_stream_still_delivers_the_final_frame_when_the_queue_was_already_full ... ok
test login_env::tests::capture_login_environment_prints_noise_before_the_marker_without_corrupting_the_dump ... ok
kill: 4353: No such process
test login_env::tests::capture_login_environment_kills_a_backgrounded_grandchild_that_held_the_pipe_open ... ok
kill: 3958: No such process
test pane::tests::a_forward_failing_once_the_worker_is_gone_still_reaps_a_program_that_ends_on_its_own ... ok

failures:

---- pane::attach_tests::a_program_that_exits_on_its_own_sends_ended_with_its_real_exit_code_after_its_last_content stdout ----

thread 'pane::attach_tests::a_program_that_exits_on_its_own_sends_ended_with_its_real_exit_code_after_its_last_content' (55349863) panicked at crates/server/src/pane.rs:2766:17:
Ended must arrive after any screen update still owed, never before it
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    pane::attach_tests::a_program_that_exits_on_its_own_sends_ended_with_its_real_exit_code_after_its_last_content

test result: FAILED. 202 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.06s

error: test failed, to rerun pass `-p steg-server --lib`
full output: .quiet/test-steg-server-accept-backoff-fixture.log
error: recipe `check-packages` failed with exit code 101
```

Check passed: `just check` exited 0 at 2026-11-18T00:13:50+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.128 on task-1.128-server-and-app-an-attached-client-is at a9b0d46ddae8 with uncommitted changes:

```text
ok      format check (0.3s)
ok      lint (3.6s)
ok      boundaries (0.1s)
ok      foundations (0.1s)
ok      foundations public surface (0.0s)
ok      design pages (0.1s)
ok      licences (0.8s)
ok      tests (-p steg -p steg-app -p steg-kernel -p steg-sdk -p steg-server) (55.2s)
ok      build (steg-sdk) (0.1s)
ok      test (steg-sdk) (0.4s)
ok      test (steg-sdk, local-socket) (0.6s)
ok      build (steg-kernel) (0.1s)
ok      test (steg-kernel) (0.2s)
ok      build (steg-server) (30.6s)
ok      test (steg-server) (56.1s)
ok      test (steg-server, race-fixture) (57.1s)
ok      test (steg-server, pty-race-fixture) (62.3s)
ok      test (steg-server, accept-backoff-fixture) (57.5s)
ok      build (steg) (1.5s)
ok      test (steg) (3.2s)
ok      test (steg, fixtures) (9.7s)
ok      build (steg-ui) (0.3s)
ok      test (steg-ui) (4.8s)
ok      build (steg-app) (1.0s)
ok      test (steg-app) (1.1s)
ok      tool tests (13.7s)
ok      architecture (0.7s)
ok      recipe names (0.1s)
ok      ui levels (0.1s)
ok      ui colours (0.1s)
```

Check passed: `just check` exited 0 at 2026-11-18T00:20:45+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.128 on task-1.128-server-and-app-an-attached-client-is at 493786d3aec6:

```text
ok      format check (0.3s)
ok      lint (6.3s)
ok      boundaries (0.1s)
ok      foundations (0.1s)
ok      foundations public surface (0.0s)
ok      design pages (0.1s)
ok      licences (0.7s)
ok      tests (-p steg -p steg-app -p steg-kernel -p steg-sdk -p steg-server) (66.0s)
ok      build (steg-sdk) (0.6s)
ok      test (steg-sdk) (2.1s)
ok      test (steg-sdk, local-socket) (2.8s)
ok      build (steg-kernel) (0.7s)
ok      test (steg-kernel) (1.1s)
ok      build (steg-server) (0.7s)
ok      test (steg-server) (55.6s)
ok      test (steg-server, race-fixture) (57.7s)
ok      test (steg-server, pty-race-fixture) (67.0s)
ok      test (steg-server, accept-backoff-fixture) (67.0s)
ok      build (steg) (2.0s)
ok      test (steg) (4.2s)
ok      test (steg, fixtures) (10.6s)
ok      build (steg-ui) (0.5s)
ok      test (steg-ui) (7.5s)
ok      build (steg-app) (1.6s)
ok      test (steg-app) (1.5s)
ok      tool tests (13.9s)
ok      architecture (0.8s)
ok      recipe names (0.1s)
ok      ui levels (0.1s)
ok      ui colours (0.1s)
```

Built per the plan (server/sdk half): ScreenUpdate::Ended/EndReason (sdk), Outbound::end_stream (a safe stream half-close that defers teardown until the writer has actually drained the final frame), the pane worker's ended flag/flush_ended/MarkClosing, force_end's new pass 0, finish_attach's TASK-1.125-race fix, and five new attach.rs socket-level tests. Full details and rationale are in the two commit messages.

Two real bugs found and fixed during the build, beyond the plan's own text:

1. The Arm handler's late-end branch (attaching to an already-ended pane) sent only Ended, with no catch-up content - found by a_program_that_exits_on_its_own_sends_ended_with_its_real_exit_code_after_its_last_content flaking under heavy machine load (the program fully ended before arm() ran, a real window since attach/arm are two separate calls). Fixed: Arm now sends a fresh Full read from the terminal before Ended on that path. Updated the two tests this changes the shape of (both now documented with why).

2. call_and_read_reply (crates/server/tests/attach.rs, pre-existing) fell through to a blocking read on any non-matching poll() result, not only on None - harmless before this task, since nothing used to produce an extra StreamControl frame on an attached stream around the same time as a pane.close/pane.detach reply. My own new forced-close test exposed it (detaching_a_pane_that_already_closed_still_releases_the_stream also started failing intermittently). Fixed to drain every already-decoded frame before reading more, the same shape reply_within already used.

One platform quirk, not a bug, worth recording for whoever touches outbound.rs's own tests next: on this machine (macOS), UnixStream::set_read_timeout returns EINVAL once the socket's own peer has already closed - not a value-rounding issue, confirmed with a minimal, unrelated repro. My own new flush_ended_still_delivers_its_frame_when_the_attachments_own_queue_is_already_full test hit this by re-setting the read timeout inside its own read loop, racing the writer thread finishing and dropping its own end of the pair; fixed by setting it once, before the writer thread (and so that peer) can possibly exist.

Checks recorded via asdlc record-check against TASK-1.128, each as just check at a clean tree:
- One early run caught a race from editing screen.rs/lib.rs while a background record-check was still reading them (noise, not a real baseline - don't read that first exit-101 entry as a finding).
- Two genuinely clean just check runs recorded (exit 0) before the final two commits; a third, final exit-0 run recorded at the pushed head (commit 2ca675d7).
- Two environmental flakes observed and confirmed unrelated by rerunning in isolation (always passed alone): login_env's own known grandchild flake, and a pre-existing, unrelated stuck-pane timing test (an_unresponsive_panes_own_commands_do_not_stall_the_connection_or_another_pane) - both coincided with TASK-1.121's own concurrent just check running on this same machine at the time (confirmed via ps).
- CI's first run failed on a third, also pre-existing and unrelated test (runtime::tests::the_general_sizing_step_reaches_every_pane_through_split_close_and_swap, a terminal-scrolling timing assertion) - reran it myself (gh run rerun --failed) and it passed clean on the second attempt; not something I changed.

Contract-change requests from the plan (doc-14's own wording, decision-2's addendum, PROTOCOL_VERSION's confirmation) are still open for you - none of those are mine to edit.

The app's own placeholder arm in crates/app/src/model.rs is the one, intentionally minimal touch in crates/app this task makes - DRAFT-1 is where the real behaviour lands, once this and TASK-1.121 are both merged.

Check FAILED: `cargo test -p steg-server --test attach -- pane_close_force_reports_ended_closed_not_the_signals_own_exit_code --exact` exited 101 at 2026-11-18T00:48:18+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.128 on task-1.128-server-and-app-an-attached-client-is at 2ca675d79c0e with uncommitted changes, last 40 of 44 lines:

```text
    |      ----------- variant in this enum
...
404 |     MarkClosing,
    |     ^^^^^^^^^^^
    |
    = note: `#[warn(dead_code)]` (part of `#[warn(unused)]`) on by default

warning: method `mark_closing` is never used
   --> crates/server/src/pane.rs:714:19
    |
508 | impl PaneHandle {
    | --------------- method in this implementation
...
714 |     pub(crate) fn mark_closing(&self) {
    |                   ^^^^^^^^^^^^

warning: `steg-server` (lib) generated 2 warnings
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.08s
     Running tests/attach.rs (target/debug/deps/attach-1c3f77caae0e73d3)

running 1 test
test pane_close_force_reports_ended_closed_not_the_signals_own_exit_code ... FAILED

failures:

---- pane_close_force_reports_ended_closed_not_the_signals_own_exit_code stdout ----

thread 'pane_close_force_reports_ended_closed_not_the_signals_own_exit_code' (55919651) panicked at crates/server/tests/attach.rs:1963:5:
assertion `left == right` failed: a forced close must report Closed, never the signal's own exit code
  left: ProgramExited { exit_code: 1 }
 right: Closed
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    pane_close_force_reports_ended_closed_not_the_signals_own_exit_code

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 22 filtered out; finished in 0.02s

error: test failed, to rerun pass `-p steg-server --test attach`
```

Commits since the review: 0.

Check passed: `cargo test -p steg-server --test attach -- pane_close_force_reports_ended_closed_not_the_signals_own_exit_code --exact` exited 0 at 2026-11-18T00:48:33+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.128 on task-1.128-server-and-app-an-attached-client-is at 2ca675d79c0e:

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.08s
     Running tests/attach.rs (target/debug/deps/attach-1c3f77caae0e73d3)

running 1 test
test pane_close_force_reports_ended_closed_not_the_signals_own_exit_code ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 22 filtered out; finished in 0.33s
```

Commits since the review: 0.

Check FAILED: `cargo test -p steg-server --test attach -- a_program_that_exits_on_its_own_sends_ended_with_its_real_exit_code --exact` exited 101 at 2026-11-18T00:49:07+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.128 on task-1.128-server-and-app-an-attached-client-is at 2ca675d79c0e with uncommitted changes:

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.08s
     Running tests/attach.rs (target/debug/deps/attach-1c3f77caae0e73d3)

running 1 test
test a_program_that_exits_on_its_own_sends_ended_with_its_real_exit_code ... FAILED

failures:

---- a_program_that_exits_on_its_own_sends_ended_with_its_real_exit_code stdout ----

thread 'a_program_that_exits_on_its_own_sends_ended_with_its_real_exit_code' (55926172) panicked at crates/server/tests/attach.rs:455:17:
no update arrived on stream 1 within 4.991071917s
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    a_program_that_exits_on_its_own_sends_ended_with_its_real_exit_code

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 22 filtered out; finished in 5.05s

error: test failed, to rerun pass `-p steg-server --test attach`
```

Commits since the review: 0.

Check FAILED: `cargo test -p steg-server --test attach -- two_clients_attached_to_one_pane_both_get_ended_for_the_same_exit --exact` exited 101 at 2026-11-18T00:49:28+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.128 on task-1.128-server-and-app-an-attached-client-is at 2ca675d79c0e with uncommitted changes:

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.08s
     Running tests/attach.rs (target/debug/deps/attach-1c3f77caae0e73d3)

running 1 test
test two_clients_attached_to_one_pane_both_get_ended_for_the_same_exit ... FAILED

failures:

---- two_clients_attached_to_one_pane_both_get_ended_for_the_same_exit stdout ----

thread 'two_clients_attached_to_one_pane_both_get_ended_for_the_same_exit' (55929681) panicked at crates/server/tests/attach.rs:455:17:
no update arrived on stream 1 within 4.992545583s
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    two_clients_attached_to_one_pane_both_get_ended_for_the_same_exit

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 22 filtered out; finished in 5.05s

error: test failed, to rerun pass `-p steg-server --test attach`
```

Commits since the review: 0.

Check passed: `cargo test -p steg-server --test attach -- a_program_that_exits_on_its_own_sends_ended_with_its_real_exit_code --exact` exited 0 at 2026-11-18T00:49:43+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.128 on task-1.128-server-and-app-an-attached-client-is at 2ca675d79c0e:

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.08s
     Running tests/attach.rs (target/debug/deps/attach-1c3f77caae0e73d3)

running 1 test
test a_program_that_exits_on_its_own_sends_ended_with_its_real_exit_code ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 22 filtered out; finished in 0.34s
```

Commits since the review: 0.

Check passed: `cargo test -p steg-server --test attach -- two_clients_attached_to_one_pane_both_get_ended_for_the_same_exit --exact` exited 0 at 2026-11-18T00:49:54+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.128 on task-1.128-server-and-app-an-attached-client-is at 2ca675d79c0e:

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.08s
     Running tests/attach.rs (target/debug/deps/attach-1c3f77caae0e73d3)

running 1 test
test two_clients_attached_to_one_pane_both_get_ended_for_the_same_exit ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 22 filtered out; finished in 0.65s
```

Commits since the review: 0.

Check FAILED: `cargo test -p steg-server --test attach -- attaching_to_a_pane_whose_program_already_ended_gets_ended_and_nothing_else --exact` exited 101 at 2026-11-18T00:51:00+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.128 on task-1.128-server-and-app-an-attached-client-is at 2ca675d79c0e with uncommitted changes:

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.08s
     Running tests/attach.rs (target/debug/deps/attach-1c3f77caae0e73d3)

running 1 test
test attaching_to_a_pane_whose_program_already_ended_gets_ended_and_nothing_else ... FAILED

failures:

---- attaching_to_a_pane_whose_program_already_ended_gets_ended_and_nothing_else stdout ----

thread 'attaching_to_a_pane_whose_program_already_ended_gets_ended_and_nothing_else' (55947372) panicked at crates/server/tests/attach.rs:455:17:
no update arrived on stream 1 within 4.999999958s
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    attaching_to_a_pane_whose_program_already_ended_gets_ended_and_nothing_else

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 22 filtered out; finished in 5.05s

error: test failed, to rerun pass `-p steg-server --test attach`
```

Commits since the review: 0.

Check passed: `cargo test -p steg-server --test attach -- attaching_to_a_pane_whose_program_already_ended_gets_ended_and_nothing_else --exact` exited 0 at 2026-11-18T00:51:14+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.128 on task-1.128-server-and-app-an-attached-client-is at 2ca675d79c0e:

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.09s
     Running tests/attach.rs (target/debug/deps/attach-1c3f77caae0e73d3)

running 1 test
test attaching_to_a_pane_whose_program_already_ended_gets_ended_and_nothing_else ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 22 filtered out; finished in 0.34s
```

Commits since the review: 0.

Check FAILED: `cargo test -p steg-server --test attach -- a_pane_that_closes_between_a_queued_attach_and_its_arm_leaves_nothing_to_detach --exact` exited 101 at 2026-11-18T00:52:16+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.128 on task-1.128-server-and-app-an-attached-client-is at 2ca675d79c0e with uncommitted changes:

```text
warning: unused imports: `EndReason` and `ScreenUpdate`
  --> crates/server/src/connection.rs:19:40
   |
19 |     ClientId, Connection, Description, EndReason, Frame, Handshake, Incoming, PROTOCOL_VERSION,
   |                                        ^^^^^^^^^
20 |     PaneId, Refusal, Request, Response, ScreenUpdate, WireRole as Role, check_hello, description,
   |                                         ^^^^^^^^^^^^
   |
   = note: `#[warn(unused_imports)]` (part of `#[warn(unused)]`) on by default

warning: `steg-server` (lib) generated 1 warning (run `cargo fix --lib -p steg-server` to apply 1 suggestion)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.08s
     Running tests/attach.rs (target/debug/deps/attach-1c3f77caae0e73d3)

running 1 test
test a_pane_that_closes_between_a_queued_attach_and_its_arm_leaves_nothing_to_detach ... FAILED

failures:

---- a_pane_that_closes_between_a_queued_attach_and_its_arm_leaves_nothing_to_detach stdout ----

thread 'a_pane_that_closes_between_a_queued_attach_and_its_arm_leaves_nothing_to_detach' (55961553) panicked at crates/server/tests/attach.rs:455:17:
no update arrived on stream 1 within 15s
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    a_pane_that_closes_between_a_queued_attach_and_its_arm_leaves_nothing_to_detach

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 22 filtered out; finished in 19.90s

error: test failed, to rerun pass `-p steg-server --test attach`
```

Commits since the review: 0.

Check passed: `cargo test -p steg-server --test attach -- a_pane_that_closes_between_a_queued_attach_and_its_arm_leaves_nothing_to_detach --exact` exited 0 at 2026-11-18T00:52:52+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.128 on task-1.128-server-and-app-an-attached-client-is at 2ca675d79c0e:

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.09s
     Running tests/attach.rs (target/debug/deps/attach-1c3f77caae0e73d3)

running 1 test
test a_pane_that_closes_between_a_queued_attach_and_its_arm_leaves_nothing_to_detach ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 22 filtered out; finished in 7.29s
```

Commits since the review: 0.

Check passed: `just check` exited 0 at 2026-11-18T01:04:12+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.128 on task-1.128-server-and-app-an-attached-client-is at 7cca969023bd:

```text
ok      format check (0.3s)
ok      lint (12.1s)
ok      boundaries (0.1s)
ok      foundations (0.1s)
ok      foundations public surface (0.0s)
ok      design pages (0.1s)
ok      licences (0.7s)
ok      tests (-p steg -p steg-app -p steg-kernel -p steg-sdk -p steg-server) (56.2s)
ok      build (steg-sdk) (0.7s)
ok      test (steg-sdk) (2.3s)
ok      test (steg-sdk, local-socket) (3.2s)
ok      build (steg-kernel) (0.5s)
ok      test (steg-kernel) (1.2s)
ok      build (steg-server) (28.8s)
ok      test (steg-server) (58.7s)
ok      test (steg-server, race-fixture) (62.0s)
ok      test (steg-server, pty-race-fixture) (64.8s)
ok      test (steg-server, accept-backoff-fixture) (64.1s)
ok      build (steg) (1.6s)
ok      test (steg) (3.9s)
ok      test (steg, fixtures) (11.9s)
ok      build (steg-ui) (0.3s)
ok      test (steg-ui) (1.4s)
ok      build (steg-app) (0.9s)
ok      test (steg-app) (1.2s)
ok      tool tests (13.4s)
ok      architecture (0.7s)
ok      recipe names (0.1s)
ok      ui levels (0.1s)
ok      ui colours (0.1s)
```

Commits since the review: 27, 26 without the Asdlc-Worker trailer.

Fixed @bruk.r2's review findings (1 Important, 2 nits):

1. [Important] Recorded a failing-then-passing control pair, against the final head, for each of the five new attach.rs socket-level tests - the gap the review found (the board's earlier evidence proved the mechanisms by other means, but never recorded these five tests actually failing against pre-fix code):
   - pane_close_force_reports_ended_closed_not_the_signals_own_exit_code: disabled force_end's pass 0 (handle.mark_closing() -> a no-op) in runtime.rs. Failed as the reviewer's own control showed (left: ProgramExited { exit_code: 1 }, right: Closed). Reverted, passed.
   - a_program_that_exits_on_its_own_sends_ended_with_its_real_exit_code and two_clients_attached_to_one_pane_both_get_ended_for_the_same_exit: disabled flush_ended's own outbound.end_stream call in pane.rs. Both failed (no update arrived within 5s). Reverted, both passed.
   - attaching_to_a_pane_whose_program_already_ended_gets_a_final_full_then_ended (renamed, see finding 3): disabled the Arm handler's ended-branch body in pane.rs (if false && let Some(position) = ...). Failed (no update arrived within 5s). Reverted, passed.
   - a_pane_that_closes_between_a_queued_attach_and_its_arm_leaves_nothing_to_detach: reverted finish_attach's armed == false branch to the pre-fix bare close_stream (no Ended) in connection.rs. Failed (no update arrived within 15s - the force-close grace window). Reverted, passed.
   Every control run unrecorded first, confirmed it failed for the stated reason with nothing from the environment in its output, then recorded; tree clean (git status) between every pair and at the end. All five mechanisms are real - none could not be made to fail.

2. [Nit] pane.rs's Arm handler: documented why the catch-up Full's seq: tracked_seq can safely be 0 (its own "no tick yet" sentinel) - no production reader (Model::apply, either test harness's apply_update) ever reads a Full's own seq, and Ended, right after, closes the stream for good, so there is no next call for it to matter to; terminal.screen() itself has no seq of its own to report instead (it is a raw read, not changes_since's own tracked point).

3. [Nit] attach.rs: reworded the stale assertion message and renamed attaching_to_a_pane_whose_program_already_ended_gets_ended_and_nothing_else to attaching_to_a_pane_whose_program_already_ended_gets_a_final_full_then_ended, matching pane.rs's own equivalent test's name.

Departures from the plan, in the plan's own terms: late attach to an already-ended pane sends one Full then Ended, where the plan's own first draft said Ended alone - found by a test flaking under load during the build (the first round of this task), fixed before the first review, and doc-14 already reflects it ("a fresh Full... then Ended").

just check recorded clean at the final pushed head (14a11971); CI green on the same commit (gh pr checks 73 --watch, 12m5s).
<!-- SECTION:NOTES:END -->

## Comments

<!-- COMMENTS:BEGIN -->
author: @bruk
created: 2026-11-17T22:51:00Z

---

Plan approved by @bruk (ffc63839f742, size-3): its worker may build it.

---

author: @bruk.r2
created: 2026-11-18T00:36:00Z

---

@bruk.r2 is reviewing TASK-1.128: a same-machine review: it waited 0m with no other engineer's reviewer, longer than review_fallback_after (0m), so a fresh reviewer on its author's machine takes it.

---

author: @bruk.r2
created: 2026-11-18T00:45:00Z

---

Review by @bruk.r2 (same-machine): changes requested. Back to In Progress for the author's worker.

Findings: 1 Important, 2 nit. Cites: review. The diff matches the plan.

[Evidence, Important] AC #4's own text ("each with a recorded failing control and passing run") and the plan's Proof section promise a recorded failing-before/passing-after run for each of the five new attach.rs socket-level tests, including the forced-close test that is the only proof of force_end's new pass-0 ordering. The task's recorded `just check` history (implementation notes) shows failing runs only at the pane.rs unit-test level (e.g. "Ended must arrive after any screen update still owed") and for unrelated flakes (login_env, lint/fmt); no recorded run ever shows an attach.rs socket-level test (e.g. pane_close_force_reports_ended_closed_not_the_signals_own_exit_code) failing against the pre-fix code. I repeated this control myself: disabled force_end's pass 0 in runtime.rs (replaced handle.mark_closing() with a no-op) and ran `cargo test -p steg-server --test attach pane_close_force_reports_ended_closed_not_the_signals_own_exit_code -- --exact`; it failed exactly as claimed (left: ProgramExited { exit_code: 1 }, right: Closed), confirming the mechanism itself is real and correct. I then reverted the edit (git checkout -- crates/server/src/runtime.rs) and confirmed git status is clean. The mechanism works, but the board's own recorded evidence for AC #4's socket-level proof step is missing.

[Bugs, Nit] crates/server/src/pane.rs, the Arm handler's late-end branch (~line 1154-1171): the Full sent before Ended is tagged seq: tracked_seq, which stays at its initial sentinel value 0 when the pane ends before any attachment ever existed (run_tick's any_work gate never fires in that case, so tracked_seq is never advanced). Harmless today since this attachment is torn down immediately by the following Ended, but worth a doc note so a future reader doesn't mistake this for a real sequence number.

[Compliance, Nit] crates/server/tests/attach.rs, attaching_to_a_pane_whose_program_already_ended_gets_ended_and_nothing_else: the assertion message "the first and only update on a late attach to an already-ended pane must be Ended" contradicts the test's own doc comment three lines above, which correctly notes a Full arrives first. Minor wording fix only.

What I checked: full diff against origin/main (excluding backlog/) across sdk/screen.rs+lib.rs+description/methods.rs, kernel/terminal.rs, app/model.rs, server/outbound.rs, server/pane.rs, server/runtime.rs, server/connection.rs, server/tests/attach.rs; doc-14 and decision-2 section 3 (including its 2026-11-17 addendum, already landed on main, matching the wire shape in code); the approved plan (ffc63839f742) and its departures section; every exhaustive ScreenUpdate match (all have named arms, no wildcards); Outbound::end_stream/pop_next's deferred-teardown logic and its real two-thread test; MarkClosing's ordering via a live negative control (above); the TASK-1.125 race fix in finish_attach; the call_and_read_reply drain-before-block fix (does not swallow or weaken any existing assertion - it only changes what happens after a non-matching poll(), matching reply_within's established shape). Confirmed the final recorded clean check (at local commit 493786d3aec6) has an identical code tree to pushed HEAD 2ca675d7 (git diff shows only backlog/ task-file differences), so the recorded evidence is against the right code modulo the AC #4 gap above. The three reported flakes (login_env grandchild test timing out on its pid file; a stuck-pane timing test coinciding with a concurrent just check from TASK-1.121; a terminal-scrolling timing assertion failing on CI's first run and passing on rerun) match what the worker described and are not findings.

What I did not check: a real SSH channel (out of scope per decision-2), the app's own future reaction to Ended (explicitly out of scope, moved to a draft task), and I did not re-run the full CI suite myself (trusted the claim that CI is green on 2ca675d7 after the reported rerun).

---

author: @bruk.r2
created: 2026-11-18T01:17:00Z

---

@bruk.r2 is reviewing TASK-1.128: a same-machine review: it waited 0m with no other engineer's reviewer, longer than review_fallback_after (0m), so a fresh reviewer on its author's machine takes it.

---

author: @bruk.r2
created: 2026-11-18T01:21:00Z

---

Review by @bruk.r2 (same-machine): clean. It stays In Review for a person to merge.

Findings: 0 Important, 0 nit. The diff matches the plan.

Second round, scoped to the round-1 fix only.

1. Code diff since round 1 (2ca675d7..14a11971, crates/server/src/pane.rs and crates/server/tests/attach.rs, 27 insertions/3 deletions): exactly a doc comment on flush_ended's catch-up Full carrying seq: tracked_seq possibly 0, a reworded assertion message, and a rename of attaching_to_a_pane_whose_program_already_ended_gets_ended_and_nothing_else to ...gets_a_final_full_then_ended. No behaviour changed. Checked the doc comment's claim directly: crates/app/src/model.rs Model::apply (ScreenUpdate::Full { screen, .. }), and both apply_update test helpers (crates/server/src/pane.rs:2616, crates/server/tests/attach.rs:482), all destructure Full with `..` and never read seq; the sdk itself (crates/sdk/src/screen.rs) has no other reader. The claim holds.

2. Evidence: the implementation notes now record a failing-then-passing control pair for each of the five new attach.rs socket-level tests (pane_close_force_reports_ended_closed_not_the_signals_own_exit_code; a_program_that_exits_on_its_own_sends_ended_with_its_real_exit_code; two_clients_attached_to_one_pane_both_get_ended_for_the_same_exit; attaching_to_a_pane_whose_program_already_ended... [recorded under its pre-rename name - the rename that followed changed only the name and a comment, not the test's body, so the control still proves the same mechanism]; a_pane_that_closes_between_a_queued_attach_and_its_arm_leaves_nothing_to_detach), each disabling a distinct mechanism (MarkClosing's pass 0, flush_ended's end_stream call, the Arm handler's late-end branch, finish_attach's armed==false fix) and failing for the stated reason. I repeated a different one than round 1's forced-close control: disabled flush_ended's end_stream call in pane.rs (scratch edit), ran `cargo test -p steg-server --test attach -- a_program_that_exits_on_its_own_sends_ended_with_its_real_exit_code --exact` (bounded by the test's own 5s wait), it failed with "no update arrived on stream 1 within ~5s" exactly as recorded, then reverted with `git checkout --` and confirmed `git status` clean. The recorded `just check` clean run is at commit 7cca969023bd; diffing it against final head 14a11971 (excluding backlog/) shows no code difference - the recorded evidence is against the final code.

3. The Departures section states the late-attach change: "late attach to an already-ended pane sends one Full then Ended, where the plan's own first draft said Ended alone - found by a test flaking under load during the build ... fixed before the first review, and doc-14 already reflects it." Present and explained.

No findings. What I did not check: anything outside this round's diff and evidence gap (round 1's own findings, the rest of the task's diff, CI) - out of this round's scope per the brief.

---
<!-- COMMENTS:END -->
