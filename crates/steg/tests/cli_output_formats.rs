//! TASK-1.120, criteria #1-#3: plain text, `--json` and `-q`/`--quiet`,
//! for every kind of result the API description declares, through the
//! real binary against a real, autostarted server over a real socket -
//! nothing doubled. `output.rs`'s own unit tests already prove every
//! case as a pure function (hand-built `Returns`/`Value`, no process);
//! this file proves the same renderer is actually wired in, end to end.
//!
//! Every helper below is duplicated from `cli_exit_statuses.rs`/
//! `server_start.rs` rather than shared, the convention every file in
//! this crate's `tests/` already follows - except the two bounded-wait
//! helpers, moved into `support/mod.rs` for this file to use rather than
//! duplicated a third time (TASK-1.120's own instruction).
//!
//! `Returns::Nested`'s own two described methods, `pane.attach` and
//! `event.watch`: `pane.attach` is real dispatch today (TASK-1.47,
//! `connection.rs`'s own `"pane.attach"` arm) and is exercised here for
//! real; `event.watch` still answers `ErrorCode::NotImplementedYet`
//! (`crates/server/src/dispatch/mod.rs`'s own `NOT_YET_NOUNS`), so it is
//! not - named here plainly rather than silently left out.

use std::path::PathBuf;
use std::process::{Command, Output, Stdio};
use std::time::Duration;

#[path = "support/mod.rs"]
mod support;
use support::{ScopedTempDir, run_bounded, wait_for_socket};

fn steg_binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_steg"))
}

/// Finds the pid holding `socket_path`'s arbiter lock open
/// (`cli_exit_statuses.rs`'s own `find_server_pid`) - needed because a
/// server this test's own command *autostarts* is spawned and
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

/// Kills, on drop, whatever process `lsof` finds holding this test's own
/// throwaway socket open - never kills by name, only the specific pid
/// found holding this exact socket's lock.
struct ServerAtSocket(PathBuf);

impl Drop for ServerAtSocket {
    fn drop(&mut self) {
        if let Some(pid) = find_server_pid(&self.0) {
            let _ = Command::new("kill").args(["-9", &pid.to_string()]).status();
        }
    }
}

/// Runs the real binary to completion against `socket`, with a minimal
/// explicit environment - never this test's own process environment
/// (SAFETY: this machine holds real credentials).
fn run_steg(args: &[&str], socket: &std::path::Path) -> Output {
    run_bounded(
        &format!("steg {}", args.join(" ")),
        Duration::from_secs(10),
        {
            let args: Vec<String> = args.iter().map(|arg| (*arg).to_owned()).collect();
            let socket = socket.to_owned();
            move || {
                Command::new(steg_binary())
                    .args(&args)
                    .env_clear()
                    .env("PATH", "/usr/bin:/bin")
                    .env("STEG_SOCKET_PATH", &socket)
                    .output()
                    .expect("the real steg binary should run to completion")
            }
        },
    )
}

fn stderr_of(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn stdout_of(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn assert_success(output: &Output) {
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr: {}",
        stderr_of(output)
    );
}

/// `session/tab/pane`, all three parts explicit - a bare pane id alone
/// is refused (`crates/server/src/dispatch/lookup.rs::resolve_pane_str`:
/// a single-segment target always needs a session *and* a tab from
/// context, even when the id itself is already globally unique, and
/// this CLI invocation has no context at all - found by hand, running
/// the real binary, before writing this helper). Reads the session's
/// own first tab (`tab list`'s first and, for every session this file
/// creates, only element) and that tab's own `current_pane`.
fn full_pane_path(socket: &std::path::Path, session_id: &str) -> String {
    let tabs = run_steg(&["tab", "list", session_id, "--json"], socket);
    assert_success(&tabs);
    let tabs_array = json_of(&tabs);
    let tab = tabs_array
        .as_array()
        .and_then(|tabs| tabs.first())
        .expect("a session this file creates always has its own first tab");
    let tab_id = tab
        .get("id")
        .and_then(serde_json::Value::as_str)
        .expect("tab_fields declares \"id\"");
    let pane_id = tab
        .get("current_pane")
        .and_then(serde_json::Value::as_str)
        .expect("the new session's first tab names its current pane");
    format!("{session_id}/{tab_id}/{pane_id}")
}

fn json_of(output: &Output) -> serde_json::Value {
    serde_json::from_str(stdout_of(output).trim()).unwrap_or_else(|err| {
        panic!(
            "expected valid JSON on stdout, got {:?}: {err}",
            stdout_of(output)
        )
    })
}

/// `session_fields()`'s own declared order (`crates/sdk/src/description/
/// methods/session.rs`) - hardcoded here as what the description says,
/// the same way `cli_api_call.rs`'s own existing tests already parse
/// specific field names, never re-derived from the running binary.
const SESSION_FIELD_NAMES: &[&str] = &["id", "label", "position", "colour", "current_tab"];

/// Builds the plain-text row this task's renderer is expected to
/// produce for one record, from its own parsed `--json` object -
/// derived from the real reply, not a value hand-typed in this test
/// (which would make the colour kernel assigns, for instance, this
/// test's own assumption rather than a fact it checked).
fn expected_plain_row(object: &serde_json::Value, field_names: &[&str]) -> String {
    field_names
        .iter()
        .map(|name| match object.get(*name) {
            None | Some(serde_json::Value::Null) => String::new(),
            Some(serde_json::Value::String(text)) => text.clone(),
            Some(other) => other.to_string(),
        })
        .collect::<Vec<_>>()
        .join("\t")
}

// --- Identifier --------------------------------------------------------------

/// `-q` is defined as "unchanged" for `Identifier` (decision 5's own
/// reasoning applied to `Identifier` too - doc-2 section 5's own
/// wording), so all four combinations of `--json`/`-q` collapse to two
/// distinct outputs: the bare id, or the `{"id": ...}` object.
#[test]
fn identifier_renders_the_bare_id_plain_and_is_unchanged_by_quiet() {
    let dir = ScopedTempDir::new();
    let socket = dir.path("sock");
    let _guard = ServerAtSocket(socket.clone());

    let plain = run_steg(&["session", "new", "--label", "ident-plain"], &socket);
    assert_success(&plain);
    let bare_id = stdout_of(&plain).trim().to_owned();
    assert!(!bare_id.is_empty(), "expected a bare id alone on a line");
    assert!(
        !bare_id.contains('{'),
        "expected the bare id, not a JSON object: {bare_id:?}"
    );
    wait_for_socket(&socket, Duration::from_secs(5));

    let quiet = run_steg(&["session", "new", "--label", "ident-quiet", "-q"], &socket);
    assert_success(&quiet);
    let quiet_id = stdout_of(&quiet).trim().to_owned();
    assert!(
        !quiet_id.contains('{'),
        "-q should still print the bare id, unchanged: {quiet_id:?}"
    );

    let as_json = run_steg(
        &["session", "new", "--label", "ident-json", "--json"],
        &socket,
    );
    assert_success(&as_json);
    let json_value = json_of(&as_json);
    assert!(
        json_value
            .get("id")
            .and_then(serde_json::Value::as_str)
            .is_some(),
        "session.new's own reply with --json is {{\"id\": ...}}, got {json_value}"
    );

    // A separate session (a fresh, unrelated id - `session.new` never
    // returns the same one twice), so this only checks `--json -q`'s own
    // *shape* against itself, never a byte-for-byte match against the
    // call above's own different id.
    let quiet_and_json = run_steg(
        &["session", "new", "--label", "ident-both", "--json", "-q"],
        &socket,
    );
    assert_success(&quiet_and_json);
    let quiet_and_json_value = json_of(&quiet_and_json);
    assert!(
        quiet_and_json_value
            .get("id")
            .and_then(serde_json::Value::as_str)
            .is_some(),
        "-q together with --json should still be the full {{\"id\": ...}} object, unchanged \
         (decision 5's own reasoning), got {quiet_and_json_value}"
    );
    assert_eq!(
        quiet_and_json_value.as_object().map(|object| object.len()),
        json_value.as_object().map(|object| object.len()),
        "-q must not drop or add fields to --json's own object shape for Identifier"
    );
}

// --- Records -----------------------------------------------------------------

#[test]
fn records_render_plain_json_and_quiet_through_a_real_server() {
    let dir = ScopedTempDir::new();
    let socket = dir.path("sock");
    let _guard = ServerAtSocket(socket.clone());

    let first = run_steg(&["session", "new", "--label", "first", "--json"], &socket);
    assert_success(&first);
    wait_for_socket(&socket, Duration::from_secs(5));
    let second = run_steg(&["session", "new", "--label", "second", "--json"], &socket);
    assert_success(&second);

    let listed_json = run_steg(&["session", "list", "--json"], &socket);
    assert_success(&listed_json);
    let records = json_of(&listed_json);
    let records = records
        .as_array()
        .expect("session.list prints a JSON array");
    assert_eq!(records.len(), 2, "exactly the two sessions created above");

    let expected_plain = records
        .iter()
        .map(|object| expected_plain_row(object, SESSION_FIELD_NAMES))
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    let expected_ids: Vec<&str> = records
        .iter()
        .map(|object| {
            object
                .get("id")
                .and_then(serde_json::Value::as_str)
                .expect("an id")
        })
        .collect();

    let plain = run_steg(&["session", "list"], &socket);
    assert_success(&plain);
    assert_eq!(stdout_of(&plain), expected_plain);

    let quiet = run_steg(&["session", "list", "-q"], &socket);
    assert_success(&quiet);
    assert_eq!(stdout_of(&quiet), expected_ids.join("\n") + "\n");

    let quiet_and_json = run_steg(&["session", "list", "-q", "--json"], &socket);
    assert_success(&quiet_and_json);
    let as_array: Vec<String> = json_of(&quiet_and_json)
        .as_array()
        .expect("a JSON array")
        .iter()
        .map(|value| value.as_str().expect("a string id").to_owned())
        .collect();
    assert_eq!(as_array, expected_ids);
}

// --- Record --------------------------------------------------------------------

#[test]
fn record_renders_plain_json_and_quiet_through_a_real_server() {
    let dir = ScopedTempDir::new();
    let socket = dir.path("sock");
    let _guard = ServerAtSocket(socket.clone());

    let created = run_steg(&["session", "new", "--label", "show-me", "--json"], &socket);
    assert_success(&created);
    let session_id = json_of(&created)
        .get("id")
        .and_then(serde_json::Value::as_str)
        .expect("an id")
        .to_owned();
    wait_for_socket(&socket, Duration::from_secs(5));

    let shown_json = run_steg(&["session", "show", &session_id, "--json"], &socket);
    assert_success(&shown_json);
    let object = json_of(&shown_json);
    let expected_plain = expected_plain_row(&object, SESSION_FIELD_NAMES);

    let plain = run_steg(&["session", "show", &session_id], &socket);
    assert_success(&plain);
    assert_eq!(stdout_of(&plain), format!("{expected_plain}\n"));

    // `-q` on a record reduces to `fields[0]` (decision 4) - `"id"`,
    // since `session_fields()` declares it first.
    let quiet = run_steg(&["session", "show", &session_id, "-q"], &socket);
    assert_success(&quiet);
    assert_eq!(stdout_of(&quiet), format!("{session_id}\n"));
}

/// Decision 6, through the real binary: a session's own `label`
/// (`crates/kernel/src/label.rs` bans only `/` and an all-digits label -
/// nothing about a tab, newline or carriage return) carries a literal
/// `\t`/`\n`/`\r` all the way from `session new --label` to `session
/// show`'s own plain-text row, where each becomes a single space;
/// `--json` keeps the exact value, found by hand first (`xxd` on the
/// real binary's own output) before writing this down as a test rather
/// than assumed from `output.rs`'s own unit tests alone.
#[test]
fn a_label_containing_a_tab_newline_and_carriage_return_is_sanitised_in_plain_text_only() {
    let dir = ScopedTempDir::new();
    let socket = dir.path("sock");
    let _guard = ServerAtSocket(socket.clone());

    let label = "a\tb\nc\rd";
    let created = run_steg(&["session", "new", "--label", label, "--json"], &socket);
    assert_success(&created);
    let session_id = json_of(&created)
        .get("id")
        .and_then(serde_json::Value::as_str)
        .expect("an id")
        .to_owned();
    wait_for_socket(&socket, Duration::from_secs(5));

    let shown_json = run_steg(&["session", "show", &session_id, "--json"], &socket);
    assert_success(&shown_json);
    assert_eq!(
        json_of(&shown_json)
            .get("label")
            .and_then(serde_json::Value::as_str),
        Some(label),
        "--json must keep the label's exact value, every control character included"
    );

    let plain = run_steg(&["session", "show", &session_id], &socket);
    assert_success(&plain);
    // Computed independently of `output.rs`'s own `sanitize_for_plain_text`
    // (never calling it), the same way `expected_plain_row` above never
    // calls the renderer's own `scalar_text`: this is what a column
    // *should* read, not a restatement of what the code under test does.
    let sanitised_label: String = label
        .chars()
        .map(|character| {
            if character == '\t' || character == '\n' || character == '\r' {
                ' '
            } else {
                character
            }
        })
        .collect();
    let expected_row = format!("{session_id}\t{sanitised_label}\t1\tviolet\t");
    let plain_text = stdout_of(&plain);
    assert!(
        plain_text.starts_with(&expected_row),
        "expected a row starting with {expected_row:?}, got {plain_text:?}"
    );
}

// --- Nothing -------------------------------------------------------------------

/// Decision 3, as a regression guard through the real binary (not only
/// `output.rs`'s own unit test): an action's reply prints nothing with
/// `--json` too, not the server's raw JSON reply (`null`, for a no-result
/// outcome) the way the pre-this-task code accidentally did - no test
/// ever locked that in, so this is the first.
#[test]
fn nothing_prints_nothing_even_with_json_through_a_real_server() {
    let dir = ScopedTempDir::new();
    let socket = dir.path("sock");
    let _guard = ServerAtSocket(socket.clone());

    let created = run_steg(
        &["session", "new", "--label", "to-close", "--json"],
        &socket,
    );
    assert_success(&created);
    let session_id = json_of(&created)
        .get("id")
        .and_then(serde_json::Value::as_str)
        .expect("an id")
        .to_owned();
    wait_for_socket(&socket, Duration::from_secs(5));

    // `--force`: a plain `session new` with no program defaults to a
    // login shell, which is itself a running program - `--force` is
    // orthogonal to this test's own point (`Returns::Nothing`'s own
    // rendering), not something it is testing.
    let closed_json = run_steg(
        &["session", "close", &session_id, "--force", "--json"],
        &socket,
    );
    assert_success(&closed_json);
    assert!(
        stdout_of(&closed_json).is_empty(),
        "stdout: {:?}",
        stdout_of(&closed_json)
    );

    let closed_quiet_json = run_steg(
        &["session", "new", "--label", "to-close-2", "--json"],
        &socket,
    );
    assert_success(&closed_quiet_json);
    let second_id = json_of(&closed_quiet_json)
        .get("id")
        .and_then(serde_json::Value::as_str)
        .expect("an id")
        .to_owned();
    let closed_both = run_steg(
        &["session", "close", &second_id, "--force", "--json", "-q"],
        &socket,
    );
    assert_success(&closed_both);
    assert!(stdout_of(&closed_both).is_empty());
}

// --- Text ------------------------------------------------------------------------

/// `pane.capture`'s own screen text, decoupled from its exact content
/// (which this test has no control over - a freshly started `/bin/cat`
/// pane's own blank screen, as `ghostty-vt` represents it): plain and
/// `--json` must agree with each other once re-parsed, and `-q` must not
/// change either (decision 5).
#[test]
fn text_renders_unaltered_plain_and_as_a_json_string() {
    let dir = ScopedTempDir::new();
    let socket = dir.path("sock");
    let _guard = ServerAtSocket(socket.clone());

    let created = run_steg(
        &[
            "session",
            "new",
            "--label",
            "capture-me",
            "--json",
            "--",
            "/bin/cat",
        ],
        &socket,
    );
    assert_success(&created);
    let session_id = json_of(&created)
        .get("id")
        .and_then(serde_json::Value::as_str)
        .expect("an id")
        .to_owned();
    wait_for_socket(&socket, Duration::from_secs(5));

    let pane_path = full_pane_path(&socket, &session_id);

    let plain = run_steg(&["pane", "capture", &pane_path], &socket);
    assert_success(&plain);
    let plain_text = stdout_of(&plain);

    let as_json = run_steg(&["pane", "capture", &pane_path, "--json"], &socket);
    assert_success(&as_json);
    let json_text = json_of(&as_json)
        .as_str()
        .expect("Returns::Text's own reply is a JSON string")
        .to_owned();
    assert_eq!(
        plain_text, json_text,
        "plain text must be the string unaltered"
    );

    let quiet = run_steg(&["pane", "capture", &pane_path, "-q"], &socket);
    assert_success(&quiet);
    assert_eq!(
        stdout_of(&quiet),
        plain_text,
        "-q must leave Text unchanged (decision 5)"
    );

    let forced = run_steg(&["session", "close", &session_id, "--force"], &socket);
    assert_success(&forced);
}

// --- Nested ------------------------------------------------------------------------

/// Decision 1, through the real binary and real dispatch: `pane.attach`
/// is real today (TASK-1.47), so its own `Returns::Nested` reply
/// (`PaneAttached`: the new stream's id and the pane's opening snapshot)
/// is available to compare for real, not only in `output.rs`'s own unit
/// tests. Plain text and `--json` must be the same bytes; `-q` must not
/// change either.
#[test]
fn nested_pane_attach_renders_as_json_unconditionally_through_a_real_server() {
    let dir = ScopedTempDir::new();
    let socket = dir.path("sock");
    let _guard = ServerAtSocket(socket.clone());

    let created = run_steg(
        &[
            "session",
            "new",
            "--label",
            "attach-me",
            "--json",
            "--",
            "/bin/cat",
        ],
        &socket,
    );
    assert_success(&created);
    let session_id = json_of(&created)
        .get("id")
        .and_then(serde_json::Value::as_str)
        .expect("an id")
        .to_owned();
    wait_for_socket(&socket, Duration::from_secs(5));

    let pane_path = full_pane_path(&socket, &session_id);

    let plain = run_steg(&["pane", "attach", &pane_path], &socket);
    assert_success(&plain);
    let as_json = run_steg(&["pane", "attach", &pane_path, "--json"], &socket);
    assert_success(&as_json);
    assert_eq!(
        plain.stdout, as_json.stdout,
        "Returns::Nested must be the same bytes plain or with --json (decision 1)"
    );

    let quiet = run_steg(&["pane", "attach", &pane_path, "-q"], &socket);
    assert_success(&quiet);
    assert_eq!(
        quiet.stdout, plain.stdout,
        "-q must leave Nested unchanged, the same reasoning as Text"
    );

    let forced = run_steg(&["session", "close", &session_id, "--force"], &socket);
    assert_success(&forced);
}

// --- -q on a record with no "id" field (decision 4) ---------------------------------

/// `setting_fields()` declares `["name", "value", "default"]`, never
/// `"id"` (`crates/sdk/src/description/methods/setting.rs`) - decision
/// 4's own distinguishing case, through the real binary rather than only
/// `output.rs`'s own hand-built unit test.
///
/// Not `api list`: checked directly first (running the real binary) and
/// found that `api.list`'s own *real* reply is the raw `Method` struct
/// array (`name: {noun, verb}`, ...), never the flat `{"method": ...,
/// "summary": ...}` shape `api_method_fields()` declares - the
/// description's own module doc (`methods/api.rs`) already names this as
/// a deliberate, pre-existing exception ("the one place... a served
/// method's actual JSON has more keys than its declared fields...
/// exempted from the fields-versus-type cross-check for exactly that
/// reason"), not something this task's renderer is wrong to read
/// literally off `fields`. `setting list`'s own reply has no such
/// exception (checked the same way: its real JSON is exactly
/// `{"name", "value", "default"}`), so it is the real test of decision
/// 4 that does not also depend on fixing that pre-existing, unrelated
/// gap.
#[test]
fn quiet_with_no_id_field_prints_the_first_declared_field() {
    let dir = ScopedTempDir::new();
    let socket = dir.path("sock");
    let _guard = ServerAtSocket(socket.clone());

    let as_json = run_steg(&["setting", "list", "--json"], &socket);
    assert_success(&as_json);
    let settings = json_of(&as_json);
    let settings = settings
        .as_array()
        .expect("setting.list prints a JSON array");
    assert!(
        !settings.is_empty(),
        "sanity: the server has at least one setting"
    );
    let expected: Vec<&str> = settings
        .iter()
        .map(|object| {
            object
                .get("name")
                .and_then(serde_json::Value::as_str)
                .expect("setting_fields declares \"name\" first")
        })
        .collect();

    wait_for_socket(&socket, Duration::from_secs(5));
    let quiet = run_steg(&["setting", "list", "-q"], &socket);
    assert_success(&quiet);
    let quiet_text = stdout_of(&quiet);
    let printed: Vec<&str> = quiet_text.lines().collect();
    assert_eq!(printed, expected);
}

// --- Criterion #2: byte-identical through a real pty and a pipe ----------------------

/// Measures the fixed, constant number of bytes this machine's own
/// `script(1)` writes to the pty before a child's first byte, even with
/// `-q`/`-e` and the pty already in raw mode (confirmed by hand before
/// writing this test: identical for `/bin/echo`, `/bin/echo` with no
/// output, and `true`) - never hard-coded, since it is not promised to
/// stay the same length on a different `script` build. Called twice by
/// the one test below, asserting the two measurements agree, so a flaky
/// measurement fails loudly rather than silently mis-slicing.
fn script_preamble_len(dir: &ScopedTempDir, label: &str) -> usize {
    let calibration = dir.path(label);
    let status = run_bounded("a script calibration call", Duration::from_secs(10), {
        let calibration = calibration.clone();
        move || {
            Command::new("script")
                .args([
                    "-q",
                    "-e",
                    calibration.to_str().expect("a UTF-8 path"),
                    "sh",
                    "-c",
                    "stty raw -echo; true",
                ])
                .env_clear()
                .env("PATH", "/usr/bin:/bin")
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .expect("script should run for calibration")
        }
    });
    assert!(status.success(), "the calibration call itself must succeed");
    let bytes = std::fs::read(&calibration).expect("reading the calibration typescript");
    assert!(
        !bytes.is_empty(),
        "expected a non-empty, deterministic preamble from this machine's own `script`"
    );
    bytes.len()
}

/// Runs `steg` with `args` under a real pty (`script(1)`, in raw/no-echo
/// mode - see `script_preamble_len`'s own doc), against `socket`, and
/// returns its captured bytes with the measured preamble stripped.
fn run_steg_via_pty(args: &[&str], socket: &std::path::Path, dir: &ScopedTempDir) -> Vec<u8> {
    let preamble_len = script_preamble_len(dir, "calibration-a");
    assert_eq!(
        preamble_len,
        script_preamble_len(dir, "calibration-b"),
        "the preamble's own length must be the same twice in a row, or this test's own \
         calibration cannot be trusted"
    );

    let typescript = dir.path("typescript");
    let mut script_args: Vec<String> = vec![
        "-q".into(),
        "-e".into(),
        typescript.to_str().expect("a UTF-8 path").to_owned(),
        "sh".into(),
        "-c".into(),
        "stty raw -echo; exec \"$@\"".into(),
        "sh".into(),
        steg_binary().to_str().expect("a UTF-8 path").to_owned(),
    ];
    script_args.extend(args.iter().map(|arg| (*arg).to_owned()));

    let status = run_bounded(
        "steg under a pty, via script(1)",
        Duration::from_secs(10),
        {
            let script_args = script_args.clone();
            let socket = socket.to_owned();
            move || {
                Command::new("script")
                    .args(&script_args)
                    .env_clear()
                    .env("PATH", "/usr/bin:/bin")
                    .env("STEG_SOCKET_PATH", &socket)
                    .stdin(Stdio::null())
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .status()
                    .expect("script should run")
            }
        },
    );
    assert!(status.success(), "steg, run under script(1), should exit 0");

    let captured = std::fs::read(&typescript).expect("reading the typescript");
    assert!(
        captured.len() >= preamble_len,
        "the typescript ({} bytes) is shorter than its own measured preamble ({preamble_len} \
         bytes) - something about this run did not match the calibration",
        captured.len()
    );
    captured[preamble_len..].to_vec()
}

/// Criterion #2: the real built binary, under a real OS-allocated pty
/// (`script(1)`'s own child runs on a pty slave, never a pipe or an
/// in-process fake), against a real server over a real Unix socket - a
/// non-empty list (an empty one's bytes are indistinguishable from
/// "nothing rendered", too weak a proof - the same reason
/// `cli_exit_statuses.rs`'s own empty-list test isn't strong enough
/// either), compared byte for byte against the identical command piped.
/// Kept to this one test, per the engineer's own instruction.
#[test]
fn a_non_empty_list_is_byte_identical_through_a_pty_and_a_pipe() {
    let dir = ScopedTempDir::new();
    let socket = dir.path("sock");
    let _guard = ServerAtSocket(socket.clone());

    let first = run_steg(&["session", "new", "--label", "pty-one"], &socket);
    assert_success(&first);
    wait_for_socket(&socket, Duration::from_secs(5));
    let second = run_steg(&["session", "new", "--label", "pty-two"], &socket);
    assert_success(&second);

    let piped = run_steg(&["session", "list"], &socket);
    assert_success(&piped);
    assert!(
        !piped.stdout.is_empty(),
        "sanity: the list must be non-empty for this comparison to mean anything"
    );

    let via_pty = run_steg_via_pty(&["session", "list"], &socket, &dir);
    assert_eq!(
        via_pty, piped.stdout,
        "the CLI's own bytes must be identical whether stdout is a pty or a pipe (doc-2 \
         section 6)"
    );
}

// Negative control for the test above, run by hand and reverted before
// committing (not a `#[test]` left in the tree): with the pty side's own
// `stty raw -echo` step removed from `run_steg_via_pty`'s command
// string, this same test fails - the pty's own line discipline turns
// every `\n` the binary writes into `\r\n` and nothing strips that back
// out, so `via_pty != piped.stdout`. Confirms the raw-mode step is
// actually load-bearing, not decorative. Recorded on the task, not
// expressible as a `#[test]` that stays in the tree (it would mean
// permanently carrying two nearly-identical command strings).
