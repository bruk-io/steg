---
id: DRAFT-1
title: 'sdk: the protocol''s logic, and the local-socket provider'
status: Draft
assignee: []
created_date: '2026-11-15T02:21:00Z'
updated_date: '2026-11-15T02:34:00Z'
labels:
  - size-3
milestone: m-0
dependencies:
  - TASK-1.45
  - TASK-1.46
parent_task_id: TASK-1
type: task
project: bruk-io-steg
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
