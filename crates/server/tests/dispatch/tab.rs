use serde_json::{Value, json};
use steg_kernel::{Settings, Tree};
use steg_sdk::{ErrorCode, SessionId, TabId};
use steg_server::dispatch;

use crate::fixtures::{err, fresh, moment, ok, request};

#[test]
fn tab_new_makes_the_new_tab_current_by_default() {
    let (mut tree, context, api) = fresh();

    let value = ok(dispatch(
        &mut tree,
        &mut Settings::new(),
        &api,
        &context,
        &request("tab.new", vec![]),
        moment(0),
    ));

    let id: TabId = value["id"].as_str().unwrap().parse().unwrap();
    assert_eq!(
        tree.session(context.default_session.unwrap())
            .unwrap()
            .current_tab(),
        id
    );
}

#[test]
fn tab_new_with_no_focus_restores_the_previous_current_tab() {
    let (mut tree, context, api) = fresh();
    let before = tree
        .session(context.default_session.unwrap())
        .unwrap()
        .clone();

    let value = ok(dispatch(
        &mut tree,
        &mut Settings::new(),
        &api,
        &context,
        &request("tab.new", vec![("focus", json!(false))]),
        moment(0),
    ));

    let new_tab: TabId = value["id"].as_str().unwrap().parse().unwrap();
    let after = tree.session(context.default_session.unwrap()).unwrap();
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
    let second_tab = tree
        .new_tab(context.default_session.unwrap(), None)
        .unwrap();
    let _third_tab = tree
        .new_tab(context.default_session.unwrap(), None)
        .unwrap();
    // `new_tab` just made `_third_tab` current (R115); focus the middle
    // one instead, so the session's current tab is neither its first nor
    // its last tab before the call below.
    tree.focus_tab(second_tab).unwrap();
    let before = tree
        .session(context.default_session.unwrap())
        .unwrap()
        .clone();
    assert_eq!(before.current_tab(), second_tab);
    assert_ne!(before.tabs()[0].id(), second_tab);
    assert_ne!(before.tabs().last().unwrap().id(), second_tab);

    let value = ok(dispatch(
        &mut tree,
        &mut Settings::new(),
        &api,
        &context,
        &request("tab.new", vec![("focus", json!(false))]),
        moment(0),
    ));

    let new_tab: TabId = value["id"].as_str().unwrap().parse().unwrap();
    let after = tree.session(context.default_session.unwrap()).unwrap();
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
        moment(0),
    ));
    let new_tab: TabId = value["id"].as_str().unwrap().parse().unwrap();

    let tabs = tree
        .session(context.default_session.unwrap())
        .unwrap()
        .tabs();
    assert_eq!(tabs[0].id(), new_tab);
    assert_eq!(tabs[1].id(), context.default_tab.unwrap());
}

#[test]
fn tab_new_with_a_middle_position_places_the_new_tab_there() {
    let (mut tree, context, api) = fresh();
    let second_tab = tree
        .new_tab(context.default_session.unwrap(), None)
        .unwrap();
    let third_tab = tree
        .new_tab(context.default_session.unwrap(), None)
        .unwrap();
    // Order so far: [default_tab, second_tab, third_tab].

    let value = ok(dispatch(
        &mut tree,
        &mut Settings::new(),
        &api,
        &context,
        &request("tab.new", vec![("position", json!(2))]),
        moment(0),
    ));
    let new_tab: TabId = value["id"].as_str().unwrap().parse().unwrap();

    let tabs: Vec<TabId> = tree
        .session(context.default_session.unwrap())
        .unwrap()
        .tabs()
        .iter()
        .map(steg_kernel::Tab::id)
        .collect();
    assert_eq!(
        tabs,
        vec![context.default_tab.unwrap(), new_tab, second_tab, third_tab]
    );
}

#[test]
fn tab_new_with_no_position_appends_at_the_end_as_before() {
    let (mut tree, context, api) = fresh();
    let second_tab = tree
        .new_tab(context.default_session.unwrap(), None)
        .unwrap();

    let value = ok(dispatch(
        &mut tree,
        &mut Settings::new(),
        &api,
        &context,
        &request("tab.new", vec![]),
        moment(0),
    ));
    let new_tab: TabId = value["id"].as_str().unwrap().parse().unwrap();

    let tabs: Vec<TabId> = tree
        .session(context.default_session.unwrap())
        .unwrap()
        .tabs()
        .iter()
        .map(steg_kernel::Tab::id)
        .collect();
    assert_eq!(
        tabs,
        vec![context.default_tab.unwrap(), second_tab, new_tab]
    );
}

#[test]
fn tab_new_with_position_zero_is_refused_and_creates_nothing() {
    let (mut tree, context, api) = fresh();
    let before = tree
        .session(context.default_session.unwrap())
        .unwrap()
        .clone();

    let error = err(dispatch(
        &mut tree,
        &mut Settings::new(),
        &api,
        &context,
        &request("tab.new", vec![("position", json!(0))]),
        moment(0),
    ));

    assert_eq!(error.code, ErrorCode::BadRequest);
    assert_eq!(
        &before,
        tree.session(context.default_session.unwrap()).unwrap()
    );
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
        .session(new_context.default_session.unwrap())
        .unwrap()
        .clone();

    let new_error = err(dispatch(
        &mut new_tree,
        &mut Settings::new(),
        &api,
        &new_context,
        &request("tab.new", vec![("position", json!(99))]),
        moment(0),
    ));

    assert_eq!(new_error.code, ErrorCode::PositionOutOfRange);
    assert_eq!(
        &before,
        new_tree
            .session(new_context.default_session.unwrap())
            .unwrap()
    );

    let (mut move_tree, move_context, _) = fresh();
    let second_tab = move_tree
        .new_tab(move_context.default_session.unwrap(), None)
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
        moment(0),
    ));

    assert_eq!(new_error.code, move_error.code);
}

#[test]
fn tab_new_with_a_position_past_the_end_and_no_focus_creates_nothing_and_restores_focus() {
    let (mut tree, context, api) = fresh();
    let before = tree
        .session(context.default_session.unwrap())
        .unwrap()
        .clone();

    let error = err(dispatch(
        &mut tree,
        &mut Settings::new(),
        &api,
        &context,
        &request(
            "tab.new",
            vec![("position", json!(99)), ("focus", json!(false))],
        ),
        moment(0),
    ));

    assert_eq!(error.code, ErrorCode::PositionOutOfRange);
    assert_eq!(
        &before,
        tree.session(context.default_session.unwrap()).unwrap()
    );
}

#[test]
fn tab_move_with_position_reorders() {
    let (mut tree, context, api) = fresh();
    let second_tab = tree
        .new_tab(context.default_session.unwrap(), None)
        .unwrap();

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
        moment(0),
    ));

    assert_eq!(
        tree.session(context.default_session.unwrap())
            .unwrap()
            .tabs()[0]
            .id(),
        second_tab
    );
}

#[test]
fn tab_close_removes_it() {
    let (mut tree, context, api) = fresh();
    let second_tab = tree
        .new_tab(context.default_session.unwrap(), None)
        .unwrap();

    ok(dispatch(
        &mut tree,
        &mut Settings::new(),
        &api,
        &context,
        &request("tab.close", vec![("target", json!(second_tab.to_string()))]),
        moment(0),
    ));

    assert!(
        tree.session(context.default_session.unwrap())
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
    tree.new_tab(context.default_session.unwrap(), None)
        .unwrap();

    ok(dispatch(
        &mut tree,
        &mut Settings::new(),
        &api,
        &context,
        &request(
            "tab.focus",
            vec![("target", json!(context.default_tab.unwrap().to_string()))],
        ),
        moment(0),
    ));

    assert_eq!(
        tree.session(context.default_session.unwrap())
            .unwrap()
            .current_tab(),
        context.default_tab.unwrap()
    );
}

// --- tab.list / tab.show (AC #1; TASK-1.89) ----------------------------------

#[test]
fn tab_show_returns_the_tabs_record() {
    let (mut tree, context, api) = fresh();

    let value = ok(dispatch(
        &mut tree,
        &mut Settings::new(),
        &api,
        &context,
        &request(
            "tab.show",
            vec![("target", json!(context.default_tab.unwrap().to_string()))],
        ),
        moment(0),
    ));

    assert_eq!(value["id"], json!(context.default_tab.unwrap().to_string()));
    assert_eq!(value["position"], json!(1));
    assert_eq!(
        value["current_pane"],
        json!(context.default_pane.unwrap().to_string())
    );
    assert_eq!(value["title"], Value::Null);
    assert_eq!(value["shown_status"], Value::Null);
}

#[test]
fn tab_show_with_an_unknown_target_is_no_such_target() {
    let (mut tree, context, api) = fresh();
    let unknown = steg_sdk::TabId::from_raw(999);

    let error = err(dispatch(
        &mut tree,
        &mut Settings::new(),
        &api,
        &context,
        &request("tab.show", vec![("target", json!(unknown.to_string()))]),
        moment(0),
    ));

    assert_eq!(error.code, ErrorCode::NoSuchTarget);
}

/// Mechanism: `tab.list`'s order is the session's own tab order (R10),
/// not creation order or any other incidental order - two tabs, read
/// back, must come in the order the session holds them.
#[test]
fn tab_list_returns_every_tab_of_the_session_in_order() {
    let (mut tree, context, api) = fresh();
    let session_id = context.default_session.unwrap();
    let second_tab = tree.new_tab(session_id, None).unwrap();

    let value = ok(dispatch(
        &mut tree,
        &mut Settings::new(),
        &api,
        &context,
        &request("tab.list", vec![("target", json!(session_id.to_string()))]),
        moment(0),
    ));

    let records = value.as_array().unwrap();
    assert_eq!(records.len(), 2);
    assert_eq!(
        records[0]["id"],
        json!(context.default_tab.unwrap().to_string())
    );
    assert_eq!(records[0]["position"], json!(1));
    assert_eq!(records[1]["id"], json!(second_tab.to_string()));
    assert_eq!(records[1]["position"], json!(2));
}

#[test]
fn tab_list_with_an_unknown_session_is_no_such_target() {
    let (mut tree, context, api) = fresh();
    let unknown = SessionId::from_raw(999);

    let error = err(dispatch(
        &mut tree,
        &mut Settings::new(),
        &api,
        &context,
        &request("tab.list", vec![("target", json!(unknown.to_string()))]),
        moment(0),
    ));

    assert_eq!(error.code, ErrorCode::NoSuchTarget);
}

/// doc-15 R10: a target that matches more than one thing is an error,
/// never a guess - `tab.list`'s own target is a session, so this needs
/// two *sessions* sharing a label (labels are never required to be
/// unique, `crates/kernel/src/label.rs`'s own doc).
#[test]
fn tab_list_with_an_ambiguous_session_target_is_ambiguous_target() {
    let (mut tree, context, api) = fresh();
    tree.session_mut(context.default_session.unwrap())
        .unwrap()
        .set_label(Some("work".to_string()))
        .unwrap();
    tree.new_session(Some("work".to_string())).unwrap();

    let error = err(dispatch(
        &mut tree,
        &mut Settings::new(),
        &api,
        &context,
        &request("tab.list", vec![("target", json!("work"))]),
        moment(0),
    ));

    assert_eq!(error.code, ErrorCode::AmbiguousTarget);
}

/// The same mechanism as the test above, for `tab.show`'s own target (a
/// tab): two tabs of the *same* session sharing a label.
#[test]
fn tab_show_with_an_ambiguous_target_is_ambiguous_target() {
    let (mut tree, context, api) = fresh();
    let session_id = context.default_session.unwrap();
    tab_mut_for_test(&mut tree, context.default_tab.unwrap())
        .set_label(Some("logs".to_string()))
        .unwrap();
    tree.new_tab(session_id, Some("logs".to_string())).unwrap();

    let error = err(dispatch(
        &mut tree,
        &mut Settings::new(),
        &api,
        &context,
        &request("tab.show", vec![("target", json!("logs"))]),
        moment(0),
    ));

    assert_eq!(error.code, ErrorCode::AmbiguousTarget);
}

/// The same mechanism again, for `pane.list`'s own target (also a tab).
/// Tests `pane.list`, not a `tab.*` method - kept here, not in
/// `dispatch::pane`, because it shares `tab_mut_for_test` and the local
/// stand-in for the private `session_owning_tab` lookup with the two
/// ambiguous-target tests directly above it; moving it would force that
/// helper into `fixtures.rs` for this one caller alone.
#[test]
fn pane_list_with_an_ambiguous_target_is_ambiguous_target() {
    let (mut tree, context, api) = fresh();
    let session_id = context.default_session.unwrap();
    tab_mut_for_test(&mut tree, context.default_tab.unwrap())
        .set_label(Some("logs".to_string()))
        .unwrap();
    tree.new_tab(session_id, Some("logs".to_string())).unwrap();

    let error = err(dispatch(
        &mut tree,
        &mut Settings::new(),
        &api,
        &context,
        &request("pane.list", vec![("target", json!("logs"))]),
        moment(0),
    ));

    assert_eq!(error.code, ErrorCode::AmbiguousTarget);
}

/// A tab's owning session, by walking every session (the same shape
/// `crates/server/src/dispatch/lookup.rs`'s own private
/// `session_owning_tab` uses internally) - this file has no direct
/// access to that private helper, so the three ambiguous-target tests
/// above build their own, minimal stand-in rather than reaching for it.
fn tab_mut_for_test(tree: &mut Tree, tab: TabId) -> &mut steg_kernel::Tab {
    let session_id = tree
        .sessions()
        .iter()
        .find(|session| session.tab(tab).is_some())
        .map(steg_kernel::Session::id)
        .expect("the tab given to this test helper is in the tree");
    tree.session_mut(session_id).unwrap().tab_mut(tab).unwrap()
}

/// Mechanism: `shown_status` comes from the kernel's own ranking
/// (decision-7: waiting, failed, bell, finished, running), not from
/// whichever pane happens to come first in the tab's own order. Built so
/// the higher-ranked pane (`Failed`) is placed *second*, after the
/// lower-ranked one (`Running`) - "return the first pane" would pick the
/// wrong one here.
#[test]
fn tab_show_and_tab_list_shown_status_follow_the_kernels_ranking_not_tab_order() {
    let (mut tree, context, api) = fresh();
    let tab_id = context.default_tab.unwrap();
    let first = context.default_pane.unwrap();
    let second = tree
        .split_pane(
            first,
            steg_sdk::Direction::Right,
            steg_sdk::PaneKind::Terminal,
        )
        .unwrap();

    // `first` (R112's first pane) is merely Running; `second` (R112's
    // second, so not simply "whichever is first") outranks it by
    // actually failing - the kernel's own `rank()` must be what decides,
    // not position.
    tree.start_program(
        first,
        steg_sdk::Program::Command {
            argv: vec!["htop".to_string()],
            cwd: "/tmp".to_string(),
        },
    )
    .unwrap();
    tree.start_program(
        second,
        steg_sdk::Program::Command {
            argv: vec!["htop".to_string()],
            cwd: "/tmp".to_string(),
        },
    )
    .unwrap();
    tree.record_exit(second, 3, moment(0)).unwrap();

    let shown = ok(dispatch(
        &mut tree,
        &mut Settings::new(),
        &api,
        &context,
        &request("tab.show", vec![("target", json!(tab_id.to_string()))]),
        moment(0),
    ));
    assert_eq!(shown["shown_status"], json!(second.to_string()));

    let listed = ok(dispatch(
        &mut tree,
        &mut Settings::new(),
        &api,
        &context,
        &request(
            "tab.list",
            vec![(
                "target",
                json!(context.default_session.unwrap().to_string()),
            )],
        ),
        moment(0),
    ));
    assert_eq!(listed[0]["shown_status"], json!(second.to_string()));
}
