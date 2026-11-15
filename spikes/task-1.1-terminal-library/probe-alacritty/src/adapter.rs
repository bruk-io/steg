//! `alacritty_terminal` 0.26 behind the probe's backend contract.
//!
//! Two things the adapter has to do that the library does not: resolve colours (the library
//! hands back palette indices and leaves the palette to the application) and notice that
//! several sequences steg needs are never reported at all.

use std::cell::RefCell;
use std::rc::Rc;

use alacritty_terminal::event::{Event, EventListener};
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::{Column, Line};
use alacritty_terminal::term::cell::Flags;
use alacritty_terminal::term::{Config, Term, TermDamage, TermMode};
use alacritty_terminal::vte::ansi::{Color, NamedColor, Processor, Rgb as VteRgb};
use probe_core::backend::{Cell, Facts, Rgb, Row, Snapshot, TerminalBackend};

const DEFAULT_FG: Rgb = Rgb::new(0xd4, 0xd4, 0xdc);
const DEFAULT_BG: Rgb = Rgb::new(0x00, 0x00, 0x00);

/// The terminal's size, which `alacritty_terminal` asks for through a trait.
struct Size {
    columns: usize,
    screen_lines: usize,
    total_lines: usize,
}

impl Dimensions for Size {
    fn total_lines(&self) -> usize {
        self.total_lines
    }

    fn screen_lines(&self) -> usize {
        self.screen_lines
    }

    fn columns(&self) -> usize {
        self.columns
    }
}

/// Everything the library reports arrives as an event, so the adapter keeps them.
#[derive(Clone, Default)]
struct Collector(Rc<RefCell<Vec<Event>>>);

impl EventListener for Collector {
    fn send_event(&self, event: Event) {
        self.0.borrow_mut().push(event);
    }
}

pub struct Alacritty {
    term: Term<Collector>,
    parser: Processor,
    events: Collector,
    cols: u16,
    rows: u16,
}

impl TerminalBackend for Alacritty {
    fn name() -> &'static str {
        "alacritty_terminal"
    }

    fn version() -> &'static str {
        "0.26.0, Apache-2.0, pure Rust"
    }

    fn open(cols: u16, rows: u16, scrollback: usize) -> anyhow::Result<Self> {
        let events = Collector::default();
        let config = Config {
            scrolling_history: scrollback,
            kitty_keyboard: true,
            ..Config::default()
        };
        let size = Size {
            columns: usize::from(cols),
            screen_lines: usize::from(rows),
            total_lines: usize::from(rows) + scrollback,
        };
        Ok(Self {
            term: Term::new(config, &size, events.clone()),
            parser: Processor::new(),
            events,
            cols,
            rows,
        })
    }

    fn feed(&mut self, bytes: &[u8]) {
        self.parser.advance(&mut self.term, bytes);
    }

    fn snapshot(&mut self) -> Snapshot {
        let grid = self.term.grid();
        let cursor = grid.cursor.point;
        let mut rows = Vec::with_capacity(usize::from(self.rows));
        for line in 0..i32::from(self.rows) {
            let row = &grid[Line(line)];
            let mut text = String::with_capacity(usize::from(self.cols));
            let mut cells = Vec::with_capacity(usize::from(self.cols));
            for column in 0..usize::from(self.cols) {
                let cell = &row[Column(column)];
                if cell.flags.contains(Flags::WIDE_CHAR_SPACER) {
                    continue;
                }
                let inverse = cell.flags.contains(Flags::INVERSE);
                let (fg, bg) = if inverse {
                    (resolve(cell.bg, DEFAULT_BG), resolve(cell.fg, DEFAULT_FG))
                } else {
                    (resolve(cell.fg, DEFAULT_FG), resolve(cell.bg, DEFAULT_BG))
                };
                text.push(cell.c);
                cells.push(Cell {
                    ch: cell.c,
                    fg,
                    bg,
                    bold: cell.flags.contains(Flags::BOLD),
                    underline: cell.flags.contains(Flags::UNDERLINE),
                });
            }
            rows.push(Row { text, cells });
        }
        Snapshot {
            rows,
            cursor_col: u16::try_from(cursor.column.0).unwrap_or(0),
            cursor_row: u16::try_from(cursor.line.0.max(0)).unwrap_or(0),
            cursor_visible: self.term.mode().contains(TermMode::SHOW_CURSOR),
        }
    }

    fn dirty_rows(&mut self) -> Option<Vec<u16>> {
        let damage = match self.term.damage() {
            TermDamage::Full => (0..self.rows).collect(),
            TermDamage::Partial(lines) => lines
                .filter_map(|line| u16::try_from(line.line).ok())
                .collect(),
        };
        self.term.reset_damage();
        Some(damage)
    }

    fn resize(&mut self, cols: u16, rows: u16) {
        self.cols = cols;
        self.rows = rows;
        self.term.resize(Size {
            columns: usize::from(cols),
            screen_lines: usize::from(rows),
            total_lines: usize::from(rows),
        });
    }

    fn scrollback_rows(&self) -> usize {
        self.term.grid().history_size()
    }

    fn facts(&mut self) -> Facts {
        let mode = *self.term.mode();
        let mut facts = Facts {
            // The library reports these through its mode flags.
            mouse_tracking: Some(mode.intersects(TermMode::MOUSE_MODE)),
            bracketed_paste: Some(mode.contains(TermMode::BRACKETED_PASTE)),
            alt_screen: Some(mode.contains(TermMode::ALT_SCREEN)),
            kitty_keyboard: Some(mode.intersects(TermMode::KITTY_KEYBOARD_PROTOCOL)),
            // The library reports a clipboard write, so an empty list means "nothing was
            // written", not "nothing can be reported".
            clipboard_writes: Some(Vec::new()),
            // Nothing in the library's API carries these: no OSC 7, no OSC 133, and vte's
            // `osc_dispatch` drops an unrecognised OSC with a debug log (vte 0.15
            // `src/ansi.rs`, the `unhandled` helper). steg would have to parse them itself.
            pwd: None,
            semantic_prompt: None,
            unknown_osc: None,
            ..Facts::default()
        };
        facts.hyperlink_at = self.term.grid()[Line(0)][Column(0)]
            .hyperlink()
            .map(|link| link.uri().to_owned());
        for event in self.events.0.borrow_mut().drain(..) {
            match event {
                Event::Title(title) => facts.title = Some(title),
                Event::ClipboardStore(_, text) => {
                    if let Some(writes) = &mut facts.clipboard_writes {
                        writes.push(text);
                    }
                }
                Event::PtyWrite(text) => facts.pty_replies.extend_from_slice(text.as_bytes()),
                _ => {}
            }
        }
        facts
    }
}

/// Turn one of the library's colours into 24-bit. The palette is the application's job here:
/// `Colors::default()` is every entry `None`, so steg would ship this table.
fn resolve(colour: Color, default: Rgb) -> Rgb {
    match colour {
        Color::Spec(VteRgb { r, g, b }) => Rgb::new(r, g, b),
        Color::Indexed(index) => palette(usize::from(index)),
        Color::Named(named) => match named {
            NamedColor::Foreground => DEFAULT_FG,
            NamedColor::Background => DEFAULT_BG,
            NamedColor::Cursor => DEFAULT_FG,
            // Every other named colour is an index into the same table.
            other => palette(other as usize),
        },
        // `Color` is a foreign enum, so a catch-all is unavoidable.
        #[expect(
            unreachable_patterns,
            reason = "Color is a foreign enum and may grow variants"
        )]
        _ => default,
    }
}

/// The xterm 256-colour table, which every terminal ships and this library does not.
fn palette(index: usize) -> Rgb {
    const BASE: [(u8, u8, u8); 16] = [
        (0x1d, 0x1f, 0x26),
        (0xe8, 0x6b, 0x6b),
        (0x8c, 0xd1, 0x7d),
        (0xe5, 0xc0, 0x7b),
        (0x7a, 0xa6, 0xf0),
        (0xc6, 0x8a, 0xf0),
        (0x6c, 0xc9, 0xc9),
        (0xc8, 0xcc, 0xd4),
        (0x4b, 0x50, 0x5c),
        (0xff, 0x8a, 0x8a),
        (0xa8, 0xe8, 0x98),
        (0xff, 0xdc, 0x95),
        (0x9a, 0xc0, 0xff),
        (0xdd, 0xa8, 0xff),
        (0x8a, 0xe8, 0xe8),
        (0xf0, 0xf2, 0xf8),
    ];
    const CUBE: [u8; 6] = [0, 95, 135, 175, 215, 255];
    match index {
        0..=15 => {
            let (r, g, b) = BASE[index];
            Rgb::new(r, g, b)
        }
        16..=231 => {
            let offset = index - 16;
            Rgb::new(
                CUBE[offset / 36],
                CUBE[(offset / 6) % 6],
                CUBE[offset % 6],
            )
        }
        232..=255 => {
            let grey = u8::try_from(8 + (index - 232) * 10).unwrap_or(u8::MAX);
            Rgb::new(grey, grey, grey)
        }
        _ => DEFAULT_FG,
    }
}
