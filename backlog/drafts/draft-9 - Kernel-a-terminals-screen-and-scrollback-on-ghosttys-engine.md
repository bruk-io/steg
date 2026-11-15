---
id: DRAFT-9
title: 'Kernel: a terminal''s screen and scrollback on ghostty''s engine'
status: Draft
assignee: []
created_date: '2026-11-15T01:32:00Z'
labels:
  - size-4
milestone: m-0
dependencies:
  - DRAFT-4
  - DRAFT-5
parent_task_id: TASK-1
type: task
project: bruk-io-steg
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
doc-9: R14, R15, R16, R92. decision-2 says what a snapshot and an update carry, and its section 9 lists what the spike's cell model lacked.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Bytes fed in produce a screen that can be read out as rows of attribute runs
- [ ] #2 Scrollback is kept to a configurable limit, 10,000 lines by default, and every row has an absolute identity
- [ ] #3 The terminal reports its working directory, its title and whether the alternate screen is on
- [ ] #4 The rows changed since a given point can be asked for, so an update need not carry the whole screen
- [ ] #5 The cell model carries wide characters, link identity, underline style and colour, and the cursor's shape
<!-- AC:END -->
