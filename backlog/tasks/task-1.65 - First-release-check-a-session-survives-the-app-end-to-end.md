---
id: TASK-1.65
title: 'First release check: a session survives the app, end to end'
status: To Do
assignee: []
created_date: '2026-11-15T01:33:00Z'
updated_date: '2026-11-16T11:36:00Z'
labels:
  - size-2
milestone: m-4
dependencies:
  - TASK-1.56
  - TASK-1.50
  - TASK-1.115
parent_task_id: TASK-1
type: task
project: bruk-io-steg
asdlc_accepted_from: DRAFT-35
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
doc-8 R2. This is what the first release exists to prove.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 With a program printing, the app is killed and reopened, and the layout, screen and scrollback are as the server has them
- [ ] #2 The CLI acts on the same session while the app is closed, and the app shows the result when it returns
- [ ] #3 R58 (doc-13): with the app, server and CLI in use, the system makes no network call the person did not ask for, shown by a recorded run; there is no telemetry, account or licence check in the code
<!-- AC:END -->
