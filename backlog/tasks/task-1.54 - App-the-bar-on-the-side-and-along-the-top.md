---
id: TASK-1.54
title: 'App: the bar, on the side and along the top'
status: To Do
assignee: []
created_date: '2026-11-15T01:33:00Z'
updated_date: '2026-11-16T02:59:00Z'
labels:
  - size-4
milestone: m-4
dependencies:
  - TASK-1.53
  - TASK-1.89
  - TASK-1.106
parent_task_id: TASK-1
type: task
project: bruk-io-steg
asdlc_accepted_from: DRAFT-24
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
doc-10: R20, R26, R79. The app's look and behaviour is the design artifact (doc-10 says which page is the source for what). Until workers can read artifacts (asdlc intent workers-read-artifacts), the orchestrator hands over what the artifact says for this task.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 On the side the bar lists every session with the open one's tabs under it; along the top the open session is a tinted track and the others are chips; one control moves it
- [ ] #2 A new session and a new tab are each a +; a tab ends in one fixed slot and a session row in two
- [ ] #3 A tab or a session is renamed in place
- [ ] #4 Every control shows the five states the design gives it, and the bar follows the design's rules for running out of room
<!-- AC:END -->

## Comments

<!-- COMMENTS:BEGIN -->
created: 2026-11-16T00:35:00Z

---

Now depends on TASK-1.89 (tab list and show, and the status a tab shows over the API), accepted 2026-11-15: the bar needs both.

---

created: 2026-11-16T02:59:00Z

---

2026-11-15, @bruk.o: the components this task shows now have tasks of their own in steg-ui (doc-17, accepted today), built into the gallery without the server. This task becomes giving that component what the server reports and sending what the user does back as requests; it no longer draws the component itself. The bar organism is TASK-1.106.

---
<!-- COMMENTS:END -->
