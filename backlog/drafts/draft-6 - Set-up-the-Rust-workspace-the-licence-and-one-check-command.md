---
id: DRAFT-6
title: Set up the Rust workspace, the licence and one check command
status: Draft
assignee: []
created_date: '2026-11-13T22:52:00Z'
updated_date: '2026-11-13T23:05:00Z'
labels:
  - size-2
milestone: m-0
dependencies: []
parent_task_id: DRAFT-1
type: task
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The frame everything else lands in: crates for the kernel, for the server and CLI binary, and for the app. R59 (MIT), R61 (the server and CLI are one binary), R58 (no telemetry, no network call the user did not ask for).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 One command runs formatting, lints and tests, and passes
- [ ] #2 The server and the CLI build as a single binary (R61)
- [ ] #3 The repository carries the MIT licence (R59)
- [ ] #4 The GPUI version is pinned
<!-- AC:END -->
