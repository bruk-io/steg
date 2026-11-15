---
id: DRAFT-39
title: 'Workspace: the server as its own program, kept apart from its clients'
status: Draft
assignee: []
created_date: '2026-11-15T01:44:00Z'
labels:
  - size-2
milestone: m-0
dependencies: []
parent_task_id: TASK-1
type: task
project: bruk-io-steg
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
doc-8 R98, and doc-1 section 7: one repository for now, with the server, the CLI and the app as packages that could each move out. Today the `steg` crate links the server and the CLI into one binary, and nothing stops a client depending on the server.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The server builds as a program of its own, and the CLI's binary and the app contain no server code
- [ ] #2 A library holds what the server and its clients share (the API description and the wire protocol's types), and it has no server logic and no dependency on the server
- [ ] #3 `boundaries.toml` refuses a dependency from the CLI or the app on the server or the kernel, each with its reason, and `just boundaries` fails when one is added
- [ ] #4 The server, the CLI and the app each build and test as a package by itself
- [ ] #5 `.claude/rules/architecture.md` states the rule, citing R98
<!-- AC:END -->
