---
id: TASK-1.113
title: >-
  Server: the race-fixture tests hung for 28 minutes on CI once; every wait in
  them gets a bound
status: In Progress
assignee:
  - '@bruk.w9'
created_date: '2026-11-16T05:42:00Z'
updated_date: '2026-11-17T01:12:00Z'
labels:
  - size-2
milestone: m-3
dependencies: []
parent_task_id: TASK-1
project: bruk-io-steg
asdlc_accepted_from: DRAFT-2
asdlc_branch: task-1.113-server-the-race-fixture-tests-hung-for
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
On 2026-11-16 pull request 55's check run 43802834909 (attempt 1, commit 51c1659, a change to crates/ui and tools only) printed 'ok test (steg-server) (12.3s)' at 04:16:46 UTC and nothing more until the job's 30-minute limit cancelled it at 04:44:21. The next step is the steg-server tests with the race-fixture feature (crates/server/tests/race.rs: several real processes racing for the arbiter's socket). Attempt 2 on the same commit passed that step in 10.6s. No earlier occurrence in the last 100 runs. A test that can wait for ever breaks the team's rule that a wait is bounded, costs 30 CI minutes when it happens, and left a worker idle for 80 minutes behind it. It may share a cause with the flakes recorded on TASK-1.82 (the arbiter's stale-socket test; the sdk's local socket reporting the address in use). Rests on doc-8 (arbitration, R1 to R3) and the team's testing rules.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The step that hung is identified from the evidence, or the notes say plainly that it could not be reproduced and what was tried
- [ ] #2 Every wait in the tests run under the race-fixture feature (on a child process, a socket, a thread or a file) has a bound, and a test that passes its bound fails with a message naming what it waited for and kills and reaps the processes it started
- [ ] #3 A test proves a bound works: a fixture process made to hang fails its test within the bound and leaves no process behind, with a recorded negative control
- [ ] #4 just check cannot spend its whole CI time limit on one hung test: the suite fails within minutes when a test hangs
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Sighting with a stack, 2026-11-16 by @bruk.o: on this machine, in TASK-1.121's worktree, 'just check' hung for 29 minutes in the step 'test (steg-server, race-fixture)'. A sample of the test binary showed one test thread still alive: outbound::tests::a_broken_write_closes_the_queue_so_the_readers_next_enqueue_fails. Its stack: the test calls outbound::run_writer on a UnixStream, which sits in Condvar::wait on the Queues lock (pthread_cond_wait), never woken. So the test calls run_writer on its own thread and relies on a write failing to end it; when the queue is empty at that moment the writer waits for work with nothing left to wake it. This is likely the same hang as the 28 minutes on CI, and it is in the unit tests built with the race-fixture feature, not in the race-fixture integration tests. Fix belongs here: the test must not be able to block forever (enqueue before running the writer, or run the writer on a thread joined with a bound), and check whether run_writer itself can miss a wake-up when the queue is closed while it waits.
<!-- SECTION:NOTES:END -->

## Comments

<!-- COMMENTS:BEGIN -->
created: 2026-11-16T13:14:00Z

---

2026-11-16, @bruk.o: a second kind of hung test, for the same lesson: TASK-1.98's new accept-backoff tests joined a reader thread with no bound and hung for 30 minutes on CI (run 43866214322) and three times on the worker's machine. Fixed in that task; when this task bounds the race tests, consider a suite-level guard so that no single test binary can run past a few minutes in just check (the fourth criterion here).

---
<!-- COMMENTS:END -->
