---
id: DRAFT-31
title: 'App: the empty window, and a program that has ended'
status: Draft
assignee: []
created_date: '2026-11-15T01:33:00Z'
labels:
  - size-2
milestone: m-0
dependencies:
  - DRAFT-25
parent_task_id: TASK-1
type: task
project: bruk-io-steg
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
doc-15 R94 and R97. The app's look and behaviour is the design artifact (doc-10 says which page is the source for what). Until workers can read artifacts (asdlc intent workers-read-artifacts), the orchestrator hands over what the artifact says for this task.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 With no sessions, and when the open session ends, the window says so and offers one thing to do; it never opens another session by itself
- [ ] #2 A pane whose command has ended keeps its output, says it ended and with what code, runs it again on Return, and closes on the close key
<!-- AC:END -->
