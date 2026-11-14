---
id: DRAFT-23
title: 'First release check: a session survives the app, end to end'
status: Draft
assignee: []
created_date: '2026-11-13T22:52:00Z'
updated_date: '2026-11-14T00:06:00Z'
labels:
  - size-3
milestone: m-0
dependencies:
  - DRAFT-15
  - DRAFT-18
  - DRAFT-20
  - DRAFT-21
  - DRAFT-22
parent_task_id: DRAFT-1
type: task
project: bruk-io-steg
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Proves the first release as a person would use it, through the real server, CLI and app: R2, R3, R19, R22 and R58.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A scripted run starts a program, quits the app, reopens it, and finds the program still running with its screen and scrollback (R2)
- [ ] #2 The same run measures typing latency and the 100 MB print against R19 and records the numbers
- [ ] #3 A check shows the server and app make no network connection in that run (R58)
<!-- AC:END -->
