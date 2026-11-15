---
id: TASK-1.51
title: 'CLI: act on the terminal it runs in'
status: To Do
assignee: []
created_date: '2026-11-15T01:33:00Z'
updated_date: '2026-11-15T20:05:00Z'
labels:
  - size-2
milestone: m-3
dependencies:
  - TASK-1.49
  - TASK-1.40
parent_task_id: TASK-1
type: task
project: bruk-io-steg
asdlc_accepted_from: DRAFT-21
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
doc-2 R33 and section 4.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Inside a steg terminal a command with no target acts on that pane, tab or session
- [ ] #2 Outside one, a command that needs a target and is given none fails with status 2 and says so
- [ ] #3 The `steg` a program finds in a pane is the one that belongs to that pane's server
<!-- AC:END -->
