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

/// Whether `value` is one `kind` accepts. `Value::Null` matches no kind
/// (every arm below checks a positive shape - a string, a number, a
/// boolean, an array - so a present-but-null value falls through every
/// one), which is what makes a null value refused for any parameter,
/// required or not, with no separate null check needed.
fn matches_kind(value: &Value, kind: &ValueKind) -> bool {
    match kind {
        ValueKind::Text | ValueKind::Target | ValueKind::Path => value.is_string(),
        ValueKind::Integer => value.is_i64() || value.is_u64(),
        ValueKind::Boolean => value.is_boolean(),
        ValueKind::Choice(words) => value
            .as_str()
            .is_some_and(|given| words.iter().any(|word| word == given)),
        ValueKind::Argv => value
            .as_array()
            .is_some_and(|items| items.iter().all(Value::is_string)),
    }
}

/// What `kind` is, for an error message naming it (`"{param} must be {kind}"`).
fn kind_description(kind: &ValueKind) -> String {
    match kind {
        ValueKind::Text => "text".into(),
        ValueKind::Integer => "an integer".into(),
        ValueKind::Boolean => "a boolean".into(),
        ValueKind::Choice(words) => format!("one of {}", words.join(", ")),
        ValueKind::Target => "a target".into(),
        ValueKind::Path => "a path".into(),
        ValueKind::Argv => "a program and its arguments".into(),
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
/// missing, or a parameter that is present (including a required one) does
/// not match its declared kind - `null`, a value of the wrong JSON type, or
/// a word outside a closed set (`ValueKind::Choice`).
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
        if let Some(value) = request.params.get(&param.name)
            && !matches_kind(value, &param.value)
        {
            return Err(ApiError::new(
                ErrorCode::BadRequest,
                format!("{} must be {}", param.name, kind_description(&param.value)),
            ));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

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

    /// A plain flag param of the given name and kind, for the fixture
    /// below - every kind `ValueKind` has, so `validate`'s type check is
    /// exercised against all of them, not just `Choice`.
    fn param(name: &str, value: ValueKind, required: bool) -> Param {
        Param {
            name: name.into(),
            role: Role::Flag,
            value,
            required,
            summary: String::new(),
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
                    param(
                        "direction",
                        ValueKind::Choice(vec!["left".into(), "right".into()]),
                        true,
                    ),
                    param("label", ValueKind::Text, false),
                    param("rows", ValueKind::Integer, false),
                    param("focus", ValueKind::Boolean, false),
                    param("cwd", ValueKind::Path, false),
                    param("to", ValueKind::Target, false),
                    param("args", ValueKind::Argv, false),
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
            vec![("direction", json!("right")), ("nonexistent", json!(10))],
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
    fn a_valid_request_using_every_kind_is_accepted() {
        let description = description_with_one_method();
        let asked = request(
            "pane.split",
            vec![
                ("direction", json!("right")),
                ("label", json!("logs")),
                ("rows", json!(10)),
                ("focus", json!(true)),
                ("cwd", json!("/tmp")),
                ("to", json!("work/2/logs")),
                ("args", json!(["htop", "-d", "5"])),
            ],
        );
        assert!(validate(&description, &asked).is_ok());
    }

    /// A null is refused for a parameter whether or not it is required -
    /// required-ness only governs *absence*, and a present null is never
    /// a value any kind accepts.
    #[test]
    fn a_null_value_is_a_bad_request_whether_or_not_the_parameter_is_required() {
        let description = description_with_one_method();
        expect_bad_request(validate(
            &description,
            &request("pane.split", vec![("direction", Value::Null)]),
        ));
        expect_bad_request(validate(
            &description,
            &request(
                "pane.split",
                vec![("direction", json!("right")), ("label", Value::Null)],
            ),
        ));
    }

    #[test]
    fn a_wrong_type_is_a_bad_request_for_every_kind() {
        let description = description_with_one_method();
        let base = |extra: (&str, serde_json::Value)| {
            request("pane.split", vec![("direction", json!("right")), extra])
        };

        expect_bad_request(validate(&description, &base(("label", json!(42)))));
        expect_bad_request(validate(&description, &base(("rows", json!("ten")))));
        expect_bad_request(validate(&description, &base(("focus", json!("yes")))));
        expect_bad_request(validate(&description, &base(("cwd", json!(42)))));
        expect_bad_request(validate(&description, &base(("to", json!(42)))));
        expect_bad_request(validate(&description, &base(("args", json!("htop")))));
        expect_bad_request(validate(&description, &base(("args", json!([1, 2])))));
    }

    #[test]
    fn the_message_names_the_parameter_and_the_kind_expected() {
        let description = description_with_one_method();
        let error = validate(
            &description,
            &request(
                "pane.split",
                vec![("direction", json!("right")), ("rows", json!("ten"))],
            ),
        )
        .expect_err("a string is not an integer");
        assert!(error.message.contains("rows"), "{}", error.message);
        assert!(error.message.contains("integer"), "{}", error.message);
    }

    #[test]
    fn request_params_defaults_to_empty_when_left_out_of_the_json() {
        let request: Request = serde_json::from_str(r#"{"method": "pane.split"}"#).unwrap();
        assert!(request.params.is_empty());
    }
}
