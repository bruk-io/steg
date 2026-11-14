//! The shape of the API description: what a client reads when it asks the server what
//! it can do, and what the CLI and the docs are generated from.
//!
//! These types are the contract. The JSON they serialize to is what a Python plugin
//! sees (R42, R75), so everything here is owned data with serde derives and nothing
//! here is Rust-specific. The one definition that fills them in is [`crate::api`].

use serde::{Deserialize, Serialize};

/// The whole API, as one value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Description {
    /// The version of the API itself, for R39's version check.
    pub api_version: Version,
    pub methods: Vec<Method>,
    pub events: Vec<Event>,
    pub errors: Vec<ErrorRecord>,
}

impl Description {
    /// Find a method by its dotted name, such as `pane.split`.
    pub fn method(&self, dotted: &str) -> Option<&Method> {
        self.methods.iter().find(|m| m.name.dotted() == dotted)
    }

    /// The nouns, in the order they first appear. `steg` alone lists these.
    pub fn nouns(&self) -> Vec<&str> {
        let mut seen: Vec<&str> = Vec::new();
        for method in &self.methods {
            if !seen.contains(&method.name.noun.as_str()) {
                seen.push(&method.name.noun);
            }
        }
        seen
    }

    /// The methods of one noun, in definition order. `steg <noun>` alone lists these.
    pub fn verbs_of(&self, noun: &str) -> Vec<&Method> {
        self.methods
            .iter()
            .filter(|m| m.name.noun == noun)
            .collect()
    }
}

/// A two-part version. The major part is what a client and server must agree on (R39).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Version {
    pub major: u32,
    pub minor: u32,
}

/// A method's name, in the two parts steg's CLI grammar is built from (doc-2, section 2).
///
/// Keeping the noun and the verb apart in the type, rather than splitting a string on a
/// dot, is what makes the grammar structural: a method that does not read as a noun and
/// a verb cannot be written down.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MethodName {
    pub noun: String,
    pub verb: String,
}

impl MethodName {
    /// The name as it goes over the wire: `pane.split`.
    pub fn dotted(&self) -> String {
        format!("{}.{}", self.noun, self.verb)
    }
}

/// One method of the API.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Method {
    pub name: MethodName,
    /// One line. It becomes clap's `about` and the docs' table entry.
    pub summary: String,
    /// The prose a person reads. It becomes clap's `long_about` and the docs' body.
    pub details: String,
    pub params: Vec<Param>,
    pub returns: Returns,
    /// The errors this method can answer with, by code.
    pub errors: Vec<ErrorCode>,
    /// The API version this method appeared in.
    pub since: Version,
}

impl Method {
    /// The parameter that is the command's one positional, if it has one.
    pub fn target(&self) -> Option<&Param> {
        self.params.iter().find(|p| p.role == Role::Target)
    }
}

/// A parameter of a method.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Param {
    /// The field name on the wire, in snake_case. The CLI flag is this with hyphens.
    pub name: String,
    pub role: Role,
    pub value: ValueKind,
    pub required: bool,
    pub summary: String,
}

impl Param {
    /// The long flag this parameter becomes: `--working-directory` for `working_directory`.
    pub fn flag(&self) -> String {
        self.name.replace('_', "-")
    }

    /// The flag that turns a switch off: `--no-zoomed` for `zoomed`.
    pub fn negated_flag(&self) -> String {
        format!("no-{}", self.flag())
    }
}

/// What a parameter is for, which is what tells the CLI generator where to put it.
///
/// This is the slot doc-2's grammar needs from the description. Without it the generator
/// could not know which parameter is the one positional, which parameter is a switch
/// that needs a `--no-` form, or which parameter is the program given after `--`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    /// The thing the verb acts on: the one positional (doc-2, section 4).
    Target,
    /// An ordinary flag that takes a value: `--direction right`.
    Flag,
    /// A boolean, which is a switch with a `--no-` counterpart (doc-2, section 5).
    Switch,
    /// The program to run, given after `--`: `steg pane split -- htop -d 5`.
    Program,
}

/// The kind of value a parameter takes. The CLI uses it to parse, the docs to describe
/// it, and the server to refuse a request that does not match.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind", content = "of")]
pub enum ValueKind {
    Text,
    Integer,
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

impl ValueKind {
    /// The word that stands for the value in usage text: `--direction <DIRECTION>`.
    pub fn placeholder(&self, param_name: &str) -> String {
        match self {
            Self::Choice(words) => words.join("|"),
            Self::Text | Self::Integer | Self::Boolean | Self::Target | Self::Path | Self::Argv => {
                param_name.to_uppercase()
            }
        }
    }
}

/// What a method prints on success. doc-2 section 6 fixes the form of each of these.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "returns")]
pub enum Returns {
    /// Nothing. An action that worked is silent.
    Nothing,
    /// One identifier, alone on a line, so a shell can capture it.
    Identifier { of: String },
    /// One record.
    Record { fields: Vec<Field> },
    /// Many records, one per line.
    Records { fields: Vec<Field> },
    /// Text, such as a pane's screen.
    Text,
}

/// A field of a record or an event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Field {
    pub name: String,
    pub value: ValueKind,
    pub summary: String,
}

/// An event a client can subscribe to (R38).
///
/// Events are in the description for the same reason methods are: `event watch --for`
/// takes one of these names, and TASK-1.11 would otherwise be a second place where the
/// set of events is written down.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Event {
    pub name: String,
    pub summary: String,
    pub fields: Vec<Field>,
    pub since: Version,
}

/// An error the API can answer with, and the CLI exit status it becomes.
///
/// The status lives here so that doc-2 section 6's table is generated too, rather than
/// being written by hand in the CLI a second time.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErrorRecord {
    pub code: ErrorCode,
    pub summary: String,
    pub exit_status: u8,
}

/// The errors a method can answer with.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    /// The request did not match the description: an unknown or missing parameter.
    BadRequest,
    /// The target matched nothing.
    NoSuchTarget,
    /// The target matched more than one thing. R10: never a guess.
    AmbiguousTarget,
    /// A program is still running and `--force` was not given.
    ProgramRunning,
}
