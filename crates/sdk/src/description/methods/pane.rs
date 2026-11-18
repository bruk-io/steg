//! The `pane` noun's methods.

use crate::description::shapes::{ErrorCode, Field, Follows, Method, Returns, ValueKind};

use super::shared::{VERSION, direction, field, flag, method, program, switch, target};

/// doc-14's pane row. `status` has a real Rust type, [`crate::Status`]
/// (`Running`, `Finished` or `Failed{exit_code}`, doc-11) or none, backed
/// by `crate::PaneRecord.status` (TASK-1.89) - but `ValueKind` (this
/// module's own vocabulary for a field's kind) has no variant for a
/// nested or structured value at all, the same reason `exit_code`'s
/// sibling field `program` ([`crate::Program`]) is `ValueKind::Text` too,
/// so `status`/`size` stay `ValueKind::Text` placeholders, named as a
/// follow-up rather than fixed here (`tab_fields`'s own doc comment makes
/// the same choice for `layout`/`shown_status`).
pub(super) fn pane_fields() -> Vec<Field> {
    vec![
        field("id", ValueKind::Text, "The pane's full identifier"),
        field("label", ValueKind::Text, "Its label, if it has one"),
        field("title", ValueKind::Text, "Its title, if it has one"),
        field(
            "kind",
            ValueKind::Choice(vec!["terminal".into()]),
            "What kind of thing it runs",
        ),
        field("cwd", ValueKind::Path, "Its program's working directory"),
        field("program", ValueKind::Text, "What runs in it"),
        field(
            "exit_code",
            ValueKind::Integer,
            "Its program's exit code, once it has ended",
        ),
        field(
            "program_status",
            ValueKind::Text,
            "The one clock-free fact about its program right now: running, or ended with \
             its exit code",
        ),
        field(
            "status",
            ValueKind::Text,
            "What steg knows about its program right now (doc-11), if anything",
        ),
        field(
            "size",
            ValueKind::Text,
            "Its terminal's size in whole cells, if a client has declared one for its tab",
        ),
    ]
}

pub(super) fn pane_methods() -> Vec<Method> {
    vec![
        Method {
            name: method("pane", "list"),
            summary: "The panes of a tab".into(),
            details: "In R112's order: the layout tree's leaves, left to right.".into(),
            params: vec![target(
                "The tab whose panes are listed. Defaults to the current tab",
            )],
            returns: Returns::Records {
                fields: pane_fields(),
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
            name: method("pane", "show"),
            summary: "One pane, in full".into(),
            details: String::new(),
            params: vec![target("The pane to show. Defaults to the current pane")],
            returns: Returns::Record {
                fields: pane_fields(),
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
            name: method("pane", "split"),
            summary: "Split a pane and run a program in the new one".into(),
            details: "Divides the target pane in the given direction and starts a program in \
                      the new pane. The new pane's identifier is the result, so a script can \
                      capture it. Focus does not move unless it is asked for."
                .into(),
            params: vec![
                target("The pane to split. Defaults to the current pane"),
                flag(
                    "direction",
                    direction(),
                    true,
                    "Which side of the target the new pane takes",
                ),
                flag(
                    "cwd",
                    ValueKind::Path,
                    false,
                    "The new program's working directory",
                ),
                flag("label", ValueKind::Text, false, "A label for the new pane"),
                switch(
                    "focus",
                    "Move keyboard focus to the new pane. It does by default",
                ),
                program(),
            ],
            returns: Returns::Identifier { of: "pane".into() },
            errors: vec![
                ErrorCode::BadRequest,
                ErrorCode::NoSuchTarget,
                ErrorCode::AmbiguousTarget,
                ErrorCode::SplitTooSmall,
                ErrorCode::ProgramFailed,
            ],
            follows: Follows::Nothing,
            since: VERSION,
        },
        Method {
            name: method("pane", "close"),
            summary: "End a pane, and whatever runs in it".into(),
            details: String::new(),
            params: vec![
                target("The pane to close. Defaults to the current pane"),
                switch("force", "Close it even though a program is still running"),
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
            name: method("pane", "move"),
            summary: "Move a pane to another tab".into(),
            details: String::new(),
            params: vec![
                target("The pane to move. Defaults to the current pane"),
                flag("to", ValueKind::Target, true, "The tab to move it to"),
            ],
            returns: Returns::Nothing,
            errors: vec![
                ErrorCode::BadRequest,
                ErrorCode::NoSuchTarget,
                ErrorCode::AmbiguousTarget,
                ErrorCode::AlreadyInTab,
                ErrorCode::SplitTooSmall,
            ],
            follows: Follows::Nothing,
            since: VERSION,
        },
        Method {
            name: method("pane", "set"),
            summary: "Change a pane's properties".into(),
            details: "Each property is a flag. A property left out is unchanged.".into(),
            params: vec![
                target("The pane to change. Defaults to the current pane"),
                flag("label", ValueKind::Text, false, "The pane's label"),
                switch("zoomed", "Whether the pane fills its tab"),
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
            name: method("pane", "unset"),
            summary: "Clear a pane's properties".into(),
            details: String::new(),
            params: vec![
                target("The pane to change. Defaults to the current pane"),
                switch("label", "The pane's label"),
                switch("zoomed", "Whether the pane fills its tab"),
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
            name: method("pane", "focus"),
            summary: "Make a pane the current pane of its tab".into(),
            details: "With `direction`, the target is the pane the move starts from, and the \
                      pane that gains focus is its neighbour; at the edge of the layout focus \
                      does not move and this exits 0, and `wrap` continues from the other side."
                .into(),
            params: vec![
                target("The pane to focus, or to move from with `direction`"),
                flag("direction", direction(), false, "Which way to move focus"),
                switch(
                    "wrap",
                    "With `direction`, continue from the other side at the edge",
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
        Method {
            name: method("pane", "resize"),
            summary: "Move the border of a pane's split".into(),
            details: String::new(),
            params: vec![
                target("The pane to resize. Defaults to the current pane"),
                flag(
                    "direction",
                    direction(),
                    true,
                    "Which side's border to move",
                ),
                flag(
                    "by",
                    ValueKind::Integer,
                    true,
                    "How far to move the border, in character cells",
                ),
            ],
            returns: Returns::Nothing,
            errors: vec![
                ErrorCode::BadRequest,
                ErrorCode::NoSuchTarget,
                ErrorCode::AmbiguousTarget,
                ErrorCode::NoSplitInDirection,
            ],
            follows: Follows::Nothing,
            since: VERSION,
        },
        Method {
            name: method("pane", "swap"),
            summary: "Exchange two panes' positions".into(),
            details: String::new(),
            params: vec![
                target("The pane to swap. Defaults to the current pane"),
                flag(
                    "with",
                    ValueKind::Target,
                    true,
                    "The pane to exchange places with",
                ),
            ],
            returns: Returns::Nothing,
            errors: vec![
                ErrorCode::BadRequest,
                ErrorCode::NoSuchTarget,
                ErrorCode::AmbiguousTarget,
                ErrorCode::PanesNotInSameTab,
            ],
            follows: Follows::Nothing,
            since: VERSION,
        },
        Method {
            name: method("pane", "send"),
            summary: "Send text or keys to the program in a pane".into(),
            details: "With neither `text` nor `key`, the CLI reads standard input to its end \
                      and sends that."
                .into(),
            params: vec![
                target("The pane to send to. Defaults to the current pane"),
                flag("text", ValueKind::Text, false, "Text to send"),
                flag(
                    "key",
                    ValueKind::Choice(crate::keys::key_names()),
                    false,
                    "A key to send, by name. In M1 a call sends at most one key - doc-2's \
                     own 'may be given more than once' waits on a list-valued flag kind",
                ),
            ],
            returns: Returns::Nothing,
            errors: vec![
                ErrorCode::BadRequest,
                ErrorCode::NoSuchTarget,
                ErrorCode::AmbiguousTarget,
                ErrorCode::ProgramNotRunning,
            ],
            follows: Follows::Nothing,
            since: VERSION,
        },
        Method {
            name: method("pane", "capture"),
            summary: "Print a pane's screen, and scrollback when asked".into(),
            details: String::new(),
            params: vec![
                target("The pane to capture. Defaults to the current pane"),
                switch("scrollback", "Include scrollback, not just the screen"),
            ],
            returns: Returns::Text,
            errors: vec![
                ErrorCode::BadRequest,
                ErrorCode::NoSuchTarget,
                ErrorCode::AmbiguousTarget,
            ],
            follows: Follows::Nothing,
            since: VERSION,
        },
        Method {
            name: method("pane", "wait"),
            summary: "Block until a named condition, then exit".into(),
            details: "Exits 0 once the condition is met; a wait that times out is status 1, \
                      which is a script's one way to tell \"the wait failed\" from \"the \
                      program failed\" (doc-2 section 6). doc-2 section 8 gives `--for` one \
                      value in the first release, `exit`, for which section 6 says this \
                      prints the program's own exit code. `exit` is this method's own word for \
                      that condition, not the dotted `pane.exited` event name event.watch's \
                      `--for` uses - doc-2 section 5 says both draw from \"the API description's \
                      event records\", but section 8's own row for this method writes `exit` \
                      literally, so that is what is declared here rather than guessed; if a \
                      shared vocabulary across the two commands is intended, that is doc-2's \
                      to settle, not this description's to invent."
                .into(),
            params: vec![
                target("The pane to wait on. Defaults to the current pane"),
                flag(
                    "for",
                    ValueKind::Choice(vec!["exit".into()]),
                    true,
                    "What to wait for. The first release has one condition, `exit`",
                ),
                flag(
                    "timeout",
                    ValueKind::Integer,
                    false,
                    "Give up after this long, with status 1",
                ),
            ],
            returns: Returns::Record {
                fields: vec![field(
                    "exit_code",
                    ValueKind::Integer,
                    "The program's own exit code",
                )],
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
            name: method("pane", "attach"),
            summary: "Opens a stream to the pane's screen".into(),
            details: "The reply is the stream's id and its opening snapshot; updates that \
                      follow arrive on that stream, not as this call's own output. The last \
                      update the stream ever carries is `Ended`, sent once the pane's program \
                      exits or the pane itself is closed, after which the stream is closed on \
                      both sides (doc-14, \"The end of a pane, on its stream\"). `steg pane \
                      attach` exists for completeness (R32) and for scripting through `api \
                      call` - it prints the id and the snapshot once and exits, the same as \
                      any other command; a program that wants the stream itself (the app, a \
                      plugin) uses the sdk's `Connection` directly, as decision-2 and doc-16 \
                      R101 already describe."
                .into(),
            params: vec![target(
                "The pane to attach to. Defaults to the current pane",
            )],
            returns: Returns::Nested {
                of: "PaneAttached".into(),
                summary: "the new stream's id, and the pane's screen at the moment of \
                          attaching"
                    .into(),
            },
            follows: Follows::Attachment {
                item: Returns::Nested {
                    of: "ScreenUpdate".into(),
                    summary: "the rows that changed, a whole screen when a client fell \
                              behind, or the pane ending"
                        .into(),
                },
                stream_id_field: "stream".into(),
            },
            errors: vec![
                ErrorCode::BadRequest,
                ErrorCode::NoSuchTarget,
                ErrorCode::AmbiguousTarget,
            ],
            since: VERSION,
        },
        Method {
            name: method("pane", "detach"),
            summary: "Ends this connection's attachment to the pane".into(),
            details: "A no-op from the client's own point of view either way - the attachment \
                      is gone once this returns - but refused with `BadRequest` when this \
                      connection has no attachment to the target pane at all, so a mistaken \
                      target is not silently accepted."
                .into(),
            params: vec![target("The attached pane. Defaults to the current pane")],
            returns: Returns::Nothing,
            follows: Follows::Nothing,
            errors: vec![
                ErrorCode::BadRequest,
                ErrorCode::NoSuchTarget,
                ErrorCode::AmbiguousTarget,
            ],
            since: VERSION,
        },
    ]
}
