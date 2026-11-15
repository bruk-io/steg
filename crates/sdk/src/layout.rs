//! A tab's layout: the tree of splits and panes doc-15 R95 describes, and
//! the shapes around it that cross the API (doc-16 R105).
//!
//! `Orientation` is a split's own axis (R95: "a split... with a direction").
//! `Direction` is the four-way compass doc-15 R9 ("focused by direction")
//! and R113 (a pane's neighbour) use, and the one TASK-1.37's planner asked
//! for. Doc-15 overloads the English word "direction" for both; the two are
//! named distinctly here so the code never confuses them.

use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::ids::PaneId;

/// A split's own axis (R95).
///
/// Crosses the API as its own word (`"horizontal"`, `"vertical"`), same as
/// [`crate::Colour`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Orientation {
    /// The split's two children sit side by side: a left pane and a right
    /// pane.
    Horizontal,
    /// The split's two children sit one above the other: a top pane and a
    /// bottom pane.
    Vertical,
}

/// The four-way compass a pane is focused, resized, split or moved toward
/// (R9, R113) — distinct from [`Orientation`], a split's own axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Direction {
    /// Toward smaller columns.
    Left,
    /// Toward larger columns.
    Right,
    /// Toward smaller rows.
    Up,
    /// Toward larger rows.
    Down,
}

impl Direction {
    /// The axis this direction moves along: [`Orientation::Horizontal`]
    /// for [`Direction::Left`]/[`Direction::Right`],
    /// [`Orientation::Vertical`] for [`Direction::Up`]/[`Direction::Down`].
    #[must_use]
    pub const fn orientation(self) -> Orientation {
        match self {
            Direction::Left | Direction::Right => Orientation::Horizontal,
            Direction::Up | Direction::Down => Orientation::Vertical,
        }
    }

    /// Whether this direction names the near, `first` side of a split
    /// along its axis (`Left` and `Up`), rather than the far, `second`
    /// side (`Right` and `Down`).
    #[must_use]
    pub const fn is_before(self) -> bool {
        matches!(self, Direction::Left | Direction::Up)
    }
}

/// A split's ratio between its two children: a permille (parts per
/// thousand) strictly between 0 and 1000, so neither child is ever
/// zero-sized (R95: "a ratio between 0 and 1").
///
/// Fixed-point, not `f64`, so every type that holds one — [`Layout`], and
/// the kernel's `Tab` and `Session` above it — keeps deriving `Eq` and
/// `Hash`. That is the in-process representation only: R95 doesn't mandate
/// it, and the wire form (`Serialize`/`Deserialize`, below) is the plain
/// fraction doc-15 R95 and doc-14 describe — an `f64` strictly between 0
/// and 1 — converted to and from permille at the edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Ratio(u16);

impl Ratio {
    /// Builds a ratio from a permille value, `None` unless it is strictly
    /// between 0 and 1000.
    #[must_use]
    pub const fn new(permille: u16) -> Option<Self> {
        if permille > 0 && permille < 1000 {
            Some(Self(permille))
        } else {
            None
        }
    }

    /// This ratio's permille value, always in `1..=999`.
    #[must_use]
    pub const fn permille(self) -> u16 {
        self.0
    }
}

impl Serialize for Ratio {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_f64(f64::from(self.0) / 1000.0)
    }
}

impl<'de> Deserialize<'de> for Ratio {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = f64::deserialize(deserializer)?;
        if !(value > 0.0 && value < 1.0) {
            return Err(D::Error::custom(format!(
                "{value} is not a ratio strictly between 0 and 1"
            )));
        }
        let permille = (value * 1000.0).round();
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "permille is finite and strictly within 0.0..1000.0, checked above"
        )]
        let permille = permille as u16;
        Ratio::new(permille).ok_or_else(|| {
            D::Error::custom(format!("{value} is not a ratio strictly between 0 and 1"))
        })
    }
}

/// A terminal's size in whole cells (R91, R100): columns and rows, never
/// pixels — the kernel does no I/O and reads no screen, so this is always
/// a plain value a caller hands in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Size {
    columns: u16,
    rows: u16,
}

impl Size {
    /// Builds a size from a column and row count.
    #[must_use]
    pub const fn new(columns: u16, rows: u16) -> Self {
        Self { columns, rows }
    }

    /// This size's column count.
    #[must_use]
    pub const fn columns(self) -> u16 {
        self.columns
    }

    /// This size's row count.
    #[must_use]
    pub const fn rows(self) -> u16 {
        self.rows
    }
}

/// A tab's layout: a tree whose leaves are panes and whose other nodes are
/// splits (R95). Reported to clients whole (doc-16 R105), so this shape —
/// not just the pane data at its leaves — lives in the sdk.
///
/// Serde's default, externally-tagged representation (`{"pane": "<id>"}` /
/// `{"split": {...}}`) is used, not an internally-tagged one: an internal
/// tag needs every variant to serialize as a map, and `Pane`'s single
/// `PaneId` field serializes as a plain string.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Layout {
    /// A leaf: one pane fills this part of the tab.
    Pane(PaneId),
    /// A split: exactly two children, arranged along `orientation` at
    /// `ratio`.
    Split {
        /// The split's axis.
        orientation: Orientation,
        /// The split's ratio between its two children.
        ratio: Ratio,
        /// The near (left, for `Horizontal`; top, for `Vertical`) child.
        first: Box<Layout>,
        /// The far (right, for `Horizontal`; bottom, for `Vertical`) child.
        second: Box<Layout>,
    },
}

impl Layout {
    /// Every pane in this layout, in doc-15 R112's order: depth-first,
    /// `first` before `second` at every split. A pane's *position* — what
    /// a target like "the second pane" resolves against — is this order,
    /// counted from one, not creation order.
    #[must_use]
    pub fn leaves(&self) -> Vec<PaneId> {
        match self {
            Layout::Pane(id) => vec![*id],
            Layout::Split { first, second, .. } => {
                let mut leaves = first.leaves();
                leaves.extend(second.leaves());
                leaves
            }
        }
    }

    /// Every leaf's absolute size, given the tab's total size: walks
    /// top-down from `total`, giving each split's two children
    /// `first = floor(parent_extent * ratio.permille() / 1000)` and
    /// `second = parent_extent - first` along the split's own axis, and
    /// the parent's extent on the other axis unchanged — so a split's two
    /// children always sum exactly to its own size.
    #[must_use]
    pub fn leaf_sizes(&self, total: Size) -> Vec<(PaneId, Size)> {
        match self {
            Layout::Pane(id) => vec![(*id, total)],
            Layout::Split {
                orientation,
                ratio,
                first,
                second,
            } => {
                let (first_total, second_total) = split_size(*orientation, *ratio, total);
                let mut sizes = first.leaf_sizes(first_total);
                sizes.extend(second.leaf_sizes(second_total));
                sizes
            }
        }
    }

    /// The nearest ancestor split of `pane` whose [`Orientation`] matches
    /// `direction`'s, and from whose far side `pane` descends — `pane` is
    /// under `first` and `direction.is_before()` is `false` (the
    /// direction points away from `first`, toward `second`), or `pane` is
    /// under `second` and `direction.is_before()` is `true`. `None` if no
    /// such ancestor exists (`pane` is at the tab's own edge on that
    /// axis, or is not in this layout at all).
    ///
    /// This is `resize_pane`'s own primitive — `neighbour` and
    /// `far_edge_pane`, below, answer their question with exact rectangle
    /// geometry instead, not this ancestor walk.
    #[must_use]
    pub fn bordering_split(&self, pane: PaneId, direction: Direction) -> Option<&Layout> {
        bordering_split_ref(self, pane, direction)
    }

    /// The mutable counterpart of [`Layout::bordering_split`].
    pub fn bordering_split_mut(
        &mut self,
        pane: PaneId,
        direction: Direction,
    ) -> Option<&mut Layout> {
        // Computed as a path (immutably) and then re-walked once, rather
        // than recursing with `&mut` directly: a function that might
        // return either a deeply-nested `&mut` or, on a different
        // branch, the outer `&mut` it started from can't satisfy the
        // borrow checker, which must conservatively tie every returned
        // borrow to the whole call's lifetime. A path has no such
        // branching — it's one straight-line descent.
        let path = bordering_split_path(self, pane, direction)?;
        Some(layout_at_path_mut(self, &path))
    }

    /// The absolute size the split [`Layout::bordering_split`] would find
    /// inherits from its own ancestors: walks the same path, dividing
    /// `total` by each ancestor's ratio, but stops at that split rather
    /// than continuing to leaves. `leaf_sizes` alone can't give this for
    /// an internal node — a node's own inherited extent isn't any one
    /// descendant leaf's size once there's further nesting on the
    /// perpendicular axis.
    #[must_use]
    pub fn inherited_size(&self, pane: PaneId, direction: Direction, total: Size) -> Option<Size> {
        inherited_size_at(self, pane, direction, total)
    }

    /// The pane adjacent to `pane` on `direction`'s side, found by exact
    /// rectangle geometry, not reading order: every leaf has a rectangle
    /// in the tab's own unit square (the root's rectangle is the whole
    /// square; a split divides its own rectangle along its axis at its
    /// ratio — the same top-down shape `leaf_sizes` walks, but kept as an
    /// exact fraction rather than rounded to a whole cell, since touching
    /// has to be an exact comparison). The neighbour is whichever other
    /// leaf's rectangle touches `pane`'s on `direction`'s side *and*
    /// overlaps it on the other axis; when several qualify (R113), the
    /// one whose overlap starts first — nearest the top for `Left`/
    /// `Right`, nearest the left for `Up`/`Down`. `None` if nothing
    /// touches that side.
    #[must_use]
    pub fn neighbour(&self, pane: PaneId, direction: Direction) -> Option<PaneId> {
        let rects = self.leaf_rects();
        let this_rect = rect_of(&rects, pane)?;
        rects
            .into_iter()
            .filter(|(id, _)| *id != pane)
            .filter(|(_, rect)| touches(*rect, this_rect, direction))
            .filter(|(_, rect)| overlaps(*rect, this_rect, direction))
            .min_by_key(|(_, rect)| tie_break_key(*rect, this_rect, direction))
            .map(|(id, _)| id)
    }

    /// The wrap target for `pane` moving `direction` off the edge of the
    /// tab: among every *other* leaf, the ones touching the tab's own
    /// edge opposite `direction` and overlapping `pane`'s span on the
    /// other axis, same tie-break as [`Layout::neighbour`]. `None` if
    /// `pane` is itself the only leaf touching that edge — a one-pane tab
    /// trivially touches every edge of its own square, and wrapping is
    /// only ever found among *other* leaves, so there is nothing to wrap
    /// to.
    #[must_use]
    pub fn far_edge_pane(&self, pane: PaneId, direction: Direction) -> Option<PaneId> {
        let rects = self.leaf_rects();
        let this_rect = rect_of(&rects, pane)?;
        rects
            .into_iter()
            .filter(|(id, _)| *id != pane)
            .filter(|(_, rect)| touches_far_edge(*rect, direction))
            .filter(|(_, rect)| overlaps(*rect, this_rect, direction))
            .min_by_key(|(_, rect)| tie_break_key(*rect, this_rect, direction))
            .map(|(id, _)| id)
    }

    /// Every leaf's exact rectangle in the tab's unit square — the
    /// geometric counterpart of `leaf_sizes`, kept as its own function
    /// (not generalised over both) so `leaf_sizes`'s whole-cell rounding
    /// never leaks into a comparison that must be exact.
    fn leaf_rects(&self) -> Vec<(PaneId, Rect)> {
        let whole = Rect {
            x0: Fraction::ZERO,
            x1: Fraction::ONE,
            y0: Fraction::ZERO,
            y1: Fraction::ONE,
        };
        let mut out = Vec::new();
        collect_leaf_rects(self, whole, &mut out);
        out
    }
}

fn rect_of(rects: &[(PaneId, Rect)], pane: PaneId) -> Option<Rect> {
    rects
        .iter()
        .find(|(id, _)| *id == pane)
        .map(|(_, rect)| *rect)
}

// Each of these three walks must return the *nearest* (innermost)
// qualifying ancestor split, not the first one found top-down from the
// root: a split further out can share the same orientation as a nested
// one closer to `pane`, and the nested one is the real border. So each
// recurses into whichever child actually holds `pane` *first*, and only
// checks whether the current level itself qualifies when that recursive
// call comes back empty — a deeper match always wins.

fn bordering_split_ref(layout: &Layout, pane: PaneId, direction: Direction) -> Option<&Layout> {
    let Layout::Split {
        orientation,
        first,
        second,
        ..
    } = layout
    else {
        return None;
    };

    if first.leaves().contains(&pane) {
        if let Some(found) = bordering_split_ref(first, pane, direction) {
            return Some(found);
        }
        if *orientation == direction.orientation() && !direction.is_before() {
            return Some(layout);
        }
        return None;
    }
    if second.leaves().contains(&pane) {
        if let Some(found) = bordering_split_ref(second, pane, direction) {
            return Some(found);
        }
        if *orientation == direction.orientation() && direction.is_before() {
            return Some(layout);
        }
        return None;
    }
    None
}

/// The sequence of child choices (`false` for `first`, `true` for
/// `second`) from the root down to the split [`Layout::bordering_split`]
/// would find. `bordering_split_mut` re-walks this path with a single
/// mutable descent, rather than recursing with `&mut` directly and
/// branching between "return what a child found" and "return myself" —
/// see its own doc comment for why that doesn't borrow-check.
fn bordering_split_path(layout: &Layout, pane: PaneId, direction: Direction) -> Option<Vec<bool>> {
    let Layout::Split {
        orientation,
        first,
        second,
        ..
    } = layout
    else {
        return None;
    };

    if first.leaves().contains(&pane) {
        if let Some(mut path) = bordering_split_path(first, pane, direction) {
            path.insert(0, false);
            return Some(path);
        }
        if *orientation == direction.orientation() && !direction.is_before() {
            return Some(Vec::new());
        }
        return None;
    }
    if second.leaves().contains(&pane) {
        if let Some(mut path) = bordering_split_path(second, pane, direction) {
            path.insert(0, true);
            return Some(path);
        }
        if *orientation == direction.orientation() && direction.is_before() {
            return Some(Vec::new());
        }
        return None;
    }
    None
}

fn layout_at_path_mut<'a>(layout: &'a mut Layout, path: &[bool]) -> &'a mut Layout {
    let [step, rest @ ..] = path else {
        return layout;
    };
    let Layout::Split { first, second, .. } = layout else {
        unreachable!("a path from `bordering_split_path` only ever names real splits")
    };
    layout_at_path_mut(if *step { second } else { first }, rest)
}

fn inherited_size_at(
    layout: &Layout,
    pane: PaneId,
    direction: Direction,
    total: Size,
) -> Option<Size> {
    let Layout::Split {
        orientation,
        ratio,
        first,
        second,
        ..
    } = layout
    else {
        return None;
    };
    let (first_total, second_total) = split_size(*orientation, *ratio, total);

    if first.leaves().contains(&pane) {
        if let Some(found) = inherited_size_at(first, pane, direction, first_total) {
            return Some(found);
        }
        if *orientation == direction.orientation() && !direction.is_before() {
            return Some(total);
        }
        return None;
    }
    if second.leaves().contains(&pane) {
        if let Some(found) = inherited_size_at(second, pane, direction, second_total) {
            return Some(found);
        }
        if *orientation == direction.orientation() && direction.is_before() {
            return Some(total);
        }
        return None;
    }
    None
}

fn split_size(orientation: Orientation, ratio: Ratio, total: Size) -> (Size, Size) {
    match orientation {
        Orientation::Horizontal => {
            let first_columns = split_extent(total.columns, ratio);
            (
                Size::new(first_columns, total.rows),
                Size::new(total.columns - first_columns, total.rows),
            )
        }
        Orientation::Vertical => {
            let first_rows = split_extent(total.rows, ratio);
            (
                Size::new(total.columns, first_rows),
                Size::new(total.columns, total.rows - first_rows),
            )
        }
    }
}

/// `floor(extent * ratio.permille() / 1000)`. Always `<= extent`, since
/// `ratio.permille() < 1000`, so the truncating cast back to `u16` never
/// loses anything.
fn split_extent(extent: u16, ratio: Ratio) -> u16 {
    let scaled = u32::from(extent) * u32::from(ratio.permille()) / 1000;
    debug_assert!(scaled <= u32::from(extent));
    scaled as u16
}

/// An exact fraction, always stored reduced with a positive denominator —
/// so two fractions computed from the same ratios are equal exactly when
/// they are the same value, never approximately. `i128` is exact for any
/// layout tree a terminal UI will realistically have; nothing here checks
/// for a tree deep enough, and with ratios awkward enough, to overflow it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Fraction {
    numerator: i128,
    denominator: i128,
}

impl Fraction {
    const ZERO: Fraction = Fraction {
        numerator: 0,
        denominator: 1,
    };
    const ONE: Fraction = Fraction {
        numerator: 1,
        denominator: 1,
    };

    fn new(numerator: i128, denominator: i128) -> Self {
        debug_assert!(denominator != 0, "a fraction's denominator is never zero");
        let sign: i128 = if denominator < 0 { -1 } else { 1 };
        let numerator = numerator * sign;
        let denominator = denominator * sign;
        let divisor = gcd(numerator.unsigned_abs(), denominator.unsigned_abs()).max(1);
        #[expect(
            clippy::cast_possible_wrap,
            reason = "divisor is a gcd of two i128's magnitudes, so it fits back in i128"
        )]
        let divisor = divisor as i128;
        Fraction {
            numerator: numerator / divisor,
            denominator: denominator / divisor,
        }
    }

    fn add(self, other: Fraction) -> Fraction {
        Fraction::new(
            self.numerator * other.denominator + other.numerator * self.denominator,
            self.denominator * other.denominator,
        )
    }

    fn sub(self, other: Fraction) -> Fraction {
        Fraction::new(
            self.numerator * other.denominator - other.numerator * self.denominator,
            self.denominator * other.denominator,
        )
    }

    fn mul_ratio(self, ratio: Ratio) -> Fraction {
        Fraction::new(
            self.numerator * i128::from(ratio.permille()),
            self.denominator * 1000,
        )
    }
}

impl PartialOrd for Fraction {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Fraction {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // Denominators are always positive (`new` normalises the sign),
        // so cross-multiplying preserves order.
        (self.numerator * other.denominator).cmp(&(other.numerator * self.denominator))
    }
}

fn gcd(a: u128, b: u128) -> u128 {
    if b == 0 { a } else { gcd(b, a % b) }
}

/// A leaf's exact rectangle in the tab's unit square: `x` the column
/// axis, `y` the row axis, both running `0..1`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Rect {
    x0: Fraction,
    x1: Fraction,
    y0: Fraction,
    y1: Fraction,
}

fn collect_leaf_rects(layout: &Layout, rect: Rect, out: &mut Vec<(PaneId, Rect)>) {
    match layout {
        Layout::Pane(id) => out.push((*id, rect)),
        Layout::Split {
            orientation,
            ratio,
            first,
            second,
        } => {
            let (first_rect, second_rect) = split_rect(*orientation, *ratio, rect);
            collect_leaf_rects(first, first_rect, out);
            collect_leaf_rects(second, second_rect, out);
        }
    }
}

fn split_rect(orientation: Orientation, ratio: Ratio, rect: Rect) -> (Rect, Rect) {
    match orientation {
        Orientation::Horizontal => {
            let mid = rect.x0.add(rect.x1.sub(rect.x0).mul_ratio(ratio));
            (
                Rect {
                    x0: rect.x0,
                    x1: mid,
                    y0: rect.y0,
                    y1: rect.y1,
                },
                Rect {
                    x0: mid,
                    x1: rect.x1,
                    y0: rect.y0,
                    y1: rect.y1,
                },
            )
        }
        Orientation::Vertical => {
            let mid = rect.y0.add(rect.y1.sub(rect.y0).mul_ratio(ratio));
            (
                Rect {
                    x0: rect.x0,
                    x1: rect.x1,
                    y0: rect.y0,
                    y1: mid,
                },
                Rect {
                    x0: rect.x0,
                    x1: rect.x1,
                    y0: mid,
                    y1: rect.y1,
                },
            )
        }
    }
}

fn touches(candidate: Rect, pane: Rect, direction: Direction) -> bool {
    match direction {
        Direction::Right => candidate.x0 == pane.x1,
        Direction::Left => candidate.x1 == pane.x0,
        Direction::Down => candidate.y0 == pane.y1,
        Direction::Up => candidate.y1 == pane.y0,
    }
}

fn touches_far_edge(candidate: Rect, direction: Direction) -> bool {
    match direction {
        Direction::Right => candidate.x0 == Fraction::ZERO,
        Direction::Left => candidate.x1 == Fraction::ONE,
        Direction::Down => candidate.y0 == Fraction::ZERO,
        Direction::Up => candidate.y1 == Fraction::ONE,
    }
}

fn overlaps(candidate: Rect, pane: Rect, direction: Direction) -> bool {
    match direction {
        Direction::Left | Direction::Right => candidate.y0.max(pane.y0) < candidate.y1.min(pane.y1),
        Direction::Up | Direction::Down => candidate.x0.max(pane.x0) < candidate.x1.min(pane.x1),
    }
}

fn tie_break_key(candidate: Rect, pane: Rect, direction: Direction) -> Fraction {
    match direction {
        Direction::Left | Direction::Right => candidate.y0.max(pane.y0),
        Direction::Up | Direction::Down => candidate.x0.max(pane.x0),
    }
}

#[cfg(test)]
mod tests {
    use super::{Direction, Layout, Orientation, Ratio, Size};
    use crate::ids::PaneId;

    fn pane(raw: u64) -> PaneId {
        PaneId::from_raw(raw)
    }

    #[test]
    fn orientation_serde_round_trips_through_its_own_word() {
        for orientation in [Orientation::Horizontal, Orientation::Vertical] {
            let json = serde_json::to_string(&orientation).unwrap();
            assert_eq!(
                serde_json::from_str::<Orientation>(&json).unwrap(),
                orientation
            );
        }
        assert_eq!(
            serde_json::to_string(&Orientation::Horizontal).unwrap(),
            "\"horizontal\""
        );
    }

    #[test]
    fn ratio_serialises_as_a_fraction_between_zero_and_one() {
        assert_eq!(
            serde_json::to_string(&Ratio::new(500).unwrap()).unwrap(),
            "0.5"
        );
    }

    #[test]
    fn ratio_round_trips_through_an_uneven_value() {
        // 333/1000 does not divide evenly; this is the case most likely to
        // expose a rounding bug in the f64 conversion.
        let ratio = Ratio::new(333).unwrap();
        let json = serde_json::to_string(&ratio).unwrap();
        let back: Ratio = serde_json::from_str(&json).unwrap();
        assert_eq!(back.permille(), 333);
    }

    #[test]
    fn ratio_round_trips_through_several_values() {
        for permille in [1_u16, 42, 250, 500, 667, 999] {
            let ratio = Ratio::new(permille).unwrap();
            let json = serde_json::to_string(&ratio).unwrap();
            let back: Ratio = serde_json::from_str(&json).unwrap();
            assert_eq!(back.permille(), permille, "round-tripping {json}");
        }
    }

    #[test]
    fn ratio_deserialize_rejects_zero_and_one() {
        assert!(serde_json::from_str::<Ratio>("0.0").is_err());
        assert!(serde_json::from_str::<Ratio>("1.0").is_err());
    }

    #[test]
    fn ratio_deserialize_rejects_outside_zero_to_one() {
        assert!(serde_json::from_str::<Ratio>("-0.5").is_err());
        assert!(serde_json::from_str::<Ratio>("1.5").is_err());
    }

    #[test]
    fn layout_serde_round_trips_a_split_with_two_leaves() {
        let layout = Layout::Split {
            orientation: Orientation::Horizontal,
            ratio: Ratio::new(500).unwrap(),
            first: Box::new(Layout::Pane(pane(1))),
            second: Box::new(Layout::Pane(pane(2))),
        };

        let json = serde_json::to_string(&layout).unwrap();
        assert_eq!(
            serde_json::from_str::<Layout>(&json).unwrap(),
            layout,
            "round-tripped through {json}"
        );
    }

    #[test]
    fn layout_pane_leaf_serialises_as_an_object_keyed_pane() {
        let json = serde_json::to_value(Layout::Pane(pane(1))).unwrap();
        assert!(json.get("pane").is_some(), "{json}");
    }

    fn split(orientation: Orientation, permille: u16, first: Layout, second: Layout) -> Layout {
        Layout::Split {
            orientation,
            ratio: Ratio::new(permille).unwrap(),
            first: Box::new(first),
            second: Box::new(second),
        }
    }

    #[test]
    fn leaves_are_depth_first_first_before_second() {
        let layout = split(
            Orientation::Horizontal,
            500,
            Layout::Pane(pane(1)),
            split(
                Orientation::Vertical,
                500,
                Layout::Pane(pane(2)),
                Layout::Pane(pane(3)),
            ),
        );

        assert_eq!(layout.leaves(), vec![pane(1), pane(2), pane(3)]);
    }

    #[test]
    fn leaf_sizes_sum_exactly_to_the_total_on_an_uneven_ratio() {
        let layout = split(
            Orientation::Horizontal,
            333,
            Layout::Pane(pane(1)),
            Layout::Pane(pane(2)),
        );

        let sizes = layout.leaf_sizes(Size::new(100, 10));

        let total_columns: u16 = sizes.iter().map(|(_, size)| size.columns()).sum();
        assert_eq!(total_columns, 100);
        assert!(sizes.iter().all(|(_, size)| size.rows() == 10));
        assert_eq!(sizes[0].1.columns(), 33);
        assert_eq!(sizes[1].1.columns(), 67);
    }

    fn grid() -> Layout {
        // A clean 2x2 grid: two columns, each split top/bottom.
        split(
            Orientation::Horizontal,
            500,
            split(
                Orientation::Vertical,
                500,
                Layout::Pane(pane(1)), // top-left
                Layout::Pane(pane(2)), // bottom-left
            ),
            split(
                Orientation::Vertical,
                500,
                Layout::Pane(pane(3)), // top-right
                Layout::Pane(pane(4)), // bottom-right
            ),
        )
    }

    #[test]
    fn neighbour_moving_right_from_bottom_left_lands_on_bottom_right_not_top_right() {
        let layout = grid();

        assert_eq!(layout.neighbour(pane(2), Direction::Right), Some(pane(4)));
    }

    #[test]
    fn neighbour_from_a_pane_spanning_two_rows_takes_the_upper_pane_across_a_column_split() {
        let layout = split(
            Orientation::Horizontal,
            500,
            Layout::Pane(pane(1)), // tall, spans both rows
            split(
                Orientation::Vertical,
                500,
                Layout::Pane(pane(2)), // upper
                Layout::Pane(pane(3)), // lower
            ),
        );

        assert_eq!(layout.neighbour(pane(1), Direction::Right), Some(pane(2)));
    }

    #[test]
    fn neighbour_is_none_at_the_tabs_own_edge() {
        let layout = split(
            Orientation::Horizontal,
            500,
            Layout::Pane(pane(1)),
            Layout::Pane(pane(2)),
        );

        assert_eq!(layout.neighbour(pane(1), Direction::Left), None);
        assert_eq!(layout.neighbour(pane(2), Direction::Right), None);
    }

    #[test]
    fn neighbour_is_none_for_a_single_pane_tab_in_every_direction() {
        let layout = Layout::Pane(pane(1));

        for direction in [
            Direction::Left,
            Direction::Right,
            Direction::Up,
            Direction::Down,
        ] {
            assert_eq!(layout.neighbour(pane(1), direction), None);
        }
    }

    #[test]
    fn wrap_from_the_rightmost_pane_lands_on_the_leftmost_pane_in_its_own_row_not_another_row() {
        let layout = grid();

        // pane(4) is bottom-right; wrapping Right should land on the
        // leftmost pane in the *bottom* row (pane(2)), not the top row.
        assert_eq!(
            layout.far_edge_pane(pane(4), Direction::Right),
            Some(pane(2))
        );
    }

    #[test]
    fn far_edge_pane_is_none_for_a_single_pane_tab_in_every_direction() {
        let layout = Layout::Pane(pane(1));

        for direction in [
            Direction::Left,
            Direction::Right,
            Direction::Up,
            Direction::Down,
        ] {
            assert_eq!(layout.far_edge_pane(pane(1), direction), None);
        }
    }

    #[test]
    fn leaf_rects_gives_adjacent_leaves_exactly_equal_touching_boundaries() {
        // bottom-left's right edge and bottom-right's left edge must be
        // the exact same Fraction for `neighbour`'s `==` test to find
        // them at all; this is an indirect check of that exactness via
        // the public `neighbour`/`far_edge_pane` surface, since the
        // `Fraction`/`Rect` types themselves are private.
        let layout = grid();

        assert_eq!(layout.neighbour(pane(2), Direction::Right), Some(pane(4)));
        assert_eq!(layout.neighbour(pane(4), Direction::Left), Some(pane(2)));
    }

    #[test]
    fn bordering_split_finds_the_nearest_ancestor_matching_the_directions_axis() {
        let layout = grid();

        let found = layout
            .bordering_split(pane(2), Direction::Right)
            .expect("bottom-left has a bordering split to its right");
        assert!(matches!(
            found,
            Layout::Split {
                orientation: Orientation::Horizontal,
                ..
            }
        ));
    }

    #[test]
    fn bordering_split_is_none_at_the_tabs_own_edge() {
        let layout = grid();

        assert!(layout.bordering_split(pane(2), Direction::Left).is_none());
    }

    #[test]
    fn inherited_size_gives_a_splits_own_rectangle_before_its_own_ratio() {
        let layout = grid();

        let size = layout
            .inherited_size(pane(2), Direction::Right, Size::new(100, 20))
            .expect("bottom-left has a bordering split to its right");
        // The root split's own inherited size is the whole tab.
        assert_eq!(size, Size::new(100, 20));
    }

    /// `outer{mid{inner{A,B},C},D}`, all `Horizontal` — three ancestor
    /// splits of the *same* orientation stacked on top of one another.
    /// This is exactly the shape that exposed a mid-build bug: an
    /// earlier version of the ancestor walk matched the *outermost*
    /// qualifying split (here, `outer`) instead of the nearest one
    /// (`inner`), because it checked a node's own match before trying
    /// its child's. Named to match the review finding's own example.
    fn deeply_nested_same_orientation() -> (Layout, PaneId, PaneId, PaneId, PaneId) {
        let a = pane(1);
        let b = pane(2);
        let c = pane(3);
        let d = pane(4);
        let inner = split(
            Orientation::Horizontal,
            500,
            Layout::Pane(a),
            Layout::Pane(b),
        );
        let mid = split(Orientation::Horizontal, 500, inner, Layout::Pane(c));
        let outer = split(Orientation::Horizontal, 500, mid, Layout::Pane(d));
        (outer, a, c, d, b)
    }

    fn leaf_id(layout: &Layout) -> PaneId {
        match layout {
            Layout::Pane(id) => *id,
            Layout::Split { .. } => panic!("expected a leaf, found a split"),
        }
    }

    #[test]
    fn bordering_split_finds_the_innermost_ancestor_not_the_outermost_at_the_deepest_level() {
        let (layout, a, _c, _d, b) = deeply_nested_same_orientation();

        let found = layout
            .bordering_split(a, Direction::Right)
            .expect("A has a bordering split to its right");
        // The innermost split (A|B), not mid (inner|C) or outer (mid|D).
        let Layout::Split { first, second, .. } = found else {
            panic!("expected a split");
        };
        assert_eq!(leaf_id(first), a);
        assert_eq!(leaf_id(second), b);
    }

    #[test]
    fn bordering_split_finds_the_matching_ancestor_at_a_middle_depth() {
        let (layout, _a, c, _d, _b) = deeply_nested_same_orientation();

        // C is `mid`'s second child; moving Left from C borders `mid`
        // itself (inner|C), not `outer`.
        let found = layout
            .bordering_split(c, Direction::Left)
            .expect("C has a bordering split to its left");
        let Layout::Split { first, second, .. } = found else {
            panic!("expected a split");
        };
        assert_eq!(leaf_id(second), c);
        assert!(matches!(first.as_ref(), Layout::Split { .. }));
    }

    #[test]
    fn bordering_split_finds_the_outermost_split_for_a_leaf_at_the_top_level() {
        let (layout, _a, _c, d, _b) = deeply_nested_same_orientation();

        // D is `outer`'s second child; moving Left from D borders
        // `outer` itself — the only correct answer at this depth.
        let found = layout
            .bordering_split(d, Direction::Left)
            .expect("D has a bordering split to its left");
        let Layout::Split { second, .. } = found else {
            panic!("expected a split");
        };
        assert_eq!(leaf_id(second), d);
    }

    #[test]
    fn bordering_split_mut_also_finds_the_innermost_ancestor() {
        let (mut layout, a, _c, _d, b) = deeply_nested_same_orientation();

        let found = layout
            .bordering_split_mut(a, Direction::Right)
            .expect("A has a bordering split to its right");
        let Layout::Split { first, second, .. } = found else {
            panic!("expected a split");
        };
        assert_eq!(leaf_id(first), a);
        assert_eq!(leaf_id(second), b);
    }

    #[test]
    fn inherited_size_matches_the_innermost_bordering_split_at_each_depth() {
        let (layout, a, c, d, _b) = deeply_nested_same_orientation();
        let total = Size::new(800, 10);

        // A's bordering split is `inner`, which inherits half of `mid`'s
        // share, which is half of `outer`'s share: 800 / 2 / 2 = 200.
        assert_eq!(
            layout.inherited_size(a, Direction::Right, total),
            Some(Size::new(200, 10))
        );
        // C's bordering split is `mid`, which inherits half of `outer`'s
        // share: 800 / 2 = 400.
        assert_eq!(
            layout.inherited_size(c, Direction::Left, total),
            Some(Size::new(400, 10))
        );
        // D's bordering split is `outer`, the root: the whole tab.
        assert_eq!(
            layout.inherited_size(d, Direction::Left, total),
            Some(total)
        );
    }
}
