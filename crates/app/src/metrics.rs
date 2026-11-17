//! The terminal's own cell grid, measured from the font steg-ui's foundations name
//! (TASK-1.117's plan §2) - plain data, no `gpui::Window`/`Context` parameter, so this is
//! testable without a window.
//!
//! **The font family, resolved once (TASK-1.126's plan).** gpui 0.2.2's own
//! `TextSystem::resolve_font` (`gpui-0.2.2/src/text_system.rs`, read directly, not
//! guessed) tries the requested `Font.family`, and on failure tries gpui's *own* hardcoded
//! fallback stack (".ZedMono", "Helvetica", "Arial", ...) - never a `Font`'s own
//! `fallbacks` field, which is consulted only for a missing *glyph* inside an
//! already-resolved family. Asking for a family nobody has installed (TASK-1.126's own
//! bug: "JetBrains Mono" on a machine that only has Menlo) silently falls through to
//! whichever of gpui's own stack happens to be present - Helvetica on macOS, a
//! proportional font - which is why the cell width measured below drifted from every
//! glyph actually painted. [`resolve_family`] picks the first of `foundations::FONT_TERMINAL`
//! that `TextSystem::all_font_names()` (public, confirmed in the same file) reports as
//! installed, *before* asking gpui for it at all, so what is measured here and what
//! `terminal_element.rs` paints are resolved from the exact same name - resolving it twice,
//! independently, is the same bug class reintroduced at the next font change.

use gpui::{Bounds, Pixels, SharedString, Window, px};
use steg_ui::foundations;

/// A terminal font's own metrics, measured once (this task never resizes the font), and the
/// family they were measured from - carried alongside them so `terminal_element.rs` paints
/// with the exact family `cell_width`/`line_height` were measured from, never a second,
/// independently resolved one (module doc).
#[derive(Debug, Clone)]
pub struct CellMetrics {
    pub cell_width: Pixels,
    pub line_height: Pixels,
    pub family: SharedString,
}

impl CellMetrics {
    /// Shapes ten `'M'`s at `foundations::TERMINAL`'s own size and divides by ten -
    /// `spikes/task-1.1-terminal-library/probe-core/src/element.rs`'s own
    /// `Metrics::cell_width` technique (steg's own prior MIT code, written fresh here for
    /// this crate, not copied), with `line_height` taken straight from foundations rather
    /// than re-derived, since steg already has the design's own real value.
    #[must_use]
    pub fn measure(window: &mut Window) -> Self {
        let installed = window.text_system().all_font_names();
        let family = resolve_family(foundations::FONT_TERMINAL, &installed).to_owned();
        let font = terminal_font(&family);
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
            family: family.into(),
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

/// The first of `candidates` that `installed` names, or `candidates`'s own first entry if
/// none do (module doc) - pure, no `gpui` type in its signature, so a test can drive it
/// with a fake `installed` list and no window at all.
#[must_use]
pub fn resolve_family<'a>(candidates: &[&'a str], installed: &[String]) -> &'a str {
    candidates
        .iter()
        .find(|candidate| installed.iter().any(|name| name == *candidate))
        .or_else(|| candidates.first())
        .copied()
        .expect("foundations::FONT_TERMINAL always names at least one family")
}

/// Every name `resolve_family` did not choose stays a fallback, for a glyph the chosen
/// family itself lacks (module doc: orthogonal to which family is requested at all, and
/// still wanted - a glyph-level concern, not this function's).
fn terminal_font(family: &str) -> gpui::Font {
    let fallbacks = foundations::FONT_TERMINAL
        .iter()
        .filter(|candidate| **candidate != family)
        .map(|candidate| (*candidate).to_owned())
        .collect();
    gpui::Font {
        family: family.to_owned().into(),
        features: gpui::FontFeatures::default(),
        fallbacks: Some(gpui::FontFallbacks::from_fonts(fallbacks)),
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

    // --- resolve_family (AC#1/AC#2): the first candidate actually installed ------------

    #[test]
    fn the_first_installed_candidate_is_chosen_even_when_it_is_not_first_in_the_list() {
        let candidates = ["JetBrains Mono", "SF Mono", "Menlo"];
        let installed = ["Menlo".to_owned(), "Helvetica".to_owned()];
        assert_eq!(resolve_family(&candidates, &installed), "Menlo");
    }

    #[test]
    fn the_lists_own_first_candidate_is_chosen_when_it_is_installed() {
        let candidates = ["JetBrains Mono", "SF Mono", "Menlo"];
        let installed = ["JetBrains Mono".to_owned(), "Menlo".to_owned()];
        assert_eq!(resolve_family(&candidates, &installed), "JetBrains Mono");
    }

    #[test]
    fn falls_back_to_the_lists_own_first_candidate_when_none_are_installed() {
        let candidates = ["JetBrains Mono", "SF Mono", "Menlo"];
        let installed = ["Helvetica".to_owned(), "Arial".to_owned()];
        assert_eq!(resolve_family(&candidates, &installed), "JetBrains Mono");
    }

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
