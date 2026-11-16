//! The whole of a "real client" for `server_start.rs`'s own SIGKILL/SIGHUP
//! tests (AC #2, plan section 7): calls `steg_sdk::local_socket::connect`
//! with `autostart: true`, exactly as a real client would, then parks so the
//! test can signal this process (or its whole process group) at a
//! controlled moment once the socket it caused to exist is confirmed up -
//! never prints anything a test needs to read, since the synchronising
//! event is the socket appearing, not this process's own output.

use std::time::Duration;

fn main() {
    // `CARGO_BIN_EXE_steg` only resolves via `env!` inside a `[[test]]`
    // target, not a `[[bin]]` one like this fixture - `server_start.rs`
    // (where it does resolve) passes it along as a plain environment
    // variable instead.
    let steg_binary =
        std::env::var("STEG_TEST_BINARY").expect("the test should set STEG_TEST_BINARY");
    let options = steg_sdk::ConnectOptions {
        binary: std::path::PathBuf::from(steg_binary),
        // `None`: resolved from `STEG_SOCKET_PATH`, which the test sets on
        // this process's own environment before spawning it - the same
        // channel `steg_sdk::local_socket`'s own resolution already reads.
        socket_path: None,
        autostart: true,
        connect_timeout: Duration::from_secs(10),
    };
    let _stream = steg_sdk::connect(&options).expect("autostart should succeed");
    // Parked, not exited: this process (and, for the SIGHUP test, its whole
    // process group) has to still exist for the test to signal it. The
    // signal itself (SIGKILL or SIGHUP) ends this loop from outside; nothing
    // here waits for anything in particular; it is never expected to return.
    loop {
        std::thread::sleep(Duration::from_secs(3600));
    }
}
