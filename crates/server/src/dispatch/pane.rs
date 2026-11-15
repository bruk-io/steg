//! The `pane` noun's dispatch arms (doc-2 section 8).

use steg_kernel::{Error, Tree};
use steg_sdk::{ApiError, Direction, PaneKind, Request};

use super::outcome::Outcome;
use super::{
    Context, bad_request, find_tab, map_kernel_error, not_implemented_yet, pane_mut, param_bool,
    param_present, param_str, resolve_pane_str, target_pane,
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
) -> Result<Outcome, ApiError> {
    match verb {
        "list" | "show" => Err(not_implemented_yet(
            "title/cwd/program/exit_code/status (TASK-1.39-1.41)",
        )),
        "split" => split(tree, context, request),
        "close" => close(tree, context, request),
        "move" => move_(tree, context, request),
        "set" => set(tree, context, request),
        "unset" => unset(tree, context, request),
        "focus" => focus(tree, context, request),
        "resize" => Err(not_implemented_yet("Tab::size() (TASK-1.42)")),
        "swap" => swap(tree, context, request),
        "send" | "capture" | "wait" => Err(not_implemented_yet(
            "Tree::start_program/record_exit/restart_program and a program on Pane (TASK-1.40)",
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

fn split(tree: &mut Tree, context: &Context, request: &Request) -> Result<Outcome, ApiError> {
    let target = target_pane(tree, context, request)?;

    if param_present(request, "cwd") || param_present(request, "program") {
        return Err(not_implemented_yet(
            "a pane's program and working directory (TASK-1.40)",
        ));
    }

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

fn close(tree: &mut Tree, context: &Context, request: &Request) -> Result<Outcome, ApiError> {
    let id = target_pane(tree, context, request)?;
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
