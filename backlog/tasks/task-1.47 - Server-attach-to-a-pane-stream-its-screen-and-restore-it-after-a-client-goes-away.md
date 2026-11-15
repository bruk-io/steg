---
id: TASK-1.47
title: >-
  Server: attach to a pane, stream its screen, and restore it after a client
  goes away
status: To Do
assignee: []
created_date: '2026-11-15T01:33:00Z'
updated_date: '2026-11-15T14:41:00Z'
labels:
  - size-5
milestone: m-0
dependencies:
  - TASK-1.45
  - TASK-1.42
  - TASK-1.40
  - TASK-1.74
parent_task_id: TASK-1
type: task
project: bruk-io-steg
asdlc_accepted_from: DRAFT-17
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
doc-8 R2, doc-9 R15, doc-15 R91. decision-2, sections 3, 5 and 6, including its two ordering rules and its back-pressure policy.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Attaching returns a snapshot, and updates follow it in order with none lost and none sent for a stream the client has not been told about
- [ ] #2 Scrollback is fetched in pages by absolute row identity
- [ ] #3 A client that falls behind gets one fresh screen-sized snapshot, without scrollback; the program is never blocked
- [ ] #4 Input arrives on the stream the client attached with; each attachment declares a size, and an update reports the terminal's size when it changes
- [ ] #5 A client killed mid-output and a new one attaching afterwards ends up with the server's screen and scrollback byte for byte, as an automated test
- [ ] #6 `spikes/task-1.2-wire-protocol/` is deleted
- [ ] #7 Attaching, input, scrollback pages and declaring a size are offered by the steg library, and this task's tests go through it (R101)
<!-- AC:END -->
