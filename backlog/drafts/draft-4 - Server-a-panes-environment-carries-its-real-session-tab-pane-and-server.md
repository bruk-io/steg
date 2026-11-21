---
id: DRAFT-4
title: 'Server: a pane''s environment carries its real session, tab, pane and server'
status: Draft
assignee: []
created_date: '2026-11-21T20:20:00Z'
labels:
  - size-2
milestone: m-3
dependencies: []
parent_task_id: TASK-1
project: bruk-io-steg
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
doc-2 section 4, Context (R33). Split out of TASK-1.51 by its plan. Today crates/server/src/runtime/start.rs launch_spec hands pane_environment an empty server string and identifiers made from 0, so a pane's STEG_SERVER is empty and its STEG_SESSION, STEG_TAB and STEG_PANE name nothing real (found by TASK-1.51's planner from a real pane). Give launch_spec the pane's real identifiers, looked up under the same lock that minted the pane, and the address the server bound (TASK-1.51's plan proposes a set-once Runtime::bind_server_address called from listen.rs run, so Runtime::new and its many test call sites do not change). STEG_BIN_DIR and PATH are already right and are not part of this. No wire, dispatch-context or CLI change here: TASK-1.51 reads these values. Keeps clear of crates/server/src/connection.rs (TASK-1.140).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A program started in a pane reads STEG_SESSION, STEG_TAB and STEG_PANE equal to that pane's own identifiers in the server's tree, proven from a real pane with an explicit program
- [ ] #2 STEG_SERVER in a pane holds the path of the socket its server listens on
- [ ] #3 A recorded control shows the test failing with the placeholders restored
<!-- AC:END -->
