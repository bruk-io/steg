//! Records a client reads back over the API (doc-16 R105).
//!
//! `SessionRecord` was the first this crate added: `session.list`/
//! `session.show` are the only served methods (TASK-1.43's dispatch task)
//! whose `returns` is a record at all.
//!
//! `SettingRecord` (TASK-1.48) is the second: `setting.list`/`setting.show`
//! read back a registry entry (`crate::setting::settings`) and its current
//! value, every field as its own wire text (R80's settings are never typed
//! per entry on the wire — see `crate::setting`'s module doc).
//!
//! `TabRecord` (TASK-1.89) is the third: `tab.list`/`tab.show`'s own record,
//! exactly `description::methods::tab_fields`'s seven fields - the shape
//! TASK-1.46's own nested-tree snapshot depends on this crate for and
//! embeds as-is, rather than defining one of its own.
//!
//! `ClientRecord` (TASK-1.141) is the fourth: `client.list`/`client.show`'s
//! own record, exactly `description::methods::client_fields`'s five
//! fields. `last_active` is always `None` for M1 - no part of the kernel's
//! `Client` tracks it yet, and this record never polls for one to appear
//! (the same reasoning [`PaneRecord::title`]/[`TabRecord::title`] already
//! give for the same shape).
//!
//! `WaitResult` (TASK-1.140) is the fifth: `pane.wait`'s own reply, exactly
//! its one declared field (`exit_code`) - a typed record rather than an
//! inline JSON object, the same reason every record above is typed.

use serde::{Deserialize, Serialize};

use crate::client_kind::ClientKind;
use crate::colour::Colour;
use crate::ids::{ClientId, PaneId, SessionId, TabId};
use crate::layout::{Layout, Size};
use crate::pane_kind::PaneKind;
use crate::status::Status;

/// One session, as doc-14's session row: `id`, `label`, `position`,
/// `colour`, `current_tab`. Field names are exactly that row's, so
/// `--json`'s promise to use the API's own field names (doc-2 section 6)
/// holds without a second list of them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionRecord {
    /// The session's full identifier.
    pub id: SessionId,
    /// Its label, if it has one.
    pub label: Option<String>,
    /// Its one-based position among the server's sessions (doc-15 R10).
    pub position: usize,
    /// Its colour (R26).
    pub colour: Colour,
    /// The id of its current tab (doc-15 R85).
    pub current_tab: TabId,
}

/// One client, as `client.show`/`client.list` return it (TASK-1.141),
/// with the same fields `description::methods::client_fields` declares.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClientRecord {
    /// The client's full identifier.
    pub id: ClientId,
    /// What kind of connection it is: the app, a CLI invocation, or a
    /// plugin.
    pub kind: ClientKind,
    /// Its label, if it has one.
    pub label: Option<String>,
    /// The sessions it views (decision-5; `Tree::sessions_viewed`).
    pub sessions: Vec<SessionId>,
    /// When it was last active, as ISO 8601 in UTC - always `None` until
    /// a later task wires tracking through (this module's own doc
    /// comment names the cost).
    pub last_active: Option<String>,
}

/// One pane, as `pane.show`/`pane.list` return it (TASK-1.76 AC #2;
/// TASK-1.89), with the same fields `description::methods::pane_fields`
/// declares, so `--json`'s promise of the API's own field names holds
/// without a second list.
///
/// `title` is always `None` here: it needs the terminal's own live OSC
/// title, which lives behind a `PaneHandle` (I/O) this record's own
/// builder (`crates/server/src/dispatch/pane.rs`) does not reach -
/// declared in the description for the task that does, not fabricated
/// here. Read fresh from the tree at the moment of the request, like
/// every other field here: nothing caches it and nothing polls for it to
/// change.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PaneRecord {
    /// The pane's full identifier.
    pub id: PaneId,
    /// Its label, if it has one.
    pub label: Option<String>,
    /// Its title, if it has one. Always `None` until a later task wires
    /// the terminal's own live title through.
    pub title: Option<String>,
    /// What kind of thing it runs. Every pane is a terminal in M1 (R12).
    pub kind: PaneKind,
    /// Its program's working directory, as the program *started* in it
    /// (`crate::Program::cwd`) - not the live directory a real OSC 7
    /// reports, which is a different, I/O-backed value this record never
    /// reaches.
    pub cwd: String,
    /// What runs in it: `"shell"`, or the given command and its
    /// arguments, space-separated.
    pub program: String,
    /// Its program's exit code, once it has ended.
    pub exit_code: Option<i32>,
    /// The one clock-free fact about its program right now (doc-15 R97):
    /// `"running"` or `"ended"`. `None` only in the window between a pane
    /// being minted and its program being started, never observed from
    /// outside that pairing (`crates/kernel/src/pane.rs`'s own doc). Named
    /// to match the kernel's own `Pane::program_status` - see
    /// [`PaneRecord::status`] for the field this one used to be.
    pub program_status: Option<String>,
    /// What steg knows about its program right now (doc-11, decision-7):
    /// running, finished, or failed with its exit code - or `None` for a
    /// shell idling at its own prompt with no typed command running
    /// (TASK-1.83), the same as for a pane with no program at all yet.
    /// Longer-lived than [`PaneRecord::program_status`] and can disagree
    /// with it (a `Finished`/`Failed` status ends on a key press or after
    /// ten minutes, well before the program needed for `program_status`
    /// to change at all) - `crates/kernel/src/pane.rs`'s own doc comment
    /// gives the same two fields, for the same reason.
    pub status: Option<Status>,
    /// Its terminal's size in whole cells, if a client has ever declared
    /// one for its tab (R91; `Tree::tab_layout`/`Tab::size`) - `None`
    /// before any client has.
    pub size: Option<Size>,
}

/// `pane.wait`'s own reply (TASK-1.140), with the same one field
/// `description::methods::pane.rs`'s own `pane.wait` declares -
/// `exit_code`, the program's own exit code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct WaitResult {
    /// The program's own exit code.
    pub exit_code: i32,
}

/// One tab, as `tab.list`/`tab.show` return it (TASK-1.89), with the same
/// fields `description::methods::tab_fields` declares - the shape
/// TASK-1.46's own nested-tree snapshot embeds as-is.
///
/// `title` is always `None`, same reasoning as [`PaneRecord::title`]: no
/// part of the kernel's tree holds one yet, and this record never polls
/// for one to appear.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TabRecord {
    /// The tab's full identifier.
    pub id: TabId,
    /// Its label, if it has one.
    pub label: Option<String>,
    /// Its title, if it has one. Always `None` until a later task wires
    /// one through.
    pub title: Option<String>,
    /// Its one-based position among its session's tabs (doc-15 R10).
    pub position: usize,
    /// The id of its current pane (doc-15 R85).
    pub current_pane: PaneId,
    /// Its layout tree (R95).
    pub layout: Layout,
    /// The pane whose status this tab's ring stands for, if any
    /// (doc-11 R50, decision-7; `Tree::tab_shown_status`).
    pub shown_status: Option<PaneId>,
}

/// One setting, as `setting.list`/`setting.show` return it: its name, its
/// current value (the override if one is set, else the default) and its
/// default, each as its own wire text (`crate::setting::Setting`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SettingRecord {
    /// The setting's name.
    pub name: String,
    /// Its current value: the override if one is set, else the default.
    pub value: String,
    /// Its value with no override.
    pub default: String,
}

/// The server's own state, as `server.show` returns it (TASK-1.88, doc-2
/// section 8): its version, how long it has been up, and how many
/// sessions and clients it holds - the same fields `description::methods::
/// server_fields` declares.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServerRecord {
    /// The server's own version string (`"steg/<cargo version>"`).
    pub version: String,
    /// How long it has been running, in seconds - time is read by the
    /// server and reaches this record as plain data (AC #1); the one
    /// honesty gap is named on `steg_kernel::Moment::since_origin`'s own
    /// doc, which is what this is built from.
    pub uptime: u64,
    /// How many sessions it holds.
    pub sessions: usize,
    /// How many clients are connected.
    pub clients: usize,
}

/// One tab with its panes embedded (TASK-1.46, AC #1) — `TabRecord`'s own
/// fields, flattened, plus the panes `TabRecord.layout`'s own leaves name by
/// id. `layout` says *where* each pane sits (a structural reference, the
/// same role `current_pane`/`current_tab` already play elsewhere: naming a
/// sibling by id, not embedding its data); `panes` is the one place that
/// pane's actual data lives. A client resolves a `layout` leaf's id against
/// this same `panes` array — nothing is stated twice, the two answer
/// different questions ("what shape" and "what is each one").
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TabTree {
    /// The tab's own fields.
    #[serde(flatten)]
    pub tab: TabRecord,
    /// Its panes, in the tab's own order.
    pub panes: Vec<PaneRecord>,
}

/// One session with its tabs embedded (TASK-1.46, AC #1) — `SessionRecord`'s
/// own fields, flattened, plus its tabs, each with its own panes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionTree {
    /// The session's own fields.
    #[serde(flatten)]
    pub session: SessionRecord,
    /// Its tabs, in the session's own order.
    pub tabs: Vec<TabTree>,
}

/// The whole tree: every session, each with its tabs, each with its panes,
/// as of one coherent moment (TASK-1.46, AC #1) — `event.watch`'s own
/// reply (a `Returns::Nested` in the description, since there is no flat
/// fields table for a nested value).
///
/// Named `TreeSnapshot`, not `Tree`: `steg_kernel::Tree` already owns that
/// name for the live, mutable structure this is a wire photograph of — a
/// different thing, in a different crate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TreeSnapshot {
    /// Every session on the host, in the server's own order.
    pub sessions: Vec<SessionTree>,
}

#[cfg(test)]
mod tests {
    use super::SessionRecord;
    use crate::colour::Colour;
    use crate::ids::{SessionId, TabId};

    fn example() -> SessionRecord {
        SessionRecord {
            id: SessionId::from_raw(1),
            label: Some("work".to_string()),
            position: 1,
            colour: Colour::Violet,
            current_tab: TabId::from_raw(2),
        }
    }

    #[test]
    fn serde_round_trips() {
        let record = example();
        let json = serde_json::to_string(&record).unwrap();
        assert_eq!(
            serde_json::from_str::<SessionRecord>(&json).unwrap(),
            record
        );
    }

    #[test]
    fn serialises_with_exactly_doc_14s_session_fields() {
        let value = serde_json::to_value(example()).unwrap();
        let object = value.as_object().expect("a session record is an object");
        let mut keys: Vec<&str> = object.keys().map(String::as_str).collect();
        keys.sort_unstable();
        assert_eq!(
            keys,
            vec!["colour", "current_tab", "id", "label", "position"]
        );
    }

    #[test]
    fn a_label_less_session_serialises_with_a_null_label_not_a_missing_field() {
        let mut record = example();
        record.label = None;
        let value = serde_json::to_value(record).unwrap();
        assert!(
            value.get("label").is_some_and(serde_json::Value::is_null),
            "{value}"
        );
    }

    /// Ties `SessionRecord`'s own shape to the description's: if a field is
    /// added to one and not the other, this fails. `session.show` and
    /// `session.list` declare the same `fields`, so one comparison covers
    /// both.
    #[test]
    fn matches_the_fields_session_show_and_session_list_declare() {
        use std::collections::HashSet;

        let api = crate::description::description();
        let value = serde_json::to_value(example()).unwrap();
        let record_keys: HashSet<&str> = value
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();

        for dotted in ["session.show", "session.list"] {
            let method = api.method(dotted).unwrap();
            let fields = match &method.returns {
                crate::description::Returns::Record { fields }
                | crate::description::Returns::Records { fields } => fields,
                other @ (crate::description::Returns::Nothing
                | crate::description::Returns::Identifier { .. }
                | crate::description::Returns::Text
                | crate::description::Returns::Nested { .. }) => {
                    panic!("{dotted}'s returns is not a record: {other:?}")
                }
            };
            let declared: HashSet<&str> = fields.iter().map(|f| f.name.as_str()).collect();
            assert_eq!(
                declared, record_keys,
                "{dotted}'s declared fields and SessionRecord's own keys must match"
            );
        }
    }

    use super::ClientRecord;
    use crate::client_kind::ClientKind;
    use crate::ids::ClientId;

    fn client_example() -> ClientRecord {
        ClientRecord {
            id: ClientId::from_raw(1),
            kind: ClientKind::App,
            label: Some("work".to_string()),
            sessions: vec![SessionId::from_raw(1)],
            last_active: None,
        }
    }

    #[test]
    fn client_record_serde_round_trips() {
        let record = client_example();
        let json = serde_json::to_string(&record).unwrap();
        assert_eq!(serde_json::from_str::<ClientRecord>(&json).unwrap(), record);
    }

    /// `last_active` is always `None` for M1 (this module's own doc), and
    /// a client with no label has none either - both serialise as `null`,
    /// not a missing field, the same contract `SessionRecord`'s own
    /// label test above proves.
    #[test]
    fn a_client_with_no_label_or_last_active_serialises_them_as_null_not_missing() {
        let mut record = client_example();
        record.label = None;
        let value = serde_json::to_value(record).unwrap();
        assert!(value.get("label").is_some_and(serde_json::Value::is_null));
        assert!(
            value
                .get("last_active")
                .is_some_and(serde_json::Value::is_null)
        );
    }

    /// Ties `ClientRecord`'s own shape to the description's, the same way
    /// `matches_the_fields_session_show_and_session_list_declare` does for
    /// `SessionRecord` above. `client.show` and `client.list` declare the
    /// same `fields`, so one comparison covers both.
    #[test]
    fn matches_the_fields_client_list_and_client_show_declare() {
        use std::collections::HashSet;

        let api = crate::description::description();
        let value = serde_json::to_value(client_example()).unwrap();
        let record_keys: HashSet<&str> = value
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();

        for dotted in ["client.list", "client.show"] {
            let method = api.method(dotted).unwrap();
            let fields = match &method.returns {
                crate::description::Returns::Record { fields }
                | crate::description::Returns::Records { fields } => fields,
                other @ (crate::description::Returns::Nothing
                | crate::description::Returns::Identifier { .. }
                | crate::description::Returns::Text
                | crate::description::Returns::Nested { .. }) => {
                    panic!("{dotted}'s returns is not a record: {other:?}")
                }
            };
            let declared: HashSet<&str> = fields.iter().map(|f| f.name.as_str()).collect();
            assert_eq!(
                declared, record_keys,
                "{dotted}'s declared fields and ClientRecord's own keys must match"
            );
        }
    }

    use super::PaneRecord;
    use crate::ids::PaneId;
    use crate::pane_kind::PaneKind;
    use crate::status::Status;

    fn pane_example() -> PaneRecord {
        PaneRecord {
            id: PaneId::from_raw(1),
            label: Some("logs".to_string()),
            title: None,
            kind: PaneKind::Terminal,
            cwd: "/tmp".to_string(),
            program: "shell".to_string(),
            exit_code: None,
            program_status: Some("running".to_string()),
            status: Some(Status::Running),
            size: None,
        }
    }

    #[test]
    fn pane_record_serde_round_trips() {
        let record = pane_example();
        let json = serde_json::to_string(&record).unwrap();
        assert_eq!(serde_json::from_str::<PaneRecord>(&json).unwrap(), record);
    }

    #[test]
    fn a_pane_with_no_title_exit_code_or_size_serialises_them_as_null_not_missing() {
        let record = pane_example();
        let value = serde_json::to_value(record).unwrap();
        assert!(value.get("title").is_some_and(serde_json::Value::is_null));
        assert!(
            value
                .get("exit_code")
                .is_some_and(serde_json::Value::is_null)
        );
        assert!(value.get("size").is_some_and(serde_json::Value::is_null));
    }

    /// `program_status`/`status` can disagree (`crates/kernel/src/pane.rs`'s
    /// own doc): a finished or failed status ends well before the program
    /// is run again, which is the whole reason the two fields exist
    /// separately rather than one being derived from the other on the wire.
    #[test]
    fn program_status_and_status_can_disagree() {
        let mut record = pane_example();
        record.program_status = Some("ended".to_string());
        record.status = None; // the `Finished`/`Failed` status already ended.

        let value = serde_json::to_value(record).unwrap();
        assert_eq!(value["program_status"], serde_json::json!("ended"));
        assert_eq!(value["status"], serde_json::Value::Null);
    }

    #[test]
    fn a_failed_status_serialises_as_an_object_keyed_failed_with_its_exit_code() {
        let mut record = pane_example();
        record.status = Some(Status::Failed { exit_code: 3 });

        let value = serde_json::to_value(record).unwrap();
        assert_eq!(
            value["status"],
            serde_json::json!({"failed": {"exit_code": 3}})
        );
    }

    /// Ties `PaneRecord`'s own shape to the description's, the same way
    /// `matches_the_fields_session_show_and_session_list_declare` does for
    /// `SessionRecord` above.
    #[test]
    fn matches_the_fields_pane_show_and_pane_list_declare() {
        use std::collections::HashSet;

        let api = crate::description::description();
        let value = serde_json::to_value(pane_example()).unwrap();
        let record_keys: HashSet<&str> = value
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();

        for dotted in ["pane.show", "pane.list"] {
            let method = api.method(dotted).unwrap();
            let fields = match &method.returns {
                crate::description::Returns::Record { fields }
                | crate::description::Returns::Records { fields } => fields,
                other @ (crate::description::Returns::Nothing
                | crate::description::Returns::Identifier { .. }
                | crate::description::Returns::Text
                | crate::description::Returns::Nested { .. }) => {
                    panic!("{dotted}'s returns is not a record: {other:?}")
                }
            };
            let declared: HashSet<&str> = fields.iter().map(|f| f.name.as_str()).collect();
            assert_eq!(
                declared, record_keys,
                "{dotted}'s declared fields and PaneRecord's own keys must match"
            );
        }
    }

    use super::WaitResult;

    #[test]
    fn wait_result_serde_round_trips() {
        let record = WaitResult { exit_code: 7 };
        let json = serde_json::to_string(&record).unwrap();
        assert_eq!(serde_json::from_str::<WaitResult>(&json).unwrap(), record);
    }

    /// Ties `WaitResult`'s own shape to the description's, the same way
    /// `matches_the_fields_session_show_and_session_list_declare` does for
    /// `SessionRecord` above.
    #[test]
    fn matches_the_fields_pane_wait_declares() {
        use std::collections::HashSet;

        let api = crate::description::description();
        let value = serde_json::to_value(WaitResult { exit_code: 7 }).unwrap();
        let record_keys: HashSet<&str> = value
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();

        let method = api.method("pane.wait").unwrap();
        let fields = match &method.returns {
            crate::description::Returns::Record { fields }
            | crate::description::Returns::Records { fields } => fields,
            other @ (crate::description::Returns::Nothing
            | crate::description::Returns::Identifier { .. }
            | crate::description::Returns::Text
            | crate::description::Returns::Nested { .. }) => {
                panic!("pane.wait's returns is not a record: {other:?}")
            }
        };
        let declared: HashSet<&str> = fields.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(
            declared, record_keys,
            "pane.wait's declared fields and WaitResult's own keys must match"
        );
    }

    use super::TabRecord;
    use crate::layout::Layout;

    fn tab_example() -> TabRecord {
        TabRecord {
            id: TabId::from_raw(1),
            label: Some("work".to_string()),
            title: None,
            position: 1,
            current_pane: PaneId::from_raw(1),
            layout: Layout::Pane(PaneId::from_raw(1)),
            shown_status: None,
        }
    }

    #[test]
    fn tab_record_serde_round_trips() {
        let record = tab_example();
        let json = serde_json::to_string(&record).unwrap();
        assert_eq!(serde_json::from_str::<TabRecord>(&json).unwrap(), record);
    }

    #[test]
    fn a_tab_with_no_title_or_shown_status_serialises_them_as_null_not_missing() {
        let record = tab_example();
        let value = serde_json::to_value(record).unwrap();
        assert!(value.get("title").is_some_and(serde_json::Value::is_null));
        assert!(
            value
                .get("shown_status")
                .is_some_and(serde_json::Value::is_null)
        );
    }

    #[test]
    fn a_tab_with_a_shown_status_carries_that_panes_id() {
        let mut record = tab_example();
        record.shown_status = Some(PaneId::from_raw(2));

        let value = serde_json::to_value(record).unwrap();
        assert_eq!(
            value["shown_status"],
            serde_json::json!(PaneId::from_raw(2).to_string())
        );
    }

    /// Ties `TabRecord`'s own shape to the description's, the same way
    /// `matches_the_fields_session_show_and_session_list_declare` does for
    /// `SessionRecord` above.
    #[test]
    fn matches_the_fields_tab_show_and_tab_list_declare() {
        use std::collections::HashSet;

        let api = crate::description::description();
        let value = serde_json::to_value(tab_example()).unwrap();
        let record_keys: HashSet<&str> = value
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();

        for dotted in ["tab.show", "tab.list"] {
            let method = api.method(dotted).unwrap();
            let fields = match &method.returns {
                crate::description::Returns::Record { fields }
                | crate::description::Returns::Records { fields } => fields,
                other @ (crate::description::Returns::Nothing
                | crate::description::Returns::Identifier { .. }
                | crate::description::Returns::Text
                | crate::description::Returns::Nested { .. }) => {
                    panic!("{dotted}'s returns is not a record: {other:?}")
                }
            };
            let declared: HashSet<&str> = fields.iter().map(|f| f.name.as_str()).collect();
            assert_eq!(
                declared, record_keys,
                "{dotted}'s declared fields and TabRecord's own keys must match"
            );
        }
    }

    use super::SettingRecord;

    fn setting_example() -> SettingRecord {
        SettingRecord {
            name: "appearance".to_string(),
            value: "system".to_string(),
            default: "system".to_string(),
        }
    }

    #[test]
    fn setting_record_serde_round_trips() {
        let record = setting_example();
        let json = serde_json::to_string(&record).unwrap();
        assert_eq!(
            serde_json::from_str::<SettingRecord>(&json).unwrap(),
            record
        );
    }

    /// Ties `SettingRecord`'s own shape to the description's, the same way
    /// `matches_the_fields_session_show_and_session_list_declare` does for
    /// `SessionRecord` above.
    #[test]
    fn matches_the_fields_setting_show_and_setting_list_declare() {
        use std::collections::HashSet;

        let api = crate::description::description();
        let value = serde_json::to_value(setting_example()).unwrap();
        let record_keys: HashSet<&str> = value
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();

        for dotted in ["setting.show", "setting.list"] {
            let method = api.method(dotted).unwrap();
            let fields = match &method.returns {
                crate::description::Returns::Record { fields }
                | crate::description::Returns::Records { fields } => fields,
                other @ (crate::description::Returns::Nothing
                | crate::description::Returns::Identifier { .. }
                | crate::description::Returns::Text
                | crate::description::Returns::Nested { .. }) => {
                    panic!("{dotted}'s returns is not a record: {other:?}")
                }
            };
            let declared: HashSet<&str> = fields.iter().map(|f| f.name.as_str()).collect();
            assert_eq!(
                declared, record_keys,
                "{dotted}'s declared fields and SettingRecord's own keys must match"
            );
        }
    }

    use super::ServerRecord;

    fn server_example() -> ServerRecord {
        ServerRecord {
            version: "steg/0.1.0".to_string(),
            uptime: 42,
            sessions: 2,
            clients: 3,
        }
    }

    #[test]
    fn server_record_serde_round_trips() {
        let record = server_example();
        let json = serde_json::to_string(&record).unwrap();
        assert_eq!(serde_json::from_str::<ServerRecord>(&json).unwrap(), record);
    }

    /// Ties `ServerRecord`'s own shape to the description's, the same way
    /// `matches_the_fields_session_show_and_session_list_declare` does for
    /// `SessionRecord` above - `server.show` is the only method with a
    /// record to check (`server.start`/`server.stop` both return Nothing).
    #[test]
    fn matches_the_fields_server_show_declares() {
        use std::collections::HashSet;

        let api = crate::description::description();
        let value = serde_json::to_value(server_example()).unwrap();
        let record_keys: HashSet<&str> = value
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();

        let method = api.method("server.show").unwrap();
        let fields = match &method.returns {
            crate::description::Returns::Record { fields }
            | crate::description::Returns::Records { fields } => fields,
            other @ (crate::description::Returns::Nothing
            | crate::description::Returns::Identifier { .. }
            | crate::description::Returns::Text
            | crate::description::Returns::Nested { .. }) => {
                panic!("server.show's returns is not a record: {other:?}")
            }
        };
        let declared: HashSet<&str> = fields.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(
            declared, record_keys,
            "server.show's declared fields and ServerRecord's own keys must match"
        );
    }

    use super::{SessionTree, TabTree, TreeSnapshot};

    fn tab_tree_example() -> TabTree {
        TabTree {
            tab: tab_example(),
            panes: vec![pane_example()],
        }
    }

    fn session_tree_example() -> SessionTree {
        SessionTree {
            session: example(),
            tabs: vec![tab_tree_example()],
        }
    }

    fn tree_snapshot_example() -> TreeSnapshot {
        TreeSnapshot {
            sessions: vec![session_tree_example()],
        }
    }

    #[test]
    fn tree_snapshot_serde_round_trips() {
        let snapshot = tree_snapshot_example();
        let json = serde_json::to_string(&snapshot).unwrap();
        assert_eq!(
            serde_json::from_str::<TreeSnapshot>(&json).unwrap(),
            snapshot
        );
    }

    /// `TabTree`'s own fields are `TabRecord`'s, flattened, plus `panes` -
    /// not a second, nested `tab` key: a client reads a tab's `id`/
    /// `label`/`layout`/... directly, the same shape every other record
    /// already has, with `panes` as the one addition.
    #[test]
    fn tab_tree_flattens_the_tab_record_and_adds_panes_beside_it() {
        let value = serde_json::to_value(tab_tree_example()).unwrap();
        let object = value.as_object().unwrap();
        assert_eq!(
            object["id"],
            serde_json::json!(tab_example().id.to_string())
        );
        assert!(object.get("tab").is_none(), "{object:?}");
        assert_eq!(object["panes"].as_array().unwrap().len(), 1);
    }

    /// Same as above, one level up: a session's own fields are flattened
    /// beside `tabs`, not nested under a second `session` key.
    #[test]
    fn session_tree_flattens_the_session_record_and_adds_tabs_beside_it() {
        let value = serde_json::to_value(session_tree_example()).unwrap();
        let object = value.as_object().unwrap();
        assert_eq!(object["id"], serde_json::json!(example().id.to_string()));
        assert!(object.get("session").is_none(), "{object:?}");
        assert_eq!(object["tabs"].as_array().unwrap().len(), 1);
    }

    /// The pane-identity question (TASK-1.46's own plan, section 2):
    /// a tab's `layout` names a pane by id (a structural reference), and
    /// that same id is one of `panes`' own entries (the data) - proven
    /// directly, not just argued, so a future change that lets them
    /// drift apart fails this test.
    #[test]
    fn a_layout_leaf_names_a_pane_that_is_actually_in_the_tabs_own_panes() {
        use crate::layout::Layout;

        let tree = tab_tree_example();
        let Layout::Pane(leaf_id) = tree.tab.layout else {
            panic!("tab_example's layout is a single pane leaf");
        };
        assert!(
            tree.panes.iter().any(|pane| pane.id == leaf_id),
            "the layout names pane {leaf_id}, which is not among this tab's own panes"
        );
    }
}
