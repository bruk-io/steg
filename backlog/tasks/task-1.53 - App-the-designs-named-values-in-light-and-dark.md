---
id: TASK-1.53
title: 'App: the design''s named values, in light and dark'
status: To Do
assignee: []
created_date: '2026-11-15T01:33:00Z'
updated_date: '2026-11-15T02:15:00Z'
labels:
  - size-3
milestone: m-0
dependencies:
  - TASK-1.52
parent_task_id: TASK-1
type: task
project: bruk-io-steg
asdlc_accepted_from: DRAFT-23
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
doc-10: R24, R25, R79. The app's look and behaviour is the design artifact (doc-10 says which page is the source for what). Until workers can read artifacts (asdlc intent workers-read-artifacts), the orchestrator hands over what the artifact says for this task.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Every colour, type size, space, corner and size in the Foundations page exists once as a named value, in both themes
- [ ] #2 The app follows the system's light or dark setting and can be pinned to either
- [ ] #3 No colour or size is written as a literal outside that one place, checked mechanically
<!-- AC:END -->
