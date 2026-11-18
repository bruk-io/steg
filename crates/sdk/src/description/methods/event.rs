//! The `event` noun's methods, and the events this build describes.

use crate::description::shapes::{ErrorCode, Event, Follows, Method, Returns, ValueKind};

use super::shared::{VERSION, field, flag, method, switch, target};

pub(super) fn event_methods(event_names: &[String]) -> Vec<Method> {
    vec![Method {
        name: method("event", "watch"),
        summary: "Subscribe to events, after reading the tree once".into(),
        details: "The reply is the tree: every session, with its tabs, each with its panes, \
                  as of the moment this subscription began (TASK-1.46) - a `TreeSnapshot` \
                  (decision-1: the sdk's own type is the single definition of its real \
                  shape). Every event after that reply, with no further request, is one of \
                  this method's own stream items: which event fired, and its own fields, as \
                  one record. The only stream in the first release."
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
        returns: Returns::Nested {
            of: "TreeSnapshot".into(),
            summary: "Every session, with its tabs, each with its panes, as of the moment \
                      this subscription began"
                .into(),
        },
        follows: Follows::Events(Returns::Records {
            fields: vec![
                field("event", ValueKind::Text, "The event's name"),
                field(
                    "data",
                    ValueKind::Text,
                    "The event's own fields, as a JSON object",
                ),
            ],
        }),
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
pub(super) fn events() -> Vec<Event> {
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
