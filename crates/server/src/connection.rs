//! The per-connection thread: the handshake, with a real timeout (AC #4,
//! TASK-1.44), then the reader/writer shape that routes a request to the
//! API (TASK-1.45 §1) - reusing `steg_sdk::wire::Connection` exactly as
//! TASK-1.70 built it, and `crate::outbound::Outbound` exactly as this
//! task's own earlier commit built it. Nothing here adds a line to the
//! sdk: the one `match` in `run_post_handshake` leans entirely on the
//! sdk's own `Incoming` classification.

use std::io::Read;
use std::os::unix::net::UnixStream;
use std::sync::Arc;
use std::sync::mpsc::Sender;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use steg_sdk::{
    ClientId, Connection, Frame, Handshake, Incoming, PROTOCOL_VERSION, Refusal, WireRole as Role,
    check_hello, description, encode_frame,
};

use crate::dispatch::Context;
use crate::outbound::{Outbound, spawn_writer};
use crate::runtime::Runtime;

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

/// How long is left before `deadline`, as of `now` - `None` once `now`
/// has reached or passed it. Pure: both instants are given, not read
/// from the clock itself, so this is tested with literal `Instant`s
/// (a captured base plus `Duration` offsets) rather than real timing or
/// sleeping.
///
/// Returns `None` for exactly zero remaining too, not only negative:
/// `UnixStream::set_read_timeout(Some(Duration::ZERO))` itself errors
/// on this platform (`cannot set a 0 duration timeout`, confirmed by a
/// standalone two-line repro, not assumed) - "no time left" and
/// "a non-zero instant away" must stay distinguishable to this
/// function's own caller, which needs to tell "already expired" apart
/// from "about to expire" without itself risking that error.
fn time_remaining(deadline: Instant, now: Instant) -> Option<Duration> {
    let remaining = deadline.checked_duration_since(now)?;
    if remaining.is_zero() {
        None
    } else {
        Some(remaining)
    }
}

/// Reads frames off `stream` until a complete one decodes, the deadline
/// passes, or the stream closes. `connection` owns the decode buffer;
/// `stream` is read into a plain, short-lived local buffer each call -
/// who owns which buffer, stated once, the same convention
/// `steg_sdk::wire`'s own doc already states for its side.
///
/// **One deadline for the whole handshake, not a per-read timeout that
/// restarts on every byte.** A review of this task found the earlier
/// version set `stream`'s read timeout once, up front, and never
/// again - which bounds each individual `read()` call, not the time
/// this function spends overall: a client sending one byte just inside
/// every timeout window, never completing a frame, kept a connection
/// (and its thread) alive for as long as it liked, each successful
/// partial read resetting the per-call clock. `deadline` is computed
/// once by the caller and carried through every iteration here
/// instead: each `read()` is given only [`time_remaining`] until it,
/// shrinking every time this loop goes around, and a `read()` is never
/// attempted at all once that reaches zero - closed the same way a
/// genuinely silent client already was, which is the correct outcome
/// (AC #4's first clause: a connection that never completes its
/// handshake is closed after a timeout - "never completes" does not
/// stop meaning that just because bytes kept arriving one at a time).
/// A hello that legitimately arrives in many small pieces still
/// succeeds, as long as the last of them lands before `deadline` -
/// nothing here penalises a slow but honest connection, only one that
/// never finishes.
///
/// Returns `None` on an expired deadline, a timed-out or failed read,
/// or a clean close with nothing decoded yet. An oversized declared
/// length is caught here too (`Decoder::next_frame`'s own
/// `FrameError::TooLarge`, surfaced as `Incoming::Malformed`) - from
/// only the 4-byte length prefix, with nothing allocated for the
/// declared body, so a hostile client sees its refusal promptly rather
/// than this waiting out the deadline; `connection.poll()` is checked
/// before the deadline on every iteration, so a frame that has already
/// fully arrived (even right at the deadline) is still decoded and
/// handed back, never silently dropped by a deadline check that ran
/// first.
fn read_one(
    stream: &mut UnixStream,
    connection: &mut Connection,
    deadline: Instant,
) -> Option<Incoming> {
    loop {
        if let Some(incoming) = connection.poll() {
            return Some(incoming);
        }
        let remaining = time_remaining(deadline, Instant::now())?;
        if stream.set_read_timeout(Some(remaining)).is_err() {
            return None;
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

/// Computes the one deadline the whole handshake is held to (`Instant::now`
/// read once, here, at the moment this connection starts its handshake -
/// not re-read per read the way the timeout alone used to be), reads
/// the first frame, and classifies it.
///
/// `connection` is the caller's own, carried through the whole
/// connection's life (TASK-1.45 §1), not a local, throwaway one: a client
/// that pipelines its first request right behind its hello, in the same
/// `read()`, would otherwise have those bytes decoded into a `Connection`
/// that is discarded the moment this function returns, losing a frame
/// that already fully arrived. Using the same `Connection` for the
/// handshake and everything after it means nothing already buffered is
/// ever lost at that boundary.
fn first_frame(
    stream: &mut UnixStream,
    connection: &mut Connection,
    handshake_timeout: Duration,
) -> FirstFrame {
    let deadline = Instant::now() + handshake_timeout;
    match read_one(stream, connection, deadline) {
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

/// Joins the writer thread and disconnects the minted client on every
/// path out of [`run_post_handshake`] - an ordinary close, a hostile
/// input closing the connection, or a panic unwinding through a call to
/// [`Runtime::handle`] (a bug in a request handler). Rust runs a local's
/// `Drop` on unwind the same as on an ordinary return, so building this
/// once, right after the writer is spawned and the client is minted, and
/// holding it for the rest of the function, is what makes every exit
/// path still clean up - even one this function's own code never
/// explicitly names - rather than leaving the writer thread blocked
/// forever and the client stuck in the tree.
///
/// **`shutdown_handle.shutdown(Both)` runs before `writer.join()`, not
/// just `outbound.close()`** - found necessary while testing this: a
/// client that keeps sending requests while never reading any reply
/// fills the OS socket's own send buffer, not only `Outbound`'s queue,
/// so the writer thread can be blocked *inside* `write_all` itself, on a
/// buffer that will never drain - a case `outbound.close()`'s own
/// condvar `notify` does nothing for, since the writer is not waiting on
/// it. `shutdown`, called on a cloned handle to the same underlying
/// socket (the same mechanism `listen.rs`'s own `shutdown_handle` already
/// relies on), forces that blocked write to fail promptly instead,
/// exactly the way it already forces a blocked *read* to see `Ok(0)`.
struct ConnectionCleanup<'a> {
    outbound: Arc<Outbound>,
    writer: Option<JoinHandle<()>>,
    shutdown_handle: UnixStream,
    runtime: &'a Runtime,
    client_id: ClientId,
}

impl Drop for ConnectionCleanup<'_> {
    fn drop(&mut self) {
        self.outbound.close();
        let _ = self.shutdown_handle.shutdown(std::net::Shutdown::Both);
        if let Some(writer) = self.writer.take() {
            let _ = writer.join();
        }
        self.runtime.disconnect_client(self.client_id);
    }
}

/// The reader loop a connection that completed its handshake runs for the
/// rest of its life (TASK-1.45 §1): drains whatever `connection` already
/// has buffered (possibly a request pipelined right behind the hello,
/// in the same read as `first_frame`'s own), then reads more. One
/// `match`, leaning entirely on the sdk's own `Incoming` classification -
/// a `Request` is answered through `runtime`; every other variant,
/// spelled out rather than a wildcard arm (so a new one the sdk adds
/// someday fails this build until it is decided, not silently swallowed),
/// is a protocol violation post-handshake and closes the connection
/// (TASK-1.45 §3 - there is no id to answer most of them with, so closing
/// is the only coherent, uniform response).
///
/// No per-frame deadline exists here, and none is added: AC #4's own
/// second clause already promises a completed handshake is never closed
/// for being idle, and a client stalled mid-frame is, at this level,
/// indistinguishable from one that is legitimately idle - both leave
/// `connection.poll()` saying "need more bytes" forever. This is this
/// task's own chosen design (TASK-1.45 §3), not a gap handed to another
/// task.
fn run_post_handshake(
    stream: &mut UnixStream,
    connection: &mut Connection,
    runtime: &Runtime,
    client_id: ClientId,
) {
    // Three independent handles to the one underlying socket: `stream`
    // itself (this function's own reads), `write_half` (moved into the
    // writer thread), and `shutdown_handle` (kept by the cleanup guard
    // solely to force a stuck writer to fail promptly - see its own doc).
    let (write_half, shutdown_handle) = match (stream.try_clone(), stream.try_clone()) {
        (Ok(write_half), Ok(shutdown_handle)) => (write_half, shutdown_handle),
        _ => {
            // No write half to hand a writer thread at all - nothing was
            // ever spawned, so there is nothing to join; the client this
            // function was given still has to be disconnected.
            runtime.disconnect_client(client_id);
            return;
        }
    };
    let outbound = Arc::new(Outbound::new());
    let writer = spawn_writer(Arc::clone(&outbound), write_half);
    let _cleanup = ConnectionCleanup {
        outbound: Arc::clone(&outbound),
        writer: Some(writer),
        shutdown_handle,
        runtime,
        client_id,
    };

    let context = Context::none();
    let description = description();

    loop {
        loop {
            match connection.poll() {
                Some(Incoming::Request { id, request }) => {
                    let response = runtime.handle(&description, &context, &request);
                    let bytes = Connection::reply(id, response);
                    if outbound.enqueue_control(bytes).is_err() {
                        // The queue is closed or full (TASK-1.45 §3: a
                        // client that floods requests and never reads any
                        // reply) - `_cleanup`'s `Drop` runs the rest.
                        return;
                    }
                }
                Some(
                    Incoming::Hello(_)
                    | Incoming::Reply { .. }
                    | Incoming::Event { .. }
                    | Incoming::UnknownEvent { .. }
                    | Incoming::StreamControl { .. }
                    | Incoming::Data { .. }
                    | Incoming::UnknownStream { .. }
                    | Incoming::Malformed(_),
                ) => return,
                None => break, // need more bytes; read some, below
            }
        }
        let mut buf = [0_u8; 4096];
        match stream.read(&mut buf) {
            Ok(0) | Err(_) => return,
            Ok(n) => connection.receive(&buf[..n]),
        }
    }
}

/// The per-connection thread body: the handshake, then (once accepted)
/// routing every request to `runtime` until the connection ends
/// (TASK-1.45 §1). Takes ownership of `stream` and runs until that
/// connection ends, on whichever thread calls it (`listen::run` spawns
/// one per accepted connection).
pub fn handle(
    mut stream: UnixStream,
    handshake_timeout: Duration,
    events: Option<Sender<ConnectionEvent>>,
    runtime: &Runtime,
) {
    if let Some(sender) = events.as_ref() {
        let _ = sender.send(ConnectionEvent::Connected);
    }

    let mut connection = Connection::new(Role::Server, description());

    match first_frame(&mut stream, &mut connection, handshake_timeout) {
        FirstFrame::NoCompleteFrame => {
            // AC #4's first clause: closed with no reply - there is
            // nothing decoded to answer.
        }
        FirstFrame::Hello(hello_payload) => {
            let server_version = concat!("steg/", env!("CARGO_PKG_VERSION"));
            let api_version = description().api_version;
            let api = format!("{}.{}", api_version.major, api_version.minor);

            // Classified here, with the sdk's own pure `check_hello`,
            // before `Connection::accept_or_refuse` makes the same
            // decision again internally to build the reply (harmless -
            // deterministic, no I/O, no second definition of anything):
            // a real, tree-tracked client (TASK-1.45 §1) is minted only
            // for a handshake that will actually be accepted, never for
            // one about to be refused. The id for the refused case is
            // never read: `accept_or_refuse` only ever embeds it in the
            // `Accept` branch of its own reply.
            let client_id = match check_hello(&hello_payload, PROTOCOL_VERSION) {
                Handshake::Accept { kind, label, .. } => runtime.connect_client(kind, label),
                Handshake::Refuse(_) => ClientId::from_raw(0),
            };
            let (handshake, reply) =
                Connection::accept_or_refuse(&hello_payload, server_version, &api, client_id);
            use std::io::Write;
            let _ = stream.write_all(&reply);
            if matches!(handshake, Handshake::Accept { .. }) {
                // Accepted: no longer a handshake to time out on - a
                // client that has finished it and then sits idle is
                // never closed for that (AC #4's second clause).
                let _ = stream.set_read_timeout(None);
                run_post_handshake(&mut stream, &mut connection, runtime, client_id);
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

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::time_remaining;

    /// `Instant::now()` is read exactly once here, as a base - every
    /// instant compared against it below is that base plus a literal
    /// `Duration` offset, never a second real clock read or a sleep.
    /// `time_remaining` itself takes both instants as plain arguments,
    /// so this is testing the arithmetic, not real timing.
    #[test]
    fn well_before_the_deadline_leaves_most_of_it_remaining() {
        let base = Instant::now();
        let deadline = base + Duration::from_secs(10);
        let now = base + Duration::from_secs(1);
        assert_eq!(time_remaining(deadline, now), Some(Duration::from_secs(9)));
    }

    #[test]
    fn one_nanosecond_before_the_deadline_is_still_some() {
        let base = Instant::now();
        let deadline = base + Duration::from_secs(10);
        let now = deadline - Duration::from_nanos(1);
        assert_eq!(time_remaining(deadline, now), Some(Duration::from_nanos(1)));
    }

    /// Exactly zero remaining is `None`, not `Some(Duration::ZERO)` -
    /// `UnixStream::set_read_timeout(Some(Duration::ZERO))` itself
    /// errors on this platform (confirmed by a standalone repro, not
    /// assumed), so this function's own caller must never be handed a
    /// zero duration to pass to it.
    #[test]
    fn exactly_at_the_deadline_is_none() {
        let base = Instant::now();
        let deadline = base + Duration::from_secs(10);
        assert_eq!(time_remaining(deadline, deadline), None);
    }

    #[test]
    fn past_the_deadline_is_none() {
        let base = Instant::now();
        let deadline = base + Duration::from_secs(10);
        let now = deadline + Duration::from_secs(1);
        assert_eq!(time_remaining(deadline, now), None);
    }
}
