---
id: TASK-1.141
title: 'Server: client list and client show'
status: In Progress
assignee:
  - '@bruk.w5'
created_date: '2026-11-18T14:10:00Z'
updated_date: '2026-11-18T22:19:00Z'
labels:
  - size-2
milestone: m-3
dependencies: []
parent_task_id: TASK-1
project: bruk-io-steg
asdlc_accepted_from: DRAFT-5
asdlc_branch: task-1.141-server-client-list-and-client-show
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Split from TASK-1.88 by its plan on 2026-11-18 (its fifth criterion). doc-2 section 8; decision-5 for clients. TASK-1.88's plan holds the worked design as a starting point: the descriptions for both methods already exist in crates/sdk/src/description/methods/client.rs, so nothing changes there; both become plain dispatch arms in a new crates/server/src/dispatch/client.rs, with a new Tree::resolve_client in the kernel. Attach has landed (TASK-1.47), so a client does view sessions today. The plan recommended leaving last_active empty for M1, as pane and tab titles are, because tracking it needs interception in the runtime and a time formatter the workspace does not have. It can be built beside the server half: they share only additive edits to dispatch/mod.rs's lists and the record and outcome variant lists.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 client list and client show answer over the API from the kernel's client model and what the server records for each connection: what each client is and the sessions it views; command line clients only with --all, as doc-2 section 8 gives
- [ ] #2 A client is found by its target the way a session is, through a resolver in the kernel that mirrors the session one; an unknown or ambiguous target answers with the codes doc-2 gives
- [ ] #3 What last_active reports is settled with the engineer in the plan: empty until it is tracked, or tracked now with what that costs
- [ ] #4 Both methods leave the not-yet list in dispatch and the completeness test covers them; they are proved without a socket the way the session and tab arms are, each with a recorded failing control
<!-- AC:END -->
