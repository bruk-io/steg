---
id: DRAFT-3
title: 'Rule: record a negative control, don''t just narrate it'
status: Draft
assignee: []
created_date: '2026-11-20T18:32:00Z'
labels:
  - rule-change
dependencies: []
references:
  - TASK-1.120
  - TASK-1.49
type: task
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found twice in review: a negative control the approved plan names as needed was performed (or is described as load-bearing) but never recorded as a failing-then-passing pair through `asdlc record-check` - only narrated in prose in the task's own notes after the fact. TASK-1.49 (comment, "[Evidence, Important] No recorded failing run exists for any of the seven negative controls...the worker's notes additionally describe two controls as 'performed by hand and reverted before committing'...these were never recorded through asdlc record-check, so there is no recorded failing run behind them either, only a narrative description after the fact") and TASK-1.120 (the plan's proof section names three negative controls; only two are recorded as failing-then-passing pairs on the task - the third, decision 3's Nothing-arm-with-json control, is described nowhere on the task at all, recorded or narrated, and the reviewer had to reproduce it independently to confirm the mechanism is real).

Add to plugins/team/rules/testing.md, after the existing "A test offered as proof that a mechanism works has been seen to fail with the mechanism removed" line: every negative control the plan names is recorded as a failing-then-passing `asdlc record-check` pair on the task, in that order, before the task is finished - a control only described in prose, or performed but not recorded, is the same as a control never run, because a reviewer (or anyone later) cannot tell a real result from a confident narration of one.
<!-- SECTION:DESCRIPTION:END -->
