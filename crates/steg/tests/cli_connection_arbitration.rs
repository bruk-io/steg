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
use std::sync::mpsc;
use std::time::Duration;

#[path = "support/mod.rs"]
mod support;
use support::scoped_socket_path;

fn steg_binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_steg"))
}

fn run_steg(args: &[&str], socket: &std::path::Path) -> Output {
    Command::new(steg_binary())
        .args(args)
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("STEG_SERVER", socket)
        .output()
        .expect("the real steg binary should run to completion")
}

/// Binds a real `UnixListener` at `path`, on its own thread, accepts
/// exactly **one** connection, drops the listener, waits `gap`, then
/// drops the accepted stream with nothing ever written to it - the
/// client-visible half of TASK-1.77's own race. Signals `ready` once
/// bound, so the test synchronises on "the listener is up" without
/// spending an accept on a throwaway probe connection.
///
/// **The listener is dropped before the accepted stream, not after**
/// (TASK-1.150): a Unix-domain `connect()` completes into the kernel's
/// listen backlog without needing a matching `accept()`, so dropping the
/// stream first - the EOF a client's first attempt sees, and the signal
/// its own immediate retry waits for - would leave a window, until the
/// listener itself is actually dropped, in which that retry's own
/// `connect()` could land in the same backlog and be torn down when the
/// listener finally closed: `ClosedBeforeHello` on both attempts, with
/// autostart's own `spawn()` never reached, and no real server ever
/// involved (confirmed directly, not assumed - `crates/sdk/src/
/// local_socket.rs`'s `connect()` only calls `spawn()` once a raw
/// `connect()` has already returned `Err`, which it had not in either
/// attempt of the failure this task found). Dropping the listener first
/// closes that window entirely - a later `connect()` attempt is refused
/// outright, nothing further queues - while the already-accepted
/// stream, on its own independent descriptor, is unaffected and still
/// delivers its EOF exactly as before, after `gap`. The same ordering
/// discipline the real server already applies in its own shutdown path,
/// for the identical reason (`crates/server/src/listen.rs`'s `run`, the
/// comment on dropping `listener` immediately after the accept loop
/// stops).
///
/// `gap` is `Duration::ZERO` for this file's ordinary callers (nothing
/// to wait for - the window above is already closed by the ordering
/// itself, not by timing) and an explicit, generous, adversarial pause
/// for
/// [`closing_the_hostile_listener_before_the_stream_leaves_no_window_for_a_late_connect`]
/// alone: that one test's whole point is to drive *this* function, not a
/// parallel copy of it, so that a regression here cannot leave it green.
fn spawn_hostile_listener(path: PathBuf, gap: Duration) -> mpsc::Receiver<()> {
    let (ready_tx, ready_rx) = mpsc::channel();
    std::thread::spawn(move || {
        let listener = UnixListener::bind(&path).expect("binding the hostile listener");
        ready_tx.send(()).expect("the test is still waiting");
        let stream = listener.accept().ok().map(|(stream, _)| stream);
        drop(listener);
        std::thread::sleep(gap);
        drop(stream);
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
    let (_socket_dir, socket) = scoped_socket_path("arbitration.sock");
    let ready = spawn_hostile_listener(socket.clone(), Duration::ZERO);
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
    let (_socket_dir, socket) = scoped_socket_path("arbitration-sanity.sock");
    let ready = spawn_hostile_listener(socket.clone(), Duration::ZERO);
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

/// TASK-1.150's own proof that `spawn_hostile_listener`'s drop order
/// closes the race, not merely avoids it by luck: an explicit, generous
/// pause sits between dropping the listener and dropping the stream -
/// deliberately adversarial, not a wait on any event - during which the
/// real `steg` binary's own first and second `connect()` attempts both
/// have every opportunity to land in the listener's backlog if it were
/// still open. With the listener already gone before the pause starts,
/// neither can; the retry instead correctly finds nothing listening,
/// autostarts a real server, and the whole call still succeeds. Drives
/// `spawn_hostile_listener` itself (not a parallel copy), so a
/// regression in the function every other test here also uses cannot
/// leave this one green. Run with the two drops in the opposite order
/// (the fixture's own order before this task), this same test fails
/// with `ClosedBeforeHello` by exactly the mechanism this task found -
/// confirmed directly, not assumed, and recorded on the task rather
/// than repeated here, since this fixture's own order is the one thing
/// this file should state only once.
#[test]
fn closing_the_hostile_listener_before_the_stream_leaves_no_window_for_a_late_connect() {
    let (_socket_dir, socket) = scoped_socket_path("arbitration-gap.sock");
    let ready = spawn_hostile_listener(socket.clone(), Duration::from_millis(200));
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
