---
id: DRAFT-25
title: Onboard steg as a repository plugin once the workspace exists
status: Draft
assignee: []
created_date: '2026-11-14T00:14:00Z'
labels:
  - size-2
milestone: m-0
dependencies:
  - DRAFT-6
type: task
project: bruk-io-steg
parent_task_id: DRAFT-1
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
asdlc's orchestrator onboards a repository the first time work needs it, but before the workspace task there are no commands to verify and no layers to read. This repeats the onboard-repo skill against the real workspace so workers get verified commands and a layer map that match the code. The layers follow .claude/rules/architecture.md.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 asdlc-plugin check passes for bruk-io-steg, with just check, just test and just build verified on a clean checkout
- [ ] #2 The plugin's layers match the crates the workspace task created
<!-- AC:END -->
