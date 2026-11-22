//! `pane.wait` over a real socket (TASK-1.140): blocks until a pane's
//! program exits, with its exit code, or its own `--timeout` passes -
//! `--timeout 0` means "check now and answer at once," a negative one is
//! refused as bad usage - proven the same way `attach.rs`/`pane_send.rs`/
//! `pane_capture.rs` already prove the rest of the API, with real pty
//! programs and a real server. The pending-wait mechanism itself
//! (`WaitGate`, `resolve_pane_waits_locked`, `timeout_wait_locked`) is
//! exercised here only through the wire - there is no worker-thread
//! command of its own to test closer to the metal, unlike capture's own
//! split (this task's own plan: "nothing here ever touches a pane's own
//! worker thread").

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
    ClientKind, Connection, ErrorCode, Incoming, Response, WaitResult, WireRole as Role,
    description, parse_server_hello,
};
use steg_server::{LoginShell, ServerConfig, Stopper, serve};

#[path = "support/mod.rs"]
mod support;
use support::{ScopedTempDir, run_bounded, scoped_socket_path};

/// This file's own connections' default read timeout - the same bound
/// `attach.rs`/`pane_send.rs`/`pane_capture.rs` already settled on
/// (TASK-1.123).
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
    stream.set_read_timeout(Some(DEFAULT_READ_TIMEOUT)).unwrap();
    let hello_bytes = Connection::send_hello("wait-test-client", ClientKind::Cli, None);
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

/// Sends `method`/`params`, without reading anything back - needed
/// whenever a later assertion wants more than one request in flight at
/// once (AC #2's own proof, and reading a pending wait's reply only once
/// its own event fires).
fn call_without_reading(
    stream: &mut UnixStream,
    connection: &mut Connection,
    method: &str,
    params: Map<String, Value>,
) -> u64 {
    let (id, bytes) = connection.call(method, params).expect("a valid call");
    stream.write_all(&bytes).unwrap();
    id
}

/// Reads frames off `stream` until the reply for `id` arrives, bounded -
/// any other reply that happens to arrive first is skipped, not treated
/// as an error (every call site that cares about ordering reads its own
/// replies without this skip instead - see
/// `an_unrelated_request_behind_a_pending_wait_answers_first_proving_the_
/// reader_was_never_blocked`, below).
fn read_reply_for(
    stream: &mut UnixStream,
    connection: &mut Connection,
    id: u64,
    timeout: Duration,
) -> Response {
    let deadline = Instant::now() + timeout;
    loop {
        match connection.poll() {
            Some(Incoming::Reply {
                id: reply_id,
                response,
            }) if reply_id == id => return response,
            Some(_) => continue,
            None => {}
        }
        assert!(
            Instant::now() < deadline,
            "timed out waiting for a reply to id {id}"
        );
        let mut buf = [0_u8; 65536];
        let n = stream.read(&mut buf).expect("reading the reply");
        assert!(n > 0, "server closed before replying");
        connection.receive(&buf[..n]);
    }
}

fn call_and_read_reply(
    stream: &mut UnixStream,
    connection: &mut Connection,
    method: &str,
    params: Map<String, Value>,
) -> Response {
    let id = call_without_reading(stream, connection, method, params);
    read_reply_for(stream, connection, id, Duration::from_secs(20))
}

/// Starts a real session whose one pane runs `program`, over the real
/// socket, and returns that pane's own full `session/tab/pane` path - the
/// same helper `attach.rs`/`pane_send.rs`/`pane_capture.rs` each already
/// have their own copy of.
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

/// Polls `pane.show` (bounded, never a bare sleep-and-hope) until
/// `pane`'s own `program_status` reads `expected` - the same reasoning
/// every other real-process wait in this crate's own tests already gives:
/// there is no event at this level to wait on instead.
fn wait_for_program_status(
    stream: &mut UnixStream,
    connection: &mut Connection,
    pane: &str,
    expected: &str,
) {
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        let mut params = Map::new();
        params.insert("target".to_string(), json!(pane));
        let response = call_and_read_reply(stream, connection, "pane.show", params);
        let Response::Ok(value) = response else {
            panic!("pane.show failed: {response:?}")
        };
        if value.get("program_status").and_then(Value::as_str) == Some(expected) {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "pane {pane} never reached program_status {expected:?}"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn wait_params(target: &str, timeout_secs: Option<i64>) -> Map<String, Value> {
    let mut params = Map::new();
    params.insert("target".to_string(), json!(target));
    params.insert("for".to_string(), json!("exit"));
    if let Some(secs) = timeout_secs {
        params.insert("timeout".to_string(), json!(secs));
    }
    params
}

// --- AC #1/#4: the real exit code, an already-ended pane, --timeout ----

/// doc-2 section 6: "`pane wait --for exit` exits 0 when the program
/// ended and prints the program's own exit code." No registration at all
/// here, since the program has already ended by the time `pane.wait`
/// runs - the engineer's own answer that `--timeout` plays no part in
/// this case either way is proven separately, below.
#[test]
fn wait_on_an_already_ended_pane_with_no_timeout_answers_at_once_with_the_real_exit_code() {
    let (_socket_dir, socket_path, stop, handle) = start_server();
    let (mut stream, mut connection) = connect_and_handshake(&socket_path);
    let pane = new_pane_running(&mut stream, &mut connection, vec!["sh", "-c", "exit 7"]);
    wait_for_program_status(&mut stream, &mut connection, &pane, "ended");

    let response = call_and_read_reply(
        &mut stream,
        &mut connection,
        "pane.wait",
        wait_params(&pane, None),
    );
    let Response::Ok(value) = response else {
        panic!("expected Ok, got {response:?}")
    };
    // The typed record (the engineer's own answer), not a bare value read
    // by key - proves the server actually serialises `WaitResult`.
    let result: WaitResult =
        serde_json::from_value(value).expect("pane.wait's reply is a WaitResult");
    assert_eq!(result.exit_code, 7);

    stop_server(&stop, handle);
}

/// AC #1's first clause: a wait on a running pane answers once the real
/// program exits, with its real code - the exit drain
/// (`resolve_pane_waits_locked`, called from `apply_event_locked`'s own
/// `Exited` arm) is what actually answers this, not the immediate
/// already-ended branch `register_wait` takes above.
///
/// Negative control, run by hand against this exact test (not committed
/// as a toggle): with `super::wait::resolve_pane_waits_locked(state,
/// pane, Ok(exit_code))` removed from `apply_event_locked`'s own `Exited`
/// arm, nothing ever answers this wait - the connection's own socket read
/// times out (`DEFAULT_READ_TIMEOUT`) rather than a reply ever arriving -
/// recorded failing then passing.
#[test]
fn wait_on_a_running_pane_answers_once_the_program_really_exits_with_its_real_code() {
    let (_socket_dir, socket_path, stop, handle) = start_server();
    let (mut stream, mut connection) = connect_and_handshake(&socket_path);
    let pane = new_pane_running(
        &mut stream,
        &mut connection,
        vec!["sh", "-c", "sleep 0.3; exit 3"],
    );

    let response = call_and_read_reply(
        &mut stream,
        &mut connection,
        "pane.wait",
        wait_params(&pane, None),
    );
    let Response::Ok(value) = response else {
        panic!("expected Ok, got {response:?}")
    };
    let result: WaitResult = serde_json::from_value(value).unwrap();
    assert_eq!(result.exit_code, 3);

    stop_server(&stop, handle);
}

/// AC #1's second clause, and the engineer's own answer: a positive
/// `--timeout` against a program that outlives it answers `TimedOut`
/// within the bound, and the message names the seconds that expired.
///
/// Negative control, run by hand against this exact test: with
/// `timed_out`'s own `format!` reverted to a plain, non-interpolated
/// string, the `message.contains` assertion below fails - recorded
/// failing then passing.
#[test]
fn wait_with_a_positive_timeout_on_an_outliving_program_answers_timed_out_naming_the_seconds() {
    let (_socket_dir, socket_path, stop, handle) = start_server();
    let (mut stream, mut connection) = connect_and_handshake(&socket_path);
    let pane = new_pane_running(&mut stream, &mut connection, vec!["sh", "-c", "sleep 100"]);

    let start = Instant::now();
    let response = call_and_read_reply(
        &mut stream,
        &mut connection,
        "pane.wait",
        wait_params(&pane, Some(1)),
    );
    let elapsed = start.elapsed();
    assert!(
        elapsed < Duration::from_secs(10),
        "a 1-second timeout must not take anywhere near this long: {elapsed:?}"
    );

    let Response::Error(error) = response else {
        panic!("expected TimedOut, got {response:?}")
    };
    assert_eq!(error.code, ErrorCode::TimedOut);
    assert!(
        error.message.contains('1'),
        "the message should name the expired timeout (1 second): {:?}",
        error.message
    );

    stop_server(&stop, handle);
}

// --- the engineer's own answers: --timeout 0 and a negative --timeout --

/// `--timeout 0` on an already-ended pane: "check now" and the pane has
/// already ended, so this is `Ok`, exactly as with no timeout given -
/// proving the zero branch does not disturb the ordinary already-ended
/// path (`register_wait`'s own step 5 runs before step 6 is ever
/// reached).
#[test]
fn timeout_zero_on_an_already_ended_pane_answers_ok_with_the_real_exit_code() {
    let (_socket_dir, socket_path, stop, handle) = start_server();
    let (mut stream, mut connection) = connect_and_handshake(&socket_path);
    let pane = new_pane_running(&mut stream, &mut connection, vec!["sh", "-c", "exit 5"]);
    wait_for_program_status(&mut stream, &mut connection, &pane, "ended");

    let response = call_and_read_reply(
        &mut stream,
        &mut connection,
        "pane.wait",
        wait_params(&pane, Some(0)),
    );
    let Response::Ok(value) = response else {
        panic!("expected Ok, got {response:?}")
    };
    let result: WaitResult = serde_json::from_value(value).unwrap();
    assert_eq!(result.exit_code, 5);

    stop_server(&stop, handle);
}

/// `--timeout 0` on a running pane: "check now and answer at once" - no
/// registration, no thread, answered well under any real wait.
///
/// Negative control, run by hand against this exact test: with
/// `register_wait`'s own `Some(0)` immediate-reply branch removed
/// (falling through to the "register indefinitely" arm instead), this
/// case never answers until the test's own cleanup ends the program -
/// recorded failing (hangs past a short bound) then passing.
#[test]
fn timeout_zero_on_a_running_pane_answers_timed_out_at_once() {
    let (_socket_dir, socket_path, stop, handle) = start_server();
    let (mut stream, mut connection) = connect_and_handshake(&socket_path);
    let pane = new_pane_running(&mut stream, &mut connection, vec!["sh", "-c", "sleep 100"]);

    let start = Instant::now();
    let response = call_and_read_reply(
        &mut stream,
        &mut connection,
        "pane.wait",
        wait_params(&pane, Some(0)),
    );
    let elapsed = start.elapsed();
    assert!(
        elapsed < Duration::from_millis(500),
        "a --timeout 0 check must answer at once, not register a wait: {elapsed:?}"
    );

    let Response::Error(error) = response else {
        panic!("expected TimedOut, got {response:?}")
    };
    assert_eq!(error.code, ErrorCode::TimedOut);

    stop_server(&stop, handle);
}

/// A negative `--timeout` is bad usage, refused before the target is
/// resolved or anything is registered - proven here by then sending an
/// ordinary wait (no timeout) against the very same, still-running pane
/// and watching it work correctly once the program really exits: nothing
/// was left half-registered by the refusal.
///
/// Negative control, run by hand against this exact test: with the
/// negative-timeout check removed from `register_wait`, `-1` falls
/// through to the positive-timeout branch, and `secs as u64` wraps it to
/// a huge duration instead of refusing the call - the first `pane.wait`
/// is silently accepted and, since this pane's own program exits well
/// before that duration ever could, answers `Ok` with the real exit code
/// instead of `BadRequest` - recorded failing (a request that should have
/// been refused instead succeeded) then passing.
#[test]
fn a_negative_timeout_on_a_running_pane_answers_bad_request_and_leaves_the_pane_untouched() {
    let (_socket_dir, socket_path, stop, handle) = start_server();
    let (mut stream, mut connection) = connect_and_handshake(&socket_path);
    let pane = new_pane_running(
        &mut stream,
        &mut connection,
        vec!["sh", "-c", "sleep 0.3; exit 2"],
    );

    let response = call_and_read_reply(
        &mut stream,
        &mut connection,
        "pane.wait",
        wait_params(&pane, Some(-1)),
    );
    let Response::Error(error) = response else {
        panic!("expected BadRequest, got {response:?}")
    };
    assert_eq!(error.code, ErrorCode::BadRequest);

    let response = call_and_read_reply(
        &mut stream,
        &mut connection,
        "pane.wait",
        wait_params(&pane, None),
    );
    let Response::Ok(value) = response else {
        panic!("expected Ok, got {response:?}")
    };
    let result: WaitResult = serde_json::from_value(value).unwrap();
    assert_eq!(result.exit_code, 2);

    stop_server(&stop, handle);
}

// --- AC #1's third clause: a pane closed while being waited on ---------

/// A pane force-closed while a wait is pending on it answers
/// `NoSuchTarget` at once - bounded well under `force_end`'s own
/// production grace (`FORCE_GRACE_DEFAULT`, 2 real seconds), proving the
/// close-path resolution (`close_and_dispatch`'s own drain) fires
/// immediately under the lock, not only once the program is actually
/// made to die.
///
/// Negative control, run by hand against this exact test: with the
/// `resolve_pane_waits_locked` call removed from `close_and_dispatch`,
/// this wait is never answered at all, not merely answered late - the
/// close has already removed `pane` from the tree by the time the
/// forced `SIGKILL` eventually ends the real `sleep 100`, so the real
/// exit that follows hits `apply_event_locked`'s own `Err(PaneNotFound)`
/// arm, which never touches `pending_waits` (that arm's own job is only
/// ever a stale-handle cleanup, documented there) - this test then hangs
/// until the read itself times out, instead of answering `NoSuchTarget`
/// well under a second - recorded failing then passing.
#[test]
fn closing_the_pane_while_a_wait_is_pending_answers_no_such_target_bounded_under_the_close_grace() {
    let (_socket_dir, socket_path, stop, handle) = start_server();
    let (mut wait_stream, mut wait_connection) = connect_and_handshake(&socket_path);
    let pane = new_pane_running(
        &mut wait_stream,
        &mut wait_connection,
        vec!["sh", "-c", "sleep 100"],
    );

    let wait_id = call_without_reading(
        &mut wait_stream,
        &mut wait_connection,
        "pane.wait",
        wait_params(&pane, None),
    );

    let (mut close_stream, mut close_connection) = connect_and_handshake(&socket_path);
    let mut close_params = Map::new();
    close_params.insert("target".to_string(), json!(pane));
    close_params.insert("force".to_string(), json!(true));
    let close_response = call_and_read_reply(
        &mut close_stream,
        &mut close_connection,
        "pane.close",
        close_params,
    );
    assert!(
        matches!(close_response, Response::Ok(_)),
        "{close_response:?}"
    );

    let start = Instant::now();
    let response = read_reply_for(
        &mut wait_stream,
        &mut wait_connection,
        wait_id,
        Duration::from_secs(5),
    );
    let elapsed = start.elapsed();
    assert!(
        elapsed < Duration::from_secs(1),
        "the close-path resolution must answer at once, not after force_end's own grace: \
         {elapsed:?}"
    );

    let Response::Error(error) = response else {
        panic!("expected NoSuchTarget, got {response:?}")
    };
    assert_eq!(error.code, ErrorCode::NoSuchTarget);

    stop_server(&stop, handle);
}

// --- AC #2: a waiting request holds neither the lock nor the reader ----

/// A `pane.wait` with no timeout against a running pane, then an
/// unrelated `pane.show` sent right behind it on the same connection,
/// without reading the wait's own reply first: the unrelated reply must
/// arrive while the wait is still outstanding, proving `register_wait`
/// released `Runtime`'s lock and this connection's own reader never
/// blocked on the pending wait.
///
/// Negative control, run by hand against this exact test (not committed
/// as a toggle): a scratch `std::thread::sleep(Duration::from_secs(15))`
/// added inside `register_wait`, right after it takes `Runtime`'s own
/// lock (before it is ever dropped), stalls `pane.show`'s own reply for
/// that whole sleep too, since it needs the same lock - past this
/// connection's own socket read timeout in practice, so the read itself
/// fails rather than `pane.show`'s own reply ever arriving - recorded
/// failing then passing; reverted before this test was committed.
#[test]
fn an_unrelated_request_behind_a_pending_wait_answers_first_proving_the_reader_was_never_blocked() {
    let (_socket_dir, socket_path, stop, handle) = start_server();
    let (mut stream, mut connection) = connect_and_handshake(&socket_path);
    let pane = new_pane_running(&mut stream, &mut connection, vec!["sh", "-c", "sleep 100"]);

    let wait_id = call_without_reading(
        &mut stream,
        &mut connection,
        "pane.wait",
        wait_params(&pane, None),
    );
    let mut show_params = Map::new();
    show_params.insert("target".to_string(), json!(pane));
    let show_id = call_without_reading(&mut stream, &mut connection, "pane.show", show_params);

    let deadline = Instant::now() + Duration::from_secs(10);
    let mut wait_answered_first = false;
    let show_response = loop {
        match connection.poll() {
            Some(Incoming::Reply { id, response }) if id == show_id => break response,
            Some(Incoming::Reply { id, .. }) if id == wait_id => {
                wait_answered_first = true;
            }
            Some(_) => continue,
            None => {
                assert!(
                    Instant::now() < deadline,
                    "timed out waiting for pane.show's own reply"
                );
                let mut buf = [0_u8; 65536];
                let n = stream.read(&mut buf).expect("reading a reply");
                assert!(n > 0, "server closed before replying");
                connection.receive(&buf[..n]);
            }
        }
    };
    assert!(
        !wait_answered_first,
        "the pending pane.wait must not answer before the unrelated pane.show sent right \
         behind it"
    );
    assert!(
        matches!(show_response, Response::Ok(_)),
        "{show_response:?}"
    );

    stop_server(&stop, handle);
}
