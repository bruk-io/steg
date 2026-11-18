use serde_json::json;
use steg_kernel::Settings;
use steg_sdk::ErrorCode;
use steg_server::dispatch;

use crate::fixtures::{err, fresh, moment, ok, request};

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
