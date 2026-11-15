//! Functional tests for `dispatch`: a real `Tree`, no doubles
//! (`.claude/rules/testing-rust.md`), driven only through the public API
//! (`steg_server::{dispatch, Context, Outcome}`), the way TASK-1.45 will
//! call it.

// `clippy.toml`'s `allow-unwrap-in-tests` only recognises `#[cfg(test)]`
// unit test modules, not a file under `tests/` - the whole of which is
// already test-only, with no production code for a stray `.unwrap()` to
// reach. rust.md's rule ("no unwrap outside tests") is still satisfied.
#![expect(
    clippy::unwrap_used,
    reason = "this whole file is a test; see the comment above"
)]

use serde_json::{Value, json};
use steg_kernel::Tree;
use steg_sdk::{Description, ErrorCode, Request, Response, SessionId, TabId, description};
use steg_server::{Context, dispatch};

fn request(method: &str, pairs: Vec<(&str, Value)>) -> Request {
    Request {
        method: method.into(),
        params: pairs.into_iter().map(|(k, v)| (k.to_owned(), v)).collect(),
    }
}

/// A tree with one session, one tab and one pane - `Tree::new`'s own
/// deterministic ids - plus a `Context` defaulted to that path and the
/// real API description.
fn fresh() -> (Tree, Context, Description) {
    let mut tree = Tree::new();
    let session = tree.new_session(None).unwrap();
    let tab = tree.session(session).unwrap().tabs()[0].id();
    let pane = tree.session(session).unwrap().tabs()[0].panes()[0].id();
    (
        tree,
        Context {
            default_session: session,
            default_tab: tab,
            default_pane: pane,
        },
        description(),
    )
}

fn ok(response: Response) -> Value {
    match response {
        Response::Ok(value) => value,
        Response::Error(error) => panic!("expected Response::Ok, got {error:?}"),
    }
}

fn err(response: Response) -> steg_sdk::ApiError {
    match response {
        Response::Error(error) => error,
        Response::Ok(value) => panic!("expected Response::Error, got {value}"),
    }
}

// --- completeness (AC #2) --------------------------------------------------

#[test]
fn every_described_method_gets_a_response_with_no_panic() {
    let (mut tree, context, api) = fresh();

    for method in &api.methods {
        let dotted = method.name.dotted();
        // A minimally-valid request: every required param present with a
        // value of the right kind, so `validate` never refuses before
        // dispatch's own arm runs - this is a smoke test against a panic
        // in any arm, not a test of each arm's own behaviour (that is
        // every other test in this file).
        let params = method
            .params
            .iter()
            .filter(|p| p.required)
            .map(|p| (p.name.as_str(), minimal_value(&p.value)))
            .collect();

        // Not asserted on: only that calling it does not panic.
        let _ = dispatch(&mut tree, &api, &context, &request(&dotted, params));
    }
}

fn minimal_value(kind: &steg_sdk::ValueKind) -> Value {
    match kind {
        steg_sdk::ValueKind::Text | steg_sdk::ValueKind::Target | steg_sdk::ValueKind::Path => {
            json!("x")
        }
        steg_sdk::ValueKind::Integer => json!(1),
        steg_sdk::ValueKind::Boolean => json!(true),
        steg_sdk::ValueKind::Choice(words) => json!(words[0]),
        steg_sdk::ValueKind::Argv => json!(["x"]),
    }
}

/// doc-2's table against every arm (not the sdk's own method-set test,
/// re-run here for good measure): for each of this task's 19
/// `NotImplementedYet` scenarios, dispatch answers that code, names what
/// it waits on, and creates nothing.
/// One `NotImplementedYet` scenario: the method called, its extra
/// parameters beyond a bare target, and a substring its error message
/// must name.
type NotYetCase = (&'static str, Vec<(&'static str, Value)>, &'static str);

#[test]
fn every_not_yet_scenario_answers_not_implemented_yet_and_changes_nothing() {
    let cases: Vec<NotYetCase> = vec![
        ("session.focus", vec![], "a client/window"),
        ("tab.list", vec![], "title"),
        ("tab.show", vec![], "title"),
        (
            "tab.move",
            vec![("to", json!("placeholder"))],
            "move a tab between sessions",
        ),
        (
            "pane.resize",
            vec![("direction", json!("right")), ("by", json!(1))],
            "Tab::size",
        ),
        ("pane.list", vec![], "title/cwd/program"),
        ("pane.show", vec![], "title/cwd/program"),
        ("pane.send", vec![], "start_program"),
        ("pane.capture", vec![], "start_program"),
        ("pane.wait", vec![("for", json!("exit"))], "start_program"),
        ("server.show", vec![], "socket and lifecycle"),
        ("server.start", vec![], "socket and lifecycle"),
        ("server.stop", vec![], "socket and lifecycle"),
        ("event.watch", vec![], "events"),
        ("client.list", vec![], "client model"),
        (
            "client.show",
            vec![("target", json!("placeholder"))],
            "client model",
        ),
        (
            "session.new",
            vec![("cwd", json!("/tmp"))],
            "program and working directory",
        ),
        (
            "tab.new",
            vec![("program", json!(["htop"]))],
            "program and working directory",
        ),
        (
            "pane.split",
            vec![("direction", json!("right")), ("cwd", json!("/tmp"))],
            "program and working directory",
        ),
    ];

    for (method, extra, expected_substring) in cases {
        let (mut tree, context, api) = fresh();
        let before = tree.sessions().to_vec();

        let error = err(dispatch(&mut tree, &api, &context, &request(method, extra)));

        assert_eq!(error.code, ErrorCode::NotImplementedYet, "{method}");
        assert!(
            error.message.contains(expected_substring),
            "{method}: {:?} does not name {expected_substring:?}",
            error.message
        );
        assert_eq!(
            tree.sessions().to_vec(),
            before,
            "{method} changed the tree"
        );
    }
}

// --- session ----------------------------------------------------------------

#[test]
fn session_new_creates_a_plain_shell_session() {
    let (mut tree, context, api) = fresh();
    let before = tree.sessions().len();

    let value = ok(dispatch(
        &mut tree,
        &api,
        &context,
        &request("session.new", vec![]),
    ));

    assert_eq!(tree.sessions().len(), before + 1);
    assert!(value.get("id").is_some());
}

#[test]
fn session_show_matches_exactly_the_fields_the_description_declares() {
    let (mut tree, context, api) = fresh();

    let value = ok(dispatch(
        &mut tree,
        &api,
        &context,
        &request("session.show", vec![]),
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
        &api,
        &context,
        &request("session.list", vec![]),
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
        | steg_sdk::Returns::Text) => panic!("{dotted}'s returns is not a record: {other:?}"),
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
        &api,
        &context,
        &request("session.close", vec![]),
    ));

    assert!(tree.session(context.default_session).is_none());
}

#[test]
fn session_move_converts_the_wire_position_before_calling_the_kernel() {
    let (mut tree, context, api) = fresh();
    let second = tree.new_session(None).unwrap();

    ok(dispatch(
        &mut tree,
        &api,
        &context,
        &request(
            "session.move",
            vec![
                ("target", json!(second.to_string())),
                ("position", json!(1)),
            ],
        ),
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
        &api,
        &context,
        &request(
            "session.move",
            vec![
                ("target", json!(second.to_string())),
                ("position", json!(0)),
            ],
        ),
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
        &api,
        &context,
        &request("session.set", vec![("label", json!("work"))]),
    ));
    assert_eq!(
        tree.session(context.default_session).unwrap().label(),
        Some("work")
    );

    ok(dispatch(
        &mut tree,
        &api,
        &context,
        &request("session.unset", vec![("label", json!(""))]),
    ));
    assert_eq!(tree.session(context.default_session).unwrap().label(), None);
}

#[test]
fn session_show_with_an_unknown_target_is_no_such_target() {
    let (mut tree, context, api) = fresh();
    let unknown = SessionId::from_raw(999);

    let error = err(dispatch(
        &mut tree,
        &api,
        &context,
        &request("session.show", vec![("target", json!(unknown.to_string()))]),
    ));

    assert_eq!(error.code, ErrorCode::NoSuchTarget);
}

// --- tab ---------------------------------------------------------------------

#[test]
fn tab_new_makes_the_new_tab_current_by_default() {
    let (mut tree, context, api) = fresh();

    let value = ok(dispatch(
        &mut tree,
        &api,
        &context,
        &request("tab.new", vec![]),
    ));

    let id: TabId = value["id"].as_str().unwrap().parse().unwrap();
    assert_eq!(
        tree.session(context.default_session).unwrap().current_tab(),
        id
    );
}

#[test]
fn tab_new_with_no_focus_restores_the_previous_current_tab() {
    let (mut tree, context, api) = fresh();
    let before = tree.session(context.default_session).unwrap().clone();

    let value = ok(dispatch(
        &mut tree,
        &api,
        &context,
        &request("tab.new", vec![("focus", json!(false))]),
    ));

    let new_tab: TabId = value["id"].as_str().unwrap().parse().unwrap();
    let after = tree.session(context.default_session).unwrap();
    assert_ne!(new_tab, before.current_tab());
    // The whole session, not only `current_tab`: its label and colour are
    // untouched, exactly one tab was added, and every tab that already
    // existed is byte-for-byte unchanged (label, panes, layout, fill,
    // size) - nothing but the one new tab moved.
    assert_eq!(after.label(), before.label());
    assert_eq!(after.colour(), before.colour());
    assert_eq!(after.current_tab(), before.current_tab());
    assert_eq!(after.tabs().len(), before.tabs().len() + 1);
    assert_eq!(after.tabs()[..before.tabs().len()], before.tabs()[..]);
    assert!(after.tabs().iter().any(|tab| tab.id() == new_tab));
}

#[test]
fn tab_move_with_position_reorders() {
    let (mut tree, context, api) = fresh();
    let second_tab = tree.new_tab(context.default_session, None).unwrap();

    ok(dispatch(
        &mut tree,
        &api,
        &context,
        &request(
            "tab.move",
            vec![
                ("target", json!(second_tab.to_string())),
                ("position", json!(1)),
            ],
        ),
    ));

    assert_eq!(
        tree.session(context.default_session).unwrap().tabs()[0].id(),
        second_tab
    );
}

#[test]
fn tab_close_removes_it() {
    let (mut tree, context, api) = fresh();
    let second_tab = tree.new_tab(context.default_session, None).unwrap();

    ok(dispatch(
        &mut tree,
        &api,
        &context,
        &request("tab.close", vec![("target", json!(second_tab.to_string()))]),
    ));

    assert!(
        tree.session(context.default_session)
            .unwrap()
            .tab(second_tab)
            .is_none()
    );
}

#[test]
fn tab_focus_makes_it_current() {
    let (mut tree, context, api) = fresh();
    // `new_tab` just made the new tab current (R115); focus back onto the
    // default tab.
    tree.new_tab(context.default_session, None).unwrap();

    ok(dispatch(
        &mut tree,
        &api,
        &context,
        &request(
            "tab.focus",
            vec![("target", json!(context.default_tab.to_string()))],
        ),
    ));

    assert_eq!(
        tree.session(context.default_session).unwrap().current_tab(),
        context.default_tab
    );
}

// --- pane --------------------------------------------------------------------

#[test]
fn pane_split_makes_the_new_pane_current_by_default() {
    let (mut tree, context, api) = fresh();

    let value = ok(dispatch(
        &mut tree,
        &api,
        &context,
        &request("pane.split", vec![("direction", json!("right"))]),
    ));

    let new_pane: steg_sdk::PaneId = value["id"].as_str().unwrap().parse().unwrap();
    assert_eq!(
        tree.session(context.default_session)
            .unwrap()
            .tab(context.default_tab)
            .unwrap()
            .current(),
        new_pane
    );
}

#[test]
fn pane_split_with_no_focus_restores_the_previous_current_pane() {
    let (mut tree, context, api) = fresh();
    let before = tree
        .session(context.default_session)
        .unwrap()
        .tab(context.default_tab)
        .unwrap()
        .clone();

    let value = ok(dispatch(
        &mut tree,
        &api,
        &context,
        &request(
            "pane.split",
            vec![("direction", json!("right")), ("focus", json!(false))],
        ),
    ));

    let new_pane: steg_sdk::PaneId = value["id"].as_str().unwrap().parse().unwrap();
    let after = tree
        .session(context.default_session)
        .unwrap()
        .tab(context.default_tab)
        .unwrap();
    assert_ne!(new_pane, before.current());
    // The whole tab, not only `current`: its label and fill are
    // untouched, exactly one pane was added, and the pane that already
    // existed is byte-for-byte unchanged (id, label, kind) - nothing but
    // the one new pane moved.
    assert_eq!(after.label(), before.label());
    assert_eq!(after.fill(), before.fill());
    assert_eq!(after.current(), before.current());
    assert_eq!(after.panes().len(), before.panes().len() + 1);
    assert_eq!(after.panes()[..before.panes().len()], before.panes()[..]);
    assert!(after.panes().iter().any(|pane| pane.id() == new_pane));
}

#[test]
fn pane_split_with_an_invalid_label_leaves_the_tree_exactly_as_it_was() {
    let (mut tree, context, api) = fresh();
    let before = tree
        .session(context.default_session)
        .unwrap()
        .tab(context.default_tab)
        .unwrap()
        .clone();

    let error = err(dispatch(
        &mut tree,
        &api,
        &context,
        &request(
            "pane.split",
            vec![("direction", json!("right")), ("label", json!("a/b"))],
        ),
    ));

    assert_eq!(error.code, ErrorCode::BadRequest);
    let after = tree
        .session(context.default_session)
        .unwrap()
        .tab(context.default_tab)
        .unwrap();
    assert_eq!(
        after, &before,
        "a rejected label must not leave the split half-done"
    );
}

#[test]
fn pane_split_with_a_program_refuses_and_creates_nothing() {
    let (mut tree, context, api) = fresh();
    let before = tree
        .session(context.default_session)
        .unwrap()
        .tab(context.default_tab)
        .unwrap()
        .panes()
        .len();

    let error = err(dispatch(
        &mut tree,
        &api,
        &context,
        &request(
            "pane.split",
            vec![("direction", json!("right")), ("program", json!(["htop"]))],
        ),
    ));

    assert_eq!(error.code, ErrorCode::NotImplementedYet);
    assert_eq!(
        tree.session(context.default_session)
            .unwrap()
            .tab(context.default_tab)
            .unwrap()
            .panes()
            .len(),
        before
    );
}

#[test]
fn pane_close_removes_the_pane() {
    let (mut tree, context, api) = fresh();
    let second = tree
        .split_pane(
            context.default_pane,
            steg_sdk::Direction::Right,
            steg_sdk::PaneKind::Terminal,
        )
        .unwrap();

    ok(dispatch(
        &mut tree,
        &api,
        &context,
        &request("pane.close", vec![("target", json!(second.to_string()))]),
    ));

    assert_eq!(
        tree.session(context.default_session)
            .unwrap()
            .tab(context.default_tab)
            .unwrap()
            .panes()
            .len(),
        1
    );
}

#[test]
fn pane_move_moves_it_to_another_tab() {
    let (mut tree, context, api) = fresh();
    let second = tree
        .split_pane(
            context.default_pane,
            steg_sdk::Direction::Right,
            steg_sdk::PaneKind::Terminal,
        )
        .unwrap();
    let second_tab = tree.new_tab(context.default_session, None).unwrap();

    ok(dispatch(
        &mut tree,
        &api,
        &context,
        &request(
            "pane.move",
            vec![
                ("target", json!(second.to_string())),
                ("to", json!(second_tab.to_string())),
            ],
        ),
    ));

    let target_tab = tree
        .session(context.default_session)
        .unwrap()
        .tab(second_tab)
        .unwrap();
    assert!(target_tab.pane(second).is_some());
}

#[test]
fn pane_set_zoomed_fills_and_unset_zoomed_unfills() {
    let (mut tree, context, api) = fresh();

    ok(dispatch(
        &mut tree,
        &api,
        &context,
        &request("pane.set", vec![("zoomed", json!(true))]),
    ));
    assert_eq!(
        tree.session(context.default_session)
            .unwrap()
            .tab(context.default_tab)
            .unwrap()
            .fill(),
        Some(context.default_pane)
    );

    ok(dispatch(
        &mut tree,
        &api,
        &context,
        &request("pane.unset", vec![("zoomed", json!(false))]),
    ));
    assert_eq!(
        tree.session(context.default_session)
            .unwrap()
            .tab(context.default_tab)
            .unwrap()
            .fill(),
        None
    );
}

#[test]
fn pane_focus_moves_to_the_neighbour() {
    let (mut tree, context, api) = fresh();
    let second = tree
        .split_pane(
            context.default_pane,
            steg_sdk::Direction::Right,
            steg_sdk::PaneKind::Terminal,
        )
        .unwrap();
    tree.focus_pane(context.default_pane).unwrap();

    ok(dispatch(
        &mut tree,
        &api,
        &context,
        &request(
            "pane.focus",
            vec![
                ("target", json!(context.default_pane.to_string())),
                ("direction", json!("right")),
            ],
        ),
    ));

    assert_eq!(
        tree.session(context.default_session)
            .unwrap()
            .tab(context.default_tab)
            .unwrap()
            .current(),
        second
    );
}

#[test]
fn pane_swap_exchanges_two_panes() {
    let (mut tree, context, api) = fresh();
    let second = tree
        .split_pane(
            context.default_pane,
            steg_sdk::Direction::Right,
            steg_sdk::PaneKind::Terminal,
        )
        .unwrap();

    ok(dispatch(
        &mut tree,
        &api,
        &context,
        &request(
            "pane.swap",
            vec![
                ("target", json!(context.default_pane.to_string())),
                ("with", json!(second.to_string())),
            ],
        ),
    ));

    let panes = tree
        .session(context.default_session)
        .unwrap()
        .tab(context.default_tab)
        .unwrap()
        .panes()
        .iter()
        .map(steg_kernel::Pane::id)
        .collect::<Vec<_>>();
    assert_eq!(panes, vec![second, context.default_pane]);
}

// --- api ----------------------------------------------------------------

#[test]
fn api_list_returns_every_method() {
    let (mut tree, context, api) = fresh();

    let value = ok(dispatch(
        &mut tree,
        &api,
        &context,
        &request("api.list", vec![]),
    ));

    assert_eq!(value.as_array().unwrap().len(), api.methods.len());
}

#[test]
fn api_show_with_a_target_returns_one_method() {
    let (mut tree, context, api) = fresh();

    let value = ok(dispatch(
        &mut tree,
        &api,
        &context,
        &request("api.show", vec![("target", json!("pane.split"))]),
    ));

    assert_eq!(value["name"]["verb"], "split");
}

#[test]
fn api_show_with_no_target_returns_the_whole_description() {
    let (mut tree, context, api) = fresh();

    let value = ok(dispatch(
        &mut tree,
        &api,
        &context,
        &request("api.show", vec![]),
    ));

    assert_eq!(
        value["methods"].as_array().unwrap().len(),
        api.methods.len()
    );
    assert!(value.get("events").is_some());
    assert!(value.get("errors").is_some());
}

#[test]
fn api_show_an_unknown_method_is_no_such_target() {
    let (mut tree, context, api) = fresh();

    let error = err(dispatch(
        &mut tree,
        &api,
        &context,
        &request("api.show", vec![("target", json!("pane.nonexistent"))]),
    ));

    assert_eq!(error.code, ErrorCode::NoSuchTarget);
}

#[test]
fn api_call_with_no_target_is_a_bad_request() {
    let (mut tree, context, api) = fresh();

    let error = err(dispatch(
        &mut tree,
        &api,
        &context,
        &request("api.call", vec![]),
    ));

    assert_eq!(error.code, ErrorCode::BadRequest);
}

#[test]
fn api_call_refuses_to_call_itself() {
    let (mut tree, context, api) = fresh();

    let error = err(dispatch(
        &mut tree,
        &api,
        &context,
        &request("api.call", vec![("target", json!("api.call"))]),
    ));

    assert_eq!(error.code, ErrorCode::BadRequest);
}

#[test]
fn api_call_of_pane_split_gives_the_same_response_a_direct_call_would() {
    // Two separately built, but identical, trees - `fresh`'s ids are
    // deterministic from `Tree::new()`, so the same request against each
    // must mint the same new pane id.
    let (mut direct_tree, direct_context, direct_api) = fresh();
    let (mut via_call_tree, via_call_context, via_call_api) = fresh();

    let direct = dispatch(
        &mut direct_tree,
        &direct_api,
        &direct_context,
        &request("pane.split", vec![("direction", json!("right"))]),
    );
    let via_call = dispatch(
        &mut via_call_tree,
        &via_call_api,
        &via_call_context,
        &request(
            "api.call",
            vec![
                ("target", json!("pane.split")),
                ("direction", json!("right")),
            ],
        ),
    );

    match (direct, via_call) {
        (Response::Ok(a), Response::Ok(b)) => assert_eq!(a, b),
        other => panic!("expected both calls to succeed: {other:?}"),
    }
}

#[test]
fn api_call_of_an_unknown_method_is_refused_the_same_way_a_direct_call_would_be() {
    let (mut tree, context, api) = fresh();

    let error = err(dispatch(
        &mut tree,
        &api,
        &context,
        &request("api.call", vec![("target", json!("pane.nonexistent"))]),
    ));

    assert_eq!(error.code, ErrorCode::BadRequest);
}

// --- shapes: Nothing/Identifier (AC #3) ------------------------------------

#[test]
fn every_served_nothing_returning_method_answers_exactly_null() {
    let (mut tree, context, api) = fresh();
    let second_pane = tree
        .split_pane(
            context.default_pane,
            steg_sdk::Direction::Right,
            steg_sdk::PaneKind::Terminal,
        )
        .unwrap();

    for (method, extra) in [
        ("session.set", vec![("label", json!("x"))]),
        ("tab.set", vec![("label", json!("x"))]),
        (
            "pane.swap",
            vec![
                ("target", json!(context.default_pane.to_string())),
                ("with", json!(second_pane.to_string())),
            ],
        ),
    ] {
        let value = ok(dispatch(&mut tree, &api, &context, &request(method, extra)));
        assert_eq!(value, Value::Null, "{method}");
    }
}

#[test]
fn every_served_identifier_returning_method_answers_exactly_one_id_field() {
    let (mut tree, context, api) = fresh();

    let value = ok(dispatch(
        &mut tree,
        &api,
        &context,
        &request("session.new", vec![]),
    ));
    let object = value.as_object().unwrap();
    assert_eq!(object.keys().collect::<Vec<_>>(), vec!["id"]);
    assert!(object["id"].is_string());
}

// --- end-to-end scenarios (AC #6) ------------------------------------------

#[test]
fn split_set_label_then_close_the_session() {
    let (mut tree, context, api) = fresh();

    let split_value = ok(dispatch(
        &mut tree,
        &api,
        &context,
        &request("pane.split", vec![("direction", json!("right"))]),
    ));
    let new_pane = split_value["id"].as_str().unwrap().to_string();

    ok(dispatch(
        &mut tree,
        &api,
        &context,
        &request(
            "pane.set",
            vec![("target", json!(new_pane)), ("label", json!("logs"))],
        ),
    ));
    let pane_id: steg_sdk::PaneId = new_pane.parse().unwrap();
    assert_eq!(
        tree.session(context.default_session)
            .unwrap()
            .tab(context.default_tab)
            .unwrap()
            .pane(pane_id)
            .unwrap()
            .label(),
        Some("logs")
    );

    ok(dispatch(
        &mut tree,
        &api,
        &context,
        &request("session.close", vec![]),
    ));
    assert!(tree.session(context.default_session).is_none());
}

#[test]
fn tab_new_with_no_focus_leaves_the_original_tab_current() {
    let (mut tree, context, api) = fresh();
    let original = context.default_tab;

    ok(dispatch(
        &mut tree,
        &api,
        &context,
        &request("tab.new", vec![("focus", json!(false))]),
    ));

    assert_eq!(
        tree.session(context.default_session).unwrap().current_tab(),
        original
    );
}
