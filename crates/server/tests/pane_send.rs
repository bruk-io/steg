//! `pane.send` over a real socket (TASK-1.90, AC #1/#2): delivers bytes,
//! text or a named key to a pane's own program through its pty, in
//! order, without the sending connection ever attaching to the pane -
//! proven with a real program in a real pty, the same shape `attach.rs`
//! already uses for `pane.attach`. "Without a client attached" is
//! proven structurally here: the connection that calls `pane.send` in
//! every test below never calls `pane.attach` at all; a second,
//! separate connection does the attaching, purely to observe what
//! arrived.
//!
//! A new file, not `attach.rs`: TASK-1.123 is actively fixing bounds in
//! that file, and the helpers below (a per-test socket directory, the
//! handshake, `call_and_read_reply`, `attach`/`next_update`) are each
//! file's own small, self-contained copies - the established convention
//! in this crate (`tests/support/mod.rs`'s own doc: "each file under
//! `tests/` is its own compilation unit"), not a new shared module.

#![expect(
    clippy::unwrap_used,
    reason = "this whole file is a test; see routing.rs's own comment"
)]

use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use serde_json::{Map, Value, json};
use steg_sdk::{
    ClientKind, Connection, ErrorCode, Incoming, PaneAttached, Response, ScreenUpdate,
    WireRole as Role, description, parse_server_hello,
};
use steg_server::{LoginShell, ServerConfig, Stopper, serve};

/// A fresh, per-test temporary *directory* (not merely a uniquely-named
/// file in the one shared `temp_dir()` every other file in this crate
/// still uses) - TASK-1.123 is about to replace those files' own
/// duplicated `unique_socket_path` helpers with one shared one; a
/// per-test directory here, rather than copying that pattern again, is
/// the smaller target for that swap, and is itself a stronger guard
/// against the kind of path collision across concurrent worktrees
/// TASK-1.123's own sweep already found once (comment on that task,
/// 2026-11-18).
fn test_socket_dir() -> PathBuf {
    // Short, on purpose: a Unix socket's own path has a small fixed
    // limit (macOS: 104 bytes including the terminator), and
    // `std::env::temp_dir()` on macOS is already a long, randomised
    // path - a verbose directory name on top of it, plus "/steg.sock",
    // can overflow that limit with no warning beyond a connection that
    // never succeeds (found while building this file). `pid` plus a
    // per-process counter is already unique for this test binary's own
    // life, which is all one test run ever needs.
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let suffix = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("psend-{}-{suffix}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("creating this test's own temp directory");
    dir
}

/// A working `PATH` is required (TASK-1.87) - every program this file
/// starts is given explicitly (never the caller's own `$SHELL`/`HOME`).
fn test_login(home: &std::path::Path) -> LoginShell {
    LoginShell {
        argv0: "/bin/sh".to_string(),
        home: home.to_str().unwrap().to_string(),
        user: None,
        base_env: vec![(
            std::ffi::OsString::from("PATH"),
            std::env::var_os("PATH").unwrap_or_default(),
        )],
    }
}

/// Starts a real server on its own socket, inside its own temporary
/// directory - `dir` is returned too, so the caller can remove it once
/// the server has stopped.
fn start_server() -> (PathBuf, PathBuf, Stopper, std::thread::JoinHandle<()>) {
    let dir = test_socket_dir();
    let socket_path = dir.join("s.sock");
    let config = ServerConfig {
        socket_path: Some(socket_path.clone()),
        handshake_timeout: Duration::from_secs(5),
        login: test_login(&dir),
        ..ServerConfig::default()
    };
    let stop = Stopper::new();
    let handle = {
        let config = config.clone();
        let stop = stop.clone();
        std::thread::spawn(move || {
            let _ = serve(&config, &stop, None);
        })
    };
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if UnixStream::connect(&socket_path).is_ok() {
            break;
        }
        assert!(Instant::now() < deadline, "server never started accepting");
        std::thread::sleep(Duration::from_millis(5));
    }
    (socket_path, dir, stop, handle)
}

/// Stops the server and removes its own temporary directory - every test
/// below calls this on its own way out (the same explicit-call
/// convention `attach.rs` already uses; a mid-test assertion panic
/// skipping it is an accepted, pre-existing risk in this crate, not
/// something this task changes).
fn stop_server(dir: &std::path::Path, stop: &Stopper, handle: std::thread::JoinHandle<()>) {
    stop.stop();
    handle.join().expect("the server thread should not panic");
    let _ = std::fs::remove_dir_all(dir);
}

fn connect_and_handshake(socket_path: &std::path::Path) -> (UnixStream, Connection) {
    let mut stream = UnixStream::connect(socket_path).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let hello_bytes = Connection::send_hello("pane-send-test-client", ClientKind::Cli, None);
    stream.write_all(&hello_bytes).unwrap();

    let mut connection = Connection::new(Role::Client, description());
    let hello_reply = loop {
        if let Some(Incoming::Hello(payload)) = connection.poll() {
            break payload;
        }
        let mut buf = [0_u8; 4096];
        let n = stream
            .read(&mut buf)
            .expect("reading the server's hello reply");
        assert!(n > 0, "server closed before replying to the hello");
        connection.receive(&buf[..n]);
    };
    parse_server_hello(&hello_reply).expect("a matching protocol version should be accepted");
    (stream, connection)
}

/// Writes a validated call and reads back its matching reply - the same
/// shape `attach.rs`'s own `call_and_read_reply` already has.
fn call_and_read_reply(
    stream: &mut UnixStream,
    connection: &mut Connection,
    method: &str,
    params: Map<String, Value>,
) -> Response {
    let (id, bytes) = connection.call(method, params).expect("a valid call");
    stream.write_all(&bytes).unwrap();
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        match connection.poll() {
            Some(Incoming::Reply {
                id: reply_id,
                response,
            }) => {
                assert_eq!(reply_id, id, "the reply must carry the id this call minted");
                return response;
            }
            Some(_) => continue,
            None => {}
        }
        assert!(
            Instant::now() < deadline,
            "timed out waiting for a reply to {method}"
        );
        let mut buf = [0_u8; 65536];
        let n = stream.read(&mut buf).expect("reading the reply");
        assert!(n > 0, "server closed before replying");
        connection.receive(&buf[..n]);
    }
}

/// Like `call_and_read_reply`, but a request that is expected to fail
/// validation before `connection.call` would even build it (an unknown
/// key name against `ValueKind::Choice`) is sent through
/// `connection.call_unvalidated` instead, so the server's own refusal is
/// what this file asserts on - not the sdk client's own, earlier one.
fn call_unvalidated_and_read_reply(
    stream: &mut UnixStream,
    connection: &mut Connection,
    method: &str,
    params: Map<String, Value>,
) -> Response {
    let (id, bytes) = connection.call_unvalidated(method, params);
    stream.write_all(&bytes).unwrap();
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        match connection.poll() {
            Some(Incoming::Reply {
                id: reply_id,
                response,
            }) => {
                assert_eq!(reply_id, id, "the reply must carry the id this call minted");
                return response;
            }
            Some(_) => continue,
            None => {}
        }
        assert!(
            Instant::now() < deadline,
            "timed out waiting for a reply to {method}"
        );
        let mut buf = [0_u8; 65536];
        let n = stream.read(&mut buf).expect("reading the reply");
        assert!(n > 0, "server closed before replying");
        connection.receive(&buf[..n]);
    }
}

/// Starts a real session whose one pane runs `program`, over the real
/// socket - the same shape `attach.rs`'s own `new_pane_running` already
/// has, returning the full `session/tab/pane` path (`pane.send`'s own
/// target needs no context - this connection has none).
fn new_pane_running(
    stream: &mut UnixStream,
    connection: &mut Connection,
    program: Vec<&str>,
) -> String {
    let mut params = Map::new();
    params.insert("program".to_string(), json!(program));
    let response = call_and_read_reply(stream, connection, "session.new", params);
    let Response::Ok(session_id) = response else {
        panic!("session.new failed: {response:?}")
    };
    let session_id = session_id
        .get("id")
        .and_then(Value::as_str)
        .expect("session.new returns {\"id\": ...}");

    let mut params = Map::new();
    params.insert("target".to_string(), json!(session_id));
    let response = call_and_read_reply(stream, connection, "tab.list", params);
    let Response::Ok(tabs) = response else {
        panic!("tab.list failed: {response:?}")
    };
    let tab_id = tabs
        .as_array()
        .expect("tab.list returns an array")
        .first()
        .expect("the new session has one tab")
        .get("id")
        .and_then(Value::as_str)
        .expect("each tab record has an id")
        .to_string();

    let mut params = Map::new();
    params.insert(
        "target".to_string(),
        json!(format!("{session_id}/{tab_id}")),
    );
    let response = call_and_read_reply(stream, connection, "pane.list", params);
    let Response::Ok(panes) = response else {
        panic!("pane.list failed: {response:?}")
    };
    let pane_id = panes
        .as_array()
        .expect("pane.list returns an array")
        .first()
        .expect("the new session has one pane")
        .get("id")
        .and_then(Value::as_str)
        .expect("each pane record has an id");

    format!("{session_id}/{tab_id}/{pane_id}")
}

/// Calls `pane.attach`, decodes the reply as `PaneAttached`, and opens
/// the new stream on `connection` - the same shape `attach.rs`'s own
/// `attach` already has.
fn attach(stream: &mut UnixStream, connection: &mut Connection, pane: &str) -> PaneAttached {
    let mut params = Map::new();
    params.insert("target".to_string(), json!(pane));
    let response = call_and_read_reply(stream, connection, "pane.attach", params);
    let Response::Ok(value) = response else {
        panic!("pane.attach failed: {response:?}")
    };
    let attached: PaneAttached =
        serde_json::from_value(value).expect("pane.attach's reply is a PaneAttached");
    connection.open_stream(attached.stream);
    attached
}

/// Reads one `Incoming::StreamControl` for `wanted_stream`, decoded as a
/// `ScreenUpdate` - bounded, via a real read with a timeout, never a
/// sleep-poll. The same shape `attach.rs`'s own `next_update` already
/// has.
fn next_update(
    stream: &mut UnixStream,
    connection: &mut Connection,
    wanted_stream: u32,
    timeout: Duration,
) -> ScreenUpdate {
    let deadline = Instant::now() + timeout;
    loop {
        match connection.poll() {
            Some(Incoming::StreamControl {
                stream: frame_stream,
                payload,
            }) if frame_stream == wanted_stream => {
                return serde_json::from_slice(&payload)
                    .expect("a screen update always deserialises");
            }
            Some(_) => {}
            None => {
                assert!(
                    Instant::now() < deadline,
                    "no update arrived on stream {wanted_stream} within {timeout:?}"
                );
                stream
                    .set_read_timeout(Some(Duration::from_millis(200)))
                    .unwrap();
                let mut buf = [0_u8; 65536];
                match stream.read(&mut buf) {
                    Ok(0) => panic!("the server closed before an update arrived"),
                    Ok(n) => connection.receive(&buf[..n]),
                    Err(e)
                        if e.kind() == std::io::ErrorKind::WouldBlock
                            || e.kind() == std::io::ErrorKind::TimedOut => {}
                    Err(e) => panic!("read failed: {e}"),
                }
            }
        }
    }
}

fn apply_update(held: &mut steg_sdk::Screen, update: ScreenUpdate) {
    match update {
        ScreenUpdate::Unchanged | ScreenUpdate::Ended { .. } => {}
        ScreenUpdate::Full { screen, .. } => *held = screen,
        ScreenUpdate::Delta { rows, cursor, .. } => {
            for (position, row) in rows {
                if let Some(slot) = held.rows.get_mut(usize::from(position)) {
                    *slot = row;
                }
            }
            held.cursor = cursor;
        }
    }
}

fn screen_text(screen: &steg_sdk::Screen) -> String {
    screen
        .rows
        .iter()
        .map(|row| row.iter().map(|run| run.text.as_str()).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n")
}

/// Polls `attached`'s own stream (bounded, via real reads, never a
/// sleep-loop) until `held`'s own text contains `marker` - the proof
/// that the bytes `pane.send` delivered actually reached the program's
/// own stdin and were echoed back by it, not merely accepted by the
/// server.
fn wait_for_marker(
    stream: &mut UnixStream,
    connection: &mut Connection,
    held: &mut steg_sdk::Screen,
    attachment_stream: u32,
    marker: &str,
    timeout: Duration,
) {
    let deadline = Instant::now() + timeout;
    while !screen_text(held).contains(marker) {
        assert!(
            Instant::now() < deadline,
            "never saw {marker:?}; last screen was: {:?}",
            screen_text(held)
        );
        let remaining = deadline.saturating_duration_since(Instant::now());
        let update = next_update(stream, connection, attachment_stream, remaining);
        apply_update(held, update);
    }
}

/// A program that prints `GOT:<line>` for every line it reads from its
/// own stdin - deterministic proof that bytes reached the program
/// (through its pty) and in what order, independent of whatever the
/// pty's own local echo does or does not show (unlike `cat`, which would
/// make the two indistinguishable).
fn echo_lines_program() -> Vec<&'static str> {
    vec![
        "sh",
        "-c",
        "while IFS= read -r line; do printf 'GOT:%s\\n' \"$line\"; done",
    ]
}

#[test]
fn pane_send_delivers_text_then_a_key_in_order_without_a_client_attached() {
    let (socket_path, dir, stop, handle) = start_server();
    // One connection creates the pane and sends to it; it never calls
    // `pane.attach` at all, anywhere in this test - the structural proof
    // of "without a client attached" (AC #1).
    let (mut sender_stream, mut sender_connection) = connect_and_handshake(&socket_path);
    let pane = new_pane_running(
        &mut sender_stream,
        &mut sender_connection,
        echo_lines_program(),
    );

    // A second, separate connection only ever observes.
    let (mut observer_stream, mut observer_connection) = connect_and_handshake(&socket_path);
    let attached = attach(&mut observer_stream, &mut observer_connection, &pane);
    let mut held = attached.snapshot;

    let mut params = Map::new();
    params.insert("target".to_string(), json!(pane));
    params.insert("text".to_string(), json!("hello"));
    let response = call_and_read_reply(
        &mut sender_stream,
        &mut sender_connection,
        "pane.send",
        params,
    );
    assert!(
        matches!(response, Response::Ok(Value::Null)),
        "pane.send --text: {response:?}"
    );

    let mut params = Map::new();
    params.insert("target".to_string(), json!(pane));
    params.insert("key".to_string(), json!("enter"));
    let response = call_and_read_reply(
        &mut sender_stream,
        &mut sender_connection,
        "pane.send",
        params,
    );
    assert!(
        matches!(response, Response::Ok(Value::Null)),
        "pane.send --key enter: {response:?}"
    );

    wait_for_marker(
        &mut observer_stream,
        &mut observer_connection,
        &mut held,
        attached.stream,
        "GOT:hello",
        Duration::from_secs(15),
    );

    stop_server(&dir, &stop, handle);
}

#[test]
fn pane_send_refuses_a_pane_with_no_running_program() {
    let (socket_path, dir, stop, handle) = start_server();
    let (mut stream, mut connection) = connect_and_handshake(&socket_path);
    let pane = new_pane_running(&mut stream, &mut connection, vec!["sh", "-c", "exit 0"]);

    // Waits for the real exit (R97: the pane stays, with no program
    // running) by polling `pane.show`'s own `program_status` - bounded,
    // no event for this exists on the client side.
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        let mut params = Map::new();
        params.insert("target".to_string(), json!(pane));
        let response = call_and_read_reply(&mut stream, &mut connection, "pane.show", params);
        let Response::Ok(record) = response else {
            panic!("pane.show failed: {response:?}")
        };
        if record["program_status"] == json!("ended") {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "the real `exit 0` never reached Ended within the bound"
        );
        std::thread::sleep(Duration::from_millis(20));
    }

    let mut params = Map::new();
    params.insert("target".to_string(), json!(pane));
    params.insert("text".to_string(), json!("x"));
    let response = call_and_read_reply(&mut stream, &mut connection, "pane.send", params);
    let Response::Error(error) = response else {
        panic!("expected pane.send to refuse an already-ended pane, got {response:?}")
    };
    assert_eq!(error.code, ErrorCode::ProgramNotRunning);

    stop_server(&dir, &stop, handle);
}

#[test]
fn pane_send_refuses_text_and_key_together() {
    let (socket_path, dir, stop, handle) = start_server();
    let (mut stream, mut connection) = connect_and_handshake(&socket_path);
    let pane = new_pane_running(&mut stream, &mut connection, echo_lines_program());

    let mut params = Map::new();
    params.insert("target".to_string(), json!(pane));
    params.insert("text".to_string(), json!("hello"));
    params.insert("key".to_string(), json!("enter"));
    let response = call_and_read_reply(&mut stream, &mut connection, "pane.send", params);
    let Response::Error(error) = response else {
        panic!("expected pane.send to refuse text and key together, got {response:?}")
    };
    assert_eq!(error.code, ErrorCode::BadRequest);

    stop_server(&dir, &stop, handle);
}

#[test]
fn pane_send_with_an_unknown_key_name_is_bad_request() {
    let (socket_path, dir, stop, handle) = start_server();
    let (mut stream, mut connection) = connect_and_handshake(&socket_path);
    let pane = new_pane_running(&mut stream, &mut connection, echo_lines_program());

    let mut params = Map::new();
    params.insert("target".to_string(), json!(pane));
    params.insert("key".to_string(), json!("not-a-real-key"));
    let response =
        call_unvalidated_and_read_reply(&mut stream, &mut connection, "pane.send", params);
    let Response::Error(error) = response else {
        panic!("expected pane.send to refuse an unknown key name, got {response:?}")
    };
    assert_eq!(error.code, ErrorCode::BadRequest);

    stop_server(&dir, &stop, handle);
}
