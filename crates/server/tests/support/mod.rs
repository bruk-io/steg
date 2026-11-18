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

/// Budget `path`'s length must stay under: `sockaddr_un`'s 104-byte
/// `sun_path` on macOS, minus 4 bytes of slack - `crates/server/src/
/// bind.rs`'s own `temp_name`/`unique_path` and `crates/sdk/src/paths.rs::
/// socket_path` both found, from a real failure, that this budget is
/// tight once `$TMPDIR`'s own length is spent, which is exactly why this
/// module's own directory name below carries no descriptive label.
const SOCKADDR_UN_SAFE_BYTES: usize = 100;

/// One path segment, built from `pid`, a per-process counter and a low-
/// bits slice of the clock - pure, so the property that matters (two
/// calls with the same `pid`/`counter` but a different `clock_low24`
/// never collide) is directly testable, below, with no real process or
/// filesystem involved. No label: the same reason every duplicated
/// `unique_socket_path` this module replaces already dropped one
/// (`accept_backoff.rs`'s own `let _ = label;`, `race.rs`'s own
/// `_label: &str` - `sockaddr_un`'s budget, not this module's own
/// choice).
#[allow(
    dead_code,
    reason = "not every file under tests/ that includes this module calls ScopedTempDir - \
              openpty_race.rs needs only wait_child_bounded"
)]
fn candidate_name(pid: u32, counter: u64, clock_low24: u32) -> String {
    format!("{pid}-{counter:x}-{clock_low24:06x}")
}

/// The low 24 bits of the current time's own subsecond nanoseconds - a
/// compact (6 hex character) clock token, not the full, much longer
/// value: kept short for the same `sockaddr_un` reason `candidate_name`
/// itself is label-free. Not meant to be unique on its own - `counter`
/// already is, within one process - this is what keeps two *different*
/// processes that the OS has assigned the same pid, at different real
/// times, from computing the same name on each one's own first call,
/// which plain `pid`+`counter` (every helper this module replaces) could
/// not rule out.
#[allow(
    dead_code,
    reason = "not every file under tests/ that includes this module calls ScopedTempDir - \
              openpty_race.rs needs only wait_child_bounded"
)]
fn clock_low24() -> u32 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos()
        & 0x00FF_FFFF
}

/// Tries each of `candidates`, in order, as a directory to create fresh
/// under `base`; returns the first one that did not already exist.
/// `std::fs::create_dir` itself is the only I/O here - the retry
/// *decision* (skip a name that collided, stop after too many) is driven
/// entirely by `candidates`, which is why this takes an iterator rather
/// than generating names itself: a test below can hand it a sequence
/// that deliberately collides first, with no reliance on a real pid or
/// the real clock.
///
/// # Panics
/// If 1000 candidates in a row all collided, or a candidate failed to
/// create for any reason other than already existing.
#[allow(
    dead_code,
    reason = "not every file under tests/ that includes this module calls ScopedTempDir - \
              openpty_race.rs needs only wait_child_bounded"
)]
fn create_unique_dir(
    base: &std::path::Path,
    candidates: impl Iterator<Item = String>,
) -> std::path::PathBuf {
    use std::os::unix::fs::PermissionsExt;

    for name in candidates.take(1000) {
        let path = base.join(name);
        match std::fs::create_dir(&path) {
            Ok(()) => {
                // `create_dir` leaves the new directory's mode wherever
                // the *ambient* umask happens to put it - fine on an
                // ordinary machine, but one test in this crate
                // (`socket_mode.rs`) deliberately sets a permissive
                // umask to prove the *socket's* own mode does not
                // depend on it, and a world-writable directory this
                // socket then lives in makes `arbiter::attempt` itself
                // correctly refuse with `UnsafeDirectory(WorldWritable)`
                // - found exactly this way, not assumed safe, when that
                // test's own umask(0) made every directory this helper
                // created inherit it. Set explicitly, the same reason
                // `bind.rs`'s own `bind_with_mode` never trusts the
                // umask for the socket itself: owner-only, nothing else.
                std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700))
                    .unwrap_or_else(|error| {
                        panic!("could not set 0700 on the scoped temp dir at {path:?}: {error}")
                    });
                return path;
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => panic!("could not create a scoped temp dir at {path:?}: {error}"),
        }
    }
    panic!("could not find a free scoped temp dir name under {base:?} after 1000 attempts");
}

/// A freshly created, per-test temporary directory - never a bare flat
/// path in the shared system temp directory, which is what let two real
/// processes that the OS assigned the same pid, at different times,
/// collide on an earlier run's own uncleaned-up socket (TASK-1.123's own
/// notes on the sighting this replaces). Removed on `Drop`, including
/// when a panic unwinds through it, so a socket this directory ever held
/// cannot outlive the test that made it.
///
/// Kept to one path segment, with no descriptive label in it
/// (`candidate_name`'s own doc) - `path`, below, is where a caller that
/// needs a *socket* inside this directory gets its own length checked
/// against `sockaddr_un`'s budget, rather than finding out only once a
/// real `bind()` fails somewhere else.
#[allow(
    dead_code,
    reason = "not every file under tests/ that includes this module calls ScopedTempDir - \
              openpty_race.rs needs only wait_child_bounded"
)]
pub struct ScopedTempDir(std::path::PathBuf);

impl ScopedTempDir {
    /// # Panics
    /// If a unique directory could not be created under the system temp
    /// directory (see [`create_unique_dir`]).
    #[must_use]
    pub fn new() -> Self {
        static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let pid = std::process::id();
        let base = std::env::temp_dir();
        let candidates = std::iter::from_fn(move || {
            let counter = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            Some(candidate_name(pid, counter, clock_low24()))
        });
        Self(create_unique_dir(&base, candidates))
    }

    /// `name` joined onto this directory, asserted to stay within
    /// `sockaddr_un`'s safe budget - loudly, here, rather than failing
    /// opaquely at a real `bind()` call later (`bind.rs`'s own doc
    /// comment: found exactly this way once already).
    ///
    /// # Panics
    /// If the resulting path is over [`SOCKADDR_UN_SAFE_BYTES`].
    #[must_use]
    pub fn path(&self, name: &str) -> std::path::PathBuf {
        let path = self.0.join(name);
        let len = path.as_os_str().len();
        assert!(
            len <= SOCKADDR_UN_SAFE_BYTES,
            "path {path:?} is {len} bytes, over the {SOCKADDR_UN_SAFE_BYTES}-byte safety \
             margin under sockaddr_un's sun_path budget - shorten `name`"
        );
        path
    }
}

impl Default for ScopedTempDir {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for ScopedTempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A fresh [`ScopedTempDir`] and one socket path inside it - the common
/// case every call site this module replaces actually needs. The
/// directory is returned alongside the path, not dropped here: a caller
/// binds it to a name (even an unused one, `_name`, never bare `_`) and
/// keeps it alive for as long as the path must stay valid.
#[allow(
    dead_code,
    reason = "not every file under tests/ that includes this module calls ScopedTempDir - \
              openpty_race.rs needs only wait_child_bounded"
)]
#[must_use]
pub fn scoped_socket_path(name: &str) -> (ScopedTempDir, std::path::PathBuf) {
    let dir = ScopedTempDir::new();
    let path = dir.path(name);
    (dir, path)
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

    use super::{ScopedTempDir, candidate_name, create_unique_dir};

    /// The shape every `unique_socket_path` this module replaces actually
    /// used: `pid` and a per-process counter, nothing else. Two different
    /// real processes that the OS happens to assign the same pid, at
    /// different times, each making this their own first call, compute
    /// this same name - the sighted collision (TASK-1.123's own notes),
    /// forced here directly rather than waited for: a real pid being
    /// reused is not something a test can make happen on demand, but
    /// feeding the identical `(pid, counter)` pair this function would
    /// have been called with on each of those two processes' own first
    /// call is exactly that scenario, named precisely rather than
    /// approximated.
    fn old_scheme_name(pid: u32, counter: u64) -> String {
        format!("{pid}-{counter}")
    }

    #[test]
    fn the_old_pid_and_counter_only_scheme_collides_when_two_processes_share_a_pid() {
        assert_eq!(
            old_scheme_name(12345, 0),
            old_scheme_name(12345, 0),
            "two processes that happen to share a pid, each on their own first call, named \
             the identical path under the scheme this module replaces"
        );
    }

    /// The fix's own property: the same forced `(pid, counter)` collision
    /// above does not collide here, because `clock_low24` differs between
    /// the two calls - not simulated by two real processes (the clock
    /// cannot be forced to match across two separate calls, which is
    /// exactly the point), but shown directly against the pure naming
    /// function with two real, different clock readings.
    #[test]
    fn candidate_name_does_not_collide_under_the_same_forced_pid_and_counter() {
        let first = candidate_name(12345, 0, 0x00_0001);
        let second = candidate_name(12345, 0, 0x00_0002);
        assert_ne!(
            first, second,
            "the same pid and counter must not collide once the clock component differs"
        );
    }

    /// `create_unique_dir`'s own retry, forced deterministically: the
    /// first candidate is made to already exist before this call, with no
    /// real pid or clock involved, so the only thing under test is
    /// "skip a collision, use the next name" - not a real filesystem race.
    #[test]
    fn create_unique_dir_skips_a_name_that_already_exists() {
        let base = std::env::temp_dir();
        let taken = base.join(format!(
            "steg-scoped-temp-dir-test-taken-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&taken);
        std::fs::create_dir(&taken).expect("creating the pre-taken name should succeed");

        let taken_name = taken.file_name().unwrap().to_str().unwrap().to_string();
        let free_name = format!("{taken_name}-free");
        let candidates = vec![taken_name.clone(), free_name.clone()].into_iter();

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            create_unique_dir(&base, candidates)
        }));

        let _ = std::fs::remove_dir_all(&taken);
        let _ = std::fs::remove_dir_all(base.join(&free_name));

        let created = result.expect("create_unique_dir should have skipped the taken name");
        assert_eq!(
            created,
            base.join(&free_name),
            "expected the already-existing candidate to be skipped in favour of the next one"
        );
    }

    /// End to end, through the real public type: a fresh directory really
    /// exists after `new()`, and really does not exist once the guard is
    /// dropped - including when the drop happens while a panic is
    /// unwinding through it, the same guarantee this module's own
    /// `ClosingWriterGuard`-style helpers elsewhere in this task rely on.
    #[test]
    fn scoped_temp_dir_exists_after_new_and_is_gone_after_drop() {
        let dir = ScopedTempDir::new();
        let path = dir.path("s.sock").parent().unwrap().to_path_buf();
        assert!(
            path.is_dir(),
            "expected {path:?} to exist right after new()"
        );
        drop(dir);
        assert!(
            !path.exists(),
            "expected {path:?} to be removed once the guard was dropped"
        );
    }

    /// `create_unique_dir`'s own explicit `0700` is load-bearing, not
    /// decorative - found only by `socket_mode.rs`'s own test failing
    /// *indirectly* (`arbiter::attempt` correctly refusing a world-
    /// writable directory under that test's own permissive umask)
    /// before this was added. Proven here directly, in the same module
    /// `ScopedTempDir` itself lives in: a fresh directory's mode is
    /// exactly `0700`, not merely "whatever this machine's own ambient
    /// umask happens to allow" (typically `0755` for a plain
    /// `create_dir` under the common `0o022` default) - this test does
    /// **not** itself touch the process-wide umask, deliberately:
    /// `socket_mode.rs`'s own doc comment already states why that is
    /// unsafe to do from a module compiled into every other integration
    /// test binary in this crate, most of which run many other tests
    /// concurrently in the same process (`socket_mode.rs` is its own,
    /// single-test binary for exactly this reason). Getting `0700`
    /// under the *ordinary* umask is still a real assertion: it would
    /// be `0755` (or looser) if the explicit `set_permissions` call
    /// were ever removed.
    #[test]
    fn a_fresh_scoped_temp_dir_is_0700() {
        use std::os::unix::fs::PermissionsExt;

        let dir = ScopedTempDir::new();
        let mode = std::fs::metadata(dir.path("s.sock").parent().unwrap())
            .unwrap()
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(
            mode, 0o700,
            "a scoped temp dir's own mode must be 0700, not whatever the ambient umask allows"
        );
    }

    /// Two directories made back to back, in the same process, never
    /// collide with each other either - the ordinary case this whole
    /// module exists to keep true even under this machine's own heavy
    /// concurrent test-binary churn, not only the forced pid-collision
    /// case above.
    #[test]
    fn two_scoped_temp_dirs_made_back_to_back_are_different() {
        let first = ScopedTempDir::new();
        let second = ScopedTempDir::new();
        assert_ne!(first.path("s.sock"), second.path("s.sock"));
    }

    /// `path`'s own safety check, proven to actually fire: a name long
    /// enough to push the result over `SOCKADDR_UN_SAFE_BYTES` panics with
    /// a message naming the length and the budget, rather than being
    /// handed back for a real `bind()` to fail on opaquely later
    /// (`bind.rs`'s own doc comment: found exactly this way once already).
    #[test]
    #[should_panic(expected = "over the 100-byte safety")]
    fn path_panics_rather_than_hand_back_something_over_the_sun_path_budget() {
        let dir = ScopedTempDir::new();
        let _ = dir.path(&"x".repeat(200));
    }
}
