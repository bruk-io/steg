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

/// Every pane `request`'s own target covers (TASK-1.78 §5): one pane for
/// `pane.close`, a tab's panes for `tab.close`, a session's tabs' panes
/// for `session.close` - the exact three lookups each of those methods'
/// own `any_running` refusal check already needed, pulled out once here
/// so `Runtime`'s force-close wrapper and those checks share one
/// implementation rather than three copies that could drift apart. Takes
/// only `request.method`, not a separate noun parameter, so a caller never
/// has to repeat which of the three it already knows it is calling.
///
/// # Errors
///
/// Returns whatever the target itself fails to resolve with
/// (`PaneNotFound`/`TabNotFound`/`SessionNotFound`, mapped) - the same
/// error any of the three methods' own dispatch arm would give for an
/// unresolvable target.
pub(crate) fn panes_in_close_scope(
    tree: &Tree,
    context: &Context,
    request: &Request,
) -> Result<Vec<PaneId>, ApiError> {
    match request.method.as_str() {
        "pane.close" => Ok(vec![target_pane(tree, context, request)?]),
        "tab.close" => {
            let tab = target_tab(tree, context, request)?;
            Ok(find_tab(tree, tab)
                .ok_or_else(|| map_kernel_error(steg_kernel::Error::TabNotFound(tab)))?
                .panes()
                .iter()
                .map(Pane::id)
                .collect())
        }
        "session.close" => {
            let session = target_session(tree, context, request)?;
            Ok(tree
                .session(session)
                .ok_or_else(|| map_kernel_error(steg_kernel::Error::SessionNotFound(session)))?
                .tabs()
                .iter()
                .flat_map(Tab::panes)
                .map(Pane::id)
                .collect())
        }
        other => unreachable!(
            "panes_in_close_scope is only ever called for the three close methods, not {other}"
        ),
    }
}

/// Whether any of `panes` has a program running right now - the one check
/// `pane.close`/`tab.close`/`session.close`'s own refusal (without
/// `--force`) needs, over whatever [`panes_in_close_scope`] found. A pane
/// id that cannot be looked up (never expected - these are only ever ids
/// `panes_in_close_scope` itself just read straight out of the same
/// `tree`) counts as not running, not an error: this function only ever
/// answers "is there something to refuse for", and a pane that has
/// vanished has nothing to refuse.
pub(crate) fn any_pane_running(tree: &Tree, panes: &[PaneId]) -> bool {
    panes.iter().any(|&pane| {
        tree.tab_of_pane(pane)
            .and_then(|tab_id| find_tab(tree, tab_id))
            .and_then(|tab| tab.pane(pane))
            .is_some_and(|pane| {
                matches!(
                    pane.program_status(),
                    Some(steg_sdk::ProgramStatus::Running)
                )
            })
    })
}

/// Resolves a session from a target path given as a bare string. Only
/// `target_session` (below) calls this today - no method's own flag is a
/// session target, unlike `resolve_tab_str`/`resolve_pane_str`, which
/// `tab.move`'s `to` and `pane.swap`'s `with` both reach through
/// `super::resolve_tab_str`/`super::resolve_pane_str` from other modules.
fn resolve_session_str(tree: &Tree, part: &str) -> Result<SessionId, ApiError> {
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
}
