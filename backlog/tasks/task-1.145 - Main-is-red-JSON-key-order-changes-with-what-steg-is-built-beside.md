---
id: TASK-1.145
title: 'Main is red: JSON key order changes with what steg is built beside'
status: In Progress
assignee:
  - '@bruk.w3'
created_date: '2026-11-21T20:09:00Z'
updated_date: '2026-11-21T20:58:00Z'
labels:
  - size-2
milestone: m-3
dependencies: []
parent_task_id: TASK-1
priority: high
project: bruk-io-steg
asdlc_accepted_from: DRAFT-4
asdlc_branch: task-1.145-main-is-red-json-key-order-changes-with
asdlc_plan_approved_by: bruk
asdlc_plan_approved_hash: '072fa2486aeb'
asdlc_plan_approved_size: size-2
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Main's CI failed at d50a5d20 (the merge of PR 87, TASK-1.120) and blocks every change whose affected tests build the app (TASK-1.136 hit it). cli::output::tests::a_field_declared_text_but_really_structured_renders_its_own_compact_json and cli::output::tests::nested_plain_and_json_are_identical_bytes in crates/steg/src/cli/output.rs assert exact JSON text with keys in sorted order. gpui turns on serde_json's preserve_order feature, and cargo unifies features across a build: built alone (-p steg, what PR CI's affected tests ran) serde_json sorts keys and the tests pass; built with the workspace (just test-all, or any run that includes the app) keys keep insertion order and the tests fail. The same holds for the product: the bytes steg prints for nested JSON depend on what it was built beside. Decide the order on purpose (recommended: declare preserve_order where steg's crates depend on serde_json, so output follows the API description's order in every build, and correct the two tests' expected text; the alternative is tests that compare parsed values and an output order left unspecified) and make it hold in both builds. The two existing tests encode the accidental order, so they change with the fix: say so in the notes. Also say whether the PR CI's affected-test selection should have caught this (PR 87 was green there). doc-2 section on output formats; TASK-1.120.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 serde_json's key order in steg's output is the same whether the steg package is built alone or with the whole workspace, chosen on purpose and stated in the notes
- [ ] #2 just test-all and the affected-tests run of a change touching crates/ui both pass; the two output.rs tests pass in both builds
- [ ] #3 A recorded pair shows the two tests failing in the whole-workspace build before the fix and passing after
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
## Decision (forced, not a judgement call)

gpui's dependency chain (schemars's `indexmap2` feature, pulled in through `gpui`/
`gpui_util`/`gpui_perf`/`gpui_http_client`) turns on serde_json's `preserve_order`
feature unconditionally. Cargo features are additive and workspace-unified: any
build that includes `steg-app` (or anything that depends on it, such as the whole
workspace) has `preserve_order` on, and nothing steg's own crates do can turn it
back off. So AC #1 ("the same order whether built alone or with the whole
workspace") has exactly one satisfiable answer: make the alone build match the
workspace build by declaring `preserve_order` on steg's own serde_json edges too,
not the other way around. The description's "alternative" (compare parsed values,
leave order unspecified) does not satisfy AC #1 on its own - "unspecified" is not
"the same" - so it is rejected, not merely not preferred.

Rejected alternative also considered: have the CLI re-print the server's raw
received bytes instead of re-serialising a parsed `Value`, which would also be
order-stable. Rejected: it changes `render`'s signature away from `&Value`
(TASK-1.120's own design, `crates/steg/src/cli/output.rs`'s module doc), and the
unit tests build `Value`s directly with `json!`, not from wire bytes, so it buys
nothing here.

The order this produces, stated concretely (AC #1's "stated in the notes"):
record/method shapes reach `Value` through `serde_json::to_value` on a typed
struct (`crates/server/src/dispatch/outcome.rs::into_value`), which builds a
`Value::Object` backed by a `Map`. `to_value` is not the same path as
`to_string(&record)`: the latter serialises straight to text and is always
declaration-ordered, feature or no feature (why `crates/sdk/src/record.rs`'s own
serialisation tests are unaffected - they never build a `Map`); `to_value` does
build one, and without `preserve_order` that `Map` is a `BTreeMap`, which sorts
keys outright - so the wire text itself is already sorted when the server is
built alone, today, before the CLI touches it. With `preserve_order` on, the same
`Map` is an `IndexMap` and keeps the struct's own declared field order (e.g.
`SessionRecord`'s `id, label, position, colour, current_tab`); that order is what
lands in the wire text, and the CLI's own parse-and-reprint
(`output.rs::json_line`) preserves it again rather than re-sorting it. So the
variance this task fixes is upstream of the CLI, at the server's own `into_value`,
not only in how the CLI re-prints - the fix (declaring the feature on every one of
steg's crates that touches serde_json, sdk through app) corrects it at every edge
that builds a `Map`, not just the CLI's. `render`'s own unit tests build `Value`s
directly with `json!` instead of parsing wire text; there, with `preserve_order`
on, the order is the `json!` macro literal's own key order (confirmed against the
real failing text below).

Confirmed this is the only place in the workspace keyed on JSON object key order:
`grep -rn '{\\"' --include='*.rs' crates/` and `grep -rn '\.keys()' --include='*.rs'
crates/` found two kinds of hit - (1) single-key `{"id": ...}` literals/doc
comments, order-irrelevant; (2) `crates/sdk/src/record.rs` and
`crates/server/tests/dispatch/{session,server,client}.rs` compare `.keys()` either
sorted (`keys.sort_unstable()`) or as a `HashSet`, both order-insensitive by
construction. Only `crates/steg/src/cli/output.rs`'s two named tests assert exact
JSON text built from a hand-written `json!` literal with two or more keys.

Limitation to state in the notes, not new scope: nothing in the suite pins the
*product's* end-to-end key order - `crates/steg/tests/cli_output_formats.rs`
builds its expectations from the parsed `--json` object by field name, order-
insensitive by construction. After this fix, AC #1's guarantee rests on the two
corrected `output.rs` unit tests plus the manifest declaration, not on any end-to-
end assertion. Not adding one: the task doesn't ask, and the brief says keep this
tight.

boundaries.toml is unaffected by the check's own behaviour: it has no
`deny-features` entry for `serde_json` on any crate (`steg-sdk`, `steg-server`,
`steg`, `steg-app` all already `allow` it), and the check reads each crate's own
declared edge, not resolved features, so declaring the feature explicitly is
exactly what it is built to see, not evade. Its `[steg]` reason text is a separate
matter - see "Files that change" below, it is already stale and this task touches
the exact line it's wrong about. deny.toml/licences are unaffected: `indexmap
2.14.2` is already resolved in Cargo.lock through gpui/schemars (`grep -n '^name =
"indexmap"' Cargo.lock`), confirmed clean today with `just deny` before any edit
(re-recorded after, as part of the final `just check`). doc-2's own "`--json` is a
contract" line (section on output formats) names full identifiers and field
names, fields only ever added - it says nothing about key order, consistent with
deciding it here rather than finding it already specified.

## Files that change

- `Cargo.toml` (root): add to `[workspace.dependencies]`:
  `serde_json = { version = "1", features = ["preserve_order"] }`, next to the
  existing `gpui` entry, with a comment stating this task's reason (TASK-1.145:
  gpui's own dependency chain already forces this feature on in any build that
  includes the app; declaring it here too makes every steg crate's own build -
  alone or with the workspace - produce the same key order, rather than leaving
  the steg-alone case sorted by accident). This is the one GLOBAL_FILES entry in
  the change (`tools/src/steg_tools/affected.py`), so it alone makes this task's
  own `just test` selection "all" - correct, since a key-order guarantee is a
  workspace-wide property, not a per-crate one; naming only `crates/steg/Cargo.toml`
  would select `-p steg` alone for `just test`, the exact blind spot that let PR 87
  through.
- `crates/sdk/Cargo.toml`: `serde_json = "1"` -> `serde_json.workspace = true`; add
  one line to the existing R106 comment pointing at the root manifest's comment for
  why, instead of restating it.
- `crates/server/Cargo.toml`: same substitution and comment pointer.
- `crates/steg/Cargo.toml`: same substitution; add the comment pointer, and
  correct the existing comment's closing line, which is already false and this
  task touches the same block - "Not used by `crates/steg/src/main.rs` itself" no
  longer holds: `crates/steg/src/cli/output.rs:16` is `use serde_json::{Map,
  Value};`, production code reached from `main.rs` (added by TASK-1.120, comment
  never updated). Correct it to say the fixture's own hand-built request is still
  the reason this is a direct dependency, but `output.rs` also uses it now.
- `crates/app/Cargo.toml`: same substitution and comment pointer.
- `boundaries.toml`: text-only correction to `[steg]`'s `reason` string, which
  repeats the same stale claim ("crates/steg/src/main.rs itself never imports
  it") - the check's behaviour does not change (no `deny-features` entry exists
  for `serde_json` there to affect), only the sentence that is now provably wrong
  given the line above it. Named here explicitly rather than silently left wrong
  just because the check still passes.
- `crates/steg/src/cli/output.rs`: correct the two tests that encode the old,
  accidental sorted order to the real `preserve_order` bytes:
  - `a_field_declared_text_but_really_structured_renders_its_own_compact_json`
    (currently asserts `"t1\t{\"panes\":[\"p1\",\"p2\"],\"split\":\"horizontal\"}\n"`,
    alphabetically sorted; the `json!` literal's own order is `split` then `panes`,
    so this is expected to become
    `"t1\t{\"split\":\"horizontal\",\"panes\":[\"p1\",\"p2\"]}\n"` - predicted, not
    hand-asserted: copied verbatim from the recorded failing run in step 5 below,
    not retyped by eye).
  - `nested_plain_and_json_are_identical_bytes` (currently asserts
    `"{\"snapshot\":{\"rows\":[]},\"stream\":\"st1\"}\n"`, sorted; the `json!`
    literal's order is `stream` then `snapshot`, expected to become
    `"{\"stream\":\"st1\",\"snapshot\":{\"rows\":[]}}\n"` - same caveat).
  These two tests encode the accidental order on purpose being replaced by the
  decided order, not a code bug being chased - say so in the task notes, since the
  description called this out explicitly.
- Task notes (`--append-notes`, never `--notes`): the decision above, the order
  named and where it is actually built (`outcome.rs::into_value`, not only the
  CLI), the limitation that nothing end-to-end pins it, the two corrected tests
  flagged as deliberate, the stale-comment correction, whether PR CI's selection
  should have caught this (see "Open questions" #3), and the recorded proof.

No change to `crates/kernel/Cargo.toml`, `crates/ui/Cargo.toml`,
`crates/sdk/src/record.rs` or any other test, `deny.toml`, or `tools/`.

## Order of work

1. Run `just test-all` once, unrecorded, and read its tail: confirm it is only the
   two tests' assertion diffs (expected vs actual JSON text), nothing from the
   environment. Then record it with `asdlc record-check` as the baseline - this
   doubles as AC #3's "before" half.
2. Edit the root `Cargo.toml`'s `[workspace.dependencies]` to add the
   `serde_json`/`preserve_order` entry and its comment.
3. Edit the four crates' `Cargo.toml` files to `serde_json.workspace = true` with
   the comment pointer each, correcting `crates/steg/Cargo.toml`'s stale closing
   line in the same edit.
4. Correct `boundaries.toml`'s `[steg]` reason string to match.
5. Record `just check-packages` (builds and tests every crate alone, in sequence,
   per the justfile's own recipe; note it runs under `set -euo pipefail` over
   `steg-sdk, steg-kernel, steg-server, steg, steg-ui, steg-app` in order, so a
   failure at `steg` aborts the recipe before `steg-ui`/`steg-app` run - read that
   as a partial run, not a second independent failure). Expected to now show the
   same two tests failing in the alone build too - the direct evidence that AC #1
   now holds (both builds agree, even though both currently fail) - distinct from
   AC #3's workspace-only "before".
6. Run the two named tests directly (or `just test-all`) to capture the exact
   bytes `preserve_order` actually produces, and copy them verbatim into the two
   assertions in `output.rs` - do not hand-type the predicted strings above without
   checking them against this output first.
7. Record `just check-packages` and `just test-all` again - both expected green
   and complete (no abort), confirming AC #1 (same order, both builds) and the
   "after" half of AC #3.
8. Record `just check` (the full gate: format, lint, boundaries, licences,
   affected tests, every package alone, tools' own tests, architecture, recipes,
   UI checks) as the final proof the change is done.
9. Append the notes described above to the task.

## Risks

- Hand-typing the expected byte strings could introduce a typo invisible to
  review; mitigated by step 6 (copy from the actual recorded failure, never
  retyped from the prediction in this plan).
- `asdlc`'s test-edit guard may refuse editing `output.rs`'s two tests (they are
  tests, and the guard exists for a reason) - if it does, stop and report rather
  than working around it; the task's own description already anticipated this and
  asked for the same.
- Minimal file overlap with concurrent work: TASK-1.140 (@bruk.w2) is in
  `crates/server/src/runtime/` and `dispatch/pane.rs`; TASK-1.51 (@bruk.w1) is in
  `crates/steg/src/cli/` but its own acceptance criteria are about target
  resolution (`doc-2 R33`/section 4), not output rendering - neither is expected to
  touch `crates/steg/src/cli/output.rs` or any `Cargo.toml`/`boundaries.toml`.
  Flagging explicitly so neither worker also edits those files: if either does,
  the overlap needs the engineer's attention, not a silent merge.
- Declaring `preserve_order` costs nothing new in the dependency graph
  (`indexmap` is already resolved via gpui/schemars) and nothing in
  `boundaries.toml`'s own checked behaviour (no `deny-features` entry for
  `serde_json` anywhere) - confirmed by reading both files and running `just deny`
  before any edit - low risk, re-verified by the final `just check` in step 8
  rather than only asserted here.

## Proof

- `asdlc record-check TASK-1.145 --in <worktree> --as bruk.w3 -- just test-all`
  (before the fix: the two named tests fail - AC #3's "before" half).
- `asdlc record-check TASK-1.145 --in <worktree> --as bruk.w3 -- just check-packages`
  (after the manifest edits, before the text fix: the same two tests now fail in
  the alone build too, and the recipe aborts there per its own `set -e` - the
  direct evidence for AC #1 that nothing else gives).
- `asdlc record-check TASK-1.145 --in <worktree> --as bruk.w3 -- just check-packages`
  (after the text fix: green and complete, no abort).
- `asdlc record-check TASK-1.145 --in <worktree> --as bruk.w3 -- just test-all`
  (after the fix: green - AC #3's "after" half, and AC #2's "just test-all ... both
  pass").
- `asdlc record-check TASK-1.145 --in <worktree> --as bruk.w3 -- just check`
  (the full gate, final proof).

AC #2's other half ("the affected-tests run of a change touching crates/ui ... both
pass") needs stating plainly rather than silently reinterpreted: confirmed with
`just show-affected` against a scratch touch under `crates/ui/`, that selection is
`-p steg-app -p steg-ui` - it never builds or runs `crates/steg`'s own tests at
all, so the two named tests are not even selected there; that half of AC #2 is
satisfied vacuously by a crates/ui change, not demonstrated by one. The selection
that actually exercises both halves together - builds `steg` (the two tests) and
`steg-app` (which pulls in gpui/preserve_order) in one `cargo test` invocation, the
same feature-unification `just test-all` has - is a change touching `crates/sdk/`,
confirmed via `just show-affected` to select
`-p steg -p steg-app -p steg-kernel -p steg-sdk -p steg-server`. Recorded as
additional evidence in the task notes, named as what actually demonstrates AC #2's
intent, alongside a literal crates/ui-touch run recorded for the letter of the
criterion.

## Open questions for the engineer

1. Confirm the decision above (force `preserve_order` via a workspace dependency)
   rather than the description's named alternative - the alternative cannot
   satisfy AC #1 given gpui's own unconditional feature, so this is presented as
   settled, not as a choice, unless something here is wrong.
2. AC #2's crates/ui-touch wording is satisfied vacuously (the relevant tests
   are not selected there at all) rather than meaningfully demonstrated - accept
   that, with the crates/sdk-touch selection recorded as the real demonstration, or
   amend AC #2's wording to name a selection that actually runs the tests.
3. Whether PR CI's affected-test selection should have caught this (the
   description asks this explicitly): no - and not only by bad luck. The
   selection's own documented design (`affected.py`'s module doc,
   `boundaries.toml`'s comment on `check-packages`) is to build a changed crate
   alone or with its dependents, which is structurally blind to any behaviour that
   depends on workspace-wide feature unification with a crate *outside* that set -
   here, `steg-app`/gpui. This fix removes *this* behaviour's dependence on that
   gap (the order is now forced the same everywhere), but the gap itself is
   general and still open for the next feature gpui (or anything else) turns on
   workspace-wide. Naming it as a follow-up for the orchestrator to draft, not
   building it here, with two candidates to choose between: (a) always widen any
   non-empty selection to also include `-p steg-app`, cheap but ad hoc - it is
   naming gpui's own effect by hand, which could stop being true; (b) run the
   selected set as `cargo test --workspace --exclude <everything not selected or a
   dependent>` so features always unify exactly as a full build would, more
   general but changes `affected.py`'s own contract (it currently never builds
   anything outside the selected set). Recommend (b) for the orchestrator to
   weigh, since (a) only works until the next crate the app depends on starts
   forcing some other feature on.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Plan (bce72f0592e9) checked by @bruk.o on 2026-11-21 against the task and doc-2's output section: ok. doc-2 calls --json a contract of identifiers and field names and says nothing on key order, so the order is decided here: serde_json's preserve_order declared once as a workspace dependency and used by sdk, server, steg and app, so a crate built alone orders keys as the whole workspace does; the two output.rs tests change to that order, their text copied from a recorded run. Proof is recorded runs of just test-all before and after, just check-packages after the manifests change and after the tests change, and just check. For the engineer: AC 2's words about a crates/ui change are met only vacuously, since that selection never runs steg's tests; a crates/sdk selection is recorded as the real demonstration. Follow-up to draft: the affected-test selection cannot see behaviour that depends on features unified across the workspace.

Correction by @bruk.o: the plan changed after the check above; the current plan is 072fa2486aeb. The one addition is a text-only correction to the reason string of boundaries.toml's steg entry and the matching comment in crates/steg/Cargo.toml, which say the steg crate never imports serde_json, untrue since TASK-1.120. Checked: no rule in boundaries.toml changes, only its prose. Ok.

Check FAILED: `just test-all` exited 101 at 2026-11-21T20:32:48+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.145 on task-1.145-main-is-red-json-key-order-changes-with at aef4b329ef03, last 40 of 64 lines:

```text
test cli::output::tests::records_with_a_non_object_element_is_a_shape_error_not_a_panic ... ok
test cli::output::tests::records_with_a_non_array_reply_is_a_shape_error_not_a_panic ... ok
test cli::output::tests::records_with_no_declared_fields_prints_nothing_in_every_combination ... ok
test cli::output::tests::text_json_prints_the_string_value_unchanged ... ok
test cli::output::tests::text_plain_is_the_string_unaltered ... ok
test cli::output::tests::text_quiet_is_the_same_as_without_quiet_either_way ... ok
test cli::output::tests::text_with_a_non_string_reply_is_a_shape_error_not_a_panic ... ok
test cli::tree::tests::autostart_given_after_no_autostart_wins ... ok
test cli::tree::tests::autostart_defaults_to_true_with_neither_flag_given ... ok
test cli::tree::tests::no_autostart_is_read_whether_given_before_or_after_the_subcommand ... ok
test cli::tree::tests::json_is_read_from_the_leaf_subcommand ... ok
test cli::tree::tests::quiet_is_read_from_the_leaf_subcommand_either_spelling ... ok

failures:

---- cli::output::tests::nested_plain_and_json_are_identical_bytes stdout ----

thread 'cli::output::tests::nested_plain_and_json_are_identical_bytes' (71589412) panicked at crates/steg/src/cli/output.rs:708:9:
assertion `left == right` failed
  left: "{\"stream\":\"st1\",\"snapshot\":{\"rows\":[]}}\n"
 right: "{\"snapshot\":{\"rows\":[]},\"stream\":\"st1\"}\n"

---- cli::output::tests::a_field_declared_text_but_really_structured_renders_its_own_compact_json stdout ----

thread 'cli::output::tests::a_field_declared_text_but_really_structured_renders_its_own_compact_json' (71589404) panicked at crates/steg/src/cli/output.rs:593:9:
assertion `left == right` failed
  left: "t1\t{\"split\":\"horizontal\",\"panes\":[\"p1\",\"p2\"]}\n"
 right: "t1\t{\"panes\":[\"p1\",\"p2\"],\"split\":\"horizontal\"}\n"
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    cli::output::tests::a_field_declared_text_but_really_structured_renders_its_own_compact_json
    cli::output::tests::nested_plain_and_json_are_identical_bytes

test result: FAILED. 35 passed; 2 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

error: test failed, to rerun pass `-p steg --lib`
full output: .quiet/tests-every-crate.log
error: recipe `test-all` failed with exit code 101
```

Check FAILED: `just check-packages` exited 101 at 2026-11-21T20:43:45+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.145 on task-1.145-main-is-red-json-key-order-changes-with at aef4b329ef03 with uncommitted changes, last 40 of 75 lines:

```text
test cli::output::tests::records_with_a_non_array_reply_is_a_shape_error_not_a_panic ... ok
test cli::output::tests::records_with_a_non_object_element_is_a_shape_error_not_a_panic ... ok
test cli::output::tests::records_with_no_declared_fields_prints_nothing_in_every_combination ... ok
test cli::output::tests::text_json_prints_the_string_value_unchanged ... ok
test cli::output::tests::text_plain_is_the_string_unaltered ... ok
test cli::output::tests::text_quiet_is_the_same_as_without_quiet_either_way ... ok
test cli::output::tests::text_with_a_non_string_reply_is_a_shape_error_not_a_panic ... ok
test cli::tree::tests::autostart_given_after_no_autostart_wins ... ok
test cli::tree::tests::autostart_defaults_to_true_with_neither_flag_given ... ok
test cli::tree::tests::json_is_read_from_the_leaf_subcommand ... ok
test cli::tree::tests::no_autostart_is_read_whether_given_before_or_after_the_subcommand ... ok
test cli::tree::tests::quiet_is_read_from_the_leaf_subcommand_either_spelling ... ok

failures:

---- cli::output::tests::a_field_declared_text_but_really_structured_renders_its_own_compact_json stdout ----

thread 'cli::output::tests::a_field_declared_text_but_really_structured_renders_its_own_compact_json' (72362964) panicked at crates/steg/src/cli/output.rs:593:9:
assertion `left == right` failed
  left: "t1\t{\"split\":\"horizontal\",\"panes\":[\"p1\",\"p2\"]}\n"
 right: "t1\t{\"panes\":[\"p1\",\"p2\"],\"split\":\"horizontal\"}\n"
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace

---- cli::output::tests::nested_plain_and_json_are_identical_bytes stdout ----

thread 'cli::output::tests::nested_plain_and_json_are_identical_bytes' (72362972) panicked at crates/steg/src/cli/output.rs:708:9:
assertion `left == right` failed
  left: "{\"stream\":\"st1\",\"snapshot\":{\"rows\":[]}}\n"
 right: "{\"snapshot\":{\"rows\":[]},\"stream\":\"st1\"}\n"


failures:
    cli::output::tests::a_field_declared_text_but_really_structured_renders_its_own_compact_json
    cli::output::tests::nested_plain_and_json_are_identical_bytes

test result: FAILED. 35 passed; 2 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

error: test failed, to rerun pass `-p steg --lib`
full output: .quiet/test-steg.log
error: recipe `check-packages` failed with exit code 101
```

Check passed: `just check-packages` exited 0 at 2026-11-21T20:49:14+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.145 on task-1.145-main-is-red-json-key-order-changes-with at aef4b329ef03 with uncommitted changes:

```text
ok      build (steg-sdk) (0.1s)
ok      test (steg-sdk) (0.3s)
ok      test (steg-sdk, local-socket) (0.4s)
ok      build (steg-kernel) (0.1s)
ok      test (steg-kernel) (0.2s)
ok      build (steg-server) (0.1s)
ok      test (steg-server) (55.9s)
ok      test (steg-server, race-fixture) (65.0s)
ok      test (steg-server, pty-race-fixture) (68.9s)
ok      test (steg-server, accept-backoff-fixture) (75.5s)
ok      build (steg) (0.6s)
ok      test (steg) (5.3s)
ok      test (steg, fixtures) (10.0s)
ok      build (steg-ui) (17.1s)
ok      test (steg-ui) (1.3s)
ok      build (steg-app) (1.7s)
ok      test (steg-app) (2.2s)
```

Check passed: `just test-all` exited 0 at 2026-11-21T20:50:29+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.145 on task-1.145-main-is-red-json-key-order-changes-with at aef4b329ef03 with uncommitted changes:

```text
ok      tests (every crate) (67.6s)
```

Check passed: `just test` exited 0 at 2026-11-21T20:52:00+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.145 on task-1.145-main-is-red-json-key-order-changes-with at aef4b329ef03 with uncommitted changes:

```text
ok      tests (every crate) (64.6s)
```

Check passed: `just check` exited 0 at 2026-11-21T20:58:56+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.145 on task-1.145-main-is-red-json-key-order-changes-with at aef4b329ef03 with uncommitted changes:

```text
ok      format check (0.4s)
ok      lint (49.4s)
ok      boundaries (0.2s)
ok      foundations (0.2s)
ok      foundations public surface (0.1s)
ok      design pages (0.1s)
ok      licences (0.7s)
ok      tests (every crate) (59.2s)
ok      build (steg-sdk) (0.1s)
ok      test (steg-sdk) (0.3s)
ok      test (steg-sdk, local-socket) (0.4s)
ok      build (steg-kernel) (0.1s)
ok      test (steg-kernel) (0.2s)
ok      build (steg-server) (0.1s)
ok      test (steg-server) (57.2s)
ok      test (steg-server, race-fixture) (61.9s)
ok      test (steg-server, pty-race-fixture) (66.4s)
ok      test (steg-server, accept-backoff-fixture) (58.7s)
ok      build (steg) (0.1s)
ok      test (steg) (2.2s)
ok      test (steg, fixtures) (5.5s)
ok      build (steg-ui) (0.3s)
ok      test (steg-ui) (0.3s)
ok      build (steg-app) (0.3s)
ok      test (steg-app) (0.7s)
ok      tool tests (41.2s)
ok      architecture (0.7s)
ok      recipe names (0.1s)
ok      ui levels (0.1s)
ok      ui colours (0.1s)
```
<!-- SECTION:NOTES:END -->

## Comments

<!-- COMMENTS:BEGIN -->
author: @bruk
created: 2026-11-21T20:30:00Z

---

Plan approved by @bruk (072fa2486aeb, size-2): its worker may build it.

---
<!-- COMMENTS:END -->
