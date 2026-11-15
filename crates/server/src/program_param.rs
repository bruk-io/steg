//! Building the [`Program`] a `session.new`/`tab.new`/`pane.split` request
//! asks for, from its `program`/`cwd` parameters (doc-9 R13; TASK-1.76
//! AC #1).
//!
//! Pure: takes the already-resolved default working directory as plain
//! data, and never reads the environment itself - [`crate::runtime`] is
//! where that default comes from (rule A or B of the plan's §2, neither
//! decided here), and `validate` (called by the caller, before this) is
//! what already confirmed `program`, if present, is an array of strings.

use steg_sdk::{Program, Request};

/// Reads `request`'s `program` (an argv array) and `cwd` (a path), and
/// builds the `Program` the new pane should run. Absent `program` is a
/// plain login shell - the description's own words on `pane.split`'s
/// `program` parameter: "Defaults to the login shell" (`PaneId`'s own
/// `argv0` resolution is the caller's job, not this function's: this only
/// ever builds the `Program` *shape*, matching `Program::Shell`'s own
/// field, `cwd`, never a concrete command to run a shell with). Absent
/// `cwd` takes `default_cwd`.
#[must_use]
pub fn program_from_request(request: &Request, default_cwd: &str) -> Program {
    let cwd = request
        .params
        .get("cwd")
        .and_then(serde_json::Value::as_str)
        .unwrap_or(default_cwd)
        .to_string();

    match request
        .params
        .get("program")
        .and_then(serde_json::Value::as_array)
    {
        Some(argv) => Program::Command {
            argv: argv
                .iter()
                .filter_map(serde_json::Value::as_str)
                .map(str::to_string)
                .collect(),
            cwd,
        },
        None => Program::Shell { cwd },
    }
}

#[cfg(test)]
mod tests {
    use super::program_from_request;
    use serde_json::json;
    use steg_sdk::{Program, Request};

    fn request(params: serde_json::Value) -> Request {
        Request {
            method: "pane.split".to_string(),
            params: params.as_object().unwrap().clone(),
        }
    }

    #[test]
    fn no_program_and_no_cwd_is_a_shell_at_the_default() {
        let program = program_from_request(&request(json!({})), "/home/bruk");
        assert_eq!(
            program,
            Program::Shell {
                cwd: "/home/bruk".to_string()
            }
        );
    }

    #[test]
    fn a_given_cwd_overrides_the_default() {
        let program = program_from_request(&request(json!({"cwd": "/tmp/project"})), "/home/bruk");
        assert_eq!(
            program,
            Program::Shell {
                cwd: "/tmp/project".to_string()
            }
        );
    }

    #[test]
    fn a_given_program_is_a_command_with_its_argv_in_order() {
        let program = program_from_request(
            &request(json!({"program": ["htop", "-d", "5"]})),
            "/home/bruk",
        );
        assert_eq!(
            program,
            Program::Command {
                argv: vec!["htop".to_string(), "-d".to_string(), "5".to_string()],
                cwd: "/home/bruk".to_string()
            }
        );
    }

    #[test]
    fn a_given_program_and_cwd_both_take_their_own_given_value() {
        let program = program_from_request(
            &request(json!({"program": ["htop"], "cwd": "/tmp/project"})),
            "/home/bruk",
        );
        assert_eq!(
            program,
            Program::Command {
                argv: vec!["htop".to_string()],
                cwd: "/tmp/project".to_string()
            }
        );
    }

    #[test]
    fn an_empty_program_array_is_a_command_with_no_arguments_not_a_shell() {
        // `program` being *present* (even empty) is what the variant
        // hinges on (TASK-1.40's own plan: the fact is stored, never
        // inferred from argv being empty) - an empty array is still a
        // deliberate `Command`, not silently reinterpreted as "absent".
        let program = program_from_request(&request(json!({"program": []})), "/home/bruk");
        assert_eq!(
            program,
            Program::Command {
                argv: Vec::new(),
                cwd: "/home/bruk".to_string()
            }
        );
    }
}
