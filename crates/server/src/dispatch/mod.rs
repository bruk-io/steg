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
    any_pane_running, find_tab, pane_mut, panes_in_close_scope, param_bool, param_i64,
    param_present, param_str, resolve_pane_str, resolve_tab_str, tab_mut, tab_position,
    target_pane, target_session, target_tab, wire_position_to_index,
};
pub use outcome::Outcome;

use steg_kernel::{Settings, Tree};
use steg_sdk::{
    ApiError, Description, ErrorCode, PaneId, Request, Response, SessionId, TabId, validate,
};

/// The defaults a request's target falls back to when its own `target`
/// parameter is left out (doc-2 section 4's context fallback), passed in
/// explicitly rather than read off a connection. `None` means this
/// connection has no default of that kind at all (TASK-1.45 §1): nothing
/// on the wire supplies a session, tab or pane today - the hello (decision-2
/// §7, doc-14 "Clients") carries only `kind`/`label`, and `STEG_SESSION`/
/// `TAB`/`PANE` (doc-2 §4) are environment variables the server puts into a
/// *pane's own program's* environment, never sent by a client. A `Context`
/// naming a real-looking id nobody gave would be a false statement, so
/// absence is represented, not filled with a placeholder id - see
/// [`Context::none`] and `dispatch/lookup.rs`'s `target_session`/
/// `target_tab`/`target_pane`, which refuse rather than guess when a
/// default is needed and missing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Context {
    /// The session a request with no session part of its target acts on,
    /// or `None` when this connection has no session context at all.
    pub default_session: Option<SessionId>,
    /// The tab a request with no tab part of its target acts on, or `None`.
    pub default_tab: Option<TabId>,
    /// The pane a request with no pane part of its target acts on, or `None`.
    pub default_pane: Option<PaneId>,
}

impl Context {
    /// A connection with nothing to fall back on at all (TASK-1.45 §1):
    /// every target-less request, and every partial target that would need
    /// a default this connection does not have, is refused with
    /// `ErrorCode::BadRequest` (doc-2 section 4/6's own "no context" case
    /// at the CLI, enforced here too) rather than resolved against an
    /// invented value.
    #[must_use]
    pub const fn none() -> Self {
        Self {
            default_session: None,
            default_tab: None,
            default_pane: None,
        }
    }
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
    all.extend(INTERCEPTED_BEFORE_DISPATCH.iter().copied());
    all
}

/// Described, fully built methods that never reach [`dispatch`] at all
/// (TASK-1.47): `connection.rs`'s own read loop intercepts them before
/// falling through to [`Runtime::handle`][crate::runtime::Runtime::handle],
/// because answering them needs a connection's own `Arc<Outbound>` and its
/// own per-connection attachment bookkeeping, neither of which `dispatch`
/// or `Runtime::handle` can reach (both exist only inside `connection.rs`,
/// created after the handshake) - the same structural reason `event.watch`
/// is expected to leave [`NOT_YET_NOUNS`] for this list too, once its own
/// server-side task lands. Test-only, for the same reason as [`handled`].
#[cfg(test)]
const INTERCEPTED_BEFORE_DISPATCH: &[(&str, &str)] = &[("pane", "attach"), ("pane", "detach")];

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
/// described method is matched by one of the per-noun `HANDLED` lists, the
/// three not-yet nouns, or `pane.attach`/`pane.detach`'s own arm
/// (TASK-1.47, built but never reached this way in the real system), which
/// `mod.rs`'s own completeness test checks against `description()` directly.
#[must_use]
pub fn dispatch(
    tree: &mut Tree,
    settings: &mut Settings,
    description: &Description,
    context: &Context,
    request: &Request,
    now: steg_kernel::Moment,
) -> Response {
    if request.method == "api.call" {
        return api::call(tree, settings, description, context, request, now);
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
            tab::dispatch(tree, context, request, verb, now)
        }
        (noun, verb) if pane::HANDLED.contains(&(noun, verb)) => {
            pane::dispatch(tree, context, request, verb, now)
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
        ("pane", "attach" | "detach") => Err(not_implemented_yet(
            "connection.rs's own interception (TASK-1.47) - a described method, fully built, \
             but never reached through dispatch in the real system: answering it needs a \
             connection's own Arc<Outbound> and attachment bookkeeping, which only exist in \
             connection.rs, created after the handshake",
        )),
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
