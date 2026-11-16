---
id: DRAFT-3
title: >-
  Rule: re-verify a claimed negative control after a test it proves is
  refactored
status: Draft
assignee: []
created_date: '2026-11-16T06:49:00Z'
labels:
  - rule-change
dependencies: []
references:
  - TASK-1.87
  - TASK-1.81
type: task
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found twice in review: a test offered as proof that a mechanism works is accepted on the strength of a negative control claimed earlier (in the plan, or in an earlier review round), without re-running that control against the test's current form - in TASK-1.87 and TASK-1.81.

TASK-1.87: the plan's own proof list for AC #1 said reverting pty.rs's env_clear() would fail the end-to-end leak test ("a variable set only in the test harness's own environment... appears in the pane anyway"). After a later review round rewrote that test around a fixture server and a throwaway home (to stop it reading the real machine's own login shell), the pane's own shell command was changed to echo only four named variables (TERM/STEG_SERVER/PATH/MARK_ZPROFILE) - STEG_TEST_LEAK_MARKER, the variable the leak assertion checks for, was never among them. The assertion `!text.contains(&marker_value)` then passed whether or not env_clear() was present: reverting it live (scratch edit, reverted) left the end-to-end test green. The control's claim was never re-verified after the refactor it survived.

TASK-1.81: `api_call_refuses_to_call_itself` was accepted as proof of the method-name-vs-target-key fix; reverting the fix made 10 of 12 sibling tests fail as expected but not this one, because its assertion checked only `error.code` (BadRequest), which the pre-fix code path also produces for an unrelated reason (a missing "target" key) - the control's claim ("this test tells `method` from `target`") was asserted, not re-checked against what actually discriminates.

Add to plugins/team/rules/testing.md: a negative control recorded once (in a plan or an earlier review round) is not evidence for the test's current form; whenever a test a control covers is edited after that control was recorded - a refactor, a fix for another finding, a rewritten fixture - the control must be re-run against the edited test before citing it as proof again, not cited by reference to its earlier run.
<!-- SECTION:DESCRIPTION:END -->
