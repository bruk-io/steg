---
id: DRAFT-7
title: 'Kernel: focus as a path, set upward'
status: Draft
assignee: []
created_date: '2026-11-15T01:32:00Z'
labels:
  - size-2
milestone: m-0
dependencies:
  - DRAFT-5
parent_task_id: TASK-1
type: task
project: bruk-io-steg
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
doc-15: R85, R88, and focusing by direction from R9. decision-5. doc-2 section 4 says what happens at the edge and with `--wrap`.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Each session has a current tab and each tab a current pane
- [ ] #2 Focusing a pane makes its tab the current tab of its session in the same step
- [ ] #3 Focusing by direction moves to the neighbour, stays put at the edge, and wraps when asked
- [ ] #4 Nothing about clients changes when focus does
<!-- AC:END -->
