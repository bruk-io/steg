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
//! that file, and the helpers below (the handshake, `call_and_read_reply`,
//! `attach`/`next_update`) are each file's own small, self-contained
//! copies - the established convention in this crate (`tests/support/
//! mod.rs`'s own doc: "each file under `tests/` is its own compilation
//! unit"), not a new shared module. The per-test socket directory itself
//! *is* shared, through `support::scoped_socket_path` (TASK-1.137): this
//! file's own former copy of that one helper is now the same as the
//! other nine `crates/server/tests/*.rs` files already use.

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
    WireRole as Role, description, key_names, parse_server_hello,
};
use steg_server::{LoginShell, ServerConfig, Stopper, serve};

#[path = "support/mod.rs"]
mod support;
use support::{ScopedTempDir, run_bounded, scoped_socket_path};

/// A working `PATH` is required (TASK-1.87) - every program this file
/// starts is given explicitly (never the caller's own `$SHELL`/`HOME`).
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

/// Starts a real server on its own socket, inside its own scoped
/// temporary directory (`support::scoped_socket_path`) - the directory
/// is returned too, so the caller keeps it alive (and, on drop, removes
/// it) for as long as the socket path must stay valid.
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

/// Stops the server - every test below calls this on its own way out
/// (the same explicit-call convention `attach.rs` already uses; a
/// mid-test assertion panic skipping it is an accepted, pre-existing
/// risk in this crate, not something this task changes). The socket
/// directory's own removal is `ScopedTempDir`'s `Drop`, not this
/// function's job now.
fn stop_server(stop: &Stopper, handle: std::thread::JoinHandle<()>) {
    stop.stop();
    run_bounded(
        "the server thread to end after stop()",
        Duration::from_secs(10),
        move || {
            handle.join().expect("the server thread should not panic");
        },
    );
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

/// The tab a `new_pane_running`/`attach` pane path names, as its own
/// target string (`session/tab`, the pane's own last segment dropped) -
/// `tab.declare-size`'s own target - the same shape `attach.rs`'s own
/// `tab_path_of` already has.
fn tab_path_of(pane_path: &str) -> String {
    let (tab_path, _pane) = pane_path
        .rsplit_once('/')
        .expect("a pane path is always session/tab/pane");
    tab_path.to_string()
}

/// `tab.declare-size` - the same bounded call-and-read-reply shape every
/// other method in this file uses, over the sdk's own typed helper.
fn declare_size(
    stream: &mut UnixStream,
    connection: &mut Connection,
    tab: &str,
    columns: u16,
    rows: u16,
) -> Response {
    let (id, bytes) = connection
        .declare_size(Some(tab), columns, rows)
        .expect("a valid call");
    stream.write_all(&bytes).unwrap();
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        if let Some(Incoming::Reply {
            id: reply_id,
            response,
        }) = connection.poll()
        {
            assert_eq!(reply_id, id, "the reply must carry the id this call minted");
            return response;
        }
        assert!(
            Instant::now() < deadline,
            "timed out waiting for tab.declare-size's reply"
        );
        stream
            .set_read_timeout(Some(Duration::from_millis(200)))
            .unwrap();
        let mut buf = [0_u8; 65536];
        match stream.read(&mut buf) {
            Ok(0) => panic!("server closed before replying"),
            Ok(n) => connection.receive(&buf[..n]),
            Err(e)
                if e.kind() == std::io::ErrorKind::WouldBlock
                    || e.kind() == std::io::ErrorKind::TimedOut => {}
            Err(e) => panic!("read failed: {e}"),
        }
    }
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

/// Reads updates on `wanted_stream` until `Ended` arrives, discarding
/// anything else first - the same shape `attach.rs`'s own `next_ended`
/// already has (TASK-1.128): a program exiting on its own ends the
/// pane's own *attachment* stream (`ProgramExited`, doc-14) even though
/// the pane itself stays, R97 - this is the event to wait on for "the
/// real exit has happened", not a poll of `pane.show`.
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

/// Reads and applies whatever updates arrive on `attachment_stream` for
/// `window`, then returns - never panics on its own; the caller compares
/// `held` before and after to see whether anything changed. A bounded
/// wait for the *absence* of an event cannot be anything but a window of
/// real time (there is no "nothing happened" event to wait on) - the
/// same shape `attach.rs`'s own `assert_nothing_more_arrives_on` already
/// uses for the identical reason.
fn drain_updates_for(
    stream: &mut UnixStream,
    connection: &mut Connection,
    held: &mut steg_sdk::Screen,
    attachment_stream: u32,
    window: Duration,
) {
    let quiet_until = Instant::now() + window;
    while Instant::now() < quiet_until {
        stream
            .set_read_timeout(Some(Duration::from_millis(50)))
            .unwrap();
        let mut buf = [0_u8; 65536];
        match stream.read(&mut buf) {
            Ok(0) => panic!("the server closed while waiting to see if anything else arrived"),
            Ok(n) => connection.receive(&buf[..n]),
            Err(e)
                if e.kind() == std::io::ErrorKind::WouldBlock
                    || e.kind() == std::io::ErrorKind::TimedOut => {}
            Err(e) => panic!("read failed: {e}"),
        }
        while let Some(incoming) = connection.poll() {
            if let Incoming::StreamControl { stream: s, payload } = incoming
                && s == attachment_stream
            {
                let update: ScreenUpdate =
                    serde_json::from_slice(&payload).expect("a screen update always deserialises");
                apply_update(held, update);
            }
        }
    }
}

/// Polls `attached`'s own stream until some line of `held`'s own text,
/// trimmed of the leading/trailing padding `od`'s own fixed-width
/// columns add, equals `expected_line` exactly - unlike
/// [`wait_for_marker`]'s substring match, an exact-line comparison is
/// what lets [`pane_send_delivers_each_named_key_with_its_documented_
/// bytes`] tell one named key's own hex dump apart from another's with
/// no risk of one being a substring of another.
fn wait_for_exact_line(
    stream: &mut UnixStream,
    connection: &mut Connection,
    held: &mut steg_sdk::Screen,
    attachment_stream: u32,
    expected_line: &str,
    timeout: Duration,
) {
    let deadline = Instant::now() + timeout;
    loop {
        if screen_text(held)
            .lines()
            .any(|line| line.trim() == expected_line)
        {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "never saw the line {expected_line:?}; last screen was: {:?}",
            screen_text(held)
        );
        let remaining = deadline.saturating_duration_since(Instant::now());
        let update = next_update(stream, connection, attachment_stream, remaining);
        apply_update(held, update);
    }
}

/// The non-empty, trimmed lines of `screen`'s own text - a one-byte-
/// per-line hex dump (see
/// [`pane_send_delivers_each_named_key_with_its_documented_bytes`]'s own
/// shell program) produces one blank line between invocations, checked
/// directly while building this test; skipping empty lines after
/// trimming is all that takes to get a clean, ordered "one line per byte
/// actually read" transcript.
fn hex_lines(screen: &steg_sdk::Screen) -> Vec<String> {
    screen_text(screen)
        .lines()
        .map(str::trim)
        .filter(|line| line.len() == 2 && line.chars().all(|c| c.is_ascii_hexdigit()))
        .map(str::to_string)
        .collect()
}

/// Waits until `hex_lines(held)` holds at least `wanted` lines, applying
/// every update along the way - bounded, via real reads, never a
/// sleep-poll.
fn wait_for_hex_line_count(
    stream: &mut UnixStream,
    connection: &mut Connection,
    held: &mut steg_sdk::Screen,
    attachment_stream: u32,
    wanted: usize,
    timeout: Duration,
) {
    let deadline = Instant::now() + timeout;
    while hex_lines(held).len() < wanted {
        assert!(
            Instant::now() < deadline,
            "never saw {wanted} hex-dump lines; last screen was: {:?}",
            screen_text(held)
        );
        let remaining = deadline.saturating_duration_since(Instant::now());
        let update = next_update(stream, connection, attachment_stream, remaining);
        apply_update(held, update);
    }
}

/// doc-2's own documented byte sequence for each named key - written
/// here independently of [`steg_sdk::named_key_bytes`], the function
/// [`pane_send_delivers_each_named_key_with_its_documented_bytes`]
/// actually exercises through the real pipeline: this table and that
/// function must never be the same definition read twice, or a wrong
/// byte sequence in the sdk's own table could never disagree with
/// itself. Only the *name* list this test iterates
/// ([`steg_sdk::key_names`]) comes from the sdk - so a key added there
/// is covered automatically, but what each one is supposed to send is
/// restated here, by hand, from doc-9's own xterm/ECMA-48 convention,
/// the same source `crates/sdk/src/keys.rs`'s own table cites.
fn documented_bytes(name: &str) -> &'static [u8] {
    match name {
        "enter" => b"\r",
        "tab" => b"\t",
        "backspace" => b"\x7f",
        "escape" => b"\x1b",
        "left" => b"\x1b[D",
        "right" => b"\x1b[C",
        "up" => b"\x1b[A",
        "down" => b"\x1b[B",
        "home" => b"\x1b[H",
        "end" => b"\x1b[F",
        "pageup" => b"\x1b[5~",
        "pagedown" => b"\x1b[6~",
        "insert" => b"\x1b[2~",
        "delete" => b"\x1b[3~",
        "f1" => b"\x1bOP",
        "f2" => b"\x1bOQ",
        "f3" => b"\x1bOR",
        "f4" => b"\x1bOS",
        "f5" => b"\x1b[15~",
        "f6" => b"\x1b[17~",
        "f7" => b"\x1b[18~",
        "f8" => b"\x1b[19~",
        "f9" => b"\x1b[20~",
        "f10" => b"\x1b[21~",
        "f11" => b"\x1b[23~",
        "f12" => b"\x1b[24~",
        other => panic!(
            "{other} is in steg_sdk::key_names() but this test has no documented bytes for it \
             - add a case above"
        ),
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
    let (_socket_dir, socket_path, stop, handle) = start_server();
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

    stop_server(&stop, handle);
}

#[test]
fn pane_send_delivers_each_named_key_with_its_documented_bytes() {
    let (_socket_dir, socket_path, stop, handle) = start_server();
    let (mut sender_stream, mut sender_connection) = connect_and_handshake(&socket_path);
    // `stty -icanon` so this pane's own pty delivers every byte as soon
    // as it arrives, rather than buffering a whole line (none of these
    // ESC sequences end in a newline); `min 1 time 0`, spelled out
    // rather than left at whatever `-icanon` happened to leave them,
    // since those two values do double duty between canonical and
    // non-canonical mode and leaving them unset can make a read block
    // waiting for more bytes than a short key sequence ever sends
    // (found while building this test). `-echo` so the raw bytes are
    // never echoed back onto the screen itself, which a VT100 emulator
    // would read as real control sequences - most of these keys *are*
    // valid cursor-movement/function-key sequences, not printable text,
    // so an echoed one would move the cursor rather than show up as
    // text. `-icrnl` so a real carriage return (`enter`'s own `\r`) is
    // not translated to `\n` before this program ever sees it (also
    // found directly while building this test - without it, `enter`
    // reads back as `0a`, not `0d`).
    //
    // One `dd`+`od` pair per byte, not one long-running `od`: `od`
    // itself turned out to buffer its *input* internally and never
    // flush until either its own buffer filled or it saw end-of-file -
    // neither ever happens on an open pty with a few bytes at a time
    // (confirmed directly: a single long-running `od -An -tx1` here
    // produced nothing at all for a 3-byte write, for minutes). `dd
    // bs=1 count=1` reads exactly one byte and closes its own stdout,
    // which gives the `od` downstream of it the end-of-file it needs to
    // flush that one byte immediately - one hex-digit pair per line,
    // the unit [`hex_lines`] reads back.
    let pane = new_pane_running(
        &mut sender_stream,
        &mut sender_connection,
        vec![
            "sh",
            "-c",
            "stty -icanon -echo -icrnl min 1 time 0; printf 'READY\\n'; \
             while :; do dd bs=1 count=1 2>/dev/null | od -An -tx1; done",
        ],
    );

    let (mut observer_stream, mut observer_connection) = connect_and_handshake(&socket_path);
    let attached = attach(&mut observer_stream, &mut observer_connection, &pane);
    let mut held = attached.snapshot;

    // A tall terminal, declared once: 26 named keys each print a
    // hex-digit line plus a blank line, and the default 24-row screen
    // (DEFAULT_PANE_SIZE) would start scrolling well before the last
    // one - found directly while building this test, where the keys
    // after the screen's own height stopped being found at all, since
    // `hex_lines` only ever reads the *active* screen, never
    // scrollback. Comfortably more rows than this test could ever need.
    let response = declare_size(
        &mut observer_stream,
        &mut observer_connection,
        &tab_path_of(&pane),
        80,
        1000,
    );
    assert!(
        matches!(response, Response::Ok(_)),
        "tab.declare-size: {response:?}"
    );
    wait_for_exact_line(
        &mut observer_stream,
        &mut observer_connection,
        &mut held,
        attached.stream,
        "READY",
        Duration::from_secs(15),
    );

    // How many hex-dump lines (one per byte actually read) have already
    // been matched against an earlier key in this loop - the next key's
    // own bytes must land in the lines *after* this point, never
    // re-matched against a previous key's own leftover output.
    let mut consumed = 0_usize;
    for name in key_names() {
        let expected_bytes = documented_bytes(&name);
        let expected_hex: Vec<String> = expected_bytes.iter().map(|b| format!("{b:02x}")).collect();

        let mut params = Map::new();
        params.insert("target".to_string(), json!(pane));
        params.insert("key".to_string(), json!(name));
        let response = call_and_read_reply(
            &mut sender_stream,
            &mut sender_connection,
            "pane.send",
            params,
        );
        assert!(
            matches!(response, Response::Ok(Value::Null)),
            "pane.send --key {name}: {response:?}"
        );

        wait_for_hex_line_count(
            &mut observer_stream,
            &mut observer_connection,
            &mut held,
            attached.stream,
            consumed + expected_hex.len(),
            Duration::from_secs(15),
        );
        let lines = hex_lines(&held);
        let actual = &lines[consumed..consumed + expected_hex.len()];
        assert_eq!(actual, expected_hex.as_slice(), "key {name:?}");
        consumed += expected_hex.len();
    }

    stop_server(&stop, handle);
}

#[test]
fn pane_send_refuses_a_pane_with_no_running_program() {
    let (_socket_dir, socket_path, stop, handle) = start_server();
    let (mut sender_stream, mut sender_connection) = connect_and_handshake(&socket_path);
    let pane = new_pane_running(
        &mut sender_stream,
        &mut sender_connection,
        vec!["sh", "-c", "exit 0"],
    );

    // Attached *before* the program exits, so this connection's own
    // snapshot/updates cover the whole of the program's short life.
    let (mut observer_stream, mut observer_connection) = connect_and_handshake(&socket_path);
    let attached = attach(&mut observer_stream, &mut observer_connection, &pane);

    // Waits for the real exit through the pane's own attachment stream
    // (TASK-1.128's `Ended`), not by polling `pane.show` - the plan's
    // own choice, changed here (Departure, named in the notes): the
    // event already exists and needs no sleep-poll in its place.
    let reason = next_ended(
        &mut observer_stream,
        &mut observer_connection,
        attached.stream,
        Duration::from_secs(15),
    );
    assert_eq!(reason, EndReason::ProgramExited { exit_code: 0 });

    let before = screen_text(&attached.snapshot);

    let mut params = Map::new();
    params.insert("target".to_string(), json!(pane));
    params.insert("text".to_string(), json!("x"));
    let response = call_and_read_reply(
        &mut sender_stream,
        &mut sender_connection,
        "pane.send",
        params,
    );

    // A real second read, not the same binding compared with itself
    // (round 2's own finding): a fresh `pane.attach` to this same,
    // already-ended pane answers with one more snapshot of its own
    // (doc-14: "attaching to a pane whose program has already ended
    // answers with a snapshot ... and then `Ended`, and nothing else"),
    // taken *after* the refused `pane.send` above. Checked before the
    // error-code assertion, the same ordering `pane_send_refuses_text_
    // and_key_together` uses, so a control that broke the refusal would
    // fail here first, if this comparison can fail at all (see below).
    let after_attached = attach(&mut observer_stream, &mut observer_connection, &pane);
    let after = screen_text(&after_attached.snapshot);

    // What this can prove, honestly, and no more: the pane's own worker
    // thread is still alive (R97 keeps it, with no program running),
    // but its `PaneCommand::Input` arm is a no-op once `current` is
    // `None` (confirmed directly against `crates/server/src/pane.rs`) -
    // so a write that reached it anyway would still change nothing a
    // second attach could see. This comparison is corroboration, not
    // the proof: it cannot, on its own, tell "refused" apart from
    // "silently accepted and had nothing to show for it" - the error
    // code, asserted below, is the real, controlled proof (this task's
    // own notes name the recorded control: `any_pane_running` inverted
    // fails on the error-code assertion, never on this comparison,
    // confirming it genuinely cannot fail by this route).
    assert_eq!(
        after, before,
        "corroboration only (see above): a second attach to this already-ended pane must see \
         the same content either way"
    );

    let Response::Error(error) = response else {
        panic!("expected pane.send to refuse an already-ended pane, got {response:?}")
    };
    assert_eq!(error.code, ErrorCode::ProgramNotRunning);

    stop_server(&stop, handle);
}

#[test]
fn pane_send_refuses_text_and_key_together() {
    let (_socket_dir, socket_path, stop, handle) = start_server();
    let (mut sender_stream, mut sender_connection) = connect_and_handshake(&socket_path);
    let pane = new_pane_running(
        &mut sender_stream,
        &mut sender_connection,
        echo_lines_program(),
    );

    // This pane is still running, so this is the real proof (unlike the
    // already-ended pane above): bytes that actually reached it would
    // be echoed back by its own program and show up here.
    let (mut observer_stream, mut observer_connection) = connect_and_handshake(&socket_path);
    let attached = attach(&mut observer_stream, &mut observer_connection, &pane);
    let mut held = attached.snapshot;
    let before = screen_text(&held);

    let mut params = Map::new();
    params.insert("target".to_string(), json!(pane));
    params.insert("text".to_string(), json!("hello"));
    params.insert("key".to_string(), json!("enter"));
    let response = call_and_read_reply(
        &mut sender_stream,
        &mut sender_connection,
        "pane.send",
        params,
    );

    // Checked *before* the error code, deliberately: this is the real
    // proof (see above), and keeping it first means a control that
    // removes the both-given check (so `response` itself would wrongly
    // become `Ok`, text's own bytes delivered) still fails here, on the
    // thing that actually matters, rather than only on the response
    // shape a step below never gets to run.
    //
    // A window bounded below, not a sleep used as a wait - the same
    // shape `attach.rs`'s own `assert_nothing_more_arrives_on` already
    // uses to prove an absence: there is no event to wait on for
    // "nothing happened".
    drain_updates_for(
        &mut observer_stream,
        &mut observer_connection,
        &mut held,
        attached.stream,
        Duration::from_millis(500),
    );
    assert_eq!(
        screen_text(&held),
        before,
        "text and key given together must deliver nothing at all, not one or the other"
    );

    let Response::Error(error) = response else {
        panic!("expected pane.send to refuse text and key together, got {response:?}")
    };
    assert_eq!(error.code, ErrorCode::BadRequest);

    stop_server(&stop, handle);
}

#[test]
fn pane_send_with_an_unknown_key_name_is_bad_request() {
    let (_socket_dir, socket_path, stop, handle) = start_server();
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

    stop_server(&stop, handle);
}
