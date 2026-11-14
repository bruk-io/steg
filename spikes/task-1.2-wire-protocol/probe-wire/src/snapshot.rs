//! The snapshot: plain data, with no emulator needed to read it.
//!
//! Both data candidates begin with one, because output printed while no client was attached has
//! no byte stream left to replay (AC #2). A row is attribute runs rather than a cell array: it
//! is what a real implementation would send, and it keeps the measured snapshot sizes honest.

use serde::{Deserialize, Serialize};

pub const FLAG_BOLD: u8 = 1;
pub const FLAG_ITALIC: u8 = 2;
pub const FLAG_UNDERLINE: u8 = 4;
pub const FLAG_INVERSE: u8 = 8;
pub const FLAG_DIM: u8 = 16;

/// A stretch of one row with the same attributes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Run {
    pub text: String,
    /// Packed 0x00RRGGBB, already resolved from the palette by the server.
    pub fg: u32,
    pub bg: u32,
    pub flags: u8,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Row {
    pub runs: Vec<Run>,
}

impl Row {
    /// The row's characters with no attributes, trailing blanks removed, which is what
    /// `pane capture` prints and what the reattach oracle compares.
    pub fn text(&self) -> String {
        let mut line = String::new();
        for run in &self.runs {
            line.push_str(&run.text);
        }
        line.trim_end().to_owned()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cursor {
    pub col: u16,
    pub row: u16,
    pub visible: bool,
}

/// Everything a client needs to show a pane as it is right now, taken at `seq`.
///
/// `seq` is what makes a reattach correct: the stream that follows a snapshot resumes at
/// `seq + 1`, and the server produces the snapshot and that subscription in one step on the
/// thread that owns the terminal, so no output is lost or repeated between them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Snapshot {
    pub seq: u64,
    pub cols: u16,
    pub rows: u16,
    pub screen: Vec<Row>,
    /// Oldest first. Up to the server's limit (R15: 10,000 lines by default).
    pub scrollback: Vec<Row>,
    /// How many rows have ever scrolled off this pane. With it, every scrollback row has an
    /// absolute id (`scrollback[0]` is `rows_ever - scrollback.len()`), so a client can ask for a
    /// page of scrollback and know exactly where it belongs, even while output continues. A
    /// window index cannot do that: the window slides, and the client would stitch two different
    /// moments together.
    pub rows_ever: u64,
    pub cursor: Cursor,
    pub alt_screen: bool,
}

impl Snapshot {
    /// The pane as text, as `pane capture` prints it.
    pub fn capture(&self, with_scrollback: bool) -> String {
        let mut out = String::new();
        if with_scrollback {
            out.push_str(&render_rows(&self.scrollback));
        }
        out.push_str(&render_rows(&self.screen));
        out
    }
}

/// Rows as lines of text, one per row, blank rows kept so the screen's shape survives.
pub fn render_rows(rows: &[Row]) -> String {
    let mut out = String::with_capacity(rows.len() * 16);
    for row in rows {
        out.push_str(&row.text());
        out.push('\n');
    }
    out
}
