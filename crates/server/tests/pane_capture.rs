//! `pane.capture` over a real socket (TASK-1.139): prints a pane's screen,
//! and its scrollback when asked, from the server's own terminal with no
//! window open - proven the same way `attach.rs`/`pane_send.rs` already
//! prove the rest of the API, with real pty programs. The worker-thread
//! mechanism itself (`PaneCommand::Capture`, `render_capture`/
//! `render_rows`) and the hand-off's own lock-release proof are both
//! proven directly, without a socket, in `crates/server/src/pane.rs`'s
//! own `capture_tests` module and `crates/server/src/runtime/input.rs`'s
//! own test module; this file proves the dispatch/`Runtime` wiring on top
//! of them, through a real socket, the same split `attach.rs`'s own
//! module doc describes.

#![expect(
    clippy::unwrap_used,
    reason = "this whole file is a test; see routing.rs's own comment"
)]

use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use serde_json::{Map, Value, json};
use steg_sdk::{
    ClientKind, Connection, EndReason, ErrorCode, Incoming, PaneAttached, Response, ScreenUpdate,
    WireRole as Role, description, parse_server_hello,
};
use steg_server::{LoginShell, ServerConfig, Stopper, serve};

#[path = "support/mod.rs"]
mod support;
use support::{ScopedTempDir, scoped_socket_path};

/// This file's own connections' default read timeout - the same bound
/// `attach.rs`/`pane_send.rs` already settled on (TASK-1.123).
const DEFAULT_READ_TIMEOUT: Duration = Duration::from_secs(15);

/// A working `PATH` is required (TASK-1.87): every program this file
/// starts is given explicitly (never the caller's own `$SHELL`/`$HOME`).
fn test_login() -> LoginShell {
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

fn start_server() -> (ScopedTempDir, PathBuf, Stopper, std::thread::JoinHandle<()>) {
    let (socket_dir, socket_path) = scoped_socket_path("s.sock");
    let config = ServerConfig {
        socket_path: Some(socket_path.clone()),
        handshake_timeout: Duration::from_secs(5),
        login: test_login(),
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
    (socket_dir, socket_path, stop, handle)
}

fn stop_server(stop: &Stopper, handle: std::thread::JoinHandle<()>) {
    stop.stop();
    handle.join().expect("the server thread should not panic");
}

fn connect_and_handshake(socket_path: &std::path::Path) -> (UnixStream, Connection) {
    let mut stream = UnixStream::connect(socket_path).unwrap();
    stream.set_read_timeout(Some(DEFAULT_READ_TIMEOUT)).unwrap();
    let hello_bytes = Connection::send_hello("capture-test-client", ClientKind::Cli, None);
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

fn call_and_read_reply(
    stream: &mut UnixStream,
    connection: &mut Connection,
    method: &str,
    params: Map<String, Value>,
) -> Response {
    let (id, bytes) = connection.call(method, params).expect("a valid call");
    stream.write_all(&bytes).unwrap();
    let deadline = Instant::now() + Duration::from_secs(20); // above DEFAULT_READ_TIMEOUT (15s)
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
/// socket, and returns that pane's own full `session/tab/pane` path - the
/// same helper `attach.rs`/`pane_send.rs` each already have their own
/// copy of.
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

fn attach(stream: &mut UnixStream, connection: &mut Connection, pane: &str) -> PaneAttached {
    let mut params = Map::new();
    params.insert("target".to_string(), json!(pane));
    let (id, bytes) = connection
        .call("pane.attach", params)
        .expect("a valid call");
    stream.write_all(&bytes).unwrap();

    let deadline = Instant::now() + Duration::from_secs(20);
    let response = loop {
        match connection.poll() {
            Some(Incoming::Reply {
                id: reply_id,
                response,
            }) => {
                assert_eq!(reply_id, id, "the reply must carry the id this call minted");
                break response;
            }
            Some(Incoming::UnknownStream { stream: s, .. }) => {
                panic!(
                    "decision-2 section 3's ordering rule was violated: a frame for stream {s} \
                     arrived before the pane.attach reply naming it did"
                );
            }
            Some(_) => {}
            None => {
                assert!(
                    Instant::now() < deadline,
                    "timed out waiting for pane.attach's reply"
                );
                let mut buf = [0_u8; 65536];
                let n = stream.read(&mut buf).expect("reading the reply");
                assert!(n > 0, "server closed before replying");
                connection.receive(&buf[..n]);
            }
        }
    };
    let Response::Ok(value) = response else {
        panic!("pane.attach failed: {response:?}")
    };
    let attached: PaneAttached =
        serde_json::from_value(value).expect("pane.attach's reply is a PaneAttached");
    connection.open_stream(attached.stream);
    attached
}

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

fn next_ended(
    stream: &mut UnixStream,
    connection: &mut Connection,
    wanted_stream: u32,
    timeout: Duration,
) -> EndReason {
    let deadline = Instant::now() + timeout;
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        assert!(
            !remaining.is_zero(),
            "Ended never arrived on stream {wanted_stream}"
        );
        if let ScreenUpdate::Ended { reason } =
            next_update(stream, connection, wanted_stream, remaining)
        {
            return reason;
        }
    }
}

fn capture(
    stream: &mut UnixStream,
    connection: &mut Connection,
    pane: &str,
    scrollback: bool,
) -> Response {
    let mut params = Map::new();
    params.insert("target".to_string(), json!(pane));
    if scrollback {
        params.insert("scrollback".to_string(), json!(true));
    }
    call_and_read_reply(stream, connection, "pane.capture", params)
}

fn close_pane_forced(stream: &mut UnixStream, connection: &mut Connection, pane: &str) {
    let mut params = Map::new();
    params.insert("target".to_string(), json!(pane));
    params.insert("force".to_string(), json!(true));
    let response = call_and_read_reply(stream, connection, "pane.close", params);
    assert!(matches!(response, Response::Ok(_)), "{response:?}");
}

/// AC #1: `pane.capture` returns the pane's screen as text, matching
/// exactly what a real program echoed - not merely `contains`, so a
/// control that stops trimming trailing whitespace per row (`render_rows`,
/// `crates/server/src/pane.rs`) is caught by this test, not only by the
/// worker-level unit test in `pane.rs` itself.
#[test]
fn pane_capture_returns_the_screen_a_real_program_wrote() {
    let (_socket_dir, socket_path, stop, handle) = start_server();
    let (mut sender_stream, mut sender_connection) = connect_and_handshake(&socket_path);
    let (mut observer_stream, mut observer_connection) = connect_and_handshake(&socket_path);

    let pane = new_pane_running(&mut sender_stream, &mut sender_connection, vec!["cat"]);

    let attached = attach(&mut observer_stream, &mut observer_connection, &pane);
    let mut held = attached.snapshot.clone();

    let mut params = Map::new();
    params.insert("target".to_string(), json!(pane));
    params.insert("text".to_string(), json!("hello capture\n"));
    let response = call_and_read_reply(
        &mut sender_stream,
        &mut sender_connection,
        "pane.send",
        params,
    );
    assert!(matches!(response, Response::Ok(_)), "{response:?}");

    wait_for_marker(
        &mut observer_stream,
        &mut observer_connection,
        &mut held,
        attached.stream,
        "hello capture",
        Duration::from_secs(15),
    );

    let response = capture(&mut sender_stream, &mut sender_connection, &pane, false);
    let Response::Ok(value) = response else {
        panic!("pane.capture failed: {response:?}")
    };
    let text = value.as_str().expect("pane.capture returns a plain string");
    assert!(
        text.lines().any(|line| line == "hello capture"),
        "expected a line reading exactly \"hello capture\" (no trailing padding), got: {text:?}"
    );

    close_pane_forced(&mut sender_stream, &mut sender_connection, &pane);
    stop_server(&stop, handle);
}

/// AC #1/#3: `--scrollback` includes earlier lines ahead of the screen,
/// oldest first; without it, those same earlier lines are absent.
/// Negative control, run by hand against this exact test (not committed
/// as a toggle): with `render_capture`'s own `scrollback` branch
/// short-circuited to the plain `screen()` path regardless of the flag,
/// the "`--scrollback` must contain the earliest line" assertion below
/// failed - recorded on the task; reverted before this test was
/// committed.
#[test]
fn pane_capture_scrollback_includes_earlier_lines_only_when_asked() {
    let (_socket_dir, socket_path, stop, handle) = start_server();
    let (mut sender_stream, mut sender_connection) = connect_and_handshake(&socket_path);
    let (mut observer_stream, mut observer_connection) = connect_and_handshake(&socket_path);

    // An explicit program, never $SHELL: prints 40 numbered lines (well
    // past the pane's own default 24-row screen, TASK-1.76 AC #3 -
    // enough on its own to push the earliest lines into scrollback, no
    // declared size needed) then sits idle reading nothing more.
    let pane = new_pane_running(
        &mut sender_stream,
        &mut sender_connection,
        vec![
            "sh",
            "-c",
            "i=0; while [ $i -lt 40 ]; do echo \"line $i\"; i=$((i+1)); done; cat",
        ],
    );

    let attached = attach(&mut observer_stream, &mut observer_connection, &pane);
    let mut held = attached.snapshot.clone();
    wait_for_marker(
        &mut observer_stream,
        &mut observer_connection,
        &mut held,
        attached.stream,
        "line 39",
        Duration::from_secs(15),
    );

    let without = capture(&mut sender_stream, &mut sender_connection, &pane, false);
    let Response::Ok(without) = without else {
        panic!("pane.capture (no scrollback) failed: {without:?}")
    };
    let without_text = without.as_str().unwrap();
    assert!(
        !without_text.contains("line 0\n") && !without_text.lines().any(|l| l == "line 0"),
        "without --scrollback, the earliest printed line must not be present: {without_text:?}"
    );

    let with = capture(&mut sender_stream, &mut sender_connection, &pane, true);
    let Response::Ok(with) = with else {
        panic!("pane.capture (scrollback) failed: {with:?}")
    };
    let with_text = with.as_str().unwrap();
    let lines: Vec<&str> = with_text.lines().collect();
    let first_index = lines
        .iter()
        .position(|&l| l == "line 0")
        .expect("with --scrollback, the earliest printed line must be present");
    let last_index = lines
        .iter()
        .position(|&l| l == "line 39")
        .expect("with --scrollback, a later line must also be present");
    assert!(
        first_index < last_index,
        "scrollback must come oldest-first, ahead of the screen: {lines:?}"
    );

    close_pane_forced(&mut sender_stream, &mut sender_connection, &pane);
    stop_server(&stop, handle);
}

/// AC #3; doc-2 section 6: "A pane started with a program stays when the
/// program ends: its screen can still be captured." Negative control, run
/// by hand against this exact test (not committed as a toggle): adding a
/// mistaken `any_pane_running` refusal to `dispatch::pane::capture` made
/// this test fail (wrongly refused) - recorded on the task; reverted
/// before this test was committed.
#[test]
fn pane_capture_still_answers_after_the_program_has_exited() {
    let (_socket_dir, socket_path, stop, handle) = start_server();
    let (mut sender_stream, mut sender_connection) = connect_and_handshake(&socket_path);
    let (mut observer_stream, mut observer_connection) = connect_and_handshake(&socket_path);

    let pane = new_pane_running(
        &mut sender_stream,
        &mut sender_connection,
        vec!["sh", "-c", "printf 'before exit'; exit 0"],
    );

    let attached = attach(&mut observer_stream, &mut observer_connection, &pane);
    next_ended(
        &mut observer_stream,
        &mut observer_connection,
        attached.stream,
        Duration::from_secs(15),
    );

    let response = capture(&mut sender_stream, &mut sender_connection, &pane, false);
    let Response::Ok(value) = response else {
        panic!(
            "pane.capture must still answer once the program has ended (doc-2 section 6), got: {response:?}"
        )
    };
    let text = value.as_str().unwrap();
    assert!(
        text.contains("before exit"),
        "expected the last screen's real content, got: {text:?}"
    );

    stop_server(&stop, handle);
}

/// A pane that is fully closed (not merely ended) answers `NoSuchTarget` -
/// the ordinary `target_pane` refusal, no mechanism of this task's own.
#[test]
fn pane_capture_of_a_closed_pane_is_no_such_target() {
    let (_socket_dir, socket_path, stop, handle) = start_server();
    let (mut sender_stream, mut sender_connection) = connect_and_handshake(&socket_path);

    let pane = new_pane_running(&mut sender_stream, &mut sender_connection, vec!["cat"]);
    close_pane_forced(&mut sender_stream, &mut sender_connection, &pane);

    let response = capture(&mut sender_stream, &mut sender_connection, &pane, false);
    let Response::Error(error) = response else {
        panic!("expected NoSuchTarget for a closed pane, got: {response:?}")
    };
    assert_eq!(error.code, ErrorCode::NoSuchTarget, "{error:?}");

    stop_server(&stop, handle);
}
