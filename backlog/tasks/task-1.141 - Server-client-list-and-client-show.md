---
id: TASK-1.141
title: 'Server: client list and client show'
status: In Progress
assignee:
  - '@bruk.w5'
created_date: '2026-11-18T14:10:00Z'
updated_date: '2026-11-18T22:47:00Z'
labels:
  - size-2
milestone: m-3
dependencies: []
parent_task_id: TASK-1
project: bruk-io-steg
asdlc_accepted_from: DRAFT-5
asdlc_branch: task-1.141-server-client-list-and-client-show
asdlc_plan_approved_by: bruk
asdlc_plan_approved_hash: '48ee00882225'
asdlc_plan_approved_size: size-2
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Split from TASK-1.88 by its plan on 2026-11-18 (its fifth criterion). doc-2 section 8; decision-5 for clients. TASK-1.88's plan holds the worked design as a starting point: the descriptions for both methods already exist in crates/sdk/src/description/methods/client.rs, so nothing changes there; both become plain dispatch arms in a new crates/server/src/dispatch/client.rs, with a new Tree::resolve_client in the kernel. Attach has landed (TASK-1.47), so a client does view sessions today. The plan recommended leaving last_active empty for M1, as pane and tab titles are, because tracking it needs interception in the runtime and a time formatter the workspace does not have. It can be built beside the server half: they share only additive edits to dispatch/mod.rs's lists and the record and outcome variant lists.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 client list and client show answer over the API from the kernel's client model and what the server records for each connection: what each client is and the sessions it views; command line clients only with --all, as doc-2 section 8 gives
- [ ] #2 A client is found by its target the way a session is, through a resolver in the kernel that mirrors the session one; an unknown or ambiguous target answers with the codes doc-2 gives
- [ ] #3 What last_active reports is settled with the engineer in the plan: empty until it is tracked, or tracked now with what that costs
- [ ] #4 Both methods leave the not-yet list in dispatch and the completeness test covers them; they are proved without a socket the way the session and tab arms are, each with a recorded failing control
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
## Open questions for the engineer

1. **AC #3 - `last_active`.** Recommended default: `None` always, for M1 - the exact same shape `PaneRecord::title`/`TabRecord::title` already use ("declared because a later task wires it through, not fabricated here"). Cost of tracking it now instead: it needs interception at every point a client's connection does something (every request, not just input - the activity concept doc-15/decision-6 ties to `Client::activity`/`record_activity` is keyed by *session*, for `latest_client`, and is bumped only by viewing/declaring/attach-related calls today, not by every API call a client makes), plus a UTC ISO-8601 time formatter this workspace does not have yet (no `chrono`/`time` dependency anywhere in `crates/sdk` or `crates/server` today - checked). Building it now is a new dependency, a new interception point in `connection.rs`'s own request-handling loop (which this task's `dispatch` arms, being plain functions over `&Tree`, cannot reach any more than `Context` can - the same structural reason `tab.declare-size` is intercepted, not dispatched), and a decision about what counts as "active" (any request? only input? only a session view changing?) nobody has made. This plan builds the `None` default only, and does not touch `connection.rs` or add a time dependency, unless the engineer asks for tracking after reading this.
2. **Whether the calling connection appears in `client list`.** Settled by inspection, not by new code: `connection.rs`'s handshake calls `Runtime::connect_client` (`runtime/lifecycle.rs:21`) before the connection's own request loop ever reaches `dispatch` (confirmed by reading `connection.rs:830` and the hello sequence), so the calling client's own `Client` entry is already in `tree.clients()` by the time its own `client.list` request runs - same as any other connected client, with no self-exclusion anywhere in the kernel or doc-2. What decides whether it is *shown* is the same `--all`/kind filter every other client gets - doc-2 section 8's row: "Every client connected to the server. Command line clients only with `--all`"; section 5's `--all` row: "...or command line clients as well". **Named explicitly, as an intended consequence, not left implicit:** the overwhelmingly common caller of `client list` is itself a short-lived CLI invocation (`steg client list` from a terminal), and under this filter it does not see itself in the default output, same as it would not see any other bare CLI client - only `--all` shows it. The app's own client, or a plugin's, does see itself, same as always, because neither is filtered. No code change beyond the kind filter below is needed to answer this; stated here so review does not mistake the omission for a bug.

## Files that change

- `crates/kernel/src/target.rs`: add `use crate::client::Client;` and `ClientId` to the `steg_sdk` import; add `Tree::resolve_client(&self, part: &str) -> Result<ClientId, Error>`, in the `impl Tree` block beside `resolve_session`, built the same way (`to_result(part, resolve(part, self.clients(), Client::id, Client::label))` - the same generic `resolve`/`to_result` `resolve_session` already calls, no new matching logic). New tests in its own `#[cfg(test)] mod tests`: resolves by full id, by label (ambiguous case included, mirroring `tree_resolve_session_resolves_by_label_and_errors_when_ambiguous`), by position, by a four-plus-character id suffix, and a not-found case - built over `tree.connect_client(ClientKind::Cli/App, label)` (already `pub`, used elsewhere in this crate's own tests).
- `crates/sdk/src/record.rs`: add `ClientRecord { id: ClientId, kind: ClientKind, label: Option<String>, sessions: Vec<SessionId>, last_active: Option<String> }`, `#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]`, doc comment per field (same shape as `SessionRecord`/`PaneRecord`/`TabRecord` above it), `last_active`'s own doc naming it always `None` for M1 per question 1 above - same wording `TabRecord::title`'s doc already uses. New tests in the existing `#[cfg(test)] mod tests`: `client_record_serde_round_trips`, `a_client_with_no_label_or_last_active_serialises_them_as_null_not_missing`, and `matches_the_fields_client_list_and_client_show_declare` (same shape as `matches_the_fields_session_show_and_session_list_declare`, reused for both `client.list` and `client.show` since they declare the same `fields`).
- `crates/sdk/src/lib.rs`: add `ClientRecord` to the `pub use record::{...}` list (alphabetical: before `PaneRecord`).
- `crates/server/src/dispatch/client.rs` (new file): the `client` noun's dispatch arms, mirroring `session.rs`'s shape.
  - `pub(super) const HANDLED: &[(&str, &str)] = &[("client", "list"), ("client", "show")];`
  - `pub(super) fn dispatch(tree: &Tree, request: &Request, verb: &str) -> Result<Outcome, ApiError>` - takes `&Tree`, not `&mut Tree`: neither verb mutates anything, and the signature should say so. Checked, not assumed: `dispatch_raw`'s `tree` parameter is `&mut Tree`, and passing a `&mut Tree`-typed expression where a function expects `&Tree` is an ordinary reborrow Rust performs automatically (the same way `fn foo(x: &i32)` accepts a `y: &mut i32` passed as `foo(y)`) - no cast needed at the call site. If step 4's edit to `mod.rs` fails to compile on this point, the fallback is an explicit `client::dispatch(&*tree, request, verb)` at the call site, not changing this function's own signature.
  - `fn record_for(tree: &Tree, client: &steg_kernel::Client) -> ClientRecord` - shared by `list`/`show`, same role `tab.rs`'s `record_for`/`session.rs`'s `record_at` play: `sessions` from `tree.sessions_viewed(client.id())`, `.expect("this client is in the tree right now")` (same style as `tab.rs`'s `record_for`'s own two `.expect`s over a tab proven in-tree by the caller) since every client this function is ever called with comes straight out of `tree.clients()` or a just-resolved id; `last_active: None` always (question 1).
  - `fn list(tree: &Tree, request: &Request) -> Outcome` - `let all = param_bool(request, "all").unwrap_or(false);` then `tree.clients().iter().filter(|c| all || c.kind() != steg_sdk::ClientKind::Cli).map(|c| record_for(tree, c)).collect()`, wrapped as `Outcome::ClientRecords`.
  - `fn show(tree: &Tree, request: &Request) -> Result<Outcome, ApiError>` - `target` read with `param_str(request, "target").expect("validate already confirmed client.show's target is present - it is required")` (per the `description::request::validate` check above, confirmed read: a required param missing is refused before `dispatch_raw`'s match runs, so this arm is never reached with it absent), `tree.resolve_client(part).map_err(map_kernel_error)?`, then `tree.client(id).expect("resolve_client just confirmed this id exists")`, wrapped as `Outcome::ClientRecord`.
  - No `Context` parameter anywhere in this file: `client.list` takes no target at all, and `client.show`'s target is `required: true` (confirmed in `crates/sdk/src/description/methods/tests.rs`'s own `doc_2_section_8_targets`, already `Some((true, ValueKind::Target))`) with no context fallback possible (decision-5: "There is no `STEG_CLIENT`") - `Context` would be an unused parameter, which is itself a wrong statement about what this file needs (`.claude/rules/rust.md`).
- `crates/server/src/dispatch/mod.rs`: add `mod client;` to the `mod` list (alphabetically after `api`, before `event`); in `handled()`, add `all.extend(client::HANDLED.iter().copied());`; remove `("client", "list"), ("client", "show"),` from `NOT_YET_NOUNS`; in `dispatch_raw`'s match, add `(noun, verb) if client::HANDLED.contains(&(noun, verb)) => client::dispatch(tree, request, verb),` beside the other per-noun guards, and delete the `("client", _) => Err(not_implemented_yet("a client model (TASK-1.42)"))` arm entirely (no client method is left not-yet). These are line-level additions/removals inside existing arrays and match arms that TASK-1.88 is editing concurrently for the `server` noun's own, separate entries - confirmed by reading the current file: the `client` and `server` lines are distinct elements in `NOT_YET_NOUNS` and distinct match arms, so this is the "additive beside it" case the brief named, not a collision; still, if TASK-1.88 merges to `origin/main` first, this task's branch needs a routine rebase/merge of this one file (expected clean - no step here depends on TASK-1.88's own code existing).
- `crates/server/src/dispatch/outcome.rs`: add `ClientRecord` to the `use steg_sdk::{...}` import; add two variants, `ClientRecord(steg_sdk::ClientRecord)` and `ClientRecords(Vec<steg_sdk::ClientRecord>)`, with `into_value` arms identical in shape to `SessionRecord`/`SessionRecords`'s. New tests: `client_record_example()` helper plus `a_client_record_serialises_to_its_own_typed_shape` and `a_vec_of_client_records_serialises_to_its_own_typed_shape`, mirroring the existing pane/tab pair exactly. Same "additive beside TASK-1.88's own new `ServerRecord` variant" note as above.
- `crates/server/tests/dispatch/client.rs` (new file): functional tests over `dispatch` through the public API only (no doubles), mirroring `tab.rs`/`session.rs`'s own shape. `fresh()` connects no clients at all, so every case here connects its own with `tree.connect_client(...)` directly, the same "direct kernel setup" pattern `cross_cutting.rs`'s `pane.send` case already uses for a different method.
- `crates/server/tests/dispatch/main.rs`: add `mod client;` to the `mod` list (alphabetically after `api`, before `cross_cutting`).
- `crates/server/tests/dispatch/cross_cutting.rs`: two edits, required for `every_not_yet_scenario_answers_not_implemented_yet_and_changes_nothing` to still pass once step 4 lands (without this file, that test asserts `ErrorCode::NotImplementedYet` for `client.list`/`client.show`, which will instead return `Ok` records - a red test, not merely a stale one).
  - Delete the `("client.list", vec![], "client model")` and `("client.show", vec![("target", json!("placeholder"))], "client model")` entries from the `cases` vec (lines 94-99 today).
  - The doc comment above the test names "this task's 19 `NotImplementedYet` scenarios" - update the count (or drop the specific number) to match `cases`' new length.
  - Confirmed, not changed: the second test's own `not_yet: &[&str]` array (`"session.focus"`, `"pane.resize"`, `"pane.detach"`, `"tab.declare-size"`, `"server.start"`, `"server.stop"`) names neither `client.list` nor `client.show` already - correct, since both are `Returns::Records`/`Returns::Record`, not `Nothing`/`Identifier`, so that test's own `filter` already skips them and needs no edit here.
  - Same "additive beside TASK-1.88" shape as `mod.rs`/`outcome.rs` above: TASK-1.88's own plan removes the three `server.*` entries from this same `cases` vec and (per its own text) the doc comment's count - a different subset of the same array, not the same lines.

## Order of work

1. Kernel: `Tree::resolve_client` in `target.rs`, with its own tests, run `just test` for `steg-kernel` alone before touching anything else.
2. sdk: `ClientRecord` in `record.rs`, its own tests, the `lib.rs` export - run the sdk's tests before touching the server crate.
3. Server, outcome: the two new `Outcome` variants and their tests in `outcome.rs`.
4. Server, dispatch: the new `dispatch/client.rs` file, then `mod.rs`'s five small edits (the `mod` line, `handled()`, `NOT_YET_NOUNS`, the new guard arm, deleting the old catch-all) - get this compiling and the existing `every_described_method_has_exactly_one_handler`/`not_implemented_yet_names_what_it_waits_on` tests green before writing new functional tests.
5. The new `crates/server/tests/dispatch/client.rs`, the `main.rs` one-line addition, and `cross_cutting.rs`'s two edits. Cases, each with a failing-then-passing control run **at the final head** - every file in this plan already landed, `just check` green - not earlier in the sequence: temporarily re-insert the removed `("client", _) => Err(not_implemented_yet("a client model (TASK-1.42)"))` arm in `mod.rs` (or, for the `--all` cases, temporarily change `dispatch/client.rs`'s `list` filter to `all` alone), re-run the new test, confirm it fails with the expected mechanism (a `NotImplementedYet` error, or the wrong clients coming back), record that failure, then revert the temporary edit and re-run to confirm it passes - recorded both times, per AC #4's own wording ("each with a recorded failing control") and `.claude/rules/testing-rust.md`'s "a test whose mechanism is removed must actually fail" convention this crate's own sibling tasks (TASK-1.88's plan) already use:
   - `client_list_returns_every_connected_client_by_default_except_cli` - connect one of each kind (`App`, `Cli`, `Plugin`), call `client.list` with no `all`, assert exactly the app and plugin ones come back (by id), in `tree.clients()`'s own order.
   - `client_list_with_all_includes_cli_clients_too` - same setup, `all: true`, assert all three come back.
   - `client_list_matches_exactly_the_fields_the_description_declares` - same `declared_fields`/`assert_same_set` shape `session.rs`'s own test uses, re-implemented locally per this crate's "no shared test-support module across noun files" convention.
   - `client_show_resolves_by_id_label_position_and_suffix` - one client with a label, `client.show` by each of the four kinds in turn, same shape as a hypothetical `session.show` by-label test (there is no existing one in `session.rs` today since session fixtures only ever add one at a time - this is the first functional-level by-label/position/suffix proof for any noun in this test binary, so it is written out fully rather than copied).
   - `client_show_reports_the_sessions_it_views` - connect a client, `Tree::view_pane` it onto the fixture's own pane (already in a session), assert `sessions` names that session's id and no other.
   - `client_show_an_unknown_target_is_no_such_target_and_an_ambiguous_one_is_ambiguous_target` - two unlabelled clients (so a bad guess never collides with a real id) plus two same-labelled clients, asserting `ErrorCode::NoSuchTarget`/`ErrorCode::AmbiguousTarget` respectively - same codes `lookup.rs`'s own `a_wrong_but_fully_explicit_target_from_context_none_is_a_real_not_found`-style tests already check for other nouns, now for `client`.
   - `client_list_and_show_change_nothing_in_the_tree` - a before/after snapshot of `tree.clients().to_vec()` around both calls, the same "a read never mutates" proof `every_not_yet_scenario_answers_not_implemented_yet_and_changes_nothing` already gives other not-yet methods, now exercised as a real passing case instead.
6. Run `just check` (or the affected subset `tools/` selects from the diff) and record it; re-run once more at the end.

## Risks

- **The merge with TASK-1.88.** Named above, file by file: every edit this plan makes to `dispatch/mod.rs`, `outcome.rs`, and the two `NotYetCase`/`not_yet` lists in `cross_cutting.rs` touches a different array element or a different match arm than TASK-1.88's own plan touches for `server`, confirmed by reading the current file rather than assumed - low risk of an actual textual conflict, but real risk of two branches racing to be "first to remove the last not-yet entry" if either plan's own line numbers drift before the other merges. Mitigated by nothing clever: whichever lands second does an ordinary `git merge`/rebase of these few files, expected clean, and re-runs the two completeness tests.
- **`Client::label`/`Client::id` as `resolve`'s `id_of`/`label_of` callbacks.** `resolve`'s generic signature expects `id_of: impl Fn(&T) -> Id` returning something `Display` - `Client::id` returns `ClientId` by value (already `Copy`, like `Session::id`/`Tab::id`/`Pane::id`), so this is the exact same shape `resolve_session`/`Session::resolve_tab`/`Tab::resolve_pane` already use; low risk, proven by the existing three call sites compiling with the same generic function.
- **`record_for`'s two `.expect`s.** Both are reached only with a `ClientId` the caller already found in `tree.clients()` (`list`) or just resolved against the same tree with no mutation in between (`show`) - no `&mut Tree` call sits between resolving and reading in either path, so neither can observe a client that has since disconnected. Same reasoning `tab.rs`'s own two `.expect`s over `tab_layout`/`tab_shown_status` already rest on, not a new pattern.
- **Scope creep into `last_active`.** Question 1 above exists specifically to keep this plan from quietly also adding a time dependency or a new interception point; the risk is a reviewer (or a later task) treating the `None` default as an oversight rather than a decision - mitigated by `ClientRecord::last_active`'s own doc comment stating the default and naming what tracking it would cost, inline, not only in this plan.

## Proof

- AC #1 (list/show answer from the kernel's client model and what the server records; CLI clients only with `--all`): `client_list_returns_every_connected_client_by_default_except_cli` and `client_list_with_all_includes_cli_clients_too`, plus `client_list_matches_exactly_the_fields_the_description_declares` and `client_show_reports_the_sessions_it_views`.
- AC #2 (found by target the way a session is; unknown/ambiguous answer with doc-2's codes): `target.rs`'s own new `Tree::resolve_client` tests (id/label/position/suffix/ambiguous/not-found at the kernel level) plus `client_show_resolves_by_id_label_position_and_suffix` and `client_show_an_unknown_target_is_no_such_target_and_an_ambiguous_one_is_ambiguous_target` at the dispatch level.
- AC #3 (`last_active` settled): question 1 above, answered in the plan with a recommended default and its cost, and `ClientRecord::last_active`'s own doc comment carrying the same answer into the code.
- AC #4 (not-yet list/completeness test cover both methods; proved without a socket, each with a recorded failing control): `mod.rs`'s existing `every_described_method_has_exactly_one_handler` test (unchanged, now passing with `client` fully covered instead of falling into `NOT_YET_NOUNS`) and `cross_cutting.rs`'s existing `every_not_yet_scenario_answers_not_implemented_yet_and_changes_nothing` (with the two `client.*` cases removed from its list, since they no longer apply) plus `client_list_and_show_change_nothing_in_the_tree`; the failing control is step 5's own instruction - each new functional test run once at the final head with its own mechanism temporarily removed (fails, recorded), then with the removal reverted (passes, recorded) - not a control run earlier in the sequence against a still-incomplete tree.
- `just check` at step 4, step 5, and once more at the end, each recorded with `asdlc record-check`.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Plan checked against doc-2 (section 8, client rows; section 5 --all) and decision-5 by @bruk.o: ok. The methods, the --all filter for command line clients, and the target errors match the spec; no description, error code or wire change. Its edits beside TASK-1.88 are on different list entries and match arms; whichever lands second rebases. For the engineer: last_active empty for M1 (recommended), and the consequence that a plain 'steg client list' does not list itself without --all.

Check passed: `just check` exited 0 at 2026-11-18T22:45:44+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.141 on task-1.141-server-client-list-and-client-show at 18da1c97edb1:

```text
ok      format check (0.3s)
ok      lint (87.4s)
ok      boundaries (0.1s)
ok      foundations (0.2s)
ok      foundations public surface (0.1s)
ok      design pages (0.1s)
ok      licences (0.7s)
skip    tests: this change touches no crate
ok      build (steg-sdk) (3.4s)
ok      test (steg-sdk) (2.3s)
ok      test (steg-sdk, local-socket) (2.7s)
ok      build (steg-kernel) (43.8s)
ok      test (steg-kernel) (1.4s)
ok      build (steg-server) (42.9s)
ok      test (steg-server) (61.6s)
ok      test (steg-server, race-fixture) (92.6s)
ok      test (steg-server, pty-race-fixture) (74.5s)
wait    test (steg-server, accept-backoff-fixture) (2 of 2 slots busy, STEG_CHECK_SLOTS=2) - waiting for a slot to free with no bound (STEG_CHECK_SLOT_WAIT_S is unset); this step's own STEG_QUIET_TIMEOUT_S bound has not started yet
ok      test (steg-server, accept-backoff-fixture) (61.4s)
ok      build (steg) (2.5s)
ok      test (steg) (3.2s)
ok      test (steg, fixtures) (7.6s)
ok      build (steg-ui) (28.3s)
ok      test (steg-ui) (1.3s)
ok      build (steg-app) (2.6s)
ok      test (steg-app) (2.1s)
ok      tool tests (40.7s)
ok      architecture (0.8s)
ok      recipe names (0.1s)
ok      ui levels (0.1s)
ok      ui colours (0.1s)
```

Check passed: `sh -c 'cd crates/kernel && cargo test --lib target:: 2>&1 | tail -60'` exited 0 at 2026-11-18T22:46:58+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.141 on task-1.141-server-client-list-and-client-show at 18da1c97edb1 with uncommitted changes:

```text
   Compiling ghostty-vt-sys v0.8.0+ghostty.1.4.0-main.83fb956
   Compiling ghostty-vt v0.8.0+ghostty.1.4.0-main.83fb956 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.141/third_party/ghostty-vt-patched)
   Compiling steg-kernel v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.141/crates/kernel)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 32.18s
     Running unittests src/lib.rs (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.141/target/debug/deps/steg_kernel-e3b407e8fed5e9f0)

running 23 tests
test target::tests::split_path_rejects_an_empty_segment ... ok
test target::tests::resolve_matches_a_full_identifier_before_any_other_kind ... ok
test target::tests::tree_resolve_client_resolves_by_full_id ... ok
test target::tests::session_resolve_tab_and_tab_resolve_pane_use_the_same_order ... ok
test target::tests::resolve_matches_an_identifier_suffix_of_four_or_more_characters ... ok
test target::tests::split_path_rejects_a_path_with_more_parts_than_the_maximum ... ok
test target::tests::resolve_matches_a_label_case_insensitively_when_no_identifier_matches ... ok
test target::tests::resolve_pane_path_rejects_a_path_with_the_wrong_number_of_segments ... ok
test target::tests::resolve_never_tries_a_suffix_shorter_than_four_characters ... ok
test target::tests::tree_resolve_client_resolves_by_one_based_position ... ok
test target::tests::resolve_tab_path_uses_an_explicit_session_part_over_the_default ... ok
test target::tests::resolve_matches_by_one_based_position_when_no_identifier_or_label_matches ... ok
test target::tests::split_path_accepts_up_to_the_maximum_number_of_parts ... ok
test target::tests::tree_resolve_client_resolves_by_label_ignoring_case ... ok
test target::tests::resolve_stops_at_label_even_when_an_ambiguous_label_match_would_also_match_by_suffix ... ok
test target::tests::resolve_tab_path_falls_back_to_the_default_session_when_the_session_part_is_left_out ... ok
test target::tests::resolve_reports_ambiguous_with_only_the_matched_candidates_when_more_than_one_matches ... ok
test target::tests::tree_resolve_client_resolves_by_identifier_suffix ... ok
test target::tests::tree_resolve_session_resolves_by_label_and_errors_when_ambiguous ... ok
test target::tests::resolve_reports_not_found_with_every_candidate_when_nothing_matches ... ok
test target::tests::resolve_reports_ambiguous_when_the_suffix_kind_matches_more_than_one ... ok
test target::tests::tree_resolve_client_errors_when_ambiguous_and_when_not_found ... ok
test target::tests::resolve_pane_path_falls_back_to_the_given_defaults_when_parts_are_left_out ... ok

test result: ok. 23 passed; 0 failed; 0 ignored; 0 measured; 307 filtered out; finished in 0.00s
```

Check passed: `sh -c 'cd crates/sdk && cargo test 2>&1 | tail -80'` exited 0 at 2026-11-18T22:47:52+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.141 on task-1.141-server-client-list-and-client-show at 18da1c97edb1 with uncommitted changes, last 40 of 79 lines:

```text
test screen::tests::screen_update_full_round_trips ... ok
test wire::frame::tests::an_oversized_declared_length_is_refused_before_anything_past_it_is_read ... ok
test wire::frame::tests::an_unknown_kind_tag_is_malformed ... ok
test wire::frame::tests::decode_waits_for_more_bytes_a_byte_at_a_time ... ok
test wire::frame::tests::encode_refuses_a_payload_over_the_cap ... ok
test wire::frame::tests::frame_constructors_set_the_expected_kind_and_stream ... ok
test wire::frame::tests::two_frames_back_to_back_decode_in_order ... ok
test wire::handshake::tests::a_first_frame_that_is_not_json_is_malformed ... ok
test wire::handshake::tests::a_client_newer_than_the_server_is_refused_naming_both_versions ... ok
test wire::handshake::tests::a_matching_version_is_accepted ... ok
test wire::handshake::tests::a_missing_required_field_is_malformed ... ok
test wire::handshake::tests::a_server_newer_than_the_client_is_refused_naming_both_versions ... ok
test wire::handshake::tests::a_wrong_magic_is_malformed_naming_the_server_version ... ok
test wire::handshake::tests::an_unknown_field_is_ignored ... ok
test wire::handshake::tests::kind_and_label_round_trip_through_the_hello ... ok
test wire::handshake::tests::parse_server_hello_reads_an_accepted_hello ... ok
test screen::tests::pane_attached_round_trips_and_carries_both_fields_in_one_object ... ok
test wire::handshake::tests::parse_server_hello_surfaces_a_refusals_message ... ok

test result: ok. 176 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests/connection.rs (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.141/target/debug/deps/connection-51dcfc64c8e8cbf8)

running 2 tests
test a_version_mismatch_is_refused_naming_both_versions ... ok
test a_client_and_server_connection_shake_hands_and_exchange_a_call_in_memory ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests/local_socket.rs (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.141/target/debug/deps/local_socket-0e096e596b4f4ad1)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests steg_sdk

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```
<!-- SECTION:NOTES:END -->

## Comments

<!-- COMMENTS:BEGIN -->
author: @bruk
created: 2026-11-18T22:35:00Z

---

Plan approved by @bruk (48ee00882225, size-2): its worker may build it.

---
<!-- COMMENTS:END -->
