---
id: DRAFT-1
title: 'tools: the LikeC4 MCP smoke test stops failing CI by timing out'
status: Draft
assignee: []
created_date: '2026-11-16T01:34:00Z'
labels:
  - size-2
milestone: m-3
dependencies: []
parent_task_id: TASK-1
project: bruk-io-steg
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Pulled out of TASK-1.82 (housekeeping, m-0) on 2026-11-15 because it is costing a re-run on real pull requests while six tasks are in flight: tools/tests/adapter/test_likec4_mcp_smoke.py::test_a_session_can_list_this_repositorys_views_and_elements_through_the_mcp_server raises TimeoutError('likec4 mcp did not answer within 30.0s') from tools/src/steg_tools/likec4_mcp.py. Seen on the pull requests of TASK-1.40, TASK-1.76 and TASK-1.77 (twice, two heads), none of which touched tools/ or architecture/, green on re-run each time. doc-16 R108 is why the test exists (the architecture model is checked).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The cause of the timeouts is established from the failed runs' logs and a reproduction, not assumed: what the 30 seconds were spent on (downloading or starting the pinned likec4 through npx, the server indexing the model, a reply lost), on CI's runner and locally under load; the notes say which
- [ ] #2 The fix addresses that cause: for example the tool is installed or cached before the test so no download happens inside the timed wait, or the test waits on the server's own readiness and not a fixed time. Raising the timeout alone is not a fix unless the cause is shown to be plain slowness with a bound, and then the bound is stated with its evidence. The test is not skipped, marked flaky or retried silently
- [ ] #3 Shown by repeated runs: the test passes N times in a row on CI (a workflow run that repeats it, linked in the notes) and locally under load, with N chosen and justified against the failure rate seen so far (four failures on pull requests 40's, 76's task's, and 44 twice)
- [ ] #4 If Node is not pinned in CI and that is part of the cause, pinning it is done here and removed from TASK-1.82's list
<!-- AC:END -->
