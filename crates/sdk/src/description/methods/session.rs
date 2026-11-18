//! The `session` noun's methods.

use crate::description::shapes::{ErrorCode, Field, Follows, Method, Returns, ValueKind};

use super::shared::{VERSION, field, flag, method, program, switch, target};

/// doc-14's session row: `id`, `label`, `position`, `colour`,
/// `current_tab`. Exactly `SessionRecord`'s fields (`crate::record`) — the
/// one place this module is checked against a real type, by
/// `crate::record::tests`.
pub(super) fn session_fields() -> Vec<Field> {
    vec![
        field("id", ValueKind::Text, "The session's full identifier"),
        field("label", ValueKind::Text, "Its label, if it has one"),
        field(
            "position",
            ValueKind::Integer,
            "Its one-based position among the server's sessions",
        ),
        field(
            "colour",
            ValueKind::Choice(vec![
                "violet".into(),
                "pink".into(),
                "orange".into(),
                "blue".into(),
                "green".into(),
            ]),
            "Its colour",
        ),
        field(
            "current_tab",
            ValueKind::Text,
            "The identifier of its current tab",
        ),
    ]
}

pub(super) fn session_methods() -> Vec<Method> {
    vec![
        Method {
            name: method("session", "list"),
            summary: "Every session on the host".into(),
            details: "One record per session, in the server's own order (R6).".into(),
            params: Vec::new(),
            returns: Returns::Records {
                fields: session_fields(),
            },
            errors: vec![ErrorCode::BadRequest],
            follows: Follows::Nothing,
            since: VERSION,
        },
        Method {
            name: method("session", "show"),
            summary: "One session, in full".into(),
            details: String::new(),
            params: vec![target(
                "The session to show. Defaults to the current session",
            )],
            returns: Returns::Record {
                fields: session_fields(),
            },
            errors: vec![
                ErrorCode::BadRequest,
                ErrorCode::NoSuchTarget,
                ErrorCode::AmbiguousTarget,
            ],
            follows: Follows::Nothing,
            since: VERSION,
        },
        Method {
            name: method("session", "new"),
            summary: "Create a session".into(),
            details: "The target is none: a session has no parent. The new session's \
                      identifier is the result, so a script can capture it."
                .into(),
            params: vec![
                flag("label", ValueKind::Text, false, "The new session's label"),
                flag(
                    "cwd",
                    ValueKind::Path,
                    false,
                    "The new program's working directory",
                ),
                switch(
                    "focus",
                    "Move keyboard focus to the new session. It does by default",
                ),
                program(),
            ],
            returns: Returns::Identifier {
                of: "session".into(),
            },
            errors: vec![ErrorCode::BadRequest, ErrorCode::ProgramFailed],
            follows: Follows::Nothing,
            since: VERSION,
        },
        Method {
            name: method("session", "close"),
            summary: "End a session, and whatever runs in it".into(),
            details: String::new(),
            params: vec![
                target("The session to close. Defaults to the current session"),
                switch("force", "Close it even though programs are still running"),
            ],
            returns: Returns::Nothing,
            errors: vec![
                ErrorCode::BadRequest,
                ErrorCode::NoSuchTarget,
                ErrorCode::AmbiguousTarget,
                ErrorCode::ProgramRunning,
            ],
            follows: Follows::Nothing,
            since: VERSION,
        },
        Method {
            name: method("session", "move"),
            summary: "Reorder a session among the server's sessions".into(),
            details: String::new(),
            params: vec![
                target("The session to move. Defaults to the current session"),
                flag(
                    "position",
                    ValueKind::Integer,
                    true,
                    "Where it goes among the server's sessions",
                ),
            ],
            returns: Returns::Nothing,
            errors: vec![
                ErrorCode::BadRequest,
                ErrorCode::NoSuchTarget,
                ErrorCode::AmbiguousTarget,
                ErrorCode::PositionOutOfRange,
            ],
            follows: Follows::Nothing,
            since: VERSION,
        },
        Method {
            name: method("session", "set"),
            summary: "Change a session's properties".into(),
            details: "Each property is a flag. A property left out is unchanged.".into(),
            params: vec![
                target("The session to change. Defaults to the current session"),
                flag("label", ValueKind::Text, false, "The session's label"),
            ],
            returns: Returns::Nothing,
            errors: vec![
                ErrorCode::BadRequest,
                ErrorCode::NoSuchTarget,
                ErrorCode::AmbiguousTarget,
            ],
            follows: Follows::Nothing,
            since: VERSION,
        },
        Method {
            name: method("session", "unset"),
            summary: "Clear a session's properties".into(),
            details: String::new(),
            params: vec![
                target("The session to change. Defaults to the current session"),
                switch("label", "The session's label"),
            ],
            returns: Returns::Nothing,
            errors: vec![
                ErrorCode::BadRequest,
                ErrorCode::NoSuchTarget,
                ErrorCode::AmbiguousTarget,
            ],
            follows: Follows::Nothing,
            since: VERSION,
        },
        Method {
            name: method("session", "focus"),
            summary: "Ask a client to show this session".into(),
            details: "A request to one client (R87), not a change to any server-held focus \
                      (R88). The caller's request succeeds once the event naming the client \
                      is queued; the server does not wait for it to be obeyed."
                .into(),
            params: vec![
                target("The session to show. Defaults to the current session"),
                flag(
                    "client",
                    ValueKind::Target,
                    false,
                    "The client to ask. Defaults to the app's client when exactly one is \
                     connected",
                ),
            ],
            returns: Returns::Nothing,
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
