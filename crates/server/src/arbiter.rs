//! One server per socket (AC #2), decided by a lock, not by `bind`:
//! binding twice at the same path cannot tell "stale file, nothing
//! listening" apart from "another process already bound here" without
//! first unlinking, and unlink-then-bind is itself the race this exists
//! to settle. [`decide`] is the whole decision, as a pure, total function
//! over one snapshot of readings - exhaustively unit-tested with no I/O.
//! [`attempt`] is the real version: it takes those readings for real,
//! loops [`decide`] while it answers [`Decision::Retry`], and is the only
//! thing in this task that touches `nix`'s `Flock` or binds a socket.

use std::fs::File;
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::thread;
use std::time::{Duration, Instant};

use nix::errno::Errno;
use nix::fcntl::{Flock, FlockArg};

use crate::bind::bind_with_mode;
use crate::config::ServerConfig;
use crate::error::Error;
use crate::paths::{self, ExistingFile};

/// How often `attempt`'s retry loop re-reads the path and re-tries the
/// lock while another process is mid-becoming-the-server. Short, because
/// the thing it is waiting for - a bind, a few syscalls - is itself short
/// (the Risks this plan's own notes name: `arbitration_timeout` must
/// comfortably exceed this, not the other way round).
const POLL_INTERVAL: Duration = Duration::from_millis(20);

/// The connect probe's own result - kept apart from [`ExistingFile`] so
/// [`decide`] holds the whole decision, rather than [`attempt`] quietly
/// making the interesting call ("is a live server actually there")
/// itself before [`decide`] ever sees it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Probe {
    /// Something accepted the connection - read as "a live server of
    /// ours is there", since [`Probe`] is only ever produced when
    /// [`ExistingFile::OursAndASocket`] already confirmed the path is a
    /// socket we own.
    Answered,
    /// Nothing accepted it - the owning process is dead and the file is
    /// stale.
    Refused,
}

/// Whether the arbiter's lock file was taken.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LockOutcome {
    Acquired,
    HeldByOther,
}

/// Why [`decide`] refused outright (never retried).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reason {
    /// The parent directory is not safe to place a socket in
    /// (`paths::directory_is_safe`).
    UnsafeDirectory(paths::Reason),
    /// Something is at the socket's own path and this task does not
    /// trust it (`paths::existing_file`'s `NotOurs`/`NotASocket`).
    UntrustedExistingFile(paths::Reason),
    /// The lock stayed contended for the whole of `arbitration_timeout`
    /// with nothing ever answering - [`attempt`]'s own deadline, not a
    /// case [`decide`] itself can reach (it has no notion of time).
    ArbitrationTimedOut,
}

impl std::fmt::Display for Reason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsafeDirectory(reason) => {
                write!(f, "the socket's directory is not safe: {reason}")
            }
            Self::UntrustedExistingFile(reason) => {
                write!(
                    f,
                    "the existing file at the socket's path is not trusted: {reason}"
                )
            }
            Self::ArbitrationTimedOut => {
                write!(
                    f,
                    "timed out waiting for another process to finish starting a server"
                )
            }
        }
    }
}

/// One snapshot's worth of decision, with no I/O and no notion of time -
/// [`attempt`] is what loops this while it answers [`Decision::Retry`]
/// and turns that loop into [`Reason::ArbitrationTimedOut`] once its own
/// deadline passes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decision {
    /// This process should become the server: bind, and serve.
    Became,
    /// A live server of ours is already there.
    AlreadyRunning,
    /// Ambiguous from this one snapshot - another process holds the lock
    /// and nothing has answered yet. Read fresh and decide again.
    Retry,
    /// Refused outright - never retried.
    Refuse(Reason),
}

/// The whole decision, as a function of what was actually read. A live,
/// answering server always wins over this process's own freshly
/// -acquired lock (the one combination `attempt`'s real loop should
/// never produce in practice - a dead holder's socket stops answering
/// the moment it dies - but `decide` still gives it a definite, safe
/// answer: yield, do not bind a second listener at a path something is
/// already serving).
#[must_use]
pub fn decide(existing: ExistingFile, probe: Option<Probe>, lock: LockOutcome) -> Decision {
    match existing {
        ExistingFile::NotOurs => {
            Decision::Refuse(Reason::UntrustedExistingFile(paths::Reason::NotOurs))
        }
        ExistingFile::NotASocket => {
            Decision::Refuse(Reason::UntrustedExistingFile(paths::Reason::NotASocket))
        }
        ExistingFile::OursAndASocket | ExistingFile::Absent => {
            if probe == Some(Probe::Answered) {
                return Decision::AlreadyRunning;
            }
            match lock {
                LockOutcome::Acquired => Decision::Became,
                LockOutcome::HeldByOther => Decision::Retry,
            }
        }
    }
}

/// What [`attempt`] reports once it has a final answer - the real
/// version of [`Decision`], carrying what [`Decision::Became`] needs a
/// real listener for, with [`Decision::Retry`] resolved by [`attempt`]'s
/// own loop rather than surfaced to its caller.
#[derive(Debug)]
pub enum Arbitration {
    /// This process is now the server: the bound listener and the held
    /// lock, which must stay open for the lifetime of the server
    /// (`listen::run`'s own job - dropping either releases the lock).
    Became(ServerHandle),
    /// A live server of ours is already there.
    AlreadyRunning,
    /// Refused outright.
    Refuse(Reason),
}

/// The bound listener and the held arbiter's lock, kept together because
/// dropping either without the other would leave the socket unprotected:
/// the lock is what makes a dead server's socket safe to remove and
/// rebind at all (`arbiter.rs`'s own doc comment), so nothing holds a
/// listener at this path without also holding this lock. Fields are
/// `pub(crate)`, not `pub`: carrying this across a detaching re-exec
/// needs the raw file descriptors, not the Rust value itself (`Command
/// ::spawn` replaces the process image), and that fd-passing is part B's
/// to add - this type gives this task's own foreground `listen::run` what
/// it needs and no more.
#[derive(Debug)]
pub struct ServerHandle {
    pub(crate) listener: std::os::unix::net::UnixListener,
    /// Destructured, not dropped as part of this whole struct, by
    /// whoever shuts the server down (`listen::run`): a struct's fields
    /// drop in declaration order, which would release this before the
    /// socket file is removed - the exact gap the shutdown order exists
    /// to avoid. Holding it is the point; nothing reads its value.
    pub(crate) lock: Flock<File>,
}

/// Tries to connect to `path` to tell "a live server of ours" apart from
/// "a stale file" - only ever called once `paths::existing_file` has
/// already confirmed the path is a socket this process owns.
fn probe(path: &Path) -> Probe {
    match UnixStream::connect(path) {
        Ok(_stream) => Probe::Answered,
        Err(_) => Probe::Refused,
    }
}

/// Tries the arbiter's lock, non-blocking. `Err` is reserved for a real
/// I/O failure distinct from the lock simply being held (`Errno::EAGAIN`,
/// matched here and turned into [`LockOutcome::HeldByOther`] rather than
/// an error - confirmed against the real crate by running exactly this
/// contention, not assumed).
fn try_lock(lock_path: &Path) -> Result<(LockOutcome, Option<Flock<File>>), Error> {
    let file = File::create(lock_path)?;
    match Flock::lock(file, FlockArg::LockExclusiveNonblock) {
        Ok(locked) => Ok((LockOutcome::Acquired, Some(locked))),
        Err((_file, Errno::EAGAIN)) => Ok((LockOutcome::HeldByOther, None)),
        Err((_file, errno)) => Err(Error::from(std::io::Error::from(errno))),
    }
}

/// One full attempt at becoming the server, or finding one already is.
///
/// # Errors
/// An [`Error::Io`] from a real I/O failure - reading the directory's or
/// the existing file's metadata, opening or locking the lock file for a
/// reason other than contention, or binding.
///
/// # Panics
/// Never in practice: `socket_path` always has a parent directory,
/// because `steg_sdk::socket_path` never returns a bare relative name
/// (the sdk's own `local_socket` provider already depends on this), and
/// `Decision::Became` is only ever reached in the branch of this
/// function's own loop that just matched `LockOutcome::Acquired`, which
/// always carries the guard `Decision::Became`'s own handling expects.
pub fn attempt(config: &ServerConfig, socket_path: &Path) -> Result<Arbitration, Error> {
    let directory = socket_path
        .parent()
        .expect("a socket path always has a parent directory - paths::socket_path never returns a bare relative name");
    if let Err(reason) = paths::directory_is_safe(directory)? {
        return Ok(Arbitration::Refuse(Reason::UnsafeDirectory(reason)));
    }

    let lock_path = paths::lock_path(socket_path);
    let deadline = Instant::now() + config.arbitration_timeout;

    loop {
        let existing = paths::existing_file(socket_path)?;
        let probe_result = match existing {
            ExistingFile::OursAndASocket => Some(probe(socket_path)),
            ExistingFile::Absent | ExistingFile::NotOurs | ExistingFile::NotASocket => None,
        };
        let (lock_outcome, lock_guard) = match existing {
            // Never worth trying the lock for a path this task refuses
            // outright - `decide` reaches the same `Refuse` regardless
            // of `lock`, so `HeldByOther` here is a placeholder never
            // acted on, not a real attempt.
            ExistingFile::NotOurs | ExistingFile::NotASocket => (LockOutcome::HeldByOther, None),
            ExistingFile::Absent | ExistingFile::OursAndASocket => try_lock(&lock_path)?,
        };

        match decide(existing, probe_result, lock_outcome) {
            Decision::Became => {
                let lock = lock_guard.expect("Decision::Became only follows LockOutcome::Acquired, which always carries the guard");
                if existing == ExistingFile::OursAndASocket {
                    // Safe now: this snapshot already proved it is a
                    // socket this process owns and nothing answered it.
                    let _ = std::fs::remove_file(socket_path);
                }
                let listener = bind_with_mode(socket_path)?;
                return Ok(Arbitration::Became(ServerHandle { listener, lock }));
            }
            Decision::AlreadyRunning => return Ok(Arbitration::AlreadyRunning),
            Decision::Refuse(reason) => return Ok(Arbitration::Refuse(reason)),
            Decision::Retry => {
                if Instant::now() >= deadline {
                    return Ok(Arbitration::Refuse(Reason::ArbitrationTimedOut));
                }
                thread::sleep(POLL_INTERVAL);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::PermissionsExt;
    use std::os::unix::net::UnixListener;
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::{Arbitration, Decision, LockOutcome, Probe, Reason, attempt, decide};
    use crate::config::ServerConfig;
    use crate::paths::{self, ExistingFile};

    /// Kept short against `sockaddr_un`'s 104-byte `sun_path` budget -
    /// `bind.rs`'s own doc comment found this matters once a temporary
    /// -name suffix is added on top.
    fn unique_socket_path() -> std::path::PathBuf {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let suffix = COUNTER.fetch_add(1, Ordering::Relaxed);
        // "a" distinguishes this file's own counter from `paths.rs`'s
        // and `bind.rs`'s - see `paths.rs`'s own comment on this.
        std::env::temp_dir().join(format!("{}-a{suffix}.sock", std::process::id()))
    }

    #[test]
    fn a_path_owned_by_someone_else_is_refused_regardless_of_the_lock() {
        assert_eq!(
            decide(ExistingFile::NotOurs, None, LockOutcome::Acquired),
            Decision::Refuse(Reason::UntrustedExistingFile(paths::Reason::NotOurs))
        );
        assert_eq!(
            decide(ExistingFile::NotOurs, None, LockOutcome::HeldByOther),
            Decision::Refuse(Reason::UntrustedExistingFile(paths::Reason::NotOurs))
        );
    }

    #[test]
    fn a_path_that_is_not_a_socket_is_refused_regardless_of_the_lock() {
        assert_eq!(
            decide(ExistingFile::NotASocket, None, LockOutcome::Acquired),
            Decision::Refuse(Reason::UntrustedExistingFile(paths::Reason::NotASocket))
        );
        assert_eq!(
            decide(ExistingFile::NotASocket, None, LockOutcome::HeldByOther),
            Decision::Refuse(Reason::UntrustedExistingFile(paths::Reason::NotASocket))
        );
    }

    #[test]
    fn an_answering_socket_we_own_is_already_running_even_holding_the_lock() {
        assert_eq!(
            decide(
                ExistingFile::OursAndASocket,
                Some(Probe::Answered),
                LockOutcome::Acquired
            ),
            Decision::AlreadyRunning
        );
    }

    #[test]
    fn an_answering_socket_we_own_is_already_running_when_the_lock_is_held_by_another() {
        assert_eq!(
            decide(
                ExistingFile::OursAndASocket,
                Some(Probe::Answered),
                LockOutcome::HeldByOther
            ),
            Decision::AlreadyRunning
        );
    }

    #[test]
    fn a_refused_socket_we_own_becomes_the_server_once_the_lock_is_acquired() {
        assert_eq!(
            decide(
                ExistingFile::OursAndASocket,
                Some(Probe::Refused),
                LockOutcome::Acquired
            ),
            Decision::Became
        );
    }

    #[test]
    fn a_refused_socket_we_own_retries_while_the_lock_is_held_by_another() {
        assert_eq!(
            decide(
                ExistingFile::OursAndASocket,
                Some(Probe::Refused),
                LockOutcome::HeldByOther
            ),
            Decision::Retry
        );
    }

    #[test]
    fn nothing_there_becomes_the_server_once_the_lock_is_acquired() {
        assert_eq!(
            decide(ExistingFile::Absent, None, LockOutcome::Acquired),
            Decision::Became
        );
    }

    #[test]
    fn nothing_there_retries_while_the_lock_is_held_by_another() {
        assert_eq!(
            decide(ExistingFile::Absent, None, LockOutcome::HeldByOther),
            Decision::Retry
        );
    }

    #[test]
    fn attempt_becomes_the_server_when_nothing_is_there() {
        let path = unique_socket_path();
        let config = ServerConfig::default();
        match attempt(&config, &path).unwrap() {
            Arbitration::Became(handle) => {
                let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
                assert_eq!(mode, 0o600);
                drop(handle);
            }
            other @ (Arbitration::AlreadyRunning | Arbitration::Refuse(_)) => {
                panic!("expected Became, got something else: {other:?}")
            }
        }
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(paths::lock_path(&path));
    }

    #[test]
    fn attempt_finds_a_real_live_listener_already_running() {
        let path = unique_socket_path();
        let listener = UnixListener::bind(&path).unwrap();
        let config = ServerConfig::default();
        match attempt(&config, &path).unwrap() {
            Arbitration::AlreadyRunning => {}
            other @ (Arbitration::Became(_) | Arbitration::Refuse(_)) => {
                panic!("expected AlreadyRunning, got something else: {other:?}")
            }
        }
        drop(listener);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn attempt_takes_over_a_stale_socket_left_by_a_dead_server() {
        let path = unique_socket_path();
        {
            // binds and is dropped without ever being removed - the
            // directory entry outlives the listener, standing in for a
            // process that died without cleaning up.
            let _listener = UnixListener::bind(&path).unwrap();
        }
        let config = ServerConfig::default();
        match attempt(&config, &path).unwrap() {
            Arbitration::Became(handle) => drop(handle),
            other @ (Arbitration::AlreadyRunning | Arbitration::Refuse(_)) => {
                panic!("expected Became, got something else: {other:?}")
            }
        }
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(paths::lock_path(&path));
    }

    #[test]
    fn attempt_refuses_a_world_writable_directory() {
        let dir = unique_socket_path(); // reused as a unique directory name
        std::fs::create_dir(&dir).unwrap();
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o777)).unwrap();
        let path = dir.join("steg.sock");
        let config = ServerConfig::default();
        let result = attempt(&config, &path).unwrap();
        assert!(
            matches!(
                result,
                Arbitration::Refuse(Reason::UnsafeDirectory(paths::Reason::WorldWritable))
            ),
            "expected Refuse(UnsafeDirectory(WorldWritable)), got {result:?}"
        );
        let _ = std::fs::remove_dir(&dir);
    }
}
