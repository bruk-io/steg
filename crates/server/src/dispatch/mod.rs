//! Dispatching a request to the kernel, over the API's description
//! (TASK-1.75; doc-8 R36, R37; doc-16 R98, R101; decision-1).
//!
//! [`dispatch`] is a plain function from a [`steg_sdk::Request`] and a
//! [`Context`] to a [`steg_sdk::Response`], over a [`steg_kernel::Tree`]
//! and the server's remembered [`steg_kernel::Settings`] (R80, decision-9;
//! TASK-1.48) — no socket, no I/O, in its signature: both are already in
//! memory, and the request is already parsed. The server's own remaining
//! job (TASK-1.44/1.45) is building a `Context` from a connection's
//! `STEG_SESSION`/`STEG_TAB`/`STEG_PANE` and calling this function
//! unchanged.

mod api;
mod kernel_error;
mod lookup;
pub(crate) mod outcome;
mod pane;
mod session;
mod setting;
mod tab;

pub(crate) use api::resolve as resolve_api_call;
pub(crate) use kernel_error::map_kernel_error;
pub(crate) use lookup::{
    find_tab, pane_mut, param_bool, param_i64, param_present, param_str, resolve_pane_str,
    resolve_tab_str, tab_mut, target_pane, target_session, target_tab, wire_position_to_index,
};
pub use outcome::Outcome;

use steg_kernel::{Settings, Tree};
use steg_sdk::{
    ApiError, Description, ErrorCode, PaneId, Request, Response, SessionId, TabId, validate,
};

/// The defaults a request's target falls back to when its own `target`
/// parameter is left out (doc-2 section 4's context fallback), passed in
/// explicitly rather than read off a connection — there is no connection
/// yet (TASK-1.44/1.45), and the kernel's own `Tree::resolve_tab_path`/
/// `resolve_pane_path` already take their defaults the same way.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Context {
    /// The session a request with no session part of its target acts on.
    pub default_session: SessionId,
    /// The tab a request with no tab part of its target acts on.
    pub default_tab: TabId,
    /// The pane a request with no pane part of its target acts on.
    pub default_pane: PaneId,
}

/// Every `(noun, verb)` pair this task dispatches, built from every
/// per-noun `HANDLED` — the method set this task actually matches on,
/// checked against `description().methods` both ways by this module's own
/// tests (AC #2). Used only by that test: the match in [`dispatch`] below
/// handles `server`/`event`/`client` inline, so nothing in the production
/// path reads this list.
#[cfg(test)]
fn handled() -> Vec<(&'static str, &'static str)> {
    let mut all = Vec::new();
    all.extend(session::HANDLED.iter().copied());
    all.extend(tab::HANDLED.iter().copied());
    all.extend(pane::HANDLED.iter().copied());
    all.extend(api::HANDLED.iter().copied());
    all.extend(setting::HANDLED.iter().copied());
    all.extend(NOT_YET_NOUNS.iter().copied());
    all
}

/// The nouns with no served arm at all in this task (`server`, `event`,
/// `client`): every one of their methods answers
/// `ErrorCode::NotImplementedYet`, naming what it waits on. Test-only, for
/// the same reason as [`handled`].
#[cfg(test)]
const NOT_YET_NOUNS: &[(&str, &str)] = &[
    ("server", "show"),
    ("server", "start"),
    ("server", "stop"),
    ("event", "watch"),
    ("client", "list"),
    ("client", "show"),
];

/// Builds the `ApiError` a not-yet-built method or branch answers with
/// (AC #2's second half): a distinct code naming what it waits on, never
/// an invented value.
pub(crate) fn not_implemented_yet(waiting_on: &str) -> ApiError {
    ApiError::new(
        ErrorCode::NotImplementedYet,
        format!("not implemented yet: waiting on {waiting_on}"),
    )
}

/// A `BadRequest` naming `message` — for a malformed value `validate`
/// itself cannot catch (such as `api.call`'s own `method`, checked before
/// the generic validation step runs at all; see `api::call`).
pub(crate) fn bad_request(message: impl Into<String>) -> ApiError {
    ApiError::new(ErrorCode::BadRequest, message.into())
}

/// `ErrorCode::ProgramRunning`, with the message every `pane.close`/
/// `tab.close`/`session.close` refusal in this task uses (TASK-1.76 §5):
/// a program is still running somewhere in scope and `--force` was not
/// given, so nothing was touched. The code's own doc comment, verbatim:
/// "A program is still running and `--force` was not given."
pub(crate) fn program_running() -> ApiError {
    ApiError::new(
        ErrorCode::ProgramRunning,
        "a program is still running; use --force",
    )
}

/// Dispatches one request against `description`, over `tree`, with
/// `context`'s defaults for any target part left out. `settings` is the
/// server's remembered overrides (R80, decision-9; TASK-1.48) — orthogonal
/// to `tree`: a setting's target is its own name, never a session/tab/pane
/// path, so it needs no `Context` either.
///
/// # Panics
///
/// Never in practice: splitting `request.method` on `.` always succeeds
/// once `validate` has already confirmed the method is in `description`
/// (every described method's name is `{noun}.{verb}`, R101), and every
/// described method is matched by one of the per-noun `HANDLED` lists or
/// the three not-yet nouns below, which `mod.rs`'s own completeness test
/// checks against `description()` directly.
#[must_use]
pub fn dispatch(
    tree: &mut Tree,
    settings: &mut Settings,
    description: &Description,
    context: &Context,
    request: &Request,
) -> Response {
    if request.method == "api.call" {
        return api::call(tree, settings, description, context, request);
    }

    if let Err(error) = validate(description, request) {
        return Response::Error(error);
    }

    let noun_verb = request
        .method
        .split_once('.')
        .expect("validate already confirmed this method is in the description");

    let result = match noun_verb {
        (noun, verb) if session::HANDLED.contains(&(noun, verb)) => {
            session::dispatch(tree, context, request, verb)
        }
        (noun, verb) if tab::HANDLED.contains(&(noun, verb)) => {
            tab::dispatch(tree, context, request, verb)
        }
        (noun, verb) if pane::HANDLED.contains(&(noun, verb)) => {
            pane::dispatch(tree, context, request, verb)
        }
        (noun, verb) if api::HANDLED.contains(&(noun, verb)) => {
            api::dispatch(description, request, verb)
        }
        (noun, verb) if setting::HANDLED.contains(&(noun, verb)) => {
            setting::dispatch(settings, request, verb)
        }
        ("server", _) => Err(not_implemented_yet(
            "the server's own socket and lifecycle (TASK-1.44)",
        )),
        ("event", _) => Err(not_implemented_yet("events (TASK-1.46)")),
        ("client", _) => Err(not_implemented_yet("a client model (TASK-1.42)")),
        _ => unreachable!(
            "validate already refused any method not in the description, and every \
             described method is in one of the lists above"
        ),
    };

    match result {
        Ok(outcome) => Response::Ok(outcome.into_value()),
        Err(error) => Response::Error(error),
    }
}

#[cfg(test)]
mod tests {
    use super::{handled, not_implemented_yet};
    use steg_sdk::{ErrorCode, description};

    #[test]
    fn every_described_method_has_exactly_one_handler() {
        let described: Vec<String> = description()
            .methods
            .iter()
            .map(|m| m.name.dotted())
            .collect();
        let handled: Vec<String> = handled()
            .iter()
            .map(|(noun, verb)| format!("{noun}.{verb}"))
            .collect();

        let mut described_sorted = described.clone();
        described_sorted.sort();
        let mut handled_sorted = handled.clone();
        handled_sorted.sort();

        assert_eq!(
            described_sorted, handled_sorted,
            "every described method must have exactly one handler, and every handler must \
             name a described method"
        );
    }

    #[test]
    fn not_implemented_yet_names_what_it_waits_on() {
        let error = not_implemented_yet("the terminal engine (TASK-1.40)");
        assert_eq!(error.code, ErrorCode::NotImplementedYet);
        assert!(error.message.contains("the terminal engine (TASK-1.40)"));
    }
}
