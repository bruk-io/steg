---
id: TASK-1.53
title: 'App: the design''s named values, in light and dark'
status: To Do
assignee: []
created_date: '2026-11-15T01:33:00Z'
updated_date: '2026-11-18T02:09:00Z'
labels:
  - size-3
milestone: m-4
dependencies: []
parent_task_id: TASK-1
type: task
project: bruk-io-steg
asdlc_accepted_from: DRAFT-23
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
doc-10: R24, R25, R79. The app's look and behaviour is the design artifact (doc-10 says which page is the source for what). Until workers can read artifacts (asdlc intent workers-read-artifacts), the orchestrator hands over what the artifact says for this task.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Every colour, type size, space, corner and size in the Foundations page exists once as a named value, in both themes
- [ ] #2 The app follows the system's light or dark setting and can be pinned to either
- [ ] #3 No colour or size is written as a literal outside that one place, checked mechanically
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Dependency on TASK-1.52 removed by @bruk.o on 2026-11-17, agreed with the engineer: the named values and the light/dark setting need a window (TASK-1.117, done), not a server connection model. If the plan finds it does need TASK-1.52 (for example to pin the theme through a server setting), it says so and the dependency goes back.
<!-- SECTION:NOTES:END -->

## Comments

<!-- COMMENTS:BEGIN -->
created: 2026-11-16T00:52:00Z

---

2026-11-15, @bruk.o: doc-17 and decision-11 move the design's named values into a package of their own (steg-ui), and three drafts cover what this task set out to do: the package and gallery, the values as a data file exported from the artifact, and foundations generated from it. If the engineer accepts them, this task is either closed as replaced or narrowed to the app using steg-ui's foundations; that is his to decide. Its dependency on TASK-1.52 (connect to the server) is ordering, not need: named values need nothing from the server.

---
<!-- COMMENTS:END -->
