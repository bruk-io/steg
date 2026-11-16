//! TASK-1.86 (doc-8 R99): the idle checker, proven in-process with real
//! sockets and threads - no process spawn needed at this layer.
//! `crates/steg/tests/idle_stop.rs` is the real-process proof ("shown end
//! to end with a real server process").
//!
//! Reuses `shutdown.rs`'s own `unique_socket_path`/`wait_until_accepting`/
//! `join_within` shape and `routing.rs`'s own `test_login`/
//! `connect_and_handshake`/`call_and_read_reply` shape - each duplicated
//! here rather than shared, the same convention this crate's other test
//! files already follow (an integration test file is its own crate; there
//! is no shared test-support module in this repository).

#![expect(
    clippy::unwrap_used,
    reason = "this whole file is a test; see routing.rs's own comment"
)]

use std::io::Read;
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::{Map, json};
use steg_sdk::{
    ClientKind, Connection, Incoming, Response, WireRole as Role, description, parse_server_hello,
};
use steg_server::{
    Error, LaunchSpec, LoginShell, ProgramLauncher, ServerConfig, SpawnedProgram, Stopper, serve,
};

fn unique_socket_path() -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let suffix = COUNTER.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!("{}-idle-stop-{suffix}.sock", std::process::id()))
}

/// A working `PATH` is required by `program_from_request`'s own default
/// `cwd` fallback landing on a real directory (`login.home`) - not by the
/// fake launcher below, which never actually execs anything.
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

fn wait_until_accepting(path: &std::path::Path) {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if UnixStream::connect(path).is_ok() {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "server never started accepting at {path:?}"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// Bounded join - never a bare `join()` - used by every test here that
/// proves "`run` returns promptly": a failure here panics with the bound
/// named, rather than hanging the suite.
fn join_within(server_thread: std::thread::JoinHandle<()>, bound: Duration) {
    let (done_tx, done_rx) = mpsc::channel::<()>();
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

fn start_server(config: ServerConfig) -> (PathBuf, Stopper, std::thread::JoinHandle<()>) {
    let socket_path = config
        .socket_path
        .clone()
        .expect("this file always sets socket_path explicitly");
    let stop = Stopper::new();
    let handle = {
        let config = config.clone();
        let stop = stop.clone();
        std::thread::spawn(move || {
            let _ = serve(&config, &stop, None);
        })
    };
    wait_until_accepting(&socket_path);
    (socket_path, stop, handle)
}

/// Real handshake over a real socket, with the sdk's own `Connection` -
/// the shape every real client actually uses (`routing.rs`'s own
/// `connect_and_handshake`, duplicated here).
fn connect_and_handshake(socket_path: &std::path::Path) -> (UnixStream, Connection) {
    let mut stream = UnixStream::connect(socket_path).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let hello_bytes = Connection::send_hello("idle-stop-test-client", ClientKind::Cli, None);
    std::io::Write::write_all(&mut stream, &hello_bytes).unwrap();

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

/// Writes a validated call and reads back its matching reply
/// (`routing.rs`'s own `call_and_read_reply`, duplicated here).
fn call_and_read_reply(
    stream: &mut UnixStream,
    connection: &mut Connection,
    method: &str,
    params: Map<String, serde_json::Value>,
) -> Response {
    let (id, bytes) = connection.call(method, params).expect("a valid call");
    std::io::Write::write_all(stream, &bytes).unwrap();
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

// --- A fake launcher: no real pty, no real process (`tests/program.rs`'s
// own `FakeLauncher`, duplicated and trimmed to what this file needs: a
// way to end the fake program from the test, nothing about its output). --

#[derive(Debug)]
struct FakeChild {
    exit_code: Mutex<mpsc::Receiver<i32>>,
}

impl portable_pty::Child for FakeChild {
    fn try_wait(&mut self) -> std::io::Result<Option<portable_pty::ExitStatus>> {
        match self
            .exit_code
            .lock()
            .expect("not poisoned: &mut self already keeps this exclusive")
            .try_recv()
        {
            Ok(code) => Ok(Some(exit_status(code))),
            Err(mpsc::TryRecvError::Empty | mpsc::TryRecvError::Disconnected) => Ok(None),
        }
    }

    fn wait(&mut self) -> std::io::Result<portable_pty::ExitStatus> {
        self.exit_code
            .lock()
            .expect("not poisoned: &mut self already keeps this exclusive")
            .recv()
            .map(exit_status)
            .map_err(|_| std::io::Error::other("the fake program's exit was never reported"))
    }

    fn process_id(&self) -> Option<u32> {
        None
    }
}

fn exit_status(code: i32) -> portable_pty::ExitStatus {
    #[expect(
        clippy::cast_sign_loss,
        reason = "a real exit code is 0..=255; this fake is never fed a negative one"
    )]
    portable_pty::ExitStatus::with_exit_code(code as u32)
}

impl portable_pty::ChildKiller for FakeChild {
    fn kill(&mut self) -> std::io::Result<()> {
        Ok(())
    }

    fn clone_killer(&self) -> Box<dyn portable_pty::ChildKiller + Send + Sync> {
        let (_tx, rx) = mpsc::channel();
        Box::new(FakeChild {
            exit_code: Mutex::new(rx),
        })
    }
}

/// What a `FakeLauncher`'s `spawn` call hands back to the test driving
/// it: ending it (`FakeDriver::end`) closes the pipe the pane's own relay
/// thread reads from (giving it a real end-of-file) and reports the exit
/// code, in that order - both are needed before the pane's own worker
/// thread will ever send `PaneEvent::Exited` (`tests/program.rs`'s own
/// `FakeDriver`, duplicated here).
struct FakeDriver {
    output: std::io::PipeWriter,
    exit: mpsc::Sender<i32>,
}

impl FakeDriver {
    fn end(self, exit_code: i32) {
        drop(self.output);
        let _ = self.exit.send(exit_code);
    }
}

/// A [`ProgramLauncher`] with no real pseudo-terminal and no real OS
/// process: each `spawn` call hands the test a [`FakeDriver`], through
/// the receiver [`FakeLauncher::new`] returns.
struct FakeLauncher {
    drivers: Mutex<mpsc::Sender<FakeDriver>>,
}

impl FakeLauncher {
    fn new() -> (Self, mpsc::Receiver<FakeDriver>) {
        let (tx, rx) = mpsc::channel();
        (
            Self {
                drivers: Mutex::new(tx),
            },
            rx,
        )
    }
}

impl ProgramLauncher for FakeLauncher {
    fn spawn(&self, _spec: &LaunchSpec) -> Result<SpawnedProgram, Error> {
        let (reader, writer) = std::io::pipe()?;
        let (exit_tx, exit_rx) = mpsc::channel();
        let driver = FakeDriver {
            output: writer,
            exit: exit_tx,
        };
        self.drivers
            .lock()
            .expect("not poisoned: nothing else locks this")
            .send(driver)
            .expect("the test still holds the matching receiver");
        Ok(SpawnedProgram {
            writer: Box::new(std::io::sink()),
            reader: Box::new(reader),
            resize: Box::new(|_cols, _rows| Ok(())),
            child: Box::new(FakeChild {
                exit_code: Mutex::new(exit_rx),
            }),
        })
    }
}

fn session_config(
    socket_path: PathBuf,
    idle_timeout: Option<Duration>,
    launcher: Arc<dyn ProgramLauncher>,
) -> ServerConfig {
    ServerConfig {
        socket_path: Some(socket_path),
        handshake_timeout: Duration::from_secs(5),
        login: test_login(),
        launcher,
        idle_timeout,
        ..ServerConfig::default()
    }
}

const IDLE_TIMEOUT: Duration = Duration::from_millis(200);
/// Must exceed one connect/accept cycle (the startup probe's own effect
/// on the deadline, section 6 of this task's plan) - a few milliseconds
/// in practice, so this margin is generous against that, not merely
/// "generous" in general.
const MARGIN: Duration = Duration::from_millis(800);

#[test]
fn a_genuinely_idle_server_stops_itself_within_a_bound() {
    let socket_path = unique_socket_path();
    let config = session_config(
        socket_path,
        Some(IDLE_TIMEOUT),
        Arc::new(steg_server::RealLauncher),
    );
    let (_socket_path, _stop, server_thread) = start_server(config);
    // No client ever connects after this - the idle checker must decide
    // on its own that there is no one here.
    join_within(server_thread, IDLE_TIMEOUT + MARGIN);
}

#[test]
fn a_session_that_exists_prevents_the_idle_stop() {
    let (launcher, _drivers) = FakeLauncher::new();
    let socket_path = unique_socket_path();
    let config = session_config(socket_path.clone(), Some(IDLE_TIMEOUT), Arc::new(launcher));
    let (socket_path, stop, server_thread) = start_server(config);

    let (mut stream, mut connection) = connect_and_handshake(&socket_path);
    let response = call_and_read_reply(&mut stream, &mut connection, "session.new", Map::new());
    let session_id = match response {
        Response::Ok(value) => value
            .get("id")
            .expect("session.new should answer with the new session's id")
            .as_str()
            .expect("the id is a string")
            .to_string(),
        Response::Error(error) => panic!("session.new failed: {error:?}"),
    };
    drop(stream); // the connection is gone; the session is not

    // The session alone must be enough to prevent the idle stop, with no
    // client connected at all - proven by a bound that would otherwise
    // have been crossed several times over.
    let (done_tx, done_rx) = mpsc::channel::<()>();
    let waiter = std::thread::spawn(move || {
        let _ = server_thread.join();
        let _ = done_tx.send(());
    });
    assert!(
        done_rx.recv_timeout(IDLE_TIMEOUT + MARGIN).is_err(),
        "a server with an open session must not idle-stop"
    );

    // Inverted, in the same run rather than a separate test: close the
    // very session that was holding it open, over a fresh connection
    // (the earlier one was dropped), and the same server now does stop
    // within an ordinary bound - proving the session, not the
    // connection, was what mattered, not a coincidence of timing. The
    // bound below is generous against `force_end`'s own already-proven
    // grace/escalate/join timing (the fake program never exits on its
    // own, so `--force` has real work to do, unlike every other test in
    // this file) - not against the idle checker, which only starts its
    // own clock once that work is done and the tree genuinely has no
    // session left.
    let (mut stream, mut connection) = connect_and_handshake(&socket_path);
    let response = call_and_read_reply(
        &mut stream,
        &mut connection,
        "session.close",
        Map::from_iter([
            ("target".to_string(), json!(session_id)),
            ("force".to_string(), json!(true)),
        ]),
    );
    assert!(
        matches!(response, Response::Ok(_)),
        "session.close should succeed: {response:?}"
    );
    drop(stream);

    done_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("the server should idle-stop once its one session is closed");
    waiter.join().expect("the waiter thread should not panic");
    stop.stop(); // idempotent no-op: the server has already stopped itself
}

#[test]
fn closing_a_shell_panes_last_session_with_no_client_connected_still_wakes_the_checker() {
    let (launcher, drivers) = FakeLauncher::new();
    let socket_path = unique_socket_path();
    let config = session_config(socket_path, Some(IDLE_TIMEOUT), Arc::new(launcher));
    let (socket_path, _stop, server_thread) = start_server(config);

    let (mut stream, mut connection) = connect_and_handshake(&socket_path);
    let response = call_and_read_reply(&mut stream, &mut connection, "session.new", Map::new());
    assert!(
        matches!(response, Response::Ok(_)),
        "session.new failed: {response:?}"
    );
    let driver = drivers
        .recv_timeout(Duration::from_secs(5))
        .expect("the fake launcher should have been asked to spawn the shell pane");
    drop(stream); // the client disconnects *before* the program "exits" -

    // Only now, with nobody connected, does the fake shell "exit" - R97's
    // own cascade (a plain shell pane closes when its program exits, and
    // that can close its tab and its session in turn) runs entirely on
    // the drain thread, with no connection involved at all.
    driver.end(0);

    join_within(server_thread, IDLE_TIMEOUT + MARGIN);
}

#[test]
fn every_sender_dropping_is_what_lets_run_return_after_stop() {
    let (launcher, drivers) = FakeLauncher::new();
    let socket_path = unique_socket_path();
    // A long `idle_timeout` here, deliberately: this test's own `stop()`
    // call is what ends the server, not the idle checker's own decision -
    // if `idle_timeout` were short, this test could not tell "stopped
    // because of my `stop()` call" from "stopped because it also idled
    // out," and the bound below would prove nothing specific.
    let config = session_config(
        socket_path,
        Some(Duration::from_secs(30)),
        Arc::new(launcher),
    );
    let (socket_path, stop, server_thread) = start_server(config);

    let (mut stream, mut connection) = connect_and_handshake(&socket_path);
    let response = call_and_read_reply(&mut stream, &mut connection, "session.new", Map::new());
    assert!(
        matches!(response, Response::Ok(_)),
        "session.new failed: {response:?}"
    );
    let driver = drivers
        .recv_timeout(Duration::from_secs(5))
        .expect("the fake launcher should have been asked to spawn the shell pane");
    drop(stream);
    // The session alone keeps the checker blocked in `Wait::Forever` (no
    // timeout at all) - the hardest case for the join bound to cover,
    // since nothing but a sender dropping can wake it.

    stop.stop();
    // `Runtime::stop()` still has one real, still-running pane to force-
    // end (`force_end`'s own signal-then-wait passes) before it gets to
    // dropping this task's own idle notifier - ending the fake program's
    // "process" right away, the same way a real one would react promptly
    // to the polite signal, is what keeps this test about the join bound
    // this task adds, not about `force_end`'s own already-proven timing.
    driver.end(0);
    join_within(server_thread, Duration::from_secs(5));
}

#[test]
fn an_explicit_starts_runtime_behaves_correctly_through_a_full_lifecycle() {
    let (launcher, _drivers) = FakeLauncher::new();
    let socket_path = unique_socket_path();
    let config = session_config(socket_path, None, Arc::new(launcher));
    let (socket_path, stop, server_thread) = start_server(config);

    let (mut stream, mut connection) = connect_and_handshake(&socket_path);
    let response = call_and_read_reply(&mut stream, &mut connection, "session.new", Map::new());
    let session_id = match response {
        Response::Ok(value) => value.get("id").unwrap().as_str().unwrap().to_string(),
        Response::Error(error) => panic!("session.new failed: {error:?}"),
    };
    let response = call_and_read_reply(
        &mut stream,
        &mut connection,
        "session.close",
        Map::from_iter([
            ("target".to_string(), json!(session_id)),
            ("force".to_string(), json!(true)),
        ]),
    );
    assert!(
        matches!(response, Response::Ok(_)),
        "session.close failed: {response:?}"
    );
    drop(stream);

    // No wait at all: there is no `idle_timeout` to wait out. A fresh
    // connect succeeding is the proof the server is still listening.
    let (_stream, _connection) = connect_and_handshake(&socket_path);

    stop.stop();
    join_within(server_thread, Duration::from_secs(5));
}

/// A [`ProgramLauncher`] whose `spawn` always panics - the test-only seam
/// review finding 1 asked for ("a connection whose handler panics after
/// the handshake"), not a production path that can panic on request: the
/// injection point (`ServerConfig.launcher`) already exists for ordinary
/// reasons, this is only an unusual implementation of it, supplied by the
/// test, the same way `FakeLauncher` above is.
struct PanickingLauncher;

impl ProgramLauncher for PanickingLauncher {
    fn spawn(&self, _spec: &LaunchSpec) -> Result<SpawnedProgram, Error> {
        panic!(
            "deliberate test panic (TASK-1.86 review finding 1): a request handler panicking \
             after the handshake must still report ConnectionEvent::Disconnected"
        );
    }
}

/// Review finding 1: `connection::handle` sent `Connected` on entry but
/// only sent the matching `Disconnected` as a plain statement at the very
/// end - skipped by a panic's unwind, since nothing below a panic ever
/// runs. One panic in a request handler left the idle checker's live-
/// client count one too high for the rest of the server's life, so an
/// on-demand server never stopped when idle again - silently, since
/// nothing else depends on that count. Fixed in `connection.rs`:
/// `ConnectionCleanup` (the guard that already ran on every path out of
/// `run_post_handshake`, including an unwind) now holds the sender too,
/// and sends `Disconnected` from its own `Drop`.
///
/// **Why this test force-closes a session it never meant to create.**
/// `PanickingLauncher::spawn` panics *inside* `Runtime::start_and_
/// dispatch`'s own match on `pane::start`'s result - a panic there skips
/// that function's own `rollback_mint` the same way the bug above skipped
/// `Disconnected`, a related but separate gap (named here, not fixed -
/// `pane::start`/`start_and_dispatch` are TASK-1.76's own code, and this
/// task's own criteria are about the idle checker, not that pairing).
/// Left alone, the panicking `session.new` leaves a real session in the
/// tree with no pane handle behind it (confirmed by reading `close_and_
/// dispatch`: a scope pane missing from `state.panes` is silently
/// filtered out of what gets force-ended, never an error), which would
/// keep `session_count()` above zero forever and confound this test's own
/// proof - closing it explicitly isolates the one thing this test is
/// actually about: the live-client count, not the mint/rollback pairing.
#[test]
fn a_panicking_request_handler_still_sends_disconnected_and_the_server_idles_out() {
    let socket_path = unique_socket_path();
    let config = session_config(socket_path, Some(IDLE_TIMEOUT), Arc::new(PanickingLauncher));
    let (socket_path, _stop, server_thread) = start_server(config);

    // Connection A: triggers the panic. Nothing ever answers this
    // request - the connection thread dies mid-dispatch - so this only
    // writes the call and then confirms the connection actually closed
    // (a real, bounded read that must see EOF or an error, never hang
    // waiting for a reply that is never coming).
    {
        let (mut stream, mut connection) = connect_and_handshake(&socket_path);
        let (_id, bytes) = connection
            .call("session.new", Map::new())
            .expect("a valid call");
        std::io::Write::write_all(&mut stream, &bytes)
            .expect("writing the call should not fail before the panic unwinds the connection");
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut buf = [0_u8; 16];
        match stream.read(&mut buf) {
            Ok(0) => {}  // closed with no reply - expected
            Err(_) => {} // reset or similar - also expected
            Ok(n) => panic!("expected the connection to close with no reply, got {n} bytes"),
        }
    }

    // Force-close the orphaned session (see this test's own doc comment)
    // over a fresh, ordinary connection - its own Connected/Disconnected
    // pair is unaffected by any of this, proven by every other test in
    // this file.
    let (mut stream, mut connection) = connect_and_handshake(&socket_path);
    let response = call_and_read_reply(&mut stream, &mut connection, "session.list", Map::new());
    let session_id = match response {
        Response::Ok(value) => value
            .as_array()
            .and_then(|sessions| sessions.first())
            .and_then(|session| session.get("id"))
            .expect("the orphaned session should still be listed")
            .as_str()
            .expect("the id is a string")
            .to_string(),
        Response::Error(error) => panic!("session.list failed: {error:?}"),
    };
    let response = call_and_read_reply(
        &mut stream,
        &mut connection,
        "session.close",
        Map::from_iter([
            ("target".to_string(), json!(session_id)),
            ("force".to_string(), json!(true)),
        ]),
    );
    assert!(
        matches!(response, Response::Ok(_)),
        "force-closing the orphaned session should succeed: {response:?}"
    );
    drop(stream);

    // No sessions left, and - with the fix - no live clients either:
    // connection A's own `Disconnected` was sent by `ConnectionCleanup`'s
    // `Drop` despite the panic, connection B's own was sent normally.
    // Without the fix, connection A's `Connected` has no match, ever, so
    // `live_clients` never returns to zero and this never returns.
    join_within(server_thread, IDLE_TIMEOUT + MARGIN);
}
