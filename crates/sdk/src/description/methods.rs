//! The one definition of steg's API (decision-1).
//!
//! [`description`] is the single source every consumer reads: the
//! dispatch task's server validates and dispatches from it, a future CLI
//! (TASK-1.49) builds its whole command tree from it, the reference docs
//! are rendered from it, and a Python plugin gets it as JSON. Nothing
//! downstream names a method, a parameter or a flag of its own.
//!
//! Every method doc-2 section 8 lists for the first release is here —
//! `version` and `completion` are not API methods (doc-2 section 2) and are
//! not. Which methods the kernel can serve today and which cannot yet is a
//! dispatch question, carried on TASK-1.43's board task for the task that
//! builds dispatch; this module describes all 42 regardless, each with the
//! shape its `returns` promises, because `fields` alone already answers
//! R37 whether or not a Rust type backs it yet (`SessionRecord` and
//! `SettingRecord` are the ones that do, in `crate::record`, since
//! `session.list`/`session.show` and `setting.list`/`setting.show` are
//! the only methods whose record the kernel can fill today).
//!
//! Grouped by noun, as section 8 is, so a method is easy to find and a
//! review against that document is a direct comparison.

use super::shapes::{
    Description, ErrorCode, ErrorRecord, Event, Field, Method, MethodName, Param, Returns, Role,
    ValueKind, Version,
};

/// The API this build of steg speaks.
pub const VERSION: Version = Version { major: 1, minor: 0 };

/// The whole API, as data.
#[must_use]
pub fn description() -> Description {
    let events = events();
    let event_names: Vec<String> = events.iter().map(|event| event.name.clone()).collect();

    let mut methods = Vec::new();
    methods.extend(session_methods());
    methods.extend(tab_methods());
    methods.extend(pane_methods());
    methods.extend(server_methods());
    methods.extend(event_methods(&event_names));
    methods.extend(client_methods());
    methods.extend(setting_methods());
    methods.extend(api_methods());

    Description {
        api_version: VERSION,
        methods,
        events,
        errors: errors(),
    }
}

// --- shared parameter builders -------------------------------------------
//
// One function per shape a parameter commonly takes, so a method's own
// list below reads as which parameters it has, not how a `Param` literal
// is built.

fn method(noun: &str, verb: &str) -> MethodName {
    MethodName {
        noun: noun.into(),
        verb: verb.into(),
    }
}

/// A role: Target parameter named `target` (doc-2 section 2: "its target
/// parameter is the positional"), whatever kind of value it takes — a
/// session/tab/pane path (`ValueKind::Target`, resolved by R10's rules) for
/// most methods, or plain text for the two whose target is a method name,
/// not a domain path (`api.show`/`api.call`; a method name is never
/// resolved by R10, so `ValueKind::Target` would be the wrong kind for it
/// even though the role is right).
fn target_of_kind(value: ValueKind, required: bool, summary: &str) -> Param {
    Param {
        name: "target".into(),
        role: Role::Target,
        value,
        required,
        summary: summary.into(),
    }
}

fn target(summary: &str) -> Param {
    target_of_kind(ValueKind::Target, false, summary)
}

fn required_target(summary: &str) -> Param {
    target_of_kind(ValueKind::Target, true, summary)
}

fn flag(name: &str, value: ValueKind, required: bool, summary: &str) -> Param {
    Param {
        name: name.into(),
        role: Role::Flag,
        value,
        required,
        summary: summary.into(),
    }
}

fn switch(name: &str, summary: &str) -> Param {
    Param {
        name: name.into(),
        role: Role::Switch,
        value: ValueKind::Boolean,
        required: false,
        summary: summary.into(),
    }
}

fn program() -> Param {
    Param {
        name: "program".into(),
        role: Role::Program,
        value: ValueKind::Argv,
        required: false,
        summary: "The program to run, and its arguments. Defaults to the login shell".into(),
    }
}

fn direction() -> ValueKind {
    ValueKind::Choice(vec![
        "left".into(),
        "right".into(),
        "up".into(),
        "down".into(),
    ])
}

fn field(name: &str, value: ValueKind, summary: &str) -> Field {
    Field {
        name: name.into(),
        value,
        summary: summary.into(),
    }
}

// --- session ---------------------------------------------------------------

/// doc-14's session row: `id`, `label`, `position`, `colour`,
/// `current_tab`. Exactly `SessionRecord`'s fields (`crate::record`) — the
/// one place this module is checked against a real type, by
/// `crate::record::tests`.
fn session_fields() -> Vec<Field> {
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

fn session_methods() -> Vec<Method> {
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
            errors: vec![ErrorCode::BadRequest],
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
            since: VERSION,
        },
        Method {
            name: method("session", "unset"),
            summary: "Clear a session's properties".into(),
            details: String::new(),
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
            since: VERSION,
        },
    ]
}

// --- tab --------------------------------------------------------------

/// doc-14's tab row. `layout` and `shown_status` are already structured
/// values with real Rust types ([`crate::Layout`]; `shown_status` is a
/// pane id or none, computed by `steg_kernel::Tree::tab_shown_status`,
/// TASK-1.41), but `ValueKind` (this module's own vocabulary for a
/// field's kind) has no variant for a nested or structured value at all —
/// so both stay `ValueKind::Text` placeholders until a `TabRecord` gives
/// this description module itself a way to say so; not shown in a flat
/// text table either way.
fn tab_fields() -> Vec<Field> {
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

fn tab_methods() -> Vec<Method> {
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
            ],
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
            since: VERSION,
        },
        Method {
            name: method("tab", "unset"),
            summary: "Clear a tab's properties".into(),
            details: String::new(),
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
            since: VERSION,
        },
    ]
}

// --- pane ---------------------------------------------------------------

/// doc-14's pane row. `status` already has a real Rust type,
/// [`crate::Status`] (`Running`, `Finished` or `Failed{exit_code}`,
/// doc-11; TASK-1.41) or none - but `ValueKind` (this module's own
/// vocabulary for a field's kind) has no variant for a nested or
/// structured value at all, the same reason `exit_code`'s sibling field
/// `program` ([`crate::Program`]) is `ValueKind::Text` too, so `status`
/// stays one until a `PaneRecord` gives this description module itself a
/// way to say so.
fn pane_fields() -> Vec<Field> {
    vec![
        field("id", ValueKind::Text, "The pane's full identifier"),
        field("label", ValueKind::Text, "Its label, if it has one"),
        field("title", ValueKind::Text, "Its title, if it has one"),
        field("cwd", ValueKind::Path, "Its program's working directory"),
        field("program", ValueKind::Text, "What runs in it"),
        field(
            "exit_code",
            ValueKind::Integer,
            "Its program's exit code, once it has ended",
        ),
        field(
            "status",
            ValueKind::Text,
            "What steg knows about its program right now (doc-11), if anything",
        ),
    ]
}

fn pane_methods() -> Vec<Method> {
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
            ],
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
            since: VERSION,
        },
        Method {
            name: method("pane", "unset"),
            summary: "Clear a pane's properties".into(),
            details: String::new(),
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
                    ValueKind::Text,
                    false,
                    "A key to send, by name. May be given more than once",
                ),
            ],
            returns: Returns::Nothing,
            errors: vec![
                ErrorCode::BadRequest,
                ErrorCode::NoSuchTarget,
                ErrorCode::AmbiguousTarget,
            ],
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
            since: VERSION,
        },
    ]
}

// --- server --------------------------------------------------------------

fn server_fields() -> Vec<Field> {
    vec![
        field("version", ValueKind::Text, "The server's own version"),
        field(
            "uptime",
            ValueKind::Integer,
            "How long it has been running, in seconds",
        ),
        field("sessions", ValueKind::Integer, "How many sessions it holds"),
        field(
            "clients",
            ValueKind::Integer,
            "How many clients are connected",
        ),
    ]
}

fn server_methods() -> Vec<Method> {
    vec![
        Method {
            name: method("server", "show"),
            summary: "The server's own state".into(),
            details: String::new(),
            params: Vec::new(),
            returns: Returns::Record {
                fields: server_fields(),
            },
            errors: vec![ErrorCode::BadRequest],
            since: VERSION,
        },
        Method {
            name: method("server", "start"),
            summary: "Start the server, with no clients attached".into(),
            details: "Stays up with no clients. With on_demand, it is the kind a client \
                      starts when it finds no server: it stops itself after 30 seconds with \
                      no sessions and no clients (doc-8 R99). `steg server start \
                      --on-demand` is what the sdk's local-socket provider runs (TASK-1.70)."
                .into(),
            params: vec![switch(
                "on_demand",
                "Start as an on-demand server, which stops itself after 30 seconds with no \
                 sessions and no clients (doc-8 R99), rather than staying up until told to \
                 stop",
            )],
            returns: Returns::Nothing,
            errors: vec![ErrorCode::BadRequest],
            since: VERSION,
        },
        Method {
            name: method("server", "stop"),
            summary: "Stop the server".into(),
            details: String::new(),
            params: vec![switch("force", "Stop it even though sessions exist")],
            returns: Returns::Nothing,
            errors: vec![ErrorCode::BadRequest, ErrorCode::ProgramRunning],
            since: VERSION,
        },
    ]
}

// --- event -----------------------------------------------------------------

fn event_methods(event_names: &[String]) -> Vec<Method> {
    vec![Method {
        name: method("event", "watch"),
        summary: "Stream events until interrupted".into(),
        details: "One record per line: which event fired, and its own fields. The only \
                  stream in the first release."
            .into(),
        params: vec![
            target("The session to watch. Defaults to the current session"),
            flag(
                "for",
                ValueKind::Choice(event_names.to_vec()),
                false,
                "Which events to keep, filtered on the server. Defaults to every event",
            ),
            switch("all", "Every session on the host, not just the target's"),
        ],
        returns: Returns::Records {
            fields: vec![
                field("event", ValueKind::Text, "The event's name"),
                field(
                    "data",
                    ValueKind::Text,
                    "The event's own fields, as a JSON object",
                ),
            ],
        },
        errors: vec![
            ErrorCode::BadRequest,
            ErrorCode::NoSuchTarget,
            ErrorCode::AmbiguousTarget,
        ],
        since: VERSION,
    }]
}

/// doc-8 R38's M1 events: session, tab and pane created, closed and
/// focused; terminal working directory, title, bell, exit and program
/// status changed. Fourteen records; TASK-1.46 fills in behaviour, not
/// this list.
fn events() -> Vec<Event> {
    let path_fields = || {
        vec![
            field("session", ValueKind::Text, "The session on this path"),
            field("tab", ValueKind::Text, "The tab on this path"),
            field("pane", ValueKind::Text, "The pane on this path"),
        ]
    };

    vec![
        Event {
            name: "session.created".into(),
            summary: "A session was created".into(),
            fields: vec![
                field("id", ValueKind::Text, "The new session's identifier"),
                field("label", ValueKind::Text, "Its label, if it has one"),
            ],
            since: VERSION,
        },
        Event {
            name: "session.closed".into(),
            summary: "A session ended, and with it every tab and pane in it".into(),
            fields: vec![field("id", ValueKind::Text, "The session's identifier")],
            since: VERSION,
        },
        Event {
            name: "session.focused".into(),
            summary: "A session became the current session of its path".into(),
            fields: path_fields(),
            since: VERSION,
        },
        Event {
            name: "tab.created".into(),
            summary: "A tab was created".into(),
            fields: vec![
                field("id", ValueKind::Text, "The new tab's identifier"),
                field("session", ValueKind::Text, "The session it was created in"),
                field("label", ValueKind::Text, "Its label, if it has one"),
            ],
            since: VERSION,
        },
        Event {
            name: "tab.closed".into(),
            summary: "A tab ended, and with it every pane in it".into(),
            fields: vec![field("id", ValueKind::Text, "The tab's identifier")],
            since: VERSION,
        },
        Event {
            name: "tab.focused".into(),
            summary: "A tab became the current tab of its session".into(),
            fields: path_fields(),
            since: VERSION,
        },
        Event {
            name: "pane.created".into(),
            summary: "A pane was created".into(),
            fields: vec![
                field("id", ValueKind::Text, "The new pane's identifier"),
                field("tab", ValueKind::Text, "The tab it was created in"),
                field("label", ValueKind::Text, "Its label, if it has one"),
            ],
            since: VERSION,
        },
        Event {
            name: "pane.closed".into(),
            summary: "A pane ended, and with it whatever ran in it".into(),
            fields: vec![field("id", ValueKind::Text, "The pane's identifier")],
            since: VERSION,
        },
        Event {
            name: "pane.focused".into(),
            summary: "A pane became the current pane of its tab".into(),
            fields: path_fields(),
            since: VERSION,
        },
        Event {
            name: "pane.cwd_changed".into(),
            summary: "A pane's program changed its working directory".into(),
            fields: vec![
                field("pane", ValueKind::Text, "The pane"),
                field("cwd", ValueKind::Path, "Its new working directory"),
            ],
            since: VERSION,
        },
        Event {
            name: "pane.title_changed".into(),
            summary: "A pane's program changed its title".into(),
            fields: vec![
                field("pane", ValueKind::Text, "The pane"),
                field("title", ValueKind::Text, "Its new title"),
            ],
            since: VERSION,
        },
        Event {
            name: "pane.bell".into(),
            summary: "A pane's program rang the bell".into(),
            fields: vec![field("pane", ValueKind::Text, "The pane")],
            since: VERSION,
        },
        Event {
            name: "pane.exited".into(),
            summary: "A pane's program ended".into(),
            fields: vec![
                field("pane", ValueKind::Text, "The pane"),
                field("exit_code", ValueKind::Integer, "The program's exit code"),
            ],
            since: VERSION,
        },
        Event {
            name: "pane.status_changed".into(),
            summary: "A pane's status changed (doc-11)".into(),
            fields: vec![
                field("pane", ValueKind::Text, "The pane"),
                field(
                    "status",
                    ValueKind::Text,
                    "The new status, as its own JSON shape, or none",
                ),
            ],
            since: VERSION,
        },
    ]
}

// --- client -----------------------------------------------------------

fn client_fields() -> Vec<Field> {
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

fn client_methods() -> Vec<Method> {
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
            since: VERSION,
        },
    ]
}

// --- setting -----------------------------------------------------------

/// `setting.list`/`setting.show`'s own row: a name, its current value
/// (the override if one is set, else the default) and its default. Every
/// field is `Text`: a setting's value is never typed per entry on the
/// wire (`crate::setting`'s module doc; TASK-1.48's plan).
fn setting_fields() -> Vec<Field> {
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

fn setting_methods() -> Vec<Method> {
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
            since: VERSION,
        },
    ]
}

// --- api -------------------------------------------------------------------

/// The projection `api.list`/`api.show` print in their flat text form: a
/// method's dotted name and its summary. `--json` carries the whole
/// `Method` (params, errors, `since`...), which has more fields than this
/// list — the one place in this description where a served method's
/// actual JSON has more keys than its declared `fields`, because the
/// value really is `Method`, not a record invented to match a projection.
/// Exempted from the `fields`-versus-type cross-check for exactly that
/// reason (see `crate::record`'s test module, and the dispatch task's
/// notes).
fn api_method_fields() -> Vec<Field> {
    vec![
        field("method", ValueKind::Text, "The method's dotted name"),
        field("summary", ValueKind::Text, "One line describing it"),
    ]
}

fn api_methods() -> Vec<Method> {
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
            since: VERSION,
        },
        Method {
            name: method("api", "call"),
            summary: "Call any method by name".into(),
            details: "For an agent, and for a method a plugin adds before the CLI knows it. \
                      Never the documented way to do something that has a command of its own. \
                      `target` names which method to call; every other parameter given is that \
                      method's own, by its own declared names — nothing is nested under a second \
                      key, so a method's parameters are never named twice."
                .into(),
            params: vec![target_of_kind(
                ValueKind::Text,
                true,
                "The method to call, by its dotted name",
            )],
            returns: Returns::Text,
            errors: vec![ErrorCode::BadRequest],
            since: VERSION,
        },
    ]
}

// --- errors --------------------------------------------------------------

/// Every error the API can answer with, and the CLI exit status it becomes
/// (doc-2 section 6).
fn errors() -> Vec<ErrorRecord> {
    vec![
        ErrorRecord {
            code: ErrorCode::BadRequest,
            summary: "The request did not match the API: an unknown method, an unknown or \
                      missing parameter, or a value outside a closed set"
                .into(),
            exit_status: 2,
        },
        ErrorRecord {
            code: ErrorCode::NoSuchTarget,
            summary: "The target matched nothing".into(),
            exit_status: 3,
        },
        ErrorRecord {
            code: ErrorCode::AmbiguousTarget,
            summary: "The target matched more than one thing, so nothing was done".into(),
            exit_status: 3,
        },
        ErrorRecord {
            code: ErrorCode::ProgramRunning,
            summary: "A program is still running and --force was not given".into(),
            exit_status: 1,
        },
        ErrorRecord {
            code: ErrorCode::PositionOutOfRange,
            summary: "The position given is beyond the end of its siblings".into(),
            exit_status: 2,
        },
        ErrorRecord {
            code: ErrorCode::SplitTooSmall,
            summary: "The split would leave a pane under the smallest size allowed".into(),
            exit_status: 1,
        },
        ErrorRecord {
            code: ErrorCode::NoSplitInDirection,
            summary: "There is no split bordering the pane on that side".into(),
            exit_status: 1,
        },
        ErrorRecord {
            code: ErrorCode::PanesNotInSameTab,
            summary: "The two panes named are not in the same tab".into(),
            exit_status: 1,
        },
        ErrorRecord {
            code: ErrorCode::AlreadyInTab,
            summary: "The pane is already in the tab it was asked to move to".into(),
            exit_status: 1,
        },
        ErrorRecord {
            code: ErrorCode::NotImplementedYet,
            summary: "The method is described, but nothing yet implements it".into(),
            exit_status: 1,
        },
        ErrorRecord {
            code: ErrorCode::EngineFailed,
            summary: "The terminal engine failed".into(),
            exit_status: 1,
        },
    ]
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::description;
    use crate::description::shapes::Role;

    /// doc-2 section 8's table, as `(noun, verb)` pairs — the list this
    /// module is checked against. `version` and `completion` are not API
    /// methods (doc-2 section 2) and are not here.
    fn doc_2_section_8() -> Vec<(&'static str, &'static str)> {
        vec![
            ("session", "list"),
            ("session", "show"),
            ("session", "new"),
            ("session", "close"),
            ("session", "move"),
            ("session", "set"),
            ("session", "unset"),
            ("session", "focus"),
            ("tab", "list"),
            ("tab", "show"),
            ("tab", "new"),
            ("tab", "close"),
            ("tab", "move"),
            ("tab", "set"),
            ("tab", "unset"),
            ("tab", "focus"),
            ("pane", "list"),
            ("pane", "show"),
            ("pane", "split"),
            ("pane", "close"),
            ("pane", "move"),
            ("pane", "set"),
            ("pane", "unset"),
            ("pane", "focus"),
            ("pane", "resize"),
            ("pane", "swap"),
            ("pane", "send"),
            ("pane", "capture"),
            ("pane", "wait"),
            ("server", "show"),
            ("server", "start"),
            ("server", "stop"),
            ("event", "watch"),
            ("client", "list"),
            ("client", "show"),
            ("api", "list"),
            ("api", "show"),
            ("api", "call"),
            ("setting", "list"),
            ("setting", "show"),
            ("setting", "set"),
            ("setting", "unset"),
        ]
    }

    #[test]
    fn the_method_set_is_exactly_doc_2_section_8s_42_pairs() {
        let api = description();
        assert_eq!(api.methods.len(), 42, "doc-2 section 8 lists 42 methods");

        let described: HashSet<(String, String)> = api
            .methods
            .iter()
            .map(|m| (m.name.noun.clone(), m.name.verb.clone()))
            .collect();
        assert_eq!(described.len(), api.methods.len(), "no duplicate method");

        let expected: HashSet<(String, String)> = doc_2_section_8()
            .into_iter()
            .map(|(noun, verb)| (noun.to_string(), verb.to_string()))
            .collect();
        assert_eq!(described, expected);
    }

    #[test]
    fn no_method_or_param_summary_or_param_summary_is_empty() {
        let api = description();
        for method in &api.methods {
            assert!(
                !method.summary.is_empty(),
                "{} has no summary",
                method.name.dotted()
            );
            for param in &method.params {
                assert!(
                    !param.summary.is_empty(),
                    "{}'s {} has no summary",
                    method.name.dotted(),
                    param.name
                );
            }
        }
    }

    #[test]
    fn no_method_declares_a_cli_only_flag_as_a_parameter() {
        // decision-1 section 2: these belong to the CLI, not the API.
        const CLI_ONLY: &[&str] = &[
            "json",
            "quiet",
            "verbose",
            "no_color",
            "no_input",
            "help",
            "host",
            "autostart",
            "no_autostart",
        ];
        let api = description();
        for method in &api.methods {
            for param in &method.params {
                assert!(
                    !CLI_ONLY.contains(&param.name.as_str()),
                    "{} declares {}, which is a CLI-only flag",
                    method.name.dotted(),
                    param.name
                );
            }
        }
    }

    #[test]
    fn a_method_has_at_most_one_target_and_at_most_one_program() {
        let api = description();
        for method in &api.methods {
            assert!(
                method
                    .params
                    .iter()
                    .filter(|p| p.role == Role::Target)
                    .count()
                    <= 1,
                "{} has more than one positional",
                method.name.dotted()
            );
            assert!(
                method
                    .params
                    .iter()
                    .filter(|p| p.role == Role::Program)
                    .count()
                    <= 1,
                "{} has more than one program",
                method.name.dotted()
            );
        }
    }

    #[test]
    fn every_error_a_method_names_has_a_record() {
        let api = description();
        for method in &api.methods {
            for code in &method.errors {
                assert!(
                    api.errors.iter().any(|record| record.code == *code),
                    "{} names {code:?}, which has no record",
                    method.name.dotted()
                );
            }
        }
    }

    #[test]
    fn events_has_r38s_fourteen_m1_names_each_once() {
        let api = description();
        assert_eq!(api.events.len(), 14);
        let names: HashSet<&str> = api.events.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names.len(), api.events.len(), "no duplicate event name");
        for expected in [
            "session.created",
            "session.closed",
            "session.focused",
            "tab.created",
            "tab.closed",
            "tab.focused",
            "pane.created",
            "pane.closed",
            "pane.focused",
            "pane.cwd_changed",
            "pane.title_changed",
            "pane.bell",
            "pane.exited",
            "pane.status_changed",
        ] {
            assert!(names.contains(expected), "missing event {expected}");
        }
    }

    #[test]
    fn errors_has_eleven_codes_with_their_exit_statuses() {
        use crate::description::shapes::ErrorCode;

        let api = description();
        assert_eq!(api.errors.len(), 11);

        let status_of = |code: ErrorCode| {
            api.errors
                .iter()
                .find(|record| record.code == code)
                .unwrap_or_else(|| panic!("{code:?} has no record"))
                .exit_status
        };
        // doc-2 section 6's own table, for the ten errors it names.
        assert_eq!(status_of(ErrorCode::BadRequest), 2);
        assert_eq!(status_of(ErrorCode::NoSuchTarget), 3);
        assert_eq!(status_of(ErrorCode::AmbiguousTarget), 3);
        assert_eq!(status_of(ErrorCode::PositionOutOfRange), 2);
        assert_eq!(status_of(ErrorCode::ProgramRunning), 1);
        assert_eq!(status_of(ErrorCode::SplitTooSmall), 1);
        assert_eq!(status_of(ErrorCode::NoSplitInDirection), 1);
        assert_eq!(status_of(ErrorCode::PanesNotInSameTab), 1);
        assert_eq!(status_of(ErrorCode::AlreadyInTab), 1);
        assert_eq!(status_of(ErrorCode::NotImplementedYet), 1);
        // `EngineFailed` (TASK-1.75): not in doc-2 section 6's table at all
        // (it predates the terminal engine) - 1, "the command ran and
        // failed", matches a genuine runtime failure better than status 2
        // (bad usage) or 3 (a target problem). Flagged on the task for the
        // engineer to confirm or overrule.
        assert_eq!(status_of(ErrorCode::EngineFailed), 1);
    }

    #[test]
    fn event_watch_for_is_drawn_from_the_event_names() {
        let api = description();
        let event_names: HashSet<&str> = api.events.iter().map(|e| e.name.as_str()).collect();

        let event_watch = api.method("event.watch").unwrap();
        let for_param = event_watch.params.iter().find(|p| p.name == "for").unwrap();
        let crate::description::shapes::ValueKind::Choice(words) = &for_param.value else {
            panic!("event.watch's for is a closed set");
        };
        assert_eq!(
            words.iter().map(String::as_str).collect::<HashSet<_>>(),
            event_names
        );
    }

    /// doc-2 section 8 gives `pane wait`'s `--for` exactly one value in
    /// the first release, `exit` - not the dotted event names
    /// `event.watch`'s `--for` draws from. See the method's own `details`
    /// for why the two are not unified here.
    #[test]
    fn pane_wait_for_is_exactly_the_one_value_doc_2_section_8_gives_it() {
        let api = description();
        let pane_wait = api.method("pane.wait").unwrap();
        let for_param = pane_wait.params.iter().find(|p| p.name == "for").unwrap();
        assert_eq!(
            for_param.value,
            crate::description::shapes::ValueKind::Choice(vec!["exit".into()])
        );
    }

    /// doc-2 section 8's Target column, written out independently of the
    /// description: `None` for "none"; `Some((required, value))` for every
    /// other row, where `required` is whether doc-2's own text (section
    /// 4's context fallback, or `api show`'s stated "with none, the whole
    /// description") gives it a default, and `value` is `ValueKind::Target`
    /// for a session/tab/pane/client path and `ValueKind::Text` for the
    /// two whose target is a bare method name (`api show`/`api call`).
    fn doc_2_section_8_targets() -> Vec<(
        &'static str,
        Option<(bool, crate::description::shapes::ValueKind)>,
    )> {
        use crate::description::shapes::ValueKind;

        vec![
            ("session.list", None),
            ("session.show", Some((false, ValueKind::Target))),
            ("session.new", None),
            ("session.close", Some((false, ValueKind::Target))),
            ("session.move", Some((false, ValueKind::Target))),
            ("session.set", Some((false, ValueKind::Target))),
            ("session.unset", Some((false, ValueKind::Target))),
            ("session.focus", Some((false, ValueKind::Target))),
            ("tab.list", Some((false, ValueKind::Target))),
            ("tab.show", Some((false, ValueKind::Target))),
            ("tab.new", Some((false, ValueKind::Target))),
            ("tab.close", Some((false, ValueKind::Target))),
            ("tab.move", Some((false, ValueKind::Target))),
            ("tab.set", Some((false, ValueKind::Target))),
            ("tab.unset", Some((false, ValueKind::Target))),
            ("tab.focus", Some((false, ValueKind::Target))),
            ("pane.list", Some((false, ValueKind::Target))),
            ("pane.show", Some((false, ValueKind::Target))),
            ("pane.split", Some((false, ValueKind::Target))),
            ("pane.close", Some((false, ValueKind::Target))),
            ("pane.move", Some((false, ValueKind::Target))),
            ("pane.set", Some((false, ValueKind::Target))),
            ("pane.unset", Some((false, ValueKind::Target))),
            ("pane.focus", Some((false, ValueKind::Target))),
            ("pane.resize", Some((false, ValueKind::Target))),
            ("pane.swap", Some((false, ValueKind::Target))),
            ("pane.send", Some((false, ValueKind::Target))),
            ("pane.capture", Some((false, ValueKind::Target))),
            ("pane.wait", Some((false, ValueKind::Target))),
            ("server.show", None),
            ("server.start", None),
            ("server.stop", None),
            ("event.watch", Some((false, ValueKind::Target))),
            ("client.list", None),
            ("client.show", Some((true, ValueKind::Target))),
            ("setting.list", None),
            ("setting.show", Some((true, ValueKind::Text))),
            ("setting.set", Some((true, ValueKind::Text))),
            ("setting.unset", Some((true, ValueKind::Text))),
            ("api.list", None),
            ("api.show", Some((false, ValueKind::Text))),
            ("api.call", Some((true, ValueKind::Text))),
        ]
    }

    /// The hole the review found: the completeness test above only checks
    /// the set of `(noun, verb)` pairs, which says nothing about whether a
    /// method's positional is actually `role: Target` (`api.show`/
    /// `api.call` had `role: Flag` instead, which a CLI generated from this
    /// description would turn into `--target <method>` rather than doc-2
    /// section 2's positional `steg api show <method>`). This test checks
    /// every method against doc-2 section 8's Target column directly,
    /// independently of whatever `target()`/`target_of_kind()` calls the
    /// method literals happen to make.
    #[test]
    fn every_method_has_a_target_exactly_as_doc_2_section_8_gives_it() {
        let api = description();
        for (dotted, expected) in doc_2_section_8_targets() {
            let described = api
                .method(dotted)
                .unwrap_or_else(|| panic!("{dotted} is not described"));
            match expected {
                None => assert!(
                    described.target().is_none(),
                    "{dotted}'s Target column is \"none\", so it should have no Target-role param"
                ),
                Some((required, value)) => {
                    let target = described
                        .target()
                        .unwrap_or_else(|| panic!("{dotted} should have a Target-role param"));
                    assert_eq!(target.required, required, "{dotted}'s target required-ness");
                    assert_eq!(target.value, value, "{dotted}'s target value kind");
                }
            }
        }
    }
}
