---
id: TASK-1.133
title: 'App: the terminal font, size, line height, ligatures and fallbacks can be set'
status: To Do
assignee: []
created_date: '2026-11-18T02:09:00Z'
labels:
  - size-3
milestone: m-4
dependencies:
  - TASK-1.126
parent_task_id: TASK-1
project: bruk-io-steg
asdlc_accepted_from: DRAFT-4
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Split from TASK-1.56 (its criterion 4) on 2026-11-17, agreed with the engineer: the bare window already draws in an installed monospace font on an exact cell grid (TASK-1.126), so making it settable no longer needs panes or the layout. TASK-1.56 cites doc-9 R19, doc-10 R23 and R29, and decision-3 section 5; the plan names which requirement this criterion traces to. Bundling a font stays a separate, undecided question (JetBrains Mono's licence is not on the allow list).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The terminal's font family, size, line height, ligatures and fallback families can each be set, and a change takes effect without restarting the app
- [ ] #2 A family that is not installed falls back as TASK-1.126 built it, and the cell grid stays exact at every size and line height: the cursor after an eighty-character line is at column 80
- [ ] #3 Where the settings live and how the app learns of a change is stated in the plan against the spec, and uses no polling
- [ ] #4 Tests cover each setting with a recorded failing control; the pull request carries an image at two sizes
<!-- AC:END -->
