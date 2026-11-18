---
id: TASK-1.130
title: >-
  sdk and server tests: one file per noun for the API description and the
  dispatch tests
status: In Progress
assignee:
  - '@bruk.w6'
created_date: '2026-11-18T02:09:00Z'
updated_date: '2026-11-18T02:25:00Z'
labels:
  - size-3
milestone: m-3
dependencies: []
parent_task_id: TASK-1
project: bruk-io-steg
asdlc_accepted_from: DRAFT-1
asdlc_branch: task-1.130-sdk-and-server-tests-one-file-per-noun
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Agreed with the engineer on 2026-11-17 to let more server tasks run at once. Of the last 40 merges, 9 touched crates/sdk/src/description/methods.rs (2,122 lines) and 9 touched crates/server/tests/dispatch.rs (2,978 lines), because every new method appends to both, so two server tasks in flight nearly always conflict. methods.rs is already organised as session_methods, tab_methods, pane_methods and so on in one file. This task only moves code. It must run while no other task is editing these files, and it can run beside the runtime.rs split, which touches different files.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The session, tab, pane, server, event, client, setting and api parts of crates/sdk/src/description/methods.rs each live in their own file, with the shared helpers in one place; the description's serialised form is byte for byte the same before and after, shown by a recorded comparison
- [ ] #2 crates/server/tests/dispatch.rs is split by noun the same way; the list of test names that run is identical before and after, none ignored, shown by a recorded comparison
- [ ] #3 No behaviour changes: no public item is added, removed or renamed, and no function body changes beyond imports and visibility
- [ ] #4 just check passes, and the notes say which files a new method now touches
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
## Files that change

sdk half (crates/sdk/src/description/):
- `crates/sdk/src/description/methods.rs` -> `crates/sdk/src/description/methods/mod.rs`: moved, then reduced to `description()`, `VERSION`'s re-export, and `mod` declarations only.
- `crates/sdk/src/description/methods/shared.rs` (new): `VERSION` and the shared parameter builders (`method`, `target_of_kind`, `target`, `required_target`, `method_target`, `flag`, `switch`, `program`, `direction`, `field`), each `pub(super)`.
- `crates/sdk/src/description/methods/session.rs` (new): `session_fields`, `session_methods`.
- `crates/sdk/src/description/methods/tab.rs` (new): `tab_fields`, `tab_methods`.
- `crates/sdk/src/description/methods/pane.rs` (new): `pane_fields`, `pane_methods`.
- `crates/sdk/src/description/methods/server.rs` (new): `server_fields`, `server_methods`.
- `crates/sdk/src/description/methods/event.rs` (new): `events`, `event_methods` (kept together: `description()` feeds `events()`'s own names into `event_methods`).
- `crates/sdk/src/description/methods/client.rs` (new): `client_fields`, `client_methods`.
- `crates/sdk/src/description/methods/setting.rs` (new): `setting_fields`, `setting_methods`.
- `crates/sdk/src/description/methods/api.rs` (new): `api_method_fields`, `api_methods`.
- `crates/sdk/src/description/methods/errors.rs` (new): `errors`.
- `crates/sdk/src/description/methods/tests.rs` (new): the whole `#[cfg(test)] mod tests` block, unchanged. These 22 tests are whole-description invariants (the doc-2 §8 set, its targets table, the events/errors counts, the nested-type allow-list), not noun-scoped, so they are not split per noun.
- `crates/sdk/src/description/mod.rs`: untouched (`mod methods; pub use methods::description;` already says nothing about `methods`'s own internal shape).

server half (crates/server/tests/):
- `crates/server/tests/dispatch.rs` -> `crates/server/tests/dispatch/main.rs`: moved (Cargo's own convention keeps the compiled test binary named `dispatch` either way - confirmed against `crates/server/Cargo.toml`, which declares no `[[test]]` entry for `dispatch` at all, so both forms reach it purely through `autotests` discovery). Reduced to the file's own top doc comment, the `#![expect(clippy::unwrap_used, ...)]` inner attribute (applies to the whole binary, every submodule included, from here), and `mod` declarations only.
- `crates/server/tests/dispatch/support.rs` (new): `moment`, `request`, `fresh`, `ok`, `err` - the five helpers every other module needs. `pub(super)`.
- `crates/server/tests/dispatch/session.rs` (new): the "session" section's 9 tests, plus its own local helpers `declared_fields`/`assert_same_set` (used only here).
- `crates/server/tests/dispatch/tab.rs` (new): the "tab" section and the "tab.list / tab.show" section, 20 tests, plus the local helper `tab_mut_for_test` (used only here).
- `crates/server/tests/dispatch/pane.rs` (new): the "pane" section and the "the ten-minute mark" section (doc-11 R83/decision-7 is a pane-status feature), 17 tests.
- `crates/server/tests/dispatch/api.rs` (new): the "api" section and its three `api.call` subsections, 17 tests.
- `crates/server/tests/dispatch/setting.rs` (new): the "setting" section, 9 tests.
- `crates/server/tests/dispatch/cross_cutting.rs` (new): the "completeness" section (2 tests), the "shapes: Nothing/Identifier" section (1 test), and the "end-to-end scenarios" section (2 tests) - 5 tests that each exercise every method or several nouns in one request chain and belong to no single noun, plus the private helper `minimal_value` (used only by these). No `server.rs`/`client.rs`/`event.rs` noun files are created: dispatch has no noun-specific behavioural tests for those three nouns today (their methods are all `NotImplementedYet`, covered generically by `cross_cutting.rs`'s own completeness/not-yet-scenario tests) - inventing empty files for them would misstate what exists.

Neither half touches `crates/server/src/**`, `crates/app/**`, any public item's name, or any test or method's behaviour.

## Order of work

Two independent halves (different crates, different proof setups, no shared file) - **recommend splitting into two child tasks** so they can be approved and built independently; this plan covers both in case the engineer prefers to keep it as one `size-3`.

### Half A: sdk description (do first - the smaller of the two)
1. `git mv crates/sdk/src/description/methods.rs crates/sdk/src/description/methods/mod.rs`. Pure move, zero content change. Commit alone.
2. Extract `methods/shared.rs` (`VERSION` + the 10 builders). `mod.rs` adds `mod shared;` and imports what it still needs directly (today: nothing - `description()` itself calls only the noun functions, `events()`, and `errors()`). Commit.
3. Extract `methods/errors.rs`. Commit.
4. Extract `methods/session.rs`. Commit.
5. Extract `methods/tab.rs`. Commit.
6. Extract `methods/pane.rs`. Commit.
7. Extract `methods/server.rs`. Commit.
8. Extract `methods/event.rs`. Commit.
9. Extract `methods/client.rs`. Commit.
10. Extract `methods/setting.rs`. Commit.
11. Extract `methods/api.rs`. Commit.
12. Extract `methods/tests.rs`. Commit.
13. Record proof (a) and (c) for this half (below), fix anything they catch.

### Half B: server dispatch tests (do second)
1. `git mv crates/server/tests/dispatch.rs crates/server/tests/dispatch/main.rs`. Pure move. Commit alone. Record proof (b) right after this commit too: the binary name and every bare test name must already be identical to the pre-move baseline, since nothing but the path changed.
2. Extract `dispatch/support.rs`. `main.rs` adds `mod support;` and the explicit imports each remaining test still needs.
3. Extract `dispatch/cross_cutting.rs` (least entangled with the rest; validates the `support` split works across unrelated test groups early).
4. Extract `dispatch/session.rs`.
5. Extract `dispatch/tab.rs`.
6. Extract `dispatch/pane.rs`.
7. Extract `dispatch/api.rs`.
8. Extract `dispatch/setting.rs`. `main.rs` now holds only its doc comment, the `#![expect]`, and `mod` lines.
9. Record proof (b) and (c) for this half, fix anything they catch.

### Last
10. `just check` on the whole tree. Record it.

## Why one binary (`dispatch/main.rs` + modules), not one binary per noun file

The task names both options. One binary wins, for reasons sharper than raw link time:

- **`crates/server/tests/support/mod.rs`'s own doc comment (lines 20-33) is the decisive evidence, already in this repo.** It records that cross-binary shared helpers forced a downgrade from `#[expect(dead_code)]` to `#[allow(dead_code, reason = "...")]`, because "an unfulfilled lint expectation has no cfg this module could gate on per binary" when not every binary that includes the shared module calls every helper in it. `dispatch.rs`'s own `#![expect(clippy::unwrap_used, ...)]` (line 11) has exactly this shape: split into N independent binaries, every one of them needs its own copy of that attribute, and the `support`-style shared-helpers file risks the same unfulfilled-expectation problem the moment one binary's subset of tests doesn't happen to call every shared helper (already true for `minimal_value`, used only by two of the eight groups). One binary with ordinary child modules keeps one `#![expect]`, with real, unconditional visibility (`pub(super)`) between modules - no lint-suppression dance to design around.
- **Test identity is close to free this way.** `git mv dispatch.rs dispatch/main.rs` keeps the compiled binary named `dispatch`. The before/after `--list` comparison (proof b) then only has to account for `module::` prefixes being added to each bare test name inside one stable binary - not binary names and hash suffixes changing on top of that.
- **Measured, not assumed, link cost.** From a warm `target/` (dependencies already built - the normal case for a `just test`/`just check` run, since most changes don't touch `steg-kernel`/`steg-sdk`), relinking the existing single `dispatch` binary after a no-op touch took 0.33s wall. The real comparison recorded as this half's own first proof step is: `cargo clean -p steg-server && time cargo test -p steg-server --no-run` on the commit right before this task's first commit, versus the same command on the commit after every `dispatch/*.rs` extraction lands - the actual delta of linking one binary against eight (support.rs itself produces no binary; `main.rs` is the one and only target) versus one. One binary gives a delta of zero by construction; this run is what proves it, rather than assuming it.
- **`just test`'s selection is unaffected either way** - confirmed from `tools/src/steg_tools/affected.py`'s own code, not measured: `owner()` maps a changed path to the crate whose folder is the longest prefix (`crates/server/` either way), and `selection()` emits `-p steg-server`; `cargo test -p steg-server` then runs every target under that crate regardless of how many `tests/*.rs` top-level files or `tests/<name>/main.rs` directories it has. Splitting a file inside `crates/server/tests/` can never change which crates `just test` selects, nor whether that selection differs between the one-binary and N-binary layouts.
- Cargo runs integration test *targets* sequentially (only the tests inside one binary run in parallel); eight small binaries of fast, in-memory, allocation-only tests is likely slower end to end than one, on top of paying eight separate process-start costs every run.

## Risks

- **A shared helper's signature drifts while moving it** (e.g. `declared_fields`/`tab_mut_for_test` need the exact same `pub(super)` visibility and parameter types in their new file) - caught immediately by `cargo check`, not by a human read; every commit above is checked before the next starts.
- **`events()`/`event_methods()` split apart by mistake** - `description()`'s own body computes `event_names` from `events()` and feeds it into `event_methods(&event_names)`; keeping both in one `methods/event.rs` file (not one each) avoids a cross-file coupling that would otherwise need two `pub(super)` functions instead of one module's own private pair.
- **A noun file ends up with zero real `.unwrap()` calls**, breaking the server half's `#![expect(clippy::unwrap_used)]` if it were (wrongly) duplicated per file - avoided entirely by keeping that attribute once, in `dispatch/main.rs`, since an inner attribute at a binary's root scope covers every child module.
- **Two sibling tasks touch adjacent files.** TASK-1.119 is in `crates/steg` and `crates/sdk` but was told to stay out of `crates/sdk/src/description/methods.rs` (and, after this task, everything under `methods/`); TASK-1.131 is in `crates/server/src/runtime.rs` only, never `crates/server/tests/`. Neither half of this task touches `crates/server/src/**`. If `just check`/`just test` fails for a reason outside this task's own files once `origin/main` is merged in (step "Last" above), that is a collision to report on the task, not to silently work around.
- **Proof (c)'s mechanical check gives a false pass** if it is not run with a real control. Each half's proof (c) run includes a scratch edit that changes one character inside one method/test body, to show the hash actually moves before trusting that it doesn't, for the real split.
- **Rename-vs-copy noise in review.** `git diff` on a `git mv` that immediately gets further content changes in the same commit can render as a full delete+add instead of a clean move. Keeping "move the file" and "extract the first new file from it" as two separate commits (steps 1 and 2 of each half) keeps the move itself reviewable as a rename with zero diff.

## Proof

**(a) The description's serialised form is byte-for-byte identical (AC #1).**
`Description` derives `Serialize`/`Deserialize` (`crates/sdk/src/description/shapes.rs:13`), so a scratch, uncommitted integration test file (the same technique already used to measure this plan, `crates/sdk/tests/zz_dump_description.rs`, deleted before any real commit) does:
```rust
#[test]
fn dump() {
    std::fs::write(
        "<scratchpad>/task-1130-description-before.json",
        serde_json::to_string_pretty(&steg_sdk::description()).unwrap(),
    ).unwrap();
}
```
Run once against the commit immediately before this task's first commit (`-before`), and again with the output path changed to `-after` against the commit after step 12 of half A. `diff <scratchpad>/task-1130-description-before.json <scratchpad>/task-1130-description-after.json` must be empty. Recorded via `asdlc record-check <task> --in <worktree> --as <worker> -- sh -c 'diff <scratchpad>/task-1130-description-before.json <scratchpad>/task-1130-description-after.json'`.
Control: before deleting the scratch test, change one method's `summary` string by one character in a throwaway edit, regenerate `-after`, and show the `diff` is no longer empty - then revert the throwaway edit.

**(b) The list of test names that run is identical before and after, none ignored (AC #2).**
Baseline, taken once before this task's first commit in each half:
- sdk: `cargo test -p steg-sdk --lib -- --list 2>&1 | grep '::' ` (every unit test under `description::methods::tests::*` plus the rest of the crate's own tests, since this half never adds or removes a test, only moves the ones inside `methods::tests` one module level - today they are `methods::tests::<name>`; after the split they will still be `methods::tests::<name>`, since `tests.rs` is declared from `methods/mod.rs` the same way `tests` was declared inside the old `methods.rs`).
- server: `cargo test -p steg-server --test dispatch -- --list` before the move (names are bare, e.g. `session_new_creates_a_plain_shell_session`), and again after every extraction (names will be `session::session_new_creates_a_plain_shell_session`, etc. - the module prefix is the one real, expected difference, named up front here rather than discovered as a surprise). Strip the leading `<module>::` from the "after" list with a one-line `sed 's/^[a-z_]*:://'` before comparing sets, and separately assert the stripped set has no internal collisions (two different modules producing the same bare name after stripping would be a real bug this check must not hide).
`git grep -c '#\[ignore\]' crates/server/tests/ crates/sdk/src/` today is `1` (only `openpty_race.rs`, outside this task's files) - confirm it is still `1` after, i.e. this task adds no `#[ignore]` anywhere.
Each comparison recorded via `asdlc record-check`.
Control: temporarily rename one test function during the split (e.g. drop a trailing `s`), run the comparison, show it fails: then fix the rename back before committing.

**(c) No function or test body changes beyond imports, `mod` declarations and visibility (AC #3).**
A scratch shell pipeline, run once against the pre-task commit and once against the final commit of each half, over exactly that half's in-scope files (the old single file's path for "before"; every new file under `methods/` or `dispatch/` for "after"):
```sh
cat <files> \
  | grep -vE '^\s*(use |mod |pub mod |#!?\[)' \
  | sed -E 's/^\s+//; s/\bpub(\(super\)|\(crate\))?\s+//' \
  | grep -v '^$' \
  | sort \
  | sha256sum
```
Sorting makes the hash insensitive to the reordering a split necessarily introduces (which file a function lives in, and its order within that file), while still catching any single character changed inside any function or test body, since every surviving line is still present, just resorted. The two hashes (before, after) must match exactly; recorded via `asdlc record-check`.
Control: before trusting a "match," make one real edit to one function/test body in a scratch copy (not committed) and show the hash changes - proving the pipeline actually discriminates, not merely always agrees.

**(d) `just check` passes on the final tree (AC #4), each run recorded via `asdlc record-check`.**

## Notes
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
After this lands, a new API method touches: the one noun file under crates/sdk/src/description/methods/ it belongs to, plus methods/tests.rs (both doc_2_section_8() and doc_2_section_8_targets(), and the "== N methods" count assertion) - that file stays the one place every method-adding task still collides, this split does not remove it, only shrinks it from a 2,122-line file to a ~400-line one. On the server side it touches the matching noun file under crates/server/tests/dispatch/ (or dispatch/cross_cutting.rs, for a method with no noun-specific behavioural test yet). Recommend splitting this task into two child tasks (sdk description; server dispatch tests) before build: different crates, different proof setups, no shared file, and either can be approved and merged without the other.
<!-- SECTION:NOTES:END -->
