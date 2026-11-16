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

/// A tiny `force_grace` for every test in this file that does not itself
/// need a real window to measure against (TASK-1.78 §3) - the AC #4 stall
/// test builds its own `Runtime` with a deliberately long grace instead
/// (see that test), since a 30ms window would make "returned before the
/// window closed" scheduling noise, not evidence.
const TEST_GRACE: Duration = Duration::from_millis(30);

fn runtime() -> (Runtime, Description) {
    (
        Runtime::new(
            Arc::new(RealLauncher),
            LoginShell {
                argv0: "/bin/sh".to_string(),
                home: "/tmp".to_string(),
            },
            TEST_GRACE,
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

/// Builds an `api.call` of `target`, carrying `pairs` as the target
/// method's own parameters alongside the selector (review finding 1:
/// every one of these must reach `Runtime::start_and_dispatch` exactly
/// as a direct call would). The selector's own key is `method`
/// (TASK-1.81), not `target` - naming the parameter here `target` keeps
/// every one of this file's own call sites unchanged.
fn api_call(target: &str, mut pairs: Vec<(&str, Value)>) -> Request {
    pairs.insert(0, ("method", json!(target)));
    request("api.call", pairs)
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
///
/// Bounded sleep-polling, not an event wait (review nit): there is no
/// event for "a snapshot's text changed" to wait on instead - `PaneEvent`
/// only ever carries `Exited`, and an event for a program's own output
/// landing is TASK-1.46's own gap, not this task's to close - so the
/// only alternative to polling the snapshot is blocking forever on text
/// that may never appear.
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

/// Whether a process with this id still exists, reaped or not - the same
/// `kill -0` probe `crate::pane`'s own tests already use.
fn process_exists(pid: u32) -> bool {
    std::process::Command::new("kill")
        .args(["-0", &pid.to_string()])
        .status()
        .unwrap()
        .success()
}

/// Starts a session whose program reports its own pid by writing it to a
/// real file, then `exec`s into `then` - the pid stays the same across
/// that `exec`, so this is the pid of whatever ends up actually running.
/// A plain file, not a `PaneEvent`, because the pid itself is the one
/// thing no event carries - `Exited`, the only variant, is about the
/// *end* of a run, not its start.
fn session_with_a_pane_reporting_its_pid(
    runtime: &Runtime,
    api: &Description,
    then: &str,
) -> (SessionId, TabId, u32) {
    static UNIQUE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let unique = UNIQUE.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let pid_file = std::env::temp_dir().join(format!(
        "steg-runtime-test-pid-{}-{unique}.pid",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&pid_file);

    let session = ok(runtime.handle(
        api,
        &no_context(),
        &request(
            "session.new",
            vec![(
                "program",
                json!([
                    "sh",
                    "-c",
                    format!("echo $$ > {} ; exec {then}", pid_file.display())
                ]),
            )],
        ),
    ));
    let session_id: SessionId = id_of(&session).parse().unwrap();
    let shown = ok(runtime.handle(
        api,
        &no_context(),
        &request("session.show", vec![("target", json!(id_of(&session)))]),
    ));
    let tab_id: TabId = shown["current_tab"].as_str().unwrap().parse().unwrap();

    let pid = poll(|| {
        std::fs::read_to_string(&pid_file)
            .ok()
            .and_then(|text| text.trim().parse().ok())
    });
    let _ = std::fs::remove_file(&pid_file);

    (session_id, tab_id, pid)
}

#[test]
fn pane_close_with_force_actually_ends_a_program_producing_output() {
    // AC #1: a program that is producing output (`yes`), not only a
    // silent one. The pane's own pid, read back from a real file, is what
    // proves the *process* ended, not only the tree entry.
    let (runtime, api) = runtime();
    let (session_id, tab_id, pid) =
        session_with_a_pane_reporting_its_pid(&runtime, &api, "yes >/dev/null");
    let context = Context {
        default_session: session_id,
        default_tab: tab_id,
        default_pane: PaneId::from_raw(0),
    };
    assert!(process_exists(pid), "must be running before the force");

    ok(runtime.handle(
        &api,
        &context,
        &request(
            "pane.close",
            vec![("target", json!("1")), ("force", json!(true))],
        ),
    ));

    assert!(
        !process_exists(pid),
        "the program itself must be ended, not merely orphaned"
    );
    let sessions = ok(runtime.handle(&api, &no_context(), &request("session.list", vec![])));
    assert_eq!(
        sessions,
        json!([]),
        "the pane, its tab and its session (R94) are gone from the tree"
    );
}

#[test]
fn pane_close_with_force_actually_ends_a_silent_program() {
    // AC #1's other half: a program that prints nothing at all - the
    // case TASK-1.40 left (a silent program kept the relay blocked in
    // `read` and the child alive).
    let (runtime, api) = runtime();
    let (session_id, tab_id, pid) =
        session_with_a_pane_reporting_its_pid(&runtime, &api, "sleep 1000");
    let context = Context {
        default_session: session_id,
        default_tab: tab_id,
        default_pane: PaneId::from_raw(0),
    };
    assert!(process_exists(pid), "must be running before the force");

    ok(runtime.handle(
        &api,
        &context,
        &request(
            "pane.close",
            vec![("target", json!("1")), ("force", json!(true))],
        ),
    ));

    assert!(
        !process_exists(pid),
        "a silent program must be ended too, not left running because it \
         never produced output for the relay thread to read"
    );
}

#[test]
fn pane_close_with_force_ends_a_program_that_traps_the_polite_signal() {
    // AC #2: the escalation, through the public path, not only `pane::
    // ending_tests`'s own direct-handle version - a program trapping the
    // polite signal is still ended, by the stated bound.
    let (runtime, api) = runtime();
    let (session_id, tab_id, pid) = session_with_a_pane_reporting_its_pid(
        &runtime,
        &api,
        "sh -c \"trap '' HUP; touch /tmp/steg-trap-ready-marker-$$ ; exec sleep 1000\"",
    );
    let marker = format!("/tmp/steg-trap-ready-marker-{pid}");
    poll(|| std::path::Path::new(&marker).exists().then_some(()));
    let _ = std::fs::remove_file(&marker);
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

    assert!(
        !process_exists(pid),
        "a program trapping the polite signal must still be ended, by escalating"
    );
}

#[test]
fn tab_close_with_force_ends_every_program_in_scope_within_one_grace_window() {
    // Brief item 4: several running panes at once, over one grace window,
    // not one per pane - proven by a real total-time bound, not only "it
    // eventually finished".
    let (runtime, api) = runtime();
    let session = ok(runtime.handle(
        &api,
        &no_context(),
        &request(
            "session.new",
            vec![("program", json!(["sh", "-c", "sleep 1000"]))],
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
    // Three more panes in the same tab, each its own real, silent
    // program - `tab.close --force` has to end all four.
    for _ in 0..3 {
        ok(runtime.handle(
            &api,
            &context,
            &request(
                "pane.split",
                vec![
                    ("target", json!("1")),
                    ("direction", json!("right")),
                    ("program", json!(["sh", "-c", "sleep 1000"])),
                ],
            ),
        ));
    }

    let start = std::time::Instant::now();
    ok(runtime.handle(
        &api,
        &context,
        &request(
            "tab.close",
            vec![
                ("target", json!(tab_id.to_string())),
                ("force", json!(true)),
            ],
        ),
    ));
    let elapsed = start.elapsed();

    assert!(
        elapsed < Duration::from_secs(5),
        "ending four panes at once took {elapsed:?} - the bound is one \
         shared grace window, not one per pane"
    );
    let sessions = ok(runtime.handle(&api, &no_context(), &request("session.list", vec![])));
    assert_eq!(
        sessions,
        json!([]),
        "the whole session closed with the tab (R94)"
    );
}

#[test]
fn session_close_with_force_ends_every_pane_of_every_tab() {
    // `session.close` itself, directly - `tab.close`'s own test above
    // already proves the one-grace-window bound this task's own
    // `force_end` gives either method; this proves `session.close`'s own
    // dispatch arm reaches the same `close_and_dispatch` wrapper, over a
    // session with two tabs, not only one.
    let (runtime, api) = runtime();
    let (session_id, tab_id, first_pid) =
        session_with_a_pane_reporting_its_pid(&runtime, &api, "sleep 1000");
    let context = Context {
        default_session: session_id,
        default_tab: tab_id,
        default_pane: PaneId::from_raw(0),
    };
    let pid_file = std::env::temp_dir().join(format!(
        "steg-runtime-test-pid-second-tab-{}.pid",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&pid_file);
    ok(runtime.handle(
        &api,
        &context,
        &request(
            "tab.new",
            vec![(
                "program",
                json!([
                    "sh",
                    "-c",
                    format!("echo $$ > {} ; exec sleep 1000", pid_file.display())
                ]),
            )],
        ),
    ));
    let second_pid: u32 = poll(|| {
        std::fs::read_to_string(&pid_file)
            .ok()
            .and_then(|text| text.trim().parse().ok())
    });
    let _ = std::fs::remove_file(&pid_file);
    assert!(process_exists(first_pid));
    assert!(process_exists(second_pid));

    ok(runtime.handle(
        &api,
        &context,
        &request(
            "session.close",
            vec![
                ("target", json!(id_of_session(session_id))),
                ("force", json!(true)),
            ],
        ),
    ));

    assert!(!process_exists(first_pid));
    assert!(!process_exists(second_pid));
    let sessions = ok(runtime.handle(&api, &no_context(), &request("session.list", vec![])));
    assert_eq!(sessions, json!([]));
}

#[test]
fn pane_close_with_force_on_an_already_ended_pane_still_cleans_up_its_handle() {
    // The idle-handle fix (TASK-1.78 §5, item 2): a plain close of a pane
    // whose program had already ended used to leave its handle (and
    // worker thread) behind forever - this proves the close succeeds and
    // the pane is gone, exercising the same `close_and_dispatch` path a
    // genuinely running pane does, just with nothing left to signal.
    let (runtime, api) = runtime();
    let session = ok(runtime.handle(
        &api,
        &no_context(),
        &request(
            "session.new",
            vec![("program", json!(["sh", "-c", "exit 0"]))],
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
    // Waits for the real exit first (R97: a command pane stays), so the
    // program is genuinely `Ended`, not merely assumed to be by now.
    poll(|| {
        let shown = ok(runtime.handle(
            &api,
            &context,
            &request("pane.show", vec![("target", json!("1"))]),
        ));
        (shown["status"] == json!("ended")).then_some(())
    });

    // No `--force` needed - nothing is running, so this is the *plain*
    // close path, not the forced one, which is exactly the case the gap
    // was in.
    ok(runtime.handle(
        &api,
        &context,
        &request("pane.close", vec![("target", json!("1"))]),
    ));

    let sessions = ok(runtime.handle(&api, &no_context(), &request("session.list", vec![])));
    assert_eq!(sessions, json!([]));
}

#[test]
fn a_pane_being_forced_is_unreachable_by_another_request_meanwhile() {
    // AC #4's second clause, in full (TASK-1.78 §6): not only a second
    // `--force`, but *any* method naming the pane being forced - proven
    // with a deliberately long grace (see this test's own `runtime`, not
    // the shared tiny `TEST_GRACE`) and a trapping program, so the window
    // is wide enough for "refused at once, not after the grace" to be
    // real evidence rather than scheduling noise.
    let runtime = Runtime::new(
        Arc::new(RealLauncher),
        LoginShell {
            argv0: "/bin/sh".to_string(),
            home: "/tmp".to_string(),
        },
        Duration::from_secs(5),
    );
    let api = description();
    let (session_id, tab_id, pid) = session_with_a_pane_reporting_its_pid(
        &runtime,
        &api,
        "sh -c \"trap '' HUP; touch /tmp/steg-unreachable-ready-$$ ; exec sleep 1000\"",
    );
    let marker = format!("/tmp/steg-unreachable-ready-{pid}");
    poll(|| std::path::Path::new(&marker).exists().then_some(()));
    let _ = std::fs::remove_file(&marker);
    let context = Context {
        default_session: session_id,
        default_tab: tab_id,
        default_pane: PaneId::from_raw(0),
    };

    let runtime = std::sync::Arc::new(runtime);
    let forcing_context = context;
    let forced_runtime = std::sync::Arc::clone(&runtime);
    let forcing = std::thread::spawn(move || {
        forced_runtime.handle(
            &description(),
            &forcing_context,
            &request(
                "pane.close",
                vec![("target", json!("1")), ("force", json!(true))],
            ),
        )
    });

    // Give the forcing thread a moment to acquire the lock and start the
    // close - not a proof of ordering by itself (the second assertion
    // below, which checks the *response*, is), just enough that this
    // request is reasonably likely to land mid-force rather than before
    // it starts at all.
    std::thread::sleep(Duration::from_millis(50));
    let during = std::time::Instant::now();
    let meanwhile = runtime.handle(
        &api,
        &context,
        &request("pane.show", vec![("target", json!("1"))]),
    );
    let elapsed = during.elapsed();

    assert!(
        elapsed < Duration::from_secs(1),
        "a request for the pane being forced took {elapsed:?} - it must be \
         refused at once (the pane is already gone, close-first), never \
         stalled for the grace window"
    );
    assert!(
        matches!(meanwhile, Response::Error(_)),
        "the pane being forced must be unreachable, not shown as if nothing \
         were happening"
    );

    forcing.join().unwrap();
    assert!(!process_exists(pid));
}

#[test]
fn a_force_racing_another_force_on_the_same_pane_never_double_acts() {
    // AC #4's own single-remover invariant, proven under real
    // concurrency: two threads force the same pane at once - exactly one
    // of them actually ends the program (the other's own call either
    // lands first and does the real work, or lands second and finds the
    // pane already gone) - never both signalling or both closing.
    let runtime = Runtime::new(
        Arc::new(RealLauncher),
        LoginShell {
            argv0: "/bin/sh".to_string(),
            home: "/tmp".to_string(),
        },
        Duration::from_secs(2),
    );
    let (session_id, tab_id, pid) =
        session_with_a_pane_reporting_its_pid(&runtime, &description(), "sleep 1000");
    let context = Context {
        default_session: session_id,
        default_tab: tab_id,
        default_pane: PaneId::from_raw(0),
    };
    let runtime = std::sync::Arc::new(runtime);

    let mut threads = Vec::new();
    for _ in 0..2 {
        let runtime = std::sync::Arc::clone(&runtime);
        threads.push(std::thread::spawn(move || {
            runtime.handle(
                &description(),
                &context,
                &request(
                    "pane.close",
                    vec![("target", json!("1")), ("force", json!(true))],
                ),
            )
        }));
    }
    let results: Vec<_> = threads.into_iter().map(|t| t.join().unwrap()).collect();

    let ok_count = results
        .iter()
        .filter(|r| matches!(r, Response::Ok(_)))
        .count();
    assert_eq!(
        ok_count, 1,
        "exactly one of the two concurrent forces must perform the real \
         close; the other must find the pane already gone"
    );
    assert!(!process_exists(pid));
}

// --- Review finding 1: api.call must start a program too ------------------

#[test]
fn session_new_through_api_call_starts_a_real_program() {
    let (runtime, api) = runtime();

    let session = ok(runtime.handle(
        &api,
        &no_context(),
        &api_call(
            "session.new",
            vec![("program", json!(["sh", "-c", "exit 9"]))],
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
    assert_eq!(record["exit_code"], json!(9));
}

#[test]
fn session_new_through_api_call_with_no_program_runs_the_login_shell() {
    let (runtime, api) = runtime();

    let session = ok(runtime.handle(&api, &no_context(), &api_call("session.new", vec![])));
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
    assert_eq!(record["status"], json!("running"));
}

#[test]
fn tab_new_through_api_call_starts_a_real_program() {
    let (runtime, api) = runtime();
    let session = ok(runtime.handle(&api, &no_context(), &request("session.new", vec![])));
    let session_id: SessionId = id_of(&session).parse().unwrap();
    let context = Context {
        default_session: session_id,
        default_tab: TabId::from_raw(0),
        default_pane: PaneId::from_raw(0),
    };

    // No explicit inner `target` here: under the current wire scheme
    // `api.call`'s own selector key is *also* called `target`
    // (TASK-1.81 renames it to `method`, not yet merged), so the two
    // would collide in the same params map. Relying on
    // `context.default_session` instead is exactly what
    // `api_call_of_pane_split_gives_the_same_response_a_direct_call_would`
    // (dispatch.rs) already does for the same reason.
    let tab = ok(runtime.handle(
        &api,
        &context,
        &api_call("tab.new", vec![("program", json!(["sh", "-c", "exit 4"]))]),
    ));
    let tab_id: TabId = id_of(&tab).parse().unwrap();
    let tab_context = Context {
        default_session: session_id,
        default_tab: tab_id,
        default_pane: PaneId::from_raw(0),
    };

    let record = poll(|| {
        let record = ok(runtime.handle(
            &api,
            &tab_context,
            &request("pane.show", vec![("target", json!("1"))]),
        ));
        (record["status"] == json!("ended")).then_some(record)
    });
    assert_eq!(record["exit_code"], json!(4));
}

#[test]
fn pane_split_through_api_call_starts_a_real_program() {
    let (runtime, api) = runtime();
    let session = ok(runtime.handle(&api, &no_context(), &request("session.new", vec![])));
    let session_id: SessionId = id_of(&session).parse().unwrap();
    let shown = ok(runtime.handle(
        &api,
        &no_context(),
        &request("session.show", vec![("target", json!(id_of(&session)))]),
    ));
    let tab_id: TabId = shown["current_tab"].as_str().unwrap().parse().unwrap();
    // The first pane's real id, resolved through a direct call (no
    // `target`/selector collision risk there) - used below as
    // `context.default_pane`, so `pane.split` through `api.call` can
    // omit its own `target` entirely and still split the right pane,
    // avoiding the same `target`-key collision named above.
    let lookup_context = Context {
        default_session: session_id,
        default_tab: tab_id,
        default_pane: PaneId::from_raw(0),
    };
    let first_pane: PaneId = ok(runtime.handle(
        &api,
        &lookup_context,
        &request("pane.show", vec![("target", json!("1"))]),
    ))["id"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();
    let context = Context {
        default_session: session_id,
        default_tab: tab_id,
        default_pane: first_pane,
    };

    let split = ok(runtime.handle(
        &api,
        &context,
        &api_call(
            "pane.split",
            vec![
                ("direction", json!("right")),
                ("program", json!(["sh", "-c", "exit 2"])),
            ],
        ),
    ));
    let new_pane_context = Context {
        default_session: session_id,
        default_tab: tab_id,
        default_pane: id_of(&split).parse().unwrap(),
    };

    let record = poll(|| {
        let record = ok(runtime.handle(&api, &new_pane_context, &request("pane.show", vec![])));
        (record["status"] == json!("ended")).then_some(record)
    });
    assert_eq!(record["exit_code"], json!(2));
}

#[test]
fn a_bad_command_through_api_call_leaves_nothing_behind() {
    let (runtime, api) = runtime();
    let before = counts(&runtime, &api);

    let error = err(runtime.handle(
        &api,
        &no_context(),
        &api_call(
            "session.new",
            vec![("program", json!(["steg-this-does-not-exist-anywhere"]))],
        ),
    ));

    assert_eq!(error.code, ErrorCode::ProgramFailed);
    assert_eq!(counts(&runtime, &api), before);
}

// --- Review finding 4: a caller-given `cwd` that does not exist ------------
//
// "Success for something that did not happen" (the engineer's own words):
// a nonexistent directory must refuse with `ProgramFailed` and leave
// nothing behind, the same shape as the bad-command tests above - never
// silently redirected to the login shell's home, which is rule A/B's own
// fallback for a request that named no `cwd` at all, not for one that
// named a bad one.

/// A path `std::fs` guarantees does not exist, without ever creating or
/// touching anything - fresher than any fixed literal (`std::process::id`
/// makes it unique per test run, so parallel runs of this file never
/// collide on it).
fn nonexistent_cwd() -> String {
    std::env::temp_dir()
        .join(format!("steg-no-such-directory-{}", std::process::id()))
        .to_str()
        .unwrap()
        .to_string()
}

#[test]
fn session_new_with_a_nonexistent_cwd_fails_and_leaves_nothing_behind() {
    let (runtime, api) = runtime();
    let before = counts(&runtime, &api);

    let error = err(runtime.handle(
        &api,
        &no_context(),
        &request("session.new", vec![("cwd", json!(nonexistent_cwd()))]),
    ));

    assert_eq!(error.code, ErrorCode::ProgramFailed);
    assert_eq!(
        counts(&runtime, &api),
        before,
        "nothing should be left behind"
    );
}

#[test]
fn tab_new_with_a_nonexistent_cwd_leaves_no_new_tab_or_pane_behind() {
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
                ("cwd", json!(nonexistent_cwd())),
            ],
        ),
    ));

    assert_eq!(error.code, ErrorCode::ProgramFailed);
    assert_eq!(counts(&runtime, &api), before);
}

#[test]
fn pane_split_with_a_nonexistent_cwd_leaves_no_new_pane_behind() {
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
                ("cwd", json!(nonexistent_cwd())),
            ],
        ),
    ));

    assert_eq!(error.code, ErrorCode::ProgramFailed);
    assert_eq!(counts(&runtime, &api), before);
}

#[test]
fn a_nonexistent_cwd_through_api_call_leaves_nothing_behind() {
    let (runtime, api) = runtime();
    let before = counts(&runtime, &api);

    let error = err(runtime.handle(
        &api,
        &no_context(),
        &api_call("session.new", vec![("cwd", json!(nonexistent_cwd()))]),
    ));

    assert_eq!(error.code, ErrorCode::ProgramFailed);
    assert_eq!(counts(&runtime, &api), before);
}
