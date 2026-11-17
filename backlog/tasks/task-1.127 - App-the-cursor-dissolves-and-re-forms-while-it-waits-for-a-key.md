---
id: TASK-1.127
title: 'App: the cursor dissolves and re-forms while it waits for a key'
status: To Do
assignee: []
created_date: '2026-11-17T03:51:00Z'
labels:
  - size-3
milestone: m-4
dependencies:
  - TASK-1.126
  - TASK-1.121
parent_task_id: TASK-1
project: bruk-io-steg
asdlc_accepted_from: DRAFT-1
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Chosen by the engineer on 2026-11-16 from the design's exploration board, Atoms - cursor motion (exploration), option 2B, and now the rule on the Cursor atom (design artifact, DesignSystem.Atoms, card 07). The cursor's cell is six pieces, two columns by three rows. While the terminal waits for a key they go out one at a time in a fixed, scattered order and come back in the reverse order; each piece is medium shade for one step on its way out and on its way back. One cycle is 1.4 seconds: whole for about 600 ms, out over 250 ms, empty for about 300 ms, back over 250 ms. The pieces and the shade are drawn by the app on the cell grid, never as characters from a font. This is periodic by nature, so it is the one stated exception to the rule against periodic wake-ups, and it is bounded: it runs only in the active pane of the focused window, only when the program has asked for a blinking cursor, and it stops after a few seconds with no key. doc-10; doc-17 for the design's named values; TASK-1.63 for Reduce Motion. Not in this task: the hollow cursor of an unfocused pane, and whether the bar and underline shapes fade the same way.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 In the active pane of the focused window, when the program has asked for a blinking cursor, the block cursor dissolves and re-forms as the Cursor atom draws it: the order of the six pieces, the medium shade step, and the 1.4 second cycle come from the design's values, not from numbers written in the app
- [ ] #2 A key press shows the cursor whole at once; after the idle time with no key it rests whole and nothing wakes the app until the next key or update; the idle time is a named value the engineer settles in the plan
- [ ] #3 Nothing wakes the app for the cursor when the window is not focused, the cursor is hidden, the program has not asked for blinking, or Reduce Motion is on; a test proves each case schedules no further frame, with recorded controls
- [ ] #4 Over a character, the character is drawn in the ground colour where the cursor's pieces cover it and in its own colour elsewhere
- [ ] #5 The step shown at a given time in the cycle is a pure function with a table test and a recorded control; the pull request carries a short recording, and stills of each step, with nothing personal on screen
<!-- AC:END -->
