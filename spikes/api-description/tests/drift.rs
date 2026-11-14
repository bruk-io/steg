//! What the spike has to prove, against the real binaries and a real socket.
//!
//! Two kinds of thing are tested here, and neither is the happy path for its own sake.
//!
//! 1. **Drift.** A change to the one definition reaches every consumer, and a consumer
//!    that falls behind fails a test. That is what `.claude/rules/architecture.md` asks
//!    for and what TASK-1.13's first acceptance criterion inherits.
//! 2. **The discriminator.** doc-2 section 5 says `--help` contacts no server. Candidate
//!    A obeys it; candidate B cannot. This is measured as a requirement, not a taste.

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

use serde_json::json;
use steg_spike_api_description::description::{Param, Role, ValueKind};
use steg_spike_api_description::server::{Request, Response, params};
use steg_spike_api_description::{api, cli_build, docs, server};

/// A running toy server that is killed however the test ends.
struct Server {
    child: Child,
    socket: PathBuf,
}

impl Server {
    /// Start a server and return once its socket is bound, waiting on the line it prints
    /// rather than on a sleep.
    fn start(name: &str) -> Self {
        let socket = std::env::temp_dir().join(format!("steg-{name}-{}.sock", std::process::id()));
        let mut child = Command::new(env!("CARGO_BIN_EXE_spike_server"))
            .arg(&socket)
            .stdout(Stdio::piped())
            .spawn()
            .expect("the server binary is built by cargo test");
        let stdout = child.stdout.take().expect("stdout was piped");
        let mut first = String::new();
        BufReader::new(stdout)
            .read_line(&mut first)
            .expect("the server says when it is listening");
        assert!(
            first.starts_with("listening on"),
            "the server said: {first}"
        );
        Self { child, socket }
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = std::fs::remove_file(&self.socket);
    }
}

/// Run one of the CLI binaries, with the socket in the context variable doc-2 names.
fn cli(binary: &str, socket: &Path, args: &[&str]) -> (i32, String, String) {
    let output = Command::new(binary)
        .env("STEG_SERVER", socket)
        .args(args)
        .output()
        .expect("the CLI binary is built by cargo test");
    (
        output.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

const CANDIDATE_A: &str = env!("CARGO_BIN_EXE_spike_cli_a");
const CANDIDATE_B: &str = env!("CARGO_BIN_EXE_spike_cli_b");

#[test]
fn a_parameter_added_to_the_definition_reaches_the_cli_the_server_the_docs_and_the_payload() {
    // The one definition, with one more parameter than it had. Nothing else is edited:
    // what follows asserts that each consumer reads the description rather than a copy.
    let mut api = api::description();
    let method = api
        .methods
        .iter_mut()
        .find(|method| method.name.dotted() == "pane.split")
        .expect("pane.split is in the definition");
    method.params.push(Param {
        name: "rows".into(),
        role: Role::Flag,
        value: ValueKind::Integer,
        required: false,
        summary: "How many rows the new pane takes".into(),
    });
    let method = api.method("pane.split").expect("it is still there").clone();

    // The CLI.
    let command = cli_build::method_command(&method);
    let arg = command
        .get_arguments()
        .find(|arg| arg.get_id() == "rows")
        .expect("the new parameter became an argument");
    assert_eq!(arg.get_long(), Some("rows"), "and a flag of the same name");

    // What was typed becomes the new parameter in the request.
    let matches = cli_build::method_command(&method)
        .try_get_matches_from(vec!["split", "--direction", "right", "--rows", "40"])
        .expect("the new flag is accepted");
    let request = cli_build::request_from_matches(&method, &matches);
    assert_eq!(
        request.params["rows"],
        json!(40),
        "and is sent with its declared type"
    );

    // The server, which validates against the description and so now accepts it.
    server::validate(&api, &request).expect("the server accepts what the description declares");

    // The docs.
    assert!(
        docs::method_page(&method, &api).contains("--rows"),
        "the new parameter reached the reference docs"
    );

    // The payload a Python plugin reads.
    let payload = serde_json::to_string(&api).expect("the description serializes");
    assert!(
        payload.contains("\"rows\""),
        "the new parameter reached the JSON payload"
    );
}

#[test]
fn a_parameter_the_description_does_not_declare_is_refused_by_the_running_server() {
    let server = Server::start("drift-undeclared");
    let request = Request {
        method: "pane.split".into(),
        params: params(vec![("direction", json!("right")), ("rows", json!(40))]),
    };
    let response = server::call(&server.socket, &request).expect("the server is listening");
    match response {
        Response::Error(error) => assert_eq!(
            format!("{:?}", error.code),
            "BadRequest",
            "an undeclared parameter is a bad request"
        ),
        Response::Ok(value) => panic!("the server accepted an undeclared parameter: {value}"),
    }
}

#[test]
fn candidate_a_prints_help_for_a_command_with_no_server_running() {
    let nowhere = std::env::temp_dir().join("steg-no-server-at-all.sock");
    let (status, out, err) = cli(CANDIDATE_A, &nowhere, &["pane", "split", "--help"]);
    assert_eq!(
        status, 0,
        "doc-2 section 5: --help is status 0. stderr: {err}"
    );
    assert!(
        out.contains("--direction"),
        "the flags are in the help text"
    );
    assert!(
        out.contains("--no-focus"),
        "including a switch's --no- form"
    );
    assert!(err.is_empty(), "help goes to stdout, not stderr");
}

#[test]
fn candidate_b_cannot_print_help_with_no_server_running() {
    let nowhere = std::env::temp_dir().join("steg-no-server-at-all.sock");
    let (status, out, err) = cli(CANDIDATE_B, &nowhere, &["pane", "split", "--help"]);
    assert_eq!(status, 4, "it fails as unreachable, whatever was asked for");
    assert!(out.is_empty(), "there is no help to print");
    assert!(err.contains("cannot reach the server"));
}

#[test]
fn both_candidates_generate_the_same_command_from_the_same_description() {
    let server = Server::start("drift-same");
    let (a_status, a_help, _) = cli(CANDIDATE_A, &server.socket, &["pane", "split", "--help"]);
    let (b_status, b_help, b_err) = cli(CANDIDATE_B, &server.socket, &["pane", "split", "--help"]);
    assert_eq!(
        (a_status, b_status),
        (0, 0),
        "with a server up, both answer help. {b_err}"
    );
    assert_eq!(
        a_help, b_help,
        "the same description generates the same command either way"
    );
}

#[test]
fn every_method_in_the_description_can_be_called_through_its_generated_command() {
    let server = Server::start("drift-every");
    let api = api::description();
    // The arguments each method needs, which is the only thing a test can know that the
    // description does not say: a value for each required parameter.
    let calls: Vec<(&str, Vec<&str>)> = vec![
        (
            "pane.split",
            vec!["pane", "split", "logs", "--direction", "right"],
        ),
        (
            "pane.set",
            vec!["pane", "set", "logs", "--label", "renamed"],
        ),
        ("pane.list", vec!["pane", "list"]),
    ];
    for method in &api.methods {
        let dotted = method.name.dotted();
        let (_, args) = calls
            .iter()
            .find(|(name, _)| *name == dotted)
            .unwrap_or_else(|| {
                panic!("{dotted} is in the description but this test does not call it")
            });
        let (status, _, err) = cli(CANDIDATE_A, &server.socket, args);
        assert_eq!(
            status, 0,
            "{dotted} failed through its generated command: {err}"
        );
    }
}

#[test]
fn a_method_prints_what_the_description_says_it_returns() {
    let server = Server::start("drift-returns");

    // Returns::Identifier: the identifier alone on a line, so a shell can capture it.
    let (status, out, err) = cli(
        CANDIDATE_A,
        &server.socket,
        &["pane", "split", "logs", "--direction", "right"],
    );
    assert_eq!(status, 0, "{err}");
    let printed = out.trim();
    assert!(printed.starts_with("pane-"), "it printed {printed:?}");
    assert_eq!(printed.lines().count(), 1, "one line, nothing else");

    // Returns::Nothing: silence.
    let (status, out, err) = cli(
        CANDIDATE_A,
        &server.socket,
        &["pane", "set", "logs", "--label", "renamed"],
    );
    assert_eq!(
        (status, out.as_str()),
        (0, ""),
        "an action that worked is silent. {err}"
    );

    // Returns::Records: one record per line, the description's fields, tab-separated.
    let (status, out, err) = cli(CANDIDATE_A, &server.socket, &["pane", "list"]);
    assert_eq!(status, 0, "{err}");
    let first = out.lines().next().expect("there are panes");
    assert_eq!(
        first.split('\t').count(),
        3,
        "id, label and working_directory: {first:?}"
    );
}

#[test]
fn an_ambiguous_target_exits_with_the_status_the_description_gives_it() {
    let server = Server::start("drift-ambiguous");
    // Both panes' identifiers end in 0011, which R10 says is an error and never a guess.
    let (status, out, err) = cli(
        CANDIDATE_A,
        &server.socket,
        &["pane", "split", "0011", "--direction", "right"],
    );
    assert_eq!(
        status, 3,
        "doc-2 section 6: a target that matched more than one thing"
    );
    assert!(out.is_empty(), "nothing on stdout");
    assert!(
        err.contains("pane-7f3c0011"),
        "the candidates are listed: {err}"
    );
}

#[test]
fn the_json_output_carries_the_api_field_names() {
    let server = Server::start("drift-json");
    let (status, out, err) = cli(CANDIDATE_A, &server.socket, &["pane", "list", "--json"]);
    assert_eq!(status, 0, "{err}");
    let records: serde_json::Value = serde_json::from_str(&out).expect("--json prints JSON");
    let first = &records[0];
    for field in ["id", "label", "working_directory"] {
        assert!(
            first.get(field).is_some(),
            "{field} is a field of the API, so of --json"
        );
    }
}

#[test]
fn the_docs_can_be_generated_from_the_servers_own_answer() {
    let server = Server::start("drift-docs");
    let from_definition = Command::new(env!("CARGO_BIN_EXE_spike_docs"))
        .output()
        .expect("the docs binary is built by cargo test");
    let from_server = Command::new(env!("CARGO_BIN_EXE_spike_docs"))
        .env("STEG_SERVER", &server.socket)
        .arg("--from-server")
        .output()
        .expect("the docs binary is built by cargo test");
    assert_eq!(
        String::from_utf8_lossy(&from_definition.stdout),
        String::from_utf8_lossy(&from_server.stdout),
        "R62's docs are the same whichever side of the one description they come from"
    );
    assert!(
        String::from_utf8_lossy(&from_server.stdout).contains("steg pane split"),
        "and they document the method"
    );
}
