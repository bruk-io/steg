//! One function per shape a parameter commonly takes, so a noun's own
//! method list reads as which parameters it has, not how a `Param`
//! literal is built - shared across every noun file in this module.

use crate::description::shapes::{Field, MethodName, Param, Role, ValueKind, Version};

/// The API this build of steg speaks.
pub(super) const VERSION: Version = Version { major: 1, minor: 0 };

pub(super) fn method(noun: &str, verb: &str) -> MethodName {
    MethodName {
        noun: noun.into(),
        verb: verb.into(),
    }
}

/// A role: Target parameter named `target` (doc-2 section 2: "its target
/// parameter is the positional"), whatever kind of value it takes — a
/// session/tab/pane path (`ValueKind::Target`, resolved by R10's rules) for
/// most methods, or plain text for the two whose target is a method name,
/// not a domain path (`api.show`/`api.call`; a method name is never
/// resolved by R10, so `ValueKind::Target` would be the wrong kind for it
/// even though the role is right).
pub(super) fn target_of_kind(value: ValueKind, required: bool, summary: &str) -> Param {
    Param {
        name: "target".into(),
        role: Role::Target,
        value,
        required,
        summary: summary.into(),
    }
}

pub(super) fn target(summary: &str) -> Param {
    target_of_kind(ValueKind::Target, false, summary)
}

pub(super) fn required_target(summary: &str) -> Param {
    target_of_kind(ValueKind::Target, true, summary)
}

/// Role: Target parameter named `method`, for `api.call`'s own selector
/// only (TASK-1.81) — not the generic `target` convention every other
/// method above uses, because api.call forwards every other parameter it
/// is given flat into the called method's own `params` (`api::call`), and
/// that method may itself declare a parameter literally named `target`
/// (`pane.split`, `setting.show`, ...). Renaming the selector, rather than
/// nesting the forwarded parameters under a second key, is what keeps the
/// two apart: `Method::target()` finds this by `role`, not by `name`, so
/// it is still the CLI's one positional (TASK-1.49) despite the name.
pub(super) fn method_target(summary: &str) -> Param {
    Param {
        name: "method".into(),
        role: Role::Target,
        value: ValueKind::Text,
        required: true,
        summary: summary.into(),
    }
}

pub(super) fn flag(name: &str, value: ValueKind, required: bool, summary: &str) -> Param {
    Param {
        name: name.into(),
        role: Role::Flag,
        value,
        required,
        summary: summary.into(),
    }
}

pub(super) fn switch(name: &str, summary: &str) -> Param {
    Param {
        name: name.into(),
        role: Role::Switch,
        value: ValueKind::Boolean,
        required: false,
        summary: summary.into(),
    }
}

pub(super) fn program() -> Param {
    Param {
        name: "program".into(),
        role: Role::Program,
        value: ValueKind::Argv,
        required: false,
        summary: "The program to run, and its arguments. Defaults to the login shell".into(),
    }
}

pub(super) fn direction() -> ValueKind {
    ValueKind::Choice(vec![
        "left".into(),
        "right".into(),
        "up".into(),
        "down".into(),
    ])
}

pub(super) fn field(name: &str, value: ValueKind, summary: &str) -> Field {
    Field {
        name: name.into(),
        value,
        summary: summary.into(),
    }
}
