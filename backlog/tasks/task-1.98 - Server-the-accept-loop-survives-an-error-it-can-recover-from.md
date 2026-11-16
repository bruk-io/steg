---
id: TASK-1.98
title: 'Server: the accept loop survives an error it can recover from'
status: To Do
assignee: []
created_date: '2026-11-16T02:00:00Z'
labels:
  - size-2
milestone: m-3
dependencies:
  - TASK-1.45
  - TASK-1.77
parent_task_id: TASK-1
project: bruk-io-steg
asdlc_accepted_from: DRAFT-2
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Raised by TASK-1.77's second review on 2026-11-15 as a decision for the engineer: listen.rs (from TASK-1.44) ends the whole server on any error from accept, and TASK-1.77's test now pins that for descriptor exhaustion (exit status 1). Neither decision-2 nor doc-8 says which is right. For a server that owns every session and running program, ending because one more client could not be accepted loses everything the user has open; the orchestrator's recommendation is to carry on. doc-8 R1, R99; doc-2 section 6. If the engineer prefers the server to end, archive this draft.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 An error from accept that does not mean the listener is broken (the process or the system is out of descriptors, a connection aborted before it was accepted, an interrupted call) does not end the server: the connection is not served, the loop waits briefly without spinning and carries on, and every session and program is untouched. The plan lists each error kind and which side it falls on, with its reason
- [ ] #2 An error that does mean the listener is unusable still ends the server with status 1, releasing the lock and removing the socket file as now
- [ ] #3 Shown with a real server through the real binary: with the descriptor limit lowered and connections held open until accept fails, the server is still running, existing connections still get replies, and once descriptors are freed a new client connects; the test TASK-1.77 added for this case (running_out_of_file_descriptors_exits_1) is replaced by it, and its change of expectation is named in the notes. Waits are bounded; no process is left
- [ ] #4 The loop cannot spin at full speed while the condition lasts: shown by a bound on how many accept attempts happen in a fixed interval, with its negative control
<!-- AC:END -->
