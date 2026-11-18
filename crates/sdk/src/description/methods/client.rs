//! The `client` noun's methods.

use crate::description::shapes::{ErrorCode, Field, Follows, Method, Returns, ValueKind};

use super::shared::{VERSION, field, method, required_target, switch};

pub(super) fn client_fields() -> Vec<Field> {
    vec![
        field("id", ValueKind::Text, "The client's full identifier"),
        field(
            "kind",
            ValueKind::Choice(vec!["app".into(), "cli".into(), "plugin".into()]),
            "What kind of client it is",
        ),
        field("label", ValueKind::Text, "Its label, if it has one"),
        field(
            "sessions",
            ValueKind::Text,
            "The sessions it views, as a JSON array of identifiers",
        ),
        field(
            "last_active",
            ValueKind::Text,
            "When it was last active, as ISO 8601 in UTC",
        ),
    ]
}

pub(super) fn client_methods() -> Vec<Method> {
    vec![
        Method {
            name: method("client", "list"),
            summary: "Every client connected to the server".into(),
            details: String::new(),
            params: vec![switch(
                "all",
                "Command line clients as well, not just the app's",
            )],
            returns: Returns::Records {
                fields: client_fields(),
            },
            errors: vec![ErrorCode::BadRequest],
            follows: Follows::Nothing,
            since: VERSION,
        },
        Method {
            name: method("client", "show"),
            summary: "What a client is, and the sessions it views".into(),
            details: String::new(),
            params: vec![required_target("The client to show")],
            returns: Returns::Record {
                fields: client_fields(),
            },
            errors: vec![
                ErrorCode::BadRequest,
                ErrorCode::NoSuchTarget,
                ErrorCode::AmbiguousTarget,
            ],
            follows: Follows::Nothing,
            since: VERSION,
        },
    ]
}
