---
id: TASK-1.150
title: >-
  Find why a freshly autostarted server closes a client's first connection
  before the hello
status: In Progress
assignee:
  - '@bruk.w5'
created_date: '2026-11-22T04:13:00Z'
updated_date: '2026-11-22T04:48:00Z'
labels:
  - size-3
milestone: m-3
dependencies: []
parent_task_id: TASK-1
project: bruk-io-steg
asdlc_accepted_from: DRAFT-2
asdlc_branch: task-1.150-find-why-a-freshly-autostarted-server
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

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
From TASK-1.137's planner, 22 November 2026, after reading both failed CI logs (same panic both times, ConnectError::ClosedBeforeHello). Two candidates, neither proven. H1 (better supported, needs no new mechanism): the test's own fixture. spawn_hostile_listener accepts one connection, drops it, then drops the listener; while the listening socket is still open the kernel queues further connections in its backlog, so the client's retry can land there between the two drops and is discarded when the listener closes - the same end-of-file as the first attempt, with autostart never run and no real server involved. That would make this a test-fixture race, not a product fault. H2: a real autostarted server closed its first connection early, which needs a second, unconfirmed race with no other symptom in the suite. Neither log records whether the autostart spawn ran in the failing case. Start by instrumenting the fixture to tell the two apart (did the spawn run), before writing any fix.
<!-- SECTION:NOTES:END -->
