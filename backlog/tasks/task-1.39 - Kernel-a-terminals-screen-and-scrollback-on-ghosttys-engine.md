---
id: TASK-1.39
title: >-
  Kernel: a terminal on ghostty's engine: its screen, its cells and what it
  reports
status: Done
assignee:
  - '@bruk.w1'
created_date: '2026-11-15T01:32:00Z'
updated_date: '2026-11-15T15:20:00Z'
labels:
  - size-4
  - review-passed
milestone: m-0
dependencies:
  - TASK-1.34
  - TASK-1.35
references:
  - task-1.39-kernel-a-terminal-s-screen-and
  - https://github.com/bruk-io/steg/pull/27
parent_task_id: TASK-1
type: task
project: bruk-io-steg
asdlc_accepted_from: DRAFT-9
asdlc_branch: task-1.39-kernel-a-terminal-s-screen-and
asdlc_plan_approved_by: bruk
asdlc_plan_approved_hash: 'b1c858e146a2'
asdlc_plan_approved_size: size-4
asdlc_review_kind: same-machine
asdlc_review_findings: 'important=0,nit=4'
asdlc_review_policies: ["review"]
asdlc_plan_matched: yes
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
Cut down to this task's part (the team lead and engineer approved the split
and R116 for links; scrollback identity and rows-changed-since-a-point are
now their own task - my design for that is moved to this task's notes, not
built here). No open design question remains in what follows.

Grounding: read `ghostty-vt` 0.8.0's actual Rust source in the cargo
registry (`terminal.rs`, `render.rs`, `screen.rs`, `style.rs`, `osc.rs`), not
decision-3's summary of it.

## 1. Where the terminal sits relative to a pane

Not on `Pane`, and not inside `Tree`. `Pane` derives `Clone, PartialEq, Eq`;
`ghostty_vt::Terminal` is none of those, and every `ghostty_vt` handle
(`Terminal`, `RenderState`, its iterators) is `!Send + !Sync` by design,
enforced by the compiler (decision-3 section 7) - putting it on `Pane`
breaks those derives and makes `Tree` itself `!Send`, which the server
would inherit everywhere `Tree` is touched. So: this task builds a
free-standing `steg_kernel::Terminal` type (new module
`crates/kernel/src/terminal.rs`), tested on its own with no `Pane` and no
`Tree` in sight, `#![forbid(unsafe_code)]` holding for it like the rest of
the crate. Where it is *stored* relative to a pane - a side table keyed by
`PaneId`, owned by the thread that creates it and never moves it - is
TASK-1.40's problem: TASK-1.40 ("run a pane's program, and know when it has
ended") is where a pane's program and its owning thread first exist, and the
registry's lifecycle belongs with that thread. This task leaves `Pane`
untouched; nothing in the tree, layout or targeting assumes a pane is a
terminal (R12), and this design doesn't touch the places that could
violate it.

## 2. Reading the screen out, the cursor, and the cell model (AC #1, #3)

New sdk module, `crates/sdk/src/screen.rs` (R105's named exception: "the
screen is the exception: what the engine holds and what travels are
different shapes"). No new crate dependency anywhere: `boundaries.toml`'s
allow list for `steg-sdk` is empty today and stays empty - these are plain
`Debug, Clone, PartialEq, Eq` structs/enums, no serde. Serialising them onto
the wire is the encoding task's problem (TASK-1.43 or later), and that task
is the one that widens `boundaries.toml` to add serde to the sdk's allow
list; this task adds nothing to any `Cargo.toml`.

Shapes, each with the engine API behind it:

- `Rgb { r, g, b }` - resolved colour. The server resolves the palette so a
  client never needs one (decision-2 section 3):
  `CellIteration::fg_color()`/`bg_color()` already return resolved
  `RgbColor`, never a palette index.
- `Underline` enum: `None, Single, Double, Curly, Dotted, Dashed` - a
  straight mirror of `ghostty_vt::style::Underline`. **Exposed directly**
  via `CellIteration::style()`, which also gives `underline_color`.
- `Wide` enum: `Narrow, Wide, SpacerTail, SpacerHead` - mirrors
  `ghostty_vt::screen::CellWide`, read via `CellIteration::raw_cell()?.wide()`.
  **Exposed directly.** The run builder (below) emits a wide cell's text
  once, marks it `Wide`, and skips the `SpacerTail` cell that follows (never
  rendered, per the engine's own doc comment), so a client knows to draw two
  columns without seeing an empty one.
- `CellStyle { fg: Option<Rgb>, bg: Option<Rgb>, bold, italic, faint, blink,
  inverse, invisible, strikethrough, overline: bool, underline: Underline,
  underline_colour: Option<Rgb>, wide: Wide, link: bool }`. `link` is
  presence only (R116: "A cell says whether it is part of a link... A
  link's address is not carried with the screen"), from
  `CellIteration::raw_cell()?.has_hyperlink()` on the render-state path
  used for drawing. R116 also settles that two adjacent links are not told
  apart here - "not told apart until one is asked about" - so no link id is
  carried; section 2 below is that asking.
- `CursorShape` enum: `Bar, Block, Underline, BlockHollow` - mirrors
  `ghostty_vt::render::CursorVisualStyle`. **Exposed directly.**
- `Cursor { x: u16, y: u16, visible: bool, blinking: bool, shape:
  CursorShape }` - from `RenderState::cursor()`, which returns all of this
  in one call.
- `Run { text: String, style: CellStyle }`; `Row = Vec<Run>` - built by
  walking `CellIterator` left to right, starting a new `Run` whenever the
  resolved `CellStyle` changes from the previous cell (decision-2 section
  3's `{"text","fg","bg","flags"}`, generalised to the richer style above).
  Text comes from `CellIteration::graphemes_utf8`.

`steg_kernel::Terminal::screen(&mut self) -> (Vec<Row>, Cursor)` (or a
small named struct wrapping both - a build-time naming choice, not a design
one): creates a `RenderState`, calls `update(&self.inner)`, walks
`RowIterator`/`CellIterator` into `Vec<Row>` via the run builder above, and
reads `Cursor` from the same snapshot. This is AC #1 and AC #3 in one
method; this task has one reader, so there is no dirty-tracking or
multi-attachment question to settle here (that belongs to the scrollback
task's design, moved to notes).

### The link's address, asked by cell (AC #4, R116)

`Terminal::link_at(&self, x: u16, y: u32) -> Result<Option<String>, Error>`.
R116: "it is asked for, by cell, when a person acts on the link" - a click,
not a draw, so this is deliberately off the render-state path used by
`screen()`. Implementation: `self.inner.grid_ref(Point::Viewport(PointCoordinate
{ x, y }))?.hyperlink_uri(buf)` - the engine's own authors say this path
(`GridRef`, reached from `Terminal::grid_ref`) "isn't meant to be used as
the core of render loop," which is exactly why it is not used in
`screen()` and is fine used once per click. `Viewport` space matches "where
a person clicked on screen right now," including when scrolled.
`hyperlink_uri` returns the byte length written, `Ok(0)` when the cell has
no hyperlink (-> `Ok(None)`), or `Err(Error::OutOfSpace { required })` for a
too-small buffer - handled the same grow-and-retry way
`Terminal::continuation()` already does in the engine crate itself: try a
small stack buffer first, retry once with a `Vec<u8>` of `required` bytes
on that error.

## 3. The working directory, the title, and the alternate screen (AC #2)

- `Terminal::cwd(&self) -> Option<String>` - from the last
  `Event::PwdChanged`, drained after every `vt_write` (see below), which
  "carries the raw value the shell emitted (a `file://` URI for OSC 7)" per
  the crate's own doc comment - this task parses that URI into a path.
  (`Terminal::pwd()` exists too, but its string is "valid until the next
  call to `vt_write`," so it cannot be the stored value across calls; the
  event is copied out instead, same reasoning as title below.)
- `Terminal::title(&self) -> String` - from the last `Event::TitleChanged`,
  copied out for the same lifetime reason.
- `Terminal::alternate_screen(&self) -> bool` - `active_screen() ==
  Screen::Alternate`. **Exposed directly**, no event needed.
- `Terminal::write(&mut self, bytes: &[u8])` calls `vt_write` and *must*
  drain `events()` into the terminal's own state every time it is called -
  nothing drains them otherwise, and they accumulate unboundedly in
  `ghostty_vt`'s own queue if not drained after every write. Named here so
  it is not discovered in review.

## 4. Resizing (AC #5, and for TASK-1.42)

`Terminal::resize(&mut self, cols: u16, rows: u16) -> Result<(), Error>`
forwards to `ghostty_vt::Terminal::resize(cols, rows, 0, 0)`. The engine
call also takes `cell_width_px`/`cell_height_px`, which feed CSI 14/16/18 t
answers and mode-2048 in-band size reports while `set_size_reports` (on by
default) is on - pixel geometry the kernel has no business holding (R102,
R105: pixels are the app's). Passing zeros means those two query answers
will be wrong until TASK-1.42 plumbs a real pixel size through; that is
this task's known, named gap for TASK-1.42 to close, not fixed here.

## 5. What crosses the API (sdk) vs. stays in the kernel (AC #6)

Crosses (new `crates/sdk/src/screen.rs`, section 2 above): `Rgb`,
`Underline`, `Wide`, `CellStyle`, `Run`, `Row`, `CursorShape`, `Cursor` -
every shape a client eventually draws from, per R105's screen exception.

Stays in the kernel (`crates/kernel/src/terminal.rs`): the `Terminal` type
itself (wraps `ghostty_vt::Terminal`, steg's own engine choice, not a shape
a client sees); the run-building walk over `RowIterator`/`CellIterator`;
`link_at`'s grid-ref lookup; `cwd`/`title` event draining; resize. None of
this is a shape that crosses the API - it is the behaviour that produces
one, which doc-16's table puts in the kernel.

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

- AC #1: plain text produces one `Run` in `screen()`'s rows; an SGR colour
  change mid-line splits it into two `Run`s with the right `fg`; moving the
  cursor (e.g. `CSI H` then more text) changes the returned `Cursor.x/y`.
- AC #2: OSC 7 (`\x1b]7;file://...\x07`) sets `cwd()`; OSC 2
  (`\x1b]2;...\x07`) sets `title()`; `CSI ? 1049 h` / `l` toggles
  `alternate_screen()`.
- AC #3: a CJK character's cell is `Wide` followed by a skipped
  `SpacerTail`; `CSI 4:3 m` plus `CSI 58:2::R:G:Bm` round-trips as
  `Underline::Curly` with its colour; `CSI 3 SP q` (DECSCUSR) sets
  `Cursor.shape == CursorShape::Underline`; an OSC 8 start/text/end sequence
  sets `link: true` on the covered run and `false` outside it.
- AC #4: after that same OSC 8 sequence, `link_at` at a cell inside the
  linked text returns `Ok(Some(uri))` with the exact URI; at a cell outside
  it, or before any link was written, returns `Ok(None)`.
- AC #5: resizing changes `cols()`/`rows()`, and text fed before and after
  a resize reads back correctly at the new width (reflow included).
- AC #6: this is structural, checked by where the code lives and by
  `just check-boundaries`/`just check-packages` passing with no new
  dependency in either crate's `Cargo.toml`, rather than by a unit test.

`just check-packages` (part of `just check`) builds and tests `steg-kernel`
and `steg-sdk` alone. `#![forbid(unsafe_code)]` stays in force; the new
kernel module contains no `unsafe` of its own.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
## Starting design for the scrollback task (moved out of this task's plan, 2026-11-15)

This is the design from this task's original plan for scrollback identity
and rows-changed-since-a-point, before the task was split (engineer-approved
split: this task is now the terminal type, screen, cell model and resize
only). Kept here so it is not lost; the scrollback task should read it as a
starting point, not a settled answer - both sections name an open risk that
is still unresolved.

### Scrollback identity

**Correction to decision-3**: it says ghostty's `max_scrollback` is a byte
estimate and "steg converts lines to bytes." That was true of the spike's
binding; `ghostty-vt` 0.8 (the crate actually chosen) also has
`Terminal::set_scrollback_max_lines(Option<usize>)`, documented as working
alongside the byte limit with "the first limit reached wins." `Terminal::new`
can call this with the configured limit (10,000 by default, R15) directly -
no byte-per-row conversion needed. Caveat that still holds: pruning is at
page granularity, so the retained count runs somewhat higher than the
limit; `Terminal::scrollback_rows()` reports the real count rather than
promising exactly the limit.

Absolute identity: `ghostty_vt::render::RowId` looks like a match for "an
absolute identity that survives rows scrolling off" but is not one - it is
explicitly opaque (`[u64; 2]`, "treat this value as opaque"), unordered, and
only populated for rows the render state actually visited (the viewport
plus a requested overscan margin - not all 10,000 scrollback rows). It
cannot answer "give me rows 500 through 600" or distinguish "here are your
rows" from "those rows are gone," which is what decision-2 section 3 needs
for paging scrollback by `rows_ever`. Its real use is a per-frame render
cache key for whoever draws - the app's problem (TASK-1.56), not the
kernel's.

Instead: `Terminal` would keep its own monotonic counter,
`rows_scrolled_off: u64`, incremented by exactly the drop in
`scrollback_rows() + viewport_rows()` (i.e. `total_rows()`) observed across
a `vt_write` call that shrank it - which only happens when pruning removes
rows, never when the active area merely grows into unused scrollback space.
A row's absolute id would be `rows_scrolled_off + its offset from the
oldest row currently retained`. The counter only grows, is never reset, and
a pruned row's id is never reissued to a different row's content.

**Open risk, unresolved**: a primary-screen resize can reflow and renumber
scrollback (the alternate screen does not). Whether reflow can move rows in
a way that breaks this counter's monotonicity is untested; TASK-1.39 named
it for TASK-1.42 too, since resize is where reflow happens.

### Rows changed since a given point

The render state's own dirty tracking (`Snapshot::dirty()`,
`RowIteration::next_dirty()`, `Snapshot::clean()`) is per-`RenderState`, not
per-client: `clean()` erases it for everyone holding that state, and
`update()` never unsets it on its own. With more than one attachment
reading at different points, the engine's dirty bits cannot be "the given
point" by themselves.

A single-reader version (no attachments yet) would need only: a
kernel-owned `seq: u64` on `Terminal`, one `RenderState` per `Terminal`, and
a `changes_since(&mut self) -> (Vec<(row_index, Row)>, Cursor, new_seq)`
-shaped method that reads the dirty rows via `next_dirty()`, builds `Row`s
only for those, reads the cursor unconditionally (cheap per `render.rs`'s
own measurement), bumps `seq`, and calls `clean()`. "The given point" would
be the caller holding the last `seq` it saw and asking again; a caller
behind by more than the dirty state can express would get everything
(`Dirty::Full`) rather than a partial answer.

**Open risk, unresolved**: once more than one caller reads at different
points (TASK-1.46/1.47), dirty flags cannot be shared read-and-cleared by
all of them. The two live options are a `RenderState` per attachment, or
one retained "last published screen" per `Terminal` that each attachment
diffs a fresh `update()` against in software. Not decided; whichever task
builds multi-attachment should settle it, informed by whichever this
scrollback task's single-reader method ends up implying.

Plan for the narrowed task checked against doc-9 (R14, R16, R92, R116), doc-16 (R105) and decision-2 by @bruk.o: ok. A free-standing kernel Terminal over the engine, not stored on a pane; the screen read out as rows of styled runs with the cursor; the cell model with wide characters, underline style and colour, cursor shape and link presence; link_at for a cell's address; working directory and title copied out of the engine's events, which are drained after every write; resize. Drawing shapes are in the sdk with no new dependency. Named for later tasks: the engine's handles cannot move between threads, and pixel sizes are passed as zero until the clients-and-size task.

Check passed: `just check-packages` exited 0 at 2026-11-15T14:43:31+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.39 on task-1.39-kernel-a-terminal-s-screen-and at edbe7af2c6e6:

```text
ok      build (steg-sdk) (0.3s)
ok      test (steg-sdk) (0.5s)
ok      build (steg-kernel) (83.0s)
ok      test (steg-kernel) (0.8s)
ok      build (steg-server) (0.2s)
ok      test (steg-server) (0.3s)
ok      build (steg) (0.2s)
ok      test (steg) (0.3s)
ok      build (steg-app) (18.6s)
ok      test (steg-app) (0.8s)
```

Check FAILED: `just check-packages` exited 101 at 2026-11-15T14:49:02+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.39 on task-1.39-kernel-a-terminal-s-screen-and at edbe7af2c6e6 with uncommitted changes, last 40 of 67 lines:

```text
test terminal::tests::osc_7_sets_the_reported_cwd ... FAILED
test tree::tests::new_session_gives_the_first_five_sessions_distinct_colours_in_order ... ok
test tree::tests::new_session_rejects_an_invalid_label_and_mints_no_id ... ok
test tree::tests::new_session_sets_the_label_given_to_it ... ok
test tree::tests::new_tab_adds_a_labelled_tab_with_one_pane_already_in_it ... ok
test tree::tests::new_tab_rejects_an_invalid_label_and_mints_no_id ... ok
test tree::tests::panes_are_in_layout_order_after_a_move ... ok
test tree::tests::session_mut_lets_a_caller_change_a_sessions_label_and_colour ... ok
test tree::tests::set_tab_size_smaller_always_succeeds_and_closes_nothing ... ok
test tree::tests::split_pane_adds_a_pane_to_an_existing_tab ... ok
test tree::tests::swap_panes_is_refused_when_the_two_panes_are_in_different_tabs ... ok
test tree::tests::the_sixth_concurrent_session_repeats_the_first_colour ... ok

failures:

---- terminal::tests::an_sgr_colour_change_splits_the_row_into_two_runs_with_the_right_fg stdout ----

thread 'terminal::tests::an_sgr_colour_change_splits_the_row_into_two_runs_with_the_right_fg' (5867844) panicked at crates/kernel/src/terminal.rs:423:9:
assertion `left == right` failed
  left: 3
 right: 2
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace

---- terminal::tests::osc_7_sets_the_reported_cwd stdout ----

thread 'terminal::tests::osc_7_sets_the_reported_cwd' (5867851) panicked at crates/kernel/src/terminal.rs:456:9:
assertion `left == right` failed
  left: Some("tmp/project")
 right: Some("/tmp/project")


failures:
    terminal::tests::an_sgr_colour_change_splits_the_row_into_two_runs_with_the_right_fg
    terminal::tests::osc_7_sets_the_reported_cwd

test result: FAILED. 129 passed; 2 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

error: test failed, to rerun pass `-p steg-kernel --lib`
full output: .quiet/test-steg-kernel.log
error: recipe `check-packages` failed with exit code 101
```

Check passed: `just check-packages` exited 0 at 2026-11-15T14:50:02+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.39 on task-1.39-kernel-a-terminal-s-screen-and at edbe7af2c6e6 with uncommitted changes:

```text
ok      build (steg-sdk) (0.1s)
ok      test (steg-sdk) (0.1s)
ok      build (steg-kernel) (0.3s)
ok      test (steg-kernel) (0.6s)
ok      build (steg-server) (0.1s)
ok      test (steg-server) (0.3s)
ok      build (steg) (0.2s)
ok      test (steg) (0.3s)
ok      build (steg-app) (0.5s)
ok      test (steg-app) (0.7s)
```

Check FAILED: `just check` exited 1 at 2026-11-15T14:50:08+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.39 on task-1.39-kernel-a-terminal-s-screen-and at edbe7af2c6e6 with uncommitted changes:

```text
FAILED  format check (exit 1, 0.1s)
Diff in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.39/crates/kernel/src/terminal.rs:127:
             Ok(len) => Ok(Some(uri_from_bytes(&buf[..len]))),
             Err(ghostty_vt::Error::OutOfSpace { required }) => {
                 let mut grown = vec![0u8; required];
[31m-                let len = grid_ref
(B[m[31m-                    .hyperlink_uri(&mut grown)
(B[m[31m-                    .map_err(Error::Engine)?;
(B[m[32m+                let len = grid_ref.hyperlink_uri(&mut grown).map_err(Error::Engine)?;
(B[m                 Ok(Some(uri_from_bytes(&grown[..len])))
             }
             Err(other) => Err(Error::Engine(other)),
Diff in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.39/crates/kernel/src/terminal.rs:352:
     };
     // `after_scheme` is `<host>/<path>`; `split_once` consumes the
     // separating `/`, so it is put back in front of the decoded path.
[31m-    let path = after_scheme.split_once('/').map_or("", |(_host, path)| path);
(B[m[32m+    let path = after_scheme
(B[m[32m+        .split_once('/')
(B[m[32m+        .map_or("", |(_host, path)| path);
(B[m     format!("/{}", percent_decode(path))
 }
 
full output: .quiet/format-check.log
error: recipe `check-fmt` failed on line 117 with exit code 1
```

Check FAILED: `just check` exited 101 at 2026-11-15T14:51:04+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.39 on task-1.39-kernel-a-terminal-s-screen-and at edbe7af2c6e6 with uncommitted changes, last 40 of 65 lines:

```text
...   |
386 | |         }
    | |_________^
    |
    = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.98.0/index.html#collapsible_if
    = note: `-D clippy::collapsible-if` implied by `-D warnings`
    = help: to override `-D warnings` add `#[allow(clippy::collapsible_if)]`
help: collapse nested if block
    |
377 ~         if bytes[i] == b'%'
378 ~             && let (Some(high), Some(low)) = (
379 |                 bytes.get(i + 1).copied().and_then(hex_digit),
...
384 |                 continue;
385 ~             }
    |

error: this lint expectation is unfulfilled
   --> crates/kernel/src/terminal.rs:294:5
    |
294 |     clippy::wildcard_enum_match_arm,
    |     ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
    |
    = note: ghostty_vt::style::Underline is #[non_exhaustive]; every current variant is covered, but the compiler still requires a wildcard arm for a foreign non_exhaustive enum
    = note: `-D unfulfilled-lint-expectations` implied by `-D warnings`
    = help: to override `-D warnings` add `#[allow(unfulfilled_lint_expectations)]`

error: this lint expectation is unfulfilled
   --> crates/kernel/src/terminal.rs:312:5
    |
312 |     clippy::wildcard_enum_match_arm,
    |     ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
    |
    = note: ghostty_vt::render::CursorVisualStyle is #[non_exhaustive]; every current variant is covered, but the compiler still requires a wildcard arm for a foreign non_exhaustive enum

error: could not compile `steg-kernel` (lib) due to 4 previous errors
warning: build failed, waiting for other jobs to finish...
error: could not compile `steg-kernel` (lib test) due to 4 previous errors
full output: .quiet/lint.log
error: recipe `lint` failed with exit code 101
```

Check passed: `just check` exited 0 at 2026-11-15T14:52:35+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.39 on task-1.39-kernel-a-terminal-s-screen-and at edbe7af2c6e6 with uncommitted changes:

```text
ok      format check (0.1s)
ok      lint (0.6s)
ok      boundaries (0.1s)
ok      licences (0.6s)
ok      tests (-p steg -p steg-app -p steg-kernel -p steg-sdk -p steg-server) (41.2s)
ok      build (steg-sdk) (0.1s)
ok      test (steg-sdk) (0.1s)
ok      build (steg-kernel) (0.3s)
ok      test (steg-kernel) (0.6s)
ok      build (steg-server) (0.2s)
ok      test (steg-server) (0.3s)
ok      build (steg) (0.2s)
ok      test (steg) (0.3s)
ok      build (steg-app) (0.3s)
ok      test (steg-app) (0.2s)
ok      tool tests (1.3s)
ok      architecture (1.0s)
ok      recipe names (0.1s)
```

Check passed: `just check` exited 0 at 2026-11-15T14:53:10+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.39 on task-1.39-kernel-a-terminal-s-screen-and at edbe7af2c6e6 with uncommitted changes:

```text
ok      format check (0.1s)
ok      lint (0.5s)
ok      boundaries (0.1s)
ok      licences (0.6s)
ok      tests (-p steg -p steg-app -p steg-kernel -p steg-sdk -p steg-server) (1.3s)
ok      build (steg-sdk) (0.1s)
ok      test (steg-sdk) (0.1s)
ok      build (steg-kernel) (0.3s)
ok      test (steg-kernel) (0.6s)
ok      build (steg-server) (0.1s)
ok      test (steg-server) (0.3s)
ok      build (steg) (0.2s)
ok      test (steg) (0.3s)
ok      build (steg-app) (0.3s)
ok      test (steg-app) (0.2s)
ok      tool tests (0.9s)
ok      architecture (1.0s)
ok      recipe names (0.0s)
```

Built as planned, no departures. Pull request #27, CI green
(https://github.com/bruk-io/steg/actions/runs/43726886769).

New: `crates/kernel/src/terminal.rs` (steg_kernel::Terminal, free-standing,
not stored on Pane), `crates/sdk/src/screen.rs` (the shapes a client draws
from). Changed: `crates/kernel/src/{lib,error}.rs` (wire the module in, add
Error::Engine), `crates/sdk/src/lib.rs` (wire the module in). No new
dependency in either crate's Cargo.toml; `just check-boundaries` passes
unchanged.

Which test proves which clause, all in crates/kernel/src/terminal.rs's
`#[cfg(test)]` module, no doubles (bytes fed in, screen read back):

- AC #1 (screen as rows of attribute runs, with the cursor):
  `writing_plain_text_produces_one_run`,
  `an_sgr_colour_change_splits_the_row_into_two_runs_with_the_right_fg`,
  `the_cursor_position_is_reported`.
- AC #2 (cwd, title, alternate screen): `osc_7_sets_the_reported_cwd`,
  `osc_2_sets_the_reported_title`,
  `entering_and_leaving_the_alternate_screen_is_reported`.
- AC #3 (cell model: wide characters, underline style/colour, cursor
  shape, link presence per R116):
  `a_wide_character_is_marked_wide_and_its_spacer_is_skipped`,
  `curly_underline_with_its_own_colour_round_trips`,
  `decscusr_sets_the_reported_cursor_shape`,
  `a_hyperlink_marks_its_cells_and_nothing_else`.
- AC #4 (the link's address by cell, R116):
  `link_at_returns_the_address_of_a_linked_cell`,
  `link_at_returns_none_for_a_cell_with_no_link`.
- AC #5 (resize): `resizing_changes_the_reported_size_and_reflows_text`.
- AC #6 (sdk/kernel split, R105): structural - the new sdk shapes carry no
  engine type and the kernel's new module holds no shape a client draws
  from directly; checked by `just check-boundaries` and `just
  check-packages` (both recorded above), not by a unit test.

Two bugs the tests themselves caught and fixed before this was clean:
`parse_pwd` was dropping the leading `/` of a `file://` URI's path
(`split_once` consumes the separating slash), and the SGR-colour test's
own row was wider than its text, which found a real emulator behaviour
worth noting rather than a bug: a cell the cursor never visits keeps its
original default style regardless of the "current" SGR, so a row with
unwritten trailing cells reports more runs than the written text alone
would suggest - expected, not a defect, and the test was narrowed to the
exact text width instead.

All `just check` / `just check-packages` runs are recorded on the task via
asdlc record-check, including the two failures that led to the fixes
above.
<!-- SECTION:NOTES:END -->

## Comments

<!-- COMMENTS:BEGIN -->
author: @bruk
created: 2026-11-15T14:41:00Z

---

Plan approved by @bruk (b1c858e146a2, size-4): its worker may build it.

---

author: @bruk.r1
created: 2026-11-15T14:56:00Z

---

@bruk.r1 is reviewing TASK-1.39: a same-machine review: it waited 0m with no other engineer's reviewer, longer than review_fallback_after (0m), so a fresh reviewer on its author's machine takes it.

---

author: @bruk.r1
created: 2026-11-15T15:08:00Z

---

Review by @bruk.r1 (same-machine): clean. It stays In Review for a person to merge.

Findings: 0 Important, 4 nit. Cites: review. The diff matches the plan.

No Important findings. Checked clause by clause against doc-9 (R14, R16, R92, R116), doc-16 (R105, R106, R110) and decision-2/decision-3, with ghostty-vt 0.8.0's real source read from the cargo registry cache (terminal.rs, render.rs, screen.rs) to verify every engine-API claim in the plan: Event::PwdChanged/TitleChanged semantics, GridRef::hyperlink_uri's Ok(0)/OutOfSpace contract, RenderState/RowIterator/CellIterator's advance-then-read shape, CursorViewport fields, and Terminal::grid_ref's own "isn't meant to be used as the core of render loop" doc comment all match the plan's claims exactly. Diff is scoped exactly to the narrowed plan (crates/kernel/src/{terminal,error,lib}.rs, crates/sdk/src/{screen,lib}.rs) with no scrollback-identity or changes-since-a-point code from TASK-1.74 present. No new dependency in either Cargo.toml (confirmed via diff), boundaries.toml unchanged, sdk allow-list still empty, #![forbid(unsafe_code)] holds, no unwrap/panic outside #[cfg(test)], Error::Engine keeps the engine error as source() per rust.md. CI is green at the PR's head commit 4e93bde (gh pr checks 27), which is what the Evidence pass rests on; the task's own recorded `just check` runs are stamped against the tasks-board commit hash (not the code commit) by asdlc's recording mechanism, so the comparison that actually matters is CI-at-head, which passes.

Nits (4, at the cap):
- [Bugs, Nit] crates/kernel/src/terminal.rs:148 `resize()` forwards to `ghostty_vt::Terminal::resize`, which (per its doc comment and `cb_write_pty` in the engine's own source) can push an `Event::PtyWrite` onto the same queue `events()` drains, when the running program has enabled mode 2048. `resize()` does not drain afterward, unlike `write()`. Not a live bug today: `events()` is `Vec::drain(..)`, so the next `write()` clears the whole queue regardless of when the entries were pushed, and `write()`'s own match arm discards `PtyWrite` unconditionally (`Event::PtyWrite(_) => {}`) since there is no pty to hand it to yet (TASK-1.40). But the plan's own stated reason for draining ("nothing drains them otherwise... named here so it is not discovered in review") was written only against `write()`'s `vt_write` path and doesn't name this one, so a future resize-heavy caller with no intervening write (e.g., repeated attach-time resizes before any input) would hold queued events longer than intended.
- [Bugs, Nit] crates/kernel/src/terminal.rs:349-352 `parse_pwd`: a malformed OSC 7 value with a host but no following `/path` (`split_once('/')` returns `None`) silently returns `"/"`, reporting the root directory as the cwd rather than signalling the value was malformed. No panic, so it meets the brief's bar, but it is a silent wrong answer rather than `None`/an error.
- [Evidence, Nit] crates/kernel/src/terminal.rs:119-135 `link_at`'s grow-and-retry branch (`Err(Error::OutOfSpace { required })`, lines 128-131) has no test exercising it; every `link_at` test uses a URI well under the 256-byte stack buffer. The brief asked by name for a test using an address longer than the first buffer.
- [Evidence, Nit] crates/kernel/src/terminal.rs:560-573 `resizing_changes_the_reported_size_and_reflows_text` is the only resize test, and it changes both axes at once (10x2 -> 5x4: narrower and taller). Growing a dimension and shrinking one in isolation are both untested.

Checked and clean: the cell-model clauses (underline style/colour, cursor shape, link presence true-inside/false-outside R116), events drained after every write for the `vt_write` path with cwd/title copied out as owned values (not borrowed past the next write), the run builder's style-change/wide-character/spacer-skip logic (traced against inputs beyond the tests: wide character at end of row via `SpacerHead`, an all-default-style unwritten row, a style change with no following text - no bug found in any), the sdk/kernel split (no engine type in `crates/sdk/src/screen.rs`, no new dependency, structural per AC #6), and that `Pane`/`Tree` are untouched (diff touches no file under pane/tree/session/tab).

---

created: 2026-11-15T15:20:00Z

---

From @bruk.o: commits after the clean review. The review by @bruk.r1 passed head 4e93bde with four nits. The board takes no recorded check on a task whose review has passed, so the worker fixed them and pushed without asdlc record-check; the check at the new head b6e0c01 is pull request 27's CI run 43729109085 (success). The earlier review does not cover these commits. The orchestrator read every non-test change between the two heads; all are in crates/kernel/src/terminal.rs and fall in two places. First, draining: the engine's events are drained by one function that write and resize both call, since a resize can queue an event too. Second, the working directory: a value is now refused, leaving the previous directory in place, when it is a file URI with a host and no slash at all, has another scheme, is a relative path, is empty or has bad percent-encoding; a file URI whose path is empty after the slash is the root, and a plain absolute path is accepted. An intermediate push, c6c84ff, had refused the root directory and every plain path; the second came from the orchestrator's own wording and both were corrected before this head. Tests were added for each of those cases (one through a real OSC 1337 sequence), for a link address longer than the first buffer, and for growing and shrinking each axis on resize.

---
<!-- COMMENTS:END -->
