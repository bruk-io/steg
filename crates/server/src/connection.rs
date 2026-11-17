//! The per-connection thread: the handshake, with a real timeout (AC #4,
//! TASK-1.44), then the reader/writer shape that routes a request to the
//! API (TASK-1.45 §1) - reusing `steg_sdk::wire::Connection` exactly as
//! TASK-1.70 built it, and `crate::outbound::Outbound` exactly as this
//! task's own earlier commit built it. Nothing here adds a line to the
//! sdk: the one `match` in `run_post_handshake` leans entirely on the
//! sdk's own `Incoming` classification.

use std::cell::RefCell;
use std::collections::HashMap;
use std::io::Read;
use std::os::unix::net::UnixStream;
use std::sync::Arc;
use std::sync::mpsc::Sender;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use steg_sdk::{
    ClientId, Connection, Description, Frame, Handshake, Incoming, PROTOCOL_VERSION, PaneId,
    Refusal, Request, Response, WireRole as Role, check_hello, description, encode_frame,
};

use crate::dispatch::{Context, bad_request, param_i64, resolve_api_call};
use crate::outbound::{Outbound, spawn_writer};
use crate::runtime::Runtime;

/// A connection's liveness, from `run`'s accept loop - and, as of
/// TASK-1.86, one thing that is not about any one connection at all
/// (`SessionCountChanged`, below). Kept as one enum rather than each
/// consumer inventing its own wrapper around it: this channel is the
/// server's own internal "something the idle checker should re-evaluate"
/// stream, not strictly a per-connection one, and TASK-1.98's own
/// addition (`AcceptRetried`, below - an accept retried after a
/// recoverable error) is the same kind of thing - naming the enum after
/// its first, narrower use rather than widening the name too is the
/// cheaper trade: `ConnectionEvent`
/// reads oddly for a variant like `SessionCountChanged`, but renaming it
/// would touch every existing sender and receiver across this crate for
/// a cosmetic gain, which isn't worth it against two variants that
/// genuinely aren't about one connection. Said here, and in this task's
/// own notes, rather than silently renamed or silently left unexplained.
///
/// `Connected` is sent the instant `accept()` returns the connection,
/// before any byte of it is read - counting from handshake-acceptance
/// instead would let a real client mid-handshake go uncounted at the
/// exact moment an idle rule might otherwise decide there is no one
/// there.
///
/// **A caveat for the idle checker (TASK-1.86), found by an earlier
/// review and still true, named here rather than hidden:**
/// `arbiter::probe`'s own liveness check - a plain `UnixStream::connect`
/// against a running server, used only to tell "a live server of ours"
/// apart from "a stale file" during arbitration - is indistinguishable
/// here from a real client arriving and leaving: `run`'s accept loop
/// has no way to know a connection was only ever a probe, so it reports
/// `Connected` then, almost immediately, `Disconnected` (the probe
/// closes its side right after connecting, never sending a hello) the
/// same as it would for any client that connected and left before
/// completing a handshake. A caller counting this seam to decide
/// "is anyone here" would see a probe exactly like a very short-lived
/// real client. The server cannot currently tell the two apart at all:
/// a probe's connection carries nothing that marks it as one (no hello,
/// no special byte), so there is no signal here to filter on even if
/// `run` wanted to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionEvent {
    Connected,
    Disconnected,
    /// A pane's exit just closed a session with no client connected to
    /// it at all (doc-15 R94/R97's own cascade: a plain shell pane
    /// closes when its program exits, and that can close its tab and
    /// its session in turn) - carries no id, since a caller that cares
    /// always re-reads the live count (`Runtime::session_counter`),
    /// never caches one against this event. Sent by `Runtime`, from the
    /// drain thread that already applies a pane's exit to the tree -
    /// never by `connection::handle`, which has no connection to send
    /// it from in this one case (TASK-1.86's own plan, "Why only the
    /// `PaneEvent::Exited` arm").
    SessionCountChanged,
    /// `run`'s accept loop hit an `accept()`/`try_clone()` error it
    /// classified as safe to retry (TASK-1.98 AC #1/#4), sent once per
    /// retry, before the loop's own backoff sleep. Not a connection at
    /// all - no stream exists on this path - so it is its own variant
    /// rather than folded into `Connected`/`Disconnected`, which both
    /// name a real one. The idle checker's own catch-all (TASK-1.86,
    /// `idle::run_checker_with_observer`'s match) treats this the same
    /// as any other variant it does not specifically care about: it
    /// wakes the loop to re-evaluate, never counted as a client arriving
    /// (confirmed by this file's own `a_recoverable_accept_retry_does_
    /// not_count_as_a_client_arriving` test, below).
    AcceptRetried(crate::listen::RecoverableAcceptError),
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
    /// Every pane this connection still attaches to, by its own
    /// worker-minted [`crate::pane::AttachmentId`] and connection-local
    /// stream id (TASK-1.47; the id, TASK-1.114 - see `AttachmentId`'s
    /// own doc for why the bare stream number is not a safe key once a
    /// second connection can attach the same pane), swept below so a
    /// connection that ends without calling `pane.detach` for all of them
    /// does not leak a dead attachment in a pane's own fan-out list
    /// forever. `Tree::disconnect_client` (below) already removes this
    /// connection's whole kernel-level `Client` row in one step, viewing
    /// list included - there is nothing left to release there; only each
    /// pane's own worker thread, which the kernel knows nothing about,
    /// still needs telling.
    pane_streams: &'a RefCell<HashMap<PaneId, (crate::pane::AttachmentId, u32)>>,
    /// TASK-1.86 review finding: a review found this guard already runs
    /// on every path out of `run_post_handshake` (a normal return, an
    /// early return, *or* an unwind) but did not hold the sender that
    /// reports `ConnectionEvent::Disconnected` - `handle`'s own plain
    /// statement at its very end, doing that, is skipped by an unwind,
    /// since a panic never reaches a line below it. A panic in a request
    /// handler (`Runtime::handle`, dispatched from this guard's own
    /// scope) left the idle checker's live-client count one too high for
    /// the rest of the server's life - R99 silently never fires again.
    /// Fixed by giving this guard the sender too, so the one send that
    /// matters is on the one path that already runs unconditionally.
    events: Option<Sender<ConnectionEvent>>,
}

impl Drop for ConnectionCleanup<'_> {
    fn drop(&mut self) {
        self.outbound.close();
        let _ = self.shutdown_handle.shutdown(std::net::Shutdown::Both);
        if let Some(writer) = self.writer.take() {
            let _ = writer.join();
        }
        for (&pane, &(attachment_id, _stream)) in self.pane_streams.borrow().iter() {
            self.runtime.forget_attachment(pane, attachment_id);
        }
        // Before `disconnect_client`: a subscriber entry keyed by this
        // same `client_id` must not linger past the id itself being
        // freed for reuse (TASK-1.46 §7) - deregistering first, while the
        // id is still unambiguously this connection's own, is what keeps
        // that ordering safe regardless of which happens to run first in
        // practice.
        self.runtime.deregister_subscriber(self.client_id);
        self.runtime.disconnect_client(self.client_id);
        // Last, after this guard's own pre-existing duties keep their
        // exact order (unchanged above) - `Drop` runs exactly once, so
        // this needs no idempotency guard of its own, only the ordering
        // the review asked for: after `Connected` (trivially true - this
        // guard cannot exist before `handle`'s own first line sends it)
        // and after this struct's other cleanup, not before it.
        if let Some(events) = &self.events {
            let _ = events.send(ConnectionEvent::Disconnected);
        }
    }
}

/// Which pane a `pane.attach`'ed stream belongs to - the reverse of
/// `pane_streams` (TASK-1.114), needed to route an `Incoming::Data` frame
/// (opaque input bytes, never a decoded request) back to the pane it
/// belongs to: unlike every other message on a connection, input names no
/// method and carries no target string to resolve, only the stream id the
/// frame itself arrived on. Kept as its own `HashMap` rather than one
/// bidirectional map: two inserts/removes at the same two call sites
/// (`pane.attach` succeeding, `pane.detach`'s `Some(stream)` arm) is
/// cheaper than a new dependency for this one, small crate. Not read by
/// `ConnectionCleanup::drop` - that sweep only ever needs pane -> stream,
/// which `pane_streams` already gives it.
type StreamPanes = RefCell<HashMap<u32, PaneId>>;

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
    events: Option<Sender<ConnectionEvent>>,
) {
    // Four independent handles to the one underlying socket: `stream`
    // itself (this function's own reads), `write_half` (moved into the
    // writer thread), `shutdown_handle` (kept by the cleanup guard solely
    // to force a stuck writer to fail promptly - see its own doc), and
    // `subscriber_shutdown` (kept by `Runtime`'s own subscriber registry,
    // TASK-1.46 §3/§7 - a slow subscriber is closed, not resynced: a
    // broadcast that cannot be queued shuts this connection's socket down
    // directly, from whichever thread is broadcasting, since nothing else
    // can make this connection's own blocked read return without one).
    let (write_half, shutdown_handle, subscriber_shutdown) =
        match (stream.try_clone(), stream.try_clone(), stream.try_clone()) {
            (Ok(write_half), Ok(shutdown_handle), Ok(subscriber_shutdown)) => {
                (write_half, shutdown_handle, subscriber_shutdown)
            }
            _ => {
                // No write half to hand a writer thread at all - nothing was
                // ever spawned, so `ConnectionCleanup` is never built on this
                // path either - both of its duties (disconnecting the client,
                // reporting `Disconnected`) are done directly here instead,
                // in the same order the guard would have used; there is
                // also nothing registered as a subscriber yet to deregister.
                runtime.disconnect_client(client_id);
                if let Some(events) = &events {
                    let _ = events.send(ConnectionEvent::Disconnected);
                }
                return;
            }
        };
    let outbound = Arc::new(Outbound::new());
    let writer = spawn_writer(Arc::clone(&outbound), write_half);
    // TASK-1.47: this connection's own attachment bookkeeping - which
    // pane each of its streams belongs to, and the next stream id to
    // hand out. Shared (via `RefCell`) between the read loop below, which
    // mutates it on every `pane.attach`/`pane.detach`, and the cleanup
    // guard, which only ever reads it, once, on the way out.
    let pane_streams: RefCell<HashMap<PaneId, (crate::pane::AttachmentId, u32)>> =
        RefCell::new(HashMap::new());
    let stream_panes: StreamPanes = RefCell::new(HashMap::new());
    let next_stream = RefCell::new(0_u32);
    // Registered the moment this connection is usable at all - before the
    // request loop below can bring in `event.watch` (TASK-1.46 §7): a
    // connection must never be asked to subscribe before it is
    // registered (`Runtime::subscribe`'s own invariant rests on this).
    runtime.register_subscriber(client_id, Arc::clone(&outbound), subscriber_shutdown);
    let _cleanup = ConnectionCleanup {
        outbound: Arc::clone(&outbound),
        writer: Some(writer),
        shutdown_handle,
        runtime,
        client_id,
        pane_streams: &pane_streams,
        events,
    };

    let state = ConnectionState {
        runtime,
        description: description(),
        context: Context::none(),
        outbound: &outbound,
        client_id,
        pane_streams: &pane_streams,
        stream_panes: &stream_panes,
        next_stream: &next_stream,
    };

    loop {
        loop {
            match connection.poll() {
                Some(Incoming::Request { id, request }) => {
                    let keep_going = handle_request(&state, connection, id, request);
                    if !keep_going {
                        // The queue is closed or full (TASK-1.45 §3: a
                        // client that floods requests and never reads any
                        // reply) - `_cleanup`'s `Drop` runs the rest.
                        return;
                    }
                }
                Some(Incoming::Data { stream, bytes }) => {
                    // TASK-1.114 (AC #1): input on an attachment's own
                    // stream, decision-2 section 3 - "a client sends input
                    // on the same stream it attached with." `stream_panes`
                    // only ever names a pane whose `pane.attach` succeeded
                    // on *this* connection (inserted there, removed at
                    // `pane.detach`/cleanup), so a frame for any other
                    // stream never decodes as `Incoming::Data` at all -
                    // `Connection::poll` itself answers `UnknownStream`
                    // for it instead (unchanged, below), which is
                    // decision-2's own "refused" for a stream this
                    // connection never opened or has already detached.
                    if let Some(&pane) = state.stream_panes.borrow().get(&stream) {
                        runtime.send_input(client_id, pane, bytes);
                    }
                }
                Some(
                    Incoming::Hello(_)
                    | Incoming::Reply { .. }
                    | Incoming::Event { .. }
                    | Incoming::UnknownEvent { .. }
                    | Incoming::StreamControl { .. }
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

/// Everything one connection's read loop needs that stays the same for
/// its whole life, bundled only so [`handle_request`] stays under
/// clippy's own argument-count lint - `pane_streams`/`next_stream` are
/// the one part of this that is actually mutated, through their own
/// `RefCell`s.
struct ConnectionState<'a> {
    runtime: &'a Runtime,
    description: Description,
    context: Context,
    outbound: &'a Arc<Outbound>,
    client_id: ClientId,
    pane_streams: &'a RefCell<HashMap<PaneId, (crate::pane::AttachmentId, u32)>>,
    stream_panes: &'a StreamPanes,
    next_stream: &'a RefCell<u32>,
}

/// Handles one decoded request, replying on `state.outbound` - `false`
/// means the connection should end (the reply queue is closed or full),
/// `true` to keep reading. `pane.attach`/`pane.detach` (TASK-1.47) and
/// `tab.declare-size` (TASK-1.114) are intercepted here, *before* falling
/// through to `runtime.handle`'s own, unchanged generic path, for the same
/// structural reason TASK-1.46 intercepts `event.watch` the same way:
/// neither `dispatch::dispatch` nor `Runtime::handle` can reach a
/// connection's own `Arc<Outbound>`/attachment bookkeeping (`pane.attach`/
/// `pane.detach`'s own reason) or even a `ClientId` at all (`tab.declare
/// -size`'s - `Context` carries no client identity, only session/tab/pane
/// defaults), both only ever available here, after the handshake.
///
/// A client reaching either method *through* `api.call` is still routed
/// here, resolved first - the same cheap, harmless second resolution
/// `Runtime::handle` already does for its own generic path (TASK-1.46 §4's
/// own reasoning): `runtime.handle`'s branch below is still given the
/// *original*, unresolved `request`, which it resolves again itself.
fn handle_request(
    state: &ConnectionState<'_>,
    connection: &mut Connection,
    id: u64,
    request: Request,
) -> bool {
    let runtime = state.runtime;
    let description = &state.description;
    let context = &state.context;
    let outbound = state.outbound;
    let client_id = state.client_id;
    let pane_streams = state.pane_streams;
    let stream_panes = state.stream_panes;
    let next_stream = state.next_stream;

    let resolved = if request.method == "api.call" {
        match resolve_api_call(&request) {
            Ok(inner) => inner,
            Err(error) => {
                let bytes = Connection::reply(id, Response::Error(error));
                return outbound.enqueue_control(bytes).is_ok();
            }
        }
    } else {
        request.clone()
    };

    match resolved.method.as_str() {
        "pane.attach" => {
            let stream = {
                let mut next = next_stream.borrow_mut();
                *next += 1;
                *next
            };
            match runtime.attach_pane(
                description,
                context,
                crate::runtime::Attaching {
                    client: client_id,
                    id,
                    stream,
                },
                &resolved,
                outbound,
            ) {
                Ok((pane, attachment_id)) => {
                    // decision-2 section 3's ordering rule: the reply is
                    // already queued (inside `attach_pane`, under its own
                    // lock) before either of these two run - the writer
                    // may only now pull data frames for this stream, and
                    // the worker is only now handed something to send to.
                    outbound.open_stream(stream);
                    runtime.arm_attachment(pane, attachment_id, Arc::clone(outbound));
                    connection.open_stream(stream);
                    pane_streams
                        .borrow_mut()
                        .insert(pane, (attachment_id, stream));
                    stream_panes.borrow_mut().insert(stream, pane);
                    true
                }
                Err(error) => {
                    let bytes = Connection::reply(id, Response::Error(error));
                    outbound.enqueue_control(bytes).is_ok()
                }
            }
        }
        "pane.detach" => match runtime.resolve_pane_target(description, context, &resolved) {
            Err(error) => {
                let bytes = Connection::reply(id, Response::Error(error));
                outbound.enqueue_control(bytes).is_ok()
            }
            Ok(pane) => {
                let entry = pane_streams.borrow_mut().remove(&pane);
                let response = match entry {
                    None => Response::Error(bad_request(
                        "this connection has no attachment to this pane",
                    )),
                    Some((attachment_id, stream)) => {
                        runtime.detach_pane(client_id, pane, attachment_id);
                        outbound.close_stream(stream);
                        connection.close_stream(stream);
                        stream_panes.borrow_mut().remove(&stream);
                        Response::Ok(serde_json::Value::Null)
                    }
                };
                let bytes = Connection::reply(id, response);
                outbound.enqueue_control(bytes).is_ok()
            }
        },
        "tab.declare-size" => {
            // Params are read and range-checked here, before `Runtime::
            // declare_size` is even called: `columns`/`rows` are the one
            // thing this method's own `ValueKind::Integer` params don't
            // already bound (R100's own kernel-level "no floor check,
            // ever" is about shrinking below the 20x4 floor, a different,
            // deliberately allowed case - see the method's own `details`
            // string; `0` is a narrower, degenerate case nothing in the
            // kernel or the engine is verified to handle).
            let response = match (
                param_i64(&resolved, "columns"),
                param_i64(&resolved, "rows"),
            ) {
                (Some(columns), Some(rows))
                    if (1..=i64::from(u16::MAX)).contains(&columns)
                        && (1..=i64::from(u16::MAX)).contains(&rows) =>
                {
                    #[expect(
                        clippy::cast_possible_truncation,
                        reason = "just bounded to 1..=u16::MAX above"
                    )]
                    match runtime.declare_size(
                        description,
                        context,
                        &resolved,
                        client_id,
                        columns as u16,
                        rows as u16,
                    ) {
                        Ok(()) => Response::Ok(serde_json::Value::Null),
                        Err(error) => Response::Error(error),
                    }
                }
                _ => Response::Error(bad_request(
                    "columns and rows must each be an integer between 1 and 65535",
                )),
            };
            let bytes = Connection::reply(id, response);
            outbound.enqueue_control(bytes).is_ok()
        }
        // `event.watch` (TASK-1.46 §4), reached directly or through
        // `api.call` (already resolved above, the same way `pane.attach`/
        // `pane.detach` are), bypasses the generic reply-after-the-fact
        // path below entirely: `Runtime::subscribe` enqueues its own
        // reply atomically with registering the filter, so nothing here
        // must enqueue a second one.
        "event.watch" => match runtime.subscribe(description, context, &resolved, client_id, id) {
            Ok(()) => true,
            Err(error) => {
                let bytes = Connection::reply(id, Response::Error(error));
                outbound.enqueue_control(bytes).is_ok()
            }
        },
        _ => {
            let response = runtime.handle(description, context, &request);
            let bytes = Connection::reply(id, response);
            outbound.enqueue_control(bytes).is_ok()
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
    mut events: Option<Sender<ConnectionEvent>>,
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
                // Moves `events` in, rather than lending it: from this
                // point on, `run_post_handshake` (through `ConnectionCleanup`'s
                // Drop) is the one place `Disconnected` is sent, on every
                // path out of it including a panic - `handle`'s own final
                // check below finds `None` here and sends nothing a
                // second time.
                run_post_handshake(
                    &mut stream,
                    &mut connection,
                    runtime,
                    client_id,
                    events.take(),
                );
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

    // Only reached with `events: Some(_)` on the paths that never called
    // `run_post_handshake` at all (no complete frame, not a hello, or a
    // refused handshake) - none of them run untrusted dispatch code, so
    // there is nothing here that panics and skips this. The accepted
    // path already sent `Disconnected` through `ConnectionCleanup`'s own
    // `Drop`, on every path out of it including a panic, and left `None`
    // behind here (`events.take()`, above) so this never sends twice.
    if let Some(events) = events {
        let _ = events.send(ConnectionEvent::Disconnected);
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::time_remaining;
    use super::{Arc, ConnectionCleanup, HashMap, Outbound, RefCell, Runtime, UnixStream};
    use crate::dispatch::Context;
    use crate::outbound::spawn_writer;
    use crate::pty::RealLauncher;
    use crate::runtime::{Attaching, LoginShell};
    use steg_sdk::{ClientKind, Request, Response};

    const TEST_GRACE: Duration = Duration::from_millis(30);
    const TEST_TICK: Duration = Duration::from_millis(1);

    fn login() -> LoginShell {
        LoginShell {
            argv0: "/bin/sh".to_string(),
            home: std::env::temp_dir().to_str().unwrap().to_string(),
            user: None,
            base_env: vec![(
                std::ffi::OsString::from("PATH"),
                std::env::var_os("PATH").unwrap_or_default(),
            )],
        }
    }

    /// Builds a session, its one tab and its one pane through `Runtime::
    /// handle` itself (no real socket), and returns the full `session/tab/
    /// pane` path - the one shape `pane.attach`'s own target resolves with
    /// no session context (R10, the same gap `attach.rs`'s own integration
    /// tests already name).
    fn new_pane_path(
        runtime: &Runtime,
        description: &steg_sdk::Description,
        context: &Context,
    ) -> String {
        let session = match runtime.handle(
            description,
            context,
            &Request {
                method: "session.new".to_string(),
                params: serde_json::json!({"program": ["sh", "-c", "sleep 100"]})
                    .as_object()
                    .unwrap()
                    .clone(),
            },
        ) {
            Response::Ok(value) => value,
            Response::Error(error) => panic!("session.new failed: {error:?}"),
        };
        let session_id = session["id"].as_str().unwrap().to_string();

        let tabs = match runtime.handle(
            description,
            context,
            &Request {
                method: "tab.list".to_string(),
                params: serde_json::json!({"target": session_id})
                    .as_object()
                    .unwrap()
                    .clone(),
            },
        ) {
            Response::Ok(value) => value,
            Response::Error(error) => panic!("tab.list failed: {error:?}"),
        };
        let tab_id = tabs.as_array().unwrap()[0]["id"]
            .as_str()
            .unwrap()
            .to_string();

        let panes = match runtime.handle(
            description,
            context,
            &Request {
                method: "pane.list".to_string(),
                params: serde_json::json!({"target": format!("{session_id}/{tab_id}")})
                    .as_object()
                    .unwrap()
                    .clone(),
            },
        ) {
            Response::Ok(value) => value,
            Response::Error(error) => panic!("pane.list failed: {error:?}"),
        };
        let pane_id = panes.as_array().unwrap()[0]["id"]
            .as_str()
            .unwrap()
            .to_string();

        format!("{session_id}/{tab_id}/{pane_id}")
    }

    /// Bounded, polling the one fact that is true exactly once the
    /// worker has actually applied an `Attach`/`Detach` (or the sweep
    /// `ConnectionCleanup::Drop` sends in its place) - never a sleep
    /// assumed long enough.
    fn wait_for_attachment_count(runtime: &Runtime, pane: steg_sdk::PaneId, expected: usize) {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let found = runtime.attachment_count_for_test(pane);
            if found == expected {
                return;
            }
            assert!(
                Instant::now() < deadline,
                "pane {pane} never reached {expected} attachments; last saw {found}"
            );
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    /// TASK-1.47 review: a connection that ends without ever calling
    /// `pane.detach` must not leave a dead attachment in a pane's own
    /// fan-out list forever - proven here by actually counting it
    /// (`Runtime::attachment_count_for_test`), not merely by a later
    /// `pane.attach` not hanging (`attach.rs`'s own integration test's
    /// own, weaker proof). `ConnectionCleanup`'s `Drop` is exercised
    /// directly - not `Runtime::forget_attachment` called by hand, which
    /// would only prove that method works, never that `Drop` actually
    /// calls it - built in-process with a real pane and a real `Outbound`/
    /// writer pair over a real `UnixStream::pair()`, but no real socket
    /// listener.
    #[test]
    fn connection_cleanups_drop_sweeps_every_attachment_it_still_holds() {
        let runtime = Runtime::new(
            Arc::new(RealLauncher::default()),
            login(),
            TEST_GRACE,
            TEST_TICK,
        );
        let description = steg_sdk::description();
        let context = Context::none();
        let path = new_pane_path(&runtime, &description, &context);

        let (client_writer, _server_reader) = UnixStream::pair().unwrap();
        let outbound = Arc::new(Outbound::new());
        let writer = spawn_writer(Arc::clone(&outbound), client_writer);

        let stream = 1_u32;
        let client_id = runtime.connect_client(ClientKind::App, None);
        let mut params = serde_json::Map::new();
        params.insert("target".to_string(), serde_json::json!(path));
        let attach_request = Request {
            method: "pane.attach".to_string(),
            params,
        };
        let (pane, attachment_id) = runtime
            .attach_pane(
                &description,
                &context,
                Attaching {
                    client: client_id,
                    id: 1,
                    stream,
                },
                &attach_request,
                &outbound,
            )
            .expect("attach_pane succeeds");
        outbound.open_stream(stream);
        runtime.arm_attachment(pane, attachment_id, Arc::clone(&outbound));
        wait_for_attachment_count(&runtime, pane, 1);

        let pane_streams = RefCell::new(HashMap::from([(pane, (attachment_id, stream))]));
        let (_shutdown_a, shutdown_b) = UnixStream::pair().unwrap();
        {
            let _cleanup = ConnectionCleanup {
                outbound,
                writer: Some(writer),
                shutdown_handle: shutdown_b,
                runtime: &runtime,
                client_id,
                pane_streams: &pane_streams,
                events: None,
            };
            // `_cleanup` drops here, running its sweep - the one thing
            // this test is about.
        }

        wait_for_attachment_count(&runtime, pane, 0);
    }

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
