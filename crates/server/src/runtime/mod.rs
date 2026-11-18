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

use steg_kernel::{Moment, Settings, Tree};
use steg_sdk::{
    ApiError, ClientId, Description, ErrorCode, PaneId, Program, Request, Response, SessionId,
    validate,
};

use crate::connection::ConnectionEvent;
use crate::dispatch::{self, Context, Filter, Outcome};
use crate::env::pane_environment;
use crate::outbound::Outbound;
use crate::pane::{self, DEFAULT_PANE_SIZE, PaneEvent, PaneHandle};
use crate::program_param::program_from_request;
use crate::pty::{LaunchSpec, ProgramLauncher};

mod attach;
mod events;
mod input;
mod lifecycle;
mod login;
#[cfg(test)]
mod testing;
mod tree;

use events::{apply_event_locked, broadcast, emit_focus_event};
use input::{apply_size_changed_locked, reapply_every_sized_tab};
use tree::session_owning_tab;

pub(crate) use attach::Attaching;
pub use login::{LoginShell, build_login_shell};

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
    // TASK-1.47: how often an attached pane's worker thread computes and
    // fans out a screen update - threaded to every `pane::start` call
    // below, the same injected-not-read shape `force_grace` already uses.
    attach_tick: Duration,
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
        attach_tick: Duration,
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
            attach_tick,
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
            self.attach_tick,
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
    // Pass 0 (TASK-1.128): tells every pane's own worker it is being
    // closed, *before* anything below signals its program - over the
    // command queue (`PaneHandle::mark_closing`, not `RunGate`), so the
    // worker records `EndReason::Closed` ahead of the `Exited` the signal
    // below is about to cause, never behind it. This ordering holds
    // because this loop is a plain, synchronous send per handle that
    // fully returns before pass 1 even starts: the relay thread that
    // would send that `Exited` cannot observe a death the signal below
    // has not caused yet, and that signal is not sent until pass 1 runs.
    // A future change that parallelises this pass with pass 1 (the way
    // passes 2-6 already use `thread::scope`, for throughput) would
    // silently reopen the bug this pass exists to fix - keep it a plain,
    // sequential loop, strictly before pass 1's own loop starts.
    //
    // Covers every caller of `force_end`, both `close_and_dispatch`
    // (forced and plain closes alike - every popped handle lands here
    // regardless of whether its program was running) and `Runtime::stop`
    // (server stop) - one new pass fixes the reason for both, with
    // nothing to change at either call site.
    for (_, handle) in &handles {
        handle.mark_closing();
    }

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

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::testing::{TEST_GRACE, TEST_TICK, login};
    use super::{LoginShell, Runtime};
    use crate::pty::RealLauncher;

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
        let runtime = Runtime::new(
            std::sync::Arc::new(RealLauncher::default()),
            login,
            TEST_GRACE,
            TEST_TICK,
        );
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
        let runtime = Runtime::new(
            std::sync::Arc::new(RealLauncher::default()),
            login(),
            TEST_GRACE,
            TEST_TICK,
        );
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
        let runtime = Runtime::new(
            std::sync::Arc::new(RealLauncher::default()),
            login(),
            TEST_GRACE,
            TEST_TICK,
        );

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
        let runtime = Runtime::new(
            std::sync::Arc::new(RealLauncher::default()),
            login(),
            TEST_GRACE,
            TEST_TICK,
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
        let runtime = Runtime::new(
            std::sync::Arc::new(RealLauncher::default()),
            login(),
            TEST_GRACE,
            TEST_TICK,
        );
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

        let runtime = Runtime::new(
            std::sync::Arc::new(RealLauncher::default()),
            login(),
            TEST_GRACE,
            TEST_TICK,
        );
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

        let runtime = Runtime::new(
            std::sync::Arc::new(RealLauncher::default()),
            login(),
            TEST_GRACE,
            TEST_TICK,
        );
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
}
