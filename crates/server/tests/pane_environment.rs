//! A pane's own `STEG_SESSION`/`STEG_TAB`/`STEG_PANE`/`STEG_SERVER` carry
//! real values (TASK-1.146, doc-2 section 4, R33) - proven the same way
//! `attach.rs`/`pane_send.rs`/`pane_wait.rs` already prove the rest of the
//! API: the sdk's own `Connection` doing the handshake and building
//! requests exactly as a real client would, against a real server over a
//! real socket, with a real pty behind the pane and an explicit program
//! (never the login shell's own interactive behaviour).
//!
//! **Not a full `env` dump.** `crates/steg/tests/login_environment.rs`'s
//! own doc records a real incident: a past negative control's whole `env`
//! dump put a real secret from this session's own environment into a board
//! comment, caught only by GitHub's push protection. That file needs a
//! *whole* dump because it asserts the *absence* of named variables (a
//! chosen-names list is what made an earlier version of it vacuous). This
//! file asserts the *presence* of four specific values instead, so the
//! pane's own program prints only those four, by name, with nothing else
//! captured - removing the leak surface outright rather than merely
//! trusting a shorter list. SAFETY: this machine's own environment holds
//! real credentials - no assertion in this file ever interpolates anything
//! but these four, test-chosen values into a failure message.

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
    ClientKind, Connection, Incoming, Response, WireRole as Role, description, parse_server_hello,
};
use steg_server::{LoginShell, ServerConfig, Stopper, serve};

#[path = "support/mod.rs"]
mod support;
use support::{ScopedTempDir, scoped_socket_path};

/// This file's own connections' default read timeout - the same bound
/// `attach.rs`/`pane_send.rs`/`pane_wait.rs` already settled on
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
    handle.join().expect("the server thread should not panic");
}

fn connect_and_handshake(socket_path: &std::path::Path) -> (UnixStream, Connection) {
    let mut stream = UnixStream::connect(socket_path).unwrap();
    stream.set_read_timeout(Some(DEFAULT_READ_TIMEOUT)).unwrap();
    let hello_bytes = Connection::send_hello("pane-environment-test-client", ClientKind::Cli, None);
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
    let deadline = Instant::now() + Duration::from_secs(20);
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

/// One real pane's own identifiers, learned the same way a real client
/// would: `session.new` -> `tab.list` -> `pane.list`, over the wire -
/// never read out of the server's own internal tree directly, since this
/// file is testing exactly what a client-shaped caller would see
/// (`attach.rs`'s own `new_pane_running` doc gives the same reasoning).
/// Returned individually, not only as the combined `session/tab/pane`
/// path `attach.rs`/`pane_wait.rs`'s own helpers return: this file's own
/// assertions need each one on its own, to compare against the matching
/// `STEG_*` line.
struct RealPane {
    session_id: String,
    tab_id: String,
    pane_id: String,
}

impl RealPane {
    fn path(&self) -> String {
        format!("{}/{}/{}", self.session_id, self.tab_id, self.pane_id)
    }
}

fn new_pane_running(
    stream: &mut UnixStream,
    connection: &mut Connection,
    program: Vec<&str>,
) -> RealPane {
    let mut params = Map::new();
    params.insert("program".to_string(), json!(program));
    let response = call_and_read_reply(stream, connection, "session.new", params);
    let Response::Ok(session) = response else {
        panic!("session.new failed: {response:?}")
    };
    let session_id = session
        .get("id")
        .and_then(Value::as_str)
        .expect("session.new returns {\"id\": ...}")
        .to_string();

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
        .expect("each pane record has an id")
        .to_string();

    RealPane {
        session_id,
        tab_id,
        pane_id,
    }
}

/// Asserts `dump` (this file's own four-line capture - never a whole
/// environment, see this file's own module doc) contains `name=value`
/// verbatim, on its own line. Safe to show both in the failure message:
/// `value` is always either a real identifier `tab.list`/`pane.list`
/// already reported back to this test over the wire, or this test's own
/// socket path - never anything from a real environment.
fn assert_line(dump: &str, name: &str, value: &str) {
    let line = format!("{name}={value}");
    assert!(
        dump.lines().any(|l| l == line),
        "expected {line:?} in the pane's own captured lines, got:\n{dump}"
    );
}

#[test]
fn a_real_panes_steg_session_tab_pane_and_server_are_the_servers_own_real_values() {
    let (socket_dir, socket_path, stop, handle) = start_server();
    let (mut stream, mut connection) = connect_and_handshake(&socket_path);

    // A path inside this test's own scoped directory - removed with it
    // (`ScopedTempDir`'s own `Drop`), never the shared system temp
    // directory a second concurrent run of this test could collide on.
    let out_path = socket_dir.path("out.txt");

    // Four lines, by name, nothing else - the pane's own program can
    // never capture more than this (this file's own module doc: why not
    // a full `env` dump). `exit 0` once written: this is an explicit
    // `Program::Command` pane (doc-2 section 6), so it stays reachable
    // by `pane.list`/`pane.wait` once its program has ended.
    let script = format!(
        "printf 'STEG_SESSION=%s\\nSTEG_TAB=%s\\nSTEG_PANE=%s\\nSTEG_SERVER=%s\\n' \
         \"$STEG_SESSION\" \"$STEG_TAB\" \"$STEG_PANE\" \"$STEG_SERVER\" > {}",
        out_path.display()
    );
    let pane = new_pane_running(&mut stream, &mut connection, vec!["sh", "-c", &script]);

    // No polling: the redirect above has already closed its file
    // descriptor by the time the program exits, which is exactly what
    // `pane.wait --for exit` blocks on - the reply only arrives once the
    // real exit has actually been observed (`wait.rs`'s own module doc),
    // so the file is complete the instant this call returns.
    let mut params = Map::new();
    params.insert("target".to_string(), json!(pane.path()));
    params.insert("for".to_string(), json!("exit"));
    params.insert("timeout".to_string(), json!(20));
    let response = call_and_read_reply(&mut stream, &mut connection, "pane.wait", params);
    let Response::Ok(_) = response else {
        panic!("pane.wait failed: {response:?}")
    };

    let dump = std::fs::read_to_string(&out_path).expect("the pane's own program wrote this file");

    // Four separate assertions, each naming the one `STEG_*` line it
    // owns - not one combined check - so a negative control that breaks
    // only one mechanism (identifiers, or the server address) is seen to
    // fail only the lines that mechanism actually supplies.
    assert_line(&dump, "STEG_SESSION", &pane.session_id);
    assert_line(&dump, "STEG_TAB", &pane.tab_id);
    assert_line(&dump, "STEG_PANE", &pane.pane_id);
    assert_line(&dump, "STEG_SERVER", socket_path.to_str().unwrap());

    stop_server(&stop, handle);
}
