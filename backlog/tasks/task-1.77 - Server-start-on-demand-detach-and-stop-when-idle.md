---
id: TASK-1.77
title: 'Server: start on demand, detach, and stop when idle'
status: To Do
assignee: []
created_date: '2026-11-15T19:23:00Z'
labels:
  - size-3
milestone: m-0
dependencies:
  - TASK-1.44
parent_task_id: TASK-1
project: bruk-io-steg
asdlc_accepted_from: DRAFT-4
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Part B of the split of TASK-1.44, on Bruk's decision of 2026-11-15. doc-8: R1, R3, R4, R39, R98, R99; doc-13 R60, R61; decision-2 section 7. Bruk decided the lifecycle on 2026-11-15: started by the first client, not by launchd, and everything about starting, detaching and stopping is the server's own code (R98).

Builds on TASK-1.44's foreground serve(): the detach wraps that same loop. The worker's plan on TASK-1.44 (its sections 2, 4, 5 and the entry point) is the starting design; read it and its plan check. Carried from that check: prefer the nix crate, already in Cargo.lock through portable-pty, to hand-written libc wrappers, and have the detached stage call setsid itself so no pre-exec closure is needed, leaving unsafe code to the one block that takes over inherited descriptors, or say why not; state what a client sees when it connects in the instant an idle server is stopping. The session count for the idle rule is zero until TASK-1.45 routes requests; say which task replaces it.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The first client that needs a server starts one through the sdk's local-socket provider; a server started explicitly stays up with no clients
- [ ] #2 A server started on demand keeps running after the client that started it exits or is killed, and its panes' environment comes from a login shell, not from that client (R99); capturing that environment cannot hang the server on a slow or interactive profile
- [ ] #3 A server started on demand stops after 30 seconds with no sessions and no clients; one with a session, or one started explicitly, does not; a client still in its handshake counts as a client (R99)
- [ ] #4 Two real clients starting a server at the same moment end up connected to the same one (R99), shown with real processes
- [ ] #5 A client's part in starting a server is to run the server's program and wait for the socket; it holds none of the logic above (R98), and nothing is added to the sdk
- [ ] #6 steg server start and steg server start --on-demand run the server, and say on standard output whether a server was started or one was already running; no test-only hook is added to the shipped binary
- [ ] #7 Every child this code spawns is waited on along every path or the notes say why not, including what the client that started the server is left holding
<!-- AC:END -->
