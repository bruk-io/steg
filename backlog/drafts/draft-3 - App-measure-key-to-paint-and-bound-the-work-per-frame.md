---
id: DRAFT-3
title: 'App: measure key to paint, and bound the work per frame'
status: Draft
assignee: []
created_date: '2026-11-16T15:26:00Z'
labels:
  - size-2
milestone: m-4
dependencies:
  - TASK-1.117
parent_task_id: TASK-1
project: bruk-io-steg
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Split from TASK-1.117 by its plan on 2026-11-16. doc-9 R19 and its open question about the server's tick; doc-10's open question about drawing. Answers early whether GPUI can draw a terminal within the latency budget. The server's attach tick is 8 ms in its configuration and is not settable from outside today: this task measures at that value and says so; whether to make it settable for the measurement is for the plan. Depends on the task that adds typing (the draft beside this one): set that dependency when both have ids. Carries TASK-1.117's original fourth and fifth criteria.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The app's own instrumentation records key to paint and key to frame for each keystroke and prints the 50th and 95th percentiles for a run; the numbers from a real run on this machine are in the notes beside doc-9 R19's budget, as a measurement and not yet as a pass or fail, with what the server's tick contributes stated
- [ ] #2 Printing a large amount of output does not freeze the window: what is drawn per frame is bounded, shown by a test of the bound with its control and by a recorded run, and the notes say what is left for TASK-1.56
- [ ] #3 How a keystroke is matched to its echo is stated with its limits; nothing in just check or CI opens a window
<!-- AC:END -->
