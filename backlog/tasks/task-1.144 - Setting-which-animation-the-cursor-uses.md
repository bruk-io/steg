---
id: TASK-1.144
title: 'Setting: which animation the cursor uses'
status: To Do
assignee: []
created_date: '2026-11-20T17:33:00Z'
labels:
  - size-3
milestone: m-0
dependencies:
  - TASK-1.127
parent_task_id: TASK-1
project: bruk-io-steg
asdlc_accepted_from: DRAFT-2
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Engineer's request of 2026-11-20: the cursor's animation should be configurable. TASK-1.127 builds only option 2B but holds the animation as data so a variant is an addition. This task adds the choice. Open for the plan: which of the board's other options are worth building, where the setting lives (the setting noun in the API description, doc-2), and whether it is per window or global.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A setting chooses the cursor's animation from a fixed list; the default is 2B (scattered dissolve through medium shade), and a steady cursor is one of the choices
- [ ] #2 At least the other animations on the design's cursor motion board that the engineer picks are built as data beside 2B (candidates: 1A scattered dissolve, 1B top to bottom wipe, 1C diagonal sweep, plain blink, plain fade), each with its values on the design's Foundations page and in foundations.json
- [ ] #3 Changing the setting changes the cursor in a running window without a restart, shown by a test, and the pull request carries a recording of each animation
<!-- AC:END -->
