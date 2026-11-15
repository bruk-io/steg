---
id: DRAFT-29
title: 'App: the command palette'
status: Draft
assignee: []
created_date: '2026-11-15T01:33:00Z'
labels:
  - size-4
milestone: m-0
dependencies:
  - DRAFT-28
  - DRAFT-23
  - DRAFT-18
parent_task_id: TASK-1
type: task
project: bruk-io-steg
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
doc-10: R21, R80. decision-9. The app's look and behaviour is the design artifact (doc-10 says which page is the source for what). Until workers can read artifacts (asdlc intent workers-read-artifacts), the orchestrator hands over what the artifact says for this task.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The first character chooses the mode: nothing goes to a session, tab or pane; `>` lists actions; `?` lists the modes
- [ ] #2 Go to lists panes that need a person first; an action that needs an answer keeps the palette open as a follow-up step
- [ ] #3 Each setting is an action that shows its current value and changes it through a step
- [ ] #4 Closing something with a program running asks first, and ending a session always asks, with the safe answer selected
- [ ] #5 The palette is the only thing that opens over the window
<!-- AC:END -->
