---
id: TASK-1.119
title: 'CLI: connecting to the server, calling a method, and the exit statuses'
status: To Do
assignee: []
created_date: '2026-11-16T13:35:00Z'
labels:
  - size-2
milestone: m-3
dependencies:
  - TASK-1.49
parent_task_id: TASK-1
project: bruk-io-steg
asdlc_accepted_from: DRAFT-2
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Split from TASK-1.49 by its plan on 2026-11-16, under the engineer's standing decision to split by default. doc-2 sections 5, 6 and 8 (R32, R34); decision-4; decision-1; doc-8 R3 and R99; doc-16 R98, R101. TASK-1.49's plan, section 'The split', part 2, holds the worked design, including the fixture that accepts and closes with no hello and the use of TASK-1.86's idle stop for the end-to-end proof if it has merged. Carries TASK-1.49's criteria 3, the autostart half of 4, and 7.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Every generated command connects through the sdk, starting a server on demand when none is running, calls its method and exits with the status doc-2 section 6 gives; --no-autostart exits 4 without starting one
- [ ] #2 Each exit status in doc-2 section 6 is produced by a test through the real binary against a real server, and the mapping from the API's error codes is read from the API description, not written a second time
- [ ] #3 With --json a command prints the server's reply unchanged, so the CLI is usable as soon as this task lands; plain text and -q are the next task's
- [ ] #4 A client whose connection is accepted and then closed before a hello arrives treats that the same as no server being there, and arbitrates again exactly once; the logic is in the sdk, where the app will use it too; shown with real processes
- [ ] #5 api call sends the parameters given as one JSON object on standard input
<!-- AC:END -->
