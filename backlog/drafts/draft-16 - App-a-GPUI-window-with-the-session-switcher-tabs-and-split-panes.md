---
id: DRAFT-16
title: 'App: a GPUI window with the session switcher, tabs and split panes'
status: Draft
assignee: []
created_date: '2026-11-13T22:52:00Z'
updated_date: '2026-11-13T23:05:00Z'
labels:
  - size-5
milestone: m-0
dependencies:
  - DRAFT-11
  - DRAFT-12
parent_task_id: DRAFT-1
type: task
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
