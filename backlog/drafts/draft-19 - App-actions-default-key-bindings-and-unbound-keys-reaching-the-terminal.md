---
id: DRAFT-19
title: 'App: actions, default key bindings, and unbound keys reaching the terminal'
status: Draft
assignee: []
created_date: '2026-11-13T22:52:00Z'
updated_date: '2026-11-14T00:06:00Z'
labels:
  - size-3
milestone: m-0
dependencies:
  - DRAFT-16
parent_task_id: DRAFT-1
type: task
project: bruk-io-steg
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
R28: every action can be bound to a key, and sensible defaults ship. R29: a key that is not bound reaches the terminal unchanged. Bindings are data, so the second release's config can supply them (R40) and a key press never waits on Python (R73).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Every layout operation in R9 has an action and a default binding
- [ ] #2 A test shows an unbound key, including one with modifiers, reaching the program unchanged (R29)
- [ ] #3 Bindings are read from data, not written into the key handling
<!-- AC:END -->
