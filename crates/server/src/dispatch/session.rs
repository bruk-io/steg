//! The `session` noun's dispatch arms (doc-2 section 8).

use steg_kernel::Tree;
use steg_sdk::{ApiError, Request, SessionId, SessionRecord};

use super::outcome::Outcome;
use super::{
    Context, bad_request, map_kernel_error, not_implemented_yet, param_bool, param_i64,
    param_present, param_str, target_session, wire_position_to_index,
};

/// Every `session.*` method this module matches on, checked against
/// `description().methods` by `mod.rs`'s own completeness test.
pub(super) const HANDLED: &[(&str, &str)] = &[
    ("session", "list"),
    ("session", "show"),
    ("session", "new"),
    ("session", "close"),
    ("session", "move"),
    ("session", "set"),
    ("session", "unset"),
    ("session", "focus"),
];

pub(super) fn dispatch(
    tree: &mut Tree,
    context: &Context,
    request: &Request,
    verb: &str,
) -> Result<Outcome, ApiError> {
    match verb {
        "list" => Ok(list(tree)),
        "show" => show(tree, context, request),
        "new" => new(tree, request),
        "close" => close(tree, context, request),
        "move" => move_(tree, context, request),
        "set" => set(tree, context, request),
        "unset" => unset(tree, context, request),
        "focus" => Err(not_implemented_yet(
            "TASK-1.109, \"Server: events for layout, for a session or tab changing, for \
             clients, and the one addressed to a single client\" - a client to address, \
             which needs an event (decision-5, R87)",
        )),
        _ => unreachable!("HANDLED names every verb this match covers"),
    }
}

/// Builds the `SessionRecord` for the session at `index` in
/// `tree.sessions()` — the record's `position` is that index, one-based
/// (doc-15 R10).
pub(super) fn record_at(sessions: &[steg_kernel::Session], index: usize) -> SessionRecord {
    let session = &sessions[index];
    SessionRecord {
        id: session.id(),
        label: session.label().map(str::to_string),
        position: index + 1,
        colour: session.colour(),
        current_tab: session.current_tab(),
    }
}

fn list(tree: &Tree) -> Outcome {
    let sessions = tree.sessions();
    Outcome::SessionRecords(
        (0..sessions.len())
            .map(|i| record_at(sessions, i))
            .collect(),
    )
}

fn show(tree: &Tree, context: &Context, request: &Request) -> Result<Outcome, ApiError> {
    let id = target_session(tree, context, request)?;
    let sessions = tree.sessions();
    // `id` can be a target resolved explicitly (already checked to exist),
    // but it can also be `context.default_session` with no check at all —
    // `Context` is caller-supplied, and a future connection's
    // `STEG_SESSION` (TASK-1.45) can name a session that has since closed.
    let index = sessions
        .iter()
        .position(|s| s.id() == id)
        .ok_or_else(|| map_kernel_error(steg_kernel::Error::SessionNotFound(id)))?;
    Ok(Outcome::SessionRecord(record_at(sessions, index)))
}

fn new(tree: &mut Tree, request: &Request) -> Result<Outcome, ApiError> {
    let label = param_str(request, "label").map(str::to_string);
    let id = tree.new_session(label).map_err(map_kernel_error)?;
    Ok(Outcome::Session(id))
}

/// `session.close` (AC #4): refuses while any pane of any of the
/// session's tabs has a program running and `--force` was not given,
/// with `ErrorCode::ProgramRunning`, touching nothing - the same
/// dispatch-level policy `pane::close`/`tab::close` use, over every pane
/// the session holds. With `--force`, this always proceeds - see
/// `pane::close`'s own doc for why ending whatever was still running is
/// not this function's own job either.
fn close(tree: &mut Tree, context: &Context, request: &Request) -> Result<Outcome, ApiError> {
    let id = target_session(tree, context, request)?;
    let force = param_bool(request, "force").unwrap_or(false);

    if !force
        && super::any_pane_running(tree, &super::panes_in_close_scope(tree, context, request)?)
    {
        return Err(super::program_running());
    }

    tree.close_session(id).map_err(map_kernel_error)?;
    Ok(Outcome::Nothing)
}

fn move_(tree: &mut Tree, context: &Context, request: &Request) -> Result<Outcome, ApiError> {
    let id = target_session(tree, context, request)?;
    let position = param_i64(request, "position").ok_or_else(|| bad_request("needs position"))?;
    let index = wire_position_to_index(position)?;
    tree.move_session(id, index).map_err(map_kernel_error)?;
    Ok(Outcome::Nothing)
}

fn set(tree: &mut Tree, context: &Context, request: &Request) -> Result<Outcome, ApiError> {
    let id = target_session(tree, context, request)?;
    if let Some(label) = param_str(request, "label") {
        let session = session_mut(tree, id)?;
        session
            .set_label(Some(label.to_string()))
            .map_err(map_kernel_error)?;
    }
    Ok(Outcome::Nothing)
}

fn unset(tree: &mut Tree, context: &Context, request: &Request) -> Result<Outcome, ApiError> {
    let id = target_session(tree, context, request)?;
    if param_present(request, "label") {
        let session = session_mut(tree, id)?;
        session.set_label(None).map_err(map_kernel_error)?;
    }
    Ok(Outcome::Nothing)
}

fn session_mut(tree: &mut Tree, id: SessionId) -> Result<&mut steg_kernel::Session, ApiError> {
    tree.session_mut(id)
        .ok_or_else(|| map_kernel_error(steg_kernel::Error::SessionNotFound(id)))
}
