---
id: TASK-1.50
title: 'CLI: send input to a pane, capture its screen, wait, and watch events'
status: To Do
assignee: []
created_date: '2026-11-15T01:33:00Z'
updated_date: '2026-11-16T00:35:00Z'
labels:
  - size-3
milestone: m-3
dependencies:
  - TASK-1.49
  - TASK-1.47
  - TASK-1.46
  - TASK-1.90
parent_task_id: TASK-1
type: task
project: bruk-io-steg
asdlc_accepted_from: DRAFT-20
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
doc-2 R35 and section 8.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 `pane send` sends standard input, or `--text`, or named keys
- [ ] #2 `pane capture` prints a pane's screen, and its scrollback when asked, with no window open
- [ ] #3 `pane wait --for exit` exits 0 and prints the program's own exit code; a timeout is status 1
- [ ] #4 `event watch` prints one record per line and keeps only the named events
<!-- AC:END -->

## Comments

<!-- COMMENTS:BEGIN -->
created: 2026-11-16T00:35:00Z

---

Now depends on TASK-1.90 (the server side of pane send, capture and wait), accepted 2026-11-15.

---
<!-- COMMENTS:END -->
