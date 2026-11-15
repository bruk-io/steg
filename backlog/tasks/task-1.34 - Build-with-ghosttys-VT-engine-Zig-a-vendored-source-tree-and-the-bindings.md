---
id: TASK-1.34
title: 'Build with ghostty''s VT engine: Zig, a vendored source tree, and the bindings'
status: To Do
assignee: []
created_date: '2026-11-15T01:32:00Z'
updated_date: '2026-11-15T02:15:00Z'
labels:
  - size-3
milestone: m-0
dependencies:
  - TASK-1.31
parent_task_id: TASK-1
type: task
project: bruk-io-steg
asdlc_accepted_from: DRAFT-4
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
R92 (doc-9) and decision-3. Bruk chose ghostty's VT engine on 2026-11-15 and accepted the heavier build. The spike's findings on the bindings are in decision-3, section 8.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The workspace builds against `ghostty-vt` with the network unavailable, from a vendored copy of ghostty's source
- [ ] #2 The Zig version is pinned, and CI builds with it
- [ ] #3 A test feeds bytes to the engine and reads the screen back
- [ ] #4 `spikes/task-1.1-terminal-library/` is deleted
<!-- AC:END -->
