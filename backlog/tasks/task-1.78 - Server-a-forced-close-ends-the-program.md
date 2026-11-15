---
id: TASK-1.78
title: 'Server: a forced close ends the program'
status: To Do
assignee: []
created_date: '2026-11-15T19:42:00Z'
updated_date: '2026-11-15T20:05:00Z'
labels:
  - size-3
milestone: m-3
dependencies:
  - TASK-1.76
parent_task_id: TASK-1
project: bruk-io-steg
asdlc_accepted_from: DRAFT-5
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Part two of the split of TASK-1.76 (2026-11-15). doc-2 section 9 (--force on the close verbs), doc-8 R97, doc-16. TASK-1.40's reviews established that shutting a pane down does not end a running program, and that a program that prints nothing leaves its relay thread blocked in read with the child alive, because the reader is a separate descriptor and closing the server's end did not hang the program up on this platform. The worker's plan on TASK-1.76, section 5, is the starting design: remove the handles under the lock, drop the lock, hang up, escalate to a kill after a grace bound, suppress the normal exit event for a forced run so the forcing caller alone removes the pane, and reply through the pane's worker. Read it and TASK-1.40's notes before planning.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 pane close, tab close and session close with --force end every running program in scope, observe each exit, and then close; for a program that is producing output and for one that prints nothing (the case TASK-1.40 left: a silent program kept the relay blocked in read and the child alive)
- [ ] #2 A program that ignores a polite signal is ended after a bounded wait; the bound and the signals used are stated, and the mechanism is established from portable-pty's own source and a real-process experiment recorded as the first step of the build
- [ ] #3 After a forced close no child is left unreaped and no relay or worker thread is left running, on every path including a program that dies by itself while being forced; shown by tests with real programs and bounded waits
- [ ] #4 A forced close of one pane does not stall requests for other panes for the length of the wait, and a pane being forced cannot be reached by another request meanwhile
- [ ] #5 No unsafe code is added without it being named in the plan with its reason and the alternative rejected
<!-- AC:END -->
