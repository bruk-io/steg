---
id: DRAFT-1
title: 'tools: check the design''s data file against the saved foundations pages'
status: Draft
assignee: []
created_date: '2026-11-16T01:05:00Z'
labels:
  - size-2
milestone: m-4
dependencies:
  - TASK-1.92
parent_task_id: TASK-1
project: bruk-io-steg
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Split from TASK-1.92 on 2026-11-15. doc-17 (R121, R122). The tool follows the existing ones in tools/src/steg_tools. TASK-1.92's plan has its design.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A tool in tools/ checks that every value in crates/ui/design/foundations.json appears in the saved copies of the design's foundations pages under the same name and with the same value, and that no named value drawn in those pages is missing from the data file; a refusal names the value and both forms
- [ ] #2 It has unit tests for its logic and a test against the repository's real files, with a negative control: one value changed in a scratch copy is refused; it is part of just check and of CI
- [ ] #3 The plan says what the tool cannot see (a value only described in prose, a page that was not saved) so that passing is not read as more than it is
<!-- AC:END -->
