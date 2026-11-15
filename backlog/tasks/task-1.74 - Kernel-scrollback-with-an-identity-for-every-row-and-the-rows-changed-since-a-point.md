---
id: TASK-1.74
title: >-
  Kernel: scrollback with an identity for every row, and the rows changed since
  a point
status: In Review
assignee:
  - '@bruk.w4'
created_date: '2026-11-15T14:15:00Z'
updated_date: '2026-11-15T17:02:00Z'
labels:
  - size-3
  - review-passed
milestone: m-0
dependencies:
  - TASK-1.39
references:
  - task-1.74-kernel-scrollback-with-an-identity-for
  - https://github.com/bruk-io/steg/pull/31
parent_task_id: TASK-1
type: task
project: bruk-io-steg
asdlc_accepted_from: DRAFT-1
asdlc_branch: task-1.74-kernel-scrollback-with-an-identity-for
asdlc_plan_approved_by: bruk
asdlc_plan_approved_hash: '02577a1ace6f'
asdlc_plan_approved_size: size-3
asdlc_review_kind: same-machine
asdlc_review_findings: 'important=0,nit=5'
asdlc_review_policies: ["review"]
asdlc_plan_matched: yes
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
doc-9 R15; decision-2 on snapshots, updates and scrollback pages. Split from TASK-1.39 on 2026-11-15 by Bruk. It carries the two open design questions that task's plan named: whether a row's identity survives the engine reflowing text on a resize, and how change is tracked once more than one reader reads at different points. The earlier plan on TASK-1.39 has a design for both to start from.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Scrollback is kept to a configurable limit, 10,000 lines by default, and the real count kept is reported
- [ ] #2 Every row has an absolute identity that only grows and is never given to different content, shown by feeding enough lines to drop rows
- [ ] #3 What happens to row identity when a resize reflows scrollback is established by test and stated, not assumed
- [ ] #4 The rows changed since a given point can be asked for, so an update need not carry the whole screen; a reader too far behind gets everything
- [ ] #5 The plan says how this serves several readers at different points, or names the task that will, without closing off either option
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
Grounding: read `ghostty-vt` 0.8.0's real source in the cargo registry
(`terminal.rs`, `render.rs`, `screen.rs`, `style.rs`), and both open questions
were settled by experiment against the real engine (scratch tests run and
then deleted - not part of this plan's diff; the permanent tests in section
6 re-establish the same facts through `steg_kernel::Terminal`'s own public
methods, never the private `engine` field, per `.claude/rules/testing-rust.md`).
Starts from TASK-1.39's notes, corrected twice below where experiment
disagreed with that design.

## 1. Open question 1, settled: a column-count resize ends an identity epoch

Experiment, at `Terminal::new(10, 3, 50)`, no wrap:

- Write 40 short lines (`total_rows=41`). Narrow to 5 cols: `total_rows=81`
  (wrapping). Widen back to 20 cols: `total_rows=41` again, byte-for-byte
  the same content - reflow alone is reversible when nothing is pruned.
- Narrow only, no write in between, with enough prior content that the
  reflowed size crosses a page boundary (`before=(993, 990)`, after
  narrowing 40 cols -> 3 cols: `(10913, 10910)` against ~13,240 expected
  from pure 40/3 reflow with nothing lost) - a resize can itself prune, with
  no `write()` call anywhere near it.
- Growing only the row count (cols unchanged): `total_rows` is unchanged
  in every case tried (41 -> 41 -> 41; also 31 -> 31 -> 31 shrinking then
  growing rows alone) - only the scrollback/viewport split moves
  (`set_resize_pull_scrollback`, on by default, pulls rows already in
  scrollback into the active area with nothing lost or renumbered).

Three facts, not assumptions: reflow can inflate `total_rows` with nothing
lost, can itself prune with no intervening write, and a row-count-only
resize never reflows or prunes at all. There is no way to tell "pruned" from
"merely reflowed" apart from the delta in `total_rows()` across a
cols-changing resize - both can produce the same drop. TASK-1.39's notes said
the counter increments "by the drop... across a `vt_write` call that shrank
it [...] which only happens when pruning removes rows, never when the active
area merely grows into unused scrollback space" - true for `write()`, and
this plan keeps it there unchanged, but false for a cols-changing `resize()`:
sometimes the drop there is real pruning, sometimes it's the reverse side of
a reversible reflow, and the API gives no way to tell them apart (`track_grid_ref`
exists for single anchors precisely because the engine gives no bulk answer).

**The rule:** identity is scoped to an epoch. `Terminal` keeps
`rows_scrolled_off: u64` (incremented only across a `write()` call, exactly
as TASK-1.39's notes said, by `total_rows_before - total_rows_after` when
positive) and `epoch: u64`. A cols-changing `resize()` (compare
`cols()` before and after; a rows-only resize never bumps this) increments
`epoch` and resets the within-epoch counter to the post-resize `total_rows()`
- it does not try to compute how much was "really" pruned, because that
number cannot be recovered. A row's absolute id is `RowId { epoch, offset:
u64 }` where `offset = rows_scrolled_off + its distance from the oldest row
currently retained`. A caller holding an id from an earlier epoch is told
those rows are gone (decision-2's existing vocabulary: "here is your page" or
"those rows are gone," never silently mapped to different content) - a
cols-changing resize is treated the same way a client falling behind is
already treated (decision-2 section 6), not as a new case.

## 2. Open question 2, settled: one `RenderState` per `Terminal`, never one per reader

Experiment: two independent `RenderState`s against one `Engine`. Both get
`Dirty::Full` on their own first `update()` (confirmed independently of each
other). Write new content, update and `clean()` render state A only; then
call `update()` on render state B, which has *never itself observed this
write*: **B's dirty state comes back `Clean`.** `RenderState::update`'s own
doc comment says it "consumes terminal/screen dirty state," and this proves
what that means: the dirty signal lives on the terminal/screen, not on each
`RenderState`, and whichever `RenderState` calls `update()` first consumes it
for everyone. A `RenderState` per attachment (TASK-1.39's notes named this as
one of two live options) is not a slower alternative to weigh against
cost - it is broken on this engine, by test, because the second reader would
silently miss changes the first reader already consumed.

**The rule:** `Terminal` keeps exactly one `RenderState` (the field TASK-1.39
already added) for its whole life; nothing in this task or any later one
creates a second. One new method:

```rust
pub fn changes_since(&mut self, seen: Option<u64>) -> Result<Update, Error>
```

`Update` (new sdk type, `crates/sdk/src/screen.rs`) is an enum:

```rust
pub enum Update {
    /// `seen` matched this terminal's current sequence: nothing changed.
    Unchanged,
    /// The rows that changed since `seen`, by their position in the
    /// viewport (`ghostty_vt::render::RowIteration::viewport_y`, not an
    /// absolute `RowId` - only a live screen streams this way; scrollback
    /// is paged by `RowId` instead, section 3).
    Delta { rows: Vec<(u16, Row)>, cursor: Cursor, seq: u64 },
    /// `seen` was `None`, too old, or this terminal's sequence wrapped past
    /// what dirty tracking can express: the whole screen, as `screen()`
    /// already returns it, plus the sequence to hold from here.
    Full { screen: Screen, seq: u64 },
}
```

`seen` is exactly "the given point as a value a caller holds" (the brief
asks this be named): the `seq` a caller last received, held across a quiet
period and handed back next time. `changes_since(None)` and a stale `seen`
both take the `Full` arm - the same method proves decision-2 section 6's
back-pressure rule ("a client that falls behind gets one fresh
screen-sized snapshot") by construction, for free, because a brand-new
`RenderState`'s (and this task's single, shared one's) first read of any
unseen content already comes back `Dirty::Full` from the engine itself. No
separate "is this caller behind" bookkeeping is needed in this task for
that to be true. `seq` increments by one on every call that returns `Delta`
or `Full`, and `self.render.clean()` is called before returning either (the
shared single `RenderState` means `clean()` here is safe: there is nothing
else reading dirty state out of this terminal for this to steal from).

This settles AC #5 for *this* task's one caller. **What it does not build,
named so TASK-1.47 does not have to rediscover it:** several clients
attached to the same pane at once still each need their own `seen`/delivery
progress, since `changes_since` can only be called once per tick (calling it
twice between writes returns `Unchanged` the second time, having already
consumed the dirty state) - TASK-1.47's fan-out is: call `changes_since` once
per tick from whoever owns the pane's thread, and copy the one resulting
`Update` onto every attachment's own queue, each tracking its own `seq` and
its own back-pressure/resync policy on top. Nothing above closes that door;
`changes_since`'s signature (an explicit `seen`, not an implicit single
reader) is built so a caller *could* in principle call it more than once
between ticks without this task assuming otherwise, even though only one
real call site (the tick) is correct given the engine's actual behaviour.

## 3. Scrollback (and screen) paging by absolute identity (AC #2, #3, settle list)

New method, for a one-off request rather than a per-frame read (the engine's
own warning on this path: "isn't meant to be used as the core of render
loop... may require traversing the full scrollback page list"):

```rust
pub fn rows(&self, first: RowId, max_rows: u16) -> Result<Page, Error>
```

`RowId` (new sdk type, a plain `{ epoch: u64, offset: u64 }`, `Copy`, `Ord`
so a client can tell whether an id is ahead or behind another) and `Page`
(`{ rows: Vec<(RowId, Row)> }`, with an empty `rows` meaning "nothing at or
after `first` in its epoch, or `first`'s epoch is not the current one" -
"gone," not an error, per the settle list and decision-2's existing
contract).

Implementation: reject (`Ok(Page { rows: vec![] })`, not `Err`) outright if
`first.epoch != self.epoch` or `first.offset < self.rows_scrolled_off_floor()`
(the oldest id this epoch still has); otherwise read
`ghostty_vt::terminal::Point::Screen(PointCoordinate { x, y })` for
`y = first.offset - floor .. first.offset - floor + max_rows`, clamped to
`total_rows()`, one `grid_ref` per cell (confirmed: `Point::Screen` is
0-indexed from the oldest row currently retained, by writing ten known
numbered lines and reading `y=0..2` and the last row back - `y=0` was the
first line ever written, not the newest). A row is built cell by cell from
`GridRef::cell()` (raw cell, for `wide`/`has_hyperlink`), `GridRef::style()`
(`fg_color`/`bg_color`/`underline_color` as unresolved `StyleColor` here -
**unlike** `CellIteration::fg_color()`/`bg_color()` on the render-state path,
which resolve a palette index for free; this path must resolve `fg`/`bg`
the same way `cell_style()` already resolves `underline_colour`, against the
same palette `RenderState::colors()` gives, read once via `self.screen()`'s
own snapshot or a fresh one - the palette is terminal-global, not
per-row) and `GridRef::graphemes()`. The run-building (style-change
boundaries, skip `SpacerTail`) is the same logic `build_row` already has;
this task factors the style-and-wide-and-link resolution step out of
`cell_style`/`build_row` into one helper both paths call, so the two paths
cannot silently drift apart on what a cell's style resolves to - the
regression this would otherwise invite stays invisible until two reads of
the same content disagree.

**Cost, measured, not promised from decision-2's number (a different
engine's API; it does not transfer):** a 100-row, 80-column page (8,000
`grid_ref` calls plus `style`/`cell`/`graphemes` each) against a terminal
holding ~10,000 scrollback rows (R15's real default) took 205 µs at the
oldest end of scrollback and 266 µs at the newest, release profile, no
measurable difference between the two - this task's proof records the same
measurement as a recorded check, not a number typed from memory, and states
it as "a few hundred µs for a 100-row page at R15's default depth," not as a
promise that holds at ten or a hundred times that depth, which is untested.

## 4. Scrollback limit, and the real count kept (AC #1)

`Terminal::new` already calls `set_scrollback_max_lines(Some(scrollback_lines))`
(TASK-1.39). One correction, found by testing at R15's real default rather
than a small one: `ghostty_vt::terminal::Options::default()`'s
`max_scrollback` (the *byte* limit) defaults to 10,000,000 and `Terminal::new`
never touches it, so "the first limit reached wins" between two limits, one
of them never configured. At R15's real 10,000-line default with realistic
~70-byte lines this does not bind first (confirmed: pruning triggered at
line 10,024, by the line limit, with `scrollback_max_bytes` still at its
10 MB default and total content nowhere near it) - but a future caller of
`Terminal::new` with longer lines or a much larger configured line count
could hit the byte limit first silently. This task's `Terminal::new` gains
`engine.set_scrollback_max_bytes(None)` (no byte limit) alongside the
existing line-limit call, so the configured line count is the only limit in
force and R15's number means what it says. `scrollback_rows_kept(&self) ->
Result<usize, Error>` (wraps `scrollback_rows()`) is the real count AC #1
asks be reported - never the configured limit, which page-granularity
pruning makes only approximate.

**How approximate, measured rather than asserted "somewhat higher" as
TASK-1.39's notes said:** at a toy 50-line limit with short synthetic lines,
one prune collapsed `total_rows` from 4,359 to 4 - two orders of magnitude
below the limit, because one ~400 KiB page held thousands of short rows. At
R15's real 10,000-line default with realistic ~70-byte lines, the retained
count instead settles into a tight sawtooth between 9,434 and 10,024 (roughly
±6% of the limit) once warmed up. Both are real, measured behaviours of the
same mechanism (page-granularity pruning) at two different scales - the
plan states both rather than picking the reassuring one, and the proof
records the second (the realistic one) as a check, not the first.

## 5. What crosses the API (sdk) vs. stays in the kernel (doc-16 R105)

New in `crates/sdk/src/screen.rs` (no new dependency; the sdk's allow list
stays empty, same as TASK-1.39):

- `RowId { epoch: u64, offset: u64 }` - `Debug, Clone, Copy, PartialEq, Eq,
  PartialOrd, Ord, Hash`.
- `Update` (section 2) and `Page` (section 3).

Stays in `crates/kernel/src/terminal.rs`: `rows_scrolled_off`, `epoch` and
the logic that advances them; `changes_since`'s use of the one `RenderState`;
`rows`'s `grid_ref` walk; the shared style/wide/link-resolution helper. None
of this is a shape a client draws from directly - it is the behaviour that
produces one.

## 6. Proof - one test per acceptance-criterion clause, through `Terminal`'s own public methods only (no reads of the private `engine` field; `.claude/rules/testing-rust.md`)

- AC #1 (configurable limit, real count reported): `scrollback_rows_kept`
  grows as lines are fed in, stays at or near a small configured limit once
  many more lines than the limit have been written (asserting a bound, e.g.
  "retained <= limit * some small factor," not an exact number, since
  page-granularity pruning is not exact by design - stated in section 4);
  at R15's real 10,000-line default, `scrollback_max_bytes` being disabled
  is checked by feeding long enough lines that the byte limit *would* have
  bound first had it still been enabled, and confirming the line limit is
  still what governs.
- AC #2 (every id grows, never reused for different content): feed enough
  lines to prune at least once (within one epoch, no resize); record each
  row's id and content as it is written (via `rows`), feed more until that
  row is pruned, and assert `rows` for its old id now returns an empty
  `Page` (gone) while a newer id for different content is never equal to an
  id already handed out.
- AC #3 (what a resize does to identity, stated and tested): a cols-changing
  resize bumps `epoch` and every id from the prior epoch comes back as a
  gone `Page` afterward (feed lines, record an id, narrow the terminal,
  assert the old id is now gone); a rows-only resize (cols unchanged) does
  *not* bump `epoch`, and an id recorded before it still resolves to the
  same content after (feed lines, record an id, grow/shrink rows only,
  assert `rows` for that id is unchanged) - the discriminating case named by
  this plan's own review, not left to be found later.
- AC #4 (rows changed since a point, full screen to a reader too far
  behind): `changes_since(None)` returns `Full`; a write, then
  `changes_since(Some(that_seq))` returns `Delta` naming only the changed
  viewport rows; calling it twice with no write in between returns
  `Unchanged` the second time (the single shared `RenderState`'s dirty state
  was already consumed); `changes_since(Some(0))` after enough unrelated
  writes that `0` can no longer be serviced returns `Full` again.
- AC #5 (serves several readers, or names what will): the two-`RenderState`
  experiment in section 2, re-established as a permanent test through the
  public API this task ships - two callers, each holding their own `seen`,
  both calling `changes_since` on the *same* `Terminal` in turn, prove the
  second caller's call still returns whatever `Delta`/`Full` the dirty state
  has for it *as of that call*, and that a second call from the first
  caller with no intervening write returns `Unchanged` - demonstrating in
  one test why TASK-1.47 must own per-attachment delivery on top of a single
  tick-owned call, not reopen it.

`just check-packages` builds and tests `steg-kernel` and `steg-sdk` alone,
same as TASK-1.39. `#![forbid(unsafe_code)]` stays in force.

## Size and risk

Kept as one task (size-3 holds): the five pieces share one grounding
(the engine's real behaviour, now measured rather than assumed) and the
acceptance criteria name one coherent capability. If review wants a smaller
cut, section 3 (scrollback/screen paging by `RowId`, the only piece that
reads engine state through a second code path and the only one with an
unmeasured cost beyond ~10,000 rows) is the clean line to split off; this
plan does not pre-split it because nothing downstream is blocked on it being
separate, and splitting it now would cost a second review pass for no
decision.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Plan checked against doc-9 (R15), doc-14 and decision-2 by @bruk.o: ok. Both open questions settled by experiment on the real engine. A row's identity is scoped to an epoch: writing increments a counter as rows are dropped; a resize that changes the column count ends the epoch, because a reflow and a real drop cannot be told apart, and a caller holding ids from an earlier epoch is told those rows are gone. A resize of rows only changes nothing. A terminal keeps exactly one render state: a second one silently misses changes the first consumed, shown by test, so several readers are served by copying one update to each, which is TASK-1.47's.

Check passed: `just check-packages` exited 0 at 2026-11-15T16:29:11+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.74 on task-1.74-kernel-scrollback-with-an-identity-for at c8dc7cdd8d3d:

```text
ok      build (steg-sdk) (3.7s)
ok      test (steg-sdk) (2.4s)
ok      build (steg-kernel) (49.1s)
ok      test (steg-kernel) (1.2s)
ok      build (steg-server) (0.2s)
ok      test (steg-server) (0.4s)
ok      build (steg) (0.2s)
ok      test (steg) (0.5s)
ok      build (steg-app) (20.8s)
ok      test (steg-app) (0.8s)
```

Check passed: `just check` exited 0 at 2026-11-15T16:41:28+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.74 on task-1.74-kernel-scrollback-with-an-identity-for at c8dc7cdd8d3d with uncommitted changes:

```text
ok      format check (0.1s)
ok      lint (0.5s)
ok      boundaries (0.1s)
ok      licences (0.8s)
ok      tests (-p steg -p steg-app -p steg-kernel -p steg-sdk -p steg-server) (0.7s)
ok      build (steg-sdk) (0.1s)
ok      test (steg-sdk) (0.2s)
ok      build (steg-kernel) (0.1s)
ok      test (steg-kernel) (0.2s)
ok      build (steg-server) (0.1s)
ok      test (steg-server) (0.1s)
ok      build (steg) (0.1s)
ok      test (steg) (0.1s)
ok      build (steg-app) (0.3s)
ok      test (steg-app) (0.3s)
ok      tool tests (1.0s)
ok      architecture (1.0s)
ok      recipe names (0.1s)
```

Check passed: `just check` exited 0 at 2026-11-15T16:48:27+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.74 on task-1.74-kernel-scrollback-with-an-identity-for at c8dc7cdd8d3d with uncommitted changes:

```text
ok      format check (0.1s)
ok      lint (0.4s)
ok      boundaries (0.1s)
ok      licences (0.7s)
ok      tests (-p steg -p steg-app -p steg-kernel -p steg-sdk -p steg-server) (0.5s)
ok      build (steg-sdk) (0.1s)
ok      test (steg-sdk) (0.2s)
ok      build (steg-kernel) (30.8s)
ok      test (steg-kernel) (0.7s)
ok      build (steg-server) (0.1s)
ok      test (steg-server) (0.4s)
ok      build (steg) (0.1s)
ok      test (steg) (0.1s)
ok      build (steg-app) (0.4s)
ok      test (steg-app) (0.3s)
ok      tool tests (1.2s)
ok      architecture (1.0s)
ok      recipe names (0.1s)
```

Check passed: `just check-packages` exited 0 at 2026-11-15T16:48:38+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.74 on task-1.74-kernel-scrollback-with-an-identity-for at c8dc7cdd8d3d with uncommitted changes:

```text
ok      build (steg-sdk) (0.1s)
ok      test (steg-sdk) (0.2s)
ok      build (steg-kernel) (0.1s)
ok      test (steg-kernel) (0.2s)
ok      build (steg-server) (0.1s)
ok      test (steg-server) (0.1s)
ok      build (steg) (0.1s)
ok      test (steg) (0.1s)
ok      build (steg-app) (0.3s)
ok      test (steg-app) (0.2s)
```

Check passed: `just check` exited 0 at 2026-11-15T16:51:25+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.74 on task-1.74-kernel-scrollback-with-an-identity-for at a9c3437efe6e:

```text
ok      format check (0.1s)
ok      lint (0.4s)
ok      boundaries (0.1s)
ok      licences (0.6s)
ok      tests (-p steg -p steg-app -p steg-kernel -p steg-sdk -p steg-server) (0.5s)
ok      build (steg-sdk) (0.1s)
ok      test (steg-sdk) (0.2s)
ok      build (steg-kernel) (0.1s)
ok      test (steg-kernel) (0.3s)
ok      build (steg-server) (0.1s)
ok      test (steg-server) (0.3s)
ok      build (steg) (0.2s)
ok      test (steg) (0.1s)
ok      build (steg-app) (0.3s)
ok      test (steg-app) (0.3s)
ok      tool tests (1.2s)
ok      architecture (1.0s)
ok      recipe names (0.0s)
```

Built as planned, with three small departures (also in commit message and PR body):

1. The plan's method name `rows(first, max_rows)` collides with the existing `Terminal::rows(&self) -> Result<u16, Error>` (the terminal's height, from TASK-1.39). Built as `Terminal::page`.
2. The plan's stated palette source for the scrollback/screen paging path ("the palette RenderState::colors() gives, read once via self.screen()'s own snapshot or a fresh one") would have called self.render.update() as a side effect, silently consuming the shared RenderState's dirty-tracking state that changes_since depends on - a real correctness bug found during build, not during planning. Built instead with the engine-level Terminal::color_palette() getter, which touches no RenderState at all.
3. Added Terminal::oldest_row(&self) -> RowId, a small accessor not named in the plan: page() is unusable without some way for a caller to discover a valid starting id, and the plan did not name one.

No serde wire form was given to RowId/Update/Page, matching the existing screen-family shapes (Screen/Cursor/Run/Row/CellStyle), none of which have one yet - keeps the whole family's wire-form decision in one place for TASK-1.47 (attach), where they first travel. No new dependency, boundaries.toml unchanged.

Which test proves which clause, all in crates/kernel/src/terminal.rs's #[cfg(test)] module, through Terminal's own public methods only (no reads of the private engine field):

- AC #1 (configurable limit, real count reported): scrollback_rows_kept_grows_as_lines_are_fed_in, the_configured_limit_keeps_the_real_count_near_itself_once_warmed_up, the_line_limit_governs_even_past_the_engines_disabled_byte_limit (also proves the new set_scrollback_max_bytes(None) fix).
- AC #2 (every id grows, never reused): every_row_id_grows_and_is_never_reused_for_different_content.
- AC #3 (what a resize does to identity): a_column_changing_resize_retires_every_id_from_before_it, a_row_only_resize_never_retires_an_id.
- AC #4 (rows changed since a point; a caller too far behind gets everything): changes_since_gives_a_full_screen_to_a_caller_with_no_point, changes_since_reports_unchanged_when_nothing_happened_since_the_last_call, changes_since_reports_a_delta_of_only_the_changed_rows, changes_since_gives_a_stale_or_foreign_point_a_full_screen_not_a_partial_one.
- AC #5 (serves several readers, or names the task that will): two_callers_sharing_one_terminal_must_each_track_their_own_point - demonstrates the single-RenderState limitation directly through the public API, and the doc comments on changes_since name TASK-1.47's fan-out job precisely.

just check recorded clean four times on the task, the last after rebasing onto origin/main with no conflicts (TASK-1.40/1.42/1.70's parallel work had not yet touched crates/kernel/src/terminal.rs or crates/sdk/src/screen.rs at rebase time). CI green on PR #31 at its head (run 43737855693).
<!-- SECTION:NOTES:END -->

## Comments

<!-- COMMENTS:BEGIN -->
author: @bruk
created: 2026-11-15T16:26:00Z

---

Plan approved by @bruk (02577a1ace6f, size-3): its worker may build it.

---

author: @bruk.r3
created: 2026-11-15T16:54:00Z

---

@bruk.r3 is reviewing TASK-1.74: a same-machine review: it waited 0m with no other engineer's reviewer, longer than review_fallback_after (0m), so a fresh reviewer on its author's machine takes it.

---

author: @bruk.r3
created: 2026-11-15T17:02:00Z

---

Review by @bruk.r3 (same-machine): clean. It stays In Review for a person to merge.

Findings: 0 Important, 5 nit. Cites: review. The diff matches the plan.

No Important findings. Checked clause by clause against doc-9 R15, doc-14, doc-16 R105, and decision-2 sections 3 and 6, with ghostty-vt 0.8.0's real source read from the cargo registry (render.rs, terminal.rs, screen.rs, style.rs) to verify every engine-API claim: RenderState::update's doc comment confirming dirty state is global and consumed on first read (grounding the one-RenderState-per-Terminal rule); Dirty::{Clean,Partial,Full} and RowIteration::next_dirty's own doc comment matching the Delta/Full branching in changes_since; Options::default().max_scrollback = 10_000_000 bytes and set_scrollback_max_bytes's own doc comment ("the first limit reached wins... None removes the byte limit") confirming the AC #1 fix is real and necessary; Style.fg_color/bg_color/underline_color are unresolved StyleColor on both the render-state and point-lookup paths, confirming resolved_cell_style's shared resolution is a genuine fix for a latent inconsistency, not redundant; Palette(pub [RgbColor; 256]) confirming the plan's departure (engine-level color_palette() instead of RenderState::colors()) is correctly unpacked via .0; PointCoordinate.y documented 0-indexed, "may exceed page size for screen/history tags", consistent with page()'s absolute addressing; grid_ref's own doc comment ("isn't meant to be used as the core of render loop... may require traversing the full scrollback page list") matching the plan's stated cost caveat.

Row identity (AC #2) and the epoch rule (AC #3): offset/epoch arithmetic in page() (terminal.rs ~297-321) is correct by inspection at the boundaries named in review - the floor (first.offset < rows_scrolled_off), an id from a prior epoch (first.epoch != self.epoch), and one past the newest row (start >= total, verified off-by-one correct) - all answer gone, never substituted, matching Page's documented contract exactly. resize()'s unconditional epoch bump on any column-count change (regardless of whether a prune actually happened) is the plan's stated, deliberate answer to "pruning and reflow can't be told apart from outside" - confirmed against the engine's own set_scrollback_max_bytes doc comment, which admits page-granularity pruning is approximate by nature. changes_since (AC #4, #5) matches the plan's stated behaviour for every named case: no point, a stale point, a point from another caller, Unchanged on a second call with nothing written, and the single-shared-RenderState limitation demonstrated through the public API. Events are drained after every engine call that can queue one; none of this task's new methods (changes_since, scrollback_rows_kept, oldest_row, page) write to the engine, so none needed to. No new dependency in either Cargo.toml, crates/kernel/clippy.toml and boundaries.toml both unchanged, #![forbid(unsafe_code)] holds, no unwrap/panic outside #[cfg(test)], no tree/pane/session file touched. CI green at the PR's head a9c3437 (run 43737855693), matching the task's own last recorded `just check` at the same commit.

Two things worth recording rather than treating as findings, since the review brief asked specifically about them: (1) the grounding experiment "a resize can itself prune, with no write() call anywhere near it" (distinct from reflow alone inflating total_rows) is not independently re-tested - the two resize/identity tests only show id retirement, which the code applies uniformly to every column-changing resize whether or not real pruning occurred, so they can't distinguish the two cases. Given the plan's own reasoning that pruning and reflow are observationally identical from outside the engine, a test isolating "real pruning happened" from "pure reflow" is likely not constructible even in principle, not just hard via the public API. (2) The second experimental claim ("a second independent RenderState's own first update() comes back Clean, having had the dirty state already consumed by the first") has no literal test either, and none is possible through Terminal's public API by design: Terminal intentionally creates exactly one RenderState for its whole life and never exposes a way to create a second. two_callers_sharing_one_terminal_must_each_track_their_own_point is the closest available proxy - it demonstrates the consequence (a second caller displaces the first as "current," and the first's own next call gets Full, not a delta) through the public API, which is the most that can be shown without the forbidden private-engine-field access testing-rust.md rules out.

Nits (5, at the cap):
- [Evidence, Nit] crates/kernel/src/terminal.rs: page() - no test requests a page that spans the seam between scrollback and the live viewport (e.g. starting a few rows before the end of scrollback and reading into the active screen). decision-2 section 3 names exactly this seam disagreeing as the historical bug that motivated absolute-identity addressing in the first place; every page test here stays wholly within one region.
- [Evidence, Nit] crates/kernel/src/terminal.rs:297-307 - no test exercises the newest-row boundary: the current last row's id resolving successfully, and exactly one id past it coming back gone. The code looks correct by inspection (`start >= total`), but only the oldest/pruned-floor boundary is exercised by a committed test; review asked for both ends.
- [Evidence, Nit] crates/kernel/src/terminal.rs:resize() - no test resizes to the exact same size (same cols and rows) to confirm a no-op resize does not bump the epoch. Only a column-changing resize and rows-only resizes (in both directions) are tested.
- [Evidence, Nit] crates/kernel/src/terminal.rs:page() - no test requests max_rows larger than what scrollback_rows_kept() actually holds. end = start.saturating_add(usize::from(max_rows)).min(total) looks correct by inspection, but this named scenario is untested.
- [Evidence, Nit] See the two paragraphs above (grounding claims re-established by test): recorded as notes rather than counted again here.

Checked and clean beyond the above: the sdk/kernel split (RowId/Update/Page carry no engine type, no serde derives, matching the rest of the screen family per the worker's own disclosed departure), the three disclosed plan departures (Terminal::page rename, engine-level color_palette() instead of RenderState::colors() to avoid stealing changes_since's dirty state, and the new oldest_row() accessor) are all sound engineering corrections with real reasons, not scope creep, and the rebase onto origin/main (no conflicts with the four parallel sdk/lib.rs-touching tasks) is exactly the known, accepted conflict named in the review brief.

---
<!-- COMMENTS:END -->
