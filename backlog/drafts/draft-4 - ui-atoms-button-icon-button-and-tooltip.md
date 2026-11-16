---
id: DRAFT-4
title: 'ui atoms: button, icon button and tooltip'
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
Design page: Atoms (Button, Icon button, Tooltip). The first components a person acts on: this task establishes what the pinned GPUI offers for focus, keyboard order and assistive technology (examples a11y.rs, focus_visible.rs, tab_stop.rs), and every later interactive component follows what it finds. doc-17 (the four levels, R117 to R125), decision-10 (the design is the artifact; a worker cannot open it, so the orchestrator supplies the saved page named below), decision-11 (GPUI directly). Read GPUI's README first, then its examples (https://github.com/zed-industries/zed/tree/main/crates/gpui/examples), checked against the pinned gpui 0.2.2 source in the cargo registry. Pure interface: no steg domain type, no I/O (R118). Depends on the icon atom.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Button, Icon button and Tooltip exist as atoms with the states their design cards give: at rest, under the pointer, pressed, keyboard focus, cannot be used; the tooltip appears when the pointer rests on its control and for keyboard focus, as the card says
- [ ] #2 Every state and variant the design's card draws is shown in the gallery, in light and in dark, each labelled as the card labels it; the pull request carries the gallery images (and a recording where the component moves or reacts), and the notes list the card's states against what the gallery shows, one by one
- [ ] #3 It is given plain values and reports what the user did (R118); every colour, size, corner and duration comes from foundations through the theme (R120, R123); the level and colour checks pass with no exemption
- [ ] #4 It is reachable and usable from the keyboard, shows the design's focus ring when focused from the keyboard and not from the pointer, and gives assistive technology its role, name and state, as far as the pinned GPUI allows: what it does not allow is established by experiment and written in the notes, not assumed (R125)
- [ ] #5 A press reports once, from the pointer and from the keyboard (space and return); a control that cannot be used reports nothing and is skipped by the Tab key; shown by tests through GPUI's test support where it reaches, and by the recording where it does not, with the plan saying which
<!-- AC:END -->
