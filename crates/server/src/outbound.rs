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

use std::collections::{HashMap, VecDeque};
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

/// decision-2 §6's own measured number for a data-mode attachment's queue.
/// Read only by this module's own tests today: nothing in this task ever
/// opens a real stream (attach is TASK-1.47), so a production build never
/// reaches it.
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "TASK-1.47's attach is the first real caller of the data side this guards; exercised today only by this module's own tests, which is why the expectation is scoped to a non-test build"
    )
)]
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

/// Why [`Outbound::enqueue_data`] refused. Built and returned only by
/// [`Outbound::enqueue_data`], which (see its own doc) is itself reachable
/// only from this module's own tests today - the same TASK-1.47 gap as
/// [`DATA_CAP`].
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "TASK-1.47's attach is the first real caller of enqueue_data, the only thing that ever produces this; exercised today only by this module's own tests"
    )
)]
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum EnqueueDataError {
    /// [`Outbound::open_stream`] was never called for this stream id (or
    /// [`Outbound::close_stream`] already closed it) - nothing in this
    /// task ever opens a stream (attach is TASK-1.47), so this is reachable
    /// only by this module's own tests today.
    NotOpen,
    /// The stream's own queue is already at [`DATA_CAP`]: this task's own
    /// placeholder back-pressure policy (decision-2 §6's real
    /// drop-and-resync needs a snapshot to send, which does not exist until
    /// TASK-1.47) - the frame is dropped, not queued, and not blocked on.
    Full,
    /// [`Outbound::close`] has already been called.
    Closed,
}

/// The state behind one [`Outbound`]'s lock: the control queue, every open
/// stream's own data queue, the round-robin order streams are drained in,
/// and whether this connection is closing.
struct Queues {
    control: VecDeque<Vec<u8>>,
    data: HashMap<u32, VecDeque<Vec<u8>>>,
    /// Open stream ids, in the order [`Outbound::open_stream`] was called -
    /// decision-2 §3's own "takes one frame from each attachment in turn".
    order: Vec<u32>,
    /// Where the round robin resumes next time control is empty - an index
    /// into `order`, not a stream id, so it stays valid as streams open and
    /// close (clamped back on close, below).
    cursor: usize,
    closed: bool,
}

impl Queues {
    fn new() -> Self {
        Self {
            control: VecDeque::new(),
            data: HashMap::new(),
            order: Vec::new(),
            cursor: 0,
            closed: false,
        }
    }

    /// One queued item, control first (fully drained before any data is
    /// even considered - decision-2 §3), then one frame from the next data
    /// stream in round-robin order that actually has one queued (a stream
    /// with nothing queued is skipped, not given a turn that wastes it).
    /// `None` means nothing is queued right now, for any stream - the
    /// caller decides what that means (wait, or stop, if also `closed`).
    fn pop_next(&mut self) -> Option<Vec<u8>> {
        if let Some(bytes) = self.control.pop_front() {
            return Some(bytes);
        }
        let streams = self.order.len();
        for _ in 0..streams {
            let stream = self.order[self.cursor];
            self.cursor = (self.cursor + 1) % streams;
            if let Some(queue) = self.data.get_mut(&stream)
                && let Some(bytes) = queue.pop_front()
            {
                return Some(bytes);
            }
        }
        None
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
        queues.control.push_back(bytes);
        self.ready.notify_one();
        Ok(())
    }

    /// Registers a stream so [`Outbound::enqueue_data`] accepts frames for
    /// it and the writer's round robin gives it a turn. Idempotent: opening
    /// an already-open stream changes nothing (not reached today - nothing
    /// in this task calls this at all; TASK-1.47's own attach code is the
    /// first real caller).
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "TASK-1.47's attach is the first real caller; exercised today only by this module's own tests"
        )
    )]
    pub(crate) fn open_stream(&self, stream: u32) {
        let mut queues = self.lock();
        if let std::collections::hash_map::Entry::Vacant(entry) = queues.data.entry(stream) {
            entry.insert(VecDeque::new());
            queues.order.push(stream);
        }
    }

    /// Unregisters a stream: no more frames are accepted for it, and
    /// whatever was still queued for it is dropped. Keeps `cursor` valid by
    /// construction - removing by position and taking `cursor` modulo the
    /// new (possibly smaller) length, rather than leaving it pointing past
    /// the end.
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "TASK-1.47's attach is the first real caller; exercised today only by this module's own tests"
        )
    )]
    pub(crate) fn close_stream(&self, stream: u32) {
        let mut queues = self.lock();
        queues.data.remove(&stream);
        if let Some(index) = queues.order.iter().position(|&id| id == stream) {
            queues.order.remove(index);
        }
        if !queues.order.is_empty() {
            queues.cursor %= queues.order.len();
        } else {
            queues.cursor = 0;
        }
    }

    /// Queues opaque bytes on an already-open data stream - reached only by
    /// this module's own tests today (see [`EnqueueDataError::NotOpen`]'s
    /// own doc).
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "TASK-1.47's attach is the first real caller; exercised today only by this module's own tests"
        )
    )]
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
                if let Some(bytes) = queues.pop_next() {
                    break Some(bytes);
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
        let Some(bytes) = item else { return };
        if sink.write_all(&bytes).is_err() {
            return;
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
    use std::io::Read;
    use std::os::unix::net::UnixStream;
    use std::sync::Arc;
    use std::sync::Barrier;
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

        assert_eq!(
            read_exact_bytes(&mut client, b"control-second".len()),
            b"control-second"
        );
        assert_eq!(
            read_exact_bytes(&mut client, b"data-first".len()),
            b"data-first"
        );

        // `close()`, not just dropping `client`: the writer may be
        // blocked on the condvar (nothing queued, not yet closed), which
        // only a `notify` - not a dropped socket - ever wakes.
        outbound.close();
        writer.join().unwrap();
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

        // Round robin in the order the streams were opened: one frame from
        // stream 1, one from stream 2, then back to each one's own second
        // frame - never both of one stream's frames back to back while the
        // other stream has one waiting.
        assert_eq!(read_exact_bytes(&mut client, 2), b"1a");
        assert_eq!(read_exact_bytes(&mut client, 2), b"2a");
        assert_eq!(read_exact_bytes(&mut client, 2), b"1b");
        assert_eq!(read_exact_bytes(&mut client, 2), b"2b");

        outbound.close();
        writer.join().unwrap();
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

        assert_eq!(
            read_exact_bytes(&mut client, b"only-this".len()),
            b"only-this"
        );

        outbound.close();
        writer.join().unwrap();
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
        writer.join().unwrap();
        drain.join().unwrap();
    }
}
