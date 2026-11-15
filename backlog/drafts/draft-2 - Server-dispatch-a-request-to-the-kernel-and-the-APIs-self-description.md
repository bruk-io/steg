---
id: DRAFT-2
title: 'Server: dispatch a request to the kernel, and the API''s self-description'
status: Draft
assignee: []
created_date: '2026-11-15T14:15:00Z'
labels:
  - size-3
milestone: m-0
dependencies:
  - TASK-1.43
parent_task_id: TASK-1
type: task
project: bruk-io-steg
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
doc-8: R36, R37; doc-16 R98, R101. decision-1. Split from TASK-1.43 on 2026-11-15 by Bruk. Dispatch is a function from a request to a response over the kernel's tree, with no socket: the socket and framing are TASK-1.44, TASK-1.45 and TASK-1.70. The earlier plan on TASK-1.43 has the design to start from: which methods the kernel can serve today and which answer that they are not built yet.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The server validates a request against the description and dispatches it, returning the typed result the sdk defines; an unknown method or parameter is refused
- [ ] #2 Every described method has a dispatch arm and every arm is described, checked both ways by a test; a method whose behaviour is not built yet answers with a distinct error naming what it waits on, never an invented value
- [ ] #3 For every served method, the value returned serialises to exactly the fields the description declares
- [ ] #4 Every kernel error maps to an error a client sees, with the code and exit status the description gives
- [ ] #5 A client can ask for every method and for one method in full, `api show` with no target returns the whole description, and any method can be called by name
- [ ] #6 Creating a tab or a split without taking focus leaves the previous focus in place (doc-15 R115)
- [ ] #7 `spikes/api-description/` is deleted
<!-- AC:END -->
