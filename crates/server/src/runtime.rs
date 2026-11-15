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

use steg_kernel::{ExitOutcome, Settings, SizeChanged, Tree};
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
}

/// Owns the kernel's tree and every pane's real runtime, and is the one
/// place a connection's request reaches both. Construct one per server
/// process (TASK-1.44/1.45 will do this once a socket exists; today only
/// this crate's own tests construct one).
pub struct Runtime {
    state: Arc<Mutex<State>>,
    launcher: Arc<dyn ProgramLauncher>,
    login: LoginShell,
    events: Sender<PaneEvent>,
    _drain: JoinHandle<()>,
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
            events: events_tx,
            _drain: drain,
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
        match request.method.as_str() {
            "session.new" | "tab.new" | "pane.split" => {
                self.start_and_dispatch(description, context, request)
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
                    request,
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
            self.events.clone(),
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
    /// takes the fallback. Never itself a source of an error: whatever
    /// goes wrong here, `dispatch`'s own call right after surfaces the
    /// real refusal (a bad target, say) on its own.
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
        PaneEvent::Exited { pane, exit_code } => match state.tree.record_exit(pane, exit_code) {
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
        },
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
    use std::sync::mpsc;
    use std::time::Duration;

    use steg_sdk::{Program, ProgramStatus, Size};

    use super::{LoginShell, Runtime, State, apply_event_locked};
    use crate::pane::PaneEvent;
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
        (
            State {
                tree,
                panes: std::collections::HashMap::new(),
                settings: steg_kernel::Settings::new(),
            },
            pane,
        )
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
        assert_eq!(found.status(), Some(ProgramStatus::Ended { exit_code: 3 }));
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
        let mut state = State {
            tree,
            panes: std::collections::HashMap::new(),
            settings: steg_kernel::Settings::new(),
        };

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
        let mut state = State {
            tree,
            panes: std::collections::HashMap::from([(gone, handle)]),
            settings: steg_kernel::Settings::new(),
        };

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

        let mut state = State {
            tree,
            panes: std::collections::HashMap::from([(pane, handle)]),
            settings: steg_kernel::Settings::new(),
        };
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

        let session = match runtime.handle(
            &description,
            &no_context,
            &Request {
                method: "session.new".to_string(),
                params: serde_json::json!({
                    "cwd": "/private/tmp",
                    "program": ["sh", "-c",
                        "printf '\\033]7;file:///private/tmp/project\\007'; sleep 100"],
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

        // Bounded, never a bare wait: `PaneHandle::cwd` is itself bounded
        // (`CWD_LOOKUP_TIMEOUT`), so this polls it, not an unbounded
        // channel - there is no event to block on for "a byte sequence
        // was written", only a value to re-read.
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
            if live.as_deref() == Some("/private/tmp/project") {
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
            Some("/private/tmp/project".to_string()),
            "the new pane must inherit the live cwd (set by OSC 7), not the \
             source pane's own started-in cwd (/private/tmp)"
        );
    }

    /// AC #3's full claim: a change to a tab's size reaches every
    /// terminal in it, on both halves (the kernel `Terminal` and the real
    /// pty), through the general step (§4) - not only at the moment a
    /// pane starts. `declare_size`/`connect_client`/`view_pane` are
    /// already public on `Tree` (TASK-1.42) and this is the seam the
    /// tasks that wire `client.*` dispatch will use the same way; no
    /// test-only hook is added anywhere for this.
    #[test]
    fn a_tabs_declared_size_reaches_every_pane_including_one_split_in_afterward() {
        use steg_sdk::{ClientKind, Request, SessionId, Size, TabId};

        let runtime = Runtime::new(std::sync::Arc::new(RealLauncher), login());
        let description = steg_sdk::description();
        let no_context = crate::dispatch::Context {
            default_session: SessionId::from_raw(0),
            default_tab: TabId::from_raw(0),
            default_pane: steg_sdk::PaneId::from_raw(0),
        };

        // A shell that re-reports its pty's own size on every `SIGWINCH`
        // - the only way to observe the *pty* side of a resize on an
        // *already-running* pane from outside, through what the engine
        // actually drives into the terminal's screen.
        let first = match runtime.handle(
            &description,
            &no_context,
            &Request {
                method: "session.new".to_string(),
                params: serde_json::json!({
                    "program": ["sh", "-c",
                        "trap 'stty size' WINCH; while :; do sleep 0.02; done"],
                })
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

        // Declare a size directly on the tree - the only way to get a
        // tab sized at all today (no `client` dispatch method exists) -
        // and apply it through the *public* `apply_size_changed`,
        // proving that entry point itself, not only `handle`'s internal
        // use of it.
        let change = {
            let mut state = runtime.state.lock().unwrap();
            let client = state.tree.connect_client(ClientKind::App, None);
            state.tree.view_pane(client, first_pane).unwrap();
            state
                .tree
                .declare_size(client, tab_id, Size::new(50, 15))
                .unwrap()
        };
        runtime.apply_size_changed(&change);

        let screen = {
            let state = runtime.state.lock().unwrap();
            state.panes[&first_pane].snapshot().unwrap()
        };
        assert_eq!(screen.rows.len(), 15, "the terminal side of AC #3");

        // The pty side: bounded polling on the shell's own real
        // `SIGWINCH`-triggered re-report. The *first* `apply_size_changed`
        // above raced the shell's own `trap` registration (a real, timing-
        // dependent race, not assumed away) - and once a resize is a
        // genuine no-op (the same size as before), the OS does not raise
        // a second `SIGWINCH` for it, so simply repeating that same call
        // can never recover from losing the race. Alternating the row
        // count by one each attempt makes every single call a real size
        // change, so every attempt gets its own real signal, until one of
        // them lands after the trap is in place; the loop then settles on
        // the actually-declared size this test is about before moving on.
        let start = std::time::Instant::now();
        let mut toggle = false;
        loop {
            toggle = !toggle;
            let probe_rows = if toggle { 15 } else { 16 };
            let probe = steg_kernel::SizeChanged {
                panes: vec![(first_pane, Size::new(50, probe_rows))],
                ..change.clone()
            };
            runtime.apply_size_changed(&probe);
            let text = {
                let state = runtime.state.lock().unwrap();
                let screen = state.panes[&first_pane].snapshot().unwrap();
                screen
                    .rows
                    .iter()
                    .map(|row| row.iter().map(|run| run.text.as_str()).collect::<String>())
                    .collect::<Vec<_>>()
                    .join("\n")
            };
            if text.contains("15 50") || text.contains("16 50") {
                break;
            }
            assert!(
                start.elapsed() < Duration::from_secs(20),
                "the pty's own real winsize was never reported; last screen \
                 text: {text:?}"
            );
            std::thread::sleep(Duration::from_millis(20));
        }
        // Settle back on the size this test is actually about, so the
        // assertions after the split below compare against it.
        runtime.apply_size_changed(&change);
        let screen = {
            let state = runtime.state.lock().unwrap();
            state.panes[&first_pane].snapshot().unwrap()
        };
        assert_eq!(screen.rows.len(), 15);

        // Now split, through the public API, with no size given: the
        // general step (§4) must reapply the tab's declared size to
        // *both* panes, not only decide the new one's initial size.
        let context = crate::dispatch::Context {
            default_session: session_id,
            default_tab: tab_id,
            default_pane: first_pane,
        };
        let split = match runtime.handle(
            &description,
            &context,
            &Request {
                method: "pane.split".to_string(),
                params:
                    serde_json::json!({"direction": "right", "program": ["sh", "-c", "sleep 100"]})
                        .as_object()
                        .unwrap()
                        .clone(),
            },
        ) {
            steg_sdk::Response::Ok(value) => value,
            steg_sdk::Response::Error(error) => panic!("pane.split failed: {error:?}"),
        };
        let new_pane: steg_sdk::PaneId = split["id"].as_str().unwrap().parse().unwrap();

        let (first_screen, new_screen, expected) = {
            let state = runtime.state.lock().unwrap();
            let expected = state.tree.tab_sizes_now(tab_id).unwrap();
            (
                state.panes[&first_pane].snapshot().unwrap(),
                state.panes[&new_pane].snapshot().unwrap(),
                expected,
            )
        };
        let expected_rows = |pane: steg_sdk::PaneId| {
            expected
                .panes
                .iter()
                .find(|&&(id, _)| id == pane)
                .map(|&(_, size)| usize::from(size.rows()))
                .unwrap()
        };
        assert_eq!(
            first_screen.rows.len(),
            expected_rows(first_pane),
            "the split must have shrunk the original pane's share, and the \
             general step must have reapplied that to its terminal"
        );
        assert_eq!(
            new_screen.rows.len(),
            expected_rows(new_pane),
            "the new pane's own initial size must already match the tab's \
             declared size"
        );
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
