//! What this task's server-starting path takes as data, rather than reads
//! or assumes (`.claude/rules/testing-rust.md`: "Time, randomness and ids
//! come in as parameters or through a trait. No test sleeps to wait for
//! something; it waits on the event.").
//!
//! `arbitration_timeout` exists for one reason: without its own field,
//! the only way to exercise the "the lock is held and never released"
//! branch (`arbiter::decide`'s `LockOutcome::HeldByOther` that never
//! converges) would be to wait out a production-length deadline, which is
//! exactly how a test quietly becomes a sleep.

use std::path::PathBuf;
use std::time::Duration;

/// How long a server started on demand, with no `--on-demand` distinction
/// yet (that is part B's), waits for a handshake before giving up on a
/// connection (AC #4).
pub const DEFAULT_HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);

/// How long [`crate::arbiter::attempt`] retries the connect probe against
/// a lock another process holds before giving up (AC #2's "leave exactly
/// one listening... without error" - the deadline for the case where that
/// does not happen).
pub const DEFAULT_ARBITRATION_TIMEOUT: Duration = Duration::from_secs(5);

/// Everything this task's server-starting path needs that would otherwise
/// be read from the environment or the clock, so every test overrides it
/// instead of waiting a production-length duration.
#[derive(Debug, Clone)]
pub struct ServerConfig {
    /// `None` means "resolve it from the real environment", the same way
    /// `steg_sdk::local_socket`'s provider does (`STEG_SOCKET_PATH`,
    /// `XDG_RUNTIME_DIR`, `TMPDIR`, `USER`, read by [`crate::listen::serve`]
    /// itself, not here - this type holds data, not the reading of it).
    /// `Some(path)` is a test's own temporary path, used directly.
    pub socket_path: Option<PathBuf>,
    /// How long a connection has to complete its handshake before this
    /// task closes it (AC #4).
    pub handshake_timeout: Duration,
    /// How long [`crate::arbiter::attempt`] waits for a contended lock's
    /// holder to finish becoming the server before giving up (AC #2).
    pub arbitration_timeout: Duration,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            socket_path: None,
            handshake_timeout: DEFAULT_HANDSHAKE_TIMEOUT,
            arbitration_timeout: DEFAULT_ARBITRATION_TIMEOUT,
        }
    }
}
