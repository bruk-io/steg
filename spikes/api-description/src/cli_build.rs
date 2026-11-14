//! The CLI generator: one description in, the whole command tree out.
//!
//! Both candidates use every function here. They differ in one line, in their binaries:
//! candidate A passes [`crate::api::description()`], which is compiled into the binary;
//! candidate B passes what the server answered the describe call with. So the comparison
//! measures when a client learns the API and nothing else.
//!
//! The mapping is doc-2 section 2's, mechanically: the method `pane.split` is
//! `steg pane split`, its target parameter is the one positional, every other parameter
//! is the flag of the same name with underscores turned into hyphens, a boolean is a
//! switch with a `--no-` form, and the program to run comes after `--`. Nothing is
//! renamed here, so there is no table of names to drift from the API.

use std::path::Path;

use clap::builder::PossibleValue;
use clap::{Arg, ArgAction, ArgMatches, Command};
use serde_json::{Map, Value};

use crate::description::{Description, ErrorCode, Field, Method, Param, Returns, Role, ValueKind};
use crate::server::{self, ApiError, Request, Response};

/// The flags every command carries (doc-2, section 5). These belong to the CLI, not to
/// the API, so they are written once here rather than in each method's parameters.
/// clap's builder takes `&'static str` for a name, a flag and a value's placeholder: its
/// `Str` and `Id` have no `From<String>`. A description that arrives at run time is owned
/// data, so its names have to be given a `'static` lifetime here.
///
/// Leaking is the right answer rather than a shortcut: these strings live as long as the
/// process does either way, and there are as many of them as the API has names. It is
/// worth noticing, though, that a description compiled into the binary would not need
/// this at all, which is one more cost on candidate B's side.
fn fixed(text: &str) -> &'static str {
    Box::leak(text.to_owned().into_boxed_str())
}

const COMMON: [(&str, char, &str); 5] = [
    ("json", '\0', "Machine-readable output"),
    ("quiet", 'q', "Print nothing on success"),
    ("verbose", 'v', "Diagnostics on stderr"),
    ("no-color", '\0', "No colour"),
    ("no-input", '\0', "Never read from the terminal"),
];

/// Build the whole command tree from a description.
pub fn command_tree(api: &Description) -> Command {
    let mut root = Command::new("steg")
        .version(fixed(&format!(
            "{}.{}",
            api.api_version.major, api.api_version.minor
        )))
        .about("A terminal workspace where the work outlives the window")
        .subcommand_required(true)
        .arg_required_else_help(true);
    for noun in api.nouns() {
        let mut group = Command::new(fixed(noun))
            .about(fixed(&format!("Act on a {noun}")))
            .subcommand_required(true)
            .arg_required_else_help(true);
        for method in api.verbs_of(noun) {
            group = group.subcommand(method_command(method));
        }
        root = root.subcommand(group);
    }
    root
}

/// Build one command from one method.
pub fn method_command(method: &Method) -> Command {
    let mut command = Command::new(fixed(&method.name.verb))
        .about(fixed(&method.summary))
        .long_about(method.details.clone());
    for param in &method.params {
        for arg in args_for(param) {
            command = command.arg(arg);
        }
    }
    for (long, short, help) in COMMON {
        let mut arg = Arg::new(long)
            .long(long)
            .help(help)
            .action(ArgAction::SetTrue);
        if short != '\0' {
            arg = arg.short(short);
        }
        command = command.arg(arg);
    }
    command
}

/// The clap arguments one parameter becomes. A switch becomes two: its `--no-` form is
/// always accepted beside it (doc-2, section 5).
fn args_for(param: &Param) -> Vec<Arg> {
    match param.role {
        Role::Target => vec![
            Arg::new(fixed(&param.name))
                .help(param.summary.clone())
                .value_name("TARGET")
                .required(param.required),
        ],
        Role::Flag => {
            let mut arg = Arg::new(fixed(&param.name))
                .long(fixed(&param.flag()))
                .help(param.summary.clone())
                .value_name(fixed(&param.value.placeholder(&param.name)))
                .required(param.required);
            if let ValueKind::Choice(words) = &param.value {
                let values: Vec<PossibleValue> = words
                    .iter()
                    .map(|word| PossibleValue::new(fixed(word)))
                    .collect();
                arg = arg.value_parser(values);
            }
            vec![arg]
        }
        Role::Switch => vec![
            Arg::new(fixed(&param.name))
                .long(fixed(&param.flag()))
                .help(param.summary.clone())
                .action(ArgAction::SetTrue)
                .overrides_with(fixed(&param.negated_flag())),
            Arg::new(fixed(&param.negated_flag()))
                .long(fixed(&param.negated_flag()))
                .help(format!("The opposite of --{}", param.flag()))
                .action(ArgAction::SetTrue)
                .overrides_with(fixed(&param.name)),
        ],
        Role::Program => vec![
            Arg::new(fixed(&param.name))
                .help(param.summary.clone())
                .value_name("PROGRAM")
                .last(true)
                .num_args(0..)
                .allow_hyphen_values(true),
        ],
    }
}

/// Turn what was typed into a request, reading the parameters the description declares.
pub fn request_from_matches(method: &Method, matches: &ArgMatches) -> Request {
    let mut params = Map::new();
    for param in &method.params {
        let value = match param.role {
            Role::Target | Role::Flag => match &param.value {
                ValueKind::Integer => matches
                    .get_one::<String>(&param.name)
                    .and_then(|text| text.parse::<i64>().ok())
                    .map(Value::from),
                ValueKind::Text
                | ValueKind::Boolean
                | ValueKind::Choice(_)
                | ValueKind::Target
                | ValueKind::Path
                | ValueKind::Argv => matches
                    .get_one::<String>(&param.name)
                    .map(|text| Value::from(text.clone())),
            },
            Role::Switch => {
                let on = matches.get_flag(&param.name);
                let off = matches.get_flag(&param.negated_flag());
                match (on, off) {
                    (true, _) => Some(Value::Bool(true)),
                    (false, true) => Some(Value::Bool(false)),
                    (false, false) => None,
                }
            }
            Role::Program => matches
                .get_many::<String>(&param.name)
                .map(|words| Value::from(words.cloned().collect::<Vec<String>>())),
        };
        if let Some(value) = value {
            params.insert(param.name.clone(), value);
        }
    }
    Request {
        method: method.name.dotted(),
        params,
    }
}

/// Print what a method returns, in the form doc-2 section 6 fixes for its kind.
fn print_result(returns: &Returns, value: &Value, as_json: bool, quiet: bool) {
    if quiet {
        return;
    }
    if as_json {
        println!("{value}");
        return;
    }
    match returns {
        Returns::Nothing => {}
        Returns::Identifier { .. } => {
            if let Some(id) = value.get("id").and_then(Value::as_str) {
                println!("{id}");
            }
        }
        Returns::Text => {
            if let Some(text) = value.as_str() {
                println!("{text}");
            }
        }
        Returns::Record { fields } => println!("{}", row(fields, value)),
        Returns::Records { fields } => {
            for record in value.as_array().map(Vec::as_slice).unwrap_or_default() {
                println!("{}", row(fields, record));
            }
        }
    }
}

/// One record as its fields in the description's order, tab-separated.
fn row(fields: &[Field], record: &Value) -> String {
    fields
        .iter()
        .map(|field| match record.get(&field.name) {
            Some(Value::String(text)) => text.clone(),
            Some(other) => other.to_string(),
            None => String::new(),
        })
        .collect::<Vec<String>>()
        .join("\t")
}

/// The exit status an error code carries, read from the description rather than from a
/// table written here: that is how doc-2 section 6's statuses stay generated too.
fn exit_status(api: &Description, code: ErrorCode) -> i32 {
    api.errors
        .iter()
        .find(|record| record.code == code)
        .map_or(1, |record| i32::from(record.exit_status))
}

/// Run the CLI: parse, call the server, print, and answer with an exit status.
///
/// `--help` and `--version` are answered by clap before this function touches the
/// socket, so they work with no server running (doc-2, section 5).
pub fn run(api: &Description, socket: &Path, argv: Vec<String>) -> i32 {
    let matches = match command_tree(api).try_get_matches_from(argv) {
        Ok(matches) => matches,
        Err(error) => {
            // clap answers help and version itself, on stdout, with status 0.
            let status = if error.use_stderr() { 2 } else { 0 };
            let _ = error.print();
            return status;
        }
    };
    let Some((noun, group)) = matches.subcommand() else {
        return 2;
    };
    let Some((verb, args)) = group.subcommand() else {
        return 2;
    };
    let dotted = format!("{noun}.{verb}");
    let Some(method) = api.method(&dotted) else {
        eprintln!("steg: {dotted} is not in the API");
        return 2;
    };
    let request = request_from_matches(method, args);
    match server::call(socket, &request) {
        Ok(Response::Ok(value)) => {
            print_result(
                &method.returns,
                &value,
                args.get_flag("json"),
                args.get_flag("quiet"),
            );
            0
        }
        Ok(Response::Error(error)) => {
            report(&error);
            exit_status(api, error.code)
        }
        Err(error) => {
            eprintln!(
                "steg: cannot reach the server at {}: {error}",
                socket.display()
            );
            4
        }
    }
}

/// Put an error on stderr, listing the candidates when a target matched more than one.
fn report(error: &ApiError) {
    eprintln!("steg: {}", error.message);
    for candidate in &error.candidates {
        eprintln!("  {candidate}");
    }
}

/// Every method in the description that the command tree has no command for.
///
/// This is the shape of the test TASK-1.13's first acceptance criterion asks for: a
/// method the CLI does not expose is a failure, not something a reader has to notice.
pub fn missing_commands(api: &Description, tree: &Command) -> Vec<String> {
    let mut missing = Vec::new();
    for method in &api.methods {
        let found = tree
            .get_subcommands()
            .find(|group| group.get_name() == method.name.noun)
            .is_some_and(|group| {
                group
                    .get_subcommands()
                    .any(|verb| verb.get_name() == method.name.verb)
            });
        if !found {
            missing.push(method.name.dotted());
        }
    }
    missing
}

/// The describe call is the API's own method, so it is not a command: `steg` has no
/// `api describe`. A client that wants the description asks for it.
pub fn describe_is_not_a_command(api: &Description) -> bool {
    api.method(server::DESCRIBE).is_none()
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::api;

    fn split_command() -> Command {
        let api = api::description();
        let method = api
            .method("pane.split")
            .expect("pane.split is in the definition");
        method_command(method)
    }

    #[test]
    fn a_methods_parameters_become_the_positional_the_flags_and_the_program() {
        let command = split_command();
        let names: Vec<&str> = command
            .get_arguments()
            .map(|arg| arg.get_id().as_str())
            .collect();
        assert!(
            names.contains(&"target"),
            "the target parameter is the one positional"
        );
        assert!(names.contains(&"direction"));
        assert!(names.contains(&"working_directory"));
        assert!(names.contains(&"focus"), "a switch keeps its name");
        assert!(
            names.contains(&"no-focus"),
            "a switch also gets its --no- form"
        );
        assert!(names.contains(&"program"));
    }

    #[test]
    fn an_underscore_in_a_parameter_becomes_a_hyphen_in_its_flag() {
        let command = split_command();
        let arg = command
            .get_arguments()
            .find(|arg| arg.get_id() == "working_directory")
            .expect("working_directory is a parameter");
        assert_eq!(arg.get_long(), Some("working-directory"));
    }

    #[test]
    fn what_was_typed_becomes_the_request_the_description_declares() {
        let api = api::description();
        let method = api
            .method("pane.split")
            .expect("pane.split is in the definition");
        let matches = method_command(method)
            .try_get_matches_from(vec![
                "split",
                "logs",
                "--direction",
                "right",
                "--working-directory",
                "/tmp",
                "--focus",
                "--",
                "htop",
                "-d",
                "5",
            ])
            .expect("that is a valid command line");
        let request = request_from_matches(method, &matches);
        assert_eq!(request.method, "pane.split");
        assert_eq!(request.params["target"], json!("logs"));
        assert_eq!(request.params["direction"], json!("right"));
        assert_eq!(request.params["working_directory"], json!("/tmp"));
        assert_eq!(request.params["focus"], json!(true));
        assert_eq!(request.params["program"], json!(["htop", "-d", "5"]));
        assert!(
            !request.params.contains_key("label"),
            "a flag not given is not sent"
        );
    }

    #[test]
    fn the_no_form_of_a_switch_sends_false() {
        let api = api::description();
        let method = api
            .method("pane.set")
            .expect("pane.set is in the definition");
        let matches = method_command(method)
            .try_get_matches_from(vec!["set", "logs", "--no-zoomed"])
            .expect("--no-zoomed is accepted beside --zoomed");
        let request = request_from_matches(method, &matches);
        assert_eq!(request.params["zoomed"], json!(false));
    }

    #[test]
    fn a_word_outside_a_closed_set_is_refused_before_the_server_is_called() {
        let api = api::description();
        let method = api
            .method("pane.split")
            .expect("pane.split is in the definition");
        let outcome =
            method_command(method).try_get_matches_from(vec!["split", "--direction", "sideways"]);
        assert!(
            outcome.is_err(),
            "the choice in the description is the CLI's value parser"
        );
    }

    #[test]
    fn every_method_in_the_description_has_a_command() {
        let api = api::description();
        assert_eq!(
            missing_commands(&api, &command_tree(&api)),
            Vec::<String>::new()
        );
    }

    #[test]
    fn a_method_with_no_command_is_named_by_the_check() {
        let api = api::description();
        // A tree built from a description missing a method is what drift would look like.
        let mut thinner = api.clone();
        thinner.methods.retain(|method| method.name.verb != "split");
        let missing = missing_commands(&api, &command_tree(&thinner));
        assert_eq!(missing, vec!["pane.split".to_owned()]);
    }

    #[test]
    fn the_exit_status_of_an_error_comes_from_the_description() {
        let api = api::description();
        assert_eq!(exit_status(&api, ErrorCode::AmbiguousTarget), 3);
        assert_eq!(exit_status(&api, ErrorCode::BadRequest), 2);
        assert_eq!(exit_status(&api, ErrorCode::ProgramRunning), 1);
    }

    #[test]
    fn the_describe_call_is_not_a_command() {
        assert!(describe_is_not_a_command(&api::description()));
    }
}
