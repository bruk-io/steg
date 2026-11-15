---
id: DRAFT-13
title: 'Server: the API description, its methods and its self-description'
status: Draft
assignee: []
created_date: '2026-11-15T01:32:00Z'
updated_date: '2026-11-15T01:44:00Z'
labels:
  - size-4
milestone: m-0
dependencies:
  - DRAFT-6
  - DRAFT-7
  - DRAFT-8
  - DRAFT-39
parent_task_id: TASK-1
type: task
project: bruk-io-steg
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
doc-8: R36, R37. decision-1. The API is one value; the server dispatches from it and serves it unchanged.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Every method the first release needs (doc-2, section 8) is defined once, in the description, with its parameters and documentation
- [ ] #2 The server validates and dispatches a request against the description, and rejects an unknown method or parameter
- [ ] #3 A client can ask for every method and for one method in full, and can call any method by name
- [ ] #4 `spikes/api-description/` is deleted
<!-- AC:END -->
