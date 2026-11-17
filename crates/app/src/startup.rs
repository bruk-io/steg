//! Connects to a server, finds or makes the labelled session, attaches to its first tab's
//! first pane, and hands the result back (TASK-1.117's plan §1/§3). Blocking, start to
//! finish - run on its own dedicated thread (`Startup::spawn`), never on gpui's own
//! background executor, whose own dispatcher backs a small, fixed pool
//! (`BackgroundExecutor::num_cpus`) this task's own real, multi-second `connect()` retry
//! loop would otherwise occupy.
//!
//! **Interrupting it (review finding, TASK-1.117).** Once `connect()` returns a socket,
//! every further step is a blocking `read()` with no timeout of its own - a server that
//! accepts the connection and then stalls (never answers the handshake, or a request)
//! would otherwise hang this thread, and so `AppWindow::teardown`'s own `join()`, forever.
//! `spawn`'s own third return value is a shared slot a `try_clone()`'d handle is published
//! into the moment `connect()` succeeds, *before* any blocking read follows it - exactly
//! the reader thread's own mechanism (`client.rs`'s `Client::stop`), just published
//! earlier, since here the socket does not exist yet when the window might need to
//! interrupt it. Closing the window while still inside `connect()`'s own retry loop is the
//! one phase this cannot reach into - there is no socket yet to shut down - but that phase
//! is itself bounded by `ConnectOptions::connect_timeout` (10s, below), so the worst case
//! is a bounded wait, never the unbounded one a stalled-after-connecting server causes.

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
    /// `{session}/{tab}` - `tab.declare_size`'s own target (TASK-1.114 §2: declaring a
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

/// What a concurrent `teardown` can interrupt a still-running [`run`] with: `None` until
/// `connect()` succeeds, `Some(handle)` from that point on - see the module doc.
pub type ShutdownSlot = Arc<Mutex<Option<UnixStream>>>;

/// Starts the whole blocking sequence on its own dedicated thread and hands the result
/// back over a oneshot the caller `.await`s - never gpui's own background executor
/// (module doc). The third value is the [`ShutdownSlot`] a concurrent `teardown` locks to
/// interrupt a still-running startup (module doc).
pub fn spawn() -> (
    JoinHandle<()>,
    oneshot::Receiver<Result<Attached, StartupError>>,
    ShutdownSlot,
) {
    let shutdown_slot: ShutdownSlot = Arc::new(Mutex::new(None));
    let (sender, receiver) = oneshot::channel();
    let slot = Arc::clone(&shutdown_slot);
    let handle = thread::spawn(move || {
        let _ = sender.send(run(&slot));
    });
    (handle, receiver, shutdown_slot)
}

fn run(shutdown_slot: &ShutdownSlot) -> Result<Attached, StartupError> {
    let binary = resolve_binary()?;
    let options = ConnectOptions {
        binary,
        socket_path: None,
        autostart: true,
        connect_timeout: Duration::from_secs(10),
    };
    let stream = connect(&options).map_err(StartupError::Connect)?;
    let shutdown_handle = stream
        .try_clone()
        .map_err(|err| StartupError::Protocol(format!("could not clone the socket: {err}")))?;
    // Published *before* the first blocking read that follows (module doc) - a
    // `teardown` racing this thread sees either `None` (still inside `connect()`, its own
    // bounded wait) or `Some` (every read from here on can be interrupted).
    let interrupt_handle = shutdown_handle
        .try_clone()
        .map_err(|err| StartupError::Protocol(format!("could not clone the socket: {err}")))?;
    *shutdown_slot.lock().unwrap_or_else(PoisonError::into_inner) = Some(interrupt_handle);
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

    use super::*;

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

    // --- review finding: the startup thread can be interrupted before it is joined ------

    #[test]
    fn publishing_a_shutdown_handle_in_the_slot_unblocks_a_blocked_read() {
        let (client_side, _server_side) = UnixStream::pair().expect("a socket pair");
        let shutdown_handle = client_side.try_clone().expect("clone the socket");
        let slot: ShutdownSlot = Arc::new(Mutex::new(Some(shutdown_handle)));

        let mut stream = client_side;
        let worker = thread::spawn(move || {
            let mut connection = Connection::new(WireRole::Client, description());
            // Never matches - this call only returns when the read it is blocked in
            // returns, which is exactly what `AppWindow::stop_startup` (main.rs) needs to
            // force from outside this thread.
            block_until(&mut stream, &mut connection, |_: Incoming| None::<()>)
        });

        // The same two steps `AppWindow::stop_startup` takes: lock the slot, take and
        // shut down whatever handle is published there.
        if let Some(handle) = slot.lock().unwrap_or_else(PoisonError::into_inner).take() {
            let _ = handle.shutdown(std::net::Shutdown::Both);
        }
        let result = bounded_join(worker);
        assert!(
            result.is_err(),
            "an interrupted read should surface as an error, not hang forever"
        );
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
}
