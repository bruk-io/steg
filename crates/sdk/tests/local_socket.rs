//! AC #3: "The optional local-socket provider connects to a running
//! server, and with none running starts one by running the steg binary and
//! waits for its socket; it can be told not to start one." Against a
//! plain listener and a fixture binary in a temp path - no real `steg`
//! binary, no network.
//!
//! An integration test, not a unit test inside `local_socket.rs`:
//! `CARGO_BIN_EXE_<name>` (to find the fixture binary's compiled path) is
//! only set for a test under `tests/`, not for the lib crate's own
//! `#[cfg(test)]` unit tests - found while building this.

#![cfg(feature = "local-socket")]
#![expect(
    clippy::disallowed_types,
    clippy::disallowed_methods,
    reason = "R103: this integration test proves the local-socket provider by binding a \
              real listener and timing a real spawn failure, same as the one production \
              module it tests (`crates/sdk/src/local_socket.rs`) is already allowed to do - \
              a test cannot prove socket-finding/starting behaviour without touching a \
              socket, a timer, or (to build a non-executable fixture file) the filesystem. \
              Kept to this test file, which exists only to exercise that one module."
)]

use std::os::unix::net::UnixListener;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use steg_sdk::{ConnectError, ConnectOptions, connect};

fn fixture_binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_fake-server-for-tests"))
}

/// A path that does not exist at all, so a test asserting "nothing was
/// spawned" can tell: if the provider tried to spawn it, that attempt
/// itself would fail distinctly rather than `connect` just happening to
/// succeed another way.
fn binary_path_that_does_not_exist() -> PathBuf {
    PathBuf::from("/nonexistent/should-not-run")
}

/// A real file that exists but is not executable (no execute bit) - the
/// other half of "the binary cannot be executed", distinct from a path
/// that does not exist at all.
fn binary_that_exists_but_is_not_executable() -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let suffix = COUNTER.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "steg-sdk-test-not-executable-{}-{suffix}",
        std::process::id()
    ));
    std::fs::write(&path, b"not a program").expect("writing the fixture file should succeed");
    path
}

/// A socket path unique to this test, under the real `TMPDIR` -
/// deliberately not a directory this test creates: each test names one
/// unused file under a directory the OS already made private to this
/// user.
fn unique_socket_path() -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let suffix = COUNTER.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "steg-sdk-test-{}-{suffix}.sock",
        std::process::id()
    ))
}

#[test]
fn connects_without_spawning_when_a_listener_is_already_bound() {
    let socket_path = unique_socket_path();
    let _listener = UnixListener::bind(&socket_path).unwrap();

    let options = ConnectOptions {
        binary: binary_path_that_does_not_exist(),
        socket_path: Some(socket_path),
        autostart: true,
        connect_timeout: Duration::from_millis(200),
    };
    assert!(connect(&options).is_ok());
}

#[test]
fn refuses_to_start_one_when_autostart_is_false() {
    let options = ConnectOptions {
        binary: binary_path_that_does_not_exist(),
        socket_path: Some(unique_socket_path()),
        autostart: false,
        connect_timeout: Duration::from_millis(200),
    };
    assert!(matches!(
        connect(&options),
        Err(ConnectError::NoServerRunning)
    ));
}

#[test]
fn starts_one_and_waits_when_nothing_is_listening_and_autostart_is_true() {
    let options = ConnectOptions {
        binary: fixture_binary(),
        socket_path: Some(unique_socket_path()),
        autostart: true,
        connect_timeout: Duration::from_secs(5),
    };
    assert!(connect(&options).is_ok());
}

#[test]
fn gives_up_with_a_timeout_when_nothing_ever_binds() {
    // `/bin/sh` is a real, spawnable program that exits immediately
    // without touching the socket, standing in for a server that never
    // answers.
    let options = ConnectOptions {
        binary: PathBuf::from("/bin/sh"),
        socket_path: Some(unique_socket_path()),
        autostart: true,
        connect_timeout: Duration::from_millis(100),
    };
    assert!(matches!(connect(&options), Err(ConnectError::Timeout)));
}

/// Review finding: `ConnectError::Spawn` was never exercised through
/// `connect()` itself. With `autostart` on and nothing listening, a
/// `binary` path that does not exist at all fails at `spawn()`, which
/// `connect()` is documented to return promptly rather than waiting out
/// `connect_timeout` for - proved here with a generous five-second
/// timeout and an assertion that this returns in well under a second of
/// it.
#[test]
fn spawn_fails_promptly_when_the_binarys_path_does_not_exist() {
    let options = ConnectOptions {
        binary: binary_path_that_does_not_exist(),
        socket_path: Some(unique_socket_path()),
        autostart: true,
        connect_timeout: Duration::from_secs(5),
    };
    let started = Instant::now();
    let result = connect(&options);
    let elapsed = started.elapsed();
    assert!(matches!(result, Err(ConnectError::Spawn(_))));
    assert!(
        elapsed < Duration::from_secs(1),
        "connect() took {elapsed:?} to fail - it should know as soon as spawn() does, not \
         wait out the 5 second connect_timeout"
    );
}

/// The other half of "the binary cannot be executed": a real file that
/// exists but has no execute bit, distinct from a path that is not there
/// at all.
#[test]
fn spawn_fails_when_the_binary_exists_but_is_not_executable() {
    let options = ConnectOptions {
        binary: binary_that_exists_but_is_not_executable(),
        socket_path: Some(unique_socket_path()),
        autostart: true,
        connect_timeout: Duration::from_secs(5),
    };
    assert!(matches!(connect(&options), Err(ConnectError::Spawn(_))));
}
