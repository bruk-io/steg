---
id: TASK-1.136
title: >-
  ui: the cursor fade's named values, exported from the design, and whole-number
  lists in the generator
status: In Review
assignee:
  - '@bruk.w5'
created_date: '2026-11-18T04:42:00Z'
updated_date: '2026-11-21T22:10:00Z'
labels:
  - size-2
milestone: m-4
dependencies: []
references:
  - task-1.136-ui-the-cursor-fade-s-named-values
  - https://github.com/bruk-io/steg/pull/91
parent_task_id: TASK-1
project: bruk-io-steg
asdlc_accepted_from: DRAFT-1
asdlc_branch: task-1.136-ui-the-cursor-fade-s-named-values
asdlc_plan_approved_by: bruk
asdlc_plan_approved_hash: '7e94152ad20d'
asdlc_plan_approved_size: size-2
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Split from TASK-1.127 by its plan on 2026-11-18 (the default is to split). doc-17 R120 to R122: every duration a component uses is a named value from foundations, the design artifact is the source, and the export is a deliberate step that arrives as a pull request. The cursor fade's values exist today only on the design's Cursor atom (Atoms, card 07) and its exploration board; they are not on the Foundations page and not in foundations.json, so TASK-1.127 cannot meet its first criterion. The orchestrator adds the values to the design's Foundations page and hands the worker the exact values and the artifact version, since workers cannot read the artifact. The values, from the Cursor atom's own keyframes: cycle 1400 ms; pieces indexed row-major (0 top-left to 5 bottom-right) go out in the order 3, 0, 4, 2, 1, 5 and return in the reverse order; each step is 3 percent of the cycle; medium shade is half way between the cursor colour and the ground. The idle time is the engineer's to settle (TASK-1.127's plan proposes seven cycles, about ten seconds). TASK-1.127's plan found the generator renders a list of plain integers as floats; that fix belongs here. TASK-1.127 depends on this task.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 crates/ui/design/foundations.json carries the cursor fade's values under the names the design's Foundations page gives them: the cycle's duration, the order of the six pieces, the step as a fraction of the cycle, the strength of medium shade, and the idle time before the cursor rests; the file records the artifact version it was exported from, and the pull request's diff shows each value
- [ ] #2 The generator writes a list of whole numbers with no unit as integers, not floats, shown by a test against a fixture with a recorded failing control; existing generated values are unchanged, shown by an empty diff of the generated file apart from the new entries
- [ ] #3 just check passes, including the foundations checks; the notes say how a component reads the new values
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
Departure from the description, per the engineer's 2026-11-20 notes: criterion 1's "idle time before the cursor rests" no longer exists (there is no idle rest; the fade runs only while the window has focus) and is not exported. The four phase fractions (whole/going/empty/returning) are exported in its place.

## 1. crates/ui/design/foundations.json: the exact shape

Bump artifact_version to "1795195942-6a0a" (version 143; the file's own convention per TASK-1.93/1.96 is to bump the whole file's version on any export touching it, even when only part of the file changed). The rest of the file is unchanged from 1794796789-1b13; check-design's Tokens/Details comparison still passes because those colours/sizes values have not moved.

Add one new entry to the "motion" list, alongside the existing motion.status-fade:

{
  "name": "motion.cursor-fade.shaded-dissolve",
  "use": "The cursor's six pieces dissolve away and reform while the window has focus, each piece passing briefly through a flat medium shade on its way out and back; stops whole the instant focus is lost. Design's exploration option 2B, 'Scattered dissolve, through medium shade' (Atoms, Cursor atom card 07; Foundations page section 12, 'Rules: accessibility') - distinct from option 1A, 'Scattered dissolve' (same piece order, no shade step), not exported here",
  "unit": "ms",
  "cycle": 1400,
  "order": [3, 0, 4, 2, 1, 5],
  "step_fraction": 0.03,
  "medium_shade_strength": 0.5,
  "phase_whole": 0.43,
  "phase_going": 0.18,
  "phase_empty": 0.21,
  "phase_returning": 0.18
}

Naming decisions, open for the engineer to overrule (section 7 below):
- The entry is named for the variant (".shaded-dissolve"), not generically "motion.cursor-fade" and not ".scattered-dissolve": the design's own exploration board has two scattered dissolves, option 1A "Scattered dissolve" (same piece order, no shade step) and option 2B "Scattered dissolve, through medium shade" (this task's value) - "scattered-dissolve" would collide with 1A if that one is ever exported later, so this entry is named for what sets 2B apart (the shade step), leaving "motion.cursor-fade.scattered-dissolve" free for 1A as a sibling entry, never a rename of this one.
- No "pieces": 6 field. The piece count is implied by order's own length; the 2-across/3-down row-major grid is how the atom draws the six positions (doc-17's own Atom test: "it draws one thing"), not a foundations value - TASK-1.127 owns that mapping.
- "order" holds only the going-out sequence. The return sequence (reverse order) is a one-line derivation (`.iter().rev()`), not a second named value - duplicating it would risk the two drifting apart with no check catching it.
- Four flat fields (phase_whole/going/empty/returning), not a nested "phases" object: `_plain_field` (section 2) has no case for a plain nested dict, and adding one would be a second, undeclared generator change beyond the whole-number-list fix this task actually owns.
- medium_shade_strength is a bare 0.5 (f32), not a colour. The atom computes the colour itself from theme.session() and theme.surface_ground() at this weight, the same way tint()'s opacity is applied by its caller rather than baked into a colour here - there is no existing mechanism in the generator for blending two theme colours into a new named one, and inventing one is out of this task's scope.

## 2. The generator fix (tools/src/steg_tools/foundations_generate.py)

Today, `_plain_field`'s list branch (for a list that isn't a list of strings) always renders `[f32; N]` with float literals, and folds in the group's own "unit" for display regardless of whether that unit actually describes the list. Two real defects follow once "order" sits beside "cycle" under the same "unit": "ms" group:
(a) the type/initialiser bug the description names: order would render as `[f32; 6]` with `3.0, 0.0, ...` instead of whole numbers.
(b) order's _DESCRIPTION const would read "order 3ms, 0ms, 4ms, 2ms, 1ms, 5ms" - wrong and misleading (the gallery shows this text; it's what a reviewer matches against the design).

Fix, reusing the existing `_DIMENSIONLESS_FIELDS` mechanism (today scalar-only: weight/weight_selected/letter_spacing - "fields that keep a group's unit sibling but aren't themselves measured in that unit") rather than inventing a second mechanism:
- Add "order", "step_fraction", "medium_shade_strength", "phase_whole", "phase_going", "phase_empty", "phase_returning" to `_DIMENSIONLESS_FIELDS`, and widen its docstring to say it now also governs the list branch, not only the scalar branch.
- In `_plain_field`'s list branch (non-string list), compute `effective_unit = None if key in _DIMENSIONLESS_FIELDS else unit` before deciding the rendering:
  - `effective_unit == "px"`: unchanged - `[Pixels; N]`, exactly today's SPACE path.
  - otherwise, if every resolved value equals its own int(): `[i32; N]` with integer literals, display joined with no unit suffix.
  - otherwise (a non-whole number, or a future ms-unit float list): `[f32; N]` as today, display still shows `effective_unit` if any.

This is a single, minimal change: SPACE (unit="px", not in `_DIMENSIONLESS_FIELDS`) takes the same path as before, byte for byte. There is no existing list in the real file with unit=None or a dimensionless-overridden unit today, so AC#2's "existing generated values are unchanged" is a true statement, checked, not assumed (step 5).

Tests to add to tools/tests/unit/test_foundations_generate.py, against a small fixture (not the real file):
- `test_a_whole_number_list_flagged_dimensionless_renders_as_integers_not_floats`: `render_plain_group("motion.cursor-fade.shaded-dissolve", {"cycle": 1400, "unit": "ms", "order": [3, 0, 4, 2, 1, 5]}, {})` asserts `"pub order: [i32; 6],"` and `"order: [3, 0, 4, 2, 1, 5],"` in the output, and that no float literal (`"3.0"`, `".0,"`) appears for it.
- `test_the_lists_description_carries_no_stray_unit_suffix`: same fixture, asserts the `_DESCRIPTION` const reads "...order 3, 0, 4, 2, 1, 5..." with no "ms" anywhere near the numbers.
- `test_a_pixel_list_is_still_rendered_as_pixels`: `render_plain_group("space", {"steps": [2, 4, 6, 8, 10, 12, 16], "unit": "px"}, {})` still yields `[Pixels; 7]` / `px(2.0)` - the explicit regression guard for AC#2's "unchanged" half.

Recorded failing control (AC#2's own requirement): at the final head, before adding "order"/the phase fields to `_DIMENSIONLESS_FIELDS` and before the list-branch fix, run the two new int-list tests through `just test-tools` and record that failing run with `asdlc record-check` (they fail against the current `[f32; N]`-always behaviour). Then make the fix, add the fields to `_DIMENSIONLESS_FIELDS`, and record a second, passing `just test-tools` run. Both runs are the one recipe, at the one head, so they're comparable.

## 3. design_pages.py's SCHEMA must learn the new entry's shape, or check-design fails

`_MOTION_ENTRY` (tools/src/steg_tools/design_pages.py) is a closed `Fields` position with no `optional` set today - every motion-list item must carry exactly name/duration/unit/use, nothing else. A second motion entry carrying cycle/order/step_fraction/medium_shade_strength/phase_* would fail `schema_violations` ("not described by the schema") the moment `just check-design` (inside `just check`, justfile:42) runs, since those fields have no position at all today. This is not a bug to route around; it is the schema doing exactly what TASK-1.96 built it to do (refuse an unrecognised key rather than silently pass it) - the fix is to describe the new shape, not bypass the check.

Widen `_MOTION_ENTRY`:
- Add the eight new fields (cycle, order, step_fraction, medium_shade_strength, phase_whole, phase_going, phase_empty, phase_returning), each `DeclaredUnseen`, sharing one reason: "the Cursor atom's own keyframes (Atoms, card 07) and the design's Foundations page section 12, 'Rules: accessibility' - not a Name/Dark/Light/Used-for or Name/Value row on Tokens.dc.html/Details.dc.html, the only two saved pages this tool reads; those pages predate this export" (the exact same shape of exception `motion.status-fade`'s own "duration" already has: "200ms is in Rules prose with no name column" - direct precedent, not a new kind of gap).
- Add `optional=frozenset({"duration", "cycle", "order", "step_fraction", "medium_shade_strength", "phase_whole", "phase_going", "phase_empty", "phase_returning"})` - "duration" becomes optional (absent on the new entry), the eight new fields are optional (absent on motion.status-fade). "name"/"unit"/"use" stay required on both.

This change is additive only: MINIMAL_DATA and every existing mutation test in tools/tests/unit/test_design_pages.py keep passing unmodified (checked by reading them: MINIMAL_DATA's motion entry supplies exactly name/duration/unit/use, all still described; no test pins `_MOTION_ENTRY`'s exact field count). `EXPECTED_SHAPES` counts page rows, not file leaves, so it's untouched. `test_leaf_classification_counts_add_up_to_the_real_files_own_leaf_count` (tools/tests/adapter/test_design_pages_real_workspace.py) computes its own expected total from the real file at test time with no hardcoded number, and `leaf_classification_counts`'s walk recurses fully into any `DeclaredUnseen` leaf (including order's six list entries) via `exhaustive_leaf_count` - so as long as every new field has a schema position (it does, all DeclaredUnseen), this test passes unmodified too; verified by reading its source, not assumed.

Add one new unit test to tools/tests/unit/test_design_pages.py: a fixture motion entry carrying every new field (`{"name": "motion.cursor-fade.shaded-dissolve", "unit": "ms", "use": "x", "cycle": 1400, "order": [3,0,4,2,1,5], "step_fraction": 0.03, "medium_shade_strength": 0.5, "phase_whole": 0.43, "phase_going": 0.18, "phase_empty": 0.21, "phase_returning": 0.18}`) alongside the existing motion.status-fade entry, asserting `validate(data, SCHEMA) == ()` - the safety net that keeps meaning something now that `_MOTION_ENTRY` describes two different shapes.

design/canvas/ is explicitly out of scope (its own README: "the durable home for... the three foundations pages (Tokens, Layout, Details) only... not the whole artifact, which has more pages (Atoms, ...) that are out of scope here"). This task does not touch design/canvas/foundations.json, gen_system.py, or project/*.dc.html - the cursor fade's values are Tier C/DeclaredUnseen from the start, exactly like motion.status-fade's duration already is, not a value this tool was ever going to check against a saved page.

## 4. Regenerate crates/ui/src/foundations/generated.rs

Run `just generate-foundations` (never hand-edit, per R121). Expect the diff against main to be exactly: the header's artifact-version line (one line, a provenance change, not a value change), and one new block (a doc comment, a struct `MotionCursorFadeShadedDissolve`, its `pub const MOTION_CURSOR_FADE_SHADED_DISSOLVE`, and its `_DESCRIPTION` const) - nothing inside any existing struct or const changes. `mod.rs`'s `pub use generated::*;` is a glob re-export (read directly; confirmed, not assumed) - no edit needed there for the new names to reach `steg_ui::foundations::MOTION_CURSOR_FADE_SHADED_DISSOLVE`.

## 5. Proof per acceptance criterion

- AC#1 (foundations.json carries the values under the design's names, records the artifact version, diff shows each value): met by step 1's JSON addition and version bump; the PR diff itself is the proof - no test needed beyond the file existing and check-design/check-foundations passing (steps 2-4).
- AC#2 (generator writes whole-number lists as integers, with a fixture test and a recorded failing control; existing values unchanged): met by step 2's tests and recorded failing-then-passing `just test-tools` runs; the "unchanged" half is shown by running `just generate-foundations` and reading `git diff crates/ui/src/foundations/generated.rs` by eye (step 4) plus the fixed `test_a_pixel_list_is_still_rendered_as_pixels` regression guard.
- AC#3 (just check passes including the foundations checks; notes say how a component reads the values): `just check` run and recorded via `asdlc record-check` at the final head (this exercises check-foundations, check-foundations-public-surface, check-design, and test-tools together, among everything else `check` runs). The "how a component reads them" sentence for the notes: TASK-1.127's Cursor atom reads `steg_ui::foundations::MOTION_CURSOR_FADE_SHADED_DISSOLVE` - `.cycle` (`Duration`) sets the animation's period (GPUI's `Animation`/an interval, per doc-17's animation.rs reference); `.order` ([i32; 6]) gives the going-out sequence, reversed by the atom itself for the return (`.iter().rev()`, never a second named value); `.step_fraction`/`.phase_whole`/`.phase_going`/`.phase_empty`/`.phase_returning` (all f32, fractions of `.cycle`) let the atom compute each phase's and step's own duration by multiplying against `.cycle`; `.medium_shade_strength` (f32) is the blend weight the atom applies between `theme.session()` and `theme.surface_ground()` to compute the medium shade colour itself. Flag for whoever builds TASK-1.127: its own plan calls the whole-cursor phase "hold_fraction" - the exported name this task actually produces is `phase_whole`, not `hold_fraction`; TASK-1.127 must read the exported name (or its plan must be corrected to match), not the other way around, since this task's plan is what the engineer approves against the design's own value-giving wording.

## 6. Risks

- The schema-closure risk (section 3) is the real one: without widening `_MOTION_ENTRY`, `just check` fails outright on a correctly-shaped foundations.json, in a way that looks like a tooling bug rather than a missing step. Caught here, before any code is written, not discovered at review time.
- The group-level "unit" leaking into a sibling field (section 2) is the second real one: without the `_DIMENSIONLESS_FIELDS` extension, `order` would silently render as a `Duration`-flavoured float list or an incorrect "ms"-suffixed description rather than failing loudly - this is why it's a named test (two of them), not just the type fix.
- Second-hand values: the exact numbers (cycle, order, fractions) come from the orchestrator, not from reading the artifact directly (workers cannot). The only automated cross-check available is check-design, and these particular fields are Tier C by design (not drawn on Tokens/Details.dc.html), so check-design cannot catch a transcription error here. Flagged in the notes, not silently assumed correct.
- i32 vs usize for `order`: chosen i32 as the generator's generic default for "a whole number with no unit" (parallel to the existing generic f32 default for a dimensionless float) rather than usize, since the generator has no way to know the field will be used as an index; TASK-1.127 casts with `as usize` at its own call sites. Open for the engineer to override (section 7).
- Naming mismatch with TASK-1.127's own plan (section 5): that task calls the whole-piece phase "hold_fraction" while this plan exports `phase_whole`. Left to the engineer/orchestrator to reconcile which plan's wording wins, rather than this task silently renaming its own export to match a sibling task's plan.

## 7. Open questions, with recommended defaults

- Entry name "motion.cursor-fade.shaded-dissolve" (now settled against the sibling-variant collision the team lead flagged; recommended default: keep it).
- Field names phase_whole/phase_going/phase_empty/phase_returning vs TASK-1.127's own "hold_fraction" (recommended default: keep phase_whole, matching this task's own decision (a) value-giving wording "whole 43 percent, going 18, empty 21, returning 18" - TASK-1.127's plan is the one to correct, since it was written before these values existed).
- order's Rust element type, i32 vs usize (recommended default: i32, section 6).
- Whether to also add a short dedicated paragraph to crates/ui/design/README.md describing this new motion shape (recommended default: skip - the README already describes "motion" generically as "every other named value... never a sentence a program must parse," and the per-field DeclaredUnseen reasons in design_pages.py carry the detail; adding prose here would be a second place for the same explanation to drift from the first).
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Engineer's decisions of 2026-11-20 (recorded by @bruk.o), design artifact version 143 (id 1795195942-6a0a), Foundations page section 12 and the Cursor atom: (a) the animation is option 2B, as the Cursor atom already draws it: cycle 1400 ms; six pieces row-major 0 to 5; out in the order 3, 0, 4, 2, 1, 5 and back in reverse; each step 3 percent of the cycle; a piece spends one step as medium shade each way, medium shade being the flat colour half way between the cursor colour and the ground (not a dither); whole 43 percent, going 18, empty 21, returning 18. (b) There is NO idle rest: the fade runs for as long as the window has focus and stops, whole, when the window loses focus. Any idle-time value or criterion clause is dropped. (c) Reduce Motion is a stub that always allows motion until TASK-1.63, with the steady cursor built behind it. (d) The animation is meant to become a setting later: build only 2B, but hold the animation as data (order, step, shade) so another variant is an addition, not a rewrite; the setting itself is a separate follow-up task. For this task: criterion 1's 'idle time before the cursor rests' no longer exists and is not exported; export the hold, out, empty and return fractions instead if the plan needs them.

Plan checked against doc-17 (R120 to R123), decision-10 and the design artifact version 143 by @bruk.o on 2026-11-20: ok. The values match the Foundations page section 12; the entry is motion.cursor-fade.shaded-dissolve (option 2B), named so option 1A can sit beside it; the idle time is dropped by the engineer's decision; the generator fix covers both the float rendering and the leaked unit in list descriptions; the closed motion schema in design_pages.py is widened so check-design still passes. The planner's remaining defaults stand: phase_* field names, i32, no README paragraph.

Check FAILED: `just --justfile /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.136/justfile --working-directory /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.136 test-tools` exited 1 at 2026-11-21T20:00:31+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.136 on task-1.136-ui-the-cursor-fade-s-named-values at b6a744b69b31 with uncommitted changes:

```text
FAILED  tool tests (exit 1, 39.8s)
........................................................................ [ 21%]
........................................................................ [ 43%]
.......................................................FF............... [ 64%]
........................................................................ [ 86%]
..............................................                           [100%]
=================================== FAILURES ===================================
_ test_a_whole_number_list_flagged_dimensionless_renders_as_integers_not_floats _

    def test_a_whole_number_list_flagged_dimensionless_renders_as_integers_not_floats() -> None:
        rust = render_plain_group(
            "motion.cursor-fade.shaded-dissolve", {"cycle": 1400, "unit": "ms", "order": [3, 0, 4, 2, 1, 5]}, {}
        )
>       assert "pub order: [i32; 6]," in rust
E       assert 'pub order: [i32; 6],' in '#[derive(Debug, Clone, Copy)]\npub struct MotionCursorFadeShadedDissolve {\n    pub cycle: std::time::Duration,\n    ...\npub const MOTION_CURSOR_FADE_SHADED_DISSOLVE_DESCRIPTION: &str = "cycle 1400ms, order 3ms, 0ms, 4ms, 2ms, 1ms, 5ms";'

tools/tests/unit/test_foundations_generate.py:337: AssertionError
___________ test_the_lists_description_carries_no_stray_unit_suffix ____________

    def test_the_lists_description_carries_no_stray_unit_suffix() -> None:
        rust = render_plain_group(
            "motion.cursor-fade.shaded-dissolve", {"cycle": 1400, "unit": "ms", "order": [3, 0, 4, 2, 1, 5]}, {}
        )
        description = rust.split("_DESCRIPTION")[1]
>       assert "order 3, 0, 4, 2, 1, 5" in description
E       assert 'order 3, 0, 4, 2, 1, 5' in ': &str = "cycle 1400ms, order 3ms, 0ms, 4ms, 2ms, 1ms, 5ms";'

tools/tests/unit/test_foundations_generate.py:348: AssertionError
=========================== short test summary info ============================
FAILED tools/tests/unit/test_foundations_generate.py::test_a_whole_number_list_flagged_dimensionless_renders_as_integers_not_floats
FAILED tools/tests/unit/test_foundations_generate.py::test_the_lists_description_carries_no_stray_unit_suffix
2 failed, 332 passed in 39.63s
full output: .quiet/tool-tests.log
error: recipe `test-tools` failed on line 98 with exit code 1
```

Check passed: `just --justfile /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.136/justfile --working-directory /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.136 test-tools` exited 0 at 2026-11-21T20:02:36+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.136 on task-1.136-ui-the-cursor-fade-s-named-values at b6a744b69b31 with uncommitted changes:

```text
ok      tool tests (39.7s)
```

Check FAILED: `just --justfile /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.136/justfile --working-directory /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.136 check` exited 101 at 2026-11-21T20:08:52+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.136 on task-1.136-ui-the-cursor-fade-s-named-values at b6a744b69b31 with uncommitted changes, last 40 of 71 lines:

```text
test cli::output::tests::records_with_a_non_object_element_is_a_shape_error_not_a_panic ... ok
test cli::output::tests::records_with_no_declared_fields_prints_nothing_in_every_combination ... ok
test cli::exit::tests::every_described_error_maps_to_its_own_exit_status ... ok
test cli::output::tests::text_json_prints_the_string_value_unchanged ... ok
test cli::output::tests::text_plain_is_the_string_unaltered ... ok
test cli::output::tests::text_quiet_is_the_same_as_without_quiet_either_way ... ok
test cli::output::tests::text_with_a_non_string_reply_is_a_shape_error_not_a_panic ... ok
test cli::tree::tests::autostart_defaults_to_true_with_neither_flag_given ... ok
test cli::tree::tests::autostart_given_after_no_autostart_wins ... ok
test cli::tree::tests::json_is_read_from_the_leaf_subcommand ... ok
test cli::tree::tests::no_autostart_is_read_whether_given_before_or_after_the_subcommand ... ok
test cli::tree::tests::quiet_is_read_from_the_leaf_subcommand_either_spelling ... ok

failures:

---- cli::output::tests::a_field_declared_text_but_really_structured_renders_its_own_compact_json stdout ----

thread 'cli::output::tests::a_field_declared_text_but_really_structured_renders_its_own_compact_json' (70899554) panicked at crates/steg/src/cli/output.rs:593:9:
assertion `left == right` failed
  left: "t1\t{\"split\":\"horizontal\",\"panes\":[\"p1\",\"p2\"]}\n"
 right: "t1\t{\"panes\":[\"p1\",\"p2\"],\"split\":\"horizontal\"}\n"
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace

---- cli::output::tests::nested_plain_and_json_are_identical_bytes stdout ----

thread 'cli::output::tests::nested_plain_and_json_are_identical_bytes' (70899562) panicked at crates/steg/src/cli/output.rs:708:9:
assertion `left == right` failed
  left: "{\"stream\":\"st1\",\"snapshot\":{\"rows\":[]}}\n"
 right: "{\"snapshot\":{\"rows\":[]},\"stream\":\"st1\"}\n"


failures:
    cli::output::tests::a_field_declared_text_but_really_structured_renders_its_own_compact_json
    cli::output::tests::nested_plain_and_json_are_identical_bytes

test result: FAILED. 35 passed; 2 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

error: test failed, to rerun pass `-p steg --lib`
full output: .quiet/tests-every-crate.log
error: recipe `test` failed with exit code 101
```

Build blocked by an unrelated, pre-existing failure in just check's full test step: crates/steg/src/cli/output.rs's cli::output::tests::a_field_declared_text_but_really_structured_renders_its_own_compact_json and cli::output::tests::nested_plain_and_json_are_identical_bytes fail when the full test suite runs together, asserting on JSON object key order (e.g. expecting {"split":...,"panes":...} but getting {"panes":...,"split":...}), while passing every time when run alone or filtered (cargo test -p steg --lib cli::output, 6/6 clean runs). Confirmed pre-existing and unrelated to this task: stashed every change this task made and reran cargo test -p steg --lib cli::output on the resulting bare-main tree - passes there too (it's the full-suite run specifically, not this task's diff, that triggers it), then restored the stash. Nothing this task touches (crates/ui, tools/) is anywhere near crates/steg/src/cli/output.rs (from the just-merged PR #87). Recorded one failing `just check` run as the official attempt (asdlc record-check, exit 101); retried the full run 3 times beforehand for diagnosis, same two tests, same wrong order, every time - consistent, not a coin-flip hash-seed flake, so a second blind retry would not help. Flagged to the team lead for direction rather than editing code outside this task's approved plan.

The failing just check is main's own failure, confirmed by the team lead: present at d50a5d20 (the PR 87 merge) in crates/steg/src/cli/output.rs, outside this task's files. It is cargo feature unification (gpui turns on serde_json's preserve_order; -p steg alone does not, so its two tests assert sorted-key text that only matches when steg is built without the app). The two failing tests: cli::output::tests::a_field_declared_text_but_really_structured_renders_its_own_compact_json and cli::output::tests::nested_plain_and_json_are_identical_bytes. Filed as its own task; will be fixed there, not here. This task's own changes are committed locally (a3a08067), not pushed. Holding per the team lead's instruction: no further just check retries, no push, no PR until notified the fix is on main.

Check passed: `just --justfile /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.136/justfile --working-directory /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.136 check` exited 0 at 2026-11-21T21:40:31+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.136 on task-1.136-ui-the-cursor-fade-s-named-values at 774873286c86:

```text
ok      format check (0.3s)
ok      lint (3.0s)
ok      boundaries (0.1s)
ok      foundations (0.1s)
ok      foundations public surface (0.0s)
ok      design pages (0.1s)
ok      licences (0.7s)
ok      tests (every crate) (77.0s)
ok      build (steg-sdk) (3.5s)
ok      test (steg-sdk) (2.5s)
ok      test (steg-sdk, local-socket) (2.9s)
ok      build (steg-kernel) (44.9s)
ok      test (steg-kernel) (1.5s)
ok      build (steg-server) (30.9s)
ok      test (steg-server) (66.4s)
ok      test (steg-server, race-fixture) (66.5s)
ok      test (steg-server, pty-race-fixture) (106.8s)
ok      test (steg-server, accept-backoff-fixture) (93.0s)
ok      build (steg) (2.5s)
ok      test (steg) (5.0s)
ok      test (steg, fixtures) (11.8s)
ok      build (steg-ui) (16.5s)
ok      test (steg-ui) (1.4s)
ok      build (steg-app) (1.5s)
ok      test (steg-app) (2.2s)
ok      tool tests (40.3s)
ok      architecture (0.7s)
ok      recipe names (0.1s)
ok      ui levels (0.1s)
ok      ui colours (0.1s)
```

Merged origin/main (e254fa2b, carrying PR 90's output.rs fix) with no conflicts. The merge touched Cargo.toml, boundaries.toml, the four crates' own Cargo.toml, crates/steg/src/cli/output.rs, and backlog/plugins docs only - nothing under tools/ or crates/ui/ - so the test-tools control pair recorded earlier at the pre-merge head still stands; not re-recorded. Ran and recorded `just check` once at the merged head (b6a744b69b31's descendant after the merge commit): exit 0, every step green including tests (every crate) - the output.rs failure is gone, and the shutdown.rs flake the team lead named did not appear. Proceeding to push and open the PR.

PR 91's first CI run (head 77487328) failed on crates/steg/tests/cli_connection_arbitration.rs's a_hostile_first_connection_does_not_stop_a_real_autostart_from_succeeding. That test is outside this task's files (crates/ui, tools/) and passed in the local just check recorded at that same head. The team lead re-ran the failed job once and noted the sighting on TASK-1.137. Holding per the team lead's instruction: not pushing anything further, not running asdlc finish, until told the re-run is green.

Built the approved plan. crates/ui/design/foundations.json gains motion.cursor-fade.shaded-dissolve (design option 2B, artifact version 1795195942-6a0a): cycle 1400ms, order [3,0,4,2,1,5] (going-out sequence only, return is the atom's own reverse), step_fraction 0.03, medium_shade_strength 0.5, phase_whole 0.43, phase_going 0.18, phase_empty 0.21, phase_returning 0.18. Per the engineer's 2026-11-20 decision there is no idle rest, so criterion 1's "idle time before the cursor rests" is dropped, not exported - the phase fractions stand in its place, the one departure the plan itself already named, not a new one.

Fixed foundations_generate.py's list branch (a field in _DIMENSIONLESS_FIELDS is now exempted from the group's unit for lists, the same as it already was for scalars): a whole-number list with no effective unit renders [i32; N], not [f32; N], with no stray unit suffix in its _DESCRIPTION. Widened design_pages.py's _MOTION_ENTRY so check-design describes the new entry's shape instead of refusing it.

Control pair recorded at the final head (774873286c86): the generator fix's failing-then-passing `just test-tools` pair was recorded at the pre-merge head (b6a744b69b31) before/after the fix, confirmed still valid post-merge since the merge touched nothing under tools/ or crates/ui/ (checked by diff, stated in the task notes) - not re-recorded. `just check` itself is recorded green at the final head (774873286c86, exit 0).

PR 91's first CI run failed on crates/steg/tests/cli_connection_arbitration.rs, outside this task's files and passing in the local just check at the same head (noted on the task and on TASK-1.137); the team lead re-ran the job once and it is green on this same head.

generated.rs's diff against the prior head is exactly the artifact-version line and the one new struct/const/description block; nothing in any existing entry changed.
<!-- SECTION:NOTES:END -->

## Comments

<!-- COMMENTS:BEGIN -->
author: @bruk
created: 2026-11-21T19:55:00Z

---

Plan approved by @bruk (7e94152ad20d, size-2): its worker may build it.

---
<!-- COMMENTS:END -->
