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
use steg_sdk::{ClientId, Description, PaneId, Request, Response};

use crate::connection::ConnectionEvent;
use crate::dispatch::{self, Context, Filter, Outcome};
use crate::outbound::Outbound;
use crate::pane::{PaneEvent, PaneHandle};
use crate::pty::ProgramLauncher;

mod attach;
mod close;
mod events;
mod input;
mod lifecycle;
mod login;
mod start;
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
}

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

#[cfg(test)]
mod tests {
    use super::Runtime;
    use super::testing::{TEST_GRACE, TEST_TICK, login};
    use crate::pty::RealLauncher;

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
}
