---
id: TASK-1.137
title: 'Server and CLI tests: every remaining wait with no bound gets one'
status: To Do
assignee: []
created_date: '2026-11-18T14:03:00Z'
labels:
  - size-3
milestone: m-3
dependencies:
  - TASK-1.123
parent_task_id: TASK-1
project: bruk-io-steg
asdlc_accepted_from: DRAFT-1
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Split from TASK-1.123 by its plan on 2026-11-18. TASK-1.113's sweep counted about 60 waits with no bound; TASK-1.123's plan re-counted against today's tree and found 150 to 200 call sites of join, recv, child wait and socket reads, most of which already have a timeout. The plan's proposed reading, for the engineer to confirm: unbounded waits get the helper; already-bounded waits are touched only where their failure message does not name the wait. Whether the fixture binary crates/server/tests/fixtures/accept_backoff_attempt.rs is in scope, and whether crates/sdk's tests are, are open questions carried from that plan. TASK-1.123's plan, sections 1 and 4, holds the inventory and the sleep list as a starting point.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Every wait on a thread, a channel, a child process or a socket in the tests of crates/server and crates/steg that has no bound today gets one through the shared helpers, and fails with a message naming what it waited for; the plan gives the inventory by file with counts and how it was found
- [ ] #2 A wait that already has a bound is left as it is unless its failure does not say what it waited for; the plan lists which those are and the engineer confirms how far to go before the build
- [ ] #3 No test waits by sleeping in a loop where an event can be waited on; each remaining poll and each fixed sleep is listed in the notes with the reason no event exists, including the 100 ms sleep in crates/sdk/src/local_socket.rs's test fixture
- [ ] #4 The files TASK-1.88 and TASK-1.90 are editing are done last and re-checked against main first; just check passes
<!-- AC:END -->
