---
id: DRAFT-4
title: 'tools: checks that keep steg-ui''s levels and named values honest'
status: Draft
assignee: []
created_date: '2026-11-16T00:52:00Z'
labels:
  - size-2
milestone: m-4
dependencies: []
parent_task_id: TASK-1
project: bruk-io-steg
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
doc-17 (R119, R120, R126: what a program can check is a check and not a rule). The checks follow the existing tools in tools/src/steg_tools (boundaries.py is the model): every refusal carries its reason.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 just check refuses an import inside crates/ui that goes from a lower level to a higher one (foundations using atoms, atoms using molecules or organisms, molecules using organisms), with a message that says which rule and why (R119)
- [ ] #2 just check refuses a colour literal in crates/ui outside foundations, with a message naming the foundations value to use instead where one matches (R120); the plan says what counts as a literal and what the check cannot see
- [ ] #3 Each check has tests in tools/ with a case that must be refused and one that must pass, and is part of just check and of CI
<!-- AC:END -->
