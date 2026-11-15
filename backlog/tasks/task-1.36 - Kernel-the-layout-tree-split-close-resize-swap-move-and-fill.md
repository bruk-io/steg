---
id: TASK-1.36
title: 'Kernel: the layout tree: split, close, resize, swap, move and fill'
status: In Progress
assignee:
  - '@bruk.w5'
created_date: '2026-11-15T01:32:00Z'
updated_date: '2026-11-15T03:41:00Z'
labels:
  - size-3
milestone: m-0
dependencies:
  - TASK-1.35
parent_task_id: TASK-1
type: task
project: bruk-io-steg
asdlc_accepted_from: DRAFT-6
asdlc_branch: task-1.36-kernel-the-layout-tree-split-close
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
doc-15: R9, R78, R95, R96, R100. Bruk confirmed R95 and R96 and set the smallest pane at 20 columns by 4 rows on 2026-11-15.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A split divides only the target pane, in the given direction, and the new pane is the tab's current pane
- [ ] #2 Closing a pane gives its space to the other child of its split, and nothing else moves
- [ ] #3 Resize changes a ratio; swap and move to another tab keep the tree valid
- [ ] #4 Filling a pane changes nothing in the tree, and at most one pane fills a tab
- [ ] #5 A split that would leave either pane under 20 columns by 4 rows is refused, and a resize stops at that floor (R100)
- [ ] #6 A tab whose total size shrinks refuses and closes nothing, and its panes may go under the floor (R100)
<!-- AC:END -->
