---
id: TASK-1.149
title: >-
  Server tests: the fd-count test stops racing its neighbours, and pane_wait's
  helper stops polling
status: To Do
assignee: []
created_date: '2026-11-22T04:12:00Z'
updated_date: '2026-11-22T05:55:00Z'
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

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Sighting for whoever plans this, 22 November 2026 (not in this task's scope unless the plan takes it): crates/server/tests/attach.rs a_declared_size_reaches_the_pty_and_is_reported_on_every_attached_stream failed once in a local just check on TASK-1.150's branch while two other builds ran on the machine; it passed alone and in the next full run. Reported by @bruk.w5 as contention, cause not established.

Second contention sighting in crates/server/tests/attach.rs, 22 November 2026: its unresponsive-pane test failed once in a local just check on TASK-1.146's branch while other builds ran; it passed alone and on a full re-run. Reported by @bruk.w2; cause not established. With the pane-size test above, that is two different attach.rs tests failing under load in one day.
<!-- SECTION:NOTES:END -->
