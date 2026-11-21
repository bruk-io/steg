---
id: DRAFT-4
title: 'Main is red: JSON key order changes with what steg is built beside'
status: Draft
assignee: []
created_date: '2026-11-21T20:09:00Z'
labels:
  - size-2
milestone: m-3
dependencies: []
parent_task_id: TASK-1
priority: high
project: bruk-io-steg
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Main's CI failed at d50a5d20 (the merge of PR 87, TASK-1.120) and blocks every change whose affected tests build the app (TASK-1.136 hit it). cli::output::tests::a_field_declared_text_but_really_structured_renders_its_own_compact_json and cli::output::tests::nested_plain_and_json_are_identical_bytes in crates/steg/src/cli/output.rs assert exact JSON text with keys in sorted order. gpui turns on serde_json's preserve_order feature, and cargo unifies features across a build: built alone (-p steg, what PR CI's affected tests ran) serde_json sorts keys and the tests pass; built with the workspace (just test-all, or any run that includes the app) keys keep insertion order and the tests fail. The same holds for the product: the bytes steg prints for nested JSON depend on what it was built beside. Decide the order on purpose (recommended: declare preserve_order where steg's crates depend on serde_json, so output follows the API description's order in every build, and correct the two tests' expected text; the alternative is tests that compare parsed values and an output order left unspecified) and make it hold in both builds. The two existing tests encode the accidental order, so they change with the fix: say so in the notes. Also say whether the PR CI's affected-test selection should have caught this (PR 87 was green there). doc-2 section on output formats; TASK-1.120.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 serde_json's key order in steg's output is the same whether the steg package is built alone or with the whole workspace, chosen on purpose and stated in the notes
- [ ] #2 just test-all and the affected-tests run of a change touching crates/ui both pass; the two output.rs tests pass in both builds
- [ ] #3 A recorded pair shows the two tests failing in the whole-workspace build before the fix and passing after
<!-- AC:END -->
