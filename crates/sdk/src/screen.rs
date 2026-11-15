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
