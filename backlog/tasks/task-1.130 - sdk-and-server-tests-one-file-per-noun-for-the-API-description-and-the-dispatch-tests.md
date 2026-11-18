---
id: TASK-1.130
title: >-
  sdk and server tests: one file per noun for the API description and the
  dispatch tests
status: In Progress
assignee:
  - '@bruk.w6'
created_date: '2026-11-18T02:09:00Z'
updated_date: '2026-11-18T03:15:00Z'
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
After this lands, a new API method touches: the one noun file under crates/sdk/src/description/methods/ it belongs to, plus methods/tests.rs (both doc_2_section_8() and doc_2_section_8_targets(), and the "== N methods" count assertion). methods/tests.rs stays the one place every method-adding task still collides - this split shrinks the file that collision happens in from 2,122 lines to roughly 230 (the size of the current #[cfg(test)] mod tests block), it does not remove the collision itself. Follow-up, not this task: if method-adding tasks keep colliding on methods/tests.rs once this lands, split it along the same seams its own assertions already group by (the doc-2 section 8 pairs/targets tables, the events/errors counts, the nested-type allow-list) - not attempted here, since all 22 of its tests are whole-description invariants, none noun-scoped, and deciding how to split an invariant test without changing what it asserts is a separate, smaller design question from this move-only task.

Split by @bruk.o on 2026-11-18, as the plan recommended: this task keeps the sdk description (crates/sdk/src/description/methods.rs into methods/ by noun); the server dispatch tests moved to a draft task, 'Server tests: the dispatch tests split by noun, in one test binary'. The title still names both halves; the criteria are the scope.

Plan checked against the task's three criteria by @bruk.o: ok. Move only: methods.rs becomes methods/ with a file per noun plus shared.rs, errors.rs and tests.rs; one commit per noun after the rename. Three mechanical proofs, each with a control: the description's serialised form byte for byte, the sdk's test list, and a normalised-line hash of the bodies. The byte-for-byte dump uses a scratch, uncommitted test file calling the real description(); the reviewer is to repeat it. methods/tests.rs remains the one file every new method still touches, named as a follow-up.

Check FAILED: `sh -c 'diff '"'"'/private/tmp/claude-501/-Users-brukhabtu-src-github-com-bruk-io-steg/a318fbf0-a659-4ad6-9bfc-86a79ba9cc76/scratchpad/task-1130-description-before.json'"'"' '"'"'/private/tmp/claude-501/-Users-brukhabtu-src-github-com-bruk-io-steg/a318fbf0-a659-4ad6-9bfc-86a79ba9cc76/scratchpad/task-1130-description-control.json'"'"''` exited 1 at 2026-11-18T02:57:59+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.130 on task-1.130-sdk-and-server-tests-one-file-per-noun at c4792d40daca with uncommitted changes:

```text
12c12
<       "summary": "Every session on the host",
---
>       "summary": "Every session on the hostX",
```

Check passed: `sh -c 'diff '"'"'/private/tmp/claude-501/-Users-brukhabtu-src-github-com-bruk-io-steg/a318fbf0-a659-4ad6-9bfc-86a79ba9cc76/scratchpad/task-1130-description-before.json'"'"' '"'"'/private/tmp/claude-501/-Users-brukhabtu-src-github-com-bruk-io-steg/a318fbf0-a659-4ad6-9bfc-86a79ba9cc76/scratchpad/task-1130-description-final.json'"'"' && echo '"'"'proof (a): description() is byte-for-byte identical before and after'"'"''` exited 0 at 2026-11-18T02:58:20+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.130 on task-1.130-sdk-and-server-tests-one-file-per-noun at c4792d40daca with uncommitted changes:

```text
proof (a): description() is byte-for-byte identical before and after
```

Check FAILED: `sh -c 'diff '"'"'/private/tmp/claude-501/-Users-brukhabtu-src-github-com-bruk-io-steg/a318fbf0-a659-4ad6-9bfc-86a79ba9cc76/scratchpad/task-1130-testlist-before.txt'"'"' '"'"'/private/tmp/claude-501/-Users-brukhabtu-src-github-com-bruk-io-steg/a318fbf0-a659-4ad6-9bfc-86a79ba9cc76/scratchpad/task-1130-testlist-control.txt'"'"''` exited 1 at 2026-11-18T02:58:47+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.130 on task-1.130-sdk-and-server-tests-one-file-per-noun at c4792d40daca with uncommitted changes:

```text
23c23
< description::methods::tests::the_method_set_is_exactly_doc_2_section_8s_45_pairs: test
---
> description::methods::tests::the_method_set_is_exactly_doc_2_section_8s_45_pair: test
```

Check passed: `/private/tmp/claude-501/-Users-brukhabtu-src-github-com-bruk-io-steg/a318fbf0-a659-4ad6-9bfc-86a79ba9cc76/scratchpad/task-1130-prove-b.sh` exited 0 at 2026-11-18T02:59:48+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.130 on task-1.130-sdk-and-server-tests-one-file-per-noun at c4792d40daca:

```text
proof (b): the sdk's test-name list is identical before and after, and no test is ignored
```

Check FAILED: `/private/tmp/claude-501/-Users-brukhabtu-src-github-com-bruk-io-steg/a318fbf0-a659-4ad6-9bfc-86a79ba9cc76/scratchpad/task-1130-prove-c.sh` exited 1 at 2026-11-18T03:00:08+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.130 on task-1.130-sdk-and-server-tests-one-file-per-noun at c4792d40daca with uncommitted changes:

```text
ok: shared.rs
ok: shared.rs
ok: session.rs
ok: tab.rs
MISMATCH: pane.rs (before 600,1014 vs after 7,421)
84c84
<             summary: "Split a pane and run a program in the new one".into(),
---
>             summary: "Split a pane and run a program in the new oneX".into(),
ok: server.rs
ok: event.rs
ok: client.rs
ok: setting.rs
ok: api.rs
ok: errors.rs
ok: tests.rs
FAIL
```

Check passed: `/private/tmp/claude-501/-Users-brukhabtu-src-github-com-bruk-io-steg/a318fbf0-a659-4ad6-9bfc-86a79ba9cc76/scratchpad/task-1130-prove-c.sh` exited 0 at 2026-11-18T03:00:31+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.130 on task-1.130-sdk-and-server-tests-one-file-per-noun at c4792d40daca:

```text
ok: shared.rs
ok: shared.rs
ok: session.rs
ok: tab.rs
ok: pane.rs
ok: server.rs
ok: event.rs
ok: client.rs
ok: setting.rs
ok: api.rs
ok: errors.rs
ok: tests.rs
PASS: every extracted file's real content matches its exact pre-task source range, unchanged but for the pub(super) visibility this split adds
```

Check FAILED: `/private/tmp/claude-501/-Users-brukhabtu-src-github-com-bruk-io-steg/a318fbf0-a659-4ad6-9bfc-86a79ba9cc76/scratchpad/task-1130-prove-attrs.sh` exited 1 at 2026-11-18T03:01:00+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.130 on task-1.130-sdk-and-server-tests-one-file-per-noun at c4792d40daca with uncommitted changes:

```text
0a1
> #[allow(dead_code)]
```

Check passed: `/private/tmp/claude-501/-Users-brukhabtu-src-github-com-bruk-io-steg/a318fbf0-a659-4ad6-9bfc-86a79ba9cc76/scratchpad/task-1130-prove-attrs.sh` exited 0 at 2026-11-18T03:01:14+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.130 on task-1.130-sdk-and-server-tests-one-file-per-noun at c4792d40daca:

```text
proof (c), attributes: the set of attribute lines is identical before and after
```

Check passed: `sh -c 'cargo test -p steg-server --test attach an_unresponsive_panes_own_commands_do_not_stall_the_connection_or_another_pane 2>&1 | tail -6'` exited 0 at 2026-11-18T03:15:31+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.130 on task-1.130-sdk-and-server-tests-one-file-per-noun at c4792d40daca:

```text

running 1 test
test an_unresponsive_panes_own_commands_do_not_stall_the_connection_or_another_pane ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 22 filtered out; finished in 15.06s
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
