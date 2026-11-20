---
id: TASK-1.136
title: >-
  ui: the cursor fade's named values, exported from the design, and whole-number
  lists in the generator
status: In Progress
assignee:
  - '@bruk.w5'
created_date: '2026-11-18T04:42:00Z'
updated_date: '2026-11-20T17:32:00Z'
labels:
  - size-2
milestone: m-4
dependencies: []
parent_task_id: TASK-1
project: bruk-io-steg
asdlc_accepted_from: DRAFT-1
asdlc_branch: task-1.136-ui-the-cursor-fade-s-named-values
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

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Engineer's decisions of 2026-11-20 (recorded by @bruk.o), design artifact version 143 (id 1795195942-6a0a), Foundations page section 12 and the Cursor atom: (a) the animation is option 2B, as the Cursor atom already draws it: cycle 1400 ms; six pieces row-major 0 to 5; out in the order 3, 0, 4, 2, 1, 5 and back in reverse; each step 3 percent of the cycle; a piece spends one step as medium shade each way, medium shade being the flat colour half way between the cursor colour and the ground (not a dither); whole 43 percent, going 18, empty 21, returning 18. (b) There is NO idle rest: the fade runs for as long as the window has focus and stops, whole, when the window loses focus. Any idle-time value or criterion clause is dropped. (c) Reduce Motion is a stub that always allows motion until TASK-1.63, with the steady cursor built behind it. (d) The animation is meant to become a setting later: build only 2B, but hold the animation as data (order, step, shade) so another variant is an addition, not a rewrite; the setting itself is a separate follow-up task. For this task: criterion 1's 'idle time before the cursor rests' no longer exists and is not exported; export the hold, out, empty and return fractions instead if the plan needs them.
<!-- SECTION:NOTES:END -->
