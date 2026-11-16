---
id: DRAFT-5
title: 'ui atoms: the status ring and the status bar'
status: Draft
assignee: []
created_date: '2026-11-16T02:46:00Z'
labels:
  - size-5
milestone: m-4
dependencies: []
parent_task_id: TASK-1
project: bruk-io-steg
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Design page: Atoms - status ring and bar (Status ring, Status bar, Each state drawn large). The riskiest drawing in the app, done early on purpose. GPUI examples painting.rs, gradient.rs, animation.rs. The values a ring takes are plain (which state, how far along), not the sdk's Status: the app maps one to the other (TASK-1.60). doc-17 (the four levels, R117 to R125), decision-10 (the design is the artifact; a worker cannot open it, so the orchestrator supplies the saved page named below), decision-11 (GPUI directly). Read GPUI's README first, then its examples (https://github.com/zed-industries/zed/tree/main/crates/gpui/examples), checked against the pinned gpui 0.2.2 source in the cargo registry. Pure interface: no steg domain type, no I/O (R118). Depends on the generated foundations (TASK-1.93).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A Status ring atom (16px: grey track, coloured arc, a mark in the middle when needed) and a Status bar atom (120px by 4px) draw each of the six states of the design's Status board from a plain value, with the design's colours for each state, in light and in dark; neither knows about the other
- [ ] #2 Every state and variant the design's card draws is shown in the gallery, in light and in dark, each labelled as the card labels it; the pull request carries the gallery images (and a recording where the component moves or reacts), and the notes list the card's states against what the gallery shows, one by one
- [ ] #3 It is given plain values and reports what the user did (R118); every colour, size, corner and duration comes from foundations through the theme (R120, R123); the level and colour checks pass with no exemption
- [ ] #4 doc-10's open question is answered with measurements: whether GPUI draws the six-section ring and a gradient fixed to its track within the app's budget (R19), measured with many rings on screen at once as a bar with many tabs would have, and what was tried; if it cannot, the plan stops and says so before anything else is built
- [ ] #5 The 200ms change of status and the Reduce Motion form of each state are built; the rolling out of a bar from a ring and the failure's motion are not this task's (TASK-1.62) and the notes say what they will need from these atoms
<!-- AC:END -->
