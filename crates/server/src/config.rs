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
use std::sync::Arc;
use std::time::Duration;

use crate::pty::{ProgramLauncher, RealLauncher};
use crate::runtime::LoginShell;

/// How long a server started on demand, with no `--on-demand` distinction
/// yet (that is part B's), waits for a handshake before giving up on a
/// connection (AC #4).
pub const DEFAULT_HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);

/// How long [`crate::arbiter::attempt`] retries the connect probe against
/// a lock another process holds before giving up (AC #2's "leave exactly
/// one listening... without error" - the deadline for the case where that
/// does not happen).
pub const DEFAULT_ARBITRATION_TIMEOUT: Duration = Duration::from_secs(5);

/// How long [`crate::login_env::resolve_login_identity`] waits for the
/// password-database lookup before falling back to a default shell/home
/// (TASK-1.87 - a local syscall that can in principle stall on a machine
/// whose accounts are bound to a network directory service, TASK-1.77's
/// own reasoning for the identical bound there).
pub const DEFAULT_LOGIN_IDENTITY_TIMEOUT: Duration = Duration::from_secs(2);

/// How long [`crate::login_env::capture_login_environment`] waits for the
/// login shell to start, source its profile, and print its environment
/// before falling back to a fixed, minimal one (TASK-1.87) - bounded
/// separately from the identity lookup above because spawning a real
/// shell and sourcing a real profile is the genuinely slow step (a
/// hanging interactive prompt, a profile script that blocks). Chosen with
/// margin against [`DEFAULT_HANDSHAKE_TIMEOUT`] (10s): this bound runs
/// once, after the socket is already bound and before the accept loop
/// starts, so it delays a client's first request, never its `connect()`.
pub const DEFAULT_LOGIN_ENVIRONMENT_TIMEOUT: Duration = Duration::from_secs(3);

/// Everything this task's server-starting path needs that would otherwise
/// be read from the environment or the clock, so every test overrides it
/// instead of waiting a production-length duration.
#[derive(Clone)]
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
    /// What [`crate::listen::run`] builds its one [`crate::Runtime`] with
    /// (TASK-1.45 §1) - defaulted to the real launcher, same as every
    /// other field here defaults to the real thing. A socket-level test
    /// that never needs a real program can override this with a fake, the
    /// same way `runtime.rs`'s own tests already inject one.
    pub launcher: Arc<dyn ProgramLauncher>,
    /// The login shell [`crate::Runtime`] falls back to when a pane names
    /// no program and no source pane to inherit a `cwd` from (TASK-1.76
    /// §2), and the identity [`crate::login_env::capture_login_environment`]
    /// captures the real environment from (TASK-1.87) - resolved from the
    /// password database at the existing pre-bind call site
    /// (`crates/steg/src/main.rs`'s `run_server_start`), never from this
    /// process's own (possibly client-inherited) `$SHELL`/`$HOME`. Its own
    /// `base_env` starts empty here: a cheap identity-only value is all
    /// this field ever holds before the socket is bound - the actual
    /// environment capture happens once, after that, inside
    /// [`crate::listen::run`].
    pub login: LoginShell,
    /// How long resolving the login identity (TASK-1.87) may take before
    /// falling back to a default shell/home - see
    /// [`DEFAULT_LOGIN_IDENTITY_TIMEOUT`].
    pub login_identity_timeout: Duration,
    /// How long capturing the login shell's own environment (TASK-1.87)
    /// may take before falling back to a fixed, minimal one - see
    /// [`DEFAULT_LOGIN_ENVIRONMENT_TIMEOUT`].
    pub login_environment_timeout: Duration,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            socket_path: None,
            handshake_timeout: DEFAULT_HANDSHAKE_TIMEOUT,
            arbitration_timeout: DEFAULT_ARBITRATION_TIMEOUT,
            launcher: Arc::new(RealLauncher),
            login: LoginShell::default(),
            login_identity_timeout: DEFAULT_LOGIN_IDENTITY_TIMEOUT,
            login_environment_timeout: DEFAULT_LOGIN_ENVIRONMENT_TIMEOUT,
        }
    }
}

impl std::fmt::Debug for ServerConfig {
    /// `launcher` is `dyn ProgramLauncher`, which carries no `Debug` bound
    /// (nothing needs one, and adding one would force every fake launcher
    /// a test builds to derive it too) - shown as a placeholder instead of
    /// widening that trait just to satisfy a derive here.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ServerConfig")
            .field("socket_path", &self.socket_path)
            .field("handshake_timeout", &self.handshake_timeout)
            .field("arbitration_timeout", &self.arbitration_timeout)
            .field("launcher", &"<dyn ProgramLauncher>")
            .field("login", &self.login)
            .finish()
    }
}
