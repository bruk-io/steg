---
id: TASK-1.71
title: >-
  Boundaries: a CLAUDE.md for each package, rules for what is judgement, and
  lints for what is not
status: To Do
assignee: []
created_date: '2026-11-15T02:35:00Z'
labels:
  - size-3
milestone: m-0
dependencies:
  - TASK-1.69
parent_task_id: TASK-1
type: task
project: bruk-io-steg
asdlc_accepted_from: DRAFT-2
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
doc-16: R104, R110, R111. Once the five packages exist, make their boundaries hard to cross by accident. What a program can check becomes a lint; what needs judgement becomes a rule scoped to the package; what an agent should know on opening a package is its CLAUDE.md. Today the only check is which crate depends on which, and the repository's rules, the plugin's layer rules and the specs say overlapping things.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Code in the sdk or the kernel that uses a file, a socket, a process, the clock or an async runtime fails `just lint`, shown by a recorded run for each package; the mechanism (for example clippy's disallowed types and methods, set per package) is named in the plan
- [ ] #2 The kernel depending on the sdk with an optional part turned on fails `just boundaries`, with a test in `tools/tests`
- [ ] #3 Every refusal's message names the requirement it enforces and why
- [ ] #4 Each of the five packages has a CLAUDE.md with what it is, what it may and may not depend on, how to run its tests, and a pointer to doc-16; none restates a lint
- [ ] #5 `.claude/rules/` has the boundary rules that need judgement, each scoped by `paths` to the package it is about; lines a lint now enforces are removed from `architecture.md` and replaced by the lint's name
- [ ] #6 The plan lists what the repository plugin's layer rules should become so that nothing is said twice; the orchestrator makes that change through asdlc-plugin
<!-- AC:END -->
