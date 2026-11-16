---
id: DRAFT-6
title: 'ui molecules: tab, session row and session chip'
status: Draft
assignee: []
created_date: '2026-11-16T02:46:00Z'
labels:
  - size-3
milestone: m-4
dependencies: []
parent_task_id: TASK-1
project: bruk-io-steg
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Design page: Molecules (Tab, Session row, Session chip). doc-17 (the four levels, R117 to R125), decision-10 (the design is the artifact; a worker cannot open it, so the orchestrator supplies the saved page named below), decision-11 (GPUI directly). Read GPUI's README first, then its examples (https://github.com/zed-industries/zed/tree/main/crates/gpui/examples), checked against the pinned gpui 0.2.2 source in the cargo registry. Pure interface: no steg domain type, no I/O (R118). Depends on the icon, the button atoms and the status ring.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Tab (in the top bar and, indented, in the side bar), Session row and Session chip exist as molecules built from atoms, with the states their design cards give: at rest, under the pointer, pressed, selected or open, with and without a status, and the widths the foundations give (a tab's usual, least and most)
- [ ] #2 Every state and variant the design's card draws is shown in the gallery, in light and in dark, each labelled as the card labels it; the pull request carries the gallery images (and a recording where the component moves or reacts), and the notes list the card's states against what the gallery shows, one by one
- [ ] #3 It is given plain values and reports what the user did (R118); every colour, size, corner and duration comes from foundations through the theme (R120, R123); the level and colour checks pass with no exemption
- [ ] #4 It is reachable and usable from the keyboard, shows the design's focus ring when focused from the keyboard and not from the pointer, and gives assistive technology its role, name and state, as far as the pinned GPUI allows: what it does not allow is established by experiment and written in the notes, not assumed (R125)
- [ ] #5 Each reports being chosen and, where the card has one, its close or its rename being asked for; renaming in place (the States page) is not this task's and the notes say what it will need
<!-- AC:END -->
