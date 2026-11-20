---
id: DRAFT-1
title: 'Rule: never record a check through a pipe that swallows its exit code'
status: Draft
assignee: []
created_date: '2026-11-20T04:47:00Z'
labels:
  - rule-change
dependencies: []
references:
  - TASK-1.142
  - TASK-1.130
type: task
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found twice in review (TASK-1.130, TASK-1.142): a check recorded via a shell pipe into tail (e.g. sh -c '... | tail -N') reports tail's exit code, not the real command's, so record-check can only ever show a pass. Add to the team review policy: record-check must run the command directly, or with set -o pipefail, never through an unguarded pipe into tail.
<!-- SECTION:DESCRIPTION:END -->
