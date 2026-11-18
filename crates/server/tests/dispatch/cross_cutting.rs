//! Tests that cross several nouns in one call sequence, or exercise
//! `dispatch`'s own completeness/shape guarantees rather than one
//! noun's behaviour, so they don't belong in any single
//! `dispatch::<noun>` module: the two completeness tests (every
//! described method, every `NotImplementedYet` scenario), the two
//! shape/end-to-end tests (every `Nothing`/`Identifier`-returning
//! method, a multi-call session/pane scenario), and
//! `unset_label_clears_on_either_switch_value_and_leaves_it_alone_when_not_given`,
//! which loops `session`/`tab`/`pane` in a single test body.
//! `tab_new_with_no_focus_leaves_the_original_tab_current` lives here
//! too, not in `dispatch::tab`, for the same "grouped by the end-to-end
//! section's purpose, not the noun named in its own identifier" reason.

use serde_json::{Value, json};
use steg_kernel::{Settings, Tree};
use steg_sdk::{ErrorCode, Response};
use steg_server::dispatch;

use crate::fixtures::{err, fresh, moment, ok, request};

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
            moment(0),
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
        ("session.focus", vec![], "TASK-1.109"),
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
        ("pane.capture", vec![], "not built yet"),
        ("pane.wait", vec![("for", json!("exit"))], "not built yet"),
        ("server.show", vec![], "socket and lifecycle"),
        ("server.start", vec![], "socket and lifecycle"),
        ("server.stop", vec![], "socket and lifecycle"),
        ("event.watch", vec![], "Runtime::subscribe"),
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
            moment(0),
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

// --- the label switch, across all three nouns that have one (TASK-1.49) ----

/// TASK-1.49's own fix: `label` is now a switch on `unset` (sdk's
/// `unset_s_label_is_a_switch_not_a_value_taking_flag`), the same shape
/// `pane.unset`'s own `zoomed` already has - and the same rule already
/// documented on `dispatch/pane.rs`'s `unset`: it clears regardless of
/// the switch's own boolean value. Proved directly here, for `label`,
/// across all three nouns that have one, rather than assumed from the
/// `zoomed` precedent: `--label` (`true`) clears it, `--no-label`
/// (`false`) *also* clears it, and leaving the flag out entirely leaves
/// the label untouched.
///
/// **Negative control, run once and not re-run (`dispatch`'s own logic is
/// untouched by this task - only the description's declared `ValueKind`
/// changed):** with `crates/server/src/dispatch/session.rs`'s `unset`
/// temporarily changed from `param_present(request, "label")` to
/// `param_bool(request, "label") == Some(true)`, the `false`-clears
/// assertion below failed (the label was left as `"work"`, not cleared) -
/// confirming this test does distinguish "clears on any presence" from
/// "clears only when true", not merely that *something* runs. Reverted
/// before this test was committed.
#[test]
fn unset_label_clears_on_either_switch_value_and_leaves_it_alone_when_not_given() {
    for noun in ["session", "tab", "pane"] {
        let (mut tree, context, api) = fresh();
        let set = format!("{noun}.set");
        let unset = format!("{noun}.unset");
        let label = |tree: &Tree| -> Option<String> {
            match noun {
                "session" => tree
                    .session(context.default_session.unwrap())
                    .unwrap()
                    .label()
                    .map(str::to_owned),
                "tab" => tree
                    .session(context.default_session.unwrap())
                    .unwrap()
                    .tab(context.default_tab.unwrap())
                    .unwrap()
                    .label()
                    .map(str::to_owned),
                "pane" => tree
                    .session(context.default_session.unwrap())
                    .unwrap()
                    .tab(context.default_tab.unwrap())
                    .unwrap()
                    .pane(context.default_pane.unwrap())
                    .unwrap()
                    .label()
                    .map(str::to_owned),
                other => panic!("no case for {other}"),
            }
        };

        // `--label` (true) clears a label that was set.
        ok(dispatch(
            &mut tree,
            &mut Settings::new(),
            &api,
            &context,
            &request(&set, vec![("label", json!("x"))]),
            moment(0),
        ));
        assert_eq!(label(&tree), Some("x".to_owned()), "{noun}: set first");
        ok(dispatch(
            &mut tree,
            &mut Settings::new(),
            &api,
            &context,
            &request(&unset, vec![("label", json!(true))]),
            moment(0),
        ));
        assert_eq!(label(&tree), None, "{noun}: --label should clear");

        // `--no-label` (false) *also* clears it - the value is discarded.
        ok(dispatch(
            &mut tree,
            &mut Settings::new(),
            &api,
            &context,
            &request(&set, vec![("label", json!("y"))]),
            moment(0),
        ));
        assert_eq!(label(&tree), Some("y".to_owned()), "{noun}: set again");
        ok(dispatch(
            &mut tree,
            &mut Settings::new(),
            &api,
            &context,
            &request(&unset, vec![("label", json!(false))]),
            moment(0),
        ));
        assert_eq!(label(&tree), None, "{noun}: --no-label should also clear");

        // Neither flag given: the label is left alone.
        ok(dispatch(
            &mut tree,
            &mut Settings::new(),
            &api,
            &context,
            &request(&set, vec![("label", json!("z"))]),
            moment(0),
        ));
        ok(dispatch(
            &mut tree,
            &mut Settings::new(),
            &api,
            &context,
            &request(&unset, vec![]),
            moment(0),
        ));
        assert_eq!(
            label(&tree),
            Some("z".to_owned()),
            "{noun}: no label flag at all should leave it unchanged"
        );
    }
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
    let tab_to_close = tree
        .new_tab(context.default_session.unwrap(), None)
        .unwrap();
    let tab_to_move = tree
        .new_tab(context.default_session.unwrap(), None)
        .unwrap();
    let tab_to_focus = tree
        .new_tab(context.default_session.unwrap(), None)
        .unwrap();
    let tab_for_pane_move = tree
        .new_tab(context.default_session.unwrap(), None)
        .unwrap();
    let pane_to_close = tree
        .split_pane(
            context.default_pane.unwrap(),
            steg_sdk::Direction::Right,
            steg_sdk::PaneKind::Terminal,
        )
        .unwrap();
    let pane_to_move = tree
        .split_pane(
            context.default_pane.unwrap(),
            steg_sdk::Direction::Right,
            steg_sdk::PaneKind::Terminal,
        )
        .unwrap();
    let pane_for_swap = tree
        .split_pane(
            context.default_pane.unwrap(),
            steg_sdk::Direction::Right,
            steg_sdk::PaneKind::Terminal,
        )
        .unwrap();
    // `pane.send` (TASK-1.90) refuses `ErrorCode::ProgramNotRunning`
    // unless something is running in the target - the default pane has
    // nothing recorded by default (`fresh()` never calls
    // `Tree::start_program`, the same reason `pane.rs`'s own
    // `pane_show_returns_the_panes_record` does this directly), so this
    // case needs the same direct kernel-level setup, done once here
    // rather than per-call below.
    tree.start_program(
        context.default_pane.unwrap(),
        steg_sdk::Program::Shell {
            cwd: "/tmp".to_string(),
        },
    )
    .unwrap();

    let not_yet: &[&str] = &[
        "session.focus",
        "pane.resize",
        "pane.detach",
        "tab.declare-size",
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
                moment(0),
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
            "session.unset" => vec![("label", json!(true))],
            "session.new" => vec![],
            "tab.close" => vec![("target", json!(tab_to_close.to_string()))],
            "tab.move" => vec![
                ("target", json!(tab_to_move.to_string())),
                ("position", json!(1)),
            ],
            "tab.set" => vec![("label", json!("x"))],
            "tab.unset" => vec![("label", json!(true))],
            "tab.focus" => vec![("target", json!(tab_to_focus.to_string()))],
            "tab.new" => vec![],
            "pane.close" => vec![("target", json!(pane_to_close.to_string()))],
            "pane.move" => vec![
                ("target", json!(pane_to_move.to_string())),
                ("to", json!(tab_for_pane_move.to_string())),
            ],
            "pane.set" => vec![("label", json!("x"))],
            "pane.unset" => vec![("label", json!(true))],
            "pane.focus" => vec![("target", json!(context.default_pane.unwrap().to_string()))],
            "pane.swap" => vec![
                ("target", json!(context.default_pane.unwrap().to_string())),
                ("with", json!(pane_for_swap.to_string())),
            ],
            "pane.split" => vec![("direction", json!("right"))],
            "pane.send" => vec![("text", json!("hi"))],
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
            moment(0),
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
        moment(0),
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
        moment(0),
    ));
    let pane_id: steg_sdk::PaneId = new_pane.parse().unwrap();
    assert_eq!(
        tree.session(context.default_session.unwrap())
            .unwrap()
            .tab(context.default_tab.unwrap())
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
        moment(0),
    ));
    assert!(tree.session(context.default_session.unwrap()).is_none());
}

#[test]
fn tab_new_with_no_focus_leaves_the_original_tab_current() {
    let (mut tree, context, api) = fresh();
    let original = context.default_tab.unwrap();

    ok(dispatch(
        &mut tree,
        &mut Settings::new(),
        &api,
        &context,
        &request("tab.new", vec![("focus", json!(false))]),
        moment(0),
    ));

    assert_eq!(
        tree.session(context.default_session.unwrap())
            .unwrap()
            .current_tab(),
        original
    );
}
