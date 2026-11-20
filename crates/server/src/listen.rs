//! The accept loop, in the foreground of whichever process calls it (AC
//! #5: "the server runs in the foreground of the process that called
//! it, with no detaching, so this part is testable alone"). [`serve`] is
//! `attempt` then, on [`Arbitration::Became`], [`run`] - part B's own
//! detach wraps around exactly these two calls, with nothing of its own
//! added to either (`attempt` pre-detach, `run` post-detach).

use std::net::Shutdown;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use nix::errno::Errno;

use crate::arbiter::{self, Arbitration};
use crate::config::ServerConfig;
use crate::connection::{self, ConnectionEvent};
use crate::error::Error;
use crate::idle;
use crate::runtime::Runtime;

/// Why `run`'s accept loop treats one `accept()`/`try_clone()` error as safe
/// to retry rather than as a reason to stop the whole server (TASK-1.98 AC
/// #1). Matched by raw errno (`classify_accept_error`, below), never by
/// `std::io::ErrorKind`: `EMFILE`/`ENFILE` have no `ErrorKind` variant that
/// round-trips through this platform's own `decode_error_kind` - confirmed
/// by reading this toolchain's own
/// `library/std/src/sys/net/connection/socket/unix.rs`, not assumed - so an
/// `ErrorKind`-based classifier could never cover the one case AC #4 is
/// about (the loop must not spin while descriptors stay exhausted).
///
/// `pub`: carried on [`ConnectionEvent::AcceptRetried`] so a caller given
/// `events` can tell which of the three kinds happened, same as any other
/// `ConnectionEvent`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoverableAcceptError {
    /// `EMFILE` (this process is out of descriptors) or `ENFILE` (the whole
    /// system is). Unlike the other two kinds, this does not clear on its
    /// own - it lasts until some descriptor, anywhere, is closed - which is
    /// exactly why the loop backs off instead of retrying at full speed
    /// while it lasts (AC #4), and why `run`'s own stop check now runs
    /// before this classifier does: a storm that never resolves on its own
    /// must still let `stop()` end the loop within one backoff interval,
    /// not leave it retrying forever.
    DescriptorsExhausted,
    /// `ECONNABORTED` - a connection the kernel already queued was reset or
    /// torn down by its own peer before this loop's `accept()` picked it
    /// up. Not this listener's fault, and nothing to do with `run`'s own
    /// stop mechanism either: `Stopper::stop` wakes a blocked `accept()`
    /// with a real `UnixStream::connect` (`wake_accept`, above), never a
    /// `shutdown`/`close` of the listener from another thread, so there is
    /// no path here where `run`'s own stop produces this error itself.
    ConnectionAborted,
    /// `EINTR` - interrupted by a signal before a connection was ready.
    /// **Checked, not assumed, to be dead in practice on this target**:
    /// `UnixListener::accept` calls `Socket::accept`, which on macOS (not in
    /// that file's `accept4`-capable target list) wraps `libc::accept` in
    /// `cvt_r`, already retrying `EINTR` internally - so this arm is a
    /// defensive backstop for a classifier that has to be correct
    /// regardless of what std happens to do today, not because it is
    /// expected to fire. Kept because AC #1 names it explicitly.
    Interrupted,
}

/// `Some(kind)` for the three cases AC #1 names as recoverable; `None` for
/// everything else, including `raw_os_error() == None` (a `std::io::Error`
/// not actually sourced from a syscall) - the same conservative default
/// `run` always had before this task, only narrowed by the three kinds
/// above and never widened past them. Matched through `nix::errno::Errno`
/// (already an allowed dependency of this crate, `boundaries.toml`), which
/// re-exports `libc` itself (`nix::libc`), so no new dependency is needed
/// for this.
fn classify_accept_error(err: &std::io::Error) -> Option<RecoverableAcceptError> {
    match err.raw_os_error().map(Errno::from_raw) {
        Some(Errno::EMFILE | Errno::ENFILE) => Some(RecoverableAcceptError::DescriptorsExhausted),
        Some(Errno::ECONNABORTED) => Some(RecoverableAcceptError::ConnectionAborted),
        Some(Errno::EINTR) => Some(RecoverableAcceptError::Interrupted),
        _ => None,
    }
}

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
/// underlying flag (and, once `run` has bound it - see
/// [`Stopper::bind_socket_path`] - the same socket path to wake) -
/// every clone tells the same `run` call to stop. Part B's idle checker
/// (not built in this task) will hold one of these and call
/// [`Stopper::stop`] once its own clock-injected predicate says to;
/// this task's own tests stand in for that caller. `stop()` is the one
/// public way to end `run`, and it always works on its own - a review
/// of this task found an earlier version where it did not (see
/// `stop()`'s own doc).
#[derive(Clone)]
pub struct Stopper {
    stop: Arc<AtomicBool>,
    /// The socket `run` is accepting on, so `stop()` can wake it - set
    /// once, by `run` itself, before it ever blocks in `accept()`
    /// (`bind_socket_path`). `None` until then, which is fine: a
    /// `stop()` called before `run` has started has nothing to wake
    /// yet, and `run` checks the flag itself immediately after binding
    /// this, before it ever enters the loop (see `run`'s own doc).
    socket_path: Arc<Mutex<Option<PathBuf>>>,
}

impl Stopper {
    #[must_use]
    pub fn new() -> Self {
        Self {
            stop: Arc::new(AtomicBool::new(false)),
            socket_path: Arc::new(Mutex::new(None)),
        }
    }

    /// Tells `run` to stop, and wakes its accept loop so that actually
    /// happens - the one public way to do either, safe to call more
    /// than once, concurrently, from any thread, before `run` has even
    /// started, or after the socket file is already gone (a failed
    /// wakeup connect is silently ignored, same as it always was - by
    /// then `run` is already on its way out regardless).
    ///
    /// **A review of this task found the previous version of this
    /// method set only the flag**, leaving the separate `stop_and_wake`
    /// free function as the only way that reliably ended `run` - `run`'s
    /// own loop only re-checks the flag once `accept()` returns, so a
    /// bare flag-set left a genuinely idle server (no connection ever
    /// arriving to make `accept()` return and notice it) blocked
    /// forever. Removed rather than documented: there is now one
    /// public way to stop, and it always works, on an idle server or
    /// otherwise.
    ///
    /// # Panics
    /// Never in practice: only if the mutex guarding the socket path is
    /// poisoned, which only happens if a previous lock holder itself
    /// panicked while holding it - neither `stop()` nor
    /// `bind_socket_path` does any work that can panic while holding
    /// the lock.
    pub fn stop(&self) {
        self.set_stop_flag();
        if let Some(path) = self
            .socket_path
            .lock()
            .expect("the socket path mutex is never held across a panic")
            .as_ref()
        {
            wake_accept(path);
        }
    }

    /// Just the flag, with no wakeup - `run` itself calls this, both
    /// through the public `stop()` above and directly, right after
    /// binding the socket path, to catch a `stop()` that raced in
    /// before there was a path to wake (see `run`'s own doc). Private:
    /// every external caller goes through `stop()`.
    fn set_stop_flag(&self) {
        self.stop.store(true, Ordering::SeqCst);
    }

    fn should_stop(&self) -> bool {
        self.stop.load(Ordering::SeqCst)
    }

    /// Records the socket `run` is accepting on, so a later `stop()`
    /// knows what to wake. Called once, by `run` itself, before it
    /// ever blocks in `accept()` - not part of the public contract: a
    /// caller of `stop()` never needs to know the path itself, only
    /// that calling `stop()` always works.
    fn bind_socket_path(&self, path: &Path) {
        *self
            .socket_path
            .lock()
            .expect("the socket path mutex is never held across a panic") =
            Some(path.to_path_buf());
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

/// What [`reap_finished`] needs from either kind of join handle std
/// gives - a plain `JoinHandle<()>` (this module's own tests) or a
/// `thread::ScopedJoinHandle<'_, ()>` ([`run`]'s own connections, spawned
/// inside its `thread::scope` so `runtime` can be borrowed rather than
/// `Arc`'d - TASK-1.45 §1). No trait in std covers both, so this one
/// does.
trait Joinable {
    fn is_finished(&self) -> bool;
    fn join(self) -> std::thread::Result<()>;
}

impl Joinable for thread::JoinHandle<()> {
    fn is_finished(&self) -> bool {
        thread::JoinHandle::is_finished(self)
    }

    fn join(self) -> std::thread::Result<()> {
        thread::JoinHandle::join(self)
    }
}

impl Joinable for thread::ScopedJoinHandle<'_, ()> {
    fn is_finished(&self) -> bool {
        thread::ScopedJoinHandle::is_finished(self)
    }

    fn join(self) -> std::thread::Result<()> {
        thread::ScopedJoinHandle::join(self)
    }
}

/// Removes every connection whose thread has already finished on its
/// own (a client that closed, was refused, or timed out) from
/// `connections`, joining each - `is_finished` already guarantees the
/// join returns at once, never blocking this loop on a connection that
/// is still open. Called once per loop iteration, right after a new
/// connection is accepted and before it is pushed: found necessary
/// while building this, not assumed - without it `connections` only
/// ever grows, holding a real file descriptor (the `try_clone`d
/// shutdown handle) and a join handle for every connection the server
/// has ever accepted, for as long as it runs. With this, the tracked
/// set holds exactly the connections still open plus whatever finished
/// since the last accept - bounded by how many connections are
/// concurrently live, never by how many have come and gone over the
/// server's lifetime (proved by
/// `many_short_connections_do_not_grow_the_tracked_set`, `shutdown.rs`).
fn reap_finished<J: Joinable>(connections: &mut Vec<(UnixStream, J)>) {
    let mut i = 0;
    while i < connections.len() {
        if connections[i].1.is_finished() {
            let (_, join) = connections.swap_remove(i);
            let _ = join.join();
        } else {
            i += 1;
        }
    }
}

/// What [`build_idle_wiring`] hands back when idle-checking is on: the
/// sender every connection should clone from, and the one checker
/// thread's own handle to join once it is safe to (`run`'s own shutdown
/// sequence, below `run`'s own body).
struct IdleWiring {
    checker_tx: Sender<ConnectionEvent>,
    checker_handle: thread::JoinHandle<()>,
}

/// Builds the one checker thread and its channel, only when `idle_timeout`
/// is `Some` (TASK-1.86, doc-8 R99) - an explicit start's `idle_timeout`
/// is `None`, and this function's own first line is the whole of why
/// that spawns nothing at all, checked directly rather than inferred from
/// a timing bound (this module's own test, below).
fn build_idle_wiring(
    idle_timeout: Option<Duration>,
    runtime: &Runtime,
    stop: &Stopper,
) -> Option<IdleWiring> {
    let idle_timeout = idle_timeout?;

    let (checker_tx, checker_rx) = mpsc::channel();
    runtime.set_idle_notifier(checker_tx.clone());

    let stop = stop.clone();
    let session_count = runtime.session_counter();
    let checker_handle = thread::Builder::new()
        .name("steg-idle-checker".to_string())
        .spawn(move || idle::run_checker(checker_rx, stop, session_count, idle_timeout))
        .expect("spawning the idle checker thread");

    Some(IdleWiring {
        checker_tx,
        checker_handle,
    })
}

/// The accept loop: spawns a thread per accepted connection
/// (`connection::handle`) until [`stop`] says to stop, then closes the
/// listener, shuts down and joins every connection still open, removes
/// the socket file, and releases the lock - in that order, so the lock
/// (the arbiter) is never given up before the path is fully vacated,
/// and the listener closes before this function spends any time
/// joining connections (see the ordering's own comment, below, for
/// why that order and not the reverse). A client whose `connect()`
/// lands after the listener closes sees exactly what it would see
/// against a server that was never there (refused, or - once the file
/// is gone - not found), which is the safe, correct reading for any
/// client (the sdk's own `local_socket::connect` treats either the
/// same way: try to start one).
///
/// **Every spawned thread is joined on every path out of this
/// function**, found necessary rather than assumed while building this:
/// a connection accepted under AC #4's own promise - "one that has
/// completed [the handshake] is not closed for being idle" - blocks in
/// `connection::handle`'s hold-open read with no timeout at all, so
/// nothing makes that thread return on its own when `stop` is called; a
/// still-open client might never send another byte. `connections`
/// tracks a `try_clone` of each accepted stream beside its thread's
/// `JoinHandle` so this function can call `shutdown` on that clone -
/// confirmed by
/// `a_completed_idle_connection_is_closed_and_run_returns_promptly_on_stop`
/// (`shutdown.rs`) to unblock a sibling descriptor's own blocked `read`
/// with `Ok(0)`, not assumed - and then `join`, bounding every thread
/// this loop ever spawned to this one shutdown step rather than to
/// however long an idle client happens to stay connected. Already
/// -finished connections are reaped as the loop runs
/// ([`reap_finished`]), so this final pass only ever waits on
/// connections genuinely still open when `stop` was called, never on
/// the server's whole history of past connections.
///
/// **Calling [`Stopper::stop`] on a genuinely idle server - no
/// connection ever arriving on its own to make `accept()` return and
/// notice the flag - still returns from this function promptly.** A
/// review of this task found that was not true of an earlier version:
/// [`Stopper::bind_socket_path`] is called first, below, specifically
/// so that `stop()` (called at any time, including before this
/// function is even entered) always has a path to wake - and the flag
/// is checked once more immediately after binding it, for the one case
/// a wakeup cannot cover: `stop()` having raced in and set the flag
/// before there was a path to wake at all. Neither check alone would
/// be enough; together they mean `accept()` is never entered at all
/// once `stop()` has been called, no matter when.
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
    stop.bind_socket_path(socket_path);

    // One `Runtime` per server process (TASK-1.45 §1), built from
    // `config`'s own launcher/login - never behind an `Arc`: every
    // connection thread spawned below borrows it instead of cloning a
    // reference-counted handle, which is what lets `runtime.stop()`,
    // near the end of this function, be a property the compiler itself
    // enforces rather than one this comment merely asserts - it cannot
    // even type-check while a `scope.spawn`ed closure might still hold
    // `&runtime`, and `thread::scope` guarantees none does by the time
    // it returns.
    //
    // `config.login.base_env` is assumed already resolved by this
    // point (TASK-1.87): capturing it is this crate's caller's own job
    // (`crates/steg/src/main.rs`'s `run_server_start`, after the socket
    // is bound and before `run` is ever called), not this function's -
    // this crate prints nothing (no `eprintln!`/`println!` appears
    // anywhere in `crates/server/src`, a line this task keeps true), and
    // a fallback worth reporting needs a caller able to print about it.
    let mut runtime = Runtime::new(
        Arc::clone(&config.launcher),
        config.login.clone(),
        config.force_grace,
        config.attach_tick,
    );

    // Unconditionally, unlike the idle wiring just below (TASK-1.88): a
    // `server.stop` request (`connection.rs`'s own interception) reaches
    // this same `Stopper` on every server, explicit or on-demand - R99's
    // other half ("a server started explicitly stops only when told to")
    // is answered by `server.stop`, which has had no way to reach this
    // `Stopper` at all until now, even though every caller of `run`
    // already builds one and passes it in.
    runtime.set_stopper(stop.clone());

    // Built before the loop, same reasoning as `runtime` above - only
    // `Some` when `config.idle_timeout` is (TASK-1.86); `None` for an
    // explicit start spawns nothing and costs nothing.
    let (checker_tx, checker_handle) = match build_idle_wiring(config.idle_timeout, &runtime, stop)
    {
        Some(wiring) => (Some(wiring.checker_tx), Some(wiring.checker_handle)),
        None => (None, None),
    };

    // Split apart before the loop, not destructured after it as before:
    // `listener` has to be owned *inside* the `thread::scope` below (so
    // it can be dropped there, closing it before the final join loop -
    // see that comment, carried over unchanged), while `lock` has to
    // survive untouched until after the socket file is removed, well
    // outside the scope. Splitting them here, rather than moving the
    // whole `handle` in and trying to get `lock` back out later, keeps
    // both lifetimes honest from the start.
    let arbiter::ServerHandle { listener, lock } = handle;

    // No early return (`?`) inside this loop, on purpose: an I/O error
    // from `accept` or `try_clone` is an error branch like any other,
    // and the cleanup below - shutting down and joining every
    // connection already spawned - has to run on that path too, not
    // only when the loop ends by `stop` being called. The error, if
    // any, is carried past the loop and returned after cleanup runs.
    let loop_result: Result<(), Error> = if stop.should_stop() {
        // Already told to stop before this function ever got as far as
        // calling `accept()` - never enters the loop at all, rather
        // than relying on a wakeup there was no path to deliver until
        // the line just above ran.
        Ok(())
    } else {
        let runtime_ref = &runtime;
        // Computed once, outside the loop: every send site in this loop -
        // TASK-1.98's own two retry arms below, and this task's own
        // per-connection clone - needs the same answer to "who gets a
        // `ConnectionEvent`," so there is exactly one place that decides
        // it. When idle-checking is on, the checker's own sender wins
        // (`AcceptRetried` must reach it too - a retry is not a client
        // arriving, which the idle checker's own catch-all already
        // handles identically to any other event it does not specifically
        // care about, `idle.rs`'s own test proves); when it's off
        // (`checker_tx` is `None`, an explicit start), this is
        // `events.clone()`, byte for byte what ran here before TASK-1.86.
        // `.or()`, not `.clone().or_else()`: computed exactly once, so
        // there is nothing to keep cheap by deferring - and, just as
        // importantly, this *fully consumes* both `checker_tx` and
        // `events` rather than leaving a clone of either alive in this
        // function's own outer scope. A leftover clone of `checker_tx`
        // here, never moved into the scope below, would still be a live
        // sender into the checker's channel after `thread::scope`
        // returns - exactly the kind of extra sender the join bound
        // below is derived *against never existing*.
        let checker_or_events = checker_tx.or(events);
        thread::scope(move |scope| {
            let mut connections: Vec<(UnixStream, thread::ScopedJoinHandle<'_, ()>)> = Vec::new();
            let accept_result = loop {
                let (stream, _addr) = match listener.accept() {
                    Ok(accepted) => accepted,
                    Err(err) => {
                        // Checked before this error is even classified, not
                        // only before the sleep below: TASK-1.98's own
                        // review found that without this, a `stop()` called
                        // while descriptors stay exhausted (`EMFILE`/
                        // `ENFILE` do not clear on their own) could go
                        // unnoticed for as long as the storm lasts, since
                        // `should_stop` was otherwise only ever checked
                        // after a *successful* `accept()` - worse than
                        // today, where any error ends the loop at once.
                        // Checking first bounds that to one loop iteration
                        // instead, every time through.
                        if stop.should_stop() {
                            break Ok(());
                        }
                        match classify_accept_error(&err) {
                            Some(kind) => {
                                if let Some(tx) = &checker_or_events {
                                    let _ = tx.send(ConnectionEvent::AcceptRetried(kind));
                                }
                                // Opportunistic: a connection that finished
                                // on its own during this storm gives its fd
                                // back now, rather than waiting for the
                                // next successful `accept()` - which, under
                                // `ENFILE`, might be a while.
                                reap_finished(&mut connections);
                                thread::sleep(config.accept_error_backoff);
                                continue;
                            }
                            None => break Err(err.into()),
                        }
                    }
                };
                reap_finished(&mut connections);
                if stop.should_stop() {
                    drop(stream);
                    break Ok(());
                }
                let shutdown_handle = match stream.try_clone() {
                    Ok(cloned) => cloned,
                    Err(err) => {
                        drop(stream);
                        if stop.should_stop() {
                            break Ok(());
                        }
                        match classify_accept_error(&err) {
                            Some(kind) => {
                                if let Some(tx) = &checker_or_events {
                                    let _ = tx.send(ConnectionEvent::AcceptRetried(kind));
                                }
                                reap_finished(&mut connections);
                                thread::sleep(config.accept_error_backoff);
                                continue;
                            }
                            None => break Err(err.into()),
                        }
                    }
                };
                let handshake_timeout = config.handshake_timeout;
                let events = checker_or_events.clone();
                let join = scope.spawn(move || {
                    connection::handle(stream, handshake_timeout, events, runtime_ref);
                });
                connections.push((shutdown_handle, join));
            };

            // Shutdown order, each step only after the last: stop
            // accepting (the loop above already has, on every path out
            // of it), close the listener, shut down and join every
            // connection still open - both still inside this same scope,
            // see below for why - then (outside it) stop the runtime,
            // remove the socket file, release the lock last - never
            // before the path is fully vacated.
            //
            // **The listener closes here, immediately after the loop
            // above stops accepting, rather than after the join below**
            // - a review of this task found an earlier version left a
            // window open between the two, during which the listener
            // was still bound while this function was busy shutting
            // down and joining already-open connections (a step that,
            // for many open connections, is not instant). A client
            // whose `connect()` landed in that window would have been
            // queued by the kernel into the listener's own backlog -
            // accepted at the OS level, but never reaching this code's
            // own `accept()` again - and then reset once the listener
            // eventually closed, rather than refused outright. Closing
            // the listener first closes that window entirely: dropping
            // it here affects only *new* connection attempts (immediately
            // refused, nothing queues after this), never the
            // already-accepted connections being joined just below,
            // which live on their own, independent file descriptors the
            // listener never touches.
            drop(listener);

            for (shutdown_handle, join) in connections {
                let _ = shutdown_handle.shutdown(Shutdown::Both);
                let _ = join.join();
            }
            accept_result
        })
    };

    // Every connection thread has, by construction, already been joined
    // above (`thread::scope` does not return until every thread it
    // spawned has ended) - `runtime` has no outstanding borrow left at
    // this point, which this line itself proves: it would not compile
    // otherwise.
    runtime.stop();

    // The checker's channel is, by this line, fully disconnected: the
    // `move |scope| { ... }` closure above returned (dropping its own
    // `checker_tx`), every per-connection clone was joined above (its own
    // thread's local destructors run even on a panic the `let _ =
    // join.join()` discards), and `runtime.stop()` just above dropped
    // `Runtime`'s own clone (`State.notify_session_count_changed`). So
    // whichever `Wait` the checker was in - blocked with no timeout, or
    // mid-bounded-wait - its blocking call now sees the channel
    // disconnect and returns; this join's bound comes from that, never
    // from a poll interval.
    if let Some(checker_handle) = checker_handle {
        let _ = checker_handle.join();
    }

    let _ = std::fs::remove_file(socket_path);
    drop(lock);
    loop_result
}

/// Resolves `config.socket_path` against the real environment when it is
/// `None` - the same preference order the sdk's own `local_socket`
/// provider uses (`STEG_SOCKET_PATH`, `XDG_RUNTIME_DIR`, `TMPDIR`, the
/// `USER` fallback), read here and fed into the sdk's own pure
/// `steg_sdk::socket_path` rather than duplicated. Every one of this
/// task's own tests sets `config.socket_path` explicitly instead, so
/// this is only ever exercised for real by a caller that does not.
///
/// `pub` (TASK-1.77): `crates/steg`'s own entry point needs the exact same
/// path `attempt`/`run` will resolve, to print and connect-test against
/// before either of those functions is called - re-deriving this logic a
/// second time in `crates/steg` would be the second definition this
/// function's own doc already argues against.
#[must_use]
pub fn resolve_socket_path(config: &ServerConfig) -> std::path::PathBuf {
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

#[cfg(test)]
mod tests {
    use std::os::unix::net::UnixStream;
    use std::sync::mpsc;
    use std::time::Duration;

    use super::{RecoverableAcceptError, classify_accept_error, reap_finished};

    /// AC #1: the three named cases, each a real `std::io::Error` built
    /// from a real errno value (`from_raw_os_error`, not a mock of
    /// `accept()` itself) - every one of these is a value the real
    /// `accept()`/`try_clone()` calls in `run`'s loop can actually return
    /// on this platform.
    #[test]
    fn descriptor_exhaustion_is_recoverable() {
        assert_eq!(
            classify_accept_error(&std::io::Error::from_raw_os_error(nix::libc::EMFILE)),
            Some(RecoverableAcceptError::DescriptorsExhausted)
        );
        assert_eq!(
            classify_accept_error(&std::io::Error::from_raw_os_error(nix::libc::ENFILE)),
            Some(RecoverableAcceptError::DescriptorsExhausted)
        );
    }

    #[test]
    fn a_connection_aborted_before_accept_picked_it_up_is_recoverable() {
        assert_eq!(
            classify_accept_error(&std::io::Error::from_raw_os_error(nix::libc::ECONNABORTED)),
            Some(RecoverableAcceptError::ConnectionAborted)
        );
    }

    #[test]
    fn an_interrupted_call_is_recoverable() {
        assert_eq!(
            classify_accept_error(&std::io::Error::from_raw_os_error(nix::libc::EINTR)),
            Some(RecoverableAcceptError::Interrupted)
        );
    }

    /// The negative control for the three tests above: two plausible but
    /// wrong neighbours (not an exhaustive list) and "no errno at all" -
    /// each must come back fatal, or a classifier that wrongly folded
    /// everything into `Some` would pass the recoverable tests above for
    /// the wrong reason.
    #[test]
    fn an_unlisted_errno_is_fatal() {
        assert_eq!(
            classify_accept_error(&std::io::Error::from_raw_os_error(nix::libc::EBADF)),
            None
        );
        assert_eq!(
            classify_accept_error(&std::io::Error::from_raw_os_error(nix::libc::EINVAL)),
            None
        );
    }

    #[test]
    fn an_error_with_no_os_code_at_all_is_fatal() {
        let err = std::io::Error::other("not from a syscall");
        assert_eq!(err.raw_os_error(), None);
        assert_eq!(classify_accept_error(&err), None);
    }

    /// A thread that finishes the instant it is spawned leaves
    /// `is_finished()` `true` almost immediately - this test waits on
    /// that (a real event), not a sleep, before calling `reap_finished`.
    fn wait_until_finished(join: &std::thread::JoinHandle<()>) {
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while !join.is_finished() {
            assert!(
                std::time::Instant::now() < deadline,
                "thread never reported finished"
            );
            std::thread::sleep(Duration::from_millis(1));
        }
    }

    #[test]
    fn a_finished_connection_is_removed_and_a_still_open_one_is_kept() {
        let (_finished_keepalive, finished_stream) = UnixStream::pair().unwrap();
        let (_open_keepalive, open_stream) = UnixStream::pair().unwrap();

        let (done_tx, done_rx) = mpsc::channel::<()>();
        let finished_join = std::thread::spawn(|| {
            // Returns immediately - this connection has already ended.
        });
        wait_until_finished(&finished_join);

        // Still genuinely blocked (never told to proceed), standing in
        // for a connection still open and holding its thread.
        let open_join = std::thread::spawn(move || {
            let _ = done_rx.recv();
        });

        let mut connections = vec![(finished_stream, finished_join), (open_stream, open_join)];

        reap_finished(&mut connections);

        assert_eq!(
            connections.len(),
            1,
            "the finished connection should have been removed, the open one kept"
        );
        assert!(!connections[0].1.is_finished());

        // Let the still-open thread end so the test does not leak it.
        drop(done_tx);
        connections.pop().unwrap().1.join().unwrap();
    }

    #[test]
    fn an_empty_set_is_left_empty() {
        let mut connections: Vec<(UnixStream, std::thread::JoinHandle<()>)> = Vec::new();
        reap_finished(&mut connections);
        assert!(connections.is_empty());
    }

    /// TASK-1.86 (idle stop): the structural proof that an explicit start
    /// (`idle_timeout: None`) never idle-stops, because it never even
    /// builds the thread that could - not a timing-bounded test, which
    /// could only ever show "didn't stop within however long we waited,"
    /// never "never." `build_idle_wiring` is private, so this test lives
    /// here rather than in a separate integration-test file, which could
    /// not reach it.
    #[test]
    fn build_idle_wiring_spawns_no_thread_when_idle_timeout_is_none() {
        let runtime = crate::runtime::Runtime::new(
            std::sync::Arc::new(crate::pty::RealLauncher::default()),
            crate::runtime::LoginShell::default(),
            Duration::from_millis(1),
            Duration::from_millis(1),
        );
        let stop = super::Stopper::new();

        let wiring = super::build_idle_wiring(None, &runtime, &stop);

        assert!(
            wiring.is_none(),
            "idle_timeout: None must build neither a channel nor a thread"
        );
    }
}
