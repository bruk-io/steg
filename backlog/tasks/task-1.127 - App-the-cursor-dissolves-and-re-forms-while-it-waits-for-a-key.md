---
id: TASK-1.127
title: 'App: the cursor dissolves and re-forms while it waits for a key'
status: In Progress
assignee:
  - '@bruk.w4'
created_date: '2026-11-17T03:51:00Z'
updated_date: '2026-11-18T04:42:00Z'
labels:
  - size-3
milestone: m-4
dependencies:
  - TASK-1.126
  - TASK-1.121
parent_task_id: TASK-1
project: bruk-io-steg
asdlc_accepted_from: DRAFT-1
asdlc_branch: task-1.127-app-the-cursor-dissolves-and-re-forms
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Chosen by the engineer on 2026-11-16 from the design's exploration board, Atoms - cursor motion (exploration), option 2B, and now the rule on the Cursor atom (design artifact, DesignSystem.Atoms, card 07). The cursor's cell is six pieces, two columns by three rows. While the terminal waits for a key they go out one at a time in a fixed, scattered order and come back in the reverse order; each piece is medium shade for one step on its way out and on its way back. One cycle is 1.4 seconds: whole for about 600 ms, out over 250 ms, empty for about 300 ms, back over 250 ms. The pieces and the shade are drawn by the app on the cell grid, never as characters from a font. This is periodic by nature, so it is the one stated exception to the rule against periodic wake-ups, and it is bounded: it runs only in the active pane of the focused window, only when the program has asked for a blinking cursor, and it stops after a few seconds with no key. doc-10; doc-17 for the design's named values; TASK-1.63 for Reduce Motion. Not in this task: the hollow cursor of an unfocused pane, and whether the bar and underline shapes fade the same way.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 In the active pane of the focused window, when the program has asked for a blinking cursor, the block cursor dissolves and re-forms as the Cursor atom draws it: the order of the six pieces, the medium shade step, and the 1.4 second cycle come from the design's values, not from numbers written in the app
- [ ] #2 A key press shows the cursor whole at once; after the idle time with no key it rests whole and nothing wakes the app until the next key or update; the idle time is a named value the engineer settles in the plan
- [ ] #3 Nothing wakes the app for the cursor when the window is not focused, the cursor is hidden, the program has not asked for blinking, or Reduce Motion is on; a test proves each case schedules no further frame, with recorded controls
- [ ] #4 Over a character, the character is drawn in the ground colour where the cursor's pieces cover it and in its own colour elsewhere
- [ ] #5 The step shown at a given time in the cycle is a pure function with a table test and a recorded control; the pull request carries a short recording, and stills of each step, with nothing personal on screen
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
## The foundations gap, found first (this gates AC#1)

`crates/ui/design/foundations.json` has no cursor-motion entry today (grepped: no "cursor" key outside `themes[].cursor: "session"`, which only names which colour fills it). The design's own durable, saved foundations pages (`design/canvas/project/{Tokens,Layout,Details}.dc.html`, TASK-1.126's own finding) carry no cursor-motion data either - grepped for "cursor" in all three, nothing beyond CSS pointer styles and the static terminal-theme swatch. The six-piece order, the 1.4s cycle, the 3% step and the medium-shade strength live only on the Atoms page's Cursor card (card 07), which has no saved/exported copy anywhere this repository can read, unlike Tokens/Layout/Details.

This is doc-17's own named gap ("Areas of concern": an unexported design value leaves the artifact and the repository apart) - the same shape TASK-1.126 hit with the terminal's content inset, and it answered it the same way this plan does: name the exact values and their shape, do not fabricate them in the app, and do not add them to `foundations.json` either, since R122 ("the export is a deliberate step... a change to it arrives as a pull request whose diff shows each value that changed") implies the data file is written from an actual read of the artifact, which this worker cannot do.

Checked, so this is not a blocker from `just check`'s side even if the values were added some other way: `check-design` (`tools/src/steg_tools/foundations.py`'s own docstring, confirmed by reading it) reads only `colours[]` and opacity names against the saved pages - "Sizes, type, motion, themes and the focus ring are otherwise unread." So a new `motion.*` entry is never checked against a saved page at all; the only real gate is `check-foundations` (generated.rs must match the data file, fixed by `just generate-foundations`) and R122's own process rule, which is a provenance rule for people, not a program check.

**For the engineer, exact JSON to add to `crates/ui/design/foundations.json`'s `motion` array** (bump `artifact_version` alongside it, same as any real export), once the Cursor atom card's values are confirmed against the artifact itself:

```json
{ "name": "motion.cursor-fade", "duration": 1400, "unit": "ms",
  "use": "One full dissolve-and-re-form cycle of the block cursor while it waits for a key" },
{ "name": "motion.cursor-fade-pattern", "order": [3, 0, 4, 2, 1, 5], "step_fraction": 0.03,
  "shade_strength": 0.5,
  "use": "The six cell pieces' own going-out order (row-major, 0 top-left..5 bottom-right); each step as a fraction of the cycle; how far medium shade sits from the cursor colour toward the ground (0=cursor colour, 1=ground)" }
```

Kept as two entries, not one, on purpose: `duration` is the only field with a unit (`ms`); `order`/`step_fraction`/`shade_strength` are dimensionless, and mixing a `unit: "ms"` group with the plain `order` list would make the generator's own `_DESCRIPTION` string wrongly suffix every index with "ms" (read in `_plain_field`: the list branch's `display` string appends the group's own `unit` to every entry, with no per-field override) - cosmetic only, but worth avoiding rather than explaining away later.

`shade_strength: 0.5` is this plan's own guess at "medium shade" (halfway between the cursor colour and the ground) - I cannot read the Atoms card, so this number is not verified against it. Flagged as a question below, not assumed settled.

**No generator change needed for `duration` or the scalars** (confirmed by reading `foundations_generate.py`'s `_plain_field`): a scalar with `unit: "ms"` already renders as `std::time::Duration`; a scalar with no group unit already renders as plain `f32`.

**One generator change this task's own files do make**: `_plain_field`'s `list()` branch, read directly, renders a list of plain numbers with no unit as `[f32; N]` - so `order` would come out as `[f32; 6]` (`3.0, 0.0, 4.0, ...`), a float array doing an index's job. This task extends that one branch: when every item is a whole number and the group carries no unit, render `[u8; N]` instead (six cell ranks fit comfortably; `_format_number` already strips the trailing `.0` for the `_DESCRIPTION` string, unaffected). New test in `tools/tests/unit/test_foundations_generate.py` for both the old float-list path (unit `px`, unchanged) and the new whole-number path. This is the one piece of this task that can be built and proven now, independent of whether/when the engineer adds the data: write the generator support and its test against a **fixture** JSON fragment, not against the real `foundations.json` (which does not have the entry yet).

**Question for the engineer** (blocks AC#1 until answered and acted on): please confirm the Cursor atom card's own `shade_strength` (or correct my 0.5 guess) and add the two JSON entries above (with the real `artifact_version`) to `crates/ui/design/foundations.json`, then run `just generate-foundations`. Until that lands, AC#1 cannot be closed in the strict sense ("come from the design's values, not numbers written in the app") - this task's own code reads `steg_ui::foundations::MOTION_CURSOR_FADE`/`MOTION_CURSOR_FADE_PATTERN` (generated consts), and is otherwise blocked on them existing. If the engineer would rather this task not wait, the alternative is to split: land the generator support and the pure animation/scheduling code now, gated behind the two consts with a `TODO`-free but honestly-named placeholder module that will not compile against real `foundations.json` until the entries exist - worse, since it leaves the branch unmergeable. Recommending the wait, not the workaround.

## Also found: TASK-1.100 overlap, for the orchestrator to de-conflict, not this task to resolve

TASK-1.100 ("ui atoms: label, key cap, swatch and cursor", To Do, no dependency edge to/from this task) builds a "Cursor" **atom in `crates/ui`** - pure, no domain type, "active and not" states - from the same Atoms-page Cursor card. This task's own fade lives in `crates/app/src/terminal_element.rs`, continuing the precedent TASK-1.126 already set (the terminal's cursor is painted directly on the cell grid in `crates/app`, not through a `crates/ui` component - doc-17's own scope carve-out: "The terminal view's drawing of cells... is not a component of this package's four levels unless its plan finds it should be"). This plan does not move it: the fade is specific to the terminal's own cell geometry (`cell_origin`, `CellMetrics`) which `crates/ui` knows nothing of, and TASK-1.100 is not built yet, so there is nothing to build on. The two pure functions this plan adds (piece-state-at-time-t, piece geometry) are plain data-in-data-out and sit in `crates/app` for that reason, not because they could not technically live in `crates/ui`. Flagging this for the orchestrator: TASK-1.100, whenever it is built, should decide for itself whether its own static "Cursor" atom borrows these functions or duplicates its own simpler (non-animated, since doc-17's R118 bars I/O/clock reads below the app layer) shape logic - this task does not pre-empt that.

## Files that change

- `crates/app/src/terminal_element.rs`: the pure piece-state function, the pure piece-geometry function, the painting of the six pieces and the clipped glyph-reveal, wired into `paint_cursor`. Also fixes the carried-over nit (task comment #1): the doc comment above `shape_and_check_run` (line 248) still names a `run_start_column` parameter the function no longer takes - corrected to describe what the function actually does now (no row-column parameter at all).
- `crates/app/src/main.rs`: new `CursorFade` state in `AttachedState` (the scheduled `Task<()>`, when the current cycle started, when the last key arrived); the pure scheduling-decision function (own module or kept in `terminal_element.rs` alongside the piece functions - leaning toward a new small `crates/app/src/cursor_fade.rs` so `main.rs` and `terminal_element.rs` both import one thing instead of each other); `on_key_down`/`send_bytes`'s own paths call into "show whole, reset idle clock" before sending; `apply_startup_result` creates the initial `CursorFade` state; the window-activation path (`_appearance_subscription`'s sibling - see mechanism below) re-evaluates scheduling.
- `crates/app/src/accessibility.rs` (new, small): `fn reduce_motion_enabled() -> bool`, a stub returning `false`, with a doc comment naming TASK-1.63 as the task that replaces it with a real OS read. Kept as its own function (not inlined) so the scheduling decision takes a plain `bool` parameter and never calls gpui/objc itself - the stub's own one caller passes the result in, the same shape `window.is_window_active()`/`focus_handle.is_focused(window)` already get called once and threaded through as plain values today in this crate's other pure functions.
- `tools/src/steg_tools/foundations_generate.py` + its test: the whole-number list -> `[u8; N]` rendering, above.
- `crates/ui/design/foundations.json` + `generated.rs`: not touched by this worker (the engineer's own step, above) - this task's own code is written against the two new consts and will not compile until they exist, which is the honest state to leave the branch in if the engineer's step is still pending when building starts.

## The pure core

### 1. Piece state at a point in the cycle

```rust
enum PieceState { Whole, Shade, Empty }

fn piece_states_at(
    fraction_through_cycle: f32,   // 0.0..1.0, already reduced mod 1 by the caller
    order: [u8; 6],                // MOTION_CURSOR_FADE_PATTERN.order
    step_fraction: f32,            // MOTION_CURSOR_FADE_PATTERN.step_fraction (0.03)
) -> [PieceState; 6]
```

Derives each piece's own four-boundary table (the brief's own "rank r: shade [43+3r,46+3r), empty, shade [97-3r,100-3r)... whole from 97-3r" - written generally from `order`/`step_fraction`, not six literal numbers, so the order and the step genuinely drive it) - `rank = order.iter().position(piece)`, `out_start = 0.5 - 3*step_fraction + rank*step_fraction`... Table-tested directly against the brief's own table (every boundary value for every rank, both edges of every interval - the `[43,46)` half-open convention matters at the boundary itself) with a control: break the function to use `order`'s own identity (rank == piece index) instead of the real permutation, rerun the rank-3/rank-0 swapped cases, watch them fail.

### 2. Piece geometry - no seams, no gaps, at any cell size

```rust
fn cursor_piece_bounds(
    cell_origin: Point<Pixels>,
    cell_width: Pixels,
    line_height: Pixels,
) -> [Bounds<Pixels>; 6]
```

Computes the two column edges (`x0 = origin.x`, `x_mid = origin.x + round(cell_width/2)`, `x1 = origin.x + cell_width`) and three row edges (`y0`, `y_third`, `y_two_thirds`, `y1`, each rounded once) **first**, then builds each of the six rectangles from two *adjacent already-rounded* edges - never from a rounded half-width doubled, which is exactly the kind of arithmetic that leaves a 1px seam or gap when `cell_width`/`line_height` are not evenly divisible by 2 or 3 (true today: `line_height` is 20px from foundations - 20/3 = 6.667). Tested at the real `cell_width`/`line_height` and at a few odd sizes (e.g. 7px/19px) asserting the six rectangles' union has no gap and no overlap, and that adjacent pieces share an exact edge (piece 0's right edge equals piece 1's left edge, etc.) - a property test over the six pairs sharing an edge, with a control that rounds each piece's own width independently (reproducing the seam) to confirm the test catches it.

### 3. The scheduling decision - pure, no gpui type, every AC#3 case a table row

```rust
enum NextWake { Never, After(Duration) }

fn next_cursor_wake(
    window_active: bool,
    pane_focused: bool,
    cursor_visible: bool,
    blink_requested: bool,
    reduce_motion: bool,
    ended: bool,
    time_since_last_key: Duration,
    rest_after_idle: Duration,     // the engineer's own named value, see question below
    time_in_cycle: Duration,
) -> NextWake
```

Returns `Never` when any of `!window_active`, `!pane_focused`, `!cursor_visible`, `!blink_requested`, `reduce_motion`, `ended`, or `time_since_last_key >= rest_after_idle` holds - the AND of every gate the brief names, each its own test with the rest of the inputs held at their "would otherwise animate" values (one case fails at a time, isolating which gate the test is actually proving - a test that flips every gate at once would not tell AC#3's six cases apart). Otherwise returns `After(duration to the next 3%-of-cycle boundary from `time_in_cycle`)`, floored at `step_fraction * MOTION_CURSOR_FADE.duration`, never shorter (no point waking mid-step) and never longer (no point sleeping past the next real state change) - its own test walks `time_in_cycle` across a full 1.4s cycle and checks the returned delay always lands exactly on the next of the 14 boundaries the brief's table implies (recounted directly from the table, not from the brief's "13" - boundaries at 0, 43, 46, 49, 52, 55, 58, 61, 82, 85, 88, 91, 94, 97, wrapping to 0/100; 14 half-open intervals, not 13 - the two 18%-wide transitions each cross six 3% boundaries, `[61,82)` is one 21%-wide all-empty interval, `[0,43)` one 43%-wide all-whole interval).

## The driver - which gpui mechanism, and exactly how it is bounded

**Scheduling**: `window.spawn(cx, async move |cx| { cx.background_executor().timer(duration).await; ...recompute and repaint, reschedule or stop... })`, its `Task<()>` stored in `AttachedState`/`CursorFade`. Not `request_animation_frame`/`on_next_frame` - both tie to the display's own refresh cadence (confirmed in gpui's own doc comment on `request_animation_frame`, `window.rs:1648`: "useful for... continuous[]" animation, i.e. every frame) and would wake far more often than the 13-42ms-apart boundaries actually need; `BackgroundExecutor::timer` (`executor.rs:357`) sleeps for exactly the bounded duration `next_cursor_wake` computed and nothing in between.

**Cancellation, proven from gpui's own source, not assumed**: "If you drop a task it will be cancelled immediately" (`gpui-0.2.2/src/executor.rs:54`, the doc comment directly on `Task`). So: on every repaint (`paint_cursor`), recompute `next_cursor_wake` from the current, freshly-read state; if it is `Never`, drop whatever `Task` was stored (setting the field to `None` is enough - the drop is what cancels the in-flight timer); if it is `After(duration)`, and nothing is already scheduled for that same boundary, replace the stored `Task` with a freshly spawned one. No separate generation counter needed for *this* mechanism (the advisor's suggested fallback) - the stored `Task` field itself is the single source of truth for "is anything scheduled", and dropping it is gpui's own guaranteed cancellation, cited above, not this task's own assumption.

**Window deactivation - the one case with no public gpui observer, and why it still holds**: gpui 0.2.2 has no public API to subscribe to window activation changes (`activation_observers` is `pub(crate)`, grepped, no public registrar method anywhere in `window.rs`/`app.rs`). But `on_active_status_change`'s own internal handler (`window.rs`, around line 1101) calls `window.refresh()` on every activation change, before anything else - and `refresh()`'s own doc/body marks the window dirty, forcing exactly one more repaint. That repaint is where `paint_cursor` re-reads `window.is_window_active()`, finds it now `false`, and drops the stored `Task` - so deactivation is caught at the very next paint it itself causes, not by a dedicated subscription. The bounded gap this leaves: if the window deactivates *mid-animation*, the fade keeps running until that one forced repaint (effectively immediate, not up to a wake period), and if the window deactivates while *resting whole* (nothing scheduled, no repaint pending otherwise) the forced repaint still happens, confirms nothing further is due, and schedules nothing - either way bounded to that one extra paint, never an ongoing wake. Written down plainly rather than assumed, since it is the one gate this task cannot test by driving a real window (workers cannot launch the app) - the `next_cursor_wake` pure function's own `window_active: false` test is the proof of the decision; this paragraph is the proof of when gpui actually calls it.

**Intra-window pane focus** (today there is exactly one pane/terminal element - `focus_handle.is_focused(window)` already read at paint time the same as `window.is_window_active()`, no separate subscription needed for the same reason: every paint re-reads it fresh.

## Key press and the idle clock

`on_key_down`/the IME commit path (`main.rs`) call a new `AttachedState` method, `show_cursor_whole_and_reset_idle_clock()`, before `send_bytes`: sets `last_key_at = Instant::now()` and `cycle_started_at = Instant::now()` (AC#2's "shows the cursor whole at once" - restarting the cycle at its own 0% is what "whole" means per the state table, not a separate flag), then `cx.notify()`/schedules a repaint so the whole state actually paints this frame rather than waiting for the next already-scheduled wake.

**Question for the engineer - the idle rest time (`rest_after_idle`), AC#2's own "named value the engineer settles"**: I have no verified precedent for "a cursor stops animating after N seconds idle" specifically (most terminals I can reason about from the task's own description keep blinking indefinitely while idle; this design's choice to rest is the exception, not something I can point at another product doing the same way) - so this is a guess, offered with the only arithmetic I can actually stand behind: each 1.4s cycle takes about 12 wakes (two 250ms transitions at 14ms-apart... no, at 3%-of-1400ms = 42ms apart, six boundaries each = 12 wakes/cycle), about 8.5 wakes/second while animating. A default of **7 cycles (~10s)** keeps the cursor visibly "alive" through a normal pause for thought without turning the one stated exception to the no-periodic-wakeup rule into an indefinite one; **5 cycles (~7s)** is the tighter alternative if the engineer weighs the wake cost higher; "never rest" (match the common pattern of continuous blinking while idle) is the no-op alternative that drops AC#2's own "idle time" requirement entirely and would need the criterion reworded. Recommending 7 cycles, as a round, legible number in terms of the cycle itself rather than an arbitrary millisecond count, for the engineer to confirm or replace.

## Reduce Motion

Grepped `gpui-0.2.2`'s full source (`src/*.rs`, `src/platform/**/*.rs`) for "reduce_motion"/"ReducedMotion"/"accessibility" - nothing at all; a genuine negative result, not an unchecked assumption. The real macOS read (`NSWorkspace.accessibilityDisplayShouldReduceMotion`) needs an Objective-C bridge this crate does not depend on today (`cargo metadata` shows `steg-app`'s only dependencies are `futures`, `gpui`, `serde_json`, `steg-sdk`, `steg-ui`, `unicode-segmentation` - no `objc`/`cocoa`/`objc2`) - the same shape of gap TASK-1.126 found and declined to close itself when `gpui`'s own `test-support` feature would have pulled in dependencies and a licence surface unrelated to that task's own bug.

This task takes the stated fallback: `accessibility::reduce_motion_enabled() -> bool` (new file, above) is a stub, always `false`, with a doc comment pointing at TASK-1.63 (titled for exactly this: "App: keyboard focus, VoiceOver, Reduce Motion and Increase Contrast") as the task that replaces it with a real platform read. `next_cursor_wake`'s own `reduce_motion: bool` parameter is fed from this one call site, so AC#3's "Reduce Motion is on" test is real and meaningful today (constructing the pure function with `reduce_motion: true` directly, no stub involved) even though there is no control yet proving the *stub* itself ever becomes `true` - stated as a gap, not quietly left out. **Question for the engineer**: confirm this split (stub now, TASK-1.63 completes it) rather than this task adding the real Obj-C read itself - the same choice TASK-1.126 faced and the plan defaults to "no" for the same reason (new dependency, new licence surface, out of this task's own named scope).

When Reduce Motion is on (once real): the cursor shows whole and steady, the same `CursorShape::Block` quad `paint_cursor` already draws today - `next_cursor_wake` returns `Never` and the six-piece path is simply never entered; no new "steady" code path to maintain beyond what already exists.

## Painting

`paint_cursor` (today: one `match` on `cursor.shape`) grows a branch: when `cursor.shape == CursorShape::Block && cursor.blinking && pane is focused/active (as above) && !reduce_motion`, compute `fraction_through_cycle` from `Instant::now() - cycle_started_at` (`imperative shell reads the clock once here; everything downstream is a plain `Duration`/`f32`, per the pure-core functions above), call `piece_states_at`, then `cursor_piece_bounds`, then for each of the six pieces: `Empty` paints nothing; `Whole`/`Shade` paints a filled quad at that piece's own bounds, in the cursor colour (`Whole`) or the blended shade colour (`Shade`, `blend(theme.cursor, theme.bg, MOTION_CURSOR_FADE_PATTERN.shade_strength)` - a small new pure `blend(Hsla, Hsla, f32) -> Hsla`, linear per RGB channel after converting from `Hsla`, with its own test rather than trusting a straight `Hsla` lerp across the hue wheel to look like "shade" rather than a colour shift).

**The character under the cursor, in the ground colour where covered** (AC#4): find which grapheme, if any, sits at `(cursor.x, cursor.y)` on the model's own grid - a new pure function `grapheme_at(row: &Row, column: u16) -> Option<(&str, &CellStyle, u16 /*its own run-start column*/, u16 /*columns per grapheme*/)>`, the same run-walking `paint_grid`'s own loop already does, pulled out so this lookup has its own test (empty cell past the last character returns `None` - nothing to redraw, the pieces paint directly over the background, which is already correct). When `Some`, shape that one grapheme once (its own tiny `ShapedLine`, in the ground colour, built the same way `shape_and_check_run`'s per-grapheme path already does - **not** re-shaped per animation step: shaped once per repaint-triggering state change, same row-cache discipline `RowCache` already gives every other run, keyed the same way `run_key` is, so a cache hit skips shaping entirely across the many wakes within one unchanged character). For each of the up to six covered pieces (`Whole`/`Shade`, never `Empty`), `window.with_content_mask(Some(ContentMask { bounds: piece_bounds }), |window| { shaped_glyph.paint(...) })` (`window.rs:2384`) - clipping the same full-cell glyph paint to that one piece's own rectangle, so the union of up to six clipped paints reconstructs exactly the covered area, and every `Empty` piece leaves the character's own already-painted, own-coloured glyph (from the ordinary row paint, unchanged) showing through untouched. Per-step cost, stated plainly: up to six quad fills plus up to six clipped paints of one already-shaped, cached line - no re-shaping, no change to how any other cell on the row is painted.

**Wide characters under the cursor**: the design's own rule speaks of "the cursor's cell", singular. Default taken here: the six pieces cover the head cell only (`cursor.x`'s own column); the spacer tail cell (if the cursor sits on a wide character) is untouched by this task, same as today. Stated as a limitation, not quietly extended - out of this task's own named scope ("whether the bar and underline shapes fade the same way" is explicitly named as deferred; a wide character under a fading block cursor is the same kind of question, not named, so left as today's behaviour rather than guessed at).

## Order of work

1. Generator support for a whole-number list (`tools/`), its own test against a fixture - buildable and provable today, independent of the foundations question above.
2. The three pure functions (`piece_states_at`, `cursor_piece_bounds`, `next_cursor_wake`) and `blend`, each with its own table tests and a recorded failing-then-passing control - all buildable against literal constants standing in for the not-yet-added foundations values, so this step does not block on the engineer's own step either.
3. `grapheme_at`, with its own tests (covered cell, empty cell, a grapheme at a non-zero run-start column - the same coordinate-space care TASK-1.126's own review round took, reused rather than relitigated).
4. Wire the real `steg_ui::foundations::MOTION_CURSOR_FADE`/`MOTION_CURSOR_FADE_PATTERN` consts in once the engineer's own step has landed (this is the one step that cannot proceed until then) - `accessibility::reduce_motion_enabled`, `CursorFade` state in `AttachedState`, the `Task` scheduling/cancellation, the key-press reset, and the painting path (pieces, shade blend, glyph-reveal clipping).
5. The doc-comment nit fix (one line, `terminal_element.rs:248`).
6. PR stills and recording (below).

## Risks

- **Blocked on the engineer's own step** (the foundations values) - named first, above, so it surfaces immediately rather than discovered mid-build. Steps 1-3 above do not depend on it and can proceed regardless.
- **The idle-rest default is a genuine guess**, not a verified precedent - named as a question, not silently picked.
- **`shade_strength: 0.5` is also a guess** at a value only the Atoms card itself states precisely - named as a question alongside the JSON.
- **The window-deactivation mechanism rests on an internal gpui code path** (`on_active_status_change` calling `refresh()`) rather than a public, documented contract - cited from source directly above, but gpui could change this between versions without it being a breaking change to its own public API; worth a one-line comment at the call site saying so, for the next person reading it.
- **No real control for the Reduce Motion *stub* flipping** - only `next_cursor_wake`'s own injected-`bool` test is real; stated plainly in the notes, same shape as TASK-1.126's own disclosed gap for its one untestable function.
- **TASK-1.100 overlap** - named above, for the orchestrator, not resolved by this plan.

## Proof, each criterion its own control

- **AC#1** (order/step/cycle from the design's values): blocked on the engineer's own step (above); once landed, a test reads `steg_ui::foundations::MOTION_CURSOR_FADE_PATTERN.order` directly (not a literal `[3,0,4,2,1,5]` written in the test) and confirms `piece_states_at` reproduces the brief's own table through it - control: edit the *generated* constant in a scratch build to the identity order and watch the same test fail, confirming the test is actually reading the generated value, not a hardcoded one.
- **AC#2** (key press shows whole at once, idle clock, named value): a test on `show_cursor_whole_and_reset_idle_clock`'s own effect (no window - it only touches two `Instant` fields) and on `next_cursor_wake` with `time_since_last_key` just under/over `rest_after_idle`, control by moving the boundary.
- **AC#3** (nothing wakes the app - six cases, each isolated): the `next_cursor_wake` table above, one test per gate, control by flipping the one gate under test back to its "would animate" value and watching the test fail for the right reason.
- **AC#4** (ground colour where covered): `grapheme_at`'s own tests, plus a live-run screenshot (the engineer's own, per the rules below) showing a fading cursor mid-transition over real text, with the covered portion of the character visibly in the terminal's own background colour.
- **AC#5** (pure step function, table test, stills, recording): `piece_states_at`'s own table test above is the control; stills and the recording per the next section.

## AC#5's stills and the recording - what workers cannot do, and what the engineer does instead

Workers must not launch the app, drive input, capture the screen, or touch the clipboard on this machine. Stills of each of the 14 states **can** be produced with no window and no app at all: a small, separate script (or a `#[test]`-adjacent example, not shipped in the app binary) that calls `piece_states_at`/`cursor_piece_bounds`/`blend` directly for each of the 14 boundary fractions and renders the six rectangles to a plain image (e.g. via a minimal software rasteriser or an SVG written out and rasterised by a tool already available) - this is rendering the *pure function's own output*, never the app's own GPUI paint path, so it proves the state table, not the paint call; worth saying plainly in the PR, not implying it is a screenshot of the running app. The recording (the cursor actually dissolving and re-forming in the real window) and a still of AC#4's ground-colour-under-the-cursor effect over real text can only come from the engineer's own run, per the standing rule - the PR body will say exactly what to capture: focus the pane, stop typing, let one full 1.4s cycle play on camera, and one still mid-transition with real text under the cursor.

## Whether this is really two changes

Recommending **yes, split**: the generator support (`tools/`, step 1) and the pure core (step 2-3) are buildable and provable now, with no dependency on the engineer's own step; the wiring, painting and PR recording (step 4-6) cannot proceed until the foundations values land. Splitting lets the first piece merge without waiting on a design-artifact edit that is entirely out of this worker's hands, and keeps the second piece's own review scoped to "does this read the real generated values correctly and paint them right" rather than also re-litigating the pure functions' own correctness a second time. If the engineer would rather keep it as one task and simply wait, that is a smaller cost than it looks (steps 1-3 together are most of this task's own line count) but is the engineer's call, not this plan's.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Split by @bruk.o on 2026-11-18, as the plan recommended: the design values and the generator's whole-number lists moved to a draft task, 'ui: the cursor fade's named values, exported from the design, and whole-number lists in the generator', which this task will depend on once it is accepted. This task keeps the pure functions, the driver, the painting and all five criteria; its first criterion is met by reading the values that task exports. On the overlap with TASK-1.100 (the Cursor atom in crates/ui): the plan keeps the fade's pure functions in crates/app with the terminal's cell geometry, under doc-17's carve-out for the terminal view; TASK-1.100 decides for itself whether to reuse them. The orchestrator agrees and has noted it on TASK-1.100.
<!-- SECTION:NOTES:END -->

## Comments

<!-- COMMENTS:BEGIN -->
created: 2026-11-17T21:24:00Z

---

Carry-over from TASK-1.126's second review (nit, not fixed there because the task was locked on passing): the doc comment above shape_and_check_run in crates/app/src/terminal_element.rs still describes a run_start_column parameter the function no longer takes. Correct it when this task touches the file. - @bruk.o

---
<!-- COMMENTS:END -->
