---
id: TASK-2
title: 'Rule: re-record evidence checks on the final commit, not the uncommitted tree'
status: To Do
assignee: []
created_date: '2026-11-14T05:23:00Z'
labels:
  - rule-change
dependencies: []
references:
  - TASK-1.2
  - TASK-1.3
type: task
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found twice in review: a worker runs its scripted checks (the acceptance-criterion demonstration, the gates/mechanical tests) while the deliverable is still uncommitted, then makes one commit at the end and only re-runs a small subset (e.g. `just check`, one `cargo test`) on that final commit. The recorded evidence for the other checks therefore points to an uncommitted tree a later reader cannot reproduce by checking out the commit alone, even when the code did not actually change. In TASK-1.3 and TASK-1.2 (bruk-io-steg), the reviewer had to independently rebuild and rerun the scripts against the final commit to confirm the evidence still held.

Add to plugins/team/review.md (or a rules/ file it points to): a worker's "Finish" step must re-record every check the task's Proof section lists against the branch's final commit, not only the cheapest ones, before `asdlc finish` - or the task's notes must say explicitly why a given check's uncommitted-tree result is known to still hold (e.g. "no file the check touches changed after that run"). This keeps the Evidence pass's "run on this branch, after its last change" criterion from being satisfied by evidence that only ever ran on an unreproducible tree.
<!-- SECTION:DESCRIPTION:END -->
