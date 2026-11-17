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

use nix::libc;
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

/// Blocks until `pid` has exited, without reaping it: the process stays
/// a zombie, which still holds its own pid - nothing else can be
/// assigned that number - until something later calls `wait()` on it.
///
/// `nix::sys::wait::waitid` excludes apple targets in its own `cfg`
/// (checked by reading `nix-0.28.0/src/sys/wait.rs`): not a real
/// platform limit, since Darwin's own `waitid(2)` documents `WNOWAIT`
/// and the `libc` crate already binds it there (checked the same way).
/// Goes through `nix::libc` - nix's own public re-export of the crate
/// it wraps (`pub use libc;`, `nix-0.28.0/src/lib.rs`) - rather than
/// adding a direct `libc` dependency for one call.
///
/// # Safety invariant this exists to establish
/// `wait_child_bounded`, below, needs to know a child has exited before
/// it is safe for its own watchdog thread to signal that pid - signalling
/// an already-*reaped* pid risks hitting a different, unrelated process
/// the OS has since reused that same number for. Waiting with `WNOWAIT`
/// first means the watchdog can only ever observe this child either
/// still running or exited-but-still-a-zombie, in which case it still
/// unambiguously owns its own pid - never reaped, so never reused -
/// which is the one invariant that makes a kill from a second thread
/// always safe here, however the two threads happen to interleave.
///
/// `unsafe`: `waitid` only ever reads the exit status of a pid this
/// process already owns as a direct child into a `siginfo_t` this call
/// owns on its own stack - no aliasing, no lifetime beyond this call.
/// Retrying on `EINTR` is the ordinary way to handle an interrupted
/// blocking syscall, not a poll for the exit itself.
fn wait_for_exit_without_reaping(pid: libc::pid_t, label: &str) {
    loop {
        // SAFETY: a zeroed `siginfo_t` is always a valid bit pattern for this
        // plain-data, no-destructor libc struct - every platform's own `waitid`
        // only ever reads whichever fields the call below actually fills in.
        let mut siginfo: libc::siginfo_t = unsafe { std::mem::zeroed() };
        // SAFETY: `&mut siginfo` is a valid, uniquely-owned pointer for the
        // call's own duration (never escapes this block), and `pid` names a
        // process this function's own caller already owns as a direct child
        // - `waitid` only ever reads that child's own exit status into it.
        let rc = unsafe {
            libc::waitid(
                libc::P_PID,
                pid as libc::id_t,
                &mut siginfo,
                libc::WEXITED | libc::WNOWAIT,
            )
        };
        if rc == 0 {
            return;
        }
        let error = std::io::Error::last_os_error();
        if error.raw_os_error() == Some(libc::EINTR) {
            continue;
        }
        panic!("{label}: waitid() itself failed: {error}");
    }
}

/// Waits for `child` up to `bound`, naming `label` and panicking if it
/// is crossed. `std::process::Child` has no bounded wait of its own;
/// taking it by value to move the real wait onto another thread (the
/// shape `tests/real_pty.rs`'s own `wait_bounded` already uses, for
/// `portable_pty::Child`) would still need some way to signal it from
/// outside that thread once `bound` is crossed, which is exactly the
/// same pid-signalling question this function answers directly, for no
/// benefit over keeping `child` as `&mut` here.
///
/// A watchdog thread races the real wait: it sleeps for `bound`, then -
/// only if the child has not been confirmed exited yet - sends
/// `SIGKILL`. The calling thread itself blocks in
/// [`wait_for_exit_without_reaping`] (not a sleep-poll: one blocking
/// call, woken by the real event), marks the child as detected once
/// that returns, *then* reaps it with the ordinary, now-fast
/// `child.wait()`. Signalling only ever reaches this process's own,
/// still-unreaped child (see that function's own doc for why) - the
/// watchdog thread is never joined, for the same reason `run_bounded`'s
/// own doc gives: once its `sleep` finally ends, it finds the flag
/// already set and does nothing, which is harmless, and joining it
/// would cost every call the *whole* bound even on the happy path
/// (found exactly this way while building this task:
/// `eight_simultaneous_starts_leave_exactly_one_server` went from a
/// fraction of a second to 80s before this was caught).
pub fn wait_child_bounded(label: &str, child: &mut Child, bound: Duration) -> ExitStatus {
    let raw_pid = child.id() as libc::pid_t;
    let pid = Pid::from_raw(raw_pid);

    let detected = Arc::new(AtomicBool::new(false));
    let timed_out = Arc::new(AtomicBool::new(false));
    let watchdog_detected = Arc::clone(&detected);
    let watchdog_timed_out = Arc::clone(&timed_out);
    let watchdog = std::thread::spawn(move || {
        std::thread::sleep(bound);
        if !watchdog_detected.load(Ordering::SeqCst) {
            watchdog_timed_out.store(true, Ordering::SeqCst);
            let _ = signal::kill(pid, Signal::SIGKILL);
        }
    });

    wait_for_exit_without_reaping(raw_pid, label);
    detected.store(true, Ordering::SeqCst);

    let status = child
        .wait()
        .unwrap_or_else(|error| panic!("{label}: wait() itself failed: {error}"));
    drop(watchdog);

    assert!(
        !timed_out.load(Ordering::SeqCst),
        "{label} did not exit within {bound:?} and was killed"
    );
    status
}

#[cfg(test)]
mod tests {
    use super::wait_child_bounded;
    use std::process::Command;
    use std::time::{Duration, Instant};

    /// The timeout path itself: a real child that ignores nothing short
    /// of `SIGKILL` and outlives the bound must still be killed and
    /// reaped promptly - `wait_child_bounded` panics in exactly this
    /// case (its own doc), so this catches that panic rather than
    /// inspecting a returned status, then checks what matters: the
    /// child is genuinely gone afterward, not merely reported as killed
    /// while still lingering as a zombie or worse.
    #[test]
    fn wait_child_bounded_kills_and_reaps_a_child_that_outlives_the_bound() {
        let mut child = Command::new("sleep")
            .arg("30")
            .spawn()
            .expect("spawning `sleep 30` should not fail");
        let pid = child.id();
        let bound = Duration::from_millis(300);

        let started = Instant::now();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            wait_child_bounded("a sleep that outlives its bound", &mut child, bound)
        }));
        let elapsed = started.elapsed();

        assert!(
            result.is_err(),
            "expected wait_child_bounded to panic once its own bound was crossed"
        );
        assert!(
            elapsed < Duration::from_secs(5),
            "expected close to the {bound:?} bound, took {elapsed:?}"
        );

        // Signal 0 only checks whether the pid still exists - `ESRCH`
        // means it is gone, not merely a zombie waiting to be reaped
        // (which `wait_child_bounded`'s own `child.wait()` already did,
        // before the panic above).
        let still_there = nix::sys::signal::kill(nix::unistd::Pid::from_raw(pid as i32), None);
        assert_eq!(
            still_there,
            Err(nix::errno::Errno::ESRCH),
            "expected the child to be fully gone after being reaped, got {still_there:?}"
        );
    }
}
