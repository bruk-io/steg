//! The shapes a client draws a terminal's screen from.
//!
//! doc-16 R105's named exception: the terminal engine and the behaviour
//! that reads it stay in the kernel, but what it holds and what a client
//! draws from are different shapes, and this module is the second one
//! (decision-2 section 3, doc-9 R14).
//!
//! TASK-1.47 puts these on the wire (this module's own doc used to say
//! "whichever task" - this is it, R106): every shape an attachment's own
//! stream carries (the snapshot, and the updates that follow it) derives
//! `serde`, already allowed in this crate for exactly this (`boundaries.
//! toml`'s `[steg-sdk]`). `RowId`/`Page` stay undecorated here - scrollback
//! paging is TASK-1.115's own.

use serde::{Deserialize, Serialize};

/// A resolved 24-bit colour.
///
/// Always resolved: the server resolves a cell's palette index, or its
/// default colour, before it reaches here, so a client never holds a
/// palette (decision-2 section 3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Rgb {
    /// Red component.
    pub r: u8,
    /// Green component.
    pub g: u8,
    /// Blue component.
    pub b: u8,
}

/// The underline styles a cell can carry (decision-2 section 9).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Underline {
    /// No underline.
    None,
    /// A single line.
    Single,
    /// Two lines.
    Double,
    /// A wavy line.
    Curly,
    /// A dotted line.
    Dotted,
    /// A dashed line.
    Dashed,
}

/// Whether a cell is a wide character, the spacer after one, or neither
/// (decision-2 section 9).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Wide {
    /// Not a wide character; this cell is one column wide.
    Narrow,
    /// A wide character; this cell and the next are its two columns.
    Wide,
    /// The second column of the wide character before this one. Carries
    /// no text of its own and is never drawn.
    SpacerTail,
    /// A filler cell at the end of a soft-wrapped line, left by a wide
    /// character that did not fit.
    SpacerHead,
}

/// The visual attributes shared by every cell in one [`Run`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CellStyle {
    /// The resolved foreground colour, or `None` for the default.
    pub fg: Option<Rgb>,
    /// The resolved background colour, or `None` for the default.
    pub bg: Option<Rgb>,
    /// Bold.
    pub bold: bool,
    /// Italic.
    pub italic: bool,
    /// Faint (reduced intensity).
    pub faint: bool,
    /// Blinking.
    pub blink: bool,
    /// Foreground and background swapped.
    pub inverse: bool,
    /// Invisible (text hidden, cell still occupies its column).
    pub invisible: bool,
    /// Struck through.
    pub strikethrough: bool,
    /// A line above the text.
    pub overline: bool,
    /// The underline style, if any.
    pub underline: Underline,
    /// The underline's own colour, or `None` to use the foreground.
    pub underline_colour: Option<Rgb>,
    /// Whether this cell is a wide character or one of its spacers.
    pub wide: Wide,
    /// Whether this cell is part of a hyperlink (doc-9 R116).
    ///
    /// Presence only: a link's address is asked for by cell, not carried
    /// here, and two adjacent links are not told apart by this flag alone.
    pub link: bool,
}

/// One run of cells sharing the same [`CellStyle`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Run {
    /// The run's text, one grapheme cluster per cell it covers (fewer
    /// characters than cells when it covers a wide character, since the
    /// wide character's spacer carries no text of its own).
    pub text: String,
    /// The style shared by every cell this run covers.
    pub style: CellStyle,
}

/// One screen row, as the attribute runs that make it up (decision-2
/// section 3: "a row is attribute runs ..., not a cell array").
pub type Row = Vec<Run>;

/// The cursor's visual shape (decision-2 section 9).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CursorShape {
    /// A thin vertical bar.
    Bar,
    /// A filled block.
    Block,
    /// A line under the cell.
    Underline,
    /// An outlined, unfilled block.
    BlockHollow,
}

/// Where the cursor is, and how it should be drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cursor {
    /// Column, zero-indexed.
    pub x: u16,
    /// Row, zero-indexed.
    pub y: u16,
    /// Whether the cursor is visible at all.
    pub visible: bool,
    /// Whether the cursor should blink.
    pub blinking: bool,
    /// The cursor's shape.
    pub shape: CursorShape,
}

/// A terminal's screen: every row, and the cursor (AC #1).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Screen {
    /// The screen's rows, top to bottom.
    pub rows: Vec<Row>,
    /// The cursor.
    pub cursor: Cursor,
}

/// `pane.attach`'s own reply (decision-2 section 3; TASK-1.46's
/// `Returns::Nested`/`Method.follows`): the new stream's id, and the
/// pane's screen at the moment of attaching. `stream` is the field
/// `Follows::Attachment::stream_id_field` names - a generated client
/// reads it to know which stream the updates that follow arrive on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PaneAttached {
    /// The attachment's own stream id, handed out by this reply (decision-2
    /// section 3: "every other stream id belongs to one attachment, and
    /// the server hands it out in the attach result").
    pub stream: u32,
    /// The pane's screen at the exact moment this attachment was
    /// registered - the point the updates that follow resume from.
    pub snapshot: Screen,
}

/// The absolute identity of a row: assigned once, by the server, and never
/// reused for different content within its [`epoch`](RowId::epoch)
/// (doc-9 R15; decision-2 section 3, "scrollback rows need absolute
/// identity, not a window position").
///
/// Scoped to an epoch because a primary-screen resize that changes the
/// column count reflows every row: this can inflate the row count with
/// nothing lost, or prune rows outright, and there is no way to tell the
/// two apart from outside the engine (a row-count-only resize does
/// neither - it only moves rows between scrollback and the active area).
/// So a column-count change ends the current epoch rather than trying to
/// map an old id onto its new position: every id from an earlier epoch is
/// answered as gone (`Page::rows` empty), never silently resolved to
/// different content.
///
/// Ordered by `epoch` first, then `offset`, so a later epoch always
/// compares greater regardless of its own offsets, and within one epoch a
/// caller can tell whether one id is ahead of or behind another.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RowId {
    /// Which identity epoch this id belongs to.
    pub epoch: u64,
    /// This row's position within its epoch: strictly greater than every
    /// id already retired from the same epoch, and never reused for
    /// different content.
    pub offset: u64,
}

/// The rows that changed since a point a caller held, or a whole screen
/// when that point can no longer be served from (AC #4).
///
/// "The given point" is the `seq` a previous [`ScreenUpdate::Delta`] or
/// [`ScreenUpdate::Full`] carried: a caller holds it and hands it back on
/// its next call. There is exactly one current point per terminal, not
/// one per caller (decision-2 section 6's back-pressure rule, "a client
/// that falls behind gets one fresh screen-sized snapshot," holds for
/// every caller but the single most recent one to be served).
///
/// Named `ScreenUpdate`, not `Update` (TASK-1.47's own rename, before this
/// type ever crossed the wire): `of` (`Returns::Nested`) is a bare type
/// name with no module path in front of it in `api show`'s own output, and
/// doc-14 already uses the plain word "update" for layout/session/tab
/// changes elsewhere in the API - a name that reads unambiguously there
/// costs nothing here, since nothing outside this crate names the old one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScreenUpdate {
    /// The point given was already current, and nothing has changed since.
    Unchanged,
    /// The rows that changed, each by its position from the top of the
    /// screen (not a [`RowId`]: only scrollback and screen paging are
    /// addressed that way - a live update is always the current screen,
    /// so a plain position is enough and matches what a client already
    /// holds from its last snapshot or delta).
    Delta {
        /// Each changed row, by its position from the top of the screen.
        rows: Vec<(u16, Row)>,
        /// The cursor, read on every call.
        cursor: Cursor,
        /// The point to hold and give back on the next call.
        seq: u64,
    },
    /// The point given was `None`, stale, or belonged to a caller other
    /// than the one most recently served: the whole screen, and the point
    /// to hold from here.
    Full {
        /// The whole screen.
        screen: Screen,
        /// The point to hold and give back on the next call.
        seq: u64,
    },
}

/// A page of rows fetched by absolute identity (AC #2, #3; decision-2
/// section 3's "a page request says which rows it wants and the answer
/// says which rows it got").
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Page {
    /// The rows this page actually holds, each with its own id.
    ///
    /// Empty means every row asked for is gone - already pruned, or from
    /// an epoch that has since ended - never a short page padded with
    /// substitutes, and never an error: asking for rows that are gone
    /// answers that they are gone.
    pub rows: Vec<(RowId, Row)>,
}

#[cfg(test)]
mod tests {
    use super::{
        CellStyle, Cursor, CursorShape, PaneAttached, Rgb, Run, Screen, ScreenUpdate, Underline,
        Wide,
    };

    fn round_trips<T>(value: &T)
    where
        T: serde::Serialize + for<'de> serde::Deserialize<'de> + PartialEq + std::fmt::Debug,
    {
        let json = serde_json::to_vec(value).expect("this task's own shapes always serialise");
        let back: T = serde_json::from_slice(&json)
            .expect("what this task's own shapes serialise always deserialises back");
        assert_eq!(&back, value, "round trip through JSON changed the value");
    }

    /// Exercises every field and every `CellStyle`-adjacent variant at
    /// once, so a forgotten `#[derive(Serialize, Deserialize)]` on a
    /// nested type fails this test rather than silently serialising as
    /// `null` or being skipped by a narrower test that never reaches it.
    fn every_underline_and_wide_style() -> Vec<CellStyle> {
        let mut styles = Vec::new();
        for underline in [
            Underline::None,
            Underline::Single,
            Underline::Double,
            Underline::Curly,
            Underline::Dotted,
            Underline::Dashed,
        ] {
            for wide in [Wide::Narrow, Wide::Wide, Wide::SpacerTail, Wide::SpacerHead] {
                for (fg, bg, underline_colour) in [
                    (None, None, None),
                    (
                        Some(Rgb { r: 1, g: 2, b: 3 }),
                        Some(Rgb { r: 4, g: 5, b: 6 }),
                        Some(Rgb { r: 7, g: 8, b: 9 }),
                    ),
                ] {
                    styles.push(CellStyle {
                        fg,
                        bg,
                        bold: true,
                        italic: false,
                        faint: true,
                        blink: false,
                        inverse: true,
                        invisible: false,
                        strikethrough: true,
                        overline: false,
                        underline,
                        underline_colour,
                        wide,
                        link: true,
                    });
                }
            }
        }
        styles
    }

    #[test]
    fn every_cell_style_combination_round_trips() {
        for style in every_underline_and_wide_style() {
            round_trips(&style);
        }
    }

    fn sample_screen() -> Screen {
        Screen {
            rows: every_underline_and_wide_style()
                .into_iter()
                .map(|style| {
                    vec![Run {
                        text: "x".to_string(),
                        style,
                    }]
                })
                .collect(),
            cursor: Cursor {
                x: 3,
                y: 5,
                visible: true,
                blinking: false,
                shape: CursorShape::BlockHollow,
            },
        }
    }

    #[test]
    fn a_screen_with_every_cell_style_round_trips() {
        round_trips(&sample_screen());
    }

    #[test]
    fn every_cursor_shape_round_trips() {
        for shape in [
            CursorShape::Bar,
            CursorShape::Block,
            CursorShape::Underline,
            CursorShape::BlockHollow,
        ] {
            round_trips(&Cursor {
                x: 0,
                y: 0,
                visible: true,
                blinking: true,
                shape,
            });
        }
    }

    #[test]
    fn screen_update_unchanged_round_trips() {
        round_trips(&ScreenUpdate::Unchanged);
    }

    #[test]
    fn screen_update_delta_round_trips() {
        round_trips(&ScreenUpdate::Delta {
            rows: vec![(0, sample_screen().rows[0].clone()), (2, vec![])],
            cursor: sample_screen().cursor,
            seq: 42,
        });
    }

    #[test]
    fn screen_update_full_round_trips() {
        round_trips(&ScreenUpdate::Full {
            screen: sample_screen(),
            seq: 7,
        });
    }

    /// `pane.attach`'s own reply shape (TASK-1.46's `Returns::Nested`
    /// names this type by `"PaneAttached"`): the stream id and the
    /// snapshot travel as one JSON object, not two separate values.
    #[test]
    fn pane_attached_round_trips_and_carries_both_fields_in_one_object() {
        let attached = PaneAttached {
            stream: 9,
            snapshot: sample_screen(),
        };
        round_trips(&attached);

        let value = serde_json::to_value(&attached).unwrap();
        let object = value
            .as_object()
            .expect("PaneAttached serialises as an object");
        assert!(
            object.contains_key("stream"),
            "the field `Follows::Attachment::stream_id_field` names must really exist: {object:?}"
        );
        assert_eq!(object.get("stream").unwrap(), 9);
        assert!(object.contains_key("snapshot"));
    }
}
