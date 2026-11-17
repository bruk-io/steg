---
id: TASK-1.126
title: 'App: the terminal draws in an installed monospace font, on the cell grid'
status: In Progress
assignee:
  - '@bruk.w8'
created_date: '2026-11-17T03:36:00Z'
updated_date: '2026-11-17T03:52:00Z'
labels:
  - size-2
milestone: m-4
dependencies:
  - TASK-1.117
parent_task_id: TASK-1
project: bruk-io-steg
asdlc_accepted_from: DRAFT-4
asdlc_branch: task-1.126-app-the-terminal-draws-in-an-installed
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found by the engineer on 2026-11-16, running the bare window with typing: the text is drawn in a proportional font and the cursor sits far to the right of the end of the text. Cause, read from the code: the design names the terminal font as JetBrains Mono, then SF Mono, then Menlo (foundations FONT_TERMINAL). The app asks GPUI for JetBrains Mono with the others as fallbacks (crates/app/src/metrics.rs and terminal_element.rs, terminal_font). JetBrains Mono is not installed on this machine, only Menlo is, and GPUI's fallbacks stand in for a missing glyph, not a missing family, so the text falls to the system's proportional font. The cell width is then measured from ten capital M in that font, so a cursor placed at column times cell width runs far ahead of the text. TASK-1.117's own screenshot showed a two-character prompt, where the drift is too small to see. Two faults: the font chosen, and that a run of text is laid out by the font's own advances and so can leave the grid whenever a glyph is not exactly one cell wide (a fallback glyph, an emoji, a wide character). doc-9, doc-10, doc-17 for the font's named value. Not in this task: cursor blink, the inverted character under a block cursor and the hollow cursor of an unfocused window, which belong to TASK-1.56.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The terminal's font is the first family in the design's list that is installed, found at start; the notes say how that is decided at the pinned GPUI version, and what happens when none is installed
- [ ] #2 A test proves the choice with a list whose first name is not installed, with a recorded failing control and passing run
- [ ] #3 Every cell's text is drawn at its own column, so a glyph narrower or wider than a cell cannot move what follows it; the cursor sits on the cell after the last character on a line of eighty characters; a test of the placement function has a recorded control
- [ ] #4 The pull request has an image of a long line with the cursor at its end, and of a listing in columns, taken with nothing personal on screen
- [ ] #5 The notes say whether steg should ship JetBrains Mono inside the app so the design's first choice is always there, with its licence, for the engineer to decide; nothing is bundled in this task
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
From the engineer's second run, 2026-11-16, after installing JetBrains Mono: the spacing is right and the cursor sits where it should, which confirms the cause. One more thing visible in that screenshot, for the plan to settle: the text starts hard against the window's left edge with no inset, and the first column looks shaved by a few pixels (the prompt's arrow, the caret of ^C and a cursor in column 0 all look cut on their left side). Check whether glyphs or the cursor in column 0 are clipped by the element's bounds, and whether the design gives the terminal an inset inside its pane (doc-10, doc-17, the design's Layout page); if the inset belongs to the pane work (TASK-1.55), say so and make sure column 0 is at least fully drawn here.
<!-- SECTION:NOTES:END -->
