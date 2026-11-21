---
id: TASK-1.145
title: 'Main is red: JSON key order changes with what steg is built beside'
status: In Progress
assignee:
  - '@bruk.w3'
created_date: '2026-11-21T20:09:00Z'
updated_date: '2026-11-21T20:28:00Z'
labels:
  - size-2
milestone: m-3
dependencies: []
parent_task_id: TASK-1
priority: high
project: bruk-io-steg
asdlc_accepted_from: DRAFT-4
asdlc_branch: task-1.145-main-is-red-json-key-order-changes-with
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
record/method shapes reach `Value` through `serde_json::to_value` on a typed,
`#[derive(Serialize)]` struct (`crates/server/src/dispatch/outcome.rs::into_value`)
- struct serialisation emits fields in their declared order regardless of
`preserve_order` (that feature only changes `Map`/parsed-object ordering), so the
wire text's key order is the sdk's own struct field order (e.g. `SessionRecord`'s
`id, label, position, colour, current_tab`). The CLI parses that text back into a
`Value` and re-prints it (`output.rs::json_line`); with `preserve_order` on, parsing
preserves the text's own key order, so the bytes a user sees carry the struct's
declared field order end to end. `render`'s own unit tests build `Value`s directly
with `json!` instead of parsing wire text; there, with `preserve_order` on, the
order is simply the `json!` macro literal's own key order (confirmed against the
real failing text below).

Confirmed this is the only place in the workspace keyed on JSON object key order:
`grep -rn '{\\"' --include='*.rs' crates/` and `grep -rn '\.keys()' --include='*.rs'
crates/` found two kinds of hit - (1) single-key `{"id": ...}` literals/doc
comments, order-irrelevant; (2) `crates/sdk/src/record.rs` and
`crates/server/tests/dispatch/{session,server,client}.rs` compare `.keys()` either
sorted (`keys.sort_unstable()`) or as a `HashSet`, both order-insensitive by
construction. `crates/sdk/src/*.rs`'s many `to_string(&record)`/`to_string(&status)`
tests serialise typed structs/enums directly, unaffected by `preserve_order` for the
reason above. Only `crates/steg/src/cli/output.rs`'s two named tests assert exact
JSON text built from a hand-written `json!` literal with two or more keys.

boundaries.toml is unaffected: it has no `deny-features` entry for `serde_json` on
any crate (`steg-sdk`, `steg-server`, `steg`, `steg-app` all already `allow` it),
and the check reads each crate's own declared edge, not resolved features, so
declaring the feature explicitly is exactly what it is built to see, not evade.
deny.toml/licences are unaffected: `indexmap 2.14.2` is already resolved in
Cargo.lock through gpui/schemars (`grep -n '^name = "indexmap"' Cargo.lock`), so no
new crate or licence enters the graph; confirmed clean today with `just deny`
before any edit (baseline, re-recorded after as part of the final `just check`).
doc-2's own "`--json` is a contract" line (section on output formats) names full
identifiers and field names, fields only ever added - it says nothing about key
order, consistent with deciding it here rather than finding it already specified.

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
- `crates/steg/Cargo.toml`: same substitution; keep the existing fixture-only
  explanation (unchanged - this crate's production code still never imports
  serde_json itself) and add the same comment pointer.
- `crates/app/Cargo.toml`: same substitution and comment pointer.
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
  named, the two corrected tests flagged as deliberate, whether PR CI's selection
  should have caught this (see "Open questions" #3), and the recorded proof.

No change to `boundaries.toml`, `deny.toml`, `crates/kernel/Cargo.toml`,
`crates/ui/Cargo.toml`, `crates/sdk/src/record.rs` or any other test, or `tools/`.

## Order of work

1. Run `just test-all` once, unrecorded, and read its tail: confirm it is only the
   two tests' assertion diffs (expected vs actual JSON text), nothing from the
   environment. Then record it with `asdlc record-check` as the baseline - this
   doubles as AC #3's "before" half.
2. Edit the root `Cargo.toml`'s `[workspace.dependencies]` to add the
   `serde_json`/`preserve_order` entry and its comment.
3. Edit the four crates' `Cargo.toml` files to `serde_json.workspace = true` with
   the comment pointer each.
4. Record `just check-packages` (builds and tests every crate alone, in sequence,
   per the justfile's own recipe) - expected to now show the *same two tests*
   failing in the alone build too. This is the direct evidence that AC #1 now
   holds (both builds agree, even though both currently fail) - distinct from
   AC #3's workspace-only "before".
5. Run the two named tests directly (or `just test-all`) to capture the exact
   bytes `preserve_order` actually produces, and copy them verbatim into the two
   assertions in `output.rs` - do not hand-type the predicted strings above without
   checking them against this output first.
6. Record `just check-packages` and `just test-all` again - both expected green,
   confirming AC #1 (same order, both builds) and the "after" half of AC #3.
7. Record `just check` (the full gate: format, lint, boundaries, licences,
   affected tests, every package alone, tools' own tests, architecture, recipes,
   UI checks) as the final proof the change is done.
8. Append the notes described above to the task.

## Risks

- Hand-typing the expected byte strings could introduce a typo invisible to
  review; mitigated by step 5 (copy from the actual recorded failure, never
  retyped from the prediction in this plan).
- `asdlc`'s test-edit guard may refuse editing `output.rs`'s two tests (they are
  tests, and the guard exists for a reason) - if it does, stop and report rather
  than working around it; the task's own description already anticipated this and
  asked for the same.
- Minimal file overlap with concurrent work: TASK-1.140 (@bruk.w2) is in
  `crates/server/src/runtime/` and `dispatch/pane.rs`; TASK-1.51 (@bruk.w1) is in
  `crates/steg/src/cli/` but its own acceptance criteria are about target
  resolution (`doc-2 R33`/section 4), not output rendering - neither is expected to
  touch `crates/steg/src/cli/output.rs` or any `Cargo.toml`. Flagging explicitly so
  neither worker also edits those files: if either does, the overlap needs the
  engineer's attention, not a silent merge.
- Declaring `preserve_order` costs nothing new in the dependency graph
  (`indexmap` is already resolved via gpui/schemars) and nothing in
  `boundaries.toml` (no `deny-features` entry for `serde_json` anywhere), confirmed
  by reading both files and running `just deny` before any edit - low risk, but
  re-verified by the final `just check` in step 7 rather than only asserted here.

## Proof

- `asdlc record-check TASK-1.145 --in <worktree> --as bruk.w3 -- just test-all`
  (before the fix: the two named tests fail - AC #3's "before" half).
- `asdlc record-check TASK-1.145 --in <worktree> --as bruk.w3 -- just check-packages`
  (after the manifest edits, before the text fix: the same two tests now fail in
  the alone build too - the direct evidence for AC #1 that nothing else gives).
- `asdlc record-check TASK-1.145 --in <worktree> --as bruk.w3 -- just check-packages`
  (after the text fix: green).
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
<!-- SECTION:NOTES:END -->
