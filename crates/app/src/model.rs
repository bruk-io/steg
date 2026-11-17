//! The terminal's own drawable state: a grid of rows, fixed at its own row count, and a
//! cursor (TASK-1.117's plan, part 1). No `Screen.size`/`ScreenUpdate::Delta.size` field
//! exists until TASK-1.114 lands, so the row count is read once, from the attach
//! snapshot, and never recomputed from an update's own content - a later task teaches the
//! model to track a reported size instead. There is no equivalent fixed column count:
//! see [`Model::from_snapshot`]'s own doc for why deriving one from a row's content is not
//! reliable, and what bounds columns instead.
//!
//! Plain data: no `gpui::Window`/`Context` parameter anywhere in this file, which is what
//! lets it be tested without a window (TASK-1.117's plan, §4).

use steg_sdk::{Cursor, Row, Screen, ScreenUpdate, Wide};
use unicode_segmentation::UnicodeSegmentation;

/// How many columns one run's own text spans: one cell per grapheme cluster it covers,
/// two for a `Wide` run (its own spacer is never sent as a separate run - confirmed by
/// reading `crates/kernel/src/terminal.rs`'s own `build_row`, which skips a `SpacerTail`
/// cell entirely rather than emitting a run for it) - `SpacerHead`/`Narrow` are one cell
/// per grapheme, same as a run with no special width at all. Counts grapheme clusters,
/// not `char`s: a `Run`'s own text is "one grapheme cluster per cell it covers"
/// (`crates/sdk/src/screen.rs`'s own doc), and a combining mark is more than one `char`
/// but one grapheme and one cell.
pub(crate) fn run_columns(run: &steg_sdk::Run) -> u16 {
    #[expect(
        clippy::cast_possible_truncation,
        reason = "a single run's own grapheme count never approaches u16::MAX on any real \
                  terminal width"
    )]
    let graphemes = run.text.graphemes(true).count() as u16;
    match run.style.wide {
        Wide::Wide => graphemes * 2,
        Wide::Narrow | Wide::SpacerHead => graphemes,
        // Never produced by the kernel's own row builder (skipped at the source), but
        // matched explicitly rather than wildcarded (rust.md: "match your own enums
        // without a `_` arm") - one column, the same as a narrow cell, is the closest
        // honest answer if one ever did arrive.
        Wide::SpacerTail => 1,
    }
}

/// The terminal's drawable state: a fixed grid, sized once, and its rows and cursor as the
/// server last reported them.
pub struct Model {
    row_count: u16,
    grid: Vec<Row>,
    cursor: Cursor,
}

impl Model {
    /// Builds the model from `pane.attach`'s own snapshot: its row count is fixed, here,
    /// for the rest of part 1's life (TASK-1.117's plan §4: part 1 never declares a size
    /// and never recomputes one from an update). A row's own *column* width is not a
    /// reliable stand-in for the pane's own column count, corrected after building this
    /// against a real server: a blank row (nothing printed into it yet, the common case
    /// right after `pane.attach`, before a shell's first prompt has reached the model)
    /// reports zero runs and so zero columns, which a "first row's width" rule would
    /// wrongly read as "this pane is zero columns wide" - observed directly (the model's
    /// own `columns` came back `0` against a freshly attached pane, which painted nothing
    /// at all even once real rows arrived, because the paint loop capped every row's own
    /// width to that wrong `0`). There is no column count on the wire to read instead on
    /// this branch alone (`pane.list`'s own `size` field is TASK-1.114's, not populated
    /// here) - so this model tracks only the row count, which `screen.rows.len()` always
    /// gives correctly regardless of content, and leaves how many columns of a row's own
    /// content fit the window to the painter's own pixel bound instead (`terminal_element`
    /// §2), never to a count derived from a row that might legitimately be blank.
    #[must_use]
    pub fn from_snapshot(screen: Screen) -> Self {
        #[expect(
            clippy::cast_possible_truncation,
            reason = "a terminal's own row count is bounded by its own scrollback/screen \
                      size, never near u16::MAX"
        )]
        let row_count = screen.rows.len() as u16;
        Self {
            row_count,
            grid: screen.rows,
            cursor: screen.cursor,
        }
    }

    /// The grid's own fixed row count (TASK-1.117's plan §4: part 1 never declares one,
    /// and never recomputes it from an update - see this type's own doc for why there is
    /// no equivalent fixed column count here).
    #[must_use]
    pub fn row_count(&self) -> u16 {
        self.row_count
    }

    #[must_use]
    pub fn rows(&self) -> &[Row] {
        &self.grid
    }

    #[must_use]
    pub fn cursor(&self) -> Cursor {
        self.cursor
    }

    /// Applies one update: only the rows a `Delta` names, or the whole grid on a `Full`
    /// resync - O(rows changed), never O(screen), on a delta (TASK-1.117's plan §2).
    /// `(columns, rows)` is never touched here - that is the whole point of this type for
    /// part 1 (§4's own "no size logic at all").
    pub fn apply(&mut self, update: ScreenUpdate) {
        match update {
            ScreenUpdate::Unchanged => {}
            ScreenUpdate::Delta { rows, cursor, .. } => {
                for (index, row) in rows {
                    if let Some(slot) = self.grid.get_mut(usize::from(index)) {
                        *slot = row;
                    }
                }
                self.cursor = cursor;
            }
            ScreenUpdate::Full { screen, .. } => {
                self.grid = screen.rows;
                self.cursor = screen.cursor;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use steg_sdk::{CellStyle, CursorShape, Run, Size, Underline};

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
            underline: Underline::None,
            underline_colour: None,
            wide: Wide::Narrow,
            link: false,
        }
    }

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

    fn cursor() -> Cursor {
        Cursor {
            x: 0,
            y: 0,
            visible: true,
            blinking: false,
            shape: CursorShape::Block,
        }
    }

    fn row(runs: Vec<Run>) -> Row {
        runs
    }

    fn snapshot(rows: Vec<Row>) -> Screen {
        // TASK-1.114: `Screen` now carries its own `size`, unused by this
        // file's own logic (module doc above) - an arbitrary valid size
        // keeps this fixture compiling, 80 columns matching every row
        // built by this test module's own `narrow_run`/`wide_run` helpers.
        let size = Size::new(80, rows.len() as u16);
        Screen {
            rows,
            cursor: cursor(),
            size,
        }
    }

    #[test]
    fn a_narrow_runs_columns_are_its_own_grapheme_count() {
        assert_eq!(run_columns(&narrow_run("hello")), 5);
    }

    #[test]
    fn a_wide_runs_columns_are_twice_its_own_grapheme_count() {
        // Two wide (CJK) characters, one run (same style) - each is one grapheme and two
        // columns: itself, and the spacer the kernel's own row builder never sends.
        assert_eq!(run_columns(&wide_run("\u{4f60}\u{597d}")), 4);
    }

    #[test]
    fn a_combining_mark_is_one_grapheme_and_one_column_not_two() {
        // "e" + combining acute accent (U+0301): two `char`s, one grapheme cluster, one
        // cell - the reason this counts graphemes, not chars (module doc).
        assert_eq!(run_columns(&narrow_run("e\u{0301}")), 1);
    }

    #[test]
    fn from_snapshot_reads_its_fixed_row_count_once() {
        let model = Model::from_snapshot(snapshot(vec![
            row(vec![narrow_run("12345")]),
            row(vec![narrow_run("     ")]),
        ]));
        assert_eq!(model.row_count(), 2);
    }

    #[test]
    fn a_blank_first_row_does_not_change_the_row_count() {
        // Found by running the real app against a real server (TASK-1.117): a pane
        // attached before its shell had printed anything has every row's own run list
        // empty, so a row count derived from content, not `rows.len()`, would read as 0 -
        // this is the regression this test pins down (`Model::from_snapshot`'s own doc).
        let model = Model::from_snapshot(snapshot(vec![row(Vec::new()), row(Vec::new())]));
        assert_eq!(model.row_count(), 2);
    }

    #[test]
    fn a_delta_touches_only_the_rows_it_names_and_never_resizes_the_grid() {
        let mut model = Model::from_snapshot(snapshot(vec![
            row(vec![narrow_run("aaaaa")]),
            row(vec![narrow_run("bbbbb")]),
            row(vec![narrow_run("ccccc")]),
        ]));
        model.apply(ScreenUpdate::Delta {
            rows: vec![(1, row(vec![narrow_run("BBBBB")]))],
            cursor: cursor(),
            seq: 1,
        });
        assert_eq!(model.rows()[0], row(vec![narrow_run("aaaaa")]));
        assert_eq!(model.rows()[1], row(vec![narrow_run("BBBBB")]));
        assert_eq!(model.rows()[2], row(vec![narrow_run("ccccc")]));
        // The fixed row count from `from_snapshot` is unchanged - no field on
        // `ScreenUpdate` on this branch could have changed it anyway, which is the point
        // this test proves: nothing in `apply` re-derives it from a delta's own content.
        assert_eq!(model.row_count(), 3);
    }

    #[test]
    fn a_full_update_replaces_the_whole_grid_but_not_the_fixed_row_count_it_was_built_with() {
        let mut model = Model::from_snapshot(snapshot(vec![row(vec![narrow_run("aaaaa")])]));
        model.apply(ScreenUpdate::Full {
            screen: snapshot(vec![row(vec![narrow_run("zzzzz")])]),
            seq: 7,
        });
        assert_eq!(model.rows(), &[row(vec![narrow_run("zzzzz")])]);
        assert_eq!(model.row_count(), 1);
    }

    #[test]
    fn unchanged_touches_nothing() {
        let mut model = Model::from_snapshot(snapshot(vec![row(vec![narrow_run("aaaaa")])]));
        model.apply(ScreenUpdate::Unchanged);
        assert_eq!(model.rows(), &[row(vec![narrow_run("aaaaa")])]);
    }
}
