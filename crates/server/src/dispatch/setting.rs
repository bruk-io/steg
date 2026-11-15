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
/// but `list` (`validate` already refuses its absence before this runs;
/// this is defence for a call path that skips `validate`, such as a
/// future direct caller).
fn target_name(request: &Request) -> Result<&str, ApiError> {
    param_str(request, "target").ok_or_else(|| bad_request("target: must be text"))
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
