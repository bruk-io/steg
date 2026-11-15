---
id: TASK-1.31
title: 'Licence check: allow permissive licences, keep refusing GPL and AGPL'
status: To Do
assignee: []
created_date: '2026-11-15T01:32:00Z'
labels:
  - size-2
milestone: m-0
dependencies: []
parent_task_id: TASK-1
type: task
project: bruk-io-steg
asdlc_accepted_from: DRAFT-1
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
R59 (doc-13) and decision-3. The repository's own licence check refuses the ghostty path today, because a build-only dependency of the crate that fetches ghostty's source is under CDLA-Permissive-2.0. Bruk accepted widening the policy on 2026-11-15.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The licence step of `just check` passes for a dependency graph that includes `ghostty-vt` and its build dependencies
- [ ] #2 A crate under GPL or AGPL is still refused, shown by a recorded run or a test
- [ ] #3 `deny.toml` names the licences it allows; it has no blanket exception and no skipped crate
<!-- AC:END -->
