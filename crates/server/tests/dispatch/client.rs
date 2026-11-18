use serde_json::json;
use steg_kernel::Settings;
use steg_sdk::{ClientKind, Description, ErrorCode};
use steg_server::dispatch;

use crate::fixtures::{err, fresh, moment, ok, request};

#[test]
fn client_list_returns_every_connected_client_by_default_except_cli() {
    let (mut tree, context, api) = fresh();
    let app = tree.connect_client(ClientKind::App, None);
    let _cli = tree.connect_client(ClientKind::Cli, None);
    let plugin = tree.connect_client(ClientKind::Plugin, None);

    let value = ok(dispatch(
        &mut tree,
        &mut Settings::new(),
        &api,
        &context,
        &request("client.list", vec![]),
        moment(0),
    ));

    let ids: Vec<String> = value
        .as_array()
        .expect("client.list returns an array")
        .iter()
        .map(|record| record["id"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(ids, vec![app.to_string(), plugin.to_string()]);
}

#[test]
fn client_list_with_all_includes_cli_clients_too() {
    let (mut tree, context, api) = fresh();
    let app = tree.connect_client(ClientKind::App, None);
    let cli = tree.connect_client(ClientKind::Cli, None);
    let plugin = tree.connect_client(ClientKind::Plugin, None);

    let value = ok(dispatch(
        &mut tree,
        &mut Settings::new(),
        &api,
        &context,
        &request("client.list", vec![("all", json!(true))]),
        moment(0),
    ));

    let ids: Vec<String> = value
        .as_array()
        .expect("client.list returns an array")
        .iter()
        .map(|record| record["id"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(
        ids,
        vec![app.to_string(), cli.to_string(), plugin.to_string()]
    );
}

#[test]
fn client_list_matches_exactly_the_fields_the_description_declares() {
    let (mut tree, context, api) = fresh();
    tree.connect_client(ClientKind::App, Some("work".to_string()));

    let value = ok(dispatch(
        &mut tree,
        &mut Settings::new(),
        &api,
        &context,
        &request("client.list", vec![]),
        moment(0),
    ));

    let records = value.as_array().expect("client.list returns an array");
    assert_eq!(records.len(), 1);
    let declared = declared_fields(&api, "client.list");
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
fn client_show_resolves_by_id_label_position_and_suffix() {
    let (mut tree, context, api) = fresh();
    let labelled = tree.connect_client(ClientKind::App, Some("work".to_string()));
    let _other = tree.connect_client(ClientKind::Cli, None);

    for target in [
        labelled.to_string(),
        "work".to_string(),
        "1".to_string(),
        labelled.to_string()[4..].to_string(),
    ] {
        let value = ok(dispatch(
            &mut tree,
            &mut Settings::new(),
            &api,
            &context,
            &request("client.show", vec![("target", json!(target))]),
            moment(0),
        ));
        assert_eq!(value["id"], json!(labelled.to_string()), "target {target}");
    }
}

#[test]
fn client_show_matches_exactly_the_fields_the_description_declares() {
    let (mut tree, context, api) = fresh();
    let client = tree.connect_client(ClientKind::App, None);

    let value = ok(dispatch(
        &mut tree,
        &mut Settings::new(),
        &api,
        &context,
        &request("client.show", vec![("target", json!(client.to_string()))]),
        moment(0),
    ));

    let declared = declared_fields(&api, "client.show");
    let returned: Vec<String> = value
        .as_object()
        .expect("a client record is an object")
        .keys()
        .cloned()
        .collect();
    assert_same_set(&declared, &returned);
}

#[test]
fn client_show_reports_the_sessions_it_views() {
    let (mut tree, context, api) = fresh();
    let client = tree.connect_client(ClientKind::App, None);
    tree.view_pane(client, context.default_pane.unwrap())
        .unwrap();

    let value = ok(dispatch(
        &mut tree,
        &mut Settings::new(),
        &api,
        &context,
        &request("client.show", vec![("target", json!(client.to_string()))]),
        moment(0),
    ));

    assert_eq!(
        value["sessions"],
        json!([context.default_session.unwrap().to_string()])
    );
}

#[test]
fn client_show_an_unknown_target_is_no_such_target_and_an_ambiguous_one_is_ambiguous_target() {
    let (mut tree, context, api) = fresh();
    let first = tree.connect_client(ClientKind::App, Some("work".to_string()));
    let second = tree.connect_client(ClientKind::Cli, Some("work".to_string()));

    let not_found = err(dispatch(
        &mut tree,
        &mut Settings::new(),
        &api,
        &context,
        &request("client.show", vec![("target", json!("nonsense"))]),
        moment(0),
    ));
    assert_eq!(not_found.code, ErrorCode::NoSuchTarget);

    let ambiguous = err(dispatch(
        &mut tree,
        &mut Settings::new(),
        &api,
        &context,
        &request("client.show", vec![("target", json!("work"))]),
        moment(0),
    ));
    assert_eq!(ambiguous.code, ErrorCode::AmbiguousTarget);
    assert_eq!(
        ambiguous.candidates,
        vec![first.to_string(), second.to_string()]
    );
}

#[test]
fn client_list_and_show_change_nothing_in_the_tree() {
    let (mut tree, context, api) = fresh();
    let client = tree.connect_client(ClientKind::App, Some("work".to_string()));
    let before = tree.clients().to_vec();

    ok(dispatch(
        &mut tree,
        &mut Settings::new(),
        &api,
        &context,
        &request("client.list", vec![]),
        moment(0),
    ));
    assert_eq!(
        tree.clients().to_vec(),
        before,
        "client.list changed the tree"
    );

    ok(dispatch(
        &mut tree,
        &mut Settings::new(),
        &api,
        &context,
        &request("client.show", vec![("target", json!(client.to_string()))]),
        moment(0),
    ));
    assert_eq!(
        tree.clients().to_vec(),
        before,
        "client.show changed the tree"
    );
}
