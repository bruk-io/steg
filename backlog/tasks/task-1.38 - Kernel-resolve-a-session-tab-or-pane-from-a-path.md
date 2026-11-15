---
id: TASK-1.38
title: 'Kernel: resolve a session, tab or pane from a path'
status: To Do
assignee: []
created_date: '2026-11-15T01:32:00Z'
updated_date: '2026-11-15T02:15:00Z'
labels:
  - size-2
milestone: m-0
dependencies:
  - TASK-1.35
parent_task_id: TASK-1
type: task
project: bruk-io-steg
asdlc_accepted_from: DRAFT-8
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
doc-15 R10 and doc-2 section 4. This is the rule a script relies on to act on the right pane.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A part is tried as a full identifier, then a label without regard to case, then a position, then an identifier suffix of four or more characters, stopping at the first kind that matches
- [ ] #2 A target that matches nothing, or more than one thing, is an error that lists the candidates and does nothing
- [ ] #3 A label containing `/`, or made only of digits, is refused
<!-- AC:END -->
