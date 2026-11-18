use serde_json::json;
use steg_kernel::Settings;
use steg_sdk::{Description, ErrorCode, SessionId};
use steg_server::dispatch;

use crate::fixtures::{err, fresh, moment, ok, request};

#[test]
fn session_new_creates_a_plain_shell_session() {
    let (mut tree, context, api) = fresh();
    let before = tree.sessions().len();

    let value = ok(dispatch(
        &mut tree,
        &mut Settings::new(),
        &api,
        &context,
        &request("session.new", vec![]),
        moment(0),
    ));

    assert_eq!(tree.sessions().len(), before + 1);
    assert!(value.get("id").is_some());
}

#[test]
fn session_show_matches_exactly_the_fields_the_description_declares() {
    let (mut tree, context, api) = fresh();

    let value = ok(dispatch(
        &mut tree,
        &mut Settings::new(),
        &api,
        &context,
        &request("session.show", vec![]),
        moment(0),
    ));

    let declared = declared_fields(&api, "session.show");
    let returned: Vec<String> = value
        .as_object()
        .expect("a session record is an object")
        .keys()
        .cloned()
        .collect();
    assert_same_set(&declared, &returned);
    assert_eq!(value["position"], 1);
}

#[test]
fn session_list_matches_exactly_the_fields_the_description_declares_per_record() {
    let (mut tree, context, api) = fresh();
    tree.new_session(Some("second".to_string())).unwrap();

    let value = ok(dispatch(
        &mut tree,
        &mut Settings::new(),
        &api,
        &context,
        &request("session.list", vec![]),
        moment(0),
    ));

    let records = value.as_array().expect("session.list returns an array");
    assert_eq!(records.len(), 2);
    let declared = declared_fields(&api, "session.list");
    for record in records {
        let keys: Vec<String> = record.as_object().unwrap().keys().cloned().collect();
        assert_same_set(&declared, &keys);
    }
}

fn declared_fields(api: &Description, dotted: &str) -> Vec<String> {
    let method = api.method(dotted).unwrap();
    match &method.returns {
        steg_sdk::Returns::Record { fields } | steg_sdk::Returns::Records { fields } => {
            fields.iter().map(|f| f.name.clone()).collect()
        }
        other @ (steg_sdk::Returns::Nothing
        | steg_sdk::Returns::Identifier { .. }
        | steg_sdk::Returns::Text
        | steg_sdk::Returns::Nested { .. }) => {
            panic!("{dotted}'s returns is not a record: {other:?}")
        }
    }
}

fn assert_same_set(expected: &[String], actual: &[String]) {
    let mut expected = expected.to_vec();
    let mut actual = actual.to_vec();
    expected.sort();
    actual.sort();
    assert_eq!(expected, actual);
}

#[test]
fn session_close_removes_it() {
    let (mut tree, context, api) = fresh();

    ok(dispatch(
        &mut tree,
        &mut Settings::new(),
        &api,
        &context,
        &request("session.close", vec![]),
        moment(0),
    ));

    assert!(tree.session(context.default_session.unwrap()).is_none());
}

#[test]
fn session_move_converts_the_wire_position_before_calling_the_kernel() {
    let (mut tree, context, api) = fresh();
    let second = tree.new_session(None).unwrap();

    ok(dispatch(
        &mut tree,
        &mut Settings::new(),
        &api,
        &context,
        &request(
            "session.move",
            vec![
                ("target", json!(second.to_string())),
                ("position", json!(1)),
            ],
        ),
        moment(0),
    ));

    assert_eq!(tree.sessions()[0].id(), second);
}

#[test]
fn session_move_rejects_a_zero_position_without_touching_the_tree() {
    let (mut tree, context, api) = fresh();
    let second = tree.new_session(None).unwrap();
    let before: Vec<SessionId> = tree
        .sessions()
        .iter()
        .map(steg_kernel::Session::id)
        .collect();

    let error = err(dispatch(
        &mut tree,
        &mut Settings::new(),
        &api,
        &context,
        &request(
            "session.move",
            vec![
                ("target", json!(second.to_string())),
                ("position", json!(0)),
            ],
        ),
        moment(0),
    ));

    assert_eq!(error.code, ErrorCode::BadRequest);
    let after: Vec<SessionId> = tree
        .sessions()
        .iter()
        .map(steg_kernel::Session::id)
        .collect();
    assert_eq!(before, after);
}

#[test]
fn session_set_and_unset_change_and_clear_the_label() {
    let (mut tree, context, api) = fresh();

    ok(dispatch(
        &mut tree,
        &mut Settings::new(),
        &api,
        &context,
        &request("session.set", vec![("label", json!("work"))]),
        moment(0),
    ));
    assert_eq!(
        tree.session(context.default_session.unwrap())
            .unwrap()
            .label(),
        Some("work")
    );

    ok(dispatch(
        &mut tree,
        &mut Settings::new(),
        &api,
        &context,
        &request("session.unset", vec![("label", json!(true))]),
        moment(0),
    ));
    assert_eq!(
        tree.session(context.default_session.unwrap())
            .unwrap()
            .label(),
        None
    );
}

#[test]
fn session_show_with_an_unknown_target_is_no_such_target() {
    let (mut tree, context, api) = fresh();
    let unknown = SessionId::from_raw(999);

    let error = err(dispatch(
        &mut tree,
        &mut Settings::new(),
        &api,
        &context,
        &request("session.show", vec![("target", json!(unknown.to_string()))]),
        moment(0),
    ));

    assert_eq!(error.code, ErrorCode::NoSuchTarget);
}
