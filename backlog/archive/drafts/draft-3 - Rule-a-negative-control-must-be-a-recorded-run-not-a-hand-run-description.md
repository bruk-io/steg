---
id: DRAFT-3
title: 'Rule: a negative control must be a recorded run, not a hand-run description'
status: Draft
assignee: []
created_date: '2026-11-17T04:37:00Z'
labels:
  - rule-change
dependencies: []
references:
  - TASK-1.113
  - TASK-1.89
type: task
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found twice in review: a mechanism's negative control was claimed in the task's own notes but never recorded with `asdlc record-check` against the test as it stands, in TASK-1.113 (the outbound.rs broken-write control and test_support.rs's own run_bounded timeout control, both described as "run by hand, not committed") and TASK-1.89 (the session-scope filter and creation-cascade-order controls, described in notes with no recorded pair; the reviewer had to repeat both itself). TASK-1.113 also had a third case with no control claimed or performed at all (quiet.py's process-group kill).

Add to plugins/team/rules/testing.md: a control is evidence only once it is recorded with `asdlc record-check` as a failing run against the mechanism-removed code and a passing run against the code as it stands, both on the task; a control described in prose, "run by hand," or not committed is not evidence and leaves the finding open, because a reader cannot tell whether the control was run against the test in its current form.
<!-- SECTION:DESCRIPTION:END -->
