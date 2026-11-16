//! Reading a request's parameters, and resolving a target to a kernel id
//! (TASK-1.76 §7): moved out of `mod.rs`, which was starting to hold both
//! these shared, generic lookups and the routing `dispatch` function
//! itself, against `.claude/rules/rust.md`. Re-exported from `mod.rs` at
//! the same `pub(crate)` visibility, so every other module's own
//! `use super::{...}` import is unchanged.

use serde_json::Value;
use steg_kernel::{Pane, Session, Tab, Tree};
use steg_sdk::{ApiError, PaneId, Request, SessionId, TabId};

use super::{Context, bad_request, map_kernel_error};

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

/// Refused when a request gives no `target` at all, or a partial one that
/// omits a part this connection has no default for (TASK-1.45 §1): bad
/// usage (doc-2 section 6's status 2), not "matched nothing" - nothing was
/// even looked up, since there was no id to look up with.
fn missing_context(kind: &str) -> ApiError {
    bad_request(format!(
        "no target given, and this connection has no {kind} context to fall back on"
    ))
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
        None => context
            .default_session
            .ok_or_else(|| missing_context("session")),
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
        None => context.default_tab.ok_or_else(|| missing_context("tab")),
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
        None => context.default_pane.ok_or_else(|| missing_context("pane")),
    }
}

/// Resolves a session from a target path given as a bare string. Only
/// `target_session` (below) calls this today - no method's own flag is a
/// session target, unlike `resolve_tab_str`/`resolve_pane_str`, which
/// `tab.move`'s `to` and `pane.swap`'s `with` both reach through
/// `super::resolve_tab_str`/`super::resolve_pane_str` from other modules.
fn resolve_session_str(tree: &Tree, part: &str) -> Result<SessionId, ApiError> {
    tree.resolve_session(part).map_err(map_kernel_error)
}

/// Whether `part` splits (on `/`) into exactly `n` segments, none of them
/// empty - doc-2 section 4's own target grammar, structural only (never
/// which *kind* of match a segment is: a full identifier, a label, a
/// position, or an identifier suffix, which stays entirely the kernel's
/// own job in `Tree::resolve_session`/`Session::resolve_tab`/
/// `Tab::resolve_pane`). Used only to decide, before any kernel call,
/// whether a default this connection might lack would actually be read -
/// a review of this task's plan preferred deciding the missing part
/// before the kernel is called, over passing a placeholder id and
/// remapping its error, and this is knowable from the path's own shape
/// and the context alone, with no kernel call needed to know it.
fn segments_exactly(part: &str, n: usize) -> bool {
    let parts: Vec<&str> = part.split('/').collect();
    parts.len() == n && parts.iter().all(|segment| !segment.is_empty())
}

/// Resolves a tab from a target path given as a bare string. doc-2
/// section 4's own grammar, `"[session/]tab"`: the session part is
/// omitted only at exactly one well-formed segment, which is the only
/// shape that ever reads `context.default_session` at all - confirmed by
/// reading `Tree::resolve_tab_path`'s own `match` on the parsed path.
/// Anything else (two well-formed segments, so the session is given
/// explicitly; or a malformed path the kernel's own `split_path` refuses
/// before ever touching the default) passes a value that is never read,
/// so its absence never matters there - `SessionId::from_raw(0)` is as
/// good a value as any for a branch proven not to read it.
pub(crate) fn resolve_tab_str(
    tree: &Tree,
    context: &Context,
    part: &str,
) -> Result<TabId, ApiError> {
    let default_session = if segments_exactly(part, 1) {
        context
            .default_session
            .ok_or_else(|| missing_context("session"))?
    } else {
        context.default_session.unwrap_or(SessionId::from_raw(0))
    };
    tree.resolve_tab_path(part, default_session)
        .map_err(map_kernel_error)
}

/// Resolves a pane from a target path given as a bare string. doc-2
/// section 4's own grammar, `"[session/][tab/]pane"`: the session part is
/// given only at exactly three well-formed segments (so `default_session`
/// is read at one or two segments), and the tab part only at two or three
/// (so `default_tab` is read only at exactly one) - the same reasoning
/// [`resolve_tab_str`] already gives, confirmed the same way by reading
/// `Tree::resolve_pane_path`'s own `match`.
pub(crate) fn resolve_pane_str(
    tree: &Tree,
    context: &Context,
    part: &str,
) -> Result<PaneId, ApiError> {
    let default_session = if segments_exactly(part, 1) || segments_exactly(part, 2) {
        context
            .default_session
            .ok_or_else(|| missing_context("session"))?
    } else {
        context.default_session.unwrap_or(SessionId::from_raw(0))
    };
    let default_tab = if segments_exactly(part, 1) {
        context.default_tab.ok_or_else(|| missing_context("tab"))?
    } else {
        context.default_tab.unwrap_or(TabId::from_raw(0))
    };
    tree.resolve_pane_path(part, default_session, default_tab)
        .map_err(map_kernel_error)
}

/// The session that holds this tab, found by searching every session's
/// own tabs (`Session::tabs`/`Tab::id`, both already public) — the same
/// read-only-lookup-across-sessions shape as `Tree::tab_of_pane`, for the
/// one direction it does not cover: a tab's label can only be changed
/// through its owning `Session`, and `Tree` exposes no public way to find
/// which session that is from a bare `TabId`. Only `tab_mut` (below)
/// calls this today.
fn session_owning_tab(tree: &Tree, tab: TabId) -> Option<SessionId> {
    tree.sessions()
        .iter()
        .find(|session| session.tab(tab).is_some())
        .map(Session::id)
}

/// A read-only reference to the tab with this id, wherever it is.
pub(crate) fn find_tab(tree: &Tree, tab: TabId) -> Option<&Tab> {
    tree.sessions().iter().find_map(|session| session.tab(tab))
}

/// `tab`'s one-based position among its own session's tabs (doc-15 R10;
/// `TabRecord.position`, TASK-1.89) - found the same way
/// [`session_owning_tab`] already does for [`tab_mut`], then the index of
/// `tab` within that session's own `tabs()` order.
pub(crate) fn tab_position(tree: &Tree, tab: TabId) -> Result<usize, ApiError> {
    let session_id = session_owning_tab(tree, tab)
        .ok_or_else(|| map_kernel_error(steg_kernel::Error::TabNotFound(tab)))?;
    let session = tree
        .session(session_id)
        .expect("session_owning_tab just found this tab in this session");
    let index = session
        .tabs()
        .iter()
        .position(|t| t.id() == tab)
        .expect("session_owning_tab just confirmed this session holds this tab");
    Ok(index + 1)
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

#[cfg(test)]
mod tests {
    use super::wire_position_to_index;
    use steg_sdk::ErrorCode;

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

    // --- Context::none() and the missing-context refusal (TASK-1.45 §1) --

    use super::{resolve_pane_str, resolve_tab_str, target_pane, target_session, target_tab};
    use crate::dispatch::Context;
    use serde_json::{Map, json};
    use steg_kernel::Tree;
    use steg_sdk::Request;

    /// A tree with one real session, tab and pane - `Tree::new`'s own
    /// deterministic ids - so a test can tell "the real one" apart from
    /// `Context::none()`'s own absence.
    fn tree_with_one_of_each() -> (Tree, super::SessionId, super::TabId, super::PaneId) {
        let mut tree = Tree::new();
        let session = tree.new_session(None).unwrap();
        let tab = tree.session(session).unwrap().tabs()[0].id();
        let pane = tree.session(session).unwrap().tabs()[0].panes()[0].id();
        (tree, session, tab, pane)
    }

    fn request_with_target(target: &str) -> Request {
        let mut params = Map::new();
        params.insert("target".to_string(), json!(target));
        Request {
            method: "irrelevant.for.this.test".to_string(),
            params,
        }
    }

    fn request_with_no_target() -> Request {
        Request {
            method: "irrelevant.for.this.test".to_string(),
            params: Map::new(),
        }
    }

    #[test]
    fn a_target_less_request_from_context_none_is_bad_request_for_every_part() {
        let (tree, _session, _tab, _pane) = tree_with_one_of_each();
        let context = Context::none();
        let request = request_with_no_target();

        assert_eq!(
            target_session(&tree, &context, &request).unwrap_err().code,
            ErrorCode::BadRequest
        );
        assert_eq!(
            target_tab(&tree, &context, &request).unwrap_err().code,
            ErrorCode::BadRequest
        );
        assert_eq!(
            target_pane(&tree, &context, &request).unwrap_err().code,
            ErrorCode::BadRequest
        );
    }

    /// The negative control for the test above, read the other way: a real
    /// default (not `Context::none()`) is still answered, unaffected -
    /// pins that the refusal is about *absence*, not about target-less
    /// requests in general.
    #[test]
    fn a_target_less_request_with_a_real_context_still_resolves() {
        let (tree, session, tab, pane) = tree_with_one_of_each();
        let context = Context {
            default_session: Some(session),
            default_tab: Some(tab),
            default_pane: Some(pane),
        };
        let request = request_with_no_target();

        assert_eq!(target_session(&tree, &context, &request), Ok(session));
        assert_eq!(target_tab(&tree, &context, &request), Ok(tab));
        assert_eq!(target_pane(&tree, &context, &request), Ok(pane));
    }

    #[test]
    fn a_fully_explicit_target_resolves_from_context_none_unaffected() {
        let (tree, session, tab, _pane) = tree_with_one_of_each();
        let context = Context::none();
        let full_path = format!("{session}/{tab}");

        assert_eq!(resolve_tab_str(&tree, &context, &full_path), Ok(tab));
    }

    /// The same claim, one level further: a fully explicit pane path
    /// (session, tab and pane all named) never touches either sentinel,
    /// even with no context at all.
    #[test]
    fn a_fully_explicit_pane_target_resolves_from_context_none_unaffected() {
        let (tree, session, tab, pane) = tree_with_one_of_each();
        let context = Context::none();
        let full_path = format!("{session}/{tab}/{pane}");

        assert_eq!(resolve_pane_str(&tree, &context, &full_path), Ok(pane));
    }

    /// A partial target (just the tab) that omits the session part it
    /// would need, from a connection with no session context: refused the
    /// same way as giving no target at all, not "session not found" about
    /// a number nobody named.
    #[test]
    fn a_partial_tab_target_omitting_the_session_part_from_context_none_is_bad_request() {
        let (tree, _session, tab, _pane) = tree_with_one_of_each();
        let context = Context::none();

        let error = resolve_tab_str(&tree, &context, &tab.to_string()).unwrap_err();
        assert_eq!(error.code, ErrorCode::BadRequest);
    }

    /// The decision is made from the path's own shape before the kernel
    /// is ever called (no sentinel id exists to leak, by construction -
    /// confirmed here rather than only argued): a path with too many
    /// segments for a tab (three) is refused by the kernel's own
    /// `InvalidTargetPath`, never `missing_context`'s `BadRequest` about
    /// an absent default - even from a `Context::none()` connection,
    /// where naively treating "no default" as the reason for any failure
    /// would have given the wrong answer.
    #[test]
    fn a_malformed_tab_target_from_context_none_is_the_kernels_own_error_not_missing_context() {
        let (tree, session, tab, pane) = tree_with_one_of_each();
        let context = Context::none();

        let path = format!("{session}/{tab}/{pane}");
        let error = resolve_tab_str(&tree, &context, &path).unwrap_err();
        // InvalidTargetPath also maps to BadRequest (kernel_error.rs), the
        // same code missing_context uses - the code alone cannot tell
        // these apart, so the message is what actually proves this is
        // the kernel's own too-many-segments refusal (its own exact
        // Display text, `error.rs`), not missing_context's.
        assert_eq!(error.code, ErrorCode::BadRequest);
        assert_eq!(
            error.message,
            format!("{path:?} is not a valid target path")
        );
    }

    /// The pane equivalent, omitting both the session and tab parts (a
    /// bare pane id) with a real `default_session` but no `default_tab` -
    /// the session resolves for real, then the tab lookup is the one that
    /// hits the missing default.
    #[test]
    fn a_partial_pane_target_omitting_the_tab_part_from_context_with_no_tab_is_bad_request() {
        let (tree, session, _tab, pane) = tree_with_one_of_each();
        let context = Context {
            default_session: Some(session),
            default_tab: None,
            default_pane: None,
        };

        let error = resolve_pane_str(&tree, &context, &pane.to_string()).unwrap_err();
        assert_eq!(error.code, ErrorCode::BadRequest);
    }

    /// `target_tab`/`target_pane` themselves (not just the lower-level
    /// `resolve_*_str` they call) still answer a fully-explicit target
    /// correctly from a `Context::none()` connection - the same claim as
    /// the two tests above, through the entry point a dispatch arm
    /// actually calls.
    #[test]
    fn target_tab_and_target_pane_resolve_a_fully_explicit_target_from_context_none() {
        let (tree, session, tab, pane) = tree_with_one_of_each();
        let context = Context::none();

        let tab_request = request_with_target(&format!("{session}/{tab}"));
        assert_eq!(target_tab(&tree, &context, &tab_request), Ok(tab));

        let pane_request = request_with_target(&format!("{session}/{tab}/{pane}"));
        assert_eq!(target_pane(&tree, &context, &pane_request), Ok(pane));
    }

    /// A genuinely wrong, fully-explicit target from a `Context::none()`
    /// connection still gets the real kernel error about the id the caller
    /// actually typed, never the missing-context refusal - the sentinel's
    /// own remapping must not swallow a real `NoSuchTarget`.
    #[test]
    fn a_wrong_but_fully_explicit_target_from_context_none_is_a_real_not_found() {
        let (tree, session, _tab, _pane) = tree_with_one_of_each();
        let context = Context::none();
        let wrong_tab = super::TabId::from_raw(999);

        let error =
            resolve_tab_str(&tree, &context, &format!("{session}/{wrong_tab}")).unwrap_err();
        assert_eq!(error.code, ErrorCode::NoSuchTarget);
    }
}
