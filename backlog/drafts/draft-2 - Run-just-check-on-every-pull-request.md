---
id: DRAFT-2
title: Run just check on every pull request
status: Draft
assignee: []
created_date: '2026-11-15T01:32:00Z'
labels:
  - size-2
milestone: m-0
dependencies: []
parent_task_id: TASK-1
type: task
project: bruk-io-steg
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The repository's one check command has to run where nobody can forget it. doc-1, section 7.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A pull request and a push to main each run `just check`, and a failing step fails the run
- [ ] #2 The toolchains are pinned in the workflow, so a run today and a run next month use the same ones
<!-- AC:END -->
