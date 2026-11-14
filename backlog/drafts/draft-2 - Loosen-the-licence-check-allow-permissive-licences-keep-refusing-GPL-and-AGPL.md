---
id: DRAFT-2
title: >-
  Loosen the licence check: allow permissive licences, keep refusing GPL and
  AGPL
status: Draft
assignee: []
created_date: '2026-11-14T05:32:00Z'
labels:
  - size-1
milestone: m-0
dependencies:
  - TASK-1.5
parent_task_id: TASK-1
type: task
project: bruk-io-steg
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Bruk's direction of 2026-11-14: 'the licence check needs to be a bit more lax'. Today deny.toml refuses any licence not on a short list, and .claude/rules/licence.md tells a worker never to add one, so a permissive licence nobody thought of stops work: TASK-1.1 found the Ghostty path refused over webpki-root-certs (CDLA-Permissive-2.0), a build-only dependency that never reaches the binary. The line that matters for an MIT project that may be distributed (R59) is strong copyleft, not the exact list. Reading by @bruk.o, for Bruk to correct: permissive and file-scoped licences are allowed and a worker may add one with its reason; GPL, AGPL and LGPL stay refused and only Bruk can change that.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 deny.toml allows CDLA-Permissive-2.0 and the other common permissive licences (0BSD, BSL-1.0, Unicode-DFS-2016, OpenSSL), each still listed by name
- [ ] #2 just check still fails when a GPL, AGPL or LGPL dependency is added, shown by a probe that is reverted
- [ ] #3 .claude/rules/licence.md says a worker may add a permissive or file-scoped licence to the allow list with a comment naming the crate and why, and must raise anything copyleft beyond file scope with Bruk
- [ ] #4 plugins/team/review.md's licence line matches the rule
<!-- AC:END -->
