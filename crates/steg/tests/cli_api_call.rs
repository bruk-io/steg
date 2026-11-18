//! TASK-1.119 AC #5: `api call`'s own parameters come from standard
//! input, as one JSON object, merged flat with the called method's own
//! names - never nested under a second key. `crates/sdk/src/wire/
//! connection.rs`'s own `call_unvalidated` unit test already proves the
//! mechanism directly (an unvalidated request really does carry both the
//! selector and the extra keys); this file proves the real effect
//! through the real binary, and the stated exception that `api call`'s
//! own output is always JSON, never gated on `--json`.

use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

fn unique_socket_path(label: &str) -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let suffix = COUNTER.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!("{}-{label}-{suffix}.sock", std::process::id()))
}

fn steg_binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_steg"))
}

fn run_steg(args: &[&str], socket: &std::path::Path) -> Output {
    Command::new(steg_binary())
        .args(args)
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("STEG_SOCKET_PATH", socket)
        .output()
        .expect("the real steg binary should run to completion")
}

fn run_steg_with_stdin(args: &[&str], socket: &std::path::Path, stdin: &str) -> Output {
    let mut child = Command::new(steg_binary())
        .args(args)
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("STEG_SOCKET_PATH", socket)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the real steg binary should spawn");
    child
        .stdin
        .take()
        .expect("stdin was piped")
        .write_all(stdin.as_bytes())
        .expect("writing to the child's stdin");
    child
        .wait_with_output()
        .expect("the real steg binary should run to completion")
}

fn json_field(output: &Output, field: &str) -> Option<String> {
    let value: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("valid JSON on stdout");
    value
        .get(field)
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned)
}

/// Kills, on drop, whatever process `lsof` finds holding `socket_path`'s
/// arbiter lock open (duplicated from `cli_exit_statuses.rs`, per this
/// crate's own convention).
struct ServerAtSocket(PathBuf);

impl Drop for ServerAtSocket {
    fn drop(&mut self) {
        let mut lock_path = self.0.as_os_str().to_owned();
        lock_path.push(".lock");
        let Ok(output) = Command::new("lsof").arg("-t").arg(&lock_path).output() else {
            return;
        };
        let text = String::from_utf8_lossy(&output.stdout);
        for pid in text.lines().map(str::trim).filter(|line| !line.is_empty()) {
            let _ = Command::new("kill").args(["-9", pid]).status();
        }
    }
}

/// The real effect: `api call pane.split` with `{"target": ..., "direction":
/// "right"}` on stdin actually splits the named pane in the named
/// direction - the JSON on stdin reached `pane.split`'s own `target` and
/// `direction` parameters flat, not nested under a second key, because a
/// nested shape would leave `pane.split` with no `target`/`direction` at
/// all and either fail outright or do nothing observable.
#[test]
fn api_call_sends_stdin_params_flat_to_the_named_method() {
    let socket = unique_socket_path("api-call-effect");
    let _server_guard = ServerAtSocket(socket.clone());

    let created = run_steg(
        &[
            "session",
            "new",
            "--label",
            "api-call-proof",
            "--json",
            "--",
            "/bin/cat",
        ],
        &socket,
    );
    assert_eq!(
        created.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&created.stderr)
    );
    let session_id = json_field(&created, "id").expect("session.new carries an id");

    let tabs = run_steg(&["tab", "list", session_id.as_str(), "--json"], &socket);
    assert_eq!(
        tabs.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&tabs.stderr)
    );
    let tab_records: Vec<serde_json::Value> =
        serde_json::from_slice(&tabs.stdout).expect("tab.list prints a JSON array");
    let tab_id = tab_records[0]
        .get("id")
        .and_then(serde_json::Value::as_str)
        .expect("the first tab has an id")
        .to_owned();
    // doc-2 section 4: a bare, one-segment target is read as "in the
    // current session", which this connection has none of - the full
    // `session/tab` path is what a connection with no context must give.
    let tab_target = format!("{session_id}/{tab_id}");

    let panes_before = run_steg(&["pane", "list", tab_target.as_str(), "--json"], &socket);
    assert_eq!(
        panes_before.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&panes_before.stderr)
    );
    let panes_before: Vec<serde_json::Value> =
        serde_json::from_slice(&panes_before.stdout).expect("pane.list prints a JSON array");
    assert_eq!(panes_before.len(), 1, "sanity: one pane before the split");
    let pane_id = panes_before[0]
        .get("id")
        .and_then(serde_json::Value::as_str)
        .expect("the first pane has an id")
        .to_owned();
    let pane_target = format!("{tab_target}/{pane_id}");

    let stdin = format!(r#"{{"target": "{pane_target}", "direction": "right"}}"#);
    let call = run_steg_with_stdin(&["api", "call", "pane.split"], &socket, &stdin);
    assert_eq!(
        call.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&call.stderr)
    );

    let panes_after = run_steg(&["pane", "list", tab_target.as_str(), "--json"], &socket);
    let panes_after: Vec<serde_json::Value> =
        serde_json::from_slice(&panes_after.stdout).expect("pane.list prints a JSON array");
    assert_eq!(
        panes_after.len(),
        2,
        "the split named by stdin's own flat params should have actually happened"
    );

    // Cleanup - never leave a program running behind this test.
    let _ = run_steg(
        &["session", "close", session_id.as_str(), "--force"],
        &socket,
    );
}

/// The stated exception (doc-2 section 3): `api call`'s own output is
/// JSON by its nature, with no `--json` given at all.
#[test]
fn api_call_prints_json_with_no_json_flag_given() {
    let socket = unique_socket_path("api-call-exception");
    let _server_guard = ServerAtSocket(socket.clone());

    let output = run_steg_with_stdin(&["api", "call", "session.list"], &socket, "{}");
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let parsed: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("api call prints JSON with no --json given");
    assert!(parsed.is_array(), "session.list's own reply is an array");
}
