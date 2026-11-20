---
id: TASK-1.142
title: just run builds the steg binary the app starts its server with
status: To Do
assignee: []
created_date: '2026-11-20T03:39:00Z'
labels:
  - size-1
milestone: m-4
dependencies: []
parent_task_id: TASK-1
project: bruk-io-steg
asdlc_accepted_from: DRAFT-2
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
