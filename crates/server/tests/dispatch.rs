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
use steg_kernel::{Settings, Tree};
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
        let _ = dispatch(
            &mut tree,
            &mut Settings::new(),
            &api,
            &context,
            &request(&dotted, params),
        );
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
        ("pane.list", vec![], "pane.show"),
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
    ];

    for (method, extra, expected_substring) in cases {
        let (mut tree, context, api) = fresh();
        let before = tree.sessions().to_vec();

        let error = err(dispatch(
            &mut tree,
            &mut Settings::new(),
            &api,
            &context,
            &request(method, extra),
        ));

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
        &mut Settings::new(),
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
        &mut Settings::new(),
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
        &mut Settings::new(),
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
        &mut Settings::new(),
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
    ));
    assert_eq!(
        tree.session(context.default_session).unwrap().label(),
        Some("work")
    );

    ok(dispatch(
        &mut tree,
        &mut Settings::new(),
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
        &mut Settings::new(),
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
        &mut Settings::new(),
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
        &mut Settings::new(),
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

/// The easy case above has only one tab, so its current tab is trivially
/// both the first and the one restored. Here the session's current tab
/// is neither first nor last before the call, which is what actually
/// exercises `tab::new` reading `session.current_tab()` rather than
/// assuming the first tab.
#[test]
fn tab_new_with_no_focus_restores_a_current_tab_that_is_not_the_first() {
    let (mut tree, context, api) = fresh();
    let second_tab = tree.new_tab(context.default_session, None).unwrap();
    let _third_tab = tree.new_tab(context.default_session, None).unwrap();
    // `new_tab` just made `_third_tab` current (R115); focus the middle
    // one instead, so the session's current tab is neither its first nor
    // its last tab before the call below.
    tree.focus_tab(second_tab).unwrap();
    let before = tree.session(context.default_session).unwrap().clone();
    assert_eq!(before.current_tab(), second_tab);
    assert_ne!(before.tabs()[0].id(), second_tab);
    assert_ne!(before.tabs().last().unwrap().id(), second_tab);

    let value = ok(dispatch(
        &mut tree,
        &mut Settings::new(),
        &api,
        &context,
        &request("tab.new", vec![("focus", json!(false))]),
    ));

    let new_tab: TabId = value["id"].as_str().unwrap().parse().unwrap();
    let after = tree.session(context.default_session).unwrap();
    assert_eq!(after.current_tab(), second_tab);
    assert_eq!(after.tabs().len(), before.tabs().len() + 1);
    assert_eq!(after.tabs()[..before.tabs().len()], before.tabs()[..]);
    assert!(after.tabs().iter().any(|tab| tab.id() == new_tab));
}

#[test]
fn tab_new_with_position_one_places_the_new_tab_first() {
    let (mut tree, context, api) = fresh();

    let value = ok(dispatch(
        &mut tree,
        &mut Settings::new(),
        &api,
        &context,
        &request("tab.new", vec![("position", json!(1))]),
    ));
    let new_tab: TabId = value["id"].as_str().unwrap().parse().unwrap();

    let tabs = tree.session(context.default_session).unwrap().tabs();
    assert_eq!(tabs[0].id(), new_tab);
    assert_eq!(tabs[1].id(), context.default_tab);
}

#[test]
fn tab_new_with_a_middle_position_places_the_new_tab_there() {
    let (mut tree, context, api) = fresh();
    let second_tab = tree.new_tab(context.default_session, None).unwrap();
    let third_tab = tree.new_tab(context.default_session, None).unwrap();
    // Order so far: [default_tab, second_tab, third_tab].

    let value = ok(dispatch(
        &mut tree,
        &mut Settings::new(),
        &api,
        &context,
        &request("tab.new", vec![("position", json!(2))]),
    ));
    let new_tab: TabId = value["id"].as_str().unwrap().parse().unwrap();

    let tabs: Vec<TabId> = tree
        .session(context.default_session)
        .unwrap()
        .tabs()
        .iter()
        .map(steg_kernel::Tab::id)
        .collect();
    assert_eq!(
        tabs,
        vec![context.default_tab, new_tab, second_tab, third_tab]
    );
}

#[test]
fn tab_new_with_no_position_appends_at_the_end_as_before() {
    let (mut tree, context, api) = fresh();
    let second_tab = tree.new_tab(context.default_session, None).unwrap();

    let value = ok(dispatch(
        &mut tree,
        &mut Settings::new(),
        &api,
        &context,
        &request("tab.new", vec![]),
    ));
    let new_tab: TabId = value["id"].as_str().unwrap().parse().unwrap();

    let tabs: Vec<TabId> = tree
        .session(context.default_session)
        .unwrap()
        .tabs()
        .iter()
        .map(steg_kernel::Tab::id)
        .collect();
    assert_eq!(tabs, vec![context.default_tab, second_tab, new_tab]);
}

#[test]
fn tab_new_with_position_zero_is_refused_and_creates_nothing() {
    let (mut tree, context, api) = fresh();
    let before = tree.session(context.default_session).unwrap().clone();

    let error = err(dispatch(
        &mut tree,
        &mut Settings::new(),
        &api,
        &context,
        &request("tab.new", vec![("position", json!(0))]),
    ));

    assert_eq!(error.code, ErrorCode::BadRequest);
    assert_eq!(&before, tree.session(context.default_session).unwrap());
}

/// A position past the end behaves exactly as `tab.move`'s own arm does
/// for the same out-of-range index (`Error::PositionOutOfRange`, mapped
/// to `ErrorCode::PositionOutOfRange`) - checked against two otherwise
/// identical trees, one for each call, so neither call's own state
/// change is in the other's way. Unlike `tab.move`, which acts on a tab
/// that already existed, `tab.new` leaves nothing behind: the tab this
/// call minted is closed again, and the session's current tab restored
/// to what it was before the call.
#[test]
fn tab_new_with_a_position_past_the_end_behaves_like_tab_move_and_creates_nothing() {
    let (mut new_tree, new_context, api) = fresh();
    let before = new_tree
        .session(new_context.default_session)
        .unwrap()
        .clone();

    let new_error = err(dispatch(
        &mut new_tree,
        &mut Settings::new(),
        &api,
        &new_context,
        &request("tab.new", vec![("position", json!(99))]),
    ));

    assert_eq!(new_error.code, ErrorCode::PositionOutOfRange);
    assert_eq!(
        &before,
        new_tree.session(new_context.default_session).unwrap()
    );

    let (mut move_tree, move_context, _) = fresh();
    let second_tab = move_tree
        .new_tab(move_context.default_session, None)
        .unwrap();
    let move_error = err(dispatch(
        &mut move_tree,
        &mut Settings::new(),
        &api,
        &move_context,
        &request(
            "tab.move",
            vec![
                ("target", json!(second_tab.to_string())),
                ("position", json!(99)),
            ],
        ),
    ));

    assert_eq!(new_error.code, move_error.code);
}

#[test]
fn tab_new_with_a_position_past_the_end_and_no_focus_creates_nothing_and_restores_focus() {
    let (mut tree, context, api) = fresh();
    let before = tree.session(context.default_session).unwrap().clone();

    let error = err(dispatch(
        &mut tree,
        &mut Settings::new(),
        &api,
        &context,
        &request(
            "tab.new",
            vec![("position", json!(99)), ("focus", json!(false))],
        ),
    ));

    assert_eq!(error.code, ErrorCode::PositionOutOfRange);
    assert_eq!(&before, tree.session(context.default_session).unwrap());
}

#[test]
fn tab_move_with_position_reorders() {
    let (mut tree, context, api) = fresh();
    let second_tab = tree.new_tab(context.default_session, None).unwrap();

    ok(dispatch(
        &mut tree,
        &mut Settings::new(),
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
        &mut Settings::new(),
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
        &mut Settings::new(),
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
        &mut Settings::new(),
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
        &mut Settings::new(),
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

/// The easy case above splits the tab's only pane, so it is trivially
/// both the split's target and the tab's current pane. Here the split's
/// target is *not* the tab's current pane, which is what actually
/// exercises `pane::split` reading the tab's own `current()` rather than
/// assuming the target it was just given.
#[test]
fn pane_split_with_no_focus_restores_a_current_pane_that_is_not_the_target() {
    let (mut tree, context, api) = fresh();
    let second_pane = tree
        .split_pane(
            context.default_pane,
            steg_sdk::Direction::Right,
            steg_sdk::PaneKind::Terminal,
        )
        .unwrap();
    // `split_pane` just made `second_pane` current (R115); the call below
    // splits `default_pane`, which is *not* current, so a correct restore
    // has to read the tab's own current pane, not assume the target.
    let before = tree
        .session(context.default_session)
        .unwrap()
        .tab(context.default_tab)
        .unwrap()
        .clone();
    assert_eq!(before.current(), second_pane);
    assert_ne!(before.current(), context.default_pane);

    let value = ok(dispatch(
        &mut tree,
        &mut Settings::new(),
        &api,
        &context,
        &request(
            "pane.split",
            vec![
                ("target", json!(context.default_pane.to_string())),
                ("direction", json!("right")),
                ("focus", json!(false)),
            ],
        ),
    ));

    let new_pane: steg_sdk::PaneId = value["id"].as_str().unwrap().parse().unwrap();
    let after = tree
        .session(context.default_session)
        .unwrap()
        .tab(context.default_tab)
        .unwrap();
    assert_eq!(after.current(), second_pane);
    assert_eq!(after.panes().len(), before.panes().len() + 1);
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
        &mut Settings::new(),
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
fn pane_split_with_a_program_mints_a_pane_but_does_not_start_it() {
    // TASK-1.76: `dispatch` stays I/O-free and does not call
    // `Tree::start_program` itself - that is `Runtime`'s own job, right
    // after this same call, under one lock (its own plan, §1/§2). Calling
    // `dispatch` directly, as this test does, is exactly the part of the
    // whole flow that mints the pane and nothing more: `program`/`cwd`
    // make no difference here at all, and the new pane's own `program()`/
    // `status()` are still `None`.
    let (mut tree, context, api) = fresh();

    let outcome = ok(dispatch(
        &mut tree,
        &mut Settings::new(),
        &api,
        &context,
        &request(
            "pane.split",
            vec![("direction", json!("right")), ("program", json!(["htop"]))],
        ),
    ));

    let new_pane: steg_sdk::PaneId = outcome["id"].as_str().unwrap().parse().unwrap();
    let pane = tree
        .session(context.default_session)
        .unwrap()
        .tab(context.default_tab)
        .unwrap()
        .pane(new_pane)
        .expect("dispatch minted this pane");
    assert_eq!(pane.program(), None);
    assert_eq!(pane.status(), None);
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
        &mut Settings::new(),
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
        &mut Settings::new(),
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
        &mut Settings::new(),
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
        &mut Settings::new(),
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
        &mut Settings::new(),
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
        &mut Settings::new(),
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
        &mut Settings::new(),
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
        &mut Settings::new(),
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
        &mut Settings::new(),
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
        &mut Settings::new(),
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
        &mut Settings::new(),
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
        &mut Settings::new(),
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
        &mut Settings::new(),
        &direct_api,
        &direct_context,
        &request("pane.split", vec![("direction", json!("right"))]),
    );
    let via_call = dispatch(
        &mut via_call_tree,
        &mut Settings::new(),
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
        &mut Settings::new(),
        &api,
        &context,
        &request("api.call", vec![("target", json!("pane.nonexistent"))]),
    ));

    assert_eq!(error.code, ErrorCode::BadRequest);
}

// --- shapes: Nothing/Identifier (AC #3) ------------------------------------

/// Every method whose `returns` is `Nothing` or `Identifier`, covered by
/// name - not a representative few - with a valid request against a
/// dedicated piece of tree state nothing else in this test touches, so
/// the order these run in cannot matter. A method of either kind absent
/// from `CASES` falls through to the `panic!` arm below, so a new one
/// added to the description cannot be missed; the closing assertion is a
/// second, independent check of the same thing, from the description's
/// own side rather than this test's.
#[test]
fn every_nothing_or_identifier_returning_method_matches_its_declared_shape() {
    let (mut tree, context, api) = fresh();

    // One dedicated id per case that creates, closes or moves something,
    // so no two cases ever compete for the same piece of state.
    let session_to_close = tree.new_session(None).unwrap();
    let session_to_move = tree.new_session(None).unwrap();
    let tab_to_close = tree.new_tab(context.default_session, None).unwrap();
    let tab_to_move = tree.new_tab(context.default_session, None).unwrap();
    let tab_to_focus = tree.new_tab(context.default_session, None).unwrap();
    let tab_for_pane_move = tree.new_tab(context.default_session, None).unwrap();
    let pane_to_close = tree
        .split_pane(
            context.default_pane,
            steg_sdk::Direction::Right,
            steg_sdk::PaneKind::Terminal,
        )
        .unwrap();
    let pane_to_move = tree
        .split_pane(
            context.default_pane,
            steg_sdk::Direction::Right,
            steg_sdk::PaneKind::Terminal,
        )
        .unwrap();
    let pane_for_swap = tree
        .split_pane(
            context.default_pane,
            steg_sdk::Direction::Right,
            steg_sdk::PaneKind::Terminal,
        )
        .unwrap();

    let not_yet: &[&str] = &[
        "session.focus",
        "pane.resize",
        "pane.send",
        "server.start",
        "server.stop",
    ];
    let mut covered: Vec<String> = Vec::new();

    for method in &api.methods {
        let dotted = method.name.dotted();
        if !matches!(
            method.returns,
            steg_sdk::Returns::Nothing | steg_sdk::Returns::Identifier { .. }
        ) {
            continue;
        }
        covered.push(dotted.clone());

        if not_yet.contains(&dotted.as_str()) {
            // Every required param filled with a minimally-valid value
            // (`pane.resize`'s `direction`/`by`), so `validate` passes
            // and the arm's own `NotImplementedYet` is what answers -
            // not a `BadRequest` for a missing required param instead.
            let params: Vec<(&str, Value)> = method
                .params
                .iter()
                .filter(|p| p.required)
                .map(|p| (p.name.as_str(), minimal_value(&p.value)))
                .collect();
            let error = err(dispatch(
                &mut tree,
                &mut Settings::new(),
                &api,
                &context,
                &request(&dotted, params),
            ));
            assert_eq!(error.code, ErrorCode::NotImplementedYet, "{dotted}");
            continue;
        }

        let extra: Vec<(&str, Value)> = match dotted.as_str() {
            "session.close" => vec![("target", json!(session_to_close.to_string()))],
            "session.move" => vec![
                ("target", json!(session_to_move.to_string())),
                ("position", json!(1)),
            ],
            "session.set" => vec![("label", json!("x"))],
            "session.unset" => vec![("label", json!(""))],
            "session.new" => vec![],
            "tab.close" => vec![("target", json!(tab_to_close.to_string()))],
            "tab.move" => vec![
                ("target", json!(tab_to_move.to_string())),
                ("position", json!(1)),
            ],
            "tab.set" => vec![("label", json!("x"))],
            "tab.unset" => vec![("label", json!(""))],
            "tab.focus" => vec![("target", json!(tab_to_focus.to_string()))],
            "tab.new" => vec![],
            "pane.close" => vec![("target", json!(pane_to_close.to_string()))],
            "pane.move" => vec![
                ("target", json!(pane_to_move.to_string())),
                ("to", json!(tab_for_pane_move.to_string())),
            ],
            "pane.set" => vec![("label", json!("x"))],
            "pane.unset" => vec![("label", json!(""))],
            "pane.focus" => vec![("target", json!(context.default_pane.to_string()))],
            "pane.swap" => vec![
                ("target", json!(context.default_pane.to_string())),
                ("with", json!(pane_for_swap.to_string())),
            ],
            "pane.split" => vec![("direction", json!("right"))],
            "setting.set" => vec![("target", json!("appearance")), ("value", json!("light"))],
            "setting.unset" => vec![("target", json!("appearance"))],
            other => panic!(
                "{other} is Nothing/Identifier-returning but this test has no case for it - \
                 add one above"
            ),
        };

        let response = dispatch(
            &mut tree,
            &mut Settings::new(),
            &api,
            &context,
            &request(&dotted, extra),
        );
        match (&method.returns, response) {
            (steg_sdk::Returns::Nothing, Response::Ok(value)) => {
                assert_eq!(value, Value::Null, "{dotted}");
            }
            (steg_sdk::Returns::Identifier { .. }, Response::Ok(value)) => {
                let object = value
                    .as_object()
                    .unwrap_or_else(|| panic!("{dotted}: an identifier result must be an object"));
                assert_eq!(object.keys().collect::<Vec<_>>(), vec!["id"], "{dotted}");
                assert!(object["id"].is_string(), "{dotted}");
            }
            (_, Response::Error(error)) => {
                panic!("{dotted} was expected to succeed, got {error:?}")
            }
            (other, Response::Ok(_)) => {
                panic!("{dotted}'s returns is neither Nothing nor Identifier: {other:?}")
            }
        }
    }

    // A second, independent check that nothing of this kind was missed:
    // from the description's own side, not this test's `match`.
    let mut expected: Vec<String> = api
        .methods
        .iter()
        .filter(|m| {
            matches!(
                m.returns,
                steg_sdk::Returns::Nothing | steg_sdk::Returns::Identifier { .. }
            )
        })
        .map(|m| m.name.dotted())
        .collect();
    expected.sort();
    covered.sort();
    assert_eq!(expected, covered);
}

// --- end-to-end scenarios (AC #6) ------------------------------------------

#[test]
fn split_set_label_then_close_the_session() {
    let (mut tree, context, api) = fresh();

    let split_value = ok(dispatch(
        &mut tree,
        &mut Settings::new(),
        &api,
        &context,
        &request("pane.split", vec![("direction", json!("right"))]),
    ));
    let new_pane = split_value["id"].as_str().unwrap().to_string();

    ok(dispatch(
        &mut tree,
        &mut Settings::new(),
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
        &mut Settings::new(),
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
        &mut Settings::new(),
        &api,
        &context,
        &request("tab.new", vec![("focus", json!(false))]),
    ));

    assert_eq!(
        tree.session(context.default_session).unwrap().current_tab(),
        original
    );
}

// --- setting (R80, decision-9; TASK-1.48) -----------------------------------

#[test]
fn setting_list_returns_every_registered_setting_with_its_current_value() {
    let (mut tree, context, api) = fresh();
    let mut settings = Settings::new();

    let value = ok(dispatch(
        &mut tree,
        &mut settings,
        &api,
        &context,
        &request("setting.list", vec![]),
    ));

    let records = value.as_array().unwrap();
    assert_eq!(
        records.len(),
        1,
        "one registered setting today (appearance)"
    );
    assert_eq!(records[0]["name"], json!("appearance"));
    assert_eq!(records[0]["value"], json!("system"));
    assert_eq!(records[0]["default"], json!("system"));
}

#[test]
fn setting_show_returns_one_setting_in_full() {
    let (mut tree, context, api) = fresh();
    let mut settings = Settings::new();

    let value = ok(dispatch(
        &mut tree,
        &mut settings,
        &api,
        &context,
        &request("setting.show", vec![("target", json!("appearance"))]),
    ));

    assert_eq!(
        value,
        json!({"name": "appearance", "value": "system", "default": "system"})
    );
}

#[test]
fn setting_show_an_unknown_name_is_no_such_target() {
    let (mut tree, context, api) = fresh();
    let mut settings = Settings::new();

    let error = err(dispatch(
        &mut tree,
        &mut settings,
        &api,
        &context,
        &request("setting.show", vec![("target", json!("nonsense"))]),
    ));

    assert_eq!(error.code, ErrorCode::NoSuchTarget);
}

/// AC #1 ("changed") and AC #2 ("applies at once"): a `set`, then an
/// independent `show` call over the *same* `Settings` value, sees the new
/// value immediately - there is no cache and no event to wait on.
#[test]
fn setting_set_changes_the_value_the_next_read_sees() {
    let (mut tree, context, api) = fresh();
    let mut settings = Settings::new();

    ok(dispatch(
        &mut tree,
        &mut settings,
        &api,
        &context,
        &request(
            "setting.set",
            vec![("target", json!("appearance")), ("value", json!("light"))],
        ),
    ));

    let value = ok(dispatch(
        &mut tree,
        &mut settings,
        &api,
        &context,
        &request("setting.show", vec![("target", json!("appearance"))]),
    ));

    assert_eq!(value["value"], json!("light"));
    assert_eq!(value["default"], json!("system"), "the default never moves");
}

#[test]
fn setting_set_rejects_an_unknown_name_with_no_such_target() {
    let (mut tree, context, api) = fresh();
    let mut settings = Settings::new();

    let error = err(dispatch(
        &mut tree,
        &mut settings,
        &api,
        &context,
        &request(
            "setting.set",
            vec![("target", json!("nonsense")), ("value", json!("anything"))],
        ),
    ));

    assert_eq!(error.code, ErrorCode::NoSuchTarget);
}

#[test]
fn setting_set_rejects_a_value_outside_the_closed_set_with_bad_request() {
    let (mut tree, context, api) = fresh();
    let mut settings = Settings::new();

    let error = err(dispatch(
        &mut tree,
        &mut settings,
        &api,
        &context,
        &request(
            "setting.set",
            vec![("target", json!("appearance")), ("value", json!("neon"))],
        ),
    ));

    assert_eq!(error.code, ErrorCode::BadRequest);

    // The rejected call changed nothing (mirrors the kernel's own
    // `a_rejected_set_changes_nothing`, proven again here through the
    // dispatch layer).
    let value = ok(dispatch(
        &mut tree,
        &mut settings,
        &api,
        &context,
        &request("setting.show", vec![("target", json!("appearance"))]),
    ));
    assert_eq!(value["value"], json!("system"));
}

#[test]
fn setting_unset_reverts_to_the_default() {
    let (mut tree, context, api) = fresh();
    let mut settings = Settings::new();

    ok(dispatch(
        &mut tree,
        &mut settings,
        &api,
        &context,
        &request(
            "setting.set",
            vec![("target", json!("appearance")), ("value", json!("dark"))],
        ),
    ));
    ok(dispatch(
        &mut tree,
        &mut settings,
        &api,
        &context,
        &request("setting.unset", vec![("target", json!("appearance"))]),
    ));

    let value = ok(dispatch(
        &mut tree,
        &mut settings,
        &api,
        &context,
        &request("setting.show", vec![("target", json!("appearance"))]),
    ));
    assert_eq!(value["value"], json!("system"));
}

#[test]
fn setting_unset_an_unknown_name_is_no_such_target() {
    let (mut tree, context, api) = fresh();
    let mut settings = Settings::new();

    let error = err(dispatch(
        &mut tree,
        &mut settings,
        &api,
        &context,
        &request("setting.unset", vec![("target", json!("nonsense"))]),
    ));

    assert_eq!(error.code, ErrorCode::NoSuchTarget);
}

/// AC #2's "still there after the app is quit and reopened," proved only
/// as far as this layer can reach with no socket or process lifecycle yet
/// (TASK-1.44/1.45 unbuilt): a fresh `Tree` and `Context` stand in for a
/// new client/window connecting, while the *same* `Settings` value stands
/// in for "the server, which outlives the app" (doc-8 R2/R3). This does
/// not show the server itself surviving a restart - that is the
/// persistence question on this task's plan, unresolved by this test.
#[test]
fn an_override_is_still_there_for_a_fresh_tree_and_context_over_the_same_settings() {
    let (mut tree, context, api) = fresh();
    let mut settings = Settings::new();

    ok(dispatch(
        &mut tree,
        &mut settings,
        &api,
        &context,
        &request(
            "setting.set",
            vec![("target", json!("appearance")), ("value", json!("dark"))],
        ),
    ));

    // A different Tree and Context, as if a new client connected - the
    // same Settings value is the only thing carried over.
    let (mut other_tree, other_context, other_api) = fresh();
    let value = ok(dispatch(
        &mut other_tree,
        &mut settings,
        &other_api,
        &other_context,
        &request("setting.show", vec![("target", json!("appearance"))]),
    ));

    assert_eq!(value["value"], json!("dark"));
}
