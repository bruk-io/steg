---
id: TASK-1.82
title: >-
  Housekeeping carried from reviews: stale decision text, Node in CI, a justfile
  comment
status: To Do
assignee: []
created_date: '2026-11-15T20:23:00Z'
updated_date: '2026-11-15T22:41:00Z'
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
