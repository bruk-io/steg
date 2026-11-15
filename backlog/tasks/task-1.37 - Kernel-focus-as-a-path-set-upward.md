---
id: TASK-1.37
title: 'Kernel: focus as a path, set upward'
status: In Progress
assignee:
  - '@bruk.w6'
created_date: '2026-11-15T01:32:00Z'
updated_date: '2026-11-15T06:03:00Z'
labels:
  - size-2
milestone: m-0
dependencies:
  - TASK-1.35
  - TASK-1.36
parent_task_id: TASK-1
type: task
project: bruk-io-steg
asdlc_accepted_from: DRAFT-7
asdlc_branch: task-1.37-kernel-focus-as-a-path-set-upward
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
doc-15: R85, R88, and focusing by direction from R9. decision-5. doc-2 section 4 says what happens at the edge and with `--wrap`.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Each session has a current tab and each tab a current pane
- [ ] #2 Focusing a pane makes its tab the current tab of its session in the same step
- [ ] #3 Focusing by direction moves to the neighbour, stays put at the edge, and wraps when asked
- [ ] #4 Nothing about clients changes when focus does
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
## What changed since the old plan

TASK-1.36 (merged) already built far more than the old plan assumed, so most of
it is now either done or wrong, not a starting point to edit in place:

- `Direction` already lives in the sdk (`crates/sdk/src/layout.rs`), re-exported
  from `steg_sdk`. No `crates/sdk/src/direction.rs` is needed — drop that file
  from the plan entirely.
- `Tab` already has a non-optional `current: PaneId` field, a `current()`
  getter, and a close-repair rule (`close_pane`, `remove_pane_for_move`), all
  with tests (`split_pane_makes_the_new_pane_current`,
  `closing_the_current_pane_moves_current_to_a_pane_that_still_exists`, the
  multi-leaf-promotion test). AC #1's tab half is done; cite these, don't
  rebuild them.
- `Tab::neighbour(from, direction, wrap) -> DirectionalFocus` and
  `DirectionalFocus { Neighbour(PaneId), Edge, Wrapped(PaneId) }` are already
  built and tested at the tab/layout level, and `DirectionalFocus` is already
  `pub` from `crates/kernel/src/lib.rs`. AC #3's geometry is done. The old
  plan's step 9 ("not built in this task") is wrong — it is buildable now, and
  this plan builds it.
- `Session` has **no** `current_tab` at all yet. That, plus three `Tree`
  methods, is the real gap this task fills.

## Decisions this plan has to make, that doc-15 and decision-5 leave open

1. **What a session's current tab becomes when that tab closes.** Neither
   doc-15 nor decision-5 says. Chosen here, mirroring `Tab::close_pane`'s own
   rule (R94's spirit: something that still exists, nearest in order): the tab
   now at the closed one's old index, or the new last tab if the closed one
   was last. Put the repair in `Session::close_tab` itself (not in `Tree`), so
   `Tree::close_tab`, `Tree::close_pane`'s cascade, and
   `Tree::move_pane_to_tab`'s cascade all get it for free from the one place.
   If the session is left with no tabs, `current_tab` is not repaired —
   `Tree::close_session` removes the session in the same call, so nothing ever
   reads the stale value.
2. **Whether a new tab becomes current.** Doc-2 section 5/9: a new tab takes
   focus by default (`--focus`/`--no-focus`). `Tab::split_pane` already sets
   the rule at its own level unconditionally — a newly split pane becomes
   current *in its tab*, with no parameter to turn that off, and no cascade
   beyond that one level (it does not also become current in its session).
   `Session::add_tab` is made consistent with that exact precedent: a newly
   added tab becomes current *in its session*, unconditionally, with no
   cascade down to any pane. The rule this settles on, stated once: **creating
   something focuses it at its own immediate parent level only; the full
   multi-level R85 cascade is what the explicit `focus` operations below do,
   and only them.** Flagged plainly, not solved here: neither level has a
   kernel parameter for `--no-focus` today. TASK-1.43 (the API layer) has to
   get that behaviour by capturing the previous current tab/pane before
   creating, and calling `Tree::focus_tab`/`Tree::focus_pane` to restore it
   when `--no-focus` is asked for.
3. **Reordering.** `current_tab` is a `TabId`, not a position, so
   `Session::move_tab`/`Tree::move_tab` can't invalidate it — stated and
   tested, not changed.
4. **No current session in the kernel.** R85 gives a current tab and a current
   pane; decision-5 point 3 puts which session a window shows in the app, not
   any server. The kernel adds nothing resembling a "current session" — said
   here so no later task invents one at this layer.
5. **Moving a pane to another tab is not a focus.** `Tree::move_pane_to_tab`
   already makes the moved pane the target tab's current pane (TASK-1.36) —
   that is `--direction`'s insertion point, not `pane focus`. It must not
   change any session's `current_tab`, including the target's. One doc-comment
   line and one test say so.

## Files that change

- `crates/kernel/src/session.rs`:
  - New field `current_tab: TabId` on `Session`, non-optional (R94: a session
    is never empty).
  - `Session::new` sets it to `first_tab`.
  - `pub const fn current_tab(&self) -> TabId` getter.
  - `pub(crate) fn set_current_tab(&mut self, id: TabId) -> Result<(), Error>`,
    membership-checked, the same shape as `Tab::focus` below (`Error::TabNotFound`
    if this session holds no tab with that id).
  - `add_tab`: after pushing the new tab, sets `self.current_tab` to its id
    unconditionally (decision 2 above). One doc-comment line explaining the
    "own immediate level only" rule.
  - `close_tab`: the repair rule from decision 1.
- `crates/kernel/src/tab.rs`:
  - One new method, `pub(crate) fn focus(&mut self, pane: PaneId) -> Result<(), Error>`,
    membership-checked, the same shape as the existing `fill_pane`:
    `Error::PaneNotFound` if `pane` is not in this tab, else `self.current = pane; Ok(())`.
    Nothing else here changes — `current`, `DirectionalFocus` and `neighbour`
    are already TASK-1.36's.
- `crates/kernel/src/tree.rs`:
  - `pub struct Focused { pub session: SessionId, pub tab: TabId, pub pane: PaneId }`
    (`Debug, Clone, Copy, PartialEq, Eq`, the same derive set as `ClosedPane`/
    `MovedPane`) — the resulting whole path after a focus operation, so
    TASK-1.46 can raise one event from it without a further lookup.
  - `pub enum FocusMove { Neighbour(Focused), Wrapped(Focused), Edge }` — the
    same three outcomes `Tab::neighbour`'s `DirectionalFocus` already finds,
    carrying the resulting path instead of a bare `PaneId` so a caller needs no
    further lookup on the two outcomes that moved something. Named
    differently from `tab::DirectionalFocus` (not generic over it) to avoid a
    same-name, different-module type and keep the diff easy to review; both
    are kept, at different levels, for different callers.
  - `pub fn focus_pane(&mut self, id: PaneId) -> Result<Focused, Error>`:
    finds the owning tab (`tab_owning_pane_mut`, existing), calls `tab.focus(id)`,
    finds that tab's owning session (`session_owning_tab_mut`, existing),
    calls `session.set_current_tab(tab_id)` — both inside the one method call,
    so AC #2's "in the same step" is one mutation the caller never sequences
    itself. Returns the resulting `Focused`. `Error::PaneNotFound` if no tab
    holds `id`.
  - `pub fn focus_tab(&mut self, id: TabId) -> Result<Focused, Error>`: finds
    the owning session (`session_owning_tab_mut`), calls `session.set_current_tab(id)`.
    Does not touch any tab's `current` pane — `tab focus` only sets the tab
    current in its session (doc-2 section 4; decision-5 point 1 cascades pane
    to tab, never tab to pane). `Error::TabNotFound` if missing.
  - `pub fn focus_direction(&mut self, from: PaneId, direction: Direction, wrap: bool) -> Result<FocusMove, Error>`:
    finds `from`'s tab, calls `tab.neighbour(from, direction, wrap)`; on
    `Neighbour`/`Wrapped`, calls `self.focus_pane(id)` (reusing it exactly —
    the neighbour or wrap target is always a pane in the same tab as `from`)
    and wraps its `Focused` in the matching `FocusMove` variant; on `Edge`,
    changes nothing and returns `FocusMove::Edge` — not an error, since doc-2
    section 4 says the command exits 0 and stays put. `Error::PaneNotFound` if
    no tab holds `from`.
  - One doc-comment line on `move_pane_to_tab` stating decision 5: it is not a
    focus, and touches no session's `current_tab`.
- `crates/kernel/src/lib.rs`: add `Focused, FocusMove` to the `pub use tree::{...}` line.
- No change to `crates/sdk` (`Direction` is already there), `crates/kernel/src/error.rs`
  (no new variant — `PaneNotFound`/`TabNotFound` cover every case), or
  `crates/kernel/src/layout.rs`-equivalent geometry (already TASK-1.36's).

## Order of work

1. `session.rs`: the field, constructor, getter, `set_current_tab`, and the
   `close_tab` repair rule, each with its own unit test (decision 1: closing
   the current tab among several lands on the one now at that index; closing
   the current *last* tab lands on the new last; closing a non-current tab
   leaves `current_tab` unchanged).
2. `session.rs`: `add_tab`'s unconditional focus (decision 2), with a test
   that a newly added tab is current.
3. `tab.rs`: `focus`, with its own-pane and not-found tests, mirroring
   `fill_pane`'s existing tests exactly.
4. `tree.rs`: `Focused`, `FocusMove`, `focus_pane`, `focus_tab`,
   `focus_direction`, each with the tests in the Proof section below.
5. `tree.rs`: the one-line doc comment on `move_pane_to_tab` plus its test
   (decision 5).
6. `lib.rs`: the export line.
7. Extend the existing `a_not_found_id_at_any_level_is_the_matching_error`
   test (tree.rs) with `focus_pane`, `focus_tab`, `focus_direction` on unknown
   ids.

## Risks

- **`--no-focus` has no kernel affordance, at either level, after this task.**
  `Tab::split_pane` and (now) `Session::add_tab` both focus unconditionally on
  creation. Flagged here, deliberately not solved here (decision 2): TASK-1.43
  must capture-and-restore via `focus_pane`/`focus_tab` rather than finding a
  kernel parameter to flip.
- The close-repair rule (decision 1) is undecided by any doc or decision
  record, so it is this plan's own choice, not a citation — a reviewer may
  press on whether "nearest in order" is the right default; the alternative
  (always the new first tab, say) is equally defensible and would need a
  one-line change plus its three tests updated.
- `session.rs`, `tab.rs` and `tree.rs` are the same files TASK-1.36 just
  landed changes in. This task's additions are new fields/methods, not edits
  to TASK-1.36's own logic (the one exception is the doc-comment line on
  `move_pane_to_tab`), so a rebase conflict here should be mechanical, not a
  logic merge.
- `FocusMove` duplicates `tab::DirectionalFocus`'s three-way shape at a
  different level with a richer payload. This is deliberate (keeps
  `tab::DirectionalFocus` untouched and its existing tests unmodified) but a
  reviewer may ask whether the two should be unified; doing so would mean
  making `tab::DirectionalFocus` generic over its payload, which this plan
  avoids as unnecessary complexity for one caller.

## Proof

All of this is kernel-only: pure data in, data out, no I/O, so every test
below is a plain unit test beside the code it proves (team:testing: kernel
code gets unit tests, no doubles). `just test` after each step in
`crates/kernel` (steg-affected selects it and nothing that depends on it yet);
`just check` before the task is called done.

- **AC #1** (each session has a current tab, each tab a current pane):
  - Tab half: already proven by TASK-1.36's tests, cited above — not rebuilt.
  - Session half: `Session::new` sets `current_tab` to its only tab (new test,
    `session.rs`); `add_tab` makes the new tab current (new test, `session.rs`,
    decision 2); the three close-repair tests (`session.rs`, decision 1) prove
    it stays valid across every close; a reorder leaves it unchanged (new
    test, `session.rs`, decision 3); a `tree.rs` test
    (`closing_the_current_tabs_last_pane_cascades_and_repairs_the_sessions_current_tab`)
    proves the repair also holds when triggered by `Tree::close_pane`'s
    cascade, not just a direct `Session::close_tab` call.
- **AC #2** (focusing a pane makes its tab current in its session, in the same
  step): `focus_pane_moves_the_pane_and_its_tabs_level_together` (`tree.rs`) —
  one call moves both levels and returns the matching `Focused`.
  `focus_tab_does_not_touch_any_tabs_current_pane` (`tree.rs`) is the contrast
  case proving `tab focus` stops at its own level (decision-5 point 1).
- **AC #3** (focus by direction moves to the neighbour, stays at the edge,
  wraps when asked): the geometry (which pane is the neighbour, which is the
  wrap target) is already proven at the `Layout`/`Tab` level by TASK-1.36 —
  cited, not rebuilt. `focus_direction_moves_to_the_neighbour_and_reports_it`,
  `focus_direction_at_the_edge_changes_nothing_and_reports_edge`,
  `focus_direction_wraps_when_asked` (`tree.rs`) prove the wiring: each
  outcome changes (or doesn't change) the tree correctly and comes back as
  the matching `FocusMove` value, so the server can tell a client which of
  the three happened.
- **AC #4** (nothing about clients changes when focus does):
  `focus_pane_changes_only_the_current_path_fields` (`tree.rs`) clones
  `tree.sessions().to_vec()` before calling `focus_pane`, clones it again
  after, and asserts that patching the *before* clone's current-path fields
  by hand (through the same `pub(crate)` setters `focus_pane` itself uses)
  makes it equal the *after* clone exactly — proving every other field
  (label, colour, tab order, pane order, layout, fill) moved nowhere. This is
  the strongest statement the kernel alone can make: there is no client type
  yet (TASK-1.42), so the proof is that focus touches only the two
  current-path fields and nothing else a client could later read
  differently. `move_pane_to_tabs_target_tabs_current_pane_moves_but_no_sessions_current_tab_does`
  (`tree.rs`, decision 5) is the contrast case: a different R9 operation that
  also sets a tab's `current` pane is not itself a focus and must not cascade.
- Not-found cases: `focus_pane`, `focus_tab`, `focus_direction` each added to
  `a_not_found_id_at_any_level_is_the_matching_error` (`tree.rs`).
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Ordered after TASK-1.36 by @bruk.o on 2026-11-15. The plan written for this task says, rightly, that focusing by direction cannot be built on a tab's flat list of panes without inventing a geometry, and it would otherwise rewrite the same files the layout task rewrites. So the whole task waits for the layout tree and is built once. The plan stays on the task as the starting point; it is to be revised against TASK-1.36's result, which is asked to provide the direction shape in the sdk and a pane's neighbour in a direction. The claim was released to free the slot.

Rewritten plan checked against doc-15 (R85, R88, R113) and decision-5 by @bruk.o: ok. It adds only what TASK-1.36 left: a session's current tab, focus_pane (sets the tab's current pane and the session's current tab in one call), focus_tab, and focus_direction acting on the three outcomes, each returning the whole path for one focus event. No current session in the kernel, and moving a pane is not a focus. For the engineer, two rules the specs do not state: when the current tab closes, the tab now at its place becomes current, or the new last tab; and a new tab always becomes current in its session, with --no-focus left to the API layer to restore the previous focus.
<!-- SECTION:NOTES:END -->
