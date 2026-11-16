---
id: DRAFT-1
title: >-
  Rule: check the plan's proof list row by row before citing controls as
  recorded
status: Draft
assignee: []
created_date: '2026-11-16T14:36:00Z'
labels:
  - rule-change
dependencies: []
references:
  - TASK-1.46
  - TASK-1.47
type: task
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found twice in review: a plan's own proof section lists specific cases/rows each needing a recorded negative-control run, but the worker's recorded notes cover some of them while silently skipping or only describing (not recording) others - in TASK-1.46 (session-scope filter, creation-cascade order, and exit-driven cascade order each had no recorded failing run, despite the plan's own §10 naming a control for each) and TASK-1.47 (controls described as "run by hand and reverted" rather than recorded through `asdlc record-check`, for every row the plan's proof section named).

Add to plugins/team/rules/testing.md: before declaring a task done, list every case/row the plan's own proof section names, and beside each one say whether a recorded run exists (via `asdlc record-check`) of that exact test failing with its mechanism removed - not merely a doc comment describing what such a run would show, and not a hand-run-and-reverted demonstration kept only in prose. A row with no recorded run is either given one or named as dropped with a reason, before the task is marked done - the same discipline review.md already asks a reviewer to apply afterward, moved earlier so it is not first caught in review.
<!-- SECTION:DESCRIPTION:END -->
