//! The shutdown ordering part of AC #5 ("the server runs in the
//! foreground... so this part is testable alone") and the seam part B's
//! idle checker will use (`Stopper`): stop accepting, close the
//! listener, remove the socket file, release the lock last - proved
//! directly, not assumed, and bounded by a channel rather than a sleep.

use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use steg_server::{ServeOutcome, ServerConfig, Stopper, serve, stop_and_wake};

/// Kept short against `sockaddr_un`'s 104-byte `sun_path` budget
/// (`crates/server/src/bind.rs`'s own doc comment found this matters).
fn unique_socket_path() -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let suffix = COUNTER.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!("{}-{suffix}.sock", std::process::id()))
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

#[test]
fn stopping_closes_the_listener_removes_the_socket_and_releases_the_lock() {
    let socket_path = unique_socket_path();
    let lock_path = {
        let mut p = socket_path.as_os_str().to_owned();
        p.push(".lock");
        PathBuf::from(p)
    };
    let config = ServerConfig {
        socket_path: Some(socket_path.clone()),
        ..ServerConfig::default()
    };
    let stop = Stopper::new();
    let (done_tx, done_rx) = mpsc::channel();
    {
        let config = config.clone();
        let stop = stop.clone();
        std::thread::spawn(move || {
            let outcome = serve(&config, &stop, None);
            let _ = done_tx.send(outcome.map(|o| matches!(o, ServeOutcome::Became)));
        });
    }
    wait_until_accepting(&socket_path);

    stop_and_wake(&stop, &socket_path);

    // Bounded on a real event (`serve` returning, sent over the
    // channel), not a sleep.
    let became = done_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("serve should return promptly once stopped")
        .expect("serve should not itself fail");
    assert!(
        became,
        "this process should have been the one that became the server"
    );

    assert!(
        !socket_path.exists(),
        "the socket file should be removed on shutdown"
    );

    // A client connecting now sees exactly what it would see against a
    // server that never existed - `ConnectionRefused` or `NotFound`,
    // both indistinguishable from "nothing is listening", which is the
    // correct, safe reading (the sdk's own `local_socket::connect`
    // treats either the same way: try to start one).
    let err = UnixStream::connect(&socket_path).unwrap_err();
    assert!(
        matches!(
            err.kind(),
            std::io::ErrorKind::NotFound | std::io::ErrorKind::ConnectionRefused
        ),
        "expected NotFound or ConnectionRefused, got {err:?}"
    );

    // The lock is released too: a fresh attempt at the same path should
    // not find it contended.
    let second = steg_server::attempt(&config, &socket_path).unwrap();
    assert!(
        matches!(second, steg_server::Arbitration::Became(_)),
        "the lock should have been released, letting a fresh attempt become the server"
    );
    if let steg_server::Arbitration::Became(handle) = second {
        drop(handle);
    }
    let _ = std::fs::remove_file(&socket_path);
    let _ = std::fs::remove_file(&lock_path);
}
