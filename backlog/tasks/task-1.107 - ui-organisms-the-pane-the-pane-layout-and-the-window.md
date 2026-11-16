---
id: TASK-1.107
title: 'ui organisms: the pane, the pane layout and the window'
status: To Do
assignee: []
created_date: '2026-11-16T02:46:00Z'
labels:
  - size-5
milestone: m-4
dependencies: []
parent_task_id: TASK-1
project: bruk-io-steg
asdlc_accepted_from: DRAFT-10
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Design pages: Organisms (Pane, Pane layout, Window), Foundations - layout and resizing (Split). With this the gallery shows the whole window with made-up content. The app's own task (TASK-1.55) then gives it what the server reports. doc-17 (the four levels, R117 to R125), decision-10 (the design is the artifact; a worker cannot open it, so the orchestrator supplies the saved page named below), decision-11 (GPUI directly). Read GPUI's README first, then its examples (https://github.com/zed-industries/zed/tree/main/crates/gpui/examples), checked against the pinned gpui 0.2.2 source in the cargo registry. Pure interface: no steg domain type, no I/O (R118). Depends on the bar organism and the pane caption and split handle molecules.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A Pane organism (a caption and a place for the program's screen, active and not), a Pane layout organism (the panes of one tab sharing the window through split handles, from a plain layout tree of ratios, down to the smallest pane the design gives) and the Window template (the bar, side or top, with the open tab's panes) exist, built from molecules and organisms
- [ ] #2 Every state and variant the design's card draws is shown in the gallery, in light and in dark, each labelled as the card labels it; the pull request carries the gallery images (and a recording where the component moves or reacts), and the notes list the card's states against what the gallery shows, one by one
- [ ] #3 It is given plain values and reports what the user did (R118); every colour, size, corner and duration comes from foundations through the theme (R120, R123); the level and colour checks pass with no exemption
- [ ] #4 It is reachable and usable from the keyboard, shows the design's focus ring when focused from the keyboard and not from the pointer, and gives assistive technology its role, name and state, as far as the pinned GPUI allows: what it does not allow is established by experiment and written in the notes, not assumed (R125)
- [ ] #5 The program's screen is a slot the app fills: the gallery fills it with fixed sample text in the terminal theme, and the terminal view itself (TASK-1.56) is not this task's. Dragging a handle reports the new ratios; the layout holds the design's rules for where space goes, shown at several window sizes in the gallery
- [ ] #6 Values the design marks as a proposal or doc-10 lists as not yet confirmed (the smallest pane, the caption's breakpoints) are taken from foundations where present and otherwise named in the notes as waiting on the engineer, not invented
<!-- AC:END -->
