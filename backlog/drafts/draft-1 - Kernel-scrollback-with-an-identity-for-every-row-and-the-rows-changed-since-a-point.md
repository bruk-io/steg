---
id: DRAFT-1
title: >-
  Kernel: scrollback with an identity for every row, and the rows changed since
  a point
status: Draft
assignee: []
created_date: '2026-11-15T14:15:00Z'
labels:
  - size-3
milestone: m-0
dependencies:
  - TASK-1.39
parent_task_id: TASK-1
type: task
project: bruk-io-steg
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
doc-9 R15; decision-2 on snapshots, updates and scrollback pages. Split from TASK-1.39 on 2026-11-15 by Bruk. It carries the two open design questions that task's plan named: whether a row's identity survives the engine reflowing text on a resize, and how change is tracked once more than one reader reads at different points. The earlier plan on TASK-1.39 has a design for both to start from.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Scrollback is kept to a configurable limit, 10,000 lines by default, and the real count kept is reported
- [ ] #2 Every row has an absolute identity that only grows and is never given to different content, shown by feeding enough lines to drop rows
- [ ] #3 What happens to row identity when a resize reflows scrollback is established by test and stated, not assumed
- [ ] #4 The rows changed since a given point can be asked for, so an update need not carry the whole screen; a reader too far behind gets everything
- [ ] #5 The plan says how this serves several readers at different points, or names the task that will, without closing off either option
<!-- AC:END -->
