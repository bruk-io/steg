//! Long-lived worker for TASK-1.112's sequential cross-process control
//! (`tests/openpty_race.rs`): starts, signals ready, waits for one release
//! byte, then opens pseudo-terminals through
//! [`steg_server::RealLauncher::open_only`] in a tight loop - the real
//! lock, the real retry, no `spawn_command` - until told to stop (its own
//! stdin closing) or its own per-worker cap of opens is reached.
//!
//! Only the first open pays this process's own startup cost; every one
//! after that runs in an already-warm process, so several of these
//! overlapping for as long as the test wants is the actual condition the
//! cross-process race needs, not diluted by a fresh `exec` every
//! iteration the way a short-lived, one-shot-per-process design would be
//! (an earlier version of this fixture - see the task's own notes for
//! why it was replaced).
//!
//! Reports each failure the instant it happens (`fail:<opens so far>`)
//! and its own final count when it stops (`done:<total>:<failures>`),
//! both lines flushed immediately - the test reads these as they arrive,
//! not only at the end.

use std::io::{Read, Write};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

fn main() {
    let max_open_attempts: u32 = std::env::var("STEG_PTY_RACE_MAX_ATTEMPTS")
        .expect("openpty-race-attempt: STEG_PTY_RACE_MAX_ATTEMPTS must be set")
        .parse()
        .expect("STEG_PTY_RACE_MAX_ATTEMPTS must be a u32");
    let per_worker_cap: u64 = std::env::var("STEG_PTY_RACE_PER_WORKER_CAP")
        .expect("openpty-race-attempt: STEG_PTY_RACE_PER_WORKER_CAP must be set")
        .parse()
        .expect("STEG_PTY_RACE_PER_WORKER_CAP must be a u64");

    // Signal ready, then block for the test's one release byte - not a
    // sleep, not polling: the test only releases every worker once every
    // one of them has reached this exact point.
    println!("ready");
    std::io::stdout()
        .flush()
        .expect("stdout is a real pipe in this fixture's own test");
    let mut release = [0u8; 1];
    std::io::stdin()
        .read_exact(&mut release)
        .expect("the test writes one release byte to this worker's own stdin");

    // A background thread blocks on whatever is left of stdin: the test
    // closing it (EOF) is the stop signal, read here on its own thread so
    // the open loop below never itself blocks on I/O between iterations -
    // it only ever checks a cheap atomic flag.
    let should_stop = Arc::new(AtomicBool::new(false));
    {
        let should_stop = Arc::clone(&should_stop);
        std::thread::spawn(move || {
            let mut sink = [0u8; 64];
            loop {
                match std::io::stdin().read(&mut sink) {
                    Ok(0) | Err(_) => {
                        should_stop.store(true, Ordering::Relaxed);
                        return;
                    }
                    Ok(_) => {}
                }
            }
        });
    }

    // `histogram[i]` counts opens that succeeded on attempt `i + 1`
    // (`histogram[0]` = succeeded first try, no retry at all) -
    // TASK-1.112's own notes use this to check whether a second attempt
    // fails disproportionately often (the independence assumption behind
    // the shipped bound's own arithmetic), not trusting it. Always
    // reported, even when `max_open_attempts == 1` (then it is always
    // `[total - failures]`, trivially).
    let mut histogram = vec![0u64; max_open_attempts as usize];
    let mut total = 0u64;
    let mut failures = 0u64;
    while !should_stop.load(Ordering::Relaxed) && total < per_worker_cap {
        let (result, attempts) = steg_server::RealLauncher::open_only(max_open_attempts, (80, 24));
        total += 1;
        match result {
            Ok(()) => histogram[(attempts - 1) as usize] += 1,
            Err(_) => {
                failures += 1;
                println!("fail:{total}");
                std::io::stdout()
                    .flush()
                    .expect("stdout is a real pipe in this fixture's own test");
            }
        }
    }

    let histogram_str = histogram
        .iter()
        .map(u64::to_string)
        .collect::<Vec<_>>()
        .join(",");
    println!("done:{total}:{failures}:{histogram_str}");
    std::io::stdout()
        .flush()
        .expect("stdout is a real pipe in this fixture's own test");
}
