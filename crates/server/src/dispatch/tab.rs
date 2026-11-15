//! The `tab` noun's dispatch arms (doc-2 section 8).

use steg_kernel::{Error, Tree};
use steg_sdk::{ApiError, Request};

use super::outcome::Outcome;
use super::{
    Context, map_kernel_error, not_implemented_yet, param_bool, param_i64, param_present,
    param_str, tab_mut, target_session, target_tab, wire_position_to_index,
};

/// Every `tab.*` method this module matches on, checked against
/// `description().methods` by `mod.rs`'s own completeness test.
pub(super) const HANDLED: &[(&str, &str)] = &[
    ("tab", "list"),
    ("tab", "show"),
    ("tab", "new"),
    ("tab", "close"),
    ("tab", "move"),
    ("tab", "set"),
    ("tab", "unset"),
    ("tab", "focus"),
];

pub(super) fn dispatch(
    tree: &mut Tree,
    context: &Context,
    request: &Request,
    verb: &str,
) -> Result<Outcome, ApiError> {
    match verb {
        "list" => Err(not_implemented_yet("title, shown_status (TASK-1.39, 1.41)")),
        "show" => Err(not_implemented_yet("title, shown_status (TASK-1.39, 1.41)")),
        "new" => new(tree, context, request),
        "close" => close(tree, context, request),
        "move" => move_(tree, context, request),
        "set" => set(tree, context, request),
        "unset" => unset(tree, context, request),
        "focus" => focus(tree, context, request),
        _ => unreachable!("HANDLED names every verb this match covers"),
    }
}

fn new(tree: &mut Tree, context: &Context, request: &Request) -> Result<Outcome, ApiError> {
    // `tab.new`'s own target is the *session* the tab is created in
    // (doc-2's "The session to create the tab in"), not a tab.
    let session_id = target_session(tree, context, request)?;

    if param_present(request, "cwd") || param_present(request, "program") {
        return Err(not_implemented_yet(
            "a pane's program and working directory (TASK-1.40)",
        ));
    }

    let label = param_str(request, "label").map(str::to_string);
    let focus = param_bool(request, "focus").unwrap_or(true);

    // Captured before `new_tab` unconditionally makes the new tab current
    // (R115), so a `--no-focus` call can put it back afterward.
    let previous_tab = tree
        .session(session_id)
        .ok_or_else(|| map_kernel_error(Error::SessionNotFound(session_id)))?
        .current_tab();

    let tab_id = tree.new_tab(session_id, label).map_err(map_kernel_error)?;

    if !focus {
        tree.focus_tab(previous_tab).map_err(map_kernel_error)?;
    }

    Ok(Outcome::Tab(tab_id))
}

fn close(tree: &mut Tree, context: &Context, request: &Request) -> Result<Outcome, ApiError> {
    let id = target_tab(tree, context, request)?;
    tree.close_tab(id).map_err(map_kernel_error)?;
    Ok(Outcome::Nothing)
}

fn move_(tree: &mut Tree, context: &Context, request: &Request) -> Result<Outcome, ApiError> {
    let id = target_tab(tree, context, request)?;

    if param_present(request, "to") {
        // `tab.move --to` moves a tab between sessions; `Tree` has no
        // operation for that today. Checked first, before `position`, so
        // a request that gives both never silently does only half of
        // what it asked for.
        return Err(not_implemented_yet(
            "a kernel operation to move a tab between sessions - Tree has none",
        ));
    }

    if let Some(position) = param_i64(request, "position") {
        let index = wire_position_to_index(position)?;
        tree.move_tab(id, index).map_err(map_kernel_error)?;
    }

    Ok(Outcome::Nothing)
}

fn set(tree: &mut Tree, context: &Context, request: &Request) -> Result<Outcome, ApiError> {
    let id = target_tab(tree, context, request)?;
    if let Some(label) = param_str(request, "label") {
        tab_mut(tree, id)?
            .set_label(Some(label.to_string()))
            .map_err(map_kernel_error)?;
    }
    Ok(Outcome::Nothing)
}

fn unset(tree: &mut Tree, context: &Context, request: &Request) -> Result<Outcome, ApiError> {
    let id = target_tab(tree, context, request)?;
    if param_present(request, "label") {
        tab_mut(tree, id)?
            .set_label(None)
            .map_err(map_kernel_error)?;
    }
    Ok(Outcome::Nothing)
}

fn focus(tree: &mut Tree, context: &Context, request: &Request) -> Result<Outcome, ApiError> {
    let id = target_tab(tree, context, request)?;
    tree.focus_tab(id).map_err(map_kernel_error)?;
    Ok(Outcome::Nothing)
}
