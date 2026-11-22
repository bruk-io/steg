---
id: TASK-1.153
title: 'Kernel: a cursor move with no cell change still reaches clients'
status: In Progress
assignee:
  - '@bruk.w4'
created_date: '2026-11-22T04:58:00Z'
updated_date: '2026-11-22T05:07:00Z'
labels:
  - size-2
milestone: m-4
dependencies: []
parent_task_id: TASK-1
project: bruk-io-steg
asdlc_accepted_from: DRAFT-1
asdlc_branch: task-1.153-kernel-a-cursor-move-with-no-cell-change
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found by the engineer in the live run of TASK-1.127 on 22 November 2026, and traced by that task's worker to main, not to the cursor change: at a shell prompt, arrow keys and backspace move the cursor but it is not redrawn until the next typed character. Cause: crates/kernel/src/terminal.rs Terminal::changes_since reads the cursor every time but attaches it only when the engine's dirty flag is Partial or Full; on Clean it returns ScreenUpdate::Unchanged, which carries no cursor (crates/sdk/src/screen.rs). The engine's dirty flag tracks cell content, not cursor position, so a cursor-only move is reported as nothing and the app's model applies nothing. A scratch test (write text, read, write CSI C, read again) fails with Unchanged; it was reverted and is not on any branch. The plan decides how the kernel knows the cursor moved (for example by remembering the cursor it last reported per sequence) and whether the update is a Delta with no rows or needs a new shape in the sdk; a new shape is a public API change and must be said plainly. Every attached client is affected, not only the app. Not checked: whether the shell's backspace erases a cell; the plan should cover both.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 After a write that only moves the cursor (for example CSI C, or a backspace that erases nothing), changes_since reports the new cursor position instead of Unchanged; a kernel test proves it and a recorded control shows it failing against today's code
- [ ] #2 When neither a cell nor the cursor has changed, changes_since still reports Unchanged, so an idle pane sends nothing
- [ ] #3 Cursor visibility and shape changes with no cell change are covered the same way, or the notes say why they already work
- [ ] #4 The app draws the cursor at its new cell after such an update, shown by a test at the app's model; just check passes
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
## What the neighbours actually do, confirmed by scratch tests (reverted, not on any branch)

Five scratch tests against `Terminal::changes_since` (write some text, consume the first `Full`, then write one more thing with no further cell content, call `changes_since(Some(seq))` again):

| Written (no cell content otherwise) | Result today |
|---|---|
| `\x1b[C` (CSI cursor-forward - a pure cursor move, the same shape an arrow key produces once the shell echoes it) | `Unchanged` (the bug) |
| `\x1b[?25l` (DECTCEM, hide the cursor) | `Unchanged` |
| `\x1b[2 q` (DECSCUSR, steady block - also flips `blinking`) | `Unchanged` |
| `\x08 \x08` (BS, space, BS - the classic raw-mode backspace that erases a cell) | **not** `Unchanged` - already reported, via `Dirty::Partial` |
| `\x08` alone (a plain backspace with no erase) | `Unchanged` - the same gap as a pure cursor move |

So: AC#3's cursor visibility and shape are **not** already covered - they fail the exact same way position does, confirmed empirically, not assumed from reading. A cell-erasing backspace already works (the erase is a real cell write, `Dirty::Partial` already carries it); a non-erasing backspace has the same gap as an arrow key. The fix in `read_cursor`'s own terms covers all of these at once: `Cursor { x, y, visible, blinking, shape }` is one struct, and comparing the whole thing, not just `x`/`y`, is what makes AC#1 and AC#3 the same mechanism rather than two.

## Where the kernel keeps "did the cursor change", and why there

`Terminal::changes_since` is called exactly once per pane per tick, by `crates/server/src/pane.rs`'s own `run_tick` (confirmed by reading it, not assumed: `run_tick(terminal: &mut Terminal, tracked_seq: &mut u64, attachments: &mut [Attachment])` takes one `tracked_seq: &mut u64` - a single, pane-wide value, not one per attachment - and calls `terminal.changes_since(seen)` exactly once, then fans the one resulting `ScreenUpdate` out to every armed attachment below, each tracking its own `caught_up_to`/`last_told_size` entirely in `pane.rs`, never inside the kernel). A stale or just-armed attachment gets its own independent `Full` from `terminal.screen()` instead (`run_tick`'s own "needs a fresh, independent... screen" branch) - a different method, which always reads the cursor fresh and unconditionally, with no "last reported" state of its own to get wrong.

This settles where a "last reported cursor" belongs: on `Terminal` itself, read and written only inside `changes_since`, exactly the same scope `self.seq` already has - not per-attachment (attachments never call the kernel directly), and not touched by `screen()` (which already reports the cursor unconditionally every time, needing no memory of what it told anyone before). The failure mode the brief names - "a last reported cursor kept in the wrong place lets one client's read hide the change from another" - would need two *different* `seen` values to reach `changes_since` for the same pane in the same tick, which the existing single-caller, single-`tracked_seq` design in `pane.rs` does not do; a kernel-level test still proves the one path that *could* matter (two direct callers at different `seen` values, as the kernel's own existing test suite already drives it, never as the server actually calls it) does not go wrong either: a stale caller's `is_current == false` branch always returns a fresh `Full` with the real cursor regardless of what the "current" caller was told, and setting `last_reported_cursor` from that `Full` too keeps the invariant consistent for whichever caller is "current" next.

## The fix, minimal and confined to one file

A new field, `last_reported_cursor: Option<Cursor>` on `Terminal` (`None` at `Terminal::new`), with the invariant "`self.seq != 0` implies `self.last_reported_cursor.is_some()`" (both are set together, in the same place, documented on the field). `changes_since`'s own `is_current` match gains one guarded arm, and one line after `outcome` is computed:

```rust
let outcome = if is_current {
    match dirty {
        Dirty::Clean if Some(cursor) != self.last_reported_cursor => {
            Some(Outcome::Delta(Vec::new()))
        }
        Dirty::Clean => None,
        Dirty::Partial => Some(Outcome::Delta(dirty_rows(&snapshot, &palette)?)),
        Dirty::Full => Some(Outcome::Full(all_rows(&snapshot, &palette)?)),
    }
} else {
    Some(Outcome::Full(all_rows(&snapshot, &palette)?))
};

if outcome.is_some() {
    self.last_reported_cursor = Some(cursor);
}

snapshot.clean().map_err(Error::Engine)?;
```

Nothing below this changes: `Some(Outcome::Delta(rows))` already bumps `self.seq` and builds `ScreenUpdate::Delta { rows, cursor, seq: self.seq }` regardless of whether `rows` is empty - the existing match arm, untouched, already does exactly the right thing for a cursor-only change once `outcome` names it. `Outcome`/`ScreenUpdate` themselves need no new variant.

## Question #3, answered, not asked: no sdk change

`ScreenUpdate::Delta { rows: Vec<(u16, Row)>, cursor: Cursor, seq: u64 }` already lets `rows` be empty while still carrying a real `cursor` and bumping `seq` - a cursor-only change is a `Delta` whose `rows` happens to be empty, not a new shape. Checked every place in the workspace that matches on `ScreenUpdate` (grepped, not assumed):

- `crates/kernel/src/terminal.rs` (the producer, this fix).
- `crates/server/src/pane.rs`'s `run_tick`: `content_changed = !matches!(outcome, ScreenUpdate::Unchanged)` - a `Delta` with empty `rows` is still a `Delta`, so `content_changed` is `true`; nowhere in the fan-out loop (`any_work`, the per-attachment send) is `rows.is_empty()` checked at all. Confirmed by reading the whole function.
- `crates/app/src/model.rs`'s `Model::apply`: `ScreenUpdate::Delta { rows, cursor, .. } => { for (index, row) in rows { ... } self.cursor = cursor; }` - looping over an empty `rows` is a no-op, and `self.cursor = cursor` is unconditional, never gated on `rows` being non-empty. **This already does the right thing today, with no change** - AC#4 needs a test, not a fix, here.
- `crates/app/src/client.rs`: decodes and forwards `ScreenUpdate` opaquely, only special-casing `Ended`; a `Delta` (empty rows or not) passes straight through as `ClientEvent::Update`.
- `crates/server/src/connection.rs`: only matches `ScreenUpdate::Ended`, for the pane-end path - untouched by this.
- `crates/steg` (the CLI): grepped, no reference to `ScreenUpdate` at all - it never sees one directly.

So: no public API change, nothing to put to the engineer as a question, and no consumer needs editing. The alternative (a new `ScreenUpdate::CursorOnly` variant) was considered and rejected here precisely because it would be a real public API change for zero gain - every consumer already handles an empty-`rows` `Delta` correctly.

## Files that change

- `crates/kernel/src/terminal.rs`: the field, the `changes_since` change above, and its own new tests.
- `crates/app/src/model.rs`: one new test proving the already-correct behaviour (no production change) - no collision with TASK-1.127 (that task's own diff never touches this file, confirmed).

No file under `third_party/` is touched - the fix is entirely in how the kernel's own wrapper (`changes_since`) decides what to report from information (`read_cursor`, `Dirty`) the engine already exposes; nothing about the engine's own dirty-tracking needs to change, and nothing here asks it to.

## Order of work

1. The field and its `Terminal::new` initialisation, with the invariant documented on the field itself.
2. The `changes_since` change above.
3. Kernel tests (each a recorded failing-then-passing pair, the control named per test below).
4. The `model.rs` test (also a recorded failing-then-passing pair, control below).
5. A short addition to `changes_since`'s own doc comment: it now also reports a cursor-only change (position, visibility or shape) as a `Delta` with no rows, and the `last_reported_cursor` invariant.
6. `just check`.

## Proof, one control per mechanism

- **AC#1**: `changes_since_reports_a_cursor_only_delta_when_only_the_cursor_moved` - write "hi", consume the `Full`, write `\x1b[C`, call `changes_since(Some(seq))`, assert it is a `Delta` with empty `rows` and the moved cursor, not `Unchanged`. Control: temporarily revert the guard to plain `Dirty::Clean => None` (today's code) and watch this fail with `Unchanged` - the same failure already seen in the scratch test above, now kept as the task's own control rather than thrown away.
- **AC#2**: `changes_since_reports_unchanged_when_nothing_happened_since_the_last_call` (already exists, still passes - the common "idle, nothing at all" case) plus a new `a_cursor_only_delta_is_reported_once_then_idle_stays_silent`: after the cursor-only `Delta` from AC#1's own scenario is reported, call `changes_since` again with its new `seq` and nothing written since - expect `Unchanged`. Control: a scratch edit that drops the `Some(cursor) != self.last_reported_cursor` comparison (always takes the cursor-only branch on any `Dirty::Clean`) and watch both this test and the existing unchanged test fail (spuriously reporting every idle tick) - proving AC#1's own fix does not quietly break AC#2.
- **AC#3**: `changes_since_reports_a_cursor_only_delta_when_only_visibility_changed` (`\x1b[?25l`) and `..._when_only_the_shape_changed` (`\x1b[2 q`, which also flips `blinking`) - same shape as AC#1's own test, each with the same control (the guard reverted), proving the one mechanism covers all three rather than needing three separate fixes.
- **Several readers, not getting this wrong**: `a_stale_callers_full_screen_still_carries_the_fresh_cursor_after_another_caller_saw_a_cursor_only_delta` - call `changes_since(Some(seq))` (current, gets the cursor-only `Delta`, bumping `seq`), then call `changes_since` again with the *old*, now-stale `seq` (standing in for a second, behind caller) and assert it gets a `Full` carrying the real, moved cursor - not confused by the first caller's own `last_reported_cursor` update. No control needed beyond the assertion itself succeeding against the real fix; this is the direct answer to "mind several attached clients", proven rather than argued.
- **AC#4**: `crates/app/src/model.rs`, `a_delta_with_no_rows_still_moves_the_cursor` - apply a `ScreenUpdate::Delta { rows: Vec::new(), cursor: <moved>, seq: 1 }` to a model built from a snapshot at the old cursor position, assert `model.cursor()` is the new one. Control: temporarily gate `self.cursor = cursor` behind `if !rows.is_empty()` in `Model::apply` (a scratch edit, reverted after recording) and watch this new test fail - proving the test is not vacuously passing against code that was already unconditionally correct, even though no production change is needed here.
- `just check` recorded clean at the final head.

## Risks

- **Backspace's real wire bytes are still unconfirmed without a live run** - both shapes (erasing, non-erasing) are tested directly at the kernel level with the two raw byte sequences, and AC#1's own fix covers the non-erasing one the same way an arrow key is covered; which one a real shell actually sends is the engineer's own to see on the next live run, not something this task can settle from here.
- **Blink is only tested bundled with shape** (DECSCUSR's own parameter sets both together) - there is no separate "blink only" escape sequence to test in isolation; noted, not a gap this task can close further.
- **This fix's placement (`Terminal`-owned, single call per tick) depends on `pane.rs`'s own current single-caller architecture** - if a later change ever let more than one caller call `changes_since` for the same pane in the same tick with different `seen` values, this field's invariant would need re-checking; worth a one-line comment at the field's own declaration saying so, for whoever touches `run_tick` next (TASK-1.146 is active nearby in `crates/server/src/runtime/*`/`listen.rs`, but not in `pane.rs` itself - no overlap today, named anyway since it is the kind of file that invariant depends on).

No collisions with TASK-1.127 (crates/app/src/{terminal_element.rs,main.rs}, untouched here), TASK-1.146 (crates/server/src/runtime/*, listen.rs, untouched here), TASK-1.137 (test-module bare joins/recvs, including crates/server/src/{outbound.rs,pane.rs} - this task adds no server tests at all), or TASK-1.51 (connection.rs, dispatch/mod.rs, untouched here).
<!-- SECTION:PLAN:END -->
