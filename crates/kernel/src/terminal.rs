//! A terminal: ghostty's VT engine, and the state the kernel reports about
//! it (doc-9 R14, R16, R92; doc-16 R105).
//!
//! What crosses the API - the screen's rows and cursor - is defined in the
//! sdk (`steg_sdk::screen`); this module is the behaviour that produces it,
//! which stays in the kernel.

use ghostty_vt::render::{CellIterator, CursorVisualStyle, RenderState, RowIterator};
use ghostty_vt::screen::{Cell as EngineCell, CellWide, Screen as EngineScreen};
use ghostty_vt::style::{StyleColor, Underline as EngineUnderline};
use ghostty_vt::terminal::{Options, Point, PointCoordinate};
use ghostty_vt::{Event, Terminal as Engine};
use steg_sdk::{CellStyle, Cursor, CursorShape, Rgb, Row, Run, Screen, Underline, Wide};

use crate::error::Error;

/// A terminal: ghostty's VT engine, plus the facts the kernel reports that
/// the engine does not hold across calls (`cwd`, `title`).
///
/// Neither `Clone`, `PartialEq`, `Send` nor `Sync`: the engine forbids all
/// four by design (decision-3 section 7), and this type does not paper
/// over it. It must be created on, and never moved from, the thread that
/// owns it - TASK-1.40's one-thread-per-pane model - which the compiler
/// enforces here with no `unsafe` (`#![forbid(unsafe_code)]` holds for
/// this crate, `crates/kernel/src/lib.rs`).
#[derive(Debug)]
pub struct Terminal {
    engine: Engine,
    render: RenderState,
    cwd: Option<String>,
    title: String,
}

impl Terminal {
    /// Creates a terminal of the given size, with scrollback kept to
    /// `scrollback_lines` rows (doc-9 R15).
    ///
    /// # Errors
    ///
    /// Returns [`Error::Engine`] if the engine could not be created or
    /// configured.
    pub fn new(cols: u16, rows: u16, scrollback_lines: usize) -> Result<Self, Error> {
        let mut engine = Engine::new(Options {
            cols,
            rows,
            ..Options::default()
        })
        .map_err(Error::Engine)?;
        engine
            .set_scrollback_max_lines(Some(scrollback_lines))
            .map_err(Error::Engine)?;
        let render = RenderState::new().map_err(Error::Engine)?;
        Ok(Self {
            engine,
            render,
            cwd: None,
            title: String::new(),
        })
    }

    /// Feeds bytes through the engine's VT parser (doc-9 R14, R92).
    ///
    /// Drains every event the write produced into this terminal's own
    /// state (`cwd`, `title`): `ghostty_vt::Terminal::events` never drains
    /// itself, and a caller that only wrote would leak them.
    pub fn write(&mut self, bytes: &[u8]) {
        self.engine.vt_write(bytes);
        let events: Vec<Event> = self.engine.events().collect();
        for event in events {
            match event {
                Event::TitleChanged(title) => self.title = title,
                Event::PwdChanged(pwd) => self.cwd = Some(parse_pwd(&pwd)),
                Event::PtyWrite(_)
                | Event::Bell
                | Event::ClipboardWrite(_)
                | Event::DesktopNotification { .. }
                | Event::ProgressReport { .. }
                | Event::RenderHold(_) => {}
            }
        }
    }

    /// Reads the current screen: every row as attribute runs, and the
    /// cursor (AC #1, AC #3; decision-2 section 3).
    ///
    /// # Errors
    ///
    /// Returns [`Error::Engine`] if the engine could not report its state.
    pub fn screen(&mut self) -> Result<Screen, Error> {
        let snapshot = self.render.update(&self.engine).map_err(Error::Engine)?;
        let palette = snapshot.colors().map_err(Error::Engine)?.palette;

        let mut row_iterator = RowIterator::new().map_err(Error::Engine)?;
        let mut row_iteration = row_iterator.update(&snapshot).map_err(Error::Engine)?;
        let mut cell_iterator = CellIterator::new().map_err(Error::Engine)?;

        let mut rows = Vec::new();
        while let Some(row) = row_iteration.next() {
            rows.push(build_row(row, &mut cell_iterator, &palette)?);
        }

        let cursor = read_cursor(&snapshot)?;
        Ok(Screen { rows, cursor })
    }

    /// The address of the hyperlink at the given viewport cell, or `None`
    /// when it has none (doc-9 R116).
    ///
    /// Asked for when a person acts on a link - a click, not a draw - so
    /// this does not use the render-state path [`Terminal::screen`] does.
    /// It uses the engine's point-lookup API instead, which the engine's
    /// own authors say "isn't meant to be used as the core of render
    /// loop" - exactly why it is reserved for this one-off query.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Engine`] if the engine could not resolve the
    /// point or the link's address.
    pub fn link_at(&self, x: u16, y: u32) -> Result<Option<String>, Error> {
        let grid_ref = self
            .engine
            .grid_ref(Point::Viewport(PointCoordinate { x, y }))
            .map_err(Error::Engine)?;
        let mut buf = [0u8; 256];
        match grid_ref.hyperlink_uri(&mut buf) {
            Ok(0) => Ok(None),
            Ok(len) => Ok(Some(uri_from_bytes(&buf[..len]))),
            Err(ghostty_vt::Error::OutOfSpace { required }) => {
                let mut grown = vec![0u8; required];
                let len = grid_ref.hyperlink_uri(&mut grown).map_err(Error::Engine)?;
                Ok(Some(uri_from_bytes(&grown[..len])))
            }
            Err(other) => Err(Error::Engine(other)),
        }
    }

    /// Resizes the terminal (point 8 of this task's brief; TASK-1.42 feeds
    /// this a real client size).
    ///
    /// Pixel geometry is passed as zero: the kernel holds no pixel sizes
    /// (R102, R105 - pixels are the app's). CSI 14/16/18 t answers and
    /// mode-2048 size reports will be wrong until TASK-1.42 plumbs a real
    /// pixel size through; a known, named gap, not fixed here.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Engine`] if the engine could not resize.
    pub fn resize(&mut self, cols: u16, rows: u16) -> Result<(), Error> {
        self.engine.resize(cols, rows, 0, 0).map_err(Error::Engine)
    }

    /// The terminal's width in columns.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Engine`] if the engine could not report it.
    pub fn cols(&self) -> Result<u16, Error> {
        self.engine.cols().map_err(Error::Engine)
    }

    /// The terminal's height in rows.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Engine`] if the engine could not report it.
    pub fn rows(&self) -> Result<u16, Error> {
        self.engine.rows().map_err(Error::Engine)
    }

    /// The terminal's working directory, as last reported by OSC 7, OSC 9
    /// or OSC 1337 (AC #2, doc-9 R16). `None` until a program reports one.
    #[must_use]
    pub fn cwd(&self) -> Option<&str> {
        self.cwd.as_deref()
    }

    /// The terminal's title, as last set by OSC 0 or OSC 2 (AC #2, doc-9
    /// R16). Empty until a program sets one.
    #[must_use]
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Whether the alternate screen is active (AC #2, doc-9 R16).
    ///
    /// # Errors
    ///
    /// Returns [`Error::Engine`] if the engine could not report it.
    pub fn alternate_screen(&self) -> Result<bool, Error> {
        Ok(self.engine.active_screen().map_err(Error::Engine)? == EngineScreen::Alternate)
    }
}

/// Builds one row's attribute runs by walking its cells left to right,
/// starting a new [`Run`] whenever the resolved [`CellStyle`] changes
/// (decision-2 section 3).
fn build_row(
    row: &ghostty_vt::render::RowIteration<'_>,
    cells: &mut CellIterator,
    palette: &[ghostty_vt::style::RgbColor; 256],
) -> Result<Row, Error> {
    let mut iteration = cells.update(row).map_err(Error::Engine)?;
    let mut runs: Row = Vec::new();
    while let Some(cell) = iteration.next() {
        let raw = cell.raw_cell().map_err(Error::Engine)?;
        let wide = wide_from(raw.wide().map_err(Error::Engine)?);

        // Never rendered (the engine's own doc comment): the preceding
        // `Wide` cell already carries this column's text and width.
        if wide == Wide::SpacerTail {
            continue;
        }

        let style = cell_style(cell, raw, wide, palette)?;
        let mut text = String::new();
        cell.graphemes_utf8(&mut text).map_err(Error::Engine)?;

        match runs.last_mut() {
            Some(last) if last.style == style => last.text.push_str(&text),
            _ => runs.push(Run { text, style }),
        }
    }
    Ok(runs)
}

/// Reads one cell's resolved style, including the fields the render-state
/// convenience getters do not resolve on their own (the underline colour,
/// which `style()` returns unresolved against the palette).
fn cell_style(
    cell: &ghostty_vt::render::CellIteration<'_>,
    raw: EngineCell,
    wide: Wide,
    palette: &[ghostty_vt::style::RgbColor; 256],
) -> Result<CellStyle, Error> {
    let style = cell.style().map_err(Error::Engine)?;
    let fg = cell.fg_color().map_err(Error::Engine)?.map(rgb_from);
    let bg = cell.bg_color().map_err(Error::Engine)?.map(rgb_from);
    let link = raw.has_hyperlink().map_err(Error::Engine)?;

    Ok(CellStyle {
        fg,
        bg,
        bold: style.bold,
        italic: style.italic,
        faint: style.faint,
        blink: style.blink,
        inverse: style.inverse,
        invisible: style.invisible,
        strikethrough: style.strikethrough,
        overline: style.overline,
        underline: underline_from(style.underline),
        underline_colour: resolve_style_color(style.underline_color, palette),
        wide,
        link,
    })
}

/// Resolves a style colour (possibly a palette index) to the RGB value a
/// client draws, the same resolution the engine already does for a cell's
/// foreground and background (decision-2 section 3).
fn resolve_style_color(
    color: StyleColor,
    palette: &[ghostty_vt::style::RgbColor; 256],
) -> Option<Rgb> {
    match color {
        StyleColor::None => None,
        StyleColor::Rgb(rgb) => Some(rgb_from(rgb)),
        StyleColor::Palette(index) => palette.get(usize::from(index.0)).copied().map(rgb_from),
    }
}

fn rgb_from(value: ghostty_vt::style::RgbColor) -> Rgb {
    Rgb {
        r: value.r,
        g: value.g,
        b: value.b,
    }
}

fn wide_from(value: CellWide) -> Wide {
    match value {
        CellWide::Narrow => Wide::Narrow,
        CellWide::Wide => Wide::Wide,
        CellWide::SpacerTail => Wide::SpacerTail,
        CellWide::SpacerHead => Wide::SpacerHead,
    }
}

// `ghostty_vt::style::Underline` is `#[non_exhaustive]`, so the compiler
// requires a wildcard arm here regardless of how many variants are
// covered - this is not the "match your own enums without one" case
// rust.md means, since this enum is not ours. Every current variant is
// named anyway, so clippy's `wildcard_enum_match_arm` does not fire (it
// only flags a `_` that stands in for a *named, uncovered* variant) and
// no `#[expect]` is needed.
fn underline_from(value: EngineUnderline) -> Underline {
    match value {
        EngineUnderline::None => Underline::None,
        EngineUnderline::Single => Underline::Single,
        EngineUnderline::Double => Underline::Double,
        EngineUnderline::Curly => Underline::Curly,
        EngineUnderline::Dotted => Underline::Dotted,
        EngineUnderline::Dashed => Underline::Dashed,
        _ => Underline::None,
    }
}

// Same reasoning as `underline_from`: `CursorVisualStyle` is
// `#[non_exhaustive]` in the engine crate, and every current variant is
// named here too.
fn cursor_shape_from(value: CursorVisualStyle) -> CursorShape {
    match value {
        CursorVisualStyle::Bar => CursorShape::Bar,
        CursorVisualStyle::Block => CursorShape::Block,
        CursorVisualStyle::Underline => CursorShape::Underline,
        CursorVisualStyle::BlockHollow => CursorShape::BlockHollow,
        _ => CursorShape::Block,
    }
}

/// Reads the cursor from a render-state snapshot in one call.
///
/// `x`/`y` are only meaningful while the cursor is within the viewport
/// (not scrolled out of view); this task's cell model carries no separate
/// flag for that, so an out-of-viewport cursor reports position `(0, 0)`.
fn read_cursor(snapshot: &ghostty_vt::render::Snapshot<'_>) -> Result<Cursor, Error> {
    let cursor = snapshot.cursor().map_err(Error::Engine)?;
    let (x, y) = cursor.viewport.map_or((0, 0), |v| (v.x, v.y));
    Ok(Cursor {
        x,
        y,
        visible: cursor.visible,
        blinking: cursor.blinking,
        shape: cursor_shape_from(cursor.visual_style),
    })
}

/// Parses the value an `Event::PwdChanged` carries into a plain path.
///
/// OSC 7 gives a `file://<host>/<path>` URI; OSC 9 and OSC 1337 already
/// give a plain path, so a value with no `file://` prefix is returned
/// unchanged.
fn parse_pwd(raw: &str) -> String {
    let Some(after_scheme) = raw.strip_prefix("file://") else {
        return raw.to_string();
    };
    // `after_scheme` is `<host>/<path>`; `split_once` consumes the
    // separating `/`, so it is put back in front of the decoded path.
    let path = after_scheme
        .split_once('/')
        .map_or("", |(_host, path)| path);
    format!("/{}", percent_decode(path))
}

/// Decodes `%XX` escapes in a URI path. Byte-oriented on purpose: it never
/// slices the input `&str` by index, so malformed input - this is a path a
/// running program chose, not a trusted value - cannot panic on a
/// non-UTF-8-boundary slice.
fn percent_decode(value: &str) -> String {
    fn hex_digit(byte: u8) -> Option<u8> {
        match byte {
            b'0'..=b'9' => Some(byte - b'0'),
            b'a'..=b'f' => Some(byte - b'a' + 10),
            b'A'..=b'F' => Some(byte - b'A' + 10),
            _ => None,
        }
    }

    let bytes = value.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && let (Some(high), Some(low)) = (
                bytes.get(i + 1).copied().and_then(hex_digit),
                bytes.get(i + 2).copied().and_then(hex_digit),
            )
        {
            out.push(high * 16 + low);
            i += 3;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// A hyperlink's URI, as UTF-8 bytes from the engine's buffer. Lossy on
/// purpose: a malformed URI from a running program should not crash the
/// query that asked about it.
fn uri_from_bytes(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

#[cfg(test)]
mod tests {
    use steg_sdk::{Rgb, Underline, Wide};

    use super::{CursorShape, Terminal};

    #[test]
    fn writing_plain_text_produces_one_run() {
        let mut term = Terminal::new(10, 2, 100).unwrap();
        term.write(b"hi");

        let screen = term.screen().unwrap();

        assert_eq!(screen.rows[0].len(), 1);
        assert_eq!(screen.rows[0][0].text, "hi");
    }

    #[test]
    fn an_sgr_colour_change_splits_the_row_into_two_runs_with_the_right_fg() {
        // Exactly as wide as the text: a cell the cursor never visits keeps
        // its original default style regardless of the "current" SGR, so a
        // wider row would leave a third, untouched run after "cd".
        let mut term = Terminal::new(4, 1, 100).unwrap();
        term.write(b"ab\x1b[38;2;10;20;30mcd");

        let screen = term.screen().unwrap();
        let runs = &screen.rows[0];

        assert_eq!(runs.len(), 2);
        assert_eq!(runs[0].text, "ab");
        assert_eq!(runs[0].style.fg, None);
        assert_eq!(runs[1].text, "cd");
        assert_eq!(
            runs[1].style.fg,
            Some(Rgb {
                r: 10,
                g: 20,
                b: 30
            })
        );
    }

    #[test]
    fn the_cursor_position_is_reported() {
        let mut term = Terminal::new(10, 3, 100).unwrap();
        // Move to row 2, column 3 (1-indexed), then print: the cursor
        // advances one cell to the right of where it printed.
        term.write(b"\x1b[2;3Hx");

        let screen = term.screen().unwrap();

        assert_eq!((screen.cursor.x, screen.cursor.y), (3, 1));
    }

    #[test]
    fn osc_7_sets_the_reported_cwd() {
        let mut term = Terminal::new(10, 2, 100).unwrap();
        assert_eq!(term.cwd(), None);

        term.write(b"\x1b]7;file:///tmp/project\x07");

        assert_eq!(term.cwd(), Some("/tmp/project"));
    }

    #[test]
    fn osc_2_sets_the_reported_title() {
        let mut term = Terminal::new(10, 2, 100).unwrap();
        assert_eq!(term.title(), "");

        term.write(b"\x1b]2;hello\x07");

        assert_eq!(term.title(), "hello");
    }

    #[test]
    fn entering_and_leaving_the_alternate_screen_is_reported() {
        let mut term = Terminal::new(10, 2, 100).unwrap();
        assert!(!term.alternate_screen().unwrap());

        term.write(b"\x1b[?1049h");
        assert!(term.alternate_screen().unwrap());

        term.write(b"\x1b[?1049l");
        assert!(!term.alternate_screen().unwrap());
    }

    #[test]
    fn a_wide_character_is_marked_wide_and_its_spacer_is_skipped() {
        let mut term = Terminal::new(10, 1, 100).unwrap();
        term.write("好a".as_bytes());

        let screen = term.screen().unwrap();
        let runs = &screen.rows[0];

        let wide_run = runs.iter().find(|r| r.text == "好").unwrap();
        assert_eq!(wide_run.style.wide, Wide::Wide);

        let narrow_run = runs.iter().find(|r| r.text == "a").unwrap();
        assert_eq!(narrow_run.style.wide, Wide::Narrow);
    }

    #[test]
    fn curly_underline_with_its_own_colour_round_trips() {
        let mut term = Terminal::new(10, 1, 100).unwrap();
        term.write(b"\x1b[4:3m\x1b[58:2::10:20:30mx");

        let screen = term.screen().unwrap();
        let run = &screen.rows[0][0];

        assert_eq!(run.style.underline, Underline::Curly);
        assert_eq!(
            run.style.underline_colour,
            Some(Rgb {
                r: 10,
                g: 20,
                b: 30
            })
        );
    }

    #[test]
    fn decscusr_sets_the_reported_cursor_shape() {
        let mut term = Terminal::new(10, 1, 100).unwrap();
        term.write(b"\x1b[4 q");

        let screen = term.screen().unwrap();

        assert_eq!(screen.cursor.shape, CursorShape::Underline);
    }

    #[test]
    fn a_hyperlink_marks_its_cells_and_nothing_else() {
        let mut term = Terminal::new(20, 1, 100).unwrap();
        term.write(b"\x1b]8;;http://example.com\x1b\\linked\x1b]8;;\x1b\\ plain");

        let screen = term.screen().unwrap();
        let runs = &screen.rows[0];

        let linked = runs.iter().find(|r| r.text.contains("linked")).unwrap();
        assert!(linked.style.link);

        let plain = runs.iter().find(|r| r.text.contains("plain")).unwrap();
        assert!(!plain.style.link);
    }

    #[test]
    fn link_at_returns_the_address_of_a_linked_cell() {
        let mut term = Terminal::new(20, 1, 100).unwrap();
        term.write(b"\x1b]8;;http://example.com\x1b\\hi\x1b]8;;\x1b\\");

        assert_eq!(
            term.link_at(0, 0).unwrap(),
            Some("http://example.com".to_string())
        );
    }

    #[test]
    fn link_at_returns_none_for_a_cell_with_no_link() {
        let mut term = Terminal::new(20, 1, 100).unwrap();
        term.write(b"plain text");

        assert_eq!(term.link_at(0, 0).unwrap(), None);
    }

    #[test]
    fn resizing_changes_the_reported_size_and_reflows_text() {
        let mut term = Terminal::new(10, 2, 100).unwrap();
        assert_eq!((term.cols().unwrap(), term.rows().unwrap()), (10, 2));

        term.write(b"\x1b[?7h0123456789");
        term.resize(5, 4).unwrap();

        assert_eq!((term.cols().unwrap(), term.rows().unwrap()), (5, 4));

        let screen = term.screen().unwrap();
        assert_eq!(screen.rows[0][0].text, "01234");
        assert_eq!(screen.rows[1][0].text, "56789");
    }
}
