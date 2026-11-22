---
id: TASK-1.127
title: 'App: the cursor dissolves and re-forms while it waits for a key'
status: In Progress
assignee:
  - '@bruk.w4'
created_date: '2026-11-17T03:51:00Z'
updated_date: '2026-11-22T04:31:00Z'
labels:
  - size-3
milestone: m-4
dependencies:
  - TASK-1.126
  - TASK-1.121
  - TASK-1.136
parent_task_id: TASK-1
project: bruk-io-steg
asdlc_accepted_from: DRAFT-1
asdlc_branch: task-1.127-app-the-cursor-dissolves-and-re-forms
asdlc_plan_approved_by: bruk
asdlc_plan_approved_hash: 'e3032b0ac5e3'
asdlc_plan_approved_size: size-3
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
## Second revision, 2026-11-20: the driver corrected - wake on state change, not every display frame

The orchestrator's own earlier wording ("GPUI's own animation/frame request") pointed at `request_animation_frame`; this revision corrects that to a timer re-armed once per state change, since the animation has only about 13-14 distinct states a 1400ms cycle and a continuous `request_animation_frame` chain would repaint the whole canvas at 60-120Hz for as long as the window sits focused and idle - most of a terminal's life - for a cursor that is actually still for 43% of each cycle. Everything below in "The driver" is rewritten; the pure gate (`should_show_cursor_fade`) and the rest of the plan are unchanged.

## First revision, 2026-11-20: rewritten against the engineer's decisions on the task's own notes

Checked `git diff HEAD origin/main` for every path this plan cites (`crates/app`, `crates/ui`, `crates/sdk/src/screen.rs`, `tools/src/steg_tools/foundations*.py`, `design/`): none of them differ from origin/main - the only drift is in `crates/sdk/src/{keys.rs,local_socket.rs,record.rs,wire/connection.rs}`, `crates/app/src/keys.rs` and server/CLI crates this plan never cites, so no merge was needed to keep this plan accurate. `Cursor`/`CursorShape` (`crates/sdk/src/screen.rs`) are unchanged - `blinking: bool` is still on the wire exactly as before.

This revision drops the Task-based timer driver, the idle-rest value and everything built on it, folds the engineer's confirmed values in (no more guesses at `shade_strength` or the idle time), and depends on TASK-1.136 (now In Progress, @bruk.w5) for the foundations export instead of asking the engineer to hand-edit `foundations.json`.

## What changed from the first plan, and why

- **No idle rest.** The fade runs for as long as the window has focus; it stops, whole, the moment the window loses focus. `rest_after_idle`, `last_key_at`, and `next_cursor_wake`'s duration-returning shape are gone. AC#2's own "idle time" sentence is superseded by the engineer's note - this plan treats AC#2 as "a key press shows the cursor whole at once and restarts the cycle" with no further clause, and AC#3's cases as the ones actually listed (window not focused, cursor hidden, no blink requested, Reduce Motion on) plus "ended", none of them idle-time-based any more.
- **The driver wakes on state change, not every display frame** (corrected in the second revision, above) - a one-shot timer re-armed from each paint that finds the gate open, targeting the next of the cycle's own ~13-14 boundaries, never `request_animation_frame`. See "The driver" below.
- **`shade_strength` and the idle fractions are no longer guesses.** The engineer's decision gives them directly: medium shade is the *flat* colour half way between the cursor colour and the ground (0.5, no dither - ruled out explicitly, see Painting below); hold 43%, out 18%, empty 21%, return 18%, matching `6 * step_fraction` (18%) for both transitions exactly, with `hold_fraction` (43%) as the one scalar not derivable from `order`/`step_fraction` alone (`empty_fraction` follows: `1 - hold - 2*len(order)*step_fraction`).
- **Depends on TASK-1.136**, not on the engineer hand-editing `foundations.json`. That task is already linked as this one's dependency on the board (from the 2026-11-18 split) and is being planned separately; this plan says exactly which named values it needs from it, including the one its own notes flagged as dropped (idle time) and the one addition this plan's own arithmetic needs that TASK-1.136's AC#1 doesn't yet name precisely (`hold_fraction`).
- **Hold the animation as data, build only 2B.** The pure functions below take `order`, `step_fraction`, `hold_fraction`, `shade_strength` as parameters, never literals - so a second variant (the "become a setting" follow-up, named at the end, not built here) is new data plus a variant selector, not a rewrite of the state machine.

## What TASK-1.136 needs to export (for that task's own plan, stated precisely so nothing is re-derived by guesswork)

From the engineer's decision (design artifact version 143, `1795195942-6a0a`, Foundations page section 12, Cursor atom):

- `motion.cursor-fade`: `duration: 1400ms`.
- `motion.cursor-fade-pattern`: `order: [3, 0, 4, 2, 1, 5]` (whole numbers, needs the generator's integer-list fix TASK-1.136 already owns), `step_fraction: 0.03`, `hold_fraction: 0.43`, `shade_strength: 0.5`.

`out_fraction`/`return_fraction` are not separate fields: both equal `order.len() as f32 * step_fraction` (6 * 0.03 = 0.18), derived in code, not exported twice. `empty_fraction` is `1.0 - hold_fraction - 2.0 * order.len() as f32 * step_fraction`. This is fewer independent values than TASK-1.136's own AC#1 lists ("the cycle's duration, the order... the step... the strength of medium shade, and the idle time" - the last one dropped by its own notes) plus the one its notes invite this plan to ask for if needed: `hold_fraction`, asked for here.

## The pure core

### 1. Piece state at a point in the cycle - now driven entirely by exported data

```rust
enum PieceState { Whole, Shade, Empty }

fn piece_states_at(
    fraction_through_cycle: f32,  // 0.0..1.0, already reduced mod 1 by the caller
    order: &[u8],                 // MOTION_CURSOR_FADE_PATTERN.order
    step_fraction: f32,
    hold_fraction: f32,
) -> Vec<PieceState>              // one per piece in `order`'s own indexing, not `order`'s own sequence
```

Unchanged in shape from the first plan's version, except every boundary is now computed from `hold_fraction`/`step_fraction`/`order.len()` rather than a mix of a literal 43% and a derived 18% - table-tested against the engineer's own table (every boundary for every rank, both edges of each half-open interval), with a control that swaps `hold_fraction` for a wrong value and watches the hold-length-dependent boundaries move while the step-dependent ones (the transition widths) do not - the two kinds of boundary come from different inputs and the test proves that, not just that the whole table matches once.

Recount, kept from the first plan since it still holds: the table has 14 half-open intervals, not 13 (`0, 43, 46, 49, 52, 55, 58, 61, 82, 85, 88, 91, 94, 97`, wrapping to `100 = 0`) - `[0,43)` whole, six 3%-wide going-out steps to `[58,61)`, `[61,82)` all empty (21%), six 3%-wide returning steps from `[82,85)`, wrapping to whole again.

### 2. Piece geometry - unchanged from the first plan

```rust
fn cursor_piece_bounds(
    cell_origin: Point<Pixels>,
    cell_width: Pixels,
    line_height: Pixels,
) -> [Bounds<Pixels>; 6]
```

Two column edges and three row edges rounded once, each of the six rectangles built from two *already-rounded, adjacent* edges, never a rounded half-width doubled - tested for no gap/overlap and shared edges at the real cell size and at odd sizes (e.g. 7px/19px), with a control that rounds each piece's width independently to reproduce a seam.

### 3. The animation gate - a bool, not a scheduling duration

```rust
fn should_show_cursor_fade(
    window_active: bool,
    pane_focused: bool,
    cursor_visible: bool,
    blink_requested: bool,
    reduce_motion: bool,
    ended: bool,
) -> bool
```

`true` only when `window_active && pane_focused && cursor_visible && blink_requested && !reduce_motion && !ended`. Six tests, each isolating one gate (the rest held at their "would otherwise animate" values) - AC#3's own cases (window not focused, cursor hidden, no blink requested, Reduce Motion on) plus `ended`, each its own control: flip the one gate under test back to "would animate" and watch that one test fail. No duration, no `Instant`, nothing time-based in this function at all - it answers "should the next frame be requested", not "when".

## The driver - wake on state change

### The pure schedule: next wake from any point in the cycle

```rust
fn next_wake(
    fraction_through_cycle: f32,  // 0.0..1.0, already reduced mod 1 by the caller
    order_len: usize,             // MOTION_CURSOR_FADE_PATTERN.order.len() (6)
    step_fraction: f32,
    hold_fraction: f32,
    cycle_duration: Duration,
) -> Duration
```

Builds the same boundary set `piece_states_at` reasons from - generically, from `hold_fraction`/`step_fraction`/`order_len`, not six literals: `{0.0, hold}`, `hold + i*step` for `i in 1..=order_len` (reaching `hold + out`, 61% today), `hold + out + empty` (`1.0 - out`, 82% today), then `hold + out + empty + i*step` for `i in 1..=order_len` (reaching `1.0`, which wraps to the next cycle's `0.0`) - `2*order_len + 2` boundaries total (14 today, matching the recount in the first revision). Finds the smallest boundary strictly greater than `fraction_through_cycle` (wrapping past `1.0` back to the first), and returns `(that boundary - fraction_through_cycle) * cycle_duration`. Pure, no `Instant`, no gpui type - table-tested by walking a fake "elapsed" clock across a full cycle (own module, no window) and asserting: (a) the returned duration always lands exactly on the next real boundary, never short (waking mid-step, wasted) or long (sleeping past a real change); (b) summing the durations from one full traversal of the cycle (restarting the walk from each landed-on boundary until back past 1.0) yields exactly `2*order_len + 2` wakes, not an assumed "13"; (c) a control that corrupts one boundary (e.g. drops a step) changes that count, proving the test actually counts real boundaries rather than trusting a fixed expectation.

**Phase from elapsed time, never accumulated** (the orchestrator's own "a late wake never accumulates drift"): every time this is called - whether from the timer firing or from an unrelated repaint - the caller recomputes `fraction_through_cycle` fresh from `(Instant::now() - cycle_started_at) % cycle_duration`, never from "the duration I last scheduled plus however long ago that was". A wake that fires a few milliseconds late (gpui's own executor is not real-time) simply computes a slightly later-than-expected `fraction_through_cycle`, finds the next boundary from *there*, and the schedule self-corrects every single wake rather than compounding error cycle over cycle.

### Scheduling and cancellation

`AttachedState` gets `cycle_started_at: Instant` and `scheduled_wake: Option<Task<()>>` (replacing the first revision's plan to drop the `Task` field entirely - it is back, for a different reason: there is now a real multi-step-away wake to hold and cancel, not "nothing further out than the next frame"). On every paint (`paint_cursor`) that finds `should_show_cursor_fade(...)` `true`: compute `fraction_through_cycle`, call `next_wake`, and - regardless of what was scheduled before - drop the existing `scheduled_wake` (if any) and replace it with a freshly spawned one: `window.spawn(cx, async move |cx| { cx.background_executor().timer(duration).await; entity.update(cx, |_, cx| cx.notify()).ok(); })`. Dropping the old task cancels it outright ("if you drop a task it will be cancelled immediately", `gpui-0.2.2/src/executor.rs:54`, the doc comment directly on `Task` - cited, not assumed, as before) - so a paint that happens for an unrelated reason (a model update, a resize) while a wake is already pending simply re-targets the schedule from the now-current phase rather than letting two timers race. When the gate is `false`, `scheduled_wake` is set to `None` - dropping whatever was pending, cancelling it, and nothing further is armed until some other event makes the gate `true` again and a paint runs.

About 13-14 of these wakes happen per 1.4s cycle (`2*order_len + 2`), roughly 9-10 a second while the gate holds - not 60-120 wakes a second, and not a polling loop: `next_wake` is never called on a fixed interval, only re-armed from a paint that already has a reason to run, and every wake it does produce corresponds to one of the animation's own real state changes (a piece crossing into or out of shade, or the hold/empty plateaus beginning or ending), never a no-op repaint of an unchanged frame.

### What makes a paint happen at all, so the schedule starts/stops without polling

- **A key press** (`show_cursor_whole`, below) always calls `cx.notify()`.
- **Ordinary model changes already notify**: `handle_client_event` (`main.rs`) already calls `cx.notify()` on every `Update` and on `Ended` - so the moment `cursor.blinking` turns `false`, `cursor.visible` turns `false`, or the pane ends, the very next (already-guaranteed) repaint sees the new gate state, drops `scheduled_wake`, and nothing further is armed.
- **Window activation, both directions**: `on_active_status_change`'s own internal handler (`gpui-0.2.2/src/window.rs`, confirmed around line 1101) calls `window.refresh()` on *every* activation change, marking the window dirty and forcing one repaint regardless of anything this crate does - no public gpui subscription API exists for this in 0.2.2 (`activation_observers` is `pub(crate)`, grepped, confirmed no public registrar), so this plan relies on gpui's own existing internal behaviour rather than a subscription this crate would need to add. On regaining focus, that forced repaint finds the gate now `true` and arms the first wake; on losing focus, it finds the gate now `false` and drops/cancels whatever was pending - both directions the same mechanism.
- **Intra-window pane focus**: today there is exactly one focusable element, so `focus_handle.is_focused(window)` read fresh at paint time is enough on its own - this plan also registers `window.on_focus_in`/`on_focus_out` (`window.rs:3481`/`3501`, both public, confirmed by reading their signatures: `on_focus_in(&mut self, handle: &FocusHandle, cx: &mut App, listener: impl FnMut(&mut Window, &mut App))`, `on_focus_out` the same shape with a `FocusOutEvent`), each calling `cx.notify()`. Both return a `Subscription` that must be held (gpui's own doc: "persists until the subscription is dropped") - two more fields on `AttachedState`, alongside the existing `_resize_subscription`/`_appearance_subscription` pattern. Cheap to add now, unreachable today (nothing else to focus to), not tested live.

### The honest cost, stated plainly, not measured live (workers must not launch the app)

While the gate holds, this produces roughly 9-10 repaints a second (`2*order_len + 2` per 1.4s cycle), not a continuous 60-120Hz redraw - each one re-walking the whole canvas (`terminal_element.rs`'s own module doc: gpui 0.2.2's `canvas` has no partial-repaint primitive, so every repaint already re-walks every visible row regardless of the fade), but the `RowCache` still shields every run except the cursor's own grapheme from reshaping, so the fade's own incremental per-wake cost is small: one `should_show_cursor_fade` call (branchless booleans), one `next_wake` call (a handful of comparisons), one `piece_states_at` call, up to six quad fills (the same gpui primitive `paint_cursor` already issues once today, now up to six times a wake), and up to six clipped paints of one already-shaped, cached line (a cache hit - no reshaping). This is bounded analytically, not profiled; whether ~9-10 extra whole-canvas repaints a second while a pane sits focused and idle is acceptable is for the engineer's own live run to actually see, which this task does not attempt.

## Key press - simplified, no idle clock

`on_key_down`/the IME commit path (`main.rs`) call a new `AttachedState` method, `show_cursor_whole()`: sets `cycle_started_at = Instant::now()`, drops/replaces `scheduled_wake` (the old schedule was targeting boundaries of a cycle that no longer applies), and calls `cx.notify()` so the whole state paints this frame and a fresh wake is armed from the new cycle origin if the gate is open. No `last_key_at`, no idle comparison - the method only ever resets the cycle, never a timeout.

**Interpretation, accepted by the orchestrator**: on regaining window focus, this plan resets `cycle_started_at = Instant::now()` (in the same activation-regained path above) rather than resuming a frozen mid-fade phase from before the window lost focus - decided by the orchestrator against the engineer's own "stops, whole, when the window loses focus" (freezing *at* whole, not wherever the fade happened to be), and consistent with the key-press behaviour already doing the same reset.

## Reduce Motion - unchanged mechanism, restated against the engineer's confirmation

`accessibility::reduce_motion_enabled() -> bool`, a stub always returning `false` ("not reduced" - motion always allowed), with a doc comment naming TASK-1.63 as the task that replaces it with a real platform read (gpui 0.2.2 exposes nothing - grepped `src/*.rs`/`src/platform/**/*.rs` for "reduce_motion"/"accessibility", confirmed empty; the real macOS read needs an Obj-C bridge this crate does not depend on, the same gap TASK-1.126 found and declined to close itself for a one-off test). `should_show_cursor_fade`'s own `reduce_motion: bool` parameter is fed from this one call site. The steady cursor "built behind it" (the engineer's own phrase) is nothing new: it is the existing `CursorShape::Block` filled quad `paint_cursor` already draws today - when the stub (or later, the real read) says motion is reduced, `should_show_cursor_fade` returns `false`, the six-piece path is never entered, and today's single-quad paint runs exactly as it always has.

## Painting

`paint_cursor` grows a branch: when `cursor.shape == CursorShape::Block && cursor.blinking && should_show_cursor_fade(...)`, compute `fraction_through_cycle` from `Instant::now() - cycle_started_at` (the clock read once, here, in the imperative shell; everything downstream of it is a plain `f32`/`Duration` through the pure functions above), call `piece_states_at`, then `cursor_piece_bounds`, then per piece: `Empty` paints nothing; `Whole` paints a filled quad in the cursor colour; `Shade` paints a filled quad in `blend(theme.cursor, theme.bg, shade_strength)` - a new pure `blend(Hsla, Hsla, f32) -> Hsla`, converting to RGB and mixing linearly per channel before converting back, not a straight `Hsla` lerp (which would cut across the hue wheel and look like a colour shift, not a shade) - with its own test. **"Flat, not a dither"**, the engineer's own words: `blend` returns one colour for the whole piece's quad, painted as a single `fill`, never a two-colour pattern or alternating pixels - worth a one-line comment at the call site ruling out the alternative explicitly, since "medium shade" could otherwise be misread as a checkerboard.

**The character under the cursor, in the ground colour where covered** (AC#4, unchanged from the first plan): a new pure `grapheme_at(row: &Row, column: u16) -> Option<(&str, &CellStyle, u16, u16)>` (grapheme text, its style, its own run-start column, its columns-per-grapheme), reusing the same run-walk `paint_grid` already does, with its own tests (covered cell, empty cell past the line's end, a grapheme at a non-zero run-start column). When `Some`, shape that one grapheme once into the existing `RowCache` (keyed the same way `run_key` already is, so repeated frames within one unchanged character cost no reshaping at all - the continuous redraw above does not multiply this cost), then for each covered (`Whole`/`Shade`, never `Empty`) piece, `window.with_content_mask(Some(ContentMask { bounds: piece_bounds }), |window| { shaped_glyph.paint(...) })` (`window.rs:2384`) in the ground colour, clipped to that one piece's rectangle - the union of up to six clipped paints reconstructs exactly the covered area; every `Empty` piece leaves the character's own already-painted, own-coloured glyph (from the ordinary row paint) showing through untouched.

**Wide characters under the cursor**: unchanged from the first plan - the six pieces cover the head cell only; the spacer tail cell is untouched, stated as a limitation, not guessed at further (the same "not in this task" shape as the bar/underline fade question the task description already defers).

## TASK-1.100 overlap - unchanged, still for the orchestrator

Noted already on both tasks per the split's own implementation notes: this task's pure functions stay in `crates/app` with the terminal's own cell geometry, under doc-17's carve-out for the terminal view; TASK-1.100's own "Cursor" atom in `crates/ui` decides for itself whether to reuse them. Not re-litigated here.

## Files that change

- `crates/app/src/terminal_element.rs`: `piece_states_at`, `cursor_piece_bounds`, `should_show_cursor_fade`, `next_wake`, `blend`, `grapheme_at`; `paint_cursor`'s new branch; the carried-over nit fix (the stale `run_start_column` doc comment at line 248).
- `crates/app/src/main.rs`: `CursorFade` state in `AttachedState` (`cycle_started_at: Instant`, `scheduled_wake: Option<Task<()>>`); `show_cursor_whole()`; the activation-regained reset; `on_focus_in`/`on_focus_out` registration (two more `Subscription` fields); wiring `should_show_cursor_fade`'s inputs from `window.is_window_active()`, `self.focus_handle.is_focused(window)`, `state.model.borrow().cursor()`, `state.ended`, and the Reduce Motion stub; the schedule/cancel logic (`next_wake`, the spawned one-shot timer task) called from `paint_cursor`'s own call site.
- `crates/app/src/accessibility.rs` (new, small): the Reduce Motion stub, as before.
- `crates/ui/src/foundations/generated.rs`: not touched by this worker - lands once TASK-1.136 is accepted and merged; this task's own code is written against `steg_ui::foundations::MOTION_CURSOR_FADE`/`MOTION_CURSOR_FADE_PATTERN` and does not compile until they exist, which is the honest state to be in while TASK-1.136 is still in progress.

## Order of work

1. The four pure functions (`piece_states_at` parameterised, `cursor_piece_bounds`, `should_show_cursor_fade`, `next_wake`) and `blend`, each table-tested with a recorded failing-then-passing control, built against literal stand-ins for the foundations constants so this does not block on TASK-1.136 landing first.
2. `grapheme_at`, with its own tests.
3. Once TASK-1.136 is accepted and merged: wire the real foundations consts, `accessibility::reduce_motion_enabled`, the `CursorFade` state (`cycle_started_at`, `scheduled_wake`), the timer-based schedule/cancel logic and its activation/focus triggers, the key-press reset, and the painting path.
4. The doc-comment nit fix.
5. PR stills and recording.

## Risks

- **Blocked on TASK-1.136** (step 3) - in progress elsewhere, not this task's own work. Steps 1-2 do not depend on it.
- **The continuous-redraw cost is real and unmeasured here** - bounded analytically at ~9-10 wakes/second while focused and idle, not profiled; the engineer's own live run is where it is actually seen.
- **The window-activation mechanism is an internal gpui behaviour, not a public contract** - cited from source, could change between gpui versions without that being a breaking change to gpui's own public API; worth a one-line comment at the call site saying so.
- **No real control for the Reduce Motion stub flipping** - only `should_show_cursor_fade`'s own injected-`bool` test is real, same disclosed gap as the first plan.
- **The schedule's own re-arming on every paint, not only on its own wake firing**: a window that is, say, resized repeatedly while the cursor fades would re-target `scheduled_wake` on every resize-triggered paint, each time cancelling and replacing the pending timer - correct (the newly computed duration is always right for the current phase) but worth noting as the one place this plan trades a little redundant rescheduling for never needing to reason about "is the currently-pending wake still for the right boundary".
- TASK-1.100 overlap, as before - for the orchestrator.

## Proof, each criterion its own control

- **AC#1**: blocked on TASK-1.136; once landed, a test reads `MOTION_CURSOR_FADE_PATTERN.order`/`step_fraction`/`hold_fraction` directly (not literals in the test) and confirms `piece_states_at` reproduces the engineer's own table through them - control: a scratch edit of the generated constant to a wrong order/hold_fraction, watching the same test fail.
- **AC#2**: a test on `show_cursor_whole`'s own effect (touches one `Instant` field, no window) and on the cycle-start reset path; no idle-time clause to prove, per the engineer's note.
- **AC#3**: `should_show_cursor_fade`'s six isolated-gate tests, each with its own one-gate-flipped control - this is "the gate yields no wake" for each named case (window not focused, cursor hidden, no blink requested, Reduce Motion on, ended): since `next_wake`/the scheduling code is only ever called when the gate is `true` (a structural invariant, not something re-tested inside `next_wake` itself), a closed gate produces no call to it at all, not merely a call that happens to return nothing - stated so the proof is honest about what each layer actually demonstrates. On top of that, `next_wake`'s own schedule tests (above): the next-wake duration from any point in the cycle lands exactly on the next real boundary (recorded failing-then-passing control: corrupt one boundary, watch the relevant cases fail), and a full traversal of the cycle yields exactly `2*order_len + 2` wakes, not an assumed count.
- **AC#4**: `grapheme_at`'s own tests, plus the engineer's own live-run still (below).
- **AC#5**: `piece_states_at`'s own table test is the control; stills and recording below.

## AC#5's stills and the recording

Unchanged from the first plan: stills of each of the 14 states are produced by calling the pure functions directly and rendering their output to an image with no window and no app involved - never a screenshot of the running app, said plainly in the PR. The recording (the fade actually running in the real window) and a still of AC#4's ground-colour effect over real text can only come from the engineer's own run; the PR body says exactly what to capture - focus the pane, let one full 1.4s cycle play on camera, one still mid-transition over real text.

## The setting - named as a follow-up, not built here

Per the engineer's decision (d): this task builds only option 2B, with `order`/`step_fraction`/`hold_fraction`/`shade_strength` as data (from TASK-1.136) rather than literals, so a second variant is new data plus a selector, not a rewrite. The setting itself (choosing a variant, remembering it, surfacing it from the palette per R80) is out of this task's own scope - recommending a follow-up task be filed for it, not filing one here (board changes are the orchestrator's).
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Split by @bruk.o on 2026-11-18, as the plan recommended: the design values and the generator's whole-number lists moved to a draft task, 'ui: the cursor fade's named values, exported from the design, and whole-number lists in the generator', which this task will depend on once it is accepted. This task keeps the pure functions, the driver, the painting and all five criteria; its first criterion is met by reading the values that task exports. On the overlap with TASK-1.100 (the Cursor atom in crates/ui): the plan keeps the fade's pure functions in crates/app with the terminal's cell geometry, under doc-17's carve-out for the terminal view; TASK-1.100 decides for itself whether to reuse them. The orchestrator agrees and has noted it on TASK-1.100.

Engineer's decisions of 2026-11-20 (recorded by @bruk.o), design artifact version 143 (id 1795195942-6a0a), Foundations page section 12 and the Cursor atom: (a) the animation is option 2B, as the Cursor atom already draws it: cycle 1400 ms; six pieces row-major 0 to 5; out in the order 3, 0, 4, 2, 1, 5 and back in reverse; each step 3 percent of the cycle; a piece spends one step as medium shade each way, medium shade being the flat colour half way between the cursor colour and the ground (not a dither); whole 43 percent, going 18, empty 21, returning 18. (b) There is NO idle rest: the fade runs for as long as the window has focus and stops, whole, when the window loses focus. Any idle-time value or criterion clause is dropped. (c) Reduce Motion is a stub that always allows motion until TASK-1.63, with the steady cursor built behind it. (d) The animation is meant to become a setting later: build only 2B, but hold the animation as data (order, step, shade) so another variant is an addition, not a rewrite; the setting itself is a separate follow-up task.

Revised plan checked against doc-17 (R120 to R123), the design's Cursor atom and Foundations section 12 (artifact version 143) and the engineer's decisions of 2026-11-20 by @bruk.o: ok. Option 2B with its values read from foundations (TASK-1.136), no idle rest, stops whole on losing focus, Reduce Motion stub, animation held as parameters. The driver is a one-shot GPUI timer to the next step boundary, re-armed only from a paint with the gate open and dropped when it closes: about 9 to 10 repaints a second while focused, none otherwise; the cost is reasoned, not measured. Regaining focus restarts the cycle from whole: decided by the orchestrator, for the engineer to overrule. Known limit for review: that a closed gate arms nothing is argued from structure and the gate function's own tests, not shown by a test of the scheduling itself. Build waits on TASK-1.136 merging.

Name to read from foundations, settled by @bruk.o on 2026-11-20: the whole-phase value this plan calls hold_fraction is exported by TASK-1.136 as phase_whole, in the entry motion.cursor-fade.shaded-dissolve. Reading it under that name is not a departure from this plan.

Check FAILED: `cargo test -p steg-app piece_states_at_matches_the_engineers_own_table` exited 101 at 2026-11-22T04:27:02+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.127 on task-1.127-app-the-cursor-dissolves-and-re-forms at 8ad1a1bd9cf4 with uncommitted changes:

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.24s
warning: the following packages contain code that will be rejected by a future version of Rust: block v0.1.6, proc-macro-error2 v2.0.1
note: to see what the problems were, use the option `--future-incompat-report`, or run `cargo report future-incompatibilities --id 1`
     Running unittests src/main.rs (target/debug/deps/steg_app-edc2b453f94d193e)

running 1 test
test terminal_element::tests::piece_states_at_matches_the_engineers_own_table ... FAILED

failures:

---- terminal_element::tests::piece_states_at_matches_the_engineers_own_table stdout ----

thread 'terminal_element::tests::piece_states_at_matches_the_engineers_own_table' (74919940) panicked at crates/app/src/terminal_element.rs:1301:13:
assertion `left == right` failed: at fraction 0.43
  left: [Shade, Shade, Shade, Shade, Shade, Shade]
 right: [Whole, Whole, Whole, Shade, Whole, Whole]
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    terminal_element::tests::piece_states_at_matches_the_engineers_own_table

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 98 filtered out; finished in 0.00s

error: test failed, to rerun pass `-p steg-app --bin steg-app`
```

Check passed: `cargo test -p steg-app piece_states_at_matches_the_engineers_own_table` exited 0 at 2026-11-22T04:27:19+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.127 on task-1.127-app-the-cursor-dissolves-and-re-forms at 8ad1a1bd9cf4 with uncommitted changes:

```text
   Compiling steg-app v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.127/crates/app)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.83s
warning: the following packages contain code that will be rejected by a future version of Rust: block v0.1.6, proc-macro-error2 v2.0.1
note: to see what the problems were, use the option `--future-incompat-report`, or run `cargo report future-incompatibilities --id 1`
     Running unittests src/main.rs (target/debug/deps/steg_app-edc2b453f94d193e)

running 1 test
test terminal_element::tests::piece_states_at_matches_the_engineers_own_table ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 98 filtered out; finished in 0.00s
```

Check FAILED: `cargo test -p steg-app the_bottom_right_piece_reaches_the_cells_own_true_far_edge` exited 101 at 2026-11-22T04:27:58+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.127 on task-1.127-app-the-cursor-dissolves-and-re-forms at 8ad1a1bd9cf4 with uncommitted changes:

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.22s
warning: the following packages contain code that will be rejected by a future version of Rust: block v0.1.6, proc-macro-error2 v2.0.1
note: to see what the problems were, use the option `--future-incompat-report`, or run `cargo report future-incompatibilities --id 1`
     Running unittests src/main.rs (target/debug/deps/steg_app-edc2b453f94d193e)

running 1 test
test terminal_element::tests::the_bottom_right_piece_reaches_the_cells_own_true_far_edge ... FAILED

failures:

---- terminal_element::tests::the_bottom_right_piece_reaches_the_cells_own_true_far_edge stdout ----

thread 'terminal_element::tests::the_bottom_right_piece_reaches_the_cells_own_true_far_edge' (74926851) panicked at crates/app/src/terminal_element.rs:1370:9:
assertion `left == right` failed
  left: Point { x: 118px, y: 70px }
 right: Point { x: 119px, y: 70px }
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    terminal_element::tests::the_bottom_right_piece_reaches_the_cells_own_true_far_edge

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 98 filtered out; finished in 0.00s

error: test failed, to rerun pass `-p steg-app --bin steg-app`
```

Check FAILED: `cargo test -p steg-app adjacent_pieces_share_an_exact_edge_at_an_odd_cell_size` exited 101 at 2026-11-22T04:28:02+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.127 on task-1.127-app-the-cursor-dissolves-and-re-forms at 8ad1a1bd9cf4 with uncommitted changes:

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.24s
warning: the following packages contain code that will be rejected by a future version of Rust: block v0.1.6, proc-macro-error2 v2.0.1
note: to see what the problems were, use the option `--future-incompat-report`, or run `cargo report future-incompatibilities --id 1`
     Running unittests src/main.rs (target/debug/deps/steg_app-edc2b453f94d193e)

running 1 test
test terminal_element::tests::adjacent_pieces_share_an_exact_edge_at_an_odd_cell_size ... FAILED

failures:

---- terminal_element::tests::adjacent_pieces_share_an_exact_edge_at_an_odd_cell_size stdout ----

thread 'terminal_element::tests::adjacent_pieces_share_an_exact_edge_at_an_odd_cell_size' (74928003) panicked at crates/app/src/terminal_element.rs:1357:9:
assertion `left == right` failed
  left: 114.0
 right: 133.0
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    terminal_element::tests::adjacent_pieces_share_an_exact_edge_at_an_odd_cell_size

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 98 filtered out; finished in 0.00s

error: test failed, to rerun pass `-p steg-app --bin steg-app`
```

Check passed: `cargo test -p steg-app the_bottom_right_piece_reaches_the_cells_own_true_far_edge` exited 0 at 2026-11-22T04:28:19+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.127 on task-1.127-app-the-cursor-dissolves-and-re-forms at 8ad1a1bd9cf4 with uncommitted changes:

```text
   Compiling steg-app v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.127/crates/app)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.88s
warning: the following packages contain code that will be rejected by a future version of Rust: block v0.1.6, proc-macro-error2 v2.0.1
note: to see what the problems were, use the option `--future-incompat-report`, or run `cargo report future-incompatibilities --id 1`
     Running unittests src/main.rs (target/debug/deps/steg_app-edc2b453f94d193e)

running 1 test
test terminal_element::tests::the_bottom_right_piece_reaches_the_cells_own_true_far_edge ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 98 filtered out; finished in 0.00s
```

Check passed: `cargo test -p steg-app adjacent_pieces_share_an_exact_edge_at_an_odd_cell_size` exited 0 at 2026-11-22T04:28:23+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.127 on task-1.127-app-the-cursor-dissolves-and-re-forms at 8ad1a1bd9cf4 with uncommitted changes:

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.24s
warning: the following packages contain code that will be rejected by a future version of Rust: block v0.1.6, proc-macro-error2 v2.0.1
note: to see what the problems were, use the option `--future-incompat-report`, or run `cargo report future-incompatibilities --id 1`
     Running unittests src/main.rs (target/debug/deps/steg_app-edc2b453f94d193e)

running 1 test
test terminal_element::tests::adjacent_pieces_share_an_exact_edge_at_an_odd_cell_size ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 98 filtered out; finished in 0.00s
```

Check FAILED: `cargo test -p steg-app the_fade_does_not_show_when_reduce_motion_is_on` exited 101 at 2026-11-22T04:28:42+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.127 on task-1.127-app-the-cursor-dissolves-and-re-forms at 8ad1a1bd9cf4 with uncommitted changes:

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.24s
warning: the following packages contain code that will be rejected by a future version of Rust: block v0.1.6, proc-macro-error2 v2.0.1
note: to see what the problems were, use the option `--future-incompat-report`, or run `cargo report future-incompatibilities --id 1`
     Running unittests src/main.rs (target/debug/deps/steg_app-edc2b453f94d193e)

running 1 test
test terminal_element::tests::the_fade_does_not_show_when_reduce_motion_is_on ... FAILED

failures:

---- terminal_element::tests::the_fade_does_not_show_when_reduce_motion_is_on stdout ----

thread 'terminal_element::tests::the_fade_does_not_show_when_reduce_motion_is_on' (74933758) panicked at crates/app/src/terminal_element.rs:1438:9:
assertion failed: !should_show_cursor_fade(true, true, true, true, true, false)
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    terminal_element::tests::the_fade_does_not_show_when_reduce_motion_is_on

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 98 filtered out; finished in 0.00s

error: test failed, to rerun pass `-p steg-app --bin steg-app`
```

Check passed: `cargo test -p steg-app the_fade_` exited 0 at 2026-11-22T04:28:59+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.127 on task-1.127-app-the-cursor-dissolves-and-re-forms at 8ad1a1bd9cf4 with uncommitted changes:

```text
   Compiling steg-app v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.127/crates/app)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.92s
warning: the following packages contain code that will be rejected by a future version of Rust: block v0.1.6, proc-macro-error2 v2.0.1
note: to see what the problems were, use the option `--future-incompat-report`, or run `cargo report future-incompatibilities --id 1`
     Running unittests src/main.rs (target/debug/deps/steg_app-edc2b453f94d193e)

running 7 tests
test terminal_element::tests::the_fade_does_not_show_when_the_pane_is_not_focused ... ok
test terminal_element::tests::the_fade_does_not_show_once_the_pane_has_ended ... ok
test terminal_element::tests::the_fade_does_not_show_when_the_cursor_is_hidden ... ok
test terminal_element::tests::the_fade_does_not_show_when_reduce_motion_is_on ... ok
test terminal_element::tests::the_fade_does_not_show_when_the_program_has_not_asked_for_blinking ... ok
test terminal_element::tests::the_fade_does_not_show_when_the_window_is_not_active ... ok
test terminal_element::tests::the_fade_shows_when_every_gate_is_open ... ok

test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 92 filtered out; finished in 0.00s
```

Check FAILED: `cargo test -p steg-app a_full_traversal_of_the_cycle_yields_exactly_the_expected_number_of_wakes` exited 101 at 2026-11-22T04:29:23+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.127 on task-1.127-app-the-cursor-dissolves-and-re-forms at 8ad1a1bd9cf4 with uncommitted changes:

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.21s
warning: the following packages contain code that will be rejected by a future version of Rust: block v0.1.6, proc-macro-error2 v2.0.1
note: to see what the problems were, use the option `--future-incompat-report`, or run `cargo report future-incompatibilities --id 1`
     Running unittests src/main.rs (target/debug/deps/steg_app-edc2b453f94d193e)

running 1 test
test terminal_element::tests::a_full_traversal_of_the_cycle_yields_exactly_the_expected_number_of_wakes ... FAILED

failures:

---- terminal_element::tests::a_full_traversal_of_the_cycle_yields_exactly_the_expected_number_of_wakes stdout ----

thread 'terminal_element::tests::a_full_traversal_of_the_cycle_yields_exactly_the_expected_number_of_wakes' (74938484) panicked at crates/app/src/terminal_element.rs:1531:9:
assertion `left == right` failed
  left: 9
 right: 14
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    terminal_element::tests::a_full_traversal_of_the_cycle_yields_exactly_the_expected_number_of_wakes

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 98 filtered out; finished in 0.00s

error: test failed, to rerun pass `-p steg-app --bin steg-app`
```

Check passed: `cargo test -p steg-app next_wake` exited 0 at 2026-11-22T04:29:36+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.127 on task-1.127-app-the-cursor-dissolves-and-re-forms at 8ad1a1bd9cf4 with uncommitted changes:

```text
   Compiling steg-app v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.127/crates/app)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.93s
warning: the following packages contain code that will be rejected by a future version of Rust: block v0.1.6, proc-macro-error2 v2.0.1
note: to see what the problems were, use the option `--future-incompat-report`, or run `cargo report future-incompatibilities --id 1`
     Running unittests src/main.rs (target/debug/deps/steg_app-edc2b453f94d193e)

running 3 tests
test terminal_element::tests::next_wake_from_just_before_the_wrap_lands_on_the_cycles_own_last_boundary ... ok
test terminal_element::tests::next_wake_falls_back_to_one_full_turn_when_nothing_is_strictly_ahead ... ok
test terminal_element::tests::next_wake_lands_exactly_on_the_next_boundary ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 96 filtered out; finished in 0.00s
```

Check passed: `cargo test -p steg-app a_full_traversal_of_the_cycle_yields_exactly_the_expected_number_of_wakes` exited 0 at 2026-11-22T04:29:46+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.127 on task-1.127-app-the-cursor-dissolves-and-re-forms at 8ad1a1bd9cf4 with uncommitted changes:

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.24s
warning: the following packages contain code that will be rejected by a future version of Rust: block v0.1.6, proc-macro-error2 v2.0.1
note: to see what the problems were, use the option `--future-incompat-report`, or run `cargo report future-incompatibilities --id 1`
     Running unittests src/main.rs (target/debug/deps/steg_app-edc2b453f94d193e)

running 1 test
test terminal_element::tests::a_full_traversal_of_the_cycle_yields_exactly_the_expected_number_of_wakes ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 98 filtered out; finished in 0.00s
```

Check passed: `cargo test -p steg-app the_wake_count_follows_order_len_not_a_fixed_constant` exited 0 at 2026-11-22T04:29:50+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.127 on task-1.127-app-the-cursor-dissolves-and-re-forms at 8ad1a1bd9cf4 with uncommitted changes:

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.24s
warning: the following packages contain code that will be rejected by a future version of Rust: block v0.1.6, proc-macro-error2 v2.0.1
note: to see what the problems were, use the option `--future-incompat-report`, or run `cargo report future-incompatibilities --id 1`
     Running unittests src/main.rs (target/debug/deps/steg_app-edc2b453f94d193e)

running 1 test
test terminal_element::tests::the_wake_count_follows_order_len_not_a_fixed_constant ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 98 filtered out; finished in 0.00s
```

Check FAILED: `cargo test -p steg-app blend_at_half_strength_is_the_rgb_midpoint` exited 101 at 2026-11-22T04:30:10+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.127 on task-1.127-app-the-cursor-dissolves-and-re-forms at 8ad1a1bd9cf4 with uncommitted changes:

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.23s
warning: the following packages contain code that will be rejected by a future version of Rust: block v0.1.6, proc-macro-error2 v2.0.1
note: to see what the problems were, use the option `--future-incompat-report`, or run `cargo report future-incompatibilities --id 1`
     Running unittests src/main.rs (target/debug/deps/steg_app-edc2b453f94d193e)

running 1 test
test terminal_element::tests::blend_at_half_strength_is_the_rgb_midpoint ... FAILED

failures:

---- terminal_element::tests::blend_at_half_strength_is_the_rgb_midpoint stdout ----

thread 'terminal_element::tests::blend_at_half_strength_is_the_rgb_midpoint' (74945306) panicked at crates/app/src/terminal_element.rs:1573:9:
r was 0
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    terminal_element::tests::blend_at_half_strength_is_the_rgb_midpoint

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 98 filtered out; finished in 0.00s

error: test failed, to rerun pass `-p steg-app --bin steg-app`
```

Check passed: `cargo test -p steg-app blend` exited 0 at 2026-11-22T04:30:22+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.127 on task-1.127-app-the-cursor-dissolves-and-re-forms at 8ad1a1bd9cf4 with uncommitted changes:

```text
   Compiling steg-app v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.127/crates/app)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.97s
warning: the following packages contain code that will be rejected by a future version of Rust: block v0.1.6, proc-macro-error2 v2.0.1
note: to see what the problems were, use the option `--future-incompat-report`, or run `cargo report future-incompatibilities --id 1`
     Running unittests src/main.rs (target/debug/deps/steg_app-edc2b453f94d193e)

running 2 tests
test terminal_element::tests::blend_at_half_strength_is_the_rgb_midpoint ... ok
test terminal_element::tests::blend_at_zero_strength_leaves_the_base_colour_unchanged ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 97 filtered out; finished in 0.00s
```

Check FAILED: `cargo test -p steg-app grapheme_at_finds_a_grapheme_at_a_non_zero_run_start_column` exited 101 at 2026-11-22T04:30:42+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.127 on task-1.127-app-the-cursor-dissolves-and-re-forms at 8ad1a1bd9cf4 with uncommitted changes:

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.23s
warning: the following packages contain code that will be rejected by a future version of Rust: block v0.1.6, proc-macro-error2 v2.0.1
note: to see what the problems were, use the option `--future-incompat-report`, or run `cargo report future-incompatibilities --id 1`
     Running unittests src/main.rs (target/debug/deps/steg_app-edc2b453f94d193e)

running 1 test
test terminal_element::tests::grapheme_at_finds_a_grapheme_at_a_non_zero_run_start_column ... FAILED

failures:

---- terminal_element::tests::grapheme_at_finds_a_grapheme_at_a_non_zero_run_start_column stdout ----

thread 'terminal_element::tests::grapheme_at_finds_a_grapheme_at_a_non_zero_run_start_column' (74949569) panicked at crates/app/src/terminal_element.rs:1616:87:
called `Option::unwrap()` on a `None` value
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    terminal_element::tests::grapheme_at_finds_a_grapheme_at_a_non_zero_run_start_column

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 98 filtered out; finished in 0.00s

error: test failed, to rerun pass `-p steg-app --bin steg-app`
```

Check passed: `cargo test -p steg-app grapheme_at` exited 0 at 2026-11-22T04:30:53+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.127 on task-1.127-app-the-cursor-dissolves-and-re-forms at 8ad1a1bd9cf4 with uncommitted changes:

```text
   Compiling steg-app v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.127/crates/app)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.85s
warning: the following packages contain code that will be rejected by a future version of Rust: block v0.1.6, proc-macro-error2 v2.0.1
note: to see what the problems were, use the option `--future-incompat-report`, or run `cargo report future-incompatibilities --id 1`
     Running unittests src/main.rs (target/debug/deps/steg_app-edc2b453f94d193e)

running 5 tests
test terminal_element::tests::grapheme_at_an_empty_row_is_none ... ok
test terminal_element::tests::grapheme_at_past_the_lines_own_end_is_none ... ok
test terminal_element::tests::grapheme_at_finds_a_grapheme_at_a_non_zero_run_start_column ... ok
test terminal_element::tests::grapheme_at_finds_the_grapheme_in_a_single_run_row ... ok
test terminal_element::tests::grapheme_at_a_wide_character_reports_two_columns_per_grapheme ... ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 94 filtered out; finished in 0.00s
```

Check FAILED: `cargo test -p steg-app show_whole_resets_the_cycle_and_drops_any_pending_wake` exited 101 at 2026-11-22T04:31:12+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.127 on task-1.127-app-the-cursor-dissolves-and-re-forms at 8ad1a1bd9cf4 with uncommitted changes:

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.23s
warning: the following packages contain code that will be rejected by a future version of Rust: block v0.1.6, proc-macro-error2 v2.0.1
note: to see what the problems were, use the option `--future-incompat-report`, or run `cargo report future-incompatibilities --id 1`
     Running unittests src/main.rs (target/debug/deps/steg_app-edc2b453f94d193e)

running 1 test
test terminal_element::tests::show_whole_resets_the_cycle_and_drops_any_pending_wake ... FAILED

failures:

---- terminal_element::tests::show_whole_resets_the_cycle_and_drops_any_pending_wake stdout ----

thread 'terminal_element::tests::show_whole_resets_the_cycle_and_drops_any_pending_wake' (74953607) panicked at crates/app/src/terminal_element.rs:1670:9:
assertion failed: state.scheduled_wake.is_none()
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    terminal_element::tests::show_whole_resets_the_cycle_and_drops_any_pending_wake

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 98 filtered out; finished in 0.00s

error: test failed, to rerun pass `-p steg-app --bin steg-app`
```

Check passed: `cargo test -p steg-app show_whole_resets_the_cycle_and_drops_any_pending_wake` exited 0 at 2026-11-22T04:31:24+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.127 on task-1.127-app-the-cursor-dissolves-and-re-forms at 8ad1a1bd9cf4 with uncommitted changes:

```text
   Compiling steg-app v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.127/crates/app)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.86s
warning: the following packages contain code that will be rejected by a future version of Rust: block v0.1.6, proc-macro-error2 v2.0.1
note: to see what the problems were, use the option `--future-incompat-report`, or run `cargo report future-incompatibilities --id 1`
     Running unittests src/main.rs (target/debug/deps/steg_app-edc2b453f94d193e)

running 1 test
test terminal_element::tests::show_whole_resets_the_cycle_and_drops_any_pending_wake ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 98 filtered out; finished in 0.00s
```
<!-- SECTION:NOTES:END -->

## Comments

<!-- COMMENTS:BEGIN -->
created: 2026-11-17T21:24:00Z

---

Carry-over from TASK-1.126's second review (nit, not fixed there because the task was locked on passing): the doc comment above shape_and_check_run in crates/app/src/terminal_element.rs still describes a run_start_column parameter the function no longer takes. Correct it when this task touches the file. - @bruk.o

---

author: @bruk
created: 2026-11-21T19:55:00Z

---

Plan approved by @bruk (e3032b0ac5e3, size-3): its worker may build it.

---
<!-- COMMENTS:END -->
