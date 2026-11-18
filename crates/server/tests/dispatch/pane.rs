use serde_json::{Value, json};
use steg_kernel::Settings;
use steg_sdk::{ErrorCode, TabId};
use steg_server::dispatch;

use crate::fixtures::{err, fresh, moment, ok, request};

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
