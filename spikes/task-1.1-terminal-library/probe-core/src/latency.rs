//! Typing latency, the first half of R19.
//!
//! One keystroke at a time: the probe writes a byte to the pseudo-terminal, the tty's own echo
//! brings it back, the pane feeds it to the emulator and asks for a repaint, and the probe stops
//! the clock twice.
//!
//! - **to paint**: the byte has been drawn into the frame gpui is building.
//! - **to frame**: gpui's next-frame callback has run. On macOS that callback is driven by the
//!   display link, so this includes the wait for the display's next refresh. At 120 Hz that wait
//!   is up to 8.3 ms on its own, which is most of R19's 16 ms.
//!
//! What neither number includes: the path from a physical key to gpui's dispatch. Posting real
//! key events with CGEvent would measure the window server more than it would measure steg, and
//! needs accessibility permission, so it was not done.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use gpui::{AsyncWindowContext, WeakEntity, Window};

use crate::backend::TerminalBackend;
use crate::element::{POLL, TerminalView};

/// The characters the probe types, one per sample, cycling.
const MARKERS: &[char] = &[
    'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i', 'j', 'k', 'l', 'm', 'n', 'o', 'p', 'q', 'r', 's',
    't', 'u', 'v', 'w', 'x', 'y', 'z',
];

struct Pending {
    /// Where the cursor was before the key was sent. The echoed byte advances it, and nothing
    /// else moves it while `cat` is the program, so a changed position means the byte has been
    /// parsed and is on the screen. Counting characters instead would miss a sample whenever a
    /// scroll dropped an older copy of the same letter.
    cursor_before: (u16, u16),
    sent_at: Instant,
    arrived: bool,
}

/// The clock, shared between the driver, the pane's pump and the paint callback. Everything here
/// is on the main thread; the `Rc<RefCell<_>>` is because gpui's paint callback is `'static` and
/// cannot borrow the view.
#[derive(Default)]
pub struct Probe {
    pending: Option<Pending>,
    pub to_paint: Vec<Duration>,
    pub to_frame: Vec<Duration>,
    pub frame_intervals: Vec<Duration>,
    pub dropped: usize,
    last_frame: Option<Instant>,
}

/// One block of a long run, so drift over time is visible instead of being averaged away by a
/// single percentile over everything.
pub struct Block {
    pub first_sample: usize,
    pub to_paint: Vec<Duration>,
    pub to_frame: Vec<Duration>,
    pub dropped: usize,
    pub wall: Duration,
}

impl Probe {
    pub fn shared() -> Rc<RefCell<Self>> {
        Rc::new(RefCell::new(Self::default()))
    }

    /// Where the cursor was when the probe sent its key, if it is waiting for the echo.
    pub fn waiting_for(&self) -> Option<(u16, u16)> {
        self.pending
            .as_ref()
            .and_then(|pending| (!pending.arrived).then_some(pending.cursor_before))
    }

    /// Start the clock for one keystroke.
    fn start(&mut self, cursor_before: (u16, u16)) {
        self.pending = Some(Pending {
            cursor_before,
            sent_at: Instant::now(),
            arrived: false,
        });
    }

    /// Called by the pane after feeding output, with where the cursor is now.
    pub fn note_arrival(&mut self, cursor_now: (u16, u16)) {
        if let Some(pending) = &mut self.pending {
            if !pending.arrived && cursor_now != pending.cursor_before {
                pending.arrived = true;
            }
        }
    }

    /// Called at the end of every paint.
    pub fn painted(probe: &Rc<RefCell<Self>>, window: &mut Window) {
        let now = Instant::now();
        let mut state = probe.borrow_mut();
        if let Some(previous) = state.last_frame.replace(now) {
            state.frame_intervals.push(now - previous);
        }
        let Some(pending) = &state.pending else {
            return;
        };
        if !pending.arrived {
            return;
        }
        let sent_at = pending.sent_at;
        state.to_paint.push(now - sent_at);
        state.pending = None;
        drop(state);

        let probe = Rc::clone(probe);
        window.on_next_frame(move |_window, _cx| {
            probe.borrow_mut().to_frame.push(sent_at.elapsed());
        });
    }

    /// Whether the keystroke in flight has been drawn.
    fn settled(&self) -> bool {
        self.pending.is_none()
    }

    fn give_up(&mut self) {
        if self.pending.take().is_some() {
            self.dropped += 1;
        }
    }
}

pub struct Plan {
    pub samples: usize,
    /// How many samples go in one reported block.
    pub block: usize,
    /// How many keystrokes before the probe presses Return.
    ///
    /// The pseudo-terminal is in canonical mode, so the line discipline holds a line until a
    /// newline arrives and stops echoing once its buffer is full - 1024 bytes on macOS. Without
    /// this, every sample after the thousandth is dropped and the run looks like a stall.
    pub line_every: usize,
    /// How often a key is pressed. About twelve a second is fast human typing, and leaves every
    /// sample well clear of the one before it.
    pub interval: Duration,
    /// How long one keystroke may take before it is counted as dropped rather than measured.
    pub deadline: Duration,
}

impl Default for Plan {
    fn default() -> Self {
        Self {
            samples: 2000,
            block: 250,
            line_every: 40,
            interval: Duration::from_millis(80),
            deadline: Duration::from_millis(500),
        }
    }
}

/// Type into a pane, sample by sample, until the plan is done.
///
/// # Errors
/// If the pane goes away while the plan is still running.
pub async fn drive<B: TerminalBackend + 'static>(
    pane: WeakEntity<TerminalView<B>>,
    cx: &mut AsyncWindowContext,
    plan: &Plan,
) -> anyhow::Result<Vec<Block>> {
    let mut blocks: Vec<Block> = Vec::new();
    let mut block_started = Instant::now();
    let mut taken = 0_usize;
    for index in 0..plan.samples {
        cx.background_executor().timer(plan.interval).await;
        let marker = MARKERS[index % MARKERS.len()];
        pane.update_in(cx, |view, _window, _cx| {
            let screen = view.screen();
            view.probe()
                .borrow_mut()
                .start((screen.cursor_col, screen.cursor_row));
            let mut byte = [0_u8; 4];
            view.send(marker.encode_utf8(&mut byte).as_bytes());
        })?;

        let started = Instant::now();
        loop {
            cx.background_executor().timer(POLL).await;
            let settled = pane.update_in(cx, |view, _window, _cx| view.probe().borrow().settled())?;
            if settled {
                break;
            }
            if started.elapsed() > plan.deadline {
                pane.update_in(cx, |view, _window, _cx| view.probe().borrow_mut().give_up())?;
                break;
            }
        }

        if (index + 1) % plan.line_every == 0 {
            // Press Return so the line discipline flushes its buffer. Not a measured sample.
            pane.update_in(cx, |view, _window, _cx| view.send(b"\r"))?;
            cx.background_executor()
                .timer(Duration::from_millis(40))
                .await;
        }

        if (index + 1) % plan.block == 0 || index + 1 == plan.samples {
            let block = pane.update_in(cx, |view, _window, _cx| {
                let probe = view.probe();
                let state = probe.borrow();
                Block {
                    first_sample: taken,
                    to_paint: state.to_paint[taken.min(state.to_paint.len())..].to_vec(),
                    to_frame: state.to_frame[taken.min(state.to_frame.len())..].to_vec(),
                    dropped: state.dropped,
                    wall: block_started.elapsed(),
                }
            })?;
            taken = pane.update_in(cx, |view, _window, _cx| {
                let probe = view.probe();
                probe.borrow().to_paint.len()
            })?;
            blocks.push(block);
            block_started = Instant::now();
        }
    }
    Ok(blocks)
}
