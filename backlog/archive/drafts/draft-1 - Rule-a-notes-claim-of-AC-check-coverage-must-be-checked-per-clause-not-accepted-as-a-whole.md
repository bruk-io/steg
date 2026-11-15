---
id: DRAFT-1
title: >-
  Rule: a notes claim of AC/check coverage must be checked per clause, not
  accepted as a whole
status: Draft
assignee: []
created_date: '2026-11-15T03:19:00Z'
labels:
  - rule-change
dependencies: []
references:
  - TASK-1.35
  - TASK-1.33
type: task
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found twice in review: a task's implementation notes claim a verification or test-coverage is complete, but a specific clause of it is not actually backed by anything recorded or present, in TASK-1.35 and TASK-1.33.

TASK-1.33 (bruk.r2's clean re-review): "The plan's file-by-file section for views/steg/packages.c4 says it was 'Verified with `npx likec4 start` during the build'... No recorded confirmation of this manual render check appears in the notes." (Its two earlier Important findings, now fixed, were the same shape at higher stakes: a proof step and an AC's required evidence listing were both claimed done with nothing recorded to show it.)

TASK-1.35 (this review): the notes say "22 kernel unit tests... cover every acceptance criterion: creation/labelling/reordering/closing of sessions and tabs (AC #1)... colour assignment... (AC #3...)". AC #3 is "A new session takes the next of the five colours not in use, and its colour can be changed" - the "colour can be changed" clause (`Session::set_colour`) has no test anywhere in the diff, and AC #1's "labelled" clause has no test of `Session::set_label` either (only `Pane::set_label` is tested directly, and `Tab`'s label only indirectly via `Tree::new_tab`). The blanket claim reads as if every clause of every cited AC is covered, and it isn't.

Add to plugins/team/rules/testing.md: a notes claim that tests cover an acceptance criterion must be checked clause by clause, not accepted as a whole - an AC with two verbs ("created... and reordered", "takes the next colour... and can be changed") needs a test for each verb, and a reviewer should grep for the mutator/method named by the untested clause before trusting a summary line that names the AC number alone.
<!-- SECTION:DESCRIPTION:END -->
