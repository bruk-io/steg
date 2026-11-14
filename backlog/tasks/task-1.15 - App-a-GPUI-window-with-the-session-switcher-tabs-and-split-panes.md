---
id: TASK-1.15
title: 'App: a GPUI window with the session switcher, tabs and split panes'
status: To Do
assignee: []
created_date: '2026-11-13T22:52:00Z'
updated_date: '2026-11-14T03:56:00Z'
labels:
  - size-5
milestone: m-0
dependencies:
  - TASK-1.10
  - TASK-1.11
  - TASK-1.27
parent_task_id: TASK-1
type: task
project: bruk-io-steg
asdlc_accepted_from: DRAFT-16
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
R20: a window shows a session switcher, the current session's tabs and the current tab's panes, and more than one window can be open. R22: fully usable with no config file. The app holds no session state: it draws what the server reports and follows its events.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The app starts with no config file, starts a server if none is running, and shows a session (R22)
- [ ] #2 Sessions, tabs and panes changed from the CLI appear in the app without a restart
- [ ] #3 Two windows can be open at once (R20)
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
From TASK-1.27's worker, relayed by @bruk.o: a design canvas renders blur, materials and shadow stacks that GPUI cannot draw, so check the look in real GPUI. A gpui-app plugin in Bruk's plugin cache has a gpui-dev-loop skill and a gpuictl shot harness that screenshots a running GPUI app; it is not enabled for workers by default, so ask @bruk.o if it is needed.
<!-- SECTION:NOTES:END -->
