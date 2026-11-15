//! Dispatching a request to the kernel, over the API's description
//! (TASK-1.75; doc-8 R36, R37; doc-16 R98, R101; decision-1).
//!
//! [`dispatch`] is a plain function from a [`steg_sdk::Request`] and a
//! [`Context`] to a [`steg_sdk::Response`], over a [`steg_kernel::Tree`] —
//! no socket, no I/O, in its signature: the tree is already in memory, and
//! the request is already parsed. The server's own remaining job
//! (TASK-1.44/1.45) is building a `Context` from a connection's
//! `STEG_SESSION`/`STEG_TAB`/`STEG_PANE` and calling this function
//! unchanged.

mod api;
mod kernel_error;
pub(crate) mod outcome;
mod pane;
mod session;
mod tab;

pub(crate) use kernel_error::map_kernel_error;
pub use outcome::Outcome;

use serde_json::Value;
use steg_kernel::{Pane, Session, Tab, Tree};
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
/// itself cannot catch (such as `api.call`'s own `target`, checked before
/// the generic validation step runs at all; see `api::call`).
pub(crate) fn bad_request(message: impl Into<String>) -> ApiError {
    ApiError::new(ErrorCode::BadRequest, message.into())
}

/// `request.params[name]` as a string, if present and a string.
pub(crate) fn param_str<'a>(request: &'a Request, name: &str) -> Option<&'a str> {
    request.params.get(name).and_then(Value::as_str)
}

/// `request.params[name]` as a boolean, if present and a boolean.
pub(crate) fn param_bool(request: &Request, name: &str) -> Option<bool> {
    request.params.get(name).and_then(Value::as_bool)
}

/// `request.params[name]` as an integer, if present and an integer.
pub(crate) fn param_i64(request: &Request, name: &str) -> Option<i64> {
    request.params.get(name).and_then(Value::as_i64)
}

/// Whether `request.params[name]` is present at all (used for `program`,
/// an `Argv` array, and `cwd`, a `Path` string — this task only needs to
/// know whether either was given, not to read its value, since both
/// branches it guards refuse before touching the kernel).
pub(crate) fn param_present(request: &Request, name: &str) -> bool {
    request.params.contains_key(name)
}

/// `request.params["target"]`, as a string, if present — the method's one
/// positional (doc-2 section 2).
fn target_str(request: &Request) -> Option<&str> {
    param_str(request, "target")
}

/// Resolves `request`'s `target` as a session, falling back to
/// `context.default_session` when it is left out.
pub(crate) fn target_session(
    tree: &Tree,
    context: &Context,
    request: &Request,
) -> Result<SessionId, ApiError> {
    match target_str(request) {
        Some(part) => resolve_session_str(tree, part),
        None => Ok(context.default_session),
    }
}

/// Resolves `request`'s `target` as a tab, falling back to
/// `context.default_tab` when it is left out.
pub(crate) fn target_tab(
    tree: &Tree,
    context: &Context,
    request: &Request,
) -> Result<TabId, ApiError> {
    match target_str(request) {
        Some(part) => resolve_tab_str(tree, context, part),
        None => Ok(context.default_tab),
    }
}

/// Resolves `request`'s `target` as a pane, falling back to
/// `context.default_pane` when it is left out.
pub(crate) fn target_pane(
    tree: &Tree,
    context: &Context,
    request: &Request,
) -> Result<PaneId, ApiError> {
    match target_str(request) {
        Some(part) => resolve_pane_str(tree, context, part),
        None => Ok(context.default_pane),
    }
}

/// Resolves a session from a target path given as a bare string — for a
/// flag whose own value is a target (`value: ValueKind::Target`), such as
/// `tab.move`'s `to`, not the method's own positional.
pub(crate) fn resolve_session_str(tree: &Tree, part: &str) -> Result<SessionId, ApiError> {
    tree.resolve_session(part).map_err(map_kernel_error)
}

/// Resolves a tab from a target path given as a bare string.
pub(crate) fn resolve_tab_str(
    tree: &Tree,
    context: &Context,
    part: &str,
) -> Result<TabId, ApiError> {
    tree.resolve_tab_path(part, context.default_session)
        .map_err(map_kernel_error)
}

/// Resolves a pane from a target path given as a bare string.
pub(crate) fn resolve_pane_str(
    tree: &Tree,
    context: &Context,
    part: &str,
) -> Result<PaneId, ApiError> {
    tree.resolve_pane_path(part, context.default_session, context.default_tab)
        .map_err(map_kernel_error)
}

/// The session that holds this tab, found by searching every session's
/// own tabs (`Session::tabs`/`Tab::id`, both already public) — the same
/// read-only-lookup-across-sessions shape as `Tree::tab_of_pane`, for the
/// one direction it does not cover: a tab's label can only be changed
/// through its owning `Session`, and `Tree` exposes no public way to find
/// which session that is from a bare `TabId`.
pub(crate) fn session_owning_tab(tree: &Tree, tab: TabId) -> Option<SessionId> {
    tree.sessions()
        .iter()
        .find(|session| session.tab(tab).is_some())
        .map(Session::id)
}

/// A read-only reference to the tab with this id, wherever it is.
pub(crate) fn find_tab(tree: &Tree, tab: TabId) -> Option<&Tab> {
    tree.sessions().iter().find_map(|session| session.tab(tab))
}

/// A mutable reference to the tab with this id, by way of its owning
/// session.
pub(crate) fn tab_mut(tree: &mut Tree, tab: TabId) -> Result<&mut Tab, ApiError> {
    let session_id = session_owning_tab(tree, tab)
        .ok_or_else(|| map_kernel_error(steg_kernel::Error::TabNotFound(tab)))?;
    let session = tree
        .session_mut(session_id)
        .expect("session_owning_tab just found this tab in this session");
    session
        .tab_mut(tab)
        .ok_or_else(|| map_kernel_error(steg_kernel::Error::TabNotFound(tab)))
}

/// A mutable reference to the pane with this id, by way of its owning
/// tab (`Tree::tab_of_pane`) and that tab's owning session.
pub(crate) fn pane_mut(tree: &mut Tree, pane: PaneId) -> Result<&mut Pane, ApiError> {
    let tab = tree
        .tab_of_pane(pane)
        .ok_or_else(|| map_kernel_error(steg_kernel::Error::PaneNotFound(pane)))?;
    tab_mut(tree, tab)?
        .pane_mut(pane)
        .ok_or_else(|| map_kernel_error(steg_kernel::Error::PaneNotFound(pane)))
}

/// Converts doc-15 R10/R112's 1-based wire position to `Tree::move_session`/
/// `move_tab`'s 0-based index. `0` or negative is a `BadRequest`, not a
/// kernel call: the boundary this task's own plan flags as a one-line bug
/// risk, so it gets its own named function and its own tests rather than
/// being folded inline into each caller.
pub(crate) fn wire_position_to_index(position: i64) -> Result<usize, ApiError> {
    if position < 1 {
        return Err(bad_request("position must be 1 or greater"));
    }
    #[expect(
        clippy::cast_sign_loss,
        reason = "position is checked >= 1 immediately above"
    )]
    Ok((position - 1) as usize)
}

/// Dispatches one request against `description`, over `tree`, with
/// `context`'s defaults for any target part left out.
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
    description: &Description,
    context: &Context,
    request: &Request,
) -> Response {
    if request.method == "api.call" {
        return api::call(tree, description, context, request);
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
    use super::{handled, not_implemented_yet, wire_position_to_index};
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

    #[test]
    fn wire_position_converts_one_based_to_zero_based() {
        assert_eq!(wire_position_to_index(1), Ok(0));
        assert_eq!(wire_position_to_index(2), Ok(1));
    }

    #[test]
    fn wire_position_rejects_zero_and_negative_without_a_kernel_call() {
        assert_eq!(
            wire_position_to_index(0).unwrap_err().code,
            ErrorCode::BadRequest
        );
        assert_eq!(
            wire_position_to_index(-1).unwrap_err().code,
            ErrorCode::BadRequest
        );
    }
}
