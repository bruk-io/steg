---
id: DRAFT-2
title: 'Tools: a check that a test''s wait has a bound'
status: Draft
assignee: []
created_date: '2026-11-18T14:03:00Z'
labels:
  - size-2
milestone: m-3
dependencies: []
parent_task_id: TASK-1
project: bruk-io-steg
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Split from TASK-1.123 by its plan on 2026-11-18 (its former criterion 2). It depends on the task that bounds the remaining waits: the tree must be clean before the check can report zero. TASK-1.123's plan, section 3, holds the design as a starting point: ast-grep-py is already a dependency of tools (used by the ui level and colour checks), so a structural check needs no new dependency, and a text search was found to flag production code in listen.rs, pane.rs, connection.rs and runtime/close.rs.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A check in tools fails just check when a test in crates/server or crates/steg calls join, recv, a child wait or a blocking socket read with no bound, with a message saying which helper to use and why
- [ ] #2 It matches structure, not text, so production code outside test modules is not flagged and bounded forms pass; a deliberate exception is marked in the code with its reason and listed by the check
- [ ] #3 It has its own tests with a recorded failing control, runs against the whole tree with zero findings, and the notes give what it adds to the time of just check
<!-- AC:END -->
