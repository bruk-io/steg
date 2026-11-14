//! Candidate D1's client: an emulator of its own, fed the snapshot and then the raw bytes.
//!
//! Two costs show up here that are not visible in the message types, and both belong in the
//! answer:
//!
//! - The snapshot has to be turned back into escape sequences to get it into an emulator, because
//!   an emulator's state cannot be set cell by cell from outside. So D1 needs a renderer from the
//!   cell model to VT bytes as well as the server's renderer from the emulator to the cell model:
//!   the same model, implemented twice, in two directions.
//! - Every client needs an emulator, including the app, which makes the choice of library
//!   (TASK-1.1) part of every client rather than of the server alone.

use alacritty_terminal::Term;
use alacritty_terminal::event::VoidListener;
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::{Column, Line};
use alacritty_terminal::term::Config;
use alacritty_terminal::term::cell::Flags;
use alacritty_terminal::vte::ansi::Processor;
use probe_wire::{
    FLAG_BOLD, FLAG_DIM, FLAG_INVERSE, FLAG_ITALIC, FLAG_UNDERLINE, Row, Snapshot, Update,
};

use crate::capture::{SCROLLBACK_LIMIT, lines};

#[derive(Clone, Copy)]
struct Size {
    cols: usize,
    rows: usize,
}

impl Dimensions for Size {
    fn total_lines(&self) -> usize {
        self.rows
    }

    fn screen_lines(&self) -> usize {
        self.rows
    }

    fn columns(&self) -> usize {
        self.cols
    }
}

pub struct Replay {
    term: Term<VoidListener>,
    parser: Processor,
    size: Size,
    pub seq: u64,
    pub resyncs: u64,
    pub byte_frames: u64,
    pub bytes: u64,
}

impl Replay {
    pub fn from_snapshot(snapshot: &Snapshot) -> Self {
        let size = Size {
            cols: usize::from(snapshot.cols).max(1),
            rows: usize::from(snapshot.rows).max(1),
        };
        // Room for the snapshot's scrollback and whatever arrives after it.
        let config = Config {
            scrolling_history: SCROLLBACK_LIMIT + 2_000,
            ..Config::default()
        };
        let mut replay = Self {
            term: Term::new(config, &size, VoidListener),
            parser: Processor::new(),
            size,
            seq: snapshot.seq,
            resyncs: 0,
            byte_frames: 0,
            bytes: 0,
        };
        replay.load(snapshot);
        replay
    }

    fn load(&mut self, snapshot: &Snapshot) {
        let bytes = render_snapshot(snapshot);
        self.parser.advance(&mut self.term, &bytes);
        self.seq = snapshot.seq;
    }

    pub fn feed(&mut self, bytes: &[u8]) {
        self.parser.advance(&mut self.term, bytes);
        self.byte_frames += 1;
        self.bytes += bytes.len() as u64;
    }

    /// A bytes-mode attachment still receives control updates: a resize, or a resync after it fell
    /// behind. Both mean "throw away what you have and start from this snapshot".
    pub fn apply_update(&mut self, update: &Update) {
        match update {
            Update::Resync { snapshot, .. } => {
                let size = Size {
                    cols: usize::from(snapshot.cols).max(1),
                    rows: usize::from(snapshot.rows).max(1),
                };
                self.size = size;
                self.term.resize(size);
                self.load(snapshot);
                self.resyncs += 1;
            }
            Update::Delta(_) => {}
        }
    }

    fn row_text(&self, line: i32) -> String {
        let row = &self.term.grid()[Line(line)];
        let mut text = String::with_capacity(self.size.cols);
        for col in 0..self.size.cols {
            let cell = &row[Column(col)];
            if cell
                .flags
                .intersects(Flags::WIDE_CHAR_SPACER | Flags::LEADING_WIDE_CHAR_SPACER)
            {
                continue;
            }
            text.push(cell.c);
        }
        text
    }

    pub fn capture(&self, with_scrollback: bool) -> String {
        let history = self.term.grid().history_size();
        let shown = if with_scrollback {
            history.min(SCROLLBACK_LIMIT)
        } else {
            0
        };
        let rows = (-(shown as i32)..self.size.rows as i32).map(|line| self.row_text(line));
        lines(rows)
    }

    pub fn screen_text(&self) -> String {
        lines((0..self.size.rows as i32).map(|line| self.row_text(line)))
    }
}

/// The snapshot as escape sequences: a reset, every row with its attributes, then the cursor.
fn render_snapshot(snapshot: &Snapshot) -> Vec<u8> {
    let mut out: Vec<u8> = Vec::with_capacity(snapshot.scrollback.len() * 16);
    out.extend_from_slice(b"\x1bc");
    let all: Vec<&Row> = snapshot
        .scrollback
        .iter()
        .chain(snapshot.screen.iter())
        .collect();
    for (index, row) in all.iter().enumerate() {
        for run in &row.runs {
            out.extend_from_slice(b"\x1b[0m");
            if run.flags & FLAG_BOLD != 0 {
                out.extend_from_slice(b"\x1b[1m");
            }
            if run.flags & FLAG_DIM != 0 {
                out.extend_from_slice(b"\x1b[2m");
            }
            if run.flags & FLAG_ITALIC != 0 {
                out.extend_from_slice(b"\x1b[3m");
            }
            if run.flags & FLAG_UNDERLINE != 0 {
                out.extend_from_slice(b"\x1b[4m");
            }
            if run.flags & FLAG_INVERSE != 0 {
                out.extend_from_slice(b"\x1b[7m");
            }
            out.extend_from_slice(colour(38, run.fg).as_bytes());
            out.extend_from_slice(colour(48, run.bg).as_bytes());
            out.extend_from_slice(run.text.as_bytes());
        }
        out.extend_from_slice(b"\x1b[0m");
        if index + 1 < all.len() {
            out.extend_from_slice(b"\r\n");
        }
    }
    out.extend_from_slice(
        format!(
            "\x1b[{};{}H",
            snapshot.cursor.row + 1,
            snapshot.cursor.col + 1
        )
        .as_bytes(),
    );
    if !snapshot.cursor.visible {
        out.extend_from_slice(b"\x1b[?25l");
    }
    out
}

fn colour(which: u16, packed: u32) -> String {
    let (r, g, b) = ((packed >> 16) & 0xff, (packed >> 8) & 0xff, packed & 0xff);
    format!("\x1b[{which};2;{r};{g};{b}m")
}
