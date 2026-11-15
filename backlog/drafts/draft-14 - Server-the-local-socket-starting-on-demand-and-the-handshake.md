---
id: DRAFT-14
title: 'Server: the local socket, starting on demand, and the handshake'
status: Draft
assignee: []
created_date: '2026-11-15T01:32:00Z'
updated_date: '2026-11-15T01:44:00Z'
labels:
  - size-3
milestone: m-0
dependencies:
  - DRAFT-39
parent_task_id: TASK-1
type: task
project: bruk-io-steg
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
doc-8: R1, R3, R4, R5, R39, R99; doc-13 R60, R61. decision-2, section 7. Bruk decided the lifecycle on 2026-11-15: started by the first client, not by launchd, and everything about starting, detaching and stopping is the server's own code (R98).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The server listens on a socket at an XDG path that only its owner can open, with the mode set rather than assumed
- [ ] #2 The first client that needs a server starts one; a server started explicitly stays up with no clients
- [ ] #3 A client and server whose protocol versions differ get a message naming both versions
- [ ] #4 A connection that never completes its handshake is closed after a timeout
- [ ] #5 A server started on demand keeps running after the client that started it exits or is killed, and its panes' environment comes from a login shell, not from that client (R99)
- [ ] #6 A server started on demand stops after 30 seconds with no sessions and no clients; one with a session, or one started explicitly, does not (R99)
- [ ] #7 Two clients starting a server at the same moment end up connected to the same one (R99)
- [ ] #8 A client's part in starting a server is to run the server's program and wait for the socket; it holds none of the logic above (R98)
<!-- AC:END -->
