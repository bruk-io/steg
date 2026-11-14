//! The one definition of steg's API.
//!
//! This module is the single source the spike is about. Everything else reads
//! [`description()`] and nothing else: the server dispatches from it and validates
//! against it, both CLIs build their commands from it, the docs are rendered from it,
//! and a Python plugin gets it as JSON. Nothing downstream names a method, a parameter
//! or a flag of its own.
//!
//! Three methods are enough to exercise the format: `pane.split` carries the acceptance
//! criterion end to end, `pane.set` shows a switch with its `--no-` form, and
//! `pane.list` shows a list of records and makes `steg pane` have more than one verb.

use crate::description::{
    Description, ErrorCode, ErrorRecord, Event, Field, Method, MethodName, Param, Returns, Role,
    ValueKind, Version,
};

/// The API this build of steg speaks.
pub const VERSION: Version = Version { major: 1, minor: 0 };

/// The whole API, as data.
///
/// Called once per process: by the server when it starts, by candidate A's CLI when it
/// builds its commands, and by the docs generator. Candidate B's CLI does not call it at
/// all; it asks the server for the same value over the socket.
pub fn description() -> Description {
    Description {
        api_version: VERSION,
        methods: vec![pane_split(), pane_set(), pane_list()],
        events: vec![pane_closed()],
        errors: errors(),
    }
}

/// `pane.split`: the method carried end to end, chosen for what it has to exercise.
///
/// A target resolved by R10's rules, a closed set of words, two optional values, a
/// program given after `--`, a typed identifier as its result, and an error a script can
/// tell apart.
fn pane_split() -> Method {
    Method {
        name: MethodName {
            noun: "pane".into(),
            verb: "split".into(),
        },
        summary: "Split the target pane and run a program in the new one".into(),
        details: "Divides the target pane in the given direction and starts a program in \
                  the new pane. The new pane's identifier is the result, so a script can \
                  capture it. Focus does not move unless it is asked for (R64)."
            .into(),
        params: vec![
            Param {
                name: "target".into(),
                role: Role::Target,
                value: ValueKind::Target,
                required: false,
                summary: "The pane to split. Defaults to the pane the command ran in".into(),
            },
            Param {
                name: "direction".into(),
                role: Role::Flag,
                value: ValueKind::Choice(vec![
                    "left".into(),
                    "right".into(),
                    "up".into(),
                    "down".into(),
                ]),
                required: true,
                summary: "Which side of the target the new pane takes".into(),
            },
            Param {
                name: "working_directory".into(),
                role: Role::Flag,
                value: ValueKind::Path,
                required: false,
                summary: "The new program's working directory".into(),
            },
            Param {
                name: "label".into(),
                role: Role::Flag,
                value: ValueKind::Text,
                required: false,
                summary: "A label for the new pane".into(),
            },
            Param {
                name: "focus".into(),
                role: Role::Switch,
                value: ValueKind::Boolean,
                required: false,
                summary: "Move keyboard focus to the new pane".into(),
            },
            Param {
                name: "program".into(),
                role: Role::Program,
                value: ValueKind::Argv,
                required: false,
                summary: "The program to run, and its arguments. Defaults to the login shell"
                    .into(),
            },
        ],
        returns: Returns::Identifier { of: "pane".into() },
        errors: vec![
            ErrorCode::BadRequest,
            ErrorCode::NoSuchTarget,
            ErrorCode::AmbiguousTarget,
        ],
        since: VERSION,
    }
}

/// `pane.set`: in the definition to show a switch, which the CLI gives a `--no-` form.
fn pane_set() -> Method {
    Method {
        name: MethodName {
            noun: "pane".into(),
            verb: "set".into(),
        },
        summary: "Change the target pane's properties".into(),
        details: "Each property is a flag. A property left out is unchanged.".into(),
        params: vec![
            Param {
                name: "target".into(),
                role: Role::Target,
                value: ValueKind::Target,
                required: false,
                summary: "The pane to change. Defaults to the pane the command ran in".into(),
            },
            Param {
                name: "label".into(),
                role: Role::Flag,
                value: ValueKind::Text,
                required: false,
                summary: "The pane's label".into(),
            },
            Param {
                name: "zoomed".into(),
                role: Role::Switch,
                value: ValueKind::Boolean,
                required: false,
                summary: "Whether the pane fills its tab".into(),
            },
        ],
        returns: Returns::Nothing,
        errors: vec![
            ErrorCode::BadRequest,
            ErrorCode::NoSuchTarget,
            ErrorCode::AmbiguousTarget,
        ],
        since: VERSION,
    }
}

/// `pane.list`: in the definition to show a list of records.
fn pane_list() -> Method {
    Method {
        name: MethodName {
            noun: "pane".into(),
            verb: "list".into(),
        },
        summary: "Every pane of the target tab, one per line".into(),
        details: "The columns are in the order below and never reordered. --json prints \
                  the same fields under the same names (doc-2, section 6)."
            .into(),
        params: vec![Param {
            name: "target".into(),
            role: Role::Target,
            value: ValueKind::Target,
            required: false,
            summary: "The tab whose panes are listed. Defaults to the current tab".into(),
        }],
        returns: Returns::Records {
            fields: pane_fields(),
        },
        errors: vec![
            ErrorCode::BadRequest,
            ErrorCode::NoSuchTarget,
            ErrorCode::AmbiguousTarget,
        ],
        since: VERSION,
    }
}

/// The fields of a pane record, shared by what `pane.list` returns and what an event carries.
fn pane_fields() -> Vec<Field> {
    vec![
        Field {
            name: "id".into(),
            value: ValueKind::Text,
            summary: "The pane's full identifier".into(),
        },
        Field {
            name: "label".into(),
            value: ValueKind::Text,
            summary: "Its label, if any".into(),
        },
        Field {
            name: "working_directory".into(),
            value: ValueKind::Path,
            summary: "Its program's working directory".into(),
        },
    ]
}

/// `pane.closed`: one event, so the format is shown to cover R38 and not methods alone.
fn pane_closed() -> Event {
    Event {
        name: "pane.closed".into(),
        summary: "A pane ended, and with it whatever ran in it".into(),
        fields: pane_fields(),
        since: VERSION,
    }
}

/// Every error the API can answer with, and the exit status the CLI gives it (doc-2, section 6).
fn errors() -> Vec<ErrorRecord> {
    vec![
        ErrorRecord {
            code: ErrorCode::BadRequest,
            summary: "The request did not match the API: an unknown or missing parameter".into(),
            exit_status: 2,
        },
        ErrorRecord {
            code: ErrorCode::NoSuchTarget,
            summary: "The target matched nothing".into(),
            exit_status: 3,
        },
        ErrorRecord {
            code: ErrorCode::AmbiguousTarget,
            summary: "The target matched more than one thing, so nothing was done".into(),
            exit_status: 3,
        },
        ErrorRecord {
            code: ErrorCode::ProgramRunning,
            summary: "A program is still running and --force was not given".into(),
            exit_status: 1,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_definition_has_the_three_methods_the_spike_needs() {
        let api = description();
        assert_eq!(api.nouns(), vec!["pane"]);
        let verbs: Vec<String> = api
            .verbs_of("pane")
            .iter()
            .map(|m| m.name.verb.clone())
            .collect();
        assert_eq!(verbs, vec!["split", "set", "list"]);
    }

    #[test]
    fn every_error_a_method_names_has_a_record_with_an_exit_status() {
        let api = description();
        for method in &api.methods {
            for code in &method.errors {
                assert!(
                    api.errors.iter().any(|record| record.code == *code),
                    "{} names {code:?}, which has no record",
                    method.name.dotted()
                );
            }
        }
    }

    #[test]
    fn a_method_has_at_most_one_target_and_at_most_one_program() {
        for method in description().methods {
            assert!(
                method
                    .params
                    .iter()
                    .filter(|p| p.role == Role::Target)
                    .count()
                    <= 1,
                "{} has more than one positional",
                method.name.dotted()
            );
            assert!(
                method
                    .params
                    .iter()
                    .filter(|p| p.role == Role::Program)
                    .count()
                    <= 1,
                "{} has more than one program",
                method.name.dotted()
            );
        }
    }
}
