---
id: DRAFT-6
title: 'Kernel: the layout tree: split, close, resize, swap, move and fill'
status: Draft
assignee: []
created_date: '2026-11-15T01:32:00Z'
labels:
  - size-3
milestone: m-0
dependencies:
  - DRAFT-5
parent_task_id: TASK-1
type: task
project: bruk-io-steg
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
doc-15: R9, R78, R95, R96. doc-15's open questions on the smallest pane and on whether R95 and R96 stand are Bruk's to answer before the plan is approved.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A split divides only the target pane, in the given direction, and the new pane is the tab's current pane
- [ ] #2 Closing a pane gives its space to the other child of its split, and nothing else moves
- [ ] #3 Resize changes a ratio; swap and move to another tab keep the tree valid
- [ ] #4 Filling a pane changes nothing in the tree, and at most one pane fills a tab
- [ ] #5 A split that would take a pane below the kernel's floor in rows and columns is refused
<!-- AC:END -->
