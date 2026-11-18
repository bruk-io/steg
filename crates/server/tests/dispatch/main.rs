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

mod cross_cutting;
mod fixtures;
mod session;

use serde_json::{Value, json};
use steg_kernel::{Settings, Tree};
use steg_sdk::{ErrorCode, Response, SessionId, TabId};
use steg_server::{Context, dispatch};

use fixtures::{err, fresh, moment, ok, request};

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

// --- pane --------------------------------------------------------------------

#[test]
fn pane_show_returns_the_panes_record() {
    let (mut tree, context, api) = fresh();
    let pane_id = context.default_pane.unwrap();
    tree.start_program(
        pane_id,
        steg_sdk::Program::Command {
            argv: vec!["htop".to_string()],
            cwd: "/tmp".to_string(),
        },
    )
    .unwrap();

    let value = ok(dispatch(
        &mut tree,
        &mut Settings::new(),
        &api,
        &context,
        &request("pane.show", vec![("target", json!(pane_id.to_string()))]),
        moment(0),
    ));

    assert_eq!(value["id"], json!(pane_id.to_string()));
    assert_eq!(value["kind"], json!("terminal"));
    assert_eq!(value["cwd"], json!("/tmp"));
    assert_eq!(value["program"], json!("htop"));
    assert_eq!(value["exit_code"], Value::Null);
    assert_eq!(value["program_status"], json!("running"));
    assert_eq!(value["status"], json!("running"));
    assert_eq!(value["title"], Value::Null);
}

/// Mechanism: `status` and `program_status` are two separate fields, each
/// read from its own place on the kernel's `Pane` - not one derived from
/// the other. A shell pane idling at its own prompt is the case that
/// proves it: `program_status` is `"running"` (the shell process really
/// is), but `status` is `None` (R97/TASK-1.83: idle, no typed command).
/// If `status` were derived from `program_status` instead of the
/// kernel's own `Pane::status`, this would read `"running"` for both.
#[test]
fn an_idle_shell_panes_program_status_and_status_disagree() {
    let (mut tree, context, api) = fresh();
    let pane_id = context.default_pane.unwrap();
    tree.start_program(
        pane_id,
        steg_sdk::Program::Shell {
            cwd: "/tmp".to_string(),
        },
    )
    .unwrap();

    let value = ok(dispatch(
        &mut tree,
        &mut Settings::new(),
        &api,
        &context,
        &request("pane.show", vec![("target", json!(pane_id.to_string()))]),
        moment(0),
    ));

    assert_eq!(value["program_status"], json!("running"));
    assert_eq!(value["status"], Value::Null);
}

/// Mechanism: a pane's `size` comes from its tab's declared size and
/// layout, and is `None` whenever the tab has no declared size at all
/// (R91, "where known") - never a guess.
#[test]
fn pane_show_size_is_none_when_the_tab_has_no_declared_size() {
    let (mut tree, context, api) = fresh();
    let pane_id = context.default_pane.unwrap();

    let value = ok(dispatch(
        &mut tree,
        &mut Settings::new(),
        &api,
        &context,
        &request("pane.show", vec![("target", json!(pane_id.to_string()))]),
        moment(0),
    ));

    assert_eq!(value["size"], Value::Null);
}

/// The other half of the mechanism above: once the tab has a declared
/// size, `pane.show`'s `size` reflects this pane's own leaf size within
/// it (`Tree::tab_layout`'s `Layout::leaf_sizes`), not the tab's whole
/// size.
#[test]
fn pane_show_size_reflects_the_tabs_declared_size_and_this_panes_own_leaf() {
    let (mut tree, context, api) = fresh();
    let tab_id = context.default_tab.unwrap();
    let pane_id = context.default_pane.unwrap();
    let client = tree.connect_client(steg_sdk::ClientKind::Cli, None);
    tree.view_pane(client, pane_id).unwrap();
    tree.declare_size(client, tab_id, steg_sdk::Size::new(100, 20))
        .unwrap();

    let value = ok(dispatch(
        &mut tree,
        &mut Settings::new(),
        &api,
        &context,
        &request("pane.show", vec![("target", json!(pane_id.to_string()))]),
        moment(0),
    ));

    assert_eq!(
        value["size"],
        json!({"columns": 100, "rows": 20}),
        "this tab's only pane fills the whole declared size"
    );
}

/// Mechanism: `pane.list`'s order is R112's layout order (the layout
/// tree's own leaves, first child before second), not creation order or
/// a reversal of it - built so the newest-created pane (`second`) sits
/// on the *left* (R112's first), the opposite of creation order, so
/// "reverse the order" or "return in creation order" would both answer
/// wrong here.
#[test]
fn pane_list_returns_every_pane_of_the_tab_in_r112s_layout_order() {
    let (mut tree, context, api) = fresh();
    let tab_id = context.default_tab.unwrap();
    let first = context.default_pane.unwrap();
    let second = tree
        .split_pane(
            first,
            steg_sdk::Direction::Left,
            steg_sdk::PaneKind::Terminal,
        )
        .unwrap();

    let value = ok(dispatch(
        &mut tree,
        &mut Settings::new(),
        &api,
        &context,
        &request("pane.list", vec![("target", json!(tab_id.to_string()))]),
        moment(0),
    ));

    let records = value.as_array().unwrap();
    assert_eq!(records.len(), 2);
    assert_eq!(
        records[0]["id"],
        json!(second.to_string()),
        "second was split to the Left of first, so it reads first in R112's order"
    );
    assert_eq!(records[1]["id"], json!(first.to_string()));
}

#[test]
fn pane_list_with_an_unknown_tab_is_no_such_target() {
    let (mut tree, context, api) = fresh();
    let unknown = TabId::from_raw(999);

    let error = err(dispatch(
        &mut tree,
        &mut Settings::new(),
        &api,
        &context,
        &request("pane.list", vec![("target", json!(unknown.to_string()))]),
        moment(0),
    ));

    assert_eq!(error.code, ErrorCode::NoSuchTarget);
}

// --- the ten-minute mark (doc-11 R83; decision-7) ----------------------------

/// The keystone test: AC #2's clock reaches `dispatch` as a `Moment`
/// parameter, and `status`'s ten-minute expiry (R83) is provable straight
/// through the function whose signature just changed for exactly this -
/// no real clock, no wait. `program_status` is read at the same two
/// moments as a negative control: it is a different, clock-free fact
/// (doc-15 R97) and must not move just because `status` did.
#[test]
fn a_finished_status_expires_through_dispatch_at_the_ten_minute_mark_while_program_status_does_not()
{
    let (mut tree, context, api) = fresh();
    let pane_id = context.default_pane.unwrap();
    tree.start_program(
        pane_id,
        steg_sdk::Program::Command {
            argv: vec!["htop".to_string()],
            cwd: "/tmp".to_string(),
        },
    )
    .unwrap();
    tree.record_exit(pane_id, 0, moment(0)).unwrap();

    let still_shown = ok(dispatch(
        &mut tree,
        &mut Settings::new(),
        &api,
        &context,
        &request("pane.show", vec![("target", json!(pane_id.to_string()))]),
        moment(599),
    ));
    assert_eq!(still_shown["status"], json!("finished"));
    assert_eq!(still_shown["program_status"], json!("ended"));

    let gone = ok(dispatch(
        &mut tree,
        &mut Settings::new(),
        &api,
        &context,
        &request("pane.show", vec![("target", json!(pane_id.to_string()))]),
        moment(600),
    ));
    assert_eq!(
        gone["status"],
        Value::Null,
        "R83: finished ends after ten minutes"
    );
    assert_eq!(
        gone["program_status"],
        json!("ended"),
        "program_status is clock-free (doc-15 R97) - it must not expire with status"
    );
}

#[test]
fn pane_split_makes_the_new_pane_current_by_default() {
    let (mut tree, context, api) = fresh();

    let value = ok(dispatch(
        &mut tree,
        &mut Settings::new(),
        &api,
        &context,
        &request("pane.split", vec![("direction", json!("right"))]),
        moment(0),
    ));

    let new_pane: steg_sdk::PaneId = value["id"].as_str().unwrap().parse().unwrap();
    assert_eq!(
        tree.session(context.default_session.unwrap())
            .unwrap()
            .tab(context.default_tab.unwrap())
            .unwrap()
            .current(),
        new_pane
    );
}

#[test]
fn pane_split_with_no_focus_restores_the_previous_current_pane() {
    let (mut tree, context, api) = fresh();
    let before = tree
        .session(context.default_session.unwrap())
        .unwrap()
        .tab(context.default_tab.unwrap())
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
        moment(0),
    ));

    let new_pane: steg_sdk::PaneId = value["id"].as_str().unwrap().parse().unwrap();
    let after = tree
        .session(context.default_session.unwrap())
        .unwrap()
        .tab(context.default_tab.unwrap())
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
            context.default_pane.unwrap(),
            steg_sdk::Direction::Right,
            steg_sdk::PaneKind::Terminal,
        )
        .unwrap();
    // `split_pane` just made `second_pane` current (R115); the call below
    // splits `default_pane`, which is *not* current, so a correct restore
    // has to read the tab's own current pane, not assume the target.
    let before = tree
        .session(context.default_session.unwrap())
        .unwrap()
        .tab(context.default_tab.unwrap())
        .unwrap()
        .clone();
    assert_eq!(before.current(), second_pane);
    assert_ne!(before.current(), context.default_pane.unwrap());

    let value = ok(dispatch(
        &mut tree,
        &mut Settings::new(),
        &api,
        &context,
        &request(
            "pane.split",
            vec![
                ("target", json!(context.default_pane.unwrap().to_string())),
                ("direction", json!("right")),
                ("focus", json!(false)),
            ],
        ),
        moment(0),
    ));

    let new_pane: steg_sdk::PaneId = value["id"].as_str().unwrap().parse().unwrap();
    let after = tree
        .session(context.default_session.unwrap())
        .unwrap()
        .tab(context.default_tab.unwrap())
        .unwrap();
    assert_eq!(after.current(), second_pane);
    assert_eq!(after.panes().len(), before.panes().len() + 1);
    assert!(after.panes().iter().any(|pane| pane.id() == new_pane));
}

#[test]
fn pane_split_with_an_invalid_label_leaves_the_tree_exactly_as_it_was() {
    let (mut tree, context, api) = fresh();
    let before = tree
        .session(context.default_session.unwrap())
        .unwrap()
        .tab(context.default_tab.unwrap())
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
        moment(0),
    ));

    assert_eq!(error.code, ErrorCode::BadRequest);
    let after = tree
        .session(context.default_session.unwrap())
        .unwrap()
        .tab(context.default_tab.unwrap())
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
    // after this same call, under one lock (its own plan, §1/§2). This
    // test calls `dispatch` directly, bypassing `Runtime` entirely, to
    // prove exactly the one thing `dispatch` alone ever does: mint the
    // pane and nothing more, with `program()`/`program_status()` still
    // `None` regardless of `program`/`cwd`.
    //
    // This is *not* a description of what any real caller sees: every
    // production path that can mint a pane - `pane.split`/`tab.new`/
    // `session.new` called directly, or through `api.call` - goes
    // through `Runtime::start_and_dispatch`, which always starts the
    // program right after this same call completes (confirmed by review:
    // the first version of this comment claimed "that is Runtime's own
    // job, right after this same call" without checking whether every
    // path actually reaches `Runtime` - `api.call` did not, until that
    // finding's fix). `grep -rn 'split_pane(\|new_tab(\|new_session('
    // over `crates/server/src` outside tests shows exactly these three
    // dispatch arms as the only callers.
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
        moment(0),
    ));

    let new_pane: steg_sdk::PaneId = outcome["id"].as_str().unwrap().parse().unwrap();
    let pane = tree
        .session(context.default_session.unwrap())
        .unwrap()
        .tab(context.default_tab.unwrap())
        .unwrap()
        .pane(new_pane)
        .expect("dispatch minted this pane");
    assert_eq!(pane.program(), None);
    assert_eq!(pane.program_status(), None);
}

#[test]
fn pane_close_removes_the_pane() {
    let (mut tree, context, api) = fresh();
    let second = tree
        .split_pane(
            context.default_pane.unwrap(),
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
        moment(0),
    ));

    assert_eq!(
        tree.session(context.default_session.unwrap())
            .unwrap()
            .tab(context.default_tab.unwrap())
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
            context.default_pane.unwrap(),
            steg_sdk::Direction::Right,
            steg_sdk::PaneKind::Terminal,
        )
        .unwrap();
    let second_tab = tree
        .new_tab(context.default_session.unwrap(), None)
        .unwrap();

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
        moment(0),
    ));

    let target_tab = tree
        .session(context.default_session.unwrap())
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
        moment(0),
    ));
    assert_eq!(
        tree.session(context.default_session.unwrap())
            .unwrap()
            .tab(context.default_tab.unwrap())
            .unwrap()
            .fill(),
        Some(context.default_pane.unwrap())
    );

    ok(dispatch(
        &mut tree,
        &mut Settings::new(),
        &api,
        &context,
        &request("pane.unset", vec![("zoomed", json!(false))]),
        moment(0),
    ));
    assert_eq!(
        tree.session(context.default_session.unwrap())
            .unwrap()
            .tab(context.default_tab.unwrap())
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
            context.default_pane.unwrap(),
            steg_sdk::Direction::Right,
            steg_sdk::PaneKind::Terminal,
        )
        .unwrap();
    tree.focus_pane(context.default_pane.unwrap()).unwrap();

    ok(dispatch(
        &mut tree,
        &mut Settings::new(),
        &api,
        &context,
        &request(
            "pane.focus",
            vec![
                ("target", json!(context.default_pane.unwrap().to_string())),
                ("direction", json!("right")),
            ],
        ),
        moment(0),
    ));

    assert_eq!(
        tree.session(context.default_session.unwrap())
            .unwrap()
            .tab(context.default_tab.unwrap())
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
            context.default_pane.unwrap(),
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
                ("target", json!(context.default_pane.unwrap().to_string())),
                ("with", json!(second.to_string())),
            ],
        ),
        moment(0),
    ));

    let panes = tree
        .session(context.default_session.unwrap())
        .unwrap()
        .tab(context.default_tab.unwrap())
        .unwrap()
        .panes()
        .iter()
        .map(steg_kernel::Pane::id)
        .collect::<Vec<_>>();
    assert_eq!(panes, vec![second, context.default_pane.unwrap()]);
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
        moment(0),
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
        moment(0),
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
        moment(0),
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
        moment(0),
    ));

    assert_eq!(error.code, ErrorCode::NoSuchTarget);
}

#[test]
fn api_call_with_no_method_is_a_bad_request() {
    let (mut tree, context, api) = fresh();

    let error = err(dispatch(
        &mut tree,
        &mut Settings::new(),
        &api,
        &context,
        &request("api.call", vec![]),
        moment(0),
    ));

    assert_eq!(error.code, ErrorCode::BadRequest);
    assert_eq!(error.message, "method: must be text");
}

/// A `method` that is present but not text takes the same early refusal
/// as an absent one (`param_str` returns `None` for either) - asserted
/// explicitly here, with its own two cases, rather than assumed from the
/// absent-`method` test above.
#[test]
fn api_call_with_a_non_text_method_is_a_bad_request() {
    for value in [json!(42), json!({"not": "text"})] {
        let (mut tree, context, api) = fresh();

        let error = err(dispatch(
            &mut tree,
            &mut Settings::new(),
            &api,
            &context,
            &request("api.call", vec![("method", value.clone())]),
            moment(0),
        ));

        assert_eq!(error.code, ErrorCode::BadRequest, "{value}");
        assert_eq!(error.message, "method: must be text", "{value}");
    }
}

/// Compares the whole error, not only the code: against the pre-TASK-1.81
/// code (selector read and stripped under `target`), this request has no
/// `target` key at all, so it is refused for *that* reason
/// (`"target: must be text"`) before the self-call check is ever reached.
/// A code-only assertion cannot tell "refuses to call itself" apart from
/// "the selector is missing", which is exactly what let this test keep
/// passing against the bug this task fixes.
#[test]
fn api_call_refuses_to_call_itself() {
    let (mut tree, context, api) = fresh();

    let error = err(dispatch(
        &mut tree,
        &mut Settings::new(),
        &api,
        &context,
        &request("api.call", vec![("method", json!("api.call"))]),
        moment(0),
    ));

    assert_eq!(error.code, ErrorCode::BadRequest);
    assert_eq!(error.message, "api.call cannot call api.call");
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
        moment(0),
    );
    let via_call = dispatch(
        &mut via_call_tree,
        &mut Settings::new(),
        &via_call_api,
        &via_call_context,
        &request(
            "api.call",
            vec![
                ("method", json!("pane.split")),
                ("direction", json!("right")),
            ],
        ),
        moment(0),
    );

    match (direct, via_call) {
        (Response::Ok(a), Response::Ok(b)) => assert_eq!(a, b),
        other => panic!("expected both calls to succeed: {other:?}"),
    }
}

/// Compares the whole `ApiError` - code, message and candidates - not
/// only the code: two paths could share a code while disagreeing on what
/// a person or a script actually sees.
#[test]
fn api_call_of_an_unknown_method_is_refused_the_same_way_a_direct_call_would_be() {
    let (mut direct_tree, direct_context, direct_api) = fresh();
    let (mut via_call_tree, via_call_context, via_call_api) = fresh();

    let direct_error = err(dispatch(
        &mut direct_tree,
        &mut Settings::new(),
        &direct_api,
        &direct_context,
        &request("pane.nonexistent", vec![]),
        moment(0),
    ));
    let via_call_error = err(dispatch(
        &mut via_call_tree,
        &mut Settings::new(),
        &via_call_api,
        &via_call_context,
        &request("api.call", vec![("method", json!("pane.nonexistent"))]),
        moment(0),
    ));

    assert_eq!(direct_error.code, ErrorCode::BadRequest);
    assert_eq!(direct_error, via_call_error);
}

/// AC #1's "the error must be the one a direct call gives", for the case
/// a parity check of successful calls alone cannot exercise: the called
/// method's own validation fails (`pane.split` with no `direction`, its
/// one required parameter).
#[test]
fn api_call_of_a_request_that_fails_validation_gives_the_same_error_a_direct_call_would() {
    let (mut direct_tree, direct_context, direct_api) = fresh();
    let (mut via_call_tree, via_call_context, via_call_api) = fresh();

    let direct_error = err(dispatch(
        &mut direct_tree,
        &mut Settings::new(),
        &direct_api,
        &direct_context,
        &request("pane.split", vec![]),
        moment(0),
    ));
    let via_call_error = err(dispatch(
        &mut via_call_tree,
        &mut Settings::new(),
        &via_call_api,
        &via_call_context,
        &request("api.call", vec![("method", json!("pane.split"))]),
        moment(0),
    ));

    assert_eq!(direct_error.code, ErrorCode::BadRequest);
    assert_eq!(direct_error, via_call_error);
}

// --- api.call reaches a method with its own required target (AC #2) ------
//
// setting.show/set/unset have a required target and no default - exactly
// the shape that could never be reached through api.call before TASK-1.81
// (api.call's own selector and the inner `target` were the same wire key).
// Named explicitly, mirroring the two tests TASK-1.48's worker wrote and
// TASK-1.81's own notes record as removed before that task merged.

#[test]
fn api_call_reaches_setting_show_with_its_required_target() {
    let (mut direct_tree, direct_context, direct_api) = fresh();
    let (mut via_call_tree, via_call_context, via_call_api) = fresh();

    let direct = ok(dispatch(
        &mut direct_tree,
        &mut Settings::new(),
        &direct_api,
        &direct_context,
        &request("setting.show", vec![("target", json!("appearance"))]),
        moment(0),
    ));
    let via_call = ok(dispatch(
        &mut via_call_tree,
        &mut Settings::new(),
        &via_call_api,
        &via_call_context,
        &request(
            "api.call",
            vec![
                ("method", json!("setting.show")),
                ("target", json!("appearance")),
            ],
        ),
        moment(0),
    ));

    assert_eq!(direct, via_call);
}

#[test]
fn api_call_reaches_setting_set_with_its_required_target() {
    let (mut direct_tree, direct_context, direct_api) = fresh();
    let (mut via_call_tree, via_call_context, via_call_api) = fresh();

    let direct = ok(dispatch(
        &mut direct_tree,
        &mut Settings::new(),
        &direct_api,
        &direct_context,
        &request(
            "setting.set",
            vec![("target", json!("appearance")), ("value", json!("light"))],
        ),
        moment(0),
    ));
    let via_call = ok(dispatch(
        &mut via_call_tree,
        &mut Settings::new(),
        &via_call_api,
        &via_call_context,
        &request(
            "api.call",
            vec![
                ("method", json!("setting.set")),
                ("target", json!("appearance")),
                ("value", json!("light")),
            ],
        ),
        moment(0),
    ));

    assert_eq!(direct, via_call);
}

#[test]
fn api_call_reaches_setting_unset_with_its_required_target() {
    let (mut direct_tree, direct_context, direct_api) = fresh();
    let (mut via_call_tree, via_call_context, via_call_api) = fresh();

    let direct = ok(dispatch(
        &mut direct_tree,
        &mut Settings::new(),
        &direct_api,
        &direct_context,
        &request("setting.unset", vec![("target", json!("appearance"))]),
        moment(0),
    ));
    let via_call = ok(dispatch(
        &mut via_call_tree,
        &mut Settings::new(),
        &via_call_api,
        &via_call_context,
        &request(
            "api.call",
            vec![
                ("method", json!("setting.unset")),
                ("target", json!("appearance")),
            ],
        ),
        moment(0),
    ));

    assert_eq!(direct, via_call);
}

// --- api.call names a pane, tab or session, not the context's default
// (AC #4) ---------------------------------------------------------------
//
// Response equality alone cannot prove this: both calls could answer
// `Nothing` even if `api.call` silently fell back to the context's
// default instead of the named target. Each test here asserts both
// halves - the named one changed, and the default did not.

#[test]
fn api_call_of_pane_set_names_the_target_pane_not_the_context_default() {
    let (mut tree, context, api) = fresh();
    let named = tree
        .split_pane(
            context.default_pane.unwrap(),
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
            "api.call",
            vec![
                ("method", json!("pane.set")),
                ("target", json!(named.to_string())),
                ("label", json!("logs")),
            ],
        ),
        moment(0),
    ));

    let tab = tree
        .session(context.default_session.unwrap())
        .unwrap()
        .tab(context.default_tab.unwrap())
        .unwrap();
    assert_eq!(tab.pane(named).unwrap().label(), Some("logs"));
    assert_eq!(
        tab.pane(context.default_pane.unwrap()).unwrap().label(),
        None
    );
}

#[test]
fn api_call_of_tab_set_names_the_target_tab_not_the_context_default() {
    let (mut tree, context, api) = fresh();
    let named = tree
        .new_tab(context.default_session.unwrap(), None)
        .unwrap();

    ok(dispatch(
        &mut tree,
        &mut Settings::new(),
        &api,
        &context,
        &request(
            "api.call",
            vec![
                ("method", json!("tab.set")),
                ("target", json!(named.to_string())),
                ("label", json!("work")),
            ],
        ),
        moment(0),
    ));

    let session = tree.session(context.default_session.unwrap()).unwrap();
    assert_eq!(session.tab(named).unwrap().label(), Some("work"));
    assert_eq!(
        session.tab(context.default_tab.unwrap()).unwrap().label(),
        None,
        "the context's default tab must be untouched"
    );
}

#[test]
fn api_call_of_session_set_names_the_target_session_not_the_context_default() {
    let (mut tree, context, api) = fresh();
    let named = tree.new_session(None).unwrap();

    ok(dispatch(
        &mut tree,
        &mut Settings::new(),
        &api,
        &context,
        &request(
            "api.call",
            vec![
                ("method", json!("session.set")),
                ("target", json!(named.to_string())),
                ("label", json!("scratch")),
            ],
        ),
        moment(0),
    ));

    assert_eq!(tree.session(named).unwrap().label(), Some("scratch"));
    assert_eq!(
        tree.session(context.default_session.unwrap())
            .unwrap()
            .label(),
        None,
        "the context's default session must be untouched"
    );
}

// --- api.call reaches every served method, with its own target given
// where it has one (AC #1 and AC #4) -------------------------------------

/// This method's own extra parameters (everything but which method is
/// being called), built fresh against `tree`/`context`. Every method with
/// a target gets a freshly created sibling - never `context`'s default -
/// so the parity loop below cannot pass by both sides quietly falling
/// back to the same default (AC #4); `api.call` itself is excluded by its
/// caller, not here. Exhaustive by name with a `panic!` fallback, so a
/// method added to the description later and missed here fails loudly
/// rather than being silently skipped from the proof (AC #1).
fn extra_params_for(
    dotted: &str,
    tree: &mut Tree,
    context: &Context,
) -> Vec<(&'static str, Value)> {
    use steg_sdk::{Direction, PaneKind};

    match dotted {
        "session.list" | "session.new" | "server.show" | "server.start" | "server.stop"
        | "client.list" | "setting.list" | "api.list" => vec![],

        "session.show" | "session.close" | "session.focus" => {
            let other = tree.new_session(None).unwrap();
            vec![("target", json!(other.to_string()))]
        }
        "session.move" => {
            let other = tree.new_session(None).unwrap();
            vec![("target", json!(other.to_string())), ("position", json!(1))]
        }
        "session.set" => {
            let other = tree.new_session(None).unwrap();
            vec![("target", json!(other.to_string())), ("label", json!("x"))]
        }
        "session.unset" => {
            let other = tree.new_session(None).unwrap();
            vec![("target", json!(other.to_string())), ("label", json!(true))]
        }

        "tab.list" | "tab.new" => {
            let other_session = tree.new_session(None).unwrap();
            vec![("target", json!(other_session.to_string()))]
        }
        "tab.show" | "tab.close" | "tab.focus" => {
            let other_session = tree.new_session(None).unwrap();
            let other_tab = tree.new_tab(other_session, None).unwrap();
            vec![("target", json!(other_tab.to_string()))]
        }
        "tab.move" => {
            // Not `to`: moving a tab between sessions is its own
            // `NotImplementedYet` scenario (`every_not_yet_scenario_...`)
            // - this method's own, already-working path is reordering by
            // `position` within one session.
            let other_session = tree.new_session(None).unwrap();
            let other_tab = tree.new_tab(other_session, None).unwrap();
            vec![
                ("target", json!(other_tab.to_string())),
                ("position", json!(1)),
            ]
        }
        "tab.set" => {
            let other_session = tree.new_session(None).unwrap();
            let other_tab = tree.new_tab(other_session, None).unwrap();
            vec![
                ("target", json!(other_tab.to_string())),
                ("label", json!("x")),
            ]
        }
        "tab.unset" => {
            let other_session = tree.new_session(None).unwrap();
            let other_tab = tree.new_tab(other_session, None).unwrap();
            vec![
                ("target", json!(other_tab.to_string())),
                ("label", json!(true)),
            ]
        }
        "tab.declare-size" => {
            let other_session = tree.new_session(None).unwrap();
            let other_tab = tree.new_tab(other_session, None).unwrap();
            vec![
                ("target", json!(other_tab.to_string())),
                ("columns", json!(80)),
                ("rows", json!(24)),
            ]
        }

        "pane.list" => {
            // This noun's own target is a tab, not a pane.
            let other_session = tree.new_session(None).unwrap();
            let other_tab = tree.session(other_session).unwrap().tabs()[0].id();
            vec![("target", json!(other_tab.to_string()))]
        }
        "pane.show" | "pane.close" | "pane.send" | "pane.capture" | "pane.attach"
        | "pane.detach" => {
            let other_pane = tree
                .split_pane(
                    context.default_pane.unwrap(),
                    Direction::Right,
                    PaneKind::Terminal,
                )
                .unwrap();
            vec![("target", json!(other_pane.to_string()))]
        }
        "pane.split" => {
            let other_pane = tree
                .split_pane(
                    context.default_pane.unwrap(),
                    Direction::Right,
                    PaneKind::Terminal,
                )
                .unwrap();
            vec![
                ("target", json!(other_pane.to_string())),
                ("direction", json!("right")),
            ]
        }
        "pane.move" => {
            let other_pane = tree
                .split_pane(
                    context.default_pane.unwrap(),
                    Direction::Right,
                    PaneKind::Terminal,
                )
                .unwrap();
            let other_tab = tree
                .new_tab(context.default_session.unwrap(), None)
                .unwrap();
            vec![
                ("target", json!(other_pane.to_string())),
                ("to", json!(other_tab.to_string())),
            ]
        }
        "pane.set" => {
            let other_pane = tree
                .split_pane(
                    context.default_pane.unwrap(),
                    Direction::Right,
                    PaneKind::Terminal,
                )
                .unwrap();
            vec![
                ("target", json!(other_pane.to_string())),
                ("label", json!("x")),
            ]
        }
        "pane.unset" => {
            let other_pane = tree
                .split_pane(
                    context.default_pane.unwrap(),
                    Direction::Right,
                    PaneKind::Terminal,
                )
                .unwrap();
            vec![
                ("target", json!(other_pane.to_string())),
                ("label", json!(true)),
            ]
        }
        "pane.focus" => {
            let other_pane = tree
                .split_pane(
                    context.default_pane.unwrap(),
                    Direction::Right,
                    PaneKind::Terminal,
                )
                .unwrap();
            vec![("target", json!(other_pane.to_string()))]
        }
        "pane.resize" => {
            let other_pane = tree
                .split_pane(
                    context.default_pane.unwrap(),
                    Direction::Right,
                    PaneKind::Terminal,
                )
                .unwrap();
            vec![
                ("target", json!(other_pane.to_string())),
                ("direction", json!("right")),
                ("by", json!(1)),
            ]
        }
        "pane.swap" => {
            let other_pane = tree
                .split_pane(
                    context.default_pane.unwrap(),
                    Direction::Right,
                    PaneKind::Terminal,
                )
                .unwrap();
            vec![
                ("target", json!(other_pane.to_string())),
                ("with", json!(context.default_pane.unwrap().to_string())),
            ]
        }
        "pane.wait" => {
            let other_pane = tree
                .split_pane(
                    context.default_pane.unwrap(),
                    Direction::Right,
                    PaneKind::Terminal,
                )
                .unwrap();
            vec![
                ("target", json!(other_pane.to_string())),
                ("for", json!("exit")),
            ]
        }

        "event.watch" => {
            let other_session = tree.new_session(None).unwrap();
            vec![("target", json!(other_session.to_string()))]
        }
        "client.show" => vec![("target", json!("x"))],

        "setting.show" | "setting.unset" => vec![("target", json!("appearance"))],
        "setting.set" => vec![("target", json!("appearance")), ("value", json!("light"))],

        "api.show" => vec![("target", json!("pane.split"))],

        other => panic!(
            "{other} has no case in extra_params_for - add one so it is covered by this \
             description-driven proof"
        ),
    }
}

/// `Response` carries no `PartialEq` (it holds an `ApiError`, which does,
/// but also a bare `serde_json::Value`, which also does - the enum itself
/// just never derived it); this is the comparable shape both sides are
/// reduced to.
fn canonical(response: Response) -> Result<Value, steg_sdk::ApiError> {
    match response {
        Response::Ok(value) => Ok(value),
        Response::Error(error) => Err(error),
    }
}

/// AC #1: every served method, called through `api.call` with its own
/// target given where it has one, gives the same response a direct call
/// would - success or error alike, so this also shows `api.call` forwards
/// a method's own failures unchanged. `api.call` itself is excluded: it
/// refuses to call itself (`api_call_refuses_to_call_itself`), so it has
/// no "direct call" to compare against.
#[test]
fn api_call_matches_a_direct_call_for_every_served_method_with_its_own_target_given() {
    let (_, _, api) = fresh();
    let mut checked = 0;

    for method in &api.methods {
        let dotted = method.name.dotted();
        if dotted == "api.call" {
            continue;
        }
        checked += 1;

        let (mut direct_tree, direct_context, direct_api) = fresh();
        let (mut via_call_tree, via_call_context, via_call_api) = fresh();

        let extra_direct = extra_params_for(&dotted, &mut direct_tree, &direct_context);
        let extra_via = extra_params_for(&dotted, &mut via_call_tree, &via_call_context);
        assert_eq!(
            extra_direct, extra_via,
            "{dotted}: two identically-built trees minted different ids"
        );

        let direct_response = dispatch(
            &mut direct_tree,
            &mut Settings::new(),
            &direct_api,
            &direct_context,
            &request(&dotted, extra_direct),
            moment(0),
        );

        let mut via_params = vec![("method", json!(dotted.clone()))];
        via_params.extend(extra_via);
        let via_response = dispatch(
            &mut via_call_tree,
            &mut Settings::new(),
            &via_call_api,
            &via_call_context,
            &request("api.call", via_params),
            moment(0),
        );

        assert_eq!(
            canonical(direct_response),
            canonical(via_response),
            "{dotted}"
        );
    }

    assert_eq!(
        checked,
        api.methods.len() - 1,
        "api.call is the one exclusion"
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
        moment(0),
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
        moment(0),
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
        moment(0),
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
        moment(0),
    ));

    let value = ok(dispatch(
        &mut tree,
        &mut settings,
        &api,
        &context,
        &request("setting.show", vec![("target", json!("appearance"))]),
        moment(0),
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
        moment(0),
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
        moment(0),
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
        moment(0),
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
        moment(0),
    ));
    ok(dispatch(
        &mut tree,
        &mut settings,
        &api,
        &context,
        &request("setting.unset", vec![("target", json!("appearance"))]),
        moment(0),
    ));

    let value = ok(dispatch(
        &mut tree,
        &mut settings,
        &api,
        &context,
        &request("setting.show", vec![("target", json!("appearance"))]),
        moment(0),
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
        moment(0),
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
        moment(0),
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
        moment(0),
    ));

    assert_eq!(value["value"], json!("dark"));
}
