//! Records a client reads back over the API (doc-16 R105).
//!
//! `SessionRecord` was the first this crate added: `session.list`/
//! `session.show` are the only served methods (TASK-1.43's dispatch task)
//! whose `returns` is a record at all. A `TabRecord`/`PaneRecord` arrives
//! with the task that gives the kernel the fields they need (title,
//! shown_status, cwd, program, exit code, status — TASK-1.39/1.41) — this
//! crate fabricates nothing to fill a shape the kernel can't produce yet.
//!
//! `SettingRecord` (TASK-1.48) is the second: `setting.list`/`setting.show`
//! read back a registry entry (`crate::setting::settings`) and its current
//! value, every field as its own wire text (R80's settings are never typed
//! per entry on the wire — see `crate::setting`'s module doc).

use serde::{Deserialize, Serialize};

use crate::colour::Colour;
use crate::ids::{SessionId, TabId};

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
                | crate::description::Returns::Text) => {
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
                | crate::description::Returns::Text) => {
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
}
