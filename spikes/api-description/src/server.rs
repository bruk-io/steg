//! The toy server, and the stand-in transport both ends of the spike use.
//!
//! **The framing here is not a decision.** Newline-delimited JSON over a Unix socket is
//! the least that carries one call so the method can be followed end to end. TASK-1.2
//! chooses the real framing and how terminal output reaches a client. What this spike
//! does fix is the layer above: the *payload* of the describe call is the contract, so
//! whatever framing TASK-1.2 picks only has to be able to carry it.
//!
//! The server reads [`crate::api::description()`] and validates every request against
//! it, so the API's surface is declared in one place and this module only holds the
//! bodies behind it. Target resolution here is the thinnest thing that shows R10's
//! ambiguity rule; TASK-1.7 owns the real one.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

use crate::api;
use crate::description::{Description, ErrorCode, ValueKind};

/// The method that returns the description itself (R37). Rex spells its own `help.all`.
pub const DESCRIBE: &str = "api.describe";

/// One request: the method's dotted name and its parameters by name.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Request {
    pub method: String,
    #[serde(default)]
    pub params: Map<String, Value>,
}

/// What the server answers with.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Response {
    Ok(Value),
    Error(ApiError),
}

/// An error, carrying the code the description lists so the CLI can look up its exit status.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApiError {
    pub code: ErrorCode,
    pub message: String,
    /// The things a target matched, when it matched more than one.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub candidates: Vec<String>,
}

impl ApiError {
    fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            candidates: Vec::new(),
        }
    }
}

/// Check a request against the description: the only thing that says what the API accepts.
///
/// # Errors
///
/// [`ErrorCode::BadRequest`] when the method is not in the description, a parameter is
/// not one the method declares, or a required parameter is missing.
pub fn validate(api: &Description, request: &Request) -> Result<(), ApiError> {
    if request.method == DESCRIBE {
        return Ok(());
    }
    let Some(method) = api.method(&request.method) else {
        return Err(ApiError::new(
            ErrorCode::BadRequest,
            format!("no such method: {}", request.method),
        ));
    };
    for name in request.params.keys() {
        if !method.params.iter().any(|p| &p.name == name) {
            return Err(ApiError::new(
                ErrorCode::BadRequest,
                format!("{} has no parameter {name}", request.method),
            ));
        }
    }
    for param in method.params.iter().filter(|p| p.required) {
        if !request.params.contains_key(&param.name) {
            return Err(ApiError::new(
                ErrorCode::BadRequest,
                format!("{} needs {}", request.method, param.name),
            ));
        }
    }
    for param in &method.params {
        if let (Some(value), ValueKind::Choice(words)) =
            (request.params.get(&param.name), &param.value)
        {
            let given = value.as_str().unwrap_or_default();
            if !words.iter().any(|word| word == given) {
                return Err(ApiError::new(
                    ErrorCode::BadRequest,
                    format!("{} must be one of {}", param.name, words.join(", ")),
                ));
            }
        }
    }
    Ok(())
}

/// A pane, as much of one as the spike needs.
#[derive(Debug, Clone)]
struct Pane {
    id: String,
    label: String,
    working_directory: String,
}

/// Everything the toy server knows. The real one keeps this in the kernel (R1).
#[derive(Debug)]
pub struct State {
    panes: Vec<Pane>,
    next: u32,
}

impl Default for State {
    fn default() -> Self {
        Self::new()
    }
}

impl State {
    /// Two panes whose identifiers end in the same four characters, so a target can be
    /// ambiguous the way R10 means.
    pub fn new() -> Self {
        Self {
            panes: vec![
                Pane {
                    id: "pane-7f3c0011".into(),
                    label: "logs".into(),
                    working_directory: "/Users/bruk/src/steg".into(),
                },
                Pane {
                    id: "pane-9d1c0011".into(),
                    label: "build".into(),
                    working_directory: "/Users/bruk/src/steg".into(),
                },
            ],
            next: 1,
        }
    }

    /// Resolve a target to one pane by R10's rules: full identifier, label, position, or
    /// an identifier suffix of four or more characters.
    ///
    /// # Errors
    ///
    /// [`ErrorCode::NoSuchTarget`] when nothing matches, and
    /// [`ErrorCode::AmbiguousTarget`], listing the candidates, when more than one does.
    fn resolve(&self, target: Option<&str>) -> Result<&Pane, ApiError> {
        let Some(target) = target else {
            // No target and no context: the current pane. The spike takes the first.
            return self
                .panes
                .first()
                .ok_or_else(|| ApiError::new(ErrorCode::NoSuchTarget, "there are no panes"));
        };
        let position = target.parse::<usize>().ok();
        let matches: Vec<&Pane> = self
            .panes
            .iter()
            .enumerate()
            .filter(|(index, pane)| {
                pane.id == target
                    || pane.label == target
                    || position.is_some_and(|n| n == index + 1)
                    || (target.len() >= 4 && pane.id.ends_with(target))
            })
            .map(|(_, pane)| pane)
            .collect();
        match matches.as_slice() {
            [] => Err(ApiError::new(
                ErrorCode::NoSuchTarget,
                format!("no pane matches {target}"),
            )),
            [one] => Ok(one),
            many => Err(ApiError {
                code: ErrorCode::AmbiguousTarget,
                message: format!("{target} matches {} panes", many.len()),
                candidates: many.iter().map(|pane| pane.id.clone()).collect(),
            }),
        }
    }
}

/// The methods this module has a body for. A test holds this against the description, so
/// neither can grow a method the other does not have.
pub fn handled() -> Vec<&'static str> {
    vec!["pane.split", "pane.set", "pane.list"]
}

/// Run one validated request.
///
/// # Errors
///
/// Whatever the method answers with: a bad request, or a target that matched nothing or
/// more than one thing.
pub fn handle(state: &mut State, api: &Description, request: &Request) -> Result<Value, ApiError> {
    validate(api, request)?;
    let target = request.params.get("target").and_then(Value::as_str);
    match request.method.as_str() {
        DESCRIBE => Ok(json!(api)),
        "pane.split" => {
            let parent = state.resolve(target)?.clone();
            let id = format!("pane-{:04x}{:04x}", state.next, 0x11_u32);
            state.next += 1;
            let label = request
                .params
                .get("label")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned();
            let working_directory = request
                .params
                .get("working_directory")
                .and_then(Value::as_str)
                .unwrap_or(&parent.working_directory)
                .to_owned();
            state.panes.push(Pane {
                id: id.clone(),
                label,
                working_directory,
            });
            Ok(json!({ "id": id }))
        }
        "pane.set" => {
            let id = state.resolve(target)?.id.clone();
            if let Some(label) = request.params.get("label").and_then(Value::as_str) {
                let label = label.to_owned();
                if let Some(pane) = state.panes.iter_mut().find(|pane| pane.id == id) {
                    pane.label = label;
                }
            }
            Ok(Value::Null)
        }
        "pane.list" => {
            let records: Vec<Value> = state
                .panes
                .iter()
                .map(|pane| {
                    json!({
                        "id": pane.id,
                        "label": pane.label,
                        "working_directory": pane.working_directory,
                    })
                })
                .collect();
            Ok(json!(records))
        }
        other => Err(ApiError::new(
            ErrorCode::BadRequest,
            format!("no such method: {other}"),
        )),
    }
}

/// Take the socket, clearing one left behind by an earlier run.
///
/// Binding is its own step so that a caller can say it is listening only once it is, and
/// a test can wait on that rather than on a sleep.
///
/// # Errors
///
/// When the socket is held by a live server, or its directory cannot be written.
pub fn bind(socket: &Path) -> std::io::Result<UnixListener> {
    if socket.exists() {
        std::fs::remove_file(socket)?;
    }
    UnixListener::bind(socket)
}

/// Serve requests until the process is killed.
///
/// # Errors
///
/// When a connection fails in a way that is not the client simply going away.
pub fn serve_on(listener: &UnixListener) -> std::io::Result<()> {
    let api = api::description();
    let mut state = State::new();
    for stream in listener.incoming() {
        let stream = stream?;
        let reader = BufReader::new(stream.try_clone()?);
        let mut writer = stream;
        for line in reader.lines() {
            let line = line?;
            if line.is_empty() {
                continue;
            }
            let response = match serde_json::from_str::<Request>(&line) {
                Ok(request) => match handle(&mut state, &api, &request) {
                    Ok(value) => Response::Ok(value),
                    Err(error) => Response::Error(error),
                },
                Err(error) => Response::Error(ApiError::new(
                    ErrorCode::BadRequest,
                    format!("unreadable request: {error}"),
                )),
            };
            let mut encoded = serde_json::to_string(&response)?;
            encoded.push('\n');
            writer.write_all(encoded.as_bytes())?;
            writer.flush()?;
        }
    }
    Ok(())
}

/// Make one call to a server over the stand-in transport.
///
/// # Errors
///
/// When the socket cannot be reached, or the server's answer cannot be read. A failure
/// to reach the server is exit status 4 in doc-2 section 6.
pub fn call(socket: &Path, request: &Request) -> std::io::Result<Response> {
    let stream = UnixStream::connect(socket)?;
    let mut writer = stream.try_clone()?;
    let mut encoded = serde_json::to_string(request)?;
    encoded.push('\n');
    writer.write_all(encoded.as_bytes())?;
    writer.flush()?;
    let mut line = String::new();
    BufReader::new(stream).read_line(&mut line)?;
    Ok(serde_json::from_str(&line)?)
}

/// Ask a server for its description (R37). This is candidate B's first act, every run.
///
/// # Errors
///
/// When the server cannot be reached, or answers with something that is not a description.
pub fn describe(socket: &Path) -> std::io::Result<Description> {
    let request = Request {
        method: DESCRIBE.into(),
        params: Map::new(),
    };
    match call(socket, &request)? {
        Response::Ok(value) => Ok(serde_json::from_value(value)?),
        Response::Error(error) => Err(std::io::Error::other(error.message)),
    }
}

/// A method's parameters as JSON, without a struct per method: the CLI builds these from
/// the description, so nothing names a parameter twice.
pub fn params(pairs: Vec<(&str, Value)>) -> Map<String, Value> {
    pairs
        .into_iter()
        .map(|(name, value)| (name.to_owned(), value))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(method: &str, pairs: Vec<(&str, Value)>) -> Request {
        Request {
            method: method.into(),
            params: params(pairs),
        }
    }

    #[test]
    fn a_parameter_the_description_does_not_declare_is_refused() {
        let api = api::description();
        let asked = request(
            "pane.split",
            vec![("direction", json!("right")), ("rows", json!(10))],
        );
        let error = validate(&api, &asked).expect_err("an undeclared parameter must be refused");
        assert_eq!(error.code, ErrorCode::BadRequest);
    }

    #[test]
    fn a_missing_required_parameter_is_refused() {
        let api = api::description();
        let error = validate(&api, &request("pane.split", vec![]))
            .expect_err("direction is required, so leaving it out must be refused");
        assert_eq!(error.code, ErrorCode::BadRequest);
    }

    #[test]
    fn a_word_outside_a_closed_set_is_refused() {
        let api = api::description();
        let asked = request("pane.split", vec![("direction", json!("sideways"))]);
        let error = validate(&api, &asked).expect_err("sideways is not a direction");
        assert_eq!(error.code, ErrorCode::BadRequest);
    }

    #[test]
    fn an_identifier_suffix_that_matches_two_panes_is_ambiguous_and_does_nothing() {
        let api = api::description();
        let mut state = State::new();
        let before = state.panes.len();
        let asked = request(
            "pane.split",
            vec![("direction", json!("right")), ("target", json!("0011"))],
        );
        let error = handle(&mut state, &api, &asked).expect_err("0011 matches two panes");
        assert_eq!(error.code, ErrorCode::AmbiguousTarget);
        assert_eq!(error.candidates.len(), 2);
        assert_eq!(
            state.panes.len(),
            before,
            "an ambiguous target must change nothing"
        );
    }

    #[test]
    fn splitting_a_pane_answers_with_the_new_panes_identifier() {
        let api = api::description();
        let mut state = State::new();
        let asked = request(
            "pane.split",
            vec![("direction", json!("right")), ("target", json!("logs"))],
        );
        let value = handle(&mut state, &api, &asked).expect("logs matches one pane");
        assert!(
            value["id"]
                .as_str()
                .is_some_and(|id| id.starts_with("pane-"))
        );
    }

    #[test]
    fn every_method_in_the_description_has_a_body_and_every_body_is_in_the_description() {
        let api = api::description();
        let described: Vec<String> = api.methods.iter().map(|m| m.name.dotted()).collect();
        for name in handled() {
            assert!(
                described.iter().any(|d| d == name),
                "{name} has a body but is not described"
            );
        }
        for name in &described {
            assert!(
                handled().contains(&name.as_str()),
                "{name} is described but has no body"
            );
        }
    }
}
