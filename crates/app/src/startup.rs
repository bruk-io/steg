//! Connects to a server, finds or makes the labelled session, attaches to its first tab's
//! first pane, and hands the result back (TASK-1.117's plan §1/§3). Blocking, start to
//! finish - run on its own dedicated thread (`spawn`), never on gpui's own background
//! executor, whose own dispatcher backs a small, fixed pool (`BackgroundExecutor::num_cpus`)
//! this task's own real, multi-second `connect()` retry loop would otherwise occupy.
//!
//! **What is actually guaranteed (three review rounds on this one mechanism settled this;
//! read it before changing any of the three pieces below).**
//!
//! 1. Closing the window never blocks the main thread on this thread, in any phase.
//!    `AppWindow::teardown` never calls `.join()` on a startup thread that has not already
//!    sent its result - a thread this side cannot be sure of unblocking must not be joined
//!    on the main thread. It marks the thread cancelled (`interrupt`, below) and lets go of
//!    the `JoinHandle` without joining it; `apply_startup_result` is the only place that
//!    joins, and only once the startup thread's own oneshot has already resolved - at that
//!    point `sender.send(result)` has already run and the thread's closure has nothing left
//!    to do, so the join is bounded and effectively instant.
//! 2. The startup thread always ends on its own within a stated bound, whether or not the
//!    main thread is still watching. While still inside `connect()`'s own retry loop (sdk-
//!    owned, not this crate's), that bound is `ConnectOptions::connect_timeout`
//!    ([`CONNECT_TIMEOUT`]) - the one phase nothing here can shorten, since no socket exists
//!    yet to act on. From the moment `connect()` returns a socket, every blocking read
//!    carries [`READ_TIMEOUT`] (a timeout on a blocking read is a bound, not polling) -
//!    cleared again the moment a successful attach is claimed, since the reader thread this
//!    socket moves to next (`client.rs`) blocks without limit by design. A `teardown`'s own
//!    `interrupt` call may still end a blocked thread sooner, by shutting down the socket it
//!    finds published - but this is an acceleration only; correctness never depends on its
//!    timing, only the read timeout does.
//! 3. A successful attach never has its socket shut down, and never carries the startup read
//!    timeout onward. [`Slot`] is the sticky state a racing `interrupt` and `attach_over`'s
//!    own last step share one lock over, so this is decided atomically rather than by a
//!    single check that might be looking at the wrong moment - see `attach_over`'s own final
//!    claim for the exact race and which side wins it.
//!
//! The one place a precise, strict guarantee still matters (rather than "ends within a
//! bound") is requirement 3: a connection already handed over as attached must never be
//! undone by a `teardown` that merely arrived late. [`Slot`]'s claim-on-success exists for
//! exactly that; everything else - the two bounds above - tolerates an imprecise race, since
//! nothing worse than "takes up to the stated bound" can follow from it.

use std::env;
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, PoisonError};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use futures::channel::oneshot;
use serde_json::{Map, Value, json};
use steg_sdk::{
    ClientKind, ConnectError, ConnectOptions, Connection, Incoming, PaneAttached, Response,
    WireRole, connect, description, parse_server_hello,
};

/// The label every session this app makes or finds carries (TASK-1.117's plan §"For the
/// engineer" point 5): plain, and distinguishable from anything a person or script makes
/// by hand. One label, one session, ever - `fix_up_session` is what keeps that true.
pub const SESSION_LABEL: &str = "steg-app";

/// Why [`run`] could not hand back a connected, attached session.
#[derive(Debug)]
pub enum StartupError {
    /// The binary to autostart could not be resolved at all (neither `STEG_BINARY` nor
    /// this process's own directory had one) - named here rather than guessed, since the
    /// sdk is explicit that a client supplies this (`crates/sdk/src/local_socket.rs`'s own
    /// doc on `ConnectOptions::binary`).
    NoBinary,
    /// Connecting to, or starting, the server failed.
    Connect(ConnectError),
    /// The handshake, or a call the startup sequence made, did not answer as this bare
    /// app's own design expects - a message for the "disconnected" placeholder, not a
    /// shell's own screen or any environment detail (never put either in a note - this
    /// message is shown in the window only).
    Protocol(String),
}

impl std::fmt::Display for StartupError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoBinary => write!(
                f,
                "could not find the steg binary to start a server with (set STEG_BINARY)"
            ),
            Self::Connect(err) => write!(f, "{err}"),
            Self::Protocol(message) => write!(f, "{message}"),
        }
    }
}

/// What a successful startup hands the main thread: a connected stream, a `Connection`
/// past the handshake with the one attachment's stream already open (decision-2 §3's
/// ordering rule), and the snapshot that stream's attachment began at.
pub struct Attached {
    pub stream: UnixStream,
    pub connection: Connection,
    pub shutdown_handle: UnixStream,
    pub snapshot: steg_sdk::Screen,
    /// The attachment's own stream id (`PaneAttached.stream`) - what the writer's own
    /// `Connection::send_data`/`Connection::open_stream` need (TASK-1.121 plan §3: the
    /// writer is a second `Connection` instance, seeded the same way the reader's was).
    pub stream_id: u32,
    /// A clone of the connected socket, for the writer's own `Connection` to write
    /// through directly - the same socket, a second handle (TASK-1.117's plan §1: "two
    /// `Connection` instances... writing directly over its own cloned handle").
    pub write_handle: UnixStream,
    /// `{session}/{tab}` - `tab.declare-size`'s own target (TASK-1.114 §2: declaring a
    /// size targets a tab, not the pane this startup attached to).
    pub tab_target: String,
}

/// Resolves the binary `ConnectOptions::binary` autostarts (TASK-1.117's plan
/// §"For the engineer" point 2): `STEG_BINARY` first, then this process's own directory
/// joined with `steg` (the CLI/server's own binary name, `crates/steg/Cargo.toml`'s
/// package name) - both a dev build (`cargo run -p steg-app` lands beside `steg` in the
/// same `target/<profile>/`) and, later, a bundle's own layout can set the override.
fn resolve_binary() -> Result<PathBuf, StartupError> {
    if let Some(path) = env::var_os("STEG_BINARY") {
        return Ok(PathBuf::from(path));
    }
    let exe = env::current_exe().map_err(|_| StartupError::NoBinary)?;
    let candidate = exe
        .parent()
        .ok_or(StartupError::NoBinary)?
        .join(if cfg!(windows) { "steg.exe" } else { "steg" });
    if candidate.exists() {
        Ok(candidate)
    } else {
        Err(StartupError::NoBinary)
    }
}

/// The sticky state a racing `teardown` and [`attach_over`] share one lock over - see the
/// module doc for why a plain `Option<UnixStream>`, checked once, missed two races a single
/// check cannot tell apart.
pub(crate) enum Slot {
    /// Not yet connected, or connected but not yet published - `interrupt` finds this when
    /// it races in too early; replaced with [`Slot::Cancelled`] either way (module doc).
    Pending,
    /// Interruptible right now: a `try_clone()`'d handle of the live socket, published the
    /// moment `connect()` succeeds, before any blocking read follows it.
    Connected(UnixStream),
    /// Terminal: either `interrupt` has already run (and already shut down whatever
    /// `Connected` handle it found, if any), or `attach_over`'s own final claim already
    /// took the handle to report success. Nothing further to do either way.
    Cancelled,
}

/// What a concurrent `teardown` locks to interrupt a still-running [`run`] - see the
/// module doc.
pub type ShutdownSlot = Arc<Mutex<Slot>>;

/// What a racing `teardown` calls: the one lock-and-replace both races in the module doc
/// are decided by. Always leaves [`Slot::Cancelled`] behind, and shuts down whatever
/// handle it displaces if it was actually [`Slot::Connected`] - a no-op, otherwise, that
/// still leaves the sticky mark `attach_over`'s own checks rely on finding later.
pub fn interrupt(slot: &ShutdownSlot) {
    let mut guard = slot.lock().unwrap_or_else(PoisonError::into_inner);
    if let Slot::Connected(handle) = std::mem::replace(&mut *guard, Slot::Cancelled) {
        let _ = handle.shutdown(std::net::Shutdown::Both);
    }
}

/// Starts the whole blocking sequence on its own dedicated thread and hands the result
/// back over a oneshot the caller `.await`s - never gpui's own background executor
/// (module doc). The third value is the [`ShutdownSlot`] a concurrent `teardown` locks to
/// interrupt a still-running startup (module doc).
pub fn spawn() -> (
    JoinHandle<()>,
    oneshot::Receiver<Result<Attached, StartupError>>,
    ShutdownSlot,
) {
    let shutdown_slot: ShutdownSlot = Arc::new(Mutex::new(Slot::Pending));
    let (sender, receiver) = oneshot::channel();
    let slot = Arc::clone(&shutdown_slot);
    let handle = thread::spawn(move || {
        let _ = sender.send(run(&slot));
    });
    (handle, receiver, shutdown_slot)
}

/// How long `connect()`'s own retry loop (sdk-owned, not this crate's) may run before
/// giving up - the one phase a closing window cannot shorten (module doc).
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

/// The bound every blocking read during [`attach_over`] carries, cleared again the moment
/// a successful attach is claimed (module doc, requirement 3: a success never carries a
/// timeout onward) - the real guarantee behind "the startup thread always ends on its own",
/// independent of whether a racing `interrupt` ever manages to shut the socket down sooner.
const READ_TIMEOUT: Duration = Duration::from_secs(10);

fn run(shutdown_slot: &ShutdownSlot) -> Result<Attached, StartupError> {
    let binary = resolve_binary()?;
    let options = ConnectOptions {
        binary,
        socket_path: None,
        autostart: true,
        connect_timeout: CONNECT_TIMEOUT,
    };
    let stream = connect(&options).map_err(StartupError::Connect)?;
    attach_over(stream, shutdown_slot, READ_TIMEOUT)
}

/// Everything after `connect()` has a socket in hand: the handshake, finding or making the
/// session, resolving the pane, and attaching to it - split out from [`run`] so a test can
/// drive it directly over a real socket pair, with no real `connect()`/autostart involved
/// (module doc's own second finding: this is also where the slot-clearing race is decided).
/// `read_timeout` is a parameter (not always [`READ_TIMEOUT`]) so a test can use a short one
/// and stay fast rather than waiting out a production-sized bound.
fn attach_over(
    stream: UnixStream,
    shutdown_slot: &ShutdownSlot,
    read_timeout: Duration,
) -> Result<Attached, StartupError> {
    stream.set_read_timeout(Some(read_timeout)).map_err(|err| {
        StartupError::Protocol(format!("could not set the socket's read timeout: {err}"))
    })?;
    let shutdown_handle = stream
        .try_clone()
        .map_err(|err| StartupError::Protocol(format!("could not clone the socket: {err}")))?;
    // Published *before* the first blocking read that follows (module doc) - a `teardown`
    // racing this thread sees either `Pending` (still inside `connect()`, its own bounded
    // wait, or in the tiny gap before this very publish) or `Connected` (every read from
    // here on can be interrupted). Finding `Cancelled` here means `interrupt` already ran
    // too early for a handle to have existed yet to shut down - the module doc's "too
    // early" race - so this socket, which `interrupt` never got the chance to touch, must
    // be shut down here instead, and startup reported as failed rather than continuing
    // into reads nothing could interrupt a second time.
    let interrupt_handle = shutdown_handle
        .try_clone()
        .map_err(|err| StartupError::Protocol(format!("could not clone the socket: {err}")))?;
    let already_cancelled = {
        let mut guard = shutdown_slot.lock().unwrap_or_else(PoisonError::into_inner);
        match *guard {
            Slot::Cancelled => true,
            Slot::Pending | Slot::Connected(_) => {
                *guard = Slot::Connected(interrupt_handle);
                false
            }
        }
    };
    if already_cancelled {
        let _ = shutdown_handle.shutdown(std::net::Shutdown::Both);
        return Err(StartupError::Protocol(
            "the window closed while this connection was being set up".into(),
        ));
    }
    let mut stream = stream;

    let hello = Connection::send_hello("steg-app", ClientKind::App, None);
    write_all(&mut stream, &hello)?;
    let mut connection = Connection::new(WireRole::Client, description());
    let hello_reply = block_until(&mut stream, &mut connection, |incoming| match incoming {
        Incoming::Hello(payload) => Some(payload),
        Incoming::Request { .. }
        | Incoming::Reply { .. }
        | Incoming::Event { .. }
        | Incoming::UnknownEvent { .. }
        | Incoming::StreamControl { .. }
        | Incoming::Data { .. }
        | Incoming::UnknownStream { .. }
        | Incoming::Malformed(_) => None,
    })?;
    parse_server_hello(&hello_reply)
        .map_err(|err| StartupError::Protocol(format!("hello: {err}")))?;

    let sessions = ok_value(call(
        &mut stream,
        &mut connection,
        "session.list",
        Map::new(),
    )?)?;
    let session_id = find_or_make_session(&mut stream, &mut connection, &sessions)?;

    let tabs = ok_value(call(
        &mut stream,
        &mut connection,
        "tab.list",
        params(&[("target", json!(session_id))]),
    )?)?;
    let tab_id = first_id(&tabs, "tab.list")?;

    let (tab_id, pane_id) = resolve_pane(&mut stream, &mut connection, &session_id, tab_id)?;
    let tab_target = format!("{session_id}/{tab_id}");

    let attach_target = format!("{tab_target}/{pane_id}");
    let (id, bytes) = connection
        .attach(Some(&attach_target))
        .map_err(|err| StartupError::Protocol(format!("pane.attach: {err:?}")))?;
    write_all(&mut stream, &bytes)?;
    let response = block_until(&mut stream, &mut connection, |incoming| match incoming {
        Incoming::Reply {
            id: reply_id,
            response,
        } if reply_id == id => Some(response),
        Incoming::Reply { .. }
        | Incoming::Hello(_)
        | Incoming::Request { .. }
        | Incoming::Event { .. }
        | Incoming::UnknownEvent { .. }
        | Incoming::StreamControl { .. }
        | Incoming::Data { .. }
        | Incoming::UnknownStream { .. }
        | Incoming::Malformed(_) => None,
    })?;
    let attached_value = ok_value(response)?;
    let attached: PaneAttached = serde_json::from_value(attached_value)
        .map_err(|err| StartupError::Protocol(format!("pane.attach: {err}")))?;
    connection.open_stream(attached.stream);
    let write_handle = stream
        .try_clone()
        .map_err(|err| StartupError::Protocol(format!("could not clone the socket: {err}")))?;

    // The "too late" decision point from the module doc: this is the last step before
    // declaring success, and the same lock `interrupt` uses to shut the connection down
    // guards it. Taking `Connected` back means this thread got here first - no concurrent
    // `teardown` has touched (or will touch) the socket, so the handle taken is simply
    // dropped, never shut down, and success is safe to report. Finding `Cancelled` instead
    // means `interrupt` already ran, under the same lock - its own `shutdown()` call has
    // therefore already completed by the time this side observes it - so this reports
    // failure instead of handing over a connection already known to be dead. `Pending`
    // cannot happen here: this function is the only writer, and it already moved the slot
    // to `Connected` (or returned early on `Cancelled`) before any of the reads above ran.
    let claimed = {
        let mut guard = shutdown_slot.lock().unwrap_or_else(PoisonError::into_inner);
        match std::mem::replace(&mut *guard, Slot::Cancelled) {
            Slot::Connected(handle) => {
                drop(handle);
                true
            }
            Slot::Pending | Slot::Cancelled => false,
        }
    };
    if !claimed {
        return Err(StartupError::Protocol(
            "the window closed while this connection was being set up".into(),
        ));
    }

    // Requirement 3's second half: a success never carries the startup read timeout
    // onward - the reader thread this socket moves to next (`client.rs`) blocks without
    // limit by design, and shares the same underlying socket (both are `try_clone()`'d
    // from it), so clearing it on either clears it for both; cleared on both anyway, not
    // relying on that alone.
    stream.set_read_timeout(None).map_err(|err| {
        StartupError::Protocol(format!("could not clear the socket's read timeout: {err}"))
    })?;
    shutdown_handle.set_read_timeout(None).map_err(|err| {
        StartupError::Protocol(format!("could not clear the socket's read timeout: {err}"))
    })?;

    Ok(Attached {
        stream,
        connection,
        shutdown_handle,
        snapshot: attached.snapshot,
        stream_id: attached.stream,
        write_handle,
        tab_target,
    })
}

/// Finds a session labelled [`SESSION_LABEL`] (the server's own order, R6 - "take the
/// first" is as far as this bare app arbitrates), or makes one.
fn find_or_make_session(
    stream: &mut UnixStream,
    connection: &mut Connection,
    sessions: &Value,
) -> Result<String, StartupError> {
    let records = sessions
        .as_array()
        .ok_or_else(|| StartupError::Protocol("session.list: not an array".into()))?;
    let labelled = records
        .iter()
        .find(|record| record.get("label").and_then(Value::as_str) == Some(SESSION_LABEL));
    if let Some(record) = labelled {
        return record
            .get("id")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned)
            .ok_or_else(|| StartupError::Protocol("session.list: a record with no id".into()));
    }
    let created = ok_value(call(
        stream,
        connection,
        "session.new",
        params(&[("label", json!(SESSION_LABEL))]),
    )?)?;
    created
        .get("id")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
        .ok_or_else(|| StartupError::Protocol("session.new: not {\"id\": ...}".into()))
}

/// Resolves the pane to attach to, within `session_id`'s own `tab_id`: the tab's first
/// pane if its program is still running, or a fresh tab in the *same* session if it has
/// already ended (TASK-1.117's plan §3 and §6 - review finding: extracted from `run` so
/// this branch is reachable by a test with no window at all).
fn resolve_pane(
    stream: &mut UnixStream,
    connection: &mut Connection,
    session_id: &str,
    tab_id: String,
) -> Result<(String, String), StartupError> {
    let pane_target = format!("{session_id}/{tab_id}");
    let panes = ok_value(call(
        stream,
        connection,
        "pane.list",
        params(&[("target", json!(pane_target))]),
    )?)?;
    let pane = first_record(&panes, "pane.list")?;
    let pane_id = pane
        .get("id")
        .and_then(Value::as_str)
        .ok_or_else(|| StartupError::Protocol("pane.list: a pane record with no id".into()))?;
    let running = pane
        .get("program_status")
        .and_then(Value::as_str)
        .is_some_and(|status| status == "running");

    if running {
        return Ok((tab_id, pane_id.to_owned()));
    }

    // The found session's own pane has nothing live behind it (TASK-1.117's plan §3): a
    // fresh tab in the *same* session, never a second session under the same label.
    let new_tab = ok_value(call(
        stream,
        connection,
        "tab.new",
        params(&[("target", json!(session_id))]),
    )?)?;
    let new_tab_id = new_tab
        .as_str()
        .ok_or_else(|| StartupError::Protocol("tab.new: not a bare identifier".into()))?
        .to_owned();
    let panes = ok_value(call(
        stream,
        connection,
        "pane.list",
        params(&[("target", json!(format!("{session_id}/{new_tab_id}")))]),
    )?)?;
    let pane = first_record(&panes, "pane.list")?;
    let pane_id = pane
        .get("id")
        .and_then(Value::as_str)
        .ok_or_else(|| StartupError::Protocol("pane.list: a pane record with no id".into()))?
        .to_owned();
    Ok((new_tab_id, pane_id))
}

fn first_id(records: &Value, method: &'static str) -> Result<String, StartupError> {
    first_record(records, method)?
        .get("id")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
        .ok_or_else(|| StartupError::Protocol(format!("{method}: a record with no id")))
}

fn first_record<'a>(records: &'a Value, method: &'static str) -> Result<&'a Value, StartupError> {
    records
        .as_array()
        .and_then(|array| array.first())
        .ok_or_else(|| StartupError::Protocol(format!("{method}: no records at all")))
}

fn params(entries: &[(&str, Value)]) -> Map<String, Value> {
    entries
        .iter()
        .map(|(key, value)| ((*key).to_owned(), value.clone()))
        .collect()
}

fn ok_value(response: Response) -> Result<Value, StartupError> {
    match response {
        Response::Ok(value) => Ok(value),
        Response::Error(error) => Err(StartupError::Protocol(error.message)),
    }
}

fn write_all(stream: &mut UnixStream, bytes: &[u8]) -> Result<(), StartupError> {
    stream
        .write_all(bytes)
        .map_err(|err| StartupError::Protocol(format!("writing to the server: {err}")))
}

/// Validates and sends one call, then blocks until its own matching reply (by id) arrives.
/// Every other frame read along the way is handed to [`Connection::poll`] and otherwise
/// ignored, the same shape `crates/server/tests/attach.rs`'s own `call_and_read_reply`
/// already uses.
fn call(
    stream: &mut UnixStream,
    connection: &mut Connection,
    method: &str,
    params: Map<String, Value>,
) -> Result<Response, StartupError> {
    let (id, bytes) = connection
        .call(method, params)
        .map_err(|err| StartupError::Protocol(format!("{method}: {err:?}")))?;
    write_all(stream, &bytes)?;
    block_until(stream, connection, |incoming| match incoming {
        Incoming::Reply {
            id: reply_id,
            response,
        } if reply_id == id => Some(response),
        Incoming::Reply { .. }
        | Incoming::Hello(_)
        | Incoming::Request { .. }
        | Incoming::Event { .. }
        | Incoming::UnknownEvent { .. }
        | Incoming::StreamControl { .. }
        | Incoming::Data { .. }
        | Incoming::UnknownStream { .. }
        | Incoming::Malformed(_) => None,
    })
}

/// Reads from `stream` and polls `connection` until `want` recognises something, ignoring
/// everything else it decodes along the way (this bare startup sequence makes one call at
/// a time and waits for its own reply, same as every call site above).
fn block_until<T>(
    stream: &mut UnixStream,
    connection: &mut Connection,
    mut want: impl FnMut(Incoming) -> Option<T>,
) -> Result<T, StartupError> {
    loop {
        if let Some(incoming) = connection.poll() {
            if let Incoming::Malformed(message) = &incoming {
                return Err(StartupError::Protocol(format!(
                    "malformed frame: {message}"
                )));
            }
            if let Some(value) = want(incoming) {
                return Ok(value);
            }
            continue;
        }
        let mut buf = [0_u8; 65536];
        let n = stream
            .read(&mut buf)
            .map_err(|err| StartupError::Protocol(format!("reading from the server: {err}")))?;
        if n == 0 {
            return Err(StartupError::Protocol(
                "the server closed the connection during startup".into(),
            ));
        }
        connection.receive(&buf[..n]);
    }
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc;
    use std::time::Instant;

    use super::*;

    /// Short enough that every test using it stays fast, and long enough to leave a clear
    /// margin between "ended quickly, via the accelerator" and "waited the bound out".
    const TEST_READ_TIMEOUT: Duration = Duration::from_millis(300);

    /// `JoinHandle::join` has no timeout - every test below that could hang if this
    /// task's own mechanism were broken instead runs the blocking call on its own thread
    /// and waits on a plain, timeout-capable `std::sync::mpsc` channel from the test's own
    /// thread: a broken mechanism fails the assertion, not the test process (the same
    /// shape `client.rs`'s own tests already use).
    fn bounded<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> T {
        let (sender, receiver) = mpsc::channel();
        thread::spawn(move || {
            let _ = sender.send(f());
        });
        receiver
            .recv_timeout(Duration::from_secs(2))
            .expect("did not finish within the test's own bound")
    }

    /// The same bound as [`bounded`], for a thread already spawned (joining it is itself
    /// the blocking call with no timeout here).
    fn bounded_join<T: Send + 'static>(handle: JoinHandle<T>) -> T {
        bounded(move || handle.join().expect("the worker thread should not panic"))
    }

    /// A real `UnixStream::pair()` (testing-rust.md: a real socket in one is cheap and is
    /// not a double) with a read timeout on each half - `bounded`/`bounded_join` only
    /// bound the worker thread a test spawns; the test's own main thread, playing the
    /// server synchronously via [`answer_one`], has no such wrapper, so a future
    /// regression that makes the client send fewer or different requests than a test
    /// expects must not leave that thread blocked in `read()` forever instead.
    fn bounded_pair() -> (UnixStream, UnixStream) {
        let (a, b) = UnixStream::pair().expect("a socket pair");
        a.set_read_timeout(Some(Duration::from_secs(2)))
            .expect("set a read timeout");
        b.set_read_timeout(Some(Duration::from_secs(2)))
            .expect("set a read timeout");
        (a, b)
    }

    /// Reads one request off `server_side`, asserts its method, and replies with `value` -
    /// the fake-server half of a real `UnixStream::pair()` (testing-rust.md: a real socket
    /// in one is cheap and is not a double), the same shape `client.rs`'s own tests and
    /// `crates/server/tests/attach.rs` already use.
    fn answer_one(
        server_side: &mut UnixStream,
        server_connection: &mut Connection,
        method: &str,
        value: Value,
    ) {
        loop {
            match server_connection.poll() {
                Some(Incoming::Request { id, request }) => {
                    assert_eq!(request.method, method, "an unexpected method was called");
                    let bytes = Connection::reply(id, Response::Ok(value));
                    server_side.write_all(&bytes).expect("write the reply");
                    return;
                }
                Some(_) => continue,
                None => {
                    let mut buf = [0_u8; 65536];
                    let n = server_side.read(&mut buf).expect("read the request");
                    assert_ne!(
                        n, 0,
                        "the client side closed before sending the expected request"
                    );
                    server_connection.receive(&buf[..n]);
                }
            }
        }
    }

    /// The hello handshake's own server half - not a `Control` frame, so `answer_one`
    /// doesn't cover it. `Connection::accept_or_refuse` is the sdk's own server-side
    /// handshake logic (`wire/connection.rs`), the same one `crates/server` itself calls.
    fn answer_hello(server_side: &mut UnixStream, server_connection: &mut Connection) {
        loop {
            match server_connection.poll() {
                Some(Incoming::Hello(payload)) => {
                    let (_, bytes) = Connection::accept_or_refuse(
                        &payload,
                        "steg-test",
                        "0.1.0",
                        steg_sdk::ClientId::from_raw(1),
                    );
                    server_side
                        .write_all(&bytes)
                        .expect("write the hello reply");
                    return;
                }
                Some(_) => continue,
                None => {
                    let mut buf = [0_u8; 65536];
                    let n = server_side.read(&mut buf).expect("read the hello");
                    assert_ne!(n, 0, "the client side closed before sending hello");
                    server_connection.receive(&buf[..n]);
                }
            }
        }
    }

    // --- find_or_make_session ------------------------------------------------------------

    #[test]
    fn a_labelled_session_is_returned_with_no_session_new_call() {
        let (client_side, _server_side) = UnixStream::pair().expect("a socket pair");
        let sessions = json!([{"id": "abc", "label": SESSION_LABEL}]);
        let result = bounded(move || {
            let mut stream = client_side;
            let mut connection = Connection::new(WireRole::Client, description());
            find_or_make_session(&mut stream, &mut connection, &sessions)
        });
        // `_server_side` was never written to or read from - the only way this bound is
        // met at all is if this path never made a call (dropped alongside `client_side`'s
        // own half, which would otherwise surface as a read/write error here instead).
        assert_eq!(result.expect("a labelled session is found"), "abc");
    }

    #[test]
    fn no_labelled_session_calls_session_new() {
        let (client_side, mut server_side) = bounded_pair();
        let sessions = json!([]);
        let worker = thread::spawn(move || {
            let mut stream = client_side;
            let mut connection = Connection::new(WireRole::Client, description());
            find_or_make_session(&mut stream, &mut connection, &sessions)
        });
        let mut server_connection = Connection::new(WireRole::Server, description());
        answer_one(
            &mut server_side,
            &mut server_connection,
            "session.new",
            json!({"id": "new-session"}),
        );
        let result = bounded_join(worker);
        assert_eq!(
            result.expect("session.new's own id is returned"),
            "new-session"
        );
    }

    // --- resolve_pane (review finding: the stale-pane/tab.new branch had no test) -------

    /// Also proves no `tab.new` call happens on this path: the fake server here only
    /// ever answers one request (`pane.list`); if `resolve_pane` tried to call `tab.new`
    /// too, the worker thread would block waiting for a second reply nothing ever sends,
    /// and `bounded_join` would time this test out rather than let it pass silently.
    #[test]
    fn a_running_pane_from_a_real_reply_is_returned_unchanged() {
        let (client_side, mut server_side) = bounded_pair();
        let worker = thread::spawn(move || {
            let mut stream = client_side;
            let mut connection = Connection::new(WireRole::Client, description());
            resolve_pane(&mut stream, &mut connection, "sess", "tab-1".to_owned())
        });
        let mut server_connection = Connection::new(WireRole::Server, description());
        answer_one(
            &mut server_side,
            &mut server_connection,
            "pane.list",
            json!([{"id": "pane-1", "program_status": "running"}]),
        );
        let result = bounded_join(worker);
        assert_eq!(
            result.expect("the running pane is returned"),
            ("tab-1".to_owned(), "pane-1".to_owned())
        );
    }

    #[test]
    fn an_ended_panes_program_gets_a_fresh_tab_in_the_same_session() {
        let (client_side, mut server_side) = bounded_pair();
        let worker = thread::spawn(move || {
            let mut stream = client_side;
            let mut connection = Connection::new(WireRole::Client, description());
            resolve_pane(&mut stream, &mut connection, "sess", "tab-1".to_owned())
        });
        let mut server_connection = Connection::new(WireRole::Server, description());
        answer_one(
            &mut server_side,
            &mut server_connection,
            "pane.list",
            json!([{"id": "pane-1", "program_status": "ended"}]),
        );
        answer_one(
            &mut server_side,
            &mut server_connection,
            "tab.new",
            json!("tab-2"),
        );
        answer_one(
            &mut server_side,
            &mut server_connection,
            "pane.list",
            json!([{"id": "pane-2", "program_status": "running"}]),
        );
        let result = bounded_join(worker);
        assert_eq!(
            result.expect("a fresh tab's pane is returned"),
            ("tab-2".to_owned(), "pane-2".to_owned())
        );
    }

    // --- second review finding: a successful attach must not be torn down by a racing
    // --- teardown that finds nothing left to interrupt (this task's own live-run bug) ---

    fn sample_pane_attached() -> PaneAttached {
        PaneAttached {
            stream: 7,
            snapshot: steg_sdk::Screen {
                rows: Vec::new(),
                cursor: steg_sdk::Cursor {
                    x: 0,
                    y: 0,
                    visible: true,
                    blinking: false,
                    shape: steg_sdk::CursorShape::Block,
                },
                // TASK-1.114: `Screen` now carries its own `size`, unused
                // by this module's own attach/interrupt-race tests - an
                // arbitrary valid size keeps this fixture compiling.
                size: steg_sdk::Size::new(80, 24),
            },
        }
    }

    /// Drives `attach_over` to a real success over a fake server, then calls the real
    /// `interrupt` - exactly what a racing `teardown` could still do right after success -
    /// and proves the connection `attach_over` handed back is still usable afterwards: one
    /// more request, one more reply, over the same stream and the same `Connection`, and
    /// its read timeout cleared (requirement 3's two halves). Proves the "too late" race
    /// from the module doc is closed: a `teardown` that only ever lands *after* a
    /// successful attach must find nothing left to interrupt.
    ///
    /// Recorded failing against the code before this fix (the slot was never cleared on
    /// success): the verification call's own reply never arrived, because `interrupt`,
    /// called unconditionally after every result, shut the connection down first.
    #[test]
    fn a_successful_attach_survives_interrupt_called_right_after() {
        let (client_side, mut server_side) = bounded_pair();
        let slot: ShutdownSlot = Arc::new(Mutex::new(Slot::Pending));
        let slot_for_worker = Arc::clone(&slot);
        let worker =
            thread::spawn(move || attach_over(client_side, &slot_for_worker, TEST_READ_TIMEOUT));

        let mut server_connection = Connection::new(WireRole::Server, description());
        answer_hello(&mut server_side, &mut server_connection);
        answer_one(
            &mut server_side,
            &mut server_connection,
            "session.list",
            json!([{"id": "sess", "label": SESSION_LABEL}]),
        );
        answer_one(
            &mut server_side,
            &mut server_connection,
            "tab.list",
            json!([{"id": "tab-1"}]),
        );
        answer_one(
            &mut server_side,
            &mut server_connection,
            "pane.list",
            json!([{"id": "pane-1", "program_status": "running"}]),
        );
        let attached_value =
            serde_json::to_value(sample_pane_attached()).expect("PaneAttached serialises");
        answer_one(
            &mut server_side,
            &mut server_connection,
            "pane.attach",
            attached_value,
        );

        let attached = bounded_join(worker).expect("attach_over succeeds");
        assert_eq!(
            attached
                .stream
                .read_timeout()
                .expect("read_timeout() succeeds"),
            None,
            "a successful attach must not carry its startup read timeout onward"
        );
        assert_eq!(
            attached
                .shutdown_handle
                .read_timeout()
                .expect("read_timeout() succeeds"),
            None,
            "a successful attach must not carry its startup read timeout onward on any clone"
        );
        let mut attached = attached;

        // The real function `AppWindow::teardown` calls, racing in right after success -
        // this must find the slot already `Cancelled` from `attach_over`'s own final claim,
        // and do nothing to the socket.
        interrupt(&slot);

        // Prove the handed-back connection is still usable: one more request, one more
        // reply, over the same stream and the same `Connection` `attach_over` returned.
        let (id, bytes) = attached
            .connection
            .call("server.show", Map::new())
            .expect("a valid call");
        attached
            .stream
            .write_all(&bytes)
            .expect("write the verification request");
        answer_one(
            &mut server_side,
            &mut server_connection,
            "server.show",
            json!({"version": "test", "uptime": 0, "sessions": 0, "clients": 0}),
        );
        let response =
            bounded(move || {
                block_until(&mut attached.stream, &mut attached.connection, |incoming| {
                    match incoming {
                        Incoming::Reply {
                            id: reply_id,
                            response,
                        } if reply_id == id => Some(response),
                        Incoming::Reply { .. }
                        | Incoming::Hello(_)
                        | Incoming::Request { .. }
                        | Incoming::Event { .. }
                        | Incoming::UnknownEvent { .. }
                        | Incoming::StreamControl { .. }
                        | Incoming::Data { .. }
                        | Incoming::UnknownStream { .. }
                        | Incoming::Malformed(_) => None,
                    }
                })
            });
        assert!(
            matches!(response, Ok(Response::Ok(_))),
            "the connection a successful attach_over returns must still be usable"
        );
    }

    /// Review comment #6's finding #1 (test c, "teardown before connect has returned"): a
    /// `teardown` landing before `attach_over` ever reaches its own publish point must not
    /// be lost. `connect()` itself is sdk-owned and not something this crate can fake a
    /// mid-retry state for without inventing scope the module doc already says is out of
    /// this crate's hands (bounded by `CONNECT_TIMEOUT` instead) - this test simulates the
    /// same ordering the real race produces without depending on a timing window actually
    /// landing: `interrupt` runs before `attach_over` is even entered, exactly as it would
    /// if `connect()` had not yet returned when the window closed.
    ///
    /// Recorded failing against the code before this fix (a plain `Option<UnixStream>`
    /// slot, checked once by `stop_startup`): finding `None` so early, it shut nothing down
    /// and fell straight into an unconditional `join()` that then waited on every
    /// subsequent read, each with no timeout of its own.
    #[test]
    fn a_teardown_that_races_in_before_the_slot_is_published_is_not_lost() {
        let (client_side, mut server_side) = bounded_pair();
        let slot: ShutdownSlot = Arc::new(Mutex::new(Slot::Pending));

        let before_interrupt = Instant::now();
        interrupt(&slot);
        assert!(
            before_interrupt.elapsed() < Duration::from_millis(50),
            "interrupt must never block the calling thread"
        );

        let started = Instant::now();
        let result = attach_over(client_side, &slot, TEST_READ_TIMEOUT);
        assert!(
            started.elapsed() < TEST_READ_TIMEOUT / 2,
            "a startup that finds the slot already cancelled before it could publish must \
             bail immediately, not wait out any part of the read timeout"
        );
        assert!(
            matches!(result, Err(StartupError::Protocol(_))),
            "a startup that finds the slot already cancelled before it could publish must \
             fail, not continue into reads nothing could interrupt a second time"
        );

        // Not just reported as failed - the socket itself must actually be shut down.
        let mut buf = [0_u8; 1];
        let read = server_side.read(&mut buf);
        assert!(
            matches!(read, Ok(0) | Err(_)),
            "a startup cancelled before publishing must still shut the socket down"
        );
    }

    /// Review comment #6's finding #1 (test b, "teardown while the server is silent
    /// mid-handshake"): once the thread is blocked waiting for the hello reply the fake
    /// server deliberately never sends, `interrupt` must still end it quickly - by shutting
    /// down the socket it finds published and unblocking the stuck read - rather than
    /// leaving it to wait out the full read timeout. Waits for the fake server to actually
    /// receive at least one byte of the client's hello before calling `interrupt`, so the
    /// ordering is real rather than a timing guess.
    ///
    /// Recorded failing against the code with `interrupt`'s own shutdown-if-`Connected`
    /// branch removed (the slot still gets marked cancelled, but nothing wakes the blocked
    /// read): the thread no longer ends "quickly" - it waits out the whole read timeout
    /// instead, bounded but no longer accelerated, exactly the gap this test exists to
    /// catch (team-lead: "check that yourself before recording").
    #[test]
    fn teardown_while_the_server_is_silent_mid_handshake_ends_quickly() {
        let (client_side, mut server_side) = bounded_pair();
        let slot: ShutdownSlot = Arc::new(Mutex::new(Slot::Pending));
        let slot_for_worker = Arc::clone(&slot);
        let started = Instant::now();
        let worker =
            thread::spawn(move || attach_over(client_side, &slot_for_worker, TEST_READ_TIMEOUT));

        // Confirms the worker reached its first blocked read (waiting for a hello reply
        // the fake server never sends) before this test interrupts it - deterministic,
        // not a sleep-based guess.
        let mut buf = [0_u8; 1];
        let read = server_side
            .read(&mut buf)
            .expect("read at least one byte of the client's hello");
        assert_ne!(
            read, 0,
            "the client should not close before this test interrupts it"
        );

        let before_interrupt = Instant::now();
        interrupt(&slot);
        assert!(
            before_interrupt.elapsed() < Duration::from_millis(50),
            "interrupt must never block the calling thread"
        );

        let result = bounded_join(worker);
        assert!(
            result.is_err(),
            "an interrupted mid-handshake read must surface as an error"
        );
        assert!(
            started.elapsed() < TEST_READ_TIMEOUT / 2,
            "a teardown landing mid-read should end the thread quickly via the early \
             shutdown, well inside the read timeout, not by waiting the timeout out"
        );
    }
}
