---
id: TASK-1.130
title: >-
  sdk and server tests: one file per noun for the API description and the
  dispatch tests
status: In Progress
assignee:
  - '@bruk.w6'
created_date: '2026-11-18T02:09:00Z'
updated_date: '2026-11-18T02:11:00Z'
labels:
  - size-3
milestone: m-3
dependencies: []
parent_task_id: TASK-1
project: bruk-io-steg
asdlc_accepted_from: DRAFT-1
asdlc_branch: task-1.130-sdk-and-server-tests-one-file-per-noun
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Agreed with the engineer on 2026-11-17 to let more server tasks run at once. Of the last 40 merges, 9 touched crates/sdk/src/description/methods.rs (2,122 lines) and 9 touched crates/server/tests/dispatch.rs (2,978 lines), because every new method appends to both, so two server tasks in flight nearly always conflict. methods.rs is already organised as session_methods, tab_methods, pane_methods and so on in one file. This task only moves code. It must run while no other task is editing these files, and it can run beside the runtime.rs split, which touches different files.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The session, tab, pane, server, event, client, setting and api parts of crates/sdk/src/description/methods.rs each live in their own file, with the shared helpers in one place; the description's serialised form is byte for byte the same before and after, shown by a recorded comparison
- [ ] #2 crates/server/tests/dispatch.rs is split by noun the same way; the list of test names that run is identical before and after, none ignored, shown by a recorded comparison
- [ ] #3 No behaviour changes: no public item is added, removed or renamed, and no function body changes beyond imports and visibility
- [ ] #4 just check passes, and the notes say which files a new method now touches
<!-- AC:END -->
