//! The accept loop, in the foreground of whichever process calls it (AC
//! #5: "the server runs in the foreground of the process that called
//! it, with no detaching, so this part is testable alone"). [`serve`] is
//! `attempt` then, on [`Arbitration::Became`], [`run`] - part B's own
//! detach wraps around exactly these two calls, with nothing of its own
//! added to either (`attempt` pre-detach, `run` post-detach).

use std::os::unix::net::UnixStream;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::thread;

use crate::arbiter::{self, Arbitration};
use crate::config::ServerConfig;
use crate::connection::{self, ConnectionEvent};
use crate::error::Error;

/// What [`serve`] reports once it has a final answer.
#[derive(Debug)]
pub enum ServeOutcome {
    /// This process was the server, and has since stopped (`run`
    /// returned - [`Stopper::stop`] was called).
    Became,
    /// A live server was already there; nothing was bound.
    AlreadyRunning,
    /// Refused outright - see [`arbiter::Reason`].
    Refuse(arbiter::Reason),
}

/// Tells [`run`]'s accept loop to stop. Cloning shares the same
/// underlying flag - every clone tells the same `run` call to stop.
/// Part B's idle checker (not built in this task) will hold one of
/// these and call [`Stopper::stop`] once its own clock-injected
/// predicate says to; this task's own tests stand in for that caller.
#[derive(Clone)]
pub struct Stopper {
    stop: Arc<AtomicBool>,
}

impl Stopper {
    #[must_use]
    pub fn new() -> Self {
        Self {
            stop: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Tells `run` to stop. Idempotent - calling it more than once, or
    /// concurrently, does nothing more than calling it once.
    pub fn stop(&self) {
        self.stop.store(true, Ordering::SeqCst);
    }

    fn should_stop(&self) -> bool {
        self.stop.load(Ordering::SeqCst)
    }
}

impl Default for Stopper {
    fn default() -> Self {
        Self::new()
    }
}

/// Wakes a thread parked in `UnixListener::accept()` with no other
/// cancellation std gives one - connecting a throwaway client to the
/// same socket makes `accept()` return, at which point the accept loop
/// checks [`Stopper::should_stop`] before it does anything else with
/// that connection (never spawning a handler thread for it - it is not a
/// real client, only a wakeup).
fn wake_accept(socket_path: &std::path::Path) {
    let _ = UnixStream::connect(socket_path);
}

/// The accept loop: spawns a thread per accepted connection
/// (`connection::handle`) until [`stop`] says to stop, then closes the
/// listener, removes the socket file, and releases the lock - in that
/// order, so the lock (the arbiter) is never given up before the path is
/// fully vacated. A client whose `connect()` lands in that instant sees
/// exactly what it would see against a server that was never there
/// (refused, or - once the file is gone - not found), which is the safe,
/// correct reading for any client (the sdk's own `local_socket::connect`
/// treats either the same way: try to start one).
///
/// # Errors
/// An [`Error::Io`] from the accept loop's own listener.
pub fn run(
    handle: arbiter::ServerHandle,
    socket_path: &std::path::Path,
    config: &ServerConfig,
    stop: &Stopper,
    events: Option<Sender<ConnectionEvent>>,
) -> Result<(), Error> {
    loop {
        let (stream, _addr) = handle.listener.accept()?;
        if stop.should_stop() {
            drop(stream);
            break;
        }
        let handshake_timeout = config.handshake_timeout;
        let events = events.clone();
        thread::spawn(move || connection::handle(stream, handshake_timeout, events));
    }

    // Shutdown order, each step only after the last: stop accepting
    // (the loop above already has), close the listener, remove the
    // socket file, release the lock last - never before the path is
    // fully vacated. Destructured rather than one `drop(handle)`: a
    // struct's fields drop in declaration order, which would release
    // the lock in the same step as closing the listener, before the
    // file is removed - the exact gap this ordering exists to avoid.
    let arbiter::ServerHandle { listener, lock } = handle;
    drop(listener);
    let _ = std::fs::remove_file(socket_path);
    drop(lock);
    Ok(())
}

/// Resolves `config.socket_path` against the real environment when it is
/// `None` - the same preference order the sdk's own `local_socket`
/// provider uses (`STEG_SOCKET_PATH`, `XDG_RUNTIME_DIR`, `TMPDIR`, the
/// `USER` fallback), read here and fed into the sdk's own pure
/// `steg_sdk::socket_path` rather than duplicated. Every one of this
/// task's own tests sets `config.socket_path` explicitly instead, so
/// this is only ever exercised for real by a caller that does not.
fn resolve_socket_path(config: &ServerConfig) -> std::path::PathBuf {
    if let Some(path) = &config.socket_path {
        return path.clone();
    }
    let explicit = std::env::var_os("STEG_SOCKET_PATH").map(std::path::PathBuf::from);
    let xdg_runtime_dir = std::env::var_os("XDG_RUNTIME_DIR").map(std::path::PathBuf::from);
    let tmp_dir = std::env::var_os("TMPDIR").map(std::path::PathBuf::from);
    let user = std::env::var("USER").ok();
    steg_sdk::socket_path(
        explicit.as_deref(),
        xdg_runtime_dir.as_deref(),
        tmp_dir.as_deref(),
        user.as_deref(),
    )
}

/// `attempt` then, on [`Arbitration::Became`], [`run`] - this task's own
/// foreground, no-detach server (AC #5). Returns once it became the
/// server and has since stopped, or immediately if it did not become
/// one at all.
///
/// # Errors
/// Whatever [`arbiter::attempt`] or [`run`] can fail with.
pub fn serve(
    config: &ServerConfig,
    stop: &Stopper,
    events: Option<Sender<ConnectionEvent>>,
) -> Result<ServeOutcome, Error> {
    let socket_path = resolve_socket_path(config);
    match arbiter::attempt(config, &socket_path)? {
        Arbitration::Became(handle) => {
            run(handle, &socket_path, config, stop, events)?;
            Ok(ServeOutcome::Became)
        }
        Arbitration::AlreadyRunning => Ok(ServeOutcome::AlreadyRunning),
        Arbitration::Refuse(reason) => Ok(ServeOutcome::Refuse(reason)),
    }
}

/// Wakes [`run`]'s accept loop after telling it to stop - the two steps
/// `Stopper` alone cannot do by itself, since connecting to the socket
/// needs the path, which `Stopper` does not hold (kept as data, not
/// behaviour tied to one socket, so the same `Stopper` type works for
/// `run` and for a future caller that never needs this helper because it
/// already knows to wake the loop its own way).
pub fn stop_and_wake(stop: &Stopper, socket_path: &std::path::Path) {
    stop.stop();
    wake_accept(socket_path);
}
