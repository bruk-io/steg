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

use gpui::{
    App, Bounds, ElementInputHandler, Entity, FocusHandle, FontStyle, FontWeight, Hsla,
    IntoElement, ParentElement as _, Pixels, Point, ShapedLine, SharedString, StrikethroughStyle,
    Styled as _, TextRun, UnderlineStyle, Window, canvas, div, fill, point, px,
};
use steg_sdk::{CellStyle, Cursor, CursorShape, Rgb, Run, Underline, Wide};
use steg_ui::foundations::TerminalTheme;
use unicode_segmentation::UnicodeSegmentation as _;

use crate::AppWindow;
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
pub fn terminal(
    model: Rc<RefCell<Model>>,
    metrics: CellMetrics,
    theme: TerminalTheme,
    row_cache: Rc<RefCell<RowCache>>,
    focus_handle: FocusHandle,
    entity: Entity<AppWindow>,
    last_bounds: Rc<std::cell::Cell<Option<Bounds<Pixels>>>>,
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

    paint_cursor(model.cursor(), metrics, theme, bounds, window);
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

/// Shapes `text` once, then checks whether every glyph it produced already sits on the
/// column its own grapheme owns (module doc). `run_start_column` is where this run's own
/// first cell sits on its row; `columns_per_grapheme` is [`columns_per_grapheme`]'s own
/// result for the run being shaped. Returns the single shaped line when the check passes,
/// or re-shapes `text` one grapheme at a time and returns that instead.
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

fn paint_cursor(
    cursor: Cursor,
    metrics: &CellMetrics,
    theme: &TerminalTheme,
    bounds: Bounds<Pixels>,
    window: &mut Window,
) {
    if !cursor.visible {
        return;
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
}
