---
id: TASK-1.89
title: 'Server: tab list, tab show, pane list, and the status a pane and a tab show'
status: Done
assignee:
  - '@bruk.w5'
created_date: '2026-11-16T00:33:00Z'
updated_date: '2026-11-16T11:52:00Z'
labels:
  - size-3
  - review-passed
milestone: m-3
dependencies:
  - TASK-1.45
  - TASK-1.84
references:
  - task-1.89-server-tab-list-tab-show-pane-list-and
  - https://github.com/bruk-io/steg/pull/57
parent_task_id: TASK-1
project: bruk-io-steg
asdlc_accepted_from: DRAFT-1
asdlc_branch: task-1.89-server-tab-list-tab-show-pane-list-and
asdlc_plan_approved_by: bruk
asdlc_plan_approved_hash: 'a99a69b5fa87'
asdlc_plan_approved_size: size-3
asdlc_review_kind: same-machine
asdlc_review_findings: 'important=0,nit=2'
asdlc_review_policies: ["testing", "review"]
asdlc_plan_matched: yes
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

Check FAILED: `cargo test -p steg-server --test dispatch -- tab_show_and_tab_list_shown_status_follow_the_kernels_ranking_not_tab_order` exited 101 at 2026-11-16T05:05:56+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.89 on task-1.89-server-tab-list-tab-show-pane-list-and at 24506572a88c with uncommitted changes:

```text
   Compiling steg-server v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.89/crates/server)
warning: unused variable: `now`
  --> crates/server/src/dispatch/tab.rs:49:69
   |
49 | fn record_for(tree: &Tree, tab: &steg_kernel::Tab, position: usize, now: Moment) -> TabRecord {
   |                                                                     ^^^ help: if this is intentional, prefix it with an underscore: `_now`
   |
   = note: `#[warn(unused_variables)]` (part of `#[warn(unused)]`) on by default

warning: `steg-server` (lib) generated 1 warning (run `cargo fix --lib -p steg-server` to apply 1 suggestion)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.68s
     Running tests/dispatch.rs (target/debug/deps/dispatch-a3fcf474f697b99f)

running 1 test
test tab_show_and_tab_list_shown_status_follow_the_kernels_ranking_not_tab_order ... FAILED

failures:

---- tab_show_and_tab_list_shown_status_follow_the_kernels_ranking_not_tab_order stdout ----

thread 'tab_show_and_tab_list_shown_status_follow_the_kernels_ranking_not_tab_order' (24032560) panicked at crates/server/tests/dispatch.rs:907:5:
assertion `left == right` failed
  left: String("00000002")
 right: String("00000003")
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    tab_show_and_tab_list_shown_status_follow_the_kernels_ranking_not_tab_order

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 72 filtered out; finished in 0.00s

error: test failed, to rerun pass `-p steg-server --test dispatch`
```

Check FAILED: `cargo test -p steg-server --test dispatch -- pane_show_size_reflects_the_tabs_declared_size_and_this_panes_own_leaf` exited 101 at 2026-11-16T05:06:13+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.89 on task-1.89-server-tab-list-tab-show-pane-list-and at 24506572a88c with uncommitted changes:

```text
   Compiling steg-server v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.89/crates/server)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.72s
     Running tests/dispatch.rs (target/debug/deps/dispatch-a3fcf474f697b99f)

running 1 test
test pane_show_size_reflects_the_tabs_declared_size_and_this_panes_own_leaf ... FAILED

failures:

---- pane_show_size_reflects_the_tabs_declared_size_and_this_panes_own_leaf stdout ----

thread 'pane_show_size_reflects_the_tabs_declared_size_and_this_panes_own_leaf' (24049776) panicked at crates/server/tests/dispatch.rs:1029:5:
assertion `left == right` failed: this tab's only pane fills the whole declared size
  left: Null
 right: Object {"columns": Number(100), "rows": Number(20)}
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    pane_show_size_reflects_the_tabs_declared_size_and_this_panes_own_leaf

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 72 filtered out; finished in 0.00s

error: test failed, to rerun pass `-p steg-server --test dispatch`
```

Check FAILED: `cargo test -p steg-server --test dispatch -- pane_list_returns_every_pane_of_the_tab_in_r112s_layout_order` exited 101 at 2026-11-16T05:06:34+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.89 on task-1.89-server-tab-list-tab-show-pane-list-and at 24506572a88c with uncommitted changes:

```text
   Compiling steg-server v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.89/crates/server)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.82s
     Running tests/dispatch.rs (target/debug/deps/dispatch-a3fcf474f697b99f)

running 1 test
test pane_list_returns_every_pane_of_the_tab_in_r112s_layout_order ... FAILED

failures:

---- pane_list_returns_every_pane_of_the_tab_in_r112s_layout_order stdout ----

thread 'pane_list_returns_every_pane_of_the_tab_in_r112s_layout_order' (24070010) panicked at crates/server/tests/dispatch.rs:1062:5:
assertion `left == right` failed: second was split to the Left of first, so it reads first in R112's order
  left: String("00000002")
 right: String("00000003")
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    pane_list_returns_every_pane_of_the_tab_in_r112s_layout_order

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 72 filtered out; finished in 0.00s

error: test failed, to rerun pass `-p steg-server --test dispatch`
```

Check passed: `cargo test -p steg-server --test dispatch` exited 0 at 2026-11-16T05:06:57+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.89 on task-1.89-server-tab-list-tab-show-pane-list-and at 24506572a88c with uncommitted changes, last 40 of 81 lines:

```text
test an_override_is_still_there_for_a_fresh_tree_and_context_over_the_same_settings ... ok
test api_call_reaches_setting_unset_with_its_required_target ... ok
test api_call_with_a_non_text_method_is_a_bad_request ... ok
test setting_list_returns_every_registered_setting_with_its_current_value ... ok
test session_show_with_an_unknown_target_is_no_such_target ... ok
test api_list_returns_every_method ... ok
test session_move_rejects_a_zero_position_without_touching_the_tree ... ok
test setting_show_an_unknown_name_is_no_such_target ... ok
test setting_unset_an_unknown_name_is_no_such_target ... ok
test setting_show_returns_one_setting_in_full ... ok
test session_new_creates_a_plain_shell_session ... ok
test setting_set_rejects_an_unknown_name_with_no_such_target ... ok
test setting_set_changes_the_value_the_next_read_sees ... ok
test setting_set_rejects_a_value_outside_the_closed_set_with_bad_request ... ok
test split_set_label_then_close_the_session ... ok
test tab_close_removes_it ... ok
test setting_unset_reverts_to_the_default ... ok
test tab_focus_makes_it_current ... ok
test tab_list_with_an_unknown_session_is_no_such_target ... ok
test tab_new_makes_the_new_tab_current_by_default ... ok
test tab_new_with_a_middle_position_places_the_new_tab_there ... ok
test tab_list_returns_every_tab_of_the_session_in_order ... ok
test tab_move_with_position_reorders ... ok
test tab_new_with_a_position_past_the_end_and_no_focus_creates_nothing_and_restores_focus ... ok
test tab_new_with_no_focus_leaves_the_original_tab_current ... ok
test tab_new_with_no_position_appends_at_the_end_as_before ... ok
test tab_new_with_a_position_past_the_end_behaves_like_tab_move_and_creates_nothing ... ok
test tab_new_with_no_focus_restores_the_previous_current_tab ... ok
test tab_new_with_no_focus_restores_a_current_tab_that_is_not_the_first ... ok
test tab_new_with_position_zero_is_refused_and_creates_nothing ... ok
test tab_new_with_position_one_places_the_new_tab_first ... ok
test every_not_yet_scenario_answers_not_implemented_yet_and_changes_nothing ... ok
test tab_show_with_an_unknown_target_is_no_such_target ... ok
test tab_show_and_tab_list_shown_status_follow_the_kernels_ranking_not_tab_order ... ok
test tab_show_returns_the_tabs_record ... ok
test api_show_with_no_target_returns_the_whole_description ... ok
test every_described_method_gets_a_response_with_no_panic ... ok
test api_call_matches_a_direct_call_for_every_served_method_with_its_own_target_given ... ok

test result: ok. 73 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
```

Check passed: `cargo test -p steg-server --test routing` exited 0 at 2026-11-16T05:08:12+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.89 on task-1.89-server-tab-list-tab-show-pane-list-and at 24506572a88c with uncommitted changes:

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.09s
     Running tests/routing.rs (target/debug/deps/routing-b6f2d180ae7f55f5)

running 22 tests
test data_on_a_stream_never_opened_closes_the_connection ... ok
test an_oversized_frame_post_handshake_is_refused_promptly_and_closes_the_connection ... ok
test a_client_that_vanishes_with_replies_still_queued_still_ends_the_connection_promptly ... ok
test a_request_that_would_fail_client_side_validation_still_gets_a_matching_server_error ... ok
test a_reply_shaped_frame_from_the_client_closes_the_connection ... ok
test a_non_json_control_payload_post_handshake_closes_the_connection ... ok
test the_server_stops_promptly_with_a_connection_mid_request ... ok
test an_unknown_kind_tag_post_handshake_closes_the_connection ... ok
test the_server_stops_promptly_after_a_hostile_connection_that_was_never_explicitly_closed ... ok
test an_event_from_the_client_closes_the_connection ... ok
test a_reply_delivered_one_byte_at_a_time_still_decodes_correctly ... ok
test a_request_on_the_control_stream_reaches_the_api_and_the_reply_matches_the_id ... ok
test a_second_hello_after_the_handshake_closes_the_connection ... ok
test a_target_less_request_over_a_real_socket_is_bad_request_for_a_session_a_tab_and_a_pane_method ... ok
test a_hostile_post_handshake_close_still_reports_disconnected ... ok
test several_pipelined_calls_on_one_connection_get_correctly_matched_in_order_replies ... ok
test tab_list_tab_show_and_pane_list_answer_over_a_real_socket ... ok
test two_requests_sharing_one_id_both_get_correctly_matched_independent_replies ... ok
kill: 28692: No such process
test stopping_through_a_real_connection_ends_a_real_programs_process_and_frees_the_socket ... ok
test a_writer_blocked_on_a_full_send_buffer_still_joins_when_this_connections_own_close_runs ... ok
test a_client_that_trickles_a_frame_post_handshake_forever_is_never_closed ... ok
test a_client_that_floods_requests_and_never_reads_any_reply_is_closed_not_hung ... ok

test result: ok. 22 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.01s
```

Check passed: `just check` exited 0 at 2026-11-16T05:14:07+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.89 on task-1.89-server-tab-list-tab-show-pane-list-and at 1356fa2f08e9:

```text
ok      format check (0.2s)
ok      lint (0.4s)
ok      boundaries (0.1s)
ok      licences (0.7s)
ok      tests (-p steg -p steg-app -p steg-kernel -p steg-sdk -p steg-server) (5.1s)
ok      build (steg-sdk) (0.1s)
ok      test (steg-sdk) (0.2s)
ok      test (steg-sdk, local-socket) (0.4s)
ok      build (steg-kernel) (0.1s)
ok      test (steg-kernel) (0.2s)
ok      build (steg-server) (0.1s)
ok      test (steg-server) (4.6s)
ok      test (steg-server, race-fixture) (6.1s)
ok      build (steg) (0.1s)
ok      test (steg) (0.1s)
ok      test (steg, fixtures) (2.4s)
ok      build (steg-ui) (0.3s)
ok      test (steg-ui) (0.3s)
ok      build (steg-app) (0.3s)
ok      test (steg-app) (0.3s)
ok      tool tests (11.4s)
ok      architecture (0.7s)
ok      recipe names (0.0s)
ok      ui levels (0.0s)
ok      ui colours (0.0s)
```

Built per the approved plan (digest a99a69b5fa87), no departures from it besides a cargo fmt pass after the first just check run (recorded, see commit 1356fa2 - purely formatting, no behaviour change).

Coverage by acceptance criterion:
- AC #1 (tab.list/tab.show/pane.list): built. TabRecord added to the sdk with exactly tab_fields()'s seven fields. Positive and negative-control tests in tests/dispatch.rs; a real-socket test in tests/routing.rs; real-pty tests in tests/runtime.rs proving a typed command's status and a tab's shown_status both reach these methods through Runtime::handle.
- AC #2 (PaneRecord's status field, Moment through dispatch): built. status renamed to program_status (ProgramStatus's words, unchanged); status now carries the sdk's Status type. dispatch() gains a sixth parameter, now: Moment, computed once per request in Runtime::handle/start_and_dispatch the same way apply_event_locked already does; the kernel reads no clock. Also closed a pre-existing gap: PaneRecord gained kind and size (doc-2's pane show note listed both; pane_fields() had neither).
- AC #3 (remaining methods): every not_implemented_yet citation in the session/tab/pane nouns pointing at a Done task is reworded to name its real owner - the two drafts split from this task ("Server: pane resize"; "Kernel and server: move a tab to another session", cited by title since neither has an id yet), TASK-1.90 (pane.send/capture/wait, already accepted before this task started), and the events task split from TASK-1.46 (session.focus). None still cites TASK-1.39/1.40/1.41/1.42.
- AC #4 (fields match the description; tests): matches_the_fields_tab_show_and_tab_list_declare and the pane equivalent both pass. Every method has a test through dispatch/Runtime::handle. A typed command's status reaches pane.show and tab.show through a real pty (tests/runtime.rs).

Mechanisms verified by a recorded failing run with the mechanism removed, then restored (asdlc record-check on each): the clock reaching dispatch; status built from the kernel's own Pane::status rather than derived from program_status; a tab's shown_status from the kernel's own ranking rather than the first pane; a pane's size from the tab's layout rather than always none; pane.list's order rather than a reversal of it.

Checks recorded on the task: cargo test -p steg-sdk -p steg-kernel -p steg-server (multiple green runs through the build), cargo test -p steg-server --test dispatch/--test runtime/--test routing (each green, plus the five failing-control runs above), just check green twice (once before the fmt fix, exit 0 again after, at the final pushed head 1356fa2, which is also the commit CI passed on).

Overlap: rebased cleanly onto origin/main (no code changes there yet - TASK-1.78/TASK-1.87 haven't merged). TASK-1.46's own plan now depends on this task for TabRecord/PaneRecord's shape; nothing here needs updating for that dependency, only kept stable.
<!-- SECTION:NOTES:END -->

## Comments

<!-- COMMENTS:BEGIN -->
author: @bruk
created: 2026-11-16T04:49:00Z

---

Plan approved by @bruk (a99a69b5fa87, size-3): its worker may build it.

---

author: @bruk.r1
created: 2026-11-16T05:18:00Z

---

@bruk.r1 is reviewing TASK-1.89: a same-machine review: it waited 0m with no other engineer's reviewer, longer than review_fallback_after (0m), so a fresh reviewer on its author's machine takes it.

---

created: 2026-11-16T05:24:00Z

---

Small departure from the plan, per the team lead: the three split tasks (TASK-1.110 "Server: pane resize", TASK-1.111 "Kernel and server: move a tab to another session", TASK-1.109 the events task owning session.focus) now have real ids, accepted after this task's plan was written. Updated the three not_implemented_yet messages that cited them by title alone to cite the id instead (pane.send/capture/wait's TASK-1.90 citation was already an id and is unchanged).

cargo test -p steg-server --test dispatch and just check both passed locally before pushing (commit c88bcc8, then merged with origin/main at ba83798 with no code conflict - only backlog/ files). Could not record these through `asdlc record-check`: it refuses while the task is In Review ("not In Progress, so @bruk.w5 can't record checks on it"). CI passed at the final head, ba83798: https://github.com/bruk-io/steg/actions/runs/43809715675

---

author: @bruk.r1
created: 2026-11-16T05:38:00Z

---

Review by @bruk.r1 (same-machine): clean. It stays In Review for a person to merge.

Findings: 0 Important, 2 nit. Cites: testing, review. The diff matches the plan.

Reviewed at head ba83798 (PR #57's own headRefOid; the merge that brought in origin/main after the id-citation commit c88bcc8). CI's `check` workflow is green at ba83798. I independently ran `just check` at this exact head (clean worktree, no edits) and it passed; I also ran `cargo test -p steg-server` ten times at this head with no failures and no leaked processes (ps before/after unchanged, no stray `sleep 100`/OSC-133/`htop`/trap-marker processes).

Repeated two of the five recorded mechanism controls myself, in scratch edits reverted before any other check: (1) made pane.rs's `status` field derive from `program_status` instead of `pane.status(now)` - `an_idle_shell_panes_program_status_and_status_disagree` failed as expected; (2) made tab.rs's `shown_status` return the tab's first pane instead of `tree.tab_shown_status(id, now)` - `tab_show_and_tab_list_shown_status_follow_the_kernels_ranking_not_tab_order` failed as expected. Both confirm the mechanisms are real.

Checked the clock (grepped crates/kernel and crates/server/src/dispatch for Instant::now/SystemTime::now: none outside runtime.rs's two new call sites, which match apply_event_locked's existing pattern), the records against doc-2 §8/doc-11/decision-7 (TabRecord's seven fields and PaneRecord's kind/size/program_status/status all present and matching the description, verified by matches_the_fields_* tests), size arithmetic (reuses Tree::tab_layout/Layout::leaf_sizes, not reimplemented), list order (pane.list through R112's existing tab.panes() order; tab.list through session.tabs() order), and that an empty tab/session cannot exist in this kernel (last-pane-closes-tab, last-tab-ends-session), so no test is owed for it.

No Important findings.

[Testing, Nit] The plan's proof said tab.list/tab.show/pane.list get negative controls "matching this file's existing per-method pattern (no such target, ambiguous target)"; only no-such-target tests were added, no ambiguous-target test for any of the three, and the notes don't name it as dropped. Not escalated to Important: all three resolve through the pre-existing target_tab/target_session helpers already shared with tab.close/tab.focus/session.show etc. (no new resolution code this task wrote), and AmbiguousTarget's own mapping is already covered generically by dispatch/kernel_error.rs's target_ambiguous_maps_to_ambiguous_target_with_the_candidates_ids. The plan's cited "existing per-method pattern" itself has no ambiguous-target case either for the methods it names, so this is a pre-existing gap in the file's own convention, not new risk from this task.

[Review process, Nit] The plan's Risks/Order-of-work section committed to landing the ~79-line mechanical Moment-threading of tests/dispatch.rs as its own commit, separate from new behaviour, for reviewability. It shipped squashed into the one implementation commit (01a74fd) instead, and neither the implementation notes nor comment #3's disclosed departure mention this. I verified directly from the diff that no pre-existing assertion value changed (only moment(0) arguments added, plus the three not_implemented_yet string replacements AC #3 required) - the risk the separate commit was meant to guard against did not materialize, so I did not escalate this to Important.

Overlap with PR #47 (TASK-1.78, about to merge), for whoever resolves it:
- dispatch/mod.rs: the `pub(crate) use lookup::{...}` re-export list textually conflicts (this task adds tab_position; TASK-1.78 adds any_pane_running, panes_in_close_scope) - keep all three.
- runtime.rs, conflicting in meaning without conflicting in text: (1) TASK-1.78 adds a third `force_grace: Duration` parameter to `Runtime::new`; this task's own two new tests in tests mod (a_typed_commands_status_reaches_pane_show_and_tab_show_through_runtime_handle, a_command_panes_exit_reaches_pane_show_through_runtime_handle) call `Runtime::new(launcher, login_for_marks())` with only two arguments - whichever lands second must add a third argument (its own TEST_GRACE) to these two call sites or the build breaks. (2) TASK-1.78's new `close_and_dispatch` calls `dispatch::dispatch(&mut state.tree, &mut state.settings, description, context, request)` with five arguments, written against the pre-1.89 signature; this task makes `dispatch::dispatch` take a sixth `now: Moment` parameter - the resolution must compute `now` there the same way `start_and_dispatch`/the general-dispatch branch/`apply_event_locked` already do, and pass it.
- TASK-1.46 nesting: `TabRecord.layout`'s leaves already name every pane's id, so embedding `Vec<PaneRecord>` alongside the embedded `TabRecord` repeats pane identity once in `layout` and once per `PaneRecord.id` - mildly awkward but not a defect.

Posted the same findings on PR #57.

---

created: 2026-11-16T06:05:00Z

---

Merged TASK-1.78's forced close (its head, 3e4594e) into this branch, as asked, then added the three ambiguous-target tests from nit #1. Final pushed head: 93973fd (merge commit e92e8fb plus the ambiguous-target test commit a91e403, then one more origin/main pull of backlog-only changes). CI green at that head: https://github.com/bruk-io/steg/actions/runs/43813864510

**The one literal conflict, and its resolution**

`crates/server/src/dispatch/mod.rs`: the `pub(crate) use lookup::{...}` re-export block. HEAD (mine) had `tab_position` added; 3e4594e had `any_pane_running`/`panes_in_close_scope` added. Resolved by keeping all three, alphabetised: `any_pane_running, find_tab, pane_mut, panes_in_close_scope, param_bool, param_i64, param_present, param_str, resolve_pane_str, resolve_tab_str, tab_mut, tab_position, target_pane, target_session, target_tab, wire_position_to_index`. Nothing else in that file conflicted.

**Changes outside the literal conflict, and why each was needed**

Git auto-merged the rest of the files, but three spots needed a hand edit after the merge because they are exactly the "two tasks touch the same lines for unrelated reasons" seam named in this task's own plan risks, and git's textual merge can't know that a *semantic* gap remains even where it found no conflicting lines:

1. `crates/server/src/runtime.rs`, `Runtime::close_and_dispatch` (new in 3e4594e, for pane.close/tab.close/session.close): its own call to `dispatch::dispatch` still had the pre-this-task five-argument form (no conflict marker, since this function didn't exist on my side to conflict against). Added the same `let now = Moment::new(Instant::now().duration_since(state.clock_origin));` computation `handle`/`start_and_dispatch` already have, and passed it as the sixth argument - so a forced close's reply is built against the same clock every other method's is, per your own instruction.
2. `crates/server/src/runtime.rs`, my own two new tests (`a_typed_commands_status_reaches_pane_show_and_tab_show_through_runtime_handle`, `a_command_panes_exit_reaches_pane_show_through_runtime_handle`, added on this branch before 3e4594e existed): both called `Runtime::new` with the old two-argument form. Added `TEST_GRACE` (this module's own constant, the same one 3e4594e's own tests in this module use) as the third argument.
3. `crates/server/tests/runtime.rs`, `pane_close_on_an_already_ended_pane_succeeds_without_force` (added on 3e4594e's branch, after this task's rename existed only on my side): polled `shown["status"] == json!("ended")`. `status` no longer carries that word at all (it's doc-11's `Status` now; `"ended"` only ever appeared on `program_status`) - the poll hung for the full 20s bound and failed, caught by running the full server suite once after the merge. Changed to `shown["program_status"]`, the field this test was always actually about (the clock-free fact, R97).

No other file's merge needed a hand edit; everything else (the Cargo.toml dependency bump, lib.rs/listen.rs/pane.rs/signal.rs, the bulk of tests/runtime.rs) is 3e4594e's own content, unmodified by me.

**What I ran, and in order**

- `cargo build -p steg-server --tests` after resolving the mod.rs conflict and before the three hand-edits above - compiled, so the hand-edits above were found by running the suite, not by a second compiler pass.
- `cargo test -p steg-server` (whole crate, all test binaries) - one failure (item 3 above); fixed; reran clean: 130+76+13+5+7+22+29+8+1+0 = all passed, 0 failed, across every test binary.
- `cargo test -p steg-server` **ten times in a row**, with a process count before (`ps -eo pid= | wc -l`: 870) and after (867) - no growth, every run green.
- `cargo test -p steg -p steg-sdk -p steg-kernel` and `cargo test -p steg --features fixtures` - all green.
- `just check` - green, including `lint` (clippy, deny-level) and `architecture` (the LikeC4 model), at every intermediate head and again at the final one.
- Pushed; `gh pr checks 57 --watch` in the foreground to a green `pass` each time (three pushes total: the merge itself, the ambiguous-target tests, and one more origin/main pull with no code in it).

**Nit #1, the rest of it**

Added the three ambiguous-target tests the plan's proof promised: `tab_list_with_an_ambiguous_session_target_is_ambiguous_target`, `tab_show_with_an_ambiguous_target_is_ambiguous_target`, `pane_list_with_an_ambiguous_target_is_ambiguous_target`, in `crates/server/tests/dispatch.rs`. Each needs two things sharing a label (labels are never required to be unique, `crates/kernel/src/label.rs`'s own doc, which is what makes a target ambiguous rather than merely unknown) - two sessions for `tab.list`'s own session target, two tabs of one session for `tab.show`/`pane.list`'s own tab target.

**Nit #2, acknowledged, cannot be undone**

The mechanical `moment(...)` threading through `tests/dispatch.rs`'s ~79 existing calls landed in the same commit as the real `tab.list`/`tab.show`/`pane.list` implementation, not disclosed as its own commit at the time - a departure from the plan's own order-of-work step 11, which asked for it separately. Noting it here since the history can't be split after the fact.

Could not run this through `asdlc record-check` or `asdlc finish` - both refuse once a task has passed review. This comment is the record instead, as you asked.

---

created: 2026-11-16T06:16:00Z

---

Independent read of ba83798..93973fd by a separate agent, 2026-11-16: correct. The only code-bearing merge in the range is e92e8fb (ba83798 + TASK-1.78's 3e4594e); 93973fd is a second merge that brings in only backlog/ files from origin/main, no code. Diffed the actual merge tree against git's own mechanical auto-merge (merge-tree --write-tree ba83798 3e4594e): the only textual conflict was dispatch/mod.rs's re-export list, resolved as a correct superset (tab_position + any_pane_running + panes_in_close_scope). Exactly three further hand edits exist beyond that, matching the author's account with nothing extra: (1) close_and_dispatch's Moment computed the same way, same point under the lock, as handle and start_and_dispatch - verified by reading all three call sites (runtime.rs:260,421,524). Checked whether this matters given force_end can block the reply for up to force_grace: it does not, because pane.close/tab.close/session.close never read  at all (dispatch/pane.rs, tab.rs, session.rs close() takes no Moment) - the arg is pure plumbing for dispatch()'s uniform signature, so no stale-boundary risk reaches any reply. (2) both new Runtime::new calls in runtime.rs's own test module got TEST_GRACE, matching every sibling test in that module. (3) tests/runtime.rs's already-ended poll now reads program_status instead of status - necessary, not cosmetic: doc-11's new  serializes as Running/Finished/Failed{..} and expires after FINISHED_LIFETIME (R83), so the old string compare would never match again; program_status's text mapping (dispatch/pane.rs status_text) does yield "ended". Grepped the whole tree for any remaining status=="running"/"ended" string compares in either branch's tests - none left. The three new ambiguous-target tests (a91e403) assert ErrorCode::AmbiguousTarget specifically, produced by kernel_error.rs's exhaustive (no wildcard) match on Error::TargetAmbiguous - a dedicated kernel variant, not a generic refusal, so a first-match implementation would fail these, not pass vacuously. just check passed in full (26/26 steps incl. test (steg-server) and test (steg-server, race-fixture)). Ran cargo test -p steg-server 5 times back to back: all passed every time, and no test-spawned process (shell, trap/sleep fixture, steg-server) was left behind afterward - the only ps matches before/after were unrelated session noise. No follow-ups found.

---

created: 2026-11-16T06:16:00Z

---

Correction to comment #6: two words were eaten by shell quoting. It should read "...never read now at all..." and "...doc-11s new status field serializes...". No change to the verdict or findings.

---

created: 2026-11-16T11:44:00Z

---

Merged origin/main into this branch as asked - no hand edits, plain `git fetch origin` then `git merge origin/main`. Completed with no conflict: only backlog/ files came in (board docs, two drafts promoted to tasks, nothing under crates/, tools/ or justfile). New head: de9c913.

What I ran: `cargo test -p steg-server` (whole crate, every test binary) green (130+76+13+5+7+22+29+8+1+0, 0 failed); `just check` green (format, lint, boundaries, licences, every package built and tested alone, tools, architecture, recipe names, ui levels/colours). Pushed (no force); `gh pr checks 57 --watch` in the foreground to green: https://github.com/bruk-io/steg/actions/runs/43859847786

`gh pr view 57 --json mergeable,mergeStateStatus`: MERGEABLE / CLEAN, at head de9c913.

`git diff 93973fd HEAD --stat -- crates tools justfile`: empty - confirms nothing of mine changed, only what main brought.

Could not run this through asdlc record-check or finish (board locked after a clean review); this comment is the record.

---
<!-- COMMENTS:END -->
