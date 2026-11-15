//! A request against the description, and validating one with no I/O
//! (R101: "the protocol's logic").
//!
//! [`validate`] is the whole of AC #2: it takes a [`Description`] and a
//! [`Request`] and says whether the request is one the description
//! declares, never touching a kernel `Tree` or anything else that could
//! fail for a reason other than "the request itself is wrong." It is meant
//! to be called from both sides of the wire: the server, to refuse a bad
//! request before doing anything (the dispatch task), and a future CLI
//! (TASK-1.49), to check a name before sending one (doc-2 section 5).

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use super::shapes::{Description, ErrorCode, ValueKind};

/// One request: a method's dotted name and its parameters by name.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Request {
    /// The method's dotted name, such as `pane.split`.
    pub method: String,
    /// Its parameters, by name.
    #[serde(default)]
    pub params: Map<String, Value>,
}

/// What a request is answered with.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Response {
    /// The method's result.
    Ok(Value),
    /// The request was refused, or the method failed.
    Error(ApiError),
}

/// An error, carrying the code the description lists so a client can look
/// up its exit status (doc-2 section 6).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApiError {
    /// The error's code.
    pub code: ErrorCode,
    /// A message for a person.
    pub message: String,
    /// What a target matched, when it matched more than one (R10).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub candidates: Vec<String>,
}

impl ApiError {
    /// Builds an error with no candidates.
    #[must_use]
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            candidates: Vec::new(),
        }
    }
}

/// Checks a request against the description: the only thing that says what
/// the API accepts. Does no I/O, and reaches for nothing outside
/// `description` and `request` — `steg-sdk`'s own `clippy.toml` refuses a
/// file, a socket, a process or the clock, so this is checked, not merely
/// written this way.
///
/// # Errors
///
/// [`ErrorCode::BadRequest`] when the method is not in the description, a
/// parameter is not one the method declares, a required parameter is
/// missing, or a value is given for a closed set (`ValueKind::Choice`) that
/// is not one of its words.
pub fn validate(description: &Description, request: &Request) -> Result<(), ApiError> {
    let Some(method) = description.method(&request.method) else {
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

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{ApiError, Request, validate};
    use crate::description::shapes::{
        Description, ErrorCode, Method, MethodName, Param, Returns, Role, ValueKind, Version,
    };

    fn request(method: &str, pairs: Vec<(&str, serde_json::Value)>) -> Request {
        Request {
            method: method.into(),
            params: pairs
                .into_iter()
                .map(|(name, value)| (name.to_owned(), value))
                .collect(),
        }
    }

    fn description_with_one_method() -> Description {
        Description {
            api_version: Version { major: 1, minor: 0 },
            methods: vec![Method {
                name: MethodName {
                    noun: "pane".into(),
                    verb: "split".into(),
                },
                summary: String::new(),
                details: String::new(),
                params: vec![
                    Param {
                        name: "direction".into(),
                        role: Role::Flag,
                        value: ValueKind::Choice(vec!["left".into(), "right".into()]),
                        required: true,
                        summary: String::new(),
                    },
                    Param {
                        name: "label".into(),
                        role: Role::Flag,
                        value: ValueKind::Text,
                        required: false,
                        summary: String::new(),
                    },
                ],
                returns: Returns::Identifier { of: "pane".into() },
                errors: vec![ErrorCode::BadRequest],
                since: Version { major: 1, minor: 0 },
            }],
            events: Vec::new(),
            errors: Vec::new(),
        }
    }

    fn expect_bad_request(result: Result<(), ApiError>) {
        match result {
            Ok(()) => panic!("expected a bad request"),
            Err(error) => assert_eq!(error.code, ErrorCode::BadRequest),
        }
    }

    #[test]
    fn an_unknown_method_is_a_bad_request() {
        let description = description_with_one_method();
        let asked = request("pane.nonexistent", vec![("direction", json!("right"))]);
        expect_bad_request(validate(&description, &asked));
    }

    #[test]
    fn a_parameter_the_description_does_not_declare_is_a_bad_request() {
        let description = description_with_one_method();
        let asked = request(
            "pane.split",
            vec![("direction", json!("right")), ("rows", json!(10))],
        );
        expect_bad_request(validate(&description, &asked));
    }

    #[test]
    fn a_missing_required_parameter_is_a_bad_request() {
        let description = description_with_one_method();
        let asked = request("pane.split", vec![]);
        expect_bad_request(validate(&description, &asked));
    }

    #[test]
    fn a_value_outside_a_closed_set_is_a_bad_request() {
        let description = description_with_one_method();
        let asked = request("pane.split", vec![("direction", json!("sideways"))]);
        expect_bad_request(validate(&description, &asked));
    }

    #[test]
    fn a_valid_request_is_accepted() {
        let description = description_with_one_method();
        let asked = request("pane.split", vec![("direction", json!("right"))]);
        assert!(validate(&description, &asked).is_ok());
    }

    #[test]
    fn request_params_defaults_to_empty_when_left_out_of_the_json() {
        let request: Request = serde_json::from_str(r#"{"method": "pane.split"}"#).unwrap();
        assert!(request.params.is_empty());
    }
}
