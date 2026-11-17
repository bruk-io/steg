//! The terminal's own cell grid, measured from the font steg-ui's foundations name
//! (TASK-1.117's plan §2) - plain data, no `gpui::Window`/`Context` parameter, so this is
//! testable without a window.

use gpui::{Bounds, Pixels, Window, px};
use steg_ui::foundations;

/// A terminal font's own metrics, measured once (this task never resizes the font).
#[derive(Debug, Clone, Copy)]
pub struct CellMetrics {
    pub cell_width: Pixels,
    pub line_height: Pixels,
}

impl CellMetrics {
    /// Shapes ten `'M'`s at `foundations::TERMINAL`'s own size and divides by ten -
    /// `spikes/task-1.1-terminal-library/probe-core/src/element.rs`'s own
    /// `Metrics::cell_width` technique (steg's own prior MIT code, written fresh here for
    /// this crate, not copied), with `line_height` taken straight from foundations rather
    /// than re-derived, since steg already has the design's own real value.
    #[must_use]
    pub fn measure(window: &mut Window) -> Self {
        let font = terminal_font();
        let font_size = foundations::TERMINAL.size;
        let probe: gpui::SharedString = "MMMMMMMMMM".into();
        let run = gpui::TextRun {
            len: probe.len(),
            font,
            color: gpui::black(),
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        let line = window
            .text_system()
            .shape_line(probe, font_size, &[run], None);
        Self {
            cell_width: line.width / 10.0,
            line_height: foundations::TERMINAL.line_height,
        }
    }

    /// How many whole rows of the grid fit inside `bounds` (the crop rule, when the
    /// window is smaller than the fixed grid) - never more than `max_rows`, since part 1
    /// never asks the grid to be taller than its own fixed row count. There is no
    /// equivalent `max_columns` here: a row's own column count is not reliably known on
    /// this branch (`model::Model`'s own doc), so how many columns of a row's own content
    /// are drawn is bounded only by the window's own pixel width, below.
    #[must_use]
    pub fn visible_rows(&self, bounds: Bounds<Pixels>, max_rows: u16) -> u16 {
        columns_that_fit(bounds.size.height, self.line_height).min(max_rows)
    }

    /// How many whole columns fit inside `bounds` - the one bound a row's own content is
    /// cropped to (module doc).
    #[must_use]
    pub fn visible_columns(&self, bounds: Bounds<Pixels>) -> u16 {
        columns_that_fit(bounds.size.width, self.cell_width)
    }
}

fn terminal_font() -> gpui::Font {
    let (family, fallbacks) = foundations::FONT_TERMINAL
        .split_first()
        .expect("foundations::FONT_TERMINAL always names at least one family");
    gpui::Font {
        family: (*family).into(),
        features: gpui::FontFeatures::default(),
        fallbacks: Some(gpui::FontFallbacks::from_fonts(
            fallbacks.iter().map(|name| (*name).to_owned()).collect(),
        )),
        weight: gpui::FontWeight::NORMAL,
        style: gpui::FontStyle::Normal,
    }
}

/// How many whole cells of `cell_size` fit in `available` - the pure arithmetic half of
/// `CellMetrics::visible_grid`, pulled out so it has its own tests with no `Window` at all.
#[must_use]
pub fn columns_that_fit(available: Pixels, cell_size: Pixels) -> u16 {
    if cell_size <= px(0.0) {
        return 0;
    }
    #[expect(
        clippy::cast_possible_truncation,
        reason = "a window's own pixel size divided by a cell's never approaches u16::MAX"
    )]
    let fit = (available / cell_size).floor() as u16;
    fit
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pixel_size_that_divides_evenly_fits_exactly() {
        assert_eq!(columns_that_fit(px(100.0), px(10.0)), 10);
    }

    #[test]
    fn a_pixel_size_that_does_not_divide_evenly_floors_not_rounds() {
        // 109px at 10px a cell is 10.9 cells - floors to 10, never rounds up to 11 (a cell
        // that does not fully fit is not drawn, TASK-1.117's plan §2's own crop rule).
        assert_eq!(columns_that_fit(px(109.0), px(10.0)), 10);
    }

    #[test]
    fn zero_available_space_fits_nothing() {
        assert_eq!(columns_that_fit(px(0.0), px(10.0)), 0);
    }

    #[test]
    fn a_zero_sized_cell_fits_nothing_rather_than_dividing_by_zero() {
        assert_eq!(columns_that_fit(px(100.0), px(0.0)), 0);
    }
}
