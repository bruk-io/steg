---
id: TASK-1.123
title: 'Server tests: bound the remaining waits found by TASK-1.113''s sweep'
status: In Progress
assignee:
  - '@bruk.w5'
created_date: '2026-11-17T01:33:00Z'
updated_date: '2026-11-18T13:48:00Z'
labels:
  - size-3
milestone: m-3
dependencies:
  - TASK-1.113
parent_task_id: TASK-1
project: bruk-io-steg
asdlc_accepted_from: DRAFT-1
asdlc_branch: task-1.123-server-tests-bound-the-remaining-waits
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Split from TASK-1.113 by its plan on 2026-11-16, under the engineer's standing decision to split by default. TASK-1.113 fixes the test that hung (outbound.rs), race.rs and openpty_race.rs, adds the shared helpers (run_bounded, wait_child_bounded) and bounds every step of just check in steg-quiet. Its sweep found about 60 more waits with no bound: join, recv, child wait and socket reads in crates/server/tests (shutdown, idle_stop, program, events, routing, handshake, runtime, real_pty), crates/steg/tests (idle_stop, server_start, login_environment) and the unit tests of a dozen server modules. None has been seen to hang. This task bounds them with TASK-1.113's helpers. Also from TASK-1.113's finding: on macOS a socket pair or an accepted socket is marked close-on-exec after it is made, so a test must not rely on dropping one end making the other end fail at once while other tests fork.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Every wait on a thread, a channel, a child process or a socket in the tests of crates/server and crates/steg has a bound, through the shared helpers, and a wait that passes its bound fails the test with a message naming what it waited for
- [ ] #2 A check in tools fails just check when a test file in those crates calls join, recv, a child wait or a blocking socket read with no bound, with a message saying which helper to use and why; it has its own tests
- [ ] #3 No test waits by sleeping in a loop where an event can be waited on; each remaining poll is listed in the notes with the reason no event exists
- [ ] #4 A line in the repository's Rust testing rule says a test must not rely on a dropped socket end failing its peer at once on macOS, with the reason
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Sightings gathered by @bruk.o on 2026-11-18, while six workers built on one machine (load average 4 to 19). None is in a file the affected tasks touched; main's CI was green throughout. (1) crates/server/tests/attach.rs, the stuck-pane and bound tests: an_unresponsive_panes_own_commands_do_not_stall_the_connection_or_another_pane (most often; fails with 'must answer within a bound', or a read timing out at the helper call_and_read_reply's own five-second socket timeout near line 149), a_second_connections_control_call_is_not_blocked_by_a_stuck_panes_lock_held_attach, pane_close_force_returns_within_its_bound_even_while_the_panes_own_worker_is_stuck, stopping_the_server_returns_within_its_bound_even_while_a_panes_worker_is_stuck, a_pane_that_closes_between_a_queued_attach_and_its_arm_leaves_nothing_to_detach. Seen by the workers of TASK-1.119, 1.130 and 1.135 across about fifteen runs of just check, a different subset each time. Run alone the first test failed six times in a row for the orchestrator (about five seconds each) while another worktree's attach test binary was running at 200 percent CPU, then passed once that had stopped: these tests flood a pane's input, and two such binaries at once starve each other even on a machine with idle cores. The in-process FLOOD_TEST_LOCK serialises them inside one binary only. (2) login_env's backgrounded-grandchild test, under load, many sightings. (3) a terminal-scrolling timing test, once on CI's first run for TASK-1.128. (4) signal::tests::send_reaches_the_whole_process_group_not_only_the_leader, once on CI's first run for TASK-1.132, green on rerun; CI runs alone, so this one is not load from other workers. For the plan: liveness bounds here prove 'answers at all', so they can be generous; the helper's default five-second read timeout is the tightest bound in the file; and a machine-wide guard for the flood tests may be worth more than widening each bound. TASK-1.132 limits concurrent cargo steps to two, which reduces but does not remove two attach binaries overlapping.

Fifth sighting class, 2026-11-18 (TASK-1.135's worker, load about 3): crates/steg/tests/server_start.rs::an_untrusted_existing_file_refuses_with_status_4_and_names_the_reason failed writing a plain file because a socket already existed at the same temporary path - a path collision between two worktrees running crates/steg's tests at once (TASK-1.119's worker was running them). Passed alone and as the whole file. The helper that makes 'unique' socket paths is duplicated across this crate's test files and is evidently not unique across processes; a per-test temporary directory would remove the class. An earlier sdk local_socket path collision is logged on TASK-1.82.

From TASK-1.119's third review (2026-11-18), two test nits left because the task locked on passing: crates/sdk/src/local_socket.rs's cooperative test fixture (Behaviour::CooperativeHeldOpen, near line 586) sleeps 100 ms so the peer does not close before the client clears its timeouts - it passed 300 of 300 runs at load 3, but it is a timing margin, not a synchronised event; have the listener thread hold the stream until the test signals it instead. And a_hello_sent_then_closed_immediately_never_reports_closed_before_hello loosened its attempt-count assertion from exactly 1 to at least 1 without needing to.
<!-- SECTION:NOTES:END -->
