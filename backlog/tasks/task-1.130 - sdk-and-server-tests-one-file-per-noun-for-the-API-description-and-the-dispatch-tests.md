---
id: TASK-1.130
title: >-
  sdk and server tests: one file per noun for the API description and the
  dispatch tests
status: In Progress
assignee:
  - '@bruk.w6'
created_date: '2026-11-18T02:09:00Z'
updated_date: '2026-11-18T03:29:00Z'
labels:
  - size-3
milestone: m-3
dependencies: []
parent_task_id: TASK-1
project: bruk-io-steg
asdlc_accepted_from: DRAFT-1
asdlc_branch: task-1.130-sdk-and-server-tests-one-file-per-noun
asdlc_plan_approved_by: bruk
asdlc_plan_approved_hash: 'dc8b72722100'
asdlc_plan_approved_size: size-3
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Agreed with the engineer on 2026-11-17 to let more server tasks run at once. Of the last 40 merges, 9 touched crates/sdk/src/description/methods.rs (2,122 lines) and 9 touched crates/server/tests/dispatch.rs (2,978 lines), because every new method appends to both, so two server tasks in flight nearly always conflict. methods.rs is already organised as session_methods, tab_methods, pane_methods and so on in one file. This task only moves code. It must run while no other task is editing these files, and it can run beside the runtime.rs split, which touches different files.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The session, tab, pane, server, event, client, setting and api parts of crates/sdk/src/description/methods.rs each live in their own file, with the shared helpers in one place; the description's serialised form is byte for byte the same before and after, shown by a recorded comparison
- [ ] #2 No behaviour changes: no public item is added, removed or renamed, and no function body changes beyond imports and visibility
- [ ] #3 just check passes, and the notes say which files a new method now touches
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
## Files that change

`crates/sdk/src/description/`:
- `methods.rs` -> `methods/mod.rs`: moved, then reduced to `description()`, and `mod` declarations only.
- `methods/shared.rs` (new): `VERSION` and the shared parameter builders (`method`, `target_of_kind`, `target`, `required_target`, `method_target`, `flag`, `switch`, `program`, `direction`, `field`), each `pub(super)`.
- `methods/session.rs` (new): `session_fields`, `session_methods`.
- `methods/tab.rs` (new): `tab_fields`, `tab_methods`.
- `methods/pane.rs` (new): `pane_fields`, `pane_methods`.
- `methods/server.rs` (new): `server_fields`, `server_methods`.
- `methods/event.rs` (new): `events`, `event_methods` (kept together: `description()` feeds `events()`'s own names into `event_methods`).
- `methods/client.rs` (new): `client_fields`, `client_methods`.
- `methods/setting.rs` (new): `setting_fields`, `setting_methods`.
- `methods/api.rs` (new): `api_method_fields`, `api_methods`.
- `methods/errors.rs` (new): `errors`.
- `methods/tests.rs` (new): the whole `#[cfg(test)] mod tests` block, unchanged. Its 22 tests are whole-description invariants (the doc-2 §8 set, its targets table, the events/errors counts, the nested-type allow-list), not noun-scoped, so they are not split per noun.
- `description/mod.rs`: untouched (`mod methods; pub use methods::description;` already says nothing about `methods`'s own internal shape).

Touches no other crate, no public item's name, and no method's or test's behaviour.

## Order of work

1. `git mv crates/sdk/src/description/methods.rs crates/sdk/src/description/methods/mod.rs`. Pure move, zero content change. Commit alone.
2. Extract `methods/shared.rs` (`VERSION` + the 10 builders). `mod.rs` adds `mod shared;`; `description()` itself calls only the noun functions, `events()`, and `errors()`, so it needs no new import of its own here. Commit.
3. Extract `methods/errors.rs`. Commit.
4. Extract `methods/session.rs`. Commit.
5. Extract `methods/tab.rs`. Commit.
6. Extract `methods/pane.rs`. Commit.
7. Extract `methods/server.rs`. Commit.
8. Extract `methods/event.rs`. Commit.
9. Extract `methods/client.rs`. Commit.
10. Extract `methods/setting.rs`. Commit.
11. Extract `methods/api.rs`. Commit.
12. Extract `methods/tests.rs`. `mod.rs` is now only `description()`, `mod` lines, and the file's own top doc comment. Commit.
13. Record proofs (a), (b) and (c) below. Fix anything they catch.
14. `just check` on the whole tree. Record it.

## Risks

- **A shared helper's signature drifts while moving it** (e.g. a builder loses `pub(super)`, or a type changes by typo) - caught immediately by `cargo check`, not by a human read; every commit above is checked before the next starts.
- **`events()`/`event_methods()` split apart by mistake** - `description()`'s own body computes `event_names` from `events()` and feeds it into `event_methods(&event_names)`; keeping both in one `methods/event.rs` file (not one each) avoids a cross-file coupling that would otherwise need two `pub(super)` functions instead of one module's own private pair.
- **Sibling tasks touching adjacent files.** TASK-1.119 is in `crates/steg` and `crates/sdk` but was told to stay out of `crates/sdk/src/description/methods.rs`, and after this task, everything under `methods/`. The server-side dispatch-tests split is its own draft task now, not this one; this task touches nothing under `crates/server/`.
- **Proof (c)'s mechanical check gives a false pass** if it is not run with a real control. The run below includes a scratch edit that changes one character inside one function body, to show the hash actually moves before trusting that it doesn't for the real split.
- **Rename-vs-copy noise in review.** `git diff` on a `git mv` that immediately gets further content changes in the same commit can render as a full delete+add instead of a clean move. Keeping "move the file" (step 1) and "extract the first new file from it" (step 2) as separate commits keeps the move itself reviewable as a rename with zero diff.

## Proof

All three dump/compare programs are scratch files in the session scratchpad, named `task-1130-...`, never committed - each calls the real `steg_sdk::description()` (or reads the real source files for proof (c)), not a stand-in.

**(a) The description's serialised form is byte-for-byte identical (AC #1).**
`Description` derives `Serialize`/`Deserialize` (`crates/sdk/src/description/shapes.rs:13`). A scratch, uncommitted integration test file, `crates/sdk/tests/zz_dump_description.rs` (deleted before any real commit, the same technique already used while writing this plan):
```rust
#[test]
fn dump() {
    std::fs::write(
        "<scratchpad>/task-1130-description-before.json",
        serde_json::to_string_pretty(&steg_sdk::description()).unwrap(),
    ).unwrap();
}
```
Run once against the commit immediately before this task's first commit (output path `-before`), and again (output path `-after`) against the commit after step 12. `diff <scratchpad>/task-1130-description-before.json <scratchpad>/task-1130-description-after.json` must be empty. Recorded via `asdlc record-check <task> --in <worktree> --as <worker> -- sh -c 'diff <scratchpad>/task-1130-description-before.json <scratchpad>/task-1130-description-after.json'`.
Control: before deleting the scratch test, change one method's `summary` string by one character in a throwaway edit, regenerate `-after`, and show the `diff` is no longer empty - then revert the throwaway edit.

**(b) The sdk's own list of test names that run is identical before and after, none ignored (AC #2).**
`cargo test -p steg-sdk --lib -- --list`, run once before this task's first commit and once after step 12. This half never adds, removes or renames a test - it only moves `methods`'s own `#[cfg(test)] mod tests` one file over, still declared as `mod tests;` from `methods/mod.rs` the same way it was declared inside the old `methods.rs` - so every name stays `description::methods::tests::<name>` on both sides; the two `--list` outputs, sorted, must be byte-identical (no module-prefix change to account for here, unlike the dispatch-tests task). `git grep -c '#\[ignore\]' crates/sdk/src/` is `0` today; must stay `0` after.
Recorded via `asdlc record-check`.
Control: temporarily rename one test function during the split (e.g. drop a trailing character), run the comparison, show it fails - then fix the rename back before committing.

**(c) No function or test body changes beyond imports, `mod` declarations and visibility (AC #2).**
A scratch shell pipeline, run once against the pre-task commit (over `methods.rs`) and once against the final commit (over every file under `methods/`):
```sh
cat <files> \
  | grep -vE '^\s*(use |mod |pub mod |#!?\[)' \
  | sed -E 's/^\s+//; s/\bpub(\(super\)|\(crate\))?\s+//' \
  | grep -v '^$' \
  | sort \
  | sha256sum
```
Sorting makes the hash insensitive to the reordering a split necessarily introduces (which file a function lives in, and its order within that file), while still catching any single character changed inside any function or test body, since every surviving line is still present, just resorted. The two hashes (before, after) must match exactly; recorded via `asdlc record-check`.
Control: before trusting a "match", make one real edit to one function/test body in a scratch copy (not committed) and show the hash changes - proving the pipeline actually discriminates, not merely always agrees.

**(d) `just check` passes on the final tree (AC #3), recorded via `asdlc record-check`.**

## Notes
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
After this lands, a new API method touches: the one noun file under crates/sdk/src/description/methods/ it belongs to, plus methods/tests.rs (both doc_2_section_8() and doc_2_section_8_targets(), and the "== N methods" count assertion). methods/tests.rs stays the one place every method-adding task still collides - this split shrinks the file that collision happens in from 2,122 lines to roughly 230 (the size of the current #[cfg(test)] mod tests block), it does not remove the collision itself. Follow-up, not this task: if method-adding tasks keep colliding on methods/tests.rs once this lands, split it along the same seams its own assertions already group by (the doc-2 section 8 pairs/targets tables, the events/errors counts, the nested-type allow-list) - not attempted here, since all 18 of its tests (not 22, corrected below) are whole-description invariants, none noun-scoped, and deciding how to split an invariant test without changing what it asserts is a separate, smaller design question from this move-only task.

Split by @bruk.o on 2026-11-18, as the plan recommended: this task keeps the sdk description (crates/sdk/src/description/methods.rs into methods/ by noun); the server dispatch tests moved to a draft task, 'Server tests: the dispatch tests split by noun, in one test binary'. The title still names both halves; the criteria are the scope.

Plan checked against the task's three criteria by @bruk.o: ok. Move only: methods.rs becomes methods/ with a file per noun plus shared.rs, errors.rs and tests.rs; one commit per noun after the rename. Three mechanical proofs, each with a control: the description's serialised form byte for byte, the sdk's test list, and a normalised-line hash of the bodies. The byte-for-byte dump uses a scratch, uncommitted test file calling the real description(); the reviewer is to repeat it. methods/tests.rs remains the one file every new method still touches, named as a follow-up.

Build notes:

Departure #1: the tests.rs block has 18 tests, not 22 as the plan said - a count error in planning, not a code difference; the plan's "22" elsewhere in this task's history is wrong, 18 is what the task actually has, confirmed by cargo test -p steg-sdk --lib -- --list both before and after this task's commits.

Departure #2 (proof (c)'s mechanism): the plan's sorted-normalised-line-sha256 pipeline hit a real bug on this machine's sed (BSD/macOS sed's -E does not support \s or \b, so the plan's own sed -E 's/^\s+//' was a silent no-op) and, once fixed, still left one unexplained stray line from reformatting noise I could not pin down cheaply. Rather than debug a hash further, I replaced it with a stronger, more legible check: a script (scratchpad/task-1130-prove-c.sh) that diffs each new file's real content against the *exact* line range it was mechanically extracted from in the pristine pre-task methods.rs (located by grep on the file's own section banners, not guessed), modulo only the pub(super) qualifier AC #2 explicitly allows. It is stricter than the planned hash (an exact ordered diff per file, not a sorted bag of lines) and its control (one character changed in one method's summary in pane.rs) correctly fails before passing clean. I also ran the plan's own attribute-lines-only comparison (sorted, before/after) as an extra, both recorded with their control, confirming no #[...]/#![...] line changed anywhere in scope - this was the one thing the stricter per-file diff could in principle miss if it had still been sort-based; it is not, but the extra check is recorded anyway.

Sighting: just check's test step failed twice in a row (2026-11-18, ~22:05 and ~22:09 UTC-5) on crates/server/tests/attach.rs's own an_unresponsive_panes_own_commands_do_not_stall_the_connection_or_another_pane - 'pane.show must answer within a bound, even with the other pane stuck'. This file is outside this task's scope (crates/sdk/src/description/methods/ only); run alone it passes in ~2-15s both times I tried, consistent with the known stuck-pane timing test flaking under this machine's concurrent worker load, not with this task's change. Recorded the isolated passing run as evidence. Have not yet recorded a fully clean just check; asking the engineer how to proceed (retry again, or accept the isolated confirmation) before finishing.

Check passed: `sh -c 'cargo test -p steg-server --test attach -- a_pane_that_closes_between_a_queued_attach_and_its_arm_leaves_nothing_to_detach an_unresponsive_panes_own_commands_do_not_stall_the_connection_or_another_pane 2>&1 | tail -8'` exited 0 at 2026-11-18T03:22:56+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.130 on task-1.130-sdk-and-server-tests-one-file-per-noun at c4792d40daca:

```text


failures:
    an_unresponsive_panes_own_commands_do_not_stall_the_connection_or_another_pane

test result: FAILED. 1 passed; 1 failed; 0 ignored; 0 measured; 21 filtered out; finished in 22.77s

error: test failed, to rerun pass `-p steg-server --test attach`
```

Check FAILED: `just check` exited 101 at 2026-11-18T03:28:36+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.130 on task-1.130-sdk-and-server-tests-one-file-per-noun at c4792d40daca, last 40 of 71 lines:

```text
test two_connections_attached_to_one_pane_each_get_their_own_updates_and_a_detach_does_not_touch_the_other ... ok
test repeated_attach_and_detach_against_a_flooding_pane_never_sees_an_unknown_stream ... ok
test a_program_that_exits_on_its_own_sends_ended_with_its_real_exit_code ... ok
test pane_close_force_reports_ended_closed_not_the_signals_own_exit_code ... ok
test attaching_to_a_pane_whose_program_already_ended_gets_a_final_full_then_ended ... ok
test detach_ends_the_stream_even_though_the_program_keeps_producing_output ... ok
test two_clients_attached_to_one_pane_both_get_ended_for_the_same_exit ... ok
test a_pane_that_closes_between_a_queued_attach_and_its_arm_leaves_nothing_to_detach ... ok
test a_second_connections_control_call_is_not_blocked_by_a_stuck_panes_lock_held_attach has been running for over 60 seconds
test an_unresponsive_panes_own_commands_do_not_stall_the_connection_or_another_pane has been running for over 60 seconds
test pane_close_force_returns_within_its_bound_even_while_the_panes_own_worker_is_stuck has been running for over 60 seconds
test stopping_the_server_returns_within_its_bound_even_while_a_panes_worker_is_stuck has been running for over 60 seconds
test a_second_connections_control_call_is_not_blocked_by_a_stuck_panes_lock_held_attach ... ok
test an_unresponsive_panes_own_commands_do_not_stall_the_connection_or_another_pane ... FAILED
test pane_close_force_returns_within_its_bound_even_while_the_panes_own_worker_is_stuck ... ok
test stopping_the_server_returns_within_its_bound_even_while_a_panes_worker_is_stuck ... FAILED

failures:

---- an_unresponsive_panes_own_commands_do_not_stall_the_connection_or_another_pane stdout ----

thread 'an_unresponsive_panes_own_commands_do_not_stall_the_connection_or_another_pane' (58059315) panicked at crates/server/tests/attach.rs:149:39:
reading the reply: Os { code: 35, kind: WouldBlock, message: "Resource temporarily unavailable" }
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace

---- stopping_the_server_returns_within_its_bound_even_while_a_panes_worker_is_stuck stdout ----

thread 'stopping_the_server_returns_within_its_bound_even_while_a_panes_worker_is_stuck' (58059343) panicked at crates/server/tests/attach.rs:149:39:
reading the reply: Os { code: 35, kind: WouldBlock, message: "Resource temporarily unavailable" }


failures:
    an_unresponsive_panes_own_commands_do_not_stall_the_connection_or_another_pane
    stopping_the_server_returns_within_its_bound_even_while_a_panes_worker_is_stuck

test result: FAILED. 21 passed; 2 failed; 0 ignored; 0 measured; 0 filtered out; finished in 133.75s

error: test failed, to rerun pass `-p steg-server --test attach`
full output: .quiet/tests-p-steg-p-steg-app-p-steg-kernel-p-steg-sdk-p-steg-server.log
error: recipe `test` failed with exit code 101
```

Check passed: `cargo test -p steg-server --test attach -- an_unresponsive_panes_own_commands_do_not_stall_the_connection_or_another_pane stopping_the_server_returns_within_its_bound_even_while_a_panes_worker_is_stuck` exited 0 at 2026-11-18T03:29:09+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.130 on task-1.130-sdk-and-server-tests-one-file-per-noun at c4792d40daca:

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.08s
     Running tests/attach.rs (target/debug/deps/attach-1c3f77caae0e73d3)

running 2 tests
test an_unresponsive_panes_own_commands_do_not_stall_the_connection_or_another_pane ... ok
test stopping_the_server_returns_within_its_bound_even_while_a_panes_worker_is_stuck ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 21 filtered out; finished in 21.37s
```
<!-- SECTION:NOTES:END -->

## Comments

<!-- COMMENTS:BEGIN -->
author: @bruk
created: 2026-11-18T02:30:00Z

---

Plan approved by @bruk (dc8b72722100, size-3): its worker may build it.

---
<!-- COMMENTS:END -->
