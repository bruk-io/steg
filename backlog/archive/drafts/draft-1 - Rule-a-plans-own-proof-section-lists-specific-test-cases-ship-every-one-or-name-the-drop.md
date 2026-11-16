---
id: DRAFT-1
title: >-
  Rule: a plan's own proof section lists specific test cases - ship every one or
  name the drop
status: Draft
assignee: []
created_date: '2026-11-16T01:53:00Z'
labels:
  - rule-change
dependencies: []
references:
  - TASK-1.78
  - TASK-1.77
type: task
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found twice in review: an approved plan's own Proof/evidence section names specific test cases or methods as what proves an acceptance criterion, and the worker ships real tests for only part of that named set, with the notes claiming the criterion is covered and saying nothing about the gap - caught only by the reviewer re-checking the plan against the diff, in TASK-1.78 and TASK-1.77.

TASK-1.77: the plan's section-5 table named all 8 outcome rows (including status 4/Refuse and status 1/Err) as the proof for AC #5; the 8 committed tests only ever exercised status 0 through the real binary - "a table is not a run."

TASK-1.78: the plan's own section 9 proof for AC #4 named three methods in one promised test ("a second thread issues pane.show (then, separately, a plain pane.close, then pane.swap) against the same pane that is being forced"), plus named pane.move/pane.set in its restated guarantee (section 6, "every method"); the committed suite tests pane.show only, and a second, differently-scoped test (two concurrent --force calls) whose own comment concedes it proves the Mutex, not the guarantee in question. pane.move/pane.set/pane.swap/plain-close against a forced pane are never exercised.

Add to plugins/team/rules/testing.md: when a plan's own proof section enumerates specific cases, methods or table rows as what proves an acceptance criterion, the worker ticks off each one against the final diff before recording the task done, and names any dropped item in the build notes as an explicit departure - a criterion is not covered by citing the plan's table or proof-step list alone, only by the tests that actually reached each item in it. Reason: review.md's evidence rule already says an unmet clause is a finding "whatever the summary says," but the two incidents above show the gap surviving to review anyway - the miss is at build time, before the summary is even written, where nothing currently makes the worker reconcile the proof section's own named list against the shipped tests one item at a time.
<!-- SECTION:DESCRIPTION:END -->
