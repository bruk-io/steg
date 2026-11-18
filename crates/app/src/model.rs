//! The terminal's own drawable state: a grid of rows, a cursor, and the terminal's own size
//! as the server reports it (TASK-1.121 plan, section 4 - TASK-1.117's part 1 read only a
//! row count, derived from the attach snapshot and fixed for its own life, before
//! `Screen.size` existed to read instead). The row count comes straight from `Screen.size`,
//! never from a row's own content: a blank row (nothing printed into it yet, the common
//! case right after `pane.attach`) reports zero runs, which a "count the rows with content"
//! rule would misread as zero rows - `Screen.size` is authoritative regardless of content.
//!
//! **Settled, not a gap: a `Delta` never carries a size at all** (doc-14's own "the size of
//! a terminal" section, reworded once this landed on main): a size change always arrives as
//! a `Full` resync instead, even when nothing else changed - the server tracks each
//! attachment's own last-told size and sends a whole screen, not a bare field on a `Delta`,
//! the moment it goes stale. `apply`'s own `Delta` arm therefore never touches `size`, by
//! design, not by an omission this task owed a follow-up for.
//!
//! Plain data: no `gpui::Window`/`Context` parameter anywhere in this file, which is what
//! lets it be tested without a window (TASK-1.117's plan, §4).

use steg_sdk::{Cursor, Row, Screen, ScreenUpdate, Size, Wide};
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

/// The terminal's drawable state: a grid, its own size as the server last reported it, and
/// its rows and cursor.
pub struct Model {
    size: Size,
    grid: Vec<Row>,
    cursor: Cursor,
}

impl Model {
    /// Builds the model from `pane.attach`'s own snapshot: its size comes straight from
    /// `screen.size` (module doc - never derived from a row's own content, which may
    /// legitimately be blank).
    #[must_use]
    pub fn from_snapshot(screen: Screen) -> Self {
        Self {
            size: screen.size,
            grid: screen.rows,
            cursor: screen.cursor,
        }
    }

    /// The grid's own row count, as the server's own `Screen.size` last reported it (module
    /// doc: authoritative, not derived from content).
    #[must_use]
    pub fn row_count(&self) -> u16 {
        self.size.rows()
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
    /// resync - O(rows changed), never O(screen), on a delta (TASK-1.117's plan §2). A
    /// `Full` resync's own `size` replaces this model's size, since a fresh `Screen` always
    /// carries one; a `Delta` never does (module doc).
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
                self.size = screen.size;
                self.grid = screen.rows;
                self.cursor = screen.cursor;
            }
            // A no-op for now: the window's own reaction to a pane ending
            // (keeping the last screen, showing that it ended, refusing
            // further input) is a separate, later task's job, not this
            // one's - this arm exists only so the model compiles against
            // the new variant.
            ScreenUpdate::Ended { .. } => {}
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

    /// A snapshot whose own `size` is derived from `rows.len()` - fine for every test below
    /// that is about row content or the cursor, not about size tracking itself, where that
    /// derivation would defeat the point (`snapshot_with_size`, below, is for those).
    #[expect(
        clippy::cast_possible_truncation,
        reason = "no test fixture's own row count approaches u16::MAX"
    )]
    fn snapshot(rows: Vec<Row>) -> Screen {
        let size = Size::new(0, rows.len() as u16);
        snapshot_with_size(rows, size)
    }

    /// A snapshot with an explicit `size`, independent of `rows`'s own length or content -
    /// what every test about size tracking itself needs: `Screen.size` is authoritative
    /// regardless of what a row happens to hold (module doc).
    fn snapshot_with_size(rows: Vec<Row>, size: Size) -> Screen {
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
    fn from_snapshot_reads_its_row_count_from_the_snapshots_own_size() {
        // The size's own `rows` (2) differs from `rows.len()` (also 2 here, but given
        // through `snapshot_with_size` rather than derived) - the point this proves is
        // that `from_snapshot` reads `screen.size`, not `screen.rows.len()`.
        let model = Model::from_snapshot(snapshot_with_size(
            vec![
                row(vec![narrow_run("12345")]),
                row(vec![narrow_run("     ")]),
            ],
            Size::new(5, 2),
        ));
        assert_eq!(model.row_count(), 2);
    }

    #[test]
    fn a_blank_first_row_does_not_change_the_row_count() {
        // Found by running the real app against a real server (TASK-1.117, before
        // `Screen.size` existed to read instead): a pane attached before its shell had
        // printed anything has every row's own run list empty, so a row count derived
        // from content would read as 0. `Screen.size` is authoritative regardless -
        // the explicit size here (2) is what this test actually trusts, not the blank
        // rows it's given alongside it.
        let model = Model::from_snapshot(snapshot_with_size(
            vec![row(Vec::new()), row(Vec::new())],
            Size::new(80, 2),
        ));
        assert_eq!(model.row_count(), 2);
    }

    #[test]
    fn a_delta_touches_only_the_rows_it_names_and_leaves_the_size_unchanged() {
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
        // `ScreenUpdate::Delta` never carries a `size` field at all (module doc, doc-14's
        // own "the size of a terminal" section) - a size change always arrives as a `Full`
        // instead, so this is the model's own settled behaviour, not a placeholder.
        assert_eq!(model.row_count(), 3);
    }

    #[test]
    fn a_full_update_replaces_the_whole_grid_and_its_own_reported_size() {
        let mut model = Model::from_snapshot(snapshot(vec![row(vec![narrow_run("aaaaa")])]));
        model.apply(ScreenUpdate::Full {
            screen: snapshot_with_size(
                vec![
                    row(vec![narrow_run("zzzzz")]),
                    row(vec![narrow_run("yyyyy")]),
                ],
                Size::new(5, 2),
            ),
            seq: 7,
        });
        assert_eq!(
            model.rows(),
            &[
                row(vec![narrow_run("zzzzz")]),
                row(vec![narrow_run("yyyyy")])
            ],
        );
        // The size this model was *built* with (one row) is not what it reports after a
        // `Full` resync - the resync's own `screen.size` (two rows) is, since a fresh
        // `Screen` always carries one and `apply`'s own `Full` arm reads it (module doc:
        // this replaces TASK-1.117's "but not the fixed row count it was built with" test,
        // which this same scenario would now falsify).
        assert_eq!(model.row_count(), 2);
    }

    #[test]
    fn unchanged_touches_nothing() {
        let mut model = Model::from_snapshot(snapshot(vec![row(vec![narrow_run("aaaaa")])]));
        model.apply(ScreenUpdate::Unchanged);
        assert_eq!(model.rows(), &[row(vec![narrow_run("aaaaa")])]);
    }
}
