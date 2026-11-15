---
id: DRAFT-26
title: 'App: the terminal view: drawing, typing, and the latency budget'
status: Draft
assignee: []
created_date: '2026-11-15T01:33:00Z'
labels:
  - size-5
milestone: m-0
dependencies:
  - DRAFT-25
  - DRAFT-17
parent_task_id: TASK-1
type: task
project: bruk-io-steg
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
doc-9 R19; doc-10 R23, R29; doc-13 R59 (the view is written fresh, with nothing taken from GPL code). decision-3, section 5.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A pane draws its terminal's rows and cursor from its attachment, and typing reaches the program
- [ ] #2 What is fed to the screen in one frame is capped, and printing 100 MB neither freezes the window nor delays input to another pane
- [ ] #3 The app's own instrumentation reports key to paint within 12 ms and key to frame within 20 ms, at the 95th percentile on a 120 Hz display, and the run is recorded
- [ ] #4 The terminal font, size, line height, ligatures and fallbacks can be set
- [ ] #5 A terminal whose size was set by another client is drawn at that size from the top left, padded or cropped, never rescaled
<!-- AC:END -->
