---
id: DRAFT-8
title: 'ui molecules: palette input, palette row and empty result'
status: Draft
assignee: []
created_date: '2026-11-16T02:46:00Z'
labels:
  - size-5
milestone: m-0
dependencies: []
parent_task_id: TASK-1
project: bruk-io-steg
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Design page: Molecules (Palette input, Palette row, Empty result). The text field is the one hard part decision-11 accepted writing: GPUI's input.rs example is the starting point. If the plan finds the text field is its own reviewable change, split it. doc-17 (the four levels, R117 to R125), decision-10 (the design is the artifact; a worker cannot open it, so the orchestrator supplies the saved page named below), decision-11 (GPUI directly). Read GPUI's README first, then its examples (https://github.com/zed-industries/zed/tree/main/crates/gpui/examples), checked against the pinned gpui 0.2.2 source in the cargo registry. Pure interface: no steg domain type, no I/O (R118). Depends on the label, key cap and icon atoms.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Palette input is a single-line text field: typing, a cursor, selection by keyboard and pointer, cut, copy and paste, undo, and input methods (accented and CJK composition), with the mode its first character chooses shown as the card shows it; it reports its text as it changes and reports submit and cancel
- [ ] #2 Palette row (with its lead, main and secondary text, key cap and right-hand text; at rest, under the pointer, selected) and Empty result exist as the cards give them
- [ ] #3 Every state and variant the design's card draws is shown in the gallery, in light and in dark, each labelled as the card labels it; the pull request carries the gallery images (and a recording where the component moves or reacts), and the notes list the card's states against what the gallery shows, one by one
- [ ] #4 It is given plain values and reports what the user did (R118); every colour, size, corner and duration comes from foundations through the theme (R120, R123); the level and colour checks pass with no exemption
- [ ] #5 It is reachable and usable from the keyboard, shows the design's focus ring when focused from the keyboard and not from the pointer, and gives assistive technology its role, name and state, as far as the pinned GPUI allows: what it does not allow is established by experiment and written in the notes, not assumed (R125)
- [ ] #6 The text field's behaviour is proved by tests through GPUI's test support for every editing operation named above, each its own test; input-method composition is shown by a recording if tests cannot reach it, and the plan says so
<!-- AC:END -->
