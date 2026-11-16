---
id: TASK-1.106
title: 'ui organisms: the bar, on the side and on top'
status: To Do
assignee: []
created_date: '2026-11-16T02:46:00Z'
updated_date: '2026-11-16T02:59:00Z'
labels:
  - size-3
milestone: m-4
dependencies:
  - TASK-1.103
parent_task_id: TASK-1
project: bruk-io-steg
asdlc_accepted_from: DRAFT-9
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Design pages: Organisms (Bar on the side, Bar on top), Foundations - layout and resizing (Track, Stack, Row). The app's own task (TASK-1.54) then gives it what the server reports. doc-17 (the four levels, R117 to R125), decision-10 (the design is the artifact; a worker cannot open it, so the orchestrator supplies the saved page named below), decision-11 (GPUI directly). Read GPUI's README first, then its examples (https://github.com/zed-industries/zed/tree/main/crates/gpui/examples), checked against the pinned gpui 0.2.2 source in the cargo registry. Pure interface: no steg domain type, no I/O (R118). Depends on the tab, session row and session chip molecules.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The bar exists as an organism in both forms the design gives: on the side (every session, the open one's tabs under it) and on top (the open session as a tinted track, every other session a chip), built from the tab, session row and session chip molecules, from a plain description of sessions and tabs
- [ ] #2 Every state and variant the design's card draws is shown in the gallery, in light and in dark, each labelled as the card labels it; the pull request carries the gallery images (and a recording where the component moves or reacts), and the notes list the card's states against what the gallery shows, one by one
- [ ] #3 It is given plain values and reports what the user did (R118); every colour, size, corner and duration comes from foundations through the theme (R120, R123); the level and colour checks pass with no exemption
- [ ] #4 It is reachable and usable from the keyboard, shows the design's focus ring when focused from the keyboard and not from the pointer, and gives assistive technology its role, name and state, as far as the pinned GPUI allows: what it does not allow is established by experiment and written in the notes, not assumed (R125)
- [ ] #5 It owns its layout and what happens between its parts: keyboard movement between tabs and sessions, a tab's width shrinking from usual to least as tabs are added, and what the design fixes for more tabs than fit; what the design does not yet draw (its own note names too many tabs, many session chips, reordering) is listed as not built and not guessed
<!-- AC:END -->
