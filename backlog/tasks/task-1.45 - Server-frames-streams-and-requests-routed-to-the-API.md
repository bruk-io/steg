---
id: TASK-1.45
title: 'Server: frames, streams, and requests routed to the API'
status: To Do
assignee: []
created_date: '2026-11-15T01:33:00Z'
updated_date: '2026-11-15T14:41:00Z'
labels:
  - size-3
milestone: m-0
dependencies:
  - TASK-1.44
  - TASK-1.43
  - TASK-1.70
  - TASK-1.75
parent_task_id: TASK-1
type: task
project: bruk-io-steg
asdlc_accepted_from: DRAFT-15
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
decision-2, sections 1 and 3: length-prefixed frames on one multiplexed connection, control on stream 0.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Frames are length-prefixed with the cap decision-2 gives, and a frame over the cap is refused without allocating for it
- [ ] #2 A request on the control stream reaches the API and its response or error comes back with the same id
- [ ] #3 Control is written before terminal data, so a flood on one stream does not delay a control call, shown by a test
- [ ] #4 Framing, the handshake and stream state come from the sdk (TASK-1.70, doc-16 R101); the server adds only the socket and the routing
<!-- AC:END -->
