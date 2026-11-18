//! Test-only helpers shared across more than one seam's own test module
//! (TASK-1.131): every one of these is called from `Runtime::new` call
//! sites in at least two sibling files, which is what earns a shared,
//! `pub(super)` home here rather than a private copy in just one of
//! them. `mod stop_tests` (in `lifecycle.rs`) keeps its own, separately
//! -defined `login`/`test_base_env`/`TEST_GRACE`/`TEST_TICK` rather than
//! sharing these - that duplication already existed before this task and
//! is this crate's own existing style, not something this move changes.
//!
//! Not named `test_support`, so it is never confused with the crate-root
//! `crate::test_support` (a different, narrower helper: `run_bounded`).

use std::collections::HashMap;
use std::time::{Duration, Instant};

use steg_sdk::Program;

use super::{LoginShell, State};
use crate::pane::PaneHandle;

/// A working `PATH` is required (TASK-1.87): `RealLauncher::spawn`'s
/// own `.env_clear()` means a `LaunchSpec.env` with no `PATH` at all
/// cannot resolve a relative program name like `"sh"` (confirmed
/// directly with `portable_pty`, not assumed - `spawn_command` fails
/// with "Unable to resolve the PATH"). Reading this *test* process's
/// own ambient `PATH` is fine here - a test, allowed to, unlike
/// production code (this function's own doc on `ServerConfig`'s
/// `login_env` module says why production code never does).
pub(super) fn test_base_env() -> Vec<(std::ffi::OsString, std::ffi::OsString)> {
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
pub(super) const TEST_GRACE: Duration = Duration::from_millis(30);

/// A tiny `attach_tick` for every `Runtime::new` in this module - none
/// of these tests attach to a pane (TASK-1.47's own tests live in
/// `pane.rs` and this crate's `tests/` directory), so the value only
/// has to be cheap, never realistic.
pub(super) const TEST_TICK: Duration = Duration::from_millis(1);

pub(super) fn login() -> LoginShell {
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
pub(super) fn test_state(
    tree: steg_kernel::Tree,
    panes: HashMap<steg_sdk::PaneId, PaneHandle>,
) -> State {
    State {
        tree,
        panes,
        settings: steg_kernel::Settings::new(),
        clock_origin: Instant::now(),
        subscribers: HashMap::new(),
        notify_session_count_changed: None,
        stopper: None,
        applied_marks: HashMap::new(),
    }
}

pub(super) fn fresh_state_with_one_pane() -> (State, steg_sdk::PaneId) {
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
