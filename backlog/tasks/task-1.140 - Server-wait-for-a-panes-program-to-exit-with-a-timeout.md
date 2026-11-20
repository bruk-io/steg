---
id: TASK-1.140
title: 'Server: wait for a pane''s program to exit, with a timeout'
status: In Progress
assignee:
  - '@bruk.w2'
created_date: '2026-11-18T14:09:00Z'
updated_date: '2026-11-20T17:12:00Z'
labels:
  - size-3
milestone: m-3
dependencies:
  - TASK-1.90
parent_task_id: TASK-1
project: bruk-io-steg
asdlc_accepted_from: DRAFT-4
asdlc_branch: task-1.140-server-wait-for-a-pane-s-program-to-exit
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Split from TASK-1.90 by its plan on 2026-11-18 (its third criterion). doc-2 sections 6 and 8: pane wait --for exit exits 0 when the program ended and prints the program's own exit code in its result; a wait that times out is status 1. TASK-1.90's plan holds the worked design as a starting point: pending waits kept in the runtime's state and resolved from the code paths that already hold the lock when a program exits or a pane is closed, with a timer only when a timeout is given that wakes at once on resolution. It needs a new module in crates/server/src/runtime/ that TASK-1.131's map did not anticipate, and probably a new error code for the timeout, which is a change to the API description for the engineer to agree. Built after pane send and pane capture: all three land in crates/server/src/dispatch/pane.rs.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 pane wait answers when the pane's program exits, with its exit code, or when the timeout given passes; a wait on a pane whose program has already ended answers at once; a pane closed while being waited on answers at once
- [ ] #2 A waiting request holds neither the runtime's lock nor a connection's reader; the plan says how the request is held and answered now that a reply is written by the connection's writer
- [ ] #3 A client that disconnects while waiting leaves no thread, timer or registration behind, shown by a test
- [ ] #4 pane wait leaves the not-yet list in dispatch; it is proved with a real program in a real pty and bounded waits, and what the timeout answers (its code and the CLI's exit status under doc-2 section 6) is settled with the engineer in the plan
<!-- AC:END -->
