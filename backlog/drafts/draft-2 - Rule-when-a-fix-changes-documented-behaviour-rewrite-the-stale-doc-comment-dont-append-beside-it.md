---
id: DRAFT-2
title: >-
  Rule: when a fix changes documented behaviour, rewrite the stale doc comment,
  don't append beside it
status: Draft
assignee: []
created_date: '2026-11-22T06:39:00Z'
labels:
  - rule-change
dependencies: []
references:
  - TASK-1.150
  - TASK-1.126
type: task
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found twice in review: a doc comment that states how a function behaves is left untouched when the behaviour changes, instead of being rewritten, in TASK-1.150 and TASK-1.126.

TASK-1.126: shape_and_check_run's doc comment kept describing a run_start_column parameter the same round's fix had removed from the function entirely - flagged a Nit, never fixed, task still merged clean.

TASK-1.150: the plan named spawn_hostile_listener's doc comment by exact line range (40-49) and said to rewrite it to describe the new, fixed drop order. The worker instead appended a second paragraph below it describing the new order and explicitly calling it "the reverse of this function's own earlier order" - but left the original paragraph verbatim, so it still states the old order in the present tense as the function's current behaviour. The two paragraphs now contradict each other on the one function whose drop order is the entire fix; a reader or a model trusting the first paragraph alone would restore the pre-fix, buggy order.

Add to plugins/team/rules/code-comments.md (new file) or another team rule file a reviewer-facing rule: when a change alters what a function does, find every doc comment that describes the old behaviour and rewrite it in place - don't leave the old paragraph standing and add a new one after it, even when the new one is accurate and even when it says it corrects or reverses the old one. A doc comment that still asserts old behaviour as present fact, anywhere in the same comment block, is the same defect as never updating it, because a future reader or an LLM can read only the first paragraph and act on it.
<!-- SECTION:DESCRIPTION:END -->
