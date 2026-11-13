---
id: DRAFT-11
title: >-
  Server: the API's methods for sessions, tabs and panes, and its
  self-description
status: Draft
assignee: []
created_date: '2026-11-13T22:52:00Z'
updated_date: '2026-11-13T23:05:00Z'
labels:
  - size-3
milestone: m-0
dependencies:
  - DRAFT-10
  - DRAFT-8
  - DRAFT-4
parent_task_id: DRAFT-1
type: task
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
R36: one documented API for every client. R37: a client can ask for every method, its parameters and its documentation. Exposes the kernel's layout and target operations (R6, R8 to R10).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Every layout operation in R6 and R9 is callable through the API and covered by a test through the socket
- [ ] #2 One call returns every method with its parameters and documentation (R37)
- [ ] #3 Targets in API calls resolve by the rules in R10
<!-- AC:END -->
