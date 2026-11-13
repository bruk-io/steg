---
id: DRAFT-14
title: 'CLI: commands generated from the API, with context, output and exit statuses'
status: Draft
assignee: []
created_date: '2026-11-13T22:52:00Z'
updated_date: '2026-11-13T23:05:00Z'
labels:
  - size-3
milestone: m-0
dependencies:
  - DRAFT-11
  - DRAFT-5
parent_task_id: DRAFT-1
type: task
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
R32: the CLI exposes everything the API exposes, generated from the API description. R33: inside a steg terminal it acts on that session and pane by default. R34: readable output by default, JSON on request, and exit statuses a script can tell apart. Follows the CLI design Bruk decided.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A test fails if the API has a method the CLI does not expose (R32)
- [ ] #2 A command run inside a steg terminal with no target acts on that terminal's session and pane (R33)
- [ ] #3 Each exit status in R34 is produced by a test
- [ ] #4 Every command can print JSON (R34)
<!-- AC:END -->
