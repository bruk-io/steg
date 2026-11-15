---
id: DRAFT-11
title: 'Kernel: status in the first release, and when each one ends'
status: Draft
assignee: []
created_date: '2026-11-15T01:32:00Z'
labels:
  - size-3
milestone: m-0
dependencies:
  - DRAFT-10
parent_task_id: TASK-1
type: task
project: bruk-io-steg
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
doc-11: R50, R82, R83. decision-7. The first release shows only what steg can know by itself. Time arrives as data, never from a clock inside the kernel.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A pane is running while its foreground program runs, and finished or failed from its exit code, including for a command typed at a shell prompt that reports prompt marks
- [ ] #2 Finished ends after 10 minutes or on input to the pane; failed ends only on input or when told the person has dealt with it; opening a tab ends nothing
- [ ] #3 A tab's shown status is the most pressing among its panes, and a session has none
<!-- AC:END -->
