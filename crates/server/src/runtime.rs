//! The server's one owning structure: the kernel's tree and every pane's
//! real runtime, and the one entry point a connection's request reaches
//! both through (TASK-1.76 §1; doc-16 R1, "the server is where I/O meets
//! the kernel").
//!
//! [`crate::dispatch::dispatch`] stays exactly as it already is - a plain
//! function over `&mut Tree`, no I/O in its signature - for every method
//! except `session.new`/`tab.new`/`pane.split`, the only three that need
//! a real process started. Those three are intercepted here, *before*
//! falling through to the pure `dispatch`, because starting a program
//! needs the launcher and the pane handles `dispatch` never sees.

use std::collections::HashMap;
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use steg_kernel::{
    CommandEnded, CommandOutputStarted, ExitOutcome, Moment, Settings, SizeChanged, Tree,
};
use steg_sdk::{
    ApiError, ClientId, ClientKind, Connection, Description, ErrorCode, PaneId, Program, Request,
    Response, SessionId, validate,
};

use crate::connection::ConnectionEvent;
use crate::dispatch::{self, Context, Filter, Outcome};
use crate::env::pane_environment;
use crate::outbound::Outbound;
use crate::pane::{self, DEFAULT_PANE_SIZE, PaneEvent, PaneHandle};
use crate::program_param::program_from_request;
use crate::pty::{LaunchSpec, ProgramLauncher};

/// The login shell's resolved identity and captured environment - the one
/// value [`Runtime`] is given rather than reading `SHELL`/`HOME` from the
/// process's own (possibly client-inherited) environment in several
/// places (TASK-1.76 §2; doc-8 R99).
///
/// Resolved in two steps, at two different points in the server's own
/// start-up (TASK-1.87 §3): [`LoginShell::resolve_identity`] is cheap (a
/// local passwd lookup, no child process) and runs *before* the socket is
/// bound, at `crates/steg/src/main.rs`'s existing pre-`attempt` call
/// site - its own `base_env` is always empty, since nothing has captured
/// one yet. `base_env` itself is filled in afterward, once, inside
/// [`crate::listen::run`] - after the socket is already bound and before
/// its accept loop starts - by calling
/// [`crate::login_env::capture_login_environment`] with this struct's own
/// `argv0`/`home`/`user` and replacing `base_env` on the result. Neither
/// step is a method on this struct: both are free functions in
/// `crate::login_env`, since only the first one needs no identity to work
/// from yet.
#[derive(Debug, Clone)]
pub struct LoginShell {
    /// What [`steg_sdk::Program::Shell`] resolves to when a pane actually
    /// starts, e.g. `/bin/zsh`.
    pub argv0: String,
    /// The default working directory when nothing else applies: no `cwd`
    /// was given, and (rule B, §2) there is no pane to inherit one from,
    /// or it has not reported one yet.
    pub home: String,
    /// The account's own login name, from the same password-database
    /// lookup as `argv0`/`home` - carried here so
    /// [`crate::login_env::capture_login_environment`] can seed `USER`/
    /// `LOGNAME` in the capture it runs later, without a second lookup.
    /// `None` only when the lookup itself found no identity at all
    /// (TASK-1.87's own honest gap: unreproducible on a real account).
    pub user: Option<String>,
    /// The login shell's own exported environment, captured once at
    /// server start (TASK-1.87) - empty until
    /// [`crate::listen::run`] fills it in; never read before then, since
    /// nothing starts a pane before the socket is even bound.
    pub base_env: Vec<(std::ffi::OsString, std::ffi::OsString)>,
}

impl Default for LoginShell {
    /// A fixed, zero-I/O value - deliberately *not* a real password
    /// -database lookup: `ServerConfig::default()` (and every test that
    /// builds one with `..ServerConfig::default()`) must stay free of
    /// hidden syscalls, so the real resolution is
    /// [`LoginShell::resolve_identity`], called explicitly, never this.
    fn default() -> Self {
        Self {
            argv0: "/bin/zsh".to_string(),
            home: "/".to_string(),
            user: None,
            base_env: Vec::new(),
        }
    }
}

impl LoginShell {
    /// Resolves the login identity from the password database
    /// (`crate::login_env::resolve_login_identity`, TASK-1.87) - never
    /// from `$SHELL`/`$HOME`/`$USER` in this process's own environment,
    /// which is exactly what R99 forbids. `base_env` is always empty on
    /// the result: this step runs before the socket is bound, and
    /// capturing the real environment is deliberately deferred past that
    /// point (§3 of this task's own plan) so a slow shell never delays a
    /// client's `connect()`, only its first request.
    #[must_use]
    pub fn resolve_identity(timeout: std::time::Duration) -> Self {
        let identity = crate::login_env::resolve_login_identity(timeout);
        Self {
            argv0: identity.shell.to_string_lossy().into_owned(),
            home: identity.home.to_string_lossy().into_owned(),
            user: identity.user,
            base_env: Vec::new(),
        }
    }
}

/// Captures `identity`'s own login shell environment and builds the
/// [`LoginShell`] a caller passes to [`Runtime::new`] from it - the
/// "capture, then report a fallback" step `crates/steg/src/main.rs`'s
/// own `run_server_start` needs, written once so it is not duplicated at
/// TASK-1.87's own fixture-only entry point for tests
/// (`crates/steg/tests/fixtures/fixture_server.rs`), which calls this
/// with a *fabricated* identity (a throwaway home, never the password
/// database) instead of [`LoginShell::resolve_identity`]'s real one -
/// everything from this call onward is the real path: the real capture,
/// the real launcher, a real client over a real socket.
///
/// Returns the reason alongside the built `LoginShell` on a fallback,
/// rather than printing anything itself - this crate does no I/O of that
/// kind (no `eprintln!`/`println!` appears anywhere in `crates/server/
/// src`, and this function keeps that true); the caller decides whether
/// and how to report it.
#[must_use]
pub fn build_login_shell(
    identity: crate::login_env::LoginIdentity,
    environment_timeout: std::time::Duration,
) -> (LoginShell, Option<crate::login_env::FallbackReason>) {
    let (base_env, fallback_reason) =
        match crate::login_env::capture_login_environment(&identity, environment_timeout) {
            crate::login_env::Capture::Captured(env) => (env, None),
            crate::login_env::Capture::Fallback(reason, env) => (env, Some(reason)),
        };
    let login = LoginShell {
        argv0: identity.shell.to_string_lossy().into_owned(),
        home: identity.home.to_string_lossy().into_owned(),
        user: identity.user,
        base_env,
    };
    (login, fallback_reason)
}

/// The kernel's tree, and every pane's real runtime, held together under
/// one lock (TASK-1.76 §1): many operations need both atomically (a
/// forced close, once built, removes a `PaneHandle` and closes the tree's
/// own pane in the same step), so one lock avoids a two-lock ordering
/// hazard rather than trading it for one.
struct State {
    tree: Tree,
    panes: HashMap<PaneId, PaneHandle>,
    // TASK-1.48 merged into `origin/main` while this task was being
    // built (named in the plan's own Risks as something to adapt to,
    // not guessed at): `dispatch` and `api::call` both now take a
    // `&mut Settings`. This task reads no setting, so an empty store,
    // threaded through unchanged, is all `Runtime` needs for now.
    settings: Settings,
    // TASK-1.41 merged into `origin/main` while this task was being
    // fixed up (same Risks entry): `Tree::record_exit` now takes a
    // `steg_kernel::Moment`, the clock-free point in time doc-11's own
    // richer status needs. `clock_origin` is this `Runtime`'s own
    // construction time - `Moment::new`'s own doc says the origin never
    // matters as long as it is picked once and reused, and a `Runtime`'s
    // own start is as good a pick as any (the server's real clock,
    // converted at the one place `apply_event_locked` calls
    // `record_exit`, never read by the kernel itself).
    clock_origin: Instant,
    /// Every connection that can receive a broadcast event (TASK-1.46 §7):
    /// registered the moment a connection is usable at all (`register_
    /// subscriber`, called right after the handshake, before any request
    /// can arrive), deregistered on every exit path (`deregister_
    /// subscriber`). A registered connection with `filter: None` has
    /// never called `event.watch` - it receives nothing, never
    /// unfiltered (`broadcast` only ever matches a `Some` filter).
    subscribers: HashMap<ClientId, Subscriber>,
    /// Told once, by `listen::run`, only when idle-checking is on
    /// (TASK-1.86) - every cascade that closes a session (R94), whether
    /// driven by a pane's own exit or by a request (`pane.close`/`tab.
    /// close`/`session.close`/`pane.move`, TASK-1.46), sends on this
    /// afterward, so the idle checker, blocked with no timeout while a
    /// session exists, learns the instant that stops being true - even
    /// when the connection that caused it is still around (a session
    /// closed by a request is also covered by that connection's own
    /// events, but the idle checker reads this channel, not a
    /// subscriber's broadcast, so the notify still has to fire). `None`
    /// until `listen::run` calls `Runtime::set_idle_notifier`; always
    /// `None` for an explicit start, which has no idle checker to tell.
    notify_session_count_changed: Option<Sender<ConnectionEvent>>,
    /// How many `PaneEvent::CommandOutputStarted`/`CommandEnded` events
    /// `apply_event_locked` has applied for each pane (TASK-1.84 review):
    /// test-only, since nothing outside a test needs it. A mark the
    /// kernel correctly ignores has no other outside effect by design,
    /// so a test proving that has nothing else to wait on - this is the
    /// one fact that is true exactly when this pane's own marks have
    /// been applied, whatever the kernel then did with them, read under
    /// the same lock `apply_event_locked` updates it under, never
    /// inferred from a sleep or from a different pane's own marks.
    #[cfg(test)]
    applied_marks: HashMap<PaneId, u64>,
}

/// One registered connection's own broadcast handle and subscription
/// filter (TASK-1.46 §7).
struct Subscriber {
    outbound: Arc<Outbound>,
    /// `None` until `event.watch` runs on this connection.
    filter: Option<Filter>,
    /// A clone of this connection's own socket, held only to shut it down
    /// from `broadcast` (AC #3: a slow subscriber is closed, not
    /// resynced). `Outbound::close()` alone stops the writer thread but
    /// does not touch the socket, and this connection's own reader is
    /// blocked in a plain `read()` with no idle timeout (TASK-1.45 §4) -
    /// nothing else can make that read return from another thread.
    shutdown: std::os::unix::net::UnixStream,
}

/// Owns the kernel's tree and every pane's real runtime, and is the one
/// place a connection's request reaches both. Construct one per server
/// process (TASK-1.44/1.45 will do this once a socket exists; today only
/// this crate's own tests construct one).
pub struct Runtime {
    state: Arc<Mutex<State>>,
    launcher: Arc<dyn ProgramLauncher>,
    login: LoginShell,
    // `Option`-wrapped so `stop` (review finding 3) can `take()` each one
    // exactly once - idempotent, since a second call finds `None` and
    // does nothing further, which is what lets both an explicit call and
    // `Drop` share the one implementation safely.
    events: Option<Sender<PaneEvent>>,
    drain: Option<JoinHandle<()>>,
    // TASK-1.78 §3: a named, injected value, the same pattern `login`
    // already uses - production reads `FORCE_GRACE_DEFAULT`; this crate's
    // own tests pass a tiny value, so the suite's own total time does not
    // grow by the production grace on every test that exercises forcing.
    force_grace: Duration,
}

/// The production default for [`Runtime::new`]'s own `force_grace`
/// parameter (TASK-1.78 §3) - two seconds: long enough that a program
/// doing real cleanup on the polite signal (flushing a file, a graceful
/// shutdown handler) has a realistic chance, short enough that a person
/// who asked to force-close is not left waiting much longer than they
/// would be for an ordinary close. Read by `listen::run`'s own `Runtime::
/// new` call (TASK-1.45 §1), the real call site this was named for
/// before that socket existed to call it.
pub const FORCE_GRACE_DEFAULT: Duration = Duration::from_secs(2);

/// How long [`force_end`] waits, after escalating, for a program that
/// ignored the polite signal to actually die (TASK-1.78 §2) - `SIGKILL`
/// cannot be trapped or blocked, so this bounds only OS scheduling and
/// reap latency, never a real wait for cooperation.
const ESCALATE_CONFIRM_TIMEOUT: Duration = Duration::from_secs(2);

/// How long [`force_end`] waits to join each worker thread once its own
/// run has been confirmed ended (TASK-1.78 §2) - the same reasoning as
/// `pane`'s own `RELAY_JOIN_TIMEOUT`: by that point the worker has already
/// broken its loop (`PaneCommand::AwaitExit`'s own fix, TASK-1.78 §1) and
/// is on its way out, so this is a hard guarantee, not a real wait.
const FORCE_JOIN_TIMEOUT: Duration = Duration::from_secs(2);

impl Runtime {
    /// Builds an empty `Runtime` (no sessions yet) and starts its one
    /// drain thread, which applies every [`PaneEvent`] a pane's real
    /// program reports - most importantly, a natural exit - to the tree
    /// (§1). `force_grace` is the grace period `force_end` gives a forced
    /// pane's own polite signal before escalating (§3) - injected, not
    /// read from anywhere, so a caller choosing to construct a `Runtime`
    /// decides it explicitly (`FORCE_GRACE_DEFAULT` for production).
    ///
    /// # Panics
    ///
    /// Never in practice: the only failure `thread::Builder::spawn` has
    /// is the OS refusing a new thread (out of resources), which this
    /// treats as fatal to the whole server rather than returning a
    /// `Runtime` that silently never drains any event.
    #[must_use]
    pub fn new(
        launcher: Arc<dyn ProgramLauncher>,
        login: LoginShell,
        force_grace: Duration,
    ) -> Self {
        let (events_tx, events_rx) = mpsc::channel();
        let state = Arc::new(Mutex::new(State {
            tree: Tree::new(),
            panes: HashMap::new(),
            settings: Settings::new(),
            clock_origin: Instant::now(),
            subscribers: HashMap::new(),
            notify_session_count_changed: None,
            #[cfg(test)]
            applied_marks: HashMap::new(),
        }));

        let drain_state = Arc::clone(&state);
        let drain = thread::Builder::new()
            .name("steg-runtime-drain".to_string())
            .spawn(move || {
                for event in events_rx {
                    let mut state = drain_state.lock().unwrap_or_else(|poisoned| {
                        // A pane's own worker thread panicking must not
                        // make the drain thread itself unable to make
                        // progress on every other pane - recovering the
                        // guard is the standard, narrow escape for a
                        // poisoned `Mutex` when the data itself is still
                        // structurally fine (R94's own invariants are
                        // upheld by the kernel's own code, not by this
                        // lock), not a panic-propagation path this thread
                        // should take.
                        poisoned.into_inner()
                    });
                    apply_event_locked(&mut state, event);
                }
            })
            .expect("spawning the one drain thread at construction");

        // `events_tx` is the `Sender` half every real pane start/restart
        // clones (`pane::start`'s own signature); nothing in `Runtime`
        // keeps the original beyond that, but the field below is what
        // every later call reads.
        Self {
            state,
            launcher,
            login,
            events: Some(events_tx),
            drain: Some(drain),
            force_grace,
        }
    }

    /// The one entry point a connection's request reaches (TASK-1.44/1.45
    /// will call this once a socket exists). For every method except the
    /// three that start a real program, this delegates straight to the
    /// existing, unchanged [`dispatch::dispatch`] - `dispatch` itself
    /// never touches the launcher or a `PaneHandle`.
    #[must_use]
    pub fn handle(
        &self,
        description: &Description,
        context: &Context,
        request: &Request,
    ) -> Response {
        // Review finding 1: `api.call` must reach the same start-wrapper
        // a direct call does, or a program minted this way is left
        // forever unstarted. Resolved *first*, before anything else -
        // the same ordering `dispatch::dispatch` itself already uses for
        // `api.call` - through `dispatch::resolve_api_call`, the one
        // implementation `dispatch::api::call` also uses, so the
        // selector's own key name (read inside that function, never
        // here) can change (TASK-1.81) with nothing in this file to
        // update. From here on, `resolved` is what every match below
        // reads - never the original `request` - since `request.method`
        // is `"api.call"` itself whenever this branch ran at all.
        let resolved = if request.method == "api.call" {
            match dispatch::resolve_api_call(request) {
                Ok(inner) => inner,
                Err(error) => return Response::Error(error),
            }
        } else {
            request.clone()
        };

        match resolved.method.as_str() {
            "session.new" | "tab.new" | "pane.split" => {
                self.start_and_dispatch(description, context, &resolved)
            }
            "pane.close" | "tab.close" | "session.close" => {
                self.close_and_dispatch(description, context, &resolved)
            }
            "pane.move" => self.move_and_dispatch(description, context, &resolved),
            _ => {
                let mut guard = self.lock();
                // One explicit deref first: `&mut guard.tree`/`&mut
                // guard.settings` in the same call would each need their
                // own `DerefMut::deref_mut(&mut guard)`, which the borrow
                // checker cannot prove don't alias even though they
                // project to disjoint fields afterward - deref'ing once
                // up front turns this into plain struct field access,
                // which it can split-borrow normally.
                let state: &mut State = &mut guard;
                let now = Moment::new(Instant::now().duration_since(state.clock_origin));
                let response = dispatch::dispatch(
                    &mut state.tree,
                    &mut state.settings,
                    description,
                    context,
                    &resolved,
                    now,
                );
                if matches!(response, Response::Ok(_)) {
                    reapply_every_sized_tab(state);
                    emit_focus_event(state, context, &resolved);
                }
                response
            }
        }
    }

    /// Applies a kernel [`SizeChanged`] to every pane it names (TASK-1.76
    /// §4) - the seam a later task's own `client.*` methods call directly
    /// once they declare a size or change who is latest (decision-6),
    /// rather than reapplying it themselves. Self-locking, for a caller
    /// that does not already hold this `Runtime`'s lock; [`Runtime::handle`]
    /// itself calls the already-locked [`apply_size_changed_locked`]
    /// instead, to avoid re-locking a `Mutex` it is already holding.
    pub fn apply_size_changed(&self, change: &SizeChanged) {
        let mut state = self.lock();
        apply_size_changed_locked(&mut state, change);
    }

    /// Mints a real, tree-tracked client (TASK-1.45 §1): `connection::
    /// handle` calls this once, right after a successful handshake, before
    /// the `ServerHello` is built - replacing `connection.rs`'s own
    /// previous `next_local_client_id()`, a local counter with no kernel
    /// backing at all (its own doc named wiring a real model in as this
    /// task's job). Self-locking, same shape as [`Runtime::apply_size_changed`].
    pub fn connect_client(&self, kind: ClientKind, label: Option<String>) -> ClientId {
        self.lock().tree.connect_client(kind, label)
    }

    /// Disconnects a client this `Runtime` minted (TASK-1.45 §1), applying
    /// every `SizeChanged` the kernel reports as a consequence (a
    /// disconnecting client may have been the latest declarer for some
    /// tab's size, decision-6) - the same `apply_size_changed_locked` every
    /// other mutation already reapplies through. Called once, on every
    /// path out of `connection::handle` (ordinary close, a protocol
    /// violation, a panic unwinding past it).
    ///
    /// # Panics
    /// Never in practice: the only way `Tree::disconnect_client` can fail
    /// is an id this `Runtime` never minted, which `connection::handle`
    /// never passes (it only ever passes back the id its own, prior
    /// `connect_client` call returned).
    pub fn disconnect_client(&self, id: ClientId) {
        let mut state = self.lock();
        let changes = state
            .tree
            .disconnect_client(id)
            .expect("connection::handle only ever disconnects a client it minted itself");
        for change in &changes {
            apply_size_changed_locked(&mut state, change);
        }
    }

    /// Registers a connection as eligible for broadcast (TASK-1.46 §7):
    /// called once, by `connection::handle`'s own `run_post_handshake`,
    /// right after its writer (`outbound`) is spawned and before the read
    /// loop that could bring in `event.watch` even starts - so a
    /// connection is never asked to subscribe before it is registered
    /// (`subscribe`'s own `.expect` relies on this). `filter: None` until
    /// `event.watch` runs: a registered-but-unfiltered connection receives
    /// nothing from `broadcast`, never everything.
    pub(crate) fn register_subscriber(
        &self,
        client_id: ClientId,
        outbound: Arc<Outbound>,
        shutdown: std::os::unix::net::UnixStream,
    ) {
        self.lock().subscribers.insert(
            client_id,
            Subscriber {
                outbound,
                filter: None,
                shutdown,
            },
        );
    }

    /// Deregisters a connection (TASK-1.46 §7): called on every exit path
    /// out of a connection (`connection::handle`'s own `ConnectionCleanup`
    /// guard), so `broadcast` never holds a handle to a writer that is
    /// gone. Idempotent: deregistering a connection that was never
    /// registered, or already was, does nothing further.
    pub fn deregister_subscriber(&self, client_id: ClientId) {
        self.lock().subscribers.remove(&client_id);
    }

    /// `event.watch` (AC #1, #3, #4): registers this connection's own
    /// filter and replies with the tree, atomically - both inside the one
    /// lock acquisition, so nothing broadcast by another thread between
    /// them can ever reach the wire ahead of this reply (TASK-1.46's own
    /// plan, section 4, the race its first draft missed). Bypasses the
    /// generic `Runtime::handle`/`Connection::reply`-afterward path
    /// entirely: `connection::handle` calls this directly (after
    /// resolving any `api.call` wrapping itself, so that path reaches
    /// here too) instead of calling `handle` and enqueueing its own
    /// reply - the reply is already on the wire by the time this returns
    /// `Ok`.
    ///
    /// On `Err`, nothing is registered or changed: the caller builds and
    /// enqueues the ordinary error reply itself, which needs no
    /// atomicity (nothing was broadcast as part of answering a request
    /// that never actually subscribed).
    ///
    /// # Errors
    /// The same validation and target-resolution errors `dispatch::
    /// dispatch` would give — a request `event.watch` does not declare, a
    /// malformed `for`/`all`, or no session to resolve (`resolve_filter`).
    ///
    /// # Panics
    /// Never in practice: `client_id` is only ever one `connection::
    /// handle` passed back, which always calls `register_subscriber`
    /// first - the same invariant `start_and_dispatch`'s own panics
    /// already rest on elsewhere in this file.
    pub fn subscribe(
        &self,
        description: &Description,
        context: &Context,
        request: &Request,
        client_id: ClientId,
        request_id: u64,
    ) -> Result<(), ApiError> {
        let mut state = self.lock();
        validate(description, request)?;
        let filter = dispatch::resolve_filter(&state.tree, context, request)?;
        let now = Moment::new(Instant::now().duration_since(state.clock_origin));
        let snapshot = dispatch::tree_snapshot(&state.tree, now);

        let subscriber = state.subscribers.get_mut(&client_id).expect(
            "register_subscriber already ran, before any request could arrive on this connection",
        );
        subscriber.filter = Some(filter);

        let value = serde_json::to_value(snapshot).expect("TreeSnapshot always serialises");
        let bytes = Connection::reply(request_id, Response::Ok(value));
        // A failed enqueue means this connection's queue is already
        // closed or full - it is tearing down concurrently (its own
        // `deregister_subscriber` is on its way), and there is nothing
        // further to do: the request still succeeded (the filter is set,
        // the tree was read), there is simply no one left to write the
        // reply to.
        let _ = subscriber.outbound.enqueue_control(bytes);
        Ok(())
    }

    /// A cheap, repeatable way to read the live session count, usable
    /// from a thread that outlives any borrow of `self` (`listen::run`'s
    /// idle checker, TASK-1.86, which must outlive the `&Runtime` every
    /// connection thread borrows inside `thread::scope`). `pub(crate)`:
    /// nothing outside this crate needs it.
    pub(crate) fn session_counter(&self) -> impl Fn() -> usize + Send + 'static {
        let state = Arc::clone(&self.state);
        move || {
            state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .tree
                .sessions()
                .len()
        }
    }

    /// Told once, by `listen::run`, only when idle-checking is on
    /// (TASK-1.86) - every pane-exit cascade that closes a session (R94)
    /// sends `ConnectionEvent::SessionCountChanged` on this afterward
    /// (`apply_event_locked`'s own `PaneEvent::Exited` arm, below). Safe
    /// to call before any pane can possibly exist: `listen::run` calls
    /// this immediately after constructing the `Runtime`, before its
    /// accept loop starts accepting anything, and a pane is only ever
    /// minted by a request on an already-accepted, already-handshaken
    /// connection.
    pub(crate) fn set_idle_notifier(&self, notify: Sender<ConnectionEvent>) {
        self.lock().notify_session_count_changed = Some(notify);
    }

    /// Stops this `Runtime`: ends every running program, reaps each one,
    /// joins every worker thread, then joins the one drain thread too
    /// (review finding 3; rewritten for TASK-1.78 §4 to reuse [`force_end`],
    /// the same mechanism a single pane's forced close uses, not the
    /// old `PaneHandle::stop`'s pid-only, unescalated `SIGHUP`, which
    /// TASK-1.76's own reviewer confirmed left a program trapping that
    /// signal running forever even once the whole server tried to stop).
    /// A behaviour change, named rather than folded in silently: this used
    /// to be serial (`N * STOP_TIMEOUT` worst case, 5s per pane); now it is
    /// `force_grace + ESCALATE_CONFIRM_TIMEOUT + FORCE_JOIN_TIMEOUT` for
    /// *any* number of panes, reaching a trapping program and a
    /// non-job-control grandchild (§0.4) the old mechanism could not. This
    /// is the method `listen::run` calls, once every connection thread
    /// has already been joined (TASK-1.45 §1 - `&mut self` is what makes
    /// that a property the compiler itself enforces there, not merely
    /// documented).
    ///
    /// Idempotent: every field this empties is `Option`-wrapped, so a
    /// second call (including the one `Drop` makes) finds nothing left to
    /// do and returns at once. Every request this `Runtime` is given
    /// after this call panics (`self.events`'s own `.expect()` inside
    /// `start_and_dispatch`) rather than silently starting a program this
    /// `Runtime` can no longer track - stopping is meant to be the last
    /// thing done with one.
    pub fn stop(&mut self) {
        let panes = {
            let mut state = self.lock();
            // Drops this `Runtime`'s own clone of the idle checker's
            // notifier (TASK-1.86), if any - one of the senders
            // `listen::run`'s own join bound (section 6 of this task's
            // plan) is derived against: a checker blocked with no
            // timeout at all (a session still open when something other
            // than its own decision calls `stop`) can only be woken by
            // its channel disconnecting, which needs every sender into
            // it gone, this one included.
            state.notify_session_count_changed = None;
            std::mem::take(&mut state.panes)
        };
        force_end(panes.into_iter().collect(), self.force_grace);

        // Dropping every clone of `events` (this one, the last) is what
        // lets the drain thread's own `for event in events_rx` end -
        // every pane's own clone already went with its worker thread,
        // stopped just above.
        drop(self.events.take());
        if let Some(drain) = self.drain.take() {
            // The same hard-guarantee reasoning as `FORCE_JOIN_TIMEOUT`:
            // every pane's own clone of `events` is already gone by this
            // point (dropped above, inside `force_end`'s own joins), so
            // the drain thread's loop has nothing left to read and is
            // already on its way out.
            let _ = crate::pane::join_with_bound(drain, FORCE_JOIN_TIMEOUT);
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// `session.new`/`tab.new`/`pane.split`: mints through the existing,
    /// unchanged `dispatch` arm, then starts the real program the plan's
    /// §2 describes, under the same lock the whole way through (§1's own
    /// justification for why that lock, not a narrower one, is what keeps
    /// "nothing else runs between minting and starting" true with no real
    /// control thread yet).
    fn start_and_dispatch(
        &self,
        description: &Description,
        context: &Context,
        request: &Request,
    ) -> Response {
        let mut state = self.lock();

        // `validate` runs again inside `dispatch` below (harmless,
        // cheap): it has to run here first too, since this function reads
        // `request.params` itself (to resolve the cwd default) before
        // `dispatch` ever does its own check.
        if let Err(error) = validate(description, request) {
            return Response::Error(error);
        }

        let default_cwd = self.resolve_default_cwd(&state, context, request);
        let program = program_from_request(request, &default_cwd);

        // Finding 4: a nonexistent directory is never "success for
        // something that did not happen". `default_cwd` above has
        // already fallen back to `login.home` for every case this
        // request named no `cwd` of its own (rule B), so the only way
        // `program.cwd()` can still fail this check is a caller-given
        // `cwd` that does not exist - checked here, before anything is
        // minted, so refusing leaves nothing behind at all (no rollback
        // needed, unlike the spawn-failure path below).
        if !std::path::Path::new(program.cwd()).is_dir() {
            return Response::Error(ApiError::new(
                ErrorCode::ProgramFailed,
                format!("cwd does not exist: {}", program.cwd()),
            ));
        }

        let response = {
            // See `handle`'s own comment: one explicit deref so the two
            // `&mut` arguments below project to disjoint fields of a
            // plain `&mut State`, not two overlapping `DerefMut` calls on
            // `state` (the `MutexGuard`) itself.
            let state: &mut State = &mut state;
            let now = Moment::new(Instant::now().duration_since(state.clock_origin));
            dispatch::dispatch(
                &mut state.tree,
                &mut state.settings,
                description,
                context,
                request,
                now,
            )
        };
        let Response::Ok(value) = response else {
            // `dispatch`'s own arm already refused or rolled back
            // whatever it does internally (the same label-rejection
            // rollback `pane.split` already had) - nothing further to
            // undo here, since nothing past this point ever ran.
            return response;
        };

        let Some(new_pane) = new_pane_from(&state.tree, request.method.as_str(), &value) else {
            // Never in practice: `dispatch` just returned `Ok` for one of
            // these three methods, which always mints exactly one pane
            // (R94) - returned as a clean error rather than a panic only
            // because this is a response path a malformed future change
            // could otherwise reach silently.
            return Response::Error(ApiError::new(
                ErrorCode::ProgramFailed,
                "the new pane could not be found right after it was minted",
            ));
        };

        state
            .tree
            .start_program(new_pane, program.clone())
            .expect("just minted this pane; nothing else has run since (§1)");

        let size = state
            .tree
            .tab_sizes_now(
                state
                    .tree
                    .tab_of_pane(new_pane)
                    .expect("just started this pane's program; its tab still holds it"),
            )
            .and_then(|change| {
                change
                    .panes
                    .iter()
                    .find(|(pane, _)| *pane == new_pane)
                    .map(|&(_, size)| (size.columns(), size.rows()))
            })
            .unwrap_or(DEFAULT_PANE_SIZE);

        let spec = self.launch_spec(&program, size);

        match pane::start(
            spec,
            new_pane,
            Arc::clone(&self.launcher),
            self.events
                .clone()
                .expect("starting a program after this Runtime has been stopped"),
        ) {
            Ok(handle) => {
                state.panes.insert(new_pane, handle);
                reapply_every_sized_tab(&mut state);
                // Creation events (§5 of this task's own plan): emitted
                // only now, after the real spawn has actually succeeded -
                // never at the earlier, still-rollback-able mint a few
                // lines above, which the `Err` arm below can still undo.
                // Outermost first: a session names its own new tab and
                // pane without a subscriber having to wait for separate
                // events to learn the shape of what was just created.
                emit_creation_events(&state, request.method.as_str(), new_pane, request);
                Response::Ok(value)
            }
            Err(error) => {
                rollback_mint(&mut state.tree, request.method.as_str(), new_pane);
                Response::Error(ApiError::new(ErrorCode::ProgramFailed, error.to_string()))
            }
        }
    }

    /// `pane.close`/`tab.close`/`session.close`: closes the tree *first*,
    /// under one lock acquisition, then ends every program that scope had
    /// running - the reverse of this task's own AC #1 wording, deliberate
    /// and explained in the task's own plan (§5/§6): the instant the lock
    /// below is released, the whole scope is already gone from the tree,
    /// so *every* other method - not only a second `--force` - gets the
    /// ordinary `PaneNotFound`/`TabNotFound`/`SessionNotFound` any stale
    /// target already gets, with nothing new threaded through `dispatch`'s
    /// own lookup helpers (AC #4, in full, not only force-vs-force).
    fn close_and_dispatch(
        &self,
        description: &Description,
        context: &Context,
        request: &Request,
    ) -> Response {
        let mut state = self.lock();

        // Read *before* `dispatch_raw` runs: once a close succeeds, the
        // tree can no longer be asked which panes it used to cover.
        let scope = match dispatch::panes_in_close_scope(&state.tree, context, request) {
            Ok(scope) => scope,
            Err(error) => return Response::Error(error),
        };
        // Same reason, for this task's own closure events (§5): the one
        // thing every case needs that the kernel's own return value
        // cannot give (which session this cascade is scoped to, for
        // `broadcast`'s own filter - needed even when the cascade stops
        // at just the pane), plus, for `session.close` only, its tabs
        // grouped with their own panes (`Tree::close_session` reports no
        // structured cascade at all, unlike the other two methods - see
        // `emit_close_events`'s own doc for why this is the one case
        // still read entirely from the tree before the close).
        let pre = match capture_close_pre(&state.tree, context, request) {
            Ok(pre) => pre,
            Err(error) => return Response::Error(error),
        };

        let outcome = {
            // See `start_and_dispatch`'s own comment: one explicit deref
            // so the two `&mut` arguments below project to disjoint
            // fields of a plain `&mut State`.
            let state: &mut State = &mut state;
            let now = Moment::new(Instant::now().duration_since(state.clock_origin));
            dispatch::dispatch_raw(
                &mut state.tree,
                &mut state.settings,
                description,
                context,
                request,
                now,
            )
        };
        let outcome = match outcome {
            Ok(outcome) => outcome,
            Err(error) => {
                // The refusal (no `--force`, something running) or any
                // other error `dispatch_raw`'s own arm gives - nothing
                // changed, so nothing to pop or end.
                return Response::Error(error);
            }
        };

        // The close just ran, under the lock still held: the whole scope
        // is gone from the tree now, atomically - not partially, since
        // `close_pane`/`close_tab`/`close_session` are each one kernel
        // operation, never a loop this could interrupt midway. Emitted
        // before `force_end` runs (still under the lock, before it is
        // dropped a few lines down) - the close events describe the
        // tree, not the program's own lifecycle, and the tree has
        // already changed by this point (TASK-1.78's own plan already
        // assumed exactly this timing for this task). Reads `outcome`
        // itself (the kernel's own report of what actually closed, AC #2
        // and the review that found this task's first draft missed it
        // for `pane.close`/`tab.close`'s own cascades) before consuming
        // it into the reply's own value, below.
        emit_close_events(&state, request.method.as_str(), &pre, &scope, &outcome);

        let response = Response::Ok(outcome.into_value());

        let to_force: Vec<(PaneId, PaneHandle)> = scope
            .into_iter()
            .filter_map(|pane| state.panes.remove(&pane).map(|handle| (pane, handle)))
            .collect();
        reapply_every_sized_tab(&mut state);
        drop(state);

        // Lock-free: no other pane's request is ever stalled by this
        // (AC #4's first clause). `to_force` holds every popped handle
        // regardless of whether its own program was genuinely running -
        // `force_end` treats an already-ended one cheaply (its own
        // `signal_hangup`/`signal_kill` no-op, `await_exit` replies at
        // once), which is also this task's fix for the adjacent gap a
        // *plain* close of an already-ended pane used to leave behind
        // (its idle handle and worker thread, never cleaned up before
        // this task - not named by any of the five criteria, included
        // here because it falls out of this same pass at no extra cost).
        force_end(to_force, self.force_grace);

        response
    }

    /// `pane.move` (TASK-1.46, found one review round after `close_and_
    /// dispatch` was built the same way): `Tree::move_pane_to_tab` can
    /// cascade into closing the pane's own source tab and, if that was
    /// the session's last tab, the source session too (R94, the same
    /// cascade `close_pane` has) - the generic `_` arm in `handle` above
    /// has no way to read that cascade back out (it only ever sees a
    /// `Response`, already erased to a `Value`), so this method, like
    /// `close_and_dispatch`, calls `dispatch_raw` directly for the typed
    /// `Outcome::PaneMoved` and emits from it.
    fn move_and_dispatch(
        &self,
        description: &Description,
        context: &Context,
        request: &Request,
    ) -> Response {
        use steg_sdk::TreeEvent;

        let mut state = self.lock();

        // Captured before the move can cascade the source tab (and its
        // session) away - the same reason `capture_close_pre` reads its
        // own scope before `dispatch_raw` runs.
        let pane = match dispatch::target_pane(&state.tree, context, request) {
            Ok(pane) => pane,
            Err(error) => return Response::Error(error),
        };
        let source_tab = state
            .tree
            .tab_of_pane(pane)
            .expect("target_pane just resolved this pane; it is in the tree");
        let source_session = session_owning_tab(&state.tree, source_tab);

        let outcome = {
            let state: &mut State = &mut state;
            let now = Moment::new(Instant::now().duration_since(state.clock_origin));
            dispatch::dispatch_raw(
                &mut state.tree,
                &mut state.settings,
                description,
                context,
                request,
                now,
            )
        };
        let outcome = match outcome {
            Ok(outcome) => outcome,
            Err(error) => return Response::Error(error),
        };

        let Outcome::PaneMoved(moved) = &outcome else {
            unreachable!("pane.move always produces Outcome::PaneMoved on success")
        };
        // Innermost first, same rule as every close path: the tab before
        // the session it was the last tab of.
        if let Some(tab) = moved.source_tab_closed {
            broadcast(&state, &TreeEvent::TabClosed { id: tab }, source_session);
        }
        if let Some(session) = moved.source_session_closed {
            broadcast(
                &state,
                &TreeEvent::SessionClosed { id: session },
                source_session,
            );
        }

        let response = Response::Ok(outcome.into_value());
        reapply_every_sized_tab(&mut state);
        response
    }

    /// Rule B, §2: the live directory of the pane this request's target
    /// inherits from (the pane being split, or the session's current
    /// pane for `tab.new`), falling back to [`LoginShell::home`] in
    /// every case the chain does not produce a real answer -
    /// `session.new` has no pane to inherit from at all, so it always
    /// takes the fallback. A reported directory that no longer exists on
    /// disk (the source pane `cd`'d somewhere since removed) falls back
    /// the same way: the caller named no `cwd` of their own here, so this
    /// is rule B's own fallback, never the explicit-`cwd` refusal the
    /// engineer asked for (that one lives in `start_and_dispatch`, over
    /// whatever `cwd` the caller actually gave). Never itself a source of
    /// an error: whatever goes wrong here, `dispatch`'s own call right
    /// after surfaces the real refusal (a bad target, say) on its own.
    fn resolve_default_cwd(&self, state: &State, context: &Context, request: &Request) -> String {
        let source_pane = match request.method.as_str() {
            "pane.split" => dispatch::target_pane(&state.tree, context, request).ok(),
            "tab.new" => dispatch::target_session(&state.tree, context, request)
                .ok()
                .and_then(|session| state.tree.session(session))
                .and_then(|session| session.tab(session.current_tab()))
                .map(steg_kernel::Tab::current),
            _ => None,
        };

        source_pane
            .and_then(|pane| state.panes.get(&pane))
            .and_then(|handle| handle.cwd().ok())
            .flatten()
            .filter(|cwd| std::path::Path::new(cwd).is_dir())
            .unwrap_or_else(|| self.login.home.clone())
    }

    /// Builds the real spawn request for `program`, resolving
    /// `Program::Shell`'s own `argv0` (never decided by `dispatch`, which
    /// only ever sees the *shape*) and today's placeholder environment -
    /// `STEG_SERVER`/`STEG_BIN_DIR` have no real source yet (TASK-1.44/
    /// 1.45 build the socket and the server's own binary location this
    /// reads from instead, the same gap `pane_environment`'s own doc
    /// already names), so this reads this process's own binary location
    /// and an empty server address rather than inventing either.
    ///
    /// **The environment composition (TASK-1.87, doc-8 R99/AC #3): the
    /// captured login shell's own environment first, then
    /// `pane_environment`'s five `STEG_*` variables and `PATH`, then
    /// `TERM` - in that order, deliberately**, since
    /// `crate::pty::RealLauncher::spawn`'s own `.env_clear()` plus a
    /// later `.env()` call for an already-set key overwriting the
    /// earlier one (verified directly against `portable_pty`'s own
    /// `CommandBuilder`, not assumed) is what makes a later entry in this
    /// `Vec` win - never `std::env::var_os("PATH")` (today's actual bug:
    /// that reads *this server process's own* `PATH`, itself inherited
    /// from whatever client happened to start it), always
    /// `self.login.base_env`'s own captured `PATH`.
    fn launch_spec(&self, program: &Program, size: (u16, u16)) -> LaunchSpec {
        let argv = match program {
            Program::Shell { .. } => vec![self.login.argv0.clone()],
            Program::Command { argv, .. } => argv.clone(),
        };
        let bin_dir = std::env::current_exe()
            .ok()
            .and_then(|exe| exe.parent().map(std::path::Path::to_path_buf))
            .unwrap_or_else(|| std::path::PathBuf::from("/usr/local/bin"));
        let current_path = self
            .login
            .base_env
            .iter()
            .find(|(key, _)| key == "PATH")
            .map(|(_, value)| value.as_os_str());
        let overrides = pane_environment(
            "",
            steg_sdk::SessionId::from_raw(0),
            steg_sdk::TabId::from_raw(0),
            steg_sdk::PaneId::from_raw(0),
            &bin_dir,
            current_path,
        );

        let mut env = self.login.base_env.clone();
        env.extend(overrides);
        env.push((
            std::ffi::OsString::from("TERM"),
            std::ffi::OsString::from(PANE_TERM),
        ));

        LaunchSpec {
            argv,
            cwd: program.cwd().to_string(),
            env,
            size,
        }
    }
}

/// `TERM` every pane gets, whatever the login shell's own profile says
/// (AC #3) - matching this repository's own existing precedent
/// (`spikes/task-1.2-wire-protocol/probe-server/src/pty.rs:49`), not
/// invented here. Deliberately not `ghostty_vt::terminfo::TERM`
/// (`"xterm-ghostty"`): that compiled terminfo entry is not installed on
/// a user's own machine, and using it would need writing
/// `ghostty_vt::terminfo::dir()`'s own cache directory somewhere
/// persistent and setting `TERMINFO` to it too - a second piece of
/// integration this task's own acceptance criteria do not ask for,
/// flagged here for the engineer to schedule separately rather than
/// silently dropped.
const PANE_TERM: &str = "xterm-256color";

/// Dropping a `Runtime` stops it (review finding 3) - the safer of the
/// two choices named in the review (stop automatically, or refuse to
/// drop while something is still running): a caller that forgets the
/// explicit call, or drops a `Runtime` on an error path, still ends every
/// running program and joins every thread rather than leaking them
/// silently. `stop`'s own `Option`-wrapped fields make this safe to run
/// twice - an explicit `stop()` followed by this `drop` finds nothing
/// left to do.
impl Drop for Runtime {
    fn drop(&mut self) {
        self.stop();
    }
}

/// The pane id `dispatch` just minted, read back from its own typed
/// `Outcome` rather than the serialised `Value` wherever that is enough
/// (`pane.split`), or looked up from the tab/session `dispatch` returned
/// (`tab.new`/`session.new` mint exactly one pane each, R94, but their own
/// `Outcome` only names the tab/session, not that one pane).
fn new_pane_from(tree: &Tree, method: &str, value: &serde_json::Value) -> Option<PaneId> {
    let id_str = value.get("id")?.as_str()?;
    match method {
        "pane.split" => id_str.parse().ok(),
        "tab.new" => {
            let tab_id: steg_sdk::TabId = id_str.parse().ok()?;
            tree.sessions()
                .iter()
                .find_map(|session| session.tab(tab_id))
                .and_then(|tab| tab.panes().first())
                .map(steg_kernel::Pane::id)
        }
        "session.new" => {
            let session_id: steg_sdk::SessionId = id_str.parse().ok()?;
            tree.session(session_id)
                .and_then(|session| session.tabs().first())
                .and_then(|tab| tab.panes().first())
                .map(steg_kernel::Pane::id)
        }
        _ => None,
    }
}

/// The session that owns `tab` - the same walk `rollback_mint`'s own
/// `session.new` arm already does inline, pulled out so the creation
/// -events helper below can use it for all three starting methods, not
/// only that one.
fn session_owning_tab(tree: &Tree, tab: steg_sdk::TabId) -> SessionId {
    tree.sessions()
        .iter()
        .find(|session| session.tab(tab).is_some())
        .map(steg_kernel::Session::id)
        .expect("the tab just minted is still in the tree")
}

/// Emits `pane.status_changed` (§5 of this task's own plan): the one
/// event site for every way a typed command's status actually changes
/// (its own callers already decide *whether* anything changed - a no-op
/// mark is never passed here at all). Silently does nothing if the pane
/// cannot be scoped to a session (the pane is already gone from the tree,
/// §5's own documented race) - there is no subscriber whose filter could
/// meaningfully match it at that point anyway.
fn emit_pane_status_changed(state: &State, pane: PaneId, status: Option<steg_sdk::Status>) {
    let Some(scope) = state
        .tree
        .tab_of_pane(pane)
        .map(|tab| session_owning_tab(&state.tree, tab))
    else {
        return;
    };
    broadcast(
        state,
        &steg_sdk::TreeEvent::PaneStatusChanged { pane, status },
        scope,
    );
}

/// What `close_and_dispatch` needs captured *before* `dispatch_raw` runs
/// (§5 of this task's own plan, revised after a review found the first
/// version missed `pane.close`/`tab.close`'s own cascades): the session
/// every event in this close is scoped to - needed even when the kernel's
/// own cascade stops at just the pane, since nothing else still names it
/// once the pane is gone - and, for `session.close` alone, its tabs
/// grouped with their own panes. `pane.close`/`tab.close` need nothing
/// else pre-captured: what they actually closed comes back from the
/// kernel itself (`Outcome::PaneClosed`/`TabClosed`, read in
/// `emit_close_events` below), not from a second read of the tree after
/// the fact - the bug a review of this task found, and the reason this
/// is no longer named `CloseCascade`.
enum ClosePre {
    /// `pane.close`: just the session it was in.
    Pane { session: SessionId },
    /// `tab.close`: just the session it was in - its own panes are
    /// `close_and_dispatch`'s own `scope` (`panes_in_close_scope`),
    /// already exactly this tab's panes, reused rather than captured a
    /// second time here.
    Tab { session: SessionId },
    /// `session.close`: the session, and its tabs in order, each with its
    /// own panes in order - `Tree::close_session` reports no structured
    /// cascade of its own at all (unlike the other two), so this is the
    /// one case still read entirely from the tree before the close.
    Session {
        id: SessionId,
        tabs: Vec<(steg_sdk::TabId, Vec<PaneId>)>,
    },
}

/// Builds the [`ClosePre`] `close_and_dispatch` needs, before it closes
/// anything.
///
/// # Errors
/// Whatever resolving the request's own target gives - the same errors
/// `dispatch`'s own close arm would refuse with, caught here first since
/// this runs before it.
fn capture_close_pre(
    tree: &Tree,
    context: &Context,
    request: &Request,
) -> Result<ClosePre, ApiError> {
    match request.method.as_str() {
        "pane.close" => {
            let pane = dispatch::target_pane(tree, context, request)?;
            let tab = tree
                .tab_of_pane(pane)
                .expect("target_pane just resolved this pane; it is in the tree");
            Ok(ClosePre::Pane {
                session: session_owning_tab(tree, tab),
            })
        }
        "tab.close" => {
            let tab = dispatch::target_tab(tree, context, request)?;
            Ok(ClosePre::Tab {
                session: session_owning_tab(tree, tab),
            })
        }
        "session.close" => {
            let session = dispatch::target_session(tree, context, request)?;
            let tabs = tree
                .session(session)
                .map(|found| {
                    found
                        .tabs()
                        .iter()
                        .map(|tab| {
                            let panes = tab.panes().iter().map(steg_kernel::Pane::id).collect();
                            (tab.id(), panes)
                        })
                        .collect()
                })
                .unwrap_or_default();
            Ok(ClosePre::Session { id: session, tabs })
        }
        other => {
            unreachable!(
                "capture_close_pre is only ever called for the three close methods, not {other}"
            )
        }
    }
}

/// Emits one close's own events, innermost first (§5 of this task's own
/// plan): every pane, then its tab if the tab closed too, then the
/// session if that closed too - one rule, on every path, whichever of the
/// three methods caused it. `pane.close`/`tab.close` read what actually
/// closed from the kernel's own report (`outcome`); `session.close` reads
/// it from `pre` instead, since `Tree::close_session` reports none of its
/// own (`capture_close_pre`'s own doc says why). `tab_scope_panes` is
/// `close_and_dispatch`'s own `scope` (`panes_in_close_scope`) - for
/// `tab.close` this is already exactly that tab's own panes, so it is
/// reused here rather than captured a second time in `ClosePre`.
///
/// # Panics
/// Never in practice: `method` is always one of the three close verbs
/// (`close_and_dispatch`'s only caller), and `outcome`/`pre` are always
/// what that exact verb's own code just produced - the two are never
/// mismatched.
fn emit_close_events(
    state: &State,
    method: &str,
    pre: &ClosePre,
    tab_scope_panes: &[PaneId],
    outcome: &Outcome,
) {
    use steg_sdk::TreeEvent;

    match method {
        "pane.close" => {
            let ClosePre::Pane { session } = pre else {
                unreachable!("pane.close's own ClosePre is always ClosePre::Pane")
            };
            let Outcome::PaneClosed(closed) = outcome else {
                unreachable!("pane.close always produces Outcome::PaneClosed on success")
            };
            // Same shape `record_exit`'s own exit-driven cascade already
            // emits from a `ClosedPane` - one function, not two (the
            // second review round's own nit; this arm used to repeat
            // `emit_close_cascade`'s body verbatim).
            emit_close_cascade(state, closed, *session);
        }
        "tab.close" => {
            let ClosePre::Tab { session } = pre else {
                unreachable!("tab.close's own ClosePre is always ClosePre::Tab")
            };
            let Outcome::TabClosed(closed) = outcome else {
                unreachable!("tab.close always produces Outcome::TabClosed on success")
            };
            for pane in tab_scope_panes {
                broadcast(state, &TreeEvent::PaneClosed { id: *pane }, *session);
            }
            broadcast(state, &TreeEvent::TabClosed { id: closed.tab }, *session);
            if closed.session_now_empty {
                broadcast(state, &TreeEvent::SessionClosed { id: *session }, *session);
            }
        }
        "session.close" => {
            let ClosePre::Session { id, tabs } = pre else {
                unreachable!("session.close's own ClosePre is always ClosePre::Session")
            };
            for (tab_id, panes) in tabs {
                for pane in panes {
                    broadcast(state, &TreeEvent::PaneClosed { id: *pane }, *id);
                }
                broadcast(state, &TreeEvent::TabClosed { id: *tab_id }, *id);
            }
            broadcast(state, &TreeEvent::SessionClosed { id: *id }, *id);
        }
        other => unreachable!(
            "emit_close_events is only ever called for the three close methods, not {other}"
        ),
    }
}

/// Emits the closure half of a cascade `ClosedPane` already describes
/// (§5 of this task's own plan), innermost first: the pane always, then
/// its tab if the cascade also closed that (R94), then its session if
/// the cascade reached that far too. `scope` is the session every event
/// in the cascade is filtered against, even once `closed.session` itself
/// names it too - the same id, read once by the caller, since this
/// cascade can never reach a session other than the one the pane itself
/// was in.
fn emit_close_cascade(state: &State, closed: &steg_kernel::ClosedPane, scope: SessionId) {
    use steg_sdk::TreeEvent;

    broadcast(state, &TreeEvent::PaneClosed { id: closed.pane }, scope);
    if let Some(tab) = closed.tab {
        broadcast(state, &TreeEvent::TabClosed { id: tab }, scope);
    }
    if let Some(session) = closed.session {
        broadcast(state, &TreeEvent::SessionClosed { id: session }, scope);
    }
}

/// Emits this task's own creation events (§5 of this task's plan),
/// outermost first: `session.new` names its new session, tab and pane;
/// `tab.new` its new tab and pane; `pane.split` its one new pane. Called
/// only once the real spawn has succeeded (`start_and_dispatch`'s own
/// `Ok` arm) - never at the earlier, still-rollback-able mint.
///
/// `request`'s own `label` param means a different thing per method (the
/// new session's, the new tab's, or the new pane's own label - doc-2's
/// own per-method parameter list, not one this function invents): read
/// once and attached to whichever of the three it actually names: the
/// others get `None`, never the same text repeated on something it was
/// never given for.
fn emit_creation_events(state: &State, method: &str, new_pane: PaneId, request: &Request) {
    use steg_sdk::TreeEvent;

    let label = dispatch::param_str(request, "label").map(str::to_owned);
    let tab_id = state
        .tree
        .tab_of_pane(new_pane)
        .expect("just started this pane's program; its tab still holds it");
    let session_id = session_owning_tab(&state.tree, tab_id);

    match method {
        "session.new" => {
            broadcast(
                state,
                &TreeEvent::SessionCreated {
                    id: session_id,
                    label,
                },
                session_id,
            );
            broadcast(
                state,
                &TreeEvent::TabCreated {
                    id: tab_id,
                    session: session_id,
                    label: None,
                },
                session_id,
            );
            broadcast(
                state,
                &TreeEvent::PaneCreated {
                    id: new_pane,
                    tab: tab_id,
                    label: None,
                },
                session_id,
            );
        }
        "tab.new" => {
            broadcast(
                state,
                &TreeEvent::TabCreated {
                    id: tab_id,
                    session: session_id,
                    label,
                },
                session_id,
            );
            broadcast(
                state,
                &TreeEvent::PaneCreated {
                    id: new_pane,
                    tab: tab_id,
                    label: None,
                },
                session_id,
            );
        }
        "pane.split" => {
            broadcast(
                state,
                &TreeEvent::PaneCreated {
                    id: new_pane,
                    tab: tab_id,
                    label,
                },
                session_id,
            );
        }
        other => unreachable!(
            "emit_creation_events is only ever called for the three starting methods, not {other}"
        ),
    }
}

/// `tab.focus`/`pane.focus` (§5 of this task's own plan): emits the whole
/// path (decision-5: "one event reports the whole path, so no client ever
/// sees the two halves apart"), re-derived from the tree *after* the
/// mutation rather than threading the kernel's own `Focused`/`FocusMove`
/// back out of `dispatch::tab::focus`/`pane::focus` (both still return
/// `Outcome::Nothing`, unchanged) - correct regardless of which of the
/// two actually moved anything, or which neighbour `pane.focus
/// --direction` landed on: re-resolving the *original* target after the
/// fact still finds the same tab (focusing never moves a pane to another
/// tab), and that tab's own `current()` is whichever pane the operation
/// actually left focused, direction or not. Every other successful
/// method is a no-op here (`None` match arm) - nothing else changes focus.
///
/// **`session.focused` is never emitted here, named as a gap rather than
/// guessed at**: decision-5 gives a session no "current" of its own at the
/// tree level at all ("each session has a current tab, and each tab has a
/// current pane" - focus is a path *down* the tree; a session is the
/// path's root, not a level something else makes current within it).
/// Nothing in this task's own criteria or in decision-5 names what tree
/// mutation would produce it; `session.focus` (R87) is a different verb
/// entirely, addressed to a client, not a tree change, and owned by
/// TASK-1.109 under its own event name (`session.focus_requested`).
fn emit_focus_event(state: &State, context: &Context, request: &Request) {
    use steg_sdk::TreeEvent;

    let tab_id = match request.method.as_str() {
        "tab.focus" => dispatch::target_tab(&state.tree, context, request).ok(),
        "pane.focus" => dispatch::target_pane(&state.tree, context, request)
            .ok()
            .and_then(|pane| state.tree.tab_of_pane(pane)),
        _ => None,
    };
    let Some(tab_id) = tab_id else {
        return;
    };
    let session_id = session_owning_tab(&state.tree, tab_id);
    let Some(pane_id) = state
        .tree
        .session(session_id)
        .and_then(|session| session.tab(tab_id))
        .map(steg_kernel::Tab::current)
    else {
        return;
    };

    let event = match request.method.as_str() {
        "tab.focus" => TreeEvent::TabFocused {
            session: session_id,
            tab: tab_id,
            pane: pane_id,
        },
        "pane.focus" => TreeEvent::PaneFocused {
            session: session_id,
            tab: tab_id,
            pane: pane_id,
        },
        _ => unreachable!("already matched above"),
    };
    broadcast(state, &event, session_id);
}

/// Undoes exactly what `start_and_dispatch` minted, when the real spawn
/// fails (§2's rollback) - the same kernel calls a rejected label's own
/// rollback already uses, so no pane, tab or session is left behind for
/// any of the three methods (AC #1).
fn rollback_mint(tree: &mut Tree, method: &str, new_pane: PaneId) {
    match method {
        "pane.split" => {
            tree.close_pane(new_pane)
                .expect("the pane just minted is still in the tree");
        }
        "tab.new" => {
            let tab = tree
                .tab_of_pane(new_pane)
                .expect("the pane just minted is still in its tab");
            tree.close_tab(tab)
                .expect("the tab just minted is still in the tree");
        }
        "session.new" => {
            let tab = tree
                .tab_of_pane(new_pane)
                .expect("the pane just minted is still in its tab");
            let session = tree
                .sessions()
                .iter()
                .find(|session| session.tab(tab).is_some())
                .map(steg_kernel::Session::id)
                .expect("the session just minted is still in the tree");
            tree.close_session(session)
                .expect("the session just minted is still in the tree");
        }
        _ => unreachable!("only the three starting methods ever mint a pane to roll back"),
    }
}

/// Records that `apply_event_locked` has applied one of `pane`'s own
/// `CommandOutputStarted`/`CommandEnded` events (TASK-1.84 review): a
/// no-op in production, so the item-level `#[cfg(test)]` split, rather
/// than one on this call's own statement (unstable on expressions), is
/// what keeps `State::applied_marks` itself test-only.
#[cfg(test)]
fn note_mark_applied(state: &mut State, pane: PaneId) {
    *state.applied_marks.entry(pane).or_insert(0) += 1;
}

#[cfg(not(test))]
fn note_mark_applied(_state: &mut State, _pane: PaneId) {}

/// Applies one [`PaneEvent`] to the tree (§1), taking an already-locked
/// `State` - what the real drain thread calls in a loop, and what a test
/// can call directly with a hand-built [`PaneEvent`] for a deterministic,
/// synchronous proof of the cascade. No self-locking wrapper exists on
/// `Runtime` for this one (unlike `apply_size_changed`): nothing outside
/// the drain thread calls it yet, so there is nothing for one to serve
/// today; add one, the same shape as `apply_size_changed`'s, if a later
/// task needs to call this from outside an already-locked context.
fn apply_event_locked(state: &mut State, event: PaneEvent) {
    match event {
        PaneEvent::Exited { pane, exit_code } => {
            // TASK-1.41: the one place the server's real clock becomes a
            // kernel `Moment` - `Instant::now()` against this `Runtime`'s
            // own `clock_origin`, never read by the kernel itself.
            let now = Moment::new(Instant::now().duration_since(state.clock_origin));
            // Captured *before* `record_exit` can close anything (the
            // same reasoning `close_and_dispatch` already uses, §5 of
            // this task's own plan): `pane.exited`'s own scope has to be
            // this pane's session even when the cascade closes it too, so
            // it is read from the tree while the pane is still in it,
            // not derived from `ClosedPane`'s own fields afterward (those
            // say *whether* the cascade reached the tab/session, not
            // which tab/session the pane was in when it did not).
            let scope = state
                .tree
                .tab_of_pane(pane)
                .map(|tab| session_owning_tab(&state.tree, tab));
            match state.tree.record_exit(pane, exit_code, now) {
                Ok(outcome) => {
                    if let Some(scope) = scope {
                        broadcast(
                            state,
                            &steg_sdk::TreeEvent::PaneExited { pane, exit_code },
                            scope,
                        );
                        if let ExitOutcome::Closed(closed) = &outcome {
                            emit_close_cascade(state, closed, scope);
                        }
                    }
                    if let ExitOutcome::Closed(closed) = outcome {
                        state.panes.remove(&closed.pane);
                        // TASK-1.86: this cascade (R94) can close a
                        // session with nobody connected at all - a shell
                        // pane whose program just exited, in a session
                        // no client is viewing. A request-driven close
                        // (`session.close`/`tab.close`/`pane.close`/
                        // `pane.move` over the wire, TASK-1.46) needs no
                        // notify of its own: each only ever runs on an
                        // already-accepted connection, and `connection::
                        // handle` sends `Disconnected` on every path out
                        // of itself, which already wakes the checker (and
                        // that connection's own `live_clients > 0` keeps
                        // the checker ineligible regardless of session
                        // count until it does). This path has no
                        // connection behind it at all, so it is the one
                        // case with nothing else to wake the checker if a
                        // session was all that was keeping it ineligible.
                        if closed.session.is_some()
                            && let Some(notify) = &state.notify_session_count_changed
                        {
                            let _ = notify.send(ConnectionEvent::SessionCountChanged);
                        }
                    }
                }
                Err(steg_kernel::Error::PaneNotFound(_)) => {
                    // TASK-1.78: this is now *exactly* what a forced close
                    // produces for its own grace window, every time, by
                    // design (`Runtime::close_and_dispatch` closes the
                    // tree *before* ending the program, §5) - not the
                    // "--force orphan" gap this arm was first written for.
                    // `state.panes` never has this pane's handle either by
                    // the time this runs (popped into `force_end`'s own
                    // local scope, never put back), so the `remove` below
                    // is a defensive no-op in practice, provably
                    // unreachable by any path this task's own design
                    // permits - kept rather than turned into an
                    // `unreachable!()`: a wrong "impossible" assumption
                    // panicking this live drain thread is worse than one
                    // harmless, documented, no-op branch.
                    state.panes.remove(&pane);
                }
                Err(other) => {
                    unreachable!(
                        "record_exit on a pane the tree still holds can only fail with \
                         ProgramNotStarted, and this task's own invariant (a program is \
                         always started immediately after minting, under the one lock) \
                         makes that unreachable here: {other}"
                    );
                }
            }
        }
        PaneEvent::CommandOutputStarted { pane } => {
            note_mark_applied(state, pane);
            match state.tree.note_command_output_started(pane) {
                Ok(CommandOutputStarted::Started) => {
                    emit_pane_status_changed(state, pane, Some(steg_sdk::Status::Running));
                }
                Ok(CommandOutputStarted::NotAShellPane) => {
                    // Nothing changed (the kernel's own doc: "a command
                    // pane's own program governs its status, never a
                    // mark") - R52's own correction (M1 for the events
                    // this task's own criteria reach) still means no
                    // event where nothing moved.
                }
                Err(steg_kernel::Error::PaneNotFound(_)) => {
                    // The mark is ignored - there is nothing else to do
                    // with one for a pane the tree no longer holds.
                    //
                    // Reachable by exactly one path, traced for TASK-
                    // 1.78's review: `close_and_dispatch` removes the
                    // pane from the tree *and* takes its `PaneHandle` out
                    // of `state.panes` under the same lock, before
                    // `force_end` has actually ended its program (§5) -
                    // a still-running program can emit an OSC 133 mark
                    // during that same grace window, forwarded by its
                    // relay/worker thread exactly as it would have been
                    // before the close. By the time this event reaches
                    // the control thread, the handle is already with
                    // `force_end`, not `state.panes` - this arm was never
                    // at risk of dropping its only `Sender` clone on this
                    // path, since there is no handle here to drop either
                    // way. `force_end` is what ends the program and
                    // joins its threads, on its own stated bound; this
                    // mark is simply overtaken by it. Not a gap
                    // close-first left open.
                }
                Err(other) => unreachable!(
                    "Tree::note_command_output_started documents only Error::PaneNotFound \
                     as a possible error; {other} is not that"
                ),
            }
        }
        PaneEvent::CommandEnded { pane, exit_code } => {
            note_mark_applied(state, pane);
            let now = Moment::new(Instant::now().duration_since(state.clock_origin));
            match state.tree.note_command_ended(pane, exit_code, now) {
                Ok(CommandEnded::Ended(status)) => {
                    emit_pane_status_changed(state, pane, Some(status));
                }
                Ok(CommandEnded::EndedWithNoExitCode) => {
                    // "the status is cleared, back to idle - not
                    // replaced with anything" (the kernel's own doc) -
                    // still a real change, reported the same as any
                    // other: `None`, not skipped.
                    emit_pane_status_changed(state, pane, None);
                }
                Ok(CommandEnded::Ignored) => {
                    // Nothing changed (an end with no start, or a second
                    // end in a row) - no event for a no-op.
                }
                Err(steg_kernel::Error::PaneNotFound(_)) => {
                    // Same reasoning as `CommandOutputStarted`'s own arm.
                }
                Err(other) => unreachable!(
                    "Tree::note_command_ended documents only Error::PaneNotFound as a \
                     possible error; {other} is not that"
                ),
            }
        }
    }
}

/// Ends every program in `handles`, reaps each, joins every worker thread -
/// bounded by `grace` (shared across every pane in `handles`, not a fresh
/// window per pane) plus two small, fixed margins, regardless of how many
/// panes are given (TASK-1.78 §2). Used by both [`Runtime::stop`] (scope:
/// every pane) and [`crate::dispatch`]'s own close-with-`--force` wrapper
/// (scope: one pane, a tab's panes, or a session's tabs' panes) - one
/// shared primitive rather than two copies of "signal, wait, escalate,
/// join" that could drift apart. Never panics, never blocks past its own
/// stated bound; takes no lock of its own and must be called with none
/// held, since its own waits are real (bounded, but real).
fn force_end(handles: Vec<(PaneId, PaneHandle)>, grace: Duration) {
    // Pass 1: signal every pane at once. `signal_hangup` goes straight
    // through `RunGate` (a `Mutex`, held only for the length of the
    // signal call itself - review finding 1), never through the
    // worker's own command queue, so this loop costs microseconds
    // regardless of how backed up any one pane's own worker is with its
    // own run's `PtyBytes`.
    for (_, handle) in &handles {
        handle.signal_hangup();
    }

    // Pass 2: wait against one shared deadline, not N deadlines - one
    // thread per pane so N panes share the deadline, not N * it.
    // `await_exit` is also `RunGate`-based (a `Condvar` wait, never the
    // worker's queue), which is what makes this a real "has the process
    // ended" check rather than "has the worker drained its backlog far
    // enough to notice" (the misclassification finding 1 found).
    let deadline = Instant::now() + grace;
    let stragglers: Vec<PaneId> = thread::scope(|scope| {
        let jobs: Vec<_> = handles
            .iter()
            .map(|(pane, handle)| {
                scope.spawn(move || {
                    let remaining = deadline.saturating_duration_since(Instant::now());
                    (*pane, handle.await_exit(remaining))
                })
            })
            .collect();
        jobs.into_iter()
            .map(|job| {
                job.join()
                    .expect("await_exit only waits on a Condvar with a bound, never panics")
            })
            .filter(|(_, ended)| !ended)
            .map(|(pane, _)| pane)
            .collect()
    });

    // Pass 3: escalate only the stragglers - same direct-`RunGate`
    // reasoning as pass 1.
    for (pane, handle) in &handles {
        if stragglers.contains(pane) {
            handle.signal_kill();
        }
    }

    // Pass 4: confirm the escalated ones actually died (`SIGKILL` cannot
    // be trapped, so this bound is a safety margin, not a real wait for
    // cooperation) - a pane that already ended during pass 2 is not
    // re-visited here.
    thread::scope(|scope| {
        for pane in &stragglers {
            if let Some((_, handle)) = handles.iter().find(|(p, _)| p == pane) {
                scope.spawn(|| {
                    handle.await_exit(ESCALATE_CONFIRM_TIMEOUT);
                });
            }
        }
    });

    // Pass 5: settle every worker thread *in parallel* - telling each
    // one its own run has ended, via `PaneHandle::settle_worker`, and
    // waiting for its reply, costs up to `FORCE_JOIN_TIMEOUT` in total
    // regardless of how many panes are in `handles`, because this runs
    // all of them at once rather than one after another. Folding this
    // into `join_worker` itself instead (settle-then-join, called once
    // per handle in the sequential pass below) was rejected during
    // review: that would have put the exact N-pane-multiplication risk
    // this task's own `force_end` exists to remove back into the one
    // place a worker's own queue can still be genuinely backed up (a
    // flooding run's `PtyBytes`) even after `RunGate` has already
    // confirmed, promptly, that the process itself is dead.
    thread::scope(|scope| {
        for (_, handle) in &handles {
            scope.spawn(|| {
                handle.settle_worker(FORCE_JOIN_TIMEOUT);
            });
        }
    });

    // Pass 6: join every worker thread, bounded - sequential, not
    // scoped: `join_worker` consumes `self` by value, and by this point
    // every handle's own worker has already been told to stop and
    // settled (pass 5), so each join is expected to return almost at
    // once, never block on another pane's own wait.
    for (_, handle) in handles {
        let _ = handle.join_worker(FORCE_JOIN_TIMEOUT);
    }
}

/// The core of [`Runtime::apply_size_changed`] (§4), taking an already-
/// locked `State`.
fn apply_size_changed_locked(state: &mut State, change: &SizeChanged) {
    for &(pane, size) in &change.panes {
        if let Some(handle) = state.panes.get(&pane) {
            let _ = handle.resize(size.columns(), size.rows());
        }
    }
}

/// `Runtime::handle`'s general step (§4): after any successful request,
/// reapplies every currently-sized tab's sizes to its panes - cheap
/// (empty today, since no `client` dispatch method exists to ever
/// declare one), and correct regardless of which verb just ran, since it
/// does not need to know which ones can change a layout.
fn reapply_every_sized_tab(state: &mut State) {
    for tab in state.tree.sized_tabs() {
        if let Some(change) = state.tree.tab_sizes_now(tab) {
            apply_size_changed_locked(state, &change);
        }
    }
}

/// Delivers one event to every subscriber whose own filter matches it
/// (TASK-1.46 §5/§6/§7) - called from every tree-changing site in this
/// file, still under the lock that site already holds, with `scope` the
/// session the event belongs to (every part-A event has one). `event`'s
/// own `to_wire` is the one place its name and body are derived - the
/// same definition a subscriber's own `TreeEvent::parse` reads back.
/// `enqueue_control` is lock-free and does no I/O (`outbound.rs`'s own
/// doc) - but a failed one *does* call `shutdown(Both)` on this
/// subscriber's own cloned socket, below, which is a real syscall made
/// while `state`'s lock is still held (non-blocking on a Unix socket in
/// practice, the same cross-thread-shutdown idiom `ConnectionCleanup`
/// already uses, not a bug - just not "nothing writes to a socket," which
/// an earlier draft of this comment overstated). AC #3's own answer to a
/// slow or closed subscriber is exactly that shutdown, called from here,
/// not a resend and not left to the writer thread to notice on its own.
fn broadcast(state: &State, event: &steg_sdk::TreeEvent, scope: SessionId) {
    let (name, body) = event.to_wire();
    for subscriber in state.subscribers.values() {
        let Some(filter) = &subscriber.filter else {
            continue;
        };
        if !filter.matches(&name, scope) {
            continue;
        }
        let bytes = Connection::emit_event(&name, body.clone());
        if subscriber.outbound.enqueue_control(bytes).is_err() {
            // AC #3: a subscriber that falls behind is closed, not
            // resynced. Shutting its own socket down (not just its
            // `Outbound`) is what actually ends the connection: it
            // unblocks a writer stuck mid-write on a full OS buffer *and*
            // this connection's own reader thread, parked in a plain
            // `read()` with no idle timeout (TASK-1.45 §4) - nothing else
            // reaches that thread from here. `ConnectionCleanup::drop`
            // runs once the reader notices and returns, which is what
            // removes this entry from `subscribers` in turn.
            let _ = subscriber.shutdown.shutdown(std::net::Shutdown::Both);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::io::Read;
    use std::os::unix::net::UnixStream;
    use std::sync::Arc;
    use std::sync::mpsc;
    use std::thread;
    use std::time::{Duration, Instant};

    use steg_sdk::{ClientId, Program, ProgramStatus, Size, Status, TreeEvent};

    use super::{
        Filter, LoginShell, Moment, Runtime, State, Subscriber, apply_event_locked, broadcast,
        force_end,
    };
    use crate::outbound::{Outbound, spawn_writer};
    use crate::pane::{PaneEvent, PaneHandle};
    use crate::pty::RealLauncher;

    /// Registers a subscriber in `state` with a real writer thread
    /// actually draining its queue onto a real, paired `UnixStream` - so
    /// `broadcast`'s own calls (`apply_event_locked`'s real code path, no
    /// doubles) can be observed from the test's own end of the pair,
    /// exactly as a real connection's reader would. `filter` is the same
    /// `Some`/`None` distinction a real connection has before/after
    /// `event.watch` - a test that wants to prove a registered-but
    /// -unfiltered subscriber receives nothing passes `None`.
    fn register_test_subscriber(
        state: &mut State,
        filter: Option<Filter>,
    ) -> (UnixStream, Arc<Outbound>, thread::JoinHandle<()>) {
        let (read_end, write_end) = UnixStream::pair().unwrap();
        let outbound = Arc::new(Outbound::new());
        let writer = spawn_writer(Arc::clone(&outbound), write_end.try_clone().unwrap());
        state.subscribers.insert(
            ClientId::from_raw(1),
            Subscriber {
                outbound: Arc::clone(&outbound),
                filter,
                shutdown: write_end,
            },
        );
        (read_end, outbound, writer)
    }

    /// Registers a subscriber with *no* writer thread at all - nothing
    /// ever drains its `Outbound`, so `CONTROL_CAP` overflows
    /// deterministically at a known call count, with no dependence on a
    /// real OS socket buffer's own size or a real writer thread's own
    /// scheduling (the real-socket version of this proof, tried first in
    /// `tests/events.rs`, was flaky for exactly that reason - named
    /// there, not hidden).
    fn register_slow_subscriber(state: &mut State, filter: Option<Filter>) -> UnixStream {
        let (read_end, write_end) = UnixStream::pair().unwrap();
        let outbound = Arc::new(Outbound::new());
        state.subscribers.insert(
            ClientId::from_raw(1),
            Subscriber {
                outbound,
                filter,
                shutdown: write_end,
            },
        );
        read_end
    }

    /// Reads exactly one event off `stream` as a `TreeEvent`, bounded -
    /// never a bare `read` with no deadline. `connection` is the caller's
    /// own, carried across every call for the same stream - a fresh
    /// `Connection` per call would discard any bytes a single `read`
    /// happened to over-fetch (more than one queued frame can arrive in
    /// one `read()`), silently losing the next event this same test still
    /// expects; found exactly that way while building this test.
    fn read_one_event(stream: &mut UnixStream, connection: &mut steg_sdk::Connection) -> TreeEvent {
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        loop {
            match connection.poll() {
                Some(steg_sdk::Incoming::Event { name, body }) => {
                    return TreeEvent::parse(&name, body).expect("a described event parses");
                }
                Some(other) => panic!("expected an event, got {other:?}"),
                None => {}
            }
            let mut buf = [0_u8; 4096];
            let n = stream.read(&mut buf).expect("reading the broadcast event");
            assert!(n > 0, "the writer closed before sending an event");
            connection.receive(&buf[..n]);
        }
    }

    /// Asserts nothing arrives on `stream` within `bound` - the no-poll
    /// proof's own negative space: a quiet window with nothing broadcast
    /// produces nothing read, not a timeout racing a real event.
    fn assert_nothing_arrives_within(stream: &mut UnixStream, bound: Duration) {
        stream.set_read_timeout(Some(bound)).unwrap();
        let mut buf = [0_u8; 16];
        match stream.read(&mut buf) {
            Ok(0) => {}
            Ok(n) => panic!("expected nothing, got {n} bytes"),
            Err(err) => assert_eq!(
                err.kind(),
                std::io::ErrorKind::WouldBlock,
                "expected a plain timeout, got {err:?}"
            ),
        }
    }

    /// Bounded, never a bare `recv` - the no-sleep rule's own reasoning
    /// for every real-process wait in this crate's tests.
    const EVENT_TIMEOUT: Duration = Duration::from_secs(20);

    /// A working `PATH` is required (TASK-1.87): `RealLauncher::spawn`'s
    /// own `.env_clear()` means a `LaunchSpec.env` with no `PATH` at all
    /// cannot resolve a relative program name like `"sh"` (confirmed
    /// directly with `portable_pty`, not assumed - `spawn_command` fails
    /// with "Unable to resolve the PATH"). Reading this *test* process's
    /// own ambient `PATH` is fine here - a test, allowed to, unlike
    /// production code (this function's own doc on `ServerConfig`'s
    /// `login_env` module says why production code never does).
    fn test_base_env() -> Vec<(std::ffi::OsString, std::ffi::OsString)> {
        vec![(
            std::ffi::OsString::from("PATH"),
            std::env::var_os("PATH").unwrap_or_default(),
        )]
    }

    /// A tiny `force_grace` for every `Runtime::new` in this module - none
    /// of these tests exercise forcing *through a `Runtime`* (the one
    /// direct `force_end` call, cleaning up an orphaned handle this
    /// module built by hand with no `Runtime` around it, is the
    /// exception), so the value only has to be cheap, never realistic.
    const TEST_GRACE: Duration = Duration::from_millis(30);

    fn login() -> LoginShell {
        LoginShell {
            argv0: "/bin/sh".to_string(),
            home: "/tmp".to_string(),
            user: None,
            base_env: test_base_env(),
        }
    }

    /// Builds a `State` for a test with no `Runtime` around it - every
    /// `clock_origin` in this module's own tests is simply "now", since
    /// none of them care about any moment but the one `apply_event_locked`
    /// reads at the instant it runs.
    fn test_state(tree: steg_kernel::Tree, panes: HashMap<steg_sdk::PaneId, PaneHandle>) -> State {
        State {
            tree,
            panes,
            settings: steg_kernel::Settings::new(),
            clock_origin: Instant::now(),
            subscribers: HashMap::new(),
            notify_session_count_changed: None,
            applied_marks: HashMap::new(),
        }
    }

    fn fresh_state_with_one_pane() -> (State, steg_sdk::PaneId) {
        let mut tree = steg_kernel::Tree::new();
        let session = tree.new_session(None).unwrap();
        let pane = tree.session(session).unwrap().tabs()[0].panes()[0].id();
        tree.start_program(
            pane,
            Program::Command {
                argv: vec!["htop".to_string()],
                cwd: "/tmp".to_string(),
            },
        )
        .unwrap();
        (test_state(tree, HashMap::new()), pane)
    }

    /// AC #2's cascade, proven deterministically: a hand-built
    /// `PaneEvent::Exited` against a `Tree` already holding a started
    /// pane, with no thread, no channel and no timing at all - the thing
    /// the real drain thread calls in a loop is exactly this function.
    #[test]
    fn apply_event_records_an_exit_and_leaves_a_command_pane_in_place() {
        let (mut state, pane) = fresh_state_with_one_pane();

        apply_event_locked(&mut state, PaneEvent::Exited { pane, exit_code: 3 });

        let found = state
            .tree
            .session(state.tree.sessions()[0].id())
            .unwrap()
            .tabs()[0]
            .pane(pane)
            .unwrap();
        assert_eq!(
            found.program_status(),
            Some(ProgramStatus::Ended { exit_code: 3 })
        );
    }

    #[test]
    fn apply_event_closes_a_shell_pane_through_the_same_cascade_as_a_manual_close() {
        let mut tree = steg_kernel::Tree::new();
        let session = tree.new_session(None).unwrap();
        let pane = tree.session(session).unwrap().tabs()[0].panes()[0].id();
        tree.start_program(
            pane,
            Program::Shell {
                cwd: "/tmp".to_string(),
            },
        )
        .unwrap();
        let mut state = test_state(tree, HashMap::new());

        // This pane is its tab's only pane, and its tab is its session's
        // only tab (R94) - a shell pane's own exit must cascade all the
        // way to the session closing, in one `apply_event` call, with no
        // further request issued.
        apply_event_locked(&mut state, PaneEvent::Exited { pane, exit_code: 0 });

        assert!(state.tree.sessions().is_empty());
    }

    /// AC #2: a command pane's own real exit, the `Stayed` outcome, emits
    /// `pane.exited` to a subscriber - the exact production path
    /// `apply_event_locked` is, not a synthetic call to `broadcast`.
    /// Negative control: comment out the one `broadcast` call this test
    /// depends on in `apply_event_locked`'s `Exited` arm - this test then
    /// times out waiting for an event that never arrives, proving it
    /// actually exercises that call.
    #[test]
    fn a_command_panes_exit_emits_pane_exited() {
        let (mut state, pane) = fresh_state_with_one_pane();
        let (mut stream, outbound, writer) = register_test_subscriber(
            &mut state,
            Some(Filter {
                name: None,
                session: None,
            }),
        );
        let mut connection =
            steg_sdk::Connection::new(steg_sdk::WireRole::Client, steg_sdk::description());

        apply_event_locked(&mut state, PaneEvent::Exited { pane, exit_code: 3 });

        assert_eq!(
            read_one_event(&mut stream, &mut connection),
            TreeEvent::PaneExited { pane, exit_code: 3 }
        );

        outbound.close();
        writer.join().unwrap();
    }

    /// AC #2: a shell pane's own exit, which cascades all the way to its
    /// session closing (R94), emits all four events in the stated order -
    /// `pane.exited` first (the causal fact), then the closure cascade
    /// innermost first. Negative control: the same comment-out-the-call
    /// proof as the test above, applied to `emit_close_cascade`'s own
    /// three calls instead - this test then sees fewer than four events
    /// and times out waiting for the rest.
    #[test]
    fn a_shell_panes_exit_emits_the_whole_cascade_in_order() {
        let mut tree = steg_kernel::Tree::new();
        let session = tree.new_session(None).unwrap();
        let tab = tree.session(session).unwrap().tabs()[0].id();
        let pane = tree.session(session).unwrap().tabs()[0].panes()[0].id();
        tree.start_program(
            pane,
            Program::Shell {
                cwd: "/tmp".to_string(),
            },
        )
        .unwrap();
        let mut state = test_state(tree, HashMap::new());
        let (mut stream, outbound, writer) = register_test_subscriber(
            &mut state,
            Some(Filter {
                name: None,
                session: None,
            }),
        );
        let mut connection =
            steg_sdk::Connection::new(steg_sdk::WireRole::Client, steg_sdk::description());

        apply_event_locked(&mut state, PaneEvent::Exited { pane, exit_code: 0 });

        assert_eq!(
            read_one_event(&mut stream, &mut connection),
            TreeEvent::PaneExited { pane, exit_code: 0 }
        );
        assert_eq!(
            read_one_event(&mut stream, &mut connection),
            TreeEvent::PaneClosed { id: pane }
        );
        assert_eq!(
            read_one_event(&mut stream, &mut connection),
            TreeEvent::TabClosed { id: tab }
        );
        assert_eq!(
            read_one_event(&mut stream, &mut connection),
            TreeEvent::SessionClosed { id: session }
        );

        outbound.close();
        writer.join().unwrap();
    }

    /// AC #2: a typed command's start and end (TASK-1.84's own marks)
    /// each emit `pane.status_changed`, with the status the kernel's own
    /// `CommandOutputStarted`/`CommandEnded` computed - the same
    /// `apply_event_locked` path a real OSC 133 mark drives, proven here
    /// deterministically with no pty, the same reasoning this module's
    /// own existing `apply_event` tests already give.
    #[test]
    fn a_typed_commands_start_and_end_emit_pane_status_changed() {
        let mut tree = steg_kernel::Tree::new();
        let session = tree.new_session(None).unwrap();
        let pane = tree.session(session).unwrap().tabs()[0].panes()[0].id();
        tree.start_program(
            pane,
            Program::Shell {
                cwd: "/tmp".to_string(),
            },
        )
        .unwrap();
        let mut state = test_state(tree, HashMap::new());
        let (mut stream, outbound, writer) = register_test_subscriber(
            &mut state,
            Some(Filter {
                name: None,
                session: None,
            }),
        );
        let mut connection =
            steg_sdk::Connection::new(steg_sdk::WireRole::Client, steg_sdk::description());

        apply_event_locked(&mut state, PaneEvent::CommandOutputStarted { pane });
        assert_eq!(
            read_one_event(&mut stream, &mut connection),
            TreeEvent::PaneStatusChanged {
                pane,
                status: Some(Status::Running)
            }
        );

        apply_event_locked(
            &mut state,
            PaneEvent::CommandEnded {
                pane,
                exit_code: Some(0),
            },
        );
        assert_eq!(
            read_one_event(&mut stream, &mut connection),
            TreeEvent::PaneStatusChanged {
                pane,
                status: Some(Status::Finished)
            }
        );

        outbound.close();
        writer.join().unwrap();
    }

    /// A subscriber registered but never having called `event.watch`
    /// (`filter: None`) receives nothing - never everything unfiltered.
    /// Negative control: pass `Some(Filter{name: None, session: None})`
    /// instead (every other test above already does, and sees events) -
    /// this test would then fail by finding an event where it expects
    /// silence, proving the assertion is real.
    #[test]
    fn an_unfiltered_subscriber_receives_nothing() {
        let (mut state, pane) = fresh_state_with_one_pane();
        let (mut stream, outbound, writer) = register_test_subscriber(&mut state, None);

        apply_event_locked(&mut state, PaneEvent::Exited { pane, exit_code: 0 });

        assert_nothing_arrives_within(&mut stream, Duration::from_millis(200));

        outbound.close();
        writer.join().unwrap();
    }

    /// AC #3: a subscriber whose own queue is full is closed, not
    /// resynced and not blocked on. With no writer thread draining it at
    /// all, `broadcast`'s own `enqueue_control` calls must eventually
    /// return `Full` - once they do, this test's own read end must see
    /// the connection close (a clean `Ok(0)`, since `shutdown(Both)` on
    /// the paired end is exactly what `UnixStream::pair` makes visible
    /// that way), not keep accepting data forever. Negative control:
    /// remove `broadcast`'s own `if ... is_err() { shutdown }` arm,
    /// replacing it with the silent `let _ = ...` it used to be - this
    /// test then fails, reading bytes indefinitely instead of a close,
    /// since nothing ever tells this connection's own (nonexistent, in
    /// this test) reader to stop.
    #[test]
    fn a_slow_subscribers_connection_is_shut_down_once_its_queue_is_full() {
        let (mut state, _pane) = fresh_state_with_one_pane();
        let mut stream = register_slow_subscriber(
            &mut state,
            Some(Filter {
                name: None,
                session: None,
            }),
        );

        // Every `session.new`'s own three creation events, repeated well
        // past `CONTROL_CAP` (256) - nothing drains this subscriber's own
        // queue at all, so this is guaranteed to overflow, not merely
        // likely to.
        for _ in 0..200 {
            let session = state.tree.new_session(None).unwrap();
            let tab = state.tree.session(session).unwrap().tabs()[0].id();
            let pane = state.tree.session(session).unwrap().tabs()[0].panes()[0].id();
            broadcast(
                &state,
                &TreeEvent::SessionCreated {
                    id: session,
                    label: None,
                },
                session,
            );
            broadcast(
                &state,
                &TreeEvent::TabCreated {
                    id: tab,
                    session,
                    label: None,
                },
                session,
            );
            broadcast(
                &state,
                &TreeEvent::PaneCreated {
                    id: pane,
                    tab,
                    label: None,
                },
                session,
            );
        }

        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut buf = [0_u8; 65536];
        loop {
            match stream.read(&mut buf) {
                Ok(0) => break, // the connection closed, as expected
                Ok(_) => {}     // still draining what was queued before the shutdown
                Err(err) => panic!("expected a clean close, got {err:?}"),
            }
        }
    }

    #[test]
    fn apply_event_on_a_pane_the_tree_no_longer_holds_removes_the_stale_handle_without_panicking() {
        // The orphan a forced close leaves (§5, until the task that ends
        // a running program on `--force`): the tree has already lost the
        // pane (simulated here directly, standing in for that future
        // close), but a `PaneHandle` for it is still in `panes` - exactly
        // what happens when that program later exits on its own.
        let tree = steg_kernel::Tree::new();
        let gone = steg_sdk::PaneId::from_raw(999);
        let (events_tx, _events_rx) = mpsc::channel();
        let handle = crate::pane::start(
            crate::pty::LaunchSpec {
                argv: vec!["sh".to_string(), "-c".to_string(), "sleep 1".to_string()],
                cwd: "/tmp".to_string(),
                env: test_base_env(),
                size: (80, 24),
            },
            gone,
            std::sync::Arc::new(RealLauncher),
            events_tx,
        )
        .unwrap();
        let mut state = test_state(tree, HashMap::from([(gone, handle)]));

        apply_event_locked(
            &mut state,
            PaneEvent::Exited {
                pane: gone,
                exit_code: 0,
            },
        );

        assert!(
            !state.panes.contains_key(&gone),
            "the stale handle must be removed, not leaked"
        );
    }

    /// Proves the plumbing *before* `apply_event` is real - a real
    /// process's real exit reaching a real channel - the same shape
    /// `crates/server/src/pane.rs`'s own tests already use to bypass
    /// `PaneHandle` for a focused check, bypassing `Runtime`'s own drain
    /// thread too so this stays deterministic and bounded.
    #[test]
    fn a_real_exit_event_drives_the_same_cascade_apply_event_runs() {
        let mut tree = steg_kernel::Tree::new();
        let session = tree.new_session(None).unwrap();
        let pane = tree.session(session).unwrap().tabs()[0].panes()[0].id();
        tree.start_program(
            pane,
            Program::Shell {
                cwd: "/tmp".to_string(),
            },
        )
        .unwrap();

        let (events_tx, events_rx) = mpsc::channel();
        let handle = crate::pane::start(
            crate::pty::LaunchSpec {
                argv: vec!["sh".to_string(), "-c".to_string(), "exit 0".to_string()],
                cwd: "/tmp".to_string(),
                env: test_base_env(),
                size: (80, 24),
            },
            pane,
            std::sync::Arc::new(RealLauncher),
            events_tx,
        )
        .unwrap();

        let event = events_rx
            .recv_timeout(EVENT_TIMEOUT)
            .expect("the real shell's own exit, within the bound");

        let mut state = test_state(tree, HashMap::from([(pane, handle)]));
        apply_event_locked(&mut state, event);

        assert!(state.tree.sessions().is_empty());
        assert!(!state.panes.contains_key(&pane));
    }

    #[test]
    fn apply_size_changed_resizes_only_the_panes_the_change_names() {
        let (mut state, pane) = fresh_state_with_one_pane();
        let (events_tx, _events_rx) = mpsc::channel();
        let handle = crate::pane::start(
            crate::pty::LaunchSpec {
                argv: vec!["sh".to_string(), "-c".to_string(), "sleep 1".to_string()],
                cwd: "/tmp".to_string(),
                env: test_base_env(),
                size: (80, 24),
            },
            pane,
            std::sync::Arc::new(RealLauncher),
            events_tx,
        )
        .unwrap();
        state.panes.insert(pane, handle);

        let change = steg_kernel::SizeChanged {
            tab: state.tree.sessions()[0].tabs()[0].id(),
            size: Size::new(40, 10),
            panes: vec![(pane, Size::new(40, 10))],
            viewers: Vec::new(),
        };
        super::apply_size_changed_locked(&mut state, &change);

        let screen = state.panes[&pane].snapshot().unwrap();
        assert_eq!(screen.rows.len(), 10);
    }

    /// `Runtime::connect_client`/`disconnect_client` (TASK-1.45 §1): a
    /// thin, self-locking wrapper proof, not a re-proof of the kernel's
    /// own cascade (`crates/kernel/src/tree.rs`'s own
    /// `latest_falls_back_to_the_next_most_recently_active_remaining_viewer_on_departure`
    /// already proves `Tree::disconnect_client` itself). What this proves
    /// is that `Runtime`'s own wrapper actually calls through to the real
    /// tree and reapplies whatever `SizeChanged`s it gets back - checked
    /// on a real pane's real pty, the same way every other
    /// `apply_size_changed` test in this module already does.
    #[test]
    fn disconnect_client_reapplies_the_next_latest_declared_size() {
        use steg_sdk::{ClientKind, Request, Size};

        let runtime = Runtime::new(std::sync::Arc::new(RealLauncher), login(), TEST_GRACE);
        let description = steg_sdk::description();
        let no_context = crate::dispatch::Context::none();
        let session = match runtime.handle(
            &description,
            &no_context,
            &Request {
                method: "session.new".to_string(),
                params: serde_json::json!({"program": ["sh", "-c", "sleep 100"]})
                    .as_object()
                    .unwrap()
                    .clone(),
            },
        ) {
            steg_sdk::Response::Ok(value) => value,
            steg_sdk::Response::Error(error) => panic!("session.new failed: {error:?}"),
        };
        let session_id: steg_sdk::SessionId = session["id"].as_str().unwrap().parse().unwrap();
        let (pane, tab_id) = {
            let state = runtime.state.lock().unwrap();
            let tab = &state.tree.session(session_id).unwrap().tabs()[0];
            (tab.panes()[0].id(), tab.id())
        };

        // `a` and `b` are minted through the public wrapper this test is
        // about; `view_pane`/`declare_size` have no `Runtime`-level
        // wrapper of their own (no dispatch method reaches them yet,
        // TASK-1.42's own gap), so those two calls reach the tree
        // directly, the same way this module's other size tests already
        // do.
        let a = runtime.connect_client(ClientKind::App, None);
        let b = runtime.connect_client(ClientKind::Cli, None);
        let first_change = {
            let mut state = runtime.state.lock().unwrap();
            state.tree.view_pane(a, pane).unwrap();
            state.tree.view_pane(b, pane).unwrap();
            state
                .tree
                .declare_size(a, tab_id, Size::new(80, 24))
                .unwrap();
            state
                .tree
                .declare_size(b, tab_id, Size::new(40, 10))
                .unwrap()
        };
        runtime.apply_size_changed(&first_change);
        assert_eq!(
            runtime.state.lock().unwrap().panes[&pane]
                .snapshot()
                .unwrap()
                .rows
                .len(),
            10,
            "b, the latest, declared 40x10 last"
        );

        runtime.disconnect_client(b);

        assert_eq!(
            runtime.state.lock().unwrap().panes[&pane]
                .snapshot()
                .unwrap()
                .rows
                .len(),
            24,
            "a's own 80x24 must be reapplied once b (the latest) disconnects"
        );
    }

    #[test]
    fn login_shell_default_is_a_fixed_zero_io_value() {
        // `ServerConfig::default()` (and every test built from
        // `..ServerConfig::default()`) must stay free of hidden syscalls
        // (TASK-1.87) - this is the thing that proves it: not a claim
        // about any real account, just that the literal fallback is
        // well-formed.
        let login = LoginShell::default();
        assert!(!login.argv0.is_empty());
        assert!(!login.home.is_empty());
        assert_eq!(login.user, None);
        assert!(login.base_env.is_empty());
    }

    #[test]
    fn login_shell_resolve_identity_never_panics_and_is_never_empty() {
        // A smoke test, not a claim about this machine's own account -
        // every real account has a passwd entry, so the fallback path
        // itself is proven by fabrication in `login_env`'s own tests.
        // `base_env` stays empty here on purpose: identity resolution
        // never captures an environment (TASK-1.87 §3 - that happens
        // later, after the socket is bound).
        let login = LoginShell::resolve_identity(Duration::from_secs(5));
        assert!(!login.argv0.is_empty());
        assert!(!login.home.is_empty());
        assert!(login.base_env.is_empty());
    }

    /// AC #3: `STEG_*` and `TERM` always win, even when a captured login
    /// shell's own profile happens to export a conflicting value for one
    /// of those same keys - proven here with a fabricated `base_env`
    /// standing in for exactly that (unlikely, but not impossible) case,
    /// through `Runtime::handle`'s own public surface, a real pty and a
    /// real `env` dump - not merely by reading `launch_spec`'s own source
    /// and trusting the ordering.
    #[test]
    fn steg_vars_and_term_always_win_over_a_conflicting_value_from_the_login_shells_own_base_env() {
        let login = LoginShell {
            base_env: vec![
                (
                    std::ffi::OsString::from("PATH"),
                    std::ffi::OsString::from("/usr/bin:/bin"),
                ),
                (
                    std::ffi::OsString::from("TERM"),
                    std::ffi::OsString::from("bogus-term-from-login-shell"),
                ),
                (
                    std::ffi::OsString::from("STEG_SERVER"),
                    std::ffi::OsString::from("bogus-server-from-login-shell"),
                ),
            ],
            ..login()
        };
        let runtime = Runtime::new(std::sync::Arc::new(RealLauncher), login, TEST_GRACE);
        let description = steg_sdk::description();
        let no_context = crate::dispatch::Context::none();

        // Two short, targeted lines, never a full `env` dump: the real
        // `PATH` this developer's own machine carries can run long enough
        // to wrap `TERM=`/`STEG_SERVER=` off the default 80x24 pane
        // before either line is even printed - found by running this
        // test, not assumed, so it is written to need no particular pane
        // size at all.
        let session = match runtime.handle(
            &description,
            &no_context,
            &steg_sdk::Request {
                method: "session.new".to_string(),
                params: serde_json::json!({
                    "program": ["sh", "-c", "echo TERM=$TERM; echo STEG_SERVER=$STEG_SERVER"]
                })
                .as_object()
                .unwrap()
                .clone(),
            },
        ) {
            steg_sdk::Response::Ok(value) => value,
            steg_sdk::Response::Error(error) => panic!("session.new failed: {error:?}"),
        };
        let session_id: steg_sdk::SessionId = session["id"].as_str().unwrap().parse().unwrap();

        let pane = {
            let state = runtime.state.lock().unwrap();
            state.tree.session(session_id).unwrap().tabs()[0].panes()[0].id()
        };

        let start = std::time::Instant::now();
        let text = loop {
            let state = runtime.state.lock().unwrap();
            let screen = state.panes[&pane].snapshot().unwrap();
            let text = screen
                .rows
                .iter()
                .map(|row| row.iter().map(|run| run.text.as_str()).collect::<String>())
                .collect::<Vec<_>>()
                .join("\n");
            drop(state);
            if text.contains("STEG_SERVER=") {
                break text;
            }
            assert!(
                start.elapsed() < Duration::from_secs(20),
                "the real program's own two lines never landed within the bound; \
                 last screen: {text:?}"
            );
            std::thread::sleep(Duration::from_millis(10));
        };

        assert!(
            text.contains("TERM=xterm-256color"),
            "the server's own TERM must win over the login shell's own base_env: {text}"
        );
        assert!(
            !text.contains("bogus-term-from-login-shell"),
            "the login shell's own conflicting TERM must not survive: {text}"
        );
        assert!(
            !text.contains("bogus-server-from-login-shell"),
            "the login shell's own conflicting STEG_SERVER must not survive: {text}"
        );
    }

    /// AC #1/#4's own "a shell pane, not only a command pane" half -
    /// `launch_spec`'s own env composition does not look at which variant
    /// `Program` is at all (only `argv` differs, read above this
    /// function's own `match`), so a shell pane gets exactly the same
    /// captured base, `STEG_*` overrides and `TERM` a command pane
    /// does - proven here directly, through `session.new` with *no*
    /// `program` at all (the shell-pane case), sending real input to the
    /// real login shell this `Runtime` resolved, rather than trusted by
    /// construction alone.
    #[test]
    fn a_shell_pane_not_only_a_command_pane_gets_the_same_composed_environment() {
        let runtime = Runtime::new(std::sync::Arc::new(RealLauncher), login(), TEST_GRACE);
        let description = steg_sdk::description();
        let no_context = crate::dispatch::Context::none();

        let session = match runtime.handle(
            &description,
            &no_context,
            &steg_sdk::Request {
                method: "session.new".to_string(),
                params: serde_json::Map::new(),
            },
        ) {
            steg_sdk::Response::Ok(value) => value,
            steg_sdk::Response::Error(error) => panic!("session.new failed: {error:?}"),
        };
        let session_id: steg_sdk::SessionId = session["id"].as_str().unwrap().parse().unwrap();

        let pane = {
            let state = runtime.state.lock().unwrap();
            state.tree.session(session_id).unwrap().tabs()[0].panes()[0].id()
        };

        {
            let state = runtime.state.lock().unwrap();
            state
                .panes
                .get(&pane)
                .unwrap()
                .send_input(b"echo TERM=$TERM\n".to_vec())
                .unwrap();
        }

        let start = std::time::Instant::now();
        let text = loop {
            let state = runtime.state.lock().unwrap();
            let screen = state.panes[&pane].snapshot().unwrap();
            let text = screen
                .rows
                .iter()
                .map(|row| row.iter().map(|run| run.text.as_str()).collect::<String>())
                .collect::<Vec<_>>()
                .join("\n");
            drop(state);
            if text.contains("TERM=xterm-256color") {
                break text;
            }
            assert!(
                start.elapsed() < Duration::from_secs(20),
                "the shell pane's own echo never landed within the bound; last screen: {text:?}"
            );
            std::thread::sleep(Duration::from_millis(10));
        };

        assert!(
            text.contains("TERM=xterm-256color"),
            "a shell pane must get the server's own TERM too: {text}"
        );
    }

    /// What a panic inside one connection's call to `Runtime::handle`
    /// (TASK-1.45 §5) actually rests on: `Runtime::lock`'s own existing
    /// `unwrap_or_else(PoisonError::into_inner)`. Proved directly, one
    /// layer down from a real connection (which this task adds no
    /// deliberate panic-injection hook to simulate end-to-end, the same
    /// reasoning TASK-1.44's review gave for rejecting a shipped
    /// test-only hook): poison the lock for real, on another thread, then
    /// confirm a subsequent call through the public `Runtime::handle`
    /// still succeeds rather than hanging or propagating the poison.
    #[test]
    fn a_panic_while_holding_the_lock_does_not_poison_a_later_call() {
        let runtime = Runtime::new(std::sync::Arc::new(RealLauncher), login(), TEST_GRACE);

        let state_handle = std::sync::Arc::clone(&runtime.state);
        let panicked = std::thread::spawn(move || {
            let _guard = state_handle.lock().unwrap();
            panic!("a bug inside a request handler, simulated for this test");
        })
        .join();
        assert!(
            panicked.is_err(),
            "the spawned thread really did panic while holding the lock"
        );

        let description = steg_sdk::description();
        let context = crate::dispatch::Context::none();
        let response = runtime.handle(
            &description,
            &context,
            &steg_sdk::Request {
                method: "session.list".to_string(),
                params: serde_json::Map::new(),
            },
        );
        match response {
            steg_sdk::Response::Ok(value) => assert_eq!(value, serde_json::json!([])),
            steg_sdk::Response::Error(error) => {
                panic!("expected Ok after recovering from the poison, got {error:?}")
            }
        }
    }

    #[test]
    fn runtime_new_does_not_panic_and_starts_empty() {
        let runtime = Runtime::new(std::sync::Arc::new(RealLauncher), login(), TEST_GRACE);
        let description = steg_sdk::description();
        let context = crate::dispatch::Context::none();
        let response = runtime.handle(
            &description,
            &context,
            &steg_sdk::Request {
                method: "session.list".to_string(),
                params: serde_json::Map::new(),
            },
        );
        match response {
            steg_sdk::Response::Ok(value) => assert_eq!(value, serde_json::json!([])),
            steg_sdk::Response::Error(error) => panic!("expected Ok, got {error:?}"),
        }
    }

    /// The idle-handle fix itself (TASK-1.78 §5, item 2), proven where it
    /// can actually be observed - `state.panes`, private to this module,
    /// not through `Runtime::handle`'s own public responses, which never
    /// expose it either way (DRAFT-1: `tests/runtime.rs`'s own black-box
    /// sibling, `pane_close_on_an_already_ended_pane_succeeds_without_
    /// force`, would pass identically with or without this fix, so it is
    /// not offered as proof of it - this is).
    #[test]
    fn a_plain_close_of_an_already_ended_pane_removes_its_idle_handle() {
        let runtime = Runtime::new(std::sync::Arc::new(RealLauncher), login(), TEST_GRACE);
        let description = steg_sdk::description();
        let no_context = crate::dispatch::Context::none();

        let session = match runtime.handle(
            &description,
            &no_context,
            &steg_sdk::Request {
                method: "session.new".to_string(),
                params: serde_json::json!({"program": ["sh", "-c", "exit 0"]})
                    .as_object()
                    .unwrap()
                    .clone(),
            },
        ) {
            steg_sdk::Response::Ok(value) => value,
            steg_sdk::Response::Error(error) => panic!("session.new failed: {error:?}"),
        };
        let session_id: steg_sdk::SessionId = session["id"].as_str().unwrap().parse().unwrap();
        let (pane, tab_id) = {
            let state = runtime.state.lock().unwrap();
            let tab = &state.tree.session(session_id).unwrap().tabs()[0];
            (tab.panes()[0].id(), tab.id())
        };

        // Waits for the real exit first (R97: a command pane stays) by
        // polling the kernel's own status directly - bounded, no event
        // for this exists on the server side (`PaneEvent::Exited` only
        // reaches the drain thread, not a test holding the lock between
        // polls).
        let start = std::time::Instant::now();
        loop {
            let ended = {
                let state = runtime.state.lock().unwrap();
                matches!(
                    state
                        .tree
                        .session(session_id)
                        .and_then(|s| s.tabs()[0].pane(pane))
                        .and_then(steg_kernel::Pane::program_status),
                    Some(steg_sdk::ProgramStatus::Ended { .. })
                )
            };
            if ended {
                break;
            }
            assert!(
                start.elapsed() < Duration::from_secs(10),
                "the real `exit 0` never reached Ended within the bound"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(
            runtime.state.lock().unwrap().panes.contains_key(&pane),
            "the idle handle must still be there right after the program ends - \
             this is the gap's own starting condition, not yet the fix"
        );

        let context = crate::dispatch::Context {
            default_session: Some(session_id),
            default_tab: Some(tab_id),
            default_pane: Some(pane),
        };
        match runtime.handle(
            &description,
            &context,
            &steg_sdk::Request {
                method: "pane.close".to_string(),
                params: serde_json::json!({"target": pane.to_string()})
                    .as_object()
                    .unwrap()
                    .clone(),
            },
        ) {
            steg_sdk::Response::Ok(_) => {}
            steg_sdk::Response::Error(error) => panic!("pane.close failed: {error:?}"),
        }

        assert!(
            !runtime.state.lock().unwrap().panes.contains_key(&pane),
            "the plain close must remove the now-pointless idle handle, \
             not leave it (and its worker thread) running forever"
        );
    }

    /// Rule B (§2): the live directory (`Terminal::cwd`, set by a real
    /// OSC 7) differs from the directory the program *started* in
    /// (`Program::cwd`, given at `session.new`) - and it is the live one
    /// `pane.split` inherits, confirmed through `Runtime::handle`'s own
    /// public surface for the actions themselves, with only the
    /// "has the OSC 7 landed yet" wait using `PaneHandle::cwd` directly
    /// (this crate's own internal access), since `pane.show` always
    /// reports a pane's started-in `Program::cwd` (pure, by this task's
    /// own design) and could never observe this.
    ///
    /// The reported directory is a real one, created for this test and
    /// removed at the end: review finding 4 added an existence check to
    /// this same fallback (see the sibling test right below, proving the
    /// *other* half - a reported directory that has since vanished), so a
    /// merely-claimed path would now fall back to home instead of proving
    /// inheritance at all.
    #[test]
    fn pane_split_inherits_the_live_cwd_of_the_pane_it_split_from_not_its_started_in_cwd() {
        use steg_sdk::{Request, SessionId};

        let runtime = Runtime::new(std::sync::Arc::new(RealLauncher), login(), TEST_GRACE);
        let description = steg_sdk::description();
        let no_context = crate::dispatch::Context::none();

        let live_cwd =
            std::env::temp_dir().join(format!("steg-live-cwd-{}-{}", std::process::id(), line!()));
        std::fs::create_dir(&live_cwd).unwrap();
        let live_cwd = std::fs::canonicalize(&live_cwd).unwrap();

        let session = match runtime.handle(
            &description,
            &no_context,
            &Request {
                method: "session.new".to_string(),
                params: serde_json::json!({
                    "cwd": "/private/tmp",
                    "program": ["sh", "-c",
                        format!(
                            "printf '\\033]7;file://{}\\007'; sleep 100",
                            live_cwd.display()
                        )],
                })
                .as_object()
                .unwrap()
                .clone(),
            },
        ) {
            steg_sdk::Response::Ok(value) => value,
            steg_sdk::Response::Error(error) => panic!("session.new failed: {error:?}"),
        };
        let session_id: SessionId = session["id"].as_str().unwrap().parse().unwrap();

        let source_pane = {
            let state = runtime.state.lock().unwrap();
            state.tree.session(session_id).unwrap().tabs()[0].panes()[0].id()
        };

        // Bounded, never a bare wait (review nit): `PaneHandle::cwd` is
        // itself bounded (`CWD_LOOKUP_TIMEOUT`), so this polls it, not an
        // unbounded channel - there is no event to wait on instead
        // (`PaneEvent` only ever carries `Exited`; an event for a changed
        // cwd is TASK-1.46's own gap, not this task's to close), so the
        // only alternative to polling a value is blocking forever on one
        // that may never change.
        let start = std::time::Instant::now();
        loop {
            let live = runtime
                .state
                .lock()
                .unwrap()
                .panes
                .get(&source_pane)
                .unwrap()
                .cwd()
                .unwrap();
            if live.as_deref() == Some(live_cwd.to_str().unwrap()) {
                break;
            }
            assert!(
                start.elapsed() < Duration::from_secs(20),
                "the real OSC 7 never landed within the bound; last seen: {live:?}"
            );
            std::thread::sleep(Duration::from_millis(10));
        }

        let tab_id = {
            let state = runtime.state.lock().unwrap();
            state.tree.tab_of_pane(source_pane).unwrap()
        };
        let context = crate::dispatch::Context {
            default_session: Some(session_id),
            default_tab: Some(tab_id),
            default_pane: Some(source_pane),
        };

        let split = match runtime.handle(
            &description,
            &context,
            &Request {
                method: "pane.split".to_string(),
                params: serde_json::json!({
                    "direction": "right",
                    "program": ["sh", "-c", "sleep 100"],
                })
                .as_object()
                .unwrap()
                .clone(),
            },
        ) {
            steg_sdk::Response::Ok(value) => value,
            steg_sdk::Response::Error(error) => panic!("pane.split failed: {error:?}"),
        };
        let new_pane: steg_sdk::PaneId = split["id"].as_str().unwrap().parse().unwrap();

        let new_pane_program = {
            let state = runtime.state.lock().unwrap();
            state.tree.session(session_id).unwrap().tabs()[0]
                .pane(new_pane)
                .unwrap()
                .program()
                .cloned()
        };
        assert_eq!(
            new_pane_program.map(|program| program.cwd().to_string()),
            Some(live_cwd.to_str().unwrap().to_string()),
            "the new pane must inherit the live cwd (set by OSC 7), not the \
             source pane's own started-in cwd (/private/tmp)"
        );

        std::fs::remove_dir(&live_cwd).unwrap();
    }

    /// Review finding 4, rule B's own half: a live cwd the source pane
    /// reported (by real OSC 7) but that no longer exists on disk by the
    /// time the split runs - the caller named no `cwd` of their own, so
    /// this is rule B's own fallback deciding there is no usable answer,
    /// never the explicit-`cwd` refusal `pane_split_with_a_nonexistent_cwd_*`
    /// (`tests/runtime.rs`) proves instead. Falls back to `login.home`,
    /// exactly as if the source pane had reported no cwd at all.
    #[test]
    fn pane_split_falls_back_to_home_when_the_inherited_live_cwd_no_longer_exists() {
        use steg_sdk::{Request, SessionId};

        let runtime = Runtime::new(std::sync::Arc::new(RealLauncher), login(), TEST_GRACE);
        let description = steg_sdk::description();
        let no_context = crate::dispatch::Context::none();

        let vanished = std::env::temp_dir().join(format!(
            "steg-vanishing-cwd-{}-{}",
            std::process::id(),
            line!()
        ));
        std::fs::create_dir(&vanished).unwrap();
        let vanished = std::fs::canonicalize(&vanished).unwrap();

        let session = match runtime.handle(
            &description,
            &no_context,
            &Request {
                method: "session.new".to_string(),
                params: serde_json::json!({
                    "cwd": "/private/tmp",
                    "program": ["sh", "-c",
                        format!(
                            "printf '\\033]7;file://{}\\007'; sleep 100",
                            vanished.display()
                        )],
                })
                .as_object()
                .unwrap()
                .clone(),
            },
        ) {
            steg_sdk::Response::Ok(value) => value,
            steg_sdk::Response::Error(error) => panic!("session.new failed: {error:?}"),
        };
        let session_id: SessionId = session["id"].as_str().unwrap().parse().unwrap();

        let source_pane = {
            let state = runtime.state.lock().unwrap();
            state.tree.session(session_id).unwrap().tabs()[0].panes()[0].id()
        };

        // Bounded, never a bare wait (review nit): `PaneHandle::cwd` is
        // itself bounded (`CWD_LOOKUP_TIMEOUT`), so this polls it, not an
        // unbounded channel - there is no event to wait on instead, since
        // `PaneEvent` only ever carries `Exited` (events for anything
        // else, including a changed cwd, are TASK-1.46's own gap, not
        // this task's to close); the only alternative to polling a value
        // is blocking forever on one that may never change. Same shape as
        // this test's own sibling above, waiting for the real OSC 7 to
        // land before the directory it names is removed out from under
        // it.
        let start = std::time::Instant::now();
        loop {
            let live = runtime
                .state
                .lock()
                .unwrap()
                .panes
                .get(&source_pane)
                .unwrap()
                .cwd()
                .unwrap();
            if live.as_deref() == Some(vanished.to_str().unwrap()) {
                break;
            }
            assert!(
                start.elapsed() < Duration::from_secs(20),
                "the real OSC 7 never landed within the bound; last seen: {live:?}"
            );
            std::thread::sleep(Duration::from_millis(10));
        }

        // The directory the source pane just reported is gone by the
        // time the split below reads it.
        std::fs::remove_dir(&vanished).unwrap();

        let tab_id = {
            let state = runtime.state.lock().unwrap();
            state.tree.tab_of_pane(source_pane).unwrap()
        };
        let context = crate::dispatch::Context {
            default_session: Some(session_id),
            default_tab: Some(tab_id),
            default_pane: Some(source_pane),
        };

        let split = match runtime.handle(
            &description,
            &context,
            &Request {
                method: "pane.split".to_string(),
                params: serde_json::json!({
                    "direction": "right",
                    "program": ["sh", "-c", "sleep 100"],
                })
                .as_object()
                .unwrap()
                .clone(),
            },
        ) {
            steg_sdk::Response::Ok(value) => value,
            steg_sdk::Response::Error(error) => {
                panic!("pane.split must fall back to home, not refuse: {error:?}")
            }
        };
        let new_pane: steg_sdk::PaneId = split["id"].as_str().unwrap().parse().unwrap();

        let new_pane_program = {
            let state = runtime.state.lock().unwrap();
            state.tree.session(session_id).unwrap().tabs()[0]
                .pane(new_pane)
                .unwrap()
                .program()
                .cloned()
        };
        assert_eq!(
            new_pane_program.map(|program| program.cwd().to_string()),
            Some(login().home),
            "a vanished inherited cwd must fall back to the login shell's home, \
             not refuse and not use the (now nonexistent) live cwd"
        );
    }

    /// Review finding 2: the plan's own §4 proof promised `pane.split`,
    /// `pane.resize`, `pane.close` and a swap, each driven through
    /// `Runtime::handle`, with both the terminal and the pty checked
    /// after each - and that the test would actually fail without the
    /// general step. `pane.resize` is still `not_implemented_yet`
    /// (TASK-1.42's own gap, not this task's to build), so this covers
    /// `pane.split` (both directions), `pane.close` and `pane.swap`, the
    /// three of the four this task's own dispatch arms serve.
    ///
    /// Every probe pane runs a tight polling loop (`while :; do stty
    /// size; sleep 0.05; done`), not a `SIGWINCH` trap: a signal-based
    /// first attempt at this test raced the trap's own installation
    /// against the one-time automatic resize the general step performs,
    /// which is a real, reproducible flake (recorded on this task) a
    /// polling loop inside the child does not have, since `stty size`
    /// only ever reads the pty's *current* winsize, with nothing to miss.
    ///
    /// The terminal side's row count is read via `Screen::rows.len()`;
    /// `Screen` has no column-count field at all (confirmed: an empty
    /// row is one zero-length run, carrying no width information), so
    /// the terminal's own column width is instead proven once, directly,
    /// right after the first resize, by printing exactly the expected
    /// number of characters and confirming none of them wrap to a
    /// second row - the same `Terminal::resize` call path every later
    /// step in this test also goes through, so this is not re-proven at
    /// every step, only established once.
    #[test]
    fn the_general_sizing_step_reaches_every_pane_through_split_close_and_swap() {
        use steg_sdk::{ClientKind, Request, SessionId, Size, TabId};

        let runtime = Runtime::new(std::sync::Arc::new(RealLauncher), login(), TEST_GRACE);
        let description = steg_sdk::description();
        let no_context = crate::dispatch::Context::none();

        fn polling_program() -> Vec<&'static str> {
            vec!["sh", "-c", "while :; do stty size; sleep 0.05; done"]
        }

        /// Like `polling_program`, but prints exactly 61 `x`s once,
        /// first - used only for `first_pane`'s own program, to prove
        /// the terminal's column width directly (see this test's own
        /// module doc), before the polling loop's own output has a
        /// chance to scroll that line away.
        fn polling_program_with_column_marker() -> Vec<&'static str> {
            vec![
                "sh",
                "-c",
                "printf '%s\n' \"$(printf 'x%.0s' $(seq 1 61))\"; \
                 while :; do stty size; sleep 0.05; done",
            ]
        }

        fn screen_text(runtime: &Runtime, pane: steg_sdk::PaneId) -> String {
            let state = runtime.state.lock().unwrap();
            let screen = state.panes[&pane].snapshot().unwrap();
            screen
                .rows
                .iter()
                .map(|row| row.iter().map(|run| run.text.as_str()).collect::<String>())
                .collect::<Vec<_>>()
                .join(
                    "
",
                )
        }

        /// Polls (bounded, never a bare wait; review nit) until `pane`'s
        /// own polling loop reports exactly `(cols, rows)` - proving the
        /// pty side. There is no event to wait on instead: `PaneEvent`
        /// only ever carries `Exited`, and an event for a program's own
        /// output landing is TASK-1.46's own gap, not this task's to
        /// close, so the only alternative to polling the snapshot is
        /// blocking forever on text that may never appear.
        fn wait_for_reported_size(runtime: &Runtime, pane: steg_sdk::PaneId, expected: (u16, u16)) {
            let wanted = format!("{} {}", expected.1, expected.0);
            let start = std::time::Instant::now();
            loop {
                let text = screen_text(runtime, pane);
                if text.lines().any(|line| line.trim() == wanted) {
                    return;
                }
                assert!(
                    start.elapsed() < Duration::from_secs(20),
                    "pane {pane} never reported pty size {wanted}; last screen: {text:?}"
                );
                std::thread::sleep(Duration::from_millis(20));
            }
        }

        /// The terminal side's row count, directly.
        fn terminal_rows(runtime: &Runtime, pane: steg_sdk::PaneId) -> usize {
            let state = runtime.state.lock().unwrap();
            state.panes[&pane].snapshot().unwrap().rows.len()
        }

        /// Every pane `tab_sizes_now` currently reports for `tab`, read
        /// under the lock - the one source of truth every assertion
        /// below checks against, never a size this test computed itself.
        fn expected_sizes(
            runtime: &Runtime,
            tab: TabId,
        ) -> std::collections::HashMap<steg_sdk::PaneId, Size> {
            let state = runtime.state.lock().unwrap();
            state
                .tree
                .tab_sizes_now(tab)
                .expect("this test always declares a size before checking")
                .panes
                .into_iter()
                .collect()
        }

        // One pane, its own session.
        let first = match runtime.handle(
            &description,
            &no_context,
            &Request {
                method: "session.new".to_string(),
                params: serde_json::json!({"program": polling_program_with_column_marker()})
                    .as_object()
                    .unwrap()
                    .clone(),
            },
        ) {
            steg_sdk::Response::Ok(value) => value,
            steg_sdk::Response::Error(error) => panic!("session.new failed: {error:?}"),
        };
        let session_id: SessionId = first["id"].as_str().unwrap().parse().unwrap();
        let (first_pane, tab_id) = {
            let state = runtime.state.lock().unwrap();
            let tab = &state.tree.session(session_id).unwrap().tabs()[0];
            (tab.panes()[0].id(), tab.id())
        };

        // Declared directly on the kernel - the only way to get a tab
        // sized at all today (no `client` dispatch method exists) - then
        // applied through the public `apply_size_changed` once, by hand,
        // exactly as a future `client.*` dispatch method would: nothing
        // about *this* first application goes through the general step,
        // since nothing has gone through `Runtime::handle` yet to trigger
        // it. Everything from here on on does.
        let initial_change = {
            let mut state = runtime.state.lock().unwrap();
            let client = state.tree.connect_client(ClientKind::App, None);
            state.tree.view_pane(client, first_pane).unwrap();
            state
                .tree
                .declare_size(client, tab_id, Size::new(61, 20))
                .unwrap()
        };
        runtime.apply_size_changed(&initial_change);

        // The terminal's own column width, proven once, checked first
        // and with a short bound: the polling loop's own output scrolls
        // this marker line away within about a second (20 rows at one
        // line every 50ms), so this has to win that race, not the other
        // way around. Bounded sleep-polling, not an event wait, for the
        // same reason as `wait_for_reported_size` above: no event exists
        // for a snapshot's text changing.
        {
            let start = std::time::Instant::now();
            loop {
                let text = screen_text(&runtime, first_pane);
                if let Some(row) = text.lines().find(|l| l.starts_with('x')) {
                    assert_eq!(
                        row.trim_end(),
                        "x".repeat(61),
                        "61 columns must fit exactly"
                    );
                    break;
                }
                assert!(
                    start.elapsed() < Duration::from_millis(800),
                    "never saw the 61-column marker row before it would have scrolled away"
                );
                std::thread::sleep(Duration::from_millis(5));
            }
        }

        wait_for_reported_size(&runtime, first_pane, (61, 20));
        assert_eq!(terminal_rows(&runtime, first_pane), 20);

        let context = crate::dispatch::Context {
            default_session: Some(session_id),
            default_tab: Some(tab_id),
            default_pane: Some(first_pane),
        };

        // Split right: two panes.
        let split_right = match runtime.handle(
            &description,
            &context,
            &Request {
                method: "pane.split".to_string(),
                params: serde_json::json!({"direction": "right", "program": polling_program()})
                    .as_object()
                    .unwrap()
                    .clone(),
            },
        ) {
            steg_sdk::Response::Ok(value) => value,
            steg_sdk::Response::Error(error) => panic!("pane.split (right) failed: {error:?}"),
        };
        let second_pane: steg_sdk::PaneId = split_right["id"].as_str().unwrap().parse().unwrap();

        let expected = expected_sizes(&runtime, tab_id);
        assert_eq!(expected.len(), 2, "two panes after one split");
        for &pane in &[first_pane, second_pane] {
            let size = expected[&pane];
            wait_for_reported_size(&runtime, pane, (size.columns(), size.rows()));
            assert_eq!(terminal_rows(&runtime, pane), usize::from(size.rows()));
        }

        // Split the first pane down too: three panes.
        let split_down = match runtime.handle(
            &description,
            &crate::dispatch::Context {
                default_session: Some(session_id),
                default_tab: Some(tab_id),
                default_pane: Some(first_pane),
            },
            &Request {
                method: "pane.split".to_string(),
                params: serde_json::json!({"direction": "down", "program": polling_program()})
                    .as_object()
                    .unwrap()
                    .clone(),
            },
        ) {
            steg_sdk::Response::Ok(value) => value,
            steg_sdk::Response::Error(error) => panic!("pane.split (down) failed: {error:?}"),
        };
        let third_pane: steg_sdk::PaneId = split_down["id"].as_str().unwrap().parse().unwrap();

        let expected = expected_sizes(&runtime, tab_id);
        assert_eq!(expected.len(), 3, "three panes after the second split");
        for &pane in &[first_pane, second_pane, third_pane] {
            let size = expected[&pane];
            wait_for_reported_size(&runtime, pane, (size.columns(), size.rows()));
            assert_eq!(terminal_rows(&runtime, pane), usize::from(size.rows()));
        }

        // Close the third pane: its space returns to the first (R95).
        match runtime.handle(
            &description,
            &context,
            &Request {
                method: "pane.close".to_string(),
                params: serde_json::json!({
                    "target": third_pane.to_string(),
                    "force": true,
                })
                .as_object()
                .unwrap()
                .clone(),
            },
        ) {
            steg_sdk::Response::Ok(_) => {}
            steg_sdk::Response::Error(error) => panic!("pane.close failed: {error:?}"),
        }

        let expected = expected_sizes(&runtime, tab_id);
        assert_eq!(expected.len(), 2, "back to two panes after the close");
        for &pane in &[first_pane, second_pane] {
            let size = expected[&pane];
            wait_for_reported_size(&runtime, pane, (size.columns(), size.rows()));
            assert_eq!(terminal_rows(&runtime, pane), usize::from(size.rows()));
        }
        let before_swap = expected;

        // Swap the two remaining panes.
        match runtime.handle(
            &description,
            &context,
            &Request {
                method: "pane.swap".to_string(),
                params: serde_json::json!({"with": second_pane.to_string()})
                    .as_object()
                    .unwrap()
                    .clone(),
            },
        ) {
            steg_sdk::Response::Ok(_) => {}
            steg_sdk::Response::Error(error) => panic!("pane.swap failed: {error:?}"),
        }

        let after_swap = expected_sizes(&runtime, tab_id);
        assert_eq!(after_swap.len(), 2);
        assert_ne!(
            after_swap, before_swap,
            "the swap must actually change which pane has which size,              or this test cannot tell a correct swap from a no-op"
        );
        for &pane in &[first_pane, second_pane] {
            let size = after_swap[&pane];
            wait_for_reported_size(&runtime, pane, (size.columns(), size.rows()));
            assert_eq!(terminal_rows(&runtime, pane), usize::from(size.rows()));
        }
    }

    /// R100's own last clause: a tab shrinking through a later
    /// declaration can push a pane under the 20x4 floor, refusing
    /// nothing and cropping instead - proven through `apply_size_changed`
    /// (the public method), to show `Runtime` applies whatever the
    /// kernel computes faithfully, with no floor logic of its own that
    /// could get it wrong.
    #[test]
    fn apply_size_changed_lets_a_pane_go_under_the_floor_when_the_kernel_says_so() {
        let (mut state, pane) = fresh_state_with_one_pane();
        let (events_tx, _events_rx) = mpsc::channel();
        let handle = crate::pane::start(
            crate::pty::LaunchSpec {
                argv: vec!["sh".to_string(), "-c".to_string(), "sleep 100".to_string()],
                cwd: "/tmp".to_string(),
                env: test_base_env(),
                size: (80, 24),
            },
            pane,
            std::sync::Arc::new(RealLauncher),
            events_tx,
        )
        .unwrap();
        state.panes.insert(pane, handle);

        let tab_id = state.tree.sessions()[0].tabs()[0].id();
        let client = state.tree.connect_client(steg_sdk::ClientKind::App, None);
        state.tree.view_pane(client, pane).unwrap();
        // Below R100's 20x4 floor on purpose - `declare_size` allows this
        // "when a window grows smaller" (R100's own last clause), and
        // `tab_sizes_now` reports exactly that, unclamped.
        let change = state
            .tree
            .declare_size(client, tab_id, Size::new(5, 1))
            .unwrap();
        assert_eq!(change.panes, vec![(pane, Size::new(5, 1))]);

        super::apply_size_changed_locked(&mut state, &change);

        let screen = state.panes[&pane].snapshot().unwrap();
        assert_eq!(
            screen.rows.len(),
            1,
            "the terminal was cropped, not refused"
        );
    }

    // --- typed command marks (TASK-1.84) ----------------------------------
    //
    // Every test below drives a real `Runtime` - its own drain thread and
    // event channel, not a hand-built `apply_event_locked` call - with a
    // real pty program that prints real OSC 133 sequences directly (no
    // shell integration exists yet; that is TASK-1.85's own job, so each
    // script here stands in for a shell that already has it). Every wait
    // is bounded; nothing here adds a new thread or a new process of its
    // own (AC #3) - marks ride the `events` channel and drain thread
    // `Runtime::new` already starts, and every test lets `Drop` (or an
    // explicit `stop()`) end whatever real program it started.

    const MARK_TIMEOUT: Duration = Duration::from_secs(20);

    fn moment_now(state: &State) -> Moment {
        Moment::new(Instant::now().duration_since(state.clock_origin))
    }

    fn find_pane(tree: &steg_kernel::Tree, pane: steg_sdk::PaneId) -> Option<&steg_kernel::Pane> {
        tree.sessions()
            .iter()
            .find_map(|session| session.tabs().iter().find_map(|tab| tab.pane(pane)))
    }

    fn pane_status(runtime: &Runtime, pane: steg_sdk::PaneId) -> Option<Status> {
        let state = runtime.state.lock().unwrap();
        let now = moment_now(&state);
        find_pane(&state.tree, pane).and_then(|p| p.status(now))
    }

    fn pane_program_status(runtime: &Runtime, pane: steg_sdk::PaneId) -> Option<ProgramStatus> {
        let state = runtime.state.lock().unwrap();
        find_pane(&state.tree, pane).and_then(steg_kernel::Pane::program_status)
    }

    fn tree_next_status_deadline(runtime: &Runtime) -> Option<Moment> {
        let state = runtime.state.lock().unwrap();
        let now = moment_now(&state);
        state.tree.next_status_deadline(now)
    }

    /// Bounded, polling (never a bare wait): there is no event a test can
    /// wait on instead - a status read is a value, not an event stream
    /// (R52's own gap, TASK-1.46) - the same reasoning every other real-
    /// process wait in this crate already gives.
    fn wait_for_pane_status(runtime: &Runtime, pane: steg_sdk::PaneId, expected: Option<Status>) {
        let start = Instant::now();
        loop {
            let found = pane_status(runtime, pane);
            if found == expected {
                return;
            }
            assert!(
                start.elapsed() < MARK_TIMEOUT,
                "pane {pane} never reached status {expected:?}; last seen {found:?}"
            );
            thread::sleep(Duration::from_millis(20));
        }
    }

    /// Bounded, polling the one fact (TASK-1.84 review) that is true
    /// exactly when `apply_event_locked` has applied `expected` of this
    /// pane's own `CommandOutputStarted`/`CommandEnded` events - read
    /// under the same lock `apply_event_locked` itself holds while
    /// updating it, so observing `expected` here means that call has
    /// already fully returned, not merely that a sleep has elapsed.
    fn wait_for_applied_mark_count(runtime: &Runtime, pane: steg_sdk::PaneId, expected: u64) {
        let start = Instant::now();
        loop {
            let found = {
                let state = runtime.state.lock().unwrap();
                state.applied_marks.get(&pane).copied().unwrap_or(0)
            };
            if found >= expected {
                return;
            }
            assert!(
                start.elapsed() < MARK_TIMEOUT,
                "pane {pane} never reached {expected} applied marks; last seen {found}"
            );
            thread::sleep(Duration::from_millis(5));
        }
    }

    fn send_input(runtime: &Runtime, pane: steg_sdk::PaneId, bytes: &[u8]) {
        let state = runtime.state.lock().unwrap();
        state.panes[&pane].send_input(bytes.to_vec()).unwrap();
    }

    /// Builds a fresh session/tab/pane on `runtime`'s own tree, started as
    /// `program` (a `Program::Shell` or `Program::Command` - R97's own
    /// hinge on whether a typed-command mark can ever change this pane's
    /// status at all), and spawns `argv` as its *real* process through
    /// `runtime`'s own event channel and drain thread - not a hand-built
    /// `apply_event_locked` call, so every test using this is exercising
    /// the real wiring §1's own ordering argument describes, not a
    /// simulation of it. `program` and `argv` are independent on purpose,
    /// the same decoupling `fresh_state_with_one_pane` already relies on
    /// elsewhere in this file: the kernel's own `Program` is bookkeeping
    /// metadata, never read by anything that decides what real process
    /// `crate::pane::start` actually runs.
    fn start_real_pane(
        runtime: &Runtime,
        program: Program,
        argv: Vec<&str>,
    ) -> (steg_sdk::SessionId, steg_sdk::TabId, steg_sdk::PaneId) {
        let (session_id, tab_id, pane) = {
            let mut state = runtime.state.lock().unwrap();
            let session_id = state.tree.new_session(None).unwrap();
            let pane = state.tree.session(session_id).unwrap().tabs()[0].panes()[0].id();
            let tab_id = state.tree.tab_of_pane(pane).unwrap();
            state.tree.start_program(pane, program).unwrap();
            (session_id, tab_id, pane)
        };
        let events = runtime
            .events
            .clone()
            .expect("runtime just built for this test");
        let handle = crate::pane::start(
            crate::pty::LaunchSpec {
                argv: argv.into_iter().map(str::to_string).collect(),
                cwd: "/tmp".to_string(),
                env: test_base_env(),
                size: (80, 24),
            },
            pane,
            std::sync::Arc::new(RealLauncher),
            events,
        )
        .unwrap();
        runtime.state.lock().unwrap().panes.insert(pane, handle);
        (session_id, tab_id, pane)
    }

    fn login_for_marks() -> LoginShell {
        LoginShell {
            argv0: "/bin/sh".to_string(),
            home: "/tmp".to_string(),
            user: None,
            base_env: test_base_env(),
        }
    }

    /// Criterion 2's first shape, and criterion 1's own wording: the
    /// kernel's status for the pane, and the tab's shown status read
    /// through the kernel, both reflect a typed command that succeeds.
    #[test]
    fn a_typed_command_that_succeeds_is_running_then_finished_and_the_tab_shows_it() {
        let runtime = Runtime::new(
            std::sync::Arc::new(RealLauncher),
            login_for_marks(),
            TEST_GRACE,
        );
        let (_, tab_id, pane) = start_real_pane(
            &runtime,
            Program::Shell {
                cwd: "/tmp".to_string(),
            },
            vec![
                "sh",
                "-c",
                "printf '\\033]133;C\\007'; sleep 0.5; printf '\\033]133;D;0\\007'; sleep 100",
            ],
        );

        wait_for_pane_status(&runtime, pane, Some(Status::Running));
        wait_for_pane_status(&runtime, pane, Some(Status::Finished));

        let state = runtime.state.lock().unwrap();
        let now = moment_now(&state);
        assert_eq!(
            state.tree.tab_shown_status(tab_id, now).unwrap(),
            Some(pane),
            "the tab's shown status must reflect the typed command, read through the kernel \
             (criterion 1)"
        );
    }

    #[test]
    fn a_typed_command_that_fails_is_failed_with_its_exit_code() {
        let runtime = Runtime::new(
            std::sync::Arc::new(RealLauncher),
            login_for_marks(),
            TEST_GRACE,
        );
        let (_, _, pane) = start_real_pane(
            &runtime,
            Program::Shell {
                cwd: "/tmp".to_string(),
            },
            vec![
                "sh",
                "-c",
                "printf '\\033]133;C\\007'; sleep 0.5; printf '\\033]133;D;3\\007'; sleep 100",
            ],
        );

        wait_for_pane_status(&runtime, pane, Some(Status::Running));
        wait_for_pane_status(&runtime, pane, Some(Status::Failed { exit_code: 3 }));
    }

    #[test]
    fn a_typed_command_still_running_stays_running_until_told_otherwise() {
        let runtime = Runtime::new(
            std::sync::Arc::new(RealLauncher),
            login_for_marks(),
            TEST_GRACE,
        );
        let (_, _, pane) = start_real_pane(
            &runtime,
            Program::Shell {
                cwd: "/tmp".to_string(),
            },
            vec![
                "sh",
                "-c",
                "printf '\\033]133;C\\007'; read line; printf '\\033]133;D;0\\007'; sleep 100",
            ],
        );

        wait_for_pane_status(&runtime, pane, Some(Status::Running));

        // This half is what makes the assertion above not vacuous: a
        // still-running test that only ever checks `Running` cannot tell
        // "the `CommandOutputStarted` arm works" from "nothing on this
        // path was ever reached". Telling the script to finish, from
        // this test, and watching the status actually move away from
        // `Running`, proves the wiring is live.
        send_input(&runtime, pane, b"\n");

        wait_for_pane_status(&runtime, pane, Some(Status::Finished));
    }

    #[test]
    fn a_shell_with_no_marks_leaves_every_status_field_unchanged_until_one_arrives() {
        let runtime = Runtime::new(
            std::sync::Arc::new(RealLauncher),
            login_for_marks(),
            TEST_GRACE,
        );
        let (_, _, pane) = start_real_pane(
            &runtime,
            Program::Shell {
                cwd: "/tmp".to_string(),
            },
            vec![
                "sh",
                "-c",
                "read line; printf '\\033]133;C\\007'; sleep 100",
            ],
        );

        // Bounded, not a race against nothing: every field R82/R83 govern
        // stays exactly as a shell idling at its own prompt starts out,
        // for as long as nothing has reported a mark.
        let start = Instant::now();
        while start.elapsed() < Duration::from_millis(300) {
            assert_eq!(
                pane_program_status(&runtime, pane),
                Some(ProgramStatus::Running),
                "a shell pane's own program status is unaffected either way"
            );
            assert_eq!(
                pane_status(&runtime, pane),
                None,
                "idle at its own prompt (R82/R83's own baseline): no mark has arrived yet"
            );
            assert_eq!(
                tree_next_status_deadline(&runtime),
                None,
                "nothing is scheduled to end on its own either"
            );
            thread::sleep(Duration::from_millis(20));
        }

        // The control named in the plan: the same harness, the same
        // pane, does detect a change once a mark actually arrives - so
        // the three "unchanged" assertions above were reading a
        // mechanism that can change, not one this test could never have
        // failed regardless of whether marks were wired up at all.
        send_input(&runtime, pane, b"\n");
        wait_for_pane_status(&runtime, pane, Some(Status::Running));
    }

    #[test]
    fn a_mark_then_the_shells_own_real_exit_cascades_with_no_panic() {
        let runtime = Runtime::new(
            std::sync::Arc::new(RealLauncher),
            login_for_marks(),
            TEST_GRACE,
        );
        let (session_id, _, pane) = start_real_pane(
            &runtime,
            Program::Shell {
                cwd: "/tmp".to_string(),
            },
            vec!["sh", "-c", "printf '\\033]133;C\\007'; sleep 0.3; exit 7"],
        );

        wait_for_pane_status(&runtime, pane, Some(Status::Running));

        // The shell pane's own real exit, right behind the mark on the
        // same channel (§1's own ordering argument: the worker drains a
        // mark from `PtyBytes` before it ever reaches a later `Exited`
        // command, so both land on `events` in that order too) - R97's
        // cascade (a shell pane closes when its program exits) must run
        // exactly as `apply_event_closes_a_shell_pane_through_the_same_
        // cascade_as_a_manual_close` already proves with no mark at all,
        // not be disturbed by a `CommandOutputStarted` just ahead of it.
        let start = Instant::now();
        loop {
            let gone = {
                let state = runtime.state.lock().unwrap();
                state.tree.session(session_id).is_none() && !state.panes.contains_key(&pane)
            };
            if gone {
                break;
            }
            assert!(
                start.elapsed() < MARK_TIMEOUT,
                "the session never closed through the real exit cascade"
            );
            thread::sleep(Duration::from_millis(20));
        }
    }

    #[test]
    fn a_mark_for_a_pane_the_tree_no_longer_holds_does_nothing_and_does_not_panic() {
        // Same shape as `apply_event_on_a_pane_the_tree_no_longer_holds_
        // removes_the_stale_handle_without_panicking`: the tree never
        // gets this pane at all, standing in for the one path this arm
        // is actually reachable by (TASK-1.78's review traced it): a
        // forced close's own grace window, where `close_and_dispatch`
        // has already removed the pane from the tree, but `force_end`
        // has not yet ended its still-running program, which can still
        // emit a mark in the meantime. On the real path the handle
        // itself has already moved into `force_end`'s own local scope,
        // out of `panes` entirely - kept in `panes` here only to prove
        // this arm does not touch it (`assert!(state.panes.contains_
        // key(...))` below), not to claim the real path ever has it
        // there too.
        let tree = steg_kernel::Tree::new();
        let gone = steg_sdk::PaneId::from_raw(999);
        let (events_tx, _events_rx) = mpsc::channel();
        let handle = crate::pane::start(
            crate::pty::LaunchSpec {
                argv: vec!["sh".to_string(), "-c".to_string(), "sleep 100".to_string()],
                cwd: "/tmp".to_string(),
                env: test_base_env(),
                size: (80, 24),
            },
            gone,
            std::sync::Arc::new(RealLauncher),
            events_tx,
        )
        .unwrap();
        let mut state = test_state(tree, HashMap::from([(gone, handle)]));

        apply_event_locked(&mut state, PaneEvent::CommandOutputStarted { pane: gone });
        apply_event_locked(
            &mut state,
            PaneEvent::CommandEnded {
                pane: gone,
                exit_code: Some(0),
            },
        );

        assert!(
            state.panes.contains_key(&gone),
            "unlike `Exited`, a mark's `PaneNotFound` must not remove the handle - the \
             program it owns may still be running"
        );

        // The handle this test built is real (a real spawned `sleep
        // 100`) - ending it here is the same cleanup path the orphan
        // would get for real, either from its own eventual exit (the
        // sibling test's own `Exited`-driven removal) or from
        // `Runtime::stop` ending every pane when the whole server
        // stops; this test has neither a tree pane nor a `Runtime`
        // around it to give it one automatically. `force_end` (TASK-
        // 1.78) is that same cleanup path's own real implementation now
        // that `PaneHandle::stop` no longer exists.
        let handle = state.panes.remove(&gone).unwrap();
        force_end(vec![(gone, handle)], TEST_GRACE);
    }

    /// A command pane's own program printing OSC 133 (R97): both marks
    /// are applied by the runtime - waited on through `applied_marks`,
    /// never a sleep or a different pane's own marks - and change
    /// nothing, since the `D` mark's own exit code (3) differs from the
    /// real exit this script ends with (0): a wrongly-applied mark would
    /// read `Failed { exit_code: 3 }`, a value the real exit path alone
    /// can never produce. Only the later, real exit drives `Finished`.
    #[test]
    fn a_command_panes_own_program_printing_marks_does_not_change_its_status() {
        let runtime = Runtime::new(
            std::sync::Arc::new(RealLauncher),
            login_for_marks(),
            TEST_GRACE,
        );

        let script = "printf '\\033]133;C\\007'; sleep 0.3; printf '\\033]133;D;3\\007'; \
                       sleep 0.3; exit 0";
        let (_, _, pane) = start_real_pane(
            &runtime,
            Program::Command {
                argv: vec!["sh".to_string(), "-c".to_string(), script.to_string()],
                cwd: "/tmp".to_string(),
            },
            vec!["sh", "-c", script],
        );

        // `start_program` already set this the moment the program did
        // (kernel `Pane::start_program`'s own doc) - independent of
        // anything the real process has printed yet.
        assert_eq!(pane_status(&runtime, pane), Some(Status::Running));

        // Both of this pane's own marks (`C`, then `D;3`) have now been
        // applied by the runtime's drain thread.
        wait_for_applied_mark_count(&runtime, pane, 2);

        assert_eq!(
            pane_status(&runtime, pane),
            Some(Status::Running),
            "a command pane's own program always wins over a mark (R97)"
        );
        assert_eq!(
            pane_program_status(&runtime, pane),
            Some(ProgramStatus::Running)
        );

        // Only the program's own real exit (`record_exit`), never `note_
        // command_ended`, moves it from here - and it reads `Finished`
        // (the real exit's own code, 0), never `Failed` (the mark's own
        // code, 3).
        wait_for_pane_status(&runtime, pane, Some(Status::Finished));
    }

    /// A pane.show/tab.show request for `pane`'s own context, over the
    /// public `Runtime::handle` - the same entry point a real connection
    /// reaches (`connection.rs`). Panics with the response's own error on
    /// anything but `Response::Ok`, since every call site below already
    /// knows the method it names must succeed.
    fn show_pane_and_tab(
        runtime: &Runtime,
        session: steg_sdk::SessionId,
        tab: steg_sdk::TabId,
        pane: steg_sdk::PaneId,
    ) -> (serde_json::Value, serde_json::Value) {
        let description = steg_sdk::description();
        let context = crate::dispatch::Context {
            default_session: Some(session),
            default_tab: Some(tab),
            default_pane: Some(pane),
        };
        let pane_record = match runtime.handle(
            &description,
            &context,
            &steg_sdk::Request {
                method: "pane.show".to_string(),
                params: serde_json::Map::new(),
            },
        ) {
            steg_sdk::Response::Ok(value) => value,
            steg_sdk::Response::Error(error) => panic!("pane.show failed: {error:?}"),
        };
        let tab_record = match runtime.handle(
            &description,
            &context,
            &steg_sdk::Request {
                method: "tab.show".to_string(),
                params: serde_json::Map::new(),
            },
        ) {
            steg_sdk::Response::Ok(value) => value,
            steg_sdk::Response::Error(error) => panic!("tab.show failed: {error:?}"),
        };
        (pane_record, tab_record)
    }

    /// AC #4's "a typed command's status reaching pane show and tab show
    /// through a real pty" (TASK-1.84's own marks), proven through the
    /// public `Runtime::handle` - not just the kernel's own `Pane::status`
    /// the way this module's sibling tests above already do (criterion 1
    /// of TASK-1.84's own plan). The kernel-level wait
    /// (`wait_for_pane_status`) is what makes the one `Runtime::handle`
    /// call below deterministic: by the time it returns, the drain thread
    /// has already applied both marks under the same lock `dispatch`
    /// itself takes.
    #[test]
    fn a_typed_commands_status_reaches_pane_show_and_tab_show_through_runtime_handle() {
        let runtime = Runtime::new(
            std::sync::Arc::new(RealLauncher),
            login_for_marks(),
            TEST_GRACE,
        );
        let (session, tab_id, pane) = start_real_pane(
            &runtime,
            Program::Shell {
                cwd: "/tmp".to_string(),
            },
            vec![
                "sh",
                "-c",
                "printf '\\033]133;C\\007'; sleep 0.5; printf '\\033]133;D;3\\007'; sleep 100",
            ],
        );

        wait_for_pane_status(&runtime, pane, Some(Status::Running));
        let (pane_record, tab_record) = show_pane_and_tab(&runtime, session, tab_id, pane);
        assert_eq!(pane_record["status"], serde_json::json!("running"));
        assert_eq!(
            tab_record["shown_status"],
            serde_json::json!(pane.to_string())
        );

        wait_for_pane_status(&runtime, pane, Some(Status::Failed { exit_code: 3 }));
        let (pane_record, tab_record) = show_pane_and_tab(&runtime, session, tab_id, pane);
        assert_eq!(
            pane_record["status"],
            serde_json::json!({"failed": {"exit_code": 3}})
        );
        assert_eq!(
            tab_record["shown_status"],
            serde_json::json!(pane.to_string())
        );
    }

    /// AC #4's "a command pane's status from its exit", through
    /// `Runtime::handle` - `program_status` and `status` agree here (both
    /// move the instant the real program exits), unlike the typed-command
    /// case above, where only `status` is driven by a mark.
    #[test]
    fn a_command_panes_exit_reaches_pane_show_through_runtime_handle() {
        let runtime = Runtime::new(
            std::sync::Arc::new(RealLauncher),
            login_for_marks(),
            TEST_GRACE,
        );
        let (session, tab_id, pane) = start_real_pane(
            &runtime,
            Program::Command {
                argv: vec!["sh".to_string(), "-c".to_string(), "exit 7".to_string()],
                cwd: "/tmp".to_string(),
            },
            vec!["sh", "-c", "exit 7"],
        );

        wait_for_pane_status(&runtime, pane, Some(Status::Failed { exit_code: 7 }));

        let (pane_record, _tab_record) = show_pane_and_tab(&runtime, session, tab_id, pane);
        assert_eq!(
            pane_record["status"],
            serde_json::json!({"failed": {"exit_code": 7}})
        );
        assert_eq!(pane_record["program_status"], serde_json::json!("ended"));
        assert_eq!(pane_record["exit_code"], serde_json::json!(7));
    }
}

#[cfg(test)]
mod stop_tests {
    use std::sync::Arc;
    use std::time::Duration;

    use steg_sdk::{PaneId, Program};

    use super::Runtime;
    use crate::pane::PaneHandle;
    use crate::pty::{LaunchSpec, RealLauncher};

    /// How long these tests poll for a real program's own printed pid
    /// before giving up - bounded, the same reasoning every other
    /// real-process wait in this crate gives.
    const POLL_TIMEOUT: Duration = Duration::from_secs(10);

    /// A working `PATH` is required (TASK-1.87): see `mod tests`'s own
    /// `test_base_env` for why - duplicated here rather than shared
    /// across modules, matching this file's own existing style of each
    /// test module carrying its own small `login`-style helper.
    fn test_base_env() -> Vec<(std::ffi::OsString, std::ffi::OsString)> {
        vec![(
            std::ffi::OsString::from("PATH"),
            std::env::var_os("PATH").unwrap_or_default(),
        )]
    }

    /// A short, real `force_grace` for this module's own tests - unlike
    /// `tests::TEST_GRACE`, every test here actually exercises `force_end`
    /// through `Runtime::stop`, so this has to be long enough for a
    /// trapping program's test to prove the escalation really happens
    /// after the polite signal is given a real chance, not so long that
    /// the whole module's run time grows needlessly.
    const TEST_GRACE: Duration = Duration::from_millis(200);

    fn login() -> super::LoginShell {
        super::LoginShell {
            argv0: "/bin/sh".to_string(),
            home: "/tmp".to_string(),
            user: None,
            base_env: test_base_env(),
        }
    }

    /// Whether a process with this id still exists, reaped or not - the
    /// same `kill -0` probe `crate::pane`'s own tests already use.
    fn process_exists(pid: u32) -> bool {
        std::process::Command::new("kill")
            .args(["-0", &pid.to_string()])
            .status()
            .expect("kill(1) is on PATH on every platform this runs on")
            .success()
    }

    /// Starts a real pane whose own program prints its pid with `$$`
    /// then `exec`s into `then` - keeping the same pid, so the one
    /// number this reads back from the screen is the pid of whatever
    /// ends up actually running, with no new accessor needed on
    /// `PaneHandle` for a test to learn it. Returns the handle's own
    /// `PaneEvent` receiver too (review nit): `Exited` is a real event a
    /// caller that cares when the program ends should wait on instead of
    /// polling, unlike the pid file below, which has no event at all.
    fn start_pane_reporting_its_pid(
        runtime: &Runtime,
        then: &str,
    ) -> (
        PaneId,
        PaneHandle,
        u32,
        std::sync::mpsc::Receiver<crate::pane::PaneEvent>,
    ) {
        let pane = {
            let mut state = runtime.state.lock().unwrap();
            let session = state.tree.new_session(None).unwrap();
            let pane = state.tree.session(session).unwrap().tabs()[0].panes()[0].id();
            state
                .tree
                .start_program(
                    pane,
                    Program::Command {
                        argv: vec!["sh".to_string()],
                        cwd: "/tmp".to_string(),
                    },
                )
                .unwrap();
            pane
        };

        // The pid is written to its own file, not the screen: a program
        // like `yes` floods the screen fast enough to scroll a printed
        // pid line out of the visible area before a poll loop can ever
        // read it back, which is a race against the test's own reading,
        // not against the program - a file has no such race. There is
        // still no event for "a file now has this content" (review nit),
        // unlike the process's own exit below, so this one stays a
        // bounded poll.
        //
        // Named with a per-call counter, not just `pane`: every test in
        // this module builds its own fresh `Tree`, so `pane` alone is not
        // unique across tests - under this crate's default, parallel
        // `cargo test` (unlike the `--test-threads=1` these tests were
        // first proven stable under) two of them really did collide on
        // the same file (`just check`'s own recorded run caught this).
        static UNIQUE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let unique = UNIQUE.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let pid_file = std::env::temp_dir().join(format!(
            "steg-stop-test-{}-{pane}-{unique}.pid",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&pid_file);
        let (events_tx, events_rx) = std::sync::mpsc::channel();
        let handle = crate::pane::start(
            LaunchSpec {
                argv: vec![
                    "sh".to_string(),
                    "-c".to_string(),
                    format!("echo $$ > {} ; exec {then}", pid_file.display()),
                ],
                cwd: "/tmp".to_string(),
                env: test_base_env(),
                size: (80, 24),
            },
            pane,
            Arc::new(RealLauncher),
            events_tx,
        )
        .unwrap();

        let start = std::time::Instant::now();
        let pid = loop {
            if let Ok(text) = std::fs::read_to_string(&pid_file)
                && let Ok(pid) = text.trim().parse()
            {
                break pid;
            }
            assert!(
                start.elapsed() < POLL_TIMEOUT,
                "never saw a pid written to {}",
                pid_file.display()
            );
            std::thread::sleep(Duration::from_millis(10));
        };
        let _ = std::fs::remove_file(&pid_file);

        (pane, handle, pid, events_rx)
    }

    #[test]
    fn stop_ends_a_program_producing_output() {
        let mut runtime = Runtime::new(Arc::new(RealLauncher), login(), TEST_GRACE);
        let (pane, handle, pid, _events_rx) = start_pane_reporting_its_pid(&runtime, "yes");
        runtime.state.lock().unwrap().panes.insert(pane, handle);
        assert!(
            process_exists(pid),
            "the program must be running before stop"
        );

        runtime.stop();

        assert!(!process_exists(pid), "stop must end the program");
        assert!(runtime.state.lock().unwrap().panes.is_empty());
    }

    #[test]
    fn stop_ends_a_silent_program() {
        let mut runtime = Runtime::new(Arc::new(RealLauncher), login(), TEST_GRACE);
        let (pane, handle, pid, _events_rx) = start_pane_reporting_its_pid(&runtime, "sleep 1000");
        runtime.state.lock().unwrap().panes.insert(pane, handle);
        assert!(
            process_exists(pid),
            "the program must be running before stop"
        );

        runtime.stop();

        assert!(!process_exists(pid), "stop must end the program too");
        assert!(runtime.state.lock().unwrap().panes.is_empty());
    }

    #[test]
    fn stop_is_a_no_op_for_a_pane_whose_program_has_already_ended() {
        let mut runtime = Runtime::new(Arc::new(RealLauncher), login(), TEST_GRACE);
        // `true` exits at once, no `exec sleep` keeping it alive - by the
        // time this pid is known at all (printed, then read back), the
        // process behind it is already gone or going; this test only
        // needs that to have fully happened, confirmed the same way the
        // other `stop` tests confirm a process ended.
        let (pane, handle, pid, events_rx) = start_pane_reporting_its_pid(&runtime, "true");
        // An event wait first, not a bare sleep-poll (review nit):
        // `Exited` is a real event here (unlike the pid file above,
        // which has none), so this waits on it rather than polling
        // `process_exists` from the start. It is not the *whole*
        // answer, though: `Exited` only means our own wait() on the
        // child has returned, not that the OS has finished reaping it
        // (observed empirically - `process_exists` can still report
        // `true` for a moment right after `Exited` fires) - there is no
        // second event for "the process table caught up", so the short
        // remaining gap is still a bounded poll, now with far less of it
        // to cover than polling from the start would.
        events_rx
            .recv_timeout(POLL_TIMEOUT)
            .expect("the real `true` should have exited within the bound");
        let start = std::time::Instant::now();
        while process_exists(pid) {
            assert!(
                start.elapsed() < POLL_TIMEOUT,
                "the process was never actually reaped after its own Exited event"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
        runtime.state.lock().unwrap().panes.insert(pane, handle);

        runtime.stop(); // must not hang or panic on an already-ended program

        assert!(runtime.state.lock().unwrap().panes.is_empty());
    }

    #[test]
    fn stop_with_no_panes_at_all_returns_at_once() {
        let mut runtime = Runtime::new(Arc::new(RealLauncher), login(), TEST_GRACE);
        runtime.stop();
        assert!(runtime.state.lock().unwrap().panes.is_empty());
    }

    /// TASK-1.78 §4's own real behaviour change: the old `PaneHandle::
    /// stop` sent only an unescalated `SIGHUP` by pid, so a program
    /// trapping it survived `Runtime::stop`/`Drop` indefinitely (confirmed
    /// by this task's own reviewer, in a scratch test against the
    /// previous mechanism). Reusing `force_end` fixes this as a direct
    /// consequence, not a special case - proven here rather than only
    /// argued.
    #[test]
    fn stop_ends_a_program_that_traps_the_polite_signal() {
        let mut runtime = Runtime::new(Arc::new(RealLauncher), login(), TEST_GRACE);

        // Waits for a real marker the nested shell writes *after*
        // installing its own trap, before calling `stop` - found
        // necessary by probing, not assumed: without it, `stop` can race
        // the newly forked shell's own first few instructions and, under
        // heavy CPU contention, send its signal before the shell has
        // reached its own `trap` builtin at all (the same race `pane::
        // ending_tests`'s own trapping test found and fixed the same
        // way). `start_pane_reporting_its_pid`'s own `exec {then}` means
        // `then` here is a nested `sh -c` so the trap can be installed
        // *inside* the process that then execs into `sleep`, not applied
        // to `trap` itself (which has no external binary to exec).
        let ready = std::env::temp_dir().join(format!(
            "steg-stop-trap-ready-{}-{}.marker",
            std::process::id(),
            line!()
        ));
        let _ = std::fs::remove_file(&ready);
        let (pane, handle, pid, _events_rx) = start_pane_reporting_its_pid(
            &runtime,
            &format!(
                "sh -c \"trap '' HUP; touch {}; exec sleep 1000\"",
                ready.display()
            ),
        );
        runtime.state.lock().unwrap().panes.insert(pane, handle);
        assert!(
            process_exists(pid),
            "the program must be running before stop"
        );
        let start = std::time::Instant::now();
        while !ready.exists() {
            assert!(
                start.elapsed() < POLL_TIMEOUT,
                "the nested shell never reached its own trap+marker"
            );
            std::thread::sleep(Duration::from_millis(5));
        }
        let _ = std::fs::remove_file(&ready);

        runtime.stop();

        assert!(
            !process_exists(pid),
            "stop must still end a program that traps the polite signal, \
             by escalating - the old mechanism could not"
        );
        assert!(runtime.state.lock().unwrap().panes.is_empty());
    }
}
