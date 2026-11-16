---
id: TASK-1.82
title: >-
  Housekeeping carried from reviews: stale decision text, Node in CI, a justfile
  comment
status: To Do
assignee: []
created_date: '2026-11-15T20:23:00Z'
updated_date: '2026-11-16T06:30:00Z'
labels:
  - size-1
milestone: m-0
dependencies: []
parent_task_id: TASK-1
project: bruk-io-steg
asdlc_accepted_from: DRAFT-6
asdlc_branch: task-1.82-housekeeping-carried-from-reviews-stale
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Small things reviews and workers noted that belong to no open task. Decision records may be hand-edited, each only through a pull request that contains nothing else (Bruk's rule). Found by: TASK-1.43 and 1.75 (decision-1), TASK-1.70 (decision-2), TASK-1.39 (decision-3), TASK-1.33 and 1.72 (Node), TASK-1.34's review (justfile).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 decision-1 no longer cites the deleted spikes/api-description (its sections 3 and 9); decision-2's text on bytes, tokio and tokio-util is marked superseded by doc-16 R102 and R106; decision-3 says ghostty-vt 0.8 has a line-based scrollback limit and names the version in use. Each decision record is changed by its own pull request containing nothing else
- [ ] #2 CI pins the Node version the LikeC4 check runs on
- [ ] #3 The justfile comment above zig-deps-seeded says what the recipe does now
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Added by @bruk.o 2026-11-15: the LikeC4 MCP smoke test in tools/ has timed out in CI on two unrelated pull requests (TASK-1.40's and TASK-1.76's), green on re-run each time. Find why it is slow or flaky on the runner and bound or fix it; a check that fails at random teaches people to re-run failures.
<!-- SECTION:NOTES:END -->

## Comments

<!-- COMMENTS:BEGIN -->
created: 2026-11-16T01:00:00Z

---

2026-11-15: third occurrence of the LikeC4 MCP smoke test timing out in CI (30s, tools/tests/adapter/test_likec4_mcp_smoke.py), on pull request 44 (TASK-1.77), whose diff touches nothing under tools/ or architecture/; green on a re-run of the same head. Earlier: TASK-1.40's and TASK-1.76's pull requests. Each occurrence costs a re-run and a check that the failure was not the change's own.

---

created: 2026-11-16T01:34:00Z

---

2026-11-15: fourth occurrence of the LikeC4 MCP smoke test timing out in CI, again on pull request 44 (TASK-1.77), on a different head (b5e2aa6). Four CI runs were queued at once. A draft pulls this one item out to be done now.

---

created: 2026-11-16T01:36:00Z

---

2026-11-15: a second test seen failing once without cause, reported by TASK-1.78's worker: crates/server arbiter::tests::attempt_takes_over_a_stale_socket_left_by_a_dead_server failed once under a full just check while six agents were building on the machine, in a file that task did not touch; clean on 8 immediate reruns. One occurrence, local, under heavy load: recorded to watch, not yet a pattern. If it is seen again, especially in CI, it gets a task of its own (the test's own timing bound under load is the first thing to look at).

---

created: 2026-11-16T03:17:00Z

---

2026-11-15: a third test seen failing once without cause, reported by TASK-1.93's worker: a steg-sdk local_socket test failed once with AddrInUse during a full just check while several agents were building on the machine; it passed alone and on every other run, and that task touched nothing in the sdk. With the arbiter's stale-socket test (noted above) this is two one-off failures in socket tests under heavy local load: both likely share a cause (a socket path or a bind racing another test or another worktree's run). Not seen in CI. If either recurs it gets a task: the first thing to check is whether the tests' socket paths are unique per process and per test.

---

created: 2026-11-16T05:42:00Z

---

2026-11-16, @bruk.o: a hang, not a failure: check run 43802834909 attempt 1 stopped after 'ok test (steg-server)' and was cancelled at 30 minutes; the next step is the race-fixture tests. Attempt 2 passed in 10.6s. Drafted as its own task.

---

created: 2026-11-16T06:30:00Z

---

2026-11-16, @bruk.o: the arbiter's socket test failed once more under full-suite parallelism in TASK-1.87's worktree (passed alone and in five further full runs). Third sighting of that test.

---
<!-- COMMENTS:END -->
