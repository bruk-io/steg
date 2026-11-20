---
id: TASK-1.137
title: 'Server and CLI tests: every remaining wait with no bound gets one'
status: In Progress
assignee:
  - '@bruk.w3'
created_date: '2026-11-18T14:03:00Z'
updated_date: '2026-11-20T17:12:00Z'
labels:
  - size-3
milestone: m-3
dependencies:
  - TASK-1.123
parent_task_id: TASK-1
project: bruk-io-steg
asdlc_accepted_from: DRAFT-1
asdlc_branch: task-1.137-server-and-cli-tests-every-remaining
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

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Sighting after TASK-1.123 merged, noted by @bruk.o on 2026-11-18: TASK-1.88's worker, on a branch that already contained TASK-1.123's fixes, had two local just check runs fail under three concurrent builds - one in login_env.rs's backgrounded-grandchild test, one in attach.rs's pty-race tests - each green on a single retry; CI was green. So TASK-1.123 narrowed these clashes but did not end them under heavy load. Which attach.rs tests failed was not recorded; whoever takes this task should try to reproduce with three worktrees building at once.

More sightings on 2026-11-18, noted by @bruk.o: with three builds running, login_env's backgrounded-grandchild test failed two consecutive local just check runs for TASK-1.141's worker and two for TASK-1.139's, all on branches that already contain TASK-1.123's widened bounds; CI stayed green each time. Under that load it now fails more often than it passes locally, so start here.
<!-- SECTION:NOTES:END -->
