//! Shared bounded-wait helpers for this crate's own integration tests
//! under `tests/` (TASK-1.113). Not shared from
//! `crates/server/src/test_support.rs` - each file under `tests/` is
//! its own compilation unit and cannot import a crate-private module,
//! so this duplicates the same shape rather than hiding the gap.

use std::process::{Child, ExitStatus};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::time::Duration;

use nix::sys::signal::{self, Signal};
use nix::unistd::Pid;

/// Runs `body` on its own thread and waits up to `bound` for it to
/// finish, panicking - naming `what` and `bound` - if it does not. Not a
/// sleep loop: the wait is a channel receive with a timeout
/// (`.claude/rules/testing-rust.md`: "no test sleeps to wait for
/// something; it waits on the event"). `#[allow]`, not `#[expect]`:
/// each file under `tests/` that includes this module (`#[path =
/// "support/mod.rs"]`) is its own separate binary, and not every one of
/// them calls every helper here - `race.rs` only needs
/// `wait_child_bounded`, `openpty_race.rs` needs this one too, and
/// `expect` would warn in whichever binary's build does use it (an
/// "unfulfilled lint expectation" has no cfg this module could gate on
/// per binary).
#[allow(
    dead_code,
    reason = "not every file under tests/ that includes this module calls this helper - \
              race.rs only needs wait_child_bounded"
)]
pub fn run_bounded<T, F>(what: &str, bound: Duration, body: F) -> T
where
    T: Send + 'static,
    F: FnOnce() -> T + Send + 'static,
{
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(body());
    });
    rx.recv_timeout(bound)
        .unwrap_or_else(|_| panic!("timed out after {bound:?} waiting for: {what}"))
}

/// Waits for `child` up to `bound`, naming `label` and panicking if it
/// is crossed. `std::process::Child` has no bounded wait of its own;
/// taking it by value to move the real wait onto another thread (the
/// shape `tests/real_pty.rs`'s own `wait_bounded` already uses, for
/// `portable_pty::Child`) would make every caller here give `child` up
/// and take it back for no benefit over this instead: a watchdog thread
/// races the real, blocking `child.wait()` on the calling thread, and
/// SIGKILLs `child` - by pid, not by owning it - if `bound` elapses
/// first. One timed sleep for the bound itself, not a loop that checks
/// and sleeps again (`.claude/rules/testing-rust.md`'s own "no test
/// sleeps to wait for something; it waits on the event" is about a
/// repeated poll, not a single deadline arm racing the real wait).
pub fn wait_child_bounded(label: &str, child: &mut Child, bound: Duration) -> ExitStatus {
    let pid = Pid::from_raw(child.id() as i32);
    let finished = Arc::new(AtomicBool::new(false));
    let killed = Arc::new(AtomicBool::new(false));
    let watchdog_finished = Arc::clone(&finished);
    let watchdog_killed = Arc::clone(&killed);
    let watchdog = std::thread::spawn(move || {
        std::thread::sleep(bound);
        if !watchdog_finished.load(Ordering::SeqCst) {
            watchdog_killed.store(true, Ordering::SeqCst);
            let _ = signal::kill(pid, Signal::SIGKILL);
        }
    });

    let status = child
        .wait()
        .unwrap_or_else(|error| panic!("{label}: wait() itself failed: {error}"));
    finished.store(true, Ordering::SeqCst);
    // Not joined: the watchdog is only ever woken by its own `sleep`,
    // which cannot be cut short once the real wait above has already
    // finished - joining it here would make every call cost the full
    // `bound`, happy path included (found exactly this way while
    // building this task: `eight_simultaneous_starts_leave_exactly_one_
    // server` went from a few seconds to 80s). Dropping the handle
    // detaches the thread; it still wakes on its own after `bound`,
    // finds `finished` already true, and does nothing - harmless, the
    // same reasoning `run_bounded`'s own doc gives for an abandoned
    // thread.
    drop(watchdog);

    assert!(
        !killed.load(Ordering::SeqCst),
        "{label} did not exit within {bound:?} and was killed"
    );
    status
}
