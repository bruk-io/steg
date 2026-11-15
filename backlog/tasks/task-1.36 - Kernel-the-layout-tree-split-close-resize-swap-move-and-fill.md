---
id: TASK-1.36
title: 'Kernel: the layout tree: split, close, resize, swap, move and fill'
status: In Progress
assignee:
  - '@bruk.w5'
created_date: '2026-11-15T01:32:00Z'
updated_date: '2026-11-15T04:00:00Z'
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

- `crates/sdk/src/layout.rs` (new). The shapes that cross the API whole (doc-16 R105 names "a layout" as its example): `Orientation { Horizontal, Vertical }` (a split's own axis — R95's "a direction"), `Direction { Left, Right, Up, Down }` (R9's "focused by direction", and the shape TASK-1.37's planner asked for — distinct from `Orientation` by name so the two can't be confused: doc-15 overloads the word "direction" for both a split's axis and focus movement; the code's doc comments say so on both types) with `direction.orientation() -> Orientation` and `direction.is_before() -> bool` (Left/Up → the near/`first` side), `Ratio` (fixed-point: `Ratio(u16)` in permille, `Ratio::new(permille: u16) -> Option<Self>` valid for `1..=999`, rejecting 0 and 1000 so a child is never zero-sized — fixed-point, not `f64`, so `Layout` and everything holding it (`Tab`, `Session`) keeps deriving `Eq`/`Hash`, which `tab.rs:23` and `session.rs:20` already derive and must not lose), `Size` (`columns: u16, rows: u16`, private fields + `new`/`columns()`/`rows()`, matching `ids.rs`'s style), and:
  ```rust
  pub enum Layout {
      Pane(PaneId),
      Split { orientation: Orientation, ratio: Ratio, first: Box<Layout>, second: Box<Layout> },
  }
  ```
  Pure methods on `Layout` (no kernel state, no I/O — usable by the kernel now and by the app later, reading a reported layout the same way):
  - `leaves(&self) -> Vec<PaneId>` — every pane in doc-15 R112's order: depth-first, `first` before `second` at every split. R112: "a pane's position as a target is its place in that order, counted from one." This is also TASK-1.38's "the tab's accessor for its panes" ordering, surfaced through `Tab::panes()` below, and TASK-1.37's tie-break (next bullet).
  - `leaf_sizes(&self, total: Size) -> Vec<(PaneId, Size)>` — walks top-down from `total`, giving each split's two children `first = floor(parent.axis_extent * ratio.permille / 1000)`, `second = parent.axis_extent - first` along the split's axis and the parent's own extent unchanged on the other axis (a split's two children always sum exactly to its own size, by construction — the rounding rule is stated once, here).
  - `bordering_split(&self, pane: PaneId, direction: Direction) -> Option<&Layout>` / `bordering_split_mut(&mut self, pane: PaneId, direction: Direction) -> Option<&mut Layout>` — the nearest ancestor split whose `orientation == direction.orientation()` and from whose far side `pane` descends (`pane` under `first` and `direction.is_before()` is false, or under `second` and `is_before()` is true: the direction points away from `pane`, toward the sibling). No such ancestor → `None`. This one shared traversal is what both `neighbour` (next bullet) and the kernel's `resize_pane` are built on, so the ancestor-finding logic exists in exactly one place.
  - `inherited_size(&self, pane: PaneId, direction: Direction, total: Size) -> Option<Size>` — the size the split found by `bordering_split` would inherit from its own ancestors (walking the same path, dividing `total` by each ancestor's ratio, stopping at that split rather than continuing to leaves). This is what `resize_pane`'s floor clamp needs and `leaf_sizes` alone can't give for an internal node (a node's own inherited extent isn't any one descendant leaf's size once there's further nesting on the perpendicular axis).
  - `neighbour(&self, pane: PaneId, direction: Direction) -> Option<PaneId>` — `bordering_split(pane, direction)`, then descend into whichever side of it does *not* contain `pane`, taking that subtree's own first leaf in R112 order. **This is the answer to "define what 'the neighbour' is when several panes share that edge": the one earliest in R112's order (first before second, recursively) — "nearest the top or left", not "most recently current", since nothing in the layout tree tracks history** (a `most-recently-current` rule would need focus history this task has no reason to carry). `None` if `bordering_split` found nothing.

- `crates/sdk/src/lib.rs`: add `mod layout;` and `pub use layout::{Direction, Layout, Orientation, Ratio, Size};`.

- `crates/kernel/src/tab.rs`: add fields `layout: Layout`, `current: PaneId`, `fill: Option<PaneId>`, `size: Option<Size>`. `Tab::new` builds `layout: Layout::Pane(first_pane)`, `current: first_pane`, `fill: None`, `size: None` — unchanged signature, so `session.rs` doesn't change. Remove `add_pane` (the flat-Vec push this task's own prior note says it's replacing) and its test.

  **R112 and `Tab::panes()`**: keep the name and the `&[Pane]` return type (so TASK-1.38, already built against `tab.panes()`, needs no change) but change what it returns: `self.panes` is now kept *stored* in R112's order at all times, maintained by one private helper, `fn reorder_panes_to_match_layout(&mut self)`, called at the end of every method that changes the tree's shape (`split_pane`, `close_pane`, `swap_panes`, and the two halves of `move_pane_to_tab`) — it sorts `self.panes` to the id sequence `self.layout.leaves()` gives. One helper, one place order correctness is enforced, rather than hand-computing the right insertion point in each of those four methods separately. `panes()`'s doc comment is updated from "in order" (creation order, the old placeholder) to "in R112's order: the layout tree's leaves, first child before second, so a pane's *position* — what TASK-1.38 resolves "the second pane" by — means this order, counted from one, not creation order."

  Rewrite `close_pane`: locate the leaf in `layout`, not in `panes`; if it has a parent split, replace the parent with the sibling subtree (this *is* "gives its space to the other child of its split" — a tree-collapse touching only the one ancestor pointer, so no other split's ratio or direction and no other leaf's position changes, satisfying AC #2's "nothing else moves" by construction) and remove the pane from `panes`, then `reorder_panes_to_match_layout`; if it has no parent (it was the tab's only pane), report `tab_now_empty: true` without touching `layout` (the tab is deleted by `Tree` immediately after, so no one ever observes a stale layout). In both cases: if the closed pane was `current`, set `current` to the first entry of `self.layout.leaves()` for whatever now occupies its place; if it was `fill`, clear `fill` to `None`.

  Add:
  ```rust
  pub fn neighbour(&self, from: PaneId, direction: Direction, wrap: bool) -> DirectionalFocus
  pub const fn current(&self) -> PaneId
  pub const fn fill(&self) -> Option<PaneId>
  pub(crate) fn split_pane(&mut self, target: PaneId, direction: Direction, new_id: PaneId, kind: PaneKind) -> Result<(), Error>
  pub(crate) fn resize_pane(&mut self, pane: PaneId, direction: Direction, ratio: Ratio) -> Result<Ratio, Error>
  pub(crate) fn swap_panes(&mut self, a: PaneId, b: PaneId) -> Result<(), Error>
  pub(crate) fn fill_pane(&mut self, pane: PaneId) -> Result<(), Error>
  pub(crate) fn unfill(&mut self)
  pub fn set_size(&mut self, size: Size)
  ```
  And, in `tab.rs` alongside `Tab` (exported from kernel's `lib.rs`, next to `PaneClosed`/`TabClosed` — a kernel decision-function output, like those, not an sdk wire shape, since nothing has decided yet whether or how it crosses the API):
  ```rust
  /// What asking for a pane's neighbour in a direction found (TASK-1.37's
  /// requested shape). Never an error: doc-2 section 4 treats "nothing that
  /// way, focus stays put" as success, so there is no failure case here —
  /// only which of the three this was.
  pub enum DirectionalFocus {
      /// The pane adjacent to `from` on that side.
      Neighbour(PaneId),
      /// Nothing that way, and `wrap` was `false` (or there was nothing to
      /// wrap *to* — see below).
      Edge,
      /// Nothing that way, `wrap` was `true`, and this is what wrapping
      /// around to the opposite end of the tab's R112 order landed on.
      Wrapped(PaneId),
  }
  ```
  `Tab::neighbour`: `self.layout.neighbour(from, direction)` — `Some(id)` → `Neighbour(id)`. `None` and `!wrap` → `Edge`. `None` and `wrap`: wrapping means treating the tab's R112-ordered panes as a loop, so the "far side" is the opposite end of `self.layout.leaves()` from `from` — `Direction::Right`/`Down` (`!direction.is_before()`) wrap to `leaves().first()`, `Direction::Left`/`Up` wrap to `leaves().last()`. **This departs from the lead's exact phrasing ("wrapped to the pane on the far side") on purpose: "the far side" in 2D only coincides with "the opposite end of R112's order" for an unnested split, and the layout tree carries no pixel positions to compute a true 2D far side with. R112's order is the one well-defined, always-available fallback, so that's what wrapping uses — flagged here and in Risks, not just in Risks, since it's a visible departure from the request, not merely an implementation detail.** If that computed pane *is* `from` (the tab has only one pane, so there is nothing else to wrap to), return `Edge`, not `Wrapped(from)` — wrapping to yourself isn't a real wrap. If `from` is not a pane in this tab at all, also `Edge` (nothing in the tree can be found for it; `Tab::neighbour` never errors, per the brief — a caller that needs to tell "invalid pane" apart from "valid pane, no neighbour" uses `Tab::pane(from)` first, as other callers already do).

  `DirectionalFocus` is defined in the kernel, not the sdk, even though `Direction` (its input) is an sdk shape: it is this task's own decision-function output — what asking the tree for a neighbour *found* — exactly like `ClosedPane`/`PaneClosed` already are, not a shape proven to cross the API; nothing has decided yet whether or how a focus-direction request's result travels on the wire, and that decision belongs to whichever task builds the API (TASK-1.43 onward), not this one.

  `split_pane`: replace the target leaf with `Layout::Split { orientation: direction.orientation(), ratio: Ratio::new(500).expect("500 is a valid permille"), first, second }`, the *new* pane on `direction`'s side (`direction.is_before()` picks which of `first`/`second` is the target vs. the new pane), the target pane on the other side; push the new `Pane` into `panes`, call `reorder_panes_to_match_layout`; set `current` to the new pane (AC #1). Floor check (shared with `move_pane_to_tab`): skip entirely if `self.size` is `None` (R100 can't be judged with no data — the plan's answer to "what a split does when no size is known": allowed, never refused, until a size arrives); otherwise, with the target's own current size (from `self.layout.leaf_sizes(self.size)`, filtered to the target), compute the two hypothetical children's sizes and refuse (`Error::SplitTooSmall { pane: target }`, no mutation) if either is under 20 columns or 4 rows **in either dimension**, not only the one the split divides — a horizontal split still halves the row count if the tab is already short on rows, and R100's floor binds on both axes.

  `resize_pane`: first, with only an immutable borrow, confirm the split exists (`self.layout.bordering_split(pane, direction)` — `None` is `Error::NoSplitInDirection { pane, direction }`) and, if `self.size` is known, call `self.layout.inherited_size(pane, direction, self.size)` for that split's own rectangle, then run the clamp search entirely over immutable data: starting from the requested `ratio`, step permille values toward whichever side currently satisfies the floor (bounded: at most 999 steps, trivial cost), checking `leaf_sizes` of *just that split's subtree* at each candidate (using the inherited size from above — scoped to the subtree, not the whole tab, so an already-shrunk, already-under-floor pane elsewhere per AC #6 doesn't block an unrelated resize) until every leaf in it is at or above 20×4. Only once a final `Ratio` is settled does `self.layout.bordering_split_mut(pane, direction)` take the mutable borrow to write it — the mutable borrow must come last, after every immutable read, or it can't compile (`inherited_size`/`leaf_sizes` need `&self.layout` while a `&mut` from `bordering_split_mut` is still held, otherwise). If `self.size` is `None`, skip the clamp and just write the requested ratio directly. This is R100's "a resize stops at that floor" (clamped, succeeds) as against split's "is refused" (rejected outright) — deliberately different outcomes for the same rule.

  `swap_panes`: both panes must resolve to leaves in *this* tab (cross-tab is "moved to another tab", R9's separate, differently-named capability, not this method) — exchange the two `Layout::Pane(id)` values in place, then `reorder_panes_to_match_layout`; nothing else in the tree changes.

  `fill_pane`/`unfill`: set/clear `self.fill`; `fill_pane` on a pane not in this tab is `Error::PaneNotFound`. Neither touches `layout` (AC #4) or calls the reorder helper (fill never changes the tree). Setting a new fill while one is already set just replaces it (R96's "at most one... at a time" is an invariant of the field).

  `set_size`: stores whatever `Size` it's given, unconditionally — no floor check, ever (AC #6: "refuses and closes nothing"). How a size reaches the kernel: nothing in the kernel measures anything; a future caller (the server, wired up by TASK-1.42 from the window's pixels and the font's cell metrics — out of this task's scope) calls `Tree::set_tab_size` whenever it learns a new size, and the kernel just remembers the latest value it was handed.

- `crates/kernel/src/tree.rs`: remove `new_pane` and its three tests (`a_failed_new_pane_does_not_consume_an_id`, `new_pane_adds_a_pane_to_an_existing_tab`, and the `new_pane` call inside `closing_one_of_several_panes_cascades_no_further_than_it_should`, switched to `split_pane`). Confirmed by `git grep -n "new_pane|add_pane"` in this worktree: every use is inside `crates/kernel` itself; no other crate (`server`, `steg`, `app` — none has kernel code yet) references either name.

  Add the public, cascading/validating entry points, each locating the owning tab then delegating to the `pub(crate)` method above after minting whatever id it needs from `self.ids`:
  ```rust
  pub fn split_pane(&mut self, target: PaneId, direction: Direction, kind: PaneKind) -> Result<PaneId, Error>
  pub fn resize_pane(&mut self, pane: PaneId, direction: Direction, ratio: Ratio) -> Result<Ratio, Error>
  pub fn swap_panes(&mut self, a: PaneId, b: PaneId) -> Result<(), Error>
  pub fn fill_pane(&mut self, pane: PaneId) -> Result<(), Error>
  pub fn unfill_pane(&mut self, tab: TabId) -> Result<(), Error>
  pub fn set_tab_size(&mut self, tab: TabId, size: Size) -> Result<(), Error>
  pub fn move_pane_to_tab(&mut self, pane: PaneId, target_tab: TabId, direction: Direction) -> Result<MovedPane, Error>
  ```
  (`Tab::neighbour` is reached as `tree.session(..).tab(..).neighbour(..)` for now, as other read-only per-tab queries already are; TASK-1.37 owns adding whatever `Tree`/session-level focus plumbing calls it, per the brief — "TASK-1.37 will add a tab's current pane and call it.")

  `move_pane_to_tab`: refuse `target_tab == pane`'s own tab (`Error::AlreadyInTab`). Otherwise: run the exact tree-collapse `close_pane` uses on the source tab (promoting the sibling, cascading tab/session closure via the existing `close_pane`/`close_tab` machinery if the source tab/session is now empty) but *without* dropping the `Pane` — move it into the target tab's `panes` — then run the exact insertion `split_pane` uses on the target tab's *current* pane, with the moved pane (not a freshly-minted one) on `direction`'s side, becoming the target tab's new current. Same floor rule as `split_pane`, checked before either tab is mutated (atomic — either both halves happen or neither does). `reorder_panes_to_match_layout` on both tabs. Returns:
  ```rust
  pub struct MovedPane {
      pub pane: PaneId,
      pub to: TabId,
      pub source_tab_closed: Option<TabId>,
      pub source_session_closed: Option<SessionId>,
  }
  ```
  mirroring `ClosedPane`'s shape.

- `crates/kernel/src/error.rs`: add `SplitTooSmall { pane: PaneId }`, `NoSplitInDirection { pane: PaneId, direction: steg_sdk::Direction }`, `PanesNotInSameTab { a: PaneId, b: PaneId }`, `AlreadyInTab { pane: PaneId, tab: TabId }`, each with a `Display` arm.

- `crates/kernel/src/lib.rs`: add `DirectionalFocus` and `MovedPane` to the `pub use` lines, alongside the existing `PaneClosed`/`TabClosed`/`ClosedPane`.

- `crates/sdk/src/layout.rs` and `crates/kernel/src/tab.rs`/`tree.rs` each get a `#[cfg(test)]` module per `testing-rust.md`: unit tests, no doubles.

## Order of work

1. `crates/sdk/src/layout.rs`: `Orientation`, `Direction` (`orientation()`/`is_before()`), `Ratio`, `Size`, `Layout::{leaves, leaf_sizes}`, with unit tests (reading order over a hand-built tree; `leaf_sizes` rounding sums exactly to `total`). Wire into `lib.rs`. No kernel dependency, checkable in isolation first.
2. `Layout::{bordering_split, bordering_split_mut, inherited_size, neighbour}` with unit tests: each of the four directions on a hand-built tree, "no ancestor that way" at the tab's edge, one case nested two levels deep, and the "several panes share the bordered edge" tie-break (picks the earliest in R112 order).
3. `Tab`: add the four new fields and `reorder_panes_to_match_layout`; rewrite `new`/`close_pane` (collapse + `current`/`fill` repair + reorder); delete `add_pane`. Update `close_pane`'s existing tests and add the `current`-validity tests.
4. `Tab::split_pane` (no floor check yet) with AC #1's tests, plus the R112 order test after a split.
5. Floor check: add it to `split_pane`; `Tab::set_size` stores unconditionally. Tests: refused-under-floor (both the divided and the undivided axis), allowed-with-unknown-size, allowed-with-floor-respected.
6. `Tab::resize_pane` on `bordering_split_mut`/`inherited_size`, with the clamp search. Tests: changes-the-ratio, stops-at-the-floor (exact boundary), no-split-in-that-direction is an error, an unrelated already-under-floor pane elsewhere doesn't block it.
7. `Tab::swap_panes` (+ R112 order test), `Tab::fill_pane`/`unfill` (AC #4 tests), `Tab::neighbour`/`DirectionalFocus` (AC and the five tests listed under TASK-1.37's requirement, below).
8. `Tree`: remove `new_pane`+tests, add the seven public entry points and `move_pane_to_tab`'s cascade-reuse (+ R112 order tests after a close and a move). Tests: AC #3's move-keeps-both-tabs-valid, AC #6's shrink-refuses-nothing.
9. `just check` and `just test` on `steg-sdk`/`steg-kernel`; fix anything found before recording.

## Risks

- **Two representations of "which panes exist" (`panes: Vec<Pane>` and `layout`'s leaves) can desync.** Inherent to the domain split (R105: the layout tree, not each pane's data, is reported to clients whole). Mitigated two ways: `reorder_panes_to_match_layout` is the single place that keeps `panes`'s *order* correct (rather than four separate hand-computed insertions), and a `debug_assert!` that the two *sets* of ids match — placed only on the paths that leave the tab non-empty (every mutating method except `close_pane`'s "this was the tab's only pane" branch, which returns `tab_now_empty: true` immediately, before any assert, and deliberately leaves `panes` empty while `layout` still names the removed pane, since `Tree` deletes the tab in the same call and no one observes the gap). `tab_now_empty` itself stays `self.panes.is_empty()`, as today — not `layout.leaves().is_empty()` — so the existing tests (`closing_the_only_pane_reports_the_tab_now_empty`, `closing_a_sessions_only_tabs_only_pane_ends_the_session`) keep passing unchanged.
- **The default split/move ratio (500 permille) is this plan's assumption, not confirmed.** R78 ties the split's origin to the design artifact, which workers can't yet read (doc-10's open question). `resize_pane` changes it afterward, so nothing depends on 500 being right, but flag it for Bruk/the engineer.
- **`neighbour`'s tie-break (earliest in R112 order) and wrap-to-the-opposite-end-of-R112-order are both deliberate simplifications**, not true 2D geometry (this shape carries no pixel positions). If TASK-1.37's UI testing shows either picks the wrong pane in a mixed-nesting layout, that is a refinement to `Layout::neighbour`/`Tab::neighbour`, not a sign the shapes here are wrong — and both rules are written down and tested, so a refinement starts from a known baseline rather than undocumented behaviour.
- **Removing `Tree::new_pane` is a public kernel API change.** Confirmed safe: `git grep -n "new_pane|add_pane"` in this worktree shows every use inside `crates/kernel` itself; no other crate has kernel code yet, and TASK-1.37/TASK-1.38 are both downstream, built against the new API.
- **Resize's floor check must be scoped to the subtree under the split being resized, not the whole tab**, or AC #6 (an already-shrunk tab may have panes under the floor) would wrongly block an unrelated resize elsewhere. `inherited_size` plus a subtree-scoped `leaf_sizes` call is specifically there to avoid this.

## Proof

Commands as named on `main` today (`just --list`): `just test` (the affected crates — `steg-sdk` and `steg-kernel` — via `steg-affected`) while building, `just check` (fmt, lint, boundaries, licences, the affected tests, the tools' tests, the architecture model) before `asdlc record-check`'s final run. TASK-1.73 may rename these later; this task uses the current names.

Acceptance criterion / requirement → test:

- **#1** (split divides only the target, in the given direction, new pane becomes current): `split_pane_divides_only_the_target_pane`, `split_pane_puts_the_new_pane_on_the_requested_side`, `split_pane_makes_the_new_pane_current`.
- **#2** (closing gives space to the sibling, nothing else moves): `closing_a_pane_promotes_its_siblings_subtree_to_its_parents_place`, `closing_a_pane_leaves_every_other_panes_position_unchanged`, `closing_the_current_pane_moves_current_to_a_pane_that_still_exists`, `closing_a_filled_pane_clears_the_fill`.
- **#3** (resize changes a ratio; swap and move keep the tree valid): `resize_pane_changes_the_bordering_splits_ratio`, `swap_panes_exchanges_two_panes_leaving_the_rest_of_the_tree_unchanged`, `swap_panes_is_refused_when_the_two_panes_are_in_different_tabs`, `move_pane_to_tab_leaves_both_tabs_with_a_valid_tree`, `move_pane_to_tab_cascades_closing_the_source_tab_when_it_was_the_last_pane`.
- **#4** (fill changes nothing in the tree; at most one at a time): `fill_pane_does_not_change_the_layout_tree`, `filling_a_second_pane_replaces_the_first_fill`, `unfill_clears_it`.
- **#5** (split refused under the floor; resize stops at it): `split_pane_is_refused_when_either_side_would_be_under_the_floor`, `split_pane_is_refused_when_the_undivided_dimension_is_already_under_the_floor`, `resize_pane_stops_exactly_at_the_floor_instead_of_going_below_it`, `split_pane_is_allowed_when_the_tabs_size_is_unknown`.
- **#6** (shrinking refuses and closes nothing; panes may go under the floor): `set_tab_size_smaller_always_succeeds_and_closes_nothing`, `a_resize_elsewhere_is_not_blocked_by_an_unrelated_pane_already_under_the_floor`.
- **R112** (a tab's panes, in layout order, counted from one; a ratio change doesn't reorder them, split/close/swap/move do): `panes_are_in_layout_order_after_a_split`, `panes_are_in_layout_order_after_a_close`, `panes_are_in_layout_order_after_a_swap`, `panes_are_in_layout_order_after_a_move`, `resize_pane_does_not_change_the_panes_order`.
- **TASK-1.37's interface** (direction shape named distinctly; `Tab::neighbour`/`DirectionalFocus`; the tie-break): `neighbour_returns_the_adjacent_pane_when_one_exists`, `neighbour_returns_edge_when_none_exists_and_wrap_is_false`, `neighbour_wraps_to_the_far_end_of_the_tabs_order_when_wrap_is_true`, `neighbour_returns_edge_rather_than_wrapping_to_itself_when_the_tab_has_only_one_pane`, `neighbour_picks_the_first_leaf_in_reading_order_when_several_panes_share_the_bordered_edge`.

Plus the sdk-level unit tests from steps 1-2 above.
<!-- SECTION:PLAN:END -->
