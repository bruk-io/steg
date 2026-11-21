---
id: DRAFT-1
title: 'Rule: check what makes a negative control pass, not just that it passes'
status: Draft
assignee: []
created_date: '2026-11-21T22:22:00Z'
labels:
  - rule-change
dependencies: []
references:
  - TASK-1.136
  - TASK-1.81
type: task
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found twice in review: a test/control that is cited as proof a specific mechanism works, but an earlier or independent branch in the code under test produces the same outcome regardless of that mechanism - so the control cannot actually discriminate "mechanism present" from "mechanism absent," in TASK-1.136 and TASK-1.81.

TASK-1.81: api_call_refuses_to_call_itself was reverted to pre-fix behaviour and still passed, because the reverted code returned the same BadRequest error for an unrelated reason (a missing "target" key) before ever reaching the method-name check the test was meant to prove.

TASK-1.136: the two new tests (test_a_whole_number_list_flagged_dimensionless_renders_as_integers_not_floats, test_the_lists_description_carries_no_stray_unit_suffix) are cited as proof that adding "order" to _DIMENSIONLESS_FIELDS fixes the list-branch bug. Removing "order" from that set alone (leaving the rest of the fix in place) leaves both tests green and the whole `just test-tools` suite green, because an independent, unconditional "every value is a whole number" branch added in the same diff renders the dimensionless-set membership irrelevant for this particular field.

Add to plugins/team/review.md (or a rules/ file it points to): when repeating a negative control, don't just revert the named mechanism and check the control still fails - trace *why* it fails, and check that no other branch, check, or earlier condition in the same code path would produce an indistinguishable result on its own. A control that still passes (or still fails for the "right-looking" reason but by the wrong path) with its named mechanism removed is not evidence for that mechanism, even if a different mechanism nearby is genuinely covered by the same test.
<!-- SECTION:DESCRIPTION:END -->
