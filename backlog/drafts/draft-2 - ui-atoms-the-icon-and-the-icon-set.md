---
id: DRAFT-2
title: 'ui atoms: the icon, and the icon set'
status: Draft
assignee: []
created_date: '2026-11-16T02:45:00Z'
labels:
  - size-3
milestone: m-4
dependencies: []
parent_task_id: TASK-1
project: bruk-io-steg
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Design pages: Atoms (Icon), Atoms - icons. doc-17 (the four levels, R117 to R125), decision-10 (the design is the artifact; a worker cannot open it, so the orchestrator supplies the saved page named below), decision-11 (GPUI directly). Read GPUI's README first, then its examples (https://github.com/zed-industries/zed/tree/main/crates/gpui/examples), checked against the pinned gpui 0.2.2 source in the cargo registry. Pure interface: no steg domain type, no I/O (R118). Depends on the generated foundations (TASK-1.93).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 An Icon atom draws each of the 19 icons of the design's Icons board at 16px by name, in the colour it is given, with the faint second tone where the design uses one; an unknown name cannot be asked for (the names are a type)
- [ ] #2 Every state and variant the design's card draws is shown in the gallery, in light and in dark, each labelled as the card labels it; the pull request carries the gallery images (and a recording where the component moves or reacts), and the notes list the card's states against what the gallery shows, one by one
- [ ] #3 It is given plain values and reports what the user did (R118); every colour, size, corner and duration comes from foundations through the theme (R120, R123); the level and colour checks pass with no exemption
- [ ] #4 The plan says how the icons' shapes reach the code from the design's saved page without being redrawn by hand or changed, and how a reviewer checks one against the design
<!-- AC:END -->
