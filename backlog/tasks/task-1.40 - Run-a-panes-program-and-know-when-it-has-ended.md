---
id: TASK-1.40
title: Run a pane's program, and know when it has ended
status: In Progress
assignee:
  - '@bruk.w1'
created_date: '2026-11-15T01:32:00Z'
updated_date: '2026-11-15T15:20:00Z'
labels:
  - size-3
milestone: m-0
dependencies:
  - TASK-1.39
parent_task_id: TASK-1
type: task
project: bruk-io-steg
asdlc_accepted_from: DRAFT-10
asdlc_branch: task-1.40-run-a-pane-s-program-and-know-when-it
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
doc-9 R13 and R16, doc-15 R97, doc-2 section 4 for the environment. The kernel holds the state; starting a process is the server's, behind an interface the server supplies.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A pane runs the login shell, or a given command, in a given working directory
- [ ] #2 When the program ends its exit code is known; a pane started with a command stays and can run the same command again, and a pane started as a shell closes
- [ ] #3 The program's environment has `STEG_SERVER`, `STEG_SESSION`, `STEG_TAB`, `STEG_PANE` and `STEG_BIN_DIR`, and the server's own directory is first on `PATH`
<!-- AC:END -->
