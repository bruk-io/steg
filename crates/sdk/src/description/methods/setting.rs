//! The `setting` noun's methods.

use crate::description::shapes::{ErrorCode, Field, Follows, Method, Returns, ValueKind};

use super::shared::{VERSION, field, flag, method, target_of_kind};

/// `setting.list`/`setting.show`'s own row: a name, its current value
/// (the override if one is set, else the default) and its default. Every
/// field is `Text`: a setting's value is never typed per entry on the
/// wire (`crate::setting`'s module doc; TASK-1.48's plan).
pub(super) fn setting_fields() -> Vec<Field> {
    vec![
        field("name", ValueKind::Text, "The setting's name"),
        field(
            "value",
            ValueKind::Text,
            "Its current value: the override if one is set, else the default",
        ),
        field("default", ValueKind::Text, "Its value with no override"),
    ]
}

pub(super) fn setting_methods() -> Vec<Method> {
    vec![
        Method {
            name: method("setting", "list"),
            summary: "Every setting, its current value and its default".into(),
            details: String::new(),
            params: Vec::new(),
            returns: Returns::Records {
                fields: setting_fields(),
            },
            errors: vec![ErrorCode::BadRequest],
            follows: Follows::Nothing,
            since: VERSION,
        },
        Method {
            name: method("setting", "show"),
            summary: "One setting: its name, current value and default".into(),
            details: String::new(),
            params: vec![target_of_kind(
                ValueKind::Text,
                true,
                "The setting to show, by its name",
            )],
            returns: Returns::Record {
                fields: setting_fields(),
            },
            errors: vec![ErrorCode::BadRequest, ErrorCode::NoSuchTarget],
            follows: Follows::Nothing,
            since: VERSION,
        },
        Method {
            name: method("setting", "set"),
            summary: "Change a setting's value".into(),
            details: "The setting is named by the positional and its new value is the \
                      flag, since here the property is the thing addressed (doc-2 \
                      section 8) — unlike `session set --label`, where the target is the \
                      session and the property is its own flag."
                .into(),
            params: vec![
                target_of_kind(ValueKind::Text, true, "The setting to change, by its name"),
                flag("value", ValueKind::Text, true, "The setting's new value"),
            ],
            returns: Returns::Nothing,
            errors: vec![ErrorCode::BadRequest, ErrorCode::NoSuchTarget],
            follows: Follows::Nothing,
            since: VERSION,
        },
        Method {
            name: method("setting", "unset"),
            summary: "Clear a setting's override, reverting it to its default".into(),
            details: String::new(),
            params: vec![target_of_kind(
                ValueKind::Text,
                true,
                "The setting to change, by its name",
            )],
            returns: Returns::Nothing,
            errors: vec![ErrorCode::BadRequest, ErrorCode::NoSuchTarget],
            follows: Follows::Nothing,
            since: VERSION,
        },
    ]
}
