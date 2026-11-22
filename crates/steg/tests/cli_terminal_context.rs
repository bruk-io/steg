//! TASK-1.51, doc-2 R33 and section 4: a CLI command with no target acts on
//! the pane, tab or session it runs in (AC #1); outside one, it refuses with
//! status 2 and says so (AC #2); and the `steg` a pane's own program finds
//! belongs to its own server (AC #3) - all proven through the real binary
//! and a real, autostarted on-demand server, never a fake transport.
//!
//! **The chain test (`a_pane_whose_own_program_is_steg_resolves_pane_show_
//! and_finds_its_own_server`) is the strongest proof, for AC #1's pane
//! clause and AC #3 together**: a real pane's own program is `steg` itself,
//! with nothing about its `STEG_SESSION`/`STEG_TAB`/`STEG_PANE`/
//! `STEG_SERVER` hand-set by this test, only by the server (TASK-1.146) and
//! read back by the sdk (TASK-1.147's rename). The tab and session clauses,
//! and a second look at the pane clause, use a hand-set-environment helper
//! instead (a `steg` subprocess this test spawns itself, given
//! `STEG_SESSION`/`TAB`/`PANE` directly): if the chain test ever fails, a
//! hand-set test failing the same way says the hello-to-context half is
//! broken; a hand-set test still passing says the break is somewhere else
//! in the chain (most likely TASK-1.146's own side, or `STEG_BIN_DIR`/
//! `PATH`).
//!
//! **Negative controls (recorded manually, not as separate test functions -
//! each is a one-line revert, run, then restore):**
//! - *Server-side*: in `crates/server/src/connection.rs::handle`, change
//!   `Context::from_hello(session.as_deref(), tab.as_deref(), pane.as_deref())`
//!   back to `Context::none()` for one run. Every test in this file that
//!   depends on context resolving (every one but the AC #2 test) must then
//!   fail with "no target given, and this connection has no .../tab/session
//!   context to fall back on" - confirming the server really is what reads
//!   the hello's context, not something that happened to work anyway.
//! - *CLI-side*: in `crates/steg/src/cli/connect.rs::connect_and_handshake`,
//!   change the three `.as_deref()` arguments passed to `connect_handshaken`
//!   to `None, None, None` for one run (leaving `connection.rs` untouched).
//!   The same tests must fail the same way - confirming the CLI really is
//!   what sends the context, not merely that the server never needed it.
//!
//! Every helper below is duplicated from `cli_exit_statuses.rs`/
//! `server_start.rs` rather than shared, the convention every file in this
//! crate's own `tests/` already follows.

use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::time::{Duration, Instant};

use serde_json::Value;

#[path = "support/mod.rs"]
mod support;
use support::ScopedTempDir;

fn steg_binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_steg"))
}

/// Finds the pid holding `socket_path`'s arbiter lock open, via `lsof -t`
/// on the lock file (`cli_exit_statuses.rs`'s own `find_server_pid`) -
/// needed because every server this file autostarts is spawned and
/// immediately dropped by the sdk's own `connect()`, so no `Child` value
/// here ever refers to it directly.
fn find_server_pid(socket_path: &std::path::Path) -> Option<u32> {
    let mut lock_path = socket_path.as_os_str().to_owned();
    lock_path.push(".lock");
    let output = Command::new("lsof")
        .arg("-t")
        .arg(&lock_path)
        .output()
        .expect("lsof should be runnable on this machine");
    let text = String::from_utf8_lossy(&output.stdout);
    let pids: Vec<&str> = text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect();
    match pids.as_slice() {
        [] => None,
        [one] => Some(
            one.parse()
                .unwrap_or_else(|err| panic!("lsof reported a non-numeric pid {one:?}: {err}")),
        ),
        many => panic!("expected at most one pid holding {lock_path:?}, got {many:?}"),
    }
}

/// Kills, on drop, whatever process `lsof` finds holding `socket_path`
/// open - the real server this test's own command autostarted indirectly
/// (`cli_exit_statuses.rs`'s own `ServerAtSocket`). Never kills by name -
/// only the specific pid found holding this test's own throwaway socket's
/// lock.
struct ServerAtSocket(PathBuf);

impl Drop for ServerAtSocket {
    fn drop(&mut self) {
        if let Some(pid) = find_server_pid(&self.0) {
            let _ = Command::new("kill").args(["-9", &pid.to_string()]).status();
        }
    }
}

fn wait_for_socket(path: &std::path::Path, timeout: Duration) {
    let deadline = Instant::now() + timeout;
    loop {
        if UnixStream::connect(path).is_ok() {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "nothing ever answered at {path:?}"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// Polls for `path` to exist and be non-empty - the real program inside the
/// real pane needs a moment to start and write it, and there is no event
/// this process can wait on instead (`login_environment.rs`'s own
/// `wait_for_output`, duplicated here).
fn wait_for_output(path: &std::path::Path, timeout: Duration) -> String {
    let deadline = Instant::now() + timeout;
    loop {
        if let Ok(text) = std::fs::read_to_string(path)
            && !text.is_empty()
        {
            return text;
        }
        assert!(
            Instant::now() < deadline,
            "the pane's own program never wrote {path:?} within {timeout:?}"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// Runs the real binary to completion against `socket`, with a minimal
/// explicit environment: `PATH` (so `sh` and the binary's own dynamic
/// loading resolve) and `STEG_SERVER`, nothing inherited (`cli_exit_
/// statuses.rs`'s own `run_steg` - SAFETY: this test process's own
/// environment is never handed to a child).
fn run_steg(args: &[&str], socket: &std::path::Path) -> Output {
    Command::new(steg_binary())
        .args(args)
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("STEG_SERVER", socket)
        .output()
        .expect("the real steg binary should run to completion")
}

/// Like [`run_steg`], with `extra` env vars added on top of `PATH`/
/// `STEG_SERVER` - the hand-set-environment tests' own way of putting a
/// CLI invocation "inside" a pane, by giving it directly the three
/// variables a real pane's own program would otherwise have to actually
/// be to carry.
fn run_steg_with_env(args: &[&str], socket: &std::path::Path, extra: &[(&str, &str)]) -> Output {
    let mut command = Command::new(steg_binary());
    command
        .args(args)
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("STEG_SERVER", socket);
    for (key, value) in extra {
        command.env(key, value);
    }
    command
        .output()
        .expect("the real steg binary should run to completion")
}

fn stdout_trimmed(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

fn stderr_of(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

/// One real pane's own identifiers, learned the same way a real client
/// would - through the CLI itself (`session new` -> `tab list` -> `pane
/// list`), never peeked out of the server's own internal tree directly,
/// since this file is testing exactly what a CLI-shaped caller would see
/// (the same reasoning `crates/server/tests/pane_environment.rs`'s own
/// `RealPane`/`new_pane_running` give, one layer further out - through the
/// compiled binary, not the sdk's `Connection` directly).
struct RealPane {
    session_id: String,
    tab_id: String,
    pane_id: String,
}

impl RealPane {
    fn path(&self) -> String {
        format!("{}/{}/{}", self.session_id, self.tab_id, self.pane_id)
    }
}

/// Starts a session with `program` as its one pane's own explicit program
/// (never the login shell - this crate's own tests never rely on an
/// interactive shell's own behaviour), and reads back its real session,
/// tab and pane ids through the CLI's own `--json` output.
fn new_pane_running(socket: &std::path::Path, program: &[&str]) -> RealPane {
    let mut new_args = vec!["session", "new", "--"];
    new_args.extend_from_slice(program);
    let created = run_steg(&new_args, socket);
    assert!(
        created.status.success(),
        "session new failed: {}",
        stderr_of(&created)
    );
    let session_id = stdout_trimmed(&created);

    let tabs = run_steg(&["tab", "list", &session_id, "--json"], socket);
    assert!(
        tabs.status.success(),
        "tab list failed: {}",
        stderr_of(&tabs)
    );
    let tabs: Vec<Value> = serde_json::from_slice(&tabs.stdout).expect("tab list prints JSON");
    let tab_id = tabs
        .first()
        .expect("the new session has one tab")
        .get("id")
        .and_then(Value::as_str)
        .expect("each tab record has an id")
        .to_string();

    let panes = run_steg(
        &["pane", "list", &format!("{session_id}/{tab_id}"), "--json"],
        socket,
    );
    assert!(
        panes.status.success(),
        "pane list failed: {}",
        stderr_of(&panes)
    );
    let panes: Vec<Value> = serde_json::from_slice(&panes.stdout).expect("pane list prints JSON");
    let pane_id = panes
        .first()
        .expect("the new session has one pane")
        .get("id")
        .and_then(Value::as_str)
        .expect("each pane record has an id")
        .to_string();

    RealPane {
        session_id,
        tab_id,
        pane_id,
    }
}

// --- AC #2: outside a pane, no target and no context is status 2 ----------

#[test]
fn pane_show_with_no_target_outside_a_pane_is_status_2_and_says_so() {
    let temp_dir = ScopedTempDir::new();
    let socket = temp_dir.path("s.sock");
    let _server_guard = ServerAtSocket(socket.clone());

    let output = run_steg(&["pane", "show"], &socket);

    assert_eq!(
        output.status.code(),
        Some(2),
        "stderr was {}",
        stderr_of(&output)
    );
    let stderr = stderr_of(&output);
    assert!(
        stderr.contains("no target given, and this connection has no pane context to fall back on"),
        "stderr was {stderr:?}"
    );
}

// --- AC #1's tab and session clauses, and a second look at its pane clause,
// --- through a hand-set environment -----------------------------------------

#[test]
fn pane_list_with_no_target_inside_a_pane_resolves_to_that_panes_own_tab() {
    let temp_dir = ScopedTempDir::new();
    let socket = temp_dir.path("s.sock");
    let _server_guard = ServerAtSocket(socket.clone());

    let real = new_pane_running(&socket, &["sh", "-c", "sleep 100"]);
    wait_for_socket(&socket, Duration::from_secs(5));
    // A decoy, in its own session and tab - never named by the context
    // below, so a wrong answer that merely picked "the first thing that
    // exists" would be caught, not only a wrong answer that picked nothing
    // at all.
    let _decoy = new_pane_running(&socket, &["sh", "-c", "sleep 100"]);

    let output = run_steg_with_env(
        &["pane", "list", "--json"],
        &socket,
        &[
            ("STEG_SESSION", real.session_id.as_str()),
            ("STEG_TAB", real.tab_id.as_str()),
            ("STEG_PANE", real.pane_id.as_str()),
        ],
    );
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr was {}",
        stderr_of(&output)
    );
    let records: Vec<Value> = serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|err| panic!("pane list should print JSON: {err}; got {output:?}"));
    assert_eq!(
        records.len(),
        1,
        "expected exactly the real pane's own tab's one pane, not the decoy's: {records:?}"
    );
    assert_eq!(
        records[0].get("id").and_then(Value::as_str),
        Some(real.pane_id.as_str())
    );
}

#[test]
fn session_show_with_no_target_inside_a_pane_resolves_to_that_panes_own_session() {
    let temp_dir = ScopedTempDir::new();
    let socket = temp_dir.path("s.sock");
    let _server_guard = ServerAtSocket(socket.clone());

    let real = new_pane_running(&socket, &["sh", "-c", "sleep 100"]);
    wait_for_socket(&socket, Duration::from_secs(5));
    let _decoy = new_pane_running(&socket, &["sh", "-c", "sleep 100"]);

    let output = run_steg_with_env(
        &["session", "show", "--json"],
        &socket,
        &[
            ("STEG_SESSION", real.session_id.as_str()),
            ("STEG_TAB", real.tab_id.as_str()),
            ("STEG_PANE", real.pane_id.as_str()),
        ],
    );
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr was {}",
        stderr_of(&output)
    );
    let record: Value = serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|err| panic!("session show should print JSON: {err}; got {output:?}"));
    assert_eq!(
        record.get("id").and_then(Value::as_str),
        Some(real.session_id.as_str()),
        "expected the real pane's own session, not the decoy's"
    );
}

// --- The chain test: AC #1's pane clause and AC #3 together, with nothing
// --- hand-set at all ---------------------------------------------------------

/// AC #1's pane clause and AC #3, proven through one real pane whose own
/// program is `steg` itself. `session new --` runs a one-line shell
/// program that first reports which `steg` its own `PATH` resolves to
/// (AC #3), then calls `steg pane show --json` with no target at all (AC
/// #1) - both lines redirected to a file this test reads back once the
/// pane's own program has ended (a command pane stays reachable once its
/// program ends, doc-2 section 6, so `pane wait --for exit` - not polling -
/// is what tells this test the file is complete).
#[test]
fn a_pane_whose_own_program_is_steg_resolves_pane_show_and_finds_its_own_server() {
    let temp_dir = ScopedTempDir::new();
    let socket = temp_dir.path("s.sock");
    let out_path = temp_dir.path("out.txt");
    let _server_guard = ServerAtSocket(socket.clone());

    let script = format!(
        "printf 'BIN=%s\\n' \"$(command -v steg)\" > '{out}'; steg pane show --json >> '{out}'",
        out = out_path.display()
    );
    let pane = new_pane_running(&socket, &["sh", "-c", &script]);
    wait_for_socket(&socket, Duration::from_secs(5));

    let waited = run_steg(
        &[
            "pane",
            "wait",
            &pane.path(),
            "--for",
            "exit",
            "--timeout",
            "20",
        ],
        &socket,
    );
    assert_eq!(
        waited.status.code(),
        Some(0),
        "pane wait should have seen the pane's own program exit: stderr was {}",
        stderr_of(&waited)
    );

    let dump = wait_for_output(&out_path, Duration::from_secs(5));

    let expected_bin_line = format!("BIN={}\n", steg_binary().display());
    assert!(
        dump.starts_with(&expected_bin_line),
        "expected the pane's own program to find the running server's own steg binary first \
         (AC #3); got {dump:?}"
    );

    let json_part = dump[expected_bin_line.len()..].trim();
    let reported: Value = serde_json::from_str(json_part)
        .unwrap_or_else(|err| panic!("pane show --json should print JSON: {err}; got {dump:?}"));
    assert_eq!(
        reported.get("id").and_then(Value::as_str),
        Some(pane.pane_id.as_str()),
        "the pane's own target-less pane show must resolve to itself (AC #1's pane clause)"
    );
}
