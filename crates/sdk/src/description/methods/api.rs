//! The `api` noun's methods.

use crate::description::shapes::{ErrorCode, Field, Follows, Method, Returns, ValueKind};

use super::shared::{VERSION, field, method, method_target, target_of_kind};

/// The projection `api.list`/`api.show` print in their flat text form: a
/// method's dotted name and its summary. `--json` carries the whole
/// `Method` (params, errors, `since`...), which has more fields than this
/// list — the one place in this description where a served method's
/// actual JSON has more keys than its declared `fields`, because the
/// value really is `Method`, not a record invented to match a projection.
/// Exempted from the `fields`-versus-type cross-check for exactly that
/// reason (see `crate::record`'s test module, and the dispatch task's
/// notes).
pub(super) fn api_method_fields() -> Vec<Field> {
    vec![
        field("method", ValueKind::Text, "The method's dotted name"),
        field("summary", ValueKind::Text, "One line describing it"),
    ]
}

pub(super) fn api_methods() -> Vec<Method> {
    vec![
        Method {
            name: method("api", "list"),
            summary: "Every method, one per line".into(),
            details: String::new(),
            params: Vec::new(),
            returns: Returns::Records {
                fields: api_method_fields(),
            },
            errors: vec![ErrorCode::BadRequest],
            follows: Follows::Nothing,
            since: VERSION,
        },
        Method {
            name: method("api", "show"),
            summary: "One method in full, or the whole API with no target".into(),
            details: "With a method named, its parameters and documentation in full. With no \
                      target, the whole API description — every method, event and error, and \
                      the API's own version — in one answer, for a client that cannot import \
                      the Rust value (decision-1: \"served to anyone who asks\")."
                .into(),
            params: vec![target_of_kind(
                ValueKind::Text,
                false,
                "The method to show, by its dotted name. With none, the whole description",
            )],
            returns: Returns::Record {
                fields: api_method_fields(),
            },
            errors: vec![ErrorCode::BadRequest],
            follows: Follows::Nothing,
            since: VERSION,
        },
        Method {
            name: method("api", "call"),
            summary: "Call any method by name".into(),
            details: "For an agent, and for a method a plugin adds before the CLI knows it. \
                      Never the documented way to do something that has a command of its own. \
                      `method` names which method to call; every other parameter given is that \
                      method's own, by its own declared names — nothing is nested under a second \
                      key, so a method's parameters are never named twice. The selector is \
                      `method`, not the usual `target`, precisely so that the called method's \
                      own `target` (where it has one, such as `pane.split` or `setting.show`) \
                      is a different key and travels through untouched (TASK-1.81)."
                .into(),
            params: vec![method_target("The method to call, by its dotted name")],
            returns: Returns::Text,
            errors: vec![ErrorCode::BadRequest],
            follows: Follows::Nothing,
            since: VERSION,
        },
    ]
}
