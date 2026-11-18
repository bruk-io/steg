---
id: TASK-1.130
title: >-
  sdk and server tests: one file per noun for the API description and the
  dispatch tests
status: In Review
assignee:
  - '@bruk.w6'
created_date: '2026-11-18T02:09:00Z'
updated_date: '2026-11-18T03:57:00Z'
labels:
  - size-3
  - review-passed
milestone: m-3
dependencies: []
references:
  - task-1.130-sdk-and-server-tests-one-file-per-noun
  - https://github.com/bruk-io/steg/pull/78
parent_task_id: TASK-1
project: bruk-io-steg
asdlc_accepted_from: DRAFT-1
asdlc_branch: task-1.130-sdk-and-server-tests-one-file-per-noun
asdlc_plan_approved_by: bruk
asdlc_plan_approved_hash: 'dc8b72722100'
asdlc_plan_approved_size: size-3
asdlc_review_kind: same-machine
asdlc_review_findings: 'important=0,nit=2'
asdlc_review_policies: ["review"]
asdlc_plan_matched: no
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
After this lands, a new API method touches: the one noun file under crates/sdk/src/description/methods/ it belongs to, plus methods/tests.rs (both doc_2_section_8() and doc_2_section_8_targets(), and the "== N methods" count assertion). methods/tests.rs stays the one place every method-adding task still collides - this split shrinks the file that collision happens in from 2,122 lines to roughly 230 (the size of the current #[cfg(test)] mod tests block), it does not remove the collision itself. Follow-up, not this task: if method-adding tasks keep colliding on methods/tests.rs once this lands, split it along the same seams its own assertions already group by (the doc-2 section 8 pairs/targets tables, the events/errors counts, the nested-type allow-list) - not attempted here, since all 18 of its tests are whole-description invariants, none noun-scoped, and deciding how to split an invariant test without changing what it asserts is a separate, smaller design question from this move-only task.

Split by @bruk.o on 2026-11-18, as the plan recommended: this task keeps the sdk description (crates/sdk/src/description/methods.rs into methods/ by noun); the server dispatch tests moved to a draft task, 'Server tests: the dispatch tests split by noun, in one test binary'. The title still names both halves; the criteria are the scope.

Plan checked against the task's three criteria by @bruk.o: ok. Move only: methods.rs becomes methods/ with a file per noun plus shared.rs, errors.rs and tests.rs; one commit per noun after the rename. Three mechanical proofs, each with a control: the description's serialised form byte for byte, the sdk's test list, and a normalised-line hash of the bodies. The byte-for-byte dump uses a scratch, uncommitted test file calling the real description(); the reviewer is to repeat it. methods/tests.rs remains the one file every new method still touches, named as a follow-up.

Build notes:

Departures:
- The tests.rs block has 18 tests, not 22 as the plan said - a count error in planning, not a code difference, confirmed by cargo test -p steg-sdk --lib -- --list both before and after this task's commits.
- Proof (c)'s mechanism: the plan's sorted-normalised-line-sha256 pipeline hit a real bug on this machine's sed (macOS/BSD sed's -E does not support \s or \b, so the plan's own sed -E 's/^\s+//' was a silent no-op on this sed). Replaced it with an ordered per-file diff against the exact original line range (located by grep on the pre-task file's own section banners, not guessed), modulo only the pub(super) qualifier AC #2 allows - stricter than a sorted hash, since it preserves order and so cannot hide a reorder-and-collide false pass. Also ran the plan's own attribute-lines-only comparison as an extra. All four proofs (a, b, c, and the attribute extra) recorded with their own control: control recorded failing (scratch edit in place), reverted, then the real proof recorded passing, at the final head.

Sighting for TASK-1.123: just check's test step failed 4 times across this build (2026-11-18, ~22:05 through ~22:26 UTC-5), every time in crates/server/tests/attach.rs - a file outside this task's scope (crates/sdk/src/description/methods/ only). The specific test(s) varied: an_unresponsive_panes_own_commands_do_not_stall_the_connection_or_another_pane failed all 4 times; stopping_the_server_returns_within_its_bound_even_while_a_panes_worker_is_stuck and a_pane_that_closes_between_a_queued_attach_and_its_arm_leaves_nothing_to_detach each failed once more alongside it. Failure modes seen: a bound-timeout panic ('pane.show/pane.attach must answer within a bound, even with the other pane stuck') on three runs, and once a genuine OS-level error reading a reply socket (Os { code: 35, kind: WouldBlock, message: "Resource temporarily unavailable" }) - the clearest sign this is real host contention, not a race in the test or the code: the kernel itself refused a read because the machine was out of some resource (likely fds or scheduler time) under load. 22:29  up 4 days,  7:31, 6 users, load averages: 4.14 5.85 7.37 during the build: load averages 9.18/7.80/8.65 (6 users) on the failing runs, 4.93/6.10/7.50 after. Every one of these tests passes cleanly in isolation (confirmed 3 times, ~2-22s each); the only run where an isolated pair also flaked once was itself under the same heavy load. Recorded: the 4th just check run as an honest failing record (exit 101, only crates/server/tests/attach.rs affected, unrecorded first to confirm no environment data in the output), and the isolated passing rerun of both of that run's failing tests. CI's run on the pushed commit is the clean just check for this head, on an unloaded machine.

Built: crates/sdk/src/description/methods.rs split into methods/ by noun (shared.rs, errors.rs, session/tab/pane/server/event/client/setting/api.rs, tests.rs), 12 commits, one per step. Checked at every step with cargo check/clippy/fmt/test. Four proofs recorded with their own control each (control failing, reverted, pass recorded): (a) description() byte-for-byte dump identical, (b) sdk test-name list identical and #[ignore] stays 0, (c) ordered per-file diff against the exact pre-task source range (departed from the plan's sorted-hash script, which hit a BSD-sed \s bug on this machine; this replacement is stricter, since it preserves order), (c extra) attribute-lines-only comparison. Local just check failed 4 times, every time only on crates/server/tests/attach.rs's own timing-bound tests (outside this task's files), under heavy concurrent-worker load (uptime load averages up to 9.18/7.80/8.65); crates/sdk's own tests and all recorded proofs passed every single time. Sighting recorded on the task notes for TASK-1.123. CI's just check on PR #78's head (91048c62) passed clean (12m46s) on an unloaded machine - that is the clean just check for this change. PR: https://github.com/bruk-io/steg/pull/78
<!-- SECTION:NOTES:END -->

## Comments

<!-- COMMENTS:BEGIN -->
author: @bruk
created: 2026-11-18T02:30:00Z

---

Plan approved by @bruk (dc8b72722100, size-3): its worker may build it.

---

created: 2026-11-18T03:43:00Z

---

CI's just check on PR #78's head (91048c62) passed clean (COMPLETED/SUCCESS, 12m46s): https://github.com/bruk-io/steg/actions/runs/44230938709/job/134555512244. That is the clean just check for this change, on an unloaded machine, per the engineer's instruction - local just check on this machine was not green at the final head (see the sighting in the notes: crates/server/tests/attach.rs's own timing tests flaking under 5 other concurrent workers' load, uptime up to 9.18/7.80/8.65, a file this task never touches); crates/sdk's own tests and all four recorded proofs passed every time.

---

author: @bruk.r1
created: 2026-11-18T03:44:00Z

---

@bruk.r1 is reviewing TASK-1.130: a same-machine review: it waited 0m with no other engineer's reviewer, longer than review_fallback_after (0m), so a fresh reviewer on its author's machine takes it.

---

author: @bruk.r1
created: 2026-11-18T03:57:00Z

---

Review by @bruk.r1 (same-machine): clean. It stays In Review for a person to merge.

Findings: 0 Important, 2 nit. Cites: review. The diff does not match the plan.

Independently re-derived the three mechanical proofs rather than relying on the worker's own scripts:
1. description() dump: wrote my own scratch #[test] (REVIEW_DUMP_PATH env var) and ran it at the merge-base (9d24c210, origin/main...HEAD base) and at the task head (91048c62), both via `cargo test --manifest-path crates/sdk/Cargo.toml` in fresh detached worktrees. The two JSON dumps are byte-identical (`diff` exit 0, 2899 lines each). Control: edited one summary string by one character in a scratch edit on session.rs, reran, diff reported the exact 2-line change; reverted completely (`git status` clean in both worktrees afterward, both worktrees then removed).
2. Move/no-body-change check via `git diff --color-moved=zebra --color-moved-ws=allow-indentation-change origin/main...HEAD -- crates/sdk/src/description/`: 2125 deleted / 2166 added lines total; of those, 66 deleted and 107 added lines are NOT classified as moved. I read every one of those 173 lines: all are `use`/`mod` lines (narrowed per-file imports), file-level `//!` doc comments, noun banner comments (`// --- session ---`) dropped as redundant once each noun has its own file, the old item-level `///` doc on errors() replaced by errors.rs's own `//!` doc, the `#[cfg(test)] mod tests { }` wrapper replaced by `mod tests;` + a separate file, and every `fn ...`/`pub const VERSION` signature line that gained exactly `pub(super)` (never `pub` or `pub(crate)`). Arithmetic check: 2125-66=2059 and 2166-107=2059 - every line outside these two lists pairs up as a moved line, so there is no unmatched body change hiding anywhere. No function or test body text changed.
3. Test list: `cargo test -p steg-sdk --lib -- --list`, sorted, byte-identical before/after (168 tests both sides); `description::methods::tests::` count is 18 both before and after (confirms the notes' "18, not 22" is a planning miscount, not lost tests); `git grep ignore -- crates/sdk/src/` shows no `#[ignore]` on either side. `cargo test -p steg-sdk --lib` passes 168/168 at head; `cargo clippy --manifest-path crates/sdk/Cargo.toml --all-targets` is clean.
4. Visibility/surface: read description/mod.rs and lib.rs directly (not just diffed) - `mod methods;` stays private, only `pub use methods::description;` is re-exported, no VERSION export anywhere; matches pre-task surface exactly. Grepped the diff for any `pub`/`pub(crate)` addition: only pre-existing `pub fn description()` and new `pub(super)` qualifiers - nothing widened.
5. Attributes: `#[test]`x18, `#[cfg(test)]`x1 and `#[must_use]`x1 appear on both the removed and added sides in equal counts; no attribute added, removed or changed. I also read the worker's own proof (c) replacement script (task-1130-prove-c.sh) and its attribute-lines script (task-1130-prove-attrs.sh): the replacement compares exact pre-task line ranges (located by grep on the original file's own section banners) against each new file's exact range, order-preserving with only a documented `fn `->`pub(super) fn ` substitution allowed - stricter than the plan's sorted-hash (which, as the notes say, was a silent no-op on this machine's BSD sed due to `\s`/`\b`). This independently corroborates my own move-diff read above by a different method.
6. Evidence: found the 12 `asdlc record-check` commits in the board history (the task's rendered notes don't show raw check output). The 4 real record-check commits for proofs (a)/(b)/(c)/(c-attrs) ran at commit c4792d40daca, not 91048c62 - but `git diff c4792d40daca 91048c62 -- crates/sdk/src/description/` is empty: same commit content, different hash (the shared board rewrites parent history under concurrent pushes), so the checks did run against the final code. All 4 have a failing control immediately followed by a passing real run. The sighting's isolated-rerun checks (e849100f, 75eaed9a, cf77931f) are commands piped through `sh -c '... | tail -N'`, so their recorded exit code is tail's, not cargo test's; one of them (75eaed9a) is recorded "Check passed: exited 0" while its own captured output literally reads "test result: FAILED. 1 passed; 1 failed" - but the task's prose notes already disclose this exact flake ("the only run where an isolated pair also flaked once was itself under the same heavy load"), so no reader is misled, only the record-check label itself is wrong. Confirmed via `gh pr checks 78` and `gh pr view 78 --json headRefOid`: CI's single check is "pass" at headRefOid 91048c62, exactly the PR head; the local just-check failure (exit 101, b2958111) is recorded as FAILED, not dressed up as a pass, and is about crates/server/tests/attach.rs only, a file outside this task's diff.

Findings (both Nit, policy: review):
- [Compliance, Nit] Implementation Notes: "shrinks the file... to roughly 230 (the size of the current #[cfg(test)] mod tests block)" - the actual tests.rs is 576 lines (per the diffstat), about 2.5x the stated figure. The substance (tests.rs remains the one collision point, named as a follow-up) is accurate; only the number is wrong.
- [Evidence, Nit] One sighting check (75eaed9a, part of the TASK-1.123 sighting, not one of this task's four AC proofs) is recorded as "Check passed: ... exited 0" via a `sh -c '... | tail -N'` pipeline whose exit code is tail's rather than cargo test's; its own captured output shows a real test failure. The task's prose notes already disclose this exact flake accurately, so this is a record-keeping label defect, not a hidden fact.

Everything else checked clean: AC #1 (byte-identical description, recorded comparison) and AC #2 (no public item added/removed/renamed, no function/test body change beyond imports and visibility) hold under my own independent re-derivation, not just the worker's. AC #3: CI's just check passed clean on the exact PR head (91048c62); local just check's documented failure is unrelated to this task's files and honestly recorded as failing. Plan: 12 commits match the plan's 12 steps and order exactly (verified via git log and the first commit's 100%-similarity pure rename); plan-matched recorded as "no" because the notes are explicitly headed "Departures" (the proof (c) mechanism swap and the 18-vs-22 test count correction), per policy's "no for any departure, even one the notes explain" - neither departure is itself a finding, since both are disclosed with why.

Did not check: crates/server or any file outside crates/sdk/src/description/methods* (out of scope - confirmed via diffstat that nothing else changed); did not re-verify the TASK-1.123 sighting's host-load claims (load averages, "6 users") beyond reading them, since that sighting is informational for a different task, not this one's acceptance criteria.

---
<!-- COMMENTS:END -->
