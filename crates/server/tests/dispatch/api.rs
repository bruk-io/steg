use serde_json::{Value, json};
use steg_kernel::{Settings, Tree};
use steg_sdk::{ErrorCode, Response};
use steg_server::{Context, dispatch};

use crate::fixtures::{err, fresh, moment, ok, request};

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
