---
id: TASK-1.36
title: 'Kernel: the layout tree: split, close, resize, swap, move and fill'
status: In Progress
assignee:
  - '@bruk.w5'
created_date: '2026-11-15T01:32:00Z'
updated_date: '2026-11-15T03:54:00Z'
labels:
  - size-3
milestone: m-0
dependencies:
  - TASK-1.35
parent_task_id: TASK-1
type: task
project: bruk-io-steg
asdlc_accepted_from: DRAFT-6
asdlc_branch: task-1.36-kernel-the-layout-tree-split-close
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
doc-15: R9, R78, R95, R96, R100. Bruk confirmed R95 and R96 and set the smallest pane at 20 columns by 4 rows on 2026-11-15.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A split divides only the target pane, in the given direction, and the new pane is the tab's current pane
- [ ] #2 Closing a pane gives its space to the other child of its split, and nothing else moves
- [ ] #3 Resize changes a ratio; swap and move to another tab keep the tree valid
- [ ] #4 Filling a pane changes nothing in the tree, and at most one pane fills a tab
- [ ] #5 A split that would leave either pane under 20 columns by 4 rows is refused, and a resize stops at that floor (R100)
- [ ] #6 A tab whose total size shrinks refuses and closes nothing, and its panes may go under the floor (R100)
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
## Files that change

- `crates/sdk/src/layout.rs` (new). The shapes that cross the API whole (doc-16 R105 names "a layout" as its example): `Orientation { Horizontal, Vertical }` (a split's axis — R95's "a direction"), `Direction { Left, Right, Up, Down }` (R9's "focused by direction"; also used to address a split for resize/split/move, see below) with `direction.orientation() -> Orientation` and `direction.is_before() -> bool` (Left/Up → the near/`first` side), `Ratio` (fixed-point: `Ratio(u16)` in permille, `Ratio::new(permille: u16) -> Option<Self>` valid for `1..=999`, rejecting 0 and 1000 so a child is never zero-sized — fixed-point, not `f64`, so `Layout`, and everything holding it (`Tab`, `Session`), keeps deriving `Eq`/`Hash`, which `tab.rs:23` and `session.rs:20` already derive and must not lose), `Size` (`columns: u16, rows: u16`, private fields + `new`/`columns()`/`rows()`, matching `ids.rs`'s style), and `Layout`:
  ```rust
  pub enum Layout {
      Pane(PaneId),
      Split { orientation: Orientation, ratio: Ratio, first: Box<Layout>, second: Box<Layout> },
  }
  ```
  Doc-15 uses the word "direction" for two different closed sets (R95's split axis, R9's focus movement); the code separates them as `Orientation` and `Direction` and says so in both types' doc comments, so a reviewer reading R95/R9 against the names isn't left to guess why.

  Pure methods on `Layout` (no kernel state, no I/O — usable by the kernel now and by the app later, reading a reported layout the same way):
  - `leaves(&self) -> Vec<PaneId>` — every pane in this layout's stable reading order: depth-first, `first` before `second` at every split. This is TASK-1.38's "list a tab's panes in a stable order for resolving by position."
  - `leaf_sizes(&self, total: Size) -> Vec<(PaneId, Size)>` — walks top-down from `total`, giving each split's two children `first = floor(parent.axis_extent * ratio.permille / 1000)`, `second = parent.axis_extent - first` along the split's axis and the parent's own extent unchanged on the other axis (so a split's two children always sum exactly to its own size, by construction — the rounding rule is stated once, here, rather than left to each caller). This is the one geometry-with-size primitive; every floor check in the kernel (split, resize, move) is built from it, not from three separate helpers.
  - `neighbour(&self, pane: PaneId, direction: Direction) -> Option<PaneId>` — TASK-1.37's "the neighbour of a pane in a direction." Walk up from `pane`'s leaf to the nearest ancestor split whose `orientation == direction.orientation()` and from whose far side `pane` descends (i.e. `pane` is under `first` and `direction.is_before()` is false, or under `second` and `direction.is_before()` is true — the direction points away from `pane`, toward the sibling). No such ancestor → `None`. Found one → descend into the sibling subtree, at every further split preferring the near side (the side adjacent to the boundary just crossed) when its orientation matches `direction`'s axis, and `first` by convention when it's the other axis (documented as a deliberate simplification: true pixel-nearest neighbour needs geometry this shape doesn't carry, and TASK-1.37 can refine it later if that proves wrong in the UI).

- `crates/sdk/src/lib.rs`: add `mod layout;` and `pub use layout::{Direction, Layout, Orientation, Ratio, Size};`.

- `crates/kernel/src/tab.rs`: add fields `layout: Layout`, `current: PaneId`, `fill: Option<PaneId>`, `size: Option<Size>`. `Tab::new` builds `layout: Layout::Pane(first_pane)`, `current: first_pane`, `fill: None`, `size: None` — unchanged signature, so `session.rs` doesn't change. Remove `add_pane` (the flat-Vec push this task's own prior note says it's replacing) and its test. Rewrite `close_pane`: locate the leaf in `layout`, not in `panes`; if it has a parent split, replace the parent with the sibling subtree (this *is* "gives its space to the other child of its split" — a tree-collapse that touches only the one ancestor pointer, so no other split's ratio or direction and no other leaf's position changes, satisfying AC #2's "nothing else moves" by construction) and remove the pane from `panes`; if it has no parent (it was the tab's only pane), report `tab_now_empty: true` without touching `layout` (the tab is deleted by `Tree` immediately after, so no one ever observes a stale layout). In both cases: if the closed pane was `current`, set `current` to `leaves().first()` of whatever is now in its place (the promoted sibling's own first leaf in reading order) — this is TASK-1.37's "current pane stays valid when a pane closes." If the closed pane was `fill`, clear `fill` to `None`.

  Add:
  ```rust
  pub fn panes_in_order(&self) -> Vec<&Pane>          // self.layout.leaves() mapped through self.pane(id)
  pub fn neighbour(&self, pane: PaneId, direction: Direction) -> Option<PaneId>  // self.layout.neighbour(pane, direction)
  pub const fn current(&self) -> PaneId
  pub const fn fill(&self) -> Option<PaneId>
  pub(crate) fn split_pane(&mut self, target: PaneId, direction: Direction, new_id: PaneId, kind: PaneKind) -> Result<(), Error>
  pub(crate) fn resize_pane(&mut self, pane: PaneId, direction: Direction, ratio: Ratio) -> Result<Ratio, Error>
  pub(crate) fn swap_panes(&mut self, a: PaneId, b: PaneId) -> Result<(), Error>
  pub(crate) fn fill_pane(&mut self, pane: PaneId) -> Result<(), Error>
  pub(crate) fn unfill(&mut self)
  pub fn set_size(&mut self, size: Size)
  ```
  `split_pane`: replace the target leaf with `Layout::Split { orientation: direction.orientation(), ratio: Ratio::new(500).expect("500 is a valid permille"), first, second }`, with the *new* pane on `direction`'s side (`direction.is_before()` picks which of `first`/`second` is the target vs. the new pane) and the existing target pane on the other side; push the new `Pane` into `panes`; set `current` to the new pane (AC #1). Floor check (shared with `move_pane_to_tab`, see below): skip entirely if `self.size` is `None` (R100 can't be judged with no data — this is the plan's answer to "what a split does when no size is known": it is allowed, never refused, until a size arrives); otherwise build the hypothetical replacement subtree, call `Layout::leaf_sizes` on it with the target's *own current* size (from `self.layout.leaf_sizes(self.size)` filtered to the target), and refuse (`Error::SplitTooSmall { pane: target }`, no mutation) if either resulting leaf is under 20 columns or 4 rows **in either dimension**, not only the one the split divides — a horizontal split still halves the row count if the *tab* is already short on rows, and R100's floor binds on both axes.

  `resize_pane`: walk up from `pane` to find the split exactly as `neighbour` does (same ancestor rule, so the two share the traversal, not two divergent implementations) — no match is `Error::NoSplitInDirection { pane, direction }`. Accumulate that split's own inherited `Size` while walking up (divide `self.size`, if known, by each ancestor's ratio along the path) — if `self.size` is `None`, skip the floor clamp and just set the ratio (R100 can't be judged, same rule as split). If known: starting from the requested `ratio`, search permille values stepping toward the side that currently satisfies the floor (bounded: at most 999 steps, trivial cost) until `leaf_sizes` of the found split's subtree (using its inherited size and the candidate ratio) has every leaf at or above 20×4; apply and return that clamped `Ratio` — this is R100's "a resize stops at that floor" (clamped, succeeds) as against split's "is refused" (rejected outright); the two are deliberately different outcomes for the same rule.

  `swap_panes`: both panes must resolve to leaves in *this* tab (cross-tab is "moved to another tab", a different capability per R9's own wording, not this method) — exchange the two `Layout::Pane(id)` values in place; nothing else in the tree changes.

  `fill_pane`/`unfill`: set/clear `self.fill`; `fill_pane` on a pane not in this tab is `Error::PaneNotFound`. Neither touches `layout` (AC #4: "filling... changes nothing in the layout tree"); setting a new fill while one is already set just replaces it (R96's "at most one... at a time" is an invariant of the field, not a thing that needs a check).

  `set_size`: stores whatever `Size` it's given, unconditionally — no floor check, ever (AC #6: "refuses and closes nothing"). This is the one-sentence answer to "how does a size reach the kernel as data": nothing in the kernel measures anything; a future caller (the server, wired up by TASK-1.42 using the window's pixels and the font's cell metrics — out of this task's scope) calls `Tree::set_tab_size` whenever it learns a new size, and the kernel just remembers the latest value it was handed.

- `crates/kernel/src/tree.rs`: remove `new_pane` and its three tests (`a_failed_new_pane_does_not_consume_an_id`, `new_pane_adds_a_pane_to_an_existing_tab`, and the `new_pane` call inside `closing_one_of_several_panes_cascades_no_further_than_it_should`, which switches to `split_pane`). Confirmed by `git grep -n "new_pane|add_pane"` in this worktree: every call is inside `crates/kernel` itself (its own tests, or `tree.rs` calling `tab.rs`'s `add_pane`); no other crate in the workspace (`server`, `steg`, `app`) references either name, so removing them breaks nothing outside the kernel.

  Add the public, cascading/validating entry points, each locating the owning tab (as `close_pane` already does) then delegating to the `pub(crate)` method above after minting whatever id it needs from `self.ids`:
  ```rust
  pub fn split_pane(&mut self, target: PaneId, direction: Direction, kind: PaneKind) -> Result<PaneId, Error>
  pub fn resize_pane(&mut self, pane: PaneId, direction: Direction, ratio: Ratio) -> Result<Ratio, Error>
  pub fn swap_panes(&mut self, a: PaneId, b: PaneId) -> Result<(), Error>
  pub fn fill_pane(&mut self, pane: PaneId) -> Result<(), Error>
  pub fn unfill_pane(&mut self, tab: TabId) -> Result<(), Error>
  pub fn set_tab_size(&mut self, tab: TabId, size: Size) -> Result<(), Error>
  pub fn move_pane_to_tab(&mut self, pane: PaneId, target_tab: TabId, direction: Direction) -> Result<MovedPane, Error>
  ```
  `move_pane_to_tab`: refuse `target_tab == pane`'s own tab (`Error::AlreadyInTab`). Otherwise: run the exact same tree-collapse `close_pane` uses on the source tab (promoting the sibling, cascading tab/session closure via the existing `close_pane`/`close_tab` machinery if the source tab/session is now empty) but *without* dropping the `Pane` — move it into the target tab's `panes` — then run the exact same insertion `split_pane` uses on the target tab's *current* pane, with the moved pane (not a freshly-minted one) on `direction`'s side, becoming the target tab's new current pane. Same floor rule as `split_pane` (refused outright if it would violate the floor in the target tab, checked before either tab is mutated — atomic: either both halves of the move happen or neither does). Returns:
  ```rust
  pub struct MovedPane {
      pub pane: PaneId,
      pub to: TabId,
      pub source_tab_closed: Option<TabId>,
      pub source_session_closed: Option<SessionId>,
  }
  ```
  mirroring `ClosedPane`'s shape.

- `crates/kernel/src/error.rs`: add `SplitTooSmall { pane: PaneId }`, `NoSplitInDirection { pane: PaneId, direction: steg_sdk::Direction }`, `PanesNotInSameTab { a: PaneId, b: PaneId }`, `AlreadyInTab { pane: PaneId, tab: TabId }`, each with a `Display` arm — exactly the kind of addition this file's own doc comment already expects from "the layout tree's split, resize and swap."

- `crates/sdk/src/layout.rs` and `crates/kernel/src/tab.rs`/`tree.rs` each get a `#[cfg(test)]` module per `testing-rust.md`: unit tests, no doubles (the kernel and the sdk's `Layout` are both pure logic, so there is nothing to fake).

## Order of work

1. `crates/sdk/src/layout.rs`: `Orientation`, `Direction` (with `orientation()`/`is_before()`), `Ratio`, `Size`, and `Layout::{leaves, leaf_sizes}` with unit tests (reading order over a hand-built tree; `leaf_sizes` rounding sums exactly to `total` on an uneven ratio). Wire into `lib.rs`. This has no kernel dependency, so it's checkable in isolation first.
2. `Layout::neighbour` with unit tests: each of the four directions on a small hand-built tree, including "no neighbour at the tab's edge" and one case nested two levels deep.
3. `Tab`: add the four new fields, rewrite `new`/`close_pane` (collapse + `current`/`fill` repair), delete `add_pane`. Update `close_pane`'s existing tests (they still pass through the same cascade shape) and add the two `current`-validity tests.
4. `Tab::split_pane` (no floor check yet — always allow) and `Tab::panes_in_order`, with AC #1's two tests and the reading-order test.
5. Floor check: add it to `split_pane` (refuse) and to `Tab::set_size`/the "unknown size" path (allow). Tests: refused-under-floor (both the divided and the undivided axis), allowed-with-unknown-size, allowed-with-floor-respected.
6. `Tab::resize_pane` sharing the ancestor walk with `neighbour`, with the clamp search. Tests: changes-the-ratio, stops-at-the-floor (exact boundary), no-split-in-that-direction is an error.
7. `Tab::swap_panes`, `Tab::fill_pane`/`unfill`. Tests per AC #3 (swap) and AC #4 (fill).
8. `Tree`: remove `new_pane`+tests, add the seven public entry points and `move_pane_to_tab`'s cascade-reuse. Tests: AC #3's move-keeps-both-tabs-valid, AC #6's shrink-refuses-nothing.
9. `just check` (fmt, lint, boundaries, licences, affected tests, architecture) and `just test` on `steg-sdk`/`steg-kernel`; fix anything it finds before recording.

## Risks

- **Two representations of "which panes exist" (`panes: Vec<Pane>` and `layout`'s leaves) can desync.** Inherent to the domain split (R105: the layout tree, not each pane's data, is what's reported to clients whole) — not something to design away. Mitigate with a `debug_assert!` in every mutating method that `self.panes.len() == self.layout.leaves().len()` and the id sets match, so a bug surfaces in tests immediately rather than as a silent inconsistency later.
- **The default split/move ratio (500 permille, an even split) is this plan's assumption, not confirmed.** R78 says the split's origin follows the design artifact, which workers can't yet read (doc-10's open question). Nothing here depends on 500 being right — `resize_pane` changes it afterward — but flag it for Bruk/the engineer in case the artifact says otherwise.
- **`neighbour`'s "nearest leaf, `first` by convention across a perpendicular split" rule is a deliberate simplification**, not true pixel-nearest-neighbour (this shape carries no pixel geometry). If TASK-1.37's UI testing shows it picks the wrong pane in a mixed-nesting layout, that's a refinement to `Layout::neighbour`, not a sign this task's shape is wrong.
- **Removing `Tree::new_pane` is a public kernel API change.** Confirmed safe: `git grep -n "new_pane|add_pane"` in this worktree shows every use inside `crates/kernel` itself; no other crate (`server`, `steg`, `app` — none has kernel code yet) and no sibling task's plan depends on its old flat-list behaviour (TASK-1.37 and TASK-1.38 are both downstream of this task and will be built against the new API).
- **Resize's floor check must be scoped to the subtree under the split being resized, not the whole tab**, or AC #6 (an already-shrunk tab may have panes under the floor) would wrongly block an unrelated resize elsewhere in the tree. The plan above computes `leaf_sizes` from that split's own inherited size (accumulated during the same ancestor walk `neighbour` does), not from the tab's root, specifically to avoid this.

## Proof

Commands as named on `main` today (`just --list`): `just test` (the affected crates — `steg-sdk` and `steg-kernel` — via `steg-affected`) while building, `just check` (fmt, lint, boundaries, licences, the affected tests, the tools' tests, the architecture model) before `asdlc record-check`'s final run. `TASK-1.73` may rename these later; this task uses the current names.

Acceptance criterion → test:

- **#1** (split divides only the target, in the given direction, new pane becomes current): `split_pane_divides_only_the_target_pane`, `split_pane_puts_the_new_pane_on_the_requested_side`, `split_pane_makes_the_new_pane_current`.
- **#2** (closing gives space to the sibling, nothing else moves): `closing_a_pane_promotes_its_siblings_subtree_to_its_parents_place`, `closing_a_pane_leaves_every_other_panes_position_unchanged` (via `panes_in_order`/structural equality on the untouched branch), `closing_the_current_pane_moves_current_to_a_pane_that_still_exists`, `closing_a_filled_pane_clears_the_fill`.
- **#3** (resize changes a ratio; swap and move keep the tree valid): `resize_pane_changes_the_bordering_splits_ratio`, `swap_panes_exchanges_two_panes_leaving_the_rest_of_the_tree_unchanged`, `move_pane_to_tab_leaves_both_tabs_with_a_valid_tree`, `move_pane_to_tab_cascades_closing_the_source_tab_when_it_was_the_last_pane`.
- **#4** (fill changes nothing in the tree; at most one at a time): `fill_pane_does_not_change_the_layout_tree`, `filling_a_second_pane_replaces_the_first_fill`, `unfill_clears_it`.
- **#5** (split refused under the floor; resize stops at it): `split_pane_is_refused_when_either_side_would_be_under_the_floor`, `split_pane_is_refused_when_the_undivided_dimension_is_already_under_the_floor`, `resize_pane_stops_exactly_at_the_floor_instead_of_going_below_it`, `split_pane_is_allowed_when_the_tabs_size_is_unknown`.
- **#6** (shrinking refuses and closes nothing; panes may go under the floor): `set_tab_size_smaller_always_succeeds_and_closes_nothing`, `a_resize_elsewhere_is_not_blocked_by_an_unrelated_pane_already_under_the_floor`.

Plus the sdk-level unit tests from steps 1-2 above (`leaves` order, `leaf_sizes` rounding, `neighbour` in all four directions and at an edge).
<!-- SECTION:PLAN:END -->
