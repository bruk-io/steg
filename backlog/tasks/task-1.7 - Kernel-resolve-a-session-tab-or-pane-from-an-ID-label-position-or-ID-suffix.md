---
id: TASK-1.7
title: >-
  Kernel: resolve a session, tab or pane from an ID, label, position or ID
  suffix
status: To Do
assignee: []
created_date: '2026-11-13T22:52:00Z'
updated_date: '2026-11-14T00:59:00Z'
labels:
  - size-3
milestone: m-0
dependencies:
  - TASK-1.6
parent_task_id: TASK-1
type: task
project: bruk-io-steg
asdlc_accepted_from: DRAFT-8
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
R10. Each is addressable by full ID, label, position, or an ID suffix of four or more characters. An ambiguous target is an error, never a guess.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Unit tests cover each of the four ways of addressing, for sessions, tabs and panes
- [ ] #2 A suffix shorter than four characters, and a target matching more than one thing, are each refused with an error naming the candidates
<!-- AC:END -->
