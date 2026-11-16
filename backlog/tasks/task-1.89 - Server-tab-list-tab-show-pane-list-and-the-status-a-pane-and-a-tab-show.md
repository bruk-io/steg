---
id: TASK-1.89
title: 'Server: tab list, tab show, pane list, and the status a pane and a tab show'
status: In Progress
assignee:
  - '@bruk.w5'
created_date: '2026-11-16T00:33:00Z'
updated_date: '2026-11-16T04:36:00Z'
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
## For the engineer

**The status field rename.** `PaneRecord.status: Option<String>` (today: `"running"`/
`"ended"`, `ProgramStatus`'s two words) is renamed to `program_status`, same meaning,
same words. `status` becomes a new field, typed `steg_sdk::Status` (doc-11: running,
finished, failed with its exit code, or none for an idle shell) - the value
`pane_fields()`'s own standing doc comment already said `status` should hold.
**What this changes today:** nothing outside this repository, and nothing inside it
either - there is no CLI yet (`crates/steg/src/main.rs` has no command generation, no
`status` rendering) and no other consumer of `PaneRecord` than this crate's own tests,
all updated as part of this plan. The only effect is on this task's own test
assertions and on the API description's self-report (`api.show pane.show`), not on any
running client. The alternative (keep `status` meaning `ProgramStatus`, invent a new
name for doc-11's value) was rejected: it contradicts `pane_fields()`'s own doc comment
and the kernel's own `Pane::program_status`/`Pane::status` naming, whose doc comment
gives the same reasoning in the kernel's own words.

**The `kind`/`size` gap on `PaneRecord`.** doc-2's `pane show` note lists "Kind, size,
working directory, title, running program, and its exit code"; `pane_fields()` has
neither `kind` nor `size` today. Both are small, so both are built here, not deferred:
`kind` is `pane.kind()` (`PaneKind`, already public, already `Serialize`) with nothing
to compute; `size` is `Option<Size>`, `None` when nobody has declared a size for the
tab yet ("where known", AC #1), built from `Tab::size()` (already public) and the same
`Tree::tab_layout`/`Layout::leaf_sizes` this plan already adds for `TabRecord.layout`.
`Size` itself gains `Serialize`/`Deserialize` (plain `{columns, rows}`, nothing to
validate) - the one new derive this adds anywhere in the sdk.

## Split, done on the board

Two drafts exist from this task's split, not yet accepted:
- **"Server: pane resize"** - `Tree::resize_pane` takes a `Ratio`; doc-2 gives
  `--direction`/`--by <cells>`. Converting a cell delta needs the pane's current leaf
  size, and `Tab::size()` can be `None` with no cell basis at all - a design question.
- **"Kernel and server: move a tab to another session"** - no `Tree` operation exists;
  one has to hold R94/R114's current-tab invariants on both sessions.

Two already had an id before this task started:
- **TASK-1.90**, accepted earlier today for `pane.send`/`pane.capture`/`pane.wait` -
  this task's own finding (dispatch is pure/no-I/O by design, so these three need a
  `Runtime`-level wrapper like `start_and_dispatch`'s, not a dispatch arm) is recorded
  there.
- **The events task split from TASK-1.46**, "Server: events for layout, for a session
  or tab changing, for clients, and the one addressed to a single client" - owns
  `session.focus` (decision-5/R87: a client to address, which needs an event).

Every `not_implemented_yet` citation this task's own grep found pointing at a Done task
is reworded to name its real owner: the two not-yet-accepted drafts by title (so the id
can be dropped in once accepted - whoever later builds either removes the citation
entirely along with the arm, so no second edit is owed for it), TASK-1.90 and the
events task by its own title (no id yet either). None is left citing TASK-1.39, 1.40,
1.41, or 1.42 - all Done.

## Files that change

- `crates/sdk/src/layout.rs`: `Size` gains `#[derive(Serialize, Deserialize)]`, plain
  `{columns, rows}` - a round-trip test added alongside its existing ones.
- `crates/sdk/src/record.rs`: `PaneRecord.status` renamed to `program_status`
  (unchanged meaning/words). New `PaneRecord.status: Option<Status>` (doc-11, via
  `pane.status(now)`). New `PaneRecord.kind: PaneKind` and `PaneRecord.size: Option<Size>`.
  New `TabRecord` (id, label, title, position, current_pane: PaneId, layout: Layout,
  shown_status: Option<PaneId>) - exactly `tab_fields()`'s seven fields, the shape
  TASK-1.46's own nested-tree snapshot now depends on this task for and embeds as-is.
  `title` on both records is always `None` - read on demand from the tree at request
  time, same as `cwd` (`Program::cwd()`, the *started-in* directory, not the live one
  `PaneHandle` separately tracks via real OSC 7); neither is cached and nothing polls
  for a change, which is what TASK-1.46's own plan assumes holds. Update/add the
  `#[cfg(test)]` fixtures and the two `matches_the_fields_*_declare` tests' example
  builders; add `TabRecord`'s own round-trip, null-field and fields-match-the-
  description tests, mirroring `PaneRecord`'s.
- `crates/sdk/src/description/methods.rs`: `pane_fields()` gains `program_status`
  (`ValueKind::Text`), `kind` (`ValueKind::Choice(vec!["terminal"])`, same pattern as
  `colour`) and `size` (`ValueKind::Text` placeholder, same choice `layout`/
  `shown_status` already made - `ValueKind` still has no structured variant; named as a
  follow-up, not fixed here). `status`'s own doc comment updated: no longer a
  placeholder waiting on a `PaneRecord`, it is `PaneRecord.status` now. `tab_fields()`'s
  doc comment gets the same "no longer a placeholder" update, pointing at `TabRecord`.
- `crates/sdk/src/lib.rs`: add `TabRecord` to the `record::` re-export.
- `crates/kernel/src/tree.rs`: add `pub fn tab_layout(&self, tab: TabId) -> Result<&Layout, Error>`,
  the same shape as `tab_shown_status` wrapping `Tab::layout()` - no visibility change on
  `Tab` itself (`tree.rs` is in the same crate; `pub(crate)` already reaches it). Two
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
  `target_tab`, finds its owning session for `position` (new small helper below), reads
  `current_pane`/`layout` (via `Tree::tab_layout`)/`shown_status` (via
  `Tree::tab_shown_status(id, now)`). `move`'s `--to` arm's message reworded to name
  "Kernel and server: move a tab to another session" by title.
- `crates/server/src/dispatch/pane.rs`: `dispatch()` gains `now: Moment`. `list` built
  for real: every pane of `target_tab`'s tab, in R112 order, each through the same
  record-builder `show` already has, now shared as one function taking `now`. That
  builder renames its `status` field to `program_status` (unchanged logic), adds
  `status: pane.status(now)` (no new rendering - `Status` already derives `Serialize`),
  `kind: pane.kind()`, and `size` (via `Tab::size()` + `Tree::tab_layout`'s
  `Layout::leaf_sizes`, `None` when the tab has no declared size). `resize`'s message
  reworded to name "Server: pane resize" by title. `send`/`capture`/`wait`'s message
  reworded to name TASK-1.90.
- `crates/server/src/dispatch/session.rs`: `focus`'s message reworded to name the
  events task split from TASK-1.46 ("Server: events for layout, for a session or tab
  changing, for clients, and the one addressed to a single client") by title, dropping
  both the stale TASK-1.42 half and the former direct TASK-1.46 reference.
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
  `pane.show` carries `kind`, `size` (both with and without a declared tab size),
  `program_status` and `status` for a running, a finished and a failed pane; the
  keystone test - record an exit at `moment(0)`, read `pane.show` at `moment(599)`
  (still `Finished`) and at `moment(600)` (gone) through `dispatch` directly, with no
  real clock and no wait - proving the ten-minute mark end to end through the function
  whose signature just changed for exactly this. A negative control reading the same
  pane's `program_status` at `moment(600)` to show it is unaffected (still "ended")
  while `status` alone has expired.
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

1. `crates/sdk/src/layout.rs`: `Size`'s new derive, its round-trip test.
2. `crates/kernel/src/tree.rs`: add `tab_layout`, with its two tests. Independent of
   everything else so far.
3. `crates/sdk/src/record.rs` + `crates/sdk/src/description/methods.rs` +
   `crates/sdk/src/lib.rs`: the field rename, the new `status`/`kind`/`size` fields,
   `TabRecord`, and the description's matching fields - the sdk compiles and its own
   tests pass before anything in `server` is touched, since `server` depends on it.
4. `crates/server/src/dispatch/mod.rs`, `api.rs`: add the `now: Moment` parameter and
   thread it through the one recursive call - compiles once `tab::dispatch`/
   `pane::dispatch` below also take it (steps 5-6 land together with this one, or the
   crate will not build in between; do all three before checking).
5. `crates/server/src/dispatch/lookup.rs`: add `tab_position`.
6. `crates/server/src/dispatch/tab.rs`: `now` param, real `list`/`show`, the reworded
   `move` message.
7. `crates/server/src/dispatch/pane.rs`: `now` param, real `list`, the renamed/added
   status/kind/size fields on `show`'s builder (shared with `list`), the reworded
   `resize`/`send`/`capture`/`wait` messages.
8. `crates/server/src/dispatch/session.rs`: the reworded `focus` message.
9. `crates/server/src/dispatch/outcome.rs`: the three new `Outcome` variants.
10. `crates/server/src/runtime.rs`: compute and pass `now` at both call sites.
11. `crates/server/tests/dispatch.rs`: the mechanical `moment(...)` threading first (get
    the file compiling and green against the renamed/added fields alone), then the new
    tests.
12. `crates/server/tests/runtime.rs`: the `program_status` rename in existing
    assertions, then the new real-pty tests.
13. `crates/server/tests/routing.rs`: the real-socket test.
14. `just check` for the whole change.

## Risks

- **The field rename and the two new fields change `PaneRecord`'s shape.** Covered for
  the engineer at the top of this plan: no reader outside this task's own tests exists
  yet, so this is free today and would not be if the CLI (TASK-1.13 onward) existed.
- **`ValueKind` has no structured variant**, so `status`/`layout`/`shown_status`/`size`
  all stay `ValueKind::Text` in the description despite being structured JSON on the
  wire - a pre-existing choice (`layout`/`shown_status`'s own doc comments already made
  it) this task repeats rather than fixes. Named as a follow-up (a new `ValueKind`
  variant, or at minimum a documented exception to doc-2 section 6's flat-list promise
  for these fields), not built here.
- **~79 mechanical edits in `tests/dispatch.rs`.** Low risk individually (additive, one
  new argument), but a large diff to review. Doing the threading as its own commit,
  before any new test is added (order-of-work step 11), keeps that diff reviewable on
  its own and separate from the new behaviour.
- **TASK-1.46, planned alongside, also touches `dispatch/mod.rs` and `runtime.rs`.**
  Both tasks add a parameter/compute a value at the same two call sites
  (`Runtime::handle`'s delegate branch, `start_and_dispatch`) for unrelated reasons
  (this task: `Moment`; TASK-1.46: an event sender). Whichever merges second rebases
  past a near-certain textual conflict on those few lines, not a design conflict - said
  here so neither worker is surprised.
- **Citing a draft by title, not id.** The two not-yet-accepted split tasks ("Server:
  pane resize"; "Kernel and server: move a tab to another session") and the events task
  are cited by title, since none has an id yet at plan time. No follow-up edit is owed
  for this: whoever later builds any of them removes the citation along with the arm it
  sits in, in the same change.

## Proof

- `cargo test -p steg-sdk layout` - `Size`'s new round-trip test.
- `cargo test -p steg-kernel tab_layout` - the two new kernel tests (one happy path, one
  `TabNotFound`).
- `cargo test -p steg-sdk record` and `cargo test -p steg-sdk description` - the
  renamed/added `PaneRecord` fields (`program_status`, `status`, `kind`, `size`), the
  new `TabRecord`, and both `matches_the_fields_*_declare` tests green for `session`,
  `pane` and the new `tab`.
- `cargo test -p steg-server --test dispatch` - the mechanical `Moment` threading
  compiles and every existing assertion still passes; the new `tab.list`/`tab.show`/
  `pane.list` happy-path and negative-control tests; `pane.show`'s `kind`/`size`
  (declared and undeclared tab size) alongside `program_status`/`status`; the keystone
  ten-minute test at `moment(599)`/`moment(600)` with no wait, plus its
  `program_status`-is-unaffected negative control.
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
  building - split off onto the tasks named above. `session.focus` building - owned by
  the events task; its reworded message is covered by reading the diff, not a test,
  since there is no behaviour change to assert on.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Final plan checked against doc-2 section 8, doc-11 (R50, R82, R83), decision-7, doc-15 and doc-16 by @bruk.o: ok, with one decision that is the engineer's in approving it. The sdk gains a tab record (id, label, title, position, current pane, layout, shown status), defined here once and embedded as it is by TASK-1.46's tree; the pane record gains its kind and its size where known. The server reads its clock once per request and passes a Moment into dispatch, which stays pure; a status's ten-minute end is shown with an injected Moment, not a wait. The decision: the pane record's existing status field (running or ended) is renamed program_status, and status carries doc-11's Status (running, finished, failed with its exit code, or nothing); nothing outside this repository reads the record yet. Every not-yet answer left in dispatch names its real owner: pane resize and moving a tab to another session by the titles of the drafts split from this task; pane send, capture and wait TASK-1.90; session focus the events task split from TASK-1.46, whose dispatch arm this task does not touch. A pane's cwd is the directory its program was started in and its title is empty until the kernel holds one: both read at the request, no polling.
<!-- SECTION:NOTES:END -->
