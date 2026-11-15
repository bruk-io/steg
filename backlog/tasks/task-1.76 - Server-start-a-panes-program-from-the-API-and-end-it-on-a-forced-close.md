---
id: TASK-1.76
title: 'Server: start a pane''s program from the API, and end it on a forced close'
status: To Do
assignee: []
created_date: '2026-11-15T18:07:00Z'
labels:
  - size-5
milestone: m-0
dependencies:
  - TASK-1.40
  - TASK-1.75
  - TASK-1.42
parent_task_id: TASK-1
project: bruk-io-steg
asdlc_accepted_from: DRAFT-4
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Joins three finished parts that do not meet yet: the kernel's rule for a pane's program (TASK-1.40), the server's pane runtime that runs a real program on a pty (TASK-1.40), and dispatch (TASK-1.75), which today refuses a program or working directory on session.new, tab.new and pane.split and treats --force as a no-op. After this a request through the API creates a pane with a real program in it, sized by the kernel, and a forced close ends it.

Carried from reviews: TASK-1.40's second review found that shutting a pane down while a program that prints nothing is running leaves the relay thread blocked and the child alive, because the reader is a separate descriptor and closing the server's end does not hang the program up on this platform; the worker judged a fix to need unsafe or a change of ownership. portable-pty's Child::clone_killer() may give a way to end the child without unsafe; confirm from the crate's source. TASK-1.39 left the pixel cell size for Terminal::resize unassigned. TASK-1.75 left dispatch/mod.rs holding behaviour against .claude/rules/rust.md; split it here if this task grows that file.

Cites doc-2 sections 8 and 9, doc-8 (R97, R99), doc-15, doc-16 (R101-R108: the kernel does no I/O; the server is where I/O meets the kernel). If the plan finds this is more than one reviewable change, the split is: programs started from the API and sized; then forced close.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 pane split, tab new and session new given a program or a working directory start that program in the new pane through the server's pane runtime, where today dispatch refuses them; with neither, the new pane runs the login shell (doc-2 section 8, doc-8 R99 for the environment once TASK-1.44 provides it)
- [ ] #2 pane show reports the running program and, once it has ended, its exit code; a shell pane that exits closes through the same cascade as a manual close (R97)
- [ ] #3 A change of a tab's size reaches every terminal in it: the kernel's size for each pane is applied to its terminal and to its pty, and the pixel cell size Terminal::resize needs has a stated source
- [ ] #4 pane close, tab close and session close without --force refuse while a program is running, with the ProgramRunning code; with --force the program is ended, its exit is observed, and its relay thread ends, for a program that is producing output and for one that prints nothing (the case TASK-1.40 left: a silent program kept the relay blocked in read and the child alive)
- [ ] #5 No path that can end a pane leaves a child unreaped or a thread running: shown by tests with real programs and bounded waits, and no unsafe code is added without it being named in the plan and approved
- [ ] #6 The kernel errors ProgramNotStarted and ShellCannotRestart, and ClientNotViewingPane and ClientNotViewingTab if a method here can return them, map to an API error code that says what happened; whether each needs its own code is decided in the plan against doc-2
<!-- AC:END -->
