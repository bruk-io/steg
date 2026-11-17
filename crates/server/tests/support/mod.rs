//! Shared bounded-wait helpers for this crate's own integration tests
//! under `tests/` (TASK-1.113). Not shared from
//! `crates/server/src/test_support.rs` - each file under `tests/` is
//! its own compilation unit and cannot import a crate-private module,
//! so this duplicates the same shape rather than hiding the gap.

use std::process::{Child, ExitStatus};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex, PoisonError};
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
/// # What this gives `wait_child_bounded`, below
/// Signalling an already-*reaped* pid risks hitting a different,
/// unrelated process the OS has since reused that same number for.
/// `WNOWAIT` is half of what rules that out: it leaves an exited child
/// a zombie, still owning its own pid - never reaped, so never reused -
/// right up until something later actually reaps it, so a kill sent
/// before that reap cannot land on another process. That is necessary,
/// not sufficient, on its own: it says nothing about *when* the reap
/// happens relative to a concurrent kill decision, only that the pid is
/// safe to signal until the reap. The other half - ordering the kill
/// before the reap even when they race - is `wait_child_bounded`'s own
/// lock; see its doc comment for the invariant the two together
/// establish, not restated here.
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
/// # The invariant that makes the watchdog's kill safe
/// A watchdog thread races the real wait: it sleeps for `bound`, then
/// checks-and-kills under `finished`'s own lock. That check and that
/// kill happen as one critical section - never a load followed by a
/// separate store, which is what the first version of this function got
/// wrong (a review caught it): the watchdog takes the lock, and only
/// while still holding it does it decide whether to signal, so nothing
/// else can run between "decided not finished yet" and "sent the kill".
///
/// The calling thread's own side of the same lock is what closes the
/// race: it blocks in [`wait_for_exit_without_reaping`] first - the
/// child becomes a zombie, still owning its own pid, the moment that
/// returns, *before* anything here touches the lock - then takes the
/// *same* lock to set `finished = true`, and only *after* releasing it
/// does it call the ordinary, now-fast `child.wait()` to actually reap.
/// A `Mutex` serialises every acquisition, so exactly one of two things
/// is true for any given moment the watchdog might wake: either it
/// acquires the lock first (the calling thread hasn't set `finished`
/// yet, so the reap hasn't happened yet either - the kill lands on a
/// still-owned, at-worst-zombie child), or the calling thread already
/// has or had the lock (in which case the watchdog, once it finally
/// acquires it, sees `finished == true` and sends nothing). A kill can
/// only be sent while holding the lock with `finished` still false; a
/// reap can only happen after the calling thread has held that same
/// lock and set `finished` - so a kill can never follow a reap, however
/// the two threads interleave. Nothing blocking happens while the lock
/// is held (the kill itself is non-blocking; `waitid` and the real
/// `wait()` both run outside it), so there is no deadlock risk either.
/// A poisoned lock is handled, not panicked on, in both places -
/// neither critical section here can itself panic, but recovering the
/// guard rather than unwrapping costs nothing and keeps a poison
/// anywhere else from turning this helper into a second failure.
///
/// The watchdog thread is never joined, for the same reason
/// `run_bounded`'s own doc gives: once its `sleep` finally ends, it
/// either already lost the lock race (nothing to do) or still runs its
/// own quick critical section and returns - either way abandoning the
/// `JoinHandle` costs nothing, and joining it would cost every call the
/// *whole* bound even on the happy path (found exactly this way while
/// building this task: `eight_simultaneous_starts_leave_exactly_one_
/// server` went from a fraction of a second to 80s before this was
/// caught).
///
/// A deterministic test of the closed interleaving itself is not
/// practical - the race this fixes is a window measured in nothing a
/// test could reliably hold open on demand. This rests on the invariant
/// above and the `Mutex`'s own documented semantics, not on a test that
/// forces the two threads through a particular order.
pub fn wait_child_bounded(label: &str, child: &mut Child, bound: Duration) -> ExitStatus {
    let raw_pid = child.id() as libc::pid_t;
    let pid = Pid::from_raw(raw_pid);

    let finished = Arc::new(Mutex::new(false));
    let timed_out = Arc::new(AtomicBool::new(false));
    let watchdog_finished = Arc::clone(&finished);
    let watchdog_timed_out = Arc::clone(&timed_out);
    let watchdog = std::thread::spawn(move || {
        std::thread::sleep(bound);
        let finished = watchdog_finished
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if !*finished {
            watchdog_timed_out.store(true, Ordering::SeqCst);
            let _ = signal::kill(pid, Signal::SIGKILL);
        }
        // `finished` is intentionally left as it was: this thread only
        // ever decides whether to kill, never claims to have reaped.
    });

    wait_for_exit_without_reaping(raw_pid, label);
    {
        let mut finished = finished.lock().unwrap_or_else(PoisonError::into_inner);
        *finished = true;
        // Released here, strictly before the real reap below - the
        // watchdog can only ever see `true` once this has happened, and
        // can only ever act while this has *not* happened, never both.
    }

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
