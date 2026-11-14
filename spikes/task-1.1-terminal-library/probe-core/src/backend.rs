//! What both candidates have to provide, so every measurement runs identical code over them.
//!
//! The trait is deliberately **not** `Send`: `libghostty-vt`'s handles are `!Send + !Sync` by
//! design, and a `Send` bound here would quietly rule one candidate out before it was measured.

/// One cell of the screen, as a renderer needs it.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Cell {
    pub ch: char,
    pub fg: Rgb,
    pub bg: Rgb,
    pub bold: bool,
    pub underline: bool,
}

/// A colour as both emulators finally resolve one: 24-bit, no palette indirection left.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Rgb {
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    /// The 0xRRGGBB form gpui's `rgb()` takes.
    pub fn hex(self) -> u32 {
        (u32::from(self.r) << 16) | (u32::from(self.g) << 8) | u32::from(self.b)
    }
}

/// The visible screen at one instant, with the text of each row alongside its cells, because the
/// renderer shapes a row as one string and the conformance checks read rows as text.
pub struct Snapshot {
    pub rows: Vec<Row>,
    pub cursor_col: u16,
    pub cursor_row: u16,
    pub cursor_visible: bool,
}

pub struct Row {
    pub text: String,
    pub cells: Vec<Cell>,
}

impl Snapshot {
    /// The screen as lines of text with trailing blanks removed, which is how a terminal
    /// behaviour is checked: bytes in, screen back (`.claude/rules/testing-rust.md`).
    pub fn lines(&self) -> Vec<String> {
        self.rows
            .iter()
            .map(|row| row.text.trim_end().to_owned())
            .collect()
    }

    /// Whether any row contains this text.
    pub fn contains(&self, needle: &str) -> bool {
        self.rows.iter().any(|row| row.text.contains(needle))
    }
}

/// What the emulator tells the application about, beyond the grid.
///
/// `None` means *this library does not report it* and steg would have to implement it: that
/// distinction is the point of the R14 matrix, so it is in the type rather than in prose.
#[derive(Default)]
pub struct Facts {
    pub title: Option<String>,
    pub pwd: Option<String>,
    pub mouse_tracking: Option<bool>,
    pub bracketed_paste: Option<bool>,
    pub alt_screen: Option<bool>,
    pub kitty_keyboard: Option<bool>,
    pub hyperlink_at: Option<String>,
    pub clipboard_writes: Option<Vec<String>>,
    pub semantic_prompt: Option<bool>,
    pub unknown_osc: Option<String>,
    pub pty_replies: Vec<u8>,
}

/// A terminal emulator, as the probe uses one.
pub trait TerminalBackend {
    /// The name that appears in every report.
    fn name() -> &'static str
    where
        Self: Sized;

    /// The version of the library being measured, for the report's header.
    fn version() -> &'static str
    where
        Self: Sized;

    fn open(cols: u16, rows: u16, scrollback: usize) -> anyhow::Result<Self>
    where
        Self: Sized;

    /// Feed bytes from the pseudo-terminal.
    fn feed(&mut self, bytes: &[u8]);

    /// Read the visible screen back.
    fn snapshot(&mut self) -> Snapshot;

    /// Which rows changed since the last call, if the library tracks that. `None` means it does
    /// not, and a renderer must redraw everything or diff the grid itself.
    fn dirty_rows(&mut self) -> Option<Vec<u16>>;

    fn resize(&mut self, cols: u16, rows: u16);

    /// How many rows of scrollback are held right now.
    fn scrollback_rows(&self) -> usize;

    /// Everything reported outside the grid, drained.
    fn facts(&mut self) -> Facts;
}
