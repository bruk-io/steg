---
id: DRAFT-1
title: 'Library: find or start a server, connect, call and subscribe'
status: Draft
assignee: []
created_date: '2026-11-15T02:21:00Z'
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
doc-8 R101, R3, R39, R99. The client side of the connection, written once in the steg library so that the CLI and the app hold no protocol code of their own. Attaching to a pane is added to the library by TASK-1.47.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A program using the library connects to a running server, and with none running starts one by running the steg binary and waits for its socket; it can be told not to start one
- [ ] #2 The library does the handshake and reports a version mismatch naming both versions
- [ ] #3 A method is called by name with its parameters and returns its result or its error; the methods come from the API description, with none written by hand
- [ ] #4 Events are subscribed to and arrive as typed values, and a dropped connection is reported, not hidden
- [ ] #5 The library's tests run against a real server over a real socket in a temporary directory
<!-- AC:END -->
