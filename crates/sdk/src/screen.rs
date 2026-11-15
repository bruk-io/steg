//! The shapes a client draws a terminal's screen from.
//!
//! doc-16 R105's named exception: the terminal engine and the behaviour
//! that reads it stay in the kernel, but what it holds and what a client
//! draws from are different shapes, and this module is the second one
//! (decision-2 section 3, doc-9 R14).
//!
//! Plain data, with no serde derive: this crate adds no dependency for it
//! (`boundaries.toml`'s empty allow list for `steg-sdk` stays empty).
//! Whichever task puts these on the wire adds serialisation at that point
//! (R106).

/// A resolved 24-bit colour.
///
/// Always resolved: the server resolves a cell's palette index, or its
/// default colour, before it reaches here, so a client never holds a
/// palette (decision-2 section 3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Rgb {
    /// Red component.
    pub r: u8,
    /// Green component.
    pub g: u8,
    /// Blue component.
    pub b: u8,
}

/// The underline styles a cell can carry (decision-2 section 9).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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
#[derive(Debug, Clone, PartialEq, Eq)]
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
#[derive(Debug, Clone, PartialEq, Eq)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Screen {
    /// The screen's rows, top to bottom.
    pub rows: Vec<Row>,
    /// The cursor.
    pub cursor: Cursor,
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
/// "The given point" is the `seq` a previous [`Update::Delta`] or
/// [`Update::Full`] carried: a caller holds it and hands it back on its
/// next call. There is exactly one current point per terminal, not one
/// per caller (decision-2 section 6's back-pressure rule, "a client that
/// falls behind gets one fresh screen-sized snapshot," holds for every
/// caller but the single most recent one to be served).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Update {
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
