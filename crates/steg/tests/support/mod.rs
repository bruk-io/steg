//! `ScopedTempDir`, shared across this crate's own integration tests
//! under `tests/` (TASK-1.123). Not shared from `crates/server/tests/
//! support/mod.rs` - each file under `tests/` is its own compilation
//! unit, in its own crate, and cannot import a module from a sibling
//! crate's own test tree, so this duplicates the identical shape rather
//! than hiding the gap (the same reasoning that module's own doc comment
//! already states for `run_bounded`/`wait_child_bounded`). This crate's
//! own tests do not need those two - just the temporary-path helper.

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
/// `unique_socket_path`/`unique_output_path` this module replaces
/// already dropped one (`cli_exit_statuses.rs`'s own `label: &str`,
/// discarded before building the path - `sockaddr_un`'s budget, not this
/// module's own choice).
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
                // ordinary machine, but `crates/server/tests/
                // socket_mode.rs` deliberately sets a permissive umask
                // to prove a socket's own mode does not depend on it,
                // and a world-writable directory a socket then lives in
                // makes `arbiter::attempt` itself correctly refuse with
                // `UnsafeDirectory(WorldWritable)` - found exactly this
                // way, not assumed safe. Set explicitly, the same reason
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
/// notes on the sighting this replaces - the exact failure was
/// `server_start.rs`'s own `an_untrusted_existing_file_refuses_with_
/// status_4_and_names_the_reason`, writing a plain file where a socket
/// from an unrelated, earlier run was still sitting). Removed on `Drop`,
/// including when a panic unwinds through it, so a socket - or output
/// file, or anything else a test puts here - cannot outlive the test
/// that made it.
///
/// Kept to one path segment, with no descriptive label in it
/// (`candidate_name`'s own doc) - `path`, below, is where a caller that
/// needs a *socket* inside this directory gets its own length checked
/// against `sockaddr_un`'s budget, rather than finding out only once a
/// real `bind()` fails somewhere else.
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
    /// comment: found exactly this way once already). Also used for a
    /// non-socket path (an output file, `login_environment.rs`'s own
    /// second use) - the same length check costs nothing there and keeps
    /// one method for everything this directory hands out.
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
    reason = "not every file under tests/ that includes this module calls \
              scoped_socket_path - login_environment.rs needs two different paths from \
              one ScopedTempDir, so it calls ScopedTempDir::new()/path() directly"
)]
#[must_use]
pub fn scoped_socket_path(name: &str) -> (ScopedTempDir, std::path::PathBuf) {
    let dir = ScopedTempDir::new();
    let path = dir.path(name);
    (dir, path)
}

#[cfg(test)]
mod tests {
    use super::{ScopedTempDir, candidate_name, create_unique_dir};

    /// The shape every `unique_socket_path`/`unique_output_path` this
    /// module replaces actually used: `pid` and a per-process counter,
    /// nothing else. Two different real processes that the OS happens to
    /// assign the same pid, at different times, each making this their
    /// own first call, compute this same name - the sighted collision
    /// (TASK-1.123's own notes), forced here directly rather than waited
    /// for: a real pid being reused is not something a test can make
    /// happen on demand, but feeding the identical `(pid, counter)` pair
    /// this function would have been called with on each of those two
    /// processes' own first call is exactly that scenario, named
    /// precisely rather than approximated.
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
    /// unwinding through it.
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
    /// decorative - found only by `crates/server/tests/socket_mode.rs`'s
    /// own test failing *indirectly* (`arbiter::attempt` correctly
    /// refusing a world-writable directory under that test's own
    /// permissive umask) before this was added. Proven here directly:
    /// a fresh directory's mode is exactly `0700`, not merely "whatever
    /// this machine's own ambient umask happens to allow" (typically
    /// `0755` for a plain `create_dir` under the common `0o022`
    /// default) - this test does **not** itself touch the process-wide
    /// umask, deliberately: that test's own doc comment already states
    /// why that is unsafe from a module compiled into every other
    /// integration test binary in a crate, most of which run many
    /// other tests concurrently in the same process. Getting `0700`
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

    /// One directory, two different names inside it - `login_environment.
    /// rs`'s own case, a socket path and an output path from the same
    /// `ScopedTempDir`.
    #[test]
    fn one_scoped_temp_dir_can_hand_out_two_different_paths() {
        let dir = ScopedTempDir::new();
        let socket_path = dir.path("s.sock");
        let output_path = dir.path("o.out");
        assert_ne!(socket_path, output_path);
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
