---
id: TASK-1.82
title: >-
  Housekeeping carried from reviews: stale decision text, Node in CI, a justfile
  comment
status: To Do
assignee: []
created_date: '2026-11-15T20:23:00Z'
labels:
  - size-1
milestone: m-0
dependencies: []
parent_task_id: TASK-1
project: bruk-io-steg
asdlc_accepted_from: DRAFT-6
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
