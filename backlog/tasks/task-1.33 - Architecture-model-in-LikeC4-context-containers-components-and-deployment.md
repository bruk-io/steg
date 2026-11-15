---
id: TASK-1.33
title: 'Architecture model in LikeC4: context, containers, components and deployment'
status: In Progress
assignee:
  - '@bruk.w2'
created_date: '2026-11-15T01:32:00Z'
updated_date: '2026-11-15T02:22:00Z'
labels:
  - size-2
milestone: m-0
dependencies: []
parent_task_id: TASK-1
type: task
project: bruk-io-steg
asdlc_accepted_from: DRAFT-3
asdlc_branch: task-1.33-architecture-model-in-likec4-context
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
doc-1 has only a five-row table of parts, and the model of servers, clients and windows was got wrong three times before decision-5 fixed it. This is the picture: how steg is divided into running parts. It is not the domain, which is doc-15.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A LikeC4 model in the repository validates, with a system context view, a container view (app, server, CLI, a plugin), and component views of the server and the app
- [ ] #2 A deployment view shows one window holding a client to a local server and another to a remote one
- [ ] #3 Two dynamic views: attaching to a pane, and focusing a pane with the path updating upward
- [ ] #4 Every name in the model is a word doc-15 or doc-1's vocabulary uses, with the same meaning
<!-- AC:END -->
