---
id: TASK-1.82
title: >-
  Housekeeping carried from reviews: stale decision text, Node in CI, a justfile
  comment
status: To Do
assignee: []
created_date: '2026-11-15T20:23:00Z'
updated_date: '2026-11-18T22:10:00Z'
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

Follow-up to draft (from the independent read of TASK-1.46's merge, 2026-11-16): no test runs idle stop and an event subscriber together through a session closing by its program ending. Follow-up to draft (TASK-1.117 review nits): the terminal element's run cache key leaves out the underline colour; StartupError::Protocol holds a string where an enum would tell causes apart.

Follow-up to draft (TASK-1.49 review nit, 2026-11-16): the CLI holds a path argument as a string (crates/steg/src/cli/values.rs, params.rs) where the approved plan's table said a path type. Follow-up to draft (TASK-1.112 review nit): tests/openpty_race.rs wait_timeout_or_kill polls with a 5 ms sleep, the same pattern as tests/race.rs; TASK-1.113 bounds those waits and could replace both.

Flake sightings, 2026-11-16 (names and symptoms only): steg-sdk's local_socket test failed once in a full just check by a socket path colliding under parallel tests, passed alone (seen by TASK-1.121's worker); a login_env test from TASK-1.87 failed once under load and passed on retry (seen by TASK-1.114's worker); crates/server/tests/attach.rs an_unresponsive_panes_own_commands_do_not_stall_the_connection_or_another_pane missed its bound once under load (noted on TASK-1.114, which owns it). The outbound hang is now explained on TASK-1.113: a socket pair inherited by a forked child.

Follow-up to draft (TASK-1.47, 2026-11-16): the unit test attach_pane_on_a_busy_panes_own_worker_does_not_stall_an_unrelated_requests_lock uses a flood of about 70 MB and measures through an unbounded lock, so its control can in principle pass by the worker catching up, or hang. The reviewer's repeat of the control did fail as recorded. Its worker has a stronger version (a flood of about 1 GB, a check before and after that the attach is still unanswered, the measurement waited on with a bounded receive). TASK-1.114 brings a socket-level test of the same property with a large flood. Replace the unit test with the stronger version in a small change.

Follow-up to decide (TASK-1.117 third review, 2026-11-16): the app keeps running after its window is closed; nothing in doc-10 says whether closing the last window quits. Decide and record it with the window work in m-4.

Patches kept for follow-ups, 2026-11-16, in /Users/brukhabtu/.claude/projects/-Users-brukhabtu-src-github-com-bruk-io-steg/follow-up-patches-2026-11-16: task-1.47-arm-returns-armed.patch (the starting point for the drafted task on a pane closing between attach and arm; applies to f91a8f27), task-1.47-stronger-lock-test.patch (a stronger version of attach's lock test; applies to f91a8f27), task-1.117-startup-stop-variant.patch (holds a test that drives a genuinely stalled connect through the real run(); applies to f8967015).

From TASK-1.132's second review (2026-11-18), two nits left because the task locked on passing: (1) tools/tests/functional/test_quiet_slots.py's docstring says widening ORDER_SLACK_S only ever makes the tests more lenient, but two assertions (second_start < first_end - ORDER_SLACK_S; waiter_started_at < long_ended_at - ORDER_SLACK_S) get stricter as it widens - correct the comment before anyone widens it; (2) a malformed STEG_CHECK_SLOT_WAIT_S now fails every step, including cheap ones and under CI where the limiter is off - fail-fast and defensible, but undocumented: say so in quiet.py's docstring.

Follow-ups from TASK-1.123 and TASK-1.90 (merged 2026-11-18), noted by @bruk.o: (1) crates/server/tests/attach.rs repeats a 20 s deadline in three helpers (near lines 138, 251, 342) where one constant derived from DEFAULT_READ_TIMEOUT would do (review nit on TASK-1.123). (2) crates/server/tests/pane_send.rs (TASK-1.90) still carries its own pid+counter socket-path helper; move it onto support::ScopedTempDir with the rest.
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

created: 2026-11-16T06:50:00Z

---

2026-11-16, @bruk.o: one failure in crates/server/tests/shutdown.rs during the second review of TASK-1.87 (ten runs of the server suite at ed80ab8); neither that task nor TASK-1.78's merge touches the file. Output not kept by the reviewer.

---

created: 2026-11-16T15:45:00Z

---

2026-11-16, @bruk.o: one timeout of outbound::tests::a_broken_write_closes_the_queue_so_the_readers_next_enqueue_fails during TASK-1.86's review, while two full suites ran at once on this machine; five of five passed alone. Also noted there, to be drafted as tasks once the first window's work has slots to spare: (1) a panic in a launcher's spawn during session new, tab new or pane split skips pane::start's rollback of the half-made session (from TASK-1.76, crates/server/src/pane.rs near line 565); (2) the scoped thread spawn for a connection in listen::run panics the whole server if a thread cannot be created (from TASK-1.98's review); (3) a pane's screen freezes while its program refuses input, because the pty write blocks the pane's worker (TASK-1.114's plan); (4) the API description has no one-line summary for a noun, so steg --help lists nouns bare (TASK-1.49's review).

---

created: 2026-11-17T02:17:00Z

---

Flake sighting, 2026-11-16 (from TASK-1.114's worktree, bruk.w7): login_env::tests::capture_login_environment_kills_a_backgrounded_grandchild_that_held_the_pipe_open failed once under just check's own heavy concurrent CPU contention (many test binaries running at once), then passed cleanly on an isolated rerun. Not touched - not this task's code, and the retry was clean.

---
<!-- COMMENTS:END -->
