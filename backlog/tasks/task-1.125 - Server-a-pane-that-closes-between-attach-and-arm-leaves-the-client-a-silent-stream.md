---
id: TASK-1.125
title: >-
  Server: a pane that closes between attach and arm leaves the client a silent
  stream
status: To Do
assignee: []
created_date: '2026-11-17T02:40:00Z'
labels:
  - size-2
milestone: m-4
dependencies:
  - TASK-1.47
parent_task_id: TASK-1
project: bruk-io-steg
asdlc_accepted_from: DRAFT-3
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found by TASK-1.47's worker on 2026-11-16 after its second review had passed, so it could not go into that task. pane.attach replies with a snapshot, then the connection arms the attachment in a second step. If the pane closes between the two, arm_attachment does nothing and says nothing: the client has been told it is attached, the stream stays open, and no update or close ever arrives on it. The client can still learn of the closure from tree events and detach, so nothing leaks past the connection, but an attached client that does not watch events waits forever. The gap between the two steps existed before TASK-1.47's lock fix and is a few instructions wide in the normal case; a busy pane widens it. The worker has a fix and a test ready as a patch in the session scratchpad (arm_attachment returns whether it armed; on false the connection closes the stream at once and records nothing). decision-2 section 3; doc-14.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 When a pane has closed by the time its attachment is armed, the client's stream is closed at once and the connection keeps no record of it; a later pane.detach for that target answers as for a pane that was never attached
- [ ] #2 A test builds that state deterministically at the runtime level and proves arm reports it, with a recorded failing control and passing run
- [ ] #3 A test at the socket level shows the client sees its stream close when the pane closes during a held-up attach, with a recorded control; or the notes say why that cannot be made deterministic and what was tried
<!-- AC:END -->
