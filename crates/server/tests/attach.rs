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
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use serde_json::{Map, Value, json};
use steg_sdk::{
    ClientKind, Connection, Incoming, PaneAttached, Response, ScreenUpdate, WireRole as Role,
    description, parse_server_hello,
};
use steg_server::{LoginShell, ServerConfig, Stopper, serve};

fn unique_socket_path() -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let suffix = COUNTER.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!("{}-attach-{suffix}.sock", std::process::id()))
}

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

fn start_server() -> (PathBuf, Stopper, std::thread::JoinHandle<()>) {
    let socket_path = unique_socket_path();
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
    (socket_path, stop, handle)
}

fn stop_server(stop: &Stopper, handle: std::thread::JoinHandle<()>) {
    stop.stop();
    handle.join().expect("the server thread should not panic");
}

/// Same shape as `routing.rs`'s own `connect_and_handshake` - the real
/// handshake, with the sdk's own `Connection`.
fn connect_and_handshake(socket_path: &std::path::Path) -> (UnixStream, Connection) {
    let mut stream = UnixStream::connect(socket_path).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
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
    let deadline = Instant::now() + Duration::from_secs(10);
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

    let deadline = Instant::now() + Duration::from_secs(10);
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

fn apply_update(held: &mut steg_sdk::Screen, update: ScreenUpdate) {
    match update {
        ScreenUpdate::Unchanged => {}
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

#[test]
fn attach_returns_the_real_screen_and_updates_follow_it() {
    let (socket_path, stop, handle) = start_server();
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
    let (socket_path, stop, handle) = start_server();
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
    let (socket_path, stop, handle) = start_server();
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
    let (socket_path, stop, handle) = start_server();
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
    let (socket_path, stop, handle) = start_server();

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
