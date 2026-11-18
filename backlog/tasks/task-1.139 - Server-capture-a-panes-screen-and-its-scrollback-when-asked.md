---
id: TASK-1.139
title: 'Server: capture a pane''s screen, and its scrollback when asked'
status: In Progress
assignee:
  - '@bruk.w7'
created_date: '2026-11-18T14:09:00Z'
updated_date: '2026-11-18T22:19:00Z'
labels:
  - size-3
milestone: m-3
dependencies:
  - TASK-1.90
parent_task_id: TASK-1
project: bruk-io-steg
asdlc_accepted_from: DRAFT-3
asdlc_branch: task-1.139-server-capture-a-pane-s-screen-and-its
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Split from TASK-1.90 by its plan on 2026-11-18 (its second criterion). doc-2 section 8, doc-8 R3. TASK-1.90's plan holds the worked design as a starting point: a new command to the pane's worker using the hand-off shape TASK-1.47 introduced for attach (clone the sender, drop the runtime lock, wait unlocked with a bound), not a bounded wait held under the lock; scrollback through the kernel terminal's existing page and oldest_row; a test that wedges a non-reading program's worker with one large write, not a flood. Built after pane send because both land in crates/server/src/dispatch/pane.rs and the pane noun's description.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 pane capture returns the pane's screen as text, and its scrollback when asked, from the server's own terminal with no window open (R3)
- [ ] #2 The terminal is read on the pane's own thread, never under the runtime's lock for longer than a hand-off: the request is handed to the pane's worker and waited on with the lock released, with a bound, so a pane whose worker is busy delays only its own capture
- [ ] #3 What 'its scrollback when asked' returns today is stated and bounded, and what is left for TASK-1.115's paging is named
- [ ] #4 pane capture leaves the not-yet list in dispatch; it is proved with a real program in a real pty and bounded waits, including a pane whose program is not reading, and no thread or child is left by any path
<!-- AC:END -->
