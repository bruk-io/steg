//! The settings the server remembers (R80, decision-9): the closed
//! registry of names, so the CLI, a future app and the docs all read the
//! same list rather than keeping their own copy (doc-16 R105).
//!
//! This is the shape alone: a name, the kind of value it takes, and its
//! default. Remembering an override and validating a value against its
//! kind is kernel behaviour (`steg_kernel`'s `setting` module), not here —
//! the same split `colour.rs` draws between the five colours (the shape)
//! and which one a new session takes (the kernel's rule).
//!
//! A setting's own `value` is restricted, by convention, to `Text`,
//! `Choice`, `Integer` or `Boolean`: the kind of thing a value given
//! inline can be. `Target`, `Path` and `Argv` describe something resolved
//! or run, never a setting's own value, and the kernel's `accepts` match
//! over `ValueKind` treats choosing one of those three for a registry
//! entry as a bug in the registry, not a client's mistake.

use crate::description::ValueKind;

/// One entry in the registry: a name, the kind of value it takes, its
/// default, and one line describing it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Setting {
    /// The setting's name, as it crosses the API (`setting.show`'s
    /// target, `setting.set`'s target, doc-2 section 8).
    pub name: &'static str,
    /// What kind of value it takes.
    pub value: ValueKind,
    /// Its value with no override, as its own wire text (every setting's
    /// value is `Text` on the wire — see the module doc).
    pub default: &'static str,
    /// One line describing it.
    pub summary: &'static str,
}

/// Every setting steg remembers, in the first release.
///
/// One entry today: `appearance` (R24), seeded here as the first concrete
/// instance of R80's generic "a setting is changed from the palette" — no
/// accepted document fixes this exact wire name, so it is this module's
/// own choice, cheap to change in one place if it isn't the right one.
/// R23's font settings (size, line height, ligatures, fallback fonts) are
/// the obvious next entries, added by whichever task first needs them.
#[must_use]
pub fn settings() -> Vec<Setting> {
    vec![Setting {
        name: "appearance",
        value: ValueKind::Choice(vec!["system".into(), "light".into(), "dark".into()]),
        default: "system",
        summary: "Whether the app follows the system's light or dark setting, or is \
                  pinned to one (R24)",
    }]
}

#[cfg(test)]
mod tests {
    use super::settings;
    use crate::description::ValueKind;

    #[test]
    fn appearance_is_a_closed_choice_of_system_light_and_dark_defaulting_to_system() {
        let appearance = settings()
            .into_iter()
            .find(|setting| setting.name == "appearance")
            .expect("appearance is a registered setting");

        assert_eq!(
            appearance.value,
            ValueKind::Choice(vec!["system".into(), "light".into(), "dark".into()])
        );
        assert_eq!(appearance.default, "system");
        assert!(!appearance.summary.is_empty());
    }

    #[test]
    fn every_name_is_unique() {
        use std::collections::HashSet;

        let names: Vec<&str> = settings().iter().map(|setting| setting.name).collect();
        let unique: HashSet<&str> = names.iter().copied().collect();
        assert_eq!(names.len(), unique.len(), "a duplicate setting name");
    }

    /// The convention the module doc states: no registry entry declares a
    /// value kind that only makes sense for a target, a path or a
    /// program's arguments.
    #[test]
    fn no_setting_declares_a_target_path_or_argv_value() {
        for setting in settings() {
            assert!(
                !matches!(
                    setting.value,
                    ValueKind::Target | ValueKind::Path | ValueKind::Argv
                ),
                "{} declares {:?}, which is not a value a setting can hold inline",
                setting.name,
                setting.value
            );
        }
    }
}
