---
id: DRAFT-1
title: 'Server: an on-demand server stops when idle'
status: Draft
assignee: []
created_date: '2026-11-15T23:40:00Z'
labels:
  - size-3
milestone: m-3
dependencies:
  - TASK-1.77
  - TASK-1.45
parent_task_id: TASK-1
project: bruk-io-steg
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Second piece of the split of TASK-1.77 (2026-11-15). doc-8 R99. The worker's plan on TASK-1.77, section 3, is the starting design: a pure predicate, a checker thread fed by ConnectionEvent, the arbiter-probe caveat carried from TASK-1.44's review. Depends on TASK-1.45 because requests over the socket are what create sessions on a real server; if the plan finds the idle rule can be built and proven without it, say so and the dependency can be dropped.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A server started on demand stops by itself after 30 seconds with no sessions and no clients; one with a session, or one started explicitly, never does (R99). The rule is a pure function of counts and a time passed in as data, with the boundary tested at 29, 30 and 31 seconds with literal values
- [ ] #2 A connection counts as a client from the moment it is accepted, not from a completed handshake, so a client mid-handshake cannot have the server stopped under it; the arbiter's liveness probe, which appears as a client arriving and leaving, does not by itself keep an idle server alive, and what a client that reconnects every 29 seconds does is stated and tested
- [ ] #3 The session count comes from the server's Runtime, and stopping uses the listener's one stop function and then stops the Runtime, ending its programs; shown end to end with a real server process, a short injected grace period and bounded waits
- [ ] #4 A client that connects in the instant the server is stopping sees what it would see with no server there and starts a fresh one; shown with real processes
<!-- AC:END -->
