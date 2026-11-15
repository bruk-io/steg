---
id: TASK-1.66
title: Onboard steg as a repository plugin
status: In Progress
assignee:
  - '@bruk.w6'
created_date: '2026-11-15T01:33:00Z'
updated_date: '2026-11-15T02:23:00Z'
labels:
  - size-2
milestone: m-0
dependencies: []
parent_task_id: TASK-1
type: task
project: bruk-io-steg
asdlc_accepted_from: DRAFT-36
asdlc_branch: task-1.66-onboard-steg-as-a-repository-plugin
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
So that workers get this repository's verified commands and rules.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The repository's plugin is generated with commands that were run, and published in the tasks repository
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Plugin bruk-io-steg published to the tasks repository at fb39566c603b.
Commands:
- build: `just setup && just build`, verified 2026-11-14
- test: `just test-all && just test-tools`, verified 2026-11-14
- lint: `just fmt-check && just lint && just boundaries && just deny`, verified 2026-11-14
- check: `just check`, verified 2026-11-14
Layers:
- kernel: crates/kernel/**
- server: crates/server/**
- cli: crates/cli/**
- app: crates/app/**
- binary: crates/steg/**
- tools: tools/**, justfile, Cargo.toml, boundaries.toml, deny.toml, clippy.toml
Rules: app.md, binary.md, cli.md, kernel.md, server.md, testing-python.md, testing-rust.md, tools.md
Skills: none

Onboarded by @bruk.o with the asdlc onboard-repo skill, from commit 13f2998, when every crate was still a frame.
- Authoring kit used. Its audit removed lines that restated the repository's own .claude/rules, removed two lines the code did not support yet, added the root Cargo.toml to the tools layer, and added the Xcode need to every command.
- Not changed: every command has both needs and verified; asdlc-plugin check accepts it.
- just check, verified on the default branch, runs no crate tests there, because just test selects by diff. commands.toml says so.
Follow-ups for the repository's own checks:
- boundaries.toml restricts only steg-kernel, so a client depending on the server fails nothing. TASK-1.69 adds it.
- .claude/rules/architecture.md and CLAUDE.md cite requirements as being in doc-1; since the specs were reorganised they are in doc-2, doc-8, doc-13 and doc-15.
- crates/steg, crates/cli and crates/server comments name archived tasks (TASK-1.2, 1.4, 1.9, 1.13).
- Checkable and unchecked: the gpui pin being exact, and tools/pyproject.toml having no dependencies.
<!-- SECTION:NOTES:END -->
