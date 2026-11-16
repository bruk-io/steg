---
id: TASK-1.89
title: 'Server: tab list, tab show, pane list, and the status a pane and a tab show'
status: In Progress
assignee:
  - '@bruk.w5'
created_date: '2026-11-16T00:33:00Z'
updated_date: '2026-11-16T04:34:00Z'
labels:
  - size-3
milestone: m-3
dependencies:
  - TASK-1.45
  - TASK-1.84
parent_task_id: TASK-1
project: bruk-io-steg
asdlc_accepted_from: DRAFT-1
asdlc_branch: task-1.89-server-tab-list-tab-show-pane-list-and
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found on 2026-11-15 from TASK-1.84's plan: no task owned the list and show methods of tab and pane beyond pane show, nor putting doc-11's status on the wire. dispatch/tab.rs answers tab list and tab show with NotImplementedYet citing TASK-1.39 and TASK-1.41, both Done; dispatch/pane.rs the same for pane list (TASK-1.76) and pane resize (TASK-1.42). The CLI (TASK-1.49) and the app's bar (TASK-1.54) both need them. doc-2 section 8, doc-11 R50, R82, R83, decision-7. The worker of TASK-1.84 recommended a second field on the pane record over changing the meaning of the first. If the plan finds the leftover methods are a separate change from the records, split them.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 tab list, tab show and pane list answer over the API with the records doc-2 section 8 gives, built from the kernel's tree: title, layout, active pane, size where known, and for a tab the status it shows (decision-7's ranking, already in the kernel)
- [ ] #2 A pane's record carries what steg knows about its program right now (doc-11: running, finished, failed with its exit code, or nothing for a shell idle at its prompt) as a field of its own beside the existing running-or-ended fact, in the sdk's Status type; the server reads its clock once per request and passes it to dispatch as a Moment, and the kernel reads none. The plan says whether the existing field keeps its name and meaning, for the engineer
- [ ] #3 The remaining session, tab and pane methods that dispatch still answers NotImplementedYet for are each either built here or named with the task that owns them: pane resize (the kernel's Tab::size exists since TASK-1.42), session focus, and tab move to another session (the kernel has no such operation). None is left pointing at a task that is Done
- [ ] #4 The description's fields for these records match what is returned, shown by the completeness test; each method has a test through Runtime::handle, and a typed command's status reaches pane show and tab show in a real-pty test
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
## Split, read first

This is more than one reviewable change. Keep in this task only the records and the
status plumbing (AC #1, #2, #4, and fixing every stale `not_implemented_yet` citation
within the session/tab/pane nouns). Recommend the engineer draft three new tasks for
the leftover methods AC #3 names, each a real kernel gap this task's own grep found,
not a records change:

- **`pane.resize`**: `Tree::resize_pane` takes a `Ratio` (permille); doc-2 gives
  `--direction`/`--by <cells>`. Converting a cell delta to a ratio needs the pane's
  current leaf size, and `Tab::size()` can be `None` (no cell basis at all then) - a
  real design question, not a wiring job.
- **`tab.move --to`**: no `Tree` operation moves a tab between sessions; one has to
  honour R94/R114's current-tab invariants on both sessions.
- **`pane.send`/`pane.capture`/`pane.wait`**: `dispatch` is deliberately pure, no I/O
  (`mod.rs`'s own doc). These three need a live `PaneHandle`, so they need a
  `Runtime`-level wrapper the same shape as `start_and_dispatch`'s, not a dispatch arm.
  Their current citation (TASK-1.40) is Done and wrong: TASK-1.40 shipped the engine:
  the gap is architectural, dispatch has no I/O in its signature by design.

`session.focus` stays cited here (TASK-1.46, events, still open) - no split needed,
just drop the stale TASK-1.42 half of its citation.

## Files that change

- `crates/sdk/src/record.rs`: rename `PaneRecord.status: Option<String>` to
  `PaneRecord.program_status: Option<String>` (unchanged meaning: `ProgramStatus`'s
  "running"/"ended", same words `pane.rs`'s own `status_text` renders today). Add
  `PaneRecord.status: Option<steg_sdk::Status>` (doc-11's richer running/finished/
  failed, or none for an idle shell) - the field the description's own `pane_fields()`
  doc comment already describes. Add `TabRecord` (id, label, title, position,
  current_pane: PaneId, layout: Layout, shown_status: Option<PaneId>) - exactly
  `tab_fields()`'s seven fields, nothing added (its own doc comment already names them;
  `size`/`kind` gaps doc-2 shows against `pane_fields()` are pre-existing and out of
  scope - name them in notes, build neither). `title` on `TabRecord` is always `None`,
  same reasoning as `PaneRecord.title`: it needs I/O this crate's types never do.
  Update/add the `#[cfg(test)]` fixtures and the two `matches_the_fields_*_declare`
  tests' example builders; add `TabRecord`'s own round-trip, null-field and
  fields-match-the-description tests, mirroring `PaneRecord`'s.
- `crates/sdk/src/description/methods.rs`: `pane_fields()` gains a `program_status`
  field (`ValueKind::Text`, "What ProgramStatus tells us now: running, or ended with its
  exit code"). Update the `status` field's own doc comment: no longer a placeholder
  waiting on a `PaneRecord`, it is `PaneRecord.status` now - `ValueKind::Text` stays
  because `ValueKind` still has no structured variant (name that gap in the comment,
  as a follow-up, not fixed here - same choice `layout`/`shown_status` already made).
  `tab_fields()`'s own doc comment gets the same "no longer a placeholder" update,
  pointing at the new `TabRecord`.
- `crates/sdk/src/lib.rs`: add `TabRecord` to the `record::` re-export.
- `crates/kernel/src/tree.rs`: add `pub fn tab_layout(&self, tab: TabId) -> Result<&Layout, Error>`,
  the same shape as `tab_shown_status` wrapping `Tab::layout()` - no visibility change on
  `Tab` itself (`tree.rs` is in the same crate, so `pub(crate)` already reaches it). Two
  tests: a known tab's layout comes back, an unknown tab is `Error::TabNotFound`.
- `crates/server/src/dispatch/mod.rs`: `dispatch()` gains a sixth parameter,
  `now: steg_kernel::Moment`, right after `request` - a request's own moment, not a
  connection default, so it does not join `Context`. Threaded only into the two
  branches that need it (`tab::dispatch`, `pane::dispatch`); `session`/`setting`/`api`
  dispatch keep their own signatures. The `api.call` interception at the top of this
  function now passes `now` to `api::call` too.
- `crates/server/src/dispatch/api.rs`: `call()` gains `now: Moment`, passed straight
  through to its own recursive `super::dispatch(...)` call - the one other production
  caller of the top-level `dispatch`.
- `crates/server/src/dispatch/tab.rs`: `dispatch()` gains `now: Moment`. `list`/`show`
  built for real (AC #1): `list` walks `target_session`'s `tabs()` in order, building a
  `TabRecord` per tab (`position` one-based in that order); `show` resolves one tab via
  `target_tab`, finds its owning session for `position` (new small helper below),
  reads `current_pane`/`layout` (via `Tree::tab_layout`)/`shown_status` (via
  `Tree::tab_shown_status(id, now)`). `move`'s `--to` arm's message is reworded to
  describe the real gap (no `Tree` op) without naming a task yet - matches this file's
  existing style for that one line.
- `crates/server/src/dispatch/pane.rs`: `dispatch()` gains `now: Moment`. `list` built
  for real: every pane of `target_tab`'s tab, in R112 order, each through the same
  record-builder `show` already has, now shared as one function taking `now`. `show`'s
  builder renames its `status` field to `program_status` (same `status_text` logic,
  unchanged) and adds `status: pane.status(now)`, no new rendering - `Status` already
  derives `Serialize`. `resize`'s message dropped the stale TASK-1.42 citation in favour
  of naming the real gap (cells vs. ratio, `Tab::size()` sometimes unknown), no task
  number yet. `send`/`capture`/`wait`'s message dropped the stale TASK-1.40 citation in
  favour of naming the real gap (needs a live `PaneHandle`; dispatch has no I/O by
  design), no task number yet.
- `crates/server/src/dispatch/session.rs`: `focus`'s message drops the stale TASK-1.42
  half, keeping only "a client to address (TASK-1.46)".
- `crates/server/src/dispatch/lookup.rs`: add `pub(crate) fn tab_position(tree: &Tree,
  tab: TabId) -> Result<usize, ApiError>` (one-based, found via the owning session's
  `tabs()` - same lookup shape `session_owning_tab` already does for `tab_mut`), used by
  `tab.rs`'s `show`.
- `crates/server/src/dispatch/outcome.rs`: add `Outcome::TabRecord`/`TabRecords`/
  `PaneRecords` variants (mirroring the existing `PaneRecord`/`SessionRecord`/
  `SessionRecords` ones), each a thin `serde_json::to_value`; extend `into_value`'s
  tests the same way.
- `crates/server/src/runtime.rs`: `Runtime::handle`'s general-dispatch branch and
  `start_and_dispatch` each compute `now` the same way `apply_event_locked` already
  does (`Moment::new(Instant::now().duration_since(state.clock_origin))`) right before
  their own call to `dispatch::dispatch`, and pass it. No change to `Runtime::handle`'s
  own public signature - callers (`connection.rs`) are unaffected.
- `crates/server/tests/dispatch.rs`: add a `fn moment(secs: u64) -> Moment` test helper
  (same pattern `kernel`'s own test modules use) and thread one `Moment` argument
  through every one of this file's ~79 direct calls to `dispatch(...)` - mechanical,
  additive, no behaviour change to the ~70 that don't care which moment. Add: tab.list/
  tab.show/pane.list happy-path tests plus their negative controls (no such target,
  ambiguous target - matching this file's existing per-method pattern); a test that
  `pane.show` carries both `program_status` and `status` for a running, a finished and
  a failed pane; the keystone test - record an exit at `moment(0)`, read `pane.show` at
  `moment(599)` (still `Finished`) and at `moment(600)` (gone) through `dispatch`
  directly, with no real clock and no wait - proving the ten-minute mark end to end
  through the function whose signature just changed for exactly this. A negative
  control reading the same pane's `program_status` at `moment(600)` to show it is
  unaffected (still "ended") while `status` alone has expired.
- `crates/server/tests/runtime.rs`: the ~7 existing assertions on `record["status"]`
  (all currently reading "running"/"ended") move to `record["program_status"]` - they
  are about the clock-free fact, which is what they always tested. Add one real-pty test
  per doc-11 state reaching `pane.show`/`tab.show`/`shown_status` through
  `Runtime::handle`: a command pane's own exit (`Status::Finished`/`Failed`), and a
  typed command's start/end mark in a shell pane (TASK-1.84's real OSC 133, the same
  technique this file's sibling tests already use) producing `Status::Running` then
  `Status::Finished`/`Failed` - this is "each method through `Runtime::handle`" and "a
  real pty" together, so it does not need its own separate real-pty pass.
- `crates/server/tests/routing.rs`: one test, over a real socket with the sdk's own
  `Connection` (this file's existing style), calling `tab.list`, `tab.show` and
  `pane.list` and checking the records come back with the right field names - "the
  lists and shows over a real socket with the sdk's client".

## Order of work

1. `crates/kernel/src/tree.rs`: add `tab_layout`, with its two tests. Runs alone,
   nothing downstream depends on anything else yet.
2. `crates/sdk/src/record.rs` + `crates/sdk/src/description/methods.rs` +
   `crates/sdk/src/lib.rs`: the field rename, the new `status` field, `TabRecord`, and
   the description's matching fields - the sdk compiles and its own tests pass before
   anything in `server` is touched, since `server` depends on it.
3. `crates/server/src/dispatch/mod.rs`, `api.rs`: add the `now: Moment` parameter and
   thread it through the one recursive call - compiles once `tab::dispatch`/
   `pane::dispatch` below also take it (steps 4-5 land together with this one, or the
   crate will not build in between; do all three before checking).
4. `crates/server/src/dispatch/lookup.rs`: add `tab_position`.
5. `crates/server/src/dispatch/tab.rs`: `now` param, real `list`/`show`, the reworded
   `move` message.
6. `crates/server/src/dispatch/pane.rs`: `now` param, real `list`, the renamed/added
   status fields on `show`'s builder (shared with `list`), the reworded `resize`/`send`/
   `capture`/`wait` messages.
7. `crates/server/src/dispatch/session.rs`: the reworded `focus` message.
8. `crates/server/src/dispatch/outcome.rs`: the three new `Outcome` variants.
9. `crates/server/src/runtime.rs`: compute and pass `now` at both call sites.
10. `crates/server/tests/dispatch.rs`: the mechanical `moment(...)` threading first (get
    the file compiling and green against the renamed field alone), then the new tests.
11. `crates/server/tests/runtime.rs`: the `program_status` rename in existing
    assertions, then the new real-pty tests.
12. `crates/server/tests/routing.rs`: the real-socket test.
13. `just check` for the whole change.

## Risks

- **The field rename breaks every reader of `PaneRecord.status` as "running"/"ended"**,
  wherever one exists outside this crate's own tests (the CLI does not exist yet, so
  the only readers today are this task's own tests, all counted above). The alternative
  - leave `status` as the old string, invent a new name for the doc-11 value - was
  rejected because it contradicts `pane_fields()`'s own standing doc comment and the
  kernel's own `Pane::status`/`Pane::program_status` naming (`crates/kernel/src/pane.rs`'s
  doc: "a single field once called `status`... would be the wrong name for either
  meaning now that both exist"). Flagged for the engineer at the top of this plan's
  predecessor message; recorded here as the decision taken, with the alternative and
  why it loses.
- **`ValueKind` has no structured variant**, so `status`/`layout`/`shown_status` all
  stay `ValueKind::Text` in the description despite being structured JSON on the wire -
  a pre-existing choice (`layout`/`shown_status`'s own doc comments already made it)
  this task repeats rather than fixes. Named as a follow-up (a new `ValueKind` variant,
  or at minimum a documented exception to doc-2 section 6's flat-list promise for these
  three fields), not built here.
- **~79 mechanical edits in `tests/dispatch.rs`.** Low risk individually (additive, one
  new argument), but a large diff to review. Doing the threading as its own commit,
  before any new test is added (order-of-work step 10), keeps that diff reviewable on
  its own and separate from the new behaviour.
- **TASK-1.46, planned alongside, also touches `dispatch/mod.rs` and `runtime.rs`.**
  Both tasks add a parameter/compute a value at the same two call sites
  (`Runtime::handle`'s delegate branch, `start_and_dispatch`) for unrelated reasons
  (this task: `Moment`; TASK-1.46: an event sender). Whichever merges second rebases
  past a near-certain textual conflict on those few lines, not a design conflict - said
  here so neither worker is surprised. TASK-1.46 should take the `events` field/variant
  naming; this task owns nothing TASK-1.46 needs to build around except the now-sixth
  `dispatch` parameter, which TASK-1.46's own calls into `dispatch` (if any) will need
  to pick up.
- **`session.focus` stays unbuilt.** Its message's own citation is correct once reworded
  (TASK-1.46 only); nothing here is a regression, but the acceptance criterion reads as
  if every leftover method moves - say explicitly in the PR that this one did not, and
  why (decision-5/R87: it needs a client to address, which needs events).

## Proof

- `cargo test -p steg-kernel tab_layout` - the two new kernel tests (one happy path, one
  `TabNotFound`).
- `cargo test -p steg-sdk record` and `cargo test -p steg-sdk description` - the renamed/
  added `PaneRecord` fields, the new `TabRecord`, and both `matches_the_fields_*_declare`
  tests green for `session`, `pane` and the new `tab`.
- `cargo test -p steg-server --test dispatch` - the mechanical `Moment` threading
  compiles and every existing assertion still passes; the new `tab.list`/`tab.show`/
  `pane.list` happy-path and negative-control tests; the keystone ten-minute test at
  `moment(599)`/`moment(600)` with no wait, plus its `program_status`-is-unaffected
  negative control.
- `cargo test -p steg-server --test runtime` - the `program_status` rename in the
  existing assertions; the new real-pty tests proving a command pane's exit and a typed
  command's real OSC 133 marks both reach `status` on `pane.show`/`tab.show`/
  `shown_status` through `Runtime::handle` (AC #4's "through a real pty" and "through
  `Runtime::handle`" together).
- `cargo test -p steg-server --test routing` - the new real-socket test for `tab.list`/
  `tab.show`/`pane.list` with the sdk's own `Connection`.
- `just check` - the whole change: format, lint, crate boundaries, the architecture
  model, and every test above.
- Dropped, not proved: `pane.resize`, `tab.move --to`, `pane.send`/`capture`/`wait`
  building - split off, per this plan's own top section. `session.focus` building -
  still blocked on TASK-1.46; its reworded message is covered by reading the diff, not
  a test, since there is no behaviour change to assert on.

## Update: ownership clarified with the team lead (TASK-1.46/events)

- `session.focus` is not mine: it is an event addressed to one client, owned by the
  new events task split from TASK-1.46. Dropped from "Files that change"/"Order of
  work": `crates/server/src/dispatch/session.rs`'s `focus` arm is left untouched,
  stale TASK-1.42 citation and all - the task that owns it fixes the citation when it
  builds the arm. Said here so the reviewer does not look for that edit; it is a
  deliberate drop, not an oversight.
- `TabRecord`'s field names (`id`, `label`, `title`, `position`, `current_pane`,
  `layout`, `shown_status`) are the shape TASK-1.46's own nested-tree snapshot embeds
  directly - it now depends on this task for the record instead of defining its own.
  Nothing changes in this plan's own design for that reason: the names already matched
  `SessionRecord`/`PaneRecord`'s own convention before this note, so the dependency
  only raises the cost of renaming later, not the shape itself.
- `cwd` (`PaneRecord`) and `title` (`PaneRecord`, `TabRecord`): both are read on demand
  from the tree at the moment of the request, never cached and nothing polls for a
  change. `cwd` is `Program::cwd()`, the directory the program *started* in - the live
  directory a real OSC 7 reports is a different, I/O-backed value `PaneHandle` holds
  (`runtime.rs`'s own `resolve_default_cwd`), never this pure record's business. `title`
  is always `None`, on both records: no part of the kernel's tree holds one yet, and
  this task adds no background watch for one. TASK-1.46's plan assumes the server
  notices neither a live directory nor a live title changing - this task's own fields
  hold that assumption already, for the same underlying reason (dispatch is pure, no
  I/O), and this note makes it explicit rather than leaving it to be inferred.
<!-- SECTION:PLAN:END -->
