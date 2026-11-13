---
id: DRAFT-9
title: 'Kernel: run a program in a terminal and keep its screen, scrollback and facts'
status: Draft
assignee: []
created_date: '2026-11-13T22:52:00Z'
updated_date: '2026-11-13T23:05:00Z'
labels:
  - size-5
milestone: m-0
dependencies:
  - DRAFT-6
  - DRAFT-2
parent_task_id: DRAFT-1
type: task
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
R13: a pane runs the login shell or a given command, with a given working directory and environment. R15: scrollback per pane up to a configurable limit, 10,000 lines by default. R16: the working directory, title and exit code are known. Uses the library the spike chose (R14).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A test starts a command in a pane and reads its output back from the screen
- [ ] #2 A test shows scrollback stopping at the configured limit (R15)
- [ ] #3 A test shows the working directory, title and exit code tracked as the program changes them (R16)
<!-- AC:END -->
