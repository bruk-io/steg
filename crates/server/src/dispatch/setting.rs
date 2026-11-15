//! The `setting` noun's dispatch arms (doc-2 section 8; R80, decision-9).
//!
//! No `Tree` and no `Context` here: a setting's target is its own name,
//! not a session/tab/pane path, so it needs neither — the one noun in
//! this module tree that doesn't.

use steg_kernel::Settings;
use steg_sdk::{ApiError, Request, SettingRecord};

use super::outcome::Outcome;
use super::{bad_request, map_kernel_error, param_str};

/// Every `setting.*` method this module matches on, checked against
/// `description().methods` by `mod.rs`'s own completeness test.
pub(super) const HANDLED: &[(&str, &str)] = &[
    ("setting", "list"),
    ("setting", "show"),
    ("setting", "set"),
    ("setting", "unset"),
];

pub(super) fn dispatch(
    settings: &mut Settings,
    request: &Request,
    verb: &str,
) -> Result<Outcome, ApiError> {
    match verb {
        "list" => list(settings),
        "show" => show(settings, request),
        "set" => set(settings, request),
        "unset" => unset(settings, request),
        _ => unreachable!("HANDLED names every verb this match covers"),
    }
}

/// `request`'s `target`, as the setting's name — required on every verb
/// but `list`. `target` is declared `required: true` with
/// `ValueKind::Text`, so `validate` already refuses an absent or
/// wrong-typed one before this module is ever reached through either
/// real entry point (`mod.rs`'s top-level `dispatch`, and `api.call`'s
/// own recursion through it, which calls that same `dispatch`). Both
/// cases below are reachable only by calling this module's own
/// `dispatch` directly, bypassing `validate` — which the tests below do,
/// so each message stays the true story for its own case rather than one
/// message covering both.
fn target_name(request: &Request) -> Result<&str, ApiError> {
    match request.params.get("target") {
        None => Err(bad_request("needs target")),
        Some(value) => value
            .as_str()
            .ok_or_else(|| bad_request("target must be text")),
    }
}

fn record_of(settings: &Settings, name: &str) -> Result<SettingRecord, ApiError> {
    let value = settings.value_of(name).map_err(map_kernel_error)?;
    let default = steg_sdk::settings()
        .into_iter()
        .find(|setting| setting.name == name)
        .expect("value_of just resolved this name against the same registry")
        .default
        .to_string();
    Ok(SettingRecord {
        name: name.to_string(),
        value,
        default,
    })
}

fn list(settings: &Settings) -> Result<Outcome, ApiError> {
    let records = steg_sdk::settings()
        .into_iter()
        .map(|setting| {
            record_of(settings, setting.name)
                .expect("every registered name resolves against the same registry")
        })
        .collect();
    Ok(Outcome::SettingRecords(records))
}

fn show(settings: &Settings, request: &Request) -> Result<Outcome, ApiError> {
    let name = target_name(request)?;
    record_of(settings, name).map(Outcome::SettingRecord)
}

fn set(settings: &mut Settings, request: &Request) -> Result<Outcome, ApiError> {
    let name = target_name(request)?;
    let value = param_str(request, "value").ok_or_else(|| bad_request("needs value"))?;
    settings.set(name, value).map_err(map_kernel_error)?;
    Ok(Outcome::Nothing)
}

fn unset(settings: &mut Settings, request: &Request) -> Result<Outcome, ApiError> {
    let name = target_name(request)?;
    settings.unset(name).map_err(map_kernel_error)?;
    Ok(Outcome::Nothing)
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use steg_kernel::Settings;
    use steg_sdk::{ErrorCode, Request};

    use super::dispatch;

    fn request(method: &str, pairs: Vec<(&str, serde_json::Value)>) -> Request {
        Request {
            method: method.into(),
            params: pairs.into_iter().map(|(k, v)| (k.to_owned(), v)).collect(),
        }
    }

    #[test]
    fn an_absent_target_says_so_rather_than_claiming_the_wrong_type() {
        let mut settings = Settings::new();
        let error = dispatch(&mut settings, &request("setting.show", vec![]), "show")
            .expect_err("no target at all");
        assert_eq!(error.code, ErrorCode::BadRequest);
        assert_eq!(error.message, "needs target");
    }

    #[test]
    fn a_non_text_target_says_so_rather_than_claiming_it_is_missing() {
        let mut settings = Settings::new();
        let error = dispatch(
            &mut settings,
            &request("setting.show", vec![("target", json!(42))]),
            "show",
        )
        .expect_err("a number, not text");
        assert_eq!(error.code, ErrorCode::BadRequest);
        assert_eq!(error.message, "target must be text");
    }

    /// `set`'s own "needs value" refusal is unreachable the same way as
    /// `target_name`'s two cases above - `value` is `required: true`
    /// with `ValueKind::Text`, so `validate` already refuses either case
    /// before this module runs. Both sub-cases reach the one message
    /// here, unlike `target_name`'s two.
    #[test]
    fn an_absent_value_is_a_bad_request() {
        let mut settings = Settings::new();
        let error = dispatch(
            &mut settings,
            &request("setting.set", vec![("target", json!("appearance"))]),
            "set",
        )
        .expect_err("no value at all");
        assert_eq!(error.code, ErrorCode::BadRequest);
        assert_eq!(error.message, "needs value");
    }

    #[test]
    fn a_non_text_value_is_also_a_bad_request() {
        let mut settings = Settings::new();
        let error = dispatch(
            &mut settings,
            &request(
                "setting.set",
                vec![("target", json!("appearance")), ("value", json!(42))],
            ),
            "set",
        )
        .expect_err("a number, not text");
        assert_eq!(error.code, ErrorCode::BadRequest);
    }
}
