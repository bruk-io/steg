---
id: DRAFT-1
title: 'ui: the icon set''s shapes and names, generated from the design''s page'
status: Draft
assignee: []
created_date: '2026-11-16T13:08:00Z'
labels:
  - size-2
milestone: m-4
dependencies: []
parent_task_id: TASK-1
project: bruk-io-steg
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Split from TASK-1.99 by its plan on 2026-11-16, under the engineer's standing decision to split by default: the shapes and names reach the code first, as the foundations' generated values did before any component used them (TASK-1.92, TASK-1.93). TASK-1.99's plan holds the worked design: the icons are the 19 elements on the saved page marked as images with a name; GPUI 0.2.2 draws an svg from bytes supplied by an asset source and keeps only its alpha, so the page's own markup can be used as it is, including the icons whose second part is drawn at 35 percent. doc-17 R120, R121; decision-10 (the design is the artifact); decision-11. The saved pages are at /Users/brukhabtu/.claude/projects/-Users-brukhabtu-src-github-com-bruk-io-steg/design-artifact-pages-icons-2026-11-16/project/ (version 1794796789-1b13). Whether the icon's three sizes become a named value of the design is the engineer's decision, recorded on TASK-1.99.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The design's Icons and Atoms pages are in design/canvas/project/, byte for byte as saved from the artifact, with the artifact's version recorded as the README records the others
- [ ] #2 A tool in tools/ extracts the 19 icons from the saved Icons page into generated code in the component crate: a type whose values are the 19 names, taken from the page, and each icon's shape exactly as the page draws it; any transformation of the page's markup is stated as a rule of the tool and tested; no icon is redrawn or retyped by hand
- [ ] #3 just check fails when the generated code does not match the saved page, with a message naming the command that regenerates it; a hand edit to the generated code fails the same way; the generator is deterministic
- [ ] #4 A reviewer can check one icon against the design without rendering the canvas: the generated code names each icon as the page does, and a test compares the extracted shape for each name with the page's own
- [ ] #5 No drawable component and no change to the gallery in this task; the level, colour, foundations and design checks pass with no exemption
<!-- AC:END -->
