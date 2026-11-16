---
id: DRAFT-1
title: 'Rule: a recorded check, not a prose claim, backs every proof step'
status: Draft
assignee: []
created_date: '2026-11-16T02:14:00Z'
labels:
  - rule-change
dependencies: []
references:
  - TASK-1.45
  - TASK-1.33
type: task
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found twice in review: implementation notes narrate that a check or negative control ran and what it showed ("recorded timing out, exit 124 at 15s"; "deliberately broke ..., confirmed just check fails") with no matching recorded check block (asdlc record-check output) anywhere on the task, in TASK-1.45 (the ConnectionCleanup shutdown-before-join negative control) and TASK-1.33 round 1 (AC #5's architecture-validation proof). In both cases every sibling proof step in the same task did have a real recorded block, so the gap is easy to miss at a glance.

Add to plugins/team/review.md (or a rules/evidence.md it points to): a sentence in the Evidence pass stating explicitly that a claim about what a check or negative control showed, with no matching `record-check` block on the task, is treated the same as a missing control - the reason being that prose is not falsifiable after the fact (as TASK-1.45's own review found: the claimed failure did not reproduce when repeated independently), while a recorded block is.
<!-- SECTION:DESCRIPTION:END -->
