---
id: DRAFT-5
title: 'Server: find why taking over a stale socket sometimes reports a live server'
status: Draft
assignee: []
created_date: '2026-11-22T21:24:00Z'
labels:
  - size-2
milestone: m-3
dependencies: []
parent_task_id: TASK-1
project: bruk-io-steg
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Seen once on 22 November 2026 in a full check of the TASK-1.127 branch (a change to crates/app only), passing on an immediate retry with no change. A different test from the one TASK-1.150 fixed (cli_connection_arbitration.rs), though in the same area: who owns the socket when a server starts. If a real server can be told AlreadyRunning by a socket whose owner is dead, an autostart can fail for a user, so this is worth knowing before M1a is called done. One other full check was running on the machine at the time.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The cause of the sighting is named with evidence, or shown not reproducible with the method stated: arbiter::tests::attempt_takes_over_a_stale_socket_left_by_a_dead_server in crates/server/src/arbiter.rs got AlreadyRunning where it expected to become the owner
- [ ] #2 The answer says plainly whether this is a fault in the test's fixture or in the product's stale-socket detection; a product fault is fixed here with a control that fails without the fix, or drafted as its own task if larger
- [ ] #3 No bound is lengthened and no retry added to make the test pass without the cause being named
<!-- AC:END -->
