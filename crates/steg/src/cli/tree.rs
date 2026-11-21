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

/// The global flags this task and TASK-1.119 add: not API parameters
/// (confirmed: `description::methods`'s own
/// `no_method_declares_a_cli_only_flag_as_a_parameter` test already lists
/// `json`/`autostart`/`no_autostart` as CLI-only, so no generated command
/// can ever collide with one of these ids), so they are hand-written here
/// rather than generated from a `Param`, each `.global(true)` so clap
/// resolves it from whichever level's own `ArgMatches` a caller reads -
/// including the leaf subcommand actually invoked - with no plumbing
/// needed through `main.rs`'s own top-level matches.
const AUTOSTART_ID: &str = "autostart";
const NO_AUTOSTART_ID: &str = "autostart__no";
const JSON_ID: &str = "json";
/// TASK-1.120's own flag: doc-2 section 5 restricts short flags to
/// `-h`/`-q`/`-v`, so `-q` is the one short form this task introduces.
const QUIET_ID: &str = "quiet";

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
        .arg_required_else_help(false)
        .arg(
            Arg::new(AUTOSTART_ID)
                .long("autostart")
                .action(ArgAction::SetTrue)
                .global(true)
                .overrides_with(NO_AUTOSTART_ID)
                .help("Start a server that is not running (doc-2 section 5; on by default)"),
        )
        .arg(
            Arg::new(NO_AUTOSTART_ID)
                .long("no-autostart")
                .action(ArgAction::SetTrue)
                .global(true)
                .overrides_with(AUTOSTART_ID)
                .help("Exit 4 instead of starting a server that is not running"),
        )
        .arg(
            Arg::new(JSON_ID)
                .long("json")
                .action(ArgAction::SetTrue)
                .global(true)
                .help("Print the server's reply unchanged, as JSON"),
        )
        .arg(
            Arg::new(QUIET_ID)
                .short('q')
                .long("quiet")
                .action(ArgAction::SetTrue)
                .global(true)
                .help("Print less: identifiers only (doc-2 section 5)"),
        );

    for noun in api.nouns() {
        root = root.subcommand(noun_command(api, noun));
    }

    root.subcommand(version_command())
        .subcommand(completion_command())
}

/// Reads the `--autostart`/`--no-autostart` pair back off a leaf
/// `ArgMatches` (TASK-1.119): `true` (the default) unless `--no-autostart`
/// was the last of the pair given anywhere on the command line.
#[must_use]
pub fn autostart_flag(matches: &clap::ArgMatches) -> bool {
    !matches.get_flag(NO_AUTOSTART_ID)
}

/// Reads `--json` back off a leaf `ArgMatches` (TASK-1.119).
#[must_use]
pub fn json_flag(matches: &clap::ArgMatches) -> bool {
    matches.get_flag(JSON_ID)
}

/// Reads `-q`/`--quiet` back off a leaf `ArgMatches` (TASK-1.120).
#[must_use]
pub fn quiet_flag(matches: &clap::ArgMatches) -> bool {
    matches.get_flag(QUIET_ID)
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

#[cfg(test)]
mod tests {
    use super::{autostart_flag, build, json_flag, quiet_flag};

    /// `--no-autostart`'s own `global(true)` is the one part clap's docs
    /// don't spell out as working from *any* level - checked here rather
    /// than assumed, both before the target (`steg --no-autostart session
    /// list`) and after it, on the leaf subcommand itself (`steg session
    /// list --no-autostart`), the two places doc-2 section 5 says a flag
    /// may come.
    #[test]
    fn no_autostart_is_read_whether_given_before_or_after_the_subcommand() {
        let api = steg_sdk::description();

        let before = build(&api)
            .try_get_matches_from(["steg", "--no-autostart", "session", "list"])
            .expect("a global flag before the subcommand should parse");
        let (_, before_leaf) = before.subcommand().expect("session");
        let (_, before_leaf) = before_leaf.subcommand().expect("list");
        assert!(!autostart_flag(before_leaf));

        let after = build(&api)
            .try_get_matches_from(["steg", "session", "list", "--no-autostart"])
            .expect("a global flag after the subcommand should parse");
        let (_, after_leaf) = after.subcommand().expect("session");
        let (_, after_leaf) = after_leaf.subcommand().expect("list");
        assert!(!autostart_flag(after_leaf));
    }

    /// With neither flag given, `autostart_flag` defaults to `true`
    /// (doc-2 section 5: "`--autostart` the default").
    #[test]
    fn autostart_defaults_to_true_with_neither_flag_given() {
        let api = steg_sdk::description();
        let matches = build(&api)
            .try_get_matches_from(["steg", "session", "list"])
            .expect("no autostart flag at all should still parse");
        let (_, leaf) = matches.subcommand().expect("session");
        let (_, leaf) = leaf.subcommand().expect("list");
        assert!(autostart_flag(leaf));
    }

    /// `--autostart` given after `--no-autostart` wins (doc-2 section 5:
    /// "both are always accepted" and the last one given wins - the same
    /// `overrides_with` mechanism every generated switch already uses).
    #[test]
    fn autostart_given_after_no_autostart_wins() {
        let api = steg_sdk::description();
        let matches = build(&api)
            .try_get_matches_from(["steg", "session", "list", "--no-autostart", "--autostart"])
            .expect("both flags together should still parse");
        let (_, leaf) = matches.subcommand().expect("session");
        let (_, leaf) = leaf.subcommand().expect("list");
        assert!(autostart_flag(leaf));
    }

    #[test]
    fn json_is_read_from_the_leaf_subcommand() {
        let api = steg_sdk::description();
        let matches = build(&api)
            .try_get_matches_from(["steg", "session", "list", "--json"])
            .expect("--json should parse");
        let (_, leaf) = matches.subcommand().expect("session");
        let (_, leaf) = leaf.subcommand().expect("list");
        assert!(json_flag(leaf));

        let without = build(&api)
            .try_get_matches_from(["steg", "session", "list"])
            .expect("no --json should still parse");
        let (_, leaf) = without.subcommand().expect("session");
        let (_, leaf) = leaf.subcommand().expect("list");
        assert!(!json_flag(leaf));
    }

    /// Mirrors `json_is_read_from_the_leaf_subcommand`, for `-q`/`--quiet`
    /// (TASK-1.120): both spellings parse, and the flag is absent by
    /// default.
    #[test]
    fn quiet_is_read_from_the_leaf_subcommand_either_spelling() {
        let api = steg_sdk::description();

        let long = build(&api)
            .try_get_matches_from(["steg", "session", "list", "--quiet"])
            .expect("--quiet should parse");
        let (_, leaf) = long.subcommand().expect("session");
        let (_, leaf) = leaf.subcommand().expect("list");
        assert!(quiet_flag(leaf));

        let short = build(&api)
            .try_get_matches_from(["steg", "session", "list", "-q"])
            .expect("-q should parse");
        let (_, leaf) = short.subcommand().expect("session");
        let (_, leaf) = leaf.subcommand().expect("list");
        assert!(quiet_flag(leaf));

        let without = build(&api)
            .try_get_matches_from(["steg", "session", "list"])
            .expect("no -q/--quiet should still parse");
        let (_, leaf) = without.subcommand().expect("session");
        let (_, leaf) = leaf.subcommand().expect("list");
        assert!(!quiet_flag(leaf));
    }
}
