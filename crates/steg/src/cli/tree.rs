//! Mapping [`Description`] to a `clap::Command` tree, mechanically
//! (decision-1 section 2's own table, doc-2 section 2's transform): one
//! function per shape, so the generator reads as the mapping it is, never
//! as a method's own case.

use clap::{Arg, ArgAction, Command};
use steg_sdk::{Description, Method, Param, Role};

use super::values::value_parser_for;

/// The two commands doc-2 section 2 names as not API methods, and the
/// only ones this module adds outside the loop over `description().
/// methods()`. `tests/cli_drift.rs` asserts the top-level command set is
/// exactly `description().nouns() ∪ NON_METHOD_COMMANDS`, so a third
/// hand-added command - or one of these two going missing - fails it.
pub const NON_METHOD_COMMANDS: &[&str] = &["version", "completion"];

/// Builds the whole tree. Neither `steg` alone nor `steg <noun>` alone
/// requires a subcommand (decision 1 in this task's own plan: doc-2
/// section 2 says both *list*, not fail) - `main.rs`'s own dispatch is
/// what actually prints the list when neither is given; this function
/// only has to leave `ArgMatches::subcommand()` free to come back `None`
/// rather than refusing to parse at all.
#[must_use]
pub fn build(api: &Description) -> Command {
    let mut root = Command::new("steg")
        .about("A terminal, driven from the command line.")
        .disable_help_subcommand(true)
        .subcommand_required(false)
        .arg_required_else_help(false);

    for noun in api.nouns() {
        root = root.subcommand(noun_command(api, noun));
    }

    root.subcommand(version_command())
        .subcommand(completion_command())
}

fn noun_command(api: &Description, noun: &str) -> Command {
    let mut command = Command::new(noun.to_owned())
        .disable_help_subcommand(true)
        .subcommand_required(false)
        .arg_required_else_help(false);
    for method in api.verbs_of(noun) {
        command = command.subcommand(verb_command(method));
    }
    command
}

fn verb_command(method: &Method) -> Command {
    let mut command = Command::new(method.name.verb.clone()).about(method.summary.clone());
    if !method.details.is_empty() {
        command = command.long_about(method.details.clone());
    }
    for param in &method.params {
        command = add_param(command, param);
    }
    command
}

/// `Role` says where the CLI puts a parameter (doc-2 sections 2, 4, 5
/// and 9); this is the whole of that mapping.
fn add_param(command: Command, param: &Param) -> Command {
    match param.role {
        Role::Target => command.arg(
            Arg::new(param.name.clone())
                .value_name(param.name.to_uppercase())
                .required(param.required)
                .value_parser(value_parser_for(&param.value))
                .help(param.summary.clone()),
        ),
        Role::Flag => command.arg(
            Arg::new(param.name.clone())
                .long(param.flag())
                .required(param.required)
                .value_parser(value_parser_for(&param.value))
                .help(param.summary.clone()),
        ),
        Role::Switch => add_switch(command, param),
        // "A program to run is given after `--`" (doc-2 section 9),
        // following `env`, `ssh` and `sudo` - clap's own `.last(true)` is
        // its documented idiom for exactly this shape.
        Role::Program => command.arg(
            Arg::new(param.name.clone())
                .value_name("PROGRAM")
                .num_args(0..)
                .last(true)
                .value_parser(value_parser_for(&param.value))
                .help(param.summary.clone()),
        ),
    }
}

/// `--name` and `--no-name`, both always accepted (doc-2 section 5), each
/// marked to override the other so whichever is given last on the command
/// line wins and neither is required - proved directly (not merely
/// asserted) by `tests/cli_drift.rs`'s own switch test, since clap's
/// `overrides_with` is the one part of this mapping doc-2's own text does
/// not spell out as a mechanism.
fn add_switch(command: Command, param: &Param) -> Command {
    let on_id = param.name.clone();
    let off_id = format!("{on_id}__no");
    command
        .arg(
            Arg::new(on_id.clone())
                .long(param.flag())
                .action(ArgAction::SetTrue)
                .overrides_with(off_id.clone())
                .help(param.summary.clone()),
        )
        .arg(
            Arg::new(off_id)
                .long(param.negated_flag())
                .action(ArgAction::SetTrue)
                .overrides_with(on_id)
                .help(param.summary.clone()),
        )
}

fn version_command() -> Command {
    Command::new("version").about("Print the version. Not an API method; contacts no server.")
}

fn completion_command() -> Command {
    Command::new("completion")
        .about("Print a shell's completion script. Not an API method; contacts no server.")
        .arg(
            Arg::new("shell")
                .value_parser(clap::value_parser!(clap_complete::Shell))
                .required(true)
                .help("The shell to generate a completion script for"),
        )
}
