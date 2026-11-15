---
id: TASK-1.63
title: 'App: keyboard focus, VoiceOver, Reduce Motion and Increase Contrast'
status: To Do
assignee: []
created_date: '2026-11-15T01:33:00Z'
updated_date: '2026-11-15T02:15:00Z'
labels:
  - size-3
milestone: m-0
dependencies:
  - TASK-1.54
  - TASK-1.55
  - TASK-1.59
parent_task_id: TASK-1
type: task
project: bruk-io-steg
asdlc_accepted_from: DRAFT-33
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
doc-10 R79, and the accessibility rules on the Foundations page. The app's look and behaviour is the design artifact (doc-10 says which page is the source for what). Until workers can read artifacts (asdlc intent workers-read-artifacts), the orchestrator hands over what the artifact says for this task.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Every control takes keyboard focus and shows the focus ring only when focus came from the keyboard
- [ ] #2 Everything reachable with the pointer is reachable with the keyboard
- [ ] #3 Every icon button and every status has a VoiceOver label saying what it is and its state
- [ ] #4 Increase Contrast adds the outlines the design gives
<!-- AC:END -->
