---
id: DRAFT-3
title: 'CLI: output formats: plain text, --json and -q'
status: Draft
assignee: []
created_date: '2026-11-16T13:35:00Z'
labels:
  - size-2
milestone: m-3
dependencies:
  - TASK-1.49
parent_task_id: TASK-1
project: bruk-io-steg
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Split from TASK-1.49 by its plan on 2026-11-16. doc-2 sections 1, 5 and 6; decision-4. TASK-1.49's plan, section 'The split', part 3, holds the worked design. Carries TASK-1.49's criteria 2 and 5. Builds on the task that connects and calls (the draft beside this one): set that dependency when both have ids.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The flags every command has, --json, and -q printing identifiers only each behave as doc-2 says, for every kind of result the API description declares, matched without a catch-all so a new kind of result fails the build until it is handled
- [ ] #2 A list is tab-separated with full identifiers, no header and no alignment, and is byte for byte the same on a terminal and piped, shown by running the real binary under a real pty and through a pipe
- [ ] #3 A nested result is printed whole with --json; its plain-text form is as the engineer decided with TASK-1.49's plan
<!-- AC:END -->
