---
id: DRAFT-2
title: 'Server and app: an attached client is told on its stream when its pane ends'
status: Draft
assignee: []
created_date: '2026-11-17T03:51:00Z'
labels:
  - size-3
milestone: m-4
dependencies:
  - TASK-1.125
  - TASK-1.114
  - TASK-1.117
parent_task_id: TASK-1
project: bruk-io-steg
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found by TASK-1.125's plan on 2026-11-16, and seen by the engineer the same day: typing exit in the bare window ends the shell and the window is never told. A stream's control frames carry screen updates and nothing else, so the wire has no way to say a stream has ended. When a pane's program ends while a client is attached, the pane's worker reports it as a tree event only; nothing is sent on the attachment's stream, and the client keeps a stream that will never carry another update. TASK-1.125 makes the server's own records right in the narrow case where a pane closes between attach and arm, but the client still hears nothing there either. This task gives a screen update a final form that says the pane has ended, with how it ended where the server knows it (the program exited with a status, or the pane was closed), sent once to every attachment and followed by the stream being closed on both sides. The bare window shows that its program has ended and stops taking keys; the drawn states of a program that has ended, and what the window offers next, stay with TASK-1.61. decision-2 section 3 and doc-14 need the new final update written in; doc-9; doc-10.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 When a pane's program ends or the pane is closed, every client attached to it receives one final update on its stream saying so, with the exit status when there is one, after any screen update still owed and before the stream is closed; no frame arrives on that stream afterwards
- [ ] #2 The same final update is sent when a pane has closed between attach and arm, in place of TASK-1.125's silent close
- [ ] #3 The sdk's typed update carries the final form, the API description and doc-14 describe it, and the drift tests cover it; a client built before this change that meets the final form does not crash
- [ ] #4 The bare window, on the final update, keeps the last screen, shows in words that the program has ended and with what status, and sends no further input; running the app again makes a new tab as it does today
- [ ] #5 Tests at the socket level cover a program that exits by itself, a forced close, and two clients attached to one pane, each with a recorded failing control and passing run; every wait has a bound
<!-- AC:END -->
