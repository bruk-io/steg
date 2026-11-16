---
id: DRAFT-1
title: 'Rule: record every negative control''s failing run, don''t just claim it'
status: Draft
assignee: []
created_date: '2026-11-16T03:01:00Z'
labels:
  - rule-change
dependencies: []
references:
  - TASK-1.78
  - TASK-1.45
type: task
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found twice in review: a worker's implementation notes claimed a test had a negative control (the mechanism removed, the test seen to fail) with no recorded check backing that specific claim, and a reviewer who reproduced the control found the test still passed - in TASK-1.78 (two tests: pane::ending_tests::restarting_then_forcing_ends_the_new_program_not_the_old_one and a_flooding_program_that_exits_by_itself_mid_grace_is_not_misclassified_as_a_straggler, the latter undermined specifically by pty/kernel buffering that let the backlog drain before the forced close ran) and in TASK-1.45 (the ConnectionCleanup::drop shutdown-before-join fix, undermined by a control queue small enough to fit the OS send buffer without the writer ever blocking). In both cases the underlying fix was real and separately provable; only the specific test's claimed control was false.

Add to plugins/team/rules/testing.md: record every negative control's failing run with `asdlc record-check`, even one you are confident will fail - a prose claim with no recorded run is itself a finding, and an end-to-end test that depends on a buffer, backlog or other timing condition to exercise its mechanism can stop discriminating that mechanism (the condition races closed before the assertion runs) without the author noticing unless the failing run is actually executed and read, not assumed from the production fix alone.
<!-- SECTION:DESCRIPTION:END -->
