---
id: DRAFT-1
title: >-
  Rule: a coverage claim needs a negative control, not just an assertion that
  matches the intended outcome
status: Draft
assignee: []
created_date: '2026-11-15T21:37:00Z'
labels:
  - rule-change
dependencies: []
references:
  - TASK-1.76
  - TASK-1.81
type: task
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found twice in review: a test the notes cite as proof that some mechanism (a refusal, a general reapply-sizes step) works actually passes even when that mechanism is removed or reverted, because the assertions only check an outcome that has another, unrelated cause - in TASK-1.81, api_call_refuses_to_call_itself passed against reverted code because the request was refused earlier for an unrelated reason (a missing "target" key); in TASK-1.76, a_tabs_declared_size_reaches_every_pane_including_one_split_in_afterward still passed with Runtime::handle's general post-dispatch sizing step disabled, because the split direction only changes columns, the test only asserts row counts, and the new pane's own row count already comes from its initial spawn size, not the step under test.

Add to plugins/team/review.md's Compliance bullet, or a new rules/testing file it points to: when a test is cited as proof that a specific mechanism (not just an API call) produced a result, the reviewer (and, before that, the worker claiming the coverage) should confirm the test cannot pass by some other path - e.g. by mentally or actually disabling the mechanism and rerunning the test - before relying on it as evidence. A coverage claim without this check can pass review while proving nothing about the thing it's named for.
<!-- SECTION:DESCRIPTION:END -->
