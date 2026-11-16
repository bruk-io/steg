---
id: TASK-1.60
title: 'App: the status ring and bar'
status: To Do
assignee: []
created_date: '2026-11-15T01:33:00Z'
updated_date: '2026-11-16T02:59:00Z'
labels:
  - size-3
milestone: m-0
dependencies:
  - TASK-1.54
  - TASK-1.55
  - TASK-1.41
  - TASK-1.102
parent_task_id: TASK-1
type: task
project: bruk-io-steg
asdlc_accepted_from: DRAFT-30
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
doc-11 R50; doc-15 R90; doc-10 R79. decision-7. In the first release the states are running with unknown progress, finished and failed. The app's look and behaviour is the design artifact (doc-10 says which page is the source for what). Until workers can read artifacts (asdlc intent workers-read-artifacts), the orchestrator hands over what the artifact says for this task.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A pane out of view shows its status as a ring on its tab; a pane in view shows it as a bar in its header; the selected tab and a session show none
- [ ] #2 A stopped bar parts round its mark, and a failed bar keeps a broken end
- [ ] #3 A click in a pane tells the server the person has dealt with it
- [ ] #4 With Reduce Motion on, every state shows its still form
<!-- AC:END -->

## Comments

<!-- COMMENTS:BEGIN -->
created: 2026-11-16T02:59:00Z

---

2026-11-15, @bruk.o: the components this task shows now have tasks of their own in steg-ui (doc-17, accepted today), built into the gallery without the server. This task becomes giving that component what the server reports and sending what the user does back as requests; it no longer draws the component itself. The status ring and bar atoms are TASK-1.102, which also answers doc-10's question about drawing the ring within budget.

---
<!-- COMMENTS:END -->
