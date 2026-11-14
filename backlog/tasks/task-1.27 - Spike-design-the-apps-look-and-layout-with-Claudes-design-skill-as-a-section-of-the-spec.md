---
id: TASK-1.27
title: >-
  Spike: design the app's look and layout with Claude's design skill, as a
  section of the spec
status: To Do
assignee: []
created_date: '2026-11-14T00:16:00Z'
labels:
  - size-3
milestone: m-0
dependencies: []
parent_task_id: TASK-1
type: spike
project: bruk-io-steg
asdlc_accepted_from: DRAFT-28
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The spec says what the app shows but not what it looks like: R20 (a window with a session switcher, tabs and panes), R21 (the command palette), R24 and R25 (light and dark, and a near-black base with warm gradient accents, violet through pink to orange, in the spirit of the Superlogical site and early Arc, with no Liquid Glass), R26 (a colour per session), R27 (terminal themes) and R50 (how a tab shows a program's status). Words cannot settle those, and the app tasks would each guess. This uses Claude's design skill to produce the visual design, and turns what Bruk accepts into a design section of the spec that the app tasks build from. It delivers a design and spec text, not code.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A design made with Claude's design skill shows the main window in dark and light: session switcher, tabs, split panes, the active pane, and a tab showing each program status
- [ ] #2 The design shows the command palette and a session carrying its own colour
- [ ] #3 The spec (doc-1) has a design section giving the colours, type, spacing and states as named values an app task can build from, with a link to the design
- [ ] #4 Bruk has seen the design and recorded what he accepted
<!-- AC:END -->
