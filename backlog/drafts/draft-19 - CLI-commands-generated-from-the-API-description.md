---
id: DRAFT-19
title: 'CLI: commands generated from the API description'
status: Draft
assignee: []
created_date: '2026-11-15T01:33:00Z'
labels:
  - size-4
milestone: m-0
dependencies:
  - DRAFT-15
parent_task_id: TASK-1
type: task
project: bruk-io-steg
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
doc-2, sections 2 to 6 and 8: R32, R34. decision-4. Nothing in the CLI names a method by hand.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Every API method has a command and every command an API method, checked by a test that fails when they drift
- [ ] #2 The grammar, the flags every command has, `--json`, and `-q` printing identifiers only each behave as doc-2 says
- [ ] #3 Each exit status in doc-2 section 6 is produced by a test
- [ ] #4 `steg version`, `steg completion <shell>` and `--help` work with no server running; `--no-autostart` exits 4 without starting one
- [ ] #5 A list is tab-separated with full identifiers, no header and no alignment, the same piped or not
<!-- AC:END -->
