---
id: DRAFT-22
title: 'App: connect to the server and hold what it reports'
status: Draft
assignee: []
created_date: '2026-11-15T01:33:00Z'
labels:
  - size-3
milestone: m-0
dependencies:
  - DRAFT-16
parent_task_id: TASK-1
type: task
project: bruk-io-steg
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
doc-10 R20 and R22; doc-15 R87 and R90. decision-5. The app draws what the server reports and never keeps a copy it treats as the truth.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The app opens a window with no config file, starting a server if none is running
- [ ] #2 A window holds a client for the server it uses, and which session it shows is the app's own state
- [ ] #3 A second window can be opened; two windows on one session show the same tab, pane and layout
- [ ] #4 After the connection drops and returns, what the window shows is what the server reports
<!-- AC:END -->
