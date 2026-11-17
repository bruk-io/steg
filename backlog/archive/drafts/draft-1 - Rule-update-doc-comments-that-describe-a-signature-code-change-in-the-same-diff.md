---
id: DRAFT-1
title: >-
  Rule: update doc comments that describe a signature/code change in the same
  diff
status: Draft
assignee: []
created_date: '2026-11-17T21:23:00Z'
labels:
  - rule-change
dependencies: []
references:
  - TASK-1.126
  - TASK-1.97
type: task
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found twice in review: a doc comment left describing the pre-change shape of the code after the same diff changed it, in TASK-1.126 and TASK-1.97.

TASK-1.126: the round-2 fix removed the `run_start_column` parameter from `shape_and_check_run` (and from the function it calls), but the doc comment directly above `shape_and_check_run` still reads "`run_start_column` is where this run's own first cell sits on its row" - describing a parameter that no longer exists in the function's own signature.

TASK-1.97: a refactor renamed a test file to avoid an import collision, but `summary_from_command`'s own docstring still named the old, no-longer-existing file as "the other one."

Both were classed Nit under review.md's own wording-skip carve-out ("unless the wording would make a reader or a model do the wrong thing") - each is a false statement next to the exact code it describes, left behind by the same change that invalidated it, and each could lead a future reader or agent to misunderstand the current signature/structure or reintroduce a fixed collision.

Add to plugins/team/rules/testing.md or a new plugins/team/rules/docs.md: when a diff changes a function's signature, a file's name, or any other fact a nearby doc comment states as true, re-read every doc comment that names that fact in the same change and update it - a doc comment is not exempt just because the compiler does not check it, and a stale one is worse than none: it asserts something false right next to the code that contradicts it.
<!-- SECTION:DESCRIPTION:END -->
