//! Criterion #1: every API method has a command and every command an API
//! method, checked by a test that fails when they drift (R32). A set
//! comparison over the description and the generated tree, never a
//! method count - TASK-1.49's own dependency graph does not include
//! TASK-1.46/1.47, whose merge moves the count from 42 to 44, and this
//! test must keep passing unchanged either way.

use std::collections::HashSet;

use clap::Command;
use serde_json::json;
use steg::cli::{NON_METHOD_COMMANDS, build, extract_params};
use steg_sdk::{Description, description};

type Pair = (String, String);

fn described_pairs(api: &Description) -> HashSet<Pair> {
    api.methods
        .iter()
        .map(|m| (m.name.noun.clone(), m.name.verb.clone()))
        .collect()
}

/// Walks the built tree's own two levels: noun, then verb - the same
/// shape `description().nouns()`/`verbs_of` reads. `version`/`completion`
/// are collected separately, not as `(noun, verb)` pairs, since neither
/// is a method at all (doc-2 section 2).
fn generated_pairs(tree: &Command) -> (HashSet<Pair>, HashSet<String>) {
    let mut pairs = HashSet::new();
    let mut non_method_top_level = HashSet::new();
    for noun_command in tree.get_subcommands() {
        let noun = noun_command.get_name().to_owned();
        if NON_METHOD_COMMANDS.contains(&noun.as_str()) {
            non_method_top_level.insert(noun);
            continue;
        }
        for verb_command in noun_command.get_subcommands() {
            pairs.insert((noun.clone(), verb_command.get_name().to_owned()));
        }
    }
    (pairs, non_method_top_level)
}

/// The whole of AC #1, as a pure function over two sets - independent of
/// clap, so the two negative controls below can drive it with synthetic
/// data and prove it actually catches drift, not merely that it ran.
fn drift(described: &HashSet<Pair>, generated: &HashSet<Pair>) -> (Vec<Pair>, Vec<Pair>) {
    let mut missing: Vec<Pair> = described.difference(generated).cloned().collect();
    let mut extra: Vec<Pair> = generated.difference(described).cloned().collect();
    missing.sort();
    extra.sort();
    (missing, extra)
}

#[test]
fn every_described_method_has_a_generated_command_and_nothing_else() {
    let api = description();
    let tree = build(&api);
    let (generated, non_method_top_level) = generated_pairs(&tree);
    let described = described_pairs(&api);

    let (missing, extra) = drift(&described, &generated);
    assert!(
        missing.is_empty(),
        "methods with no generated command: {missing:?}"
    );
    assert!(
        extra.is_empty(),
        "generated commands with no method: {extra:?}"
    );

    let expected_non_method: HashSet<String> = NON_METHOD_COMMANDS
        .iter()
        .map(|s| (*s).to_owned())
        .collect();
    assert_eq!(
        non_method_top_level, expected_non_method,
        "the only top-level commands that are not nouns must be exactly NON_METHOD_COMMANDS"
    );
}

#[test]
fn non_method_commands_is_exactly_version_and_completion() {
    assert_eq!(NON_METHOD_COMMANDS, &["version", "completion"]);
}

/// Negative control: proves `drift` actually detects a method with no
/// command, rather than passing vacuously.
#[test]
fn drift_catches_a_method_with_no_command() {
    let described: HashSet<Pair> = [
        ("pane".to_owned(), "split".to_owned()),
        ("pane".to_owned(), "close".to_owned()),
    ]
    .into();
    let generated: HashSet<Pair> = [("pane".to_owned(), "split".to_owned())].into();

    let (missing, extra) = drift(&described, &generated);
    assert_eq!(missing, vec![("pane".to_owned(), "close".to_owned())]);
    assert!(extra.is_empty());
}

/// Negative control: proves `drift` actually detects a command with no
/// method, rather than passing vacuously.
#[test]
fn drift_catches_a_command_with_no_method() {
    let described: HashSet<Pair> = [("pane".to_owned(), "split".to_owned())].into();
    let generated: HashSet<Pair> = [
        ("pane".to_owned(), "split".to_owned()),
        ("pane".to_owned(), "bogus".to_owned()),
    ]
    .into();

    let (missing, extra) = drift(&described, &generated);
    assert!(missing.is_empty());
    assert_eq!(extra, vec![("pane".to_owned(), "bogus".to_owned())]);
}

// --- the switch mapping (doc-2 section 5) -----------------------------

fn parse(args: &[&str]) -> (String, String, clap::ArgMatches) {
    let api = description();
    let matches = build(&api)
        .try_get_matches_from(std::iter::once("steg").chain(args.iter().copied()))
        .unwrap_or_else(|e| panic!("{args:?} should parse: {e}"));
    let (noun, noun_matches) = matches.subcommand().expect("a noun");
    let (verb, verb_matches) = noun_matches.subcommand().expect("a verb");
    (noun.to_owned(), verb.to_owned(), verb_matches.clone())
}

#[test]
fn a_switchs_last_occurrence_on_the_command_line_wins() {
    let api = description();

    let (noun, verb, matches) = parse(&["pane", "unset", "--no-label", "--label"]);
    let method = api.method(&format!("{noun}.{verb}")).unwrap();
    let params = extract_params(method, &matches);
    assert_eq!(
        params["label"],
        json!(true),
        "--label given last should win"
    );

    let (noun, verb, matches) = parse(&["pane", "unset", "--label", "--no-label"]);
    let method = api.method(&format!("{noun}.{verb}")).unwrap();
    let params = extract_params(method, &matches);
    assert_eq!(
        params["label"],
        json!(false),
        "--no-label given last should win"
    );
}

#[test]
fn a_switch_given_neither_way_is_absent_from_params() {
    let api = description();
    let (noun, verb, matches) = parse(&["pane", "unset"]);
    let method = api.method(&format!("{noun}.{verb}")).unwrap();
    let params = extract_params(method, &matches);
    assert!(
        !params.contains_key("label"),
        "a property left out must stay absent, not default to a value (doc-2 section 3)"
    );
}
