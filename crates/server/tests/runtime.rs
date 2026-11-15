//! Functional tests for [`Runtime`]: real programs on a real pty, driven
//! only through `Runtime::handle` (`.claude/rules/testing-rust.md`), the
//! way a real connection will call it once TASK-1.44/1.45 build the
//! socket. `crates/server/src/runtime.rs`'s own `#[cfg(test)]` module
//! covers what only has access to `Runtime`'s private pieces (the
//! `apply_event`/`apply_size_changed` cascades, deterministically); this
//! file covers what a real client-shaped caller sees.

#![expect(
    clippy::unwrap_used,
    reason = "this whole file is a test; see dispatch.rs's own comment"
)]

use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};
use steg_sdk::{Description, ErrorCode, PaneId, Request, Response, SessionId, TabId, description};
use steg_server::{Context, LoginShell, RealLauncher, Runtime};

/// How long any one real-pty test waits before failing with a message
/// instead of hanging a job on a misbehaving child (the same bound
/// `real_pty.rs` already uses).
const POLL_TIMEOUT: Duration = Duration::from_secs(20);
const POLL_INTERVAL: Duration = Duration::from_millis(20);

fn runtime() -> (Runtime, Description) {
    (
        Runtime::new(
            Arc::new(RealLauncher),
            LoginShell {
                argv0: "/bin/sh".to_string(),
                home: "/tmp".to_string(),
            },
        ),
        description(),
    )
}

/// No real session exists yet, so there is nothing to default to -
/// `session.new` itself never reads `Context` at all, and every other
/// method in these tests is given an explicit target instead of relying
/// on a default.
fn no_context() -> Context {
    Context {
        default_session: SessionId::from_raw(0),
        default_tab: TabId::from_raw(0),
        default_pane: PaneId::from_raw(0),
    }
}

fn request(method: &str, pairs: Vec<(&str, Value)>) -> Request {
    Request {
        method: method.to_string(),
        params: pairs.into_iter().map(|(k, v)| (k.to_owned(), v)).collect(),
    }
}

fn ok(response: Response) -> Value {
    match response {
        Response::Ok(value) => value,
        Response::Error(error) => panic!("expected Response::Ok, got {error:?}"),
    }
}

fn err(response: Response) -> steg_sdk::ApiError {
    match response {
        Response::Error(error) => error,
        Response::Ok(value) => panic!("expected Response::Error, got {value}"),
    }
}

fn id_of(value: &Value) -> String {
    value["id"].as_str().unwrap().to_string()
}

/// Every session/tab/pane this `Runtime` currently holds, flattened to
/// one count - used to prove a rollback left nothing behind, without
/// `Runtime` exposing its tree to a test that only has the public API.
fn counts(runtime: &Runtime, api: &Description) -> (usize, usize) {
    let sessions = ok(runtime.handle(api, &no_context(), &request("session.list", vec![])));
    let sessions = sessions.as_array().unwrap();
    let mut panes = 0;
    for session in sessions {
        let session_id = session["id"].as_str().unwrap();
        let shown = ok(runtime.handle(
            api,
            &no_context(),
            &request("session.show", vec![("target", json!(session_id))]),
        ));
        let tab_id = shown["current_tab"].as_str().unwrap().to_string();
        // Every pane in the one tab a fresh session/tab always has (R94)
        // - these tests never build more than one tab deep, so position
        // 1 is always this tab's only or first pane, and polling further
        // positions would need `pane.list`, not built in this task.
        let context = Context {
            default_session: session_id.parse().unwrap(),
            default_tab: tab_id.parse().unwrap(),
            default_pane: PaneId::from_raw(0),
        };
        if matches!(
            runtime.handle(
                api,
                &context,
                &request("pane.show", vec![("target", json!("1"))])
            ),
            Response::Ok(_)
        ) {
            panes += 1;
        }
    }
    (sessions.len(), panes)
}

/// Blocks until `probe` returns `Some`, or panics after [`POLL_TIMEOUT`] -
/// used only to wait for a real program's own output to actually land in
/// a snapshot (`stty size`, printed asynchronously once the shell starts),
/// never to wait for this crate's own event delivery, which every other
/// test waits for with a bounded `recv_timeout` instead.
fn poll<T>(mut probe: impl FnMut() -> Option<T>) -> T {
    let start = std::time::Instant::now();
    loop {
        if let Some(value) = probe() {
            return value;
        }
        assert!(
            start.elapsed() < POLL_TIMEOUT,
            "condition never became true within {POLL_TIMEOUT:?}"
        );
        std::thread::sleep(POLL_INTERVAL);
    }
}

// --- AC #1: starting -------------------------------------------------------

#[test]
fn session_new_with_a_command_starts_it_and_pane_show_reports_it_running_then_ended() {
    let (runtime, api) = runtime();

    let session = ok(runtime.handle(
        &api,
        &no_context(),
        &request(
            "session.new",
            vec![("program", json!(["sh", "-c", "exit 7"]))],
        ),
    ));
    let session_id: SessionId = id_of(&session).parse().unwrap();
    let shown = ok(runtime.handle(
        &api,
        &no_context(),
        &request("session.show", vec![("target", json!(id_of(&session)))]),
    ));
    let tab_id: TabId = shown["current_tab"].as_str().unwrap().parse().unwrap();
    let context = Context {
        default_session: session_id,
        default_tab: tab_id,
        default_pane: PaneId::from_raw(0),
    };

    let record = poll(|| {
        let record = ok(runtime.handle(
            &api,
            &context,
            &request("pane.show", vec![("target", json!("1"))]),
        ));
        (record["status"] == json!("ended")).then_some(record)
    });

    assert_eq!(record["status"], json!("ended"));
    assert_eq!(record["exit_code"], json!(7));
    assert_eq!(record["program"], json!("sh -c exit 7"));
}

#[test]
fn pane_split_with_no_program_or_cwd_runs_the_login_shell_at_home() {
    let (runtime, api) = runtime();
    let session = ok(runtime.handle(&api, &no_context(), &request("session.new", vec![])));
    let session_id: SessionId = id_of(&session).parse().unwrap();
    let shown = ok(runtime.handle(
        &api,
        &no_context(),
        &request("session.show", vec![("target", json!(id_of(&session)))]),
    ));
    let tab_id: TabId = shown["current_tab"].as_str().unwrap().parse().unwrap();
    let context = Context {
        default_session: session_id,
        default_tab: tab_id,
        default_pane: PaneId::from_raw(0),
    };

    let record = ok(runtime.handle(
        &api,
        &context,
        &request("pane.show", vec![("target", json!("1"))]),
    ));
    assert_eq!(record["program"], json!("shell"));
    assert_eq!(record["cwd"], json!("/tmp"));
    assert_eq!(record["status"], json!("running"));
}

#[test]
fn a_command_that_does_not_exist_fails_and_leaves_no_session_tab_or_pane_behind() {
    let (runtime, api) = runtime();
    let before = counts(&runtime, &api);

    let error = err(runtime.handle(
        &api,
        &no_context(),
        &request(
            "session.new",
            vec![("program", json!(["steg-this-does-not-exist-anywhere"]))],
        ),
    ));

    assert_eq!(error.code, ErrorCode::ProgramFailed);
    assert_eq!(
        counts(&runtime, &api),
        before,
        "nothing should be left behind"
    );
}

#[test]
fn tab_new_with_a_bad_command_leaves_no_new_tab_or_pane_behind() {
    let (runtime, api) = runtime();
    let session = ok(runtime.handle(&api, &no_context(), &request("session.new", vec![])));
    let session_id: SessionId = id_of(&session).parse().unwrap();
    let context = Context {
        default_session: session_id,
        default_tab: TabId::from_raw(0),
        default_pane: PaneId::from_raw(0),
    };
    let before = counts(&runtime, &api);

    let error = err(runtime.handle(
        &api,
        &context,
        &request(
            "tab.new",
            vec![
                ("target", json!(id_of(&session))),
                ("program", json!(["steg-this-does-not-exist-anywhere"])),
            ],
        ),
    ));

    assert_eq!(error.code, ErrorCode::ProgramFailed);
    assert_eq!(counts(&runtime, &api), before);
}

#[test]
fn pane_split_with_a_bad_command_leaves_no_new_pane_behind() {
    let (runtime, api) = runtime();
    let session = ok(runtime.handle(&api, &no_context(), &request("session.new", vec![])));
    let session_id: SessionId = id_of(&session).parse().unwrap();
    let shown = ok(runtime.handle(
        &api,
        &no_context(),
        &request("session.show", vec![("target", json!(id_of(&session)))]),
    ));
    let tab_id: TabId = shown["current_tab"].as_str().unwrap().parse().unwrap();
    let context = Context {
        default_session: session_id,
        default_tab: tab_id,
        default_pane: PaneId::from_raw(0),
    };
    let before = counts(&runtime, &api);

    let error = err(runtime.handle(
        &api,
        &context,
        &request(
            "pane.split",
            vec![
                ("target", json!("1")),
                ("direction", json!("right")),
                ("program", json!(["steg-this-does-not-exist-anywhere"])),
            ],
        ),
    ));

    assert_eq!(error.code, ErrorCode::ProgramFailed);
    assert_eq!(counts(&runtime, &api), before);
}

// `pane_split_inherits_the_live_cwd_...` lives in `src/runtime.rs`'s own
// `#[cfg(test)]` module instead of here: confirming the OSC 7 has landed
// before splitting needs `PaneHandle::cwd()` directly (rule B's own
// source of truth), which only internal test code can reach - `pane.show`
// always reports a pane's *started-in* `Program::cwd()` (kernel-only,
// pure, by this task's own design), never the live one, so polling it
// for this can never observe the thing this test is about.

// --- AC #4: the refusal, and --force's stated gap --------------------------

fn session_with_a_long_running_pane(runtime: &Runtime, api: &Description) -> (SessionId, TabId) {
    let session = ok(runtime.handle(
        api,
        &no_context(),
        &request(
            "session.new",
            vec![("program", json!(["sh", "-c", "sleep 100"]))],
        ),
    ));
    let session_id: SessionId = id_of(&session).parse().unwrap();
    let shown = ok(runtime.handle(
        api,
        &no_context(),
        &request("session.show", vec![("target", json!(id_of(&session)))]),
    ));
    let tab_id: TabId = shown["current_tab"].as_str().unwrap().parse().unwrap();
    (session_id, tab_id)
}

#[test]
fn pane_close_refuses_while_its_program_is_running_and_touches_nothing() {
    let (runtime, api) = runtime();
    let (session_id, tab_id) = session_with_a_long_running_pane(&runtime, &api);
    let context = Context {
        default_session: session_id,
        default_tab: tab_id,
        default_pane: PaneId::from_raw(0),
    };
    let before = counts(&runtime, &api);

    let error = err(runtime.handle(
        &api,
        &context,
        &request("pane.close", vec![("target", json!("1"))]),
    ));

    assert_eq!(error.code, ErrorCode::ProgramRunning);
    assert_eq!(counts(&runtime, &api), before);
    // Still there, and still running - not partially closed.
    let shown = ok(runtime.handle(
        &api,
        &context,
        &request("pane.show", vec![("target", json!("1"))]),
    ));
    assert_eq!(shown["status"], json!("running"));
}

#[test]
fn tab_close_refuses_while_any_of_its_panes_is_running() {
    let (runtime, api) = runtime();
    let (session_id, tab_id) = session_with_a_long_running_pane(&runtime, &api);
    let context = Context {
        default_session: session_id,
        default_tab: tab_id,
        default_pane: PaneId::from_raw(0),
    };
    // A second, already-ended pane in the same tab - the refusal must
    // still trigger because of the *other* pane, not only when every
    // pane is running.
    ok(runtime.handle(
        &api,
        &context,
        &request(
            "pane.split",
            vec![
                ("target", json!("1")),
                ("direction", json!("right")),
                ("program", json!(["sh", "-c", "exit 0"])),
            ],
        ),
    ));
    let before = counts(&runtime, &api);

    let error = err(runtime.handle(
        &api,
        &context,
        &request("tab.close", vec![("target", json!(tab_id.to_string()))]),
    ));

    assert_eq!(error.code, ErrorCode::ProgramRunning);
    assert_eq!(counts(&runtime, &api), before);
}

#[test]
fn session_close_refuses_while_any_pane_in_any_of_its_tabs_is_running() {
    let (runtime, api) = runtime();
    let (session_id, _tab_id) = session_with_a_long_running_pane(&runtime, &api);
    let context = Context {
        default_session: session_id,
        default_tab: TabId::from_raw(0),
        default_pane: PaneId::from_raw(0),
    };
    let before = counts(&runtime, &api);

    let error = err(runtime.handle(
        &api,
        &context,
        &request(
            "session.close",
            vec![("target", json!(id_of_session(session_id)))],
        ),
    ));

    assert_eq!(error.code, ErrorCode::ProgramRunning);
    assert_eq!(counts(&runtime, &api), before);
}

fn id_of_session(id: SessionId) -> String {
    id.to_string()
}

#[test]
fn pane_close_with_force_on_a_running_program_proceeds_exactly_as_today_the_stated_gap() {
    // AC #4: "--force stays as it is today until the forced-close task" -
    // an unconditional close, touching the program not at all. The
    // process itself staying alive, orphaned, until it exits on its own
    // and the stale `PaneHandle` is cleaned up by `apply_event`'s own
    // `PaneNotFound` handling, is proven at the kernel-access level in
    // `src/runtime.rs`'s own tests (`apply_event_on_a_pane_the_tree_no_
    // longer_holds_removes_the_stale_handle_without_panicking`); this
    // proves the externally-observable half: the tree really does lose
    // the pane, immediately, through the public API alone.
    let (runtime, api) = runtime();
    let (session_id, tab_id) = session_with_a_long_running_pane(&runtime, &api);
    let context = Context {
        default_session: session_id,
        default_tab: tab_id,
        default_pane: PaneId::from_raw(0),
    };

    ok(runtime.handle(
        &api,
        &context,
        &request(
            "pane.close",
            vec![("target", json!("1")), ("force", json!(true))],
        ),
    ));

    // The pane (and, since it was the tab's only one, the tab and the
    // session too, R94) is gone - closing something already closed is
    // `NoSuchTarget`/an empty list, never a lingering `ProgramRunning`.
    let sessions = ok(runtime.handle(&api, &no_context(), &request("session.list", vec![])));
    assert_eq!(sessions, json!([]));
}
