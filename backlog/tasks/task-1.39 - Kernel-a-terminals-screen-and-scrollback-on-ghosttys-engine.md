---
id: TASK-1.39
title: >-
  Kernel: a terminal on ghostty's engine: its screen, its cells and what it
  reports
status: In Progress
assignee:
  - '@bruk.w1'
created_date: '2026-11-15T01:32:00Z'
updated_date: '2026-11-15T14:15:00Z'
labels:
  - size-4
milestone: m-0
dependencies:
  - TASK-1.34
  - TASK-1.35
parent_task_id: TASK-1
type: task
project: bruk-io-steg
asdlc_accepted_from: DRAFT-9
asdlc_branch: task-1.39-kernel-a-terminal-s-screen-and
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
doc-9: R14, R16, R92, R116. decision-2 says what a snapshot carries. Split on 2026-11-15 by Bruk from the task that also covered scrollback identity and changed rows, which is now a task of its own: this part has no open design question and unblocks running a program, status, and clients and size. Its plan covers both parts and is to be cut down to this one.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Bytes fed in produce a screen that can be read out as rows of attribute runs, with the cursor
- [ ] #2 The terminal reports its working directory, its title and whether the alternate screen is on
- [ ] #3 The cell model carries wide characters, underline style and colour, the cursor's shape, and whether a cell is part of a link (R116)
- [ ] #4 The address of the link at a given cell can be asked for, and a cell with no link answers that there is none (R116)
- [ ] #5 The terminal can be resized, and text reads back correctly at the new size
- [ ] #6 The shapes a client draws from are defined in the sdk; the terminal itself and the engine stay in the kernel (doc-16 R105)
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
Too big for one reviewable change. Below is the full design (it answers all 8
of the brief's questions, since they interlock), but I recommend splitting
before building, at the seam marked **SPLIT** below: Part A (terminal type,
feeding bytes, full-screen read-out and cell model, reported facts, resize)
is independently testable and has no open design question; Part B
(scrollback identity, rows-changed-since-a-point) depends on Part A's
`Terminal` type existing and carries the two unsettled design choices (how
identity survives pruning, and single- vs multi-attachment dirty tracking).
If kept as one task, build and land in that order anyway.

Grounding: read `ghostty-vt` 0.8.0's actual Rust source in the cargo
registry (`terminal.rs`, `render.rs`, `screen.rs`, `style.rs`, `osc.rs`,
`snapshot.rs`), not decision-3's summary of it. Two corrections to the
decision record follow from that reading, called out inline.

## 1. Where the terminal sits relative to a pane

Not on `Pane`, and not inside `Tree`. `Pane` derives `Clone, PartialEq, Eq`;
`ghostty_vt::Terminal` is none of those and is `!Send + !Sync` by design
(decision-3 section 7). Putting it on `Pane` breaks those derives and makes
`Tree` itself `!Send`, which the server would inherit everywhere `Tree` is
touched. So: this task builds a free-standing `steg_kernel::Terminal` type
(new module `crates/kernel/src/terminal.rs`), tested on its own with no
`Pane` and no `Tree` in sight. Where it is *stored* relative to a pane - a
side table keyed by `PaneId`, owned by the thread that creates it - is
TASK-1.40's problem, not this one's: TASK-1.40 ("run a pane's program, and
know when it has ended") is where a pane's program and its owning thread
first exist, and the registry's lifecycle belongs with that thread, not with
a value this task can hand over before it does. This task leaves `Pane`
untouched. Nothing in the tree, layout or targeting ever assumes a pane is a
terminal (R12) - this design doesn't touch the places that could violate it.

## 2. Reading the screen out, and the cell model (AC #1, #5)

New sdk module, `crates/sdk/src/screen.rs` (R105's named exception: "the
screen is the exception: what the engine holds and what travels are
different shapes"). No new crate dependency: `boundaries.toml`'s allow list
for `steg-sdk` is empty today and stays empty - these are plain
`Debug, Clone, PartialEq, Eq` structs/enums, no serde. Serialising them is
the wire-encoding task's problem (TASK-1.43 or later) and that task is the
one that will need to widen `boundaries.toml` to add serde to the sdk's
allow list; this task adds nothing to any `Cargo.toml`.

Shapes, each with the engine API behind it:

- `Rgb { r, g, b }` - resolved colour. The server resolves the palette so a
  client never needs one (decision-2 section 3): `CellIteration::fg_color()`
  and `bg_color()` already return resolved `RgbColor`, never a palette
  index.
- `Underline` enum: `None, Single, Double, Curly, Dotted, Dashed` - a
  straight mirror of `ghostty_vt::style::Underline`. **Exposed directly.**
- `CellStyle { fg: Option<Rgb>, bg: Option<Rgb>, bold, italic, faint, blink,
  inverse, invisible, strikethrough, overline: bool, underline: Underline,
  underline_colour: Option<Rgb>, wide: Wide, link: bool }`.
  - Underline style and colour: `CellIteration::style()` returns
    `ghostty_vt::style::Style { underline, underline_color, .. }` directly.
    **Exposed directly.**
  - Wide characters: `Wide` mirrors `ghostty_vt::screen::CellWide` (`Narrow,
    Wide, SpacerTail, SpacerHead`), read via
    `CellIteration::raw_cell()?.wide()`. **Exposed directly** - a run
    builder (below) emits the wide cell's text once and marks it `Wide`,
    then skips the `SpacerTail` cell that follows (never rendered, per the
    engine's own doc comment), so a client knows to draw two columns without
    seeing an empty one.
  - Link identity: **not exposed the way the requirement is written.** The
    render-state path (`CellIteration`) only has
    `raw_cell()?.has_hyperlink()` - presence, no URI, no stable id to tell
    two different links on the same row apart. The actual URI lives on
    `GridRef::hyperlink_uri`, reached only through
    `Terminal::grid_ref(Point)`, which the crate's own doc comment says
    "isn't meant to be used as the core of render loop." This task ships
    `link: bool` (presence only, from the render-state path) and goes no
    further: resolving a link's URI, and telling adjacent cells with
    different links apart, is left undone. The alternative - steg's own
    OSC 8 pre-parser, matching `ghostty_vt::osc::OscCommandType::
    HYPERLINK_START`/`HYPERLINK_END` - is a second byte-level parse over
    every byte of output, which is exactly the cost decision-3 section 4
    said choosing ghostty avoided for OSC 7 and OSC 133. Naming it here
    rather than building it: it is real work, costed the same way decision-3
    costs R49's OSC 7501 parser, and a later task's call, not a quiet
    addition to this one.
- `Run { text: String, style: CellStyle }`; `Row = Vec<Run>` - built by
  walking `CellIterator` left to right and starting a new `Run` whenever the
  resolved `CellStyle` changes from the previous cell (decision-2 section 3's
  `{"text","fg","bg","flags"}`, generalised to the richer style above). Text
  comes from `CellIteration::graphemes_utf8`.
- `CursorShape` enum: `Bar, Block, Underline, BlockHollow` - mirrors
  `ghostty_vt::render::CursorVisualStyle`. **Exposed directly.**
- `Cursor { x: u16, y: u16, visible: bool, blinking: bool, shape:
  CursorShape }` - from `RenderState::cursor()`, which returns all of this
  in one call (`viewport`, `visible`, `blinking`, `visual_style`).

`steg_kernel::Terminal::screen(&mut self) -> Row-and-cursor shape above`:
creates (or reuses, see Part B) a `RenderState`, calls `update(&self.inner)`,
walks `RowIterator`/`CellIterator` into `Vec<Row>` via the run builder, reads
`Cursor` from the same snapshot. AC #1's "read out as rows of attribute
runs" and AC #5's fields are both this one method.

## 3. Scrollback identity (AC #2) **SPLIT above this line is Part A; below, Part B**

**Correction to decision-3**: it says ghostty's `max_scrollback` is a byte
estimate and "steg converts lines to bytes." That was true of the spike's
binding; `ghostty-vt` 0.8 (the crate actually chosen) also has
`Terminal::set_scrollback_max_lines(Option<usize>)`, documented as working
alongside the byte limit with "the first limit reached wins." `Terminal::new`
calls this with the configured limit (10,000 by default, R15) directly - no
byte-per-row conversion needed. Caveat that still holds: pruning is at page
granularity, so the retained count runs somewhat higher than the limit;
`Terminal::scrollback_rows()` reports the real count rather than promising
exactly the limit.

Absolute identity: `ghostty_vt::render::RowId` looks like a match for "an
absolute identity that survives rows scrolling off" but is not one - it is
explicitly opaque (`[u64; 2]`, "treat this value as opaque"), unordered, and
only populated for rows the render state actually visited (the viewport plus
a requested overscan margin - not all 10,000 scrollback rows). It cannot
answer "give me rows 500 through 600" or distinguish "here are your rows"
from "those rows are gone," which is what decision-2 section 3 needs for
paging scrollback by `rows_ever`. Its real use is a per-frame render cache
key for whoever draws - the app's problem (TASK-1.56), not the kernel's.

Instead: `Terminal` keeps its own monotonic counter, `rows_scrolled_off:
u64`, incremented by exactly the drop in `scrollback_rows() +
viewport_rows()` (i.e. `total_rows()`) observed across a `vt_write` call that
shrank it - which only happens when pruning removes rows, never when the
active area merely grows into unused scrollback space. A row's absolute id
is `rows_scrolled_off + its offset from the oldest row currently retained`.
The counter only grows, is never reset, and a pruned row's id is never
reissued to a different row's content. Reflow risk, named and left open: a
primary-screen resize can reflow and renumber scrollback (the alternate
screen does not); whether reflow can move rows in a way that breaks this
counter's monotonicity is untested here and is this section's one open risk
for whoever builds resize-driven reflow in earnest.

## 4. Rows changed since a given point (AC #4)

The render state's own dirty tracking (`Snapshot::dirty()`,
`RowIteration::next_dirty()`, `Snapshot::clean()`) is per-`RenderState`, not
per-client: `clean()` erases it for everyone holding that state, and
`update()` never unsets it on its own. With more than one attachment reading
at different points, the engine's dirty bits cannot be "the given point" by
themselves.

This task's scope (single reader, no attachments yet - those are
TASK-1.46/1.47) needs only: a kernel-owned `seq: u64` on `Terminal`,
one `RenderState` per `Terminal`, and a `changes_since(&mut self) ->
(Vec<(row_index, Row)>, Cursor, new_seq)`-shaped method that reads the dirty
rows via `next_dirty()`, builds `Row`s only for those, reads the cursor
unconditionally (cheap per `render.rs`'s own measurement), bumps `seq`, and
calls `clean()`. "The given point" is the caller holding the last `seq` it
saw and asking again; a caller behind by more than the dirty state can
express gets everything (`Dirty::Full`) rather than a partial answer.

Open design question, named for whoever builds multi-attachment
(TASK-1.46/1.47), not resolved here: once more than one caller reads at
different points, dirty flags cannot be shared read-and-cleared by all of
them. The two live options are a `RenderState` per attachment, or one
retained "last published screen" per `Terminal` that each attachment diffs a
fresh `update()` against in software. This task's single-reader method does
not foreclose either.

## 5. The working directory, the title, and the alternate screen (AC #3)

- `Terminal::cwd(&self) -> Option<String>` - from the last
  `Event::PwdChanged` drained after `vt_write` (see below), which "carries
  the raw value the shell emitted (a `file://` URI for OSC 7)" per the
  crate's own doc comment - this task parses that URI into a path. (`
  Terminal::pwd()` exists too, but its string is "valid until the next call
  to `vt_write`," so it cannot be the stored value across calls; the event
  is copied out instead, same reasoning as title below.)
- `Terminal::title(&self) -> String` - from the last `Event::TitleChanged`,
  copied out for the same lifetime reason (`Terminal::title()`'s string is
  equally short-lived).
- `Terminal::alternate_screen(&self) -> bool` - `active_screen() ==
  Screen::Alternate`. **Exposed directly**, no event needed.
- `Terminal::write(&mut self, bytes: &[u8])` calls `vt_write` and then
  *must* drain `events()` into the terminal's own state every time - nothing
  drains them otherwise, and they accumulate unboundedly in
  `ghostty_vt`'s own queue if not drained after every write. This is the one
  correctness point easy to miss silently, so it is named here rather than
  discovered in review.

## 6. What crosses the API (sdk) vs. stays in the kernel

Crosses (new `crates/sdk/src/screen.rs`, section 2 above): `Rgb`,
`Underline`, `Wide`, `CellStyle`, `Run`, `Row`, `CursorShape`, `Cursor` - all
shapes a client eventually draws from, per R105's screen exception.

Stays in the kernel (`crates/kernel/src/terminal.rs`): the `Terminal` type
itself (wraps `ghostty_vt::Terminal`, which is steg's own engine choice, not
a shape a client sees); the run-building walk over `RowIterator`/
`CellIterator`; the scrollback identity counter and its derivation; the
dirty-tracking-to-diff translation and `seq`; `cwd`/`title` event draining;
resize. None of this is a shape that crosses the API - it is the behaviour
that produces one, which doc-16's table puts in the kernel.

## 7. The `!Send + !Sync` handles, as a constraint for later tasks

Not solved here, stated for TASK-1.40 and the server tasks: every
`ghostty_vt` handle (`Terminal`, `RenderState`, the iterators) is `!Send +
!Sync`, enforced by the compiler, not by convention. `steg_kernel::Terminal`
inherits this with no `unsafe` and no opt-out - `crates/kernel/src/lib.rs`
already `#![forbid(unsafe_code)]`s the whole crate. A terminal must be
created on the thread that will own it for its whole life and never moved;
TASK-1.40's one-thread-per-pane model (decision-3 section 7) is the only
shape that fits, and the side-table registry section 1 defers to it is
where that ownership lives.

## 8. Resizing (for TASK-1.42)

`Terminal::resize(&mut self, cols: u16, rows: u16) -> Result<(), Error>`
forwards to `ghostty_vt::Terminal::resize(cols, rows, 0, 0)`. The engine call
also takes `cell_width_px`/`cell_height_px`, which feed CSI 14/16/18 t
answers and mode-2048 in-band size reports while `set_size_reports` (on by
default) is on - pixel geometry the kernel has no business holding (R102,
R105: pixels are the app's). Passing zeros here means those two query
answers will be wrong until TASK-1.42 plumbs a real pixel size through; that
is this task's known, named gap for TASK-1.42 to close, not fixed here. Also
named for TASK-1.42: a primary-screen resize reflows and can renumber
scrollback (section 3's open risk) - the alternate screen does not reflow.

## Errors

`crate::error::Error` gains one variant wrapping `ghostty_vt::Error`
(itself `Debug, Clone, Copy, PartialEq, Eq`, so it fits the existing derive
list with no further changes), with the engine error kept as `source()`
per rust.md ("the cause kept as its source... never by turning it into a
string"). Every fallible `Terminal` method documents its `# Errors`.

## Proof

Everything here is pure computation over the vendored engine - unit tests
beside the code in `#[cfg(test)]` modules, no `tests/` folder, no doubles,
exactly like the existing `ghostty_vt_round_trips_bytes_to_the_screen` proof
in `crates/kernel/src/lib.rs` (`.claude/rules/testing-rust.md`: "a terminal
behaviour is tested by feeding bytes in and reading the screen back"). One
test per acceptance-criterion clause, by the byte sequence that proves it:

- AC #1: plain text produces one `Run`; an SGR colour change mid-line splits
  it into two `Run`s with the right `fg`.
- AC #2: default limit (10,000) and a configured smaller limit are each
  honoured within the page-granularity slack `scrollback_rows()` reports;
  feeding enough lines to prune proves `rows_scrolled_off` only grows and a
  pruned row's absolute id is never reissued to new content.
- AC #3: OSC 7 (`\x1b]7;file://...\x07`) sets `cwd()`; OSC 2
  (`\x1b]2;...\x07`) sets `title()`; `CSI ? 1049 h` / `l` toggles
  `alternate_screen()`.
- AC #4: a second `changes_since` call with no writes between reports no
  changed rows; writing to one row and reading again reports only that row.
- AC #5: a CJK character's cell is `Wide` followed by a skipped
  `SpacerTail`; `CSI 4:3 m` plus `CSI 58:2::R:G:Bm` round-trips as
  `Underline::Curly` with its colour; an OSC 8 start/text/end sequence sets
  `link: true` on the covered run (presence only, per section 2); `CSI 3 SP
  q` (DECSCUSR) sets `Cursor.shape == CursorShape::Underline`.
- Resize (point 8, feeds TASK-1.42): resizing changes `cols()`/`rows()` and
  reflowed text reads back correctly at the new width.

`just check-packages` (part of `just check`) builds and tests `steg-kernel`
and `steg-sdk` alone - this task adds no dependency to either crate's
`Cargo.toml`, so `just check-boundaries` passes unchanged. `#![forbid(unsafe_code)]`
stays in force; the new kernel module contains no `unsafe` of its own.
<!-- SECTION:PLAN:END -->
