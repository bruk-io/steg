---
id: DRAFT-1
title: >-
  Rule: a proof test's assertion must actually depend on the mechanism it claims
  to prove
status: Draft
assignee: []
created_date: '2026-11-16T01:17:00Z'
labels:
  - rule-change
dependencies: []
references:
  - TASK-1.84
  - TASK-1.81
type: task
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found twice in review: a test offered as proof of a mechanism keeps passing when that mechanism is removed or reverted, because the assertion can be satisfied by something else entirely - in TASK-1.81 (api_call_refuses_to_call_itself asserted only error.code, which a missing-selector error shares with the real refusal) and in TASK-1.84 (a_command_panes_own_program_printing_marks_does_not_change_its_status: the pane's status is already Running from start_program before any mark, and the script's own real exit drives Finished through the ordinary exit path regardless of whether the command-mark arms do anything, so removing drain_command_marks entirely left this test green).

Add to .claude/rules/testing-rust.md: A test that proves a mechanism must assert something that only that mechanism can produce - not a value already true before it runs, and not one a different path (an unrelated default, a real process exit, a case the code shares with a broken one) would also produce. Before trusting such a test, ask what else could make this assertion pass, and check that your negative control removes exactly the thing you mean to prove.
<!-- SECTION:DESCRIPTION:END -->
