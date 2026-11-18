//! TASK-1.119 AC #4, corroboration with real processes - the sharpest,
//! load-bearing proof (the counting one, in both modes) lives in
//! `crates/sdk/src/local_socket.rs`'s own unit tests, in-process, where a
//! precise accept count can be asserted with no race against a real
//! process's own timing. This file is the weaker, real-process check the
//! plan named on purpose: a hostile in-process listener thread (not a
//! separate OS process - identical from the socket's own point of view,
//! the same convention `cli_no_server.rs` already uses) stands in for a
//! server that accepts and closes with no hello, and the real `steg`
//! binary, run as a genuine child process, is asserted only to succeed
//! within a bound - not on a specific retry count, which a real
//! process's own timing can't be pinned to as tightly as the in-process
//! test already does precisely.

use std::io::Read;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::time::Duration;

fn unique_socket_path(label: &str) -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let suffix = COUNTER.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!("{}-{label}-{suffix}.sock", std::process::id()))
}

fn steg_binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_steg"))
}

fn run_steg(args: &[&str], socket: &std::path::Path) -> Output {
    Command::new(steg_binary())
        .args(args)
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("STEG_SOCKET_PATH", socket)
        .output()
        .expect("the real steg binary should run to completion")
}

/// Binds a real `UnixListener` at `path`, on its own thread, accepts
/// exactly **one** connection and drops it immediately with nothing
/// written - the client-visible half of TASK-1.77's own race - then
/// drops the listener itself, so the socket file answers nothing further
/// and a later direct `connect()` there gets refused outright, the same
/// way a real server's own socket behaves once it is truly gone (never a
/// second hostile accept, which would consume this task's own one
/// retry and leave nothing for a real autostart to reach). Signals
/// `ready` once bound, so the test synchronises on "the listener is up"
/// without spending an accept on a throwaway probe connection.
fn spawn_hostile_listener(path: PathBuf) -> mpsc::Receiver<()> {
    let (ready_tx, ready_rx) = mpsc::channel();
    std::thread::spawn(move || {
        let listener = UnixListener::bind(&path).expect("binding the hostile listener");
        ready_tx.send(()).expect("the test is still waiting");
        if let Ok((stream, _)) = listener.accept() {
            drop(stream);
        }
        drop(listener);
    });
    ready_rx
}

/// Kills, on drop, whatever process `lsof` finds holding `socket_path`'s
/// arbiter lock open - the real server this test's own command
/// autostarts after the hostile listener's socket is gone is spawned and
/// immediately dropped by the sdk's own `connect()` (the same reason
/// `cli_exit_statuses.rs`'s own `ServerAtSocket` exists, duplicated here
/// per this crate's convention).
struct ServerAtSocket(PathBuf);

impl Drop for ServerAtSocket {
    fn drop(&mut self) {
        let mut lock_path = self.0.as_os_str().to_owned();
        lock_path.push(".lock");
        let Ok(output) = Command::new("lsof").arg("-t").arg(&lock_path).output() else {
            return;
        };
        let text = String::from_utf8_lossy(&output.stdout);
        for pid in text.lines().map(str::trim).filter(|line| !line.is_empty()) {
            let _ = Command::new("kill").args(["-9", pid]).status();
        }
    }
}

/// The corroborating real-process proof: a hostile listener stands where
/// a server would be; the real CLI, told to autostart, still succeeds -
/// which it can only do by surviving the EOF-before-hello race and
/// starting a genuine server of its own once the hostile listener's
/// socket is gone (the hostile thread never answers a hello at all, so
/// any success here is the retry path, not a fluke).
#[test]
fn a_hostile_first_connection_does_not_stop_a_real_autostart_from_succeeding() {
    let socket = unique_socket_path("arbitration");
    let ready = spawn_hostile_listener(socket.clone());
    ready
        .recv_timeout(Duration::from_secs(2))
        .expect("the hostile listener should bind within the bound");
    let _server_guard = ServerAtSocket(socket.clone());

    let output = run_steg(&["session", "list", "--json"], &socket);
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "[]");
}

/// Sanity, not the criterion itself: without the retry, a hostile first
/// connection would simply look like "a server answered and then the
/// connection died" to anything that didn't know to try again - this
/// just confirms the hostile listener really does behave that way at the
/// raw socket level, independent of this task's own code, so a failure
/// in the test above is read as this task's own retry breaking, not as
/// the fixture never having worked.
#[test]
fn the_hostile_listener_itself_accepts_and_closes_with_nothing_written() {
    let socket = unique_socket_path("arbitration-sanity");
    let ready = spawn_hostile_listener(socket.clone());
    ready
        .recv_timeout(Duration::from_secs(2))
        .expect("the hostile listener should bind within the bound");

    let mut stream = UnixStream::connect(&socket).expect("connecting to the hostile listener");
    let mut buf = [0_u8; 16];
    let n = stream
        .read(&mut buf)
        .expect("reading from the hostile listener");
    assert_eq!(
        n, 0,
        "the hostile listener should write nothing before closing"
    );
}
