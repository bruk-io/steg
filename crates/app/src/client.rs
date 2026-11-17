//! The connection, once startup has handed it over: a dedicated thread blocks on the
//! socket and decodes with the sdk's own `Connection`; each update reaches the foreground
//! task over a channel it `.await`s - never polled (TASK-1.117's plan §1).

use std::io::{self, Read, Write as _};
use std::net::Shutdown;
use std::os::unix::net::UnixStream;
use std::thread::{self, JoinHandle};

use futures::channel::mpsc::{self, UnboundedReceiver, UnboundedSender};
use steg_sdk::{Connection, Incoming, ScreenUpdate};

/// What the reader thread hands the foreground task.
#[derive(Debug)]
pub enum ClientEvent {
    Update(ScreenUpdate),
    /// The connection ended - a clean EOF, an error, or a frame this read-only app's own
    /// design does not expect after startup (TASK-1.117's plan §1: part 1 makes no call
    /// at all once attached, so anything but a `StreamControl` on the one open stream is
    /// a protocol-level surprise, not specially recovered from).
    Disconnected,
}

/// The connection's own reader thread, kept only so [`Client::stop`] can end it - the
/// channel it feeds is returned separately by [`Client::start`] and owned directly by
/// whichever foreground task `.await`s it (nothing else needs to reach through this type
/// to use the channel).
pub struct Client {
    shutdown_handle: UnixStream,
    reader: Option<JoinHandle<()>>,
}

impl Client {
    /// Starts the reader thread, which owns `stream` and `connection` from here on - this
    /// call never touches either again. `shutdown_handle` is a clone of the same socket,
    /// kept only so [`Client::stop`] can unblock the reader's own `read()`. Returns the
    /// `Client` itself (for `stop`) and the channel's receiving end, moved into whichever
    /// foreground task `.await`s it.
    #[must_use]
    pub fn start(
        stream: UnixStream,
        connection: Connection,
        shutdown_handle: UnixStream,
    ) -> (Self, UnboundedReceiver<ClientEvent>) {
        let (sender, receiver) = mpsc::unbounded();
        let reader = thread::spawn(move || read_loop(stream, connection, &sender));
        (
            Self {
                shutdown_handle,
                reader: Some(reader),
            },
            receiver,
        )
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

/// The main thread's own, encode-only `Connection` (TASK-1.117's plan §1: "two `Connection`
/// instances, not one shared behind a lock" - the reader thread owns the decode-only one,
/// `read_loop` above; this is the other half, TASK-1.121's own writer). Writes directly over
/// its own cloned handle to the same socket - no channel, no lock, because only the main
/// thread ever builds a new request after startup.
pub(crate) struct Writer {
    handle: UnixStream,
    connection: Connection,
    stream: u32,
    /// `{session}/{tab}` - `tab.declare_size`'s own target (TASK-1.114 §2).
    tab_target: String,
}

impl Writer {
    /// `connection` must already have `stream` open (`Connection::open_stream`), mirroring
    /// what the reader's own `Connection` did with the same id - two independent sdk state
    /// objects representing the one logical attachment, seeded the same way.
    pub(crate) fn new(
        handle: UnixStream,
        connection: Connection,
        stream: u32,
        tab_target: String,
    ) -> Self {
        Self {
            handle,
            connection,
            stream,
            tab_target,
        }
    }

    /// Sends `bytes` as input on the attachment's own stream (TASK-1.121 plan §1/§3).
    /// Best-effort: a write that fails here is not specially recovered from - the same
    /// underlying socket failing will also end the reader thread's own `read()` shortly
    /// after, which is what actually moves this window to `Phase::Disconnected`.
    pub(crate) fn send_input(&mut self, bytes: Vec<u8>) {
        if let Ok(frame) = self.connection.send_data(self.stream, &bytes) {
            let _ = self.handle.write_all(&frame);
        }
    }

    /// Declares `columns`/`rows` for this attachment's own tab (TASK-1.121 plan §4). The
    /// reply (`Response::Ok(Null)` or an error) lands on stream 0, decoded by the *reader's*
    /// own `Connection`, not this one - `client.rs`'s own `drain` tolerates an
    /// `Incoming::Reply` for exactly this reason, and this app acts the same way whether it
    /// arrives or not: there is nothing to do differently on a `BadRequest` here, since
    /// `size::grid_size`'s own floor already keeps a zero size from ever being sent.
    pub(crate) fn declare_size(&mut self, columns: u16, rows: u16) {
        if let Ok((_id, frame)) =
            self.connection
                .declare_size(Some(&self.tab_target), columns, rows)
        {
            let _ = self.handle.write_all(&frame);
        }
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
            Incoming::StreamControl { payload, .. } => {
                match serde_json::from_slice::<ScreenUpdate>(&payload) {
                    Ok(update) => update,
                    Err(_) => return false,
                }
            }
            // TASK-1.121's own writer (a second `Connection` instance, `Writer` above)
            // calls `tab.declare_size` after startup, whose reply lands here, on the
            // reader's own decode-only `Connection` - ignored, not correlated to anything
            // (this app acts the same way whether it succeeds or not, `Writer::declare_
            // size`'s own doc), and not the protocol-level surprise it would have been
            // before this task, when nothing on this connection ever called again.
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
    use std::time::Duration;

    use steg_sdk::{Connection, Description, Frame, ScreenUpdate, WireRole, encode_frame};

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
        let (client, receiver) = Client::start(app_side, connection, shutdown_handle);
        (server_side, client, receiver)
    }

    /// `JoinHandle::join` has no timeout, and neither does `.await`ing an `mpsc` receiver
    /// with nothing ever sent - so every test below that could hang if this task's own
    /// mechanism were broken instead runs the blocking call on its own thread and waits on
    /// a plain, timeout-capable `std::sync::mpsc` channel from the test's own thread: a
    /// broken mechanism fails the assertion, not the test process.
    fn bounded<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> T {
        let (sender, receiver) = mpsc::channel();
        thread::spawn(move || {
            let _ = sender.send(f());
        });
        receiver
            .recv_timeout(Duration::from_secs(2))
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

    #[test]
    fn stop_unblocks_and_joins_the_reader_thread_within_the_tests_own_bound() {
        let (_server_side, mut client, _receiver) = start_client();
        // `Client::stop` itself calls `JoinHandle::join`, which has no timeout - `bounded`
        // is what keeps this test from hanging if `shutdown()` ever stopped unblocking
        // the reader's own `read()` (the mechanism this proves).
        bounded(move || client.stop());
    }
}
