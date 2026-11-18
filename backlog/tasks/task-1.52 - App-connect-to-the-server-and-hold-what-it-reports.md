---
id: TASK-1.52
title: 'App: connect to the server and hold what it reports'
status: To Do
assignee: []
created_date: '2026-11-15T01:33:00Z'
updated_date: '2026-11-18T02:09:00Z'
labels:
  - size-3
milestone: m-4
dependencies:
  - TASK-1.46
parent_task_id: TASK-1
type: task
project: bruk-io-steg
asdlc_accepted_from: DRAFT-22
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

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Carried from TASK-1.70 (2026-11-15): the sdk's local-socket provider is given the path of the binary to run to start a server. The CLI can use its own executable; the app is a separate program, so it cannot. This task's plan says how the app finds the steg binary (R61: one bundle). Also, as the first app task with something on screen: a change to what the app draws needs an image in its pull request (team rule), so this task or TASK-1.53, whichever draws first, works out how a worker captures the built app's window and attaches it.

Note by @bruk.o, 2026-11-17: criterion 1 (the app opens a window with no config file, starting a server if none is running) is already delivered by the bare window (TASK-1.117, crates/app/src/startup.rs). The plan should start from that and say what is left: a client per window, which session it shows, a second window, and what the window shows after the connection drops and returns (today a lost connection has no way back short of relaunching - found on TASK-1.121).
<!-- SECTION:NOTES:END -->
