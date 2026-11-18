//! The connection, once startup has handed it over: a dedicated thread blocks on the
//! socket and decodes with the sdk's own `Connection`; each update reaches the foreground
//! task over a channel it `.await`s - never polled (TASK-1.117's plan §1). A second,
//! dedicated thread owns the write half (review finding 1, `Writer` below): no socket
//! write ever happens on the UI thread, for input or for declared sizes alike.

use std::io::{self, Read, Write as _};
use std::net::Shutdown;
use std::os::unix::net::UnixStream;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, SyncSender, TrySendError, sync_channel};
use std::thread::{self, JoinHandle};

use futures::channel::mpsc::UnboundedSender;
use steg_sdk::{Connection, EndReason, Incoming, ScreenUpdate};

/// What the reader thread, and now the writer thread too, hand the foreground task - one
/// shared channel, so either side reporting the connection lost reaches the window through
/// the exact same path (review finding 1: "a write error or disconnect on the writer
/// thread reaches the window the same way a reader-side disconnect does today").
#[derive(Debug)]
pub enum ClientEvent {
    Update(ScreenUpdate),
    /// The pane has ended (TASK-1.129; doc-14, "The end of a pane, on its stream") - its
    /// own, final `ScreenUpdate`, decoded by `drain` into its own event rather than folded
    /// into `Update`, since the window's own reaction (keep the last screen, show why, stop
    /// sending) is not an ordinary screen update. Never followed by `Disconnected` for this
    /// same attachment: `drain` closes this connection's own side of the stream the moment
    /// it decodes this (`Connection::close_stream`), and `crates/app/src/main.rs`'s own
    /// handler stops reading further events once it has this one.
    Ended(EndReason),
    /// The connection ended - a clean EOF, a write that failed, this task's own writer
    /// queue filling up behind a peer that never reads (`Writer`'s own module doc), or a
    /// frame this read-only app's own design does not expect after startup (TASK-1.117's
    /// plan §1: part 1 makes no call at all once attached, so anything but a
    /// `StreamControl` on the one open stream is a protocol-level surprise, not specially
    /// recovered from).
    Disconnected,
}

/// The connection's own reader thread, kept only so [`Client::stop`] can end it - the
/// channel it feeds is created by the caller and shared with [`Writer`] (module doc), so
/// either side's own `Disconnected` reaches the one place the window listens.
pub struct Client {
    shutdown_handle: UnixStream,
    reader: Option<JoinHandle<()>>,
}

impl Client {
    /// Starts the reader thread, which owns `stream` and `connection` from here on - this
    /// call never touches either again. `shutdown_handle` is a clone of the same socket,
    /// kept only so [`Client::stop`] can unblock the reader's own `read()`. `events` is
    /// the shared sender [`Writer::new`] also holds a clone of.
    pub fn start(
        stream: UnixStream,
        connection: Connection,
        shutdown_handle: UnixStream,
        events: UnboundedSender<ClientEvent>,
    ) -> Self {
        let reader = thread::spawn(move || read_loop(stream, connection, &events));
        Self {
            shutdown_handle,
            reader: Some(reader),
        }
    }

    /// Unblocks the reader thread's own `read()` (`shutdown()` on one fd of a socket ends
    /// a blocking `read()` on another fd of the same socket - standard POSIX behaviour)
    /// and joins it. Called once, on window close: no thread and no socket outlive the
    /// window (TASK-1.117's plan §1/§3). `join()` itself has no timeout - this is
    /// expected to return promptly once `read()` is unblocked, not bounded by the API;
    /// `tests::` is what actually proves promptness, with its own bounded poll.
    pub fn stop(&mut self) {
        let _ = self.shutdown_handle.shutdown(Shutdown::Both);
        if let Some(reader) = self.reader.take() {
            let _ = reader.join();
        }
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        self.stop();
    }
}

/// One thing the UI thread asked the writer thread to send, in the order it asked - a
/// single queue and a single consuming thread is what keeps input and declared sizes in
/// the order the UI issued them (review finding 1's own ordering requirement): two
/// separate queues could not promise that once either one's own write actually blocks.
enum WriterCommand {
    Input(Vec<u8>),
    DeclareSize { columns: u16, rows: u16 },
}

/// How many commands the writer's own queue holds before a sender is told it is full
/// (review finding 1: "say what bounds the queue"). `try_send` never blocks the caller
/// regardless of this number - the bound's own job is to cap how large the backlog can
/// grow once the kernel's own much larger 8 KiB socket buffer (measured on this machine,
/// `net.local.stream.sendspace`) is *also* exhausted, not to guarantee any particular
/// detection time. Plain keystroke-at-a-time typing, even at a fast held-key autorepeat
/// rate, produces only a few bytes a command and would take minutes to exhaust the
/// kernel's own buffer first - this bound plays no part in that case. What it does bound
/// is the case review finding 1 named: a single oversized command (a large paste, several
/// kilobytes in one `WriterCommand::Input`) that by itself can exceed the kernel buffer
/// and block the writer thread's one in-flight write - after that, up to this many more
/// commands can still queue up (comfortably more than a few seconds of further typing or
/// resizing) before the connection is reported lost, while keeping the worst-case backlog
/// a small, fixed amount of memory instead of unbounded growth.
const WRITER_QUEUE_CAPACITY: usize = 64;

/// The main thread's own writer (review finding 1): owns nothing about the socket
/// directly any more - every `send_input`/`declare_size` call hands its own bytes to a
/// dedicated thread over a bounded channel and returns at once, never blocking on the
/// socket itself. That thread blocks on the channel's own `recv()` until there is
/// something to send - no polling, no timer, the same shape the reader thread's own
/// blocking `read()` already has. `try_send`'s own non-blocking contract is what makes
/// "returns at once" true even when the queue is full, not merely when it has room.
pub(crate) struct Writer {
    commands: Option<SyncSender<WriterCommand>>,
    thread: Option<JoinHandle<()>>,
    /// A clone of the write handle, kept only to unblock the writer thread's own blocking
    /// `write_all` from [`Writer::stop`] - the same mechanism [`Client::stop`] already
    /// uses for the reader (`shutdown()` on one fd fails a blocked write on another fd of
    /// the same socket, the identical POSIX guarantee, not a different one).
    shutdown_handle: UnixStream,
    events: UnboundedSender<ClientEvent>,
    /// Guards `events` so `Disconnected` is reported once, the first time either a
    /// `try_send` finds the queue full or the writer thread's own write fails - shared
    /// (not merely duplicated) between this struct and the writer thread's own closure,
    /// since either side can be the one that first notices.
    reported: Arc<AtomicBool>,
}

impl Writer {
    /// `connection` must already have `stream` open (`Connection::open_stream`), mirroring
    /// what the reader's own `Connection` did with the same id - two independent sdk state
    /// objects representing the one logical attachment, seeded the same way. `events` is
    /// the shared sender [`Client::start`] also holds a clone of (module doc).
    pub(crate) fn new(
        handle: UnixStream,
        connection: Connection,
        stream: u32,
        tab_target: String,
        events: UnboundedSender<ClientEvent>,
    ) -> Self {
        let shutdown_handle = handle
            .try_clone()
            .expect("clone the write handle for Writer::stop");
        let (commands_tx, commands_rx) = sync_channel(WRITER_QUEUE_CAPACITY);
        let thread_events = events.clone();
        let reported = Arc::new(AtomicBool::new(false));
        let thread_reported = Arc::clone(&reported);
        let thread = thread::spawn(move || {
            write_loop(
                handle,
                connection,
                stream,
                tab_target,
                &commands_rx,
                &thread_events,
                &thread_reported,
            );
        });
        Self {
            commands: Some(commands_tx),
            thread: Some(thread),
            shutdown_handle,
            events,
            reported,
        }
    }

    /// Hands `bytes` to the writer thread as input on the attachment's own stream
    /// (TASK-1.121 plan §1/§3) - a non-blocking enqueue, never a socket write on this
    /// thread (review finding 1). If the queue is already full (the stalled-peer case
    /// `WRITER_QUEUE_CAPACITY`'s own doc names) or the writer thread has already ended,
    /// the connection is treated as lost: reported once, the same way a reader-side
    /// disconnect already is, never silently dropped with no consequence at all.
    pub(crate) fn send_input(&mut self, bytes: Vec<u8>) {
        self.try_send(WriterCommand::Input(bytes));
    }

    /// Hands `columns`/`rows` to the writer thread to declare for this attachment's own
    /// tab (TASK-1.121 plan §4) - the same non-blocking enqueue `send_input` uses, through
    /// the same single queue, which is what keeps the two in the order the UI issued them.
    pub(crate) fn declare_size(&mut self, columns: u16, rows: u16) {
        self.try_send(WriterCommand::DeclareSize { columns, rows });
    }

    /// Makes every later `send_input`/`declare_size` call a silent no-op, with no
    /// `Disconnected` either - the backstop `crates/app/src/main.rs`'s own `Ended`
    /// handler uses (TASK-1.129), never [`Writer::stop`]. `stop` also shuts the socket
    /// down, "the identical POSIX guarantee" [`Client::stop`]'s own doc already states
    /// for the reader - and that socket is the *same* one the reader thread blocks its
    /// own `read()` on (`crates/app/src/startup.rs`'s own `Attached::write_handle`: "a
    /// clone of the connected socket"). Calling `stop` once a pane has ended would
    /// therefore also end the reader's own read with `Ok(0)`, producing a `Disconnected`
    /// right behind `Ended` that overwrites the window's own last screen and message with
    /// "disconnected" - found by an advisor review of this task's own plan, before any
    /// code existed, not by a test.
    ///
    /// Only drops the command sender - no `shutdown()`, no `join()`. `try_send`'s own
    /// `None` branch, just below, already makes every later send harmless; the writer
    /// thread itself ends on its own, the next time `write_loop`'s own `commands.recv()`
    /// sees the sender gone, and [`Writer::stop`] (called later, by `teardown`, on window
    /// close) still joins it then, same as always.
    pub(crate) fn disarm(&mut self) {
        self.commands = None;
    }

    fn try_send(&mut self, command: WriterCommand) {
        let Some(commands) = &self.commands else {
            return;
        };
        match commands.try_send(command) {
            Ok(()) => {}
            Err(TrySendError::Full(_) | TrySendError::Disconnected(_)) => {
                report_disconnected(&self.events, &self.reported);
            }
        }
    }

    /// Unblocks the writer thread - whether it is idle in `commands_rx.recv()` (dropping
    /// the sender ends that wait, since no further command can ever arrive) or stuck
    /// inside its own blocking `write_all` (`shutdown_handle`'s own `shutdown()` call ends
    /// that instead, module doc) - and joins it. Called once, on window close, the same
    /// shape [`Client::stop`] already has for the reader: no thread outlives the window,
    /// and this never joins a thread that could still be running for an unbounded reason,
    /// since one of these two calls is guaranteed to end it first (review finding 1: "must
    /// not join a thread that can never finish").
    pub(crate) fn stop(&mut self) {
        self.commands = None;
        let _ = self.shutdown_handle.shutdown(Shutdown::Both);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl Drop for Writer {
    fn drop(&mut self) {
        self.stop();
    }
}

/// The writer thread's own body: blocks on `commands.recv()` (no polling, no timer) until
/// there is something to send, builds the frame for it, and writes it - ending the moment
/// either the channel closes (nothing further can ever arrive, module doc on
/// `Writer::stop`) or a write fails (the connection is gone; reported once, the same
/// `ClientEvent::Disconnected` path the reader thread already uses, and nothing further is
/// attempted on an already-dead socket).
fn write_loop(
    mut handle: UnixStream,
    mut connection: Connection,
    stream: u32,
    tab_target: String,
    commands: &Receiver<WriterCommand>,
    events: &UnboundedSender<ClientEvent>,
    reported: &Arc<AtomicBool>,
) {
    while let Ok(command) = commands.recv() {
        let frame = match command {
            WriterCommand::Input(bytes) => connection.send_data(stream, &bytes).ok(),
            WriterCommand::DeclareSize { columns, rows } => connection
                .declare_size(Some(&tab_target), columns, rows)
                .ok()
                .map(|(_id, frame)| frame),
        };
        let Some(frame) = frame else {
            // Validation failed against this task's own well-formed input (never expected
            // in practice - `size::grid_size`'s own floor already keeps a zero size from
            // reaching here, and `send_data` only refuses a stream this connection has not
            // itself opened, which this one always has) - best-effort, same as before this
            // task's own writer thread existed: skip this one command, keep serving the
            // queue rather than treating a validation bug as a dead connection.
            continue;
        };
        if handle.write_all(&frame).is_err() {
            report_disconnected(events, reported);
            return;
        }
    }
}

/// Sends `Disconnected` the first time either caller (`Writer::try_send`'s own full or
/// dead queue, or `write_loop`'s own failed write) calls this - the `swap` is what makes
/// "once" true across the two threads that can each call it, not merely documented.
fn report_disconnected(events: &UnboundedSender<ClientEvent>, reported: &Arc<AtomicBool>) {
    if !reported.swap(true, Ordering::SeqCst) {
        let _ = events.unbounded_send(ClientEvent::Disconnected);
    }
}

fn read_loop(
    mut stream: UnixStream,
    mut connection: Connection,
    sender: &UnboundedSender<ClientEvent>,
) {
    let mut buf = [0_u8; 65536];
    loop {
        match stream.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => connection.receive(&buf[..n]),
            Err(err) if err.kind() == io::ErrorKind::Interrupted => continue,
            Err(_) => break,
        }
        if !drain(&mut connection, sender) {
            break;
        }
    }
    let _ = sender.unbounded_send(ClientEvent::Disconnected);
}

/// Drains every already-decoded frame before the next `read()` (TASK-1.117's plan §2: a
/// backlog collapses into however many events this batch actually needs, not one
/// `read()`/wake per frame). Returns `false` when the connection should stop - a
/// surprising frame, same as an I/O error above.
fn drain(connection: &mut Connection, sender: &UnboundedSender<ClientEvent>) -> bool {
    while let Some(incoming) = connection.poll() {
        let update = match incoming {
            Incoming::StreamControl { stream, payload } => {
                match serde_json::from_slice::<ScreenUpdate>(&payload) {
                    // TASK-1.129: the pane's own final update - its own event, not an
                    // ordinary `Update`, and not the protocol-level surprise that would
                    // stop this loop (`return false`, below): this is the stream's own
                    // last, expected frame, and `read_loop`'s own cleanup line sends
                    // `Disconnected` unconditionally once this loop breaks, which must
                    // never fire for this reason and race `main.rs`'s own handling of
                    // `Ended`. `close_stream` is this connection's own half of "closed on
                    // both sides" (decision-2 §3; doc-14, "The end of a pane, on its
                    // stream") - the server's own half already happened before this frame
                    // was ever sent.
                    Ok(ScreenUpdate::Ended { reason }) => {
                        connection.close_stream(stream);
                        if sender.unbounded_send(ClientEvent::Ended(reason)).is_err() {
                            return false;
                        }
                        continue;
                    }
                    Ok(update) => update,
                    Err(_) => return false,
                }
            }
            // TASK-1.121's own writer thread calls `tab.declare-size` after startup, whose
            // reply lands here, on the reader's own decode-only `Connection` - ignored,
            // not correlated to anything (this app acts the same way whether it succeeds
            // or not, `Writer`'s own doc), and not the protocol-level surprise it would
            // have been before this task, when nothing on this connection ever called
            // again.
            Incoming::Reply { .. } => continue,
            // Still a protocol-level surprise this app's own design does not produce or
            // expect: no event subscription, no second attachment, no handshake replay.
            Incoming::Hello(_)
            | Incoming::Request { .. }
            | Incoming::Event { .. }
            | Incoming::UnknownEvent { .. }
            | Incoming::Data { .. }
            | Incoming::UnknownStream { .. }
            | Incoming::Malformed(_) => return false,
        };
        if sender.unbounded_send(ClientEvent::Update(update)).is_err() {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc;
    use std::time::{Duration, Instant};

    use futures::channel::mpsc::UnboundedReceiver;
    use serde_json::Value;
    use steg_sdk::{
        Connection, Description, Frame, Response, ScreenUpdate, WireRole, encode_frame,
    };

    use super::*;

    fn description_with_no_methods() -> Description {
        Description {
            api_version: steg_sdk::Version { major: 1, minor: 0 },
            methods: Vec::new(),
            events: Vec::new(),
            errors: Vec::new(),
        }
    }

    fn encode_update(stream: u32, update: &ScreenUpdate) -> Vec<u8> {
        let payload = serde_json::to_vec(update).expect("ScreenUpdate always serialises");
        encode_frame(&Frame::control(stream, payload)).expect("fits the frame cap")
    }

    fn sample_update() -> ScreenUpdate {
        ScreenUpdate::Unchanged
    }

    /// A real, local, in-process socket pair, not a double (testing-rust.md: a real
    /// socket in one is cheap and is not a double) - one half stands in for the server.
    fn start_client() -> (UnixStream, Client, UnboundedReceiver<ClientEvent>) {
        let (server_side, app_side) = UnixStream::pair().expect("a socket pair");
        let shutdown_handle = app_side.try_clone().expect("clone the socket");
        let mut connection = Connection::new(WireRole::Client, description_with_no_methods());
        connection.open_stream(7);
        let (events, receiver) = futures::channel::mpsc::unbounded();
        let client = Client::start(app_side, connection, shutdown_handle, events);
        (server_side, client, receiver)
    }

    /// `JoinHandle::join` has no timeout, and neither does `.await`ing an `mpsc` receiver
    /// with nothing ever sent - so every test below that could hang if this task's own
    /// mechanism were broken instead runs the blocking call on its own thread and waits on
    /// a plain, timeout-capable `std::sync::mpsc` channel from the test's own thread: a
    /// broken mechanism fails the assertion, not the test process.
    fn bounded<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> T {
        bounded_within(Duration::from_secs(2), f)
    }

    fn bounded_within<T: Send + 'static>(
        timeout: Duration,
        f: impl FnOnce() -> T + Send + 'static,
    ) -> T {
        let (sender, receiver) = mpsc::channel();
        thread::spawn(move || {
            let _ = sender.send(f());
        });
        receiver
            .recv_timeout(timeout)
            .expect("did not finish within the test's own bound")
    }

    #[test]
    fn a_control_frame_on_the_open_stream_arrives_as_an_update() {
        let (mut server_side, _client, mut receiver) = start_client();
        server_side
            .write_all(&encode_update(7, &sample_update()))
            .expect("write to the server side");

        let event =
            bounded(move || futures::executor::block_on(futures::StreamExt::next(&mut receiver)));
        assert!(matches!(
            event,
            Some(ClientEvent::Update(ScreenUpdate::Unchanged))
        ));
    }

    #[test]
    fn a_frame_on_a_stream_never_opened_ends_the_connection() {
        let (mut server_side, _client, mut receiver) = start_client();
        server_side
            .write_all(&encode_update(9, &sample_update()))
            .expect("write to the server side");

        let event =
            bounded(move || futures::executor::block_on(futures::StreamExt::next(&mut receiver)));
        assert!(matches!(event, Some(ClientEvent::Disconnected)));
    }

    // --- TASK-1.129: the pane ending, as its own client event ------------------------

    fn sample_ended(reason: EndReason) -> ScreenUpdate {
        ScreenUpdate::Ended { reason }
    }

    #[test]
    fn an_ended_update_arrives_as_its_own_client_event_not_update_or_disconnected() {
        let (mut server_side, _client, mut receiver) = start_client();
        server_side
            .write_all(&encode_update(
                7,
                &sample_ended(EndReason::ProgramExited { exit_code: 3 }),
            ))
            .expect("write to the server side");

        let event =
            bounded(move || futures::executor::block_on(futures::StreamExt::next(&mut receiver)));
        assert!(
            matches!(
                event,
                Some(ClientEvent::Ended(EndReason::ProgramExited {
                    exit_code: 3
                }))
            ),
            "expected Ended, not Update or Disconnected: {event:?}"
        );
    }

    /// `drain`'s own `connection.close_stream(stream)` call (TASK-1.129; AC #2's "the
    /// client closes its own side of the stream") - proven here in isolation, at the
    /// `Connection`/`drain` level, by what happens if one more frame ever arrived on the
    /// same stream afterward: it decodes as a protocol surprise, not an ordinary update.
    /// In the real, wired-up app this is never actually reached - `main.rs`'s own `Ended`
    /// handler stops the receive task for good the moment it sees this event, so nothing
    /// is left to read a frame that follows it - this test exercises the lower-level
    /// mechanism on its own, not a path the running app ever takes.
    #[test]
    fn ended_closes_this_connections_own_side_so_a_later_frame_on_the_same_stream_is_unknown() {
        let (mut server_side, _client, mut receiver) = start_client();
        server_side
            .write_all(&encode_update(7, &sample_ended(EndReason::Closed)))
            .expect("write the Ended frame");
        server_side
            .write_all(&encode_update(7, &sample_update()))
            .expect("write a frame on the same stream, afterward");

        let (first, second) = bounded(move || {
            futures::executor::block_on(async {
                let first = futures::StreamExt::next(&mut receiver).await;
                let second = futures::StreamExt::next(&mut receiver).await;
                (first, second)
            })
        });
        assert!(
            matches!(first, Some(ClientEvent::Ended(EndReason::Closed))),
            "expected Ended first: {first:?}"
        );
        assert!(
            matches!(second, Some(ClientEvent::Disconnected)),
            "a frame on a stream this connection already closed its own side of must \
             decode as a protocol surprise, not an ordinary update: {second:?}"
        );
    }

    #[test]
    fn stop_unblocks_and_joins_the_reader_thread_within_the_tests_own_bound() {
        let (_server_side, mut client, _receiver) = start_client();
        // `Client::stop` itself calls `JoinHandle::join`, which has no timeout - `bounded`
        // is what keeps this test from hanging if `shutdown()` ever stopped unblocking
        // the reader's own `read()` (the mechanism this proves).
        bounded(move || client.stop());
    }

    // --- Writer (TASK-1.121 plan §6: "a real socket, no server, no window") -------------

    /// `server_side`'s own `Connection` must `open_stream` the same id too, mirroring what
    /// a real server's own per-connection bookkeeping does - `Connection::poll`'s own
    /// `open_streams` check is symmetric on both sides of the wire (`wire/connection.rs`),
    /// not something only a client needs.
    fn start_writer(
        stream: u32,
    ) -> (
        UnixStream,
        Connection,
        Writer,
        UnboundedReceiver<ClientEvent>,
    ) {
        let (server_side, app_side) = UnixStream::pair().expect("a socket pair");
        server_side
            .set_read_timeout(Some(Duration::from_secs(2)))
            .expect("set a read timeout");
        let mut writer_connection = Connection::new(WireRole::Client, steg_sdk::description());
        writer_connection.open_stream(stream);
        let (events, receiver) = futures::channel::mpsc::unbounded();
        let writer = Writer::new(
            app_side,
            writer_connection,
            stream,
            "sess/tab-1".to_owned(),
            events,
        );
        let mut server_connection = Connection::new(WireRole::Server, steg_sdk::description());
        server_connection.open_stream(stream);
        (server_side, server_connection, writer, receiver)
    }

    fn read_one(server_side: &mut UnixStream, server_connection: &mut Connection) -> Incoming {
        loop {
            if let Some(incoming) = server_connection.poll() {
                return incoming;
            }
            let mut buf = [0_u8; 65536];
            let n = server_side
                .read(&mut buf)
                .expect("read what the writer wrote");
            assert_ne!(n, 0, "the writer's own handle closed with nothing to read");
            server_connection.receive(&buf[..n]);
        }
    }

    #[test]
    fn declare_size_writes_a_tab_declare_size_request_naming_the_tab_and_the_size() {
        let (mut server_side, mut server_connection, mut writer, _events) = start_writer(7);
        writer.declare_size(80, 24);

        let Incoming::Request { request, .. } = read_one(&mut server_side, &mut server_connection)
        else {
            panic!("expected a request, not something else decoded off the same bytes");
        };
        assert_eq!(request.method, "tab.declare-size");
        assert_eq!(
            request.params.get("target").and_then(Value::as_str),
            Some("sess/tab-1"),
        );
        assert_eq!(
            request.params.get("columns").and_then(Value::as_i64),
            Some(80)
        );
        assert_eq!(request.params.get("rows").and_then(Value::as_i64), Some(24));
    }

    #[test]
    fn send_input_writes_a_data_frame_on_the_attachments_own_stream_not_stream_0() {
        let (mut server_side, mut server_connection, mut writer, _events) = start_writer(7);
        writer.send_input(b"hello".to_vec());

        let Incoming::Data { stream, bytes } = read_one(&mut server_side, &mut server_connection)
        else {
            panic!("expected a data frame, not something else decoded off the same bytes");
        };
        assert_eq!(stream, 7);
        assert_eq!(bytes, b"hello");
    }

    #[test]
    fn input_and_declared_sizes_arrive_on_the_wire_in_the_order_the_ui_issued_them() {
        let (mut server_side, mut server_connection, mut writer, _events) = start_writer(7);
        writer.send_input(b"a".to_vec());
        writer.declare_size(80, 24);
        writer.send_input(b"b".to_vec());

        let first = read_one(&mut server_side, &mut server_connection);
        let second = read_one(&mut server_side, &mut server_connection);
        let third = read_one(&mut server_side, &mut server_connection);

        assert!(
            matches!(&first, Incoming::Data { bytes, .. } if bytes == b"a"),
            "expected the first input, got {first:?}"
        );
        assert!(
            matches!(&second, Incoming::Request { request, .. } if request.method == "tab.declare-size"),
            "expected the declared size between the two inputs, got {second:?}"
        );
        assert!(
            matches!(&third, Incoming::Data { bytes, .. } if bytes == b"b"),
            "expected the second input, got {third:?}"
        );
    }

    /// `Writer::disarm` (TASK-1.129): the backstop `main.rs`'s own `Ended` handler uses
    /// instead of `Writer::stop` (that method's own doc has the full reasoning for why).
    /// Proves both halves of "a silent no-op": nothing reaches the wire, and no
    /// `Disconnected` fires either (unlike a full queue or a dead socket, which both
    /// report the connection lost - `disarm` is not that; the writer thread just quietly
    /// has nothing left to do).
    #[test]
    fn writer_disarm_makes_every_later_send_a_silent_no_op_with_no_disconnected_either() {
        let (mut server_side, _server_connection, mut writer, mut events) = start_writer(7);
        server_side
            .set_read_timeout(Some(Duration::from_millis(200)))
            .expect("set a short read timeout");

        writer.disarm();
        writer.send_input(b"after disarm".to_vec());
        writer.declare_size(80, 24);

        let mut buf = [0_u8; 64];
        let read = server_side.read(&mut buf);
        assert!(
            matches!(
                read.as_ref(),
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock
                    || e.kind() == std::io::ErrorKind::TimedOut
            ),
            "nothing should reach the wire after disarm: {read:?}"
        );

        // Polled, not checked once: the writer thread still needs a moment to notice its
        // own sender is gone and end - this gives it a real, bounded window to report
        // `Disconnected` if it ever wrongly did, rather than checking before it could.
        let deadline = Instant::now() + Duration::from_millis(200);
        let mut saw_event = None;
        while Instant::now() < deadline {
            if let Ok(event) = events.try_recv() {
                saw_event = Some(event);
                break;
            }
        }
        assert!(
            saw_event.is_none(),
            "no event, Disconnected included, should ever fire after disarm: {saw_event:?}"
        );
    }

    // --- a reply is actually observable on the connection (or isn't, silently) -----------
    //
    // What found TASK-1.117's own stop_startup bug (a connection shut down right after a
    // successful attach, before this task's own writer or reader ever touch it again) was
    // never that a request's own bytes were shaped wrong - they weren't; `declare_size`'s
    // own request above is correct either way. It was that nothing this task's own two
    // tests above check would ever have noticed the connection was already dead. These two
    // tests are what closes that gap at this level: one proves a reply really is
    // observable over the same real socket when the connection is alive (what the real
    // reader thread would eventually decode); the other proves the exact shape of the real
    // bug - once the connection is already shut down, declaring a size neither panics nor
    // reports a reply, and instead reaches the window as `ClientEvent::Disconnected`
    // (review finding 1's own requirement, superseding the plain "fire and forget, nothing
    // observable" shape these two tests were first written to prove).

    /// Like `start_writer`, but also returns a *separate* read handle on the same socket -
    /// standing in for the real reader thread's own handle (`Client::start`'s own `stream`
    /// parameter), which is who actually decodes a `tab.declare-size` reply in production.
    fn start_writer_with_reader_handle(
        stream: u32,
    ) -> (
        UnixStream,
        Connection,
        Writer,
        UnixStream,
        UnboundedReceiver<ClientEvent>,
    ) {
        let (server_side, app_side) = UnixStream::pair().expect("a socket pair");
        server_side
            .set_read_timeout(Some(Duration::from_secs(2)))
            .expect("set a read timeout");
        let app_read_side = app_side
            .try_clone()
            .expect("clone the app's own socket handle");
        app_read_side
            .set_read_timeout(Some(Duration::from_millis(500)))
            .expect("set a read timeout");
        let mut writer_connection = Connection::new(WireRole::Client, steg_sdk::description());
        writer_connection.open_stream(stream);
        let (events, receiver) = futures::channel::mpsc::unbounded();
        let writer = Writer::new(
            app_side,
            writer_connection,
            stream,
            "sess/tab-1".to_owned(),
            events,
        );
        let mut server_connection = Connection::new(WireRole::Server, steg_sdk::description());
        server_connection.open_stream(stream);
        (
            server_side,
            server_connection,
            writer,
            app_read_side,
            receiver,
        )
    }

    /// Reads from `read_side` for up to half a second total, decoding with `connection` -
    /// `None` if nothing arrives in that bound (a dead connection, or a slow one; this
    /// test's own control only ever produces the former). A bounded, non-panicking read,
    /// unlike this file's other tests' own `.expect()` on every read - deliberately, since
    /// "nothing ever arrives" is this test's own passing case for the control below, not a
    /// broken test harness.
    fn try_read_reply(read_side: &mut UnixStream, connection: &mut Connection) -> Option<Incoming> {
        let deadline = Instant::now() + Duration::from_millis(500);
        loop {
            if let Some(incoming) = connection.poll() {
                return Some(incoming);
            }
            if Instant::now() >= deadline {
                return None;
            }
            let mut buf = [0_u8; 65536];
            match read_side.read(&mut buf) {
                Ok(0) | Err(_) => return None,
                Ok(n) => connection.receive(&buf[..n]),
            }
        }
    }

    #[test]
    fn declare_sizes_own_reply_is_observable_over_the_connection_when_it_is_alive() {
        let (mut server_side, mut server_connection, mut writer, mut app_read_side, _events) =
            start_writer_with_reader_handle(7);
        writer.declare_size(80, 24);

        let Incoming::Request { id, .. } = read_one(&mut server_side, &mut server_connection)
        else {
            panic!("expected a request, not something else decoded off the same bytes");
        };
        let reply_bytes = Connection::reply(id, Response::Ok(serde_json::Value::Null));
        server_side
            .write_all(&reply_bytes)
            .expect("write the reply");

        let mut reader_connection = Connection::new(WireRole::Client, steg_sdk::description());
        let reply = try_read_reply(&mut app_read_side, &mut reader_connection);
        assert!(
            matches!(reply, Some(Incoming::Reply { .. })),
            "a reply should be observable on a live connection, got {reply:?}",
        );
    }

    #[test]
    fn once_the_connection_is_already_shut_down_declare_size_reports_disconnected_and_no_reply_is_ever_seen()
     {
        let (_server_side, _server_connection, mut writer, mut app_read_side, mut events) =
            start_writer_with_reader_handle(7);
        // The exact shape of TASK-1.117's own bug: the connection is shut down (by
        // something upstream of this task's own code) before this task's own writer is
        // ever asked to do anything with it.
        app_read_side
            .shutdown(std::net::Shutdown::Both)
            .expect("shut the connection down, simulating the real bug");

        writer.declare_size(80, 24);

        // The writer thread's own write fails on the now-dead socket, which it reports as
        // `Disconnected` - review finding 1's own requirement, where this used to be
        // silent (the plain "fire and forget" shape the original two tests here proved).
        let event =
            bounded(move || futures::executor::block_on(futures::StreamExt::next(&mut events)));
        assert!(
            matches!(event, Some(ClientEvent::Disconnected)),
            "a write failure must be reported the same way a reader-side disconnect is, got {event:?}",
        );

        let mut reader_connection = Connection::new(WireRole::Client, steg_sdk::description());
        let reply = try_read_reply(&mut app_read_side, &mut reader_connection);
        assert!(
            reply.is_none(),
            "no reply should ever be observed on an already-shut-down connection, got {reply:?}",
        );
    }

    // --- review finding 1: no socket write on the UI thread ------------------------------

    /// A peer that holds the socket open but never reads from it again - the exact
    /// condition review finding 1 named. Returns the still-open peer handle too, since
    /// dropping it would make every further write fail with a plain broken-pipe error
    /// instead of genuinely filling the kernel's own socket buffer, which is what this
    /// test (and its own control) actually needs to exercise.
    fn writer_against_a_peer_that_never_reads(
        stream: u32,
    ) -> (UnixStream, Writer, UnboundedReceiver<ClientEvent>) {
        let (server_side, app_side) = UnixStream::pair().expect("a socket pair");
        let mut writer_connection = Connection::new(WireRole::Client, steg_sdk::description());
        writer_connection.open_stream(stream);
        let (events, receiver) = futures::channel::mpsc::unbounded();
        let writer = Writer::new(
            app_side,
            writer_connection,
            stream,
            "sess/tab-1".to_owned(),
            events,
        );
        (server_side, writer, receiver)
    }

    /// Comfortably past both the kernel's own 8 KiB default socket buffer (measured on
    /// this machine, `net.local.stream.sendspace`/`recvspace`, not assumed) and this
    /// task's own `WRITER_QUEUE_CAPACITY` - enough calls that the old, direct-write
    /// implementation is guaranteed to block on this exact test, not merely "probably".
    const PROMPT_RETURN_CALLS: usize = 8192 / 64 + WRITER_QUEUE_CAPACITY + 32;

    #[test]
    fn send_input_returns_promptly_even_when_the_peer_never_reads() {
        let (_server_side, mut writer, _events) = writer_against_a_peer_that_never_reads(7);
        let bytes = vec![b'x'; 64];

        let elapsed = bounded_within(Duration::from_secs(5), move || {
            let started = Instant::now();
            for _ in 0..PROMPT_RETURN_CALLS {
                writer.send_input(bytes.clone());
            }
            started.elapsed()
        });

        assert!(
            elapsed < Duration::from_millis(500),
            "send_input must never block on the socket itself, however many calls \
             the queue behind it has already dropped",
        );
    }

    #[test]
    fn once_the_queue_is_full_behind_a_stalled_peer_the_connection_is_treated_as_lost() {
        let (_server_side, mut writer, mut events) = writer_against_a_peer_that_never_reads(7);
        let bytes = vec![b'x'; 64];
        for _ in 0..PROMPT_RETURN_CALLS {
            writer.send_input(bytes.clone());
        }

        let event =
            bounded(move || futures::executor::block_on(futures::StreamExt::next(&mut events)));
        assert!(
            matches!(event, Some(ClientEvent::Disconnected)),
            "a queue that never drains must be reported the same way a dead connection \
             is, not silently dropped with no consequence at all, got {event:?}",
        );
    }

    #[test]
    fn a_full_queue_reports_disconnected_exactly_once_even_across_many_further_calls() {
        let (_server_side, mut writer, mut events) = writer_against_a_peer_that_never_reads(7);
        let bytes = vec![b'x'; 64];
        // Several times past the bound, so `try_send` fails on many further calls, not
        // just the one that first crosses it - the shape under test is "every failed
        // `try_send` reports", not merely "the first one does".
        for _ in 0..(PROMPT_RETURN_CALLS * 4) {
            writer.send_input(bytes.clone());
        }

        let count = bounded(move || {
            let mut count = 0;
            let deadline = Instant::now() + Duration::from_millis(500);
            while Instant::now() < deadline {
                if let Ok(ClientEvent::Disconnected) = events.try_recv() {
                    count += 1;
                }
            }
            count
        });
        assert_eq!(
            count, 1,
            "a full queue must report the connection lost exactly once, not once per \
             failed try_send",
        );
    }

    #[test]
    fn stop_joins_the_writer_within_its_bound_even_while_it_is_blocked_in_a_write() {
        let (_server_side, mut writer, _events) = writer_against_a_peer_that_never_reads(7);
        let bytes = vec![b'x'; 64];
        // Enough to guarantee the writer thread's own `write_all` is genuinely stuck
        // inside the kernel's own full socket buffer, not merely queued behind it -
        // `stop`'s own job is to unblock exactly this, not only an idle `recv()`.
        for _ in 0..PROMPT_RETURN_CALLS {
            writer.send_input(bytes.clone());
        }
        // `Writer::stop` itself calls `JoinHandle::join`, which has no timeout - `bounded`
        // is what keeps this test from hanging if `shutdown()` ever stopped unblocking a
        // writer thread stuck in its own `write_all` (the mechanism this proves).
        bounded(move || writer.stop());
    }
}
