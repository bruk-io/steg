---
id: DRAFT-1
title: >-
  Rule: a named error/edge-case branch needs a test exercising it, not just a
  happy-path test
status: Draft
assignee: []
created_date: '2026-11-15T15:09:00Z'
labels:
  - rule-change
dependencies: []
references:
  - TASK-1.39
  - TASK-1.35
type: task
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found twice in review: implemented, working code for a specific branch or method is left with no test exercising it, even though the plan, an acceptance criterion, or the review brief named that exact case as worth proving - in TASK-1.39 (the link_at grow-and-retry path for an address longer than the 256-byte stack buffer, and resize tested in only one combined direction, never growing or shrinking alone) and in TASK-1.35 (Session::set_colour and Session::set_label implemented and claimed as covered by the notes, with no test for either).

Add to plugins/team/review.md's Evidence section (or a new rules/testing-rust.md line): when a function has more than one return/error path (e.g. an Ok/Err split, a retry branch, a growth-or-shrink pair), a clause claiming it is "covered" needs a test reaching each distinct path, not just the first one exercised by the primary happy-path test - the same bar review.md already states for acceptance-criterion clauses should extend explicitly to a function's own branches when the branch was named in the plan or brief as a risk.
<!-- SECTION:DESCRIPTION:END -->
