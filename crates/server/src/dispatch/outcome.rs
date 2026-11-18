//! What a served arm hands back, and the one place it becomes the JSON
//! value a client reads (AC #1, AC #3).
//!
//! One variant per shape a served method in this task actually produces —
//! no more: a variant with no producer would say something untrue
//! (`.claude/rules/rust.md`). `api.call` never builds one of these itself;
//! it recurses and returns the inner call's own `Response` unchanged
//! (`mod.rs`).

use serde_json::Value;
use steg_sdk::{
    ClientRecord, Description, Method, PaneId, PaneRecord, SessionId, SessionRecord, SettingRecord,
    TabId, TabRecord,
};

/// A served method's typed result, before it becomes the `Value` a
/// [`steg_sdk::Response::Ok`] carries.
#[derive(Debug, Clone, PartialEq)]
pub enum Outcome {
    /// An action that worked, with nothing to report (`Returns::Nothing`).
    Nothing,
    /// A new session's identifier (`Returns::Identifier`).
    Session(SessionId),
    /// A new tab's identifier.
    Tab(TabId),
    /// A new pane's identifier.
    Pane(PaneId),
    /// One session, in full.
    SessionRecord(SessionRecord),
    /// Every session.
    SessionRecords(Vec<SessionRecord>),
    /// One method, in full (`api.show` with a target).
    Method(Box<Method>),
    /// Every method (`api.list`).
    Methods(Vec<Method>),
    /// The whole API description (`api.show` with no target).
    Description(Box<Description>),
    /// One setting, in full (`setting.show`).
    SettingRecord(SettingRecord),
    /// Every setting (`setting.list`).
    SettingRecords(Vec<SettingRecord>),
    /// One pane, in full (`pane.show`; TASK-1.76 AC #2).
    PaneRecord(PaneRecord),
    /// Every pane of a tab (`pane.list`; TASK-1.89).
    PaneRecords(Vec<PaneRecord>),
    /// One tab, in full (`tab.show`; TASK-1.89).
    TabRecord(TabRecord),
    /// Every tab of a session (`tab.list`; TASK-1.89).
    TabRecords(Vec<TabRecord>),
    /// One client, in full (`client.show`; TASK-1.141).
    ClientRecord(ClientRecord),
    /// Every client connected to the server (`client.list`; TASK-1.141).
    ClientRecords(Vec<ClientRecord>),
    /// `pane.close`: what `Tree::close_pane` actually closed (TASK-1.46) -
    /// serialises the same as `Nothing` (doc-2's own `Returns::Nothing`
    /// contract for this method is unchanged; this variant exists so
    /// `close_and_dispatch` can read the kernel's own cascade directly,
    /// from what it returned, rather than re-deriving it with a second
    /// read of the tree).
    PaneClosed(steg_kernel::ClosedPane),
    /// `tab.close`: what `Tree::close_tab` actually closed, same reason.
    TabClosed(steg_kernel::TabClosed),
    /// `pane.move`: what `Tree::move_pane_to_tab` actually did, including
    /// whether it cascaded into closing the source tab and/or session
    /// (R94, TASK-1.46 - the same class of bug `PaneClosed`/`TabClosed`
    /// above were added for, found one round later on this method). Same
    /// reason: serialises as `Nothing` (doc-2's own `Returns::Nothing`
    /// contract for `pane.move` is unchanged); exists so `Runtime`'s own
    /// `move_and_dispatch` can read the kernel's own cascade directly.
    PaneMoved(steg_kernel::MovedPane),
}

impl Outcome {
    /// Turns this outcome into the `Value` a client reads back.
    ///
    /// An id variant becomes `{"id": "<it>"}` — naming decision-1 section
    /// 8's own flagged rough edge ("the real format should name that
    /// field") rather than leaving the field unnamed, as the spike did.
    /// The record/`Method`/`Methods`/`Description` variants are
    /// `serde_json::to_value` of the already-typed value: the sdk already
    /// defines their wire form (R105), so this function adds nothing of
    /// its own to it.
    ///
    /// # Panics
    ///
    /// Never in practice: every type here derives `Serialize` over plain
    /// data with no cycle and no non-finite float — `Ratio` already
    /// refuses the one float that could ever fail (`NaN`/infinity) before
    /// it reaches a `Tree`, so `serde_json::to_value` cannot fail on any
    /// value this task ever builds.
    #[must_use]
    pub fn into_value(self) -> Value {
        match self {
            Self::Nothing => Value::Null,
            Self::Session(id) => id_value(id),
            Self::Tab(id) => id_value(id),
            Self::Pane(id) => id_value(id),
            Self::SessionRecord(record) => {
                serde_json::to_value(record).expect("a SessionRecord always serialises")
            }
            Self::SessionRecords(records) => {
                serde_json::to_value(records).expect("a Vec<SessionRecord> always serialises")
            }
            Self::Method(method) => {
                serde_json::to_value(*method).expect("a Method always serialises")
            }
            Self::Methods(methods) => {
                serde_json::to_value(methods).expect("a Vec<Method> always serialises")
            }
            Self::Description(description) => {
                serde_json::to_value(*description).expect("a Description always serialises")
            }
            Self::SettingRecord(record) => {
                serde_json::to_value(record).expect("a SettingRecord always serialises")
            }
            Self::SettingRecords(records) => {
                serde_json::to_value(records).expect("a Vec<SettingRecord> always serialises")
            }
            Self::PaneRecord(record) => {
                serde_json::to_value(record).expect("a PaneRecord always serialises")
            }
            Self::PaneRecords(records) => {
                serde_json::to_value(records).expect("a Vec<PaneRecord> always serialises")
            }
            Self::TabRecord(record) => {
                serde_json::to_value(record).expect("a TabRecord always serialises")
            }
            Self::TabRecords(records) => {
                serde_json::to_value(records).expect("a Vec<TabRecord> always serialises")
            }
            Self::ClientRecord(record) => {
                serde_json::to_value(record).expect("a ClientRecord always serialises")
            }
            Self::ClientRecords(records) => {
                serde_json::to_value(records).expect("a Vec<ClientRecord> always serialises")
            }
            Self::PaneClosed(_) | Self::TabClosed(_) | Self::PaneMoved(_) => Value::Null,
        }
    }
}

fn id_value(id: impl std::fmt::Display) -> Value {
    serde_json::json!({ "id": id.to_string() })
}

#[cfg(test)]
mod tests {
    use super::Outcome;
    use serde_json::json;
    use steg_sdk::{PaneId, SessionId, TabId};

    #[test]
    fn nothing_becomes_null() {
        assert_eq!(Outcome::Nothing.into_value(), serde_json::Value::Null);
    }

    #[test]
    fn an_id_variant_becomes_an_object_naming_the_id_field() {
        let id = SessionId::from_raw(1);
        assert_eq!(
            Outcome::Session(id).into_value(),
            json!({ "id": id.to_string() })
        );
        let tab = TabId::from_raw(2);
        assert_eq!(
            Outcome::Tab(tab).into_value(),
            json!({ "id": tab.to_string() })
        );
        let pane = PaneId::from_raw(3);
        assert_eq!(
            Outcome::Pane(pane).into_value(),
            json!({ "id": pane.to_string() })
        );
    }

    #[test]
    fn a_session_record_serialises_to_its_own_typed_shape() {
        let record = steg_sdk::SessionRecord {
            id: SessionId::from_raw(1),
            label: Some("work".to_string()),
            position: 1,
            colour: steg_sdk::Colour::Violet,
            current_tab: TabId::from_raw(2),
        };

        let value = Outcome::SessionRecord(record.clone()).into_value();

        assert_eq!(value, serde_json::to_value(record).unwrap());
    }

    #[test]
    fn a_setting_record_serialises_to_its_own_typed_shape() {
        let record = steg_sdk::SettingRecord {
            name: "appearance".to_string(),
            value: "system".to_string(),
            default: "system".to_string(),
        };

        let value = Outcome::SettingRecord(record.clone()).into_value();

        assert_eq!(value, serde_json::to_value(record).unwrap());
    }

    fn pane_record_example() -> steg_sdk::PaneRecord {
        steg_sdk::PaneRecord {
            id: PaneId::from_raw(1),
            label: None,
            title: None,
            kind: steg_sdk::PaneKind::Terminal,
            cwd: "/tmp".to_string(),
            program: "shell".to_string(),
            exit_code: None,
            program_status: Some("running".to_string()),
            status: Some(steg_sdk::Status::Running),
            size: None,
        }
    }

    #[test]
    fn a_pane_record_serialises_to_its_own_typed_shape() {
        let record = pane_record_example();

        let value = Outcome::PaneRecord(record.clone()).into_value();

        assert_eq!(value, serde_json::to_value(record).unwrap());
    }

    #[test]
    fn a_vec_of_pane_records_serialises_to_its_own_typed_shape() {
        let records = vec![pane_record_example()];

        let value = Outcome::PaneRecords(records.clone()).into_value();

        assert_eq!(value, serde_json::to_value(records).unwrap());
    }

    fn tab_record_example() -> steg_sdk::TabRecord {
        steg_sdk::TabRecord {
            id: TabId::from_raw(1),
            label: None,
            title: None,
            position: 1,
            current_pane: PaneId::from_raw(1),
            layout: steg_sdk::Layout::Pane(PaneId::from_raw(1)),
            shown_status: None,
        }
    }

    #[test]
    fn a_tab_record_serialises_to_its_own_typed_shape() {
        let record = tab_record_example();

        let value = Outcome::TabRecord(record.clone()).into_value();

        assert_eq!(value, serde_json::to_value(record).unwrap());
    }

    #[test]
    fn a_vec_of_tab_records_serialises_to_its_own_typed_shape() {
        let records = vec![tab_record_example()];

        let value = Outcome::TabRecords(records.clone()).into_value();

        assert_eq!(value, serde_json::to_value(records).unwrap());
    }

    fn client_record_example() -> steg_sdk::ClientRecord {
        steg_sdk::ClientRecord {
            id: steg_sdk::ClientId::from_raw(1),
            kind: steg_sdk::ClientKind::App,
            label: None,
            sessions: vec![SessionId::from_raw(1)],
            last_active: None,
        }
    }

    #[test]
    fn a_client_record_serialises_to_its_own_typed_shape() {
        let record = client_record_example();

        let value = Outcome::ClientRecord(record.clone()).into_value();

        assert_eq!(value, serde_json::to_value(record).unwrap());
    }

    #[test]
    fn a_vec_of_client_records_serialises_to_its_own_typed_shape() {
        let records = vec![client_record_example()];

        let value = Outcome::ClientRecords(records.clone()).into_value();

        assert_eq!(value, serde_json::to_value(records).unwrap());
    }
}
