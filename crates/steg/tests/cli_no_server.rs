//! Criterion #4's no-server half: `steg version`, `steg completion
//! <shell>`, and `--help` at every level of the tree work with no server
//! running, and this task's own engineer decision 1 (`steg`/`steg <noun>`
//! alone list, status 0, as doc-2 section 2 says) - through the real
//! binary, never this machine's real server (`STEG_SOCKET_PATH` always
//! points at a throwaway path).
//!
//! The one thing worth a real negative control here: that these commands
//! truly never attempt a connection, not merely that they happen to
//! succeed anyway. `STEG_SOCKET_PATH` is pointed at a socket this test
//! itself is listening on - if any of them tried to connect, `accept()`
//! would return - and every test below asserts it never does, within a
//! bounded wait.

use std::os::unix::net::UnixListener;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::mpsc;
use std::time::Duration;

fn steg_binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_steg"))
}

#[path = "support/mod.rs"]
mod support;
use support::scoped_socket_path;

/// Runs `args` against the real binary with `STEG_SOCKET_PATH` pointed at
/// `socket` - a path something *is* listening on, so a command that
/// reached for a connection would succeed, not merely fail quietly.
fn run_with_a_real_listener_at(args: &[&str], socket: &Path) -> Output {
    Command::new(steg_binary())
        .args(args)
        .env("STEG_SOCKET_PATH", socket)
        .output()
        .expect("the real steg binary should run to completion")
}

/// Asserts `listener` never accepted a connection within `window` - the
/// one named exception to "no sleeps," the same as `crates/steg/tests/
/// server_start.rs`'s own: proving an absence needs a bounded wait, since
/// there is no event to wait on instead.
fn assert_never_accepted(listener: UnixListener, window: Duration) {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let accepted = listener.accept();
        let _ = tx.send(accepted.is_ok());
    });
    match rx.recv_timeout(window) {
        Err(mpsc::RecvTimeoutError::Timeout) => {} // never accepted - the expected case
        Ok(true) => panic!("a connection was accepted; the command reached for the server"),
        Ok(false) | Err(mpsc::RecvTimeoutError::Disconnected) => {
            panic!("accept() returned an error, which still means it was called")
        }
    }
}

fn assert_contacts_no_server(args: &[&str], expected_status: i32) {
    let (_socket_dir, socket) = scoped_socket_path("s.sock");
    let listener = UnixListener::bind(&socket).expect("binding the throwaway listener");

    let output = run_with_a_real_listener_at(args, &socket);
    assert_eq!(
        output.status.code(),
        Some(expected_status),
        "{args:?}: stderr was {}",
        String::from_utf8_lossy(&output.stderr)
    );

    assert_never_accepted(listener, Duration::from_millis(500));
    let _ = std::fs::remove_file(&socket);
}

#[test]
fn version_contacts_no_server() {
    assert_contacts_no_server(&["version"], 0);
}

#[test]
fn bare_help_contacts_no_server() {
    assert_contacts_no_server(&["--help"], 0);
}

#[test]
fn a_commands_own_help_contacts_no_server() {
    assert_contacts_no_server(&["pane", "split", "--help"], 0);
}

#[test]
fn completion_contacts_no_server() {
    let (_socket_dir, socket) = scoped_socket_path("s.sock");
    let listener = UnixListener::bind(&socket).expect("binding the throwaway listener");

    let output = run_with_a_real_listener_at(&["completion", "zsh"], &socket);
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr was {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        !output.stdout.is_empty(),
        "a completion script should print something"
    );

    assert_never_accepted(listener, Duration::from_millis(500));
    let _ = std::fs::remove_file(&socket);
}

/// Engineer decision 1 (this task's plan): `steg` alone lists the nouns
/// and exits 0, as doc-2 section 2 says - not the spike's own choice of
/// failing with status 2.
#[test]
fn bare_steg_lists_the_nouns_and_exits_0_and_contacts_no_server() {
    let (_socket_dir, socket) = scoped_socket_path("s.sock");
    let listener = UnixListener::bind(&socket).expect("binding the throwaway listener");

    let output = run_with_a_real_listener_at(&[], &socket);
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr was {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.lines().any(|line| line == "pane"), "{stdout:?}");

    assert_never_accepted(listener, Duration::from_millis(500));
    let _ = std::fs::remove_file(&socket);
}

/// The same decision, for `steg <noun>` alone.
#[test]
fn bare_noun_lists_its_verbs_and_exits_0_and_contacts_no_server() {
    let (_socket_dir, socket) = scoped_socket_path("s.sock");
    let listener = UnixListener::bind(&socket).expect("binding the throwaway listener");

    let output = run_with_a_real_listener_at(&["pane"], &socket);
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr was {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.lines().any(|line| line == "split"), "{stdout:?}");

    assert_never_accepted(listener, Duration::from_millis(500));
    let _ = std::fs::remove_file(&socket);
}

/// Negative control for `assert_never_accepted` itself: a command that
/// *does* reach for a connection (this task's own placeholder, which
/// never does - so this proves the mechanism on a method we know is
/// unreachable any other way: by connecting directly, bypassing the CLI
/// entirely, to confirm the listener and the bounded wait actually
/// detect an accepted connection rather than always timing out).
#[test]
fn assert_never_accepted_itself_fails_when_a_connection_is_made() {
    let (_socket_dir, socket) = scoped_socket_path("s.sock");
    let listener = UnixListener::bind(&socket).expect("binding the throwaway listener");

    let connecting_socket = socket.clone();
    std::thread::spawn(move || {
        let _ = std::os::unix::net::UnixStream::connect(&connecting_socket);
    });

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        assert_never_accepted(listener, Duration::from_secs(2));
    }));
    assert!(
        result.is_err(),
        "assert_never_accepted should have panicked once a connection was actually accepted"
    );

    let _ = std::fs::remove_file(&socket);
}
