//! TASK-1.119 AC #1-#3: every exit status doc-2 section 6 names, each
//! produced through the real binary against a real server (or, for
//! statuses that need no server at all, against a throwaway socket path
//! nothing answers at), with the mapping read from the API description's
//! own `errors` table (`exit.rs`'s own unit tests prove that half
//! directly) - never a parallel table here. `--json` and its contrast
//! (AC #3), and a real SIGINT (status 130), are also here, since both are
//! part of the same exit-status story this file tells end to end.
//!
//! Every helper below is duplicated from `server_start.rs`/`idle_stop.rs`
//! rather than shared, the convention every file in this crate's `tests/`
//! already follows.

use std::io::{Read, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::process::{Child, Command, Output, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

#[path = "support/mod.rs"]
mod support;
use support::scoped_socket_path;

fn steg_binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_steg"))
}

/// Kills and reaps its child on drop, including when a panicking
/// assertion unwinds past it (`server_start.rs`'s own `KillOnDrop`).
struct KillOnDrop {
    child: Child,
    done: bool,
}

impl Drop for KillOnDrop {
    fn drop(&mut self) {
        if !self.done {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

/// Finds the pid holding `socket_path`'s arbiter lock open, via `lsof -t`
/// (`server_start.rs`'s own `find_server_pid`) - needed because a server
/// this test's own command *autostarts* is spawned and immediately
/// dropped by the sdk's own `connect()`, so no `Child` value here ever
/// refers to it directly.
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
/// open - the cleanup for a server this test's own command autostarted
/// indirectly (`server_start.rs`'s own `ServerAtSocket`). Never kills by
/// name - only the specific pid found holding this test's own throwaway
/// socket's lock.
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

fn wait_bounded(child: &mut Child, timeout: Duration) -> std::process::ExitStatus {
    let deadline = Instant::now() + timeout;
    loop {
        if let Some(status) = child.try_wait().expect("try_wait should not fail") {
            return status;
        }
        assert!(
            Instant::now() < deadline,
            "the process did not exit within the bound"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// Runs the real binary to completion against `socket`, with a minimal
/// explicit environment: `PATH` (so `/bin/cat` and the binary's own
/// dynamic loading resolve) and `STEG_SERVER`, nothing inherited -
/// this test's own process environment is never handed to a child
/// (SAFETY: this machine holds real credentials).
fn run_steg(args: &[&str], socket: &std::path::Path) -> Output {
    Command::new(steg_binary())
        .args(args)
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("STEG_SERVER", socket)
        .output()
        .expect("the real steg binary should run to completion")
}

fn run_steg_with_stdin(args: &[&str], socket: &std::path::Path, stdin: &str) -> Output {
    let mut child = Command::new(steg_binary())
        .args(args)
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("STEG_SERVER", socket)
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

fn stderr_of(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn stdout_of(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

// --- status 4: the server could not be reached or started -----------------

#[test]
fn no_autostart_with_nothing_running_exits_4() {
    let (_socket_dir, socket) = scoped_socket_path("no-autostart.sock");
    let output = run_steg(&["session", "list", "--no-autostart"], &socket);
    assert_eq!(
        output.status.code(),
        Some(4),
        "stderr: {}",
        stderr_of(&output)
    );
    assert!(stdout_of(&output).is_empty());
    assert!(
        UnixStream::connect(&socket).is_err(),
        "--no-autostart must never have started a server"
    );
}

// --- status 0, and AC #3 (--json, and its contrast) ------------------------

/// AC #1's success path and AC #3's own criterion in one: a real
/// autostarted server answers `session.list` as `[]`, printed unchanged
/// with `--json`; `session.new --json` then exercises `Returns::Identifier`'s
/// own JSON shape; a final `session.list --json` shows exactly the one
/// record created, with the API's own field names.
#[test]
fn a_real_round_trip_through_an_autostarted_server_prints_json_unchanged() {
    let (_socket_dir, socket) = scoped_socket_path("autostart-json.sock");
    let _server_guard = ServerAtSocket(socket.clone());

    let empty = run_steg(&["session", "list", "--json"], &socket);
    assert_eq!(
        empty.status.code(),
        Some(0),
        "stderr: {}",
        stderr_of(&empty)
    );
    assert_eq!(stdout_of(&empty).trim(), "[]");
    wait_for_socket(&socket, Duration::from_secs(5));

    let created = run_steg(
        &["session", "new", "--label", "exit-status-proof", "--json"],
        &socket,
    );
    assert_eq!(
        created.status.code(),
        Some(0),
        "stderr: {}",
        stderr_of(&created)
    );
    let created_value: serde_json::Value =
        serde_json::from_str(stdout_of(&created).trim()).expect("session.new prints JSON");
    assert!(
        created_value
            .get("id")
            .and_then(serde_json::Value::as_str)
            .is_some(),
        "session.new's own reply is {{\"id\": ...}} (Returns::Identifier)"
    );

    let listed = run_steg(&["session", "list", "--json"], &socket);
    assert_eq!(
        listed.status.code(),
        Some(0),
        "stderr: {}",
        stderr_of(&listed)
    );
    let records: Vec<serde_json::Value> =
        serde_json::from_str(stdout_of(&listed).trim()).expect("session.list prints a JSON array");
    assert_eq!(records.len(), 1, "exactly the one session created above");
    assert_eq!(
        records[0].get("label").and_then(serde_json::Value::as_str),
        Some("exit-status-proof")
    );
}

/// AC #3's own built-in contrast: the identical successful call, without
/// `--json`, prints nothing - true here only because the list is empty
/// (zero records, `Returns::Records`, renders as zero lines): once
/// TASK-1.120's own renderer exists, a *non-empty* `session list` prints
/// a real tab-separated line, so this is the degenerate case, not a
/// demonstration that plain-text rendering does nothing in general -
/// `cli_output_formats.rs`'s own tests are that proof. Extended below to
/// say so for real, rather than leaving a reader to assume general
/// "nothing" from an empty-list coincidence.
#[test]
fn without_json_a_successful_command_prints_nothing_on_stdout() {
    let (_socket_dir, socket) = scoped_socket_path("no-json-success.sock");
    let _server_guard = ServerAtSocket(socket.clone());

    let output = run_steg(&["session", "list"], &socket);
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr: {}",
        stderr_of(&output)
    );
    assert!(
        stdout_of(&output).is_empty(),
        "stdout: {:?}",
        stdout_of(&output)
    );
    assert!(stderr_of(&output).is_empty());

    // The non-empty case, so this test still earns its own name once
    // TASK-1.120's renderer exists: a session really does print a real
    // line, built from the same `--json` reply's own fields, in the
    // description's own declared order (`session_fields()`,
    // `crates/sdk/src/description/methods/session.rs`).
    let created = run_steg(
        &["session", "new", "--label", "plain-contrast", "--json"],
        &socket,
    );
    assert_eq!(
        created.status.code(),
        Some(0),
        "stderr: {}",
        stderr_of(&created)
    );
    // `session.new`'s own reply is `Returns::Identifier` - `{"id": ...}`
    // alone, never the full record - so the expected row is built from
    // `session.list`'s own reply instead (`Returns::Records`, every
    // field `session_fields()` declares), the same shape a real caller
    // would read it from.
    wait_for_socket(&socket, Duration::from_secs(5));
    let listed_json = run_steg(&["session", "list", "--json"], &socket);
    assert_eq!(
        listed_json.status.code(),
        Some(0),
        "stderr: {}",
        stderr_of(&listed_json)
    );
    let records: Vec<serde_json::Value> = serde_json::from_str(stdout_of(&listed_json).trim())
        .expect("session.list prints a JSON array");
    assert_eq!(records.len(), 1, "exactly the one session created above");
    let created_value = &records[0];

    let listed_plain = run_steg(&["session", "list"], &socket);
    assert_eq!(
        listed_plain.status.code(),
        Some(0),
        "stderr: {}",
        stderr_of(&listed_plain)
    );
    let field_text = |name: &str| match created_value.get(name) {
        None | Some(serde_json::Value::Null) => String::new(),
        Some(serde_json::Value::String(text)) => text.clone(),
        Some(other) => other.to_string(),
    };
    let expected_row = ["id", "label", "position", "colour", "current_tab"]
        .iter()
        .map(|name| field_text(name))
        .collect::<Vec<_>>()
        .join("\t");
    assert_eq!(stdout_of(&listed_plain), format!("{expected_row}\n"));
}

/// `--json` is never read as "also formats errors as JSON": an error
/// still prints nothing on stdout and its message on stderr, with the
/// mapped status, exactly as without `--json`.
#[test]
fn with_json_a_failed_command_still_prints_the_message_on_stderr_not_json() {
    let (_socket_dir, socket) = scoped_socket_path("json-error.sock");
    let _server_guard = ServerAtSocket(socket.clone());

    let output = run_steg(&["session", "show", "deadbeef00000000", "--json"], &socket);
    assert_eq!(
        output.status.code(),
        Some(3),
        "stderr: {}",
        stderr_of(&output)
    );
    assert!(stdout_of(&output).is_empty());
    assert!(!stderr_of(&output).is_empty());
}

// --- status 3: the target matched nothing ----------------------------------

#[test]
fn a_target_that_matches_nothing_exits_3() {
    let (_socket_dir, socket) = scoped_socket_path("no-such-target.sock");
    let _server_guard = ServerAtSocket(socket.clone());

    let output = run_steg(&["session", "show", "deadbeef00000000"], &socket);
    assert_eq!(
        output.status.code(),
        Some(3),
        "stderr: {}",
        stderr_of(&output)
    );
}

// --- status 2: bad usage ---------------------------------------------------

#[test]
fn a_value_outside_a_closed_set_exits_2_with_no_server_contacted() {
    let (_socket_dir, socket) = scoped_socket_path("bad-usage.sock");
    let listener = UnixListener::bind(&socket).expect("binding a throwaway listener");

    let output = run_steg(
        &["pane", "split", "--direction", "sideways", "--", "/bin/cat"],
        &socket,
    );
    assert_eq!(
        output.status.code(),
        Some(2),
        "stderr: {}",
        stderr_of(&output)
    );

    // Never actually reached for a connection: clap's own usage error is
    // refused before any of this task's own connecting code runs.
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(listener.accept().is_ok());
    });
    assert_eq!(
        rx.recv_timeout(Duration::from_millis(300)),
        Err(mpsc::RecvTimeoutError::Timeout),
        "a usage error should never attempt a connection"
    );
}

#[test]
fn malformed_stdin_on_api_call_exits_2() {
    let (_socket_dir, socket) = scoped_socket_path("api-call-malformed.sock");
    let output = run_steg_with_stdin(&["api", "call", "session.list"], &socket, "not json");
    assert_eq!(
        output.status.code(),
        Some(2),
        "stderr: {}",
        stderr_of(&output)
    );
    assert!(stdout_of(&output).is_empty());
}

/// A review finding on this task: stdin over the frame's own cap used
/// to reach `Connection::call_unvalidated`'s own `encode_frame(...)
/// .expect(...)` and panic (exit 101) instead of this command's own
/// documented bad-usage status. Oversized, never valid JSON either way
/// (irrelevant - the size check runs first, before any parsing), so the
/// server is never actually reached.
#[test]
fn oversized_stdin_on_api_call_exits_2_without_panicking() {
    let (_socket_dir, socket) = scoped_socket_path("api-call-oversized.sock");
    // Valid JSON (so it is the *size* check that must catch this, not
    // the earlier "is this even JSON" one) and comfortably over
    // MAX_PAYLOAD (8 MiB less a 5-byte header) without hand-copying that
    // constant here a second time.
    let oversized = format!(r#"{{"padding": "{}"}}"#, "x".repeat(9 * 1024 * 1024));
    let output = run_steg_with_stdin(&["api", "call", "session.list"], &socket, &oversized);
    assert_eq!(
        output.status.code(),
        Some(2),
        "stderr: {}",
        stderr_of(&output)
    );
    assert!(stdout_of(&output).is_empty());
}

// --- status 1: the command ran and failed, through real dispatch ----------

/// `ProgramRunning`, through real dispatch (not a stub that could land
/// under this task): a session with a program still running refuses to
/// close without `--force`. The program is named explicitly (`/bin/cat`),
/// never a bare shell - a started pane's shell comes from the account
/// database, not this process's `HOME`.
#[test]
fn a_program_still_running_refuses_close_with_status_1() {
    let (_socket_dir, socket) = scoped_socket_path("program-running.sock");
    let _server_guard = ServerAtSocket(socket.clone());

    // `session.new -- /bin/cat` mints the session and starts the program
    // for its own first pane in one call (`Runtime::start_and_dispatch`) -
    // an explicit program, never a bare shell, per this plan's own risk
    // note (a started pane's shell otherwise comes from the account
    // database, not this process's `HOME`).
    let created = run_steg(
        &[
            "session",
            "new",
            "--label",
            "status-1-proof",
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
        stderr_of(&created)
    );
    wait_for_socket(&socket, Duration::from_secs(5));
    let created_value: serde_json::Value =
        serde_json::from_str(stdout_of(&created).trim()).expect("session.new prints JSON");
    let session_id = created_value
        .get("id")
        .and_then(serde_json::Value::as_str)
        .expect("session.new's own reply carries an id")
        .to_owned();

    let close = run_steg(&["session", "close", session_id.as_str()], &socket);
    assert_eq!(
        close.status.code(),
        Some(1),
        "stderr: {}",
        stderr_of(&close)
    );
    assert!(stdout_of(&close).is_empty());

    // Cleanup, forced - never leave a program running behind this test.
    let forced = run_steg(
        &["session", "close", session_id.as_str(), "--force"],
        &socket,
    );
    assert_eq!(
        forced.status.code(),
        Some(0),
        "stderr: {}",
        stderr_of(&forced)
    );
}

// --- status 130: interrupted ------------------------------------------------

/// Reads off `stream` until a complete hello frame is decoded (the
/// server-side half of a handshake this fixture plays for real) - the
/// same shape `crates/sdk/src/local_socket.rs`'s own test module uses,
/// duplicated here per this crate's convention.
fn read_one_hello(stream: &mut UnixStream) -> Vec<u8> {
    use steg_sdk::{Decoder, Kind};
    let mut decoder = Decoder::new();
    let mut buf = [0_u8; 4096];
    loop {
        match decoder.next_frame() {
            Ok(Some(frame)) if frame.kind == Kind::Hello => return frame.payload,
            Ok(Some(_)) => continue,
            Ok(None) => {}
            Err(err) => panic!("malformed hello: {err}"),
        }
        let n = stream.read(&mut buf).expect("reading the client's hello");
        assert_ne!(n, 0, "the client closed before sending a hello");
        decoder.push(&buf[..n]);
    }
}

/// A hostile-but-cooperative fixture: completes a real hello (so the real
/// `steg` binary gets past the handshake - the one place in today's build
/// nothing bounds the wait, the reply read) and then never answers the
/// call at all. On its own thread; the listener is dropped, and the test
/// process ends, once the test itself returns.
fn spawn_hello_then_silent_fixture(path: PathBuf) -> mpsc::Receiver<()> {
    use steg_sdk::{ClientId, Connection};
    let (ready_tx, ready_rx) = mpsc::channel();
    std::thread::spawn(move || {
        let listener = UnixListener::bind(&path).expect("binding the test listener");
        ready_tx.send(()).expect("the test is still waiting");
        let Ok((mut stream, _)) = listener.accept() else {
            return;
        };
        let payload = read_one_hello(&mut stream);
        let (_, bytes) = Connection::accept_or_refuse(
            &payload,
            "steg-test-server",
            "1.0",
            ClientId::from_raw(1),
        );
        let _ = stream.write_all(&bytes);
        // Never answers anything further - the client's own call read
        // blocks here with nothing to wake it but a signal.
        std::thread::sleep(Duration::from_secs(60));
    });
    ready_rx
}

/// Doc-2's "130 | Interrupted": with no SIGINT handler installed (this
/// task's own point 8, taken), an interrupted `steg` dies by the signal -
/// what a shell renders as 130 (128 + 2). Proved as the real, observable
/// fact rather than assumed: `ExitStatus::signal() == Some(libc::SIGINT)`.
#[test]
fn sigint_during_a_hung_call_ends_the_process_by_signal() {
    let (_socket_dir, socket) = scoped_socket_path("sigint.sock");
    let ready = spawn_hello_then_silent_fixture(socket.clone());
    ready
        .recv_timeout(Duration::from_secs(2))
        .expect("the test fixture should bind within the bound");

    let mut child = KillOnDrop {
        child: Command::new(steg_binary())
            .args(["session", "list"])
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .env("STEG_SERVER", &socket)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("the real steg binary should spawn"),
        done: false,
    };

    // Confirm it is genuinely still running - hung on the call's own
    // reply, which the fixture above never sends - before signalling it.
    std::thread::sleep(Duration::from_millis(300));
    assert!(
        child
            .child
            .try_wait()
            .expect("try_wait should not fail")
            .is_none(),
        "the client should still be hung waiting for a reply"
    );

    let status = Command::new("kill")
        .args(["-s", "INT", &child.child.id().to_string()])
        .status()
        .expect("sending SIGINT should succeed");
    assert!(status.success(), "sending SIGINT should succeed");

    let exit_status = wait_bounded(&mut child.child, Duration::from_secs(5));
    child.done = true;

    use std::os::unix::process::ExitStatusExt;
    assert_eq!(
        exit_status.signal(),
        Some(2), // SIGINT
        "the process should have been ended by SIGINT, not exited on its own"
    );
    assert_eq!(exit_status.code(), None);
}
