---
id: TASK-1.56
title: 'App: the terminal view: drawing, typing, and the latency budget'
status: To Do
assignee: []
created_date: '2026-11-15T01:33:00Z'
updated_date: '2026-11-17T03:36:00Z'
labels:
  - size-5
milestone: m-4
dependencies:
  - TASK-1.55
  - TASK-1.47
  - TASK-1.114
  - TASK-1.115
  - TASK-1.117
  - TASK-1.121
  - TASK-1.122
parent_task_id: TASK-1
type: task
project: bruk-io-steg
asdlc_accepted_from: DRAFT-26
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

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Carried (2026-11-15): Terminal::resize and the pty are given a pixel cell size of zero (unknown) everywhere today (TASK-1.39, 1.40, 1.76). This task's terminal view is the first thing that knows real pixel dimensions; its plan says how they reach the server and the pty.

From the engineer's first run of the bare window, 2026-11-16, for this task's cursor work: the cursor never blinks though the server reports whether it should; a block cursor is painted over the character and hides it where it should show it inverted; nothing changes when the window loses focus. Blink is periodic by nature, so decide with the engineer how it sits with the rule against periodic wake-ups (for example a timer that runs only while the window is focused and the cursor is set to blink).
<!-- SECTION:NOTES:END -->

## Comments

<!-- COMMENTS:BEGIN -->
created: 2026-11-16T12:54:00Z

---

2026-11-16, @bruk.o: now builds on TASK-1.117, the thin slice the engineer asked for (a bare window with one terminal you can type in). That task makes the first terminal view and measures key to paint; this one keeps the budget as a pass or fail on a 120 Hz display, the 100 MB flood with a second pane, font settings, a size set by another client, and fitting the view into the pane layout. Its first criterion (rows and cursor from the attachment, typing reaches the program) is met by the slice and is re-proven here inside a pane.

---
<!-- COMMENTS:END -->
