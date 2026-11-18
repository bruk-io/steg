//! AC #1 ("the mode set rather than assumed"), proved against a
//! deliberately permissive process umask - the one test in this task
//! that mutates the process-wide umask, in its own integration-test
//! binary (`cargo test` gives each `tests/*.rs` file its own process),
//! so it cannot corrupt any other test sharing a process under `cargo
//! test`'s default parallelism. `bind.rs`'s own unit tests (run under
//! the ambient umask, never set here) cover the ordinary case; this is
//! the one that proves the mode survives a *hostile* one.

use std::os::unix::fs::PermissionsExt;

use nix::sys::stat::{Mode, umask};
use steg_server::{Arbitration, ServerConfig, attempt};

#[path = "support/mod.rs"]
mod support;
use support::scoped_socket_path;

#[test]
fn the_socket_is_0600_even_under_a_permissive_umask() {
    let previous = umask(Mode::empty());
    let (_socket_dir, path) = scoped_socket_path("s.sock");
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
