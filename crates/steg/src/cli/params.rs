//! Turning a parsed command's `ArgMatches` into the method's own
//! parameters, by name - the same shape `steg_sdk::description::Request::
//! params` takes. Shared by this task's own placeholder ([`super::run`])
//! and, unchanged, by TASK-1.49b's real `Connection::call` once it
//! replaces the placeholder: this is calling logic that does not change
//! between the two.

use clap::ArgMatches;
use serde_json::{Map, Value};
use steg_sdk::{Method, Role, ValueKind};

/// Reads every one of `method`'s params out of `matches`, inserting a key
/// only when the command line actually gave it a value - absence here is
/// what lets dispatch's own "a property left out is unchanged" rule
/// (doc-2 section 3) and "defaults to the login shell" (program) work
/// with no extra code on this side.
#[must_use]
pub fn extract_params(method: &Method, matches: &ArgMatches) -> Map<String, Value> {
    let mut params = Map::new();
    for param in &method.params {
        let value = match param.role {
            Role::Switch => switch_value(matches, &param.name).map(Value::Bool),
            Role::Program => program_value(matches, &param.name),
            Role::Target | Role::Flag => scalar_value(matches, &param.name, &param.value),
        };
        if let Some(value) = value {
            params.insert(param.name.clone(), value);
        }
    }
    params
}

/// `None` when neither `--name` nor `--no-name` was given at all - the
/// one case that must stay absent from `params`, so `set --zoomed` a
/// property leaves it unchanged (doc-2 section 3) rather than resetting
/// it to a default. `Some(true)`/`Some(false)` otherwise, from whichever
/// of the pair `tree::add_switch`'s own `overrides_with` left set.
fn switch_value(matches: &ArgMatches, name: &str) -> Option<bool> {
    let off_id = format!("{name}__no");
    if matches.get_flag(&off_id) {
        Some(false)
    } else if matches.get_flag(name) {
        Some(true)
    } else {
        None
    }
}

/// `None` when no program was given after `--` at all (so dispatch's own
/// "defaults to the login shell" applies); `Some` with a JSON array of
/// its words otherwise, even an empty `--` with nothing following it -
/// doc-2 does not distinguish those two, and neither does this.
fn program_value(matches: &ArgMatches, name: &str) -> Option<Value> {
    let words: Vec<Value> = matches
        .get_many::<String>(name)?
        .map(|word| Value::String(word.clone()))
        .collect();
    Some(Value::Array(words))
}

fn scalar_value(matches: &ArgMatches, name: &str, kind: &ValueKind) -> Option<Value> {
    match kind {
        ValueKind::Integer => matches.get_one::<i64>(name).copied().map(Value::from),
        ValueKind::Boolean => matches.get_one::<bool>(name).copied().map(Value::Bool),
        ValueKind::Text
        | ValueKind::Target
        | ValueKind::Path
        | ValueKind::Argv
        | ValueKind::Choice(_) => matches.get_one::<String>(name).cloned().map(Value::String),
    }
}
