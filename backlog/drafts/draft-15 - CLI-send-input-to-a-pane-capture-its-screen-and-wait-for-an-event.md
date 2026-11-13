---
id: DRAFT-15
title: 'CLI: send input to a pane, capture its screen, and wait for an event'
status: Draft
assignee: []
created_date: '2026-11-13T22:52:00Z'
updated_date: '2026-11-13T23:05:00Z'
labels:
  - size-3
milestone: m-0
dependencies:
  - DRAFT-14
  - DRAFT-12
  - DRAFT-13
parent_task_id: DRAFT-1
type: task
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
R35. This is what lets a script, or later an agent, drive a terminal it is not attached to.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A test sends text and keys to a pane and sees the program receive them
- [ ] #2 A test captures a pane's screen and its scrollback as text
- [ ] #3 A test waits for a program to exit and gets its exit code
<!-- AC:END -->
