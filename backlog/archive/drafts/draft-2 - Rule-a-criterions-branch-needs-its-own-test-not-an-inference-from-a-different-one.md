---
id: DRAFT-2
title: >-
  Rule: a criterion's branch needs its own test, not an inference from a
  different one
status: Draft
assignee: []
created_date: '2026-11-17T04:27:00Z'
labels:
  - rule-change
dependencies: []
references:
  - TASK-1.125
  - TASK-1.97
type: task
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found twice in review: a worker's notes (or a review's own narrow pass) declare an acceptance criterion's clause covered by reasoning about a nearby test or the code's own shape, when no test actually executes the branch that clause names - in TASK-1.125 and TASK-1.97.

TASK-1.97: the notes dropped the "mid-exchange" timeout branch's own negative control, saying "P2 above already exercises it directly" - P2 could only ever reach the "to start" branch (`first` is set False only after a successful read), so the mid-exchange branch was asserted covered by a control that structurally could not reach it.

TASK-1.125: the implementation notes claim AC #1 ("the client's stream is closed at once ... a later pane.detach ... answers as for a pane never attached") is delivered by `arm_attachment_answers_false_once_the_pane_has_already_closed`, a test that calls `Runtime::arm_attachment` directly and never reaches `connection.rs`'s own `pane.attach` match arm (the `if armed {...} else {...}` branch that actually closes the stream and leaves `pane_streams` empty). The notes' "so a later pane.detach answers correctly" is an inference from reading the code, not a recorded run of any test that calls `pane.detach` after a false arm.

Add to plugins/team/review.md (or a new plugins/team/rules/testing.md rule, whichever the rule-change's reviewer prefers): when a criterion or plan names a specific branch, path or consequence as needing proof, the test behind it must call code that actually executes that exact branch - not a different test whose passing is merely consistent with it, and not a description of what the code does read by eye. Before citing a test as a clause's proof, name the line inside that test where execution enters the branch in question; if no such line exists, the clause is unproven, whatever the summary concludes.
<!-- SECTION:DESCRIPTION:END -->
