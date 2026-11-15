//! A tab: an ordered page within a session, and its layout tree (R95).

use steg_sdk::{Direction, Layout, Orientation, PaneId, PaneKind, Ratio, Size, TabId};

use crate::error::Error;
use crate::label;
use crate::pane::Pane;

/// R100: no split or resize leaves a terminal pane smaller than this.
const MIN_COLUMNS: u16 = 20;
const MIN_ROWS: u16 = 4;

/// The engineer confirmed on 2026-11-15 that a split divides the target
/// pane in half. `move_pane_to_tab` (in `tree.rs`) uses the same default.
const SPLIT_RATIO: Ratio = match Ratio::new(500) {
    Some(ratio) => ratio,
    None => unreachable!(),
};

/// What happened when a pane was closed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PaneClosed {
    /// The pane that was closed.
    pub pane: PaneId,
    /// Whether the tab that held it has no panes left (R94: closing a
    /// tab's last pane closes the tab). The tab is left in place, empty,
    /// for `Tree` to close.
    pub tab_now_empty: bool,
}

/// What asking a tab for a pane's neighbour in a direction found (R113).
/// Never an error: doc-2 section 4 treats "nothing that way, focus stays
/// put" as success, so there is no failure case — only which of the
/// three this was.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirectionalFocus {
    /// The pane adjacent to `from` on that side.
    Neighbour(PaneId),
    /// Nothing that way (and, if wrapping was asked for, nothing to wrap
    /// to either — see [`DirectionalFocus::Wrapped`]).
    Edge,
    /// Nothing that way, wrapping was asked for, and this is what
    /// wrapping around to the opposite edge of the tab landed on.
    Wrapped(PaneId),
}

/// A tab: an ordered page within a session. Its panes sit at the leaves
/// of `layout`, a tree of splits (R95); `panes` holds each pane's own
/// data (its label, kind, and later its program and status), kept in
/// R112's order — `layout`'s leaves, first child before second — by
/// `reorder_panes_to_match_layout`, so `panes()` can hand that order out
/// directly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tab {
    id: TabId,
    label: Option<String>,
    panes: Vec<Pane>,
    layout: Layout,
    current: PaneId,
    fill: Option<PaneId>,
    size: Option<Size>,
}

impl Tab {
    /// Builds a tab with exactly one pane already in it (R94: a tab is
    /// never created empty), which is both its whole layout and its
    /// current pane.
    pub(crate) fn new(id: TabId, first_pane: PaneId, first_pane_kind: PaneKind) -> Self {
        Self {
            id,
            label: None,
            panes: vec![Pane::new(first_pane, first_pane_kind)],
            layout: Layout::Pane(first_pane),
            current: first_pane,
            fill: None,
            size: None,
        }
    }

    /// This tab's id.
    #[must_use]
    pub const fn id(&self) -> TabId {
        self.id
    }

    /// This tab's label, if it has one.
    #[must_use]
    pub fn label(&self) -> Option<&str> {
        self.label.as_deref()
    }

    /// Makes `pane` this tab's current pane (R85). Used directly by
    /// `split_pane` (R115, unconditionally) and, through
    /// `Tree::focus_pane`/`Tree::focus_direction`, by the explicit focus
    /// operations.
    ///
    /// # Errors
    ///
    /// Returns [`Error::PaneNotFound`] if `pane` is not in this tab.
    pub(crate) fn focus(&mut self, pane: PaneId) -> Result<(), Error> {
        if self.pane(pane).is_none() {
            return Err(Error::PaneNotFound(pane));
        }
        self.current = pane;
        Ok(())
    }

    /// Sets or clears this tab's label.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidLabel`] if `label` contains `/`, or is made
    /// only of digits (doc-2 section 4).
    pub fn set_label(&mut self, label: Option<String>) -> Result<(), Error> {
        self.label = label::validate(label)?;
        Ok(())
    }

    /// This tab's panes, in R112's order: the layout tree's leaves, first
    /// child before second at every split. A pane's *position* — what a
    /// target like "the second pane" resolves against — means this
    /// order, counted from one, not creation order.
    #[must_use]
    pub fn panes(&self) -> &[Pane] {
        &self.panes
    }

    /// The pane with this id, if this tab holds it.
    #[must_use]
    pub fn pane(&self, id: PaneId) -> Option<&Pane> {
        self.panes.iter().find(|pane| pane.id() == id)
    }

    /// The pane with this id, if this tab holds it.
    pub fn pane_mut(&mut self, id: PaneId) -> Option<&mut Pane> {
        self.panes.iter_mut().find(|pane| pane.id() == id)
    }

    /// This tab's current pane (doc-15 R85).
    #[must_use]
    pub const fn current(&self) -> PaneId {
        self.current
    }

    /// The pane filling this tab, if any (R96).
    #[must_use]
    pub const fn fill(&self) -> Option<PaneId> {
        self.fill
    }

    /// What's in `direction` from `from`, optionally wrapping around the
    /// tab's far edge if there's nothing that way (R113, TASK-1.37's
    /// requested shape). `from` not being a pane in this tab is itself
    /// just "nothing found" — `Edge` — not an error; a caller that needs
    /// to tell that apart from "found, but no neighbour" uses
    /// [`Tab::pane`] first.
    #[must_use]
    pub fn neighbour(&self, from: PaneId, direction: Direction, wrap: bool) -> DirectionalFocus {
        if let Some(id) = self.layout.neighbour(from, direction) {
            return DirectionalFocus::Neighbour(id);
        }
        if wrap && let Some(id) = self.layout.far_edge_pane(from, direction) {
            return DirectionalFocus::Wrapped(id);
        }
        DirectionalFocus::Edge
    }

    /// Stores this tab's current size in whole cells. Unconditional — no
    /// floor check, ever (R100's last clause, AC #6: a tab whose size
    /// shrinks "refuses and closes nothing"). Nothing in the kernel
    /// measures anything; a future caller (the server, from the window's
    /// pixels and the font's cell metrics) calls this whenever it learns
    /// a new size.
    pub fn set_size(&mut self, size: Size) {
        self.size = Some(size);
    }

    /// Splits `target`'s leaf, putting the already-minted `new_id` on
    /// `direction`'s side of it and making `new_id` this tab's current
    /// pane (AC #1). Refused outright — no mutation — if `self.size` is
    /// known and either resulting pane would be under R100's floor in
    /// either dimension; allowed unconditionally if `self.size` is
    /// `None`, since the floor can't be judged without it.
    ///
    /// # Errors
    ///
    /// [`Error::PaneNotFound`] if `target` is not in this tab.
    /// [`Error::SplitTooSmall`] if the floor would be violated.
    pub(crate) fn split_pane(
        &mut self,
        target: PaneId,
        direction: Direction,
        new_id: PaneId,
        kind: PaneKind,
    ) -> Result<(), Error> {
        if self.pane(target).is_none() {
            return Err(Error::PaneNotFound(target));
        }
        if self.would_violate_floor_inserting(target, direction, new_id) {
            return Err(Error::SplitTooSmall { pane: target });
        }

        let inserted = insert_leaf(&mut self.layout, target, new_id, direction);
        debug_assert!(inserted, "target was just confirmed present above");

        self.panes.push(Pane::new(new_id, kind));
        self.reorder_panes_to_match_layout();
        self.current = new_id;
        debug_assert!(self.panes_match_layout());

        Ok(())
    }

    /// Closes the pane with this id, giving its space to the other child
    /// of its split (AC #2), and nothing else in the tree — a tree
    /// collapse touching only the one ancestor pointer. If this was the
    /// tab's only pane, reports `tab_now_empty: true` without touching
    /// `layout` at all: `Tree` deletes the tab in the same call, so the
    /// stale value is never observed. `current`/`fill`, if they named
    /// the closed pane, are repaired to stay valid (R85, TASK-1.37).
    ///
    /// # Errors
    ///
    /// Returns [`Error::PaneNotFound`] if this tab holds no pane with that
    /// id.
    pub(crate) fn close_pane(&mut self, id: PaneId) -> Result<PaneClosed, Error> {
        if self.pane(id).is_none() {
            return Err(Error::PaneNotFound(id));
        }

        let promoted_leaf = remove_leaf(&mut self.layout, id);
        self.panes.retain(|pane| pane.id() != id);

        let Some(promoted_leaf) = promoted_leaf else {
            return Ok(PaneClosed {
                pane: id,
                tab_now_empty: true,
            });
        };

        self.reorder_panes_to_match_layout();
        if self.current == id {
            self.current = promoted_leaf;
        }
        if self.fill == Some(id) {
            self.fill = None;
        }
        debug_assert!(self.panes_match_layout());

        Ok(PaneClosed {
            pane: id,
            tab_now_empty: false,
        })
    }

    /// Changes the ratio of the split bordering `pane` on `direction`'s
    /// side, clamping to the nearest ratio that keeps every leaf in that
    /// split's subtree at or above R100's floor (if `self.size` is
    /// known) rather than refusing (R100: "a resize stops at that
    /// floor" — clamped, always succeeds, as against a split's outright
    /// refusal). Returns the ratio actually applied. Allowed
    /// unconditionally, at the requested ratio, if `self.size` is
    /// `None`.
    ///
    /// # Errors
    ///
    /// Returns [`Error::NoSplitInDirection`] if `pane` has no split
    /// bordering it on that side.
    pub(crate) fn resize_pane(
        &mut self,
        pane: PaneId,
        direction: Direction,
        ratio: Ratio,
    ) -> Result<Ratio, Error> {
        let Some(found) = self.layout.bordering_split(pane, direction) else {
            return Err(Error::NoSplitInDirection { pane, direction });
        };
        let Layout::Split {
            orientation,
            first,
            second,
            ..
        } = found
        else {
            unreachable!("bordering_split only ever returns a Split variant")
        };
        let orientation = *orientation;
        let first = (**first).clone();
        let second = (**second).clone();

        let applied = if let Some(total) = self.size {
            let inherited = self
                .layout
                .inherited_size(pane, direction, total)
                .expect("bordering_split just found this split, so inherited_size must too");
            clamp_ratio_to_floor(orientation, ratio, inherited, &first, &second)
        } else {
            ratio
        };

        let split = self
            .layout
            .bordering_split_mut(pane, direction)
            .expect("bordering_split just found this split, so bordering_split_mut must too");
        let Layout::Split { ratio: stored, .. } = split else {
            unreachable!("bordering_split_mut only ever returns a Split variant")
        };
        *stored = applied;

        Ok(applied)
    }

    /// Exchanges the positions of two panes in this tab's layout tree.
    /// Every split's ratio and direction, and every other pane's
    /// position, are untouched (AC #3) — only which pane id occupies
    /// each of the two leaves changes.
    ///
    /// # Errors
    ///
    /// Returns [`Error::PaneNotFound`] if either pane is not in this tab.
    pub(crate) fn swap_panes(&mut self, a: PaneId, b: PaneId) -> Result<(), Error> {
        if self.pane(a).is_none() {
            return Err(Error::PaneNotFound(a));
        }
        if self.pane(b).is_none() {
            return Err(Error::PaneNotFound(b));
        }

        swap_leaves(&mut self.layout, a, b);
        self.reorder_panes_to_match_layout();
        debug_assert!(self.panes_match_layout());

        Ok(())
    }

    /// Sets which pane fills this tab (R96), replacing any previous fill.
    /// Never touches `layout` (AC #4: filling changes nothing in it).
    ///
    /// # Errors
    ///
    /// Returns [`Error::PaneNotFound`] if `pane` is not in this tab.
    pub(crate) fn fill_pane(&mut self, pane: PaneId) -> Result<(), Error> {
        if self.pane(pane).is_none() {
            return Err(Error::PaneNotFound(pane));
        }
        self.fill = Some(pane);
        Ok(())
    }

    /// Clears this tab's fill, if it has one.
    pub(crate) fn unfill(&mut self) {
        self.fill = None;
    }

    /// Whether splitting `target` to insert a new leaf `new_id` on
    /// `direction`'s side would violate R100's floor — a dry run, no
    /// mutation, used inline by [`Tab::split_pane`] and by
    /// `Tree::move_pane_to_tab` (in `tree.rs`) *before* it removes
    /// anything from the source tab, since the move must be atomic:
    /// either both halves happen or neither does, which means the
    /// target's floor has to be checked before the source is touched at
    /// all. `false` (never violates) if `self.size` is `None` — R100
    /// can't be judged without it.
    pub(crate) fn would_violate_floor_inserting(
        &self,
        target: PaneId,
        direction: Direction,
        new_id: PaneId,
    ) -> bool {
        let Some(total) = self.size else {
            return false;
        };
        let Some(target_size) = self
            .layout
            .leaf_sizes(total)
            .into_iter()
            .find(|(id, _)| *id == target)
            .map(|(_, size)| size)
        else {
            return false; // `target` isn't in this tab; the caller's own check catches that.
        };
        let hypothetical = hypothetical_split(direction, target, new_id);
        !satisfies_floor_sizes(hypothetical.leaf_sizes(target_size))
    }

    /// Removes `target`'s leaf from both `layout` and `panes`, returning
    /// the removed `Pane` itself (not dropped — for `move_pane_to_tab`,
    /// which relocates it into another tab) alongside the same
    /// [`PaneClosed`] cascade information [`Tab::close_pane`] reports,
    /// so `Tree::move_pane_to_tab` can run the identical
    /// `close_tab`/`close_session` cascade `Tree::close_pane` does.
    ///
    /// # Panics
    ///
    /// Never in practice: callers confirm `target` is present first (the
    /// floor check in `would_violate_floor_inserting` has already run by
    /// the time this is called, which itself requires `target` to
    /// exist).
    pub(crate) fn remove_pane_for_move(&mut self, target: PaneId) -> (Pane, PaneClosed) {
        let promoted_leaf = remove_leaf(&mut self.layout, target);
        let position = self
            .panes
            .iter()
            .position(|pane| pane.id() == target)
            .expect("callers confirm target is present before calling this");
        let removed = self.panes.remove(position);

        let Some(promoted_leaf) = promoted_leaf else {
            return (
                removed,
                PaneClosed {
                    pane: target,
                    tab_now_empty: true,
                },
            );
        };

        self.reorder_panes_to_match_layout();
        if self.current == target {
            self.current = promoted_leaf;
        }
        if self.fill == Some(target) {
            self.fill = None;
        }
        debug_assert!(self.panes_match_layout());

        (
            removed,
            PaneClosed {
                pane: target,
                tab_now_empty: false,
            },
        )
    }

    /// The target tab's half of a move: inserts an already-existing
    /// `Pane` (relocated from another tab by `move_pane_to_tab`, not
    /// freshly minted) on `direction`'s side of this tab's current pane,
    /// making it current. Infallible — the floor was already checked by
    /// `would_violate_floor_inserting` before `Tree::move_pane_to_tab`
    /// committed to removing anything from the source tab.
    pub(crate) fn insert_pane_for_move(&mut self, direction: Direction, moved: Pane) {
        let target = self.current;
        let moved_id = moved.id();

        let inserted = insert_leaf(&mut self.layout, target, moved_id, direction);
        debug_assert!(inserted, "a tab's current pane is always in its own layout");

        self.panes.push(moved);
        self.reorder_panes_to_match_layout();
        self.current = moved_id;
        debug_assert!(self.panes_match_layout());
    }

    /// Reorders `panes` to match `layout.leaves()` — the single place
    /// R112's order is enforced, rather than hand-computing the right
    /// insertion point in every method that changes the tree's shape.
    fn reorder_panes_to_match_layout(&mut self) {
        let order = self.layout.leaves();
        self.panes.sort_by_key(|pane| {
            order
                .iter()
                .position(|id| *id == pane.id())
                .unwrap_or(usize::MAX)
        });
    }

    /// Whether `panes`'s id set exactly matches `layout.leaves()`'s —
    /// the two representations of "which panes exist" this tab keeps,
    /// which must never desync except transiently inside
    /// [`Tab::close_pane`]'s "this was the only pane" branch (which
    /// returns before any caller can observe it).
    fn panes_match_layout(&self) -> bool {
        let mut pane_ids: Vec<PaneId> = self.panes.iter().map(Pane::id).collect();
        let mut leaf_ids = self.layout.leaves();
        pane_ids.sort_unstable_by_key(|id| id.into_raw());
        leaf_ids.sort_unstable_by_key(|id| id.into_raw());
        pane_ids == leaf_ids
    }
}

/// The two-leaf layout a split or a move's insertion would produce,
/// before any mutation: `new_id` on `direction`'s side of `target`, at
/// the shared default ratio (see `SPLIT_RATIO`'s doc comment). Built so
/// its floor check can reuse `Layout::leaf_sizes` directly rather than
/// duplicating that rounding rule.
fn hypothetical_split(direction: Direction, target: PaneId, new_id: PaneId) -> Layout {
    let target_leaf = Layout::Pane(target);
    let new_leaf = Layout::Pane(new_id);
    let (first, second) = if direction.is_before() {
        (new_leaf, target_leaf)
    } else {
        (target_leaf, new_leaf)
    };
    Layout::Split {
        orientation: direction.orientation(),
        ratio: SPLIT_RATIO,
        first: Box::new(first),
        second: Box::new(second),
    }
}

/// Whether every leaf size names a pane at or above R100's floor in both
/// dimensions — not only the one a split or resize divides.
fn satisfies_floor_sizes(sizes: Vec<(PaneId, Size)>) -> bool {
    sizes
        .into_iter()
        .all(|(_, size)| size.columns() >= MIN_COLUMNS && size.rows() >= MIN_ROWS)
}

fn satisfies_floor(
    orientation: Orientation,
    ratio: Ratio,
    inherited: Size,
    first: &Layout,
    second: &Layout,
) -> bool {
    let hypothetical = Layout::Split {
        orientation,
        ratio,
        first: Box::new(first.clone()),
        second: Box::new(second.clone()),
    };
    satisfies_floor_sizes(hypothetical.leaf_sizes(inherited))
}

/// The nearest ratio to `requested` that keeps every leaf in the split's
/// subtree at or above R100's floor, searched outward from `requested`
/// in both directions (bounded: at most 999 steps, trivial cost) — R100:
/// "a resize stops at that floor", so it stops as close as possible to
/// what was asked for, on whichever side is nearer. If nothing in range
/// satisfies the floor (the split's own inherited size is already too
/// small for it, independent of this resize — R100 tolerates that),
/// returns `requested` unchanged: nothing here improves on a refusal.
fn clamp_ratio_to_floor(
    orientation: Orientation,
    requested: Ratio,
    inherited: Size,
    first: &Layout,
    second: &Layout,
) -> Ratio {
    if satisfies_floor(orientation, requested, inherited, first, second) {
        return requested;
    }

    let start = requested.permille();
    for distance in 1..1000 {
        if let Some(lower) = start.checked_sub(distance)
            && let Some(candidate) = Ratio::new(lower)
            && satisfies_floor(orientation, candidate, inherited, first, second)
        {
            return candidate;
        }
        let higher = start + distance;
        if higher < 1000
            && let Some(candidate) = Ratio::new(higher)
            && satisfies_floor(orientation, candidate, inherited, first, second)
        {
            return candidate;
        }
    }

    requested
}

/// Splices `new_id` into `layout` on `direction`'s side of `target`,
/// replacing `target`'s leaf with a new split. `true` if `target` was
/// found (and the splice happened), `false` otherwise. Mutates entirely
/// in place via `*layout = ...`, so — unlike `bordering_split_mut` —
/// there is no competing-borrow problem to work around: nothing here
/// ever returns a reference into the tree.
fn insert_leaf(layout: &mut Layout, target: PaneId, new_id: PaneId, direction: Direction) -> bool {
    match layout {
        Layout::Pane(id) if *id == target => {
            *layout = hypothetical_split(direction, target, new_id);
            true
        }
        Layout::Pane(_) => false,
        Layout::Split { first, second, .. } => {
            insert_leaf(first, target, new_id, direction)
                || insert_leaf(second, target, new_id, direction)
        }
    }
}

/// Removes `target`'s leaf, replacing its *parent* split with the
/// sibling subtree (R95: "gives its space to the other child of its
/// split") — mutating in place via `*layout = ...`, moving the sibling's
/// owned value out with `std::mem::replace` rather than cloning it.
/// Returns the promoted subtree's own first leaf in R112 order, or
/// `None` if `target` was this subtree's only leaf (no parent exists to
/// replace — the caller's cue that the whole tab, not just one pane, is
/// now empty).
fn remove_leaf(layout: &mut Layout, target: PaneId) -> Option<PaneId> {
    match layout {
        Layout::Pane(_) => None,
        Layout::Split { first, second, .. } => {
            let first_is_target = matches!(first.as_ref(), Layout::Pane(id) if *id == target);
            let second_is_target = matches!(second.as_ref(), Layout::Pane(id) if *id == target);

            if first_is_target {
                let promoted = std::mem::replace(second.as_mut(), Layout::Pane(target));
                let promoted_leaf = promoted.leaves().first().copied();
                *layout = promoted;
                return promoted_leaf;
            }
            if second_is_target {
                let promoted = std::mem::replace(first.as_mut(), Layout::Pane(target));
                let promoted_leaf = promoted.leaves().first().copied();
                *layout = promoted;
                return promoted_leaf;
            }
            remove_leaf(first, target).or_else(|| remove_leaf(second, target))
        }
    }
}

/// Exchanges `a` and `b` wherever either appears as a leaf in `layout`.
/// No split's ratio or direction, and no other leaf, changes.
fn swap_leaves(layout: &mut Layout, a: PaneId, b: PaneId) {
    match layout {
        Layout::Pane(id) if *id == a => *id = b,
        Layout::Pane(id) if *id == b => *id = a,
        Layout::Pane(_) => {}
        Layout::Split { first, second, .. } => {
            swap_leaves(first, a, b);
            swap_leaves(second, a, b);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{DirectionalFocus, Tab};
    use crate::error::Error;
    use steg_sdk::{Direction, Layout, PaneId, PaneKind, Ratio, Size, TabId};

    fn tab_with_one_pane() -> (Tab, PaneId) {
        let first = PaneId::from_raw(1);
        (
            Tab::new(TabId::from_raw(1), first, PaneKind::Terminal),
            first,
        )
    }

    #[test]
    fn split_pane_divides_only_the_target_pane() {
        let (mut tab, first) = tab_with_one_pane();
        let second = tab
            .split_pane(
                first,
                Direction::Right,
                PaneId::from_raw(2),
                PaneKind::Terminal,
            )
            .map(|()| PaneId::from_raw(2));
        assert!(second.is_ok());
        assert_eq!(tab.panes().len(), 2);
        assert!(tab.pane(first).is_some());
        assert!(tab.pane(PaneId::from_raw(2)).is_some());
    }

    #[test]
    fn split_pane_puts_the_new_pane_on_the_requested_side() {
        let (mut tab, first) = tab_with_one_pane();
        let new_id = PaneId::from_raw(2);
        tab.split_pane(first, Direction::Right, new_id, PaneKind::Terminal)
            .unwrap();

        // `new_id` is on the Right side, so it reads second: first,
        // then new_id, in R112 order.
        assert_eq!(
            tab.panes().iter().map(super::Pane::id).collect::<Vec<_>>(),
            vec![first, new_id]
        );
    }

    #[test]
    fn split_pane_makes_the_new_pane_current() {
        let (mut tab, first) = tab_with_one_pane();
        let new_id = PaneId::from_raw(2);
        tab.split_pane(first, Direction::Right, new_id, PaneKind::Terminal)
            .unwrap();

        assert_eq!(tab.current(), new_id);
    }

    #[test]
    fn split_pane_on_an_unknown_pane_is_an_error() {
        let (mut tab, _first) = tab_with_one_pane();
        let unknown = PaneId::from_raw(999);

        assert_eq!(
            tab.split_pane(
                unknown,
                Direction::Right,
                PaneId::from_raw(2),
                PaneKind::Terminal
            ),
            Err(Error::PaneNotFound(unknown))
        );
    }

    #[test]
    fn split_pane_is_refused_when_either_side_would_be_under_the_floor() {
        let (mut tab, first) = tab_with_one_pane();
        tab.set_size(Size::new(39, 10)); // halves to 19 and 19 columns: under 20.

        assert_eq!(
            tab.split_pane(
                first,
                Direction::Right,
                PaneId::from_raw(2),
                PaneKind::Terminal
            ),
            Err(Error::SplitTooSmall { pane: first })
        );
        assert_eq!(tab.panes().len(), 1);
    }

    #[test]
    fn split_pane_is_refused_when_the_undivided_dimension_is_already_under_the_floor() {
        let (mut tab, first) = tab_with_one_pane();
        tab.set_size(Size::new(100, 3)); // rows already under the floor.

        // A horizontal split halves columns, but both halves still
        // inherit the already-too-short 3 rows.
        assert_eq!(
            tab.split_pane(
                first,
                Direction::Right,
                PaneId::from_raw(2),
                PaneKind::Terminal
            ),
            Err(Error::SplitTooSmall { pane: first })
        );
    }

    #[test]
    fn split_pane_is_allowed_when_the_tabs_size_is_unknown() {
        let (mut tab, first) = tab_with_one_pane();

        assert!(
            tab.split_pane(
                first,
                Direction::Right,
                PaneId::from_raw(2),
                PaneKind::Terminal
            )
            .is_ok()
        );
    }

    #[test]
    fn split_pane_is_allowed_when_the_floor_is_respected() {
        let (mut tab, first) = tab_with_one_pane();
        tab.set_size(Size::new(100, 20));

        assert!(
            tab.split_pane(
                first,
                Direction::Right,
                PaneId::from_raw(2),
                PaneKind::Terminal
            )
            .is_ok()
        );
    }

    #[test]
    fn panes_are_in_layout_order_after_a_split() {
        let (mut tab, first) = tab_with_one_pane();
        let new_id = PaneId::from_raw(2);
        tab.split_pane(first, Direction::Left, new_id, PaneKind::Terminal)
            .unwrap();

        // new_id is on the Left, so it reads first.
        assert_eq!(
            tab.panes().iter().map(super::Pane::id).collect::<Vec<_>>(),
            vec![new_id, first]
        );
    }

    #[test]
    fn closing_a_pane_that_is_not_the_only_one_leaves_the_tab_open() {
        let (mut tab, first) = tab_with_one_pane();
        let second = PaneId::from_raw(2);
        tab.split_pane(first, Direction::Right, second, PaneKind::Terminal)
            .unwrap();

        let closed = tab.close_pane(second).unwrap();

        assert_eq!(closed.pane, second);
        assert!(!closed.tab_now_empty);
        assert_eq!(tab.panes().len(), 1);
        assert!(tab.pane(first).is_some());
    }

    #[test]
    fn closing_the_only_pane_reports_the_tab_now_empty() {
        let (mut tab, first) = tab_with_one_pane();

        let closed = tab.close_pane(first).unwrap();

        assert!(closed.tab_now_empty);
        assert!(tab.panes().is_empty());
    }

    #[test]
    fn closing_an_unknown_pane_is_an_error() {
        let (mut tab, _first) = tab_with_one_pane();
        let unknown = PaneId::from_raw(999);

        assert_eq!(tab.close_pane(unknown), Err(Error::PaneNotFound(unknown)));
    }

    #[test]
    fn set_label_changes_it() {
        let (mut tab, _first) = tab_with_one_pane();
        assert_eq!(tab.label(), None);

        tab.set_label(Some("logs".to_string())).unwrap();

        assert_eq!(tab.label(), Some("logs"));
    }

    #[test]
    fn set_label_rejects_an_invalid_label() {
        let (mut tab, _first) = tab_with_one_pane();

        assert_eq!(
            tab.set_label(Some("123".to_string())),
            Err(Error::InvalidLabel("123".to_string()))
        );
        assert_eq!(tab.label(), None);
    }

    #[test]
    fn closing_a_pane_promotes_its_siblings_subtree_to_its_parents_place() {
        let (mut tab, first) = tab_with_one_pane();
        let second = PaneId::from_raw(2);
        let third = PaneId::from_raw(3);
        tab.split_pane(first, Direction::Right, second, PaneKind::Terminal)
            .unwrap();
        tab.split_pane(second, Direction::Down, third, PaneKind::Terminal)
            .unwrap();
        // layout: Horizontal{first: first, second: Vertical{first: second, second: third}}

        tab.close_pane(second).unwrap();

        // third is promoted directly into the outer split's second slot.
        assert_eq!(
            tab.panes().iter().map(super::Pane::id).collect::<Vec<_>>(),
            vec![first, third]
        );
    }

    #[test]
    fn closing_a_pane_leaves_every_other_panes_position_unchanged() {
        let (mut tab, first) = tab_with_one_pane();
        let second = PaneId::from_raw(2);
        let third = PaneId::from_raw(3);
        tab.split_pane(first, Direction::Right, second, PaneKind::Terminal)
            .unwrap();
        tab.split_pane(first, Direction::Down, third, PaneKind::Terminal)
            .unwrap();
        // layout: Horizontal{first: Vertical{first: first, second: third}, second: second}

        tab.close_pane(second).unwrap();

        assert_eq!(
            tab.panes().iter().map(super::Pane::id).collect::<Vec<_>>(),
            vec![first, third]
        );
    }

    #[test]
    fn closing_the_current_pane_moves_current_to_a_pane_that_still_exists() {
        let (mut tab, first) = tab_with_one_pane();
        let second = PaneId::from_raw(2);
        tab.split_pane(first, Direction::Right, second, PaneKind::Terminal)
            .unwrap();
        assert_eq!(tab.current(), second);

        tab.close_pane(second).unwrap();

        assert_eq!(tab.current(), first);
    }

    #[test]
    fn closing_the_current_pane_whose_sibling_is_a_multi_leaf_split_picks_the_promoted_subtrees_first_leaf()
     {
        let (mut tab, a) = tab_with_one_pane();
        let b = PaneId::from_raw(2);
        let c = PaneId::from_raw(3);
        tab.split_pane(a, Direction::Right, b, PaneKind::Terminal)
            .unwrap(); // {A, B}
        tab.split_pane(b, Direction::Down, c, PaneKind::Terminal)
            .unwrap(); // {A, {B, C}}
        assert_eq!(tab.current(), c);

        // Swap A and C's *positions*, not `current` (which stays the id
        // "C", untouched by a swap): C now sits where A was, at the
        // root's first slot, whose sibling is the multi-leaf {B, A}
        // split (A having swapped into C's old slot).
        tab.swap_panes(a, c).unwrap();
        assert_eq!(tab.current(), c);

        tab.close_pane(c).unwrap();

        // The promoted subtree is {first: B, second: A}; current moves
        // to its *first* leaf in R112 order — B, not A, even though A
        // is also still in the tab.
        assert_eq!(tab.current(), b);
    }

    #[test]
    fn closing_a_filled_pane_clears_the_fill() {
        let (mut tab, first) = tab_with_one_pane();
        let second = PaneId::from_raw(2);
        tab.split_pane(first, Direction::Right, second, PaneKind::Terminal)
            .unwrap();
        tab.fill_pane(second).unwrap();
        assert_eq!(tab.fill(), Some(second));

        tab.close_pane(second).unwrap();

        assert_eq!(tab.fill(), None);
    }

    #[test]
    fn resize_pane_changes_the_bordering_splits_ratio() {
        let (mut tab, first) = tab_with_one_pane();
        let second = PaneId::from_raw(2);
        tab.split_pane(first, Direction::Right, second, PaneKind::Terminal)
            .unwrap();

        let applied = tab
            .resize_pane(first, Direction::Right, Ratio::new(700).unwrap())
            .unwrap();

        assert_eq!(applied, Ratio::new(700).unwrap());
    }

    #[test]
    fn resize_pane_does_not_change_the_panes_order() {
        let (mut tab, first) = tab_with_one_pane();
        let second = PaneId::from_raw(2);
        tab.split_pane(first, Direction::Right, second, PaneKind::Terminal)
            .unwrap();
        let before: Vec<PaneId> = tab.panes().iter().map(super::Pane::id).collect();

        tab.resize_pane(first, Direction::Right, Ratio::new(700).unwrap())
            .unwrap();

        let after: Vec<PaneId> = tab.panes().iter().map(super::Pane::id).collect();
        assert_eq!(before, after);
    }

    #[test]
    fn resize_pane_stops_exactly_at_the_floor_instead_of_going_below_it() {
        let (mut tab, first) = tab_with_one_pane();
        let second = PaneId::from_raw(2);
        tab.split_pane(first, Direction::Right, second, PaneKind::Terminal)
            .unwrap();
        tab.set_size(Size::new(100, 10));

        // Asking for 950 (first gets 95 columns, second gets 5 — under
        // the floor) should clamp to the nearest ratio that still gives
        // the second side exactly the floor's 20 columns: 809 (first:
        // floor(100 * 0.809) = 80, second: 20); 810 would already give
        // second only 19.
        let applied = tab
            .resize_pane(first, Direction::Right, Ratio::new(950).unwrap())
            .unwrap();

        assert_eq!(applied, Ratio::new(809).unwrap());
    }

    #[test]
    fn resize_pane_with_no_split_in_that_direction_is_an_error() {
        let (mut tab, first) = tab_with_one_pane();
        let second = PaneId::from_raw(2);
        tab.split_pane(first, Direction::Right, second, PaneKind::Terminal)
            .unwrap();

        assert_eq!(
            tab.resize_pane(first, Direction::Left, Ratio::new(500).unwrap()),
            Err(Error::NoSplitInDirection {
                pane: first,
                direction: Direction::Left,
            })
        );
    }

    #[test]
    fn an_unrelated_already_under_floor_pane_does_not_block_a_resize_elsewhere() {
        let (mut tab, first) = tab_with_one_pane();
        let second = PaneId::from_raw(2);
        let third = PaneId::from_raw(3);
        tab.split_pane(first, Direction::Right, second, PaneKind::Terminal)
            .unwrap();
        tab.split_pane(second, Direction::Down, third, PaneKind::Terminal)
            .unwrap();
        // Shrink to something whose right half (second's split) already
        // can't keep both halves at or above the floor in rows.
        tab.set_size(Size::new(100, 3));

        // Resizing the *outer* (first vs. {second,third}) split doesn't
        // touch the inner, already-too-short split at all.
        let applied = tab
            .resize_pane(first, Direction::Right, Ratio::new(700).unwrap())
            .unwrap();
        assert_eq!(applied, Ratio::new(700).unwrap());
    }

    #[test]
    fn resize_pane_clamps_upward_when_the_requested_ratio_would_starve_the_near_side() {
        let (mut tab, first) = tab_with_one_pane();
        let second = PaneId::from_raw(2);
        tab.split_pane(first, Direction::Right, second, PaneKind::Terminal)
            .unwrap();
        tab.set_size(Size::new(100, 10));

        // Asking for 50 (first gets 5 columns, under the floor) clamps
        // up to 200 (first: 20, second: 80) — the mirror image of
        // `resize_pane_stops_exactly_at_the_floor_instead_of_going_below_it`,
        // which only exercised the downward clamp.
        let applied = tab
            .resize_pane(first, Direction::Right, Ratio::new(50).unwrap())
            .unwrap();

        assert_eq!(applied, Ratio::new(200).unwrap());
    }

    #[test]
    fn resize_pane_leaves_an_already_legal_ratio_unchanged() {
        let (mut tab, first) = tab_with_one_pane();
        let second = PaneId::from_raw(2);
        tab.split_pane(first, Direction::Right, second, PaneKind::Terminal)
            .unwrap();
        tab.set_size(Size::new(100, 10));

        // 500 (50 and 50 columns) is already well clear of the floor,
        // with a known size: the clamp search's first check (try the
        // request itself) should succeed immediately.
        let applied = tab
            .resize_pane(first, Direction::Right, Ratio::new(500).unwrap())
            .unwrap();

        assert_eq!(applied, Ratio::new(500).unwrap());
    }

    /// `outer{mid{inner{A,B},C},D}`, all `Horizontal` — the same shape
    /// named in the review finding that this mid-build bug exposed in
    /// `bordering_split`'s ancestor walk, built here through the public
    /// `split_pane` (not a hand-built `Layout`, since this test is
    /// specifically about `resize_pane` going through both
    /// `bordering_split` and `bordering_split_mut`).
    #[test]
    fn resize_pane_in_a_deeply_nested_tab_changes_only_the_innermost_matching_split() {
        let (mut tab, a) = tab_with_one_pane();
        let d = PaneId::from_raw(2);
        let c = PaneId::from_raw(3);
        let b = PaneId::from_raw(4);
        tab.split_pane(a, Direction::Right, d, PaneKind::Terminal)
            .unwrap(); // outer{A,D}
        tab.split_pane(a, Direction::Right, c, PaneKind::Terminal)
            .unwrap(); // outer{mid{A,C},D}
        tab.split_pane(a, Direction::Right, b, PaneKind::Terminal)
            .unwrap(); // outer{mid{inner{A,B},C},D}

        tab.resize_pane(a, Direction::Right, Ratio::new(700).unwrap())
            .unwrap();

        // Read the raw tree (this test is inside `tab`'s own module, so
        // the private `layout` field is visible) to confirm only the
        // innermost split's ratio changed — the outermost-ancestor bug
        // this guards against would have changed `outer`'s instead.
        let Layout::Split {
            ratio: outer_ratio,
            first: mid,
            ..
        } = &tab.layout
        else {
            panic!("expected a split");
        };
        assert_eq!(*outer_ratio, Ratio::new(500).unwrap());
        let Layout::Split {
            ratio: mid_ratio,
            first: inner,
            ..
        } = mid.as_ref()
        else {
            panic!("expected a split");
        };
        assert_eq!(*mid_ratio, Ratio::new(500).unwrap());
        let Layout::Split {
            ratio: inner_ratio, ..
        } = inner.as_ref()
        else {
            panic!("expected a split");
        };
        assert_eq!(*inner_ratio, Ratio::new(700).unwrap());
    }

    #[test]
    fn swap_panes_exchanges_two_panes_leaving_the_rest_of_the_tree_unchanged() {
        let (mut tab, first) = tab_with_one_pane();
        let second = PaneId::from_raw(2);
        let third = PaneId::from_raw(3);
        tab.split_pane(first, Direction::Right, second, PaneKind::Terminal)
            .unwrap();
        tab.split_pane(first, Direction::Down, third, PaneKind::Terminal)
            .unwrap();
        // layout: Horizontal{first: Vertical{first: first, second: third}, second: second}

        tab.swap_panes(first, second).unwrap();

        // first and second have traded places in the order; third is
        // untouched.
        assert_eq!(
            tab.panes().iter().map(super::Pane::id).collect::<Vec<_>>(),
            vec![second, third, first]
        );
    }

    #[test]
    fn panes_are_in_layout_order_after_a_swap() {
        let (mut tab, first) = tab_with_one_pane();
        let second = PaneId::from_raw(2);
        tab.split_pane(first, Direction::Right, second, PaneKind::Terminal)
            .unwrap();

        tab.swap_panes(first, second).unwrap();

        assert_eq!(
            tab.panes().iter().map(super::Pane::id).collect::<Vec<_>>(),
            vec![second, first]
        );
    }

    #[test]
    fn swap_panes_with_an_unknown_pane_is_an_error() {
        let (mut tab, first) = tab_with_one_pane();
        let unknown = PaneId::from_raw(999);

        assert_eq!(
            tab.swap_panes(first, unknown),
            Err(Error::PaneNotFound(unknown))
        );
    }

    #[test]
    fn fill_pane_does_not_change_the_layout_tree() {
        let (mut tab, first) = tab_with_one_pane();
        let second = PaneId::from_raw(2);
        tab.split_pane(first, Direction::Right, second, PaneKind::Terminal)
            .unwrap();
        let layout_before = tab.clone();

        tab.fill_pane(second).unwrap();

        assert_eq!(tab.panes().len(), layout_before.panes().len());
        assert_eq!(
            tab.panes().iter().map(super::Pane::id).collect::<Vec<_>>(),
            layout_before
                .panes()
                .iter()
                .map(super::Pane::id)
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn filling_a_second_pane_replaces_the_first_fill() {
        let (mut tab, first) = tab_with_one_pane();
        let second = PaneId::from_raw(2);
        tab.split_pane(first, Direction::Right, second, PaneKind::Terminal)
            .unwrap();

        tab.fill_pane(first).unwrap();
        tab.fill_pane(second).unwrap();

        assert_eq!(tab.fill(), Some(second));
    }

    #[test]
    fn unfill_clears_it() {
        let (mut tab, first) = tab_with_one_pane();
        tab.fill_pane(first).unwrap();

        tab.unfill();

        assert_eq!(tab.fill(), None);
    }

    #[test]
    fn focus_changes_the_current_pane() {
        let (mut tab, first) = tab_with_one_pane();
        let second = PaneId::from_raw(2);
        tab.split_pane(first, Direction::Right, second, PaneKind::Terminal)
            .unwrap();
        // `split_pane` already made `second` current.
        assert_eq!(tab.current(), second);

        tab.focus(first).unwrap();

        assert_eq!(tab.current(), first);
    }

    #[test]
    fn focus_rejects_an_unknown_pane() {
        let (mut tab, _first) = tab_with_one_pane();
        let unknown = PaneId::from_raw(999);

        assert_eq!(tab.focus(unknown), Err(Error::PaneNotFound(unknown)));
    }

    #[test]
    fn neighbour_returns_the_adjacent_pane_when_one_exists() {
        let (mut tab, first) = tab_with_one_pane();
        let second = PaneId::from_raw(2);
        tab.split_pane(first, Direction::Right, second, PaneKind::Terminal)
            .unwrap();

        assert_eq!(
            tab.neighbour(first, Direction::Right, false),
            DirectionalFocus::Neighbour(second)
        );
    }

    #[test]
    fn neighbour_returns_edge_when_none_exists_and_wrap_is_false() {
        let (mut tab, first) = tab_with_one_pane();
        let second = PaneId::from_raw(2);
        tab.split_pane(first, Direction::Right, second, PaneKind::Terminal)
            .unwrap();

        assert_eq!(
            tab.neighbour(second, Direction::Right, false),
            DirectionalFocus::Edge
        );
    }

    #[test]
    fn neighbour_wraps_to_the_far_end_of_the_tabs_order_when_wrap_is_true() {
        let (mut tab, first) = tab_with_one_pane();
        let second = PaneId::from_raw(2);
        tab.split_pane(first, Direction::Right, second, PaneKind::Terminal)
            .unwrap();

        assert_eq!(
            tab.neighbour(second, Direction::Right, true),
            DirectionalFocus::Wrapped(first)
        );
    }

    #[test]
    fn neighbour_and_wrap_are_both_edge_in_every_direction_for_a_single_pane_tab() {
        let (tab, first) = tab_with_one_pane();

        for direction in [
            Direction::Left,
            Direction::Right,
            Direction::Up,
            Direction::Down,
        ] {
            assert_eq!(
                tab.neighbour(first, direction, true),
                DirectionalFocus::Edge
            );
        }
    }
}
