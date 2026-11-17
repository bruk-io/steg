---
id: DRAFT-1
title: >-
  Rule: a coverage-proof test must check the real implementation, not a parallel
  list of its own names
status: Draft
assignee: []
created_date: '2026-11-17T23:10:00Z'
labels:
  - rule-change
dependencies: []
references:
  - TASK-1.121
  - TASK-1.126
type: task
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found twice in review: a proof step that is supposed to make an uncovered case "fail the build" instead checks a hand-maintained list against the test's own fixture, so it can never fail even when the real implementation changes - in TASK-1.121 and TASK-1.126.

TASK-1.126: PR images offered as proof that a per-glyph safety check works were both ASCII-only, so neither could distinguish the check working from a no-op - the exact branch the bug lived in never appeared in the evidence.

TASK-1.121: `keys.rs`'s own `the_table_covers_every_named_key_this_module_maps` was meant to be the plan's own named proof ("a row added to the plan's own prose without a matching assertion fails the build, not just a missed case") but instead asserts a hardcoded list of key names against the test's own `table()` fixture, never against `named_key_bytes`'s real match arms - adding, removing or renaming a mapped key would not fail this test or the build either way.

Add to plugins/team/rules/testing.md (or wherever "A test offered as proof that a mechanism works has been seen to fail with the mechanism removed" lives): a coverage/completeness check cited as proof that every case in a table, enum or list is exercised must read its own completeness from the real implementation (the match arms, the enum's variants, the function under test) rather than from a second, hand-written list that only mirrors it - a control for such a check should include adding a real case to the implementation and confirming the check then fails, not just confirming it passes today.
<!-- SECTION:DESCRIPTION:END -->
