---
id: DRAFT-35
title: 'First release check: a session survives the app, end to end'
status: Draft
assignee: []
created_date: '2026-11-15T01:33:00Z'
labels:
  - size-2
milestone: m-0
dependencies:
  - DRAFT-26
  - DRAFT-20
parent_task_id: TASK-1
type: task
project: bruk-io-steg
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
doc-8 R2. This is what the first release exists to prove.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 With a program printing, the app is killed and reopened, and the layout, screen and scrollback are as the server has them
- [ ] #2 The CLI acts on the same session while the app is closed, and the app shows the result when it returns
<!-- AC:END -->
