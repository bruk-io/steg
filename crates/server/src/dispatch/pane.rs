//! The `pane` noun's dispatch arms (doc-2 section 8).

use steg_kernel::{Error, Moment, Pane, Tree};
use steg_sdk::{ApiError, Direction, PaneKind, PaneRecord, Program, ProgramStatus, Request, Size};

use super::outcome::Outcome;
use super::{
    Context, bad_request, find_tab, map_kernel_error, not_implemented_yet, pane_mut, param_bool,
    param_present, param_str, resolve_pane_str, target_pane, target_tab,
};

/// Every `pane.*` method this module matches on, checked against
/// `description().methods` by `mod.rs`'s own completeness test.
pub(super) const HANDLED: &[(&str, &str)] = &[
    ("pane", "list"),
    ("pane", "show"),
    ("pane", "split"),
    ("pane", "close"),
    ("pane", "move"),
    ("pane", "set"),
    ("pane", "unset"),
    ("pane", "focus"),
    ("pane", "resize"),
    ("pane", "swap"),
    ("pane", "send"),
    ("pane", "capture"),
    ("pane", "wait"),
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
        "split" => split(tree, context, request),
        "close" => close(tree, context, request),
        "move" => move_(tree, context, request),
        "set" => set(tree, context, request),
        "unset" => unset(tree, context, request),
        "focus" => focus(tree, context, request),
        "resize" => Err(not_implemented_yet(
            "TASK-1.110, \"Server: pane resize\" - Tree::resize_pane takes a ratio, not a \
             cell delta, and Tab::size() can be unknown",
        )),
        "swap" => swap(tree, context, request),
        "send" | "capture" | "wait" => Err(not_implemented_yet(
            "TASK-1.90 - dispatch has no I/O by design; these need a Runtime-level wrapper, \
             not a dispatch arm",
        )),
        _ => unreachable!("HANDLED names every verb this match covers"),
    }
}

/// The description's four direction words, parsed back to
/// [`Direction`] — `validate` already restricted a present `direction` to
/// this closed set, so the fallback arm is unreachable, not a possible
/// `BadRequest`.
fn parse_direction(word: &str) -> Direction {
    match word {
        "left" => Direction::Left,
        "right" => Direction::Right,
        "up" => Direction::Up,
        "down" => Direction::Down,
        _ => unreachable!("validate already restricted direction to this closed set"),
    }
}

/// Builds the `PaneRecord` for `pane`, in the tab `tab_id` names, as of
/// `now` (AC #2; TASK-1.89) - pure, since `Program`/`ProgramStatus`/
/// `Status` already live on the kernel's own `Pane`
/// (`crates/kernel/src/pane.rs`), not behind a `PaneHandle`. `title` is
/// always `None` (`PaneRecord`'s own doc: that needs the terminal's live
/// OSC title, which is I/O this function does not reach). `size` is
/// `None` whenever the tab holds no declared size yet (R91) -
/// `pane_size` below reads `Tree::tab_layout`/`Tab::size` the same way
/// `tab::record_for` reads `tab_layout` for `TabRecord.layout`.
fn record_for(tree: &Tree, tab_id: steg_sdk::TabId, pane: &Pane, now: Moment) -> PaneRecord {
    PaneRecord {
        id: pane.id(),
        label: pane.label().map(str::to_string),
        title: None,
        kind: pane.kind(),
        cwd: pane
            .program()
            .map(Program::cwd)
            .unwrap_or_default()
            .to_string(),
        program: pane.program().map(program_text).unwrap_or_default(),
        exit_code: match pane.program_status() {
            Some(ProgramStatus::Ended { exit_code }) => Some(exit_code),
            _ => None,
        },
        program_status: pane.program_status().map(status_text),
        status: pane.status(now),
        size: pane_size(tree, tab_id, pane.id()),
    }
}

/// A pane's terminal size in whole cells, if its tab has one (R91):
/// `Tab::size()` combined with `Tree::tab_layout`'s own `Layout::leaf_sizes`,
/// found by this pane's id - `None` whenever the tab's size is unknown,
/// never a guess.
fn pane_size(tree: &Tree, tab_id: steg_sdk::TabId, pane_id: steg_sdk::PaneId) -> Option<Size> {
    let tab = find_tab(tree, tab_id)?;
    let total = tab.size()?;
    let layout = tree.tab_layout(tab_id).ok()?;
    layout
        .leaf_sizes(total)
        .into_iter()
        .find(|(id, _)| *id == pane_id)
        .map(|(_, size)| size)
}

/// `pane.show` (AC #2): one pane, in full, through [`record_for`].
fn show(
    tree: &Tree,
    context: &Context,
    request: &Request,
    now: Moment,
) -> Result<Outcome, ApiError> {
    let id = target_pane(tree, context, request)?;
    let tab_id = tree
        .tab_of_pane(id)
        .ok_or_else(|| map_kernel_error(Error::PaneNotFound(id)))?;
    let pane = find_tab(tree, tab_id)
        .and_then(|tab| tab.pane(id))
        .ok_or_else(|| map_kernel_error(Error::PaneNotFound(id)))?;

    Ok(Outcome::PaneRecord(record_for(tree, tab_id, pane, now)))
}

/// `pane.list` (AC #1): every pane of a tab, in R112's order, each through
/// [`record_for`].
fn list(
    tree: &Tree,
    context: &Context,
    request: &Request,
    now: Moment,
) -> Result<Outcome, ApiError> {
    let tab_id = target_tab(tree, context, request)?;
    let tab = find_tab(tree, tab_id).ok_or_else(|| map_kernel_error(Error::TabNotFound(tab_id)))?;
    let records = tab
        .panes()
        .iter()
        .map(|pane| record_for(tree, tab_id, pane, now))
        .collect();
    Ok(Outcome::PaneRecords(records))
}

/// `pane.show`'s `program` field, as a short word a person reads: the
/// kind (`Program` itself only ever means one of these two things; no
/// wildcard arm, so a third kind fails this to build, same reasoning
/// `Pane::kind`'s own doc gives), never the whole `Program` value's debug
/// form.
fn program_text(program: &Program) -> String {
    match program {
        Program::Shell { .. } => "shell".to_string(),
        Program::Command { argv, .. } => argv.join(" "),
    }
}

/// `pane.show`'s `status` field, as a plain word - `ProgramStatus`'s own
/// serde form is a tagged JSON shape (`"running"` or `{"ended":{...}}`),
/// not the plain `ValueKind::Text` the description declares, so this is
/// its own, smaller rendering rather than reusing serde here.
fn status_text(status: ProgramStatus) -> String {
    match status {
        ProgramStatus::Running => "running".to_string(),
        ProgramStatus::Ended { .. } => "ended".to_string(),
    }
}

fn split(tree: &mut Tree, context: &Context, request: &Request) -> Result<Outcome, ApiError> {
    let target = target_pane(tree, context, request)?;

    let direction = param_str(request, "direction")
        .map(parse_direction)
        .ok_or_else(|| bad_request("needs direction"))?;
    let focus = param_bool(request, "focus").unwrap_or(true);

    let tab_id = tree
        .tab_of_pane(target)
        .ok_or_else(|| map_kernel_error(Error::PaneNotFound(target)))?;
    // Captured before `split_pane` unconditionally makes the new pane
    // current in its tab (R115), so a `--no-focus` call can put it back.
    let previous_current = find_tab(tree, tab_id)
        .expect("tab_of_pane just found this tab")
        .current();

    let new_pane = tree
        .split_pane(target, direction, PaneKind::Terminal)
        .map_err(map_kernel_error)?;

    if let Some(label) = param_str(request, "label") {
        let labelled = pane_mut(tree, new_pane).and_then(|pane| {
            pane.set_label(Some(label.to_string()))
                .map_err(map_kernel_error)
        });
        if let Err(error) = labelled {
            // A rejected label (doc-2 section 4: contains `/`, or is all
            // digits) must not leave the split half-done - the pane this
            // call minted is closed again, and the tab's current pane
            // restored to what it was before the split, so the caller's
            // `BadRequest` is paired with a tree that looks exactly as it
            // did before this call, not with a pane nobody asked to keep.
            tree.close_pane(new_pane)
                .expect("the pane this call just split is still in the tree");
            tree.set_tab_current_pane(tab_id, previous_current)
                .expect("the tab this call just split from still holds its previous pane");
            return Err(error);
        }
    }

    if !focus {
        tree.set_tab_current_pane(tab_id, previous_current)
            .map_err(map_kernel_error)?;
    }

    Ok(Outcome::Pane(new_pane))
}

/// `pane.close` (AC #4): refuses while the pane's own program is running
/// and `--force` was not given, with `ErrorCode::ProgramRunning`, touching
/// nothing. This is a dispatch-level policy over the kernel's own
/// `Pane::program_status()`, because the kernel's `close_pane` has no concept of
/// "refuse" (TASK-1.40's own plan rejected threading a force flag through
/// its signature, the same reasoning this plan's own §5 repeats).
/// `--force` on a pane whose program is running stays exactly as it is
/// today: an unconditional close that does not touch the program at all
/// (the gap §5 names, until the task that forces the program itself to
/// end).
fn close(tree: &mut Tree, context: &Context, request: &Request) -> Result<Outcome, ApiError> {
    let id = target_pane(tree, context, request)?;
    let force = param_bool(request, "force").unwrap_or(false);

    if !force {
        let tab_id = tree
            .tab_of_pane(id)
            .ok_or_else(|| map_kernel_error(Error::PaneNotFound(id)))?;
        let pane = find_tab(tree, tab_id)
            .and_then(|tab| tab.pane(id))
            .ok_or_else(|| map_kernel_error(Error::PaneNotFound(id)))?;
        if matches!(pane.program_status(), Some(ProgramStatus::Running)) {
            return Err(super::program_running());
        }
    }

    tree.close_pane(id).map_err(map_kernel_error)?;
    Ok(Outcome::Nothing)
}

fn move_(tree: &mut Tree, context: &Context, request: &Request) -> Result<Outcome, ApiError> {
    let pane = target_pane(tree, context, request)?;
    let to = param_str(request, "to").ok_or_else(|| bad_request("needs to"))?;
    let target_tab = super::resolve_tab_str(tree, context, to)?;
    // The description carries no direction for `pane.move` (doc-2 section
    // 8: only `target` and `to`); `Tree::move_pane_to_tab` needs one to
    // place the pane within the target tab. `Direction::Right` is this
    // dispatch's own default - appending at the end of the target tab -
    // documented here since the description gives no way for a caller to
    // ask for anything else.
    tree.move_pane_to_tab(pane, target_tab, Direction::Right)
        .map_err(map_kernel_error)?;
    Ok(Outcome::Nothing)
}

fn set(tree: &mut Tree, context: &Context, request: &Request) -> Result<Outcome, ApiError> {
    let id = target_pane(tree, context, request)?;
    if let Some(label) = param_str(request, "label") {
        pane_mut(tree, id)?
            .set_label(Some(label.to_string()))
            .map_err(map_kernel_error)?;
    }
    if let Some(zoomed) = param_bool(request, "zoomed") {
        if zoomed {
            tree.fill_pane(id).map_err(map_kernel_error)?;
        } else {
            unfill(tree, id)?;
        }
    }
    Ok(Outcome::Nothing)
}

fn unset(tree: &mut Tree, context: &Context, request: &Request) -> Result<Outcome, ApiError> {
    let id = target_pane(tree, context, request)?;
    if param_present(request, "label") {
        pane_mut(tree, id)?
            .set_label(None)
            .map_err(map_kernel_error)?;
    }
    if param_present(request, "zoomed") {
        // `unset` always clears, regardless of the switch's own boolean
        // value - the same rule governs `unset --label` above.
        unfill(tree, id)?;
    }
    Ok(Outcome::Nothing)
}

/// Clears a pane's fill, given only the pane (not its tab) — `Tree::unfill_pane`
/// takes a `TabId`, so this looks the tab up first (`Tree::tab_of_pane`).
fn unfill(tree: &mut Tree, pane: steg_sdk::PaneId) -> Result<(), ApiError> {
    let tab_id = tree
        .tab_of_pane(pane)
        .ok_or_else(|| map_kernel_error(Error::PaneNotFound(pane)))?;
    tree.unfill_pane(tab_id).map_err(map_kernel_error)
}

fn focus(tree: &mut Tree, context: &Context, request: &Request) -> Result<Outcome, ApiError> {
    let pane = target_pane(tree, context, request)?;
    match param_str(request, "direction") {
        Some(word) => {
            let direction = parse_direction(word);
            let wrap = param_bool(request, "wrap").unwrap_or(false);
            // doc-2 section 4: at the edge, with nothing to wrap to
            // either, focus does not move and this still exits 0 - never
            // an error, so `FocusMove::Edge` is not matched separately.
            tree.focus_direction(pane, direction, wrap)
                .map_err(map_kernel_error)?;
        }
        None => {
            tree.focus_pane(pane).map_err(map_kernel_error)?;
        }
    }
    Ok(Outcome::Nothing)
}

fn swap(tree: &mut Tree, context: &Context, request: &Request) -> Result<Outcome, ApiError> {
    let pane = target_pane(tree, context, request)?;
    let with = param_str(request, "with").ok_or_else(|| bad_request("needs with"))?;
    let other = resolve_pane_str(tree, context, with)?;
    tree.swap_panes(pane, other).map_err(map_kernel_error)?;
    Ok(Outcome::Nothing)
}
