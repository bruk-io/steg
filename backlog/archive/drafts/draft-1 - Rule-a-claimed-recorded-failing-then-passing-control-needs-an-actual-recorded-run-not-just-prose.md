---
id: DRAFT-1
title: >-
  Rule: a claimed recorded failing-then-passing control needs an actual recorded
  run, not just prose
status: Draft
assignee: []
created_date: '2026-11-18T00:46:00Z'
labels:
  - rule-change
dependencies: []
references:
  - TASK-1.128
  - TASK-1.121
type: task
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found twice in review: a task's own notes or plan claim a mechanism has "a recorded failing-then-passing control" (or equivalent), but the board's actual recorded `just check`/`record-check` history never shows the specific test failing against the pre-fix code - only the reviewer's own, independently-repeated scratch control confirms it. TASK-1.128 (AC #4's socket-level tests, e.g. the forced-close MarkClosing-ordering test: no recorded run anywhere shows it failing pre-fix, though my own control confirmed the mechanism is real) and TASK-1.121 (Cmd-V paste: notes first claimed "its own unit tests and a recorded failing-then-passing control," later corrected, but the actual tests never reached the real clipboard-read path at all).

Add to plugins/team/rules/testing-rust.md (or wherever `asdlc record-check`'s own convention is documented): when a plan or acceptance criterion requires "a recorded failing control and passing run" for a specific test, the worker must run `asdlc record-check` (or equivalent) at the point that test is failing, before fixing it, not only at the final green state - a prose description in the implementation notes that a test "was recorded failing first" is not itself evidence unless a recorded run shows it.
<!-- SECTION:DESCRIPTION:END -->
