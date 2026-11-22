---
id: TASK-1.153
title: 'Kernel: a cursor move with no cell change still reaches clients'
status: In Progress
assignee:
  - '@bruk.w4'
created_date: '2026-11-22T04:58:00Z'
updated_date: '2026-11-22T04:59:00Z'
labels:
  - size-2
milestone: m-4
dependencies: []
parent_task_id: TASK-1
project: bruk-io-steg
asdlc_accepted_from: DRAFT-1
asdlc_branch: task-1.153-kernel-a-cursor-move-with-no-cell-change
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found by the engineer in the live run of TASK-1.127 on 22 November 2026, and traced by that task's worker to main, not to the cursor change: at a shell prompt, arrow keys and backspace move the cursor but it is not redrawn until the next typed character. Cause: crates/kernel/src/terminal.rs Terminal::changes_since reads the cursor every time but attaches it only when the engine's dirty flag is Partial or Full; on Clean it returns ScreenUpdate::Unchanged, which carries no cursor (crates/sdk/src/screen.rs). The engine's dirty flag tracks cell content, not cursor position, so a cursor-only move is reported as nothing and the app's model applies nothing. A scratch test (write text, read, write CSI C, read again) fails with Unchanged; it was reverted and is not on any branch. The plan decides how the kernel knows the cursor moved (for example by remembering the cursor it last reported per sequence) and whether the update is a Delta with no rows or needs a new shape in the sdk; a new shape is a public API change and must be said plainly. Every attached client is affected, not only the app. Not checked: whether the shell's backspace erases a cell; the plan should cover both.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 After a write that only moves the cursor (for example CSI C, or a backspace that erases nothing), changes_since reports the new cursor position instead of Unchanged; a kernel test proves it and a recorded control shows it failing against today's code
- [ ] #2 When neither a cell nor the cursor has changed, changes_since still reports Unchanged, so an idle pane sends nothing
- [ ] #3 Cursor visibility and shape changes with no cell change are covered the same way, or the notes say why they already work
- [ ] #4 The app draws the cursor at its new cell after such an update, shown by a test at the app's model; just check passes
<!-- AC:END -->
