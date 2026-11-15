---
id: TASK-1.55
title: 'App: panes, their headers, and the layout'
status: To Do
assignee: []
created_date: '2026-11-15T01:33:00Z'
updated_date: '2026-11-15T02:15:00Z'
labels:
  - size-4
milestone: m-0
dependencies:
  - TASK-1.53
parent_task_id: TASK-1
type: task
project: bruk-io-steg
asdlc_accepted_from: DRAFT-25
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
doc-10 R20; doc-15 R78, R95, R96. The app's look and behaviour is the design artifact (doc-10 says which page is the source for what). Until workers can read artifacts (asdlc intent workers-read-artifacts), the orchestrator hands over what the artifact says for this task.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Panes are drawn from the server's layout, with the design's gap, and the active pane is marked the three ways the design gives
- [ ] #2 A pane's header shows its title, directory and controls, and drops parts in the design's order as the pane narrows
- [ ] #3 Dragging the gap between two panes resizes them; split, fill and close work from the header
- [ ] #4 A split the layout would refuse shows its control as unusable
<!-- AC:END -->
