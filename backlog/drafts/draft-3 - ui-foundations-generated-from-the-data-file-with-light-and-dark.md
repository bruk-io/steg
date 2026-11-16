---
id: DRAFT-3
title: 'ui: foundations generated from the data file, with light and dark'
status: Draft
assignee: []
created_date: '2026-11-16T00:52:00Z'
labels:
  - size-3
milestone: m-4
dependencies: []
parent_task_id: TASK-1
project: bruk-io-steg
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
doc-17 (R120, R121, R123, R124), decision-11. Depends on the package and on the data file. gpui's own colour and pixel types are used at the edge; say in the plan how a hex colour becomes one and whether anything is lost.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A tool in tools/ writes crates/ui/src/foundations/generated.rs from design/foundations.json, deterministically, with the design's names carried through; it has its own tests, and just check fails with a message saying how to regenerate when the generated file does not match the data file (R121)
- [ ] #2 foundations offers each value through the theme in force: a colour has its light and dark form and a component asks the theme for it, never one form directly; values with no pair, such as a size, are plain (R123). The form of a value with no pair is settled in the plan (doc-17's open question)
- [ ] #3 The gallery shows the foundations: every colour as a swatch with its name in light and in dark, the ramps, the type sizes and the spacing steps, and both terminal themes' 16 colours, and the pull request carries that image (R124)
- [ ] #4 A reviewer can pick any value in the design's foundations page and find it by the same name in the gallery image and in the generated code; the notes list the names and count them against the data file
<!-- AC:END -->
