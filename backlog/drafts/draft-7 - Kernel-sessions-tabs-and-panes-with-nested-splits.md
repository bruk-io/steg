---
id: DRAFT-7
title: 'Kernel: sessions, tabs and panes with nested splits'
status: Draft
assignee: []
created_date: '2026-11-13T22:52:00Z'
labels:
  - size-5
dependencies:
  - DRAFT-6
type: task
parent_task_id: DRAFT-1
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The state the server owns, as pure logic with no I/O. R6: sessions are created, labelled, reordered, listed and destroyed. R8: ordered tabs holding panes in nested horizontal and vertical splits. R9: split, close, focus by direction, resize, swap, move to another tab, zoom and back. R12: a pane has a kind, though only terminals exist.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Unit tests cover each operation in R6 and R9, including closing the last pane of a tab and zooming a pane in a nested split
- [ ] #2 Labels can be set and cleared on sessions, tabs and panes
- [ ] #3 A pane records its kind, and nothing in the layout code assumes a terminal (R12)
<!-- AC:END -->
