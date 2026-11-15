---
id: TASK-1.35
title: 'Kernel: sessions, tabs and panes as one tree, with its rules'
status: In Progress
assignee:
  - '@bruk.w3'
created_date: '2026-11-15T01:32:00Z'
updated_date: '2026-11-15T02:23:00Z'
labels:
  - size-3
milestone: m-0
dependencies: []
parent_task_id: TASK-1
type: task
project: bruk-io-steg
asdlc_accepted_from: DRAFT-5
asdlc_branch: task-1.35-kernel-sessions-tabs-and-panes-as-one
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
doc-15: R6, R8, R12, R26, R93, R94. The kernel is logic only (the repository's architecture rule): data in, data out, no I/O.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Sessions and tabs can be created, labelled, reordered and closed, and each pane, tab and session has exactly one parent
- [ ] #2 Closing a tab's last pane closes the tab, and closing a session's last tab ends the session
- [ ] #3 A new session takes the next of the five colours not in use, and its colour can be changed
- [ ] #4 A pane has a kind, and nothing in the tree assumes it is a terminal
- [ ] #5 The crate does no I/O, and the boundaries check passes
<!-- AC:END -->
