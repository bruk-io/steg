//! AC #1 ("the mode set rather than assumed"), proved against a
//! deliberately permissive process umask - the one test in this task
//! that mutates the process-wide umask, in its own integration-test
//! binary (`cargo test` gives each `tests/*.rs` file its own process),
//! so it cannot corrupt any other test sharing a process under `cargo
//! test`'s default parallelism. `bind.rs`'s own unit tests (run under
//! the ambient umask, never set here) cover the ordinary case; this is
//! the one that proves the mode survives a *hostile* one.

use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use nix::sys::stat::{Mode, umask};
use steg_server::{Arbitration, ServerConfig, attempt};

/// Kept short against `sockaddr_un`'s 104-byte `sun_path` budget
/// (`crates/server/src/bind.rs`'s own doc comment found this matters).
fn unique_socket_path() -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let suffix = COUNTER.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!("{}-{suffix}.sock", std::process::id()))
}

#[test]
fn the_socket_is_0600_even_under_a_permissive_umask() {
    let previous = umask(Mode::empty());
    let path = unique_socket_path();
    let config = ServerConfig::default();
    let result = attempt(&config, &path);
    umask(previous); // restored before any assertion can early-return
    let Ok(Arbitration::Became(handle)) = result else {
        panic!("expected to become the server, got {result:?}");
    };
    let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
    assert_eq!(
        mode, 0o600,
        "the socket's mode must be 0600 regardless of the process umask"
    );
    drop(handle);
    let _ = std::fs::remove_file(&path);
}
