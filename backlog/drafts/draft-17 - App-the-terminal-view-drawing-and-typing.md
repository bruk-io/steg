---
id: DRAFT-17
title: 'App: the terminal view, drawing and typing'
status: Draft
assignee: []
created_date: '2026-11-13T22:52:00Z'
labels:
  - size-5
dependencies:
  - DRAFT-16
  - DRAFT-13
  - DRAFT-2
type: task
parent_task_id: DRAFT-1
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Draws a terminal pane in GPUI and sends keys to it, written fresh (R59). R14: full-screen programs render correctly. R19: the typing latency and throughput targets. R23: font, size, line height, ligatures and fallback fonts are configurable.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A shell in a pane can be used for ordinary work: typing, colours, cursor, resize
- [ ] #2 Typing latency and the 100 MB print are measured against the numbers the library spike settled (R19)
- [ ] #3 Each font setting in R23 takes effect
<!-- AC:END -->
