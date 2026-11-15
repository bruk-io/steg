---
id: DRAFT-14
title: 'Server: the local socket, starting on demand, and the handshake'
status: Draft
assignee: []
created_date: '2026-11-15T01:32:00Z'
labels:
  - size-3
milestone: m-0
dependencies: []
parent_task_id: TASK-1
type: task
project: bruk-io-steg
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
doc-8: R1, R3, R4, R5, R39; doc-13 R60, R61. decision-2, section 7. doc-8's open question on the server's lifecycle on macOS is Bruk's to answer before the plan is approved.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The server listens on a socket at an XDG path that only its owner can open, with the mode set rather than assumed
- [ ] #2 The first client that needs a server starts one; a server started explicitly stays up with no clients
- [ ] #3 A client and server whose protocol versions differ get a message naming both versions
- [ ] #4 A connection that never completes its handshake is closed after a timeout
<!-- AC:END -->
