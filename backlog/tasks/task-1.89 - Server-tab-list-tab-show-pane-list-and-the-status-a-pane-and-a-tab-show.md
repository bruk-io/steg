---
id: TASK-1.89
title: 'Server: tab list, tab show, pane list, and the status a pane and a tab show'
status: In Progress
assignee:
  - '@bruk.w5'
created_date: '2026-11-16T00:33:00Z'
updated_date: '2026-11-16T05:05:00Z'
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
asdlc_plan_approved_by: bruk
asdlc_plan_approved_hash: 'a99a69b5fa87'
asdlc_plan_approved_size: size-3
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

Check passed: `cargo test -p steg-sdk -p steg-kernel -p steg-server` exited 0 at 2026-11-16T04:51:45+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.89 on task-1.89-server-tab-list-tab-show-pane-list-and at 24506572a88c, last 40 of 884 lines:

```text
test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.06s

     Running tests/shutdown.rs (target/debug/deps/shutdown-28715a1c11dd3b93)

running 8 tests
test stop_called_twice_concurrently_on_an_idle_server_is_harmless ... ok
test a_stalled_handshake_is_closed_and_run_returns_promptly_on_stop ... ok
test stop_on_a_genuinely_idle_server_returns_from_run_within_a_bound ... ok
test stopping_closes_the_listener_removes_the_socket_and_releases_the_lock ... ok
test a_completed_idle_connection_is_closed_and_run_returns_promptly_on_stop ... ok
test many_short_connections_do_not_grow_the_tracked_set ... ok
test stopping_while_a_trickling_handshake_is_in_progress_still_returns_within_a_bound ... ok
test stop_racing_a_client_connecting_still_returns_within_a_bound ... ok

test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.13s

     Running tests/socket_mode.rs (target/debug/deps/socket_mode-7da8aa152f6f03c1)

running 1 test
test the_socket_is_0600_even_under_a_permissive_umask ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests steg_kernel

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests steg_sdk

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests steg_server

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

Check passed: `cargo test -p steg-sdk -p steg-kernel` exited 0 at 2026-11-16T04:56:00+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.89 on task-1.89-server-tab-list-tab-show-pane-list-and at 24506572a88c with uncommitted changes, last 40 of 507 lines:

```text
test wire::frame::tests::frame_constructors_set_the_expected_kind_and_stream ... ok
test wire::frame::tests::two_frames_back_to_back_decode_in_order ... ok
test wire::handshake::tests::a_client_newer_than_the_server_is_refused_naming_both_versions ... ok
test wire::handshake::tests::a_first_frame_that_is_not_json_is_malformed ... ok
test wire::handshake::tests::a_matching_version_is_accepted ... ok
test wire::handshake::tests::a_missing_required_field_is_malformed ... ok
test wire::handshake::tests::a_server_newer_than_the_client_is_refused_naming_both_versions ... ok
test wire::handshake::tests::a_wrong_magic_is_malformed_naming_the_server_version ... ok
test wire::handshake::tests::an_unknown_field_is_ignored ... ok
test wire::handshake::tests::kind_and_label_round_trip_through_the_hello ... ok
test wire::handshake::tests::parse_server_hello_reads_an_accepted_hello ... ok
test wire::handshake::tests::parse_server_hello_surfaces_a_refusals_message ... ok

test result: ok. 142 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests/connection.rs (target/debug/deps/connection-5aaf1820065b9c7d)

running 2 tests
test a_version_mismatch_is_refused_naming_both_versions ... ok
test a_client_and_server_connection_shake_hands_and_exchange_a_call_in_memory ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests/local_socket.rs (target/debug/deps/local_socket-b8a3fda317535471)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests steg_kernel

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests steg_sdk

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

Check passed: `cargo test -p steg-server --lib --test runtime` exited 0 at 2026-11-16T05:00:21+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.89 on task-1.89-server-tab-list-tab-show-pane-list-and at 24506572a88c with uncommitted changes, last 40 of 151 lines:

```text
test runtime::tests::pane_split_falls_back_to_home_when_the_inherited_live_cwd_no_longer_exists ... ok
test runtime::tests::pane_split_inherits_the_live_cwd_of_the_pane_it_split_from_not_its_started_in_cwd ... ok
test runtime::tests::a_typed_command_still_running_stays_running_until_told_otherwise ... ok
kill: 52570: No such process
test runtime::stop_tests::stop_ends_a_program_producing_output ... ok
test runtime::tests::a_shell_with_no_marks_leaves_every_status_field_unchanged_until_one_arrives ... ok
test runtime::tests::the_general_sizing_step_reaches_every_pane_through_split_close_and_swap ... ok
test runtime::tests::a_mark_then_the_shells_own_real_exit_cascades_with_no_panic ... ok
test runtime::tests::a_typed_commands_status_reaches_pane_show_and_tab_show_through_runtime_handle ... ok
test runtime::tests::a_typed_command_that_fails_is_failed_with_its_exit_code ... ok
test runtime::tests::a_typed_command_that_succeeds_is_running_then_finished_and_the_tab_shows_it ... ok
test runtime::tests::a_command_panes_own_program_printing_marks_does_not_change_its_status ... ok
kill: 52569: No such process
test pane::tests::a_forward_failing_once_the_worker_is_gone_still_reaps_a_program_that_ends_on_its_own ... ok

test result: ok. 117 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.02s

     Running tests/runtime.rs (target/debug/deps/runtime-1af0225d788df591)

running 18 tests
test a_nonexistent_cwd_through_api_call_leaves_nothing_behind ... ok
test session_new_with_a_nonexistent_cwd_fails_and_leaves_nothing_behind ... ok
test pane_split_with_no_program_or_cwd_runs_the_login_shell_at_home ... ok
test session_new_through_api_call_with_no_program_runs_the_login_shell ... ok
test session_close_refuses_while_any_pane_in_any_of_its_tabs_is_running ... ok
test pane_split_with_a_nonexistent_cwd_leaves_no_new_pane_behind ... ok
test tab_new_with_a_nonexistent_cwd_leaves_no_new_tab_or_pane_behind ... ok
test a_command_that_does_not_exist_fails_and_leaves_no_session_tab_or_pane_behind ... ok
test pane_close_refuses_while_its_program_is_running_and_touches_nothing ... ok
test a_bad_command_through_api_call_leaves_nothing_behind ... ok
test pane_close_with_force_on_a_running_program_proceeds_exactly_as_today_the_stated_gap ... ok
test pane_split_with_a_bad_command_leaves_no_new_pane_behind ... ok
test tab_close_refuses_while_any_of_its_panes_is_running ... ok
test tab_new_with_a_bad_command_leaves_no_new_tab_or_pane_behind ... ok
test session_new_with_a_command_starts_it_and_pane_show_reports_it_running_then_ended ... ok
test session_new_through_api_call_starts_a_real_program ... ok
test tab_new_through_api_call_starts_a_real_program ... ok
test pane_split_through_api_call_starts_a_real_program ... ok

test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.06s
```

Check passed: `cargo test -p steg-server --test dispatch` exited 0 at 2026-11-16T05:03:05+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.89 on task-1.89-server-tab-list-tab-show-pane-list-and at 24506572a88c with uncommitted changes, last 40 of 67 lines:

```text
test setting_list_returns_every_registered_setting_with_its_current_value ... ok
test api_call_of_session_set_names_the_target_session_not_the_context_default ... ok
test session_set_and_unset_change_and_clear_the_label ... ok
test session_show_matches_exactly_the_fields_the_description_declares ... ok
test setting_set_changes_the_value_the_next_read_sees ... ok
test session_list_matches_exactly_the_fields_the_description_declares_per_record ... ok
test setting_unset_an_unknown_name_is_no_such_target ... ok
test session_move_converts_the_wire_position_before_calling_the_kernel ... ok
test session_move_rejects_a_zero_position_without_touching_the_tree ... ok
test api_call_with_a_non_text_method_is_a_bad_request ... ok
test pane_swap_exchanges_two_panes ... ok
test session_close_removes_it ... ok
test tab_move_with_position_reorders ... ok
test setting_unset_reverts_to_the_default ... ok
test tab_new_with_a_middle_position_places_the_new_tab_there ... ok
test setting_show_returns_one_setting_in_full ... ok
test setting_set_rejects_an_unknown_name_with_no_such_target ... ok
test split_set_label_then_close_the_session ... ok
test api_call_of_tab_set_names_the_target_tab_not_the_context_default ... ok
test tab_new_with_no_focus_leaves_the_original_tab_current ... ok
test tab_new_with_a_position_past_the_end_and_no_focus_creates_nothing_and_restores_focus ... ok
test tab_new_with_no_focus_restores_a_current_tab_that_is_not_the_first ... ok
test tab_close_removes_it ... ok
test tab_new_makes_the_new_tab_current_by_default ... ok
test api_call_of_pane_set_names_the_target_pane_not_the_context_default ... ok
test tab_new_with_no_position_appends_at_the_end_as_before ... ok
test api_call_of_pane_split_gives_the_same_response_a_direct_call_would ... ok
test tab_new_with_a_position_past_the_end_behaves_like_tab_move_and_creates_nothing ... ok
test tab_new_with_no_focus_restores_the_previous_current_tab ... ok
test tab_new_with_position_zero_is_refused_and_creates_nothing ... ok
test tab_focus_makes_it_current ... ok
test api_call_reaches_setting_show_with_its_required_target ... ok
test tab_new_with_position_one_places_the_new_tab_first ... ok
test api_list_returns_every_method ... ok
test every_not_yet_scenario_answers_not_implemented_yet_and_changes_nothing ... ok
test api_show_with_no_target_returns_the_whole_description ... ok
test every_described_method_gets_a_response_with_no_panic ... ok
test api_call_matches_a_direct_call_for_every_served_method_with_its_own_target_given ... ok

test result: ok. 61 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
```

Check FAILED: `cargo test -p steg-server --test dispatch -- a_finished_status_expires_through_dispatch_at_the_ten_minute_mark` exited 101 at 2026-11-16T05:05:21+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.89 on task-1.89-server-tab-list-tab-show-pane-list-and at 24506572a88c with uncommitted changes:

```text
   Compiling steg-server v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.89/crates/server)
warning: unused variable: `now`
  --> crates/server/src/dispatch/pane.rs:83:66
   |
83 | fn record_for(tree: &Tree, tab_id: steg_sdk::TabId, pane: &Pane, now: Moment) -> PaneRecord {
   |                                                                  ^^^ help: if this is intentional, prefix it with an underscore: `_now`
   |
   = note: `#[warn(unused_variables)]` (part of `#[warn(unused)]`) on by default

warning: `steg-server` (lib) generated 1 warning (run `cargo fix --lib -p steg-server` to apply 1 suggestion)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.77s
     Running tests/dispatch.rs (target/debug/deps/dispatch-a3fcf474f697b99f)

running 1 test
test a_finished_status_expires_through_dispatch_at_the_ten_minute_mark_while_program_status_does_not ... FAILED

failures:

---- a_finished_status_expires_through_dispatch_at_the_ten_minute_mark_while_program_status_does_not stdout ----

thread 'a_finished_status_expires_through_dispatch_at_the_ten_minute_mark_while_program_status_does_not' (23998301) panicked at crates/server/tests/dispatch.rs:1128:5:
assertion `left == right` failed: R83: finished ends after ten minutes
  left: String("finished")
 right: Null
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    a_finished_status_expires_through_dispatch_at_the_ten_minute_mark_while_program_status_does_not

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 72 filtered out; finished in 0.00s

error: test failed, to rerun pass `-p steg-server --test dispatch`
```

Check FAILED: `cargo test -p steg-server --test dispatch -- an_idle_shell_panes_program_status_and_status_disagree a_finished_status_expires_through_dispatch_at_the_ten_minute_mark` exited 101 at 2026-11-16T05:05:35+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.89 on task-1.89-server-tab-list-tab-show-pane-list-and at 24506572a88c with uncommitted changes, last 40 of 42 lines:

```text
  --> crates/server/src/dispatch/pane.rs:83:66
   |
83 | fn record_for(tree: &Tree, tab_id: steg_sdk::TabId, pane: &Pane, now: Moment) -> PaneRecord {
   |                                                                  ^^^ help: if this is intentional, prefix it with an underscore: `_now`
   |
   = note: `#[warn(unused_variables)]` (part of `#[warn(unused)]`) on by default

warning: `steg-server` (lib) generated 1 warning (run `cargo fix --lib -p steg-server` to apply 1 suggestion)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.68s
     Running tests/dispatch.rs (target/debug/deps/dispatch-a3fcf474f697b99f)

running 2 tests
test an_idle_shell_panes_program_status_and_status_disagree ... FAILED
test a_finished_status_expires_through_dispatch_at_the_ten_minute_mark_while_program_status_does_not ... FAILED

failures:

---- an_idle_shell_panes_program_status_and_status_disagree stdout ----

thread 'an_idle_shell_panes_program_status_and_status_disagree' (24013158) panicked at crates/server/tests/dispatch.rs:983:5:
assertion `left == right` failed
  left: String("running")
 right: Null
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace

---- a_finished_status_expires_through_dispatch_at_the_ten_minute_mark_while_program_status_does_not stdout ----

thread 'a_finished_status_expires_through_dispatch_at_the_ten_minute_mark_while_program_status_does_not' (24013157) panicked at crates/server/tests/dispatch.rs:1128:5:
assertion `left == right` failed: R83: finished ends after ten minutes
  left: String("finished")
 right: Null


failures:
    a_finished_status_expires_through_dispatch_at_the_ten_minute_mark_while_program_status_does_not
    an_idle_shell_panes_program_status_and_status_disagree

test result: FAILED. 0 passed; 2 failed; 0 ignored; 0 measured; 71 filtered out; finished in 0.00s

error: test failed, to rerun pass `-p steg-server --test dispatch`
```
<!-- SECTION:NOTES:END -->

## Comments

<!-- COMMENTS:BEGIN -->
author: @bruk
created: 2026-11-16T04:49:00Z

---

Plan approved by @bruk (a99a69b5fa87, size-3): its worker may build it.

---
<!-- COMMENTS:END -->
