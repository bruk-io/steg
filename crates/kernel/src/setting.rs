//! Remembering a setting changed from the palette (R80, decision-9).
//!
//! [`Settings`] is the rule and the override map, both pure data: no I/O,
//! same as every other type in this crate. It depends on
//! `steg_sdk::settings()` for the closed list of names, their kinds and
//! their defaults (doc-16 R105) — nothing here invents a second list.
//! Actually keeping one of these alive for the life of a running server,
//! and (if a later task adds it) writing it to disk, is the server
//! crate's job (TASK-1.48's plan, "where I/O meets the kernel").

use std::collections::HashMap;

use steg_sdk::{Setting, ValueKind, settings};

use crate::error::Error;

/// The settings the server remembers, as overrides on top of
/// `steg_sdk::settings()`'s own defaults.
///
/// A setting with no override reads back its default; `set` changes an
/// override at once (R80: "applies at once"); `unset` clears one, which
/// is not an error even when nothing was overridden (the same reading
/// `pane.unset`'s "a property left out is unchanged" already gives an
/// absent property).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Settings {
    overrides: HashMap<String, String>,
}

impl Settings {
    /// An empty store: every setting reads back its own default.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// `name`'s current value: its override if one is set, else its
    /// default.
    ///
    /// # Errors
    ///
    /// [`Error::UnknownSetting`] if `name` is not in `steg_sdk::settings()`.
    pub fn value_of(&self, name: &str) -> Result<String, Error> {
        let schema = find(name)?;
        Ok(self
            .overrides
            .get(name)
            .cloned()
            .unwrap_or_else(|| schema.default.to_string()))
    }

    /// Changes `name`'s override to `value`, applying at once.
    ///
    /// # Errors
    ///
    /// [`Error::UnknownSetting`] if `name` is not in `steg_sdk::settings()`;
    /// [`Error::InvalidSettingValue`] if `value` does not match `name`'s
    /// own declared kind.
    pub fn set(&mut self, name: &str, value: &str) -> Result<(), Error> {
        let schema = find(name)?;
        if !accepts(&schema.value, value) {
            return Err(Error::InvalidSettingValue {
                name: name.to_string(),
                value: value.to_string(),
            });
        }
        self.overrides.insert(name.to_string(), value.to_string());
        Ok(())
    }

    /// Clears `name`'s override, reverting it to its default.
    ///
    /// # Errors
    ///
    /// [`Error::UnknownSetting`] if `name` is not in `steg_sdk::settings()`
    /// — checked even though there may be nothing to clear, the same way
    /// `session.unset`'s target must still resolve.
    pub fn unset(&mut self, name: &str) -> Result<(), Error> {
        find(name)?;
        self.overrides.remove(name);
        Ok(())
    }
}

/// The registry entry named `name`, or [`Error::UnknownSetting`].
fn find(name: &str) -> Result<Setting, Error> {
    settings()
        .into_iter()
        .find(|setting| setting.name == name)
        .ok_or_else(|| Error::UnknownSetting(name.to_string()))
}

/// Whether `value`'s wire text is one `kind` accepts.
///
/// `Target`, `Path` and `Argv` are never a setting's own declared kind
/// (`steg_sdk::setting`'s module doc, and `no_setting_declares_a_target_path_or_argv_value`
/// in that crate) — reaching one of those arms means a registry entry
/// itself is wrong, not that a client gave a bad value, so this panics
/// rather than inventing an answer for a case the registry promises never
/// to produce.
fn accepts(kind: &ValueKind, value: &str) -> bool {
    match kind {
        ValueKind::Text => true,
        ValueKind::Choice(words) => words.iter().any(|word| word == value),
        ValueKind::Integer => value.parse::<i64>().is_ok(),
        ValueKind::Boolean => value == "true" || value == "false",
        ValueKind::Target | ValueKind::Path | ValueKind::Argv => {
            unreachable!("a setting's declared kind is never target, path or argv")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Settings;
    use crate::error::Error;

    #[test]
    fn a_fresh_store_reports_every_settings_own_default() {
        let settings = Settings::new();
        assert_eq!(settings.value_of("appearance").unwrap(), "system");
    }

    #[test]
    fn set_changes_the_value_the_next_read_sees() {
        let mut settings = Settings::new();
        settings.set("appearance", "light").unwrap();
        assert_eq!(settings.value_of("appearance").unwrap(), "light");
    }

    #[test]
    fn set_rejects_an_unknown_setting_name() {
        let mut settings = Settings::new();
        assert_eq!(
            settings.set("nonsense", "anything"),
            Err(Error::UnknownSetting("nonsense".to_string()))
        );
    }

    #[test]
    fn set_rejects_a_value_outside_the_closed_set() {
        let mut settings = Settings::new();
        assert_eq!(
            settings.set("appearance", "neon"),
            Err(Error::InvalidSettingValue {
                name: "appearance".to_string(),
                value: "neon".to_string(),
            })
        );
    }

    #[test]
    fn a_rejected_set_changes_nothing() {
        let mut settings = Settings::new();
        assert!(settings.set("appearance", "neon").is_err());
        assert_eq!(settings.value_of("appearance").unwrap(), "system");
    }

    #[test]
    fn unset_after_set_reverts_to_the_default() {
        let mut settings = Settings::new();
        settings.set("appearance", "dark").unwrap();
        settings.unset("appearance").unwrap();
        assert_eq!(settings.value_of("appearance").unwrap(), "system");
    }

    #[test]
    fn unsetting_a_setting_with_no_override_is_not_an_error_and_changes_nothing() {
        let mut settings = Settings::new();
        assert_eq!(settings.unset("appearance"), Ok(()));
        assert_eq!(settings.value_of("appearance").unwrap(), "system");
    }

    #[test]
    fn unset_still_rejects_an_unknown_setting_name() {
        let mut settings = Settings::new();
        assert_eq!(
            settings.unset("nonsense"),
            Err(Error::UnknownSetting("nonsense".to_string()))
        );
    }

    #[test]
    fn value_of_rejects_an_unknown_setting_name() {
        let settings = Settings::new();
        assert_eq!(
            settings.value_of("nonsense"),
            Err(Error::UnknownSetting("nonsense".to_string()))
        );
    }
}
