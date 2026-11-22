---
id: DRAFT-1
title: >-
  Rule: a test cited as proof of a mechanism must actually exercise that
  mechanism
status: Draft
assignee: []
created_date: '2026-11-22T05:57:00Z'
labels:
  - rule-change
dependencies: []
references:
  - TASK-1.150
  - TASK-1.136
type: task
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found twice in review: a test or control is credited in the task's notes as proving a specific mechanism, but the code path it actually exercises is a different one (a duplicate/parallel helper, or an unrelated branch), so the credited proof does not discriminate the mechanism it is attached to - in TASK-1.150 and TASK-1.136.

- TASK-1.150: the new "permanent regression guard" test calls a separate, independently written helper (spawn_hostile_listener_with_gap) instead of the actual fixed function (spawn_hostile_listener) that the other two tests, and the bug, depend on. A regression that reverted the real fixed function would leave this test green.
- TASK-1.136: the recorded failing-then-passing just test-tools pair is credited as proving the list-branch fix "reuses the existing _DIMENSIONLESS_FIELDS mechanism," but removing the relevant entry from that set left the same tests green - the real branch ignores the set entirely for lists.

Add to plugins/team/review.md (or a rules file it already cites, e.g. .claude/rules/testing-rust.md): when a task's notes name which test or control proves a mechanism, the reviewer must repeat that control by disabling or reverting the named mechanism specifically (not a nearby one, and not a different code path reached through a duplicate helper) and confirm the named test actually goes red; a control that passes with the named mechanism removed, or that exercises a copy/parallel implementation instead of the real one, is no evidence for it, even when some other test or path is genuinely correct.
<!-- SECTION:DESCRIPTION:END -->
