//! Typed events a subscribed client reads after `event.watch`'s own reply
//! (TASK-1.46; decision-2 section 3: an event carries no id). One variant
//! per event the description declares (`description::methods::events`) —
//! TASK-1.109 adds its own variants to this same enum, not a second one.

use serde::{Deserialize, Serialize};

use crate::ids::{PaneId, SessionId, TabId};
use crate::status::Status;

/// One of the API's own events, already matched by name against the
/// `Description` (`wire::connection::Incoming::Event`) and now parsed into
/// its own typed fields — mechanical, not new design: every field here is
/// exactly what `description::methods::events()` already declares for that
/// event's name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "event", content = "data")]
pub enum TreeEvent {
    /// `session.created`.
    #[serde(rename = "session.created")]
    SessionCreated {
        /// The new session's identifier.
        id: SessionId,
        /// Its label, if it has one.
        label: Option<String>,
    },
    /// `session.closed`.
    #[serde(rename = "session.closed")]
    SessionClosed {
        /// The session's identifier.
        id: SessionId,
    },
    /// `session.focused`.
    #[serde(rename = "session.focused")]
    SessionFocused {
        /// The session on this path.
        session: SessionId,
        /// The tab on this path.
        tab: TabId,
        /// The pane on this path.
        pane: PaneId,
    },
    /// `tab.created`.
    #[serde(rename = "tab.created")]
    TabCreated {
        /// The new tab's identifier.
        id: TabId,
        /// The session it was created in.
        session: SessionId,
        /// Its label, if it has one.
        label: Option<String>,
    },
    /// `tab.closed`.
    #[serde(rename = "tab.closed")]
    TabClosed {
        /// The tab's identifier.
        id: TabId,
    },
    /// `tab.focused`.
    #[serde(rename = "tab.focused")]
    TabFocused {
        /// The session on this path.
        session: SessionId,
        /// The tab on this path.
        tab: TabId,
        /// The pane on this path.
        pane: PaneId,
    },
    /// `pane.created`.
    #[serde(rename = "pane.created")]
    PaneCreated {
        /// The new pane's identifier.
        id: PaneId,
        /// The tab it was created in.
        tab: TabId,
        /// Its label, if it has one.
        label: Option<String>,
    },
    /// `pane.closed`.
    #[serde(rename = "pane.closed")]
    PaneClosed {
        /// The pane's identifier.
        id: PaneId,
    },
    /// `pane.focused`.
    #[serde(rename = "pane.focused")]
    PaneFocused {
        /// The session on this path.
        session: SessionId,
        /// The tab on this path.
        tab: TabId,
        /// The pane on this path.
        pane: PaneId,
    },
    /// `pane.cwd_changed`.
    #[serde(rename = "pane.cwd_changed")]
    PaneCwdChanged {
        /// The pane.
        pane: PaneId,
        /// Its new working directory.
        cwd: String,
    },
    /// `pane.title_changed`.
    #[serde(rename = "pane.title_changed")]
    PaneTitleChanged {
        /// The pane.
        pane: PaneId,
        /// Its new title.
        title: String,
    },
    /// `pane.bell`.
    #[serde(rename = "pane.bell")]
    PaneBell {
        /// The pane.
        pane: PaneId,
    },
    /// `pane.exited`.
    #[serde(rename = "pane.exited")]
    PaneExited {
        /// The pane.
        pane: PaneId,
        /// The program's exit code.
        exit_code: i32,
    },
    /// `pane.status_changed`.
    #[serde(rename = "pane.status_changed")]
    PaneStatusChanged {
        /// The pane.
        pane: PaneId,
        /// The new status, or none.
        status: Option<Status>,
    },
}

impl TreeEvent {
    /// Parses one `Incoming::Event`'s own `name`/`body` into its typed
    /// shape — the name is already known to be one of the description's
    /// own events by the time this is called (`Incoming::Event`, not
    /// `UnknownEvent`); this only has to match it to the right variant and
    /// deserialise its body.
    ///
    /// # Errors
    /// Whatever `serde_json` gives for a body that does not match the
    /// shape this event's own name declares.
    pub fn parse(name: &str, body: serde_json::Value) -> Result<Self, serde_json::Error> {
        // `TreeEvent`'s own externally-tagged representation already
        // expects exactly `{"event": "<name>", "data": <body>}` - building
        // that envelope here, rather than giving this enum a second,
        // hand-written match on `name`, is what keeps the two in lockstep:
        // adding a variant above is the only thing a later task has to do.
        let envelope = serde_json::json!({ "event": name, "data": body });
        serde_json::from_value(envelope)
    }
}

#[cfg(test)]
mod tests {
    use super::TreeEvent;
    use crate::ids::{PaneId, SessionId, TabId};

    #[test]
    fn parses_a_pane_closed_event_by_name() {
        let event = TreeEvent::parse(
            "pane.closed",
            serde_json::json!({ "id": PaneId::from_raw(1).to_string() }),
        )
        .unwrap();
        assert_eq!(
            event,
            TreeEvent::PaneClosed {
                id: PaneId::from_raw(1)
            }
        );
    }

    #[test]
    fn parses_a_whole_path_event() {
        let event = TreeEvent::parse(
            "pane.focused",
            serde_json::json!({
                "session": SessionId::from_raw(1).to_string(),
                "tab": TabId::from_raw(2).to_string(),
                "pane": PaneId::from_raw(3).to_string(),
            }),
        )
        .unwrap();
        assert_eq!(
            event,
            TreeEvent::PaneFocused {
                session: SessionId::from_raw(1),
                tab: TabId::from_raw(2),
                pane: PaneId::from_raw(3),
            }
        );
    }

    #[test]
    fn parsing_an_unknown_name_fails_rather_than_guessing() {
        assert!(TreeEvent::parse("made.up", serde_json::json!({})).is_err());
    }

    #[test]
    fn a_pane_status_changed_event_round_trips_a_none_status() {
        let event = TreeEvent::PaneStatusChanged {
            pane: PaneId::from_raw(1),
            status: None,
        };
        let value = serde_json::to_value(&event).unwrap();
        let parsed: TreeEvent = serde_json::from_value(value).unwrap();
        assert_eq!(parsed, event);
    }

    /// Every variant's own fields match the description's own declared
    /// fields for that event name exactly - the completeness check this
    /// enum gets that `TreeSnapshot` cannot (its own reply is nested, with
    /// no flat fields list to check against; event bodies stay flat).
    #[test]
    fn every_variant_matches_the_fields_its_own_event_declares() {
        use std::collections::HashSet;

        let api = crate::description::description();
        let examples: Vec<(&str, TreeEvent)> = vec![
            (
                "session.created",
                TreeEvent::SessionCreated {
                    id: SessionId::from_raw(1),
                    label: None,
                },
            ),
            (
                "session.closed",
                TreeEvent::SessionClosed {
                    id: SessionId::from_raw(1),
                },
            ),
            (
                "session.focused",
                TreeEvent::SessionFocused {
                    session: SessionId::from_raw(1),
                    tab: TabId::from_raw(1),
                    pane: PaneId::from_raw(1),
                },
            ),
            (
                "tab.created",
                TreeEvent::TabCreated {
                    id: TabId::from_raw(1),
                    session: SessionId::from_raw(1),
                    label: None,
                },
            ),
            (
                "tab.closed",
                TreeEvent::TabClosed {
                    id: TabId::from_raw(1),
                },
            ),
            (
                "tab.focused",
                TreeEvent::TabFocused {
                    session: SessionId::from_raw(1),
                    tab: TabId::from_raw(1),
                    pane: PaneId::from_raw(1),
                },
            ),
            (
                "pane.created",
                TreeEvent::PaneCreated {
                    id: PaneId::from_raw(1),
                    tab: TabId::from_raw(1),
                    label: None,
                },
            ),
            (
                "pane.closed",
                TreeEvent::PaneClosed {
                    id: PaneId::from_raw(1),
                },
            ),
            (
                "pane.focused",
                TreeEvent::PaneFocused {
                    session: SessionId::from_raw(1),
                    tab: TabId::from_raw(1),
                    pane: PaneId::from_raw(1),
                },
            ),
            (
                "pane.cwd_changed",
                TreeEvent::PaneCwdChanged {
                    pane: PaneId::from_raw(1),
                    cwd: "/tmp".to_string(),
                },
            ),
            (
                "pane.title_changed",
                TreeEvent::PaneTitleChanged {
                    pane: PaneId::from_raw(1),
                    title: "x".to_string(),
                },
            ),
            (
                "pane.bell",
                TreeEvent::PaneBell {
                    pane: PaneId::from_raw(1),
                },
            ),
            (
                "pane.exited",
                TreeEvent::PaneExited {
                    pane: PaneId::from_raw(1),
                    exit_code: 0,
                },
            ),
            (
                "pane.status_changed",
                TreeEvent::PaneStatusChanged {
                    pane: PaneId::from_raw(1),
                    status: None,
                },
            ),
        ];
        assert_eq!(
            examples.len(),
            14,
            "every one of the description's own events"
        );

        for (name, example) in examples {
            let event = api
                .event(name)
                .unwrap_or_else(|| panic!("{name} is not described"));
            let declared: HashSet<&str> = event.fields.iter().map(|f| f.name.as_str()).collect();

            let value = serde_json::to_value(&example).unwrap();
            let body = value["data"].as_object().unwrap();
            let actual: HashSet<&str> = body.keys().map(String::as_str).collect();

            assert_eq!(
                declared, actual,
                "{name}'s declared fields and TreeEvent's own variant must match"
            );
        }
    }
}
