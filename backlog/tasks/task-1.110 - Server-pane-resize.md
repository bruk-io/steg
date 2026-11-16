---
id: TASK-1.110
title: 'Server: pane resize'
status: To Do
assignee: []
created_date: '2026-11-16T04:33:00Z'
labels:
  - size-3
milestone: m-0
dependencies:
  - TASK-1.89
parent_task_id: TASK-1
project: bruk-io-steg
asdlc_accepted_from: DRAFT-2
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Split from TASK-1.89 on 2026-11-15: its plan found that pane resize is a design question, not wiring. Tree::resize_pane takes a ratio; the command gives cells; Tab::size() can be none. doc-2 section 8, decision-6, doc-15 (layout).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 pane resize is built as doc-2 section 8 gives it: growing or shrinking a pane by a number of cells in a direction changes the split's ratio in the kernel, and the record returned shows the result
- [ ] #2 The plan settles what a number of cells means when the tab's size is not known (no client is viewing it, so the kernel has no cell basis): refuse with a stated code, apply against a stated default, or accept a ratio as well; and what happens at the smallest pane the design gives and when there is no split in that direction. These are stated for the engineer, not assumed (decision-6: the latest client sets a terminal's size)
- [ ] #3 The conversion from cells to a ratio is logic with no I/O and has its own tests at the edges (zero, one cell, more than the pane has, an odd size); the method is proved through Runtime::handle and over a real socket; it leaves the not-yet list in dispatch
<!-- AC:END -->
