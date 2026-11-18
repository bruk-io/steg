//! The `tab` noun's methods.

use crate::description::shapes::{ErrorCode, Field, Follows, Method, Returns, ValueKind};

use super::shared::{VERSION, field, flag, method, program, switch, target};

/// doc-14's tab row. `layout` and `shown_status` are already structured
/// values with real Rust types ([`crate::Layout`]; `shown_status` is a
/// pane id or none, computed by `steg_kernel::Tree::tab_shown_status`),
/// backed by `crate::TabRecord` (TASK-1.89), but `ValueKind` (this
/// module's own vocabulary for a field's kind) has no variant for a
/// nested or structured value at all — so both stay `ValueKind::Text`
/// placeholders, named as a follow-up (a new `ValueKind` variant) rather
/// than fixed here; not shown in a flat text table either way.
pub(super) fn tab_fields() -> Vec<Field> {
    vec![
        field("id", ValueKind::Text, "The tab's full identifier"),
        field("label", ValueKind::Text, "Its label, if it has one"),
        field("title", ValueKind::Text, "Its title, if it has one"),
        field(
            "position",
            ValueKind::Integer,
            "Its one-based position among its session's tabs",
        ),
        field(
            "current_pane",
            ValueKind::Text,
            "The identifier of its current pane",
        ),
        field(
            "layout",
            ValueKind::Text,
            "Its layout tree, as its own JSON shape (doc-14)",
        ),
        field(
            "shown_status",
            ValueKind::Text,
            "The pane whose status this tab's ring stands for, if any",
        ),
    ]
}

pub(super) fn tab_methods() -> Vec<Method> {
    vec![
        Method {
            name: method("tab", "list"),
            summary: "The tabs of a session".into(),
            details: String::new(),
            params: vec![target(
                "The session whose tabs are listed. Defaults to the current session",
            )],
            returns: Returns::Records {
                fields: tab_fields(),
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
            name: method("tab", "show"),
            summary: "One tab, in full".into(),
            details: String::new(),
            params: vec![target("The tab to show. Defaults to the current tab")],
            returns: Returns::Record {
                fields: tab_fields(),
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
            name: method("tab", "new"),
            summary: "Create a tab in a session".into(),
            details: "The target is the session the tab is created in. The new tab's \
                      identifier is the result, so a script can capture it."
                .into(),
            params: vec![
                target("The session to create the tab in. Defaults to the current session"),
                flag("label", ValueKind::Text, false, "The new tab's label"),
                flag(
                    "cwd",
                    ValueKind::Path,
                    false,
                    "The new program's working directory",
                ),
                flag(
                    "position",
                    ValueKind::Integer,
                    false,
                    "Where among its siblings the new tab goes. Defaults to the end",
                ),
                switch(
                    "focus",
                    "Move keyboard focus to the new tab. It does by default",
                ),
                program(),
            ],
            returns: Returns::Identifier { of: "tab".into() },
            errors: vec![
                ErrorCode::BadRequest,
                ErrorCode::NoSuchTarget,
                ErrorCode::AmbiguousTarget,
                ErrorCode::ProgramFailed,
            ],
            follows: Follows::Nothing,
            since: VERSION,
        },
        Method {
            name: method("tab", "close"),
            summary: "End a tab, and whatever runs in it".into(),
            details: String::new(),
            params: vec![
                target("The tab to close. Defaults to the current tab"),
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
            name: method("tab", "move"),
            summary: "Reorder a tab, or move it to another session".into(),
            details: String::new(),
            params: vec![
                target("The tab to move. Defaults to the current tab"),
                flag(
                    "position",
                    ValueKind::Integer,
                    false,
                    "Where it goes among its siblings",
                ),
                flag(
                    "to",
                    ValueKind::Target,
                    false,
                    "The session to move it to. Defaults to its own session",
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
            name: method("tab", "set"),
            summary: "Change a tab's properties".into(),
            details: "Each property is a flag. A property left out is unchanged.".into(),
            params: vec![
                target("The tab to change. Defaults to the current tab"),
                flag("label", ValueKind::Text, false, "The tab's label"),
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
            name: method("tab", "unset"),
            summary: "Clear a tab's properties".into(),
            details: String::new(),
            params: vec![
                target("The tab to change. Defaults to the current tab"),
                switch("label", "The tab's label"),
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
            name: method("tab", "focus"),
            summary: "Make a tab the current tab of its session".into(),
            details: "Does not change the tab's own current pane (decision-5).".into(),
            params: vec![target("The tab to focus. Defaults to the current tab")],
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
            name: method("tab", "declare-size"),
            summary: "Declare the caller's own size for the tab".into(),
            details: "The size is the calling client's for the target tab, in whole cells. \
                      The terminal's actual size follows doc-15 R91/decision-6 (the latest \
                      client - the last one to send input or declare a size - wins); every \
                      attached client is told the size in force on its own attachment's \
                      stream once it changes, not only the caller (TASK-1.114)."
                .into(),
            params: vec![
                target("The tab to declare a size for. Defaults to the current tab"),
                flag(
                    "columns",
                    ValueKind::Integer,
                    true,
                    "The declared width, in whole cells. Must be at least 1",
                ),
                flag(
                    "rows",
                    ValueKind::Integer,
                    true,
                    "The declared height, in whole cells. Must be at least 1",
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
