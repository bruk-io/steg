---
id: TASK-1.90
title: 'Server: send input to a pane, capture its screen, and wait for its program'
status: To Do
assignee: []
created_date: '2026-11-16T00:34:00Z'
labels:
  - size-3
milestone: m-3
dependencies:
  - TASK-1.45
  - TASK-1.78
parent_task_id: TASK-1
project: bruk-io-steg
asdlc_accepted_from: DRAFT-2
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found on 2026-11-15: TASK-1.50 (CLI: send, capture, wait, watch) needs these three methods from the server and no task built them; dispatch/pane.rs answers all three NotImplementedYet citing TASK-1.40, which is Done. Attaching and streaming are TASK-1.47's and not this task's: these are the one-shot forms a script uses. doc-2 section 8, doc-8 R3. TASK-1.50 should depend on this once accepted.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 pane send delivers bytes, text or named keys to the pane's program through its pty, in order, without a client attached; a pane with no running program refuses with the code doc-2 gives
- [ ] #2 pane capture returns the pane's screen as text, and its scrollback when asked, from the server's own terminal with no window open (R3); the terminal is read on the pane's own thread, never under the runtime's lock for longer than a hand-off
- [ ] #3 pane wait answers when the pane's program exits with its exit code, or when the timeout given passes, without holding the runtime's lock or a connection's reader while it waits; a pane closed while being waited on answers at once; the plan says how a waiting request is held now that a reply is written by the connection's writer (TASK-1.45)
- [ ] #4 The three methods leave the not-yet list in dispatch; each is proved with a real program in a real pty and bounded waits, and no thread or child is left by any path, including a client that disconnects while waiting
<!-- AC:END -->
