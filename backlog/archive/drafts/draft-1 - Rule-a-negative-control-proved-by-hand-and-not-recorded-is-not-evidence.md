---
id: DRAFT-1
title: 'Rule: a negative control proved ''by hand'' and not recorded is not evidence'
status: Draft
assignee: []
created_date: '2026-11-16T14:03:00Z'
labels:
  - rule-change
dependencies: []
references:
  - TASK-1.47
  - TASK-1.33
type: task
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found twice in review: a worker's implementation notes describe a proof-test's negative control as run "by hand" (or "by hand and reverted") instead of recorded through `asdlc record-check`, and review.md's Evidence rule already says this is not sufficient - in TASK-1.47 and TASK-1.33.

TASK-1.33: "Also proved by hand, not recorded as a check: deliberately broke architecture/models/steg/steg.c4, confirmed `just check` fails (AC #5), restored it, confirmed `just check` passes again." Review finding: "Only a prose description exists; no recorded check run shows the failure."

TASK-1.47: implementation notes say "every proof has a negative control run by hand and reverted" for ~10 controls the plan names (stale cached screen, NeedsResync vs CaughtUp, skipped seq increment, swapped ordering steps, unconditional wake-up, blocking enqueue_data, stop-while-armed, dropped derive, skipped validate, renamed field) - none appear as a recorded failing run anywhere on the task; every recorded check is a build/format/lint failure or a passing full-suite run.

Add to plugins/team/review.md (or plugins/team/rules/testing.md, wherever the team prefers): a line making explicit that a control a worker describes as "run by hand"/"proved by hand" without a corresponding `asdlc record-check` failing run counts as not recorded at all, the same as a missing control - so a worker's own prose assurance is not mistaken for the recorded evidence the rule already requires, and a reviewer does not have to re-derive this from the Evidence rule's general wording each time.
<!-- SECTION:DESCRIPTION:END -->
