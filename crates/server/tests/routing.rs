//! AC #1 ("a frame over the cap is refused without allocating"), AC #2
//! ("a request on the control stream reaches the API and its response or
//! error comes back with the same id"), AC #3's second proof part (a real
//! connection actually routes its replies through `Outbound` - the first
//! part, a genuine flood proving control-before-data, is
//! `outbound.rs`'s own `#[cfg(test)]` module), and the hostile-input cases
//! TASK-1.45's plan names in §3, each proved over a real socket with the
//! sdk's own `Connection` doing the handshake and building requests
//! exactly as a real client would.

#![expect(
    clippy::unwrap_used,
    reason = "this whole file is a test; see dispatch/main.rs's own comment"
)]

use std::io::{Read, Write};
use std::os::fd::AsRawFd;
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::time::{Duration, Instant};

// `nix::libc` (re-exported, unconditionally - `nix-0.28.0/src/lib.rs`'s
// own `pub use libc;`), not a new dependency: `steg-server` already
// depends on `nix` for the arbiter's lock and owner checks, and this is
// the same crate's own raw `getsockopt`/`ioctl` surface, used only by
// this file's own test below to read real socket buffer sizes and poll
// `FIONREAD` - nothing production reaches.
use nix::libc;
use serde_json::{Map, Value, json};
use steg_sdk::{
    CONTROL_STREAM, ClientHello, ClientKind, Connection, Control, Decoder, ErrorCode, Frame,
    Incoming, PROTOCOL_VERSION, Response, WireRole as Role, description, encode_frame,
    parse_server_hello,
};
use steg_server::{ConnectionEvent, LoginShell, ServerConfig, Stopper, serve};

fn unique_socket_path() -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let suffix = COUNTER.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!("{}-routing-{suffix}.sock", std::process::id()))
}

/// A working `PATH` is required (TASK-1.87): `RealLauncher::spawn`'s own
/// `.env_clear()` means `LoginShell::default()`'s own empty `base_env`
/// cannot resolve a relative program name like `"sh"` - this file's own
/// `stopping_through_a_real_connection_ends_a_real_programs_process_and_
/// frees_the_socket` is the one test here that actually starts a real
/// program through `session.new`, over the real socket. This test
/// file's own ambient `PATH` is fine to read here (a test, not
/// production code).
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

/// Same shape as `handshake.rs`'s own `start_server`, with a generous,
/// fixed handshake timeout - this file is about what happens *after* the
/// handshake, not the handshake itself.
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

/// Real handshake over a real socket, with the sdk's own `Connection` -
/// the shape every real client (CLI, app, plugin) actually uses, not a
/// hand-rolled stand-in.
fn connect_and_handshake(socket_path: &std::path::Path) -> (UnixStream, Connection) {
    let mut stream = UnixStream::connect(socket_path).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let hello_bytes = Connection::send_hello("routing-test-client", ClientKind::Cli, None);
    stream.write_all(&hello_bytes).unwrap();

    let mut connection = Connection::new(Role::Client, description());
    let hello_reply = loop {
        if let Some(incoming) = connection.poll() {
            match incoming {
                Incoming::Hello(payload) => break payload,
                other @ (Incoming::Request { .. }
                | Incoming::Reply { .. }
                | Incoming::Event { .. }
                | Incoming::UnknownEvent { .. }
                | Incoming::StreamControl { .. }
                | Incoming::Data { .. }
                | Incoming::UnknownStream { .. }
                | Incoming::Malformed(_)) => {
                    panic!("expected the server's hello reply, got {other:?}")
                }
            }
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

/// Writes a validated call and reads back its matching reply - the shape
/// a real client's own request/response loop takes.
fn call_and_read_reply(
    stream: &mut UnixStream,
    connection: &mut Connection,
    method: &str,
    params: Map<String, Value>,
) -> (u64, Response) {
    let (id, bytes) = connection.call(method, params).expect("a valid call");
    stream.write_all(&bytes).unwrap();
    // One deadline for the whole wait, not just each individual read -
    // this file's own flood test found that distinction matters under
    // load (a bound on each read alone lets the total time a read loop
    // runs for grow without one).
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Some(Incoming::Reply {
            id: reply_id,
            response,
        }) = connection.poll()
        {
            assert_eq!(reply_id, id, "the reply must carry the id this call minted");
            return (reply_id, response);
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

/// Reads exactly `want` replies, in the order they arrive - one overall
/// deadline for the whole wait, the same reasoning `call_and_read_reply`'s
/// own doc gives.
fn read_n_replies(
    stream: &mut UnixStream,
    connection: &mut Connection,
    want: usize,
) -> Vec<(u64, Response)> {
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut replies = Vec::new();
    while replies.len() < want {
        if let Some(Incoming::Reply { id, response }) = connection.poll() {
            replies.push((id, response));
            continue;
        }
        assert!(
            Instant::now() < deadline,
            "timed out waiting for {want} replies, got {}",
            replies.len()
        );
        let mut buf = [0_u8; 4096];
        let n = stream.read(&mut buf).unwrap();
        assert!(n > 0);
        connection.receive(&buf[..n]);
    }
    replies
}

/// Asserts `stream` closes - a clean `Ok(0)` or any error other than a
/// timeout - within its own read timeout. The one shared assertion every
/// hostile-input test below ends with.
fn assert_closes(stream: &mut UnixStream, bound: Duration) {
    stream.set_read_timeout(Some(bound)).unwrap();
    let mut buf = [0_u8; 16];
    match stream.read(&mut buf) {
        Ok(0) => {}
        Ok(n) => panic!("expected the connection to close, got {n} more bytes"),
        Err(err) => assert_ne!(
            err.kind(),
            std::io::ErrorKind::WouldBlock,
            "the connection should have closed within {bound:?}"
        ),
    }
}

// --- AC #2: a request reaches the API and its reply matches the id -------

#[test]
fn a_request_on_the_control_stream_reaches_the_api_and_the_reply_matches_the_id() {
    let (socket_path, stop, handle) = start_server();
    let (mut stream, mut connection) = connect_and_handshake(&socket_path);

    let (id, response) =
        call_and_read_reply(&mut stream, &mut connection, "session.list", Map::new());

    match response {
        Response::Ok(value) => assert_eq!(value, json!([]), "a fresh server has no sessions"),
        Response::Error(error) => panic!("session.list failed: {error:?}"),
    }
    assert!(id > 0, "the client's own call minted a real id");

    stop_server(&stop, handle);
}

/// Server-side `validate` still runs, independent of the client's own
/// pre-send check: a hand-encoded request naming a method the
/// description does not have (bypassing `Connection::call`'s own
/// `validate`) still gets a matching, correctly-coded error reply at the
/// same id - not a hang, not a crash, not a silently-dropped frame.
#[test]
fn a_request_that_would_fail_client_side_validation_still_gets_a_matching_server_error() {
    let (socket_path, stop, handle) = start_server();
    let (mut stream, mut connection) = connect_and_handshake(&socket_path);

    let control = Control::Request {
        id: 42,
        request: steg_sdk::Request {
            method: "not.a.real.method".to_string(),
            params: Map::new(),
        },
    };
    let bytes = encode_frame(&Frame::control(
        CONTROL_STREAM,
        control.to_payload().unwrap(),
    ))
    .unwrap();
    stream.write_all(&bytes).unwrap();

    let reply = loop {
        if let Some(Incoming::Reply { id, response }) = connection.poll() {
            break (id, response);
        }
        let mut buf = [0_u8; 4096];
        let n = stream.read(&mut buf).unwrap();
        assert!(n > 0);
        connection.receive(&buf[..n]);
    };
    assert_eq!(
        reply.0, 42,
        "the reply must carry the same id the request named"
    );
    match reply.1 {
        Response::Error(error) => assert_eq!(error.code, steg_sdk::ErrorCode::BadRequest),
        Response::Ok(value) => panic!("an unknown method must not succeed: {value}"),
    }

    stop_server(&stop, handle);
}

/// A request id reused while nothing is actually ever "in flight" on one
/// connection (TASK-1.45 §3): the server never tracks ids, so two
/// requests sharing one id each get their own, independently-correct
/// reply.
#[test]
fn two_requests_sharing_one_id_both_get_correctly_matched_independent_replies() {
    let (socket_path, stop, handle) = start_server();
    let (mut stream, mut connection) = connect_and_handshake(&socket_path);

    let first = Control::Request {
        id: 7,
        request: steg_sdk::Request {
            method: "session.list".to_string(),
            params: Map::new(),
        },
    };
    let second = Control::Request {
        id: 7,
        request: steg_sdk::Request {
            method: "session.new".to_string(),
            params: Map::new(),
        },
    };
    stream
        .write_all(
            &encode_frame(&Frame::control(CONTROL_STREAM, first.to_payload().unwrap())).unwrap(),
        )
        .unwrap();
    stream
        .write_all(
            &encode_frame(&Frame::control(
                CONTROL_STREAM,
                second.to_payload().unwrap(),
            ))
            .unwrap(),
        )
        .unwrap();

    let replies = read_n_replies(&mut stream, &mut connection, 2);

    assert_eq!(replies[0].0, 7);
    assert_eq!(replies[1].0, 7);
    match &replies[0].1 {
        Response::Ok(value) => assert_eq!(*value, json!([]), "session.list, answered first"),
        Response::Error(error) => panic!("session.list failed: {error:?}"),
    }
    match &replies[1].1 {
        Response::Ok(value) => assert!(
            value.get("id").is_some(),
            "session.new, answered second, with a real new session"
        ),
        Response::Error(error) => panic!("session.new failed: {error:?}"),
    }

    stop_server(&stop, handle);
}

/// Pipelining several calls on one connection and getting correctly
/// matched, in-order replies is AC #3's second proof part: it shows the
/// real production path (`connection::handle`) actually routes every
/// reply through `Outbound`, the same type `outbound.rs`'s own tests
/// drive directly for the priority claim itself.
#[test]
fn several_pipelined_calls_on_one_connection_get_correctly_matched_in_order_replies() {
    let (socket_path, stop, handle) = start_server();
    let (mut stream, mut connection) = connect_and_handshake(&socket_path);

    let calls = ["session.list", "session.new", "session.list"];
    let mut sent_ids = Vec::new();
    for method in calls {
        let (id, bytes) = connection.call(method, Map::new()).unwrap();
        sent_ids.push(id);
        stream.write_all(&bytes).unwrap();
    }

    let replies = read_n_replies(&mut stream, &mut connection, calls.len());

    assert_eq!(
        replies.iter().map(|(id, _)| *id).collect::<Vec<_>>(),
        sent_ids,
        "replies arrive in the same order the calls were sent, each with its own id"
    );
    assert!(matches!(replies[0].1, Response::Ok(_)));
    assert!(matches!(replies[1].1, Response::Ok(_)));
    // The second session.list (after session.new) must report one
    // session now - proves these are real, independently-dispatched
    // calls, not three copies of the same cached answer.
    match &replies[2].1 {
        Response::Ok(Value::Array(sessions)) => assert_eq!(sessions.len(), 1),
        other @ (Response::Ok(_) | Response::Error(_)) => {
            panic!("expected one session, got {other:?}")
        }
    }

    stop_server(&stop, handle);
}

/// The plan's own proof section said `Context::none()` and the
/// missing-target refusal are proved over a real socket - this is that
/// proof, over a session, a tab and a pane method, not only
/// `dispatch/lookup.rs`'s own unit tests (which exercise the same
/// decision without a connection at all). `target` is an optional
/// parameter on every one of these three methods (`validate` lets a
/// target-less request through), so a `BadRequest` reaching the client
/// here can only have come from dispatch's own missing-context
/// refusal, not the client- or server-side `validate` check.
#[test]
fn a_target_less_request_over_a_real_socket_is_bad_request_for_a_session_a_tab_and_a_pane_method() {
    let (socket_path, stop, handle) = start_server();
    let (mut stream, mut connection) = connect_and_handshake(&socket_path);

    for method in ["session.show", "tab.focus", "pane.show"] {
        let (_, response) = call_and_read_reply(&mut stream, &mut connection, method, Map::new());
        match response {
            Response::Error(error) => {
                assert_eq!(error.code, ErrorCode::BadRequest, "{method}: {error:?}");
                assert!(
                    error.message.contains("context"),
                    "{method}: expected a missing-context message, got {}",
                    error.message
                );
            }
            Response::Ok(value) => {
                panic!("{method} with no target and no context must not succeed: {value}")
            }
        }
    }

    stop_server(&stop, handle);
}

/// TASK-1.89's own proof item: "the lists and shows over a real socket
/// with the sdk's client" - `tab.list`, `tab.show` and `pane.list`, each
/// answered through a real connection and a real `Connection` doing its
/// own encoding/decoding, not just the in-process `dispatch` calls
/// `dispatch/main.rs`'s own tests already cover.
#[test]
fn tab_list_tab_show_and_pane_list_answer_over_a_real_socket() {
    let (socket_path, stop, handle) = start_server();
    let (mut stream, mut connection) = connect_and_handshake(&socket_path);

    let (_, response) =
        call_and_read_reply(&mut stream, &mut connection, "session.new", Map::new());
    let session = match response {
        Response::Ok(value) => value,
        Response::Error(error) => panic!("session.new failed: {error:?}"),
    };
    let session_id = session["id"].as_str().unwrap().to_string();

    let mut show_params = Map::new();
    show_params.insert("target".to_string(), json!(session_id));
    let (_, response) =
        call_and_read_reply(&mut stream, &mut connection, "session.show", show_params);
    let shown = match response {
        Response::Ok(value) => value,
        Response::Error(error) => panic!("session.show failed: {error:?}"),
    };
    let tab_id = shown["current_tab"].as_str().unwrap().to_string();

    let mut list_params = Map::new();
    list_params.insert("target".to_string(), json!(session_id));
    let (_, response) = call_and_read_reply(&mut stream, &mut connection, "tab.list", list_params);
    let tabs = match response {
        Response::Ok(value) => value,
        Response::Error(error) => panic!("tab.list failed: {error:?}"),
    };
    let tabs = tabs.as_array().unwrap();
    assert_eq!(tabs.len(), 1);
    assert_eq!(tabs[0]["id"], json!(tab_id));
    assert_eq!(tabs[0]["position"], json!(1));

    // A bare tab id needs this connection's own session context to
    // resolve, which this socket-only connection has none of
    // (`dispatch/lookup.rs`'s own missing-context refusal) - the full
    // `session/tab` path resolves with no context at all.
    let mut tab_show_params = Map::new();
    tab_show_params.insert(
        "target".to_string(),
        json!(format!("{session_id}/{tab_id}")),
    );
    let (_, response) =
        call_and_read_reply(&mut stream, &mut connection, "tab.show", tab_show_params);
    let tab = match response {
        Response::Ok(value) => value,
        Response::Error(error) => panic!("tab.show failed: {error:?}"),
    };
    assert_eq!(tab["id"], json!(tab_id));
    let current_pane = tab["current_pane"].as_str().unwrap().to_string();

    let mut pane_list_params = Map::new();
    pane_list_params.insert(
        "target".to_string(),
        json!(format!("{session_id}/{tab_id}")),
    );
    let (_, response) =
        call_and_read_reply(&mut stream, &mut connection, "pane.list", pane_list_params);
    let panes = match response {
        Response::Ok(value) => value,
        Response::Error(error) => panic!("pane.list failed: {error:?}"),
    };
    let panes = panes.as_array().unwrap();
    assert_eq!(panes.len(), 1);
    assert_eq!(panes[0]["id"], json!(current_pane));
    assert_eq!(panes[0]["kind"], json!("terminal"));

    stop_server(&stop, handle);
}

// --- AC #1: a frame over the cap, post-handshake --------------------------

#[test]
fn an_oversized_frame_post_handshake_is_refused_promptly_and_closes_the_connection() {
    let (socket_path, stop, handle) = start_server();
    let (mut stream, _connection) = connect_and_handshake(&socket_path);

    let started = Instant::now();
    stream.write_all(&0xFFFF_FFFF_u32.to_be_bytes()).unwrap();
    assert_closes(&mut stream, Duration::from_secs(2));
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "an oversized length is refused promptly, not after waiting anything out"
    );

    stop_server(&stop, handle);
}

// --- Hostile input, post-handshake (TASK-1.45 §3) -------------------------

#[test]
fn an_unknown_kind_tag_post_handshake_closes_the_connection() {
    let (socket_path, stop, handle) = start_server();
    let (mut stream, _connection) = connect_and_handshake(&socket_path);

    let mut bytes = 5_u32.to_be_bytes().to_vec(); // header-only body
    bytes.extend_from_slice(&CONTROL_STREAM.to_be_bytes());
    bytes.push(99); // not a known Kind tag
    stream.write_all(&bytes).unwrap();

    assert_closes(&mut stream, Duration::from_secs(2));
    stop_server(&stop, handle);
}

#[test]
fn a_non_json_control_payload_post_handshake_closes_the_connection() {
    let (socket_path, stop, handle) = start_server();
    let (mut stream, _connection) = connect_and_handshake(&socket_path);

    stream
        .write_all(
            &encode_frame(&Frame::control(CONTROL_STREAM, b"not json at all".to_vec())).unwrap(),
        )
        .unwrap();

    assert_closes(&mut stream, Duration::from_secs(2));
    stop_server(&stop, handle);
}

#[test]
fn a_reply_shaped_frame_from_the_client_closes_the_connection() {
    let (socket_path, stop, handle) = start_server();
    let (mut stream, _connection) = connect_and_handshake(&socket_path);

    let reply = Control::Reply {
        id: 1,
        response: Response::Ok(json!(null)),
    };
    stream
        .write_all(
            &encode_frame(&Frame::control(CONTROL_STREAM, reply.to_payload().unwrap())).unwrap(),
        )
        .unwrap();

    assert_closes(&mut stream, Duration::from_secs(2));
    stop_server(&stop, handle);
}

#[test]
fn an_event_from_the_client_closes_the_connection() {
    let (socket_path, stop, handle) = start_server();
    let (mut stream, _connection) = connect_and_handshake(&socket_path);

    let event = Control::Event {
        name: "pretending.to.be.the.server".to_string(),
        body: json!({}),
    };
    stream
        .write_all(
            &encode_frame(&Frame::control(CONTROL_STREAM, event.to_payload().unwrap())).unwrap(),
        )
        .unwrap();

    assert_closes(&mut stream, Duration::from_secs(2));
    stop_server(&stop, handle);
}

#[test]
fn a_second_hello_after_the_handshake_closes_the_connection() {
    let (socket_path, stop, handle) = start_server();
    let (mut stream, _connection) = connect_and_handshake(&socket_path);

    let hello = ClientHello::new("test-client", PROTOCOL_VERSION, ClientKind::Cli, None);
    let bytes = encode_frame(&Frame::hello(serde_json::to_vec(&hello).unwrap())).unwrap();
    stream.write_all(&bytes).unwrap();

    assert_closes(&mut stream, Duration::from_secs(2));
    stop_server(&stop, handle);
}

#[test]
fn data_on_a_stream_never_opened_closes_the_connection() {
    let (socket_path, stop, handle) = start_server();
    let (mut stream, _connection) = connect_and_handshake(&socket_path);

    // No attach exists yet (TASK-1.47), so every non-zero stream is, by
    // construction, one this connection never opened.
    stream
        .write_all(
            &encode_frame(&Frame::data(7, b"typed while nothing is attached".to_vec())).unwrap(),
        )
        .unwrap();

    assert_closes(&mut stream, Duration::from_secs(2));
    stop_server(&stop, handle);
}

/// Real-stack coverage for the broken-write fix (`outbound.rs`'s own
/// `a_broken_write_closes_the_queue_so_the_readers_next_enqueue_fails`
/// is what isolates the mechanism itself, and says in its own doc why a
/// routing-level test cannot do that instead): several replies queued,
/// then the client vanishes entirely before reading any of them - the
/// connection must still end cleanly and promptly, reported over the
/// events channel, with nothing stuck.
#[test]
fn a_client_that_vanishes_with_replies_still_queued_still_ends_the_connection_promptly() {
    let socket_path = unique_socket_path();
    let config = ServerConfig {
        socket_path: Some(socket_path.clone()),
        handshake_timeout: Duration::from_secs(5),
        ..ServerConfig::default()
    };
    let stop = Stopper::new();
    let (tx, rx) = mpsc::channel::<ConnectionEvent>();
    let handle = {
        let config = config.clone();
        let stop = stop.clone();
        std::thread::spawn(move || {
            let _ = serve(&config, &stop, Some(tx));
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
    assert_eq!(
        rx.recv_timeout(Duration::from_secs(5)).unwrap(),
        ConnectionEvent::Connected
    );
    assert_eq!(
        rx.recv_timeout(Duration::from_secs(5)).unwrap(),
        ConnectionEvent::Disconnected
    );

    let (mut stream, mut connection) = connect_and_handshake(&socket_path);
    assert_eq!(
        rx.recv_timeout(Duration::from_secs(5)).unwrap(),
        ConnectionEvent::Connected
    );

    for _ in 0..5 {
        let (_, bytes) = connection.call("session.list", Map::new()).unwrap();
        stream.write_all(&bytes).unwrap();
    }
    drop(stream); // the client is simply gone - never read a single reply

    assert_eq!(
        rx.recv_timeout(Duration::from_secs(5)).unwrap(),
        ConnectionEvent::Disconnected,
        "the connection must still end promptly once the client has vanished, even with \
         replies still queued for it"
    );

    stop_server(&stop, handle);
}

/// A client that keeps sending requests while never reading any reply
/// (TASK-1.45 §3) eventually fills `Outbound`'s own control queue; past
/// that point the connection is closed, not hung and not grown without
/// bound. `session.list` is used throughout: cheap, and its reply is
/// small and constant-sized, so this is really about the queue filling,
/// not about the size of any one reply.
#[test]
fn a_client_that_floods_requests_and_never_reads_any_reply_is_closed_not_hung() {
    let (socket_path, stop, handle) = start_server();
    let (mut stream, mut connection) = connect_and_handshake(&socket_path);

    // A write timeout, not just the read timeout `connect_and_handshake`
    // already set: once the server stops reading (closing its own side
    // of the connection as this test means it to), this loop's own
    // `write_all` calls can block on a full OS send buffer with nothing
    // ever draining it, for as long as the OS lets a write wait - found
    // by this test itself hanging under heavy concurrent build load on
    // this machine, well past the drain loop below (which already had
    // its own deadline), because the loop never even got that far
    // (recorded on the task).
    stream
        .set_write_timeout(Some(Duration::from_secs(2)))
        .unwrap();

    // Comfortably past the control queue's own 256-entry cap - every
    // call is sent, none is ever read back.
    for _ in 0..2000 {
        let Ok((_, bytes)) = connection.call("session.list", Map::new()) else {
            break;
        };
        if stream.write_all(&bytes).is_err() {
            break; // the server already closed its side (or this write
            // timed out) - fine, either way, that's the point: nothing
            // here waits for ever.
        }
    }

    // Not `assert_closes`: this test never reads a single reply, so the
    // OS socket buffer may still hold some of the server's own already
    // -written replies (sent before the queue filled) - a real close is
    // only visible once those are drained, not on the very first read.
    //
    // **One deadline for the whole drain, not just a per-read timeout** -
    // the same bound shape as `connection.rs`'s own handshake deadline,
    // for the same reason: under load, the server's writer thread can
    // deliver its backlog in many small, individually-fast reads (each
    // one well inside a per-read timeout), so bounding only each read
    // would let the *total* wall-clock time this loop runs for grow
    // without bound - found by this test itself hanging for minutes
    // under heavy concurrent build load on this machine, with every
    // other test in this file finishing in under a second, before this
    // fix (recorded on the task).
    let deadline = Instant::now() + Duration::from_secs(10);
    stream
        .set_read_timeout(Some(Duration::from_secs(1)))
        .unwrap();
    let mut buf = [0_u8; 65536];
    let closed = loop {
        if Instant::now() >= deadline {
            break false;
        }
        match stream.read(&mut buf) {
            Ok(0) => break true,
            Ok(_) => continue, // more backlog to drain first
            Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => continue, // re-check the deadline above
            Err(_) => break true, // a reset is also a legitimate close
        }
    };
    assert!(
        closed,
        "the connection must close once the control queue fills, not stay open indefinitely"
    );

    stop_server(&stop, handle);
}

/// No per-frame deadline exists after the handshake (TASK-1.45 §3): a
/// client that trickles a frame forever, well past what would have been
/// a handshake-phase timeout, is never closed for it - the server has no
/// way, and is not meant to have one, to tell this apart from an app
/// window that is simply idle.
#[test]
fn a_client_that_trickles_a_frame_post_handshake_forever_is_never_closed() {
    let (socket_path, stop, handle) = start_server();
    let (mut stream, _connection) = connect_and_handshake(&socket_path);

    // A declared length well inside the accepted range, large enough
    // that the handful of bytes trickled after it never completes a
    // frame on its own.
    let declared_len: u32 = 1_000_000;
    let mut bytes = declared_len.to_be_bytes().to_vec();
    bytes.extend_from_slice(&CONTROL_STREAM.to_be_bytes());
    bytes.push(1); // Control kind
    for byte in &bytes {
        stream.write_all(std::slice::from_ref(byte)).unwrap();
    }

    // Idle well past any handshake-phase timeout this server has ever
    // used in this suite, then prove the connection is still open: a
    // short read times out, rather than observing a close - the one
    // case with no event to wait on instead of a bounded sleep
    // (handshake.rs's own established exception to "no sleeps").
    std::thread::sleep(Duration::from_millis(500));
    stream
        .set_read_timeout(Some(Duration::from_millis(200)))
        .unwrap();
    let mut buf = [0_u8; 16];
    match stream.read(&mut buf) {
        Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {}
        Err(err) => panic!("expected the read to time out (still open), got {err}"),
        Ok(n) => panic!("expected no bytes and no close, got {n} bytes"),
    }

    stop_server(&stop, handle);
}

// --- Thread joining, representative of every path (TASK-1.45 §9) ---------

/// Every path out of `run_post_handshake` runs through the same
/// `ConnectionCleanup` guard (`connection.rs`), so proving the server
/// shuts down promptly - which it cannot do with a writer thread still
/// blocked on its own condvar, or a client still stuck in the tree
/// holding the drain thread's own lock - after a representative hostile
/// case (one that never explicitly names a cleanup step) is proof the
/// mechanism runs on every path, not only the ones with a test of their
/// own above.
#[test]
fn the_server_stops_promptly_after_a_hostile_connection_that_was_never_explicitly_closed() {
    let (socket_path, stop, handle) = start_server();
    let (mut stream, _connection) = connect_and_handshake(&socket_path);

    stream
        .write_all(&encode_frame(&Frame::control(CONTROL_STREAM, b"garbage".to_vec())).unwrap())
        .unwrap();
    // Deliberately not waiting for the close here - `stop()` below must
    // still work even while this connection's own reader thread may
    // still be in the middle of closing itself down.

    let started = Instant::now();
    stop.stop();
    handle.join().expect("the server thread should not panic");
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "run() must return promptly even with a hostile connection mid-close"
    );
}

/// Same claim, for a request still genuinely in flight when `stop()` is
/// called - `Runtime::handle`'s own call completes (or the connection's
/// read blocks forever with nothing left to send), and `stop()` still
/// unblocks and joins everything.
#[test]
fn the_server_stops_promptly_with_a_connection_mid_request() {
    let (socket_path, stop, handle) = start_server();
    let (mut stream, mut connection) = connect_and_handshake(&socket_path);

    let (_, bytes) = connection.call("session.list", Map::new()).unwrap();
    stream.write_all(&bytes).unwrap();
    // Not reading the reply - this connection's reader thread may still
    // be inside `Runtime::handle` or about to enqueue the reply when
    // `stop()` races in.

    let started = Instant::now();
    stop.stop();
    handle.join().expect("the server thread should not panic");
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "run() must return promptly even with a request in flight"
    );
}

/// The plan's own section 9 proof, named explicitly rather than only
/// implied: `listen::run`'s shutdown order (join every connection, stop
/// the `Runtime` - ending every real program - then remove the socket
/// file, then release the arbiter lock) proved end to end, with a real
/// pane's real program, through a real connection and a real `stop()` -
/// not `Runtime::stop()` called directly, which `runtime/lifecycle.rs`'s own
/// `stop_tests` module already proves on its own. The ordering itself is
/// structurally guaranteed by `run`'s own sequential, synchronous body
/// (confirmed by reading it: each step is a plain statement after the
/// last, nothing racing); what this test adds is that the *real stack*
/// (a real `session.new`, a real pty, a real `stop()` through a real
/// connection) really does end a real process, not only that
/// `Runtime::stop()` does in isolation.
#[test]
fn stopping_through_a_real_connection_ends_a_real_programs_process_and_frees_the_socket() {
    let (socket_path, stop, handle) = start_server();
    let (mut stream, mut connection) = connect_and_handshake(&socket_path);

    // A real program that writes its own pid to a file, then sleeps -
    // the same technique `runtime/lifecycle.rs`'s own `stop_tests` module already
    // uses, since no API method reports a pane's pid directly.
    let pid_file = std::env::temp_dir().join(format!(
        "steg-routing-stop-test-{}-{}.pid",
        std::process::id(),
        line!()
    ));
    let _ = std::fs::remove_file(&pid_file);
    let mut params = Map::new();
    params.insert(
        "program".to_string(),
        json!([
            "sh",
            "-c",
            format!("echo $$ > {} ; exec sleep 100", pid_file.display())
        ]),
    );
    let (_, response) = call_and_read_reply(&mut stream, &mut connection, "session.new", params);
    assert!(matches!(response, Response::Ok(_)), "{response:?}");

    let deadline = Instant::now() + Duration::from_secs(10);
    let pid: u32 = loop {
        if let Ok(text) = std::fs::read_to_string(&pid_file)
            && let Ok(pid) = text.trim().parse()
        {
            break pid;
        }
        assert!(
            Instant::now() < deadline,
            "never saw a pid written to {pid_file:?}"
        );
        std::thread::sleep(Duration::from_millis(10));
    };
    let _ = std::fs::remove_file(&pid_file);
    assert!(
        process_exists(pid),
        "the real program must be running before this test stops the server"
    );

    stop.stop();
    handle.join().expect("the server thread should not panic");

    assert!(
        !process_exists(pid),
        "stopping through a real connection must end the real program listen::run's own \
         Runtime owned, the same way Runtime::stop() does on its own"
    );
    assert!(
        !socket_path.exists(),
        "the socket file must be removed once run() has returned"
    );

    // The lock really was released, not merely the fd leaked and
    // eventually reclaimed: a fresh serve() at the same path becomes the
    // server again, which the arbiter would refuse if anything still
    // held it.
    let config = ServerConfig {
        socket_path: Some(socket_path.clone()),
        ..ServerConfig::default()
    };
    let second_stop = Stopper::new();
    let second_handle = {
        let config = config.clone();
        let second_stop = second_stop.clone();
        std::thread::spawn(move || {
            let _ = serve(&config, &second_stop, None);
        })
    };
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if UnixStream::connect(&socket_path).is_ok() {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "a second serve() never started accepting - the first run() may not have released \
             the lock"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
    stop_server(&second_stop, second_handle);
}

/// Whether a process with this id still exists, reaped or not - the same
/// `kill -0` probe `runtime/lifecycle.rs`'s own `stop_tests` module already uses.
fn process_exists(pid: u32) -> bool {
    std::process::Command::new("kill")
        .args(["-0", &pid.to_string()])
        .status()
        .expect("kill(1) is on PATH on every platform this runs on")
        .success()
}

/// `ConnectionEvent::Disconnected` actually arrives for a connection this
/// task's own new routing closes (not just one TASK-1.44's handshake
/// phase already covered) - the one seam a later idle counter
/// (TASK-1.77) reads.
#[test]
fn a_hostile_post_handshake_close_still_reports_disconnected() {
    let socket_path = unique_socket_path();
    let config = ServerConfig {
        socket_path: Some(socket_path.clone()),
        handshake_timeout: Duration::from_secs(5),
        ..ServerConfig::default()
    };
    let stop = Stopper::new();
    let (tx, rx) = mpsc::channel::<ConnectionEvent>();
    let handle = {
        let config = config.clone();
        let stop = stop.clone();
        std::thread::spawn(move || {
            let _ = serve(&config, &stop, Some(tx));
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
    // The readiness probe just above is itself a connection, accepted and
    // then dropped with no hello - it reports its own Connected/
    // Disconnected pair, which must be drained here (same technique
    // `shutdown.rs`'s own `start_truly_idle_server` uses) so this test
    // only ever observes events for the one connection it actually cares
    // about, below.
    assert_eq!(
        rx.recv_timeout(Duration::from_secs(5)).unwrap(),
        ConnectionEvent::Connected
    );
    assert_eq!(
        rx.recv_timeout(Duration::from_secs(5)).unwrap(),
        ConnectionEvent::Disconnected
    );

    let (mut stream, _connection) = connect_and_handshake(&socket_path);
    assert_eq!(
        rx.recv_timeout(Duration::from_secs(5)).unwrap(),
        ConnectionEvent::Connected
    );

    stream
        .write_all(&encode_frame(&Frame::control(CONTROL_STREAM, b"garbage".to_vec())).unwrap())
        .unwrap();
    assert_eq!(
        rx.recv_timeout(Duration::from_secs(5)).unwrap(),
        ConnectionEvent::Disconnected,
        "the hostile close must still be reported, the same seam TASK-1.77's idle counter reads"
    );

    stop_server(&stop, handle);
}

/// Decoding a frame off a real byte stream fed one byte at a time is
/// already the sdk's own proof (`crates/sdk/src/wire/frame.rs`'s own
/// tests); this is its one companion over a real post-handshake
/// connection: a `Decoder`-equivalent read loop on the client's own side
/// still sees exactly the replies the server sent, confirming nothing in
/// this task's own routing depends on a frame arriving in one `read()`
/// call.
#[test]
fn a_reply_delivered_one_byte_at_a_time_still_decodes_correctly() {
    let (socket_path, stop, handle) = start_server();
    let mut stream = UnixStream::connect(&socket_path).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let hello_bytes = Connection::send_hello("routing-test-client", ClientKind::Cli, None);
    stream.write_all(&hello_bytes).unwrap();

    let mut decoder = Decoder::new();
    let hello_reply = loop {
        if let Some(frame) = decoder.next_frame().unwrap() {
            break frame.payload;
        }
        let mut buf = [0_u8; 1];
        let n = stream.read(&mut buf).unwrap();
        assert!(n > 0);
        decoder.push(&buf[..n]);
    };
    parse_server_hello(&hello_reply).unwrap();

    let mut connection = Connection::new(Role::Client, description());
    let (id, bytes) = connection.call("session.list", Map::new()).unwrap();
    stream.write_all(&bytes).unwrap();

    let reply = loop {
        if let Some(frame) = decoder.next_frame().unwrap() {
            break frame.payload;
        }
        let mut buf = [0_u8; 1];
        let n = stream.read(&mut buf).unwrap();
        assert!(n > 0);
        decoder.push(&buf[..n]);
    };
    let Control::Reply {
        id: reply_id,
        response,
    } = Control::from_payload(&reply).unwrap()
    else {
        panic!("expected a reply");
    };
    assert_eq!(reply_id, id);
    assert!(matches!(response, Response::Ok(_)));

    stop_server(&stop, handle);
}

/// `stream`'s own kernel-reported buffer size (`SO_SNDBUF`/`SO_RCVBUF`,
/// `which`) - read from the real socket rather than assumed, so a test
/// that needs to exceed it can size itself from the actual number on
/// whatever machine runs it, not a guessed constant.
///
/// # Panics
/// If `getsockopt` itself fails - never in practice for these two,
/// always-supported options on an open, connected socket.
fn socket_buffer_size(stream: &UnixStream, which: libc::c_int) -> usize {
    let mut value: libc::c_int = 0;
    let mut len = libc::socklen_t::try_from(size_of::<libc::c_int>()).expect("a small constant");
    // SAFETY: `stream.as_raw_fd()` names a real, open socket for the
    // duration of this call (`stream` is only borrowed, not consumed);
    // `value` and `len` are a correctly sized, initialised `c_int` and
    // its own byte length, which is exactly the shape SO_SNDBUF/
    // SO_RCVBUF (always a plain `c_int` option on every platform this
    // runs on) write back through.
    let result = unsafe {
        libc::getsockopt(
            stream.as_raw_fd(),
            libc::SOL_SOCKET,
            which,
            std::ptr::addr_of_mut!(value).cast(),
            &mut len,
        )
    };
    assert_eq!(
        result,
        0,
        "getsockopt failed: {}",
        std::io::Error::last_os_error()
    );
    usize::try_from(value).unwrap_or(0)
}

/// Bytes sitting in `stream`'s own receive buffer right now, not yet
/// read (`FIONREAD`) - queried without consuming them, so a test can
/// watch whether more keep arriving without draining what is already
/// there.
///
/// # Panics
/// If the `ioctl` itself fails - never in practice for `FIONREAD` on an
/// open socket.
fn bytes_available_to_read(stream: &UnixStream) -> usize {
    let mut available: libc::c_int = 0;
    // SAFETY: `stream.as_raw_fd()` names a real, open socket for the
    // duration of this call; `FIONREAD` writes exactly one `c_int`
    // through the pointer given, which `available` is.
    let result = unsafe { libc::ioctl(stream.as_raw_fd(), libc::FIONREAD, &mut available) };
    assert_eq!(
        result,
        0,
        "ioctl(FIONREAD) failed: {}",
        std::io::Error::last_os_error()
    );
    usize::try_from(available).unwrap_or(0)
}

/// Waits, bounded, until `stream`'s own [`bytes_available_to_read`] is
/// positive and reads the same on two consecutive polls, 20ms apart -
/// the one fact that can only hold once whatever is writing to the other
/// end has written something and then genuinely stopped (blocked on a
/// full buffer, or finished), never a fixed sleep standing in for
/// "probably by now". No OS primitive blocks on an exact byte count
/// staying put, so this is a bounded poll, the same shape this crate's
/// own `runtime/start.rs` tests already use for a fact with no event of its
/// own (e.g. waiting for a real OSC 7 to land) - the sleep between polls
/// is pacing, not the wait itself; the wait is the condition checked
/// before it.
///
/// # Panics
/// If the bound passes before two consecutive, positive, equal readings
/// are seen.
fn wait_until_bytes_available_plateaus(stream: &UnixStream, bound: Duration) -> usize {
    let deadline = Instant::now() + bound;
    let mut last = bytes_available_to_read(stream);
    loop {
        assert!(
            Instant::now() < deadline,
            "bytes available to read never plateaued above zero within {bound:?} \
             (last seen: {last})"
        );
        std::thread::sleep(Duration::from_millis(20));
        let now = bytes_available_to_read(stream);
        if now > 0 && now == last {
            return now;
        }
        last = now;
    }
}

/// The headline bug this task found and fixed (`connection.rs`'s
/// `ConnectionCleanup`): a writer thread blocked inside `write_all` on a
/// full OS send buffer never sees `Outbound::close()`'s own condvar
/// notify - only `shutdown(Both)` forces a blocked write to fail.
///
/// Makes the writer genuinely block, not merely slow, and *proves* it
/// (a review of this task found an earlier version of this test only
/// slept 300ms and assumed it): reads this connection's own real
/// `SO_SNDBUF`/`SO_RCVBUF` from the kernel, sends enough `api.list`
/// calls (each large, ~30KB, measured directly) that their total
/// comfortably exceeds both buffers' sum with a wide margin, then calls
/// [`wait_until_bytes_available_plateaus`] (bounded) for the bytes
/// already delivered to stop growing at a value above zero, which can
/// only happen once the writer has written something and then stopped
/// (blocked, since nothing on this side has read anything yet). This
/// connection's own close is then triggered from the inside (a hostile
/// frame), not a server-wide `stop()` - `listen.rs`'s own, separate
/// shutdown mechanism would also unblock a stuck writer and could mask
/// this exact bug, so this test must never call `stop()` before the
/// assertion below.
#[test]
fn a_writer_blocked_on_a_full_send_buffer_still_joins_when_this_connections_own_close_runs() {
    let socket_path = unique_socket_path();
    let config = ServerConfig {
        socket_path: Some(socket_path.clone()),
        handshake_timeout: Duration::from_secs(5),
        ..ServerConfig::default()
    };
    let stop = Stopper::new();
    let (tx, rx) = mpsc::channel::<ConnectionEvent>();
    let handle = {
        let config = config.clone();
        let stop = stop.clone();
        std::thread::spawn(move || {
            let _ = serve(&config, &stop, Some(tx));
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
    // Drains the readiness probe's own Connected/Disconnected pair (same
    // technique as a_hostile_post_handshake_close_still_reports_disconnected).
    assert_eq!(
        rx.recv_timeout(Duration::from_secs(5)).unwrap(),
        ConnectionEvent::Connected
    );
    assert_eq!(
        rx.recv_timeout(Duration::from_secs(5)).unwrap(),
        ConnectionEvent::Disconnected
    );

    let (mut stream, mut connection) = connect_and_handshake(&socket_path);
    assert_eq!(
        rx.recv_timeout(Duration::from_secs(5)).unwrap(),
        ConnectionEvent::Connected
    );

    // Sized from the real kernel, not assumed: this connection's own
    // SO_SNDBUF and SO_RCVBUF, read directly, are what actually bounds
    // how much the server can have in flight before its own write
    // blocks - a fixed guess would be wrong on a machine with unusually
    // large buffers.
    let send_buf = socket_buffer_size(&stream, libc::SO_SNDBUF);
    let recv_buf = socket_buffer_size(&stream, libc::SO_RCVBUF);
    let margin_target = (send_buf + recv_buf) * 3;

    // `api.list`'s own *reply* (not the tiny request this connection
    // sends to ask for it) is what has to exceed `margin_target` - a
    // measured, documented floor, not read live each time: the reply's
    // size is this codebase's own API description, deterministic and
    // under this repository's own control, not the OS's. Measured
    // directly (one run, `cargo test -- --nocapture` on this exact
    // call): 30181 bytes; 25000 is a conservative floor under that.
    const REPLY_BYTES_FLOOR: usize = 25_000;
    let calls_needed = (margin_target / REPLY_BYTES_FLOOR) + 2;
    let mut sent_calls = 0_usize;
    let mut sent_ids = Vec::new();
    while sent_calls < calls_needed {
        let (id, bytes) = connection.call("api.list", Map::new()).unwrap();
        stream.write_all(&bytes).unwrap();
        sent_ids.push(id);
        sent_calls += 1;
    }

    // Waits for the one fact that can only hold once the writer has
    // genuinely written something and then stopped - no sleep standing
    // in for "probably by now" (a review of this task found the earlier
    // version of this test did exactly that).
    let plateaued_at = wait_until_bytes_available_plateaus(&stream, Duration::from_secs(5));

    stream
        .write_all(&encode_frame(&Frame::control(CONTROL_STREAM, b"garbage".to_vec())).unwrap())
        .unwrap();

    let disconnected = rx.recv_timeout(Duration::from_secs(5));
    assert_eq!(
        disconnected,
        Ok(ConnectionEvent::Disconnected),
        "the connection must still be cleaned up (and reported) within a bound, even with its \
         writer thread genuinely blocked inside write_all on a full send buffer when this \
         connection's own close runs"
    );

    // Confirms what the plateau already established, now from the
    // client's own total: some bytes genuinely arrived (the writer did
    // write), and not all of them did (the writer really was cut off
    // mid-delivery, not merely unobserved) - drains whatever is left and
    // adds it to what the plateau check already read off the socket
    // without consuming it.
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    let mut received = 0_usize;
    let mut buf = [0_u8; 65536];
    loop {
        match stream.read(&mut buf) {
            Ok(0) | Err(_) => break,
            Ok(n) => received += n,
        }
    }
    let total_sent: usize = sent_calls * REPLY_BYTES_FLOOR;
    assert!(
        plateaued_at > 0,
        "the plateau wait already requires this, restated here for a single clear assertion \
         alongside the one below"
    );
    assert!(
        received > 0,
        "the writer must have written something before this connection's own close ran"
    );
    assert!(
        received < total_sent,
        "the writer must have been cut off mid-delivery (received {received} bytes of at least \
         {total_sent} sent, across {sent_calls} calls); if every reply fully arrived, this test \
         never actually exercised a blocked write"
    );
    let _ = sent_ids; // kept only for a human reading a failure, not asserted on

    stop_server(&stop, handle);
}
