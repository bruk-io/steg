//! The shape of the API description: what a client reads when it asks for
//! every method (R37), and what the CLI, the reference docs and this
//! crate's own validation are built from (decision-1).
//!
//! These types are the contract. The JSON they serialize to is what a
//! Python plugin sees (R42, R75), so everything here is owned data with
//! serde derives and nothing here is Rust-specific. The one definition that
//! fills them in is [`crate::description::methods::description`].

use serde::{Deserialize, Serialize};

/// The whole API, as one value (decision-1).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Description {
    /// The version of the API itself, for R39's version check.
    pub api_version: Version,
    /// Every method (doc-2 section 8).
    pub methods: Vec<Method>,
    /// Every event a client can subscribe to (R38).
    pub events: Vec<Event>,
    /// Every error the API can answer with.
    pub errors: Vec<ErrorRecord>,
}

impl Description {
    /// Finds a method by its dotted name, such as `pane.split`.
    #[must_use]
    pub fn method(&self, dotted: &str) -> Option<&Method> {
        self.methods.iter().find(|m| m.name.dotted() == dotted)
    }

    /// Finds an event by its name, such as `pane.closed` (R38). Used to
    /// resolve an event arriving on the wire against the description that
    /// declared it (`wire::connection`), rather than leaving it as a bare
    /// string nobody checked.
    #[must_use]
    pub fn event(&self, name: &str) -> Option<&Event> {
        self.events.iter().find(|event| event.name == name)
    }

    /// The nouns, in the order they first appear. `steg` alone lists these.
    #[must_use]
    pub fn nouns(&self) -> Vec<&str> {
        let mut seen: Vec<&str> = Vec::new();
        for method in &self.methods {
            if !seen.contains(&method.name.noun.as_str()) {
                seen.push(&method.name.noun);
            }
        }
        seen
    }

    /// The methods of one noun, in definition order. `steg <noun>` alone
    /// lists these.
    #[must_use]
    pub fn verbs_of(&self, noun: &str) -> Vec<&Method> {
        self.methods
            .iter()
            .filter(|m| m.name.noun == noun)
            .collect()
    }
}

/// A two-part version. The major part is what a client and server must
/// agree on (R39).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Version {
    /// Bumped on a breaking change to the API.
    pub major: u32,
    /// Bumped on an additive change.
    pub minor: u32,
}

/// A method's name, in the two parts steg's CLI grammar is built from
/// (doc-2, section 2).
///
/// Keeping the noun and the verb apart in the type, rather than splitting a
/// string on a dot, is what makes the grammar structural: a method that
/// does not read as a noun and a verb cannot be written down.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MethodName {
    /// The noun: `pane`, `session`...
    pub noun: String,
    /// The verb: `split`, `list`...
    pub verb: String,
}

impl MethodName {
    /// The name as it goes over the wire: `pane.split`.
    #[must_use]
    pub fn dotted(&self) -> String {
        format!("{}.{}", self.noun, self.verb)
    }
}

/// One method of the API.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Method {
    /// The method's name.
    pub name: MethodName,
    /// One line. It becomes clap's `about` and the docs' table entry.
    pub summary: String,
    /// The prose a person reads. It becomes `long_about` and the docs'
    /// body.
    pub details: String,
    /// The method's parameters, in the order they are shown.
    pub params: Vec<Param>,
    /// What the method answers with, and so what the CLI prints.
    pub returns: Returns,
    /// The errors this method can answer with, by code.
    pub errors: Vec<ErrorCode>,
    /// The API version this method appeared in.
    pub since: Version,
}

impl Method {
    /// The parameter that is the command's one positional, if it has one.
    #[must_use]
    pub fn target(&self) -> Option<&Param> {
        self.params.iter().find(|p| p.role == Role::Target)
    }

    /// The parameter that is the program given after `--`, if it has one.
    #[must_use]
    pub fn program(&self) -> Option<&Param> {
        self.params.iter().find(|p| p.role == Role::Program)
    }
}

/// A parameter of a method.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Param {
    /// The field name on the wire, in snake_case. The CLI flag is this
    /// with hyphens.
    pub name: String,
    /// Where the CLI puts it.
    pub role: Role,
    /// What kind of value it takes.
    pub value: ValueKind,
    /// Whether a call without it is a bad request.
    pub required: bool,
    /// One line, which reaches both `--help` and the docs.
    pub summary: String,
}

impl Param {
    /// The long flag this parameter becomes: `--working-directory` for
    /// `working_directory`.
    #[must_use]
    pub fn flag(&self) -> String {
        self.name.replace('_', "-")
    }

    /// The flag that turns a switch off: `--no-zoomed` for `zoomed`.
    #[must_use]
    pub fn negated_flag(&self) -> String {
        format!("no-{}", self.flag())
    }
}

/// What a parameter is for, which is what tells the CLI generator where to
/// put it (doc-2, section 2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    /// The thing the verb acts on: the one positional (doc-2, section 4).
    Target,
    /// An ordinary flag that takes a value: `--direction right`.
    Flag,
    /// A boolean, which is a switch with a `--no-` counterpart (doc-2,
    /// section 5).
    Switch,
    /// The program to run, given after `--`: `steg pane split -- htop -d 5`.
    Program,
}

/// The kind of value a parameter takes. The CLI uses it to parse, the docs
/// to describe it, and the server to refuse a request that does not match.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind", content = "of")]
pub enum ValueKind {
    /// Free text.
    Text,
    /// A whole number.
    Integer,
    /// A switch (doc-2, section 5).
    Boolean,
    /// A closed set of words, such as a direction.
    Choice(Vec<String>),
    /// A target path: a session, tab or pane, resolved by R10's rules.
    Target,
    /// A filesystem path.
    Path,
    /// A program and its arguments.
    Argv,
}

/// What a method prints on success. doc-2 section 6 fixes the form of each
/// of these.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "returns")]
pub enum Returns {
    /// Nothing. An action that worked is silent.
    Nothing,
    /// One identifier, alone on a line, so a shell can capture it.
    Identifier {
        /// What the identifier is of: `"session"`, `"tab"`, `"pane"`.
        of: String,
    },
    /// One record.
    Record {
        /// Its fields, in the order they are printed.
        fields: Vec<Field>,
    },
    /// Many records, one per line.
    Records {
        /// Each record's fields, in the order they are printed.
        fields: Vec<Field>,
    },
    /// Text, such as a pane's screen.
    Text,
}

/// A field of a record or an event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Field {
    /// The field's name on the wire.
    pub name: String,
    /// What kind of value it holds.
    pub value: ValueKind,
    /// One line describing it.
    pub summary: String,
}

/// An event a client can subscribe to (R38).
///
/// Events are in the description for the same reason methods are:
/// `event watch --for` takes one of these names, and nothing downstream
/// starts a list of its own.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Event {
    /// The event's name, such as `pane.closed`.
    pub name: String,
    /// One line describing it.
    pub summary: String,
    /// The fields it carries.
    pub fields: Vec<Field>,
    /// The API version it appeared in.
    pub since: Version,
}

/// An error the API can answer with, and the CLI exit status it becomes.
///
/// The status lives here so that doc-2 section 6's table is generated too,
/// rather than being written by hand in the CLI a second time.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErrorRecord {
    /// The error's code.
    pub code: ErrorCode,
    /// One line describing it.
    pub summary: String,
    /// The CLI exit status it becomes (doc-2 section 6).
    pub exit_status: u8,
}

/// The errors a method can answer with.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    /// The request did not match the description: an unknown method, an
    /// unknown or missing parameter, or a value outside a closed set.
    BadRequest,
    /// The target matched nothing.
    NoSuchTarget,
    /// The target matched more than one thing. R10: never a guess.
    AmbiguousTarget,
    /// A program is still running and `--force` was not given.
    ProgramRunning,
    /// A move or reorder named a position beyond the end of its siblings.
    PositionOutOfRange,
    /// A split would leave a pane under doc-15 R100's floor.
    SplitTooSmall,
    /// A resize named a side with no split bordering it.
    NoSplitInDirection,
    /// A swap named two panes that are not in the same tab.
    PanesNotInSameTab,
    /// A move named the pane's own tab as the target.
    AlreadyInTab,
    /// The method is described, but nothing yet implements it.
    NotImplementedYet,
    /// The terminal engine (`ghostty-vt`) failed.
    EngineFailed,
}

#[cfg(test)]
mod tests {
    use super::{
        Description, ErrorCode, ErrorRecord, Event, Field, Method, MethodName, Param, Returns,
        Role, ValueKind, Version,
    };

    fn example_method() -> Method {
        Method {
            name: MethodName {
                noun: "pane".into(),
                verb: "split".into(),
            },
            summary: "Split the target pane and run a program in the new one".into(),
            details: "Divides the target pane in the given direction.".into(),
            params: vec![
                Param {
                    name: "target".into(),
                    role: Role::Target,
                    value: ValueKind::Target,
                    required: false,
                    summary: "The pane to split".into(),
                },
                Param {
                    name: "direction".into(),
                    role: Role::Flag,
                    value: ValueKind::Choice(vec!["left".into(), "right".into()]),
                    required: true,
                    summary: "Which side the new pane takes".into(),
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
                    summary: "The program to run".into(),
                },
            ],
            returns: Returns::Identifier { of: "pane".into() },
            errors: vec![ErrorCode::BadRequest, ErrorCode::NoSuchTarget],
            since: Version { major: 1, minor: 0 },
        }
    }

    #[test]
    fn a_method_round_trips_through_json() {
        let method = example_method();
        let json = serde_json::to_string(&method).unwrap();
        assert_eq!(serde_json::from_str::<Method>(&json).unwrap(), method);
    }

    #[test]
    fn target_and_program_find_their_own_param() {
        let method = example_method();
        assert_eq!(method.target().unwrap().name, "target");
        assert_eq!(method.program().unwrap().name, "program");
    }

    #[test]
    fn param_flag_and_negated_flag_turn_underscores_into_hyphens() {
        let param = Param {
            name: "working_directory".into(),
            role: Role::Flag,
            value: ValueKind::Path,
            required: false,
            summary: String::new(),
        };
        assert_eq!(param.flag(), "working-directory");
        assert_eq!(param.negated_flag(), "no-working-directory");
    }

    #[test]
    fn method_name_dotted_joins_noun_and_verb() {
        let name = MethodName {
            noun: "pane".into(),
            verb: "split".into(),
        };
        assert_eq!(name.dotted(), "pane.split");
    }

    fn example_description() -> Description {
        Description {
            api_version: Version { major: 1, minor: 0 },
            methods: vec![
                example_method(),
                Method {
                    name: MethodName {
                        noun: "pane".into(),
                        verb: "list".into(),
                    },
                    ..example_method()
                },
                Method {
                    name: MethodName {
                        noun: "session".into(),
                        verb: "list".into(),
                    },
                    ..example_method()
                },
            ],
            events: vec![Event {
                name: "pane.closed".into(),
                summary: "A pane ended".into(),
                fields: Vec::new(),
                since: Version { major: 1, minor: 0 },
            }],
            errors: vec![ErrorRecord {
                code: ErrorCode::BadRequest,
                summary: "bad request".into(),
                exit_status: 2,
            }],
        }
    }

    #[test]
    fn method_finds_by_dotted_name() {
        let description = example_description();
        assert_eq!(description.method("pane.list").unwrap().name.verb, "list");
        assert!(description.method("pane.nonexistent").is_none());
    }

    #[test]
    fn event_finds_by_name() {
        let description = example_description();
        assert_eq!(
            description.event("pane.closed").unwrap().name,
            "pane.closed"
        );
        assert!(description.event("pane.nonexistent").is_none());
    }

    #[test]
    fn nouns_are_in_first_appearance_order_with_no_duplicates() {
        let description = example_description();
        assert_eq!(description.nouns(), vec!["pane", "session"]);
    }

    #[test]
    fn verbs_of_filters_to_one_noun_in_definition_order() {
        let description = example_description();
        let verbs: Vec<&str> = description
            .verbs_of("pane")
            .iter()
            .map(|m| m.name.verb.as_str())
            .collect();
        assert_eq!(verbs, vec!["split", "list"]);
    }

    #[test]
    fn an_event_round_trips_through_json() {
        let event = Event {
            name: "pane.closed".into(),
            summary: "A pane ended".into(),
            fields: vec![Field {
                name: "id".into(),
                value: ValueKind::Text,
                summary: "The pane's id".into(),
            }],
            since: Version { major: 1, minor: 0 },
        };
        let json = serde_json::to_string(&event).unwrap();
        assert_eq!(serde_json::from_str::<Event>(&json).unwrap(), event);
    }
}
