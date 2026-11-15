---
id: DRAFT-1
title: 'Rule: record every plan-required proof step as a run, not a description'
status: Draft
assignee: []
created_date: '2026-11-15T03:10:00Z'
labels:
  - rule-change
dependencies: []
references:
  - TASK-1.33
  - TASK-1.32
type: task
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found twice in review: a proof step the approved plan calls for was only described in prose in the task's notes, with no recorded run backing it, in TASK-1.33 and TASK-1.32.

- TASK-1.32: the plan's Proof step 1 called for a local `just test-all` pass alongside `just check`; the notes recorded only `just check`, no local `just test-all` run (flagged as a Nit since GitHub Actions had run `test-all` for real, so the gap was covered by other evidence).
- TASK-1.33: the plan's proof for AC #5 called for `just check` to be run once against a deliberately broken model and the failing outcome "recorded on the task"; the notes instead say "proved by hand, not recorded as a check." The plan's proof for AC #6 called for every element's title to be "listed against its source in the build notes"; no such listing exists anywhere in the notes - the step is simply absent, not even described.

Add to plugins/team/rules/evidence.md (new file): when a plan's proof section names a specific run (a command, an expected outcome, a comparison to be shown), the worker records that run on the task with `asdlc record-check` or an equivalent explicit entry before marking the acceptance criterion done - a sentence in the notes asserting the outcome is not a substitute, even for a demonstration meant to be reverted. The reviewer's Evidence pass already treats this as a finding (review.md); the gap is that workers keep omitting the recorded run rather than the policy failing to say so, so the correction belongs in a rule workers read while building, not only in the reviewer's checklist.
<!-- SECTION:DESCRIPTION:END -->
