---
id: DRAFT-10
title: 'Server: local socket, start on demand, and version check'
status: Draft
assignee: []
created_date: '2026-11-13T22:52:00Z'
updated_date: '2026-11-13T23:05:00Z'
labels:
  - size-3
milestone: m-0
dependencies:
  - DRAFT-6
  - DRAFT-3
parent_task_id: DRAFT-1
type: task
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
R1: a server process owns the state. R3: it starts the first time a client needs it, and can be started explicitly to stay up with no clients. R4: macOS, the user's own account, no root. R5: a socket only the owning user can open, and no network port. R39: a mismatched client and server work together or fail naming both versions. R60: XDG paths. Includes design decision 4, the server's lifecycle on macOS, settled in this task's plan.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A client that finds no server starts one and connects (R3)
- [ ] #2 A server started explicitly stays up with no clients attached (R3)
- [ ] #3 The socket refuses another user's connection, and the server listens on no network port (R5)
- [ ] #4 A client with a different protocol version gets a message naming both versions (R39)
- [ ] #5 Config, state and the socket are under XDG paths (R60)
<!-- AC:END -->
