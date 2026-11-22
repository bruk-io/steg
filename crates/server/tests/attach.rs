//! `pane.attach`/`pane.detach` over a real socket (TASK-1.47, step 4): the
//! method wiring in `connection.rs`/`Runtime`, proven the same way
//! `routing.rs` proves the rest of the API - the sdk's own `Connection`
//! doing the handshake and building requests exactly as a real client
//! would, against a real server over a real socket, with a real pty
//! behind the pane. The worker-thread mechanism itself (the tick, the
//! fan-out, back-pressure) is proven directly in `crates/server/src/
//! pane.rs`'s own `attach_tests` module; this file proves the wiring on
//! top of it.

#![expect(
    clippy::unwrap_used,
    reason = "this whole file is a test; see routing.rs's own comment"
)]

use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde_json::{Map, Value, json};
use steg_sdk::{
    ClientKind, Connection, EndReason, ErrorCode, Frame, Incoming, PaneAttached, Response,
    ScreenUpdate, WireRole as Role, description, encode_frame, parse_server_hello,
};
use steg_server::{LoginShell, ServerConfig, Stopper, serve};

#[path = "support/mod.rs"]
mod support;
use support::{ScopedTempDir, run_bounded, scoped_socket_path};

/// This file's own connections' default read timeout (TASK-1.123) - see
/// `connect_and_handshake`'s own doc for why 15s, not the 5s it replaces.
const DEFAULT_READ_TIMEOUT: Duration = Duration::from_secs(15);

/// The deadline `call_and_read_reply`/`attach`/`declare_size` each wait on
/// for a reply - above `DEFAULT_READ_TIMEOUT` (TASK-1.123), named once
/// rather than duplicated as the same literal three times (TASK-1.137).
const REPLY_DEADLINE: Duration = DEFAULT_READ_TIMEOUT.saturating_add(Duration::from_secs(5));

/// A working `PATH` is required (TASK-1.87), the same reasoning
/// `routing.rs`'s own `test_login` gives - this file's own tests all
/// start a real program over the real socket.
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
    // 90s, derived rather than measured once: `force_end`'s own nominal
    // worst case for one pane is `force_grace + ESCALATE_CONFIRM_TIMEOUT +
    // FORCE_JOIN_TIMEOUT` (2s + 2s + 2s = 6s, this file's own comment
    // above `pane_close_force_returns_within_its_bound_even_while_the_
    // panes_own_worker_is_stuck`), plus `force_end`'s own pass 6
    // (`join_worker`) running sequentially per pane - the flood tests
    // below stop a server with two panes (`stuck_pane`/`other_pane`),
    // so up to one more `FORCE_JOIN_TIMEOUT` (2s) on top, ~10s nominal
    // worst case for this file's own tests specifically. That same test's
    // own already-reviewed bound for a single force_end-involving wait
    // under this machine's own CPU contention is 30s - about 5x the
    // single-pane nominal (6s) - a generosity this sweep found was
    // itself not quite enough: this exact join measured over 32s once.
    // Applying that same ~5x multiplier to the two-pane nominal (10s)
    // gives ~50s; 90s keeps real margin above both that and the one
    // measured run, while staying well under `cross_process_flood_
    // guard`'s own 180s and the 900s per-step ceiling `steg-quiet`
    // itself imposes (`tools/src/steg_tools/quiet.py`).
    run_bounded(
        "the server thread to end after stop()",
        Duration::from_secs(90),
        move || {
            handle.join().expect("the server thread should not panic");
        },
    );
}

/// Same shape as `routing.rs`'s own `connect_and_handshake` - the real
/// handshake, with the sdk's own `Connection`.
///
/// This connection's own "default" read timeout, `DEFAULT_READ_TIMEOUT`
/// below, is set here once and relied on by every helper in this file
/// that does not pass its own explicit bound (`call_and_read_reply`,
/// `reply_within`'s own restore step) - 15s, not the 5s this was before
/// TASK-1.123, which this file's own sighted flood-test starvation
/// (this task's own notes) found was the tightest bound in the file:
/// a liveness margin proving "answers at all" under this machine's own
/// concurrent CPU contention, the same reasoning this file's own
/// `an_unresponsive_panes_own_commands_do_not_stall_the_connection_or_
/// another_pane` already settled on for its own two "eventually drains"
/// checks.
fn connect_and_handshake(socket_path: &std::path::Path) -> (UnixStream, Connection) {
    let mut stream = UnixStream::connect(socket_path).unwrap();
    stream.set_read_timeout(Some(DEFAULT_READ_TIMEOUT)).unwrap();
    let hello_bytes = Connection::send_hello("attach-test-client", ClientKind::Cli, None);
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

/// Writes a validated call and reads back its matching reply, the same
/// shape `routing.rs`'s own `call_and_read_reply` already has - every
/// non-matching `Incoming` read along the way is simply ignored here
/// (the race test below reads frames itself, more carefully, instead of
/// calling this).
fn call_and_read_reply(
    stream: &mut UnixStream,
    connection: &mut Connection,
    method: &str,
    params: Map<String, Value>,
) -> Response {
    let (id, bytes) = connection.call(method, params).expect("a valid call");
    stream.write_all(&bytes).unwrap();
    let deadline = Instant::now() + REPLY_DEADLINE;
    loop {
        // Drains every already-decoded frame before reading more
        // (TASK-1.128: an attached call like `pane.close --force` can
        // now also produce a `StreamControl` frame - `Ended` - on the
        // wire around the same time as its own reply; falling through to
        // a blocking read on any non-matching `poll()` result, rather
        // than only on `None`, could then block forever on a socket that
        // already has the real reply sitting decoded, one `poll()` call
        // away - the same gap `reply_within`'s own, already-correct loop
        // shape does not have).
        match connection.poll() {
            Some(Incoming::Reply {
                id: reply_id,
                response,
            }) => {
                assert_eq!(reply_id, id, "the reply must carry the id this call minted");
                return response;
            }
            Some(_) => continue, // some other frame - fine, poll again before reading more
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
/// socket, and returns that pane's own id (`pane.list`'s own `id` field -
/// this file's own tests give it as `pane.attach`/`pane.detach`'s target).
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

    // pane.list's own target is a tab, not a session (R10) - tab.list's
    // target is the session, so the new session's one tab is found first.
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

    // A bare tab id needs this connection's own session context to
    // resolve, which this socket-only connection has none of
    // (`dispatch/lookup.rs`'s own missing-context refusal, the same gap
    // `routing.rs`'s own `tab_list_tab_show_and_pane_list_answer_over_a_
    // real_socket` names) - the full `session/tab` path resolves with no
    // context at all.
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

    // A bare pane id needs this connection's own session/tab context to
    // resolve too, same reasoning as the tab id above - the full path is
    // what this file's own `attach`/`detach` targets with.
    format!("{session_id}/{tab_id}/{pane_id}")
}

/// Calls `pane.attach`, decodes the reply as `PaneAttached`, and opens the
/// new stream on `connection` (the client's own half of decision-2
/// section 3's ordering rule: only once the reply naming it is in hand).
/// Calls `pane.attach` and waits for its matching reply - unlike
/// `call_and_read_reply`, every *other* decoded frame along the way is
/// inspected too, not silently discarded: an `Incoming::UnknownStream`
/// arriving before this connection has even seen the reply naming that
/// stream is exactly decision-2 section 3's ordering rule being violated,
/// and must fail this call, not be ignored while this function keeps
/// waiting only for the one variant it wants.
fn attach(stream: &mut UnixStream, connection: &mut Connection, pane: &str) -> PaneAttached {
    let mut params = Map::new();
    params.insert("target".to_string(), json!(pane));
    let (id, bytes) = connection
        .call("pane.attach", params)
        .expect("a valid call");
    stream.write_all(&bytes).unwrap();

    let deadline = Instant::now() + REPLY_DEADLINE;
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
            Some(_) => {} // some other, unrelated frame - fine
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

fn detach(
    stream: &mut UnixStream,
    connection: &mut Connection,
    pane: &str,
    attachment_stream: u32,
) {
    let mut params = Map::new();
    params.insert("target".to_string(), json!(pane));
    let response = call_and_read_reply(stream, connection, "pane.detach", params);
    assert!(matches!(response, Response::Ok(_)), "{response:?}");
    connection.close_stream(attachment_stream);
}

/// The tab a `new_pane_running`/`attach` pane path names, as its own
/// target string (`session/tab`, the pane's own last segment dropped) -
/// `tab.declare-size`'s own target (TASK-1.114: a client declares a
/// *tab's* size, which `Layout::leaf_sizes` splits across its panes).
fn tab_path_of(pane_path: &str) -> String {
    let (tab_path, _pane) = pane_path
        .rsplit_once('/')
        .expect("a pane path is always session/tab/pane");
    tab_path.to_string()
}

/// Sends raw bytes on an attachment's own stream (TASK-1.114, AC #1):
/// `Connection::send_data` already refuses a stream this connection has
/// not itself opened (checked client-side, before anything is encoded),
/// the same guard `connection.open_stream`'s own doc names.
fn send_input(
    stream: &mut UnixStream,
    connection: &Connection,
    attachment_stream: u32,
    bytes: &[u8],
) {
    let frame = connection
        .send_data(attachment_stream, bytes)
        .expect("a stream this connection has open");
    stream.write_all(&frame).unwrap();
}

/// `tab.declare-size` (TASK-1.114): the same bounded call-and-read-reply
/// shape every other method in this file uses.
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
    let deadline = Instant::now() + REPLY_DEADLINE;
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

/// Like `call_and_read_reply`, but never panics on a timeout: `None` means
/// no reply for `id` arrived within `timeout` - this file's own proof that
/// a *specific* request is stuck (TASK-1.114, proof item 9), as opposed to
/// the connection or another request being stuck. Every other decoded
/// frame read along the way (another request's reply, an update on some
/// other stream) is kept, not dropped, so a caller can still see what else
/// arrived while waiting.
fn reply_within(
    stream: &mut UnixStream,
    connection: &mut Connection,
    id: u64,
    timeout: Duration,
) -> Option<Response> {
    // Every other helper in this file assumes the connection's own
    // `DEFAULT_READ_TIMEOUT` (`connect_and_handshake`'s own setting) -
    // restored on every exit from this function, not left at whatever
    // this function's own short, internal polling timeout last set it
    // to, which would otherwise make the *next* helper's own blocking
    // read fail with `WouldBlock` instead of really waiting.
    let result = reply_within_with_leaky_timeout(stream, connection, id, timeout);
    stream.set_read_timeout(Some(DEFAULT_READ_TIMEOUT)).unwrap();
    result
}

fn reply_within_with_leaky_timeout(
    stream: &mut UnixStream,
    connection: &mut Connection,
    id: u64,
    timeout: Duration,
) -> Option<Response> {
    let deadline = Instant::now() + timeout;
    loop {
        match connection.poll() {
            Some(Incoming::Reply {
                id: reply_id,
                response,
            }) if reply_id == id => return Some(response),
            Some(_) => {} // some other frame - fine, keep waiting for `id`
            None => {
                let remaining = deadline.saturating_duration_since(Instant::now());
                if remaining.is_zero() {
                    return None;
                }
                stream
                    .set_read_timeout(Some(remaining.min(Duration::from_millis(200))))
                    .unwrap();
                let mut buf = [0_u8; 65536];
                match stream.read(&mut buf) {
                    Ok(0) => return None,
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

/// Reads one `Incoming::StreamControl` for `wanted_stream`, decoded as a
/// `ScreenUpdate` - bounded, via a real read with a timeout, never a
/// sleep-poll: the event this waits for is the frame itself. Panics if an
/// `Incoming::UnknownStream` for `wanted_stream` ever arrives first -
/// decision-2 section 3's ordering rule, the one thing a correct server
/// never produces (the spike's own invariant: `unknown_stream_frames=0`).
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
            Some(Incoming::UnknownStream {
                stream: frame_stream,
                ..
            }) if frame_stream == wanted_stream => {
                panic!(
                    "decision-2 section 3's ordering rule was violated: a frame for stream \
                     {wanted_stream} arrived before this client was told it existed"
                );
            }
            Some(_) => {} // some other stream's frame, or an unrelated control frame - ignored
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

/// `Ended` is not applied here at all - a caller that cares about it
/// checks for it before deciding whether there is still a screen update
/// to apply (TASK-1.128).
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

/// Asserts nothing arrives on `wanted_stream` for `window` (TASK-1.128,
/// AC #1's "no frame arrives on that stream afterward") - the same shape
/// `detach_ends_the_stream_even_though_the_program_keeps_producing_output`'s
/// own inline check uses, pulled out so every test below that proves this
/// shares one definition. This connection's own side never calls `close_
/// stream` for `wanted_stream` in the tests that use this (unlike `detach`),
/// so a frame the server still sent would decode as `StreamControl`, not
/// `UnknownStream` - only the former is checked for.
fn assert_nothing_more_arrives_on(
    stream: &mut UnixStream,
    connection: &mut Connection,
    wanted_stream: u32,
    window: Duration,
) {
    let quiet_until = Instant::now() + window;
    while Instant::now() < quiet_until {
        match connection.poll() {
            Some(Incoming::StreamControl {
                stream: frame_stream,
                ..
            }) if frame_stream == wanted_stream => {
                panic!("a frame arrived on stream {wanted_stream} after its own Ended")
            }
            Some(_) => {}
            None => {
                stream
                    .set_read_timeout(Some(Duration::from_millis(20)))
                    .unwrap();
                let mut buf = [0_u8; 65536];
                match stream.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => connection.receive(&buf[..n]),
                    Err(_) => {}
                }
            }
        }
    }
}

#[test]
fn attach_returns_the_real_screen_and_updates_follow_it() {
    let (_socket_dir, socket_path, stop, handle) = start_server();
    let (mut stream, mut connection) = connect_and_handshake(&socket_path);

    let pane = new_pane_running(
        &mut stream,
        &mut connection,
        vec!["sh", "-c", "printf attach-test-marker-xyz; exec sleep 100"],
    );

    let attached = attach(&mut stream, &mut connection, &pane);
    assert!(attached.stream > 0, "a real stream id was handed out");

    // The marker may already be in the opening snapshot (if the shell's
    // own `printf` won the race with this attach), or may still be
    // coming as an update - applied exactly as a real client would,
    // bounded, never a sleep.
    let mut held = attached.snapshot;
    let deadline = Instant::now() + Duration::from_secs(5);
    while !screen_text(&held).contains("attach-test-marker-xyz") {
        assert!(
            Instant::now() < deadline,
            "never saw the marker, holding: {:?}",
            screen_text(&held)
        );
        let update = next_update(
            &mut stream,
            &mut connection,
            attached.stream,
            Duration::from_secs(5),
        );
        apply_update(&mut held, update);
    }

    stop_server(&stop, handle);
}

#[test]
fn detach_ends_the_stream_even_though_the_program_keeps_producing_output() {
    let (_socket_dir, socket_path, stop, handle) = start_server();
    let (mut stream, mut connection) = connect_and_handshake(&socket_path);

    // Steady, ongoing output - slow enough (50 lines/s) that this test's
    // own continuous draining below never lets `Outbound`'s own 64-frame
    // queue (decision-2 section 6's `DATA_CAP`) come anywhere near full.
    // A genuine flood would (correctly) make `Outbound::close_stream`
    // alone enough to explain "no frame arrives" once the queue and the
    // OS socket buffer both saturate - confirmed directly by a negative
    // control run with the worker-side detach disabled, which still
    // passed, for exactly that confounded reason - so this test is
    // deliberately paced to isolate what it actually claims: the worker
    // itself stops computing and sending anything for this attachment,
    // not merely that a saturated queue already stopped delivering.
    let pane = new_pane_running(
        &mut stream,
        &mut connection,
        vec!["sh", "-c", "while true; do echo flooding; sleep 0.02; done"],
    );

    let attached = attach(&mut stream, &mut connection, &pane);
    // At least a few real updates, continuously drained, so this test is
    // attached to a stream that really was delivering something, before
    // detaching it.
    for _ in 0..3 {
        let _ = next_update(
            &mut stream,
            &mut connection,
            attached.stream,
            Duration::from_secs(5),
        );
    }

    detach(&mut stream, &mut connection, &pane, attached.stream);

    // Keeps draining (never letting a backlog build up, which this
    // pacing already avoids) for a bounded window well past several of
    // the program's own print intervals - the positive proof above
    // already showed real updates arrive at this rate when attached, so
    // a quiet window of this length is detach's own doing, not an idle
    // pane or a saturated queue.
    let quiet_until = Instant::now() + Duration::from_millis(500);
    while Instant::now() < quiet_until {
        match connection.poll() {
            // Checked both ways on purpose: this file's own `detach`
            // helper already closed the stream on this connection's own
            // side, so anything the server still sent for it decodes as
            // `UnknownStream`, not `StreamControl` - either one is this
            // test's own proof the server kept sending after detach, and
            // checking only the first would have made this test's own
            // negative control pass vacuously (found by actually running
            // it, not assumed: an earlier revision of this test did
            // exactly that, silently).
            Some(Incoming::StreamControl {
                stream: frame_stream,
                ..
            }) if frame_stream == attached.stream => {
                panic!("an update arrived for a stream this connection already detached")
            }
            Some(Incoming::UnknownStream {
                stream: frame_stream,
                ..
            }) if frame_stream == attached.stream => {
                panic!(
                    "the server kept sending for a stream this connection already detached \
                     (decoded as UnknownStream, since this connection also closed its own \
                     side of it)"
                );
            }
            Some(_) => {}
            None => {
                stream
                    .set_read_timeout(Some(Duration::from_millis(20)))
                    .unwrap();
                let mut buf = [0_u8; 65536];
                match stream.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => connection.receive(&buf[..n]),
                    Err(_) => {}
                }
            }
        }
    }

    stop_server(&stop, handle);
}

#[test]
fn detaching_a_pane_this_connection_never_attached_to_is_a_bad_request() {
    let (_socket_dir, socket_path, stop, handle) = start_server();
    let (mut stream, mut connection) = connect_and_handshake(&socket_path);

    let pane = new_pane_running(
        &mut stream,
        &mut connection,
        vec!["sh", "-c", "exec sleep 100"],
    );

    let mut params = Map::new();
    params.insert("target".to_string(), json!(pane));
    let response = call_and_read_reply(&mut stream, &mut connection, "pane.detach", params);
    match response {
        Response::Error(error) => {
            assert_eq!(error.code, steg_sdk::ErrorCode::BadRequest);
        }
        Response::Ok(_) => panic!("detaching a pane never attached to must not succeed"),
    }

    stop_server(&stop, handle);
}

/// decision-2 section 3's ordering rule, end to end through the real
/// wiring this task's own step 4 adds - repeated several times against a
/// pane that is already flooding output the moment each attach lands, to
/// give a real race a real chance: `next_update`/`attach` already panic
/// the instant `Incoming::UnknownStream` appears for the new stream,
/// which is the one thing a correct server never produces (the spike's
/// own invariant, `unknown_stream_frames=0`).
#[test]
fn repeated_attach_and_detach_against_a_flooding_pane_never_sees_an_unknown_stream() {
    let (_socket_dir, socket_path, stop, handle) = start_server();
    let (mut stream, mut connection) = connect_and_handshake(&socket_path);

    let pane = new_pane_running(
        &mut stream,
        &mut connection,
        vec!["sh", "-c", "while true; do echo flooding; done"],
    );

    for _ in 0..20 {
        let attached = attach(&mut stream, &mut connection, &pane);
        let _ = next_update(
            &mut stream,
            &mut connection,
            attached.stream,
            Duration::from_secs(5),
        );
        detach(&mut stream, &mut connection, &pane, attached.stream);
    }

    stop_server(&stop, handle);
}

/// A connection that ends without calling `pane.detach` must not leave
/// the pane's own worker thread (or the server as a whole) stuck: a
/// second connection can still attach the same pane afterward, and the
/// server still stops cleanly - the cleanup sweep `ConnectionCleanup`
/// adds (TASK-1.47), proven by its absence not crashing or hanging
/// anything, not by inspecting the worker's own attachment list directly.
#[test]
fn disconnecting_without_detaching_does_not_leak_or_block_a_later_attach() {
    let (_socket_dir, socket_path, stop, handle) = start_server();

    let pane = {
        let (mut stream, mut connection) = connect_and_handshake(&socket_path);
        let pane = new_pane_running(
            &mut stream,
            &mut connection,
            vec!["sh", "-c", "exec sleep 100"],
        );
        let attached = attach(&mut stream, &mut connection, &pane);
        assert!(attached.stream > 0);
        pane
        // `stream`/`connection` drop here, ending the connection with no
        // `pane.detach` ever sent - the case this test is about.
    };

    let (mut stream, mut connection) = connect_and_handshake(&socket_path);
    let attached = attach(&mut stream, &mut connection, &pane);
    assert!(
        attached.stream > 0,
        "a second connection must still be able to attach the same pane"
    );

    stop_server(&stop, handle);
}

/// TASK-1.114's own real bug, caught before it ever shipped: `PaneCommand::
/// Arm`/`Detach` used to match an attachment by the connection-supplied
/// `stream` number alone, which is only unique *within* one connection -
/// two different connections attached to the same pane both mint
/// `stream == 1` for their own first attachment, since each one's own
/// `next_stream` counter starts fresh. Matching on that number alone found
/// whichever entry happened to come first in the worker's own list,
/// regardless of which connection a command actually named - detaching one
/// connection's attachment silently detached the other's instead. The pane
/// worker now mints its own `AttachmentId` at `Attach` time (TASK-1.114)
/// and `Arm`/`Detach` key off that, never the bare stream number.
#[test]
fn two_connections_attached_to_one_pane_each_get_their_own_updates_and_a_detach_does_not_touch_the_other()
 {
    let (_socket_dir, socket_path, stop, handle) = start_server();
    let (mut stream_a, mut connection_a) = connect_and_handshake(&socket_path);

    // Steady, paced output, continuously drained by both connections below
    // - the same reasoning `detach_ends_the_stream_even_though_the_program_
    // keeps_producing_output`'s own comment gives for not flooding: this
    // test's own proof needs updates to keep arriving on the survivor,
    // which a saturated queue would mask.
    let pane = new_pane_running(
        &mut stream_a,
        &mut connection_a,
        vec!["sh", "-c", "while true; do echo flooding; sleep 0.02; done"],
    );

    let (mut stream_b, mut connection_b) = connect_and_handshake(&socket_path);

    let attached_a = attach(&mut stream_a, &mut connection_a, &pane);
    let attached_b = attach(&mut stream_b, &mut connection_b, &pane);
    // Both connections mint their own stream ids independently, starting
    // from the same count - the exact precondition the bug needed: this
    // assertion is not the proof itself, only confirmation the race this
    // test is built to catch is actually being given a chance.
    assert_eq!(
        attached_a.stream, attached_b.stream,
        "this test only proves anything if both connections mint the same \
         stream number for their own first attachment"
    );

    // Each connection really does get its own, independent updates -
    // never checked by reading the wire's own stream id and hoping that is
    // enough, since both sides use the same number: only that each
    // connection's own decoded update arrives on its own connection.
    for _ in 0..2 {
        let _ = next_update(
            &mut stream_a,
            &mut connection_a,
            attached_a.stream,
            Duration::from_secs(5),
        );
        let _ = next_update(
            &mut stream_b,
            &mut connection_b,
            attached_b.stream,
            Duration::from_secs(5),
        );
    }

    detach(&mut stream_a, &mut connection_a, &pane, attached_a.stream);

    // The bug this test is named for: detaching A's attachment (bare
    // stream number 1) must never also detach B's (also bare stream
    // number 1) - B's connection must still be receiving real updates
    // right after A's own detach returns.
    let _ = next_update(
        &mut stream_b,
        &mut connection_b,
        attached_b.stream,
        Duration::from_secs(5),
    );

    stop_server(&stop, handle);
}

/// TASK-1.47 review's own nit: a pane that closes while this connection
/// is still attached to it must still be detachable - `pane.detach`'s
/// own target resolution goes through the kernel's tree first, which no
/// longer knows a closed pane at all, so without a fallback this would
/// otherwise fail at that step and never reach `close_stream`, leaking
/// the stream on both sides for the rest of the connection's life.
#[test]
fn detaching_a_pane_that_already_closed_still_releases_the_stream() {
    let (_socket_dir, socket_path, stop, handle) = start_server();
    let (mut stream, mut connection) = connect_and_handshake(&socket_path);

    let pane = new_pane_running(&mut stream, &mut connection, vec!["sh", "-c", "exit 0"]);
    let attached = attach(&mut stream, &mut connection, &pane);
    assert!(attached.stream > 0);

    let mut params = Map::new();
    params.insert("target".to_string(), json!(pane));
    params.insert("force".to_string(), json!(true));
    let response = call_and_read_reply(&mut stream, &mut connection, "pane.close", params);
    assert!(
        matches!(response, Response::Ok(_)),
        "pane.close failed: {response:?}"
    );

    let mut params = Map::new();
    params.insert("target".to_string(), json!(pane));
    let response = call_and_read_reply(&mut stream, &mut connection, "pane.detach", params);
    assert!(
        matches!(response, Response::Ok(_)),
        "detaching an already-closed pane must still succeed, not fail at target \
         resolution: {response:?}"
    );

    stop_server(&stop, handle);
}

// --- TASK-1.114: input and a declared size from an attached client --------

/// AC #1: bytes a client sends on an attachment's own stream reach the
/// pane's real program, in order - `cat` echoes its stdin back, so the
/// screen shows the sent text once from the pty driver's own echo and
/// once more from `cat`'s own copy, confirming the bytes really reached
/// the program's stdin, not merely the pty (proof item 1).
///
/// Control: with `connection.rs`'s own `Incoming::Data` arm removed (left
/// in the closing bucket, as it was before this task), this test fails
/// because the connection closes on the first input frame instead of the
/// marker ever appearing - run by hand and reverted, recorded via
/// `asdlc record-check` both ways.
#[test]
fn typed_bytes_reach_a_real_program_in_order() {
    let (_socket_dir, socket_path, stop, handle) = start_server();
    let (mut stream, mut connection) = connect_and_handshake(&socket_path);

    let pane = new_pane_running(&mut stream, &mut connection, vec!["cat"]);
    let attached = attach(&mut stream, &mut connection, &pane);

    // Four separate frames, not one: proves ordering across frames, not
    // merely that one frame's own bytes stay in order.
    for byte in [b"o", b"r", b"d", b"e"] {
        send_input(&mut stream, &connection, attached.stream, byte);
    }
    send_input(&mut stream, &connection, attached.stream, b"r\n");

    let mut held = attached.snapshot;
    let deadline = Instant::now() + Duration::from_secs(5);
    while !screen_text(&held).contains("order\r\norder")
        && !screen_text(&held).contains("order\norder")
    {
        assert!(
            Instant::now() < deadline,
            "never saw the typed text echoed and copied back by cat, holding: {:?}",
            screen_text(&held)
        );
        let update = next_update(
            &mut stream,
            &mut connection,
            attached.stream,
            Duration::from_secs(5),
        );
        apply_update(&mut held, update);
    }

    stop_server(&stop, handle);
}

/// AC #1: input makes the sender R91's "latest" and clears a `Finished`/
/// `Failed` status (`Tree::note_input`, doc-11 R83) - proof item 2. A
/// command pane whose command already exited is used so this does not
/// depend on a live program reading anything: `note_input` is about
/// client activity on the *pane*, independent of whether anything is
/// currently running.
///
/// Control: with the `tree.note_input` call removed from
/// `Runtime::send_input`, this test fails because the status never
/// clears - recorded both ways through `asdlc record-check`.
#[test]
fn input_clears_a_finished_status() {
    let (_socket_dir, socket_path, stop, handle) = start_server();
    let (mut stream, mut connection) = connect_and_handshake(&socket_path);

    let pane = new_pane_running(&mut stream, &mut connection, vec!["sh", "-c", "exit 0"]);

    // Wait for the command to actually end and its status to show
    // (bounded, on the event - `pane.show`'s own report, not a sleep).
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let mut params = Map::new();
        params.insert("target".to_string(), json!(pane));
        let response = call_and_read_reply(&mut stream, &mut connection, "pane.show", params);
        let Response::Ok(record) = response else {
            panic!("pane.show failed: {response:?}")
        };
        // Not merely non-null: a freshly-started pane's own status is
        // already `Some(Status::Running)`, also non-null - this must wait
        // specifically for the *ended* status (`"finished"`), which only
        // arrives once the program has actually exited.
        if record.get("status").and_then(Value::as_str) == Some("finished") {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "the command never reached a Finished status: {record:?}"
        );
        std::thread::sleep(Duration::from_millis(20));
    }

    let attached = attach(&mut stream, &mut connection, &pane);
    send_input(&mut stream, &connection, attached.stream, b"x");

    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let mut params = Map::new();
        params.insert("target".to_string(), json!(pane));
        let response = call_and_read_reply(&mut stream, &mut connection, "pane.show", params);
        let Response::Ok(record) = response else {
            panic!("pane.show failed: {response:?}")
        };
        if record.get("status").is_some_and(Value::is_null) {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "input never cleared the Finished status: {record:?}"
        );
        std::thread::sleep(Duration::from_millis(20));
    }

    stop_server(&stop, handle);
}

/// AC #2: a declared size reaches the real pty (observed by the program
/// itself through `stty size`, after a `SIGWINCH` its own trap catches),
/// and is reported on *every* attached stream, including one that never
/// declared a size of its own - proof items 3 and 5 (the kernel's own
/// "latest wins" rule is unit-tested directly in `crates/kernel/src/
/// tree.rs`; this is the end-to-end wiring on top of it).
///
/// Also item 4, specifically, and not by construction: client A attaches
/// *before* client B, so B's own `PaneCommand::Arm` has not necessarily
/// reached the pane's worker yet by the time B's `tab.declare-size` call
/// arms a tick - this ordering is what this task's own build first caught
/// a real bug with (a pane-wide "has the size changed since the last
/// report" flag, consumed by whichever tick ran first, left the *other*
/// attachment stale forever). Control: reverting `Attachment::
/// last_told_size`'s own per-attachment tracking in `crate::pane::run_tick`
/// back to a single, pane-wide flag reproduces exactly that failure in
/// this test - recorded through `asdlc record-check`, both ways.
#[test]
fn a_declared_size_reaches_the_pty_and_is_reported_on_every_attached_stream() {
    let (_socket_dir, socket_path, stop, handle) = start_server();
    let (mut stream_a, mut connection_a) = connect_and_handshake(&socket_path);
    let (mut stream_b, mut connection_b) = connect_and_handshake(&socket_path);

    let pane = new_pane_running(&mut stream_a, &mut connection_a, vec!["sh"]);
    let tab = tab_path_of(&pane);

    // Client A attaches and declares nothing; client B attaches to the
    // *same* pane and is the one that declares a size - item 3's own
    // wording: "including one that declared no size itself" is A.
    let attached_a = attach(&mut stream_a, &mut connection_a, &pane);
    let attached_b = attach(&mut stream_b, &mut connection_b, &pane);

    fn drain_until_size(
        stream: &mut UnixStream,
        connection: &mut Connection,
        wanted_stream: u32,
        columns: u16,
        rows: u16,
    ) {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            assert!(Instant::now() < deadline, "never saw size {columns}x{rows}");
            let update = next_update(stream, connection, wanted_stream, Duration::from_secs(5));
            if let ScreenUpdate::Full { screen, .. } = &update
                && screen.size.columns() == columns
                && screen.size.rows() == rows
            {
                return;
            }
        }
    }

    let response = declare_size(&mut stream_b, &mut connection_b, &tab, 100, 30);
    assert!(matches!(response, Response::Ok(_)), "{response:?}");

    // Both streams must see the new size - the proof that "every
    // attachment is told", not only the declarer (client B, above). A
    // size catch-up always arrives as a `Full` (`Screen` always carries
    // the current size; `Delta` carries none - see `steg_sdk::screen`'s
    // own doc for why), never a bare field on a `Delta`.
    drain_until_size(&mut stream_a, &mut connection_a, attached_a.stream, 100, 30);
    drain_until_size(&mut stream_b, &mut connection_b, attached_b.stream, 100, 30);

    // And the real pty: a plain command run at the shell's own prompt
    // reports the real dimensions through `stty size` ("<rows> <cols>")
    // - sent as input on client B's own attachment, the same mechanism
    // AC #1's own test already proves, deliberately not a signal-
    // dependent trap (real, but asynchronous relative to the shell's own
    // `sleep`, and not worth the flakiness here: this item is about the
    // declared size reaching the real pty, not about signal delivery
    // timing, which is the OS's concern, not this task's).
    send_input(
        &mut stream_b,
        &connection_b,
        attached_b.stream,
        b"stty size\n",
    );

    let mut held = attached_b.snapshot;
    let deadline = Instant::now() + Duration::from_secs(5);
    while !screen_text(&held).contains("30 100") {
        assert!(
            Instant::now() < deadline,
            "the program never reported the real pty size, holding: {:?}",
            screen_text(&held)
        );
        let update = next_update(
            &mut stream_b,
            &mut connection_b,
            attached_b.stream,
            Duration::from_secs(5),
        );
        apply_update(&mut held, update);
    }

    // A *second*, independent declare, confirming a later redeclare keeps
    // working (not only the pane's very first tick, which the kernel's
    // own `is_current == false` already forces a `Full` for regardless
    // of this task's own code) - declared by A this time, not B, so both
    // directions are exercised. In practice ghostty-vt marks *every*
    // resize dirty (confirmed directly: growing or shrinking rows alone,
    // with no column change, still yields `Dirty::Full`/`Partial` here,
    // not `Clean` - bringing previously-scrolled rows into view, or
    // moving visible ones into scrollback, is itself a content change to
    // the engine), so this does not by itself isolate `run_tick`'s own
    // `needs_size_catch_up` branch from a `Dirty::Clean` tick - that
    // precise case (a stale recorded size with nothing else to report)
    // is unit-tested directly in `crate::pane::attach_tests`, where the
    // precondition can be constructed without depending on whether any
    // real resize ever produces it.
    let response = declare_size(&mut stream_a, &mut connection_a, &tab, 100, 45);
    assert!(matches!(response, Response::Ok(_)), "{response:?}");
    drain_until_size(&mut stream_a, &mut connection_a, attached_a.stream, 100, 45);
    drain_until_size(&mut stream_b, &mut connection_b, attached_b.stream, 100, 45);

    stop_server(&stop, handle);
}

/// AC #3: a client that calls `pane.detach` releases its claim, and the
/// survivor's terminal returns to *its own* declared size - told on its
/// own stream, not merely recomputed server-side (proof item 6). The
/// release mechanism itself (`Tree::stop_viewing`/`reapply_latest`) is
/// already built and unit-tested in `crates/kernel/src/tree.rs`; this
/// proves the end-to-end wiring, with a real size difference to detect,
/// not only that a stale attachment is swept.
///
/// Control: removing `forget_attachment`/the `detach_pane` call's own
/// reapply step makes this test fail because the survivor's stream never
/// reports the reapplied size - recorded through `asdlc record-check`.
#[test]
fn detaching_releases_the_claim_and_the_survivor_returns_to_its_own_declared_size() {
    let (_socket_dir, socket_path, stop, handle) = start_server();
    let (mut stream_a, mut connection_a) = connect_and_handshake(&socket_path);
    let (mut stream_b, mut connection_b) = connect_and_handshake(&socket_path);

    let pane = new_pane_running(&mut stream_a, &mut connection_a, vec!["sh"]);
    let tab = tab_path_of(&pane);

    let attached_a = attach(&mut stream_a, &mut connection_a, &pane);
    let attached_b = attach(&mut stream_b, &mut connection_b, &pane);

    // B declares first (60x15), then A declares (100x30) - A is now
    // latest unconditionally (declaring is activity, decision-6), so the
    // tab's size is A's. Drained past both clients' own "I declared"
    // updates before detaching, so the detach's own effect below is not
    // confused with these.
    let response = declare_size(&mut stream_b, &mut connection_b, &tab, 60, 15);
    assert!(matches!(response, Response::Ok(_)), "{response:?}");
    let response = declare_size(&mut stream_a, &mut connection_a, &tab, 100, 30);
    assert!(matches!(response, Response::Ok(_)), "{response:?}");

    fn drain_until_size(
        stream: &mut UnixStream,
        connection: &mut Connection,
        wanted_stream: u32,
        columns: u16,
        rows: u16,
    ) {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            assert!(Instant::now() < deadline, "never saw size {columns}x{rows}");
            let update = next_update(stream, connection, wanted_stream, Duration::from_secs(5));
            if let ScreenUpdate::Full { screen, .. } = &update
                && screen.size.columns() == columns
                && screen.size.rows() == rows
            {
                return;
            }
        }
    }

    drain_until_size(&mut stream_a, &mut connection_a, attached_a.stream, 100, 30);
    drain_until_size(&mut stream_b, &mut connection_b, attached_b.stream, 100, 30);

    // A detaches: releases its claim. B is the only remaining viewer, so
    // `reapply_latest`'s own fallback makes B latest again, and B's own
    // last-declared size (60x15, differing from the current 100x30) is
    // reapplied - a real, visible change, not a no-op.
    detach(&mut stream_a, &mut connection_a, &pane, attached_a.stream);

    drain_until_size(&mut stream_b, &mut connection_b, attached_b.stream, 60, 15);

    stop_server(&stop, handle);
}

/// AC #3: a client whose connection is dropped hard (not a polite
/// `pane.detach`) releases its claim the same way - proof item 7.
/// `ConnectionCleanup::Drop`'s own sweep (TASK-1.47) calls `Runtime::
/// disconnect_client`, which reapplies every `SizeChanged` the kernel
/// reports (`Tree::disconnect_client`'s own doc) - the same release path
/// a graceful `pane.detach` takes, from a different trigger.
///
/// Control: removing `ConnectionCleanup::Drop`'s `disconnect_client` call
/// makes this test fail the same way the graceful-detach test's own
/// control does.
#[test]
fn a_dropped_connection_releases_its_claim_the_same_way() {
    let (_socket_dir, socket_path, stop, handle) = start_server();
    let (mut stream_a, mut connection_a) = connect_and_handshake(&socket_path);
    let (mut stream_b, mut connection_b) = connect_and_handshake(&socket_path);

    let pane = new_pane_running(&mut stream_a, &mut connection_a, vec!["sh"]);
    let tab = tab_path_of(&pane);

    let attached_a = attach(&mut stream_a, &mut connection_a, &pane);
    let attached_b = attach(&mut stream_b, &mut connection_b, &pane);

    let response = declare_size(&mut stream_b, &mut connection_b, &tab, 60, 15);
    assert!(matches!(response, Response::Ok(_)), "{response:?}");
    let response = declare_size(&mut stream_a, &mut connection_a, &tab, 100, 30);
    assert!(matches!(response, Response::Ok(_)), "{response:?}");

    fn drain_until_size(
        stream: &mut UnixStream,
        connection: &mut Connection,
        wanted_stream: u32,
        columns: u16,
        rows: u16,
    ) {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            assert!(Instant::now() < deadline, "never saw size {columns}x{rows}");
            let update = next_update(stream, connection, wanted_stream, Duration::from_secs(5));
            if let ScreenUpdate::Full { screen, .. } = &update
                && screen.size.columns() == columns
                && screen.size.rows() == rows
            {
                return;
            }
        }
    }

    drain_until_size(&mut stream_a, &mut connection_a, attached_a.stream, 100, 30);
    drain_until_size(&mut stream_b, &mut connection_b, attached_b.stream, 100, 30);

    // A's connection is dropped hard, not detached politely - the socket
    // itself closes, which is what `ConnectionCleanup::Drop`'s own sweep
    // reacts to on the server's side (a real half-close, not a syscall
    // this test fakes).
    drop(stream_a);
    drop(connection_a);

    drain_until_size(&mut stream_b, &mut connection_b, attached_b.stream, 60, 15);

    stop_server(&stop, handle);
}

/// AC #2: a declared size of `0` is refused with `BadRequest`, never
/// reaching `Tree::declare_size` or the engine (proof item 11; R100's own
/// "no floor check, ever" is about shrinking *below* 20x4, a different,
/// deliberately allowed case - `0` is a narrower, degenerate one nothing
/// in the kernel or the engine is verified to handle).
///
/// Control: removing the range check in `connection.rs`'s `tab.declare
/// -size` arm makes this test fail - recorded through `asdlc record-check`.
#[test]
fn declaring_a_size_of_zero_is_refused() {
    let (_socket_dir, socket_path, stop, handle) = start_server();
    let (mut stream, mut connection) = connect_and_handshake(&socket_path);

    let pane = new_pane_running(
        &mut stream,
        &mut connection,
        vec!["sh", "-c", "exec sleep 100"],
    );
    let tab = tab_path_of(&pane);
    // Declaring needs an attachment first (`Tree::declare_size`'s own
    // precondition, R91's "a client attaches before it declares") - this
    // test is about the size itself, so that precondition is met here,
    // not left out (which would refuse for an unrelated reason).
    let _attached = attach(&mut stream, &mut connection, &pane);

    for (columns, rows) in [(0, 24), (80, 0), (0, 0)] {
        let response = declare_size(&mut stream, &mut connection, &tab, columns, rows);
        match response {
            Response::Error(error) => assert_eq!(error.code, ErrorCode::BadRequest),
            Response::Ok(_) => panic!("declaring {columns}x{rows} must not succeed"),
        }
    }

    stop_server(&stop, handle);
}

/// decision-2's own "refused" for a frame on a stream this connection has
/// already detached (proof item 10): `pane.detach` closes the stream on
/// both sides, so a later `Incoming::Data` for it decodes as `Incoming::
/// UnknownStream`, which `connection.rs`'s own read loop already closes
/// the connection for (TASK-1.45 §3, unchanged by this task) - this test
/// confirms that existing mechanism still applies once this task's own
/// `Incoming::Data` arm exists beside it, by building the frame directly
/// (bypassing `Connection::send_data`'s own client-side guard, which
/// would otherwise refuse to build this exact frame at all).
#[test]
fn a_frame_on_an_already_detached_stream_closes_the_connection() {
    let (_socket_dir, socket_path, stop, handle) = start_server();
    let (mut stream, mut connection) = connect_and_handshake(&socket_path);

    let pane = new_pane_running(
        &mut stream,
        &mut connection,
        vec!["sh", "-c", "exec sleep 100"],
    );
    let attached = attach(&mut stream, &mut connection, &pane);
    detach(&mut stream, &mut connection, &pane, attached.stream);

    let frame = encode_frame(&Frame::data(attached.stream, b"x".to_vec()))
        .expect("a data frame for an id this test controls directly");
    stream.write_all(&frame).unwrap();

    stream.set_read_timeout(Some(DEFAULT_READ_TIMEOUT)).unwrap();
    let mut buf = [0_u8; 4096];
    match stream.read(&mut buf) {
        Ok(0) => {} // the connection closed - decision-2's "refused"
        Ok(n) => panic!("expected the connection to close, got {n} more bytes"),
        Err(e) => panic!("expected a clean close (Ok(0)), got {e}"),
    }

    stop_server(&stop, handle);
}

/// Floods a non-reading program's pty with enough input to keep that
/// pane's own worker thread genuinely busy inside a real `write_all` for
/// a bounded-but-real stretch of wall-clock time - measured directly on
/// this machine (`pty_block_test2.py`, this task's own scratch script,
/// not committed): a pty with nobody reading its slave does not refuse
/// further writes here at any size this task tried (up to 370 MB over
/// 10 s) - it sustains a steady, bounded throughput (~37 MB/s) instead of
/// the hard, buffer-exhaustion block a Linux `n_tty` queue gives. Either
/// shape proves the same thing this task's own plan needs (the worker
/// thread's `write_all` stays busy, not returned, for a real stretch of
/// time): 20 chunks of 7 MB (under the wire's own 8 MiB frame cap,
/// decision-2 section 3), each its own `PaneCommand::Input` behind the
/// last on the pane's own command queue, keeps that worker busy for
/// several real seconds - comfortably past every bounded check below,
/// without making the whole test suite slow.
fn flood_stdin(stream: &mut UnixStream, connection: &Connection, attachment_stream: u32) {
    let chunk = vec![b'x'; 7 * 1024 * 1024];
    for _ in 0..10 {
        send_input(stream, connection, attachment_stream, &chunk);
    }
}

/// Every test that calls [`flood_stdin`] holds this for its own whole
/// body: `cargo test`'s default thread-per-test parallelism would
/// otherwise run several of these at once, each assuming close to this
/// machine's own full measured throughput (`flood_stdin`'s own doc) -
/// true one at a time, not guaranteed when several compete for the same
/// underlying pty/socket I/O at once. Every other test in this file is
/// unaffected: only the handful that actually flood take this lock.
static FLOOD_TEST_LOCK: Mutex<()> = Mutex::new(());

/// Where the flood tests' own cross-process lock file lives - outside
/// every worktree, the same reasoning `tools/src/steg_tools/
/// quiet.py::lock_directory` already uses for TASK-1.132's own slots:
/// never `$TMPDIR`/`std::env::temp_dir()` (per-worktree-sandboxed, and
/// the whole point here is to serialise *across* worktrees, the
/// opposite of every other path in this file, which is deliberately
/// unique per test) - `$XDG_CACHE_HOME/steg/test-locks`, else
/// `$HOME/.cache/steg/test-locks`.
fn flood_lock_path() -> PathBuf {
    let base = std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(
                std::env::var_os("HOME")
                    .expect("HOME should be set on any machine this test suite runs on"),
            )
            .join(".cache")
        });
    base.join("steg")
        .join("test-locks")
        .join("attach-flood.lock")
}

/// Acquires the machine-wide, cross-process flood-test lock, bounded
/// (TASK-1.123): a watchdog thread does the real, possibly-blocking
/// `flock` (`nix::fcntl::Flock::lock`, `FlockArg::LockExclusive` - the
/// same primitive `arbiter.rs` already uses for the arbiter's own lock,
/// just blocking here instead of non-blocking), and this thread waits
/// on it through a bounded channel receive (the same `run_bounded`
/// shape `tests/support/mod.rs` already has) - a wait for the lock
/// itself fails this test with a message naming what it waited for,
/// rather than becoming the next unbounded wait this task was meant to
/// remove.
///
/// Released by the OS the instant the returned guard's own fd closes,
/// for any reason including this process being `SIGKILL`ed -
/// `nix::fcntl::Flock`'s own `Drop` already unlocks explicitly on an
/// ordinary exit, and the kernel does the identical thing regardless
/// once the fd table is torn down
/// (`tools/src/steg_tools/quiet.py::held_slot`'s own doc states the
/// same guarantee, for the same reason: "the kernel drops the flock the
/// instant this process's last reference to that fd closes... There is
/// no lock, pid, or state file recording who holds a slot, so a killed
/// run leaves nothing behind to clean up"). No stale-lock detection or
/// cleanup code is written here because none is needed.
///
/// Does not deadlock with TASK-1.132's own two machine-wide cargo
/// slots: a worktree that already holds one of those two slots never
/// needs the *other* one to finish its own run, and never releases its
/// held slot back mid-run for someone else to use - this lock is the
/// only *additional* resource a flood test ever waits on beyond what it
/// already, irrevocably holds, so at most one side of this lock is ever
/// waiting, and the side holding it is never itself waiting on anything
/// the waiter holds. A cycle needs two resources each side waits on in
/// opposite order; the slot-then-lock ordering here (a slot is always
/// acquired before this lock is ever requested, and released only after
/// this lock already was) rules that out by construction.
///
/// # Panics
/// If the lock could not be acquired within 180s, or failed for a
/// reason other than contention.
///
/// 180s, not 60s (TASK-1.143): the five tests that take this lock run
/// four-to-a-worktree (the four `steg-server` feature variants `just
/// check` builds), so one worktree's own run alone can already queue up
/// to 20 acquisitions of this one machine-wide lock; a second worktree
/// building at the same time, slowing down whichever process currently
/// holds it, adds more queue depth on top. Measured directly, not
/// guessed: TASK-1.143's own bounded experiment ran these four binaries
/// together twice with two sibling `cargo build --workspace` processes
/// genuinely overlapping as load - 3 of those 8 runs (2 repetitions x 4
/// binaries) failed, every one of them
/// `a_second_connections_control_call_is_not_blocked_by_a_stuck_panes_
/// lock_held_attach` and no other test in this file, every one of them at
/// the old 60s bound - while every one of those same 8 runs' own total
/// suite time (31 tests each, including the other four already-guarded
/// tests) still finished within about 150s. 180s is comfortably past that
/// observed worst case, still a liveness bound rather than removed
/// (AC #4), and still well inside `steg-quiet`'s own 900s per-step
/// timeout (`tools/src/steg_tools/quiet.py`), the bound that would
/// otherwise kill this whole test binary first.
fn cross_process_flood_guard() -> nix::fcntl::Flock<std::fs::File> {
    let path = flood_lock_path();
    std::fs::create_dir_all(
        path.parent()
            .expect("flood_lock_path always returns a path with a parent"),
    )
    .expect("creating the flood lock's own directory should succeed");
    let file =
        std::fs::File::create(&path).expect("creating/opening the flood lock file should succeed");

    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let result = nix::fcntl::Flock::lock(file, nix::fcntl::FlockArg::LockExclusive);
        let _ = tx.send(result.map_err(|(_file, errno)| errno));
    });
    let bound = Duration::from_secs(180);
    match rx.recv_timeout(bound) {
        Ok(Ok(locked)) => locked,
        Ok(Err(errno)) => {
            panic!("could not lock the cross-process flood-test lock at {path:?}: {errno}")
        }
        Err(_) => panic!(
            "timed out after {bound:?} waiting for the cross-process flood-test lock at {path:?} - \
             another worktree's own flood test held it for longer than this bound allows"
        ),
    }
}

/// AC #1's own cost, proven directly (TASK-1.114's plan, §1 and proof
/// item 9): flooding a program that never reads its stdin at all (`sleep`)
/// makes that *one* pane's own worker thread stuck in a blocking
/// `write_all` - proven here, not assumed, by a second `pane.attach` to
/// the very same pane never getting a reply (it queues behind the stuck
/// write on that pane's own command channel) - while, on bounded waits,
/// (a) a plain control call on the *same* connection still answers and
/// (b) a second, unrelated pane on the same connection still receives
/// updates. If either (a) or (b) ever failed, that would be a real
/// regression this plan names nowhere as a limit, not something to
/// accept - this test's job is to confirm nothing beyond the one named
/// limit (that stuck pane's own screen freezing) rides along with it.
#[test]
fn an_unresponsive_panes_own_commands_do_not_stall_the_connection_or_another_pane() {
    let _flood_guard = FLOOD_TEST_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let _cross_process_guard = cross_process_flood_guard();
    let (_socket_dir, socket_path, stop, handle) = start_server();
    let (mut stream, mut connection) = connect_and_handshake(&socket_path);

    let stuck_pane = new_pane_running(
        &mut stream,
        &mut connection,
        vec!["sh", "-c", "exec sleep 100"],
    );
    let other_pane = new_pane_running(
        &mut stream,
        &mut connection,
        vec!["sh", "-c", "while true; do echo flooding; sleep 0.02; done"],
    );

    let attached_stuck = attach(&mut stream, &mut connection, &stuck_pane);
    let attached_other = attach(&mut stream, &mut connection, &other_pane);
    // At least one real update from the other pane before flooding, so
    // this test's own later check (another update still arrives) proves
    // something that was genuinely still flowing, not a pane that
    // happened never to produce anything.
    let _ = next_update(
        &mut stream,
        &mut connection,
        attached_other.stream,
        Duration::from_secs(5),
    );

    flood_stdin(&mut stream, &connection, attached_stuck.stream);

    // Proof the stuck pane's own worker really is stuck: a second attach
    // to it, queued behind the flood on that pane's own command channel,
    // gets no reply within a short, bounded window.
    let mut params = Map::new();
    params.insert("target".to_string(), json!(stuck_pane));
    let (stuck_attach_id, bytes) = connection
        .call("pane.attach", params)
        .expect("a valid call");
    stream.write_all(&bytes).unwrap();
    let reply = reply_within(
        &mut stream,
        &mut connection,
        stuck_attach_id,
        Duration::from_secs(1),
    );
    assert!(
        reply.is_none(),
        "the stuck pane's own worker answered a second attach - it was not actually stuck: {reply:?}"
    );

    // (a) a plain control call on the same connection still answers -
    // `reply_within`, not `call_and_read_reply`: the abandoned second
    // attach's own reply (above) is still outstanding and will arrive on
    // this same connection at some later, unpredictable point once the
    // flood drains, so this call must skip past it by id, not assume the
    // next reply frame is its own.
    let mut params = Map::new();
    params.insert("target".to_string(), json!(other_pane));
    let (show_id, bytes) = connection.call("pane.show", params).expect("a valid call");
    stream.write_all(&bytes).unwrap();
    // 15s, not 5s: this only has to prove the control call answers *at
    // all* (never truly stuck), not race a tight deadline against this
    // machine's own, unrelated CPU contention - `just check` runs several
    // other test binaries at the same time, which can starve this one's
    // own socket reads well past 5s without the server itself being slow.
    let response = reply_within(
        &mut stream,
        &mut connection,
        show_id,
        Duration::from_secs(15),
    )
    .expect("pane.show must answer within a bound, even with the other pane stuck");
    assert!(matches!(response, Response::Ok(_)), "{response:?}");

    // (b) the other pane still receives updates - 15s for the same
    // contention-margin reason as the control call above.
    let _ = next_update(
        &mut stream,
        &mut connection,
        attached_other.stream,
        Duration::from_secs(15),
    );

    stop_server(&stop, handle);
}

/// A plain control call from a connection that never touched the stuck
/// pane answers quickly, proving the server-wide freeze TASK-1.114's own
/// accepted limit disclaims ("only that pane's screen") - not merely that
/// one connection's own request queue is sequential.
///
/// `an_unresponsive_panes_own_commands_do_not_stall_the_connection_or_
/// another_pane`, above, issues every one of its calls over a single
/// connection - its own `(a)` check only proves that *this* connection's
/// read loop eventually gets back around to `pane.show` once the stuck
/// pane's worker drains (`connection.rs`'s own `run_post_handshake` reads
/// and handles one request at a time per connection, so that much is true
/// of a perfectly healthy server too, and would pass even if every other
/// connection in the process were also frozen).
///
/// This test instead issues `pane.show` from a *second*, otherwise idle
/// connection, opened only after the first connection's own attach to the
/// stuck pane is already known-stuck (the same proof `(a)` above uses: no
/// reply within a short bound). `connection.rs` intercepts `pane.attach`
/// before `Runtime::handle`'s own generic path (`Runtime::attach_pane`),
/// and `pane.show` reaches that generic path, which needs `Runtime::lock`
/// the same as everything else - so if `attach_pane` ever holds that lock
/// across its own wait on the stuck pane's worker, this second
/// connection's unrelated `pane.show` queues up behind it too, and this
/// test fails.
///
/// 2s, not 15s: unlike the "eventually drains" checks above, this one
/// does not wait on the stuck pane at all if the server is healthy - nothing
/// here competes with the pty flood for a multi-second event, so a bound
/// this much tighter than this file's own "eventually drains" margin still
/// has plenty of room for `just check`'s own concurrent CPU contention.
///
/// Control (TASK-1.114, following TASK-1.47's own review finding that
/// `Runtime::attach_pane` holds the runtime lock across `PaneHandle::
/// attach`'s unbounded `recv`): recorded failing against the pre-fix
/// `attach_pane` via `asdlc record-check` - this second connection's
/// `pane.show` got no reply within 2s, confirming the lock actually is
/// held across the wait today. Recorded passing once TASK-1.47's
/// restructuring (not touched by this task) stops holding the lock across
/// that wait.
///
/// 12x `flood_stdin`'s own batch (840MB, not 70MB), on a dedicated
/// connection used for nothing else: this measured (an earlier draft of
/// this test, using `flood_stdin`'s own 70MB) draining well inside the 2s
/// bound below - too fast to tell a held lock apart from a released one,
/// since either way the real check would then be answered by a fully
/// recovered worker, not a stuck one. The sending side itself never
/// blocks on the pty at all - it is a socket write the server's reader
/// drains into the pane's own unbounded command channel - so sending 12x
/// the data does not cost anywhere near 12x the wall-clock time; only the
/// one pane worker's own serial, blocking pty writes do, and those stay
/// well behind both bounded checks below.
#[test]
fn a_second_connections_control_call_is_not_blocked_by_a_stuck_panes_lock_held_attach() {
    let _flood_guard = FLOOD_TEST_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let _cross_process_guard = cross_process_flood_guard();
    let (_socket_dir, socket_path, stop, handle) = start_server();
    let (mut stream_a, mut connection_a) = connect_and_handshake(&socket_path);

    let stuck_pane = new_pane_running(
        &mut stream_a,
        &mut connection_a,
        vec!["sh", "-c", "exec sleep 100"],
    );
    let other_pane = new_pane_running(
        &mut stream_a,
        &mut connection_a,
        vec!["sh", "-c", "while true; do echo flooding; sleep 0.02; done"],
    );
    let _attached_other = attach(&mut stream_a, &mut connection_a, &other_pane);

    // The second connection used for the real check below, opened and
    // handshaken *before* the flood starts: a brand new connection's own
    // handshake needs `Runtime::register_subscriber` (TASK-1.46 §7),
    // which takes `Runtime::lock` too, so opening it only after the pane
    // is already stuck would at best conflate "a new connection can even
    // join" with "an already-connected one's plain control call still
    // answers" - two different proofs. Opening it first removes that
    // ambiguity: whatever happens next can only be explained by this
    // connection's own later `pane.show`, not by its own handshake.
    let (mut stream_b, mut connection_b) = connect_and_handshake(&socket_path);

    // A dedicated connection for the flood, so nothing else this test
    // does ever shares a socket with it.
    let (mut flood_stream, mut flood_connection) = connect_and_handshake(&socket_path);
    let attached_stuck = attach(&mut flood_stream, &mut flood_connection, &stuck_pane);
    let chunk = vec![b'x'; 7 * 1024 * 1024];
    for _ in 0..120 {
        send_input(
            &mut flood_stream,
            &flood_connection,
            attached_stuck.stream,
            &chunk,
        );
    }

    // Proof the stuck pane's own worker really is stuck, the same shape
    // as the single-connection test above: a second attach to it, queued
    // behind the flood on that pane's own command channel, gets no reply
    // within a short, bounded window.
    let mut params = Map::new();
    params.insert("target".to_string(), json!(stuck_pane));
    let (stuck_attach_id, bytes) = connection_a
        .call("pane.attach", params)
        .expect("a valid call");
    stream_a.write_all(&bytes).unwrap();
    let reply = reply_within(
        &mut stream_a,
        &mut connection_a,
        stuck_attach_id,
        Duration::from_secs(1),
    );
    assert!(
        reply.is_none(),
        "the stuck pane's own worker answered a second attach - it was not actually stuck: {reply:?}"
    );

    // The real check: `connection_b`, already connected before the flood
    // started and never having touched `stuck_pane` at all, issues one
    // plain control call and must get an answer quickly - never
    // serialized behind a different connection's own worker-bound attach.
    let mut params = Map::new();
    params.insert("target".to_string(), json!(other_pane));
    let (show_id, bytes) = connection_b
        .call("pane.show", params)
        .expect("a valid call");
    stream_b.write_all(&bytes).unwrap();
    let response = reply_within(
        &mut stream_b,
        &mut connection_b,
        show_id,
        Duration::from_secs(2),
    )
    .expect(
        "a second connection's plain control call must answer within 2s, \
             never waiting on a different connection's own stuck-pane attach",
    );
    assert!(matches!(response, Response::Ok(_)), "{response:?}");

    stop_server(&stop, handle);
}

/// `pane.close --force` returns within its own documented bound
/// (`force_grace + ESCALATE_CONFIRM_TIMEOUT + FORCE_JOIN_TIMEOUT`) even
/// while that exact pane's own worker is stuck in a blocking `write_all`.
/// `force_end`'s own six passes (`crates/server/src/runtime/close.rs`) never
/// wait on the worker thread past a stated timeout; only the final two
/// (`settle_worker`/`join_worker`) touch it at all, and both are bounded.
#[test]
fn pane_close_force_returns_within_its_bound_even_while_the_panes_own_worker_is_stuck() {
    let _flood_guard = FLOOD_TEST_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let _cross_process_guard = cross_process_flood_guard();
    let (_socket_dir, socket_path, stop, handle) = start_server();
    let (mut stream, mut connection) = connect_and_handshake(&socket_path);

    let pane = new_pane_running(
        &mut stream,
        &mut connection,
        vec!["sh", "-c", "exec sleep 100"],
    );
    let attached = attach(&mut stream, &mut connection, &pane);
    flood_stdin(&mut stream, &connection, attached.stream);

    let mut params = Map::new();
    params.insert("target".to_string(), json!(pane));
    params.insert("force".to_string(), json!(true));
    let (id, bytes) = connection.call("pane.close", params).expect("a valid call");
    stream.write_all(&bytes).unwrap();
    let started = Instant::now();
    // `reply_within`, not `call_and_read_reply`: a generous bound, not a
    // tight race against this machine's own, unrelated CPU contention
    // (`just check` runs several other test binaries at the same time) -
    // this item is about `force_end` having a bound *at all*, not about
    // how close to force_grace's own nominal ~6s this specific run lands.
    let response = reply_within(&mut stream, &mut connection, id, Duration::from_secs(30))
        .expect("pane.close --force must answer within a bound, even against a stuck worker");
    let elapsed = started.elapsed();
    assert!(matches!(response, Response::Ok(_)), "{response:?}");
    assert!(
        elapsed < Duration::from_secs(30),
        "pane.close --force took {elapsed:?} against a stuck worker - force_end's own bound is \
         meant to hold regardless"
    );

    stop_server(&stop, handle);
}

/// Stopping the whole server returns within its own documented bound even
/// while one pane's worker is stuck the same way - `Runtime::stop` uses
/// the identical `force_end` a forced `pane.close` does, for every pane
/// still running (`crates/server/src/runtime/lifecycle.rs`'s own doc on `Runtime::
/// stop`).
#[test]
fn stopping_the_server_returns_within_its_bound_even_while_a_panes_worker_is_stuck() {
    let _flood_guard = FLOOD_TEST_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let _cross_process_guard = cross_process_flood_guard();
    let (_socket_dir, socket_path, stop, handle) = start_server();
    let (mut stream, mut connection) = connect_and_handshake(&socket_path);

    let pane = new_pane_running(
        &mut stream,
        &mut connection,
        vec!["sh", "-c", "exec sleep 100"],
    );
    let attached = attach(&mut stream, &mut connection, &pane);
    flood_stdin(&mut stream, &connection, attached.stream);

    let started = Instant::now();
    stop_server(&stop, handle);
    let elapsed = started.elapsed();
    // 30s, not 10s - the same reasoning `pane_close_force_returns_within_
    // its_bound_even_while_the_panes_own_worker_is_stuck`'s own comment
    // gives: a generous margin against this machine's own unrelated CPU
    // contention, not a tight race against `force_end`'s nominal ~6s.
    assert!(
        elapsed < Duration::from_secs(30),
        "stopping the server took {elapsed:?} against a stuck pane's worker - Runtime::stop's \
         own bound is meant to hold regardless"
    );
}

/// TASK-1.125's own race, reproduced over two real connections rather
/// than inferred from `finish_attach`'s own unit tests (which exercise
/// `armed: bool` as a given, not the real timing that produces it) - the
/// end-to-end test TASK-1.125 could not build on its own, now possible
/// with this task's own flood/stuck-pane machinery plus TASK-1.125's fix.
///
/// Connection two's own `pane.attach` is deliberately queued behind a
/// large, deterministic flood on the pane's own worker (the same
/// technique `a_second_connections_control_call_is_not_blocked_by_a_
/// stuck_panes_lock_held_attach` uses, and for the same reason: a fixed
/// small batch risks draining before the race below gets a chance to
/// run). While it is still queued, connection one force-closes the pane.
/// `Runtime`'s own close-and-force removes the pane from its own map
/// under the lock, synchronously, *before* `force_end`'s signal/kill
/// passes even run (`crates/server/src/runtime/close.rs`, the `to_force`
/// extraction ahead of `force_end`) - so the pane is already gone by the
/// time connection two's queued attach is finally reached. The worker
/// thread itself is not torn down until `force_end`'s own later passes,
/// so it drains its backlog and answers that queued attach normally,
/// unaware the pane it belongs to has already been removed - exactly the
/// gap between "attach succeeded" and "arm found nothing to arm" TASK-
/// 1.125 named. `arm_attachment` (runtime/attach.rs) checks the pane is still in
/// its own map before arming; `finish_attach` (connection.rs) does not
/// record the attachment when it is not. What this test checks is not
/// connection two's own `pane.attach` reply (which may itself report
/// success - the "silent stream" TASK-1.125's own title names), but that
/// a later `pane.detach` for the same target never succeeds, rather than
/// succeeding against nothing or leaking (the exact error code observed
/// is `NoSuchTarget`, not `BadRequest` - the target itself is gone too,
/// not merely never attached by this connection; see this test's own
/// real check for why).
///
/// Control: reverted `arm_attachment` to `fn arm_attachment(...) -> bool
/// { let state = self.lock(); if let Some(handle) = state.panes.get(&pane)
/// { let _ = handle.arm(attachment_id, outbound); } true }` (armed
/// unconditionally, TASK-1.125's own bug) - the `pane.detach` below then
/// answered `Ok`, not an error, confirmed via `asdlc record-check`
/// before being reverted back.
#[test]
fn a_pane_that_closes_between_a_queued_attach_and_its_arm_leaves_nothing_to_detach() {
    let _flood_guard = FLOOD_TEST_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let _cross_process_guard = cross_process_flood_guard();
    let (_socket_dir, socket_path, stop, handle) = start_server();
    let (mut stream_one, mut connection_one) = connect_and_handshake(&socket_path);

    let pane = new_pane_running(
        &mut stream_one,
        &mut connection_one,
        vec!["sh", "-c", "exec sleep 100"],
    );
    let attached_one = attach(&mut stream_one, &mut connection_one, &pane);

    // A dedicated, large, synchronous flood - deterministic regardless
    // of this machine's own real pty throughput (see the lock-contention
    // test above for the same reasoning): big enough that the worker has
    // not drained it by the time this test reaches its own real check.
    let chunk = vec![b'x'; 7 * 1024 * 1024];
    for _ in 0..120 {
        send_input(
            &mut stream_one,
            &connection_one,
            attached_one.stream,
            &chunk,
        );
    }

    let (mut stream_two, mut connection_two) = connect_and_handshake(&socket_path);
    let mut params = Map::new();
    params.insert("target".to_string(), json!(pane));
    let (attach_id, bytes) = connection_two
        .call("pane.attach", params)
        .expect("a valid call");
    stream_two.write_all(&bytes).unwrap();

    // Proof connection two's own attach really is still queued, same
    // shape as this file's own other stuck-pane proofs: no reply within
    // a short, bounded window.
    let reply = reply_within(
        &mut stream_two,
        &mut connection_two,
        attach_id,
        Duration::from_secs(1),
    );
    assert!(
        reply.is_none(),
        "connection two's own attach answered before the flood drained - this test's own race \
         never had a chance to run: {reply:?}"
    );

    // Connection one force-closes the pane while connection two's attach
    // is still queued behind the flood. `reply_within`, not `call_and_
    // read_reply`: the same reasoning `pane_close_force_returns_within_
    // its_bound_even_while_the_panes_own_worker_is_stuck`'s own comment
    // gives - `call_and_read_reply`'s own underlying socket read uses
    // this connection's plain 5s default timeout, which `force_grace`
    // plus CI's own contention can exceed even though force_end's real
    // bound is much wider.
    let mut params = Map::new();
    params.insert("target".to_string(), json!(pane));
    params.insert("force".to_string(), json!(true));
    let (close_id, bytes) = connection_one
        .call("pane.close", params)
        .expect("a valid call");
    stream_one.write_all(&bytes).unwrap();
    let response = reply_within(
        &mut stream_one,
        &mut connection_one,
        close_id,
        Duration::from_secs(30),
    )
    .expect("pane.close --force must answer within a bound");
    assert!(
        matches!(response, Response::Ok(_)),
        "pane.close --force failed: {response:?}"
    );

    // Connection two's own queued attach now resolves - the worker
    // thread is still alive at this point (torn down only by `force_
    // end`'s own later passes), so it drains its backlog and answers the
    // queued command normally, with a real snapshot (truthfully - the
    // pane was still there when it was read).
    let reply = reply_within(
        &mut stream_two,
        &mut connection_two,
        attach_id,
        Duration::from_secs(15),
    )
    .expect("pane.attach must answer within a bound");
    let attached: PaneAttached = match reply {
        Response::Ok(value) => {
            serde_json::from_value(value).expect("pane.attach's reply is a PaneAttached")
        }
        Response::Error(error) => panic!("pane.attach failed: {error:?}"),
    };
    connection_two.open_stream(attached.stream);

    // TASK-1.128: told the same way a worker-side end is, in place of
    // TASK-1.125's own silent close - the pane's worker itself was
    // already gone by the time `arm_attachment` ran, so this is `finish_
    // attach`'s own fix (`connection.rs`), not the worker's `flush_ended`.
    let update = next_update(
        &mut stream_two,
        &mut connection_two,
        attached.stream,
        Duration::from_secs(15),
    );
    assert_eq!(
        update,
        ScreenUpdate::Ended {
            reason: EndReason::Closed
        },
        "a pane that closed between attach and arm must be told the same way a worker-side \
         end is, not silently - TASK-1.125's own bug"
    );
    assert_nothing_more_arrives_on(
        &mut stream_two,
        &mut connection_two,
        attached.stream,
        Duration::from_millis(300),
    );

    // The real check: a `pane.detach` for the same target must not
    // succeed - this connection holds no real attachment to arm or
    // release, and `pane.close --force` already removed the target from
    // the tree too, so the actual code observed is `NoSuchTarget` (the
    // target itself is gone), not the `BadRequest` a merely-never-
    // attached-but-still-live pane would answer with (`detaching_a_pane_
    // this_connection_never_attached_to_is_a_bad_request`, above) -
    // `connection.rs`'s own already-closed-pane fallback only resolves
    // through this connection's `pane_streams`, which has nothing here
    // either way. Either error code is a correct refusal; `Ok` is the
    // one outcome TASK-1.125's own "silent stream" bug produced.
    let mut params = Map::new();
    params.insert("target".to_string(), json!(pane));
    let (detach_id, bytes) = connection_two
        .call("pane.detach", params)
        .expect("a valid call");
    stream_two.write_all(&bytes).unwrap();
    let response = reply_within(
        &mut stream_two,
        &mut connection_two,
        detach_id,
        Duration::from_secs(15),
    )
    .expect("pane.detach must answer within a bound");
    match response {
        Response::Error(error) => {
            assert_eq!(error.code, steg_sdk::ErrorCode::NoSuchTarget, "{error:?}");
        }
        Response::Ok(_) => panic!(
            "pane.detach for a pane that closed between attach and arm must not succeed - \
             TASK-1.125's own 'silent stream' bug"
        ),
    }

    stop_server(&stop, handle);
}

// --- TASK-1.128: the pane ending, on an attachment's own stream -----------

/// Reads updates on `wanted_stream` until `Ended` arrives, discarding
/// anything else first. The very first tick a pane's own worker ever
/// runs always delivers a real `Full`, even with nothing printed (the
/// kernel's own documented rule for a point it has never seen: `None` -
/// `crates/server/src/pane.rs`'s own `a_new_attachments_first_tick_is_a_
/// full_screen_not_a_delta`) - `flush_ended`'s own forced last tick is no
/// exception, so a client already attached and armed before the pane
/// ends can see that catch-up `Full` land immediately ahead of `Ended`.
/// This is AC #1's own "after any screen update still owed" working as
/// designed, not a bug - a bare, single `next_update` call cannot assume
/// `Ended` is the very first frame read.
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

/// AC #1: a program that exits by itself (no close, forced or otherwise,
/// in progress) sends `Ended{ProgramExited}` with its own real exit code,
/// after any screen update still owed, and nothing arrives on the stream
/// afterward.
#[test]
fn a_program_that_exits_on_its_own_sends_ended_with_its_real_exit_code() {
    let (_socket_dir, socket_path, stop, handle) = start_server();
    let (mut stream, mut connection) = connect_and_handshake(&socket_path);

    let pane = new_pane_running(&mut stream, &mut connection, vec!["sh", "-c", "exit 7"]);
    let attached = attach(&mut stream, &mut connection, &pane);

    let reason = next_ended(
        &mut stream,
        &mut connection,
        attached.stream,
        Duration::from_secs(5),
    );
    assert_eq!(
        reason,
        EndReason::ProgramExited { exit_code: 7 },
        "a program that exits on its own must report its real exit code, not Closed"
    );
    assert_nothing_more_arrives_on(
        &mut stream,
        &mut connection,
        attached.stream,
        Duration::from_millis(300),
    );

    stop_server(&stop, handle);
}

/// AC #1: `pane.close --force` on a pane with one attached, still-running
/// client reports `Ended{Closed}`, not the program's own exit code -
/// `force_end`'s new `MarkClosing` pass, proven here at the one level
/// that actually exercises its real ordering against pass 1's signal (the
/// bug an advisor review found in this plan's own first draft: `pane.rs`'s
/// own unit test for this proves only the worker's flag-precedence logic,
/// never `force_end`'s pass order, which is a real property only at this,
/// the socket level).
#[test]
fn pane_close_force_reports_ended_closed_not_the_signals_own_exit_code() {
    let (_socket_dir, socket_path, stop, handle) = start_server();
    let (mut stream, mut connection) = connect_and_handshake(&socket_path);

    let pane = new_pane_running(
        &mut stream,
        &mut connection,
        vec!["sh", "-c", "exec sleep 100"],
    );
    let attached = attach(&mut stream, &mut connection, &pane);

    let mut params = Map::new();
    params.insert("target".to_string(), json!(pane));
    params.insert("force".to_string(), json!(true));
    let (_close_id, bytes) = connection.call("pane.close", params).expect("a valid call");
    stream.write_all(&bytes).unwrap();

    let reason = next_ended(
        &mut stream,
        &mut connection,
        attached.stream,
        Duration::from_secs(30),
    );
    assert_eq!(
        reason,
        EndReason::Closed,
        "a forced close must report Closed, never the signal's own exit code"
    );
    assert_nothing_more_arrives_on(
        &mut stream,
        &mut connection,
        attached.stream,
        Duration::from_millis(300),
    );
    // `pane.close`'s own reply (`_close_id`) is deliberately left
    // undrained: `next_ended`/`assert_nothing_more_arrives_on` above both
    // discard any frame that is not what they are each looking for
    // (`next_update`'s own `Some(_) => {}`), so chasing that reply too,
    // in either order, would race losing it against one of those two
    // calls doing exactly that - not worth the fragility for a reply
    // `pane_close_force_returns_within_its_bound_even_while_the_panes_
    // own_worker_is_stuck` already covers on its own.

    stop_server(&stop, handle);
}

/// AC #1: two clients attached to one pane both get `Ended`, independently.
/// This is the fan-out `flush_ended` does over every armed attachment,
/// not only the one that happened to ask first.
#[test]
fn two_clients_attached_to_one_pane_both_get_ended_for_the_same_exit() {
    let (_socket_dir, socket_path, stop, handle) = start_server();
    let (mut stream_a, mut connection_a) = connect_and_handshake(&socket_path);
    let pane = new_pane_running(&mut stream_a, &mut connection_a, vec!["sh", "-c", "exit 4"]);
    let attached_a = attach(&mut stream_a, &mut connection_a, &pane);

    let (mut stream_b, mut connection_b) = connect_and_handshake(&socket_path);
    let attached_b = attach(&mut stream_b, &mut connection_b, &pane);

    let reason_a = next_ended(
        &mut stream_a,
        &mut connection_a,
        attached_a.stream,
        Duration::from_secs(5),
    );
    let reason_b = next_ended(
        &mut stream_b,
        &mut connection_b,
        attached_b.stream,
        Duration::from_secs(5),
    );
    let expected = EndReason::ProgramExited { exit_code: 4 };
    assert_eq!(
        reason_a, expected,
        "connection A must see the real exit code"
    );
    assert_eq!(
        reason_b, expected,
        "connection B must see the real exit code too"
    );

    assert_nothing_more_arrives_on(
        &mut stream_a,
        &mut connection_a,
        attached_a.stream,
        Duration::from_millis(300),
    );
    assert_nothing_more_arrives_on(
        &mut stream_b,
        &mut connection_b,
        attached_b.stream,
        Duration::from_millis(300),
    );

    stop_server(&stop, handle);
}

/// AC #1: attaching to a pane whose program has already ended, well after
/// the fact - no race, no flood, just a plain second `pane.attach` once
/// the first client has already seen `Ended`. Proves this is the ordinary
/// way of looking at an ended pane (R97 keeps it and its scrollback
/// around), not merely something a tight race happens to produce. A
/// fresh `Full` arrives first (see the comment at this test's own second
/// `attach`, below), then `Ended`, and nothing at all afterward.
#[test]
fn attaching_to_a_pane_whose_program_already_ended_gets_a_final_full_then_ended() {
    let (_socket_dir, socket_path, stop, handle) = start_server();
    let (mut stream_a, mut connection_a) = connect_and_handshake(&socket_path);
    let pane = new_pane_running(&mut stream_a, &mut connection_a, vec!["sh", "-c", "exit 0"]);
    let attached_a = attach(&mut stream_a, &mut connection_a, &pane);

    let reason = next_ended(
        &mut stream_a,
        &mut connection_a,
        attached_a.stream,
        Duration::from_secs(5),
    );
    assert_eq!(
        reason,
        EndReason::ProgramExited { exit_code: 0 },
        "confirms the program has really, fully ended before the second attach below"
    );

    let (mut stream_b, mut connection_b) = connect_and_handshake(&socket_path);
    let attached_b = attach(&mut stream_b, &mut connection_b, &pane);
    // A fresh `Full` arrives first (`Attach`'s own reply snapshot is not
    // "on this stream," and a client can race it closing ahead of its
    // own, separate `arm` call - pane.rs's own equivalent unit test has
    // the full argument), then `Ended`.
    let reason_b = next_ended(
        &mut stream_b,
        &mut connection_b,
        attached_b.stream,
        Duration::from_secs(5),
    );
    assert_eq!(
        reason_b,
        EndReason::ProgramExited { exit_code: 0 },
        "a late attach to an already-ended pane must eventually see Ended, after its own \
         catch-up Full"
    );
    assert_nothing_more_arrives_on(
        &mut stream_b,
        &mut connection_b,
        attached_b.stream,
        Duration::from_millis(300),
    );

    stop_server(&stop, handle);
}
