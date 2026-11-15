---
id: DRAFT-4
title: 'App: find in a pane, and links'
status: Draft
assignee: []
created_date: '2026-11-15T20:06:00Z'
labels:
  - size-3
milestone: m-0
dependencies:
  - TASK-1.57
parent_task_id: TASK-1
project: bruk-io-steg
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Split from TASK-1.57 (2026-11-15). doc-9 R17, R116. The app's look and behaviour is the design artifact (doc-10 says which page is the source for what). Until workers can read artifacts (asdlc intent workers-read-artifacts), the orchestrator hands over what the artifact says for this task. A change to what the app draws carries an image in its pull request, and a recording where it is motion or interaction.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Find opens in the pane's own header, marks every match, moves to the next, and closes leaving the view where it was
- [ ] #2 A link a program marks can be opened
- [ ] #3 A link is drawn from the cell's link flag, and its address is asked for from the server by cell when the person acts on it (doc-9 R116); the method that asks is added to the API description and to doc-2's list
<!-- AC:END -->
