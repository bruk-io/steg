---
id: DRAFT-2
title: 'Server: runtime.rs split into modules by concern'
status: Draft
assignee: []
created_date: '2026-11-18T02:09:00Z'
labels:
  - size-3
milestone: m-3
dependencies: []
parent_task_id: TASK-1
project: bruk-io-steg
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Agreed with the engineer on 2026-11-17 to let more server tasks run at once. runtime.rs was touched by 12 of the last 40 merges, the most of any file. This task only moves code. It must run while no other task is editing crates/server/src/runtime.rs, and it can run beside the split of the API description and the dispatch tests. Whether pane.rs (2,577 lines) and connection.rs (1,314) deserve the same is for the plan to say, as a follow-up and not as part of this task.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 crates/server/src/runtime.rs (4,936 lines) becomes a module with one file per concern, such as the tree and its lookups, attach and arm, input and declared size, close and force_end, start and stop; the plan names the split and why each seam is there
- [ ] #2 Runtime's public surface is unchanged, and the tests move with the code they test
- [ ] #3 No function body changes beyond imports and visibility, shown mechanically: a recorded comparison of function bodies or of moved lines before and after
- [ ] #4 The list of test names that run is identical before and after, none ignored; just check passes
<!-- AC:END -->
