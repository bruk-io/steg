//! `server.stop` (TASK-1.88, AC #2/#3) over a real socket, a real
//! `listen::run` on a background thread, real `UnixStream` clients - no
//! process spawn needed at this layer (`idle_stop.rs`'s own shape).
//!
//! What this file does *not* re-prove, because it already exists
//! elsewhere: `force_end`'s own escalation of a program that traps the
//! polite signal (`crates/server/src/runtime/lifecycle.rs`'s own
//! `stop_tests` module) and the full stop-removes-the-socket-releases-
//! the-lock sequence triggered by a direct `Stopper::stop()` call
//! (`routing.rs`'s own
//! `stopping_through_a_real_connection_ends_a_real_programs_process_and_
//! frees_the_socket`). What is new here, and what this file is about:
//! `server.stop`'s own refusal check, over the real wire; that the reply
//! is read back whole before anything closes (the flush-then-stop
//! ordering this task adds); and that triggering stop through the real
//! `server.stop` request - not a test harness calling `Stopper::stop()`
//! directly - reaches that same, already-proven mechanism.

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
    ClientKind, Connection, ErrorCode, Incoming, Response, WireRole as Role, description,
    parse_server_hello,
};
use steg_server::{LoginShell, ServerConfig, Stopper, serve};

#[path = "support/mod.rs"]
mod support;
use support::{ScopedTempDir, run_bounded, scoped_socket_path};

/// A working `PATH` is required (TASK-1.87): the one test here that
/// starts a real, long-lived program needs `sh` resolvable - the same
/// reason `routing.rs`'s own `test_login` reads this.
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

/// Same shape as `routing.rs`'s own `start_server` - a real `listen::run`
/// on a background thread, over a fresh, per-test socket path
/// (`ScopedTempDir`, TASK-1.123).
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

/// Real handshake over a real socket, with the sdk's own `Connection` -
/// `routing.rs`'s own `connect_and_handshake`, duplicated here (this
/// crate's own convention: no shared module beyond `tests/support/
/// mod.rs`'s bounded-wait/socket-path helpers).
fn connect_and_handshake(socket_path: &std::path::Path) -> (UnixStream, Connection) {
    let mut stream = UnixStream::connect(socket_path).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let hello_bytes = Connection::send_hello("server-stop-test-client", ClientKind::Cli, None);
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

fn stop_params(force: bool) -> Map<String, Value> {
    let mut params = Map::new();
    if force {
        params.insert("force".to_string(), json!(true));
    }
    params
}

/// Whether a process with this id still exists, reaped or not - the same
/// `kill -0` probe `routing.rs`'s own `process_exists` already uses.
fn process_exists(pid: u32) -> bool {
    std::process::Command::new("kill")
        .args(["-0", &pid.to_string()])
        .status()
        .expect("kill(1) is on PATH on every platform this runs on")
        .success()
}

/// Bounded join - a failure here panics naming the bound, rather than
/// hanging the suite (`idle_stop.rs`'s own `join_within`, duplicated).
fn join_within(server_thread: std::thread::JoinHandle<()>, bound: Duration) {
    let (done_tx, done_rx) = std::sync::mpsc::channel::<()>();
    let waiter = std::thread::spawn(move || {
        server_thread
            .join()
            .expect("the server thread should not panic");
        let _ = done_tx.send(());
    });
    done_rx
        .recv_timeout(bound)
        .unwrap_or_else(|_| panic!("run should have returned within {bound:?} of being stopped"));
    waiter.join().expect("the waiter thread should not panic");
}

/// AC #2's own refusal: a real session exists, no `--force` -
/// `ProgramRunning`, and the server is still up afterward (a second,
/// ordinary request over the same connection succeeds).
#[test]
fn server_stop_without_force_while_a_session_exists_refuses_and_the_server_keeps_running() {
    let (_socket_dir, socket_path, stop, handle) = start_server();
    let (mut stream, mut connection) = connect_and_handshake(&socket_path);

    let mut new_params = Map::new();
    new_params.insert("program".to_string(), json!(["sh", "-c", "sleep 100"]));
    let response = call_and_read_reply(&mut stream, &mut connection, "session.new", new_params);
    assert!(
        matches!(response, Response::Ok(_)),
        "session.new failed: {response:?}"
    );

    let response = call_and_read_reply(
        &mut stream,
        &mut connection,
        "server.stop",
        stop_params(false),
    );
    let Response::Error(error) = response else {
        panic!("expected a refusal while a session exists, got {response:?}");
    };
    assert_eq!(error.code, ErrorCode::ProgramRunning);

    // The server is still up: an ordinary request over the same
    // connection still succeeds.
    let response = call_and_read_reply(&mut stream, &mut connection, "server.show", Map::new());
    assert!(
        matches!(response, Response::Ok(_)),
        "the server must still answer after a refused stop: {response:?}"
    );

    stop.stop();
    run_bounded(
        "the server thread to end after stop()",
        Duration::from_secs(5),
        move || {
            handle.join().expect("the server thread should not panic");
        },
    );
}

/// AC #2 in full, with no session at all (so no `--force` is even
/// needed): the caller's own connection reads the complete, correct
/// `Ok(null)` reply - decoded whole off the wire, which `call_and_read_
/// reply`'s own framing already makes byte-exact, not merely "eventually
/// got *something*" - and only then does the server actually stop:
/// `run()`'s own background thread returns within a bound, the socket
/// file is gone, and a second client's `connect()` finds no server.
#[test]
fn server_stop_with_no_sessions_flushes_the_reply_then_stops_and_a_second_client_sees_no_server() {
    let (_socket_dir, socket_path, _stop, handle) = start_server();
    let (mut stream, mut connection) = connect_and_handshake(&socket_path);

    let response = call_and_read_reply(
        &mut stream,
        &mut connection,
        "server.stop",
        stop_params(false),
    );
    match response {
        Response::Ok(value) => assert_eq!(value, Value::Null),
        Response::Error(error) => panic!("expected Ok(null), got {error:?}"),
    }

    join_within(handle, Duration::from_secs(10));

    assert!(
        !socket_path.exists(),
        "the socket file must be removed once run() has returned"
    );
    assert!(
        UnixStream::connect(&socket_path).is_err(),
        "a second client must find no server at the now-removed socket path"
    );

    // The lock really was released, not merely leaked: a fresh serve()
    // at the same path becomes the server again (the arbiter would
    // refuse otherwise) - `routing.rs`'s own equivalent check,
    // duplicated here since this is the one place `server.stop` itself
    // (not a direct `Stopper::stop()` call) is what released it.
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
            "a second serve() never started accepting - the first stop may not have released \
             the lock"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
    second_stop.stop();
    run_bounded(
        "the second server thread to end after stop()",
        Duration::from_secs(5),
        move || {
            second_handle
                .join()
                .expect("the second server thread should not panic");
        },
    );
}

/// `--force` with a real session: the program actually running is
/// actually ended, through `server.stop`'s own real trigger - not
/// `Runtime::stop()` called directly (`lifecycle.rs`'s own proof) nor a
/// test harness's direct `Stopper::stop()` call (`routing.rs`'s own
/// proof), but the API method itself.
#[test]
fn server_stop_with_force_ends_a_real_running_programs_process() {
    let (_socket_dir, socket_path, _stop, handle) = start_server();
    let (mut stream, mut connection) = connect_and_handshake(&socket_path);

    let pid_file = std::env::temp_dir().join(format!(
        "steg-server-stop-test-{}-{}.pid",
        std::process::id(),
        line!()
    ));
    let _ = std::fs::remove_file(&pid_file);
    let mut new_params = Map::new();
    new_params.insert(
        "program".to_string(),
        json!([
            "sh",
            "-c",
            format!("echo $$ > {} ; exec sleep 100", pid_file.display())
        ]),
    );
    let response = call_and_read_reply(&mut stream, &mut connection, "session.new", new_params);
    assert!(
        matches!(response, Response::Ok(_)),
        "session.new failed: {response:?}"
    );

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

    let response = call_and_read_reply(
        &mut stream,
        &mut connection,
        "server.stop",
        stop_params(true),
    );
    assert!(
        matches!(response, Response::Ok(_)),
        "server.stop with --force should succeed while a session exists: {response:?}"
    );

    join_within(handle, Duration::from_secs(10));

    assert!(
        !process_exists(pid),
        "server.stop --force must end the real program, through the real API method"
    );
}
