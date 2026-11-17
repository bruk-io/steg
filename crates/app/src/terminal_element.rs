//! Paints the fixed grid read-only (TASK-1.117's plan §2): a window's own background in
//! the terminal theme's own `bg`, the grid from its top-left origin at its natural pixel
//! size, letterboxed if the window is bigger, cropped if smaller. Written fresh for this
//! crate, from gpui's own `canvas`/`ShapedLine` API and rustdoc - no GPL code read or
//! copied (R59; `.claude/rules/licence.md`); the general shape (a per-row shaping cache,
//! painting runs at `origin + cell_width * column`, a cursor quad) follows steg's own
//! prior MIT spike, `spikes/task-1.1-terminal-library/probe-core/src/element.rs`, which
//! already proved this technique on this exact pinned gpui version (decision-3).

use std::cell::RefCell;
use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash as _, Hasher as _};
use std::rc::Rc;

use gpui::{
    App, Bounds, ElementInputHandler, Entity, FocusHandle, FontStyle, FontWeight, Hsla,
    IntoElement, ParentElement as _, Pixels, ShapedLine, SharedString, StrikethroughStyle,
    Styled as _, TextRun, UnderlineStyle, Window, canvas, div, fill, point, px,
};
use steg_sdk::{CellStyle, Cursor, CursorShape, Rgb, Run, Underline};
use steg_ui::foundations::TerminalTheme;

use crate::AppWindow;
use crate::metrics::CellMetrics;
use crate::model::{Model, run_columns};

fn rgb_to_hsla(rgb: Rgb) -> Hsla {
    gpui::rgb(u32::from(rgb.r) << 16 | u32::from(rgb.g) << 8 | u32::from(rgb.b)).into()
}

/// Shaped rows kept between frames, so a row that has not changed costs no reshaping even
/// though the whole grid repaints every frame (gpui 0.2.2 has no partial-repaint primitive
/// for a `canvas` element - TASK-1.117's plan §2).
#[derive(Default)]
pub(crate) struct RowCache {
    lines: HashMap<u64, ShapedLine>,
}

impl RowCache {
    fn shaped(&mut self, key: u64, build: impl FnOnce() -> ShapedLine) -> ShapedLine {
        if let Some(line) = self.lines.get(&key) {
            return line.clone();
        }
        if self.lines.len() > 4096 {
            self.lines.clear();
        }
        let line = build();
        self.lines.insert(key, line.clone());
        line
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
            clippy::cast_precision_loss,
            reason = "a row index on a terminal screen is a small integer, nowhere near \
                      f32's own precision limit"
        )]
        let y = bounds.origin.y + metrics.line_height * (index as f32);

        let mut column: u16 = 0;
        for run in row {
            let width = run_columns(run);
            if column >= visible_columns {
                break;
            }
            let text_run = to_text_run(run, theme);
            if text_run.len != 0 {
                let key = run_key(index, column, run);
                let shaped = row_cache
                    .borrow_mut()
                    .shaped(key, || shape_run(window, &run.text, text_run));
                let origin = point(bounds.origin.x + metrics.cell_width * f32::from(column), y);
                if let Err(error) = shaped.paint_background(origin, metrics.line_height, window, cx)
                {
                    eprintln!("painting a run's background failed: {error}");
                }
                if let Err(error) = shaped.paint(origin, metrics.line_height, window, cx) {
                    eprintln!("painting a run failed: {error}");
                }
            }
            column = column.saturating_add(width);
        }
    }

    paint_cursor(model.cursor(), metrics, theme, bounds, window);
}

fn to_text_run(run: &Run, theme: &TerminalTheme) -> TextRun {
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
        font: terminal_font(style),
        color: fg,
        background_color: bg,
        underline,
        strikethrough,
    }
}

fn terminal_font(style: &CellStyle) -> gpui::Font {
    let (family, fallbacks) = steg_ui::foundations::FONT_TERMINAL
        .split_first()
        .expect("foundations::FONT_TERMINAL always names at least one family");
    gpui::Font {
        family: (*family).into(),
        features: gpui::FontFeatures::default(),
        fallbacks: Some(gpui::FontFallbacks::from_fonts(
            fallbacks.iter().map(|name| (*name).to_owned()).collect(),
        )),
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

fn shape_run(window: &mut Window, text: &str, run: TextRun) -> ShapedLine {
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
    // `f32::from(u16)` is a lossless widening conversion, not a precision-losing `as` cast.
    let origin = point(
        bounds.origin.x + metrics.cell_width * f32::from(cursor.x),
        bounds.origin.y + metrics.line_height * f32::from(cursor.y),
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
}
