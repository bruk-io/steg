---
id: TASK-1.129
title: 'App: the bare window shows that its program has ended, and stops taking keys'
status: To Do
assignee: []
created_date: '2026-11-17T22:39:00Z'
labels:
  - size-2
milestone: m-4
dependencies:
  - TASK-1.128
  - TASK-1.121
parent_task_id: TASK-1
project: bruk-io-steg
asdlc_accepted_from: DRAFT-1
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
