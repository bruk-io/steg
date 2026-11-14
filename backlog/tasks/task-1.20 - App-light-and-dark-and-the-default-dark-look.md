---
id: TASK-1.20
title: 'App: light and dark, and the default dark look'
status: To Do
assignee: []
created_date: '2026-11-13T22:52:00Z'
updated_date: '2026-11-14T03:57:00Z'
labels:
  - size-2
milestone: m-0
dependencies:
  - TASK-1.15
  - TASK-1.27
parent_task_id: TASK-1
type: task
project: bruk-io-steg
asdlc_accepted_from: DRAFT-21
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
R24: the app follows the system's light and dark setting and can be pinned to either. R25: the default dark look is a near-black base with warm gradient accents, violet through pink to orange, with no Liquid Glass and no dependence on macOS 26 materials.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Changing the system setting changes the app, and pinning holds it (R24)
- [ ] #2 Bruk has seen the default dark look and accepted it (R25)
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
From TASK-1.27's worker, relayed by @bruk.o: a design canvas renders blur, materials and shadow stacks that GPUI cannot draw, so check the look in real GPUI. A gpui-app plugin in Bruk's plugin cache has a gpui-dev-loop skill and a gpuictl shot harness that screenshots a running GPUI app; it is not enabled for workers by default, so ask @bruk.o if it is needed.
<!-- SECTION:NOTES:END -->
