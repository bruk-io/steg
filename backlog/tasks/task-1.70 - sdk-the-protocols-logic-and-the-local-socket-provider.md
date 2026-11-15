---
id: TASK-1.70
title: 'sdk: the protocol''s logic, and the local-socket provider'
status: In Progress
assignee:
  - '@bruk.w5'
created_date: '2026-11-15T02:21:00Z'
updated_date: '2026-11-15T15:52:00Z'
labels:
  - size-3
milestone: m-0
dependencies:
  - TASK-1.43
parent_task_id: TASK-1
type: task
project: bruk-io-steg
asdlc_accepted_from: DRAFT-1
asdlc_branch: task-1.70-sdk-the-protocol-s-logic-and-the-local
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
doc-16: R101, R102, R103, R106; doc-8 R3, R39, R99. The protocol written once, with no I/O, for the server, the CLI and the app to consume, and the one provider that finds or starts a local server. Attaching to a pane is added to the sdk by TASK-1.47.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The sdk encodes and decodes frames, does the handshake and tracks each stream's state as logic that takes bytes and returns bytes and events, with no socket, clock or runtime, and its tests need none (R102)
- [ ] #2 A consumer supplies the transport through an interface the sdk defines; a test drives a client and a server end against each other in memory
- [ ] #3 The optional local-socket provider connects to a running server, and with none running starts one by running the steg binary and waits for its socket; it can be told not to start one (R103)
- [ ] #4 A version mismatch is reported naming both versions (R39)
- [ ] #5 A method is called by name with its parameters and returns its result or its error, and events arrive as typed values; the methods come from the API description, with none written by hand
- [ ] #6 No GPUI type and no particular async runtime appears in the sdk's public interface (R106)
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Order changed by @bruk.o on 2026-11-15: the handshake and framing are this task's and the server's socket task (TASK-1.44) uses them, so this comes first. The local-socket provider is tested here against a listener in a temporary directory; starting a real server end to end is proved in TASK-1.44.
<!-- SECTION:NOTES:END -->
