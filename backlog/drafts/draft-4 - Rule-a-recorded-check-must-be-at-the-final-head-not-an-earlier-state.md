---
id: DRAFT-4
title: 'Rule: a recorded check must be at the final head, not an earlier state'
status: Draft
assignee: []
created_date: '2026-11-22T20:55:00Z'
labels:
  - rule-change
dependencies: []
references:
  - TASK-1.153
  - TASK-1.32
type: task
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found twice in review: a check or negative control is recorded against an earlier, uncommitted or pre-merge state and then the notes claim it covers the final head, in TASK-1.153 and TASK-1.32.

- TASK-1.153: the notes claim "Every control recorded failing-then-passing via asdlc record-check at the final head (5eb25574)". All seven recorded pairs (three cursor-only-Delta tests, two idle-stays-silent tests, the stale-caller test, the model.rs test) are stamped "... at 8e720fcfaf1a with uncommitted changes" - a board-only commit, not this branch's one real commit. Reproducing one control at the true final head gave the same failure but at a different line number than recorded (1883 vs 1884), and the other three recorded panic lines are each two lines off their counterparts at the final head - proving edits (the plan's own doc-comment step) landed in the ~23 minutes between the last individual recording and the final `just check`. Only the aggregate `just check` was actually run at the final head; none of the seven mechanism-specific negative controls were.
- TASK-1.32: the plan's Proof step ("just check and just test-all both pass locally first") was recorded for `just test-all` at an earlier commit, not the branch's final commit (which absorbed a merge of main's intervening changes); `just check` was correctly rerun at the final head but `just test-all` was not. The reviewer judged it a Nit because other evidence (GitHub Actions runs) covered the gap, but flagged it under the same policy line.

Add to plugins/team/review.md's Evidence pass (or a rules file it already cites): when a worker records a check or a negative control mid-session and the branch is committed or merged afterward, the last recorded run of each check or control named as proof must be re-run and re-recorded at the branch's actual final commit before the task goes to review - an aggregate check (e.g. `just check`) passing at the final head does not stand in for a mechanism-specific negative control recorded earlier, because the aggregate only proves the mechanism present, never the control's failing half.
<!-- SECTION:DESCRIPTION:END -->
