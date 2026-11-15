---
id: TASK-1.37
title: 'Kernel: focus as a path, set upward'
status: In Progress
assignee:
  - '@bruk.w6'
created_date: '2026-11-15T01:32:00Z'
updated_date: '2026-11-15T05:00:00Z'
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
## Files that change

- `crates/sdk/src/direction.rs` (new): `Direction` enum (`Left`, `Right`, `Up`, `Down`), `Debug, Clone, Copy, PartialEq, Eq, Hash`. It crosses the API (`--direction` on `pane split`, `pane resize`, `pane focus`, doc-2 section 4), so it is defined once in the sdk (doc-16 R105), not in the kernel.
- `crates/sdk/src/lib.rs`: `mod direction; pub use direction::Direction;`.
- `crates/kernel/src/session.rs`: a non-optional `current_tab: TabId` field on `Session`, set to `first_tab` in `Session::new` (R94 guarantees a session is never built empty, so no `Option`). `current_tab()` getter, `pub(crate) fn set_current_tab(&mut self, id: TabId)`. `close_tab` repairs it (see Order of work, step 3).
- `crates/kernel/src/tab.rs`: the same pattern — non-optional `current_pane: PaneId`, set to `first_pane` in `Tab::new`, `current_pane()` getter, `pub(crate) fn set_current_pane`, repaired in `close_pane`.
- `crates/kernel/src/tree.rs`: `Tree::focus_pane(PaneId) -> Result<(), Error>` and `Tree::focus_tab(TabId) -> Result<(), Error>` (AC #1, #2). No new `Error` variants: `PaneNotFound`/`TabNotFound` already exist and cover both.

No change to `crates/kernel/src/error.rs` or `crates/kernel/src/pane.rs`.

## Order of work

1. Add `Direction` to the sdk (file and re-export above), with a unit test that the four variants are distinct. TASK-1.36 (`pane split`/`resize`'s own `--direction`) needs this exact shape too; if its worker lands it first, this step becomes a no-op taken from their branch, not a second definition.
2. `Session`: add `current_tab`, set at construction, getter, `pub(crate)` setter. In `close_tab`, when the tab removed was the current one, repair it: the session's current tab becomes the tab that takes its place in order — the one now at that same index, or, if the closed tab was last, the new last tab. Closing a tab that is not current leaves `current_tab` unchanged. (If the session is left with no tabs, `current_tab` is not repaired — `Tree::close_session` removes the session on the same call, so nothing reads it after.) Unit tests for both branches (closing the current tab among several; closing a non-current one) beside the existing `close_tab` tests.
3. `Tab`: the same field, construction, getter, setter and repair rule for `current_pane`, on `close_pane`. Same two unit tests.
4. `Tree::focus_pane(id)`: find the session whose some tab holds this pane (a new private helper, `session_owning_pane_mut`, alongside the existing `session_owning_tab_mut`/`tab_owning_pane_mut`), set that tab's `current_pane` to `id`, then that session's `current_tab` to the tab's id — both inside the one method call, so AC #2 ("in the same step") is a single mutation, not two the caller must sequence. `PaneNotFound` if no tab holds it.
5. `Tree::focus_tab(id)`: find the owning session (`session_owning_tab_mut`, already there), set its `current_tab`. Does not touch any tab's `current_pane` — `tab focus` only sets the tab current in its session (doc-2 section 4; decision-5 point 1 cascades pane to tab, never tab to pane). `TabNotFound` if missing.
6. Tree-level tests: every session/tab built through `new_session`/`new_tab`/`new_pane` always has a valid current (AC #1); `focus_pane` moves both levels in one call (AC #2); `focus_tab` leaves the pane level alone; both report the matching not-found error on an unknown id; closing the pane or tab that was current, including through the existing close cascade, leaves `current_tab`/`current_pane` pointing at something that still exists (composes steps 2 and 3's repairs across the cascade `close_pane`/`close_tab` already do).
7. AC #4 test, `tree.rs`: build a tree with labelled, coloured sessions, tabs and panes; call `focus_pane`; assert every id, label, colour, pane kind and ordering is exactly what it was, and the only change anywhere in the tree is the focused-on tab's `current_pane` and its session's `current_tab`. This is the strongest statement the kernel alone can make toward R88/R90 (nothing about clients changes): there is no client type yet (TASK-1.42), so the proof is that focusing touches only the current-path fields and nothing else a client could later read differently.
8. `new_session`/`new_tab`/`new_pane` do not change any current path themselves. Whether a newly created tab or pane becomes current is the `--focus`/`--no-focus` decision at the CLI/API layer (doc-2 section 9), made by calling `focus_tab`/`focus_pane` afterward — out of this task's scope, left to TASK-1.43.
9. **Not built in this task, and said here plainly**: AC #3, focusing by direction. `doc-2` section 4's `--direction`/`--wrap` on `pane focus` needs the neighbour of a pane in a direction, which needs the layout tree's geometry (nested horizontal/vertical splits, R95) to answer honestly. `Tab::panes` is today a flat `Vec<Pane>` (its own doc comment calls it "a placeholder for TASK-1.36's layout tree") with no up/down adjacency and no principled left/right adjacency either — building this now would mean inventing a geometry for a list, which the orchestrator's brief says not to do.

   The interface the two tasks need to agree on, so TASK-1.36 knows what its tree must support and this task knows what to call once it exists:

   ```rust
   // On Tab, once TASK-1.36 has replaced `panes: Vec<Pane>` with the split tree.
   pub(crate) enum DirectionalFocus {
       Neighbour(PaneId),   // the pane in that direction
       Edge,                // already at the edge, wrap is off or there is nowhere to wrap to
       Wrapped(PaneId),     // wrap is on; this is the pane at the far side
   }
   pub(crate) fn neighbour(&self, from: PaneId, direction: Direction, wrap: bool) -> DirectionalFocus;
   ```

   `Tree::focus_direction(from: PaneId, direction: Direction, wrap: bool) -> Result<DirectionalFocus, Error>` would call this and, on `Neighbour`/`Wrapped`, do exactly what `focus_pane` does (set the tab's `current_pane` and the session's `current_tab` in one step); on `Edge`, change nothing and return `Edge` — not an error, since doc-2 section 4 says the command exits 0 and stays put. `DirectionalFocus` is kernel-internal, not an sdk shape: the wire-facing result of `pane focus --direction` is success or a target error either way: whether it moved or stayed at the edge is not a field the API reports.

   This task recommends adding TASK-1.36 as a dependency of TASK-1.37, so step 9 is built once `Tab`'s split tree (and the `neighbour` query above) exists, rather than guessed at now. The alternative — moving AC #3 out to a new task once TASK-1.36 lands — needs the `finish-moved` label, which is not a worker's to set; that choice is the engineer's.

## Risks

- The close-repair rule (steps 2, 3) is the one place a reviewer will press hardest, since it is the orchestrator's explicit ask: both directions (closing the current vs. a non-current sibling, closing the last vs. a middle one) need their own test, at both the session and the tab level.
- `Direction` (step 1) is wanted by both this task and TASK-1.36; if both land it, one worktree's rebase hits a trivial conflict in `crates/sdk/src/direction.rs` and `lib.rs`. Low risk, resolved by keeping either copy since this plan fixes the shape both agree on.
- `crates/kernel/src/tab.rs` and `crates/kernel/src/tree.rs` are also where TASK-1.36 is working (replacing `panes: Vec<Pane>` with the split tree, and changing `close_pane`'s cascade to give a closed pane's space to the other child of its split). This task's changes to those files (the `current_pane` field and its repair rule, `focus_pane`/`focus_tab`) are additive and about a different concern (focus, not layout), but whichever of the two tasks merges second will need to rebase through the other's changes to the same functions.
- AC #3 is left undelivered by design (step 9). If the engineer does not sequence TASK-1.36 ahead of it, this task cannot honestly close against its own acceptance criteria — said here rather than worked around with an invented geometry.
- Making both currents non-optional (never `Option<TabId>`/`Option<PaneId>`) means every construction path must set them correctly; a missed path leaves a dangling id that only an integration-style test across `new_session`/`new_tab`/`new_pane` would catch (covered by step 6's AC #1 test, run against every creation path, not just one).

## Proof

Recipe names as they are on `origin/main` today (`just check`'s line there is `check-fmt lint check-boundaries deny test test-tools check-architecture check-recipes`; this worktree's own `justfile` predates that rename from TASK-1.73 and should not be relied on for names).

- `just test` after each step in `crates/sdk` and `crates/kernel` (steg-affected selects both, since `kernel` depends on `sdk`): the new `direction.rs` unit test (step 1); `session.rs`'s and `tab.rs`'s close-repair tests (steps 2, 3) prove the repair rule directly; `tree.rs`'s tests from step 6 prove AC #1 (every creation path has a valid current) and AC #2 (`focus_pane` moves both levels in one call, and `focus_tab` does not move the pane level); the not-found tests extend the existing `a_not_found_id_at_any_level_is_the_matching_error` table.
- `tree.rs`'s AC #4 test (step 7) is the proof for AC #4, in the same `just test` run.
- `just check` before the task is called done: `check-fmt`, `lint`, `check-boundaries` (the new `direction.rs` module is plain data with no I/O and no new external dependency, so it should pass with no change to `boundaries.toml`), `deny`, `test`, `test-tools`, `check-architecture`, `check-recipes`.
- AC #3 has no proof here: it is out of scope for this task's commits (step 9), pending TASK-1.36 or a resequencing the engineer decides on.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Ordered after TASK-1.36 by @bruk.o on 2026-11-15. The plan written for this task says, rightly, that focusing by direction cannot be built on a tab's flat list of panes without inventing a geometry, and it would otherwise rewrite the same files the layout task rewrites. So the whole task waits for the layout tree and is built once. The plan stays on the task as the starting point; it is to be revised against TASK-1.36's result, which is asked to provide the direction shape in the sdk and a pane's neighbour in a direction. The claim was released to free the slot.
<!-- SECTION:NOTES:END -->
