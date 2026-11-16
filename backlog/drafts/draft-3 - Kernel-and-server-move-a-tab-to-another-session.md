---
id: DRAFT-3
title: 'Kernel and server: move a tab to another session'
status: Draft
assignee: []
created_date: '2026-11-16T04:33:00Z'
labels:
  - size-3
milestone: m-0
dependencies:
  - TASK-1.89
parent_task_id: TASK-1
project: bruk-io-steg
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Split from TASK-1.89 on 2026-11-15: dispatch answers tab move to another session as not implemented because the kernel has no such operation. doc-2 section 8, doc-15 (R94, R114), decision-5.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The kernel gains an operation that moves a tab from one session to another at a position, keeping every rule the tree holds for both sessions: each still has a current tab (R94, R114), a session left with no tab is dealt with as the spec says, focus and the tab's panes, their programs and their viewers are unaffected, and positions in both sessions stay dense
- [ ] #2 tab move --to is built on it in dispatch and answers as doc-2 section 8 gives; a request that gives both a position and another session does all of it or none; it leaves the not-yet list
- [ ] #3 Every rule named above has its own kernel test, with the tree's invariants checked after each move; the method is proved through Runtime::handle
<!-- AC:END -->
