//! The local-socket provider (R103): finds a running server, or with none
//! running starts one and waits for its socket, with an option not to
//! start one. The one place in this crate allowed to do real I/O
//! (`.claude/rules/sdk.md`) - std only, blocking, no dependency.
//!
//! What this hands TASK-1.44: the spawn step always runs exactly `<binary>
//! server start --on-demand` (doc-2's own `server start` command, widened
//! with the `on_demand` switch `description::methods` now declares - see
//! that method's own `details` for why `server.start`/`.stop`/`.show`
//! cannot themselves be how a process with nothing to connect to becomes
//! one). What `server start --on-demand` does once spawned is not this
//! module's: try to connect first, and become the server - bind, detach,
//! serve, with R99's on-demand lifecycle - only on failure, never
//! recursing into this module's own autostart path.
//!
//! The lock half of "the socket's path and lock" is explicitly not this
//! module's either: it only spawns and waits for the socket to accept a
//! connection. Resolving two simultaneous starts to one survivor (R99) is
//! invisible here - this module only ever sees "the socket now accepts,
//! or it still doesn't."

#![expect(
    clippy::disallowed_types,
    clippy::disallowed_methods,
    reason = "R103: the local-socket provider is the sdk's one permitted exception to no \
              I/O of its own (doc-16 R102/R110) - it finds or starts a local server using \
              std's process and Unix-socket APIs directly, blocking, with its own retry \
              deadline. `.claude/rules/sdk.md` scopes this expectation to this module alone."
)]

use std::env;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use crate::paths::socket_path;

/// How to reach, or start, the local server.
#[derive(Debug, Clone)]
pub struct ConnectOptions {
    /// What to run to become a server when none is found. No default
    /// anywhere in this crate - the CLI (`steg` package) sets this to
    /// `std::env::current_exe()`, correct only because the server and the
    /// CLI ship as one binary (R61/R98); the app is a separate program
    /// (doc-16 level 2) and must supply wherever it finds its own copy,
    /// which this crate does not decide.
    pub binary: PathBuf,
    /// `None` means "compute it from the real environment"
    /// (`XDG_RUNTIME_DIR`/`TMPDIR`/`STEG_SOCKET_PATH`/`USER`, read here).
    /// `Some(path)` is an explicit override - a test's temp directory, or
    /// later a person's own debugging - used directly, and set as
    /// `STEG_SOCKET_PATH` on a spawned child's environment so it resolves
    /// the identical path through the same `paths::socket_path` call.
    pub socket_path: Option<PathBuf>,
    /// Start a server when none is found. `false` is what doc-2's
    /// `--no-autostart` becomes: [`ConnectError::NoServerRunning`], which
    /// a consumer (TASK-1.49) maps to exit status 4.
    pub autostart: bool,
    /// How long to wait for a freshly spawned server's socket to answer
    /// before giving up with [`ConnectError::Timeout`].
    pub connect_timeout: Duration,
}

/// Why [`connect`] could not return a connected socket.
#[derive(Debug)]
pub enum ConnectError {
    /// Nothing was listening, and `autostart` was `false`.
    NoServerRunning,
    /// `binary` could not be spawned at all.
    Spawn(std::io::Error),
    /// A server was spawned but its socket never answered within
    /// `connect_timeout`.
    Timeout,
}

impl std::fmt::Display for ConnectError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoServerRunning => write!(f, "no steg server is running"),
            Self::Spawn(err) => write!(f, "could not start a steg server: {err}"),
            Self::Timeout => write!(f, "timed out waiting for the steg server's socket"),
        }
    }
}

impl std::error::Error for ConnectError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::NoServerRunning | Self::Timeout => None,
            Self::Spawn(err) => Some(err),
        }
    }
}

fn resolve_path(options: &ConnectOptions) -> PathBuf {
    if let Some(path) = &options.socket_path {
        return path.clone();
    }
    let explicit = env::var_os("STEG_SOCKET_PATH").map(PathBuf::from);
    let xdg_runtime_dir = env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from);
    let tmp_dir = env::var_os("TMPDIR").map(PathBuf::from);
    let user = env::var("USER").ok();
    socket_path(
        explicit.as_deref(),
        xdg_runtime_dir.as_deref(),
        tmp_dir.as_deref(),
        user.as_deref(),
    )
}

/// Connects to a running server, or - with `autostart` and none running -
/// spawns one and waits for its socket.
///
/// This never waits on the spawned process (below): a bad `binary` - one
/// that does not exist, or exists but cannot be executed - is reported as
/// [`ConnectError::Spawn`] as soon as the spawn itself fails, without
/// waiting out `connect_timeout`. A consumer that calls this with
/// `autostart` many times over a long life (the app, reconnecting) can
/// accumulate a process-table entry per call until it itself exits, for
/// as long as the real server takes to detach and exit the process this
/// spawned - entirely TASK-1.44's to keep short, not a cost this function
/// can avoid by waiting, which R99 forbids it from doing.
///
/// # Errors
/// [`ConnectError::NoServerRunning`] when nothing is listening and
/// `autostart` is `false`; [`ConnectError::Spawn`] when `binary` could not
/// be run at all; [`ConnectError::Timeout`] when a spawned server's socket
/// never answers within `connect_timeout`.
pub fn connect(options: &ConnectOptions) -> Result<UnixStream, ConnectError> {
    let path = resolve_path(options);

    if let Ok(stream) = UnixStream::connect(&path) {
        return Ok(stream);
    }

    if !options.autostart {
        return Err(ConnectError::NoServerRunning);
    }

    let mut command = Command::new(&options.binary);
    command
        .args(["server", "start", "--on-demand"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    if let Some(socket_path) = &options.socket_path {
        command.env("STEG_SOCKET_PATH", socket_path);
    }
    // R99's detaching, re-parenting or double-fork, and stopping are the
    // server's own code; this drops the `Child` handle once spawn
    // succeeds and never wait()s on it. `spawn()` itself still fails
    // synchronously when `binary` cannot be exec'd at all (missing, or
    // not executable) - the OS reports that before this call returns, so
    // `?` here returns `ConnectError::Spawn` immediately, before the
    // retry loop and its deadline below are ever reached.
    command.spawn().map_err(ConnectError::Spawn)?;

    let deadline = Instant::now() + options.connect_timeout;
    loop {
        if let Ok(stream) = UnixStream::connect(&path) {
            return Ok(stream);
        }
        if Instant::now() >= deadline {
            return Err(ConnectError::Timeout);
        }
        thread::sleep(Duration::from_millis(20));
    }
}

/// Test support only: the whole of `tests/fixtures/fake_server.rs`'s
/// `main` is one call into this. Lives here, under this module's own
/// `#![expect]`, deliberately - a `[[bin]]` fixture needs real I/O to
/// stand in for "a server now accepts there" at all, and
/// `.claude/rules/sdk.md` is explicit that nowhere else in the crate
/// gets this expectation, so the fixture's own file calls this instead
/// of carrying a second one.
///
/// Resolves the socket path exactly as [`connect`]'s spawn step expects a
/// real server to: from its own inherited environment, via the same
/// `paths::socket_path`, with no path ever passed on its argv (which this
/// function, and a real `steg server start --on-demand`, both ignore).
///
/// # Panics
/// If the socket cannot be bound - there is nothing a test fixture can
/// usefully do instead.
#[doc(hidden)]
pub fn run_fake_server_for_tests() {
    let explicit = env::var_os("STEG_SOCKET_PATH").map(PathBuf::from);
    let xdg_runtime_dir = env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from);
    let tmp_dir = env::var_os("TMPDIR").map(PathBuf::from);
    let user = env::var("USER").ok();
    let path = socket_path(
        explicit.as_deref(),
        xdg_runtime_dir.as_deref(),
        tmp_dir.as_deref(),
        user.as_deref(),
    );
    let listener = UnixListener::bind(&path).expect("fake-server-for-tests: bind failed");
    thread::sleep(Duration::from_secs(30));
    drop(listener);
}

// AC #3's own tests live in `tests/local_socket.rs`, not here: they need
// the `fake-server-for-tests` fixture binary, and `CARGO_BIN_EXE_<name>`
// is only set for an integration test, not for this module's own
// `#[cfg(test)]` (a lib unit test binary) - found while building this,
// not assumed.
#[cfg(test)]
mod tests {
    use super::ConnectError;

    #[test]
    fn display_names_the_binary_error_and_the_timeout() {
        assert!(format!("{}", ConnectError::NoServerRunning).contains("no steg server"));
        assert!(format!("{}", ConnectError::Timeout).contains("timed out"));
        let spawn_err = ConnectError::Spawn(std::io::Error::other("boom"));
        assert!(format!("{spawn_err}").contains("boom"));
    }
}
