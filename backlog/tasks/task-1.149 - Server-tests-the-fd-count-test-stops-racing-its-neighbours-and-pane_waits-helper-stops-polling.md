---
id: TASK-1.149
title: >-
  Server tests: the fd-count test stops racing its neighbours, and pane_wait's
  helper stops polling
status: To Do
assignee: []
created_date: '2026-11-22T04:12:00Z'
labels:
  - size-2
milestone: m-3
dependencies: []
parent_task_id: TASK-1
project: bruk-io-steg
asdlc_accepted_from: DRAFT-1
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Split out of TASK-1.137 by its plan (22 November 2026). Two sighted problems that are not missing bounds. (1) shutdown.rs: every wait in the fd-count test is already bounded; it fails because it counts the process's /dev/fd entries while the file's six other tests run concurrently. TASK-1.137's plan proposes the FLOOD_TEST_LOCK pattern from attach.rs, taken by all seven tests; the alternative is to change what the test measures. (2) pane_wait.rs: the helper polls with a 20 ms sleep (review nit on TASK-1.140); the file's own pane.wait for exit replaces it with no extra protocol setup, and cannot hang on a pane that has already ended. Touches only those two test files.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 crates/server/tests/shutdown.rs many_short_connections_do_not_grow_the_tracked_set no longer counts file descriptors while the file's other tests run: a recorded control shows it failing when the serialisation is removed and a neighbour holds descriptors open
- [ ] #2 crates/server/tests/pane_wait.rs wait_for_program_status waits on the pane's exit through pane wait, with no sleep-and-poll loop left in the file
- [ ] #3 just check passes
<!-- AC:END -->
