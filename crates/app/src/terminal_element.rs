//! Paints the fixed grid read-only (TASK-1.117's plan §2): a window's own background in
//! the terminal theme's own `bg`, the grid from its top-left origin at its natural pixel
//! size, letterboxed if the window is bigger, cropped if smaller. Written fresh for this
//! crate, from gpui's own `canvas`/`ShapedLine` API and rustdoc - no GPL code read or
//! copied (R59; `.claude/rules/licence.md`); the general shape (a per-row shaping cache,
//! painting runs at `origin + cell_width * column`, a cursor quad) follows steg's own
//! prior MIT spike, `spikes/task-1.1-terminal-library/probe-core/src/element.rs`, which
//! already proved this technique on this exact pinned gpui version (decision-3).
//!
//! **Every cell its own column (TASK-1.126's plan).** Shaping a whole run as one
//! `ShapedLine` and trusting the font's own advances to position every glyph after the
//! first is only safe when every glyph in it actually lands on its own column boundary -
//! true for plain ASCII in a genuinely monospace font, not guaranteed for anything that
//! can trigger a glyph-level fallback (`Font.fallbacks`, metrics.rs's module doc), a wide
//! character, or any font whose advance is not exactly the cell width. The invariant this
//! task's AC#3 names - a glyph narrower or wider than a cell cannot move what follows it -
//! holds unconditionally, not only in that common case: [`shape_and_check_run`] shapes the
//! run once, then checks every glyph `ShapedLine` exposes (it `#[deref]`s to
//! `Arc<LineLayout>`, confirmed in gpui's own `text_system/line.rs`/`line_layout.rs`)
//! against the column its own grapheme owns. If every glyph already sits where the grid
//! says it must, the single shaped line is painted as one call - an optimisation guarded
//! by a runtime check that the invariant already holds for that specific run, not a quiet
//! exception to it. If any glyph does not, the run is re-shaped and painted grapheme by
//! grapheme instead, each its own tiny shaped line at its own column's explicit origin -
//! correct, at the cost of one shape/paint call per grapheme rather than one per run;
//! [`RowCache`] caches whichever of the two a run actually needed, so a cache hit never
//! replays the wrong path for the wrong run.
//!
//! **A run's own column never enters this check (second review finding).** The check
//! compares each glyph's position against the column its own grapheme owns *within the
//! run*, never the run's absolute column on the row: `shape_and_check_run` shapes only the
//! run's own text, never the row, so gpui's own glyph positions are already relative to the
//! run's start (confirmed in gpui's `line_layout.rs`). The row's absolute column is
//! [`cell_origin`]'s job alone, applied separately when painting. A first version of this
//! mixed the two spaces in, so any run not starting at column 0 - an ordinary colour
//! change mid-line, say - always "failed" the check and took the slow path regardless of
//! whether it was actually safe; verified fixed with a scratch debug count on a real line
//! mixing several mid-row, differently-coloured ASCII runs with a wide CJK character and an
//! emoji sharing one run: every ASCII run, wherever it sat on the row, took the fast,
//! single-line path, and only the run needing the fallback took the slow one.

use std::cell::RefCell;
use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash as _, Hasher as _};
use std::rc::Rc;
use std::time::{Duration, Instant};

use gpui::{
    App, AsyncWindowContext, Bounds, ContentMask, ElementInputHandler, Entity, FocusHandle,
    FontStyle, FontWeight, Hsla, IntoElement, ParentElement as _, Pixels, Point, ShapedLine,
    SharedString, StrikethroughStyle, Styled as _, Task, TextRun, UnderlineStyle, Window, canvas,
    div, fill, point, px,
};
use steg_sdk::{CellStyle, Cursor, CursorShape, Rgb, Row, Run, Underline, Wide};
use steg_ui::foundations::TerminalTheme;
use unicode_segmentation::UnicodeSegmentation as _;

use crate::AppWindow;
use crate::accessibility::reduce_motion_enabled;
use crate::metrics::CellMetrics;
use crate::model::{Model, run_columns};

fn rgb_to_hsla(rgb: Rgb) -> Hsla {
    gpui::rgb(u32::from(rgb.r) << 16 | u32::from(rgb.g) << 8 | u32::from(rgb.b)).into()
}

/// How a run was painted, kept exactly as painted - so a cache hit replays the same path,
/// never mixing the fast path's single line with the slow path's per-grapheme one (module
/// doc).
#[derive(Clone)]
enum CachedRun {
    /// Every glyph already sat on its own column - the whole run painted as one line.
    /// Boxed: `ShapedLine` itself is large (its own `SmallVec` of decoration runs), and
    /// this variant would otherwise force every `PerGrapheme` clone to carry that size too.
    Whole(Box<ShapedLine>),
    /// At least one glyph did not - one tiny shaped line per grapheme, in the run's own
    /// left-to-right order, each painted at its own column (module doc).
    PerGrapheme(Rc<[ShapedLine]>),
}

/// Shaped rows kept between frames, so a row that has not changed costs no reshaping even
/// though the whole grid repaints every frame (gpui 0.2.2 has no partial-repaint primitive
/// for a `canvas` element - TASK-1.117's plan §2).
#[derive(Default)]
pub(crate) struct RowCache {
    lines: HashMap<u64, CachedRun>,
}

impl RowCache {
    fn painted(&mut self, key: u64, build: impl FnOnce() -> CachedRun) -> CachedRun {
        if let Some(cached) = self.lines.get(&key) {
            return cached.clone();
        }
        if self.lines.len() > 4096 {
            self.lines.clear();
        }
        let cached = build();
        self.lines.insert(key, cached.clone());
        cached
    }
}

/// Renders the fixed grid as one element: the window's own background first, then the
/// grid from the model, cropped to whatever of it fits `bounds` (TASK-1.117's plan §2).
///
/// `focus_handle`/`entity` are TASK-1.121's own addition, for the one call in this crate
/// that must run from inside a real paint phase: `window.handle_input` ("should only be
/// called as part of the paint phase of element drawing", `input.rs`'s own doc) registers
/// the `EntityInputHandler` `AppWindow` implements, which is what lets a dead key, the
/// emoji picker or any other IME composition reach the program at all (`main.rs`'s own
/// `EntityInputHandler` impl; `keys.rs`'s own doc comment on the dedup this and
/// `on_key_down` - registered declaratively, in `main.rs`'s `render`, not here - together
/// guarantee). `last_bounds` is this element's own last-painted bounds, read back by
/// `AppWindow::bounds_for_range` to anchor the IME's candidate window at the terminal's own
/// cursor cell - `None` only before the first paint.
/// `cursor_fade`/`ended` are TASK-1.127's own addition - `cursor_fade` is this window's own
/// fade state (below), `ended` is whether the pane has ended (TASK-1.129), one of the
/// fade's own gates.
#[expect(
    clippy::too_many_arguments,
    reason = "each argument is a distinct, named input the grid, the input handler and the \
              cursor fade each need; bundling them would not make any of them optional or \
              grouped by a real relationship between them"
)]
pub fn terminal(
    model: Rc<RefCell<Model>>,
    metrics: CellMetrics,
    theme: TerminalTheme,
    row_cache: Rc<RefCell<RowCache>>,
    focus_handle: FocusHandle,
    entity: Entity<AppWindow>,
    last_bounds: Rc<std::cell::Cell<Option<Bounds<Pixels>>>>,
    cursor_fade: Rc<RefCell<CursorFadeState>>,
    ended: bool,
) -> impl IntoElement {
    div().size_full().bg(theme.bg).child(
        canvas(
            move |_bounds, _window, _cx| (),
            move |bounds, (), window, cx| {
                window.handle_input(
                    &focus_handle,
                    ElementInputHandler::new(bounds, entity.clone()),
                    cx,
                );
                last_bounds.set(Some(bounds));
                paint_grid(
                    &model.borrow(),
                    &metrics,
                    &theme,
                    &row_cache,
                    bounds,
                    window,
                    cx,
                );
                paint_cursor(
                    &model.borrow(),
                    &metrics,
                    &theme,
                    bounds,
                    window,
                    cx,
                    &row_cache,
                    &cursor_fade,
                    &focus_handle,
                    ended,
                    &entity,
                );
            },
        )
        // `Canvas` carries its own `StyleRefinement`, empty by default (confirmed from
        // gpui's own `elements/canvas.rs`) - the wrapping div's own `size_full` does not
        // propagate to it. Found by running the real app: the canvas's own bounds came
        // back `0px` tall until this was added, which is why nothing painted at all.
        .size_full(),
    )
}

/// A fresh, empty cache - one per window, created once (TASK-1.117's plan's own file list:
/// `terminal_element.rs` owns this cache's type).
pub fn row_cache() -> Rc<RefCell<RowCache>> {
    Rc::new(RefCell::new(RowCache::default()))
}

/// A fresh cursor-fade state - one per window, created once, alongside [`row_cache`].
pub fn cursor_fade_state() -> Rc<RefCell<CursorFadeState>> {
    Rc::new(RefCell::new(CursorFadeState::default()))
}

fn paint_grid(
    model: &Model,
    metrics: &CellMetrics,
    theme: &TerminalTheme,
    row_cache: &Rc<RefCell<RowCache>>,
    bounds: Bounds<Pixels>,
    window: &mut Window,
    cx: &mut App,
) {
    let visible_rows = metrics.visible_rows(bounds, model.row_count());
    let visible_columns = metrics.visible_columns(bounds);

    for (index, row) in model
        .rows()
        .iter()
        .enumerate()
        .take(usize::from(visible_rows))
    {
        #[expect(
            clippy::cast_possible_truncation,
            reason = "a row index on a terminal screen never approaches u16::MAX"
        )]
        let row_index = index as u16;

        let mut column: u16 = 0;
        for run in row {
            let width = run_columns(run);
            if column >= visible_columns {
                break;
            }
            let text_run = to_text_run(run, theme, &metrics.family);
            if text_run.len != 0 {
                let key = run_key(index, column, run);
                let per_grapheme_width = columns_per_grapheme(run);
                let cached = row_cache.borrow_mut().painted(key, || {
                    shape_and_check_run(
                        window,
                        &run.text,
                        text_run,
                        metrics.cell_width,
                        per_grapheme_width,
                    )
                });
                let placement = RunPlacement {
                    start_column: column,
                    columns_per_grapheme: per_grapheme_width,
                    row: row_index,
                };
                if let Err(error) =
                    paint_cached_run(&cached, &placement, bounds.origin, metrics, window, cx)
                {
                    eprintln!("painting a run failed: {error}");
                }
            }
            column = column.saturating_add(width);
        }
    }
}

/// How many columns one grapheme of `run` spans - `run_columns`'s own per-run total
/// divided evenly across its graphemes, since a `Run`'s own `style.wide` applies to every
/// grapheme in it alike (`run_columns`'s own doc: one style, one width, for the whole run).
fn columns_per_grapheme(run: &Run) -> u16 {
    match run.style.wide {
        Wide::Wide => 2,
        Wide::Narrow | Wide::SpacerHead | Wide::SpacerTail => 1,
    }
}

/// The pixel origin of the cell at (`column`, `row`) - the one place that arithmetic is
/// done, shared by every caller that places something on the grid (module doc; TASK-1.126's
/// plan names this the seam TASK-1.127's own cursor-fade work builds on next, kept clean of
/// how anything is actually painted).
#[must_use]
fn cell_origin(
    origin: Point<Pixels>,
    cell_width: Pixels,
    line_height: Pixels,
    column: u16,
    row: u16,
) -> Point<Pixels> {
    point(
        origin.x + cell_width * f32::from(column),
        origin.y + line_height * f32::from(row),
    )
}

/// The state of one of the cursor's six cell pieces at a point in its fade cycle
/// (TASK-1.127's plan, "Painting"; the design's own Cursor atom, option 2B,
/// `steg_ui::foundations::MOTION_CURSOR_FADE_SHADED_DISSOLVE`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PieceState {
    /// The cursor's own colour.
    Whole,
    /// The flat colour half way between the cursor colour and the ground (the engineer's
    /// own wording: "not a dither") - one step on the way out, one on the way back.
    Shade,
    /// Nothing painted for this piece - the character under it, if any, shows through in
    /// its own colour (module doc, "Painting").
    Empty,
}

/// The state of all six of the cursor's cell pieces at `fraction_through_cycle` (`0.0` to
/// `1.0`, already reduced mod 1 by the caller). Pieces are row-major, `0` top-left to `5`
/// bottom-right; `order` (`MOTION_CURSOR_FADE_SHADED_DISSOLVE.order`) names which piece
/// goes out at which rank, `0` first, and returns in the reverse order. Every boundary is
/// derived from `order`, `step_fraction`, `phase_whole` and `phase_returning` - never six
/// literal percentages written here - so a second variant (a different order, step or
/// phase) is new data, not a rewrite of this function (TASK-1.127's plan, "hold the
/// animation as data").
///
/// For the piece of rank `r` (`0` to `order.len() - 1`): whole until
/// `phase_whole + r*step_fraction`; shade for one `step_fraction`; empty until
/// `(1 - phase_returning) + (order.len() - 1 - r)*step_fraction`; shade for one more
/// `step_fraction`; whole again from there. Table-tested against the engineer's own
/// table (every boundary, both edges of each half-open interval).
#[must_use]
fn piece_states_at(
    fraction_through_cycle: f32,
    order: &[i32; 6],
    step_fraction: f32,
    phase_whole: f32,
    phase_returning: f32,
) -> [PieceState; 6] {
    #[expect(
        clippy::cast_precision_loss,
        reason = "six pieces, nowhere near f32's precision limit"
    )]
    let piece_count = order.len() as f32;
    let return_start = 1.0 - phase_returning;
    let mut states = [PieceState::Whole; 6];
    for piece in 0_i32..6 {
        // `order` not naming every piece from 0 to 5 exactly once is a data error outside
        // this function's own reach (the exported constant's own shape, not something it
        // re-validates) - left `Whole`, the least surprising default, rather than panic.
        let Some(rank) = order.iter().position(|&p| p == piece) else {
            continue;
        };
        #[expect(
            clippy::cast_precision_loss,
            reason = "rank is 0..=5, nowhere near f32's precision limit"
        )]
        let rank = rank as f32;
        let shade_out_start = phase_whole + rank * step_fraction;
        let shade_out_end = shade_out_start + step_fraction;
        let shade_back_start = return_start + (piece_count - 1.0 - rank) * step_fraction;
        let shade_back_end = return_start + (piece_count - rank) * step_fraction;
        let state = if fraction_through_cycle < shade_out_start {
            PieceState::Whole
        } else if fraction_through_cycle < shade_out_end {
            PieceState::Shade
        } else if fraction_through_cycle < shade_back_start {
            PieceState::Empty
        } else if fraction_through_cycle < shade_back_end {
            PieceState::Shade
        } else {
            PieceState::Whole
        };
        #[expect(
            clippy::cast_sign_loss,
            reason = "piece is always 0..6, built from that exact range above"
        )]
        let piece_index = piece as usize;
        states[piece_index] = state;
    }
    states
}

/// The six rectangles the cursor's own cell is divided into for the fade (TASK-1.127's
/// plan, "Painting"): two columns, three rows, row-major `0` top-left to `5` bottom-right.
/// Every column/row edge is computed once and rounded once (`floor`, matching
/// [`cell_origin`]'s own convention of whole-pixel placement), and every rectangle is built
/// from two of those already-rounded, adjacent edges - never a rounded half-width or
/// third-height doubled or tripled, which is exactly the kind of arithmetic that leaves a
/// seam or a gap when `cell_width`/`line_height` do not divide evenly by 2 or 3 (true at
/// the design's own 20px line height: `20.0 / 3.0 = 6.667`).
#[must_use]
fn cursor_piece_bounds(
    origin: Point<Pixels>,
    cell_width: Pixels,
    line_height: Pixels,
) -> [Bounds<Pixels>; 6] {
    let x0 = origin.x;
    let x_mid = (origin.x + cell_width / 2.0).floor();
    let x1 = origin.x + cell_width;
    let y0 = origin.y;
    let y_third = (origin.y + line_height / 3.0).floor();
    let y_two_thirds = (origin.y + line_height * 2.0 / 3.0).floor();
    let y1 = origin.y + line_height;

    let rect = |left: Pixels, top: Pixels, right: Pixels, bottom: Pixels| {
        Bounds::new(point(left, top), gpui::size(right - left, bottom - top))
    };
    [
        rect(x0, y0, x_mid, y_third),           // 0: top-left
        rect(x_mid, y0, x1, y_third),           // 1: top-right
        rect(x0, y_third, x_mid, y_two_thirds), // 2: middle-left
        rect(x_mid, y_third, x1, y_two_thirds), // 3: middle-right
        rect(x0, y_two_thirds, x_mid, y1),      // 4: bottom-left
        rect(x_mid, y_two_thirds, x1, y1),      // 5: bottom-right
    ]
}

/// Whether the cursor's six-piece fade should be shown at all (TASK-1.127's plan, "The
/// animation gate") - every one of AC#3's own cases, isolated: the active pane (`window_
/// active && pane_focused`) of the focused window, a visible cursor, a program that has
/// asked for blinking, Reduce Motion off, and the pane not ended. No duration, no `Instant`
/// - this only answers "should the fade run right now", never "when".
#[must_use]
fn should_show_cursor_fade(
    window_active: bool,
    pane_focused: bool,
    cursor_visible: bool,
    blink_requested: bool,
    reduce_motion: bool,
    ended: bool,
) -> bool {
    window_active && pane_focused && cursor_visible && blink_requested && !reduce_motion && !ended
}

/// The duration from `fraction_through_cycle` to the next of the cycle's own boundaries,
/// built generically from `order_len`/`step_fraction`/`phase_whole`/`phase_returning`,
/// never six (or fourteen) literals (TASK-1.127's plan, "The driver"): the going-out
/// steps, the all-empty plateau's own start and end, and the returning steps.
/// `2*order_len + 2` boundaries in all (14 at the design's own six pieces): `phase_whole`,
/// then `phase_whole + i*step_fraction` for `i` in `1..=order_len` (reaching the all-empty
/// plateau's own start); `1 - phase_returning`, then `1 - phase_returning +
/// i*step_fraction` for `i` in `1..=order_len` (reaching `1.0`, which wraps to the next
/// cycle's `0.0`).
///
/// Pure - no `Instant`, no gpui type - so the caller supplies `fraction_through_cycle`
/// freshly computed from elapsed time every time, never from accumulating this function's
/// own previous answer (so a late wake never drifts).
#[must_use]
fn next_wake(
    fraction_through_cycle: f32,
    order_len: usize,
    step_fraction: f32,
    phase_whole: f32,
    phase_returning: f32,
    cycle: Duration,
) -> Duration {
    let return_start = 1.0 - phase_returning;
    let mut boundaries: Vec<f32> = Vec::with_capacity(2 * order_len + 2);
    boundaries.push(phase_whole);
    for i in 1..=order_len {
        #[expect(
            clippy::cast_precision_loss,
            reason = "a handful of pieces, nowhere near f32's precision limit"
        )]
        let i = i as f32;
        boundaries.push(phase_whole + i * step_fraction);
    }
    boundaries.push(return_start);
    for i in 1..=order_len {
        #[expect(
            clippy::cast_precision_loss,
            reason = "a handful of pieces, nowhere near f32's precision limit"
        )]
        let i = i as f32;
        boundaries.push(return_start + i * step_fraction);
    }
    debug_assert_eq!(boundaries.len(), 2 * order_len + 2);

    let next_boundary = boundaries
        .iter()
        .copied()
        .filter(|&boundary| boundary > fraction_through_cycle)
        .fold(None, |closest: Option<f32>, boundary| {
            Some(closest.map_or(boundary, |closest| closest.min(boundary)))
        })
        // Nothing strictly ahead in this cycle - the next boundary is the first one of the
        // next cycle, one full turn away.
        .unwrap_or(boundaries[0] + 1.0);

    cycle.mul_f32(next_boundary - fraction_through_cycle)
}

/// A flat blend of `toward` into `base`, by `strength` (`0.0` leaves `base` unchanged,
/// `1.0` is `toward`) - reuses gpui's own `Hsla::blend` (`color.rs`'s own doc: blends
/// `other` on top of `self` based on `other`'s own alpha, converting through `Rgba` rather
/// than lerping across the hue wheel) by giving `toward` the requested `strength` as its
/// alpha. This is what makes "flat colour, not a dither" (the engineer's own wording for
/// medium shade) hold: one blended colour, painted as a single fill, never a two-colour
/// pattern.
#[must_use]
fn blend(base: Hsla, toward: Hsla, strength: f32) -> Hsla {
    base.blend(Hsla {
        a: strength,
        ..toward
    })
}

/// The grapheme sitting at `(column)` on `row`, if any, with its own style, the column its
/// own run starts at, and how many columns each of its graphemes spans - the same run-walk
/// [`paint_grid`]'s own loop already does, pulled out so the cursor's own ground-coloured
/// glyph reveal (AC#4) has one grapheme to shape, not a whole run. `None` past the last
/// character on the line (nothing to redraw there - the cursor's own pieces paint directly
/// over the background, already correct with no glyph beneath them).
#[must_use]
fn grapheme_at(row: &Row, column: u16) -> Option<(&str, &CellStyle, u16, u16)> {
    let mut run_start_column = 0_u16;
    for run in row {
        let width = run_columns(run);
        if column < run_start_column.saturating_add(width) {
            let per_grapheme = columns_per_grapheme(run);
            if per_grapheme == 0 {
                return None;
            }
            let grapheme_rank = (column - run_start_column) / per_grapheme;
            let grapheme = run.text.graphemes(true).nth(usize::from(grapheme_rank))?;
            return Some((grapheme, &run.style, run_start_column, per_grapheme));
        }
        run_start_column = run_start_column.saturating_add(width);
    }
    None
}

/// Shapes `text` once, then checks whether every glyph it produced already sits on the
/// column its own grapheme owns (module doc). `columns_per_grapheme` is
/// [`columns_per_grapheme`]'s own result for the run being shaped - this function takes no
/// row-column parameter at all: the row's absolute column is [`cell_origin`]'s job alone,
/// applied separately when painting (second review finding, TASK-1.126). Returns the
/// single shaped line when the check passes, or re-shapes `text` one grapheme at a time
/// and returns that instead.
fn shape_and_check_run(
    window: &mut Window,
    text: &str,
    text_run: TextRun,
    cell_width: Pixels,
    columns_per_grapheme: u16,
) -> CachedRun {
    let whole = shape_text(window, text, text_run.clone());
    if every_glyph_sits_on_its_own_column(&whole, text, cell_width, columns_per_grapheme) {
        return CachedRun::Whole(Box::new(whole));
    }
    let per_grapheme = text
        .graphemes(true)
        .map(|grapheme| {
            let mut grapheme_run = text_run.clone();
            grapheme_run.len = grapheme.len();
            shape_text(window, grapheme, grapheme_run)
        })
        .collect();
    CachedRun::PerGrapheme(per_grapheme)
}

/// A small, fixed tolerance for floating-point shaping/rounding, not for a glyph that is
/// genuinely a fraction of a pixel off the grid - TASK-1.126's own build-time risk, settled
/// once a real run showed what a truly monospace resolved font actually does.
const GLYPH_POSITION_EPSILON: Pixels = px(0.5);

/// Whether every glyph `shaped` carries sits exactly on the column its own grapheme owns,
/// relative to the run's own start - collects each glyph's own `(byte index, x position)`
/// pair (`shaped` `#[deref]`s to `gpui::LineLayout`, whose `runs` and their `glyphs` carry
/// `position` and `index`, gpui's own `line_layout.rs`) and hands every decision to
/// [`every_glyph_position_is_safe`], the plain function over that data this delegates to -
/// no `gpui` type reaches past this one, thin, gpui-facing wrapper (review finding,
/// TASK-1.126: split out so the actual decision is testable with no window at all).
fn every_glyph_sits_on_its_own_column(
    shaped: &ShapedLine,
    text: &str,
    cell_width: Pixels,
    columns_per_grapheme: u16,
) -> bool {
    let glyph_positions: Vec<(usize, Pixels)> = shaped
        .runs
        .iter()
        .flat_map(|run| run.glyphs.iter())
        .map(|glyph| (glyph.index, glyph.position.x))
        .collect();
    every_glyph_position_is_safe(text, &glyph_positions, cell_width, columns_per_grapheme)
}

/// The pure core of the per-glyph safety check (module doc). `glyph_positions` are each a
/// shaped glyph's own byte index into `text` and its own `x` position - both **relative to
/// the run's own start**: `shape_and_check_run` shapes only the run's own text, never the
/// row it sits in (confirmed in gpui's own `line_layout.rs`: "the position of this glyph in
/// its containing line", where the "line" here is this one run, shaped alone), so a glyph's
/// `position.x` is never an absolute column on the row. A run's own absolute column
/// (`cell_origin`'s own job, applied separately when painting) must never enter this
/// comparison - review finding, TASK-1.126: the first version of this function added it in
/// here too, so any run not starting at column 0 always compared its own run-relative
/// positions against an absolute expectation and always "failed", silently taking the slow
/// path on every row with more than one styled run.
#[must_use]
fn every_glyph_position_is_safe(
    text: &str,
    glyph_positions: &[(usize, Pixels)],
    cell_width: Pixels,
    columns_per_grapheme: u16,
) -> bool {
    let grapheme_starts: Vec<usize> = text
        .grapheme_indices(true)
        .map(|(start, _)| start)
        .collect();
    for &(index, position_x) in glyph_positions {
        let Some(grapheme_rank) = grapheme_starts.iter().rposition(|&start| start <= index) else {
            // No grapheme starts at or before this glyph's own byte index - gpui shaped
            // something this function cannot account for; treat as unsafe rather than
            // assume it is fine.
            return false;
        };
        #[expect(
            clippy::cast_possible_truncation,
            reason = "a single run's own grapheme count never approaches u16::MAX"
        )]
        let grapheme_rank = grapheme_rank as u16;
        let expected_x = cell_width * f32::from(grapheme_rank * columns_per_grapheme);
        if (position_x - expected_x).abs() > GLYPH_POSITION_EPSILON {
            return false;
        }
    }
    true
}

/// Paints whichever path `cached` took: the single shaped line at its own run-start
/// origin, or one tiny shaped line per grapheme, each at its own column's explicit origin
/// (module doc) - a glyph belonging to one grapheme can never move where the next
/// grapheme's own shaped line is painted, since each is its own call with its own origin.
/// Where one run sits on the grid - bundled so [`paint_cached_run`] takes one placement
/// argument rather than three separate column/row numbers.
struct RunPlacement {
    start_column: u16,
    columns_per_grapheme: u16,
    row: u16,
}

fn paint_cached_run(
    cached: &CachedRun,
    placement: &RunPlacement,
    bounds_origin: Point<Pixels>,
    metrics: &CellMetrics,
    window: &mut Window,
    cx: &mut App,
) -> Result<(), String> {
    match cached {
        CachedRun::Whole(shaped) => {
            let origin = cell_origin(
                bounds_origin,
                metrics.cell_width,
                metrics.line_height,
                placement.start_column,
                placement.row,
            );
            paint_shaped_line(shaped, origin, metrics.line_height, window, cx)
        }
        CachedRun::PerGrapheme(lines) => {
            for (index, shaped) in lines.iter().enumerate() {
                #[expect(
                    clippy::cast_possible_truncation,
                    reason = "a single run's own grapheme count never approaches u16::MAX"
                )]
                let grapheme_rank = index as u16;
                let column =
                    placement.start_column + grapheme_rank * placement.columns_per_grapheme;
                let origin = cell_origin(
                    bounds_origin,
                    metrics.cell_width,
                    metrics.line_height,
                    column,
                    placement.row,
                );
                paint_shaped_line(shaped, origin, metrics.line_height, window, cx)?;
            }
            Ok(())
        }
    }
}

fn paint_shaped_line(
    shaped: &ShapedLine,
    origin: Point<Pixels>,
    line_height: Pixels,
    window: &mut Window,
    cx: &mut App,
) -> Result<(), String> {
    shaped
        .paint_background(origin, line_height, window, cx)
        .map_err(|error| format!("background: {error}"))?;
    shaped
        .paint(origin, line_height, window, cx)
        .map_err(|error| format!("text: {error}"))
}

fn to_text_run(run: &Run, theme: &TerminalTheme, family: &str) -> TextRun {
    let style = &run.style;
    let mut fg = style.fg.map(rgb_to_hsla).unwrap_or(theme.fg);
    let mut bg = style.bg.map(rgb_to_hsla);
    if style.inverse {
        let swapped_bg = fg;
        fg = bg.unwrap_or(theme.bg);
        bg = Some(swapped_bg);
    }
    if style.invisible {
        fg = fg.opacity(0.0);
    } else if style.faint {
        fg = fg.opacity(0.5);
    }
    let underline = match style.underline {
        Underline::None => None,
        Underline::Curly => Some(UnderlineStyle {
            color: Some(style.underline_colour.map(rgb_to_hsla).unwrap_or(fg)),
            thickness: px(1.0),
            wavy: true,
        }),
        // gpui 0.2.2's `UnderlineStyle` only distinguishes solid/wavy
        // (`text_system.rs`'s own field list) - Single/Double/Dotted/Dashed all paint
        // solid, a stated approximation doc-9's acceptance set does not depend on
        // telling apart (TASK-1.117's plan §2).
        Underline::Single | Underline::Double | Underline::Dotted | Underline::Dashed => {
            Some(UnderlineStyle {
                color: Some(style.underline_colour.map(rgb_to_hsla).unwrap_or(fg)),
                thickness: px(1.0),
                wavy: false,
            })
        }
    };
    let strikethrough = style.strikethrough.then(|| StrikethroughStyle {
        color: Some(fg),
        thickness: px(1.0),
    });
    TextRun {
        len: run.text.len(),
        font: terminal_font(style, family),
        color: fg,
        background_color: bg,
        underline,
        strikethrough,
    }
}

/// The font this run paints with - `family` is `CellMetrics`'s own resolved family
/// (metrics.rs's module doc), never re-derived from `foundations::FONT_TERMINAL`
/// independently: two call sites resolving separately is the bug this task fixes.
/// `fallbacks` still carries every other name in `FONT_TERMINAL`, for a glyph `family`
/// itself lacks - orthogonal to which family was resolved, and still wanted.
fn terminal_font(style: &CellStyle, family: &str) -> gpui::Font {
    let fallbacks = steg_ui::foundations::FONT_TERMINAL
        .iter()
        .filter(|candidate| **candidate != family)
        .map(|candidate| (*candidate).to_owned())
        .collect();
    gpui::Font {
        family: family.to_owned().into(),
        features: gpui::FontFeatures::default(),
        fallbacks: Some(gpui::FontFallbacks::from_fonts(fallbacks)),
        weight: if style.bold {
            FontWeight::BOLD
        } else {
            FontWeight::NORMAL
        },
        style: if style.italic {
            FontStyle::Italic
        } else {
            FontStyle::Normal
        },
    }
}

fn shape_text(window: &mut Window, text: &str, run: TextRun) -> ShapedLine {
    let text: SharedString = text.to_owned().into();
    let font_size = steg_ui::foundations::TERMINAL.size;
    window
        .text_system()
        .shape_line(text, font_size, &[run], None)
}

fn run_key(row: usize, column: u16, run: &Run) -> u64 {
    let mut hasher = DefaultHasher::new();
    row.hash(&mut hasher);
    column.hash(&mut hasher);
    run.text.hash(&mut hasher);
    run.style
        .fg
        .map(|rgb| (rgb.r, rgb.g, rgb.b))
        .hash(&mut hasher);
    run.style
        .bg
        .map(|rgb| (rgb.r, rgb.g, rgb.b))
        .hash(&mut hasher);
    run.style.bold.hash(&mut hasher);
    run.style.italic.hash(&mut hasher);
    run.style.inverse.hash(&mut hasher);
    run.style.invisible.hash(&mut hasher);
    run.style.faint.hash(&mut hasher);
    run.style.strikethrough.hash(&mut hasher);
    run.style.underline.hash(&mut hasher);
    // Review nit, fixed: a real stale-cache bug without this - two runs identical except
    // for underline colour collided on the same key, and the first one shaped won on a
    // cache hit, painting the wrong colour.
    run.style
        .underline_colour
        .map(|rgb| (rgb.r, rgb.g, rgb.b))
        .hash(&mut hasher);
    // Review nit, fixed: without this, the key relied on an unstated invariant (identical
    // text always carries the same east-asian-width classification, so no two cache
    // entries sharing a key ever actually need a different columns_per_grapheme) - true in
    // practice, but a literal hash of the field removes the need to rely on it at all.
    run.style.wide.hash(&mut hasher);
    hasher.finish()
}

/// This window's own cursor-fade state (TASK-1.127's plan): when the current fade cycle
/// started, whatever wake is currently scheduled for it (dropping it cancels it outright -
/// "if you drop a task it will be cancelled immediately", `gpui-0.2.2/src/executor.rs:54`,
/// the doc comment directly on `Task`), and whether the *previous* paint found the window
/// active and this pane focused - compared against the current paint so a transition into
/// focus can be told apart from an ordinary repaint while already focused, with no gpui
/// subscription at all (0.2.2 exposes no public one for window activation; TASK-1.127's
/// plan). One per window, held alongside [`RowCache`] for the same reason: it must survive
/// across repaints, never rebuilt per paint.
pub(crate) struct CursorFadeState {
    cycle_started_at: Instant,
    scheduled_wake: Option<Task<()>>,
    was_in_focus: bool,
}

impl Default for CursorFadeState {
    fn default() -> Self {
        Self {
            cycle_started_at: Instant::now(),
            scheduled_wake: None,
            was_in_focus: false,
        }
    }
}

impl CursorFadeState {
    /// A key press (or a committed IME composition) shows the cursor whole at once and
    /// restarts the cycle (AC#2) - drops whatever wake was pending (it was targeting a
    /// boundary of a cycle that no longer applies) and starts a fresh one from now; the
    /// next paint, triggered by the caller's own `cx.notify()`, re-arms a wake for the new
    /// cycle if the gate is still open.
    pub(crate) fn show_whole(&mut self) {
        self.cycle_started_at = Instant::now();
        self.scheduled_wake = None;
    }
}

/// Drives TASK-1.127's own cursor fade and paints the cursor - the six-piece dissolve for
/// a focused, visible, blinking block cursor with Reduce Motion off, or the steady shape
/// this function has always drawn otherwise (module doc, "Painting"). `fade`/`entity` are
/// this task's own addition: `fade` is this window's own state, above; `entity` is this
/// window's own handle, needed so the timer's own async task (`schedule_fade_wake`, below)
/// can notify it once it fires.
#[expect(
    clippy::too_many_arguments,
    reason = "each argument is a distinct, named input the fade's own gate and painting \
              need; bundling them into one struct would not make any of them optional or \
              grouped by a real relationship"
)]
fn paint_cursor(
    model: &Model,
    metrics: &CellMetrics,
    theme: &TerminalTheme,
    bounds: Bounds<Pixels>,
    window: &mut Window,
    cx: &mut App,
    row_cache: &Rc<RefCell<RowCache>>,
    fade: &Rc<RefCell<CursorFadeState>>,
    focus_handle: &FocusHandle,
    ended: bool,
    entity: &Entity<AppWindow>,
) {
    let cursor = model.cursor();
    if !cursor.visible {
        fade.borrow_mut().scheduled_wake = None;
        return;
    }

    let window_active = window.is_window_active();
    let pane_focused = focus_handle.is_focused(window);
    let now_in_focus = window_active && pane_focused;
    {
        let mut fade_state = fade.borrow_mut();
        // A transition into focus - the window regaining activation, or (once a later
        // task adds a second focusable pane) this one gaining it - restarts the cycle
        // from whole: decided by the orchestrator against the engineer's own "stops,
        // whole, when the window loses focus" (regained focus starts fresh, never
        // resumes a frozen mid-fade phase). Seen by comparing against the *previous*
        // paint's own state, not a gpui subscription (`CursorFadeState`'s own doc) -
        // so this is noticed on whichever paint happens to run next, not at the exact
        // instant of the OS event.
        if now_in_focus && !fade_state.was_in_focus {
            fade_state.cycle_started_at = Instant::now();
        }
        fade_state.was_in_focus = now_in_focus;
    }

    let origin = cell_origin(
        bounds.origin,
        metrics.cell_width,
        metrics.line_height,
        cursor.x,
        cursor.y,
    );
    let size = gpui::size(metrics.cell_width, metrics.line_height);
    let colour = theme.cursor;

    let gate = should_show_cursor_fade(
        window_active,
        pane_focused,
        cursor.visible,
        cursor.blinking,
        reduce_motion_enabled(),
        ended,
    );

    if cursor.shape == CursorShape::Block && gate {
        paint_cursor_fade(
            model, metrics, theme, origin, cursor, window, cx, row_cache, fade, entity,
        );
        return;
    }

    fade.borrow_mut().scheduled_wake = None;
    match cursor.shape {
        CursorShape::Block => window.paint_quad(fill(Bounds::new(origin, size), colour)),
        CursorShape::BlockHollow => {
            window.paint_quad(gpui::outline(
                Bounds::new(origin, size),
                colour,
                gpui::BorderStyle::default(),
            ));
        }
        CursorShape::Underline => {
            let bar_height = px(2.0);
            let bar_origin = point(origin.x, origin.y + size.height - bar_height);
            window.paint_quad(fill(
                Bounds::new(bar_origin, gpui::size(size.width, bar_height)),
                colour,
            ));
        }
        CursorShape::Bar => {
            let bar_width = px(2.0);
            window.paint_quad(fill(
                Bounds::new(origin, gpui::size(bar_width, size.height)),
                colour,
            ));
        }
    }
}

/// The six-piece dissolve itself, once [`paint_cursor`] has decided the gate is open and
/// the shape is a block: computes the fraction through the cycle from elapsed time (the
/// clock read once, here, in the imperative shell - everything downstream is a plain
/// `f32`/`Duration` through the pure functions above), paints the six pieces, reveals the
/// character beneath them in the ground colour (AC#4), and schedules the next wake.
#[expect(
    clippy::too_many_arguments,
    reason = "each argument is a distinct, named input the pieces, the glyph reveal and \
              the schedule each need"
)]
fn paint_cursor_fade(
    model: &Model,
    metrics: &CellMetrics,
    theme: &TerminalTheme,
    origin: Point<Pixels>,
    cursor: Cursor,
    window: &mut Window,
    cx: &mut App,
    row_cache: &Rc<RefCell<RowCache>>,
    fade: &Rc<RefCell<CursorFadeState>>,
    entity: &Entity<AppWindow>,
) {
    let pattern = steg_ui::foundations::MOTION_CURSOR_FADE_SHADED_DISSOLVE;
    let cycle_started_at = fade.borrow().cycle_started_at;
    let elapsed_seconds = cycle_started_at.elapsed().as_secs_f32();
    let cycle_seconds = pattern.cycle.as_secs_f32();
    let fraction = if cycle_seconds > 0.0 {
        (elapsed_seconds / cycle_seconds).rem_euclid(1.0)
    } else {
        0.0
    };

    let states = piece_states_at(
        fraction,
        &pattern.order,
        pattern.step_fraction,
        pattern.phase_whole,
        pattern.phase_returning,
    );
    let piece_bounds = cursor_piece_bounds(origin, metrics.cell_width, metrics.line_height);
    let shade_colour = blend(theme.cursor, theme.bg, pattern.medium_shade_strength);

    for (index, state) in states.iter().enumerate() {
        match state {
            PieceState::Empty => {}
            PieceState::Whole => window.paint_quad(fill(piece_bounds[index], theme.cursor)),
            PieceState::Shade => window.paint_quad(fill(piece_bounds[index], shade_colour)),
        }
    }

    paint_cursor_glyph_reveal(
        model,
        metrics,
        theme,
        origin,
        cursor,
        &states,
        &piece_bounds,
        window,
        cx,
        row_cache,
    );

    let next = next_wake(
        fraction,
        pattern.order.len(),
        pattern.step_fraction,
        pattern.phase_whole,
        pattern.phase_returning,
        pattern.cycle,
    );
    schedule_fade_wake(entity, window, cx, next, fade);
}

/// Over the cursor's own cell, the character is drawn in the ground colour where the
/// cursor's pieces cover it and in its own colour elsewhere (AC#4). The character under
/// the cursor, if any, was already painted in its own colour by [`paint_grid`]'s ordinary
/// pass; this only re-paints the covered portion, clipped to each covered (`Whole`/
/// `Shade`, never `Empty`) piece's own rectangle in turn, so every `Empty` piece leaves
/// what was already there showing through untouched. Shaped once into the shared
/// [`RowCache`] (keyed by [`cursor_glyph_key`], never [`run_key`]'s own key space), so the
/// many wakes within one unchanged character cost no reshaping - only the clipped paint
/// calls repeat.
#[expect(
    clippy::too_many_arguments,
    reason = "each argument is a distinct, named input this needs; bundling them would not \
              make any of them optional or grouped by a real relationship"
)]
fn paint_cursor_glyph_reveal(
    model: &Model,
    metrics: &CellMetrics,
    theme: &TerminalTheme,
    origin: Point<Pixels>,
    cursor: Cursor,
    states: &[PieceState; 6],
    piece_bounds: &[Bounds<Pixels>; 6],
    window: &mut Window,
    cx: &mut App,
    row_cache: &Rc<RefCell<RowCache>>,
) {
    let Some(row) = model.rows().get(usize::from(cursor.y)) else {
        return;
    };
    let Some((grapheme, style, _run_start_column, _columns_per_grapheme)) =
        grapheme_at(row, cursor.x)
    else {
        return;
    };

    let text_run = TextRun {
        len: grapheme.len(),
        font: terminal_font(style, &metrics.family),
        color: theme.bg,
        background_color: None,
        underline: None,
        strikethrough: None,
    };
    let key = cursor_glyph_key(cursor.y, cursor.x, grapheme);
    let cached = row_cache.borrow_mut().painted(key, || {
        CachedRun::Whole(Box::new(shape_text(window, grapheme, text_run)))
    });
    let CachedRun::Whole(shaped) = cached else {
        // `shape_and_check_run`'s own per-grapheme path never mints a key through
        // `cursor_glyph_key` - this function always builds `CachedRun::Whole` above, so
        // reaching here would mean a collision with an ordinary run's own key, the same
        // risk every entry in this shared, un-namespaced `u64` cache already accepts
        // (`run_key`'s own doc). Nothing to paint rather than painting the wrong thing.
        return;
    };

    for (index, state) in states.iter().enumerate() {
        if matches!(state, PieceState::Empty) {
            continue;
        }
        let mask = ContentMask {
            bounds: piece_bounds[index],
        };
        window.with_content_mask(Some(mask), |window| {
            let _ = shaped.paint(origin, metrics.line_height, window, cx);
        });
    }
}

/// A cache key for the cursor's own ground-coloured glyph reveal (AC#4) - distinct from
/// [`run_key`]'s own hashing (no style fields: the colour painted here is always the
/// ground, never the run's own), salted so it cannot land in the same part of the key
/// space as an ordinary run by construction, rather than by the same luck every other
/// entry in this shared cache already relies on.
fn cursor_glyph_key(row: u16, column: u16, grapheme: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    "cursor-glyph-reveal".hash(&mut hasher);
    row.hash(&mut hasher);
    column.hash(&mut hasher);
    grapheme.hash(&mut hasher);
    hasher.finish()
}

/// Replaces whatever wake was pending with a fresh one targeting `duration` from now.
/// Dropping the old `Task` (by overwriting `fade`'s own field) cancels it outright
/// (`CursorFadeState`'s own doc), so a paint that re-targets the schedule - because it ran
/// for some other reason, such as a model update, while a wake was already pending - never
/// races a timer that was about to fire for the old phase. The timer itself is gpui's own
/// `BackgroundExecutor::timer` (`executor.rs:357`), not a display-frame request: it sleeps
/// for exactly `duration` and nothing in between, then notifies this window's own entity so
/// the next paint re-evaluates the gate and either re-arms or stops.
fn schedule_fade_wake(
    entity: &Entity<AppWindow>,
    window: &mut Window,
    cx: &mut App,
    duration: Duration,
    fade: &Rc<RefCell<CursorFadeState>>,
) {
    let weak = entity.downgrade();
    let task = window.spawn(cx, async move |cx: &mut AsyncWindowContext| {
        cx.background_executor().timer(duration).await;
        let _ = weak.update(cx, |_, cx| cx.notify());
    });
    fade.borrow_mut().scheduled_wake = Some(task);
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- cell_origin (AC#3): the placement function, with no window at all -------------

    #[test]
    fn the_cell_after_an_eighty_character_line_sits_at_column_eighty() {
        let origin = point(px(0.0), px(0.0));
        let cell_width = px(8.0);
        let line_height = px(20.0);
        // An 80-character line fills columns 0..=79; the cursor that follows it sits on
        // column 80 - exactly where the eighty-first cell starts, never drifted by however
        // the eighty characters themselves were shaped (TASK-1.126's own acceptance
        // criterion).
        let after_the_line = cell_origin(origin, cell_width, line_height, 80, 0);
        assert_eq!(after_the_line, point(px(640.0), px(0.0)));
    }

    #[test]
    fn a_cells_origin_moves_down_by_a_whole_line_height_per_row() {
        let origin = point(px(0.0), px(0.0));
        let cell_width = px(8.0);
        let line_height = px(20.0);
        assert_eq!(
            cell_origin(origin, cell_width, line_height, 0, 3),
            point(px(0.0), px(60.0))
        );
    }

    #[test]
    fn a_cells_origin_is_offset_from_a_non_zero_bounds_origin() {
        let origin = point(px(100.0), px(50.0));
        let cell_width = px(8.0);
        let line_height = px(20.0);
        assert_eq!(
            cell_origin(origin, cell_width, line_height, 2, 1),
            point(px(116.0), px(70.0))
        );
    }

    // --- every_glyph_position_is_safe (AC#3): the pure core of the per-glyph safety
    // --- check, with no `gpui` type and no window at all --------------------------------

    #[test]
    fn a_run_whose_glyphs_are_all_on_grid_is_safe() {
        // Three one-byte ASCII graphemes, each landing exactly on its own run-relative
        // column - the shape a genuinely monospace font actually produces.
        let positions = [(0, px(0.0)), (1, px(10.0)), (2, px(20.0))];
        assert!(every_glyph_position_is_safe("abc", &positions, px(10.0), 1));
    }

    #[test]
    fn a_glyph_off_by_more_than_the_tolerance_is_unsafe() {
        // The third glyph sits 2px off its own expected column - well past
        // GLYPH_POSITION_EPSILON (0.5px) - as a proportional font's own advances would
        // produce.
        let positions = [(0, px(0.0)), (1, px(10.0)), (2, px(22.0))];
        assert!(!every_glyph_position_is_safe(
            "abc",
            &positions,
            px(10.0),
            1
        ));
    }

    #[test]
    fn positions_relative_to_the_run_are_safe_even_when_the_run_starts_at_a_non_zero_column() {
        // Review finding, TASK-1.126: the first version of this check compared these same
        // run-relative positions against an *absolute* row column, so any run not starting
        // at column 0 always "failed" here, regardless of whether the glyphs were actually
        // safe. These positions are exactly what gpui reports for a run shaped on its own
        // (confirmed by reading gpui's own line_layout.rs: a glyph's own position is
        // relative to the shaped line it came from, never the row) - the function must
        // judge them safe purely on their own terms, with no column of the row entering
        // this comparison at all (there is no such parameter to pass one through any more).
        let positions = [(0, px(0.0)), (1, px(10.0)), (2, px(20.0))];
        assert!(every_glyph_position_is_safe("xyz", &positions, px(10.0), 1));
    }

    fn style() -> CellStyle {
        CellStyle {
            fg: None,
            bg: None,
            bold: false,
            italic: false,
            faint: false,
            blink: false,
            inverse: false,
            invisible: false,
            strikethrough: false,
            overline: false,
            underline: Underline::Single,
            underline_colour: None,
            wide: steg_sdk::Wide::Narrow,
            link: false,
        }
    }

    /// Review nit, fixed: two runs identical except for `underline_colour` must not share
    /// a cache key, or the first one shaped wins on a cache hit and paints the wrong
    /// underline colour.
    #[test]
    fn two_runs_differing_only_in_underline_colour_get_different_keys() {
        let red = Rgb { r: 255, g: 0, b: 0 };
        let blue = Rgb { r: 0, g: 0, b: 255 };
        let run_red = Run {
            text: "hi".to_owned(),
            style: CellStyle {
                underline_colour: Some(red),
                ..style()
            },
        };
        let run_blue = Run {
            text: "hi".to_owned(),
            style: CellStyle {
                underline_colour: Some(blue),
                ..style()
            },
        };
        assert_ne!(run_key(0, 0, &run_red), run_key(0, 0, &run_blue));
    }

    #[test]
    fn two_runs_with_the_same_underline_colour_share_a_key() {
        let red = Rgb { r: 255, g: 0, b: 0 };
        let run = Run {
            text: "hi".to_owned(),
            style: CellStyle {
                underline_colour: Some(red),
                ..style()
            },
        };
        assert_eq!(run_key(0, 0, &run), run_key(0, 0, &run.clone()));
    }

    // --- every_glyph_sits_on_its_own_column (AC#3): not unit-tested, and why ------------
    //
    // This needs a real `ShapedLine` to inspect - gpui's own headless `TestAppContext`
    // would give one with no real window, but it is only exported behind gpui's own
    // `test-support` feature (confirmed: `#[cfg(any(test, feature = "test-support"))]` on
    // its re-export in gpui's `app.rs`), which this crate does not enable - adding it
    // would pull in `rand`, `leak-detection`/`backtrace`, and gpui's own `wayland`/`x11`
    // features (new dependencies, a new licence surface to check, unrelated to this bug),
    // not something to add silently for one test. A hand-built fake `ShapedLine` was the
    // other option tried and also found closed: `ShapedGlyph.id` is a `GlyphId`, and
    // `GlyphId`'s own inner field is `pub(crate)` (`gpui-0.2.2/src/text_system.rs`) - not
    // constructible from outside gpui at all, feature flag or not.
    //
    // Per this task's own plan, the stated fallback: `resolve_family` and `cell_origin`
    // keep their unit tests above; `every_glyph_sits_on_its_own_column` itself is verified
    // by the live run instead - a real attached terminal's text actually lines up, and the
    // PR's own screenshots (AC#4) are the recorded evidence of it, not a unit test.

    // --- piece_states_at (AC#1, AC#5): the pure step function, against the engineer's
    // --- own table (TASK-1.127's plan, "The pure core") ---------------------------------

    use PieceState::{Empty as E, Shade as S, Whole as W};

    const ORDER: [i32; 6] = [3, 0, 4, 2, 1, 5];
    const STEP: f32 = 0.03;
    const PHASE_WHOLE: f32 = 0.43;
    const PHASE_RETURNING: f32 = 0.18;

    #[test]
    fn piece_states_at_matches_the_engineers_own_table() {
        // Each row is a fraction at one of the cycle's own 14 boundaries, and the state
        // of pieces 0 through 5 at that instant - worked out independently from the
        // engineer's own per-rank table (TASK-1.127's plan), not from this function's own
        // formula, so this is a real check against the design, not the function checking
        // itself.
        let cases: &[(f32, [PieceState; 6])] = &[
            (0.00, [W, W, W, W, W, W]),
            (0.43, [W, W, W, S, W, W]),
            (0.46, [S, W, W, E, W, W]),
            (0.49, [E, W, W, E, S, W]),
            (0.52, [E, W, S, E, E, W]),
            (0.55, [E, S, E, E, E, W]),
            (0.58, [E, E, E, E, E, S]),
            (0.61, [E, E, E, E, E, E]),
            (0.82, [E, E, E, E, E, S]),
            (0.85, [E, S, E, E, E, W]),
            (0.88, [E, W, S, E, E, W]),
            (0.91, [E, W, W, E, S, W]),
            (0.94, [S, W, W, E, W, W]),
            (0.97, [W, W, W, S, W, W]),
        ];
        for (fraction, expected) in cases {
            assert_eq!(
                piece_states_at(*fraction, &ORDER, STEP, PHASE_WHOLE, PHASE_RETURNING),
                *expected,
                "at fraction {fraction}"
            );
        }
    }

    #[test]
    fn piece_states_at_boundaries_move_with_phase_whole_not_with_a_fixed_literal() {
        // Control: at the real `PHASE_WHOLE` (0.43), piece 3 (rank 0) is already shading
        // by 0.43. A smaller `phase_whole` must move that boundary earlier, proving the
        // 0.43 in the table above came from the parameter, not a literal written into
        // this function.
        let real = piece_states_at(0.43, &ORDER, STEP, PHASE_WHOLE, PHASE_RETURNING);
        assert_eq!(real[3], S);

        let moved = piece_states_at(0.43, &ORDER, STEP, 0.40, PHASE_RETURNING);
        assert_eq!(
            moved[3], E,
            "with phase_whole moved to 0.40, piece 3 should already be empty by 0.43, not \
             still shading at the old boundary"
        );
    }

    // --- cursor_piece_bounds (AC#1): the six-piece geometry, no seam or gap -------------

    #[test]
    fn adjacent_pieces_share_an_exact_edge_at_an_odd_cell_size() {
        let origin = point(px(0.0), px(0.0));
        let cell_width = px(7.0);
        let line_height = px(19.0);
        let bounds = cursor_piece_bounds(origin, cell_width, line_height);

        for (left, right) in [(0, 1), (2, 3), (4, 5)] {
            assert_eq!(
                bounds[left].origin.x + bounds[left].size.width,
                bounds[right].origin.x,
                "piece {left} and piece {right} should share an exact column edge"
            );
        }
        for (top, bottom) in [(0, 2), (2, 4), (1, 3), (3, 5)] {
            assert_eq!(
                bounds[top].origin.y + bounds[top].size.height,
                bounds[bottom].origin.y,
                "piece {top} and piece {bottom} should share an exact row edge"
            );
        }

        // The six pieces' own total area equals the cell's own area exactly - no gap, no
        // overlap, even though 7px/19px divide unevenly by 2 and 3.
        let total_area: f32 = bounds
            .iter()
            .map(|b| f32::from(b.size.width) * f32::from(b.size.height))
            .sum();
        assert_eq!(total_area, f32::from(cell_width) * f32::from(line_height));
    }

    #[test]
    fn the_bottom_right_piece_reaches_the_cells_own_true_far_edge() {
        let origin = point(px(100.0), px(50.0));
        let cell_width = px(19.0);
        let line_height = px(20.0);
        let bounds = cursor_piece_bounds(origin, cell_width, line_height);
        let far_corner = point(
            bounds[5].origin.x + bounds[5].size.width,
            bounds[5].origin.y + bounds[5].size.height,
        );
        assert_eq!(
            far_corner,
            point(origin.x + cell_width, origin.y + line_height)
        );
    }

    #[test]
    fn independently_rounded_piece_sizes_would_leave_a_gap_at_the_cells_own_far_edge() {
        // The control `cursor_piece_bounds` itself avoids: if the bottom-right piece's own
        // far corner were built from `origin + rounded_size * index` rather than the
        // bounds' own true `x1`/`y1`, an odd cell size leaves a gap.
        let origin = point(px(0.0), px(0.0));
        let cell_width = px(19.0);
        let line_height = px(20.0);
        let half_width = (cell_width / 2.0).floor();
        let third_height = (line_height / 3.0).floor();
        let buggy_bottom_right_origin = point(origin.x + half_width, origin.y + third_height * 2.0);
        let buggy_far_corner = point(
            buggy_bottom_right_origin.x + half_width,
            buggy_bottom_right_origin.y + third_height,
        );
        assert_ne!(
            buggy_far_corner,
            point(origin.x + cell_width, origin.y + line_height),
            "this control should miss the cell's own true far edge - if it doesn't, it no \
             longer demonstrates the bug cursor_piece_bounds avoids"
        );
    }

    // --- should_show_cursor_fade (AC#3): the gate, one isolated case per test ----------

    #[test]
    fn the_fade_shows_when_every_gate_is_open() {
        assert!(should_show_cursor_fade(
            true, true, true, true, false, false
        ));
    }

    #[test]
    fn the_fade_does_not_show_when_the_window_is_not_active() {
        assert!(!should_show_cursor_fade(
            false, true, true, true, false, false
        ));
    }

    #[test]
    fn the_fade_does_not_show_when_the_pane_is_not_focused() {
        assert!(!should_show_cursor_fade(
            true, false, true, true, false, false
        ));
    }

    #[test]
    fn the_fade_does_not_show_when_the_cursor_is_hidden() {
        assert!(!should_show_cursor_fade(
            true, true, false, true, false, false
        ));
    }

    #[test]
    fn the_fade_does_not_show_when_the_program_has_not_asked_for_blinking() {
        assert!(!should_show_cursor_fade(
            true, true, true, false, false, false
        ));
    }

    #[test]
    fn the_fade_does_not_show_when_reduce_motion_is_on() {
        assert!(!should_show_cursor_fade(
            true, true, true, true, true, false
        ));
    }

    #[test]
    fn the_fade_does_not_show_once_the_pane_has_ended() {
        assert!(!should_show_cursor_fade(
            true, true, true, true, false, true
        ));
    }

    // --- next_wake (AC#3): the schedule itself, pure, no window -------------------------

    #[test]
    fn next_wake_lands_exactly_on_the_next_boundary() {
        let cycle = Duration::from_millis(1400);
        // From fraction 0.40 (within the whole region), the next boundary is
        // `PHASE_WHOLE` (0.43) - 0.03 of the cycle away.
        let duration = next_wake(0.40, 6, STEP, PHASE_WHOLE, PHASE_RETURNING, cycle);
        let expected_seconds = 0.03 * cycle.as_secs_f32();
        assert!(
            (duration.as_secs_f32() - expected_seconds).abs() < 1e-4,
            "expected ~{expected_seconds}s, got {}s",
            duration.as_secs_f32()
        );
    }

    #[test]
    fn next_wake_from_just_before_the_wrap_lands_on_the_cycles_own_last_boundary() {
        let cycle = Duration::from_millis(1400);
        // `1.0` itself is always in the boundary set (`return_start + order_len*step`,
        // 0.82 + 6*0.03 = 1.00 at the design's own values) - the wrap is already an
        // ordinary boundary, not a special case, so from 0.99 the next wake is simply
        // 0.01 of the cycle away, landing exactly on it.
        let duration = next_wake(0.99, 6, STEP, PHASE_WHOLE, PHASE_RETURNING, cycle);
        let expected_seconds = 0.01 * cycle.as_secs_f32();
        assert!(
            (duration.as_secs_f32() - expected_seconds).abs() < 1e-3,
            "expected ~{expected_seconds}s, got {}s",
            duration.as_secs_f32()
        );
    }

    #[test]
    fn next_wake_falls_back_to_one_full_turn_when_nothing_is_strictly_ahead() {
        // Defensive, not expected from real elapsed-time input (always reduced into
        // `0.0..1.0` by `rem_euclid`, so always strictly below the cycle's own `1.0`
        // boundary): a `fraction_through_cycle` already at or past every real boundary
        // still returns a sane, forward-only answer - one full turn to the first
        // boundary of the cycle after this one - rather than panicking or returning a
        // non-positive duration.
        let cycle = Duration::from_millis(1400);
        let duration = next_wake(1.0, 6, STEP, PHASE_WHOLE, PHASE_RETURNING, cycle);
        let expected_seconds = PHASE_WHOLE * cycle.as_secs_f32();
        assert!(
            (duration.as_secs_f32() - expected_seconds).abs() < 1e-3,
            "expected ~{expected_seconds}s, got {}s",
            duration.as_secs_f32()
        );
    }

    /// Walks a full cycle by repeatedly calling `next_wake` from wherever the previous
    /// wake landed, counting how many wakes it takes to reach the cycle's own end -
    /// `next_wake`'s own schedule, not an assumed constant. `phase_returning` is taken as
    /// a parameter, not fixed at the real `PHASE_RETURNING`: a self-consistent cycle needs
    /// `phase_returning == order_len * step_fraction` (the returning region's own total
    /// width), which only holds for `order_len = 6` at the design's own `PHASE_RETURNING`
    /// (0.18 = 6*0.03) - the control below uses a different, still self-consistent, pair.
    fn count_wakes_across_a_full_cycle(
        order_len: usize,
        phase_returning: f32,
        cycle: Duration,
    ) -> u32 {
        let mut fraction = 0.0_f32;
        let mut wakes = 0_u32;
        loop {
            let duration = next_wake(
                fraction,
                order_len,
                STEP,
                PHASE_WHOLE,
                phase_returning,
                cycle,
            );
            fraction += duration.as_secs_f32() / cycle.as_secs_f32();
            wakes += 1;
            if fraction >= 1.0 - 1e-6 {
                break;
            }
            assert!(
                wakes <= 100,
                "did not converge - next_wake is not landing on real boundaries"
            );
        }
        wakes
    }

    #[test]
    fn a_full_traversal_of_the_cycle_yields_exactly_the_expected_number_of_wakes() {
        let cycle = Duration::from_millis(1400);
        assert_eq!(
            count_wakes_across_a_full_cycle(6, PHASE_RETURNING, cycle),
            2 * 6 + 2
        );
    }

    #[test]
    fn the_wake_count_follows_order_len_not_a_fixed_constant() {
        // Control: a different (still self-consistent: phase_returning = order_len*step)
        // piece count must change the boundary count too, proving `2*order_len + 2` is
        // not a hardcoded "14" this function happens to return.
        let cycle = Duration::from_millis(1400);
        let order_len = 5;
        #[expect(
            clippy::cast_precision_loss,
            reason = "a handful of pieces, nowhere near f32's precision limit"
        )]
        let phase_returning = order_len as f32 * STEP;
        assert_eq!(
            count_wakes_across_a_full_cycle(order_len, phase_returning, cycle),
            2 * order_len as u32 + 2
        );
    }

    // --- blend (AC#1): a flat colour, never a dither ------------------------------------

    #[test]
    fn blend_at_half_strength_is_the_rgb_midpoint() {
        let red: Hsla = gpui::rgb(0xff_00_00).into();
        let blue: Hsla = gpui::rgb(0x00_00_ff).into();
        let result = blend(red, blue, 0.5);
        let rgba = gpui::Rgba::from(result);
        assert!((rgba.r - 0.5).abs() < 1e-3, "r was {}", rgba.r);
        assert!((rgba.g - 0.0).abs() < 1e-3, "g was {}", rgba.g);
        assert!((rgba.b - 0.5).abs() < 1e-3, "b was {}", rgba.b);
    }

    #[test]
    fn blend_at_zero_strength_leaves_the_base_colour_unchanged() {
        let base: Hsla = gpui::rgb(0x33_66_99).into();
        let toward: Hsla = gpui::rgb(0xff_ff_ff).into();
        assert_eq!(blend(base, toward, 0.0), base);
    }

    // --- grapheme_at (AC#4): the cursor's own ground-coloured glyph reveal -------------

    fn narrow_run(text: &str) -> Run {
        Run {
            text: text.to_owned(),
            style: style(),
        }
    }

    fn wide_run(text: &str) -> Run {
        Run {
            text: text.to_owned(),
            style: CellStyle {
                wide: Wide::Wide,
                ..style()
            },
        }
    }

    #[test]
    fn grapheme_at_finds_the_grapheme_in_a_single_run_row() {
        let row: Row = vec![narrow_run("hello")];
        let (grapheme, _style, run_start_column, per_grapheme) = grapheme_at(&row, 1).unwrap();
        assert_eq!(grapheme, "e");
        assert_eq!(run_start_column, 0);
        assert_eq!(per_grapheme, 1);
    }

    #[test]
    fn grapheme_at_finds_a_grapheme_at_a_non_zero_run_start_column() {
        // Two runs, same style shape: the second run's own grapheme must be found at its
        // own column, never the first run's (the same coordinate-space care TASK-1.126's
        // own review round took for `every_glyph_position_is_safe`).
        let row: Row = vec![narrow_run("abc"), narrow_run("xyz")];
        let (grapheme, _style, run_start_column, per_grapheme) = grapheme_at(&row, 4).unwrap();
        assert_eq!(grapheme, "y");
        assert_eq!(run_start_column, 3);
        assert_eq!(per_grapheme, 1);
    }

    #[test]
    fn grapheme_at_a_wide_character_reports_two_columns_per_grapheme() {
        let row: Row = vec![wide_run("\u{4f60}\u{597d}")];
        let (grapheme, _style, run_start_column, per_grapheme) = grapheme_at(&row, 0).unwrap();
        assert_eq!(grapheme, "\u{4f60}");
        assert_eq!(run_start_column, 0);
        assert_eq!(per_grapheme, 2);
    }

    #[test]
    fn grapheme_at_past_the_lines_own_end_is_none() {
        let row: Row = vec![narrow_run("hi")];
        assert!(grapheme_at(&row, 5).is_none());
    }

    #[test]
    fn grapheme_at_an_empty_row_is_none() {
        let row: Row = Vec::new();
        assert!(grapheme_at(&row, 0).is_none());
    }

    // --- cursor_glyph_key (AC#4): distinct from run_key's own key space -----------------

    #[test]
    fn cursor_glyph_key_differs_from_an_ordinary_runs_key_for_the_same_text() {
        let run = Run {
            text: "e".to_owned(),
            style: style(),
        };
        assert_ne!(cursor_glyph_key(0, 1, "e"), run_key(0, 1, &run));
    }

    // --- CursorFadeState::show_whole (AC#2): no window at all ---------------------------

    #[test]
    fn show_whole_resets_the_cycle_and_drops_any_pending_wake() {
        let mut state = CursorFadeState::default();
        let before = state.cycle_started_at;
        // `Task::ready` needs no window or executor (gpui's own public constructor) -
        // standing in for a real wake that was pending, so clearing it is actually
        // exercised, not merely still `None` from `default()`.
        state.scheduled_wake = Some(Task::ready(()));
        state.show_whole();
        assert!(
            state.cycle_started_at >= before,
            "the cycle's own origin should move forward (or stay, on a low-resolution \
             clock), never backward"
        );
        assert!(state.scheduled_wake.is_none());
    }
}
