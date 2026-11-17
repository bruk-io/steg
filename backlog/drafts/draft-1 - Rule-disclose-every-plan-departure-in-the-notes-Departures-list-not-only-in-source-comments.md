---
id: DRAFT-1
title: >-
  Rule: disclose every plan departure in the notes' Departures list, not only in
  source comments
status: Draft
assignee: []
created_date: '2026-11-17T04:23:00Z'
labels:
  - rule-change
dependencies: []
references:
  - TASK-1.114
  - TASK-1.89
type: task
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found twice in review: a worker changes something the approved plan specifically named, and the change is never listed in the task's own "Departures/decisions, with reasons" notes - only in source comments, or not flagged as a departure at all - in TASK-1.114 and TASK-1.89.

TASK-1.114: the plan's section 2 (quoting doc-14 verbatim) specified adding size: Option<Size> to ScreenUpdate::Delta. The shipped mechanism (Attachment::last_told_size forcing a fresh Full on any size-stale attachment) is a different, substantial redesign, well-reasoned in a source comment in crates/sdk/src/screen.rs ("doc-14's own wording... was tried and dropped") but never surfaced in the task's Departures list, which covers three unrelated items instead.

TASK-1.89: the plan's Risks/Order-of-work section committed to landing a ~79-line mechanical test refactor as its own commit, separate from new behaviour. It shipped squashed into one commit instead, and neither the implementation notes nor the worker's own disclosed-departure comment mention it.

Add to plugins/team/rules/testing.md (or a new plugins/team/rules/plan-departures.md, whichever the rule-change's reviewer prefers): when a worker departs from what the approved plan explicitly named - a mechanism, a file, a commit structure, an order of work - the departure and its reason belong in the task's Implementation Notes "Departures/decisions, with reasons" list before the task goes to review, every time, even when the departure is also explained in a source comment or is judged to be an improvement on the plan. A reviewer should never have to read source comments to discover that the plan was not followed.
<!-- SECTION:DESCRIPTION:END -->
