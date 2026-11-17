//! The `tab` noun's dispatch arms (doc-2 section 8).

use steg_kernel::{Error, Moment, Tree};
use steg_sdk::{ApiError, Request, TabId, TabRecord};

use super::outcome::Outcome;
use super::{
    Context, map_kernel_error, not_implemented_yet, param_bool, param_i64, param_present,
    param_str, tab_mut, tab_position, target_session, target_tab, wire_position_to_index,
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
    now: Moment,
) -> Result<Outcome, ApiError> {
    match verb {
        "list" => list(tree, context, request, now),
        "show" => show(tree, context, request, now),
        "new" => new(tree, context, request),
        "close" => close(tree, context, request),
        "move" => move_(tree, context, request),
        "set" => set(tree, context, request),
        "unset" => unset(tree, context, request),
        "focus" => focus(tree, context, request),
        _ => unreachable!("HANDLED names every verb this match covers"),
    }
}

/// Builds the `TabRecord` for `tab` (AC #1), `position` already known to
/// the caller (`list` has it from its own walk; `show` looks it up through
/// [`tab_position`]) - shared so the two verbs never compute it two
/// different ways.
pub(super) fn record_for(
    tree: &Tree,
    tab: &steg_kernel::Tab,
    position: usize,
    now: Moment,
) -> TabRecord {
    let id = tab.id();
    TabRecord {
        id,
        label: tab.label().map(str::to_string),
        title: None,
        position,
        current_pane: tab.current(),
        layout: tree
            .tab_layout(id)
            .expect("this tab is in the tree right now")
            .clone(),
        shown_status: tree
            .tab_shown_status(id, now)
            .expect("this tab is in the tree right now"),
    }
}

/// `tab.list` (AC #1): every tab of a session, in its own order, each
/// through [`record_for`].
fn list(
    tree: &Tree,
    context: &Context,
    request: &Request,
    now: Moment,
) -> Result<Outcome, ApiError> {
    let session_id = target_session(tree, context, request)?;
    let session = tree
        .session(session_id)
        .ok_or_else(|| map_kernel_error(Error::SessionNotFound(session_id)))?;
    let records = session
        .tabs()
        .iter()
        .enumerate()
        .map(|(index, tab)| record_for(tree, tab, index + 1, now))
        .collect();
    Ok(Outcome::TabRecords(records))
}

/// `tab.show` (AC #1): one tab, in full, through the same [`record_for`]
/// `list` uses.
fn show(
    tree: &Tree,
    context: &Context,
    request: &Request,
    now: Moment,
) -> Result<Outcome, ApiError> {
    let id: TabId = target_tab(tree, context, request)?;
    let position = tab_position(tree, id)?;
    let tab = super::find_tab(tree, id).ok_or_else(|| map_kernel_error(Error::TabNotFound(id)))?;
    Ok(Outcome::TabRecord(record_for(tree, tab, position, now)))
}

fn new(tree: &mut Tree, context: &Context, request: &Request) -> Result<Outcome, ApiError> {
    // `tab.new`'s own target is the *session* the tab is created in
    // (doc-2's "The session to create the tab in"), not a tab.
    let session_id = target_session(tree, context, request)?;

    // Converted (and so refused, if malformed) before anything is
    // created, same as every other refusal in this arm - even though the
    // bound `move_tab` checks it against (how many siblings there will
    // be) is only known once the tab exists, so a position past the end
    // is still caught below, after creation, not here.
    let position = match param_i64(request, "position") {
        Some(wire) => Some(wire_position_to_index(wire)?),
        None => None,
    };

    let label = param_str(request, "label").map(str::to_string);
    let focus = param_bool(request, "focus").unwrap_or(true);

    // Captured before `new_tab` unconditionally makes the new tab current
    // (R115), so a `--no-focus` call, or the rollback below, can put it
    // back afterward.
    let previous_tab = tree
        .session(session_id)
        .ok_or_else(|| map_kernel_error(Error::SessionNotFound(session_id)))?
        .current_tab();

    let tab_id = tree.new_tab(session_id, label).map_err(map_kernel_error)?;

    if let Some(index) = position {
        // doc-2 section 8: `tab.new`'s own `position` is "where among its
        // siblings the new tab goes", not left for the caller to achieve
        // with a second `tab.move` call. `Tree::move_tab` is the same
        // conversion `tab.move`'s own arm already composes with
        // `wire_position_to_index`, used here as half of one outcome: if
        // the position is past the end, nothing is left behind - the tab
        // this call just created is closed again, and the session's
        // current tab restored to what it was before this call, same as
        // `pane.split`'s own rollback on a rejected label.
        if let Err(error) = tree.move_tab(tab_id, index) {
            tree.close_tab(tab_id)
                .expect("the tab this call just created is still in the tree");
            tree.focus_tab(previous_tab)
                .expect("the session this call just created a tab in still holds its previous tab");
            return Err(map_kernel_error(error));
        }
    }

    if !focus {
        tree.focus_tab(previous_tab).map_err(map_kernel_error)?;
    }

    Ok(Outcome::Tab(tab_id))
}

/// `tab.close` (AC #4): refuses while any of the tab's panes has a program
/// running and `--force` was not given, with `ErrorCode::ProgramRunning`,
/// touching nothing - the same dispatch-level policy `pane::close` uses,
/// over every pane the tab holds rather than one. With `--force`, this
/// always proceeds - see `pane::close`'s own doc for why ending whatever
/// was still running is not this function's own job either.
fn close(tree: &mut Tree, context: &Context, request: &Request) -> Result<Outcome, ApiError> {
    let id = target_tab(tree, context, request)?;
    let force = param_bool(request, "force").unwrap_or(false);

    if !force
        && super::any_pane_running(tree, &super::panes_in_close_scope(tree, context, request)?)
    {
        return Err(super::program_running());
    }

    let closed = tree.close_tab(id).map_err(map_kernel_error)?;
    Ok(Outcome::TabClosed(closed))
}

fn move_(tree: &mut Tree, context: &Context, request: &Request) -> Result<Outcome, ApiError> {
    let id = target_tab(tree, context, request)?;

    if param_present(request, "to") {
        // `tab.move --to` moves a tab between sessions; `Tree` has no
        // operation for that today. Checked first, before `position`, so
        // a request that gives both never silently does only half of
        // what it asked for. TASK-1.111, "Kernel and server: move a tab
        // to another session", split from this task and since accepted.
        return Err(not_implemented_yet(
            "TASK-1.111, \"Kernel and server: move a tab to another session\" - Tree has no \
             operation to move a tab between sessions",
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
