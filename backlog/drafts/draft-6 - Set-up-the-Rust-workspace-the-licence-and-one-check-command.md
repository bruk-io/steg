---
id: DRAFT-6
title: Set up the Rust workspace, the licence and one check command
status: Draft
assignee: []
created_date: '2026-11-13T22:52:00Z'
updated_date: '2026-11-14T00:12:00Z'
labels:
  - size-2
milestone: m-0
dependencies: []
parent_task_id: DRAFT-1
type: task
project: bruk-io-steg
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
- [ ] #5 The check command refuses a dependency whose licence is not on an allow list that excludes GPL and AGPL (R59)
- [ ] #6 The check command refuses a crate dependency that crosses the boundaries in .claude/rules/architecture.md, starting with the kernel depending on anything that does I/O
- [ ] #7 The justfile's recipes (check, test, test-all, fmt, lint, deny, build, run) each do real work on the workspace, and none passes by doing nothing
- [ ] #8 just test runs only the crates a change touches and the crates that depend on them, chosen by steg-affected in tools/ and not by whoever runs it; a change to the workspace manifest, the lock file or the toolchain runs everything. The selection is proven against the real workspace, since tools/ has only been tested against made-up crates
- [ ] #9 Every recipe prints one line when it passes, and on failure prints the failure and the path of the full log (steg-quiet in tools/)
<!-- AC:END -->
