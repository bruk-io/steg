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
use std::time::Instant;

use steg_kernel::{ExitOutcome, Moment, Settings, SizeChanged, Tree};
use steg_sdk::{ApiError, Description, ErrorCode, PaneId, Program, Request, Response, validate};

use crate::dispatch::{self, Context};
use crate::env::pane_environment;
use crate::pane::{self, DEFAULT_PANE_SIZE, PaneEvent, PaneHandle};
use crate::program_param::program_from_request;
use crate::pty::{LaunchSpec, ProgramLauncher};

/// The login shell's resolved program and home directory - the one value
/// [`Runtime`] is given rather than reading `SHELL`/`HOME` from the
/// process environment itself in several places (TASK-1.76 §2).
/// [`LoginShell::from_process_env`] is a named stand-in for TASK-1.77's
/// own draft (start on demand, detach, the login shell resolved from the
/// password database): when that lands, its own resolution replaces this
/// one's call site, and nothing downstream of this struct changes.
#[derive(Debug, Clone)]
pub struct LoginShell {
    /// What [`steg_sdk::Program::Shell`] resolves to when a pane actually
    /// starts, e.g. `/bin/zsh`.
    pub argv0: String,
    /// The default working directory when nothing else applies: no `cwd`
    /// was given, and (rule B, §2) there is no pane to inherit one from,
    /// or it has not reported one yet.
    pub home: String,
}

impl LoginShell {
    /// Reads `SHELL`/`HOME` from this process's own environment, falling
    /// back to `/bin/zsh`/`/` if either is unset.
    #[must_use]
    pub fn from_process_env() -> Self {
        Self {
            argv0: std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".to_string()),
            home: std::env::var("HOME").unwrap_or_else(|_| "/".to_string()),
        }
    }
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
}

impl Runtime {
    /// Builds an empty `Runtime` (no sessions yet) and starts its one
    /// drain thread, which applies every [`PaneEvent`] a pane's real
    /// program reports - most importantly, a natural exit - to the tree
    /// (§1).
    ///
    /// # Panics
    ///
    /// Never in practice: the only failure `thread::Builder::spawn` has
    /// is the OS refusing a new thread (out of resources), which this
    /// treats as fatal to the whole server rather than returning a
    /// `Runtime` that silently never drains any event.
    #[must_use]
    pub fn new(launcher: Arc<dyn ProgramLauncher>, login: LoginShell) -> Self {
        let (events_tx, events_rx) = mpsc::channel();
        let state = Arc::new(Mutex::new(State {
            tree: Tree::new(),
            panes: HashMap::new(),
            settings: Settings::new(),
            clock_origin: Instant::now(),
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
                let response = dispatch::dispatch(
                    &mut state.tree,
                    &mut state.settings,
                    description,
                    context,
                    &resolved,
                );
                if matches!(response, Response::Ok(_)) {
                    reapply_every_sized_tab(state);
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

    /// Stops this `Runtime`: ends every running program (`PaneHandle::
    /// stop`'s own `SIGHUP` by pid, no escalation - see its own doc for
    /// the line this draws against TASK-1.78), reaps each one, shuts its
    /// worker thread down and joins it, then joins the one drain thread
    /// too (review finding 3 - this is the server stopping, not `--force`
    /// on one pane, which stays untouched by `--force` itself, R94's own
    /// stated gap, until TASK-1.78). TASK-1.78 can reuse `PaneHandle::
    /// stop`'s own mechanism (`clone_killer`, taken before `child` moves
    /// into the relay thread) directly for a single pane; what it still
    /// has to add on top is the grace period before escalating and what a
    /// client is told, neither of which this method needs.
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
            std::mem::take(&mut state.panes)
        };
        for (_, handle) in panes {
            handle.stop();
        }

        // Dropping every clone of `events` (this one, the last) is what
        // lets the drain thread's own `for event in events_rx` end -
        // every pane's own clone already went with its worker thread,
        // stopped just above.
        drop(self.events.take());
        if let Some(drain) = self.drain.take() {
            let _ = crate::pane::join_with_bound(drain, crate::pane::STOP_TIMEOUT);
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
            dispatch::dispatch(
                &mut state.tree,
                &mut state.settings,
                description,
                context,
                request,
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
                Response::Ok(value)
            }
            Err(error) => {
                rollback_mint(&mut state.tree, request.method.as_str(), new_pane);
                Response::Error(ApiError::new(ErrorCode::ProgramFailed, error.to_string()))
            }
        }
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
    fn launch_spec(&self, program: &Program, size: (u16, u16)) -> LaunchSpec {
        let argv = match program {
            Program::Shell { .. } => vec![self.login.argv0.clone()],
            Program::Command { argv, .. } => argv.clone(),
        };
        let bin_dir = std::env::current_exe()
            .ok()
            .and_then(|exe| exe.parent().map(std::path::Path::to_path_buf))
            .unwrap_or_else(|| std::path::PathBuf::from("/usr/local/bin"));
        let env = pane_environment(
            "",
            steg_sdk::SessionId::from_raw(0),
            steg_sdk::TabId::from_raw(0),
            steg_sdk::PaneId::from_raw(0),
            &bin_dir,
            std::env::var_os("PATH").as_deref(),
        );
        LaunchSpec {
            argv,
            cwd: program.cwd().to_string(),
            env,
            size,
        }
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
            match state.tree.record_exit(pane, exit_code, now) {
                Ok(ExitOutcome::Closed(closed)) => {
                    state.panes.remove(&closed.pane);
                }
                Ok(ExitOutcome::Stayed) => {}
                Err(steg_kernel::Error::PaneNotFound(_)) => {
                    // The orphan a forced close leaves (§5, until the task
                    // that ends a running program on `--force`): the tree no
                    // longer has this pane at all, so there is nothing left
                    // to record an exit on - the one thing left to do is stop
                    // leaking the now-pointless `PaneHandle`.
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

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::mpsc;
    use std::time::{Duration, Instant};

    use steg_sdk::{Program, ProgramStatus, Size};

    use super::{LoginShell, Runtime, State, apply_event_locked};
    use crate::pane::{PaneEvent, PaneHandle};
    use crate::pty::RealLauncher;

    /// Bounded, never a bare `recv` - the no-sleep rule's own reasoning
    /// for every real-process wait in this crate's tests.
    const EVENT_TIMEOUT: Duration = Duration::from_secs(20);

    fn login() -> LoginShell {
        LoginShell {
            argv0: "/bin/sh".to_string(),
            home: "/tmp".to_string(),
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
                env: Vec::new(),
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
                env: Vec::new(),
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
                env: Vec::new(),
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

    #[test]
    fn login_shell_from_process_env_never_panics_and_is_never_empty() {
        // A smoke test, not a claim about this machine's own `SHELL`/
        // `HOME` - those are read once, by whichever binary builds the
        // real `Runtime`, and this only proves the fallback path exists
        // and is well-formed.
        let login = LoginShell::from_process_env();
        assert!(!login.argv0.is_empty());
        assert!(!login.home.is_empty());
    }

    #[test]
    fn runtime_new_does_not_panic_and_starts_empty() {
        let runtime = Runtime::new(std::sync::Arc::new(RealLauncher), login());
        let description = steg_sdk::description();
        let context = crate::dispatch::Context {
            default_session: steg_sdk::SessionId::from_raw(0),
            default_tab: steg_sdk::TabId::from_raw(0),
            default_pane: steg_sdk::PaneId::from_raw(0),
        };
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
        use steg_sdk::{Request, SessionId, TabId};

        let runtime = Runtime::new(std::sync::Arc::new(RealLauncher), login());
        let description = steg_sdk::description();
        let no_context = crate::dispatch::Context {
            default_session: SessionId::from_raw(0),
            default_tab: TabId::from_raw(0),
            default_pane: steg_sdk::PaneId::from_raw(0),
        };

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
            default_session: session_id,
            default_tab: tab_id,
            default_pane: source_pane,
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
        use steg_sdk::{Request, SessionId, TabId};

        let runtime = Runtime::new(std::sync::Arc::new(RealLauncher), login());
        let description = steg_sdk::description();
        let no_context = crate::dispatch::Context {
            default_session: SessionId::from_raw(0),
            default_tab: TabId::from_raw(0),
            default_pane: steg_sdk::PaneId::from_raw(0),
        };

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
            default_session: session_id,
            default_tab: tab_id,
            default_pane: source_pane,
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

        let runtime = Runtime::new(std::sync::Arc::new(RealLauncher), login());
        let description = steg_sdk::description();
        let no_context = crate::dispatch::Context {
            default_session: SessionId::from_raw(0),
            default_tab: TabId::from_raw(0),
            default_pane: steg_sdk::PaneId::from_raw(0),
        };

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
            default_session: session_id,
            default_tab: tab_id,
            default_pane: first_pane,
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
                default_session: session_id,
                default_tab: tab_id,
                default_pane: first_pane,
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
                env: Vec::new(),
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

    fn login() -> super::LoginShell {
        super::LoginShell {
            argv0: "/bin/sh".to_string(),
            home: "/tmp".to_string(),
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
                env: Vec::new(),
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
        let mut runtime = Runtime::new(Arc::new(RealLauncher), login());
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
        let mut runtime = Runtime::new(Arc::new(RealLauncher), login());
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
        let mut runtime = Runtime::new(Arc::new(RealLauncher), login());
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
        let mut runtime = Runtime::new(Arc::new(RealLauncher), login());
        runtime.stop();
        assert!(runtime.state.lock().unwrap().panes.is_empty());
    }
}
