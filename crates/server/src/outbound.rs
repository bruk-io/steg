//! One connection's outbound writer and its queues (TASK-1.45 §2; decision-2
//! §3/§6).
//!
//! Server code, not sdk code, on purpose: doc-16 R102 says the sdk does no
//! I/O, and R1/`lib.rs`'s own module doc say the server is where I/O meets
//! the kernel. Everything here schedules *when* bytes the sdk's own
//! `Connection::reply`/`emit_event`/`send_data` already encoded hit a real
//! socket - nothing here parses or builds a frame, so this is not a second
//! definition of the protocol, only the server's own scheduling over a
//! socket it owns.
//!
//! `Outbound` stays `pub(crate)`: the one thing this module's own tests need
//! to drive a real flood against a real socket is already visible to a
//! `#[cfg(test)]` module in the same crate (`testing-rust.md`'s own
//! convention: "unit tests prove kernel logic and sit beside it... with no
//! doubles"), so nothing here needs to be `pub` for that - widening the
//! crate's own public surface only to serve a test is exactly what TASK-1.44's
//! review twice flagged (the rejected `STEG_TEST_REPORT_OUTCOME_TO`, the
//! always-built fixture binary).

use std::collections::{HashMap, HashSet, VecDeque};
use std::io::Write;
use std::sync::{Condvar, Mutex};
use std::thread::{self, JoinHandle};

/// decision-2 §3's own control-queue bound is unstated (control traffic was
/// never the decisive question there); 256 is this task's own generous
/// guess for a well-behaved client pipelining many quick requests - see
/// [`EnqueueControlError::Full`] for what happens past it. Named so a later
/// measurement (TASK-1.47, with real traffic) can revise it without
/// surprise.
const CONTROL_CAP: usize = 256;

/// decision-2 §6's own measured number for a data-mode attachment's queue:
/// a bounded queue per attachment, so a slow client's backlog costs a
/// fresh, screen-sized resync (TASK-1.47's own tick, `crate::pane::
/// run_tick`), never an unbounded wait or a growing queue.
const DATA_CAP: usize = 64;

/// Why [`Outbound::enqueue_control`] refused.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum EnqueueControlError {
    /// The queue is already at [`CONTROL_CAP`]: a client that keeps sending
    /// requests while never reading any reply (TASK-1.45 §3) - the caller's
    /// own job is to close the connection on this, not grow the queue or
    /// block the reader on a write the client will never read.
    Full,
    /// [`Outbound::close`] has already been called.
    Closed,
}

/// Why [`Outbound::enqueue_data`] refused. [`crate::pane::run_tick`] (the
/// tick TASK-1.47 adds) treats [`EnqueueDataError::Full`] and
/// [`EnqueueDataError::NotOpen`] identically: mark the attachment as
/// needing a fresh resync and try again next tick, never block.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum EnqueueDataError {
    /// [`Outbound::open_stream`] was never called for this stream id (or
    /// [`Outbound::close_stream`] already closed it) - reachable today
    /// when a tick runs for an attachment the worker knows about but this
    /// `Outbound` has not yet been told to open (TASK-1.47's own ordering
    /// rule: an attachment is handed its `Outbound` only after the writer
    /// has been told its stream exists, `crate::pane`'s own `PaneCommand::
    /// Arm`), or after it has already been closed.
    NotOpen,
    /// The stream's own queue is already at [`DATA_CAP`]: decision-2 §6's
    /// real drop-and-resync policy - the frame is dropped, not queued, and
    /// not blocked on; a fresh, screen-sized snapshot replaces it on the
    /// very next tick.
    Full,
    /// [`Outbound::close`] has already been called.
    Closed,
}

/// One queued control frame, and - only for [`Outbound::
/// enqueue_control_and_wait_until_written`]'s own callers (TASK-1.88) - a
/// one-shot sender that fires strictly after this exact frame's bytes
/// have left [`run_writer`]'s own `write_all`, never before and never for
/// any other frame. `None` for every ordinary [`Outbound::enqueue_control`]
/// call - nothing waits on those, so there is nothing to send.
struct ControlItem {
    bytes: Vec<u8>,
    sent: Option<std::sync::mpsc::Sender<()>>,
}

/// The state behind one [`Outbound`]'s lock: the control queue, every open
/// stream's own data queue, the round-robin order streams are drained in,
/// and whether this connection is closing.
struct Queues {
    control: VecDeque<ControlItem>,
    data: HashMap<u32, VecDeque<Vec<u8>>>,
    /// Open stream ids, in the order [`Outbound::open_stream`] was called -
    /// decision-2 §3's own "takes one frame from each attachment in turn".
    order: Vec<u32>,
    /// Where the round robin resumes next time control is empty - an index
    /// into `order`, not a stream id, so it stays valid as streams open and
    /// close (clamped back on close, below).
    cursor: usize,
    closed: bool,
    /// Streams [`Outbound::end_stream`] has queued a final frame for and
    /// marked for teardown (TASK-1.128; decision-2 §3's own addendum:
    /// "queue the last frame before tearing the stream down, never the
    /// reverse") - [`Queues::pop_next`] removes a stream from here, and
    /// from `data`/`order`, the instant it pops that stream's own queue
    /// empty, never before: [`Outbound::close_stream`] drops whatever is
    /// still queued the moment it is called, which would silently drop
    /// the very frame `end_stream` just queued if the two ran back to
    /// back with no synchronisation.
    closing: HashSet<u32>,
}

impl Queues {
    fn new() -> Self {
        Self {
            control: VecDeque::new(),
            data: HashMap::new(),
            order: Vec::new(),
            cursor: 0,
            closed: false,
            closing: HashSet::new(),
        }
    }

    /// One queued item, control first (fully drained before any data is
    /// even considered - decision-2 §3), then one frame from the next data
    /// stream in round-robin order that actually has one queued (a stream
    /// with nothing queued is skipped, not given a turn that wastes it).
    /// `None` means nothing is queued right now, for any stream - the
    /// caller decides what that means (wait, or stop, if also `closed`).
    ///
    /// A stream marked `closing` (TASK-1.128) is torn down right here,
    /// under the same lock, the instant the frame just popped from it
    /// leaves its own queue empty - never before (a stream with more
    /// still queued behind its own final frame keeps being served
    /// normally, by every rule above, until that frame too is the one
    /// popped).
    ///
    /// Returns the popped item's own ack sender alongside its bytes
    /// (TASK-1.88) - `None` for every data-stream frame (never carries
    /// one) and for an ordinary [`Outbound::enqueue_control`] item (also
    /// `None`); `Some` only for the one kind
    /// [`Outbound::enqueue_control_and_wait_until_written`] builds.
    fn pop_next(&mut self) -> Option<(Vec<u8>, Option<std::sync::mpsc::Sender<()>>)> {
        if let Some(item) = self.control.pop_front() {
            return Some((item.bytes, item.sent));
        }
        let streams = self.order.len();
        for _ in 0..streams {
            let stream = self.order[self.cursor];
            self.cursor = (self.cursor + 1) % streams;
            if let Some(queue) = self.data.get_mut(&stream)
                && let Some(bytes) = queue.pop_front()
            {
                if queue.is_empty() && self.closing.contains(&stream) {
                    self.remove_stream(stream);
                }
                return Some((bytes, None));
            }
        }
        None
    }

    /// Unregisters `stream` from every bookkeeping structure that names
    /// it (`data`, `order`, `closing`), keeping `cursor` valid by
    /// construction - the shared guts of [`Outbound::close_stream`] (an
    /// immediate, unconditional teardown) and [`Queues::pop_next`]'s own
    /// drained-and-closing teardown (deferred until the queue is actually
    /// empty), so the two can never drift apart into two different ideas
    /// of what "remove a stream" means.
    fn remove_stream(&mut self, stream: u32) {
        self.data.remove(&stream);
        self.closing.remove(&stream);
        if let Some(index) = self.order.iter().position(|&id| id == stream) {
            self.order.remove(index);
        }
        if !self.order.is_empty() {
            self.cursor %= self.order.len();
        } else {
            self.cursor = 0;
        }
    }
}

/// One connection's outbound side: the control queue and every open
/// attachment's own data queue, shared between whichever thread enqueues
/// (the reader, for control; TASK-1.47's attach code, for data) and the one
/// writer thread that drains them onto the real socket (`spawn_writer`,
/// below).
pub(crate) struct Outbound {
    queues: Mutex<Queues>,
    ready: Condvar,
}

impl Outbound {
    pub(crate) fn new() -> Self {
        Self {
            queues: Mutex::new(Queues::new()),
            ready: Condvar::new(),
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Queues> {
        self.queues
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Queues a control-kind frame's already-encoded bytes (a reply, or in
    /// a later task an event) - drained by the writer thread ahead of any
    /// data, every time (decision-2 §3).
    pub(crate) fn enqueue_control(&self, bytes: Vec<u8>) -> Result<(), EnqueueControlError> {
        let mut queues = self.lock();
        if queues.closed {
            return Err(EnqueueControlError::Closed);
        }
        if queues.control.len() >= CONTROL_CAP {
            return Err(EnqueueControlError::Full);
        }
        queues.control.push_back(ControlItem { bytes, sent: None });
        self.ready.notify_one();
        Ok(())
    }

    /// Queues a control-kind frame exactly like [`Outbound::
    /// enqueue_control`], then blocks the *calling* thread until
    /// [`run_writer`] has actually called `write_all` for these exact
    /// bytes and it returned, or until `bound` passes, or until it
    /// becomes clear it never will (the queue was already closed, or the
    /// writer ends before reaching this item - a broken socket) -
    /// whichever comes first (TASK-1.88, `server.stop`'s own AC #2: "the
    /// reply must be written and flushed to the caller before its
    /// connection is closed", given that replies are written by this
    /// connection's own writer thread, asynchronously, not by whichever
    /// thread calls this).
    ///
    /// Confirmed directly, not assumed: once `write_all` has returned
    /// `Ok` on one end of a real `UnixStream` pair, a `shutdown(Both)` on
    /// a `try_clone()`'d duplicate of that same end - exactly the shape
    /// `connection.rs`'s own `shutdown_handle` already is - does not
    /// discard those bytes before the peer reads them (a throwaway
    /// 2,000-trial probe against a real socket pair, 0 losses). That is
    /// what makes "wait for the write to return, then act" sufficient: a
    /// caller does not need the *client* to have read anything, only
    /// this connection's own writer thread to have handed the bytes to
    /// the kernel.
    ///
    /// Returns `Ok(())` in every outcome but a failed enqueue
    /// ([`EnqueueControlError::Full`]/[`EnqueueControlError::Closed`],
    /// surfaced exactly as `enqueue_control` already does) - whether the
    /// wait actually saw the write happen, timed out, or found the
    /// channel disconnected, there is nothing further for a caller to do
    /// in any of those three cases: either the bytes are already written,
    /// or the connection is already gone and there is nothing left to
    /// lose by proceeding.
    pub(crate) fn enqueue_control_and_wait_until_written(
        &self,
        bytes: Vec<u8>,
        bound: std::time::Duration,
    ) -> Result<(), EnqueueControlError> {
        let (sent_tx, sent_rx) = std::sync::mpsc::channel();
        {
            let mut queues = self.lock();
            if queues.closed {
                return Err(EnqueueControlError::Closed);
            }
            if queues.control.len() >= CONTROL_CAP {
                return Err(EnqueueControlError::Full);
            }
            queues.control.push_back(ControlItem {
                bytes,
                sent: Some(sent_tx),
            });
            self.ready.notify_one();
        }
        let _ = sent_rx.recv_timeout(bound);
        Ok(())
    }

    /// Registers a stream so [`Outbound::enqueue_data`] accepts frames for
    /// it and the writer's round robin gives it a turn. Idempotent: opening
    /// an already-open stream changes nothing. Called by `connection.rs`'s
    /// own `pane.attach` handling (TASK-1.47), after the attach reply is
    /// already queued (decision-2 section 3's ordering rule).
    pub(crate) fn open_stream(&self, stream: u32) {
        let mut queues = self.lock();
        if let std::collections::hash_map::Entry::Vacant(entry) = queues.data.entry(stream) {
            entry.insert(VecDeque::new());
            queues.order.push(stream);
        }
    }

    /// Unregisters a stream *right now*: no more frames are accepted for
    /// it, and whatever was still queued for it is dropped unread -
    /// called by `connection.rs`'s own `pane.detach` handling (TASK-1.47)
    /// and by `finish_attach`'s `armed == false` branch (TASK-1.125),
    /// neither of which ever has a frame still worth delivering on the
    /// stream at the point it calls this. **Never call this right after
    /// `enqueue_data`/`end_stream` on the same stream** - it would race
    /// the writer thread for whether the frame just queued is ever
    /// actually written, and this method does not wait to find out; it
    /// drops whatever is there, synchronously, under its own lock, every
    /// time. [`Outbound::end_stream`] is the primitive for "deliver this
    /// one last frame, then tear the stream down."
    pub(crate) fn close_stream(&self, stream: u32) {
        self.lock().remove_stream(stream);
    }

    /// Queues opaque bytes on an already-open data stream - called once per
    /// armed attachment, per tick, by `crate::pane::run_tick` (TASK-1.47).
    pub(crate) fn enqueue_data(&self, stream: u32, bytes: Vec<u8>) -> Result<(), EnqueueDataError> {
        let mut queues = self.lock();
        if queues.closed {
            return Err(EnqueueDataError::Closed);
        }
        let Some(queue) = queues.data.get_mut(&stream) else {
            return Err(EnqueueDataError::NotOpen);
        };
        if queue.len() >= DATA_CAP {
            return Err(EnqueueDataError::Full);
        }
        queue.push_back(bytes);
        self.ready.notify_one();
        Ok(())
    }

    /// Queues `bytes` as a stream's own last frame, then marks it for
    /// teardown once drained (TASK-1.128; decision-2 §3's own addendum:
    /// "queue the last frame before tearing the stream down, never the
    /// reverse") - the fix for a real bug [`Outbound::close_stream`]'s
    /// own doc now names: calling that right after `enqueue_data` races
    /// the writer thread for whether the just-queued frame is ever
    /// actually written, since `close_stream` drops whatever is queued
    /// the instant it runs. Here, [`Queues::pop_next`] is what removes
    /// the stream - only once it has actually popped this frame off the
    /// stream's own, now-empty queue, never before, so the frame is
    /// guaranteed to reach the writer's own `write_all` (module doc) even
    /// under contention.
    ///
    /// Never refuses for being full, past [`DATA_CAP`], the way `enqueue_
    /// data` does: if the queue is already at its cap, the single oldest
    /// frame in it is dropped first, to make room for exactly this one -
    /// decision-2 §6 already treats a backlogged queue's own content as
    /// disposable catch-up data, and unlike an ordinary screen update,
    /// this frame has no "try again next tick" to fall back on if it were
    /// dropped instead. Only ever the oldest one frame, never the whole
    /// queue: a content update this same flush enqueued through `enqueue_
    /// data` moments before calling this (`crate::pane::flush_ended`'s own
    /// shape - AC #1's "after any screen update still owed") is the
    /// *newest* thing queued, nowhere near `DATA_CAP` frames behind, and
    /// must still reach the client, in order, ahead of `Ended` itself.
    ///
    /// # Errors
    /// [`EnqueueDataError::NotOpen`]/[`EnqueueDataError::Closed`], the
    /// same as `enqueue_data` - never [`EnqueueDataError::Full`], since
    /// this makes room rather than refusing past the cap.
    pub(crate) fn end_stream(&self, stream: u32, bytes: Vec<u8>) -> Result<(), EnqueueDataError> {
        let mut queues = self.lock();
        if queues.closed {
            return Err(EnqueueDataError::Closed);
        }
        let Some(queue) = queues.data.get_mut(&stream) else {
            return Err(EnqueueDataError::NotOpen);
        };
        if queue.len() >= DATA_CAP {
            queue.pop_front();
        }
        queue.push_back(bytes);
        queues.closing.insert(stream);
        self.ready.notify_one();
        Ok(())
    }

    /// Tells the writer thread to stop once it has drained whatever is
    /// already queued (not before: a reply enqueued just before this is
    /// called still gets written, as long as the socket itself is still
    /// good) - idempotent and safe to call from any thread, any number of
    /// times. Every `enqueue_*` call refuses after this.
    pub(crate) fn close(&self) {
        self.lock().closed = true;
        self.ready.notify_all();
    }
}

/// The writer thread body: drains `outbound`'s queues onto `sink` in
/// priority order (control first, then one data frame per open stream in
/// turn), blocking on nothing but the condvar between items, until
/// [`Outbound::close`] has been called *and* nothing is left queued, or a
/// write to `sink` fails (the connection's socket is gone - nothing further
/// to do). Never holds `outbound`'s lock while writing (`Queues::pop_next`
/// returns an owned `Vec<u8>`, released before `write_all` runs), so a
/// producer enqueueing more work is never blocked on this thread's own I/O.
pub(crate) fn run_writer<W: Write>(outbound: &Outbound, mut sink: W) {
    loop {
        let item = {
            let mut queues = outbound.lock();
            loop {
                if let Some(popped) = queues.pop_next() {
                    break Some(popped);
                }
                if queues.closed {
                    break None;
                }
                queues = outbound
                    .ready
                    .wait(queues)
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
            }
        };
        let Some((bytes, ack)) = item else { return };
        if sink.write_all(&bytes).is_err() {
            // The socket itself is broken (not the "client never reads"
            // case `Outbound::close` already covers from the reader's
            // own side) - a review of this task found that, left
            // unmarked, the reader could keep computing and queueing
            // replies nobody will ever send, up to `CONTROL_CAP`, before
            // anything notices. Closing here means the reader's own next
            // `enqueue_control` fails and the connection ends promptly
            // instead. `ack` (if any) is simply dropped here, unsent -
            // its own receiver sees the channel disconnect and returns,
            // the same outcome `enqueue_control_and_wait_until_written`'s
            // own doc already names as needing no special handling.
            outbound.close();
            return;
        }
        // Fires strictly after this exact frame's bytes left `write_all`
        // above - never before, never for any other frame (TASK-1.88).
        if let Some(ack) = ack {
            let _ = ack.send(());
        }
    }
}

/// Spawns [`run_writer`] on its own thread, over an owned `sink` - the
/// shape `connection::handle` uses in production (a `try_clone`d write half
/// of the connection's own `UnixStream`); this module's own tests call
/// [`run_writer`] directly instead, over a real `UnixStream::pair()`, so
/// they can join it without going through a `JoinHandle` wrapper first.
pub(crate) fn spawn_writer<W: Write + Send + 'static>(
    outbound: std::sync::Arc<Outbound>,
    sink: W,
) -> JoinHandle<()> {
    thread::Builder::new()
        .name("steg-connection-writer".to_string())
        .spawn(move || run_writer(&outbound, sink))
        .expect("spawning this connection's writer thread")
}

#[cfg(test)]
mod tests {
    use super::{DATA_CAP, EnqueueControlError, EnqueueDataError, Outbound, run_writer};
    use crate::test_support::{ClosingWriterGuard, run_bounded};
    use std::io::{self, Read, Write};
    use std::os::unix::net::UnixStream;
    use std::sync::Arc;
    use std::sync::Barrier;
    use std::sync::mpsc;
    use std::time::{Duration, Instant};

    /// Reads exactly `len` bytes off `stream`, looping reads - a
    /// `UnixStream` is not guaranteed to deliver everything in one `read()`
    /// call.
    fn read_exact_bytes(stream: &mut UnixStream, len: usize) -> Vec<u8> {
        let mut buf = vec![0_u8; len];
        stream.read_exact(&mut buf).expect("the writer's own bytes");
        buf
    }

    #[test]
    fn control_is_drained_before_any_data_even_when_data_was_enqueued_first() {
        let outbound = Arc::new(Outbound::new());
        outbound.open_stream(7);
        outbound.enqueue_data(7, b"data-first".to_vec()).unwrap();
        outbound
            .enqueue_control(b"control-second".to_vec())
            .unwrap();

        let (mut client, server) = UnixStream::pair().unwrap();
        let writer_outbound = Arc::clone(&outbound);
        let writer = std::thread::spawn(move || run_writer(&writer_outbound, server));
        // Built right after the spawn, before any assertion that could
        // panic: its `Drop` closes the queue and bound-joins the writer
        // on every exit from this function, not only the happy one
        // (`test_support.rs`'s own doc - hygiene, not a fix for any
        // particular hang).
        let _writer_guard = ClosingWriterGuard::new(Arc::clone(&outbound), writer);

        assert_eq!(
            read_exact_bytes(&mut client, b"control-second".len()),
            b"control-second"
        );
        assert_eq!(
            read_exact_bytes(&mut client, b"data-first".len()),
            b"data-first"
        );
    }

    #[test]
    fn data_streams_are_drained_round_robin_one_frame_at_a_time() {
        let outbound = Arc::new(Outbound::new());
        outbound.open_stream(1);
        outbound.open_stream(2);
        outbound.enqueue_data(1, b"1a".to_vec()).unwrap();
        outbound.enqueue_data(2, b"2a".to_vec()).unwrap();
        outbound.enqueue_data(1, b"1b".to_vec()).unwrap();
        outbound.enqueue_data(2, b"2b".to_vec()).unwrap();

        let (mut client, server) = UnixStream::pair().unwrap();
        let writer_outbound = Arc::clone(&outbound);
        let writer = std::thread::spawn(move || run_writer(&writer_outbound, server));
        let _writer_guard = ClosingWriterGuard::new(Arc::clone(&outbound), writer);

        // Round robin in the order the streams were opened: one frame from
        // stream 1, one from stream 2, then back to each one's own second
        // frame - never both of one stream's frames back to back while the
        // other stream has one waiting.
        assert_eq!(read_exact_bytes(&mut client, 2), b"1a");
        assert_eq!(read_exact_bytes(&mut client, 2), b"2a");
        assert_eq!(read_exact_bytes(&mut client, 2), b"1b");
        assert_eq!(read_exact_bytes(&mut client, 2), b"2b");
    }

    #[test]
    fn a_stream_with_nothing_queued_is_skipped_not_given_an_empty_turn() {
        let outbound = Arc::new(Outbound::new());
        outbound.open_stream(1); // never gets a frame
        outbound.open_stream(2);
        outbound.enqueue_data(2, b"only-this".to_vec()).unwrap();

        let (mut client, server) = UnixStream::pair().unwrap();
        let writer_outbound = Arc::clone(&outbound);
        let writer = std::thread::spawn(move || run_writer(&writer_outbound, server));
        let _writer_guard = ClosingWriterGuard::new(Arc::clone(&outbound), writer);

        assert_eq!(
            read_exact_bytes(&mut client, b"only-this".len()),
            b"only-this"
        );
    }

    #[test]
    fn closing_drains_what_is_already_queued_then_the_writer_returns() {
        let outbound = Outbound::new();
        outbound.enqueue_control(b"last-one".to_vec()).unwrap();
        outbound.close();

        let (mut client, server) = UnixStream::pair().unwrap();
        run_writer(&outbound, server); // returns on its own: no thread needed

        assert_eq!(
            read_exact_bytes(&mut client, b"last-one".len()),
            b"last-one"
        );
    }

    #[test]
    fn enqueue_refuses_once_closed() {
        let outbound = Outbound::new();
        outbound.open_stream(1);
        outbound.close();

        assert_eq!(
            outbound.enqueue_control(b"too-late".to_vec()),
            Err(EnqueueControlError::Closed)
        );
        assert_eq!(
            outbound.enqueue_data(1, b"too-late".to_vec()),
            Err(EnqueueDataError::Closed)
        );
    }

    /// A `Write` that always fails, standing in for a broken socket.
    ///
    /// An earlier version of this test used a real `UnixStream::pair()`
    /// with the client half dropped, reasoning that the very next write
    /// on the other half would fail at once. TASK-1.113's own hang
    /// (`run_writer` sitting forever in `Condvar::wait`, CI, 2026-11-16)
    /// traced to exactly that assumption being unreliable on macOS:
    /// `std`'s own `UnixStream::pair` cannot set `CLOEXEC` atomically
    /// there (`sys/net/connection/socket/unix.rs`'s own `Socket::
    /// new_pair` - the atomic `SOCK_CLOEXEC` path is Linux-like targets
    /// only; macOS calls plain `socketpair()` then two separate, later
    /// `fcntl` calls), so a `fork` from another thread of this same test
    /// binary landing in that gap - and this crate's own test suite
    /// forks real processes routinely, concurrently, by design
    /// (`signal.rs`, `login_env.rs`, `pty.rs`, `pane.rs`) - inherits the
    /// dropped client's own fd and keeps the "closed" socket's write
    /// from failing at all. Reproduced directly while finding this (a
    /// scratch probe, not committed): 0 failures in 800,000+ trials with
    /// nothing else forking, versus roughly 1 success in 4 once eight
    /// threads were forking real processes alongside the probe. A bound
    /// alone would only turn that hang into an intermittent failure
    /// under exactly the contention this suite creates for itself; this
    /// type removes the dependency on socket or fork timing entirely.
    struct AlwaysFailsToWrite;

    impl Write for AlwaysFailsToWrite {
        fn write(&mut self, _buf: &[u8]) -> io::Result<usize> {
            Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "AlwaysFailsToWrite always fails",
            ))
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    /// A review of this task found: if the writer thread ends because
    /// the write itself fails (a broken socket, not the "client never
    /// reads" case `Outbound::close` already covers from the reader's
    /// own side), nothing used to mark the queue closed or notify the
    /// reader - it could keep computing and queueing replies nobody
    /// will ever send, up to `CONTROL_CAP`, before anything noticed.
    ///
    /// **Why this stays a direct `Outbound`/`run_writer` test, with no
    /// routing-level counterpart over a real connection** (a second
    /// review of this task asked for one, or this reason): the obvious
    /// way to make a real connection's own write fail without the
    /// reader also seeing an immediate close - the client half-closing
    /// only its own read side - does not do that on this platform,
    /// confirmed directly rather than assumed (a standalone repro:
    /// `UnixStream::pair()`, one side `shutdown(Shutdown::Read)`, the
    /// other side writes 2MB with every single `write()` call
    /// succeeding - a half-closed read side here never makes the
    /// peer's own writes fail or block). A real connection's client
    /// fully disconnecting instead - the only reliable way left to make
    /// the write fail - *also* makes the reader's own, already-existing
    /// `Ok(0)`/error handling in `run_post_handshake`'s read loop close
    /// the connection at the same moment, independently of whether this
    /// fix exists at all (confirmed by reading `connection.rs`: that
    /// path needs nothing from `Outbound` to decide to close). A
    /// routing-level test built that way could not tell "the writer's
    /// own broken-write handling ended this" apart from "the reader's
    /// own, unrelated EOF handling already would have" - it would pass
    /// identically with this fix removed, which is exactly the "test
    /// that still passes with its mechanism removed proves nothing"
    /// case `review.md`'s own Evidence pass warns against. This test
    /// drives `run_writer` directly, with no reader and no connection
    /// at all in the way, specifically so it isolates the one thing the
    /// routing-level shape cannot.
    #[test]
    fn a_broken_write_closes_the_queue_so_the_readers_next_enqueue_fails() {
        let outbound = Arc::new(Outbound::new());
        outbound.enqueue_control(b"first".to_vec()).unwrap();

        // Bounded too (AC #2's own letter), in addition to the sink above
        // removing the dependency on the write actually failing: belt
        // and suspenders, not the fix itself.
        let writer_outbound = Arc::clone(&outbound);
        run_bounded(
            "run_writer to end once its one write fails",
            Duration::from_secs(5),
            move || run_writer(&writer_outbound, AlwaysFailsToWrite),
        );

        assert_eq!(
            outbound.enqueue_control(b"second".to_vec()),
            Err(EnqueueControlError::Closed),
            "the writer's own broken-write exit must close the queue, the same as an explicit \
             Outbound::close() would"
        );
    }

    #[test]
    fn enqueue_data_on_a_never_opened_stream_is_refused() {
        let outbound = Outbound::new();
        assert_eq!(
            outbound.enqueue_data(42, b"nope".to_vec()),
            Err(EnqueueDataError::NotOpen)
        );
    }

    #[test]
    fn enqueue_data_on_a_closed_stream_is_refused_the_same_way() {
        let outbound = Outbound::new();
        outbound.open_stream(1);
        outbound.close_stream(1);
        assert_eq!(
            outbound.enqueue_data(1, b"nope".to_vec()),
            Err(EnqueueDataError::NotOpen)
        );
    }

    /// decision-2 §6's own placeholder policy for this task (real
    /// drop-and-resync is TASK-1.47's): past the cap, a data frame is
    /// dropped - refused, not queued, and the stream itself stays open and
    /// usable for the next frame.
    #[test]
    fn a_full_data_queue_drops_the_new_frame_rather_than_growing_or_blocking() {
        let outbound = Outbound::new();
        outbound.open_stream(1);
        for i in 0..DATA_CAP {
            outbound
                .enqueue_data(1, format!("frame-{i}").into_bytes())
                .unwrap();
        }

        assert_eq!(
            outbound.enqueue_data(1, b"one-too-many".to_vec()),
            Err(EnqueueDataError::Full)
        );

        // The stream is still open and the queue still has exactly its cap
        // worth of frames - proven by draining every one of them and
        // seeing the dropped frame is not among them. Run on its own
        // thread, concurrently read: `write_all` could otherwise block on
        // a full OS socket buffer with nothing reading it yet.
        outbound.close();
        let (mut client, server) = UnixStream::pair().unwrap();
        let writer = std::thread::spawn(move || run_writer(&outbound, server));
        for i in 0..DATA_CAP {
            assert_eq!(
                read_exact_bytes(&mut client, format!("frame-{i}").len()),
                format!("frame-{i}").into_bytes()
            );
        }
        let mut leftover = [0_u8; 1];
        assert_eq!(
            client.read(&mut leftover).unwrap(),
            0,
            "nothing more was written: the one-too-many frame was really dropped"
        );
        writer.join().unwrap();
    }

    // --- `Outbound::end_stream` (TASK-1.128) -----------------------------

    /// Over a full queue, `end_stream` still delivers its own final frame -
    /// it makes room by dropping only the single oldest frame, never the
    /// whole queue (a content update enqueued moments before it, by the
    /// same flush, is the *newest* thing queued, nowhere near `DATA_CAP`
    /// frames behind, and must not be lost).
    #[test]
    fn end_stream_still_delivers_the_final_frame_when_the_queue_was_already_full() {
        let outbound = Outbound::new();
        outbound.open_stream(1);
        for i in 0..DATA_CAP {
            outbound
                .enqueue_data(1, format!("frame-{i}").into_bytes())
                .unwrap();
        }

        outbound.end_stream(1, b"the-end".to_vec()).unwrap();
        outbound.close();

        let (mut client, server) = UnixStream::pair().unwrap();
        let writer = std::thread::spawn(move || run_writer(&outbound, server));

        // The single oldest frame (frame-0) was dropped to make room -
        // every other already-queued frame is still delivered, in order,
        // ahead of the final one.
        for i in 1..DATA_CAP {
            assert_eq!(
                read_exact_bytes(&mut client, format!("frame-{i}").len()),
                format!("frame-{i}").into_bytes()
            );
        }
        assert_eq!(read_exact_bytes(&mut client, b"the-end".len()), b"the-end");

        let mut leftover = [0_u8; 1];
        assert_eq!(
            client.read(&mut leftover).unwrap(),
            0,
            "nothing arrives on the stream after its own final frame"
        );
        writer.join().unwrap();
    }

    /// A `Write` that forwards to a real socket, then blocks until the
    /// test explicitly says to proceed - used to pin down, deterministically
    /// rather than by timing, exactly when `run_writer` loops back to pop
    /// its own next frame (the point `end_stream`'s own teardown happens,
    /// inside `Queues::pop_next`, strictly before the popped frame's bytes
    /// are written at all).
    struct StepWriter {
        inner: UnixStream,
        proceed: mpsc::Receiver<()>,
    }

    impl Write for StepWriter {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.inner.write_all(buf)?;
            let _ = self.proceed.recv();
            Ok(buf.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            self.inner.flush()
        }
    }

    /// The real two-thread proof behind `end_stream`'s own doc ("the
    /// writer thread ... removes the stream - only once it has actually
    /// popped this frame off the stream's own, now-empty queue, never
    /// before"): a content update queued just ahead of the final frame
    /// (the exact shape `crate::pane::flush_ended` produces) is written
    /// first, and the stream is still registered while the writer thread
    /// is genuinely paused between that write and popping the final
    /// frame - this reaches into `Queues` directly (this test module is
    /// nested inside the same file, so it can) because the public API has
    /// no other way to observe "is this stream still registered" without
    /// itself mutating something.
    #[test]
    fn end_stream_tears_the_stream_down_only_once_the_writer_actually_drains_it() {
        let outbound = Arc::new(Outbound::new());
        outbound.open_stream(2);
        outbound.enqueue_data(2, b"content".to_vec()).unwrap();
        outbound.end_stream(2, b"the-end".to_vec()).unwrap();
        outbound.close();

        let (mut client, server) = UnixStream::pair().unwrap();
        let (proceed_tx, proceed_rx) = mpsc::channel();
        let step = StepWriter {
            inner: server,
            proceed: proceed_rx,
        };
        let writer_outbound = Arc::clone(&outbound);
        let writer = std::thread::spawn(move || run_writer(&writer_outbound, step));

        // "content"'s bytes are on the wire - `StepWriter::write` forwards
        // them before it ever blocks - but the writer thread itself is
        // stuck inside that same call, so it has not looped back to pop
        // "the-end" yet: stream 2 is still registered.
        assert_eq!(read_exact_bytes(&mut client, b"content".len()), b"content");
        assert!(
            outbound
                .queues
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .data
                .contains_key(&2),
            "the stream must still be registered before its own final frame has been popped"
        );

        // Lets the writer's own `write("content")` return; it loops back,
        // pops "the-end" (emptying stream 2's own queue while it is marked
        // closing - the teardown itself happens here, inside `pop_next`,
        // under the lock, strictly before the write below even starts),
        // then calls `write("the-end")`, which forwards its bytes before
        // blocking again.
        proceed_tx.send(()).unwrap();
        assert_eq!(read_exact_bytes(&mut client, b"the-end".len()), b"the-end");
        assert!(
            !outbound
                .queues
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .data
                .contains_key(&2),
            "the stream must be torn down the instant its own final frame is popped, not later"
        );

        proceed_tx.send(()).unwrap(); // lets `write("the-end")` itself return
        writer.join().unwrap();
    }

    // --- `Outbound::enqueue_control_and_wait_until_written` (TASK-1.88) ---

    /// Pins the ordering deterministically via `StepWriter`, not by timing
    /// (the same technique `end_stream_tears_the_stream_down_only_once_the_
    /// writer_actually_drains_it` already uses above): the calling thread's
    /// own wait must not return - its ack has not fired - while `run_writer`
    /// is still genuinely blocked *inside* `write_all` for this exact frame,
    /// even though `StepWriter::write` has already forwarded the bytes onto
    /// the real socket by that point (it forwards first, then blocks) - the
    /// bytes reaching the wire is not what this method waits on; `write_all`
    /// itself *returning* is. Only once `write_all` is let return (by
    /// releasing `StepWriter`) does the ack fire and the wait return.
    #[test]
    fn enqueue_control_and_wait_until_written_does_not_return_until_write_all_itself_returns() {
        let outbound = Arc::new(Outbound::new());
        let (mut client, server) = UnixStream::pair().unwrap();
        let (proceed_tx, proceed_rx) = mpsc::channel();
        let step = StepWriter {
            inner: server,
            proceed: proceed_rx,
        };
        let writer_outbound = Arc::clone(&outbound);
        let writer = std::thread::spawn(move || run_writer(&writer_outbound, step));

        let (done_tx, done_rx) = mpsc::channel();
        let wait_outbound = Arc::clone(&outbound);
        std::thread::spawn(move || {
            let result = wait_outbound.enqueue_control_and_wait_until_written(
                b"stop-reply".to_vec(),
                Duration::from_secs(5),
            );
            let _ = done_tx.send(result);
        });

        // Deterministic absence, not a race against a clock: `StepWriter`
        // cannot release on its own - nothing but this test's own
        // `proceed_tx.send` below can ever unblock it - so a bounded wait
        // finding nothing here is a structural fact, not a timing guess
        // that could flake under load (the same reasoning this crate's
        // own idle-checker tests already rely on for an absence window).
        assert_eq!(
            done_rx.recv_timeout(Duration::from_millis(300)),
            Err(mpsc::RecvTimeoutError::Timeout),
            "the wait must not return while run_writer is still inside write_all for this frame"
        );

        // The client already has the bytes - `StepWriter` forwards before
        // it blocks - but that is not what the method waits on.
        let mut buf = vec![0_u8; b"stop-reply".len()];
        client.read_exact(&mut buf).unwrap();
        assert_eq!(buf, b"stop-reply");
        assert_eq!(
            done_rx.recv_timeout(Duration::from_millis(50)),
            Err(mpsc::RecvTimeoutError::Timeout),
            "the client reading the bytes must not itself unblock the wait"
        );

        proceed_tx.send(()).unwrap(); // lets write_all itself return
        assert_eq!(
            done_rx.recv_timeout(Duration::from_secs(5)),
            Ok(Ok(())),
            "the wait must return promptly once write_all has actually returned"
        );

        outbound.close();
        writer.join().unwrap();
    }

    /// The timeout/closed-queue half: nothing ever arrives to ack a frame
    /// whose writer has already ended (the queue closes itself, the same
    /// way a broken write already does) - must not hang past `bound`.
    #[test]
    fn enqueue_control_and_wait_until_written_returns_promptly_once_the_queue_is_already_closed() {
        let outbound = Outbound::new();
        outbound.close();

        let result = outbound
            .enqueue_control_and_wait_until_written(b"too-late".to_vec(), Duration::from_secs(5));

        assert_eq!(result, Err(EnqueueControlError::Closed));
    }

    #[test]
    fn enqueue_control_and_wait_until_written_delivers_the_frame_over_a_real_writer() {
        let outbound = Arc::new(Outbound::new());
        let (mut client, server) = UnixStream::pair().unwrap();
        let writer_outbound = Arc::clone(&outbound);
        let writer = std::thread::spawn(move || run_writer(&writer_outbound, server));
        let _writer_guard = ClosingWriterGuard::new(Arc::clone(&outbound), writer);

        let result = outbound
            .enqueue_control_and_wait_until_written(b"real-reply".to_vec(), Duration::from_secs(5));

        assert_eq!(result, Ok(()));
        assert_eq!(
            read_exact_bytes(&mut client, b"real-reply".len()),
            b"real-reply"
        );
    }

    /// A client that keeps sending requests while never reading any reply
    /// (TASK-1.45 §3) fills the control queue; past that point the caller
    /// (not this module) is expected to close the connection - what this
    /// proves is only that the queue itself refuses to grow past its cap.
    #[test]
    fn a_full_control_queue_refuses_rather_than_growing_without_bound() {
        let outbound = Outbound::new();
        for i in 0..super::CONTROL_CAP {
            outbound
                .enqueue_control(format!("reply-{i}").into_bytes())
                .unwrap();
        }
        assert_eq!(
            outbound.enqueue_control(b"one-too-many".to_vec()),
            Err(EnqueueControlError::Full)
        );
    }

    /// The real-socket, real-thread proof behind AC #3: a genuine flood on
    /// a data stream (a `Barrier`-timed burst of many sizeable frames, all
    /// enqueued before the writer is even given a chance to start draining
    /// them - not merely "many calls in a loop" with time to drain between
    /// them) must not delay a control frame enqueued immediately after it.
    /// Negative control: this module's own tests/outbound.rs-adjacent
    /// coverage; see this task's own notes for the recorded run with the
    /// writer's priority reversed.
    #[test]
    fn a_genuine_flood_on_a_data_stream_never_delays_a_control_reply() {
        const FLOOD_FRAMES: usize = 5_000;
        const FRAME_BYTES: usize = 1024;

        let outbound = Arc::new(Outbound::new());
        outbound.open_stream(1);

        // Floods the stream from a second thread, synchronised with this
        // one via a barrier so the writer has nothing queued at all until
        // both threads are ready - the writer thread (spawned after this
        // barrier) genuinely has to contend with the whole flood already
        // sitting in the queue by the time it starts, not a trickle it
        // could keep up with as it goes.
        let barrier = Arc::new(Barrier::new(2));
        let flood_outbound = Arc::clone(&outbound);
        let flood_barrier = Arc::clone(&barrier);
        let flooder = std::thread::spawn(move || {
            flood_barrier.wait();
            for _ in 0..FLOOD_FRAMES {
                let _ = flood_outbound.enqueue_data(1, vec![0_u8; FRAME_BYTES]);
            }
        });

        barrier.wait();
        // A real race with the flooder, on purpose: the control frame may
        // land before, during, or after the flood is enqueued - the
        // writer's own priority is what must make its *delivery* prompt
        // regardless, not the order these two calls happen to run in.
        let enqueued_control_at = Instant::now();
        outbound
            .enqueue_control(b"control-under-flood".to_vec())
            .unwrap();

        let (mut client, server) = UnixStream::pair().unwrap();
        let writer_outbound = Arc::clone(&outbound);
        let writer = std::thread::spawn(move || run_writer(&writer_outbound, server));
        // On the happy path this is a no-op by the time it drops (`drain`,
        // below, already joins the writer indirectly - `client` only
        // sees EOF once the writer's own sink is dropped, which is after
        // `run_writer` returns). It earns its keep on the panicking path:
        // the latency assertion below, under real load, is exactly the
        // kind of thing that could trip - without this, that would leave
        // both `writer` and `flooder` parked with nothing to end them.
        let _writer_guard = ClosingWriterGuard::new(Arc::clone(&outbound), writer);

        let received = read_exact_bytes(&mut client, b"control-under-flood".len());
        let latency = enqueued_control_at.elapsed();
        assert_eq!(received, b"control-under-flood");
        assert!(
            latency < Duration::from_millis(500),
            "a control reply must not wait behind a data flood; took {latency:?}"
        );

        flooder.join().unwrap();
        outbound.close();
        // Drain the rest of the flood so the writer thread actually exits
        // (it would otherwise still be blocked trying to write the flood's
        // own, still-queued frames into a client that has stopped reading).
        let drain = std::thread::spawn(move || {
            let mut sink = [0_u8; 65536];
            while client.read(&mut sink).unwrap_or(0) > 0 {}
        });
        drain.join().unwrap();
    }
}
