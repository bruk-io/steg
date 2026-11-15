---
id: TASK-1.84
title: >-
  Server: pass a typed command's start and end from a pane's terminal to the
  kernel
status: To Do
assignee: []
created_date: '2026-11-15T21:43:00Z'
updated_date: '2026-11-15T23:37:00Z'
labels:
  - size-2
milestone: m-4
dependencies:
  - TASK-1.79
  - TASK-1.76
  - TASK-1.83
parent_task_id: TASK-1
project: bruk-io-steg
asdlc_accepted_from: DRAFT-3
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Piece three of the split of TASK-1.79 (2026-11-15). Depends on the kernel piece being accepted and built as well; add that dependency when the drafts have ids. doc-11 R50, R82; doc-16 (the server is where I/O meets the kernel).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 When a pane's terminal reports a prompt-marked command starting or ending, the server passes it to the kernel with a clock reading as a Moment, through the same route a program's exit already takes (TASK-1.76's Runtime::apply_event), and pane show and the tab's shown status reflect it
- [ ] #2 Shown end to end with a real shell in a real pty that emits the marks: a command that succeeds, one that fails, and one still running; and a shell that emits none, where nothing changes
- [ ] #3 No child is left unreaped and no thread running by anything this adds, and the tests stop the runtime they start
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Carried from TASK-1.83's planning (2026-11-15): pane.show's wire field 'status' is built today from the program's status (running or ended), in crates/server/src/dispatch/pane.rs, not from the display status the kernel now keeps (running, finished, failed, with its ending rules). This task is where the server first has a clock reading to ask the kernel for the display status, so its plan says what pane show and the tab's shown status report over the API, against doc-2 and doc-11, and whether the field's meaning changes or a second field is needed; name any description change for the engineer.
<!-- SECTION:NOTES:END -->
