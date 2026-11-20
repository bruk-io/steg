---
id: TASK-1.142
title: just run builds the steg binary the app starts its server with
status: In Progress
assignee:
  - '@bruk.w1'
created_date: '2026-11-20T03:39:00Z'
updated_date: '2026-11-20T03:41:00Z'
labels:
  - size-1
milestone: m-4
dependencies: []
parent_task_id: TASK-1
project: bruk-io-steg
asdlc_accepted_from: DRAFT-2
asdlc_branch: task-1.142-just-run-builds-the-steg-binary-the-app
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found 2026-11-19 when the engineer ran the app from main: the window came up with no server. The app does autostart a server (crates/app/src/startup.rs resolve_binary: STEG_BINARY, else the steg binary beside its own executable), but the run recipe in the justfile only runs cargo run --package steg-app, which never builds the steg package. The steg binary in target/debug was from 15 Nov and answered 'not yet implemented', so the autostart spawned a program that could not serve. Fix in the justfile's run recipe: build the steg package first (same ZIG handling as the recipe already has), then run the app. No product code changes. Follow-up worth noting in the plan, not building here: what the app shows when the binary it autostarts fails.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 From a clean target directory, just run alone gives a window with a working pane: the server binary the app autostarts is built from the same source as the app
- [ ] #2 A stale steg binary beside the app is rebuilt by just run before the app launches
- [ ] #3 just check still passes, including the recipe-name check
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Read the worktree's justfile to find the run recipe (line 229).
2. Update the run recipe's comment to explain why it builds steg first: the app starts its server from the steg binary beside its own executable via resolve_binary() in startup.rs, so the binary must be current.
3. Add a new line before 'cargo run --package steg-app' that runs 'cargo build --package steg' with the same environment variable handling (ZIG export and rust workspace check).
4. Verify no other recipes are changed and no product code is modified.
5. Run 'just --dry-run run' to show the build line before the run line.
6. Run 'just check' with 600s timeout to verify all checks pass.
7. Commit the change with the standard trailer.
8. Push the branch to origin and open a PR.
9. Wait for CI to pass and then finish the task.
<!-- SECTION:PLAN:END -->
