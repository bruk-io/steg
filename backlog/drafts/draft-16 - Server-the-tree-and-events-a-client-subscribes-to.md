---
id: DRAFT-16
title: 'Server: the tree, and events a client subscribes to'
status: Draft
assignee: []
created_date: '2026-11-15T01:33:00Z'
labels:
  - size-3
milestone: m-0
dependencies:
  - DRAFT-15
  - DRAFT-12
  - DRAFT-11
parent_task_id: TASK-1
type: task
project: bruk-io-steg
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
doc-8 R38 and doc-14. A client asks for the tree once and then receives changes.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 One request returns the tree: sessions, tabs and panes with their focus fields, layout and status
- [ ] #2 Events report sessions, tabs and panes created, closed and changed; a focus change is one event carrying the whole path; a layout change carries the tab's layout whole
- [ ] #3 Events report status changes and clients connecting, leaving, and starting or stopping viewing a session
- [ ] #4 One event can be addressed to a single client, asking it to show a session
- [ ] #5 A subscription can be narrowed to named events on the server
<!-- AC:END -->
