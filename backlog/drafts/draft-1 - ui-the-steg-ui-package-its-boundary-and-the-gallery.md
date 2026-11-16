---
id: DRAFT-1
title: 'ui: the steg-ui package, its boundary, and the gallery'
status: Draft
assignee: []
created_date: '2026-11-16T00:52:00Z'
updated_date: '2026-11-16T00:52:00Z'
labels:
  - size-3
milestone: m-4
dependencies: []
parent_task_id: TASK-1
project: bruk-io-steg
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
doc-17 (R117, R124), decision-11, doc-16's package tables as changed on 2026-11-15. The first piece of the UI work: it needs nothing from the server and overlaps none of the server tasks. GPUI's own examples are the reference (https://github.com/zed-industries/zed/tree/main/crates/gpui/examples: window.rs, hello_world.rs, testing.rs); they follow GPUI's main branch and steg pins gpui 0.2.2, so check an example against the pinned version. Carried from TASK-1.52's notes: how a UI pull request's screenshot is captured is decided here.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 crates/ui exists as package steg-ui with the folders doc-17 gives (foundations, atoms, molecules, organisms, gallery); boundaries.toml allows it gpui and nothing of steg's, and allows the app steg-ui, each with its reason; the app depends on it (R117)
- [ ] #2 The gallery is a program in the package that opens a window and shows what the package holds, in light and in dark, switchable; with nothing built yet it shows one placeholder drawn with GPUI so that the window, the theme switch and the capture are proved (R124)
- [ ] #3 A just recipe captures the gallery to an image file without a person at the screen, or the plan says why that cannot be done on this GPUI version and what a pull request shows instead; the pull request for this task carries that image (team rule: a UI pull request shows an image)
- [ ] #4 The architecture model, the affected-crates tool and just check know the new package: the LikeC4 model has it with its one allowed dependency, and a change under crates/ui selects its tests and the app's
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Read first: GPUI's README, https://github.com/zed-industries/zed/blob/main/crates/gpui/README.md (the engineer: 'a really good intro to it'), then the examples; accessibility examples are a11y.rs, focus_visible.rs and tab_stop.rs.
<!-- SECTION:NOTES:END -->
