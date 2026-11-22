---
id: DRAFT-2
title: >-
  Find why a freshly autostarted server closes a client's first connection
  before the hello
status: Draft
assignee: []
created_date: '2026-11-22T04:13:00Z'
labels:
  - size-3
milestone: m-3
dependencies: []
parent_task_id: TASK-1
project: bruk-io-steg
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found by TASK-1.137's planner (22 November 2026). The test has failed twice in CI in one day (steg PR 91's CI, and main at 4ccc8344). Both times the client reported that the server accepted the connection and closed it before a hello arrived, twice: both handshake attempts met end-of-file, not a timeout. Every wait on the path already has a bound, so this is not a test-bounds problem. It reads as a product question: either the client's retry met the test's hostile listener both times, or the real, just-started server closed its first connection early. Unknowns dominate, so start by reproducing (repeat the one test) and reading the autostart and arbitration paths in crates/sdk/src/local_socket.rs and crates/server/src/arbiter.rs and listen.rs. Never paste a failing run's output or any environment text onto the board.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The cause of ClosedBeforeHello on both handshake attempts against a real autostarted server is named, from the code and a reproduction or from evidence that rules candidates out
- [ ] #2 Either the fix is made with a test that fails without it, or a follow-up task states the fix and why it was not made here
- [ ] #3 crates/steg/tests/cli_connection_arbitration.rs a_hostile_first_connection_does_not_stop_a_real_autostart_from_succeeding passes in a recorded run of many repetitions
<!-- AC:END -->
