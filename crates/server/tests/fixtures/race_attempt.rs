//! `crates/server/tests/race.rs`'s own fixture (AC #2): calls
//! `steg_server::attempt` directly against `STEG_SERVER`, prints
//! which `Arbitration` it got - flushed before it would otherwise block
//! in `run` - and, only if it became the server, calls `run` and blocks
//! until killed. Its own argv is unused; everything it needs comes from
//! the environment, the same convention the sdk's own
//! `fake-server-for-tests` fixture already follows for the same reason
//! (no path is ever passed on a real `steg`'s argv either).

use std::io::Write;
use std::path::PathBuf;
use std::time::Duration;

use steg_server::{Arbitration, ServerConfig, Stopper, attempt, run};

fn main() {
    let socket_path = PathBuf::from(
        std::env::var_os("STEG_SERVER").expect("race-attempt: STEG_SERVER must be set"),
    );
    let arbitration_timeout = std::env::var("STEG_ARBITRATION_TIMEOUT_MS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .map(Duration::from_millis)
        .unwrap_or(steg_server::DEFAULT_ARBITRATION_TIMEOUT);
    let config = ServerConfig {
        socket_path: Some(socket_path.clone()),
        arbitration_timeout,
        ..ServerConfig::default()
    };

    let outcome = attempt(&config, &socket_path);
    let handle = match outcome {
        Ok(Arbitration::Became(handle)) => {
            println!("became");
            std::io::stdout()
                .flush()
                .expect("stdout is a real pipe in this fixture's own test");
            handle
        }
        Ok(Arbitration::AlreadyRunning) => {
            println!("already_running");
            return;
        }
        Ok(Arbitration::Refuse(reason)) => {
            println!("refuse:{reason}");
            return;
        }
        Err(err) => {
            println!("error:{err}");
            return;
        }
    };

    // Blocks until the test `kill()`s this process - a graceful stop via
    // `Stopper` is this task's own `shutdown.rs` test's job, not this
    // fixture's: the race test only needs to know who won, and reaps
    // the winner by killing it, not by asking it to stop.
    let stop = Stopper::new();
    let _ = run(handle, &socket_path, &config, &stop, None);
}
