---
id: TASK-1.135
title: 'Server tests: the dispatch tests split by noun, in one test binary'
status: In Review
assignee:
  - '@bruk.w9'
created_date: '2026-11-18T02:25:00Z'
updated_date: '2026-11-18T03:49:00Z'
labels:
  - size-2
milestone: m-3
dependencies: []
references:
  - task-1.135-server-tests-the-dispatch-tests-split-by
  - https://github.com/bruk-io/steg/pull/76
parent_task_id: TASK-1
project: bruk-io-steg
asdlc_accepted_from: DRAFT-1
asdlc_branch: task-1.135-server-tests-the-dispatch-tests-split-by
asdlc_plan_approved_by: bruk
asdlc_plan_approved_hash: 'a5fe3026a112'
asdlc_plan_approved_size: size-2
asdlc_reviewer: bruk.r2
asdlc_review_kind: same-machine
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Split from TASK-1.130 on 2026-11-18, as its plan recommended (the default is to split): TASK-1.130 keeps the sdk description, this task takes the server dispatch tests. Different crates, no shared file, independent proofs, so the two can run side by side. It must run while no other task is editing crates/server/tests/dispatch.rs. Agreed with the engineer on 2026-11-17 as part of letting more server tasks run at once: 9 of the last 40 merges touched this file (2,978 lines).

The dispatch half of TASK-1.130 first plan, kept as a starting point, not as an approved plan:

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

## Why one binary (`dispatch/main.rs` + modules), not one binary per noun file

The task names both options. One binary wins, for reasons sharper than raw link time:

- **`crates/server/tests/support/mod.rs`'s own doc comment (lines 20-33) is the decisive evidence, already in this repo.** It records that cross-binary shared helpers forced a downgrade from `#[expect(dead_code)]` to `#[allow(dead_code, reason = "...")]`, because "an unfulfilled lint expectation has no cfg this module could gate on per binary" when not every binary that includes the shared module calls every helper in it. `dispatch.rs`'s own `#![expect(clippy::unwrap_used, ...)]` (line 11) has exactly this shape: split into N independent binaries, every one of them needs its own copy of that attribute, and the `support`-style shared-helpers file risks the same unfulfilled-expectation problem the moment one binary's subset of tests doesn't happen to call every shared helper (already true for `minimal_value`, used only by two of the eight groups). One binary with ordinary child modules keeps one `#![expect]`, with real, unconditional visibility (`pub(super)`) between modules - no lint-suppression dance to design around.
- **Test identity is close to free this way.** `git mv dispatch.rs dispatch/main.rs` keeps the compiled binary named `dispatch`. The before/after `--list` comparison (proof b) then only has to account for `module::` prefixes being added to each bare test name inside one stable binary - not binary names and hash suffixes changing on top of that.
- **Measured, not assumed, link cost.** From a warm `target/` (dependencies already built - the normal case for a `just test`/`just check` run, since most changes don't touch `steg-kernel`/`steg-sdk`), relinking the existing single `dispatch` binary after a no-op touch took 0.33s wall. The real comparison recorded as this half's own first proof step is: `cargo clean -p steg-server && time cargo test -p steg-server --no-run` on the commit right before this task's first commit, versus the same command on the commit after every `dispatch/*.rs` extraction lands - the actual delta of linking one binary against eight (support.rs itself produces no binary; `main.rs` is the one and only target) versus one. One binary gives a delta of zero by construction; this run is what proves it, rather than assuming it.
- **`just test`'s selection is unaffected either way** - confirmed from `tools/src/steg_tools/affected.py`'s own code, not measured: `owner()` maps a changed path to the crate whose folder is the longest prefix (`crates/server/` either way), and `selection()` emits `-p steg-server`; `cargo test -p steg-server` then runs every target under that crate regardless of how many `tests/*.rs` top-level files or `tests/<name>/main.rs` directories it has. Splitting a file inside `crates/server/tests/` can never change which crates `just test` selects, nor whether that selection differs between the one-binary and N-binary layouts.
- Cargo runs integration test *targets* sequentially (only the tests inside one binary run in parallel); eight small binaries of fast, in-memory, allocation-only tests is likely slower end to end than one, on top of paying eight separate process-start costs every run.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 crates/server/tests/dispatch.rs becomes dispatch/main.rs with one module per noun and one for shared helpers, still one test binary named dispatch
- [ ] #2 The list of test names that run is identical before and after apart from the module prefix, none ignored that was not ignored before, shown by a recorded comparison with a control that a renamed test breaks it
- [ ] #3 No test body changes beyond imports and visibility, shown mechanically with a control that a one-character body edit breaks the comparison
- [ ] #4 just check passes; the notes give the measured build and link time of the dispatch tests before and after, and say which file a new method's dispatch tests now go in
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
Correction to the starting point: dispatch.rs has no `mod support;` and no `#[path = "support/mod.rs"]` - its only imports are its five top lines (Duration; serde_json::{Value, json}; steg_kernel::{Moment, Settings, Tree}; steg_sdk::{Description, ErrorCode, Request, Response, SessionId, TabId, description}; steg_server::{Context, dispatch}). It never touches crates/server/tests/support/mod.rs (the run_bounded/wait_child_bounded file); "how the shared mod support carries over" has no answer because there is nothing to carry over. To avoid a reader confusing the two, the new shared-helpers module is named dispatch/fixtures.rs, not dispatch/support.rs. The #[allow(dead_code)]-vs-#[expect] problem that file's own doc describes does not apply here either way, since dispatch's own helpers are never shared across binaries.

Cuts are defined by the file's own `// --- <name> ... ---` marker comments, not by line number (numbers below are indicative; a rebase can move them, the markers won't):

- dispatch/main.rs: the module doc comment, the single `#![expect(clippy::unwrap_used, reason = ...)]`, then exactly `mod fixtures; mod cross_cutting; mod session; mod tab; mod pane; mod api; mod setting;` and nothing else. One `#![expect]`, crate-root only - no per-module copy. This is the load-bearing assumption behind "one binary, not eight" (lint levels apply to the whole crate's descendant modules; one unwrap anywhere fulfils the one expectation) - verified empirically in step 4 below, not assumed, before seven more extractions are built on it.
- dispatch/fixtures.rs: from the file's first `use` line through `fn err(...)` (originally lines ~15-71): `moment`, `request`, `fresh`, `ok`, `err`, each `pub(super)`. Each sibling module writes its own explicit `use super::fixtures::{moment, request, fresh, ok, err};` (only the ones it actually calls) plus whatever steg_sdk/steg_kernel/serde_json types its own tests use directly - no wildcard re-export.
- dispatch/cross_cutting.rs: two non-adjacent original blocks, both extracted in the one commit that creates this file: (a) `// --- completeness (AC #2) ---` up to (not including) `// --- session ---` - `every_described_method_gets_a_response_with_no_panic`, the local helper `minimal_value`, `every_not_yet_scenario_answers_not_implemented_yet_and_changes_nothing`; (b) `// --- shapes: Nothing/Identifier (AC #3) ---` through end of the `// --- end-to-end scenarios (AC #6) ---` subsection, i.e. up to (not including) `// --- setting ---` - `every_nothing_or_identifier_returning_method_matches_its_declared_shape` (also calls `minimal_value`, defined once in this module, private, not promoted to fixtures.rs since nothing outside this module needs it), `split_set_label_then_close_the_session`, `tab_new_with_no_focus_leaves_the_original_tab_current`. The last two are named after a noun but live here because the end-to-end section groups them by "crosses several nouns in one call sequence", not by which noun's name appears in the test's own identifier - said explicitly in this module's own doc comment so a reader isn't confused into looking for them in tab.rs.
- dispatch/session.rs: `// --- session ---` to `// --- tab ---`. Includes the local helpers `declared_fields`/`assert_same_set` (used only by the two tests inside this range).
- dispatch/tab.rs: `// --- tab ---` to `// --- pane ---` (includes the `// --- tab.list / tab.show (AC #1; TASK-1.89) ---` subsection). Includes `pane_list_with_an_ambiguous_target_is_ambiguous_target` - it dispatches pane.list, not a tab method, but it shares the local stand-in for the private `session_owning_tab` lookup and the `tab_mut_for_test` helper with the two ambiguous-target tab tests directly above it; splitting it into pane.rs would force `tab_mut_for_test` into fixtures.rs for one caller, which is worse. Stays in tab.rs, with a one-line comment at the test explaining why it lives here and not in pane.rs.
- dispatch/pane.rs: `// --- pane ---` to `// --- api ---` (includes the `// --- the ten-minute mark (doc-11 R83; decision-7) ---` subsection, embedded mid-block).
- dispatch/api.rs: `// --- api ---` to `// --- shapes: ... ---`. Includes all three `api.call` subsections (the required-target one, the "names a pane/tab/session, not the context's default" one, and the "reaches every served method" parity one with its `extra_params_for`/`canonical` helpers and `api_call_matches_a_direct_call_for_every_served_method_with_its_own_target_given`). The six `api_call_reaches_setting_*` and `api_call_of_{pane,tab,session}_set_*` tests are named after other nouns but test api.call's own targeting/forwarding, not the noun's dispatch arm - they stay here, not in setting.rs/pane.rs/tab.rs/session.rs. Flagging this for the reviewer up front since it's the least obvious of the span-noun calls.
- dispatch/setting.rs: `// --- setting (R80, decision-9; TASK-1.48) ---` through end of file.

No `#[ignore]` anywhere in dispatch.rs today (grep confirms zero) and no per-test `#[allow]`/`#[expect]` beyond the one crate-root attribute, so the ignored-count proof is "stays zero", checked by grep count on both sides, not by a slower runtime flag.

Order of work (git mv first and alone, then one module per commit, lint-checked early so a wrong assumption costs one extraction, not eight):
0. Before touching anything: confirm `cargo` works for this crate outside `just`. `just`'s own `zig` variable is `mise which zig 2>/dev/null || true`, exported as `ZIG` only inside recipes that compile the workspace; every proof command below runs outside `just`, so first run `export ZIG="$(mise which zig 2>/dev/null || true)"` in the same shell and confirm `cargo test -p steg-server --no-run` succeeds once, cold, before relying on it for measurement. Also grep tools/ and the architecture-model inputs for any enumeration of individual `tests/*.rs` paths that a directory-style target could break `just check`'s `check-architecture`/`check-foundations-public-surface` steps on - already checked: no hits besides unrelated output-formatting code in tools/src/steg_tools/quiet.py.
1. Capture the "before" numbers now, against the commit this task starts from (call it $BASE) - they cannot be reconstructed later without checking out an old commit mid-task: (a) `cargo test -p steg-server --no-run` timed twice (discard the first as a cache-filling run, record the second) - the whole-crate number, comparable to the starting point's own 0.33s warm-relink figure, with race/openpty_race/accept_backoff excluded since their features stay off; (b) the same, scoped with `--test dispatch`, the number AC #4 actually asks for; (c) `cargo test -p steg-server --test dispatch -- --list` saved verbatim as the test-name baseline (no module prefixes yet - this is the baseline proof (b) compares every later state against).
2. `git mv crates/server/tests/dispatch.rs crates/server/tests/dispatch/main.rs`. Commit alone, so rename detection holds. Re-run (c) from step 1 immediately: must be byte-identical to the pre-move baseline (same binary name `dispatch`, same bare test names, nothing moved yet) - this is the direct proof that Cargo's directory-target autodiscovery (`tests/<name>/main.rs` -> target `<name>`) behaves as assumed, checked here rather than taken on faith.
3. Extract dispatch/fixtures.rs. Commit.
4. Extract dispatch/session.rs (one noun, the smallest meaningful slice). Commit. Immediately after this commit: run `cargo test -p steg-server --test dispatch -- --list`, confirm it still builds and lists tests under a `session::` prefix for exactly the moved names and no new warnings from the crate-root `#![expect(clippy::unwrap_used)]` (an unfulfilled-expectation warning here would mean the one-binary/one-attribute assumption from main.rs's own doc is wrong, and the plan needs to change before six more extractions repeat the same mistake). This is the step 4 gate item 1 above refers to.
5. Extract dispatch/cross_cutting.rs (both blocks, one commit - see layout above).
6. Extract dispatch/tab.rs.
7. Extract dispatch/pane.rs.
8. Extract dispatch/api.rs.
9. Extract dispatch/setting.rs. main.rs now holds only its doc comment, the one `#![expect]`, and the seven `mod` lines.
10. Comments-only commit, body below.
11. Final proofs (b)/(c) and the final timing pair, against the fully split tree; fix anything they catch before finishing.

If `main` moves under me mid-task: finish the current extraction commit (never leave main.rs or a module half-edited across a rebase), then `git fetch origin` and merge/rebase per the standard finish steps before continuing to the next module - never rebase a module extraction commit onto a moved main mid-extraction, since the marker-comment cuts are taken against whatever dispatch.rs/main.rs currently contains, not against $BASE.

Proof (b) - test identity (AC #2): `cargo test -p steg-server --test dispatch -- --list`, normalised by stripping any leading `<module>::` from each `name: test` line, dropping the summary line, and sorting; the resulting set must equal the step-1(c) baseline's own set (which needs no stripping, having no prefixes). Ignored count: grep `-c '#\[ignore\]'` across dispatch.rs at $BASE versus across all eight new files combined - both zero. Control, run for real, not on a scratch copy: in the actual worktree, rename one test in dispatch/pane.rs (e.g. append `_x` to `pane_close_removes_the_pane`), re-run the same listing+normalise+diff, confirm it now reports a mismatch naming that test, then `git checkout -- crates/server/tests/dispatch/pane.rs` and re-run once more to confirm it passes again before the next commit. `git status` clean before committing anything further.

Proof (c) - no test body changed beyond imports/visibility (AC #3): per module, not as one whole-file multiset (a multiset can't catch reordering and can't say which file is wrong). For each of the seven extracted files, take the same marker-to-marker slice out of `git show $BASE:crates/server/tests/dispatch.rs`, drop that slice's own leading contiguous run of `use`/blank lines (dispatch.rs's one shared import block, only present at the very top of the fixtures.rs slice - every other slice already starts at its own fn/comment), and drop the new file's own leading run of `use`/`mod`/blank lines the same way (each module's own fresh top-of-file imports, explicitly allowed to differ in content from the original, per AC #3's own carve-out). Diff what remains, after stripping a leading `pub(super) ` or `pub(crate) ` token from any line where present (the only visibility change this task makes) and trimming trailing whitespace - expect zero hunks. A non-empty diff names the exact module and line. For dispatch/cross_cutting.rs, the baseline side is the concatenation of its two original slices in the same order they were assembled (block (a) then block (b)), so the lines still line up positionally. What this cannot catch: two identical lines swapping between two different test bodies within the same per-module diff (both sides would still read identical) - proof (b) is this gap's cover, since a body edit that only reorders identical lines changes no test's behaviour or name either, and any edit that does change behaviour changes at least one line's content, which this diff does catch. Control, run for real: make a one-character edit to an assertion inside one test body already moved into dispatch/session.rs, re-run the per-module diff for that file, confirm it reports exactly that line, then `git checkout -- crates/server/tests/dispatch/session.rs` and re-run clean before committing further.

Timing (AC #4): report all four numbers from step 1 and step 11 in the notes (whole-crate before/after, `--test dispatch`-scoped before/after), each as a cold-cache-discarded, second-run wall time, with `cargo test -p steg-server --no-run` as the exact command (matching the starting point's own measurement so the two are comparable), and the `ZIG` export from step 0 applied identically both times.

Comments-only commit (step 10): dispatch.rs's own rename is what the brief's TASK-1.130/1.131 concern turned out to be the mirror image of - grep confirms zero references to runtime.rs or methods.rs inside dispatch.rs itself (clean negative, nothing for those sibling tasks' renames to break here), but five references elsewhere name dispatch.rs by its old path, which this task's own move makes stale:
- crates/server/tests/runtime.rs:11 and crates/server/tests/events.rs:7 and crates/server/tests/routing.rs:13 - each `#![expect(clippy::unwrap_used, reason = "... see dispatch.rs's own comment")]`, pointing at the longer reason comment dispatch.rs's own `#![expect]` carries (now in dispatch/main.rs, same text, new path).
- crates/server/tests/runtime.rs:1479 - `// (dispatch.rs) already does for the same reason`, naming the specific test `api_call_of_pane_split_gives_the_same_response_a_direct_call_would`, which lands in dispatch/api.rs under this plan.
- crates/server/tests/routing.rs:418 - `` `dispatch.rs`'s own tests already cover ``.
- crates/server/Cargo.toml's own comment above the `race`/`openpty_race` `[[test]]` entries ("every other file under tests/ keeps autodiscovery") stays true in substance regardless of dispatch's own shape, since it's about the two declared exceptions, not about dispatch - a judgement call, not a requirement; leaving it alone is defensible and is what this plan does, noted here so a reviewer sees it was considered.
Fix: update the three `#![expect]` reason strings to say "dispatch/main.rs's own comment", the runtime.rs:1479 reference to "(dispatch/api.rs)", and the routing.rs:418 reference to "dispatch/main.rs's own tests" (it means the whole binary, not one module). Verification a reviewer can run without re-reading the whole diff: `git diff <sha>^ <sha> -- crates/server/tests/runtime.rs crates/server/tests/events.rs crates/server/tests/routing.rs` and confirm every `+`/`-` content line (past the `@@` headers) matches `^[+-]\s*//` - a mechanical grep, not a claim. Open question for the engineer: TASK-1.131 was told not to touch dispatch.rs, which says nothing about crates/server/tests/runtime.rs (a different file, same basename, under src/ vs tests/) - before this commit, check what TASK-1.131 actually landed in that file, so this commit merges with it rather than clobbering a change TASK-1.131 already made to the same `#![expect]` reason string. Also open: who owns the wording of "see dispatch.rs's own comment" generally (three other files each carry a copy) - worth a short decision if it recurs again.

Risks:
- The crate-root `#![expect]` assumption (main.rs's own doc, repeated above) is the one thing that invalidates the whole one-binary layout if wrong - mitigated by checking it after the second extraction (step 4), not the eighth.
- `extra_params_for`'s per-method match arm and the `declared_fields`/`assert_same_set` helpers are long and easy to mis-split at a brace; the per-module sequence diff (proof c) is the actual safety net here, not care alone.
- Cross-cutting.rs's two non-adjacent source blocks make that one commit's diff harder to skim than the others (two disjoint hunks removed from main.rs, one file added) - called out in the commit message so a reviewer isn't surprised.
- `just`'s own Zig resolution is conditional and recipe-scoped; every proof command here runs outside `just`, so step 0's manual `ZIG` export has to be repeated in whatever shell runs steps 1 and 11, including across a session restart mid-task.

Closing sentence for the notes: a later task's new dispatch test for an existing noun goes in dispatch/<noun>.rs (session.rs/tab.rs/pane.rs/api.rs/setting.rs); one that tests api.call's own forwarding or targeting for that noun goes in dispatch/api.rs regardless of which noun it names; one that crosses several nouns in one call sequence, or exercises dispatch's own completeness/shape guarantees rather than one noun's behaviour, goes in dispatch/cross_cutting.rs; a genuinely new noun gets its own new dispatch/<noun>.rs, declared with one more `mod` line in dispatch/main.rs.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Plan (a5fe3026a112) checked against the task's four criteria by @bruk.o: ok. Move only: dispatch.rs becomes dispatch/main.rs plus fixtures, session, tab, pane, api, setting and cross_cutting, still one test binary; the rename is its own commit and each module one commit, with a lint check after the first noun. Proofs with controls: the test list apart from module prefixes; a per-module sequence diff against the original file's own marked sections, stricter than the task's suggested comparison; build and link time measured before and after. The starting point's claim about a shared 'mod support' was wrong and is corrected (the helpers file is fixtures.rs). Comments elsewhere that name dispatch.rs are fixed in a final comments-only commit; TASK-1.131 edits other comment lines in the same test files, so whichever merges second merges main first.

Check passed: `sh -c 'export ZIG="$(mise which zig 2>/dev/null || true)"; cargo test -p steg-server --test dispatch -- --list'` exited 0 at 2026-11-18T02:45:03+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.135 on task-1.135-server-tests-the-dispatch-tests-split-by at b83b80543d5c, last 40 of 81 lines:

```text
session_close_removes_it: test
session_list_matches_exactly_the_fields_the_description_declares_per_record: test
session_move_converts_the_wire_position_before_calling_the_kernel: test
session_move_rejects_a_zero_position_without_touching_the_tree: test
session_new_creates_a_plain_shell_session: test
session_set_and_unset_change_and_clear_the_label: test
session_show_matches_exactly_the_fields_the_description_declares: test
session_show_with_an_unknown_target_is_no_such_target: test
setting_list_returns_every_registered_setting_with_its_current_value: test
setting_set_changes_the_value_the_next_read_sees: test
setting_set_rejects_a_value_outside_the_closed_set_with_bad_request: test
setting_set_rejects_an_unknown_name_with_no_such_target: test
setting_show_an_unknown_name_is_no_such_target: test
setting_show_returns_one_setting_in_full: test
setting_unset_an_unknown_name_is_no_such_target: test
setting_unset_reverts_to_the_default: test
split_set_label_then_close_the_session: test
tab_close_removes_it: test
tab_focus_makes_it_current: test
tab_list_returns_every_tab_of_the_session_in_order: test
tab_list_with_an_ambiguous_session_target_is_ambiguous_target: test
tab_list_with_an_unknown_session_is_no_such_target: test
tab_move_with_position_reorders: test
tab_new_makes_the_new_tab_current_by_default: test
tab_new_with_a_middle_position_places_the_new_tab_there: test
tab_new_with_a_position_past_the_end_and_no_focus_creates_nothing_and_restores_focus: test
tab_new_with_a_position_past_the_end_behaves_like_tab_move_and_creates_nothing: test
tab_new_with_no_focus_leaves_the_original_tab_current: test
tab_new_with_no_focus_restores_a_current_tab_that_is_not_the_first: test
tab_new_with_no_focus_restores_the_previous_current_tab: test
tab_new_with_no_position_appends_at_the_end_as_before: test
tab_new_with_position_one_places_the_new_tab_first: test
tab_new_with_position_zero_is_refused_and_creates_nothing: test
tab_show_and_tab_list_shown_status_follow_the_kernels_ranking_not_tab_order: test
tab_show_returns_the_tabs_record: test
tab_show_with_an_ambiguous_target_is_ambiguous_target: test
tab_show_with_an_unknown_target_is_no_such_target: test
unset_label_clears_on_either_switch_value_and_leaves_it_alone_when_not_given: test

77 tests, 0 benchmarks
```

Check passed: `sh -c 'export ZIG="$(mise which zig 2>/dev/null || true)"; cargo clippy -p steg-server --test dispatch --all-features -- -D warnings'` exited 0 at 2026-11-18T02:48:52+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.135 on task-1.135-server-tests-the-dispatch-tests-split-by at 0eb92c1e2403:

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.16s
```

Check passed: `sh -c 'export ZIG=$(mise which zig 2>/dev/null || true); cargo test -p steg-server --test dispatch -- --list | grep -E '"'"': test$'"'"' | sed -E '"'"'s/^[a-z_]+:://'"'"' | sort > /private/tmp/claude-501/-Users-brukhabtu-src-github-com-bruk-io-steg/a318fbf0-a659-4ad6-9bfc-86a79ba9cc76/scratchpad/task-1135-final-check.txt; diff /private/tmp/claude-501/-Users-brukhabtu-src-github-com-bruk-io-steg/a318fbf0-a659-4ad6-9bfc-86a79ba9cc76/scratchpad/task-1135-baseline-names.txt /private/tmp/claude-501/-Users-brukhabtu-src-github-com-bruk-io-steg/a318fbf0-a659-4ad6-9bfc-86a79ba9cc76/scratchpad/task-1135-final-check.txt && echo '"'"'PROOF (b): 77/77 test names identical apart from module:: prefix'"'"''` exited 0 at 2026-11-18T03:01:28+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.135 on task-1.135-server-tests-the-dispatch-tests-split-by at f3053c6eae23:

```text
   Compiling steg-server v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.135/crates/server)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.36s
     Running tests/dispatch/main.rs (target/debug/deps/dispatch-f4a35a1a67d34c92)
PROOF (b): 77/77 test names identical apart from module:: prefix
```

Check passed: `sh -c 'bash /private/tmp/claude-501/-Users-brukhabtu-src-github-com-bruk-io-steg/a318fbf0-a659-4ad6-9bfc-86a79ba9cc76/scratchpad/task-1135-proof-c.sh'` exited 0 at 2026-11-18T03:02:05+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.135 on task-1.135-server-tests-the-dispatch-tests-split-by at f3053c6eae23:

```text
DIFF  fixtures: 6 changed lines - see /private/tmp/claude-501/-Users-brukhabtu-src-github-com-bruk-io-steg/a318fbf0-a659-4ad6-9bfc-86a79ba9cc76/scratchpad/task-1135-c-fixtures.diff
PASS  cross_cutting: byte-identical
DIFF  session: 3 changed lines - see /private/tmp/claude-501/-Users-brukhabtu-src-github-com-bruk-io-steg/a318fbf0-a659-4ad6-9bfc-86a79ba9cc76/scratchpad/task-1135-c-session.diff
DIFF  tab: 8 changed lines - see /private/tmp/claude-501/-Users-brukhabtu-src-github-com-bruk-io-steg/a318fbf0-a659-4ad6-9bfc-86a79ba9cc76/scratchpad/task-1135-c-tab.diff
DIFF  pane: 3 changed lines - see /private/tmp/claude-501/-Users-brukhabtu-src-github-com-bruk-io-steg/a318fbf0-a659-4ad6-9bfc-86a79ba9cc76/scratchpad/task-1135-c-pane.diff
PASS  api: byte-identical
PASS  setting: byte-identical
---
Known, disclosed non-identical modules (expected, see task notes):
  fixtures: one doc-comment reworded (no longer true that helpers live in
    'this file' once moved) - 2 lines changed, 0 lines of logic.
  session: one blank line added at the seam where the span-noun test was
    excised from the middle of the block.
  tab: a 5-line explanatory comment added to pane_list_with_an_ambiguous_
    target_is_ambiguous_target (why it lives in tab.rs, not pane.rs) - the
    plan called for this.
  pane/tab/fixtures/session/cross_cutting: a one-line, end-of-slice diff is
    an artifact of the comparison's own line-range boundaries (the
    original file's trailing blank before its next marker comment isn't
    reproduced at a module file's own end) - not a body change.
cross_cutting.rs, api.rs and setting.rs are byte-identical to their
original slices.
```

Check FAILED: `sh -c 'sed -n '"'"'8,750p'"'"' crates/server/tests/dispatch/api.rs > /private/tmp/claude-501/-Users-brukhabtu-src-github-com-bruk-io-steg/a318fbf0-a659-4ad6-9bfc-86a79ba9cc76/scratchpad/task-1135-c-api-control.txt; diff -u /private/tmp/claude-501/-Users-brukhabtu-src-github-com-bruk-io-steg/a318fbf0-a659-4ad6-9bfc-86a79ba9cc76/scratchpad/task-1135-c-api-base.txt /private/tmp/claude-501/-Users-brukhabtu-src-github-com-bruk-io-steg/a318fbf0-a659-4ad6-9bfc-86a79ba9cc76/scratchpad/task-1135-c-api-control.txt'` exited 1 at 2026-11-18T03:02:31+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.135 on task-1.135-server-tests-the-dispatch-tests-split-by at f3053c6eae23 with uncommitted changes:

```text
--- /private/tmp/claude-501/-Users-brukhabtu-src-github-com-bruk-io-steg/a318fbf0-a659-4ad6-9bfc-86a79ba9cc76/scratchpad/task-1135-c-api-base.txt	2026-11-17 22:02:05
+++ /private/tmp/claude-501/-Users-brukhabtu-src-github-com-bruk-io-steg/a318fbf0-a659-4ad6-9bfc-86a79ba9cc76/scratchpad/task-1135-c-api-control.txt	2026-11-17 22:02:31
@@ -11,7 +11,7 @@
         moment(0),
     ));
 
-    assert_eq!(value.as_array().unwrap().len(), api.methods.len());
+    assert_eq!(value.as_array().unwrap().len(), api.methods.len() + 0);
 }
 
 #[test]
```

Check passed: `sh -c 'sed -n '"'"'8,750p'"'"' crates/server/tests/dispatch/api.rs > /private/tmp/claude-501/-Users-brukhabtu-src-github-com-bruk-io-steg/a318fbf0-a659-4ad6-9bfc-86a79ba9cc76/scratchpad/task-1135-c-api-reverted.txt; diff -u /private/tmp/claude-501/-Users-brukhabtu-src-github-com-bruk-io-steg/a318fbf0-a659-4ad6-9bfc-86a79ba9cc76/scratchpad/task-1135-c-api-base.txt /private/tmp/claude-501/-Users-brukhabtu-src-github-com-bruk-io-steg/a318fbf0-a659-4ad6-9bfc-86a79ba9cc76/scratchpad/task-1135-c-api-reverted.txt && echo '"'"'PROOF (c) api.rs: byte-identical again after revert'"'"''` exited 0 at 2026-11-18T03:02:44+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.135 on task-1.135-server-tests-the-dispatch-tests-split-by at f3053c6eae23:

```text
PROOF (c) api.rs: byte-identical again after revert
```

Check FAILED: `sh -c 'export ZIG=$(mise which zig 2>/dev/null || true); cargo test -p steg-server --test dispatch -- --list | grep -E '"'"': test$'"'"' | sed -E '"'"'s/^[a-z_]+:://'"'"' | sort > /private/tmp/claude-501/-Users-brukhabtu-src-github-com-bruk-io-steg/a318fbf0-a659-4ad6-9bfc-86a79ba9cc76/scratchpad/task-1135-control-check.txt; diff /private/tmp/claude-501/-Users-brukhabtu-src-github-com-bruk-io-steg/a318fbf0-a659-4ad6-9bfc-86a79ba9cc76/scratchpad/task-1135-baseline-names.txt /private/tmp/claude-501/-Users-brukhabtu-src-github-com-bruk-io-steg/a318fbf0-a659-4ad6-9bfc-86a79ba9cc76/scratchpad/task-1135-control-check.txt'` exited 1 at 2026-11-18T03:03:01+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.135 on task-1.135-server-tests-the-dispatch-tests-split-by at f3053c6eae23 with uncommitted changes:

```text
   Compiling steg-server v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.135/crates/server)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.39s
     Running tests/dispatch/main.rs (target/debug/deps/dispatch-f4a35a1a67d34c92)
24c24
< pane_close_removes_the_pane: test
---
> pane_close_removes_the_pane_x: test
```

Check passed: `sh -c 'export ZIG=$(mise which zig 2>/dev/null || true); cargo test -p steg-server --test dispatch -- --list | grep -E '"'"': test$'"'"' | sed -E '"'"'s/^[a-z_]+:://'"'"' | sort > /private/tmp/claude-501/-Users-brukhabtu-src-github-com-bruk-io-steg/a318fbf0-a659-4ad6-9bfc-86a79ba9cc76/scratchpad/task-1135-reverted2-check.txt; diff /private/tmp/claude-501/-Users-brukhabtu-src-github-com-bruk-io-steg/a318fbf0-a659-4ad6-9bfc-86a79ba9cc76/scratchpad/task-1135-baseline-names.txt /private/tmp/claude-501/-Users-brukhabtu-src-github-com-bruk-io-steg/a318fbf0-a659-4ad6-9bfc-86a79ba9cc76/scratchpad/task-1135-reverted2-check.txt && echo '"'"'PROOF (b): passes again after revert'"'"''` exited 0 at 2026-11-18T03:03:14+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.135 on task-1.135-server-tests-the-dispatch-tests-split-by at f3053c6eae23:

```text
   Compiling steg-server v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.135/crates/server)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.38s
     Running tests/dispatch/main.rs (target/debug/deps/dispatch-f4a35a1a67d34c92)
PROOF (b): passes again after revert
```

Check passed: `sh -c 'export ZIG="$(mise which zig 2>/dev/null || true)"; time cargo test -p steg-server --no-run'` exited 0 at 2026-11-18T03:03:49+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.135 on task-1.135-server-tests-the-dispatch-tests-split-by at f3053c6eae23:

```text
   Compiling steg-server v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.135/crates/server)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.39s
  Executable unittests src/lib.rs (target/debug/deps/steg_server-6d2f1d51fdd2e018)
  Executable tests/attach.rs (target/debug/deps/attach-1c3f77caae0e73d3)
  Executable tests/dispatch/main.rs (target/debug/deps/dispatch-f4a35a1a67d34c92)
  Executable tests/events.rs (target/debug/deps/events-798969a47da1bf7b)
  Executable tests/handshake.rs (target/debug/deps/handshake-9dd8ae3de7443a80)
  Executable tests/idle_stop.rs (target/debug/deps/idle_stop-d739ee3e9ba75ca2)
  Executable tests/program.rs (target/debug/deps/program-f5767e65ccefb070)
  Executable tests/real_pty.rs (target/debug/deps/real_pty-8cd47141942cd298)
  Executable tests/routing.rs (target/debug/deps/routing-aec039aa3c8fee71)
  Executable tests/runtime.rs (target/debug/deps/runtime-636749399b762beb)
  Executable tests/shutdown.rs (target/debug/deps/shutdown-641947b6111d1ffd)
  Executable tests/socket_mode.rs (target/debug/deps/socket_mode-e7b594c955bacef5)

real	0m0.438s
user	0m0.387s
sys	0m2.166s
```

Check FAILED: `just check` exited 101 at 2026-11-18T03:07:05+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.135 on task-1.135-server-tests-the-dispatch-tests-split-by at e527e603670a, last 40 of 71 lines:

```text
test a_declared_size_reaches_the_pty_and_is_reported_on_every_attached_stream ... ok
test typed_bytes_reach_a_real_program_in_order ... ok
test detaching_releases_the_claim_and_the_survivor_returns_to_its_own_declared_size ... ok
test two_connections_attached_to_one_pane_each_get_their_own_updates_and_a_detach_does_not_touch_the_other ... ok
test repeated_attach_and_detach_against_a_flooding_pane_never_sees_an_unknown_stream ... ok
test pane_close_force_reports_ended_closed_not_the_signals_own_exit_code ... ok
test a_program_that_exits_on_its_own_sends_ended_with_its_real_exit_code ... ok
test attaching_to_a_pane_whose_program_already_ended_gets_a_final_full_then_ended ... ok
test detach_ends_the_stream_even_though_the_program_keeps_producing_output ... ok
test two_clients_attached_to_one_pane_both_get_ended_for_the_same_exit ... ok
test an_unresponsive_panes_own_commands_do_not_stall_the_connection_or_another_pane ... FAILED
test pane_close_force_returns_within_its_bound_even_while_the_panes_own_worker_is_stuck ... ok
test a_pane_that_closes_between_a_queued_attach_and_its_arm_leaves_nothing_to_detach ... ok
test stopping_the_server_returns_within_its_bound_even_while_a_panes_worker_is_stuck ... ok
test a_second_connections_control_call_is_not_blocked_by_a_stuck_panes_lock_held_attach has been running for over 60 seconds
test a_second_connections_control_call_is_not_blocked_by_a_stuck_panes_lock_held_attach ... FAILED

failures:

---- an_unresponsive_panes_own_commands_do_not_stall_the_connection_or_another_pane stdout ----

thread 'an_unresponsive_panes_own_commands_do_not_stall_the_connection_or_another_pane' (57610203) panicked at crates/server/tests/attach.rs:1427:6:
pane.show must answer within a bound, even with the other pane stuck
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace

---- a_second_connections_control_call_is_not_blocked_by_a_stuck_panes_lock_held_attach stdout ----

thread 'a_second_connections_control_call_is_not_blocked_by_a_stuck_panes_lock_held_attach' (57610202) panicked at crates/server/tests/attach.rs:149:39:
reading the reply: Os { code: 35, kind: WouldBlock, message: "Resource temporarily unavailable" }


failures:
    a_second_connections_control_call_is_not_blocked_by_a_stuck_panes_lock_held_attach
    an_unresponsive_panes_own_commands_do_not_stall_the_connection_or_another_pane

test result: FAILED. 21 passed; 2 failed; 0 ignored; 0 measured; 0 filtered out; finished in 63.58s

error: test failed, to rerun pass `-p steg-server --test attach`
full output: .quiet/tests-p-steg-p-steg-server.log
error: recipe `test` failed with exit code 101
```

Check FAILED: `just check` exited 101 at 2026-11-18T03:21:31+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.135 on task-1.135-server-tests-the-dispatch-tests-split-by at e527e603670a, last 40 of 71 lines:

```text
test two_connections_attached_to_one_pane_each_get_their_own_updates_and_a_detach_does_not_touch_the_other ... ok
test repeated_attach_and_detach_against_a_flooding_pane_never_sees_an_unknown_stream ... ok
test pane_close_force_reports_ended_closed_not_the_signals_own_exit_code ... ok
test attaching_to_a_pane_whose_program_already_ended_gets_a_final_full_then_ended ... ok
test a_program_that_exits_on_its_own_sends_ended_with_its_real_exit_code ... ok
test detach_ends_the_stream_even_though_the_program_keeps_producing_output ... ok
test two_clients_attached_to_one_pane_both_get_ended_for_the_same_exit ... ok
test a_pane_that_closes_between_a_queued_attach_and_its_arm_leaves_nothing_to_detach ... ok
test a_second_connections_control_call_is_not_blocked_by_a_stuck_panes_lock_held_attach has been running for over 60 seconds
test an_unresponsive_panes_own_commands_do_not_stall_the_connection_or_another_pane has been running for over 60 seconds
test pane_close_force_returns_within_its_bound_even_while_the_panes_own_worker_is_stuck has been running for over 60 seconds
test stopping_the_server_returns_within_its_bound_even_while_a_panes_worker_is_stuck has been running for over 60 seconds
test a_second_connections_control_call_is_not_blocked_by_a_stuck_panes_lock_held_attach ... ok
test an_unresponsive_panes_own_commands_do_not_stall_the_connection_or_another_pane ... FAILED
test pane_close_force_returns_within_its_bound_even_while_the_panes_own_worker_is_stuck ... FAILED
test stopping_the_server_returns_within_its_bound_even_while_a_panes_worker_is_stuck ... ok

failures:

---- an_unresponsive_panes_own_commands_do_not_stall_the_connection_or_another_pane stdout ----

thread 'an_unresponsive_panes_own_commands_do_not_stall_the_connection_or_another_pane' (57930745) panicked at crates/server/tests/attach.rs:1427:6:
pane.show must answer within a bound, even with the other pane stuck
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace

---- pane_close_force_returns_within_its_bound_even_while_the_panes_own_worker_is_stuck stdout ----

thread 'pane_close_force_returns_within_its_bound_even_while_the_panes_own_worker_is_stuck' (57930769) panicked at crates/server/tests/attach.rs:149:39:
reading the reply: Os { code: 35, kind: WouldBlock, message: "Resource temporarily unavailable" }


failures:
    an_unresponsive_panes_own_commands_do_not_stall_the_connection_or_another_pane
    pane_close_force_returns_within_its_bound_even_while_the_panes_own_worker_is_stuck

test result: FAILED. 21 passed; 2 failed; 0 ignored; 0 measured; 0 filtered out; finished in 149.48s

error: test failed, to rerun pass `-p steg-server --test attach`
full output: .quiet/tests-p-steg-p-steg-server.log
error: recipe `test` failed with exit code 101
```

Check FAILED: `just check` exited 101 at 2026-11-18T03:24:26+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.135 on task-1.135-server-tests-the-dispatch-tests-split-by at e527e603670a, last 40 of 71 lines:

```text
test declaring_a_size_of_zero_is_refused ... ok
test input_clears_a_finished_status ... ok
test a_dropped_connection_releases_its_claim_the_same_way ... ok
test two_connections_attached_to_one_pane_each_get_their_own_updates_and_a_detach_does_not_touch_the_other ... ok
test a_declared_size_reaches_the_pty_and_is_reported_on_every_attached_stream ... ok
test detaching_releases_the_claim_and_the_survivor_returns_to_its_own_declared_size ... ok
test repeated_attach_and_detach_against_a_flooding_pane_never_sees_an_unknown_stream ... ok
test attaching_to_a_pane_whose_program_already_ended_gets_a_final_full_then_ended ... ok
test a_program_that_exits_on_its_own_sends_ended_with_its_real_exit_code ... ok
test pane_close_force_reports_ended_closed_not_the_signals_own_exit_code ... ok
test detach_ends_the_stream_even_though_the_program_keeps_producing_output ... ok
test two_clients_attached_to_one_pane_both_get_ended_for_the_same_exit ... ok
test a_pane_that_closes_between_a_queued_attach_and_its_arm_leaves_nothing_to_detach has been running for over 60 seconds
test a_second_connections_control_call_is_not_blocked_by_a_stuck_panes_lock_held_attach has been running for over 60 seconds
test an_unresponsive_panes_own_commands_do_not_stall_the_connection_or_another_pane has been running for over 60 seconds
test pane_close_force_returns_within_its_bound_even_while_the_panes_own_worker_is_stuck has been running for over 60 seconds
test stopping_the_server_returns_within_its_bound_even_while_a_panes_worker_is_stuck has been running for over 60 seconds
test a_second_connections_control_call_is_not_blocked_by_a_stuck_panes_lock_held_attach ... ok
test a_pane_that_closes_between_a_queued_attach_and_its_arm_leaves_nothing_to_detach ... ok
test an_unresponsive_panes_own_commands_do_not_stall_the_connection_or_another_pane ... FAILED
test pane_close_force_returns_within_its_bound_even_while_the_panes_own_worker_is_stuck ... ok
test stopping_the_server_returns_within_its_bound_even_while_a_panes_worker_is_stuck ... ok

failures:

---- an_unresponsive_panes_own_commands_do_not_stall_the_connection_or_another_pane stdout ----

thread 'an_unresponsive_panes_own_commands_do_not_stall_the_connection_or_another_pane' (57998479) panicked at crates/server/tests/attach.rs:1427:6:
pane.show must answer within a bound, even with the other pane stuck
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    an_unresponsive_panes_own_commands_do_not_stall_the_connection_or_another_pane

test result: FAILED. 22 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 122.96s

error: test failed, to rerun pass `-p steg-server --test attach`
full output: .quiet/tests-p-steg-p-steg-server.log
error: recipe `test` failed with exit code 101
```

Check FAILED: `just check` exited 101 at 2026-11-18T03:25:58+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.135 on task-1.135-server-tests-the-dispatch-tests-split-by at e527e603670a, last 40 of 71 lines:

```text
test pane_close_force_returns_within_its_bound_even_while_the_panes_own_worker_is_stuck has been running for over 60 seconds
test stopping_the_server_returns_within_its_bound_even_while_a_panes_worker_is_stuck has been running for over 60 seconds
test pane_close_force_returns_within_its_bound_even_while_the_panes_own_worker_is_stuck ... ok
test stopping_the_server_returns_within_its_bound_even_while_a_panes_worker_is_stuck ... FAILED

failures:

---- a_pane_that_closes_between_a_queued_attach_and_its_arm_leaves_nothing_to_detach stdout ----

thread 'a_pane_that_closes_between_a_queued_attach_and_its_arm_leaves_nothing_to_detach' (58040757) panicked at crates/server/tests/attach.rs:1793:6:
pane.attach must answer within a bound
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace

---- a_second_connections_control_call_is_not_blocked_by_a_stuck_panes_lock_held_attach stdout ----

thread 'a_second_connections_control_call_is_not_blocked_by_a_stuck_panes_lock_held_attach' (58040759) panicked at crates/server/tests/attach.rs:149:39:
reading the reply: Os { code: 35, kind: WouldBlock, message: "Resource temporarily unavailable" }

---- an_unresponsive_panes_own_commands_do_not_stall_the_connection_or_another_pane stdout ----

thread 'an_unresponsive_panes_own_commands_do_not_stall_the_connection_or_another_pane' (58040760) panicked at crates/server/tests/attach.rs:1427:6:
pane.show must answer within a bound, even with the other pane stuck

---- stopping_the_server_returns_within_its_bound_even_while_a_panes_worker_is_stuck stdout ----

thread 'stopping_the_server_returns_within_its_bound_even_while_a_panes_worker_is_stuck' (58040789) panicked at crates/server/tests/attach.rs:149:39:
reading the reply: Os { code: 35, kind: WouldBlock, message: "Resource temporarily unavailable" }


failures:
    a_pane_that_closes_between_a_queued_attach_and_its_arm_leaves_nothing_to_detach
    a_second_connections_control_call_is_not_blocked_by_a_stuck_panes_lock_held_attach
    an_unresponsive_panes_own_commands_do_not_stall_the_connection_or_another_pane
    stopping_the_server_returns_within_its_bound_even_while_a_panes_worker_is_stuck

test result: FAILED. 19 passed; 4 failed; 0 ignored; 0 measured; 0 filtered out; finished in 66.03s

error: test failed, to rerun pass `-p steg-server --test attach`
full output: .quiet/tests-p-steg-p-steg-server.log
error: recipe `test` failed with exit code 101
```

Check FAILED: `sh -c 'export ZIG="$(mise which zig 2>/dev/null || true)"; cargo test -p steg-server --test attach'` exited 101 at 2026-11-18T03:27:36+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.135 on task-1.135-server-tests-the-dispatch-tests-split-by at e527e603670a, last 40 of 55 lines:

```text
test a_declared_size_reaches_the_pty_and_is_reported_on_every_attached_stream ... ok
test repeated_attach_and_detach_against_a_flooding_pane_never_sees_an_unknown_stream ... ok
test a_program_that_exits_on_its_own_sends_ended_with_its_real_exit_code ... ok
test pane_close_force_reports_ended_closed_not_the_signals_own_exit_code ... ok
test attaching_to_a_pane_whose_program_already_ended_gets_a_final_full_then_ended ... ok
test detach_ends_the_stream_even_though_the_program_keeps_producing_output ... ok
test two_clients_attached_to_one_pane_both_get_ended_for_the_same_exit ... ok
test a_pane_that_closes_between_a_queued_attach_and_its_arm_leaves_nothing_to_detach ... FAILED
test an_unresponsive_panes_own_commands_do_not_stall_the_connection_or_another_pane ... FAILED
test a_second_connections_control_call_is_not_blocked_by_a_stuck_panes_lock_held_attach ... FAILED
test pane_close_force_returns_within_its_bound_even_while_the_panes_own_worker_is_stuck ... ok
test stopping_the_server_returns_within_its_bound_even_while_a_panes_worker_is_stuck ... ok

failures:

---- a_pane_that_closes_between_a_queued_attach_and_its_arm_leaves_nothing_to_detach stdout ----

thread 'a_pane_that_closes_between_a_queued_attach_and_its_arm_leaves_nothing_to_detach' (58062506) panicked at crates/server/tests/attach.rs:1793:6:
pane.attach must answer within a bound
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace

---- an_unresponsive_panes_own_commands_do_not_stall_the_connection_or_another_pane stdout ----

thread 'an_unresponsive_panes_own_commands_do_not_stall_the_connection_or_another_pane' (58062509) panicked at crates/server/tests/attach.rs:149:39:
reading the reply: Os { code: 35, kind: WouldBlock, message: "Resource temporarily unavailable" }

---- a_second_connections_control_call_is_not_blocked_by_a_stuck_panes_lock_held_attach stdout ----

thread 'a_second_connections_control_call_is_not_blocked_by_a_stuck_panes_lock_held_attach' (58062508) panicked at crates/server/tests/attach.rs:149:39:
reading the reply: Os { code: 35, kind: WouldBlock, message: "Resource temporarily unavailable" }


failures:
    a_pane_that_closes_between_a_queued_attach_and_its_arm_leaves_nothing_to_detach
    a_second_connections_control_call_is_not_blocked_by_a_stuck_panes_lock_held_attach
    an_unresponsive_panes_own_commands_do_not_stall_the_connection_or_another_pane

test result: FAILED. 20 passed; 3 failed; 0 ignored; 0 measured; 0 filtered out; finished in 55.28s

error: test failed, to rerun pass `-p steg-server --test attach`
```

Built per the approved plan, with one discovery and the already-reported out-of-plan comments-only commit.

Commits (10, in order): git mv dispatch.rs -> dispatch/main.rs (alone); extract fixtures.rs; extract session.rs; extract cross_cutting.rs; extract tab.rs; extract pane.rs; extract api.rs; extract setting.rs; fix five stale "dispatch.rs" references in crates/server/tests/{runtime,events,routing}.rs (comments only); merge origin/main.

Checks recorded on this task (each via asdlc record-check): the test listing immediately after the git-mv commit (byte-identical bare names, proving the move alone changes nothing); the lint gate after the first noun extraction (session.rs) - confirms the one crate-root #![expect(clippy::unwrap_used)] in main.rs still covers every child module, the load-bearing assumption behind one binary not eight; clippy -D warnings after every extraction commit (all clean); the full test listing at the final head (77/77, 0 ignored); the per-module sequence-diff script (proof c) at the final head; both proofs' controls recorded failing then passing (rename a test; a one-character body edit) against the real worktree, reverted with git checkout before the next commit each time; a representative touch-then-relink timing sample; six just-check/attach-alone runs (five full just check, one standalone cargo test -p steg-server --test attach), the last one confirmed to carry nothing from the environment before being left as the recorded failing run; a final just check could not be made green on this machine - see below.

Departures from the plan:
1. Discovered while extracting session.rs: unset_label_clears_on_either_switch_value_and_leaves_it_alone_when_not_given loops session/tab/pane in one test body - a span-noun test the plan's marker-comment cuts didn't single out (it sits inside the "// --- session ---" block, between session_set_and_unset_change_and_clear_the_label and session_show_with_an_unknown_target_is_no_such_target). Moved to dispatch/cross_cutting.rs per the plan's own rule for tests that cross a noun boundary, not to session.rs.
2. fixtures.rs - moment's own doc comment reworded from "Most of this file's calls pass moment(0)" to "Most calls across this binary pass moment(0)", since fixtures.rs itself makes no calls to dispatch once the file split - the original wording stopped being accurate. Two /// lines changed; confirmed comment-only, no code line touched.
3. session.rs - one blank line added between session_set_and_unset_change_and_clear_the_label's closing brace and session_show_with_an_unknown_target_is_no_such_target's #[test] attribute: the seam left behind once the span-noun unset_label test (originally between these two) was excised to cross_cutting.rs. One blank line; confirmed whitespace-only, no code line touched.
4. tab.rs - a 5-line /// doc-comment addition on pane_list_with_an_ambiguous_target_is_ambiguous_target, explaining why it lives in tab.rs and not pane.rs (shares tab_mut_for_test and a local lookup stand-in with two tab tests directly above it) - called for by the plan itself ("a one-line comment ... explaining why it lives here"). Five /// lines added; confirmed comment-only, no code line touched.
5. pane.rs - the comparison script's one-line diff here is not a change to pane.rs's own content: the baseline slice includes the original file's trailing blank separator line (the whitespace before "// --- api ---"'s marker comment in the pre-move file), which a standalone module file naturally doesn't reproduce at its own end-of-file. An artifact of where the comparison's slice boundary was drawn, not a line added or removed from this file.
None of the five touches a code line - each is checkable against the diffs recorded on this task and quoted in the PR body.

Local `just check` was not green at the final head (e527e603) on this machine, and not because of anything in dispatch: on every one of six runs (five full just check, one standalone `cargo test -p steg-server --test attach`), the dispatch binary passed 77/77. The failures were confined every time to crates/server/tests/attach.rs's stuck-pane/bound timing tests - a file this task does not touch - under the load of several other builds running on this machine at once (load average seen ranged roughly 3.4 to 19.4 across the six runs; even the standalone attach-alone run, at load ~4-8, still failed 3 of 5). A different subset failed each run:
- a_pane_that_closes_between_a_queued_attach_and_its_arm_leaves_nothing_to_detach - panicked "pane.attach must answer within a bound" (attach.rs:1793): its own bounded wait for the reply timed out.
- an_unresponsive_panes_own_commands_do_not_stall_the_connection_or_another_pane - panicked "pane.show must answer within a bound, even with the other pane stuck" (attach.rs:1427): the same bounded-wait timeout shape.
- a_second_connections_control_call_is_not_blocked_by_a_stuck_panes_lock_held_attach - panicked reading the reply with Os { code: 35, kind: WouldBlock } (attach.rs:149): a non-blocking socket read came back WouldBlock instead of the reply inside the test's own bound.
- pane_close_force_returns_within_its_bound_even_while_the_panes_own_worker_is_stuck - the same WouldBlock failure, in at least one run.
- stopping_the_server_returns_within_its_bound_even_while_a_panes_worker_is_stuck - the same WouldBlock failure, in at least one run.
The last just-check run was recorded on this task as it ran (exit 101), confirmed beforehand to carry nothing from the environment in its output. CI's run on PR 76's head (e527e603) is the clean check for this change, confirmed green both by my own foregrounded `gh pr checks --watch` and by the engineer directly.

PR: https://github.com/bruk-io/steg/pull/76 (head e527e603) - the body carries the same local-check/CI-is-clean statement, the five test names, and the Departures list.

Where a later task's new dispatch test goes: for an existing noun, in dispatch/<noun>.rs (session.rs/tab.rs/pane.rs/api.rs/setting.rs). One that tests api.call's own forwarding or targeting for that noun goes in dispatch/api.rs regardless of which noun it names. One that crosses several nouns in one call sequence, or exercises dispatch's own completeness/shape guarantees rather than one noun's behaviour, goes in dispatch/cross_cutting.rs. A genuinely new noun gets its own new dispatch/<noun>.rs, declared with one more mod line in dispatch/main.rs.
<!-- SECTION:NOTES:END -->

## Comments

<!-- COMMENTS:BEGIN -->
author: @bruk
created: 2026-11-18T02:41:00Z

---

Plan approved by @bruk (a5fe3026a112, size-2): its worker may build it.

---

author: @bruk.r2
created: 2026-11-18T03:49:00Z

---

@bruk.r2 is reviewing TASK-1.135: a same-machine review: it waited 0m with no other engineer's reviewer, longer than review_fallback_after (0m), so a fresh reviewer on its author's machine takes it.

---
<!-- COMMENTS:END -->
