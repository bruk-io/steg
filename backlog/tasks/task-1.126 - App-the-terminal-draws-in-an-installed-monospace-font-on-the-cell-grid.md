---
id: TASK-1.126
title: 'App: the terminal draws in an installed monospace font, on the cell grid'
status: In Progress
assignee:
  - '@bruk.w8'
created_date: '2026-11-17T03:36:00Z'
updated_date: '2026-11-17T04:03:00Z'
labels:
  - size-2
milestone: m-4
dependencies:
  - TASK-1.117
parent_task_id: TASK-1
project: bruk-io-steg
asdlc_accepted_from: DRAFT-4
asdlc_branch: task-1.126-app-the-terminal-draws-in-an-installed
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found by the engineer on 2026-11-16, running the bare window with typing: the text is drawn in a proportional font and the cursor sits far to the right of the end of the text. Cause, read from the code: the design names the terminal font as JetBrains Mono, then SF Mono, then Menlo (foundations FONT_TERMINAL). The app asks GPUI for JetBrains Mono with the others as fallbacks (crates/app/src/metrics.rs and terminal_element.rs, terminal_font). JetBrains Mono is not installed on this machine, only Menlo is, and GPUI's fallbacks stand in for a missing glyph, not a missing family, so the text falls to the system's proportional font. The cell width is then measured from ten capital M in that font, so a cursor placed at column times cell width runs far ahead of the text. TASK-1.117's own screenshot showed a two-character prompt, where the drift is too small to see. Two faults: the font chosen, and that a run of text is laid out by the font's own advances and so can leave the grid whenever a glyph is not exactly one cell wide (a fallback glyph, an emoji, a wide character). doc-9, doc-10, doc-17 for the font's named value. Not in this task: cursor blink, the inverted character under a block cursor and the hollow cursor of an unfocused window, which belong to TASK-1.56.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The terminal's font is the first family in the design's list that is installed, found at start; the notes say how that is decided at the pinned GPUI version, and what happens when none is installed
- [ ] #2 A test proves the choice with a list whose first name is not installed, with a recorded failing control and passing run
- [ ] #3 Every cell's text is drawn at its own column, so a glyph narrower or wider than a cell cannot move what follows it; the cursor sits on the cell after the last character on a line of eighty characters; a test of the placement function has a recorded control
- [ ] #4 The pull request has an image of a long line with the cursor at its end, and of a listing in columns, taken with nothing personal on screen
- [ ] #5 The notes say whether steg should ship JetBrains Mono inside the app so the design's first choice is always there, with its licence, for the engineer to decide; nothing is bundled in this task
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
## Build base

This worktree is cut from main, which has no crates/app window yet (confirmed: crates/app/src/main.rs here is still the pre-TASK-1.117 stub). Before any of the work below, merge origin's task-1.117 branch (not main) into this task's branch - that is where metrics.rs, terminal_element.rs and the rest of the bare window live, at the merged-main head team-lead already verified for TASK-1.117. No other file outside crates/app is expected to differ once that merge lands; confirm with `git diff origin/main HEAD --stat` showing only app files, the same check used on TASK-1.117 itself.

## Root cause, confirmed by reading gpui 0.2.2's own source, not guessed

`gpui-0.2.2/src/text_system.rs`: `TextSystem::resolve_font` tries the requested `Font.family` via the private `font_id`, and on failure tries gpui's own hardcoded `fallback_font_stack` (".ZedMono", ".ZedSans", "Helvetica", "Segoe UI", "Ubuntu", "Adwaita Sans", "Cantarell", "Noto Sans", "DejaVu Sans", "Arial") - never the `Font.fallbacks` field `terminal_font()` sets. `Font.fallbacks` is consulted only during shaping, for a missing *glyph* in an otherwise-resolved family (confirmed: it never appears in `resolve_font`/`font_id`). On this machine, "JetBrains Mono" fails to resolve and gpui falls through to "Helvetica" (present on every macOS system) - a proportional font - which is exactly the observed bug. If literally none of `fallback_font_stack` resolves either, `resolve_font` panics; that boundary is gpui's own and out of this task's reach.

The public, read-only way to check whether a family is installed before asking for it: `TextSystem::all_font_names() -> Vec<String>` (also public via `WindowTextSystem`, which `#[deref]`s to `Arc<TextSystem>` - `window.text_system().all_font_names()` already works today in metrics.rs/terminal_element.rs). It returns every family the OS reports, plus gpui's own `fallback_font_stack` names, plus ".SystemUIFont".

Decision for AC#1's "what happens when none is installed": if none of `FONT_TERMINAL`'s names appear in `all_font_names()`, request `FONT_TERMINAL`'s own first entry anyway (today's behaviour) and let gpui's `resolve_font` fall through to its own internal stack as it already does - this task does not invent a second fallback chain on top of gpui's own.

## Files that change
- `crates/app/src/metrics.rs`: add a pure function resolving the first of a candidate list that appears in an `installed: &[String]` slice (falls back to the list's own first entry if none match - no window, no gpui types in its signature, testable directly). `CellMetrics::measure` calls `window.text_system().all_font_names()` once, resolves the family through it, and stores the resolved family name on `CellMetrics` (a new field) alongside `cell_width`/`line_height`, so it is computed exactly once per window and carried wherever a cell is drawn.
- `crates/app/src/terminal_element.rs`: `terminal_font(style)` takes the resolved family from `CellMetrics` instead of independently calling `FONT_TERMINAL.split_first()` - today these two call sites can resolve differently, which is the same bug class this task fixes, just reintroduced at the next font change if left as two sources of truth. `Font.fallbacks` keeps carrying `FONT_TERMINAL`'s remaining names, unchanged - that is glyph-level substitution within the resolved family, orthogonal to which family is requested, and still wanted. `paint_grid`'s per-run shape-and-paint gets the per-glyph safety check and conditional per-grapheme fallback described below; a new pure function computes a cell's own origin from its row/column, shared by it and `paint_cursor` (today each computes `bounds.origin + metrics.*_width * index` separately, inline).
- `crates/app/src/model.rs`: no change expected; `run_columns`'s existing grapheme segmentation is reused (via the same `unicode-segmentation` dependency already declared for it - no new manifest entry) to map a shaped glyph's byte index back to its own grapheme/column in `terminal_element.rs`.

## Order of work

1. Merge TASK-1.117's branch (above) and confirm `just check` still passes before touching anything - a clean base to build on.
2. Thread the resolved family through first, before any paint-logic change: the `CellMetrics` field, the pure resolution function and its tests (AC#1, AC#2), and `terminal_element.rs`'s `terminal_font` reading the resolved family instead of re-deriving it. This is the highest-risk step - get the single source of truth right before the per-cell placement work below is built on top of it, or the two changes race each other.
3. Per-cell-safe placement (AC#3). After `shape_run` produces a `ShapedLine` (which `#[deref]`s to `Arc<LineLayout>`, confirmed in gpui's own `text_system/line.rs`/`line_layout.rs`), walk its `runs`/`glyphs`: each `ShapedGlyph` carries `position: Point<Pixels>` and `index` (the byte offset into the run's own text). Map each glyph's `index` to the grapheme/column it belongs to (the same counting `run_columns` already does) and check `glyph.position.x` equals that column's own expected origin. If every glyph in the run checks out - the common case, a truly monospace resolved font, plain ASCII, nothing that triggered a glyph-level fallback - paint the single shaped line exactly as today (cheap, one shape and one paint call per run, the row cache keeps working unchanged). If any glyph's position is off, re-shape and paint that run per grapheme instead, each one its own tiny shaped line painted at its own column's explicit origin, so a wrongly-advanced glyph cannot move anything after it. The row cache's key needs to also cover which path a run took (or cache at grapheme granularity on the slow path), so a cache hit never returns the wrong shape for the wrong path.

   This makes AC#3's own wording ("a glyph... cannot move what follows it") hold unconditionally, not just in the common case: the single-shape path is an optimisation guarded by a runtime check that the invariant already holds for that specific run, not a quiet exception to the criterion. Extract the row/column-to-pixel-origin arithmetic into one pure function shared by this and `paint_cursor`, so AC#3's placement test has one function to call.
4. Tests, no window, each with a recorded failing-then-passing control:
   - The family-resolution function: a candidate list whose first name is absent from a fake `installed` list resolves to the second name; control is the function naively returning the list's own first entry regardless.
   - The cell-origin function: an 80-character line's next-cell origin (where the cursor sits after it) lands at exactly column 80's own origin, `80 * cell_width`; control is an off-by-one or a function that does not account for the row.
   - The per-glyph safety check itself needs a shaped line to inspect, which needs gpui's own text system - gpui ships a headless `TestAppContext` for exactly this (`app/test_context.rs`, no real display), the same shape its own `examples/testing.rs` uses; this plan treats that as satisfying the task's "no window" phrasing (no real macOS window/display, not literally no gpui context at all) and will say so plainly in the notes rather than assume it quietly. If `TestAppContext` cannot shape text without more than this task's scope (a risk, not yet confirmed), the fallback is to test only the pure arithmetic (origin function, resolution function) and note the per-glyph check as verified by the live run's screenshots instead - said in the notes if it comes to that, not decided now.
5. Column 0: confirm empirically (no real window during planning, so this is a build-time check) whether a glyph or the cursor in column 0 is clipped by the canvas's own bounds. I have no Artifact tool in this worker's tool list, so I cannot read the design's Layout page myself to confirm whatever inset value it names; that value, if any, is unverified by me and - since this bare window has no pane chrome at all yet (TASK-1.55 is where that would live) - may well belong there, not here. This task's own, narrower obligation, independent of that value: column 0 must be fully drawn, not clipped by the element's own bounds - checkable in code (nothing paints at an x less than the canvas's own origin) and in the PR's screenshots, without needing the design's number.
6. PR images (AC#4): a long line (eighty characters, matching AC#3's own test) with the cursor sitting at its end, and a listing in columns - both with nothing personal on screen, taken the same way TASK-1.117's own screenshots were.
7. Notes for AC#5 (nothing bundled in this task): JetBrains Mono ships under the SIL Open Font License 1.1, which is not currently in `deny.toml`'s allow list (confirmed: grepped for it, no entry - the policy is an allow-list, anything absent is refused). gpui 0.2.2 exposes `TextSystem::add_fonts(&self, fonts: Vec<Cow<'static, [u8]>>) -> Result<()>`, so a bundled font file could be embedded with `include_bytes!` and registered before the first resolution, if the engineer chooses to - a decision for them, with the licence addition deny.toml would need if they do; nothing implemented here.

## Out of scope
Cursor blink, the inverted character under a block cursor, and the unfocused window's hollow cursor - all TASK-1.56.

## Risks
- Threading the resolved family through two independent call sites is the step most likely to be half-done (fixed in one, missed in the other) - ordered first, above, specifically so the per-cell work in step 3 is built on one source of truth rather than racing it.
- Whether `TestAppContext` can shape text and expose `ShapedLine`'s per-glyph positions without a real display is assumed from gpui's own `testing.rs` example, not yet proven in this repository - the first real risk to resolve once building starts; the notes will say plainly if it does not hold and what was tested instead.
- Whether a genuinely monospace resolved font's own shaped glyphs land exactly on `column * cell_width` with no subpixel rounding (which would make the "common case" always take the slow path, defeating the point of it) is a build-time finding, not something the plan can assume - will be checked with a real run once the code exists, and the safety margin (exact equality vs. a small epsilon) decided then, stated in the notes either way.
- The column-0 clipping cause (the glyph's own left-side bearing vs. a genuinely missing inset) is unconfirmed until it is actually looked at on a real run; this plan commits only to "column 0 fully drawn", not to matching a design value I cannot read.

## Proof
- `cargo test -p steg-app` (new tests for the family-resolution function and the cell-origin function, each with a recorded failing-then-passing control via `asdlc record-check`).
- `just check`, recorded, at the final head.
- A live run (the standing protocol: draws, stays connected fifteen seconds idle, close and relaunch re-attaches) plus the two PR screenshots named in AC#4, confirming column 0 is not clipped and the per-cell placement holds on real text.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
From the engineer's second run, 2026-11-16, after installing JetBrains Mono: the spacing is right and the cursor sits where it should, which confirms the cause. One more thing visible in that screenshot, for the plan to settle: the text starts hard against the window's left edge with no inset, and the first column looks shaved by a few pixels (the prompt's arrow, the caret of ^C and a cursor in column 0 all look cut on their left side). Check whether glyphs or the cursor in column 0 are clipped by the element's bounds, and whether the design gives the terminal an inset inside its pane (doc-10, doc-17, the design's Layout page); if the inset belongs to the pane work (TASK-1.55), say so and make sure column 0 is at least fully drawn here.

Plan checked against the task's criteria, doc-17 and the design by @bruk.o: ok. The cause is read from gpui 0.2.2, not guessed: resolving a font tries the family asked for and then gpui's own fixed list, which reaches Helvetica; the fallbacks on a Font are used only for a glyph missing from a family already found. The plan picks the family once, in one place, for both measuring and painting, and paints a run as one shaped line only when every glyph is checked to sit on its own column, otherwise cell by cell. On the inset, which the worker could not read: the design gives 6px between panes and to the window edge (the gutter), and names no inset for the terminal's text inside a pane. So this task's duty is that column 0 is fully drawn; an inset inside the pane is a gap in the design for the engineer to settle with the pane work (TASK-1.55). JetBrains Mono's licence, SIL OFL 1.1, is not on the licence check's allow list, so shipping the font needs that list changed as well as the engineer's decision.
<!-- SECTION:NOTES:END -->
