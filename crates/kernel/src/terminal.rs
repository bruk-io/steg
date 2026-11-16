//! A terminal: ghostty's VT engine, and the state the kernel reports about
//! it (doc-9 R14, R16, R92; doc-16 R105).
//!
//! What crosses the API - the screen's rows and cursor - is defined in the
//! sdk (`steg_sdk::screen`); this module is the behaviour that produces it,
//! which stays in the kernel.

use ghostty_vt::render::{
    CellIterator, CursorVisualStyle, Dirty, RenderState, RowIteration, RowIterator, Snapshot,
};
use ghostty_vt::screen::{CellWide, Screen as EngineScreen};
use ghostty_vt::style::{
    RgbColor as EngineRgb, Style as EngineStyle, StyleColor, Underline as EngineUnderline,
};
use ghostty_vt::terminal::{Options, Point, PointCoordinate};
use ghostty_vt::{Event, Terminal as Engine};
use steg_sdk::{
    CellStyle, Cursor, CursorShape, Page, Rgb, Row, RowId, Run, Screen, ScreenUpdate, Size,
    Underline, Wide,
};

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
    /// The one render state this terminal ever creates (doc-9's scrollback
    /// task: proved by test that a second, independent render state
    /// against the same engine does not work - its own first update sees
    /// whatever the first render state's `update()` already consumed, not
    /// what changed since *this* reader last looked. `screen()` and
    /// [`Terminal::changes_since`] are the only two callers that may read
    /// it, and must not be mixed by two different external callers acting
    /// as if each had its own independent view).
    render: RenderState,
    cwd: Option<String>,
    title: String,
    /// How many rows have been permanently retired within the current
    /// [`epoch`](Terminal::epoch) - the oldest row still retained has this
    /// as its [`RowId::offset`]. Incremented only across a [`write`](Terminal::write)
    /// call, by however many rows a prune dropped; a column-count-changing
    /// [`resize`](Terminal::resize) instead ends the epoch (see `epoch`),
    /// since there is no way to tell a resize's own pruning from reflow
    /// alone growing or shrinking the row count.
    rows_scrolled_off: u64,
    /// Bumped by a column-count-changing resize, which reflows the
    /// primary screen and leaves no row at the position any previously
    /// issued [`RowId`] named - an id from an earlier epoch is always
    /// answered as gone, never remapped.
    epoch: u64,
    /// The point [`Terminal::changes_since`] hands out and checks a
    /// caller's own point against. Starts at `0`, a value never handed
    /// out to a caller (the first call that returns anything bumps it to
    /// at least `1`), so a caller claiming `0` is always treated the same
    /// as a caller with no point at all.
    seq: u64,
    /// Every [`CommandMark`] queued since the last [`Terminal::
    /// take_command_marks`] (doc-9 R14, OSC 133; TASK-1.79). Drained by
    /// `crates/server/src/pane.rs`'s own worker thread after every
    /// [`write`](Terminal::write) (TASK-1.84): in practice this holds at
    /// most the marks that one `write` call produced, for the short time
    /// between that call and the next drain, never the whole life of a
    /// pane. `resize` never adds to it - `Terminal::resize` only ever
    /// queues `Event::PtyWrite` through `drain_events`, never `Event::
    /// SemanticPrompt`, which only ever comes from parsing a program's
    /// written bytes - so nothing outside `write` needs to drain this
    /// list.
    command_marks: Vec<CommandMark>,
}

/// What a shell reported about a prompt-marked command, from OSC 133
/// (doc-9 R14; TASK-1.79). Only `C` (output started) and `D` (command
/// ended) - this task's own scope; `A`/`B` (prompt/input start) are read
/// by [`command_mark_from`] and dropped, since nothing yet needs them.
/// Adding a variant for them later is additive. Never the engine's own
/// `ghostty_vt::terminal::SemanticPromptEvent`/`SemanticPromptKind` -
/// this is the kernel's own type, so nothing downstream has to depend on
/// `ghostty-vt`'s shapes to read it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandMark {
    /// OSC 133 `C`: the command started producing output.
    OutputStarted,
    /// OSC 133 `D`: the command ended. `None` when the shell reported no
    /// exit code (a bare `D`) or one the engine's own OSC parser could
    /// not read - both report the same way, verified by test - never
    /// guessed.
    Ended {
        /// The command's exit code, if the shell reported one.
        exit_code: Option<i32>,
    },
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
        // `Options::default()`'s byte limit (10 MB) is otherwise still in
        // force alongside the line limit, and "the first limit reached
        // wins" (the engine's own doc comment): with long enough lines
        // the byte limit could silently bind before `scrollback_lines`
        // does, which would make R15's configured number not mean what
        // it says. Disabling it makes the line limit the only one in
        // force.
        engine
            .set_scrollback_max_bytes(None)
            .map_err(Error::Engine)?;
        let render = RenderState::new().map_err(Error::Engine)?;
        Ok(Self {
            engine,
            render,
            cwd: None,
            title: String::new(),
            rows_scrolled_off: 0,
            epoch: 0,
            seq: 0,
            command_marks: Vec::new(),
        })
    }

    /// Feeds bytes through the engine's VT parser (doc-9 R14, R92).
    ///
    /// Drains every event the write produced into this terminal's own
    /// state (`cwd`, `title`): see [`Terminal::drain_events`]. Also
    /// updates [`Terminal::rows_scrolled_off`] when this write pruned
    /// scrollback (a drop in `total_rows`, confirmed by test to happen
    /// only on a real prune for a plain write - never merely because the
    /// active area grew into unused scrollback space, which is a resize's
    /// doing, not a write's).
    ///
    /// Stays infallible, as before: reading `total_rows` around the write
    /// is best-effort bookkeeping, not part of this method's contract, so
    /// an engine error from that secondary read is swallowed rather than
    /// surfaced here.
    pub fn write(&mut self, bytes: &[u8]) {
        let before = self.engine.total_rows().ok();
        self.engine.vt_write(bytes);
        self.drain_events();
        if let (Some(before), Ok(after)) = (before, self.engine.total_rows())
            && after < before
        {
            self.rows_scrolled_off += (before - after) as u64;
        }
    }

    /// Drains whatever `ghostty_vt::Terminal::events` is holding into this
    /// terminal's own state.
    ///
    /// `events` never drains itself, and more than [`Terminal::write`]
    /// can push onto it - [`Terminal::resize`] does too, when mode 2048
    /// (in-band size reports) is on - so every engine call that can queue
    /// an event calls this afterwards, rather than leaving it to whichever
    /// `write` happens to come next.
    fn drain_events(&mut self) {
        let events: Vec<Event> = self.engine.events().collect();
        for event in events {
            match event {
                Event::TitleChanged(title) => self.title = title,
                Event::PwdChanged(pwd) => {
                    if let Some(cwd) = parse_pwd(&pwd) {
                        self.cwd = Some(cwd);
                    }
                }
                Event::SemanticPrompt(prompt) => {
                    if let Some(mark) = command_mark_from(prompt) {
                        self.command_marks.push(mark);
                    }
                }
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
        let rows = all_rows(&snapshot, &palette)?;
        let cursor = read_cursor(&snapshot)?;
        let size = self.current_size()?;
        Ok(Screen { rows, cursor, size })
    }

    /// The terminal's current size, read fresh from the engine (TASK-1.114;
    /// decision-2 section 3, doc-14's size-on-the-wire requirement).
    ///
    /// # Errors
    ///
    /// Returns [`Error::Engine`] if the engine could not report it.
    fn current_size(&self) -> Result<Size, Error> {
        Ok(Size::new(self.cols()?, self.rows()?))
    }

    /// The rows that changed since `seen`, or a whole screen when `seen`
    /// cannot be resumed from (AC #4, #5).
    ///
    /// `seen` is the `seq` a previous [`ScreenUpdate::Delta`] or
    /// [`ScreenUpdate::Full`] carried; `None` for a caller that has never
    /// called this before. There is exactly one current point per
    /// terminal (this method owns the one [`RenderState`] this terminal
    /// has - proved by test that a second, independent render state does
    /// not see what changed for *it*, only what the first one has not
    /// already consumed), so every caller but the single most recently
    /// served one has a `seen` that is not exactly that point, and is
    /// answered with the whole screen rather than a guess at what it is
    /// missing. That is decision-2 section 6's back-pressure rule ("a
    /// client that falls behind gets one fresh screen-sized snapshot"),
    /// extended here to "or was never caught up to begin with."
    ///
    /// Calling this twice with nothing written in between returns
    /// [`ScreenUpdate::Unchanged`] the second time: the first call already
    /// consumed whatever the engine had marked dirty. Several callers
    /// sharing one terminal must not each call this expecting their own
    /// independent view - TASK-1.47 is where several attachments to one
    /// pane are built, and its job is to call this once per tick and fan
    /// the one result out, each attachment tracking its own delivery
    /// progress on top; nothing here assumes there is only ever one
    /// caller, only that calling this is itself the single point of
    /// truth for "what has this terminal not yet reported."
    ///
    /// # Errors
    ///
    /// Returns [`Error::Engine`] if the engine could not report its state.
    pub fn changes_since(&mut self, seen: Option<u64>) -> Result<ScreenUpdate, Error> {
        // `seq` starts at 0, which this method never hands out (the first
        // response bumps it to at least 1), so a caller claiming 0 is
        // never "current" - same as a caller with no point at all.
        let is_current = self.seq != 0 && seen == Some(self.seq);

        let snapshot = self.render.update(&self.engine).map_err(Error::Engine)?;
        let dirty = snapshot.dirty().map_err(Error::Engine)?;
        let palette = snapshot.colors().map_err(Error::Engine)?.palette;
        let cursor = read_cursor(&snapshot)?;

        let outcome = if is_current {
            match dirty {
                Dirty::Clean => None,
                Dirty::Partial => Some(Outcome::Delta(dirty_rows(&snapshot, &palette)?)),
                Dirty::Full => Some(Outcome::Full(all_rows(&snapshot, &palette)?)),
            }
        } else {
            Some(Outcome::Full(all_rows(&snapshot, &palette)?))
        };

        snapshot.clean().map_err(Error::Engine)?;

        Ok(match outcome {
            None => ScreenUpdate::Unchanged,
            Some(Outcome::Delta(rows)) => {
                self.seq += 1;
                ScreenUpdate::Delta {
                    rows,
                    cursor,
                    seq: self.seq,
                }
            }
            Some(Outcome::Full(rows)) => {
                self.seq += 1;
                ScreenUpdate::Full {
                    screen: Screen {
                        rows,
                        cursor,
                        size: self.current_size()?,
                    },
                    seq: self.seq,
                }
            }
        })
    }

    /// The real number of scrollback rows this terminal is holding (AC
    /// #1): never the configured limit, which page-granularity pruning
    /// in the engine only approximates (within a few percent at R15's
    /// real 10,000-line default; measured far looser at a much smaller
    /// configured limit, where a single page can hold many times the
    /// limit's own row count).
    ///
    /// # Errors
    ///
    /// Returns [`Error::Engine`] if the engine could not report it.
    pub fn scrollback_rows_kept(&self) -> Result<usize, Error> {
        self.engine.scrollback_rows().map_err(Error::Engine)
    }

    /// The absolute identity of the oldest row this terminal still
    /// retains in the current epoch - the smallest id
    /// [`Terminal::page`] will answer for, and so the way a caller
    /// bootstraps into absolute addressing without already knowing a
    /// valid id (not named explicitly in this task's plan, found to be
    /// missing once `page` had to be called from anywhere other than an
    /// id `page` itself had already handed back).
    #[must_use]
    pub fn oldest_row(&self) -> RowId {
        RowId {
            epoch: self.epoch,
            offset: self.rows_scrolled_off,
        }
    }

    /// A page of up to `max_rows` rows starting at `first`, addressed by
    /// absolute identity rather than position (AC #2, #3; decision-2
    /// section 3). Covers scrollback and the active area alike - both are
    /// addressed the same way.
    ///
    /// An empty page means every row asked for is gone: `first` names a
    /// row already pruned from the current epoch, or an epoch that has
    /// since ended - never an error, and never a short page padded with
    /// substitutes.
    ///
    /// A one-off query, like [`Terminal::link_at`]: each cell is read
    /// through the engine's point-lookup API, which the engine's own
    /// authors say "may require traversing the full scrollback page list"
    /// and "isn't meant to be used as the core of render loop" - measured
    /// at a few hundred microseconds for a 100-row, 80-column page against
    /// ~10,000 scrollback rows (R15's real default), not promised beyond
    /// that depth, which is untested.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Engine`] if the engine could not read a cell.
    pub fn page(&self, first: RowId, max_rows: u16) -> Result<Page, Error> {
        if first.epoch != self.epoch || first.offset < self.rows_scrolled_off {
            return Ok(Page { rows: Vec::new() });
        }

        let total = self.engine.total_rows().map_err(Error::Engine)?;
        let start = usize::try_from(first.offset - self.rows_scrolled_off).unwrap_or(usize::MAX);
        if start >= total {
            return Ok(Page { rows: Vec::new() });
        }
        let end = start.saturating_add(usize::from(max_rows)).min(total);

        let cols = self.cols()?;
        let palette = self.engine.color_palette().map_err(Error::Engine)?.0;

        let mut rows = Vec::with_capacity(end - start);
        for position in start..end {
            let id = RowId {
                epoch: self.epoch,
                offset: self.rows_scrolled_off + position as u64,
            };
            let row = self.build_page_row(position as u32, cols, &palette)?;
            rows.push((id, row));
        }
        Ok(Page { rows })
    }

    /// Builds one row's attribute runs for [`Terminal::page`], cell by
    /// cell through the engine's point-lookup API rather than the
    /// render-state path [`Terminal::screen`] uses (that path only ever
    /// visits the viewport, plus whatever overscan is requested - never
    /// an arbitrary absolute row).
    fn build_page_row(&self, y: u32, cols: u16, palette: &[EngineRgb; 256]) -> Result<Row, Error> {
        let mut runs: Row = Vec::new();
        for x in 0..cols {
            let grid_ref = self
                .engine
                .grid_ref(Point::Screen(PointCoordinate { x, y }))
                .map_err(Error::Engine)?;
            let raw = grid_ref.cell().map_err(Error::Engine)?;
            let wide = wide_from(raw.wide().map_err(Error::Engine)?);

            // Same reasoning as `build_row`: never rendered, the
            // preceding `Wide` cell already carries this column.
            if wide == Wide::SpacerTail {
                continue;
            }

            let style = grid_ref.style().map_err(Error::Engine)?;
            let has_hyperlink = raw.has_hyperlink().map_err(Error::Engine)?;
            let resolved = resolved_cell_style(style, wide, has_hyperlink, palette);
            let text = graphemes_at(&grid_ref)?;

            match runs.last_mut() {
                Some(last) if last.style == resolved => last.text.push_str(&text),
                _ => runs.push(Run {
                    text,
                    style: resolved,
                }),
            }
        }
        Ok(runs)
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
    /// Drains events the same way [`Terminal::write`] does
    /// ([`Terminal::drain_events`]): a resize with mode 2048 (in-band size
    /// reports) on queues one, and it must not wait for a later `write` to
    /// be drained.
    ///
    /// Ends the current identity epoch when this resize changed the
    /// column count (confirmed by test: a column-count change reflows the
    /// primary screen, which can both inflate the row count with nothing
    /// lost and itself prune rows outright, with no way to tell those two
    /// apart from the outside - so every [`RowId`] issued before this
    /// point is retired rather than guessed at). A row-count-only resize
    /// never does this: confirmed by test, it only moves rows between
    /// scrollback and the active area, never reflowing or pruning.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Engine`] if the engine could not resize, or if its
    /// column count or row total could not be read back afterwards.
    pub fn resize(&mut self, cols: u16, rows: u16) -> Result<(), Error> {
        let cols_before = self.engine.cols().map_err(Error::Engine)?;
        let result = self.engine.resize(cols, rows, 0, 0).map_err(Error::Engine);
        self.drain_events();

        if result.is_ok() {
            let cols_after = self.engine.cols().map_err(Error::Engine)?;
            if cols_after != cols_before {
                self.epoch += 1;
                self.rows_scrolled_off = self.engine.total_rows().map_err(Error::Engine)? as u64;
            }
        }

        result
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

    /// The terminal's working directory, as last reported by OSC 7, OSC
    /// 9;9 or OSC 1337 `CurrentDir=` (AC #2, doc-9 R16). `None` until a
    /// program reports one; a value this crate cannot parse confidently
    /// as a directory (see [`parse_pwd`]) leaves it at whatever it was
    /// before.
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

    /// Every [`CommandMark`] queued since the last call (doc-9 R14, OSC
    /// 133; TASK-1.79). Drains what it returns - unlike `cwd`/`title`,
    /// which are continuous state a later report simply overwrites, this
    /// is a sequence of discrete occurrences: more than one can arrive
    /// from a single [`Terminal::write`] call (two `C`s in a row, for
    /// example), and none of them are lost between two reads just because
    /// nothing asked in between.
    pub fn take_command_marks(&mut self) -> Vec<CommandMark> {
        std::mem::take(&mut self.command_marks)
    }
}

/// What a call to [`Terminal::changes_since`] found, before it is turned
/// into the [`ScreenUpdate`] variant that also carries the cursor and the new
/// `seq` (kept out of the sdk shape so this module can build the rows
/// first and only touch `self.seq` once, after the borrow on
/// `self.render` the snapshot holds has ended).
enum Outcome {
    Delta(Vec<(u16, Row)>),
    Full(Vec<Row>),
}

/// Walks every row the given snapshot holds (the viewport, plus whatever
/// overscan is requested - none, here), building each with [`build_row`].
/// Shared by [`Terminal::screen`] and [`Terminal::changes_since`]'s
/// [`ScreenUpdate::Full`] arm, so a whole-screen read means the same thing from
/// either caller.
fn all_rows(snapshot: &Snapshot<'_>, palette: &[EngineRgb; 256]) -> Result<Vec<Row>, Error> {
    let mut row_iterator = RowIterator::new().map_err(Error::Engine)?;
    let mut row_iteration = row_iterator.update(snapshot).map_err(Error::Engine)?;
    let mut cell_iterator = CellIterator::new().map_err(Error::Engine)?;

    let mut rows = Vec::new();
    while let Some(row) = row_iteration.next() {
        rows.push(build_row(row, &mut cell_iterator, palette)?);
    }
    Ok(rows)
}

/// Walks only the rows the given snapshot's render state has marked
/// dirty, each paired with its position from the top of the screen
/// (`RowIteration::next_dirty`'s own `u16`). Used by
/// [`Terminal::changes_since`]'s [`ScreenUpdate::Delta`] arm; never called when
/// the snapshot's own [`Dirty`] is `Full` or `Clean` (`next_dirty` would
/// return every row, or none, respectively - the caller already branches
/// on `Dirty` to pick the cheaper read).
fn dirty_rows(
    snapshot: &Snapshot<'_>,
    palette: &[EngineRgb; 256],
) -> Result<Vec<(u16, Row)>, Error> {
    let mut row_iterator = RowIterator::new().map_err(Error::Engine)?;
    let mut row_iteration = row_iterator.update(snapshot).map_err(Error::Engine)?;
    let mut cell_iterator = CellIterator::new().map_err(Error::Engine)?;

    let mut rows = Vec::new();
    while let Some((y, row)) = row_iteration.next_dirty() {
        rows.push((y, build_row(row, &mut cell_iterator, palette)?));
    }
    Ok(rows)
}

/// Builds one row's attribute runs by walking its cells left to right,
/// starting a new [`Run`] whenever the resolved [`CellStyle`] changes
/// (decision-2 section 3).
fn build_row(
    row: &RowIteration<'_>,
    cells: &mut CellIterator,
    palette: &[EngineRgb; 256],
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

        let style = cell.style().map_err(Error::Engine)?;
        let has_hyperlink = raw.has_hyperlink().map_err(Error::Engine)?;
        let resolved = resolved_cell_style(style, wide, has_hyperlink, palette);
        let mut text = String::new();
        cell.graphemes_utf8(&mut text).map_err(Error::Engine)?;

        match runs.last_mut() {
            Some(last) if last.style == resolved => last.text.push_str(&text),
            _ => runs.push(Run {
                text,
                style: resolved,
            }),
        }
    }
    Ok(runs)
}

/// Resolves one cell's drawable style from its raw style and flags.
///
/// Shared by [`build_row`] (the render-state path) and
/// [`Terminal::build_page_row`] (the scrollback/screen paging path, which
/// reads a row's cells through the engine's point-lookup API instead), so
/// the two cannot silently resolve the same cell to a different
/// [`CellStyle`] - a risk this task's own review named: the render-state
/// path's `CellIteration::fg_color`/`bg_color` resolve a palette index
/// for free, but the point-lookup path's `GridRef::style` does not, so
/// both paths resolve `fg`/`bg`/`underline_colour` here, uniformly,
/// through [`resolve_style_color`].
fn resolved_cell_style(
    style: EngineStyle,
    wide: Wide,
    has_hyperlink: bool,
    palette: &[EngineRgb; 256],
) -> CellStyle {
    CellStyle {
        fg: resolve_style_color(style.fg_color, palette),
        bg: resolve_style_color(style.bg_color, palette),
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
        link: has_hyperlink,
    }
}

/// Resolves a style colour (possibly a palette index) to the RGB value a
/// client draws, the same resolution the engine already does for a cell's
/// foreground and background (decision-2 section 3).
fn resolve_style_color(color: StyleColor, palette: &[EngineRgb; 256]) -> Option<Rgb> {
    match color {
        StyleColor::None => None,
        StyleColor::Rgb(rgb) => Some(rgb_from(rgb)),
        StyleColor::Palette(index) => palette.get(usize::from(index.0)).copied().map(rgb_from),
    }
}

/// Reads one cell's grapheme cluster through the point-lookup API
/// ([`Terminal::build_page_row`]), growing the buffer once if the first,
/// fixed-size try is too small - the same grow-and-retry shape
/// [`Terminal::link_at`] already uses for a hyperlink's address.
fn graphemes_at(grid_ref: &ghostty_vt::screen::GridRef<'_>) -> Result<String, Error> {
    let mut buf = [' '; 8];
    match grid_ref.graphemes(&mut buf) {
        Ok(n) => Ok(buf[..n].iter().collect()),
        Err(ghostty_vt::Error::OutOfSpace { required }) => {
            let mut grown = vec![' '; required];
            let n = grid_ref.graphemes(&mut grown).map_err(Error::Engine)?;
            Ok(grown[..n].iter().collect())
        }
        Err(other) => Err(Error::Engine(other)),
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
fn read_cursor(snapshot: &Snapshot<'_>) -> Result<Cursor, Error> {
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

/// Turns one engine-level OSC 133 step into this crate's own
/// [`CommandMark`], or `None` for a step this task doesn't need
/// (`PromptStart`/`InputStart` - OSC 133 `A`/`B`). `has_exit_code` is the
/// engine's own say on whether `exit_code` means anything - a bare `D`
/// and a `D` with a code the engine's OSC parser could not read both set
/// it to `false` (verified by test), so trusting it here is already
/// correct; nothing here second-guesses it.
fn command_mark_from(event: ghostty_vt::terminal::SemanticPromptEvent) -> Option<CommandMark> {
    use ghostty_vt::terminal::SemanticPromptKind;
    match event.kind {
        SemanticPromptKind::OutputStart => Some(CommandMark::OutputStarted),
        SemanticPromptKind::CommandEnd => Some(CommandMark::Ended {
            exit_code: event.has_exit_code.then_some(event.exit_code),
        }),
        SemanticPromptKind::PromptStart | SemanticPromptKind::InputStart => None,
    }
}

/// Parses the value an `Event::PwdChanged` carries into a plain path.
///
/// `PwdChanged` carries the raw, unvalidated value of whichever of three
/// OSC sequences set it (checked against ghostty's own parsers,
/// `third_party/ghostty/src/terminal/osc/parsers/{report_pwd,osc9,
/// iterm2}.zig`):
///
/// - OSC 7 (`report_pwd.zig`): whatever bytes follow `7;`, unparsed - a
///   `file://<host>/<path>` URI in practice, but the engine does not check
///   that it is one.
/// - OSC 9;9 (`osc9.zig`, a ConEmu extension): whatever bytes follow
///   `9;9;`, equally unvalidated - ghostty's own test feeds it `"ab"`.
/// - OSC 1337 `CurrentDir=` (`iterm2.zig`): whatever follows `=`, same
///   story - ghostty's own test feeds it `"abc123"`.
///
/// So this crate is the only place that decides what counts as a
/// directory: a `file://` URI with a non-empty path (`/` for an empty one,
/// since `file:///` and `file://host/` both mean the root), or a value
/// with no scheme that is itself an absolute path. Anything else - another
/// scheme, a relative path, an empty value, or a `file://` path whose
/// percent-encoding does not decode - is `None`, and the caller keeps
/// whatever `cwd` it already had rather than replace it with a guess.
fn parse_pwd(raw: &str) -> Option<String> {
    if let Some(after_scheme) = raw.strip_prefix("file://") {
        // `after_scheme` is `<host>/<path>`; `split_once` consumes the
        // separating `/`, so it is put back in front of the decoded path.
        // A host with no `/` at all - not even a trailing one - has no
        // path to take, well-formed or not.
        let (_host, path) = after_scheme.split_once('/')?;
        return Some(format!("/{}", percent_decode(path)?));
    }
    if raw.starts_with('/') {
        return Some(raw.to_string());
    }
    None
}

/// Decodes `%XX` escapes in a URI path. `None` on any malformed escape,
/// rather than passing invalid-looking bytes through unchanged: the
/// caller treats that as a value it cannot parse confidently, not as a
/// path with a few odd characters in it. Byte-oriented on purpose: it
/// never slices the input `&str` by index, so malformed input - this is a
/// path a running program chose, not a trusted value - cannot panic on a
/// non-UTF-8-boundary slice.
fn percent_decode(value: &str) -> Option<String> {
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
        if bytes[i] == b'%' {
            let high = bytes.get(i + 1).copied().and_then(hex_digit)?;
            let low = bytes.get(i + 2).copied().and_then(hex_digit)?;
            out.push(high * 16 + low);
            i += 3;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8(out).ok()
}

/// A hyperlink's URI, as UTF-8 bytes from the engine's buffer. Lossy on
/// purpose: a malformed URI from a running program should not crash the
/// query that asked about it.
fn uri_from_bytes(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

#[cfg(test)]
mod tests {
    use steg_sdk::{Rgb, RowId, ScreenUpdate, Underline, Wide};

    use super::{CommandMark, CursorShape, Terminal};

    // --- OSC 133 command marks (doc-9 R14; TASK-1.79) -----------------------

    #[test]
    fn output_started_mark_on_osc_133_c() {
        let mut term = Terminal::new(10, 2, 100).unwrap();

        term.write(b"\x1b]133;C\x07");

        assert_eq!(term.take_command_marks(), vec![CommandMark::OutputStarted]);
    }

    #[test]
    fn command_ended_with_its_exit_code_on_osc_133_d() {
        let mut term = Terminal::new(10, 2, 100).unwrap();

        term.write(b"\x1b]133;C\x07\x1b]133;D;0\x07");

        assert_eq!(
            term.take_command_marks(),
            vec![
                CommandMark::OutputStarted,
                CommandMark::Ended { exit_code: Some(0) }
            ]
        );
    }

    #[test]
    fn command_ended_with_no_exit_code_reports_none() {
        // A bare `D` - some shells send this to close an already-open mark
        // with nothing more to say (zsh/fish/bash's own integration
        // scripts all have this path).
        let mut term = Terminal::new(10, 2, 100).unwrap();

        term.write(b"\x1b]133;D\x07");

        assert_eq!(
            term.take_command_marks(),
            vec![CommandMark::Ended { exit_code: None }]
        );
    }

    #[test]
    fn command_ended_with_a_malformed_exit_code_reports_none() {
        // The engine's own OSC parser treats a code it can't read the same
        // way it treats no code at all - verified by test, not assumed, so
        // nothing here needs its own validation beyond trusting the
        // engine's `has_exit_code`.
        let mut term = Terminal::new(10, 2, 100).unwrap();

        term.write(b"\x1b]133;D;abc\x07");

        assert_eq!(
            term.take_command_marks(),
            vec![CommandMark::Ended { exit_code: None }]
        );
    }

    #[test]
    fn command_ended_fires_with_no_preceding_output_started() {
        // The engine enforces no ordering between marks ("shells differ in
        // what they report... handle each event on its own" - the sys
        // crate's own doc comment on the callback type).
        let mut term = Terminal::new(10, 2, 100).unwrap();

        term.write(b"\x1b]133;D;5\x07");

        assert_eq!(
            term.take_command_marks(),
            vec![CommandMark::Ended { exit_code: Some(5) }]
        );
    }

    #[test]
    fn two_output_started_marks_in_a_row_are_both_kept() {
        // A shell may redraw the same prompt more than once (the sys
        // crate's own doc comment: "treat a repeated PROMPT_START as
        // harmless"); this crate passes each mark through rather than
        // deduping - idempotency, if wanted, is for whatever reads
        // `take_command_marks` next, not for this module.
        let mut term = Terminal::new(10, 2, 100).unwrap();

        term.write(b"\x1b]133;C\x07\x1b]133;C\x07\x1b]133;D;7\x07");

        assert_eq!(
            term.take_command_marks(),
            vec![
                CommandMark::OutputStarted,
                CommandMark::OutputStarted,
                CommandMark::Ended { exit_code: Some(7) }
            ]
        );
    }

    #[test]
    fn a_mark_split_across_two_writes_still_resolves() {
        // The real engine's own OSC parser carries a partial sequence's
        // state across two separate `write` calls - exactly the buffering
        // a hand-rolled byte-scanner over the pty stream would have to
        // reimplement for itself (decision-3's own argument for choosing
        // an engine with a real parser in the first place).
        let mut term = Terminal::new(10, 2, 100).unwrap();

        term.write(b"\x1b]133;D;");
        term.write(b"9\x07");

        assert_eq!(
            term.take_command_marks(),
            vec![CommandMark::Ended { exit_code: Some(9) }]
        );
    }

    #[test]
    fn take_command_marks_drains_what_it_returns() {
        let mut term = Terminal::new(10, 2, 100).unwrap();
        term.write(b"\x1b]133;C\x07");

        let first = term.take_command_marks();
        let second = term.take_command_marks();

        assert_eq!(first, vec![CommandMark::OutputStarted]);
        assert_eq!(second, vec![]);
    }

    #[test]
    fn a_shell_with_no_prompt_marks_produces_no_command_marks() {
        // AC #4 of the parent, now-split task: nothing is guessed. Plain
        // output with no OSC 133 at all produces an empty queue, not a
        // default or inferred mark.
        let mut term = Terminal::new(10, 2, 100).unwrap();

        term.write(b"ordinary output, no escape sequences\n");

        assert_eq!(term.take_command_marks(), vec![]);
    }

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
    fn a_file_uri_with_no_host_and_no_path_reports_the_root() {
        let mut term = Terminal::new(10, 2, 100).unwrap();

        term.write(b"\x1b]7;file:///\x07");

        assert_eq!(term.cwd(), Some("/"));
    }

    #[test]
    fn a_file_uri_with_a_host_and_an_empty_path_reports_the_root() {
        let mut term = Terminal::new(10, 2, 100).unwrap();

        term.write(b"\x1b]7;file://host/\x07");

        assert_eq!(term.cwd(), Some("/"));
    }

    #[test]
    fn a_file_uri_with_a_host_and_no_slash_at_all_is_ignored() {
        let mut term = Terminal::new(10, 2, 100).unwrap();
        term.write(b"\x1b]7;file:///tmp/first\x07");
        assert_eq!(term.cwd(), Some("/tmp/first"));

        // A host with nothing after it at all - not even a trailing `/` -
        // has no path to take, well-formed or not.
        term.write(b"\x1b]7;file://host\x07");

        assert_eq!(term.cwd(), Some("/tmp/first"));
    }

    #[test]
    fn a_plain_absolute_path_is_accepted_through_osc_1337_current_dir() {
        // OSC 1337 `CurrentDir=` (`iterm2.zig`) carries whatever follows
        // `=` with no validation of its own - ghostty's own test feeds it
        // a bare `"abc123"` - so a plain absolute path through it is a
        // real path the engine supports, not an invented one.
        let mut term = Terminal::new(10, 2, 100).unwrap();

        term.write(b"\x1b]1337;CurrentDir=/tmp/plain\x07");

        assert_eq!(term.cwd(), Some("/tmp/plain"));
    }

    #[test]
    fn a_value_with_another_scheme_is_ignored() {
        let mut term = Terminal::new(10, 2, 100).unwrap();
        term.write(b"\x1b]7;file:///tmp/first\x07");
        assert_eq!(term.cwd(), Some("/tmp/first"));

        term.write(b"\x1b]7;http://x/y\x07");

        assert_eq!(term.cwd(), Some("/tmp/first"));
    }

    #[test]
    fn a_relative_path_is_ignored() {
        let mut term = Terminal::new(10, 2, 100).unwrap();
        term.write(b"\x1b]7;file:///tmp/first\x07");
        assert_eq!(term.cwd(), Some("/tmp/first"));

        term.write(b"\x1b]7;relative/path\x07");

        assert_eq!(term.cwd(), Some("/tmp/first"));
    }

    #[test]
    fn an_empty_value_is_ignored() {
        let mut term = Terminal::new(10, 2, 100).unwrap();
        term.write(b"\x1b]7;file:///tmp/first\x07");
        assert_eq!(term.cwd(), Some("/tmp/first"));

        term.write(b"\x1b]7;\x07");

        assert_eq!(term.cwd(), Some("/tmp/first"));
    }

    #[test]
    fn bad_percent_encoding_is_ignored() {
        let mut term = Terminal::new(10, 2, 100).unwrap();
        term.write(b"\x1b]7;file:///tmp/first\x07");
        assert_eq!(term.cwd(), Some("/tmp/first"));

        // `%zz` is not a hex escape.
        term.write(b"\x1b]7;file:///tmp/%zzbad\x07");

        assert_eq!(term.cwd(), Some("/tmp/first"));
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
    fn link_at_grows_its_buffer_for_an_address_longer_than_the_first_try() {
        // Longer than `link_at`'s first, 256-byte buffer, so this only
        // passes if the `OutOfSpace` branch retries with a bigger one.
        let long_uri = format!("http://example.com/{}", "a".repeat(300));
        assert!(long_uri.len() > 256);

        let mut term = Terminal::new(20, 1, 100).unwrap();
        term.write(format!("\x1b]8;;{long_uri}\x1b\\hi\x1b]8;;\x1b\\").as_bytes());

        assert_eq!(term.link_at(0, 0).unwrap(), Some(long_uri));
    }

    #[test]
    fn resize_drains_the_in_band_size_report_event_so_it_does_not_leak_to_the_next_read() {
        use ghostty_vt::terminal::{Mode, ModeKind};

        let mut term = Terminal::new(10, 2, 100).unwrap();
        term.engine
            .set_mode(Mode::new(2048, ModeKind::Dec), true)
            .unwrap();

        term.resize(5, 3).unwrap();

        // `resize` must have drained whatever it queued itself: nothing
        // is left over for a later read of the engine's own queue to see
        // (`Terminal` has no public accessor for it - this module's own
        // test code can reach the private `engine` field directly, which
        // is the only way to observe that the queue, not just this type's
        // own state, ended up empty).
        let leftover: Vec<_> = term.engine.events().collect();
        assert!(
            leftover.is_empty(),
            "resize must drain what it queues itself, not leave it for the next write/resize \
             to see: {leftover:?}"
        );
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

    #[test]
    fn growing_only_the_columns_keeps_the_text_readable_at_the_new_width() {
        let mut term = Terminal::new(5, 2, 100).unwrap();
        term.write(b"\x1b[?7h01234");

        term.resize(10, 2).unwrap();

        assert_eq!((term.cols().unwrap(), term.rows().unwrap()), (10, 2));
        let screen = term.screen().unwrap();
        assert_eq!(screen.rows[0][0].text, "01234");
    }

    #[test]
    fn shrinking_only_the_columns_reflows_the_text() {
        let mut term = Terminal::new(10, 2, 100).unwrap();
        term.write(b"\x1b[?7h0123456789");

        term.resize(5, 2).unwrap();

        assert_eq!((term.cols().unwrap(), term.rows().unwrap()), (5, 2));
        let screen = term.screen().unwrap();
        assert_eq!(screen.rows[0][0].text, "01234");
        assert_eq!(screen.rows[1][0].text, "56789");
    }

    #[test]
    fn growing_only_the_rows_keeps_the_text_readable_at_the_new_height() {
        let mut term = Terminal::new(5, 2, 100).unwrap();
        term.write(b"\x1b[?7h01234");

        term.resize(5, 5).unwrap();

        assert_eq!((term.cols().unwrap(), term.rows().unwrap()), (5, 5));
        let screen = term.screen().unwrap();
        assert_eq!(screen.rows[0][0].text, "01234");
    }

    #[test]
    fn shrinking_only_the_rows_keeps_the_active_lines_text() {
        let mut term = Terminal::new(5, 4, 100).unwrap();
        term.write(b"\x1b[?7h01234\r\n56789");

        term.resize(5, 2).unwrap();

        assert_eq!((term.cols().unwrap(), term.rows().unwrap()), (5, 2));
        let screen = term.screen().unwrap();
        assert_eq!(screen.rows[0][0].text, "01234");
        assert_eq!(screen.rows[1][0].text, "56789");
    }

    // --- AC #1: a configurable scrollback limit, with the real count kept reported ---

    #[test]
    fn scrollback_rows_kept_grows_as_lines_are_fed_in() {
        let mut term = Terminal::new(10, 2, 500).unwrap();
        assert_eq!(term.scrollback_rows_kept().unwrap(), 0);

        for i in 0..40 {
            term.write(format!("line{i:03}\r\n").as_bytes());
        }

        // 40 lines into a 2-row viewport: most of them are in scrollback.
        assert!(
            (30..=40).contains(&term.scrollback_rows_kept().unwrap()),
            "expected scrollback to have grown with the lines written: {}",
            term.scrollback_rows_kept().unwrap()
        );
    }

    #[test]
    fn the_configured_limit_keeps_the_real_count_near_itself_once_warmed_up() {
        let mut term = Terminal::new(10, 2, 500).unwrap();
        let line = "x".repeat(60);

        // Many more lines than the 500-line limit, so pruning has
        // happened more than once by the end.
        for _ in 0..4000 {
            term.write(format!("{line}\r\n").as_bytes());
        }

        let kept = term.scrollback_rows_kept().unwrap();
        // Page-granularity pruning only approximates the configured
        // limit, and how loosely depends on how many rows fit in one
        // ~400 KiB page: this task's plan measured a tight 9,434-10,024
        // sawtooth (about +/-6%) around a real 10,000-line limit with
        // realistic line lengths, but a much larger overshoot - several
        // times the limit - at a small configured limit with short
        // lines, because one page then holds far more rows than the
        // limit itself. This test's own 500-line limit with 60-byte
        // lines sits in between: the bound below is generous enough not
        // to be fragile against the engine's exact pruning schedule,
        // while still proving pruning bounds growth at all (4,000 lines
        // were written; nowhere near that many are kept).
        assert!(
            (400..=3500).contains(&kept),
            "expected the retained count to be bounded well under the 4,000 lines written, not {kept}"
        );
    }

    #[test]
    fn the_line_limit_governs_even_past_the_engines_disabled_byte_limit() {
        // `ghostty_vt::terminal::Options::default()`'s byte limit (10 MB)
        // is otherwise still in force alongside the line limit, and "the
        // first limit reached wins" (the engine's own doc comment) -
        // `Terminal::new` disables it so the configured line count is the
        // only limit left. This writes well past 10 MB of content while
        // staying far under a generous line limit: if the byte limit were
        // still active, content would have started pruning around line
        // ~200 (10 MB / ~50,000 bytes per line), long before this test's
        // 300 lines or its 1,000-line configured limit.
        let mut term = Terminal::new(10, 2, 1000).unwrap();
        let line = "x".repeat(50_000);
        for _ in 0..300 {
            term.write(format!("{line}\r\n").as_bytes());
        }

        let kept = term.scrollback_rows_kept().unwrap();
        assert!(
            kept > 250,
            "the byte limit appears to still be in force: only {kept} rows kept"
        );
    }

    // --- AC #2: every row has an absolute identity that only grows and is never reused ---

    #[test]
    fn every_row_id_grows_and_is_never_reused_for_different_content() {
        let mut term = Terminal::new(10, 2, 50).unwrap();

        // Feed enough numbered lines to prune scrollback at least once,
        // all within one epoch (no resize anywhere near this test).
        for i in 0..3000 {
            term.write(format!("line{i:05}\r\n").as_bytes());
        }

        let floor = term.oldest_row();
        let page = term.page(floor, 1).unwrap();
        assert!(
            !page.rows.is_empty(),
            "the oldest retained row should still answer for its own id"
        );
        let (oldest_id, oldest_row) = page.rows[0].clone();
        assert_eq!(oldest_id, floor);
        let oldest_text: String = oldest_row.iter().map(|run| run.text.as_str()).collect();

        // An id older than the floor (already pruned) is gone, not
        // substituted with whatever now happens to be nearby.
        if oldest_id.offset > 0 {
            let pruned_id = RowId {
                epoch: oldest_id.epoch,
                offset: oldest_id.offset - 1,
            };
            assert!(
                term.page(pruned_id, 1).unwrap().rows.is_empty(),
                "a row older than the floor should be reported gone"
            );
        }

        // Write enough more lines to prune further, past the row that
        // was oldest a moment ago.
        for i in 3000..6000 {
            term.write(format!("line{i:05}\r\n").as_bytes());
        }

        // The previously-oldest row's id must still be gone - never
        // remapped to whatever content now sits at a higher offset.
        assert!(
            term.page(oldest_id, 1).unwrap().rows.is_empty(),
            "a pruned id must never answer with different content later"
        );

        // The id handed to the oldest row now is strictly greater, and
        // its content is different (the id was not reused).
        let new_floor = term.oldest_row();
        assert!(new_floor.offset > oldest_id.offset);
        let new_page = term.page(new_floor, 1).unwrap();
        let new_text: String = new_page.rows[0].1[0].text.clone();
        assert_ne!(new_text, oldest_text);
    }

    #[test]
    fn a_page_spanning_the_seam_between_scrollback_and_the_active_screen_reads_consecutive_rows_in_order()
     {
        // decision-2 section 3 names exactly this seam - a page that
        // starts in scrollback and crosses into the live screen - as the
        // historical bug absolute identity was built to fix (two
        // different moments stitched together at the seam).
        let mut term = Terminal::new(10, 3, 1000).unwrap();
        for i in 0..20 {
            term.write(format!("L{i:02}\r\n").as_bytes());
        }

        let scrollback = term.scrollback_rows_kept().unwrap();
        let viewport_rows = usize::from(term.rows().unwrap());
        let floor = term.oldest_row();
        assert!(
            scrollback >= 2,
            "need at least two scrollback rows before the seam for this test"
        );

        // Two rows before the seam, reading straight through it.
        let start = RowId {
            epoch: floor.epoch,
            offset: floor.offset + scrollback as u64 - 2,
        };
        let max_rows = 2 + viewport_rows as u16;

        let page = term.page(start, max_rows).unwrap();

        assert_eq!(page.rows.len(), usize::from(max_rows));
        for pair in page.rows.windows(2) {
            assert_eq!(pair[0].0.epoch, pair[1].0.epoch);
            assert_eq!(
                pair[1].0.offset,
                pair[0].0.offset + 1,
                "ids across the seam must stay consecutive"
            );
        }

        // The last `viewport_rows` entries are exactly today's active
        // screen, read independently through `screen()`: the two reads
        // must agree row for row across the seam.
        let screen = term.screen().unwrap();
        for i in 0..viewport_rows {
            let (_, page_row) = &page.rows[2 + i];
            assert_eq!(
                *page_row, screen.rows[i],
                "row {i} of the active area should match between page() and screen()"
            );
        }
    }

    #[test]
    fn the_newest_rows_id_resolves_and_one_past_it_is_gone() {
        let mut term = Terminal::new(10, 2, 100).unwrap();
        term.write(b"hi");

        let floor = term.oldest_row();
        let total = term.scrollback_rows_kept().unwrap() + usize::from(term.rows().unwrap());
        let newest_offset = floor.offset + total as u64 - 1;

        let newest = RowId {
            epoch: floor.epoch,
            offset: newest_offset,
        };
        assert!(
            !term.page(newest, 1).unwrap().rows.is_empty(),
            "the newest row's own id should resolve"
        );

        let one_past = RowId {
            epoch: floor.epoch,
            offset: newest_offset + 1,
        };
        assert!(
            term.page(one_past, 1).unwrap().rows.is_empty(),
            "one past the newest row should be reported gone, not substituted"
        );
    }

    #[test]
    fn a_page_request_larger_than_what_is_kept_returns_only_what_exists() {
        let mut term = Terminal::new(10, 2, 100).unwrap();
        term.write(b"hi");

        let floor = term.oldest_row();
        let total = term.scrollback_rows_kept().unwrap() + usize::from(term.rows().unwrap());

        let page = term.page(floor, u16::MAX).unwrap();

        assert_eq!(
            page.rows.len(),
            total,
            "a page request larger than what is kept must return only what exists, not pad or error"
        );
    }

    // --- AC #3: what a resize does to row identity ---

    #[test]
    fn a_column_changing_resize_retires_every_id_from_before_it() {
        let mut term = Terminal::new(40, 3, 1000).unwrap();
        for i in 0..3000 {
            term.write(format!("line{i:05}\r\n").as_bytes());
        }
        let id_before = term.oldest_row();
        assert!(!term.page(id_before, 1).unwrap().rows.is_empty());

        term.resize(10, 3).unwrap(); // column count changes: reflows

        assert!(
            term.page(id_before, 1).unwrap().rows.is_empty(),
            "an id from before a column-changing resize must be reported gone, not remapped"
        );
    }

    #[test]
    fn a_row_only_resize_never_retires_an_id() {
        let mut term = Terminal::new(10, 10, 100).unwrap();
        for i in 0..30 {
            term.write(format!("line{i:03}\r\n").as_bytes());
        }
        let id = term.oldest_row();
        let before = term.page(id, 1).unwrap();
        assert!(!before.rows.is_empty());
        let before_text = before.rows[0].1[0].text.clone();

        term.resize(10, 3).unwrap(); // rows only, cols unchanged
        term.resize(10, 20).unwrap(); // rows only, the other direction

        let after = term.page(id, 1).unwrap();
        assert!(
            !after.rows.is_empty(),
            "a row-only resize must not retire this id"
        );
        assert_eq!(before_text, after.rows[0].1[0].text);
    }

    #[test]
    fn a_resize_to_the_same_size_does_not_end_the_epoch() {
        let mut term = Terminal::new(10, 3, 100).unwrap();
        for i in 0..10 {
            term.write(format!("line{i:02}\r\n").as_bytes());
        }
        let id = term.oldest_row();
        let before = term.page(id, 1).unwrap();
        assert!(!before.rows.is_empty());

        term.resize(10, 3).unwrap(); // same cols, same rows: a true no-op

        assert_eq!(
            term.oldest_row(),
            id,
            "a no-op resize must not move the floor or bump the epoch"
        );
        let after = term.page(id, 1).unwrap();
        assert!(
            !after.rows.is_empty(),
            "a no-op resize must not retire this id"
        );
        assert_eq!(before.rows[0].1, after.rows[0].1);
    }

    // --- AC #4: the rows changed since a given point ---

    /// Unwraps an [`ScreenUpdate::Full`] for a test assertion, or fails with a
    /// message naming which variant actually came back. A plain
    /// wildcard arm is refused here (`clippy::wildcard_enum_match_arm`,
    /// since `ScreenUpdate` is this crate's own enum, not a foreign one), so
    /// every arm is named.
    fn expect_full(update: ScreenUpdate) -> (steg_sdk::Screen, u64) {
        match update {
            ScreenUpdate::Full { screen, seq } => (screen, seq),
            ScreenUpdate::Unchanged => panic!("expected Full, got Unchanged"),
            ScreenUpdate::Delta { .. } => panic!("expected Full, got Delta"),
        }
    }

    /// Same as [`expect_full`], for [`ScreenUpdate::Delta`].
    fn expect_delta(update: ScreenUpdate) -> Vec<(u16, steg_sdk::Row)> {
        match update {
            ScreenUpdate::Delta { rows, .. } => rows,
            ScreenUpdate::Unchanged => panic!("expected Delta, got Unchanged"),
            ScreenUpdate::Full { .. } => panic!("expected Delta, got Full"),
        }
    }

    #[test]
    fn changes_since_gives_a_full_screen_to_a_caller_with_no_point() {
        let mut term = Terminal::new(10, 2, 100).unwrap();
        term.write(b"hi");

        let (screen, seq) = expect_full(term.changes_since(None).unwrap());
        assert_eq!(screen.rows[0][0].text, "hi");
        assert!(seq > 0);
    }

    #[test]
    fn changes_since_reports_unchanged_when_nothing_happened_since_the_last_call() {
        let mut term = Terminal::new(10, 2, 100).unwrap();
        term.write(b"hi");
        let (_, seq) = expect_full(term.changes_since(None).unwrap());

        assert_eq!(
            term.changes_since(Some(seq)).unwrap(),
            ScreenUpdate::Unchanged
        );
    }

    #[test]
    fn changes_since_reports_a_delta_of_only_the_changed_rows() {
        let mut term = Terminal::new(10, 3, 100).unwrap();
        term.write(b"first\r\nsecond");
        let (_, seq) = expect_full(term.changes_since(None).unwrap());

        term.write(b"\r\nthird");

        let rows = expect_delta(term.changes_since(Some(seq)).unwrap());
        assert!(!rows.is_empty());
        let changed: Vec<u16> = rows.iter().map(|(y, _)| *y).collect();
        assert!(
            changed.contains(&2),
            "row 2 (\"third\") should be in the delta: {changed:?}"
        );
    }

    #[test]
    fn changes_since_gives_a_stale_or_foreign_point_a_full_screen_not_a_partial_one() {
        let mut term = Terminal::new(10, 2, 100).unwrap();
        term.write(b"hi");
        let _ = term.changes_since(None).unwrap();
        term.write(b" there");

        let (screen, _) = expect_full(term.changes_since(Some(999)).unwrap());
        assert_eq!(screen.rows[0][0].text, "hi there");
    }

    // --- AC #5: several readers at different points ---

    #[test]
    fn two_callers_sharing_one_terminal_must_each_track_their_own_point() {
        let mut term = Terminal::new(20, 2, 100).unwrap();
        term.write(b"first");

        // Caller A reads first and gets everything (`Full`, as any first
        // read must).
        let (_, seq_a) = expect_full(term.changes_since(None).unwrap());

        term.write(b" second");

        // Caller B, with no point of its own, also gets a whole screen -
        // not whatever A has already consumed from the shared render
        // state (proving a `RenderState` per reader would not be needed
        // even if it worked, since this one shared state already answers
        // a late caller correctly - and proving it does *not* work is
        // what the next assertion is for).
        let (screen, _) = expect_full(term.changes_since(None).unwrap());
        assert_eq!(screen.rows[0][0].text, "first second");

        // Caller A, calling again with its *own* previous point, now also
        // gets a whole screen rather than a delta of what changed since
        // its own last read: B's call displaced A as "the" caller this
        // terminal considers current. A single shared `RenderState`
        // cannot serve two readers their own deltas at once - exactly why
        // TASK-1.47's fan-out (one call per tick, copied to every
        // attachment) is the only correct way to serve several readers,
        // not an independent call per attachment.
        let _ = expect_full(term.changes_since(Some(seq_a)).unwrap());
    }
}
