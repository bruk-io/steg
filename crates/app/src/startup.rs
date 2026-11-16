//! Connects to a server, finds or makes the labelled session, attaches to its first tab's
//! first pane, and hands the result back (TASK-1.117's plan §1/§3). Blocking, start to
//! finish - run on its own dedicated thread (`Startup::spawn`), never on gpui's own
//! background executor, whose own dispatcher backs a small, fixed pool
//! (`BackgroundExecutor::num_cpus`) this task's own real, multi-second `connect()` retry
//! loop would otherwise occupy.

use std::env;
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
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

/// Starts the whole blocking sequence on its own dedicated thread and hands the result
/// back over a oneshot the caller `.await`s - never gpui's own background executor
/// (module doc).
pub fn spawn() -> (
    JoinHandle<()>,
    oneshot::Receiver<Result<Attached, StartupError>>,
) {
    let (sender, receiver) = oneshot::channel();
    let handle = thread::spawn(move || {
        let _ = sender.send(run());
    });
    (handle, receiver)
}

fn run() -> Result<Attached, StartupError> {
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

    let pane_target = format!("{session_id}/{tab_id}");
    let panes = ok_value(call(
        &mut stream,
        &mut connection,
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

    let (tab_id, pane_id) = if running {
        (tab_id, pane_id.to_owned())
    } else {
        // The found session's own pane has nothing live behind it (TASK-1.117's plan §3):
        // a fresh tab in the *same* session, never a second session under the same label.
        let new_tab = ok_value(call(
            &mut stream,
            &mut connection,
            "tab.new",
            params(&[("target", json!(session_id))]),
        )?)?;
        let new_tab_id = new_tab
            .as_str()
            .ok_or_else(|| StartupError::Protocol("tab.new: not a bare identifier".into()))?
            .to_owned();
        let panes = ok_value(call(
            &mut stream,
            &mut connection,
            "pane.list",
            params(&[("target", json!(format!("{session_id}/{new_tab_id}")))]),
        )?)?;
        let pane = first_record(&panes, "pane.list")?;
        let pane_id = pane
            .get("id")
            .and_then(Value::as_str)
            .ok_or_else(|| StartupError::Protocol("pane.list: a pane record with no id".into()))?
            .to_owned();
        (new_tab_id, pane_id)
    };

    let attach_target = format!("{session_id}/{tab_id}/{pane_id}");
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

    Ok(Attached {
        stream,
        connection,
        shutdown_handle,
        snapshot: attached.snapshot,
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
