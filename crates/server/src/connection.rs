//! The per-connection thread: the handshake, with a real timeout (AC #4),
//! reusing `steg_sdk::wire::Connection` exactly as TASK-1.70 built it -
//! nothing here adds a line to the sdk. Once accepted, this task holds a
//! connection open only to report its liveness; routing a request past
//! the hello is TASK-1.45's named scope, not this one's.

use std::io::Read;
use std::os::unix::net::UnixStream;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::Sender;
use std::time::Duration;

use steg_sdk::{ClientId, Connection, Handshake, Incoming, WireRole as Role, description};

/// A connection's liveness, from `run`'s accept loop (the seam part B's
/// idle checker will consume - this task has no idle rule of its own to
/// feed it to). Sent `Connected` the instant `accept()` returns the
/// connection, before any byte of it is read - counting from handshake
/// -acceptance instead would let a real client mid-handshake go
/// uncounted at the exact moment an idle rule might otherwise decide
/// there is no one there (part B's problem to get right, not this task's
/// to create by counting too late).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionEvent {
    Connected,
    Disconnected,
}

/// A client id local to this one server's lifetime, minted sequentially
/// as connections are accepted. Not the kernel's own client model
/// (`steg_kernel`'s `Ids::next_client` is private to that crate, and in
/// any case there is no `Tree` here to mint one through - wiring a real
/// session/client model to this connection is TASK-1.45's job): this is
/// only ever used for the handshake's own `ServerHello.client` field,
/// which the sdk's wire protocol requires but this task does not
/// otherwise interpret.
fn next_local_client_id() -> ClientId {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    ClientId::from_raw(NEXT.fetch_add(1, Ordering::Relaxed))
}

/// Reads frames off `stream` until a complete one decodes, or the read
/// times out / the stream closes. `connection` owns the decode buffer;
/// `stream` is read into a plain, short-lived local buffer each call -
/// who owns which buffer, stated once, the same convention
/// `steg_sdk::wire`'s own doc already states for its side.
///
/// Returns `None` on a timed-out or failed read, or a clean close with
/// nothing decoded yet.
fn read_one(stream: &mut UnixStream, connection: &mut Connection) -> Option<Incoming> {
    loop {
        if let Some(incoming) = connection.poll() {
            return Some(incoming);
        }
        let mut buf = [0_u8; 4096];
        match stream.read(&mut buf) {
            Ok(0) => return None, // a clean close with no complete frame
            Ok(n) => connection.receive(&buf[..n]),
            Err(_) => return None, // a timed-out or otherwise failed read
        }
    }
}

/// The per-connection thread body: the handshake, then holding the
/// connection open - reporting only that it is alive - until it closes.
/// Takes ownership of `stream` and runs until that connection ends, on
/// whichever thread calls it (`listen::run` spawns one per accepted
/// connection).
pub fn handle(
    mut stream: UnixStream,
    handshake_timeout: Duration,
    events: Option<Sender<ConnectionEvent>>,
) {
    if let Some(sender) = events.as_ref() {
        let _ = sender.send(ConnectionEvent::Connected);
    }

    let outcome = (|| {
        stream.set_read_timeout(Some(handshake_timeout)).ok()?;
        let mut connection = Connection::new(Role::Server, description());
        match read_one(&mut stream, &mut connection)? {
            Incoming::Hello(payload) => Some(payload),
            // Anything else as the very first frame is not a hello at
            // all - closed the same way a timed-out read is: no reply,
            // since the sdk's own `check_hello` is what names a reason
            // when the payload at least parses as JSON, and this is a
            // case that never reaches it.
            Incoming::Request { .. }
            | Incoming::Reply { .. }
            | Incoming::Event { .. }
            | Incoming::UnknownEvent { .. }
            | Incoming::StreamControl { .. }
            | Incoming::Data { .. }
            | Incoming::UnknownStream { .. }
            | Incoming::Malformed(_) => None,
        }
    })();

    if let Some(hello_payload) = outcome {
        let server_version = concat!("steg/", env!("CARGO_PKG_VERSION"));
        let api_version = description().api_version;
        let api = format!("{}.{}", api_version.major, api_version.minor);
        let (handshake, reply) = Connection::accept_or_refuse(
            &hello_payload,
            server_version,
            &api,
            next_local_client_id(),
        );
        use std::io::Write;
        let _ = stream.write_all(&reply);
        if matches!(handshake, Handshake::Accept { .. }) {
            // Accepted: no longer a handshake to time out on - a client
            // that has finished it and then sits idle is never closed
            // for that (AC #4's second clause).
            let _ = stream.set_read_timeout(None);
            let mut sink = [0_u8; 4096];
            // This task reads nothing past the hello: routing a further
            // request is TASK-1.45's. Reading to end-of-file (or an
            // error) is only ever this connection closing, which is all
            // this task reports.
            while matches!(stream.read(&mut sink), Ok(n) if n > 0) {}
        }
    }

    // `run` (`listen.rs`) holds a `try_clone`d duplicate of this same
    // local socket endpoint for as long as this connection is tracked,
    // so this thread's own fd closing on return (a plain `close()`,
    // what simply dropping `stream` would do) is not enough on its own
    // to end the connection - a `close()` only drops this fd's own
    // reference and does not notify the peer while the duplicate stays
    // open. `shutdown` acts on the socket itself regardless of how many
    // local duplicates exist, so it is what actually makes a timed-out
    // or refused peer see its connection end promptly, on every exit
    // path out of this function, whether or not `run`'s own clone has
    // been reaped yet (found necessary while building this: without
    // it, a stalled handshake that times out with no later connection
    // arriving to trigger a reap pass left the peer's read blocked
    // indefinitely instead of closed).
    let _ = stream.shutdown(std::net::Shutdown::Both);

    if let Some(events) = events {
        let _ = events.send(ConnectionEvent::Disconnected);
    }
}
