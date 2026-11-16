//! TASK-1.86 (doc-8 R99): the idle checker, shown end to end with a real
//! server process - `crates/server/tests/idle_stop.rs` is the in-process
//! proof of the mechanism itself; this is the one place that proves it
//! through the real `fixture_server` binary, with a short injected
//! `idle_timeout` (`STEG_TEST_IDLE_TIMEOUT_MS`) rather than the real 30
//! seconds.
//!
//! Reuses `server_start.rs`'s own `KillOnDrop`/`wait_for_socket`/
//! `wait_bounded` shape, duplicated here rather than shared (the same
//! convention every other file in this crate's `tests/` already follows).

use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

fn unique_socket_path() -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let suffix = COUNTER.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!("{}-idle-stop-{suffix}.sock", std::process::id()))
}

fn fixture_server_binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_fixture_server"))
}

fn steg_binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_steg"))
}

/// Kills and reaps its child on drop, including when a panicking assertion
/// unwinds past it (`server_start.rs`'s own `KillOnDrop`, duplicated here).
struct KillOnDrop {
    child: Child,
    done: bool,
}

impl KillOnDrop {
    fn new(child: Child) -> Self {
        Self { child, done: false }
    }
}

impl Drop for KillOnDrop {
    fn drop(&mut self) {
        if !self.done {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

fn wait_for_socket(path: &std::path::Path, timeout: Duration) {
    let deadline = Instant::now() + timeout;
    loop {
        if UnixStream::connect(path).is_ok() {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "nothing ever answered at {path:?}"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// Bounded, polling wait for `child` to exit - never a bare `wait()`,
/// which would hang this test instead of failing it with a message
/// naming the bound, if the process under test never actually exits.
fn wait_bounded(child: &mut Child, timeout: Duration) -> std::process::ExitStatus {
    let deadline = Instant::now() + timeout;
    loop {
        if let Some(status) = child.try_wait().expect("try_wait should not fail") {
            return status;
        }
        assert!(
            Instant::now() < deadline,
            "the process did not exit within the bound"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}

const IDLE_TIMEOUT_MS: u64 = 200;
const MARGIN: Duration = Duration::from_secs(5);

#[test]
fn an_on_demand_server_with_no_clients_stops_itself_and_a_fresh_connect_afterward_starts_a_new_one()
{
    let socket_path = unique_socket_path();
    let mut server = KillOnDrop::new(
        Command::new(fixture_server_binary())
            .env("STEG_SOCKET_PATH", &socket_path)
            .env("STEG_TEST_IDLE_TIMEOUT_MS", IDLE_TIMEOUT_MS.to_string())
            .env("STEG_TEST_LOGIN_SHELL", "/bin/zsh")
            .env("STEG_TEST_LOGIN_HOME", std::env::temp_dir())
            .env("STEG_TEST_LOGIN_USER", "steg-test-user")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("the fixture_server binary should spawn"),
    );
    wait_for_socket(&socket_path, Duration::from_secs(5));

    // No client ever connects after this - the idle checker, in the real
    // binary, must decide on its own that there is no one here.
    let status = wait_bounded(
        &mut server.child,
        Duration::from_millis(IDLE_TIMEOUT_MS) + MARGIN,
    );
    server.done = true; // already exited on its own; nothing left for `Drop` to do
    assert!(
        status.success(),
        "the server should have exited cleanly on its own: {status:?}"
    );

    // A fresh connect, after the server has already gone, is refused or
    // not found - either is correct (the same ambiguity `steg-server`'s
    // own `stop_racing_a_client_connecting_still_returns_within_a_bound`
    // already names): what matters is that it is *not* a stream that then
    // reads successfully, which would mean the old server was somehow
    // still there.
    assert!(
        UnixStream::connect(&socket_path).is_err(),
        "the old server's socket should not still answer"
    );

    // A fresh start at the same path succeeds and says so - the sdk's own
    // autostart logic already treats any connect failure as "no server,
    // start one," so this needs no new client code at all (AC #4's
    // ordinary-timing case).
    let mut second = KillOnDrop::new(
        Command::new(steg_binary())
            .args(["server", "start", "--on-demand"])
            .env("STEG_SOCKET_PATH", &socket_path)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("the real steg binary should spawn"),
    );
    let stdout = second
        .child
        .stdout
        .take()
        .expect("stdout was piped for this child");
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        use std::io::BufRead;
        let mut line = String::new();
        let _ = std::io::BufReader::new(stdout).read_line(&mut line);
        let _ = tx.send(line);
    });
    let first_line = rx
        .recv_timeout(Duration::from_secs(5))
        .expect("the second start should print a line within the bound");
    assert_eq!(
        first_line.trim(),
        "started a new server",
        "a fresh start at the vacated path should become the server, not find one already there"
    );
    second.child.kill().ok();
    second.child.wait().ok();
    second.done = true;
}
