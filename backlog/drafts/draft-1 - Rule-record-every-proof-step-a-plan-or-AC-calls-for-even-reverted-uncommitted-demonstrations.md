---
id: DRAFT-1
title: >-
  Rule: record every proof step a plan or AC calls for, even
  reverted/uncommitted demonstrations
status: Draft
assignee: []
created_date: '2026-11-15T02:54:00Z'
labels:
  - rule-change
dependencies: []
references:
  - TASK-1.69
  - TASK-1.32
type: task
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found twice in review: a plan or acceptance criterion calls for a specific demonstration or proof step, and the worker's notes assert it happened but no `asdlc record-check` run backs it, in TASK-1.69 and TASK-1.32.

- TASK-1.32 (reviewed by @bruk.r2): the plan's Proof step 1 calls for a local `just test-all` run before relying on CI; only a local `just check` is recorded. Logged as a Nit because a stronger CI run covered the same ground.
- TASK-1.69 (reviewed by @bruk.r1): AC#3 explicitly requires `just boundaries` failing on a disallowed dependency "shown by a recorded run." The worker ran the two live demonstrations, then reverted the manifest edits, and the notes say this wasn't recorded "since nothing was committed in that state" - but the same task's own notes contain a FAILED `just check` recorded *with uncommitted changes* at 02:43:17, proving `asdlc record-check` does capture a failing command on a dirty tree. No recorded run of `just boundaries` failing exists anywhere on the task. Logged as Important: nothing else evidences AC#3.

Add to plugins/team/review.md or a relevant rule file: a plan or acceptance criterion that calls for a demonstration (including one meant to be reverted or never committed, such as proving a check fails on a disallowed change) is not satisfied by prose in the task's notes alone - run `asdlc record-check` on it before reverting, the same as any other check. The reason: `asdlc record-check` can and does record a run against an uncommitted or soon-to-be-reverted tree state (TASK-1.69's FAILED entry at 02:43:17 shows this working), so "nothing was committed" is not a reason to skip recording it.
<!-- SECTION:DESCRIPTION:END -->
