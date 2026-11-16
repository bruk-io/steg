---
id: TASK-1.49
title: 'CLI: commands generated from the API description, in the steg package'
status: In Progress
assignee:
  - '@bruk.w5'
created_date: '2026-11-15T01:33:00Z'
updated_date: '2026-11-16T13:26:00Z'
labels:
  - size-4
milestone: m-3
dependencies:
  - TASK-1.45
  - TASK-1.77
parent_task_id: TASK-1
type: task
project: bruk-io-steg
asdlc_accepted_from: DRAFT-19
asdlc_branch: task-1.49-cli-commands-generated-from-the-api
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
doc-2, sections 2 to 6 and 8: R32, R34. decision-4. Nothing in the CLI names a method by hand.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Every API method has a command and every command an API method, checked by a test that fails when they drift
- [ ] #2 The grammar, the flags every command has, `--json`, and `-q` printing identifiers only each behave as doc-2 says
- [ ] #3 Each exit status in doc-2 section 6 is produced by a test
- [ ] #4 `steg version`, `steg completion <shell>` and `--help` work with no server running; `--no-autostart` exits 4 without starting one
- [ ] #5 A list is tab-separated with full identifiers, no header and no alignment, the same piped or not
- [ ] #6 The CLI lives in the `steg` package and reaches the server only through the sdk (doc-16 R98, R101)
- [ ] #7 A client whose connection is accepted and then closed before a hello arrives treats that the same as no server being there, and arbitrates again once; shown with real processes against a server stopping as the client connects (moved from TASK-1.86's fourth criterion, whose plan the engineer approved on 2026-11-16: nothing built before this task reads a hello)
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Dependency added by @bruk.o 2026-11-15: TASK-1.77 adds the steg entry point for server start and starting a server on demand, which the generated CLI needs to reach a server at all and must keep as its one hand-written case.

Carried from TASK-1.75's build (2026-11-15), for this task's plan to settle against doc-2: (1) pane.move calls Tree::move_pane_to_tab, which needs a direction that doc-2 section 8 gives pane move no parameter for; dispatch defaults to the right. (2) unset --label requires a value to be sent that dispatch then discards (the description declares the parameter with a value). (3) the api.call target collision is its own draft task.

Carried from TASK-1.77's planning (2026-11-15): when a server is stopping, a connection the accept loop takes in the instant it sees the stop flag is dropped with no reply, so the client sees connect succeed and then end of file, a third outcome beside refused and not found. The CLI's client must treat end of file before a server hello as no server there, and start one, exactly as it treats a refusal; say whether that belongs in the sdk's local-socket provider (which today only waits for the socket) or in the handshake's caller.

TASK-1.89 (tab list, tab show, pane list) and TASK-1.88 (server and client methods) were accepted on 2026-11-15 for methods the generated commands will call; this task's own tests should use methods that exist when it is built and need not wait for those.
<!-- SECTION:NOTES:END -->
