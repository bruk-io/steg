---
id: DRAFT-1
title: 'Server tests: bound the remaining waits found by TASK-1.113''s sweep'
status: Draft
assignee: []
created_date: '2026-11-17T01:33:00Z'
labels:
  - size-3
milestone: m-3
dependencies:
  - TASK-1.113
parent_task_id: TASK-1
project: bruk-io-steg
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
