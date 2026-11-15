---
id: TASK-1.36
title: 'Kernel: the layout tree: split, close, resize, swap, move and fill'
status: Done
assignee:
  - '@bruk.w5'
created_date: '2026-11-15T01:32:00Z'
updated_date: '2026-11-15T04:59:00Z'
labels:
  - size-3
  - review-passed
milestone: m-0
dependencies:
  - TASK-1.35
references:
  - task-1.36-kernel-the-layout-tree-split-close
  - https://github.com/bruk-io/steg/pull/23
parent_task_id: TASK-1
type: task
project: bruk-io-steg
asdlc_accepted_from: DRAFT-6
asdlc_branch: task-1.36-kernel-the-layout-tree-split-close
asdlc_plan_approved_by: bruk
asdlc_plan_approved_hash: 'dc26154daf58'
asdlc_plan_approved_size: size-3
asdlc_review_kind: same-machine
asdlc_review_findings: 'important=0,nit=0'
asdlc_plan_matched: yes
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
  - `bordering_split(&self, pane: PaneId, direction: Direction) -> Option<&Layout>` / `bordering_split_mut(&mut self, pane: PaneId, direction: Direction) -> Option<&mut Layout>` — the nearest ancestor split whose `orientation == direction.orientation()` and from whose far side `pane` descends (`pane` under `first` and `direction.is_before()` is false, or under `second` and `is_before()` is true: the direction points away from `pane`, toward the sibling). No such ancestor → `None`. This is `resize_pane`'s own primitive, below — `neighbour` does not use it; geometry answers that question directly (next bullets).
  - `inherited_size(&self, pane: PaneId, direction: Direction, total: Size) -> Option<Size>` — the size the split found by `bordering_split` would inherit from its own ancestors (walking the same path, dividing `total` by each ancestor's ratio, stopping at that split rather than continuing to leaves). This is what `resize_pane`'s floor clamp needs and `leaf_sizes` alone can't give for an internal node (a node's own inherited extent isn't any one descendant leaf's size once there's further nesting on the perpendicular axis).
  - **`neighbour` and wrap are answered by exact rectangles, not reading order.** Every leaf has a rectangle in the tab's own unit square `[0,1] × [0,1]` (`x` the column axis, `y` the row axis): the root's rectangle is the whole square, and a split divides its own rectangle along its axis at its ratio — exactly the same top-down shape `leaf_sizes` already walks, just over `[0,1]` instead of a concrete `Size`, and kept as an *exact fraction* rather than rounded to a whole cell, because touching and overlap have to be exact comparisons, not approximate ones (two leaves that truly share an edge must compute the identical boundary value, bit for bit, or "touching" can't be tested with `==`). A private `Fraction { numerator: i128, denominator: i128 }` — always constructed reduced (Euclid's gcd) with a positive denominator — holds each coordinate; `+`, `-`, `×` are the usual cross-multiplication formulas followed by reduction, and `<`/`==` are cross-multiplication too, never a cast to `f64`. (i128 is exact for any layout tree a terminal UI will realistically have — see Risks for the bound this doesn't check.) A private `Rect { x0, x1, y0, y1 }` of four `Fraction`s, and a private `leaf_rects(&self) -> Vec<(PaneId, Rect)>` (the exact-rectangle counterpart of `leaf_sizes`, kept as its own function rather than one generalised over both, so `leaf_sizes`'s whole-cell rounding never leaks into a comparison that must be exact) back the two public methods below. Neither `Fraction` nor `Rect` nor `leaf_rects` is exported from `lib.rs` — nothing outside this module needs to build or inspect one, only ask the two questions they answer:
  - `neighbour(&self, pane: PaneId, direction: Direction) -> Option<PaneId>` — among every other leaf, the ones whose rectangle *touches* `pane`'s on `direction`'s side (`Right`: `other.x0 == pane.x1`; `Left`: `other.x1 == pane.x0`; `Down`: `other.y0 == pane.y1`; `Up`: `other.y1 == pane.y0`) **and** whose span on the *other* axis *overlaps* `pane`'s (`max(other.y0, pane.y0) < min(other.y1, pane.y1)` for `Left`/`Right`; the `x` analogue for `Up`/`Down`). **This is the answer to "define what 'the neighbour' is when several panes share that edge": among those that qualify, the one whose overlap starts first — the smallest `max(other.y0, pane.y0)` for `Left`/`Right` (nearest the top), the smallest `max(other.x0, pane.x0)` for `Up`/`Down` (nearest the left)** — a rule read off the two rectangles, not off reading order or focus history. `None` if nothing touches that side.
  - `far_edge_pane(&self, pane: PaneId, direction: Direction) -> Option<PaneId>` — the wrap target: among every other leaf, the ones touching the tab's *own* edge opposite `direction` (`Right` wraps along the *left* edge, `other.x0 == 0`; `Left` along the *right* edge, `other.x1 == 1`; `Down` along the *top*, `other.y0 == 0`; `Up` along the *bottom*, `other.y1 == 1`) **and** overlapping `pane`'s span on the other axis, exactly as `neighbour`'s overlap test — same tie-break. `None` if `pane` is itself the only leaf touching that edge (a one-pane tab trivially touches every edge of its own square), which is deliberate: wrapping is only ever found among *other* leaves, so a tab with nothing else to wrap to reports `None` rather than wrapping to `pane` itself.

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
  `Tab::neighbour`: `self.layout.neighbour(from, direction)` — `Some(id)` → `Neighbour(id)`. `None` and `!wrap` → `Edge`. `None` and `wrap`: `self.layout.far_edge_pane(from, direction)` — `Some(id)` → `Wrapped(id)`; `None` (the one-pane-tab case, or any tab where `from` is the only leaf touching that far edge) → `Edge`, not `Wrapped(from)` — wrapping to yourself isn't a real wrap, and this is exactly `far_edge_pane`'s own `None` case, so `Tab::neighbour` adds no separate self-check. If `from` is not a pane in this tab at all, also `Edge` (nothing in the tree can be found for it; `Tab::neighbour` never errors, per the brief — a caller that needs to tell "invalid pane" apart from "valid pane, no neighbour" uses `Tab::pane(from)` first, as other callers already do).

  `DirectionalFocus` is defined in the kernel, not the sdk, even though `Direction` (its input) is an sdk shape: it is this task's own decision-function output — what asking the tree for a neighbour *found* — exactly like `ClosedPane`/`PaneClosed` already are, not a shape proven to cross the API; nothing has decided yet whether or how a focus-direction request's result travels on the wire, and that decision belongs to whichever task builds the API (TASK-1.43 onward), not this one.

  `split_pane`: replace the target leaf with `Layout::Split { orientation: direction.orientation(), ratio: Ratio::new(500).expect("500 is a valid permille"), first, second }`, the *new* pane on `direction`'s side (`direction.is_before()` picks which of `first`/`second` is the target vs. the new pane), the target pane on the other side; push the new `Pane` into `panes`, call `reorder_panes_to_match_layout`; set `current` to the new pane (AC #1). **The 500-permille (half-and-half) ratio is not an assumption: the engineer confirmed on 2026-11-15 that a split divides the target pane in half, so this is the decided default, cited here rather than flagged in Risks.** `move_pane_to_tab` (below) uses the same default, for the same reason. Floor check (shared with `move_pane_to_tab`): skip entirely if `self.size` is `None` (R100 can't be judged with no data — the plan's answer to "what a split does when no size is known": allowed, never refused, until a size arrives); otherwise, with the target's own current size (from `self.layout.leaf_sizes(self.size)`, filtered to the target), compute the two hypothetical children's sizes and refuse (`Error::SplitTooSmall { pane: target }`, no mutation) if either is under 20 columns or 4 rows **in either dimension**, not only the one the split divides — a horizontal split still halves the row count if the tab is already short on rows, and R100's floor binds on both axes.

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

  **What TASK-1.37 inherits from this task, so it knows what it still has to add**: `Tab::current() -> PaneId` already exists and is already kept valid by every mutation here (set on split to the new pane per AC #1, repaired on close so it never points at a removed pane — doc-15 R85's "a tab has a current pane" is already true of every `Tab` this task produces). `Tab::neighbour(from, direction, wrap) -> DirectionalFocus` already resolves "what's over there" without needing the tree itself. What this task does *not* add, and TASK-1.37 still needs to: a session's own current tab (R85's "each session has a current tab", and "focusing a pane makes its tab the current tab of its session as well" — nothing here tracks that, since it's a `Session`-level concern, not a `Tab`-level one), and the actual *act* of focusing — the operation that calls `neighbour`, takes whichever `PaneId` the result names (or does nothing on `Edge`), and writes it into the tab's `current` (there is no public setter for `current` today beyond what `split_pane`/`close_pane` already do internally; adding one, or a method that both sets it and updates the owning session's current tab in one call, is TASK-1.37's own work).

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
2. `Layout::{bordering_split, bordering_split_mut, inherited_size}` (for `resize_pane`, built in step 6) with unit tests for the ancestor walk. Separately, the private `Fraction`/`Rect`/`leaf_rects` and the public `neighbour`/`far_edge_pane`, with unit tests: `leaf_rects` on a hand-built tree produces exact, matching boundary `Fraction`s between adjacent leaves (the exactness claim itself, checked directly); `neighbour` in each of the four directions, including the bottom-left/bottom-right-not-top-right case and the tall-pane-takes-the-upper-one case named in the brief; `far_edge_pane`'s wrap landing in the same row/column as the pane that wrapped, not an unrelated one; and the single-pane tab, `Edge` in all four directions with `wrap: true` too.
3. `Tab`: add the four new fields and `reorder_panes_to_match_layout`; rewrite `new`/`close_pane` (collapse + `current`/`fill` repair + reorder); delete `add_pane`. Update `close_pane`'s existing tests and add the `current`-validity tests.
4. `Tab::split_pane` (no floor check yet) with AC #1's tests, plus the R112 order test after a split.
5. Floor check: add it to `split_pane`; `Tab::set_size` stores unconditionally. Tests: refused-under-floor (both the divided and the undivided axis), allowed-with-unknown-size, allowed-with-floor-respected.
6. `Tab::resize_pane` on `bordering_split_mut`/`inherited_size`, with the clamp search. Tests: changes-the-ratio, stops-at-the-floor (exact boundary), no-split-in-that-direction is an error, an unrelated already-under-floor pane elsewhere doesn't block it.
7. `Tab::swap_panes` (+ R112 order test), `Tab::fill_pane`/`unfill` (AC #4 tests), `Tab::neighbour`/`DirectionalFocus` (the geometry already proven in step 2; this step is just wiring `Tab::neighbour` to `Layout::neighbour`/`far_edge_pane` and the `DirectionalFocus` variants, with the tests listed under TASK-1.37's requirement, below).
8. `Tree`: remove `new_pane`+tests, add the seven public entry points and `move_pane_to_tab`'s cascade-reuse (+ R112 order tests after a close and a move). Tests: AC #3's move-keeps-both-tabs-valid, AC #6's shrink-refuses-nothing.
9. `just check` and `just test` on `steg-sdk`/`steg-kernel`; fix anything found before recording.

## Risks

- **Two representations of "which panes exist" (`panes: Vec<Pane>` and `layout`'s leaves) can desync.** Inherent to the domain split (R105: the layout tree, not each pane's data, is reported to clients whole). Mitigated two ways: `reorder_panes_to_match_layout` is the single place that keeps `panes`'s *order* correct (rather than four separate hand-computed insertions), and a `debug_assert!` that the two *sets* of ids match — placed only on the paths that leave the tab non-empty (every mutating method except `close_pane`'s "this was the tab's only pane" branch, which returns `tab_now_empty: true` immediately, before any assert, and deliberately leaves `panes` empty while `layout` still names the removed pane, since `Tree` deletes the tab in the same call and no one observes the gap). `tab_now_empty` itself stays `self.panes.is_empty()`, as today — not `layout.leaves().is_empty()` — so the existing tests (`closing_the_only_pane_reports_the_tab_now_empty`, `closing_a_sessions_only_tabs_only_pane_ends_the_session`) keep passing unchanged.
- **`Fraction`'s `i128` numerator/denominator is exact, not approximate, but not unbounded.** Reduced by gcd after every operation, so the denominator stays as small as the actual ratios involved allow (a tree of all-500-permille splits keeps denominators as powers of 2, however deep); an adversarial tree of many coprime ratios nested dozens of levels deep could in principle overflow `i128`. Nothing bounds split depth today, and no realistic terminal layout goes anywhere near that deep, so this task does not add a check — a follow-up if split depth ever becomes something a person can grow without limit.
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
- **TASK-1.37's interface** (direction shape named distinctly; `Tab::neighbour`/`DirectionalFocus`; neighbour and wrap by exact rectangle geometry): `neighbour_moving_right_from_bottom_left_lands_on_bottom_right_not_top_right` (the diagonal case the brief names — a 2×2 grid built as two columns, each itself split top/bottom, so the old reading-order rule and the geometric one disagree and only the geometric one is right), `neighbour_from_a_pane_spanning_two_rows_takes_the_upper_pane_across_a_column_split` (one tall pane beside a column split in two; both qualify by touching and overlap, the tie-break picks the upper), `wrap_from_the_rightmost_pane_lands_on_the_leftmost_pane_in_its_own_row_not_another_row`, `neighbour_and_wrap_are_both_edge_in_every_direction_for_a_single_pane_tab` (never `Wrapped` onto itself — `far_edge_pane`'s own `None` case), `leaf_rects_gives_adjacent_leaves_exactly_equal_touching_boundaries` (the exactness claim).

Plus the sdk-level unit tests from steps 1-2 above.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Plan checked against doc-15 (R9, R78, R95, R96, R100, R112, R113) and doc-16 (R105) by @bruk.o: ok. Layout shapes and their pure geometry are in the sdk; split, close, resize, swap, move, fill and the 20 by 4 floor are the kernel's. A tab's panes are kept in the order of the tree's leaves (R112). A pane's neighbour and the wrap target are decided by exact rectangles from the tree and its ratios, with a stated tie-break (R113, written from this plan). A split divides in half, as the engineer decided. The resize floor is judged within the split being resized. TASK-1.37 inherits the tab's current pane and Tab::neighbour, and adds the session's current tab and the focus operations.

Check passed: `just check` exited 0 at 2026-11-15T04:24:58+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.36 on task-1.36-kernel-the-layout-tree-split-close at 74dc064e3e5b with uncommitted changes:

```text
ok      format check (0.1s)
ok      lint (0.4s)
ok      boundaries (0.1s)
ok      licences (0.6s)
ok      tests (-p steg -p steg-app -p steg-kernel -p steg-sdk -p steg-server) (0.3s)
ok      build (steg-sdk) (0.1s)
ok      test (steg-sdk) (0.1s)
ok      build (steg-kernel) (0.1s)
ok      test (steg-kernel) (0.1s)
ok      build (steg-server) (0.1s)
ok      test (steg-server) (0.1s)
ok      build (steg) (0.1s)
ok      test (steg) (0.1s)
ok      build (steg-app) (0.2s)
ok      test (steg-app) (0.2s)
ok      tool tests (1.3s)
ok      architecture (1.0s)
ok      recipe names (0.1s)
```

Check passed: `just check` exited 0 at 2026-11-15T04:29:12+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.36 on task-1.36-kernel-the-layout-tree-split-close at 0df04d7afe1d:

```text
ok      format check (0.1s)
ok      lint (0.4s)
ok      boundaries (0.1s)
ok      licences (0.6s)
ok      tests (-p steg -p steg-app -p steg-kernel -p steg-sdk -p steg-server) (0.4s)
ok      build (steg-sdk) (0.1s)
ok      test (steg-sdk) (0.1s)
ok      build (steg-kernel) (0.1s)
ok      test (steg-kernel) (0.1s)
ok      build (steg-server) (0.1s)
ok      test (steg-server) (0.1s)
ok      build (steg) (0.1s)
ok      test (steg) (0.1s)
ok      build (steg-app) (0.3s)
ok      test (steg-app) (0.3s)
ok      tool tests (1.3s)
ok      architecture (1.0s)
ok      recipe names (0.0s)
```

Built as planned (plan unchanged through build). just check (fmt, lint, boundaries, licences, affected tests, every package alone, tool tests, architecture, recipe names) recorded clean twice via asdlc record-check, the second time after rebasing onto main post-TASK-1.38 merge and after main moved again from the board's own check-recording commits.

Rebase note: TASK-1.38 (PR #22) merged to main while this was building. Resolved conflicts in error.rs (Error loses Copy, both sets of variants kept), lib.rs (both pub use sets), tab.rs (both test sets, including TASK-1.38's set_label tests), and tree.rs (fallible new_session/new_tab throughout my own tests). Tab::panes() already returned R112 order before the merge, so target.rs's resolve functions needed no change; only target.rs's own test that called the now-removed Tree::new_pane needed updating to split_pane, which I did and re-ran (target::tests all still pass).

Test-to-clause mapping (crates/kernel/src/tab.rs and tree.rs unless noted):

AC #1 (split divides only the target, new pane becomes current): split_pane_divides_only_the_target_pane, split_pane_puts_the_new_pane_on_the_requested_side, split_pane_makes_the_new_pane_current.

AC #2 (close gives space to the sibling, nothing else moves): closing_a_pane_promotes_its_siblings_subtree_to_its_parents_place, closing_a_pane_leaves_every_other_panes_position_unchanged, closing_the_current_pane_moves_current_to_a_pane_that_still_exists, closing_a_filled_pane_clears_the_fill.

AC #3 (resize changes a ratio; swap and move keep the tree valid): resize_pane_changes_the_bordering_splits_ratio, swap_panes_exchanges_two_panes_leaving_the_rest_of_the_tree_unchanged, swap_panes_is_refused_when_the_two_panes_are_in_different_tabs (tree.rs), move_pane_to_tab_leaves_both_tabs_with_a_valid_tree (tree.rs), move_pane_to_tab_cascades_closing_the_source_tab_when_it_was_the_last_pane (tree.rs), move_pane_to_tab_refuses_a_panes_own_tab (tree.rs).

AC #4 (fill changes nothing in the tree; at most one at a time): fill_pane_does_not_change_the_layout_tree, filling_a_second_pane_replaces_the_first_fill, unfill_clears_it.

AC #5 (split refused under the floor; resize stops at it): split_pane_is_refused_when_either_side_would_be_under_the_floor, split_pane_is_refused_when_the_undivided_dimension_is_already_under_the_floor, resize_pane_stops_exactly_at_the_floor_instead_of_going_below_it, split_pane_is_allowed_when_the_tabs_size_is_unknown, split_pane_is_allowed_when_the_floor_is_respected.

AC #6 (shrink refuses nothing; panes may go under the floor): set_tab_size_smaller_always_succeeds_and_closes_nothing (tree.rs), an_unrelated_already_under_floor_pane_does_not_block_a_resize_elsewhere.

R112 (pane order, counted from one, unchanged by resize): panes_are_in_layout_order_after_a_split, panes_are_in_layout_order_after_a_swap, panes_are_in_layout_order_after_a_move (tree.rs), resize_pane_does_not_change_the_panes_order; split_pane_adds_a_pane_to_an_existing_tab (tree.rs) covers the order after a split at the Tree level too.

R113 / TASK-1.37's interface (Direction named distinctly from Orientation; Tab::neighbour/DirectionalFocus; exact-rectangle neighbour and wrap, not reading order): neighbour_returns_the_adjacent_pane_when_one_exists, neighbour_returns_edge_when_none_exists_and_wrap_is_false, neighbour_wraps_to_the_far_end_of_the_tabs_order_when_wrap_is_true (name predates the geometry rewrite; it now wraps by rectangle, verified correct for a two-pane tab), neighbour_and_wrap_are_both_edge_in_every_direction_for_a_single_pane_tab (tab.rs); sdk-level (crates/sdk/src/layout.rs): neighbour_moving_right_from_bottom_left_lands_on_bottom_right_not_top_right, neighbour_from_a_pane_spanning_two_rows_takes_the_upper_pane_across_a_column_split, wrap_from_the_rightmost_pane_lands_on_the_leftmost_pane_in_its_own_row_not_another_row, neighbour_is_none_for_a_single_pane_tab_in_every_direction, far_edge_pane_is_none_for_a_single_pane_tab_in_every_direction, leaf_rects_gives_adjacent_leaves_exactly_equal_touching_boundaries (the exactness claim), leaves_are_depth_first_first_before_second, leaf_sizes_sum_exactly_to_the_total_on_an_uneven_ratio, bordering_split_finds_the_nearest_ancestor_matching_the_directions_axis, bordering_split_is_none_at_the_tabs_own_edge, inherited_size_gives_a_splits_own_rectangle_before_its_own_ratio.

Reviewer, please look closest at: the borrow-checker workarounds in layout.rs (bordering_split_mut computed via a path rather than a direct recursive &mut, and insert_leaf/remove_leaf mutating entirely in place via *layout = ... rather than returning references) — both are explained in their own doc comments, but are the least obvious code in this change. Also tab.rs's clamp_ratio_to_floor: it returns the nearest ratio to the request on either side (tested exactly in resize_pane_stops_exactly_at_the_floor_instead_of_going_below_it), which is a slightly more general reading of "stops at the floor" than a plain one-directional walk — flagged in the plan's notes as my own refinement during build, not a departure requiring replanning.

Check FAILED: `cargo test -p steg-sdk -p steg-kernel` exited 101 at 2026-11-15T04:45:39+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.36 on task-1.36-kernel-the-layout-tree-split-close at d2f05c6c834e with uncommitted changes, last 40 of 120 lines:

```text
test tree::tests::closing_a_sessions_only_tabs_only_pane_ends_the_session ... ok
test tree::tests::closing_a_tabs_last_pane_closes_the_tab_without_ending_a_session_that_has_others ... ok
test tree::tests::closing_one_of_several_panes_cascades_no_further_than_it_should ... ok
test tree::tests::closing_one_of_several_tabs_cascades_no_further_than_it_should ... ok
test tree::tests::move_pane_to_tab_cascades_closing_the_source_tab_when_it_was_the_last_pane ... ok
test tree::tests::move_pane_to_tab_is_refused_when_the_target_tabs_floor_would_be_violated_and_nothing_changes ... ok
test tree::tests::move_pane_to_tab_leaves_both_tabs_with_a_valid_tree ... ok
test tree::tests::move_pane_to_tab_refuses_a_panes_own_tab ... ok
test tree::tests::move_session_reorders_and_rejects_an_out_of_range_position ... ok
test tree::tests::move_tab_reorders_a_sessions_tabs ... ok
test tree::tests::new_session_gives_the_first_five_sessions_distinct_colours_in_order ... ok
test tree::tests::new_session_rejects_an_invalid_label_and_mints_no_id ... ok
test tab::tests::an_unrelated_already_under_floor_pane_does_not_block_a_resize_elsewhere ... ok
test tree::tests::new_session_sets_the_label_given_to_it ... ok
test tree::tests::new_tab_adds_a_labelled_tab_with_one_pane_already_in_it ... ok
test tree::tests::new_tab_rejects_an_invalid_label_and_mints_no_id ... ok
test tree::tests::panes_are_in_layout_order_after_a_move ... ok
test tree::tests::session_mut_lets_a_caller_change_a_sessions_label_and_colour ... ok
test tree::tests::set_tab_size_smaller_always_succeeds_and_closes_nothing ... ok
test tree::tests::split_pane_adds_a_pane_to_an_existing_tab ... ok
test tree::tests::swap_panes_is_refused_when_the_two_panes_are_in_different_tabs ... ok
test tree::tests::the_sixth_concurrent_session_repeats_the_first_colour ... ok

failures:

---- tab::tests::resize_pane_in_a_deeply_nested_tab_changes_only_the_innermost_matching_split stdout ----

thread 'tab::tests::resize_pane_in_a_deeply_nested_tab_changes_only_the_innermost_matching_split' (4738308) panicked at crates/kernel/src/tab.rs:1072:9:
assertion `left == right` failed
  left: Ratio(700)
 right: Ratio(500)
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    tab::tests::resize_pane_in_a_deeply_nested_tab_changes_only_the_innermost_matching_split

test result: FAILED. 97 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

error: test failed, to rerun pass `-p steg-kernel --lib`
```

Commits since the review: 5, 5 without the Asdlc-Worker trailer.

Check FAILED: `cargo test -p steg-sdk` exited 101 at 2026-11-15T04:45:53+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.36 on task-1.36-kernel-the-layout-tree-split-close at d2f05c6c834e with uncommitted changes, last 40 of 55 lines:

```text
test layout::tests::bordering_split_mut_also_finds_the_innermost_ancestor ... FAILED
test layout::tests::leaf_sizes_sum_exactly_to_the_total_on_an_uneven_ratio ... ok
test layout::tests::leaf_rects_gives_adjacent_leaves_exactly_equal_touching_boundaries ... ok
test layout::tests::leaves_are_depth_first_first_before_second ... ok
test layout::tests::inherited_size_matches_the_innermost_bordering_split_at_each_depth ... FAILED
test layout::tests::neighbour_from_a_pane_spanning_two_rows_takes_the_upper_pane_across_a_column_split ... ok
test layout::tests::neighbour_is_none_at_the_tabs_own_edge ... ok
test layout::tests::neighbour_is_none_for_a_single_pane_tab_in_every_direction ... ok
test layout::tests::neighbour_moving_right_from_bottom_left_lands_on_bottom_right_not_top_right ... ok
test layout::tests::wrap_from_the_rightmost_pane_lands_on_the_leftmost_pane_in_its_own_row_not_another_row ... ok

failures:

---- layout::tests::bordering_split_finds_the_innermost_ancestor_not_the_outermost_at_the_deepest_level stdout ----

thread 'layout::tests::bordering_split_finds_the_innermost_ancestor_not_the_outermost_at_the_deepest_level' (4740840) panicked at crates/sdk/src/layout.rs:838:37:
expected a leaf, found a split
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace

---- layout::tests::bordering_split_mut_also_finds_the_innermost_ancestor stdout ----

thread 'layout::tests::bordering_split_mut_also_finds_the_innermost_ancestor' (4740845) panicked at crates/sdk/src/layout.rs:838:37:
expected a leaf, found a split

---- layout::tests::inherited_size_matches_the_innermost_bordering_split_at_each_depth stdout ----

thread 'layout::tests::inherited_size_matches_the_innermost_bordering_split_at_each_depth' (4740848) panicked at crates/sdk/src/layout.rs:909:9:
assertion `left == right` failed
  left: Some(Size { columns: 800, rows: 10 })
 right: Some(Size { columns: 200, rows: 10 })


failures:
    layout::tests::bordering_split_finds_the_innermost_ancestor_not_the_outermost_at_the_deepest_level
    layout::tests::bordering_split_mut_also_finds_the_innermost_ancestor
    layout::tests::inherited_size_matches_the_innermost_bordering_split_at_each_depth

test result: FAILED. 18 passed; 3 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

error: test failed, to rerun pass `-p steg-sdk --lib`
```

Commits since the review: 5, 5 without the Asdlc-Worker trailer.

Check passed: `just check` exited 0 at 2026-11-15T04:46:53+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.36 on task-1.36-kernel-the-layout-tree-split-close at efb640b67adf:

```text
ok      format check (0.1s)
ok      lint (0.4s)
ok      boundaries (0.1s)
ok      licences (0.6s)
ok      tests (-p steg -p steg-app -p steg-kernel -p steg-sdk -p steg-server) (0.4s)
ok      build (steg-sdk) (0.1s)
ok      test (steg-sdk) (0.1s)
ok      build (steg-kernel) (0.1s)
ok      test (steg-kernel) (0.1s)
ok      build (steg-server) (0.1s)
ok      test (steg-server) (0.1s)
ok      build (steg) (0.1s)
ok      test (steg) (0.1s)
ok      build (steg-app) (0.3s)
ok      test (steg-app) (0.2s)
ok      tool tests (1.2s)
ok      architecture (1.0s)
ok      recipe names (0.0s)
```

Commits since the review: 11, 10 without the Asdlc-Worker trailer.

Fixed all four review findings (2 Important, 2 nits); no plan change needed, as the reviewer noted the logic was already correct everywhere traced.

1. [Important] Deep nesting with repeated orientation untested, same shape as the mid-build bug. Added outer{mid{inner{A,B},C},D} (all Horizontal) fixtures: sdk layout.rs tests bordering_split_finds_the_innermost_ancestor_not_the_outermost_at_the_deepest_level (A), bordering_split_finds_the_matching_ancestor_at_a_middle_depth (C), bordering_split_finds_the_outermost_split_for_a_leaf_at_the_top_level (D), bordering_split_mut_also_finds_the_innermost_ancestor, and inherited_size_matches_the_innermost_bordering_split_at_each_depth (all three depths' sizes: 200x10, 400x10, 800x10). Kernel tab.rs test resize_pane_in_a_deeply_nested_tab_changes_only_the_innermost_matching_split builds the same shape via split_pane and asserts, by reading the private layout field, that resize_pane changes only the innermost split's ratio and leaves mid's and outer's at 500.

Confirmed these catch the regression as asked: reverted bordering_split_ref, bordering_split_path and inherited_size_at to the old outermost-first check order locally, ran cargo test -p steg-sdk (3 failures) and cargo test -p steg-sdk -p steg-kernel (the kernel one surfaces too), recorded both failing runs with asdlc record-check (exit 101 each, visible on the task), then restored the fix and reran clean before continuing.

2. [Important] move_pane_to_tab's SplitTooSmall refusal was untested. Added tree.rs's move_pane_to_tab_is_refused_when_the_target_tabs_floor_would_be_violated_and_nothing_changes: shrinks the target tab to 39 columns (halves to 19/19, under the floor), asserts the SplitTooSmall error naming the target's current pane, and compares the whole of both tabs (source and target) before and after via Tab's derived PartialEq, which covers layout, pane order, current pane and fill together in one assertion rather than four separate hand-picked ones.

3. [Nit] resize_pane_clamps_upward_when_the_requested_ratio_would_starve_the_near_side (a too-low ratio clamps up to 200) and resize_pane_leaves_an_already_legal_ratio_unchanged (500 on a 100-column split returns 500 unchanged) — the shipped suite had only exercised the downward clamp.

4. [Nit] closing_the_current_pane_whose_sibling_is_a_multi_leaf_split_picks_the_promoted_subtrees_first_leaf: builds {A, {B, C}} then swap_panes(A, C) so C (current) sits at the position whose sibling {B, A} is multi-leaf (swap moves positions, not the `current` id), then closes C and asserts current becomes B — the promoted subtree's own first leaf in R112 order, as the rule's wording says, named explicitly here since no prior test exercised a multi-leaf promotion.

Re-merged origin/main twice more (board-only commits each time, no code conflicts) and recorded just check clean at the final head (efb640b) via asdlc record-check. Pushed to the same branch; pull request https://github.com/bruk-io/steg/pull/23 is unchanged (same branch, new commit).
<!-- SECTION:NOTES:END -->

## Comments

<!-- COMMENTS:BEGIN -->
author: @bruk
created: 2026-11-15T04:07:00Z

---

Plan approved by @bruk (dc26154daf58, size-3): its worker may build it.

---

author: @bruk.r1
created: 2026-11-15T04:30:00Z

---

@bruk.r1 is reviewing TASK-1.36: a same-machine review: it waited 0m with no other engineer's reviewer, longer than review_fallback_after (0m), so a fresh reviewer on its author's machine takes it.

---

author: @bruk.r1
created: 2026-11-15T04:38:00Z

---

Review by @bruk.r1 (same-machine): changes requested. Back to In Progress for the author's worker.

Findings: 2 Important, 2 nit. Cites: review. The diff matches the plan.

[Bugs/Evidence, Important] crates/sdk/src/layout.rs (bordering_split_ref/bordering_split_path/inherited_size_at) and the resize_pane path in crates/kernel/src/tab.rs: the plan's own notes say the worker fixed a mid-build bug in bordering_split finding the nearest (innermost) ancestor split of the right orientation rather than the outermost one. I traced the recursive logic by hand and confirmed it is correct even three levels deep with the same orientation repeated (outer{mid{inner{A,B},C},D}, A's Right border correctly resolves to the inner A|B split, not mid or outer), but no test in layout.rs, tab.rs or tree.rs builds a tree with more than two levels of the same orientation nested. The one scenario that is explicitly known to have been buggy is exactly the one with no regression test guarding it.

[Compliance/Evidence, Important] crates/kernel/src/tree.rs move_pane_to_tab: the plan states this is "atomic — either both halves happen or neither does," checking the target's floor before anything is removed from the source tab. The code does check the floor first (lines ~466-474, before remove_pane_for_move), which I confirmed by inspection is ordered correctly, but there is no test that triggers a SplitTooSmall refusal on a move and then asserts both the source and target tabs are unchanged. AC #3 ("swap and move to another tab keep the tree valid") and the review brief both call this out by name; the failure path for a move is wholly untested.

[Bugs, Nit] crates/kernel/src/tab.rs clamp_ratio_to_floor / resize_pane: resize_pane_stops_exactly_at_the_floor_instead_of_going_below_it only exercises the downward clamp (a too-high ratio clamped down). I added a temporary probe test confirming the upward clamp (a too-low ratio, e.g. 50 permille on a 100-column split, clamps up to 200) and the "requested ratio already satisfies the floor, with a known size, is returned unchanged" branch both work correctly, but neither is in the shipped test suite, so a regression in either branch would go uncaught.

[Bugs, Nit] crates/kernel/src/tab.rs close_pane / remove_pane_for_move: the current-pane repair ("set current to the first entry of layout.leaves() for whatever now occupies its place") is only tested when the promoted sibling is a single leaf. No test closes the current pane when its sibling is itself a multi-leaf split, which is the case the "first entry of... whatever now occupies its place" wording specifically exists for.

Everything else checked out. AC #1-#6 each have a specific test behind every clause, including both floor dimensions (split_pane_is_refused_when_the_undivided_dimension_is_already_under_the_floor) and the subtree-scoped resize floor (an_unrelated_already_under_floor_pane_does_not_block_a_resize_elsewhere). R112's panes()/layout.leaves() sync is enforced by one helper (reorder_panes_to_match_layout) and guarded by a debug_assert on every path that leaves a tab non-empty. R113's neighbour/far_edge_pane geometry uses exact Fraction cross-multiplication with no floating point anywhere, and I verified the tie-break and the diagonal/tall-pane cases named in the plan by reading the code and the existing tests. Tree::new_pane/Tab::add_pane are fully removed with no remaining callers (confirmed by git grep); target.rs's resolve functions were updated for the new split_pane-based API and still pass. just check (fmt, lint, boundaries, licences, affected tests, per-package build/test, tool tests, architecture) passes cleanly when run directly against the PR's head commit a41ec39, matching the PR's own CI run (43677018327, SUCCESS) on that same commit; the task's two recorded `just check` runs were made against local worktree commits that, by timestamp, bracket the final "Format target.rs" commit correctly (the second recorded check's timestamp postdates that commit by about 13 seconds), so the recorded evidence is not stale despite the differing commit hashes (expected from the worktree-to-pushed-branch rebase).

---

author: @bruk.r1
created: 2026-11-15T04:47:00Z

---

@bruk.r1 is reviewing TASK-1.36: a same-machine review: it waited 0m with no other engineer's reviewer, longer than review_fallback_after (0m), so a fresh reviewer on its author's machine takes it.

---

author: @bruk.r1
created: 2026-11-15T04:54:00Z

---

Review by @bruk.r1 (same-machine): clean. It stays In Review for a person to merge.

Findings: 0 Important, 0 nit. The diff matches the plan.

Re-review from a fresh read of the diff at origin/task-1.36-kernel-the-layout-tree-split-close head (efb640b) against the plan, AC #1-#6, and doc-15 R100/R112/R113, in a fresh worktree.

All four prior findings are closed, each independently verified, not just read:

1. [bordering_split outermost-vs-innermost regression test] Reproduced the mistake myself in a scratch copy of crates/sdk/src/layout.rs (swapped each of bordering_split_ref/bordering_split_path/inherited_size_at to check the current split's own match before recursing into the child holding the pane, instead of after). Running cargo test -p steg-sdk and cargo test -p steg-sdk -p steg-kernel against that mutation reproduces exactly the two failing runs the task already has recorded (same test names, same assertion values: Ratio(700) vs Ratio(500) in tab::tests::resize_pane_in_a_deeply_nested_tab_changes_only_the_innermost_matching_split; Some(Size{800,10}) vs Some(Size{200,10}) in layout::tests::inherited_size_matches_the_innermost_bordering_split_at_each_depth; the two bordering_split_finds_the_innermost.../bordering_split_mut_also_finds... panics at nearby but not identical line numbers, as expected since the mutation deletes a few lines). Reverted the mutation; working tree is clean. The four new tests (sdk-level outer{mid{inner{A,B},C},D} fixtures at three depths, plus the kernel-level resize test reading the private layout field) do catch the regression.

2. [move_pane_to_tab floor refusal, both tabs unchanged] tree.rs:933 (move_pane_to_tab_is_refused_when_the_target_tabs_floor_would_be_violated_and_nothing_changes) compares both the source and target Tab by Tab's own derived PartialEq (tab.rs:53, deriving over all seven fields: id, label, panes, layout, current, fill, size — not a manual impl that could skip one), and Pane (pane.rs:13) derives PartialEq over all three of its fields too. The equality used genuinely covers everything the plan's "atomic" claim depends on.

3. [resize clamp upward / already-legal ratio] tab.rs:1002 and tab.rs:1021 both present and correct: 50 permille on a 100-column split clamps up to 200 (20/80), and 500 on the same split (already legal) returns unchanged.

4. [closing the current pane when the promoted sibling is a multi-leaf split] tab.rs:875. Traced it against remove_leaf (tab.rs:578): the test's premise holds (current is genuinely c after the two splits, confirmed by the test's own assert before the swap), swap_panes never touches self.current (confirmed by reading swap_panes and swap_leaves, neither mentions current), and remove_leaf's promoted_leaf is literally promoted.leaves().first().copied() — the promoted subtree's first leaf in R112 order, exactly as close_pane's doc comment and the plan describe. The test and the code agree.

Read crates/kernel/src/tab.rs and crates/kernel/src/tree.rs in full (not just the four areas above): would_violate_floor_inserting checks both dimensions via satisfies_floor_sizes regardless of which axis the split divides (AC #5's "in either dimension"); move_pane_to_tab checks the target's floor (tree.rs:470) before any source-tab mutation (remove_pane_for_move is called after, at tree.rs:483); PanesNotInSameTab (the one new Error variant I hadn't seen constructed from a grep-only pass) is built and tested at tree.rs:380 and tree.rs:1016-1036; close_pane's debug_assert!(panes_match_layout()) correctly sits after the tab-now-empty early return in every one of the three mutating paths that can empty a tab (close_pane, remove_pane_for_move). Also read error.rs, lib.rs x2 and target.rs's diff in full: small, consistent with the plan, nothing missed.

No regressions since the first review: diffed 531a3df (the fix commit) against the two merge commits that followed and confirmed the fix itself touched only test code plus one import line, and `just check` passes clean at HEAD (efb640b) when run directly in a fresh worktree, matching the task's own recorded run. PR #23's CI (run 43678259820) is SUCCESS on headRefOid efb640b, the same commit. No code conflicts from main (board-only commits, confirmed by the diffs between the three merge commits).

No new findings.

---
<!-- COMMENTS:END -->
