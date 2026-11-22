//! Starting panes (TASK-1.131 seam: "starting panes" -
//! `session.new`/`tab.new`/`pane.split`, the three methods that start a
//! real program).

use std::sync::Arc;
use std::time::Instant;

use steg_kernel::{Moment, Tree};
use steg_sdk::{
    ApiError, Description, ErrorCode, PaneId, Program, Request, Response, SessionId, TabId,
    validate,
};

use super::tree::session_owning_tab;
use super::{State, broadcast, reapply_every_sized_tab};
use crate::dispatch::{self, Context};
use crate::env::pane_environment;
use crate::pane::{self, DEFAULT_PANE_SIZE};
use crate::program_param::program_from_request;
use crate::pty::LaunchSpec;

impl super::Runtime {
    /// `session.new`/`tab.new`/`pane.split`: mints through the existing,
    /// unchanged `dispatch` arm, then starts the real program the plan's
    /// §2 describes, under the same lock the whole way through (§1's own
    /// justification for why that lock, not a narrower one, is what keeps
    /// "nothing else runs between minting and starting" true with no real
    /// control thread yet).
    pub(super) fn start_and_dispatch(
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

        // Looked up here, under the same lock that minted `new_pane` just
        // above (TASK-1.146) - this pane's real identifiers, handed to
        // `launch_spec` below in place of the `SessionId`/`TabId`/`PaneId`
        // placeholders it used to invent. `tab_id` was already computed,
        // inline, only for the size lookup; `session_owning_tab` is
        // already imported and used later in this same file
        // (`emit_creation_events`) - both lifted into bindings here, no
        // new lookup logic.
        let tab_id = state
            .tree
            .tab_of_pane(new_pane)
            .expect("just started this pane's program; its tab still holds it");
        let session_id = session_owning_tab(&state.tree, tab_id);

        let size = state
            .tree
            .tab_sizes_now(tab_id)
            .and_then(|change| {
                change
                    .panes
                    .iter()
                    .find(|(pane, _)| *pane == new_pane)
                    .map(|&(_, size)| (size.columns(), size.rows()))
            })
            .unwrap_or(DEFAULT_PANE_SIZE);

        // This server's own real bound address (TASK-1.146) - `""` for a
        // `Runtime` nothing has called `bind_server_address` on (every
        // existing test `Runtime::new` site), which `steg_sdk::paths::
        // socket_path` already treats as absent (TASK-1.147), never as a
        // real override.
        let server_address = state.server_address.clone().unwrap_or_default();

        let spec = self.launch_spec(
            &program,
            size,
            &server_address,
            session_id,
            tab_id,
            new_pane,
        );

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
    /// only ever sees the *shape*) and the pane's real environment.
    /// `server`/`session`/`tab`/`pane` are this pane's real identifiers
    /// and this server's real bound address (TASK-1.146) - computed by
    /// this function's one caller, `start_and_dispatch`, under the same
    /// lock that minted the pane, and handed in as plain data rather than
    /// read here. **This function must never call `self.lock()`**: its
    /// caller already holds that guard for the whole surrounding call, so
    /// a second acquisition here would self-deadlock. `bin_dir` (this
    /// process's own binary location, for `STEG_BIN_DIR` and `PATH`) is
    /// still read directly - nothing mints that value under any lock.
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
    fn launch_spec(
        &self,
        program: &Program,
        size: (u16, u16),
        server: &str,
        session: SessionId,
        tab: TabId,
        pane: PaneId,
    ) -> LaunchSpec {
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
        let overrides = pane_environment(server, session, tab, pane, &bin_dir, current_path);

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

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::super::testing::{TEST_GRACE, TEST_TICK, login};
    use super::super::{LoginShell, Runtime};
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
