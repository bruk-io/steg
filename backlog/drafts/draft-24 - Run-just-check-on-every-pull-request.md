---
id: DRAFT-24
title: Run just check on every pull request
status: Draft
assignee: []
created_date: '2026-11-14T00:14:00Z'
labels:
  - size-2
milestone: m-0
dependencies:
  - DRAFT-6
type: task
project: bruk-io-steg
parent_task_id: DRAFT-1
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Nothing runs the checks on a pull request today, so a review relies on what the worker recorded. A GitHub workflow runs the same just check a person runs, on macOS, so the two cannot differ. Supports the review policy's Evidence pass.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A pull request to main runs just check on macOS and shows its result as a status check
- [ ] #2 The workflow calls just check and nothing else, so CI and a worktree run the same thing
- [ ] #3 A failing step's quiet output and its full log are both readable from the run
<!-- AC:END -->
