---
id: DRAFT-12
title: 'Kernel: clients, what each views, and the size a terminal takes'
status: Draft
assignee: []
created_date: '2026-11-15T01:32:00Z'
labels:
  - size-3
milestone: m-0
dependencies:
  - DRAFT-9
parent_task_id: TASK-1
type: task
project: bruk-io-steg
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
doc-15: R86, R89, R91. decision-5 and decision-6.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A client can connect and leave, and which sessions it views follows from what it is attached to
- [ ] #2 The latest client in a session is the last to send input or declare a new size
- [ ] #3 A terminal takes the size the latest client declared for it, and every other viewer is told that size
<!-- AC:END -->
