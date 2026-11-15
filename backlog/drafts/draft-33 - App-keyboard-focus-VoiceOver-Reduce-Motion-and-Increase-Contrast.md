---
id: DRAFT-33
title: 'App: keyboard focus, VoiceOver, Reduce Motion and Increase Contrast'
status: Draft
assignee: []
created_date: '2026-11-15T01:33:00Z'
labels:
  - size-3
milestone: m-0
dependencies:
  - DRAFT-24
  - DRAFT-25
  - DRAFT-29
parent_task_id: TASK-1
type: task
project: bruk-io-steg
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
