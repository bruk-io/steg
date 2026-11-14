//! One thread owns one terminal: bytes in over a channel, snapshots and updates out.
//!
//! This is the ownership shape `.claude/rules/rust.md` prescribes, and the one TASK-1.1 found
//! that `libghostty-vt` forces (its handles are `!Send + !Sync`), so the answer holds for either
//! library. `alacritty_terminal` is the instrument here, not the choice.
//!
//! Two things in this module are load-bearing for the protocol, not for the instrument:
//!
//! - The snapshot and the subscription that follows it are produced in one step, so an
//!   attachment's stream resumes at exactly `snapshot.seq + 1`.
//! - When an attachment's bounded queue is full, nothing is queued for it until it has been
//!   given a fresh snapshot. That is the back-pressure policy, and it is why neither data
//!   candidate can avoid the snapshot path.

use std::collections::{HashMap, VecDeque};
use std::io::Write as _;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::time::{Duration, Instant};

use alacritty_terminal::Term;
use alacritty_terminal::event::VoidListener;
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::{Column, Line};
use alacritty_terminal::term::cell::{Cell, Flags};
use alacritty_terminal::term::{Config, TermMode};
use alacritty_terminal::vte::ansi::{Color, NamedColor, Processor, Rgb};
use probe_wire::{
    Cursor, DataMode, Delta, FLAG_BOLD, FLAG_DIM, FLAG_INVERSE, FLAG_ITALIC, FLAG_UNDERLINE, Frame,
    Row, Run, ScrollbackResult, Snapshot, Stats, Update, diff_screen,
};
use tokio::sync::{Notify, mpsc, oneshot};

use crate::pty::Pty;

/// R15's default, and the only scrollback limit this spike keeps.
pub const SCROLLBACK_LIMIT: usize = 10_000;

/// Terminal output is fed to the parser in slices this size, and the scrollback is accounted
/// after each one. Smaller than the emulator's own history, so no line can scroll past unseen.
const FEED_SLICE: usize = 8 * 1024;

/// The emulator's own history is drained into the server's scrollback once it reaches this, which
/// keeps the count of scrolled-off lines exact however much output goes through.
const DRAIN_HISTORY_AT: usize = 2_000;

const DEFAULT_FG: u32 = 0x00d0_d0d0;
const DEFAULT_BG: u32 = 0x0000_0000;

/// What the server does when an attachment's queue is full.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Overflow {
    /// Drop what could not be queued and send one fresh snapshot when there is room again.
    Resync,
    /// Wait for the client. Measured because it is the policy R19's second sentence forbids:
    /// it stalls the program and every other pane this thread serves.
    Block,
}

impl Overflow {
    /// # Errors
    /// Returns the name back when it is neither policy.
    pub fn named(name: &str) -> Result<Self, String> {
        match name {
            "resync" => Ok(Self::Resync),
            "block" => Ok(Self::Block),
            other => Err(format!(
                "unknown overflow policy {other}, expected resync or block"
            )),
        }
    }
}

/// What the async side sends to the thread that owns the terminal.
pub enum TermMsg {
    PtyBytes(Vec<u8>),
    Input(Vec<u8>),
    Resize {
        cols: u16,
        rows: u16,
    },
    Capture {
        scrollback: bool,
        reply: oneshot::Sender<String>,
    },
    Scrollback {
        from: u64,
        to: u64,
        reply: oneshot::Sender<ScrollbackResult>,
    },
    Attach {
        attachment: u32,
        stream: u32,
        mode: DataMode,
        cols: u16,
        rows: u16,
        queue: mpsc::Sender<Frame>,
        notify: Arc<Notify>,
        reply: oneshot::Sender<Snapshot>,
    },
    Detach {
        attachment: u32,
    },
    Stats {
        reply: oneshot::Sender<Stats>,
    },
    /// Every attachment: the size it asked for (two clients on one pane each keep their own,
    /// R7), and what the server has queued for it and had to drop.
    Attachments {
        reply: oneshot::Sender<Vec<Attached>>,
    },
}

/// Counters the measurements read out of the server itself.
#[derive(Debug, Default)]
pub struct Counters {
    pub wire_bytes: std::sync::atomic::AtomicU64,
    pub wire_frames: std::sync::atomic::AtomicU64,
}

/// One attachment, as `pane.attachments` reports it.
pub struct Attached {
    pub attachment: u32,
    pub cols: u16,
    pub rows: u16,
    pub mode: &'static str,
    pub sent: u64,
    pub dropped: u64,
    pub overflowed: bool,
}

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

struct Attachment {
    stream: u32,
    mode: DataMode,
    cols: u16,
    rows: u16,
    queue: mpsc::Sender<Frame>,
    notify: Arc<Notify>,
    /// True once a frame could not be queued: nothing more is queued until a fresh snapshot is.
    overflowed: bool,
    dropped: u64,
    sent: u64,
}

pub struct TerminalThread {
    pane: u32,
    term: Term<VoidListener>,
    parser: Processor,
    pty: Pty,
    size: Size,
    seq: u64,
    tick: Duration,
    overflow: Overflow,
    /// The server's own scrollback, R15's window. Exact: the emulator's history is drained into
    /// it before the emulator can drop a line.
    scrollback: VecDeque<Row>,
    /// Every row that has ever scrolled off, counted, so each one has an absolute id.
    rows_ever: u64,
    history_seen: usize,
    pending_appended: Vec<Row>,
    pending_dropped: u32,
    prev_screen: Vec<Row>,
    prev_cursor: Cursor,
    attachments: HashMap<u32, Attachment>,
    pty_bytes: u64,
    ticks: u64,
    overflows: u64,
    resyncs: u64,
    blocked: Duration,
    force_resync: Option<String>,
}

impl TerminalThread {
    pub fn new(
        pane: u32,
        pty: Pty,
        cols: u16,
        rows: u16,
        tick: Duration,
        overflow: Overflow,
    ) -> Self {
        let size = Size {
            cols: usize::from(cols),
            rows: usize::from(rows),
        };
        let config = Config {
            scrolling_history: DRAIN_HISTORY_AT * 2,
            ..Config::default()
        };
        let term = Term::new(config, &size, VoidListener);
        Self {
            pane,
            term,
            parser: Processor::new(),
            pty,
            size,
            seq: 0,
            tick,
            overflow,
            scrollback: VecDeque::new(),
            rows_ever: 0,
            history_seen: 0,
            pending_appended: Vec::new(),
            pending_dropped: 0,
            prev_screen: Vec::new(),
            prev_cursor: Cursor {
                col: 0,
                row: 0,
                visible: true,
            },
            attachments: HashMap::new(),
            pty_bytes: 0,
            ticks: 0,
            overflows: 0,
            resyncs: 0,
            blocked: Duration::ZERO,
            force_resync: None,
        }
    }

    /// The thread's whole life. Returns when the connection to the async side is dropped or a
    /// shutdown is asked for.
    pub fn run(mut self, rx: Receiver<TermMsg>) {
        let mut published = Instant::now();
        loop {
            let waited = match rx.recv_timeout(self.tick.max(Duration::from_millis(1))) {
                Ok(msg) => {
                    if !self.handle(msg) {
                        break;
                    }
                    false
                }
                Err(RecvTimeoutError::Timeout) => true,
                Err(RecvTimeoutError::Disconnected) => break,
            };
            if waited || published.elapsed() >= self.tick {
                self.publish();
                published = Instant::now();
            }
        }
    }

    /// False means stop.
    fn handle(&mut self, msg: TermMsg) -> bool {
        match msg {
            TermMsg::PtyBytes(bytes) => self.feed(&bytes),
            TermMsg::Input(bytes) => {
                // The thread that owns the terminal owns the pty's writer too, so input and
                // output have one owner. Input is small; this write does not hold up a tick.
                let _ = self.pty.writer.write_all(&bytes);
                let _ = self.pty.writer.flush();
            }
            TermMsg::Resize { cols, rows } => self.resize(cols, rows),
            TermMsg::Capture { scrollback, reply } => {
                let snapshot = self.snapshot();
                let _ = reply.send(snapshot.capture(scrollback));
            }
            TermMsg::Scrollback { from, to, reply } => {
                // Absolute ids in, absolute ids out. Anything the server no longer holds is
                // simply absent, and `first_row` says so.
                let held = self.scrollback.len() as u64;
                let window_start = self.rows_ever - held;
                let first = from.max(window_start);
                let last = to.min(self.rows_ever);
                let rows: Vec<Row> = if first < last {
                    self.scrollback
                        .iter()
                        .skip((first - window_start) as usize)
                        .take((last - first) as usize)
                        .cloned()
                        .collect()
                } else {
                    Vec::new()
                };
                let _ = reply.send(ScrollbackResult {
                    rows,
                    first_row: first,
                    rows_ever: self.rows_ever,
                    held: held.min(u64::from(u32::MAX)) as u32,
                });
            }
            TermMsg::Attach {
                attachment,
                stream,
                mode,
                cols,
                rows,
                queue,
                notify,
                reply,
            } => {
                // Flush what is pending to the attachments that already exist, so the snapshot
                // this new one gets is exactly the state the delta stream continues from. This
                // is the atomicity the protocol needs: no output is lost or repeated across an
                // attach.
                self.publish();
                let snapshot = self.snapshot();
                self.attachments.insert(
                    attachment,
                    Attachment {
                        stream,
                        mode,
                        cols,
                        rows,
                        queue,
                        notify,
                        overflowed: false,
                        dropped: 0,
                        sent: 0,
                    },
                );
                let _ = reply.send(snapshot);
            }
            TermMsg::Detach { attachment } => {
                self.attachments.remove(&attachment);
            }
            TermMsg::Stats { reply } => {
                let stats = Stats {
                    pane: self.pane,
                    pty_bytes: self.pty_bytes,
                    ticks: self.ticks,
                    attachments: self.attachments.len() as u32,
                    overflows: self.overflows,
                    resyncs: self.resyncs,
                    blocked_nanos: self.blocked.as_nanos().min(u128::from(u64::MAX)) as u64,
                };
                let _ = reply.send(stats);
            }
            TermMsg::Attachments { reply } => {
                let sizes = self
                    .attachments
                    .iter()
                    .map(|(id, attachment)| Attached {
                        attachment: *id,
                        cols: attachment.cols,
                        rows: attachment.rows,
                        mode: attachment.mode.name(),
                        sent: attachment.sent,
                        dropped: attachment.dropped,
                        overflowed: attachment.overflowed,
                    })
                    .collect();
                let _ = reply.send(sizes);
            }
        }
        true
    }

    fn feed(&mut self, bytes: &[u8]) {
        self.pty_bytes += bytes.len() as u64;
        for slice in bytes.chunks(FEED_SLICE) {
            self.parser.advance(&mut self.term, slice);
            self.seq += 1;
            self.account_scrollback();
            // Candidate D1: the bytes go out as they arrive, which is where its latency
            // advantage comes from and why it cannot coalesce.
            self.send_bytes(slice);
        }
    }

    /// Move whatever scrolled off the screen into the server's own scrollback, exactly.
    fn account_scrollback(&mut self) {
        let history = self.term.grid().history_size();
        if history > self.history_seen {
            let fresh = history - self.history_seen;
            for back in (1..=fresh).rev() {
                let row = self.read_row(-(back as i32));
                self.pending_appended.push(row.clone());
                self.scrollback.push_back(row);
                self.rows_ever += 1;
                if self.scrollback.len() > SCROLLBACK_LIMIT {
                    self.scrollback.pop_front();
                    self.pending_dropped += 1;
                }
            }
            self.history_seen = history;
        }
        if history >= DRAIN_HISTORY_AT {
            // Everything here is already in `self.scrollback`, so dropping it loses nothing and
            // keeps the next accounting exact.
            self.term.grid_mut().clear_history();
            self.history_seen = 0;
        }
    }

    fn read_row(&self, line: i32) -> Row {
        let grid_row = &self.term.grid()[Line(line)];
        let mut runs: Vec<Run> = Vec::new();
        for col in 0..self.size.cols {
            let cell: &Cell = &grid_row[Column(col)];
            if cell
                .flags
                .intersects(Flags::WIDE_CHAR_SPACER | Flags::LEADING_WIDE_CHAR_SPACER)
            {
                continue;
            }
            let fg = resolve(cell.fg, DEFAULT_FG, DEFAULT_BG);
            let bg = resolve(cell.bg, DEFAULT_FG, DEFAULT_BG);
            let flags = pack_flags(cell.flags);
            match runs.last_mut() {
                Some(run) if run.fg == fg && run.bg == bg && run.flags == flags => {
                    run.text.push(cell.c);
                }
                _ => runs.push(Run {
                    text: cell.c.to_string(),
                    fg,
                    bg,
                    flags,
                }),
            }
        }
        Row { runs }
    }

    fn screen(&self) -> Vec<Row> {
        (0..self.size.rows)
            .map(|line| self.read_row(line as i32))
            .collect()
    }

    fn cursor(&self) -> Cursor {
        let point = self.term.grid().cursor.point;
        Cursor {
            col: point.column.0.min(usize::from(u16::MAX)) as u16,
            row: point.line.0.max(0).min(i32::from(u16::MAX)) as u16,
            visible: self.term.mode().contains(TermMode::SHOW_CURSOR),
        }
    }

    fn snapshot(&self) -> Snapshot {
        Snapshot {
            seq: self.seq,
            cols: self.size.cols as u16,
            rows: self.size.rows as u16,
            screen: self.screen(),
            scrollback: self.scrollback.iter().cloned().collect(),
            rows_ever: self.rows_ever,
            cursor: self.cursor(),
            alt_screen: self.term.mode().contains(TermMode::ALT_SCREEN),
        }
    }

    fn resize(&mut self, cols: u16, rows: u16) {
        self.size = Size {
            cols: usize::from(cols),
            rows: usize::from(rows),
        };
        self.term.resize(self.size);
        self.pty.resize(cols, rows);
        self.prev_screen.clear();
        self.force_resync = Some(format!("resized to {cols}x{rows}"));
    }

    /// One tick: tell every attachment what changed, in the form its mode asked for.
    fn publish(&mut self) {
        self.ticks += 1;
        if let Some(reason) = self.force_resync.take() {
            let snapshot = self.snapshot();
            let ids: Vec<u32> = self.attachments.keys().copied().collect();
            for id in ids {
                self.send_resync(id, &snapshot, &reason);
            }
            self.prev_screen = snapshot.screen;
            self.prev_cursor = snapshot.cursor;
            self.pending_appended.clear();
            self.pending_dropped = 0;
            return;
        }

        // An attachment that fell behind gets one fresh snapshot and nothing else, which is the
        // coalescing D2 gets for free and D1 cannot have.
        let behind: Vec<u32> = self
            .attachments
            .iter()
            .filter(|(_, attachment)| attachment.overflowed)
            .map(|(id, _)| *id)
            .collect();
        if !behind.is_empty() {
            let snapshot = self.snapshot();
            for id in behind {
                self.send_resync(id, &snapshot, "the attachment fell behind");
            }
        }

        let screen = self.screen();
        let cursor = self.cursor();
        let changes = diff_screen(&self.prev_screen, &screen);
        let appended = std::mem::take(&mut self.pending_appended);
        let dropped = std::mem::replace(&mut self.pending_dropped, 0);
        let quiet =
            changes.is_empty() && appended.is_empty() && dropped == 0 && cursor == self.prev_cursor;
        self.prev_screen = screen;
        self.prev_cursor = cursor;
        if quiet {
            return;
        }

        let alt_screen = self.term.mode().contains(TermMode::ALT_SCREEN);
        let with_scrollback = Delta {
            seq: self.seq,
            rows: changes.clone(),
            scrollback_appended: appended,
            scrollback_dropped: dropped,
            rows_ever: self.rows_ever,
            cursor,
            alt_screen,
        };
        let screen_only = Delta {
            seq: self.seq,
            rows: changes,
            scrollback_appended: Vec::new(),
            scrollback_dropped: 0,
            rows_ever: self.rows_ever,
            cursor,
            alt_screen,
        };
        for (mode, delta) in [
            (DataMode::Deltas, with_scrollback),
            (DataMode::Screen, screen_only),
        ] {
            let targets: Vec<(u32, u32)> = self
                .attachments
                .iter()
                .filter(|(_, attachment)| attachment.mode == mode && !attachment.overflowed)
                .map(|(id, attachment)| (*id, attachment.stream))
                .collect();
            if targets.is_empty() {
                continue;
            }
            let Ok(payload) = serde_json::to_vec(&Update::Delta(delta)) else {
                continue;
            };
            for (id, stream) in targets {
                self.send(id, Frame::control(stream, payload.clone()));
            }
        }
    }

    fn send_bytes(&mut self, slice: &[u8]) {
        let targets: Vec<(u32, u32)> = self
            .attachments
            .iter()
            .filter(|(_, attachment)| attachment.mode == DataMode::Bytes && !attachment.overflowed)
            .map(|(id, attachment)| (*id, attachment.stream))
            .collect();
        for (id, stream) in targets {
            self.send(id, Frame::data(stream, slice.to_vec()));
        }
    }

    fn send_resync(&mut self, id: u32, snapshot: &Snapshot, reason: &str) {
        // A resync for a `screen` attachment carries no scrollback: that client never tracks it
        // and asks for a page when it wants one. Measured, this is the difference between a
        // 1.3 MB resync and a 3.5 KB one, and a client that falls behind gets a lot of them.
        let screen_only = self
            .attachments
            .get(&id)
            .is_some_and(|attachment| attachment.mode == DataMode::Screen);
        let mut carried = snapshot.clone();
        if screen_only {
            carried.scrollback.clear();
        }
        let update = Update::Resync {
            snapshot: Box::new(carried),
            reason: reason.to_owned(),
        };
        let Ok(payload) = serde_json::to_vec(&update) else {
            return;
        };
        let Some(attachment) = self.attachments.get(&id) else {
            return;
        };
        let frame = Frame::control(attachment.stream, payload);
        match attachment.queue.try_send(frame) {
            Ok(()) => {
                attachment.notify.notify_one();
                self.resyncs += 1;
                if let Some(attachment) = self.attachments.get_mut(&id) {
                    attachment.overflowed = false;
                }
            }
            Err(mpsc::error::TrySendError::Full(_)) => {}
            Err(mpsc::error::TrySendError::Closed(_)) => {
                self.attachments.remove(&id);
            }
        }
    }

    fn send(&mut self, id: u32, frame: Frame) {
        let Some(attachment) = self.attachments.get_mut(&id) else {
            return;
        };
        match self.overflow {
            Overflow::Resync => match attachment.queue.try_send(frame) {
                Ok(()) => {
                    attachment.sent += 1;
                    attachment.notify.notify_one();
                }
                Err(mpsc::error::TrySendError::Full(_)) => {
                    attachment.overflowed = true;
                    attachment.dropped += 1;
                    self.overflows += 1;
                }
                Err(mpsc::error::TrySendError::Closed(_)) => {
                    self.attachments.remove(&id);
                }
            },
            Overflow::Block => {
                let started = Instant::now();
                let sent = attachment.queue.blocking_send(frame);
                self.blocked += started.elapsed();
                match sent {
                    Ok(()) => {
                        attachment.sent += 1;
                        attachment.notify.notify_one();
                    }
                    Err(_) => {
                        self.attachments.remove(&id);
                    }
                }
            }
        }
    }
}

fn pack_flags(flags: Flags) -> u8 {
    let mut packed = 0;
    if flags.contains(Flags::BOLD) {
        packed |= FLAG_BOLD;
    }
    if flags.contains(Flags::ITALIC) {
        packed |= FLAG_ITALIC;
    }
    if flags.intersects(Flags::ALL_UNDERLINES) {
        packed |= FLAG_UNDERLINE;
    }
    if flags.contains(Flags::INVERSE) {
        packed |= FLAG_INVERSE;
    }
    if flags.contains(Flags::DIM) {
        packed |= FLAG_DIM;
    }
    packed
}

/// The palette resolution the server does so a client needs none. A real server would also apply
/// the program's own OSC 4 overrides; this is the instrument's simplification, noted in the
/// document rather than hidden.
fn resolve(color: Color, fg: u32, bg: u32) -> u32 {
    match color {
        Color::Spec(Rgb { r, g, b }) => pack(r, g, b),
        Color::Indexed(index) => indexed(index),
        Color::Named(named) => match named {
            NamedColor::Foreground | NamedColor::BrightForeground | NamedColor::DimForeground => fg,
            NamedColor::Background => bg,
            NamedColor::Cursor => fg,
            other => indexed((other as u8) & 0x0f),
        },
    }
}

const fn pack(r: u8, g: u8, b: u8) -> u32 {
    ((r as u32) << 16) | ((g as u32) << 8) | (b as u32)
}

fn indexed(index: u8) -> u32 {
    const ANSI: [u32; 16] = [
        0x0000_0000,
        0x00cc_0403,
        0x0019_cb00,
        0x00ce_cb00,
        0x000d_73cc,
        0x00cb_1ed1,
        0x000d_cdcd,
        0x00dd_dddd,
        0x006f_6f6f,
        0x00ff_0000,
        0x0000_ff00,
        0x00ff_ff00,
        0x000c_7eff,
        0x00ff_00ff,
        0x0000_ffff,
        0x00ff_ffff,
    ];
    match index {
        0..=15 => ANSI[usize::from(index)],
        16..=231 => {
            let step = |value: u8| -> u8 { if value == 0 { 0 } else { 55 + value * 40 } };
            let cube = index - 16;
            pack(step(cube / 36), step((cube % 36) / 6), step(cube % 6))
        }
        232..=255 => {
            let grey = 8 + (index - 232) * 10;
            pack(grey, grey, grey)
        }
    }
}
