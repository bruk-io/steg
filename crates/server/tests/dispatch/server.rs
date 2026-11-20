//! `server.show`/`server.start` (TASK-1.88, doc-2 section 8) - plain
//! `dispatch` arms, driven through the public `dispatch` function exactly
//! like every other noun in this binary. `server.stop` is not here: it is
//! intercepted in `connection.rs`, never reached through `dispatch` in the
//! real system (the same reason `pane.attach`/`pane.detach`/
//! `tab.declare-size` have no file here either).

use serde_json::json;
use steg_kernel::Settings;
use steg_sdk::{ClientKind, Description};
use steg_server::dispatch;

use crate::fixtures::{fresh, moment, ok, request};

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
fn server_show_matches_exactly_the_fields_the_description_declares() {
    let (mut tree, context, api) = fresh();

    let value = ok(dispatch(
        &mut tree,
        &mut Settings::new(),
        &api,
        &context,
        &request("server.show", vec![]),
        moment(0),
    ));

    let declared = declared_fields(&api, "server.show");
    let returned: Vec<String> = value
        .as_object()
        .expect("a server record is an object")
        .keys()
        .cloned()
        .collect();
    assert_same_set(&declared, &returned);
}

/// AC #1's own "time is read by the server and reaches anything pure as
/// data" - proven directly: `server.show`'s own `uptime` is exactly the
/// `Moment` this call is given, not re-read from a real clock anywhere
/// downstream (which would make this test flaky, never a fixed number).
#[test]
fn server_show_reports_exactly_the_moment_its_given_as_uptime() {
    let (mut tree, context, api) = fresh();

    let value = ok(dispatch(
        &mut tree,
        &mut Settings::new(),
        &api,
        &context,
        &request("server.show", vec![]),
        moment(321),
    ));

    assert_eq!(value["uptime"], json!(321));
}

#[test]
fn server_show_counts_every_session_and_client_the_tree_holds() {
    let (mut tree, context, api) = fresh();
    // `fresh()` already minted one session; two more here, plus two
    // connected clients, so the counts below are not "1" by coincidence.
    tree.new_session(None).unwrap();
    tree.new_session(None).unwrap();
    tree.connect_client(ClientKind::App, None);
    tree.connect_client(ClientKind::Cli, None);

    let value = ok(dispatch(
        &mut tree,
        &mut Settings::new(),
        &api,
        &context,
        &request("server.show", vec![]),
        moment(0),
    ));

    assert_eq!(value["sessions"], json!(3));
    assert_eq!(value["clients"], json!(2));
}

#[test]
fn server_show_with_no_sessions_or_clients_reports_zero() {
    let (mut tree, context, api) = fresh();
    // `fresh()` minted one session for its own `Context` defaults to
    // point at - closed here so this test's own claim ("zero") is real.
    let session = context.default_session.unwrap();
    tree.close_session(session).unwrap();

    let value = ok(dispatch(
        &mut tree,
        &mut Settings::new(),
        &api,
        &context,
        &request("server.show", vec![]),
        moment(0),
    ));

    assert_eq!(value["sessions"], json!(0));
    assert_eq!(value["clients"], json!(0));
}

#[test]
fn server_start_always_answers_nothing_regardless_of_on_demand() {
    let (mut tree, context, api) = fresh();

    for params in [
        vec![],
        vec![("on_demand", json!(true))],
        vec![("on_demand", json!(false))],
    ] {
        let value = ok(dispatch(
            &mut tree,
            &mut Settings::new(),
            &api,
            &context,
            &request("server.start", params),
            moment(0),
        ));
        assert_eq!(value, serde_json::Value::Null);
    }
}
