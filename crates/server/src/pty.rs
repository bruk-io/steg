//! The pseudo-terminal and the process running in it (doc-16 R1: "the
//! server is where I/O meets the kernel"; TASK-1.40).
//!
//! [`ProgramLauncher`] is the interface the brief means by "an interface
//! the server supplies": real I/O behind it ([`RealLauncher`], over
//! `portable-pty`), so the orchestration in `crate::pane` is driven by a
//! fake at the boundary in tests instead of a real OS process every time
//! (`.claude/rules/testing-rust.md`). Opening the pty and spawning the
//! process both happen inside one call to [`ProgramLauncher::spawn`], on
//! whichever thread calls it - `portable-pty`'s own master, writer and
//! child handles are `Send` (decision-3 section 7's probe: only the
//! terminal engine is pinned to its own thread), so a bad command is a
//! plain `Result` here, never an error that has to escape a spawned
//! thread.

use std::ffi::OsString;
use std::io::{Read, Write};

use crate::error::Error;

/// What to run, and in what, for one pseudo-terminal.
///
/// Built by the caller from a pane's stored `steg_sdk::Program` (resolving
/// a plain shell to a concrete `argv` is the caller's job - see
/// `crate::pane::start`), its environment (`crate::env::pane_environment`)
/// and its size - never computed inside [`ProgramLauncher::spawn`] itself,
/// so a functional test can hand a `FakeLauncher` a `LaunchSpec` with no
/// real socket, no real installed binary and no real OS process at all.
#[derive(Debug, Clone)]
pub struct LaunchSpec {
    /// The program and its arguments, in the order given on the command
    /// line: `argv[0]` is the program to run.
    pub argv: Vec<String>,
    /// Its working directory.
    pub cwd: String,
    /// The pane's *whole* environment (TASK-1.87) - never "the overrides
    /// on top of whatever this process inherits," as it was before this
    /// task: [`RealLauncher::spawn`] clears `portable_pty::CommandBuilder`'s
    /// own ambient seeding (`.env_clear()`) before applying this, which is
    /// what makes "a pane's program starts with the login shell's own
    /// environment, not the server's" (doc-8 R99) actually true rather
    /// than only true for the five `STEG_*` keys. The caller is
    /// responsible for the whole thing: the captured login shell's own
    /// environment first, then the `STEG_*` overrides and `TERM`
    /// (`crate::runtime::Runtime::launch_spec`) - a key given twice wins
    /// on whichever entry is later in this `Vec`, by construction (later
    /// `.env()` calls overwrite earlier ones for the same key).
    pub env: Vec<(OsString, OsString)>,
    /// The pseudo-terminal's size, in columns and rows.
    pub size: (u16, u16),
}

/// What spawning a [`LaunchSpec`] gives back: the pieces a pane's worker
/// thread needs, already split so each can move to a different thread -
/// the writer and the resize closure stay on the pane's own thread, the
/// reader and the child move to the short relay thread `crate::pane`
/// spawns for this one run.
pub struct SpawnedProgram {
    /// Writes to the program's standard input.
    pub writer: Box<dyn Write + Send>,
    /// Reads the program's output. Reaches end-of-file once the program
    /// has exited and nothing else holds the pty's slave side open.
    pub reader: Box<dyn Read + Send>,
    /// Resizes the pseudo-terminal.
    pub resize: Box<dyn Fn(u16, u16) -> Result<(), Error> + Send>,
    /// A handle to wait for the program's exit code, or to kill it.
    pub child: Box<dyn portable_pty::Child + Send + Sync>,
    /// How many attempts opening this pane's pseudo-terminal took
    /// (TASK-1.112) - `1` means it succeeded on the first try, with no
    /// retry. `crate::pane::start`/`PaneHandle::restart` read this to
    /// report [`crate::pane::PaneEvent::OpenRetried`] when it is more
    /// than `1`, so a retry is never silent.
    pub open_attempts: u32,
}

/// Starts a program in a pseudo-terminal (doc-9 R13).
pub trait ProgramLauncher: Send + Sync {
    /// # Errors
    ///
    /// Returns [`Error::Pty`] if the pseudo-terminal could not be opened,
    /// or the program could not be spawned in it (for instance, the
    /// command does not exist).
    fn spawn(&self, spec: &LaunchSpec) -> Result<SpawnedProgram, Error>;
}

/// Serialises every real `openpty()` call this process makes (TASK-1.76
/// review, second round): measured directly on macOS, 2026-11-15 - a
/// bare call to libc's `openpty` from many threads of one process at
/// once, with no Rust and none of this crate's own code loaded, fails
/// intermittently with the platform's own `Os { code: -6 }`, while the
/// number of ptys actually alive stays well under any real ceiling (the
/// task's own review record has the isolated reproduction). The open is
/// not safe to call concurrently *within one process* on this platform;
/// nothing short of serialising it removes that part of the race, so
/// this stays a lock around the call itself, deterministic and free,
/// rather than left to the retry below to absorb along with everything
/// else (TASK-1.112's own investigation: a sustained, lock-protected,
/// otherwise-identical run still failed once in 1,277 opens with the
/// lock removed, and zero times in several thousand with it in place -
/// kept for that reason, not reopened).
///
/// Held only for the retried `openpty` call below, never across the
/// spawn or anything that waits: `portable-pty`'s own
/// `PtyPair::slave.spawn_command` is a separate call this lock does not
/// need to cover, and nothing after the open blocks on I/O.
static OPENPTY_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Retries `try_open` on *any* error it returns, up to `max_attempts`
/// attempts total, with no delay between them - a plain function
/// parameter, not a mock of anything (`.claude/rules/testing-rust.md`),
/// so this is driven directly by unit tests below with no real pty
/// involved at all. Returns the attempt count alongside whichever
/// `Result` the last call to `try_open` produced: `Ok((value, n))` where
/// `n` is the attempt that finally succeeded (`1` means no retry was
/// needed), `Err((error, max_attempts))` once every attempt has failed.
fn open_with_retry<T, E>(
    max_attempts: u32,
    mut try_open: impl FnMut() -> Result<T, E>,
) -> Result<(T, u32), (E, u32)> {
    let mut attempt = 0;
    loop {
        attempt += 1;
        match try_open() {
            Ok(value) => return Ok((value, attempt)),
            Err(_) if attempt < max_attempts => continue,
            Err(error) => return Err((error, attempt)),
        }
    }
}

/// `OPENPTY_LOCK` plus the bounded retry (TASK-1.112): a real,
/// *cross-process* race on this platform (measured directly - a bare
/// `openpty()` from several separate, single-threaded OS processes,
/// nothing shared between them, still fails intermittently at a rate
/// that scales with how many processes are hammering at once) cannot be
/// reached by any lock local to this process, and a real server is one
/// process sharing this platform's pty subsystem with whatever else is
/// running on the same machine at that moment. Retries on any error
/// (not a specific one): `portable-pty` discards the structured
/// `io::Error` before it reaches this crate (its own `unix.rs`:
/// `bail!("failed to openpty: {:?}", io::Error::last_os_error())` - a
/// formatted string, not something a `match` can reach), and the two
/// sightings of this failure recorded on the task do not even agree on
/// the error's sign, so there is nothing reliable left to match. No
/// sleep between attempts - the race this retries clears on the very
/// next call, not after a wait (see the task's own notes for the
/// measurement this rests on).
///
/// Shared by [`RealLauncher::spawn`] and [`RealLauncher::open_only`]
/// (test support, below) - the one path either ever opens a real pty
/// through, so both call the real lock and the real retry, never a
/// hand-rolled stand-in.
fn open_pty(
    max_open_attempts: u32,
    cols: u16,
    rows: u16,
) -> (
    Result<portable_pty::PtyPair, Box<dyn std::error::Error + Send + Sync>>,
    u32,
) {
    let pty_system = portable_pty::native_pty_system();
    let _guard = OPENPTY_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    match open_with_retry(max_open_attempts, || {
        pty_system.openpty(portable_pty::PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })
    }) {
        Ok((pair, attempts)) => (Ok(pair), attempts),
        Err((cause, attempts)) => (Err(cause.into()), attempts),
    }
}

/// How many times [`RealLauncher::default`] retries a failed `openpty()`
/// call in total (TASK-1.112): from the measured cross-process rate
/// (worst local figure, ~0.4% of opens failing under heavy concurrent
/// contention, treated as independent per attempt - see the task's own
/// notes for the measurement that checks that assumption instead of
/// trusting it), the chance all 5 attempts fail at that same density is
/// about `0.004⁵ ≈ 1×10⁻¹²` per open. An extra attempt costs a real
/// server a few more syscalls only on the rare open that needed one at
/// all; it is never on the path of an open that succeeds first try.
const DEFAULT_OPENPTY_MAX_ATTEMPTS: u32 = 5;

/// The real implementation, over `portable-pty`. Mirrors
/// `spikes/task-1.2-wire-protocol/probe-server/src/pty.rs`'s own shape,
/// which already proved this reads cleanly against this exact problem.
#[derive(Debug, Clone, Copy)]
pub struct RealLauncher {
    max_open_attempts: u32,
}

impl RealLauncher {
    /// `max_open_attempts` must be at least 1 (meaning "no retry") - a
    /// test builds `RealLauncher::new(1)` directly to get today's
    /// un-retried behaviour, for instance the race fixture's own
    /// permanent negative control (TASK-1.112).
    ///
    /// # Panics
    ///
    /// Panics if `max_open_attempts` is `0`.
    #[must_use]
    pub fn new(max_open_attempts: u32) -> Self {
        assert!(
            max_open_attempts >= 1,
            "max_open_attempts must be at least 1 (0 would mean \"never try to open\"), got {max_open_attempts}"
        );
        Self { max_open_attempts }
    }

    /// Test support only (TASK-1.112): exactly the locked, retried open
    /// step [`RealLauncher::spawn`] itself uses - no `spawn_command`
    /// after it, closing both sides immediately on success so a caller
    /// looping this does not accumulate real file descriptors. `pub`
    /// because the fixture that proves the cross-process race
    /// (`tests/fixtures/openpty_race_attempt.rs`) is a separate
    /// compilation unit in this same crate, the same reason
    /// `race_attempt.rs` already calls several otherwise-internal
    /// pieces of this crate directly; not meant for any other caller.
    /// Returns how many attempts this one open took either way, so a
    /// caller can tabulate the distribution across many opens (the
    /// measurement named above, and the race fixture's own counts).
    pub fn open_only(max_open_attempts: u32, size: (u16, u16)) -> (Result<(), String>, u32) {
        let (cols, rows) = size;
        match open_pty(max_open_attempts, cols, rows) {
            (Ok(pair), attempts) => {
                drop(pair);
                (Ok(()), attempts)
            }
            (Err(cause), attempts) => (Err(cause.to_string()), attempts),
        }
    }

    /// How many attempts this launcher retries a failed open before
    /// giving up - the same bound `RealLauncher::new` was given, or
    /// `Default`'s own shipped value. `pub` so the cross-process proof
    /// test (`tests/openpty_race.rs`) can drive its fixture at exactly
    /// production's own default, from one source, rather than
    /// hardcoding the number a second time where it could drift out of
    /// sync (TASK-1.112).
    #[must_use]
    pub fn max_open_attempts(&self) -> u32 {
        self.max_open_attempts
    }
}

impl Default for RealLauncher {
    fn default() -> Self {
        Self::new(DEFAULT_OPENPTY_MAX_ATTEMPTS)
    }
}

impl ProgramLauncher for RealLauncher {
    fn spawn(&self, spec: &LaunchSpec) -> Result<SpawnedProgram, Error> {
        let (cols, rows) = spec.size;
        let (pair, open_attempts) = match open_pty(self.max_open_attempts, cols, rows) {
            (Ok(pair), attempts) => (pair, attempts),
            (Err(cause), attempts) => {
                return Err(Error::Pty(
                    format!("failed to openpty after {attempts} attempt(s): {cause}").into(),
                ));
            }
        };

        let mut command =
            portable_pty::CommandBuilder::from_argv(spec.argv.iter().map(OsString::from).collect());
        command.cwd(&spec.cwd);
        // `.env_clear()` (TASK-1.87, doc-8 R99): without this, the ambient
        // environment `CommandBuilder::from_argv` seeds from on its own
        // (this *server* process's own environment, itself inherited from
        // whatever client happened to start it) would still reach every
        // pane underneath `spec.env`'s own entries - exactly the leak R99
        // forbids. `spec.env` is now the pane's whole environment, not an
        // overlay (this struct's own doc comment).
        command.env_clear();
        for (key, value) in &spec.env {
            command.env(key, value);
        }

        let child = pair
            .slave
            .spawn_command(command)
            .map_err(|cause| Error::Pty(cause.into()))?;
        // Dropping the slave is what lets the program's own exit reach the
        // reader as end-of-file: the pty's slave side only closes once
        // nothing holds it open, and this process's own copy is the thing
        // standing in the way (mirrors the spike's `pty.rs`: "The
        // program ended. The pane and its screen stay: that is R2").
        drop(pair.slave);

        let reader = pair
            .master
            .try_clone_reader()
            .map_err(|cause| Error::Pty(cause.into()))?;
        let writer = pair
            .master
            .take_writer()
            .map_err(|cause| Error::Pty(cause.into()))?;
        let master = pair.master;
        let resize: Box<dyn Fn(u16, u16) -> Result<(), Error> + Send> =
            Box::new(move |cols, rows| {
                master
                    .resize(portable_pty::PtySize {
                        rows,
                        cols,
                        pixel_width: 0,
                        pixel_height: 0,
                    })
                    .map_err(|cause| Error::Pty(cause.into()))
            });

        Ok(SpawnedProgram {
            writer,
            reader,
            resize,
            child,
            open_attempts,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::open_with_retry;

    #[test]
    fn succeeds_first_time_needs_no_retry() {
        let mut calls = 0;
        let result: Result<(u32, u32), (&str, u32)> = open_with_retry(5, || {
            calls += 1;
            Ok::<u32, &str>(42)
        });
        assert_eq!(result, Ok((42, 1)));
        assert_eq!(calls, 1, "a call that succeeds first try must not retry");
    }

    #[test]
    fn fails_once_then_succeeds_counts_one_retry() {
        let mut calls = 0;
        let result = open_with_retry(5, || {
            calls += 1;
            if calls == 1 { Err("transient") } else { Ok(42) }
        });
        assert_eq!(result, Ok((42, 2)));
        assert_eq!(calls, 2);
    }

    #[test]
    fn fails_every_time_surfaces_after_exactly_the_bound() {
        let mut calls = 0;
        let result: Result<((), u32), (&str, u32)> = open_with_retry(3, || {
            calls += 1;
            Err("always fails")
        });
        assert_eq!(result, Err(("always fails", 3)));
        assert_eq!(
            calls, 3,
            "must stop at exactly the bound, neither short nor long"
        );
    }
}
