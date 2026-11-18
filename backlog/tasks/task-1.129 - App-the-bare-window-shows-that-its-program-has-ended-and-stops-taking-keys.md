---
id: TASK-1.129
title: 'App: the bare window shows that its program has ended, and stops taking keys'
status: In Progress
assignee:
  - '@bruk.w4'
created_date: '2026-11-17T22:39:00Z'
updated_date: '2026-11-18T02:42:00Z'
labels:
  - size-2
milestone: m-4
dependencies:
  - TASK-1.128
  - TASK-1.121
parent_task_id: TASK-1
project: bruk-io-steg
asdlc_accepted_from: DRAFT-1
asdlc_branch: task-1.129-app-the-bare-window-shows-that-its
asdlc_plan_approved_by: bruk
asdlc_plan_approved_hash: '95871fc3dfbf'
asdlc_plan_approved_size: size-2
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Split from TASK-1.128 on 2026-11-17 (its plan recommended it; the default is to split). TASK-1.128 keeps the wire message and the server side; this task is the bare window reacting to it (TASK-1.128 former AC #4). It starts once TASK-1.128 and TASK-1.121 (typing) are both merged, because all three touch crates/app/src/main.rs and client.rs. The drawn states of an ended program and what the window offers next stay with TASK-1.61. Open for the engineer at plan time: whether the window should do anything more than show the line and refuse keys (for example close itself), and quit-on-last-window-closed. TASK-1.128 leaves a minimal placeholder arm for the new update in the app so it compiles; this task replaces it.

The app section of TASK-1.128 first plan (hash fb44af323f0f), kept here as a starting point, not as an approved plan:

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
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 On the final update, the bare window keeps the last screen and shows in words that the program has ended and with what status, or that the pane was closed; the wording comes from one function with its own tests for both reasons
- [ ] #2 Once the final update has arrived the window sends no further input, and the client closes its own side of the stream; a test covers the final update arriving as its own client event, not as an ordinary update or a disconnect
- [ ] #3 Running the app again after its program has ended makes a new tab, as it does today
- [ ] #4 The pull request carries an image of the window after a program exits with a status and after a forced close, from a live run with nothing personal on screen
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
## Design decided by this plan (one advisor review found a real bug in the
first draft - see "The backstop" below - fixed before writing this)

### `ClientEvent` gains `Ended`, decoded in `drain`, not folded into `Update`

`crates/app/src/client.rs`: `ClientEvent::Ended(steg_sdk::EndReason)`
beside `Update`/`Disconnected`. `drain`'s `Incoming::StreamControl { stream,
payload }` arm (captures `stream`, no longer `..`) decodes the payload;
`ScreenUpdate::Ended { reason }` becomes `connection.close_stream(stream)`
(the client's own half of "closed on both sides" - decision-2/doc-14) then
`ClientEvent::Ended(reason)`, `continue`d - not returned `false`: this is
not a protocol surprise, it is this stream's own last, expected frame, and
`read_loop`'s own cleanup line sends `Disconnected` unconditionally once
the loop breaks, which would overwrite whatever `main.rs` does with
`Ended` a moment later if `drain` ever stopped here.

### The backstop for "sends no further input": `Writer::disarm`, never
`Writer::stop` - the bug an advisor review found in this plan's own first
draft

`Writer::stop`'s own doc says it shuts the socket down, "the identical
POSIX guarantee" `Client::stop`'s doc already states for the reader -
because `Attached::write_handle` (`startup.rs:103-106`) is documented as
"a clone of the connected socket," the *same* socket the reader thread
blocks its own `read()` on. Calling `stop` from `handle_client_event` on
`Ended` would therefore also unblock the reader's `read()` with `Ok(0)`,
producing a `ClientEvent::Disconnected` right behind `Ended` - which
overwrites `Phase::Attached` with `Phase::Disconnected`, losing the last
screen and the message AC #1 exists to show. Caught before any code was
written, by an advisor review of this exact plan.

`crates/app/src/client.rs` gains `Writer::disarm(&mut self) { self.commands
= None; }` - no `shutdown()`, no `join()`. `try_send`'s own existing `let
Some(commands) = &self.commands else { return; }` already makes every
later `send_input`/`declare_size` a silent no-op with no `Disconnected`
either; the writer thread itself ends on its own, the next time `commands
.recv()` sees the sender dropped, and `Writer::stop` (called later, by
`teardown`, on window close) still joins it then, same as always.

### A later `Disconnected` must not overwrite `Ended` - stopping the
receive loop, not guarding the arm, is what makes that true

Once `Ended` arrives, `drain`'s own `close_stream` means a later frame on
that stream (there should never be one; decision-2's server-side half
already guarantees it) decodes as `UnknownStream`, which `drain` still
treats as a protocol surprise (`return false`), and `read_loop` sends
`Disconnected` regardless of why the loop broke. Rather than add a second,
explicit guard on the `Disconnected` arm (checking "is `ended` already
set" there too - a second place that has to agree with the first),
`handle_client_event`'s own `Ended` arm returns `true`: the receive task
stops reading the channel at all, so nothing - a stray `Disconnected`, a
server stop, anything else - can ever reach `handle_client_event` again to
overwrite the phase. The reader thread itself is left running, parked in
its own `read()`, harmless until the window closes (`teardown`'s own job,
unchanged) - only the *task that forwards its events to the window* stops.

One ordering note, checked rather than assumed: a late-attach catch-up
`Full` and the `Ended` right behind it can both already be decoded and
queued on the one unbounded channel before the receive task reads either
(both produced in the same `drain` pass over one `read()`'s worth of
bytes). That is still safe - the channel is drained strictly in order, so
the `Full` is applied first and only then does `Ended` stop the loop - but
it depends on that ordering guarantee holding, not on the task happening
to stop mid-batch.

### The window opens already on an ended pane - handled, not special-cased

Late attach (TASK-1.128) sends a catch-up `Full` then `Ended` on the same
stream; `resolve_pane`'s own `program_status != "running"` check
(`startup.rs`, confirmed unchanged - `an_ended_panes_program_gets_a_fresh_
tab_in_the_same_session` is still there and still passes) already avoids
attaching to a pane already known dead at the moment it is read, but a
pane can still end in the tiny window between that read and the real
`pane.attach` moments later - the same race TASK-1.128's own late-attach
fix exists for. `apply_startup_result` builds `Phase::Attached` with
`ended: None` from the attach reply's own snapshot exactly as it does
today; the `Full` (if any) and `Ended` that follow moments later go
through the ordinary `ClientEvent` path above, with no special case -
confirmed by reasoning through the code, not assumed, and the same client
.rs test that proves `Ended` arrives as its own event also covers it
arriving as the very first frame on the stream (no prior `Update` sent at
all in that test), which is exactly this scenario's own shape.

### `describe_end`, and the render tree - identical to today when not ended

`crates/app/src/model.rs`: `pub(crate) fn describe_end(reason: steg_sdk::
EndReason) -> String` (`EndReason` is `Copy` - by value, no borrow to
thread through the render path) - `ProgramExited { exit_code }` -> "the
program ended with status {exit_code}"; `Closed` -> "the pane was closed".
`Model::apply`'s own `ScreenUpdate::Ended` arm stays a no-op (re-worded:
this is now the *final* design, not a placeholder - "ended" is connection
-lifecycle state, not terminal-grid state, so it belongs on `AttachedState`
, not `Model`).

`crates/app/src/main.rs`'s `Render` for `Phase::Attached`: when `state.
ended` is `None`, the element tree stays byte-for-byte what it is today
(`div().size_full().track_focus(...).on_key_down(...).child(terminal)`) -
an advisor review named the risk directly: wrapping every window in a new
flex-column container, even when not ended, would shrink the terminal by
one row's worth of space on every ordinary window, a regression `size.rs`
's own `grid_size`/`declare_size` (TASK-1.121) and the paint budget
(TASK-1.122) would both silently inherit, and nothing in this task's own
reach (no window, no screenshot) would catch. Only the `Some(reason)`
branch builds the taller tree (terminal in a `flex_1` child, `describe_
end`'s own text in a `flex_none` row below, reusing `Theme`'s own tokens -
no new design-system component, per this task's own scope).

## Open for the engineer (my own recommended default first, then the
alternatives - not decided here)

- **Does the window do anything beyond showing the line and refusing
  keys?** Default: no - it keeps the last screen, shows the one line, and
  stops accepting input, nothing else (no self-close, no restart
  affordance). Alternatives: close the window automatically (rejected as
  the default - it would make the very state this task exists to show
  invisible); offer a restart (TASK-1.61's own job, once there is a
  restart API to call at all - none exists yet).
- **Quit-on-last-window-closed:** not addressed by this task - `main()`
  opens exactly one window, once, with no multi-window support anywhere
  in this crate yet, so "last window" is not yet a real question this
  app's own code can act on.
- **The exact wording:** default "the program ended with status {exit_
  code}" / "the pane was closed" - plain statements of fact, no judgement
  about whether a given status counts as success. Alternative considered
  and not chosen: wording success/failure differently by exit code (e.g.
  "finished" vs "failed") - that is exactly the kind of drawn-state
  judgement TASK-1.61 owns, and doc-10 explicitly defers every drawn state
  of an ended program to it; this task's own plain text is a stopgap, not
  a first draft of that design.

## Files that change

- `crates/app/src/client.rs`: `ClientEvent::Ended`; `drain`'s `StreamControl`
  arm split (above); `Writer::disarm`. Three new tests (Proof, below).
- `crates/app/src/model.rs`: `describe_end` and its own two tests; the
  `Ended` arm's comment reworded (no behaviour change there).
- `crates/app/src/main.rs`: `AttachedState.ended: Option<steg_sdk::
  EndReason>` (`None` at attach); `handle_client_event`'s new `Ended` arm
  (sets `ended`, calls `state.writer.disarm()`, notifies, returns `true`);
  `send_bytes` and `on_key_down`'s paste branch each skip their own send
  when `ended.is_some()` (clarity/avoiding a pointless clipboard read -
  `disarm` is what actually guarantees nothing reaches the wire, these are
  not load-bearing on their own); `on_resize` skips declaring a size the
  same way, for the same reason (team's own question: "should an ended
  window still declare sizes" - no); `Render`'s `Phase::Attached` arm
  split into the unchanged tree (`ended: None`) and the taller one
  (`Some(reason)`, above).
- Nothing in `crates/sdk`/`crates/server` changes - this task is entirely
  `crates/app`.

## Order of work

1. `model.rs`: `describe_end`, both tests - written failing first (a stub
   returning `String::new()`), recorded failing, then the real
   implementation, recorded passing.
2. `client.rs`: `ClientEvent::Ended`, `drain`'s split, `Writer::disarm`,
   and the three new tests, each with its own failing-then-passing
   control (Proof, below). Adding `ClientEvent::Ended` makes `main.rs`'s
   own `handle_client_event` match non-exhaustive at once (`wildcard_
   enum_match_arm = "deny"` is workspace-wide - the same shape TASK-1.128
   hit adding `ScreenUpdate::Ended`, one crate over) - this step also adds
   a minimal, real arm there (`ClientEvent::Ended(reason) => { ... }` with
   the actual field set and `disarm()` called, not a bare placeholder),
   so the crate compiles and this step's own tests run; step 3 is the
   render and the three guard sites, not the arm itself.
3. `main.rs`: the three guard sites and the render split. Checked with a
   plain `git diff` on the `Phase::Attached` render arm: the `None`
   branch's own lines must appear as context, not as part of the diff -
   if that branch shows as rewritten rather than unchanged, the tree is
   not actually identical to today's and the risk below has landed, not
   merely been named.
4. `just check` at a clean tree.
5. Hand off to the engineer for AC #4's two screenshots - a live run
   this task does not perform (workers do not inject input, capture the
   screen, or touch the clipboard on this machine).

## Risks

- **A render regression for the ordinary, not-ended window** (named above)
  - mitigated by keeping that branch's own element tree unchanged (step
  3's own `git diff` check), and by asking the engineer's own live run
  (already planned for AC #4) to also confirm the terminal's own size
  looks unchanged from before this task, not only that the ended state
  renders.
- **Three separate guard sites in `main.rs`** (`send_bytes`, the paste
  branch, `on_resize`) could be missed by a future edit - mitigated by
  `Writer::disarm` being the one, correctness-bearing backstop none of the
  three actually depend on; a missed guard wastes a clipboard read or a
  size computation, never reaches the wire.
- **`main.rs`'s own wiring (which `ClientEvent` sets what state, and what
  actually renders) has no automated test** - `crates/app/src/terminal_
  element.rs:711-727` already states this crate's own precedent for
  exactly this gap (gpui's `test-support` feature deliberately not
  enabled - new dependencies and licence surface for one test; a hand
  -built `ShapedLine`/`GlyphId` not constructible from outside gpui
  either) and its own chosen fallback, a live run. This task follows the
  same precedent rather than reopening that question: `describe_end`'s own
  wording and `client.rs`'s own event/disarm mechanisms are proven by
  real tests; the glue between them and the screen is proven only by the
  engineer's own live run (AC #4).
- **The "window opens already ended" race** (above) is handled by the
  existing mechanisms with nothing special to add - named so a reviewer
  does not read its absence from "files that change" as an oversight.

## Proof, one path per acceptance criterion

- **AC #1** (last screen kept; wording from one function, tested for both
  reasons): `describe_end`'s own two unit tests, each recorded failing
  (stub) then passing (real text). The render wiring itself - that `Some
  (reason)` really appends this exact line, under the last screen, in the
  real window - is proven only by the engineer's own live run (the
  `terminal_element.rs` precedent above); no automated test claims more
  than that.
- **AC #2** (own client event, not an update or a disconnect; no further
  input; closes its own side):
  - `an_ended_update_arrives_as_its_own_client_event_not_update_or_
    disconnected`: real encoded `Ended` bytes through a real socket pair
    into `drain`, asserting `ClientEvent::Ended(reason)` specifically.
    Control: temporarily let the `Ended` payload fall through to the
    ordinary `Update` path: fails, decoded as `Update`, not `Ended`.
    Reverted, passes.
  - `ended_closes_this_connections_own_side_so_a_later_frame_on_the_same_
    stream_is_unknown`: feeds `Ended` then one more, ordinary frame on the
    same stream; asserts the second decodes as a protocol surprise (this
    test's own comment names explicitly that in the real, wired-up app no
    later frame is ever processed this way at all, since the receive task
    has already stopped by then - this proves the lower-level `Connection`
    mechanism in isolation, not a reachable production path). Control:
    skip the `close_stream` call: fails, the second frame instead decodes
    as an ordinary `Update`. Reverted, passes.
  - `writer_disarm_makes_every_later_send_a_silent_no_op_with_no_
    disconnected_either`: `disarm()`, then `send_input`/`declare_size`;
    asserts nothing arrives on the server side within a bound and no
    `Disconnected` fires either. Control: skip calling `disarm`: fails,
    the send does arrive. Reverted, passes.
  - "No further input actually leaves `main.rs`'s own three call sites" -
    not separately tested (the same `terminal_element.rs` gap as AC #1):
    proven by `disarm`'s own test above plus the engineer's own live run
    (typing after the message shows produces no visible effect).
- **AC #3** (relaunch makes a new tab): nothing in this diff touches
  `resolve_pane` or `pane.list`'s own `program_status` check - no new
  control, since there is no new mechanism to prove. The existing,
  unmodified `startup.rs` test (`an_ended_panes_program_gets_a_fresh_tab_
  in_the_same_session`) continues to pass under `just test`. Offered,
  not pre-empted: if recorded evidence is wanted anyway, I can temporarily
  break `resolve_pane`'s own check and record that existing test failing
  then passing as a control for *that* mechanism once you say so; not
  done unasked, since this diff does not touch it.
- **AC #4** (the PR carries two images): no automated test. The engineer
  supplies both screenshots from their own live run (program exit with a
  status; a forced close) - this task does not drive the app, capture the
  screen, or touch the clipboard.

No contract-change request: nothing in `doc-10`/`doc-14`/decision-2 needs
a word changed for this task - the wording this task adds is explicitly a
plain-text stopgap doc-10 already defers to TASK-1.61, not part of the
design system doc-10 describes.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Plan (95871fc3dfbf) checked against doc-14's 'The end of a pane, on its stream', doc-10 and TASK-1.128/1.121 as merged by @bruk.o: ok. It builds only the window's reaction; Ended is its own client event; input stops by disarming the writer without touching the shared socket, so no disconnect follows; the not-ended render tree is unchanged. Three questions go to the engineer with the plan (nothing beyond the line and refusing keys; quit-on-last-window not addressed; the wording). For criterion 3 the orchestrator asks for the offered control on the existing resolve_pane test to be recorded. Criterion 4's images come from the engineer's own run: workers do not drive the app on this machine.

Check FAILED: `cargo test -p steg-app -- describe_end` exited 101 at 2026-11-18T02:26:34+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.129 on task-1.129-app-the-bare-window-shows-that-its at 2dd9f5117641 with uncommitted changes, last 40 of 43 lines:

```text
124 | pub(crate) fn describe_end(reason: EndReason) -> String {
    |                            ^^^^^^ help: if this is intentional, prefix it with an underscore: `_reason`
    |
    = note: `#[warn(unused_variables)]` (part of `#[warn(unused)]`) on by default

warning: `steg-app` (bin "steg-app" test) generated 1 warning (run `cargo fix --bin "steg-app" -p steg-app --tests` to apply 1 suggestion)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.23s
warning: the following packages contain code that will be rejected by a future version of Rust: block v0.1.6, proc-macro-error2 v2.0.1
note: to see what the problems were, use the option `--future-incompat-report`, or run `cargo report future-incompatibilities --id 1`
     Running unittests src/main.rs (target/debug/deps/steg_app-edc2b453f94d193e)

running 2 tests
test model::tests::describe_end_names_the_programs_own_exit_status ... FAILED
test model::tests::describe_end_names_the_pane_as_closed ... FAILED

failures:

---- model::tests::describe_end_names_the_programs_own_exit_status stdout ----

thread 'model::tests::describe_end_names_the_programs_own_exit_status' (56589636) panicked at crates/app/src/model.rs:136:9:
assertion `left == right` failed
  left: ""
 right: "the program ended with status 3"
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace

---- model::tests::describe_end_names_the_pane_as_closed stdout ----

thread 'model::tests::describe_end_names_the_pane_as_closed' (56589635) panicked at crates/app/src/model.rs:144:9:
assertion `left == right` failed
  left: ""
 right: "the pane was closed"


failures:
    model::tests::describe_end_names_the_pane_as_closed
    model::tests::describe_end_names_the_programs_own_exit_status

test result: FAILED. 0 passed; 2 failed; 0 ignored; 0 measured; 68 filtered out; finished in 0.00s

error: test failed, to rerun pass `-p steg-app --bin steg-app`
```

Check passed: `cargo test -p steg-app -- describe_end` exited 0 at 2026-11-18T02:26:49+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.129 on task-1.129-app-the-bare-window-shows-that-its at 2dd9f5117641 with uncommitted changes:

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.23s
warning: the following packages contain code that will be rejected by a future version of Rust: block v0.1.6, proc-macro-error2 v2.0.1
note: to see what the problems were, use the option `--future-incompat-report`, or run `cargo report future-incompatibilities --id 1`
     Running unittests src/main.rs (target/debug/deps/steg_app-edc2b453f94d193e)

running 2 tests
test model::tests::describe_end_names_the_pane_as_closed ... ok
test model::tests::describe_end_names_the_programs_own_exit_status ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 68 filtered out; finished in 0.00s
```

Check FAILED: `cargo test -p steg-app -- an_ended_update_arrives` exited 101 at 2026-11-18T02:30:41+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.129 on task-1.129-app-the-bare-window-shows-that-its at 2dd9f5117641 with uncommitted changes, last 40 of 44 lines:

```text
    |                                       ^^^^^^ help: try ignoring the field: `stream: _`
    |
    = note: `#[warn(unused_variables)]` (part of `#[warn(unused)]`) on by default

warning: variant `Ended` is never constructed
  --> crates/app/src/client.rs:32:5
   |
23 | pub enum ClientEvent {
   |          ----------- variant in this enum
...
32 |     Ended(EndReason),
   |     ^^^^^
   |
   = note: `ClientEvent` has a derived impl for the trait `Debug`, but this is intentionally ignored during dead code analysis
   = note: `#[warn(dead_code)]` (part of `#[warn(unused)]`) on by default

warning: `steg-app` (bin "steg-app" test) generated 2 warnings (run `cargo fix --bin "steg-app" -p steg-app --tests` to apply 1 suggestion)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.22s
warning: the following packages contain code that will be rejected by a future version of Rust: block v0.1.6, proc-macro-error2 v2.0.1
note: to see what the problems were, use the option `--future-incompat-report`, or run `cargo report future-incompatibilities --id 1`
     Running unittests src/main.rs (target/debug/deps/steg_app-edc2b453f94d193e)

running 1 test
test client::tests::an_ended_update_arrives_as_its_own_client_event_not_update_or_disconnected ... FAILED

failures:

---- client::tests::an_ended_update_arrives_as_its_own_client_event_not_update_or_disconnected stdout ----

thread 'client::tests::an_ended_update_arrives_as_its_own_client_event_not_update_or_disconnected' (56623209) panicked at crates/app/src/client.rs:474:9:
expected Ended, not Update or Disconnected: Some(Update(Ended { reason: ProgramExited { exit_code: 3 } }))
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    client::tests::an_ended_update_arrives_as_its_own_client_event_not_update_or_disconnected

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 72 filtered out; finished in 0.00s

error: test failed, to rerun pass `-p steg-app --bin steg-app`
```

Check passed: `cargo test -p steg-app -- an_ended_update_arrives` exited 0 at 2026-11-18T02:30:58+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.129 on task-1.129-app-the-bare-window-shows-that-its at 2dd9f5117641 with uncommitted changes:

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.29s
warning: the following packages contain code that will be rejected by a future version of Rust: block v0.1.6, proc-macro-error2 v2.0.1
note: to see what the problems were, use the option `--future-incompat-report`, or run `cargo report future-incompatibilities --id 1`
     Running unittests src/main.rs (target/debug/deps/steg_app-edc2b453f94d193e)

running 1 test
test client::tests::an_ended_update_arrives_as_its_own_client_event_not_update_or_disconnected ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 72 filtered out; finished in 0.00s
```

Check FAILED: `cargo test -p steg-app -- ended_closes_this_connections` exited 101 at 2026-11-18T02:31:15+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.129 on task-1.129-app-the-bare-window-shows-that-its at 2dd9f5117641 with uncommitted changes:

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.25s
warning: the following packages contain code that will be rejected by a future version of Rust: block v0.1.6, proc-macro-error2 v2.0.1
note: to see what the problems were, use the option `--future-incompat-report`, or run `cargo report future-incompatibilities --id 1`
     Running unittests src/main.rs (target/debug/deps/steg_app-edc2b453f94d193e)

running 1 test
test client::tests::ended_closes_this_connections_own_side_so_a_later_frame_on_the_same_stream_is_unknown ... FAILED

failures:

---- client::tests::ended_closes_this_connections_own_side_so_a_later_frame_on_the_same_stream_is_unknown stdout ----

thread 'client::tests::ended_closes_this_connections_own_side_so_a_later_frame_on_the_same_stream_is_unknown' (56647047) panicked at crates/app/src/client.rs:518:9:
a frame on a stream this connection already closed its own side of must decode as a protocol surprise, not an ordinary update: Some(Update(Unchanged))
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    client::tests::ended_closes_this_connections_own_side_so_a_later_frame_on_the_same_stream_is_unknown

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 72 filtered out; finished in 0.00s

error: test failed, to rerun pass `-p steg-app --bin steg-app`
```

Check passed: `cargo test -p steg-app -- ended_closes_this_connections` exited 0 at 2026-11-18T02:31:30+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.129 on task-1.129-app-the-bare-window-shows-that-its at 2dd9f5117641 with uncommitted changes:

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.22s
warning: the following packages contain code that will be rejected by a future version of Rust: block v0.1.6, proc-macro-error2 v2.0.1
note: to see what the problems were, use the option `--future-incompat-report`, or run `cargo report future-incompatibilities --id 1`
     Running unittests src/main.rs (target/debug/deps/steg_app-edc2b453f94d193e)

running 1 test
test client::tests::ended_closes_this_connections_own_side_so_a_later_frame_on_the_same_stream_is_unknown ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 72 filtered out; finished in 0.00s
```

Check FAILED: `cargo test -p steg-app -- writer_disarm_makes_every_later_send` exited 101 at 2026-11-18T02:31:50+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.129 on task-1.129-app-the-bare-window-shows-that-its at 2dd9f5117641 with uncommitted changes:

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.26s
warning: the following packages contain code that will be rejected by a future version of Rust: block v0.1.6, proc-macro-error2 v2.0.1
note: to see what the problems were, use the option `--future-incompat-report`, or run `cargo report future-incompatibilities --id 1`
     Running unittests src/main.rs (target/debug/deps/steg_app-edc2b453f94d193e)

running 1 test
test client::tests::writer_disarm_makes_every_later_send_a_silent_no_op_with_no_disconnected_either ... FAILED

failures:

---- client::tests::writer_disarm_makes_every_later_send_a_silent_no_op_with_no_disconnected_either stdout ----

thread 'client::tests::writer_disarm_makes_every_later_send_a_silent_no_op_with_no_disconnected_either' (56671693) panicked at crates/app/src/client.rs:659:9:
nothing should reach the wire after disarm: Ok(21)
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    client::tests::writer_disarm_makes_every_later_send_a_silent_no_op_with_no_disconnected_either

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 72 filtered out; finished in 0.00s

error: test failed, to rerun pass `-p steg-app --bin steg-app`
```

Check passed: `cargo test -p steg-app -- writer_disarm_makes_every_later_send` exited 0 at 2026-11-18T02:32:06+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.129 on task-1.129-app-the-bare-window-shows-that-its at 2dd9f5117641 with uncommitted changes:

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.24s
warning: the following packages contain code that will be rejected by a future version of Rust: block v0.1.6, proc-macro-error2 v2.0.1
note: to see what the problems were, use the option `--future-incompat-report`, or run `cargo report future-incompatibilities --id 1`
     Running unittests src/main.rs (target/debug/deps/steg_app-edc2b453f94d193e)

running 1 test
test client::tests::writer_disarm_makes_every_later_send_a_silent_no_op_with_no_disconnected_either ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 72 filtered out; finished in 0.40s
```

Check FAILED: `cargo test -p steg-app -- an_ended_panes_program` exited 101 at 2026-11-18T02:36:09+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.129 on task-1.129-app-the-bare-window-shows-that-its at 2dd9f5117641 with uncommitted changes:

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.23s
warning: the following packages contain code that will be rejected by a future version of Rust: block v0.1.6, proc-macro-error2 v2.0.1
note: to see what the problems were, use the option `--future-incompat-report`, or run `cargo report future-incompatibilities --id 1`
     Running unittests src/main.rs (target/debug/deps/steg_app-edc2b453f94d193e)

running 1 test
test startup::tests::an_ended_panes_program_gets_a_fresh_tab_in_the_same_session ... FAILED

failures:

---- startup::tests::an_ended_panes_program_gets_a_fresh_tab_in_the_same_session stdout ----

thread 'startup::tests::an_ended_panes_program_gets_a_fresh_tab_in_the_same_session' (56764566) panicked at crates/app/src/startup.rs:619:21:
assertion `left != right` failed: the client side closed before sending the expected request
  left: 0
 right: 0
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    startup::tests::an_ended_panes_program_gets_a_fresh_tab_in_the_same_session

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 72 filtered out; finished in 0.00s

error: test failed, to rerun pass `-p steg-app --bin steg-app`
```

Check passed: `cargo test -p steg-app -- an_ended_panes_program` exited 0 at 2026-11-18T02:36:28+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.129 on task-1.129-app-the-bare-window-shows-that-its at 2dd9f5117641 with uncommitted changes:

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.23s
warning: the following packages contain code that will be rejected by a future version of Rust: block v0.1.6, proc-macro-error2 v2.0.1
note: to see what the problems were, use the option `--future-incompat-report`, or run `cargo report future-incompatibilities --id 1`
     Running unittests src/main.rs (target/debug/deps/steg_app-edc2b453f94d193e)

running 1 test
test startup::tests::an_ended_panes_program_gets_a_fresh_tab_in_the_same_session ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 72 filtered out; finished in 0.00s
```

Check FAILED: `just check` exited 1 at 2026-11-18T02:36:41+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.129 on task-1.129-app-the-bare-window-shows-that-its at 2dd9f5117641 with uncommitted changes, last 40 of 41 lines:

```text
Diff in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.129/crates/app/src/client.rs:480:
         assert!(
             matches!(
                 event,
[31m-                Some(ClientEvent::Ended(EndReason::ProgramExited { exit_code: 3 }))
(B[m[32m+                Some(ClientEvent::Ended(EndReason::ProgramExited {
(B[m[32m+                    exit_code: 3
(B[m[32m+                }))
(B[m             ),
             "expected Ended, not Update or Disconnected: {event:?}"
         );
Diff in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.129/crates/app/src/main.rs:537:
                         .flex_col()
                         .track_focus(&self.focus_handle)
                         .on_key_down(cx.listener(Self::on_key_down))
[31m-                        .child(
(B[m[31m-                            div().flex_1().child(terminal_element::terminal(
(B[m[31m-                                Rc::clone(&state.model),
(B[m[31m-                                state.metrics.clone(),
(B[m[31m-                                terminal_theme,
(B[m[31m-                                Rc::clone(&state.row_cache),
(B[m[31m-                                self.focus_handle.clone(),
(B[m[31m-                                cx.entity(),
(B[m[31m-                                Rc::clone(&state.last_bounds),
(B[m[31m-                            )),
(B[m[31m-                        )
(B[m[32m+                        .child(div().flex_1().child(terminal_element::terminal(
(B[m[32m+                            Rc::clone(&state.model),
(B[m[32m+                            state.metrics.clone(),
(B[m[32m+                            terminal_theme,
(B[m[32m+                            Rc::clone(&state.row_cache),
(B[m[32m+                            self.focus_handle.clone(),
(B[m[32m+                            cx.entity(),
(B[m[32m+                            Rc::clone(&state.last_bounds),
(B[m[32m+                        )))
(B[m                         .child(
                             div()
                                 .flex_none()
full output: .quiet/format-check.log
error: recipe `check-fmt` failed on line 157 with exit code 1
```

Check FAILED: `just check` exited 101 at 2026-11-18T02:42:12+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.129 on task-1.129-app-the-bare-window-shows-that-its at 2dd9f5117641 with uncommitted changes, last 40 of 79 lines:

```text
test pane::attach_tests::a_program_that_exits_on_its_own_sends_ended_with_its_real_exit_code_after_its_last_content ... ok
test pane::attach_tests::mark_closing_sent_before_a_programs_own_exit_is_the_reason_ended_reports ... ok
kill: 85895: No such process
test runtime::stop_tests::stop_ends_a_program_that_traps_the_polite_signal ... ok
test runtime::tests::an_unfiltered_subscriber_receives_nothing ... ok
test pane::ending_tests::await_exit_is_not_delayed_by_a_backlog_on_the_workers_own_queue ... ok
test test_support::tests::a_body_that_never_returns_fails_fast_naming_what_it_waited_for - should panic ... ok
test pane::ending_tests::signal_hangup_reaches_a_backgrounded_grandchild_too ... ok
test idle::tests::the_checker_blocks_with_no_timeout_while_ineligible_and_waits_exactly_once_while_eligible ... ok
test runtime::tests::the_general_sizing_step_reaches_every_pane_through_split_close_and_swap ... ok
test runtime::tests::a_mark_then_the_shells_own_real_exit_cascades_with_no_panic ... ok
test runtime::tests::a_shell_with_no_marks_leaves_every_status_field_unchanged_until_one_arrives ... ok
test pane::ending_tests::signal_kill_ends_a_program_that_traps_the_polite_signal ... ok
test runtime::tests::a_typed_command_that_fails_is_failed_with_its_exit_code ... ok
test runtime::tests::a_typed_command_that_succeeds_is_running_then_finished_and_the_tab_shows_it ... ok
test runtime::tests::a_typed_commands_status_reaches_pane_show_and_tab_show_through_runtime_handle ... ok
test runtime::tests::a_command_panes_own_program_printing_marks_does_not_change_its_status ... ok
test login_env::tests::capture_login_environment_never_leaks_a_variable_set_only_in_the_callers_own_environment ... ok
kill: 85891: No such process
test pane::tests::a_forward_failing_once_the_worker_is_gone_still_reaps_a_program_that_ends_on_its_own ... ok
test login_env::tests::capture_login_environment_falls_back_on_a_shell_that_exits_non_zero ... ok
test login_env::tests::capture_login_environment_prints_noise_before_the_marker_without_corrupting_the_dump ... ok
test login_env::tests::capture_login_environment_kills_a_backgrounded_grandchild_that_held_the_pipe_open ... FAILED

failures:

---- login_env::tests::capture_login_environment_kills_a_backgrounded_grandchild_that_held_the_pipe_open stdout ----

thread 'login_env::tests::capture_login_environment_kills_a_backgrounded_grandchild_that_held_the_pipe_open' (56979137) panicked at crates/server/src/login_env.rs:659:13:
the backgrounded grandchild's own pid file never appeared


failures:
    login_env::tests::capture_login_environment_kills_a_backgrounded_grandchild_that_held_the_pipe_open

test result: FAILED. 202 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 10.02s

error: test failed, to rerun pass `-p steg-server --lib`
full output: .quiet/test-steg-server-race-fixture.log
error: recipe `check-packages` failed with exit code 101
```
<!-- SECTION:NOTES:END -->

## Comments

<!-- COMMENTS:BEGIN -->
author: @bruk
created: 2026-11-18T02:25:00Z

---

Plan approved by @bruk (95871fc3dfbf, size-2): its worker may build it.

---
<!-- COMMENTS:END -->
