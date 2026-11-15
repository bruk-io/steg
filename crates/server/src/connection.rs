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

use steg_sdk::{
    ClientId, Connection, Frame, Handshake, Incoming, PROTOCOL_VERSION, Refusal, WireRole as Role,
    description, encode_frame,
};

/// A connection's liveness, from `run`'s accept loop (the seam part B's
/// idle checker will consume - this task has no idle rule of its own to
/// feed it to). Sent `Connected` the instant `accept()` returns the
/// connection, before any byte of it is read - counting from handshake
/// -acceptance instead would let a real client mid-handshake go
/// uncounted at the exact moment an idle rule might otherwise decide
/// there is no one there (part B's problem to get right, not this task's
/// to create by counting too late).
///
/// **A caveat for part B's idle rule, not this task's to fix (a review
/// of this task found it, named here rather than hidden):**
/// `arbiter::probe`'s own liveness check - a plain `UnixStream::connect`
/// against a running server, used only to tell "a live server of ours"
/// apart from "a stale file" during arbitration - is indistinguishable
/// here from a real client arriving and leaving: `run`'s accept loop
/// has no way to know a connection was only ever a probe, so it reports
/// `Connected` then, almost immediately, `Disconnected` (the probe
/// closes its side right after connecting, never sending a hello) the
/// same as it would for any client that connected and left before
/// completing a handshake. A caller counting this seam to decide
/// "is anyone here" - as TASK-1.77's idle checker will - would see a
/// probe exactly like a very short-lived real client. The server
/// cannot currently tell the two apart at all: a probe's connection
/// carries nothing that marks it as one (no hello, no special byte),
/// so there is no signal here to filter on even if `run` wanted to.
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
/// nothing decoded yet. An oversized declared length is caught here too
/// (`Decoder::next_frame`'s own `FrameError::TooLarge`, surfaced as
/// `Incoming::Malformed`) - from only the 4-byte length prefix, with
/// nothing allocated for the declared body, so a hostile client sees
/// its refusal promptly rather than this waiting out the handshake
/// timeout.
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

/// What the first thing read off a connection means for the handshake.
enum FirstFrame {
    /// The read timed out, or the peer closed, before a complete frame
    /// arrived - including a client that sent only part of a frame and
    /// then stalled (`Decoder::next_frame` returns "need more bytes"
    /// for that, indistinguishable at this level from having sent
    /// nothing at all). AC #4's first clause: closed with no reply -
    /// there is nothing here to name a reason from.
    NoCompleteFrame,
    /// A decoded `Hello` frame, forwarded to the sdk's own
    /// `Connection::accept_or_refuse` unchanged - every bad-but
    /// -JSON-shaped payload (wrong magic, missing `protocol`, a
    /// mismatched version) is already that function's own decision via
    /// `check_hello`, proved by the sdk's own tests.
    Hello(Vec<u8>),
    /// Anything else decoded as the very first frame: the framing
    /// itself failed to decode (`Incoming::Malformed`, including the
    /// oversized-length case above), or it decoded fine but was not a
    /// hello at all (a control request, reply, event, or data frame
    /// arriving first). decision-2 section 7's own contract covers
    /// both under one umbrella ("a first frame that is not JSON, has
    /// the wrong magic, is a control frame instead of a hello, or is
    /// missing protocol") - the two cases `check_hello` already decides
    /// (wrong magic, missing protocol) arrive as `Hello` above with a
    /// bad payload; these are the two it never sees at all, because
    /// there is no hello payload to hand it. The `String` is a
    /// human-readable reason, carried into the same
    /// `Refusal::Malformed` naming the server's version that
    /// `check_hello` would have produced had it been reachable.
    NotAHello(String),
}

/// Describes why a decoded [`Incoming`] other than `Hello` is not a
/// hello, for [`FirstFrame::NotAHello`]'s reason. `Hello` itself is
/// handled by the caller before this is reached.
fn describe_non_hello(incoming: &Incoming) -> String {
    match incoming {
        Incoming::Hello(_) => unreachable!("Hello is handled before this is called"),
        Incoming::Malformed(reason) => format!("the first frame did not decode: {reason}"),
        Incoming::Request { .. } => "the first frame was a request, not a hello".to_owned(),
        Incoming::Reply { .. } => "the first frame was a reply, not a hello".to_owned(),
        Incoming::Event { .. } => "the first frame was an event, not a hello".to_owned(),
        Incoming::UnknownEvent { .. } => {
            "the first frame was an unknown event, not a hello".to_owned()
        }
        Incoming::StreamControl { .. } => {
            "the first frame was a stream-control frame, not a hello".to_owned()
        }
        Incoming::Data { .. } => "the first frame carried stream data, not a hello".to_owned(),
        Incoming::UnknownStream { .. } => {
            "the first frame was for a stream that was never opened, not a hello".to_owned()
        }
    }
}

/// Sets the read timeout, reads the first frame, and classifies it.
fn first_frame(stream: &mut UnixStream, handshake_timeout: Duration) -> FirstFrame {
    if stream.set_read_timeout(Some(handshake_timeout)).is_err() {
        return FirstFrame::NoCompleteFrame;
    }
    let mut connection = Connection::new(Role::Server, description());
    match read_one(stream, &mut connection) {
        None => FirstFrame::NoCompleteFrame,
        Some(Incoming::Hello(payload)) => FirstFrame::Hello(payload),
        Some(other) => FirstFrame::NotAHello(describe_non_hello(&other)),
    }
}

/// Builds the bytes of a malformed-hello refusal for a first frame that
/// never reached the sdk's own `check_hello` at all (there was no hello
/// payload to hand it) - the same shape `Connection::accept_or_refuse`
/// would have written had `check_hello` decided this itself
/// (`Refusal::Malformed`, naming the server's version; R39's "both
/// versions named" has no client version to name here, which is
/// `Refusal::Malformed`'s own documented case, not something this adds).
/// Built from the sdk's own public `Refusal`/`Frame`/`encode_frame` -
/// nothing added to the sdk for this.
fn malformed_hello_reply(reason: String) -> Vec<u8> {
    let refusal = Refusal::Malformed {
        reason,
        server: PROTOCOL_VERSION,
    };
    let payload =
        serde_json::to_vec(&refusal.to_wire()).expect("Refused always serialises to JSON");
    encode_frame(&Frame::hello(payload)).expect("a refusal fits under the frame cap")
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

    match first_frame(&mut stream, handshake_timeout) {
        FirstFrame::NoCompleteFrame => {
            // AC #4's first clause: closed with no reply - there is
            // nothing decoded to answer.
        }
        FirstFrame::Hello(hello_payload) => {
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
                // Accepted: no longer a handshake to time out on - a
                // client that has finished it and then sits idle is
                // never closed for that (AC #4's second clause).
                let _ = stream.set_read_timeout(None);
                let mut sink = [0_u8; 4096];
                // This task reads nothing past the hello: routing a
                // further request is TASK-1.45's. Reading to
                // end-of-file (or an error) is only ever this
                // connection closing, which is all this task reports.
                while matches!(stream.read(&mut sink), Ok(n) if n > 0) {}
            }
        }
        FirstFrame::NotAHello(reason) => {
            use std::io::Write;
            let reply = malformed_hello_reply(reason);
            let _ = stream.write_all(&reply);
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
