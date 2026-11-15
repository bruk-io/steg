---
id: DRAFT-21
title: 'CLI: act on the terminal it runs in'
status: Draft
assignee: []
created_date: '2026-11-15T01:33:00Z'
labels:
  - size-2
milestone: m-0
dependencies:
  - DRAFT-19
  - DRAFT-10
parent_task_id: TASK-1
type: task
project: bruk-io-steg
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
