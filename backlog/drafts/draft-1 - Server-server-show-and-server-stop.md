---
id: DRAFT-1
title: 'Server: server show, and server stop'
status: Draft
assignee: []
created_date: '2026-11-16T00:01:00Z'
labels:
  - size-3
milestone: m-3
dependencies:
  - TASK-1.45
  - TASK-1.77
  - TASK-1.78
parent_task_id: TASK-1
project: bruk-io-steg
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found on 2026-11-15 while checking TASK-1.77's plan: no task owned the server noun's methods. Dispatch answers every server method with NotImplementedYet, and TASK-1.77 adds only the steg entry point that starts a server process, so nothing could tell an explicitly started server to stop except killing it. doc-2 sections 8 and 9 (server show, server stop, --force), doc-8 R3 and R99. TASK-1.77's plan considered and set aside a signal handler for an explicit server (it needs unsafe or a new dependency, and none of its criteria asked for one): this task decides it. Depends on TASK-1.78 for ending programs and on TASK-1.45 for requests reaching the server at all.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 server show answers over the API with the server's version, how long it has been up, and its sessions and clients, as doc-2 section 8 gives; time is read by the server and reaches anything pure as data
- [ ] #2 server stop stops the server: it refuses with the ProgramRunning code while any session exists unless --force is given (doc-2 sections 8 and 9), answers the caller before the connection closes, then stops accepting, ends every program by the forced-close mechanism of TASK-1.78, joins every thread, removes the socket file and releases the lock; afterwards a client finds no server there
- [ ] #3 This is how a server started explicitly is told to stop (R99: it stops only when told to). The plan says what an interrupt or a termination signal from the terminal or a supervisor does to an explicit server: whether it is handled as a stop or left to end the process, what each leaves behind (the socket file, running programs), and if handling it needs unsafe code or a new dependency, that is named for the engineer
- [ ] #4 server start through the API on a running server answers as doc-2 gives for a server already running; the three server methods leave the not-yet list in dispatch and the completeness test covers them
<!-- AC:END -->
