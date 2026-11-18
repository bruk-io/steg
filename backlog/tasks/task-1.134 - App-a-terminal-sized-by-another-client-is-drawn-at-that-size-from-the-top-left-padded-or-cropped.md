---
id: TASK-1.134
title: >-
  App: a terminal sized by another client is drawn at that size from the top
  left, padded or cropped
status: To Do
assignee: []
created_date: '2026-11-18T02:09:00Z'
labels:
  - size-2
milestone: m-4
dependencies:
  - TASK-1.121
parent_task_id: TASK-1
project: bruk-io-steg
asdlc_accepted_from: DRAFT-5
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Split from TASK-1.56 (its criterion 5) on 2026-11-17, agreed with the engineer. doc-14's Size section (R91): a client draws at the terminal's size, top left, padded or cropped. The server side is built (TASK-1.114) and the bare window declares its own size (TASK-1.121); what is missing is the window drawing a size that is not its own. It needs no panes or layout, so it need not wait for TASK-1.55.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 When the terminal's size was set by another client, the window draws it at that size from the top left: padded where the window is larger, cropped where it is smaller, never rescaled or reflowed
- [ ] #2 The window's own declared size is unchanged by this, and the latest-client rule in doc-14's Size section decides whose size the terminal has
- [ ] #3 A pure function decides the drawn region from the terminal's size and the window's, with tests for larger, smaller and equal in each direction and a recorded failing control
- [ ] #4 The pull request carries an image of a padded and of a cropped terminal, from two clients on one pane
<!-- AC:END -->
