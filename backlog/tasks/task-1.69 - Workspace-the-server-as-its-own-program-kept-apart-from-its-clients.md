---
id: TASK-1.69
title: >-
  Workspace: the server's code kept apart from its clients, and the steg
  library's package
status: To Do
assignee: []
created_date: '2026-11-15T01:44:00Z'
updated_date: '2026-11-15T02:21:00Z'
labels:
  - size-2
milestone: m-0
dependencies: []
parent_task_id: TASK-1
type: task
project: bruk-io-steg
asdlc_accepted_from: DRAFT-39
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
doc-8 R98 and R101, doc-13 R61, and doc-1 section 7: one repository for now, with the server, the CLI, the app and the steg library as packages that could each move out. Today nothing stops a client's package depending on the server, and there is no library.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The server and the CLI ship as one binary, built by a package that only puts the two together (R61); the server's package and the CLI's package do not depend on each other
- [ ] #2 The app's package contains no server code and does not depend on the server or the kernel
- [ ] #3 A package for the steg library exists, and the CLI and the app depend on it; it has no server logic and no dependency on the server (R101)
- [ ] #4 `boundaries.toml` refuses each dependency above that is not allowed, with its reason, and `just boundaries` fails when one is added
- [ ] #5 The server, the CLI, the app and the library each build and test as a package by itself
- [ ] #6 `.claude/rules/architecture.md` states the rule, citing R98 and R101
<!-- AC:END -->
