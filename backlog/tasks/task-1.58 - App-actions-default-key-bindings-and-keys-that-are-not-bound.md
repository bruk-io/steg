---
id: TASK-1.58
title: 'App: actions, default key bindings, and keys that are not bound'
status: To Do
assignee: []
created_date: '2026-11-15T01:33:00Z'
updated_date: '2026-11-18T02:09:00Z'
labels:
  - size-3
milestone: m-4
dependencies:
  - TASK-1.121
parent_task_id: TASK-1
type: task
project: bruk-io-steg
asdlc_accepted_from: DRAFT-28
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
doc-10: R28, R29.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Every action can be bound to a key, and the defaults the design names ship with the app
- [ ] #2 A key that is not bound reaches the terminal unchanged
- [ ] #3 A key press never waits on anything outside the app and the server
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Dependency changed from TASK-1.52 to TASK-1.121 by @bruk.o on 2026-11-17, agreed with the engineer: actions and key bindings build on the window's key handling (TASK-1.121: the key table, the writer), not on the connection model. If the plan finds it needs TASK-1.52, it says so and the dependency goes back.
<!-- SECTION:NOTES:END -->
