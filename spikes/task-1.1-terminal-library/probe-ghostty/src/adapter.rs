//! Ghostty's VT engine behind the probe's backend contract.
//!
//! **Which bindings, and why.** The task named `libghostty-vt` 0.2.2. That crate cannot be
//! built on this machine: its `build.rs` pins ghostty commit `a887df42`, whose `build.zig`
//! calls `requireZig("0.15.2")` and so demands Zig 0.15.x, and Zig 0.15.2 cannot link anything
//! against the macOS 27 SDK (`libSystem.B.tbd` lists only `x86_64`, `arm64e` and `arm64e.x1`
//! targets, no plain `arm64-macos`, so every libSystem symbol comes out undefined - even for an
//! empty `zig build-exe`). `ghostty-vt` 0.8.0 is an explicit fork of those bindings, pinned to
//! ghostty `1.4.0-main+83fb956` and built with Zig 0.17, which does link here. It is the same C
//! library, so it is what the engine is measured through. The two facts are kept apart in the
//! document: the engine's numbers, and the state of the bindings.
//!
//! Three things to notice while reading: the library resolves colours itself, it tracks dirty
//! rows for a renderer, and every handle is `!Send + !Sync` by design, so the adapter and
//! everything it owns must stay on one thread.

use ghostty_vt::render::{CellIterator, Dirty, RenderState, RowIterator};
use ghostty_vt::screen::{CellWide, RowSemanticPrompt};
use ghostty_vt::style::{RgbColor, Underline};
use ghostty_vt::terminal::{Event, Mode, Options, Point, PointCoordinate};
use ghostty_vt::Terminal;
use probe_core::backend::{Cell, Facts, Rgb, Row, Snapshot, TerminalBackend};

const DEFAULT_FG: Rgb = Rgb::new(0xd4, 0xd4, 0xdc);
const DEFAULT_BG: Rgb = Rgb::new(0x00, 0x00, 0x00);

/// Roughly what a row of scrollback costs, so a limit in lines can be asked for in bytes.
/// `Options::max_scrollback` is a byte estimate here, pruned at page granularity, so a limit in
/// lines has to be converted. Measured, not guessed: with a 2.56 MB budget this library held
/// 2,283 rows of the 100 MB fixture, which is about 1,120 bytes a row at 120 columns. The value
/// below is that, rounded up, so a 10,000-line request really does hold about 10,000 rows and
/// the memory figures can be compared with a library that counts lines.
const BYTES_PER_ROW: usize = 1_152;

/// What the library reported, drained from its event queue after each write.
#[derive(Default)]
struct Reported {
    title: Option<String>,
    pwd: Option<String>,
    clipboard: Vec<String>,
    pty_replies: Vec<u8>,
    bells: usize,
    notifications: usize,
}

pub struct Ghostty {
    term: Terminal,
    render: RenderState,
    row_iterator: RowIterator,
    cell_iterator: CellIterator,
    reported: Reported,
    cols: u16,
    rows: u16,
}

impl TerminalBackend for Ghostty {
    fn name() -> &'static str {
        "ghostty-vt"
    }

    fn version() -> &'static str {
        "0.8.0 bindings (Apache-2.0) over ghostty 1.4.0-main+83fb956 (MIT), built with zig 0.17.1"
    }

    fn open(cols: u16, rows: u16, scrollback: usize) -> anyhow::Result<Self> {
        let term = Terminal::new(Options {
            cols,
            rows,
            max_scrollback: scrollback.saturating_mul(BYTES_PER_ROW),
        })?;
        Ok(Self {
            term,
            render: RenderState::new()?,
            row_iterator: RowIterator::new()?,
            cell_iterator: CellIterator::new()?,
            reported: Reported::default(),
            cols,
            rows,
        })
    }

    fn feed(&mut self, bytes: &[u8]) {
        self.term.vt_write(bytes);
        if self.term.has_events() {
            self.collect();
        }
    }

    fn snapshot(&mut self) -> Snapshot {
        let mut rows = Vec::with_capacity(usize::from(self.rows));
        let mut cursor = (0_u16, 0_u16);
        let mut cursor_visible = false;
        let read = (|| -> ghostty_vt::error::Result<()> {
            let snapshot = self.render.update(&self.term)?;
            cursor_visible = snapshot.cursor_visible()?;
            if let Some(at) = snapshot.cursor_viewport()? {
                cursor = (at.x, u16::try_from(at.y).unwrap_or(0));
            }
            let mut row_iteration = self.row_iterator.update(&snapshot)?;
            while let Some(row) = row_iteration.next() {
                let mut text = String::with_capacity(usize::from(self.cols));
                let mut cells = Vec::with_capacity(usize::from(self.cols));
                let mut cell_iteration = self.cell_iterator.update(row)?;
                while let Some(cell) = cell_iteration.next() {
                    // A wide character occupies two cells: the second is a spacer with no text
                    // of its own, and putting a blank there would split 日本語 into 日 本 語.
                    if cell.raw_cell()?.wide()? == CellWide::SpacerTail {
                        continue;
                    }
                    let before = text.len();
                    if cell.graphemes_len()? == 0 {
                        text.push(' ');
                    } else {
                        cell.graphemes_utf8(&mut text)?;
                    }
                    let ch = text[before..].chars().next().unwrap_or(' ');
                    let style = cell.style()?;
                    cells.push(Cell {
                        ch,
                        fg: colour(cell.fg_color()?, DEFAULT_FG),
                        bg: colour(cell.bg_color()?, DEFAULT_BG),
                        bold: style.bold,
                        underline: style.underline != Underline::None,
                    });
                }
                drop(cell_iteration);
                rows.push(Row { text, cells });
            }
            Ok(())
        })();
        if let Err(error) = read {
            eprintln!("reading the screen back failed: {error}");
        }
        Snapshot {
            rows,
            cursor_col: cursor.0,
            cursor_row: cursor.1,
            cursor_visible,
        }
    }

    fn dirty_rows(&mut self) -> Option<Vec<u16>> {
        let mut dirty = Vec::new();
        let read = (|| -> ghostty_vt::error::Result<()> {
            let snapshot = self.render.update(&self.term)?;
            match snapshot.dirty()? {
                Dirty::Clean => return Ok(()),
                Dirty::Full => {
                    dirty.extend(0..self.rows);
                    snapshot.set_dirty(Dirty::Clean)?;
                    return Ok(());
                }
                Dirty::Partial => {}
            }
            let mut row_iteration = self.row_iterator.update(&snapshot)?;
            let mut index = 0_u16;
            while let Some(row) = row_iteration.next() {
                if row.dirty()? {
                    dirty.push(index);
                    row.set_dirty(false)?;
                }
                index += 1;
            }
            Ok(())
        })();
        if let Err(error) = read {
            eprintln!("reading dirty rows failed: {error}");
        }
        Some(dirty)
    }

    fn resize(&mut self, cols: u16, rows: u16) {
        self.cols = cols;
        self.rows = rows;
        if let Err(error) = self.term.resize(cols, rows, 8, 17) {
            eprintln!("resize failed: {error}");
        }
    }

    fn scrollback_rows(&self) -> usize {
        self.term.scrollback_rows().unwrap_or(0)
    }

    fn facts(&mut self) -> Facts {
        self.collect();
        let semantic = self.semantic_prompt_seen();
        let hyperlink = self.hyperlink_at(0, 0);
        Facts {
            title: self.reported.title.clone(),
            pwd: self.reported.pwd.clone(),
            mouse_tracking: self.term.is_mouse_tracking().ok(),
            bracketed_paste: self.term.mode(Mode::BRACKETED_PASTE).ok(),
            alt_screen: self.term.mode(Mode::ALT_SCREEN_SAVE).ok(),
            kitty_keyboard: self
                .term
                .kitty_keyboard_flags()
                .ok()
                .map(|flags| !flags.is_empty()),
            hyperlink_at: hyperlink,
            clipboard_writes: Some(std::mem::take(&mut self.reported.clipboard)),
            // The library parses OSC 133 into per-row state, read back below.
            semantic_prompt: semantic,
            // No event carries an OSC the library does not know: its parser types an
            // unrecognised one as `CommandType::Invalid` and nothing reaches the application.
            unknown_osc: None,
            pty_replies: std::mem::take(&mut self.reported.pty_replies),
        }
    }
}

impl Ghostty {
    /// Drain the library's event queue into what the probe reports.
    fn collect(&mut self) {
        for event in self.term.events() {
            match event {
                Event::TitleChanged(title) => self.reported.title = Some(title),
                Event::PwdChanged(pwd) => self.reported.pwd = Some(pwd),
                Event::ClipboardWrite(write) => {
                    for content in write.contents {
                        self.reported
                            .clipboard
                            .push(String::from_utf8_lossy(&content.data).into_owned());
                    }
                }
                Event::PtyWrite(bytes) => self.reported.pty_replies.extend_from_slice(&bytes),
                Event::Bell => self.reported.bells += 1,
                Event::DesktopNotification { .. } | Event::ProgressReport { .. } => {
                    self.reported.notifications += 1;
                }
                Event::RenderHold(_) => {}
            }
        }
    }

    /// Whether any row on the screen carries an OSC 133 prompt mark.
    fn semantic_prompt_seen(&mut self) -> Option<bool> {
        let mut seen = false;
        let read = (|| -> ghostty_vt::error::Result<()> {
            let snapshot = self.render.update(&self.term)?;
            let mut row_iteration = self.row_iterator.update(&snapshot)?;
            while let Some(row) = row_iteration.next() {
                if row.raw_row()?.semantic_prompt()? != RowSemanticPrompt::None {
                    seen = true;
                }
            }
            Ok(())
        })();
        match read {
            Ok(()) => Some(seen),
            Err(error) => {
                eprintln!("reading prompt marks failed: {error}");
                None
            }
        }
    }

    /// The hyperlink on one cell, if the library knows one.
    fn hyperlink_at(&self, x: u16, y: u32) -> Option<String> {
        let grid = self
            .term
            .grid_ref(Point::Viewport(PointCoordinate { x, y }))
            .ok()?;
        let mut buffer = vec![0_u8; 2048];
        let written = grid.hyperlink_uri(&mut buffer).ok()?;
        if written == 0 {
            return None;
        }
        String::from_utf8(buffer[..written].to_vec()).ok()
    }
}

fn colour(reported: Option<RgbColor>, default: Rgb) -> Rgb {
    reported.map_or(default, |RgbColor { r, g, b }| Rgb::new(r, g, b))
}
