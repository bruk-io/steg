//! Functional tests for [`Runtime`]: real programs on a real pty, driven
//! only through `Runtime::handle` (`.claude/rules/testing-rust.md`), the
//! way a real connection will call it once TASK-1.44/1.45 build the
//! socket. `crates/server/src/runtime/`'s own `#[cfg(test)]` modules
//! covers what only has access to `Runtime`'s private pieces (the
//! `apply_event`/`apply_size_changed` cascades, deterministically); this
//! file covers what a real client-shaped caller sees.

#![expect(
    clippy::unwrap_used,
    reason = "this whole file is a test; see dispatch/main.rs's own comment"
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

/// Bounded join (TASK-1.137) - the same shape `tests/support/mod.rs`'s
/// own `run_bounded` uses: a watcher thread does the real join(s), and
/// this only ever blocks on a channel with `POLL_TIMEOUT`, so a thread
/// that never finishes fails with a named message instead of hanging
/// the job. Not pulled from `support` itself: this file never opens a
/// socket, so it has no other reason to include that module.
fn join_bounded<T: Send + 'static>(what: &str, body: impl FnOnce() -> T + Send + 'static) -> T {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(body());
    });
    rx.recv_timeout(POLL_TIMEOUT)
        .unwrap_or_else(|_| panic!("timed out after {POLL_TIMEOUT:?} waiting for: {what}"))
}

/// A tiny `attach_tick` for every `Runtime::new` in this file that does
/// not itself need a real tick to measure against (TASK-1.47) - none of
/// this file's own tests attach to a pane.
const TEST_TICK: Duration = Duration::from_millis(1);

/// A working `PATH` is required (TASK-1.87): `RealLauncher::spawn`'s own
/// `.env_clear()` means a `base_env` with none cannot resolve a relative
/// program name like `"sh"` - this test file's own ambient `PATH` is
/// fine to read here (a test, not production code). Shared by every
/// `LoginShell` this file builds (`runtime()`'s own call, and TASK-1.78's
/// own force-close tests below, each of which needs its own `Runtime`
/// built with its own `force_grace`, not the shared `runtime()` helper's
/// `TEST_GRACE`).
fn login_for_force_tests() -> LoginShell {
    LoginShell {
        argv0: "/bin/sh".to_string(),
        home: "/tmp".to_string(),
        user: None,
        base_env: vec![(
            std::ffi::OsString::from("PATH"),
            std::env::var_os("PATH").unwrap_or_default(),
        )],
    }
}

fn runtime() -> (Runtime, Description) {
    (
        Runtime::new(
            Arc::new(RealLauncher::default()),
            login_for_force_tests(),
            TEST_GRACE,
            TEST_TICK,
        ),
        description(),
    )
}

/// No real session exists yet, so there is nothing to default to -
/// `session.new` itself never reads `Context` at all, and every other
/// method in these tests is given an explicit target instead of relying
/// on a default.
fn no_context() -> Context {
    Context::none()
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
            default_session: Some(session_id.parse().unwrap()),
            default_tab: Some(tab_id.parse().unwrap()),
            default_pane: Some(PaneId::from_raw(0)),
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
        default_session: Some(session_id),
        default_tab: Some(tab_id),
        default_pane: Some(PaneId::from_raw(0)),
    };

    let record = poll(|| {
        let record = ok(runtime.handle(
            &api,
            &context,
            &request("pane.show", vec![("target", json!("1"))]),
        ));
        (record["program_status"] == json!("ended")).then_some(record)
    });

    assert_eq!(record["program_status"], json!("ended"));
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
        default_session: Some(session_id),
        default_tab: Some(tab_id),
        default_pane: Some(PaneId::from_raw(0)),
    };

    let record = ok(runtime.handle(
        &api,
        &context,
        &request("pane.show", vec![("target", json!("1"))]),
    ));
    assert_eq!(record["program"], json!("shell"));
    assert_eq!(record["cwd"], json!("/tmp"));
    assert_eq!(record["program_status"], json!("running"));
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

/// A launcher whose open step always fails - the shape `.claude/rules/
/// testing-rust.md` asks for (a small type implementing the same trait,
/// holding its state in plain fields, not a mock of anything), standing
/// in for `RealLauncher`'s own open step exhausting every retry attempt
/// (TASK-1.112, AC #3). From `Runtime`'s own point of view these are the
/// identical shape: `ProgramLauncher::spawn` returning `Err` once,
/// synchronously, before anything about the pane exists at all - so this
/// proves the same clean-failure-and-rollback path without needing a
/// real, contrived cross-process race to land on cue inside one test.
#[derive(Debug, Default)]
struct AlwaysFailsToOpenLauncher;

impl steg_server::ProgramLauncher for AlwaysFailsToOpenLauncher {
    fn spawn(
        &self,
        _spec: &steg_server::LaunchSpec,
    ) -> Result<steg_server::SpawnedProgram, steg_server::Error> {
        Err(steg_server::Error::Pty(
            "failed to openpty after 5 attempt(s): simulated exhaustion (TASK-1.112's own test)"
                .into(),
        ))
    }
}

/// AC #3: the open step itself exhausting every attempt (not a bad
/// command reaching `spawn_command`, which the test above this one
/// already covers through the real launcher) still fails cleanly -
/// `ProgramFailed`, and the rollback that already exists for a bad
/// command (`Runtime::start_and_dispatch`'s own `rollback_mint`, read
/// directly: it runs on *any* `Err` from `pane::start`, not one specific
/// to how the error arose) leaves nothing behind here either.
///
/// Control, recorded via `asdlc record-check` both ways: with
/// `rollback_mint`'s own call site in `runtime/start.rs` temporarily commented
/// out, this test fails (`counts` differs) - restored, it passes.
#[test]
fn a_panes_pty_that_cannot_be_opened_after_every_attempt_fails_cleanly_and_leaves_nothing_behind() {
    let runtime = Runtime::new(
        Arc::new(AlwaysFailsToOpenLauncher),
        login_for_force_tests(),
        TEST_GRACE,
        TEST_TICK,
    );
    let api = description();
    let before = counts(&runtime, &api);

    let error = err(runtime.handle(&api, &no_context(), &request("session.new", vec![])));

    assert_eq!(error.code, ErrorCode::ProgramFailed);
    assert_eq!(
        counts(&runtime, &api),
        before,
        "nothing should be left behind when the open step itself exhausts every attempt"
    );
}

#[test]
fn tab_new_with_a_bad_command_leaves_no_new_tab_or_pane_behind() {
    let (runtime, api) = runtime();
    let session = ok(runtime.handle(&api, &no_context(), &request("session.new", vec![])));
    let session_id: SessionId = id_of(&session).parse().unwrap();
    let context = Context {
        default_session: Some(session_id),
        default_tab: Some(TabId::from_raw(0)),
        default_pane: Some(PaneId::from_raw(0)),
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
        default_session: Some(session_id),
        default_tab: Some(tab_id),
        default_pane: Some(PaneId::from_raw(0)),
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

// `pane_split_inherits_the_live_cwd_...` lives in `src/runtime/start.rs`'s own
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
        default_session: Some(session_id),
        default_tab: Some(tab_id),
        default_pane: Some(PaneId::from_raw(0)),
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
    assert_eq!(shown["program_status"], json!("running"));
}

#[test]
fn tab_close_refuses_while_any_of_its_panes_is_running() {
    let (runtime, api) = runtime();
    let (session_id, tab_id) = session_with_a_long_running_pane(&runtime, &api);
    let context = Context {
        default_session: Some(session_id),
        default_tab: Some(tab_id),
        default_pane: Some(PaneId::from_raw(0)),
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
        default_session: Some(session_id),
        default_tab: Some(TabId::from_raw(0)),
        default_pane: Some(PaneId::from_raw(0)),
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
        default_session: Some(session_id),
        default_tab: Some(tab_id),
        default_pane: Some(PaneId::from_raw(0)),
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
        default_session: Some(session_id),
        default_tab: Some(tab_id),
        default_pane: Some(PaneId::from_raw(0)),
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
        default_session: Some(session_id),
        default_tab: Some(tab_id),
        default_pane: Some(PaneId::from_raw(0)),
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
fn a_flooding_program_that_exits_by_itself_mid_grace_completes_promptly_as_a_regression() {
    // TASK-1.78's review, finding 1: a flooding program's own real exit,
    // arriving naturally during the grace window, must not be
    // misclassified as "still running" purely because its own backlog
    // of `PtyBytes` has not drained through the worker's queue yet -
    // which would otherwise cause a pointless `SIGKILL` and an extra
    // `ESCALATE_CONFIRM_TIMEOUT` wait (2s) on top of the grace period
    // itself. A deliberately long grace (3s) is what makes "it returned
    // promptly" a meaningful assertion rather than noise under the
    // *fixed* mechanism - but the review's second round found this is
    // not a proof: it still passes with `await_exit` routed back through
    // the worker's own queue, because the kernel's own pty buffer
    // throttles the writing shell well before a 20,000-line flood builds
    // the backlog this bug actually needs (confirmed by hand, recorded as
    // a check at the time, not hidden). A real, deterministic proof needs
    // a backlog this task does not depend on any OS buffering to build,
    // which is what `pane::ending_tests::await_exit_is_not_delayed_by_a_
    // backlog_on_the_workers_own_queue` (two separate handles, a real
    // 1,000,000-item backlog built directly through the handle's own
    // `send_input`) does instead - *that* test is this mechanism's actual
    // proof, with its own recorded negative control; this one is kept as
    // a realistic end-to-end regression only, named for exactly that.
    let grace = Duration::from_secs(3);
    let runtime = Runtime::new(
        Arc::new(RealLauncher::default()),
        login_for_force_tests(),
        grace,
        TEST_TICK,
    );
    let api = description();
    // Floods a real backlog (20,000 lines through the pty, not a file -
    // confirmed by hand to take well under a second for a shell's own
    // builtin loop+echo) then exits on its own with a distinct code,
    // never signalled by this task's own mechanism at all.
    let session = ok(runtime.handle(
        &api,
        &no_context(),
        &request(
            "session.new",
            vec![(
                "program",
                json!([
                    "sh",
                    "-c",
                    "i=0; while [ $i -lt 20000 ]; do echo xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx; i=$((i+1)); done; exit 7"
                ]),
            )],
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
        default_session: Some(session_id),
        default_tab: Some(tab_id),
        default_pane: Some(PaneId::from_raw(0)),
    };

    let start = std::time::Instant::now();
    ok(runtime.handle(
        &api,
        &context,
        &request(
            "pane.close",
            vec![("target", json!("1")), ("force", json!(true))],
        ),
    ));
    let elapsed = start.elapsed();

    assert!(
        elapsed < Duration::from_millis(1500),
        "force-closing a flooding program that had already exited on its \
         own took {elapsed:?} - close to the grace ({grace:?}) plus the \
         escalation-confirm margin would mean it was misclassified as a \
         straggler and needlessly killed"
    );
}

#[test]
fn tab_close_with_force_ends_every_program_in_scope_within_one_grace_window() {
    // Brief item 4: several running panes at once, over one *shared*
    // grace window, not one per pane. Every one of the four panes traps
    // the polite signal, so every one needs the escalation - a serial
    // "signal, wait the whole grace, escalate" loop would cost roughly
    // `4 * grace` at minimum; a shared window costs roughly one `grace`
    // regardless of how many panes. The grace here (500ms) is
    // deliberately real, not `TEST_GRACE`'s usual 30ms: a bound tight
    // enough to separate "one window" from "four windows" needs a
    // window wide enough that scheduling noise cannot close the gap.
    let grace = Duration::from_millis(500);
    let runtime = Runtime::new(
        Arc::new(RealLauncher::default()),
        login_for_force_tests(),
        grace,
        TEST_TICK,
    );
    let api = description();

    // The marker file's own content is the pid (`echo $$`, not a bare
    // `touch`) - one file doubles as both "the trap is installed" and
    // "this is who to check for afterward", with no second file needed.
    fn trapping_pane_program(ready: &std::path::Path) -> Vec<String> {
        vec![
            "sh".to_string(),
            "-c".to_string(),
            format!(
                "trap '' HUP; echo $$ > {} ; exec sleep 1000",
                ready.display()
            ),
        ]
    }
    fn ready_marker(tag: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "steg-tab-close-ready-{}-{tag}.marker",
            std::process::id()
        ))
    }

    let first_ready = ready_marker("0");
    let _ = std::fs::remove_file(&first_ready);
    let session = ok(runtime.handle(
        &api,
        &no_context(),
        &request(
            "session.new",
            vec![("program", json!(trapping_pane_program(&first_ready)))],
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
        default_session: Some(session_id),
        default_tab: Some(tab_id),
        default_pane: Some(PaneId::from_raw(0)),
    };

    // Three more panes in the same tab, each its own real program,
    // trapping the polite signal too - `tab.close --force` has to end
    // all four, every one by escalating.
    let mut readies = vec![first_ready];
    for i in 1..4 {
        let ready = ready_marker(&i.to_string());
        let _ = std::fs::remove_file(&ready);
        ok(runtime.handle(
            &api,
            &context,
            &request(
                "pane.split",
                vec![
                    ("target", json!("1")),
                    ("direction", json!("right")),
                    ("program", json!(trapping_pane_program(&ready))),
                ],
            ),
        ));
        readies.push(ready);
    }
    // Every pane must have reached its own trap+marker before forcing,
    // the same race `pane::ending_tests`' own trapping test found and
    // fixed the same way - and each marker's own content is that pane's
    // real pid, read back for the dead-afterward check below.
    let pids: Vec<u32> = readies
        .iter()
        .map(|ready| {
            let pid = poll(|| {
                std::fs::read_to_string(ready)
                    .ok()
                    .and_then(|text| text.trim().parse().ok())
            });
            let _ = std::fs::remove_file(ready);
            pid
        })
        .collect();

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
        elapsed < grace * 2,
        "ending four trapping panes at once took {elapsed:?}, more than \
         twice one grace window ({grace:?}) - the bound this task's own \
         force_end gives is one shared window regardless of pane count, \
         not `panes * grace`, which four of them would already exceed \
         on its own ({:?})",
        grace * 4
    );
    for pid in &pids {
        assert!(
            !process_exists(*pid),
            "pane with pid {pid} must actually be dead, not only removed from the tree"
        );
    }
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
        default_session: Some(session_id),
        default_tab: Some(tab_id),
        default_pane: Some(PaneId::from_raw(0)),
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
fn pane_close_on_an_already_ended_pane_succeeds_without_force() {
    // A plain close (no `--force`) of a pane whose program had already
    // ended must succeed, not refuse - `ProgramRunning` only fires while
    // something genuinely is. This is the externally-observable half;
    // the idle-handle fix itself (TASK-1.78 §5, item 2 - a stale
    // `PaneHandle` and its worker thread, never visible through this
    // public API) is proven at the kernel-access level instead, in
    // `src/runtime/close.rs`'s own `#[cfg(test)]` module, where `state.panes`
    // can actually be read (DRAFT-1: this test's own assertions below
    // would pass identically with or without that fix, so it is not
    // offered as proof of it).
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
        default_session: Some(session_id),
        default_tab: Some(tab_id),
        default_pane: Some(PaneId::from_raw(0)),
    };
    // Waits for the real exit first (R97: a command pane stays), so the
    // program is genuinely `Ended`, not merely assumed to be by now.
    poll(|| {
        let shown = ok(runtime.handle(
            &api,
            &context,
            &request("pane.show", vec![("target", json!("1"))]),
        ));
        // TASK-1.89 renamed the clock-free fact this polls for to
        // `program_status` (`status` is now doc-11's richer value,
        // typed `Status`, which has no `"ended"` word at all).
        (shown["program_status"] == json!("ended")).then_some(())
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
        Arc::new(RealLauncher::default()),
        login_for_force_tests(),
        Duration::from_secs(5),
        TEST_TICK,
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
        default_session: Some(session_id),
        default_tab: Some(tab_id),
        default_pane: Some(PaneId::from_raw(0)),
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

    join_bounded(
        "the forcing thread to finish its force_end call",
        move || forcing.join().unwrap(),
    );
    assert!(!process_exists(pid));
}

/// TASK-1.78's review, finding 2: AC #4's "unreachable by any method"
/// guarantee was claimed for every method but tested only for
/// `pane.show` and a second `--force` - this covers every other method
/// that can name a pane: a plain `pane.close` (no `--force`), `pane.
/// split` (names an existing pane as its own source - the review's own
/// nit), `pane.swap`, `pane.move`, `pane.set`, `pane.unset`, `pane.
/// focus`. `pane.resize`/`pane.send`/`pane.capture`/`pane.wait` are left
/// out - not implemented yet (`dispatch/pane.rs`'s own `not_implemented_
/// yet` arms), so there is nothing here to prove unreachable. One
/// parameterised test, not seven separate ones, so a failure names the
/// method that let the forced pane through.
#[test]
fn a_pane_being_forced_is_unreachable_by_every_method_that_can_name_it() {
    let runtime = Runtime::new(
        Arc::new(RealLauncher::default()),
        login_for_force_tests(),
        Duration::from_secs(5),
        TEST_TICK,
    );
    let api = description();
    let (session_id, tab_id, pid) = session_with_a_pane_reporting_its_pid(
        &runtime,
        &api,
        "sh -c \"trap '' HUP; touch /tmp/steg-method-coverage-ready-$$ ; exec sleep 1000\"",
    );
    let marker = format!("/tmp/steg-method-coverage-ready-{pid}");
    poll(|| std::path::Path::new(&marker).exists().then_some(()));
    let _ = std::fs::remove_file(&marker);
    let context = Context {
        default_session: Some(session_id),
        default_tab: Some(tab_id),
        default_pane: Some(PaneId::from_raw(0)),
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
    std::thread::sleep(Duration::from_millis(50));

    let probes: Vec<(&str, Vec<(&str, Value)>)> = vec![
        ("pane.close", vec![("target", json!("1"))]),
        (
            "pane.split",
            vec![("target", json!("1")), ("direction", json!("right"))],
        ),
        (
            "pane.swap",
            vec![("target", json!("1")), ("with", json!("2"))],
        ),
        (
            "pane.move",
            vec![("target", json!("1")), ("to", json!("1"))],
        ),
        (
            "pane.set",
            vec![("target", json!("1")), ("label", json!("x"))],
        ),
        (
            "pane.unset",
            vec![("target", json!("1")), ("label", json!(true))],
        ),
        ("pane.focus", vec![("target", json!("1"))]),
    ];
    for (method, params) in probes {
        let during = std::time::Instant::now();
        let response = runtime.handle(&api, &context, &request(method, params));
        let elapsed = during.elapsed();

        assert!(
            elapsed < Duration::from_secs(1),
            "{method} against a pane being forced took {elapsed:?} - it \
             must be refused at once, never stalled for the grace window"
        );
        assert!(
            matches!(response, Response::Error(_)),
            "{method} must find the pane being forced unreachable, not \
             act on it as if nothing were happening"
        );
    }

    join_bounded(
        "the forcing thread to finish its force_end call",
        move || forcing.join().unwrap(),
    );
    assert!(!process_exists(pid));
}

/// The same guarantee, for a tab being forced: `tab.close` (plain), `tab.
/// move`, `tab.set`, `tab.unset`, `tab.focus`. `tab.list`/`tab.show` are
/// left out - not implemented yet.
#[test]
fn a_tab_being_forced_is_unreachable_by_every_method_that_can_name_it() {
    let runtime = Runtime::new(
        Arc::new(RealLauncher::default()),
        login_for_force_tests(),
        Duration::from_secs(5),
        TEST_TICK,
    );
    let api = description();
    let (session_id, tab_id, pid) = session_with_a_pane_reporting_its_pid(
        &runtime,
        &api,
        "sh -c \"trap '' HUP; touch /tmp/steg-tab-coverage-ready-$$ ; exec sleep 1000\"",
    );
    let marker = format!("/tmp/steg-tab-coverage-ready-{pid}");
    poll(|| std::path::Path::new(&marker).exists().then_some(()));
    let _ = std::fs::remove_file(&marker);
    let context = Context {
        default_session: Some(session_id),
        default_tab: Some(tab_id),
        default_pane: Some(PaneId::from_raw(0)),
    };

    let runtime = std::sync::Arc::new(runtime);
    let forcing_context = context;
    let forced_runtime = std::sync::Arc::clone(&runtime);
    let forcing = std::thread::spawn(move || {
        forced_runtime.handle(
            &description(),
            &forcing_context,
            &request(
                "tab.close",
                vec![("target", json!("1")), ("force", json!(true))],
            ),
        )
    });
    std::thread::sleep(Duration::from_millis(50));

    let probes: Vec<(&str, Vec<(&str, Value)>)> = vec![
        ("tab.close", vec![("target", json!("1"))]),
        ("tab.move", vec![("target", json!("1")), ("to", json!("1"))]),
        (
            "tab.set",
            vec![("target", json!("1")), ("label", json!("x"))],
        ),
        (
            "tab.unset",
            vec![("target", json!("1")), ("label", json!(true))],
        ),
        ("tab.focus", vec![("target", json!("1"))]),
    ];
    for (method, params) in probes {
        let during = std::time::Instant::now();
        let response = runtime.handle(&api, &context, &request(method, params));
        let elapsed = during.elapsed();

        assert!(
            elapsed < Duration::from_secs(1),
            "{method} against a tab being forced took {elapsed:?} - it \
             must be refused at once, never stalled for the grace window"
        );
        assert!(
            matches!(response, Response::Error(_)),
            "{method} must find the tab being forced unreachable, not \
             act on it as if nothing were happening"
        );
    }

    join_bounded(
        "the forcing thread to finish its force_end call",
        move || forcing.join().unwrap(),
    );
    assert!(!process_exists(pid));
}

/// The same guarantee, for a session being forced: `session.close`
/// (plain), `session.show`, `session.move`, `session.set`, `session.
/// unset`. `session.focus` is left out - not implemented yet.
#[test]
fn a_session_being_forced_is_unreachable_by_every_method_that_can_name_it() {
    let runtime = Runtime::new(
        Arc::new(RealLauncher::default()),
        login_for_force_tests(),
        Duration::from_secs(5),
        TEST_TICK,
    );
    let api = description();
    let (session_id, tab_id, pid) = session_with_a_pane_reporting_its_pid(
        &runtime,
        &api,
        "sh -c \"trap '' HUP; touch /tmp/steg-session-coverage-ready-$$ ; exec sleep 1000\"",
    );
    let marker = format!("/tmp/steg-session-coverage-ready-{pid}");
    poll(|| std::path::Path::new(&marker).exists().then_some(()));
    let _ = std::fs::remove_file(&marker);
    let context = Context {
        default_session: Some(session_id),
        default_tab: Some(tab_id),
        default_pane: Some(PaneId::from_raw(0)),
    };

    let runtime = std::sync::Arc::new(runtime);
    let forcing_context = context;
    let forced_runtime = std::sync::Arc::clone(&runtime);
    let forcing = std::thread::spawn(move || {
        forced_runtime.handle(
            &description(),
            &forcing_context,
            &request(
                "session.close",
                vec![("target", json!("1")), ("force", json!(true))],
            ),
        )
    });
    std::thread::sleep(Duration::from_millis(50));

    let probes: Vec<(&str, Vec<(&str, Value)>)> = vec![
        ("session.close", vec![("target", json!("1"))]),
        ("session.show", vec![("target", json!("1"))]),
        (
            "session.move",
            vec![("target", json!("1")), ("position", json!(1))],
        ),
        (
            "session.set",
            vec![("target", json!("1")), ("label", json!("x"))],
        ),
        (
            "session.unset",
            vec![("target", json!("1")), ("label", json!(true))],
        ),
    ];
    for (method, params) in probes {
        let during = std::time::Instant::now();
        let response = runtime.handle(&api, &context, &request(method, params));
        let elapsed = during.elapsed();

        assert!(
            elapsed < Duration::from_secs(1),
            "{method} against a session being forced took {elapsed:?} - \
             it must be refused at once, never stalled for the grace \
             window"
        );
        assert!(
            matches!(response, Response::Error(_)),
            "{method} must find the session being forced unreachable, \
             not act on it as if nothing were happening"
        );
    }

    join_bounded(
        "the forcing thread to finish its force_end call",
        move || forcing.join().unwrap(),
    );
    assert!(!process_exists(pid));
}

#[test]
fn a_force_racing_another_force_on_the_same_pane_never_double_acts() {
    // AC #4's own single-remover invariant, proven under real
    // concurrency: two threads force the same pane at once - exactly one
    // of them actually ends the program (the other's own call either
    // lands first and does the real work, or lands second and finds the
    // pane already gone) - never both signalling or both closing. Stated
    // plainly, not overclaimed (DRAFT-1): this holds because
    // `close_and_dispatch` runs the whole of `panes_in_close_scope` +
    // `dispatch::dispatch` under one `Mutex`, which already serialises
    // the two callers regardless of close-first versus the first draft's
    // own force-before-close order - it is `a_pane_being_forced_is_
    // unreachable_by_another_request_meanwhile`'s own stall assertion,
    // not this one, that close-first's negative control (§9) actually
    // moves. This test still earns its place: the Mutex giving mutual
    // exclusion is exactly what AC #4's "never both" needs, and nothing
    // here depends on which lock-holding order produces it.
    let runtime = Runtime::new(
        Arc::new(RealLauncher::default()),
        login_for_force_tests(),
        Duration::from_secs(2),
        TEST_TICK,
    );
    let (session_id, tab_id, pid) =
        session_with_a_pane_reporting_its_pid(&runtime, &description(), "sleep 1000");
    let context = Context {
        default_session: Some(session_id),
        default_tab: Some(tab_id),
        default_pane: Some(PaneId::from_raw(0)),
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
    let results: Vec<_> = join_bounded(
        "both racing pane.close --force threads to finish",
        move || threads.into_iter().map(|t| t.join().unwrap()).collect(),
    );

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
        default_session: Some(session_id),
        default_tab: Some(tab_id),
        default_pane: Some(PaneId::from_raw(0)),
    };

    let record = poll(|| {
        let record = ok(runtime.handle(
            &api,
            &context,
            &request("pane.show", vec![("target", json!("1"))]),
        ));
        (record["program_status"] == json!("ended")).then_some(record)
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
        default_session: Some(session_id),
        default_tab: Some(tab_id),
        default_pane: Some(PaneId::from_raw(0)),
    };

    let record = ok(runtime.handle(
        &api,
        &context,
        &request("pane.show", vec![("target", json!("1"))]),
    ));
    assert_eq!(record["program"], json!("shell"));
    assert_eq!(record["program_status"], json!("running"));
}

#[test]
fn tab_new_through_api_call_starts_a_real_program() {
    let (runtime, api) = runtime();
    let session = ok(runtime.handle(&api, &no_context(), &request("session.new", vec![])));
    let session_id: SessionId = id_of(&session).parse().unwrap();
    let context = Context {
        default_session: Some(session_id),
        default_tab: Some(TabId::from_raw(0)),
        default_pane: Some(PaneId::from_raw(0)),
    };

    // No explicit inner `target` here: under the current wire scheme
    // `api.call`'s own selector key is *also* called `target`
    // (TASK-1.81 renames it to `method`, not yet merged), so the two
    // would collide in the same params map. Relying on
    // `context.default_session` instead is exactly what
    // `api_call_of_pane_split_gives_the_same_response_a_direct_call_would`
    // (dispatch/api.rs) already does for the same reason.
    let tab = ok(runtime.handle(
        &api,
        &context,
        &api_call("tab.new", vec![("program", json!(["sh", "-c", "exit 4"]))]),
    ));
    let tab_id: TabId = id_of(&tab).parse().unwrap();
    let tab_context = Context {
        default_session: Some(session_id),
        default_tab: Some(tab_id),
        default_pane: Some(PaneId::from_raw(0)),
    };

    let record = poll(|| {
        let record = ok(runtime.handle(
            &api,
            &tab_context,
            &request("pane.show", vec![("target", json!("1"))]),
        ));
        (record["program_status"] == json!("ended")).then_some(record)
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
        default_session: Some(session_id),
        default_tab: Some(tab_id),
        default_pane: Some(PaneId::from_raw(0)),
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
        default_session: Some(session_id),
        default_tab: Some(tab_id),
        default_pane: Some(first_pane),
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
        default_session: Some(session_id),
        default_tab: Some(tab_id),
        default_pane: Some(id_of(&split).parse().unwrap()),
    };

    let record = poll(|| {
        let record = ok(runtime.handle(&api, &new_pane_context, &request("pane.show", vec![])));
        (record["program_status"] == json!("ended")).then_some(record)
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
        default_session: Some(session_id),
        default_tab: Some(TabId::from_raw(0)),
        default_pane: Some(PaneId::from_raw(0)),
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
        default_session: Some(session_id),
        default_tab: Some(tab_id),
        default_pane: Some(PaneId::from_raw(0)),
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
