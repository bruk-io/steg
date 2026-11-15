---
id: DRAFT-3
title: >-
  Rule: a departure from the plan belongs in the task's own notes, not left for
  the reviewer to reconstruct from git history
status: Draft
assignee: []
created_date: '2026-11-15T18:04:00Z'
labels:
  - rule-change
dependencies: []
references:
  - TASK-1.40
  - TASK-1.33
type: task
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found twice in review: a real departure from the approved plan reached review only because the
reviewer went looking, not because the worker's own notes said what changed and why, in TASK-1.40
and TASK-1.33.

- TASK-1.33: the approved plan committed to updating plugins/bruk-io-steg/commands.toml's
  [check].needs and re-stamping its verified date "as part of this task's build." The diff left it
  untouched; the notes attributed the skip to "the engineer's instruction," but no such instruction
  was recorded anywhere on the task until the review found the gap and the engineer commented
  after the fact.
- TASK-1.40 (second review): merging origin/main pulled in another task's dispatch module, whose
  exhaustive match over the kernel's errors forced a real design decision not in the plan - which
  of an existing API error code to reuse for four new kernel error variants (and, separately, that
  choice was wrong for two of the four). The notes logged two other post-review rebases by name but
  said nothing about this one; the only sentence that touched the merged task described an earlier,
  pre-code state of it ("board-only commits, no code conflicts") and was never revisited once that
  task actually shipped code and forced a choice.

Add to plugins/team/rules/ (or review.md itself, since this is about what a worker records, not
what a reviewer checks): whenever a merge, a rebase, or any step outside the plan's own order of
work forces a decision the plan did not make - which file to touch, which existing name or code to
reuse, which case to handle how - say so in the task's own notes at the time, the same way a
deliberate in-plan refinement already gets named. A reviewer finding the departure in git history
that the worker's own notes never mention is the failure mode this should prevent, not just a
missing justification for one already visible in the diff.
<!-- SECTION:DESCRIPTION:END -->
