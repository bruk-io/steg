---
id: DRAFT-1
title: 'Server: tab list, tab show, pane list, and the status a pane and a tab show'
status: Draft
assignee: []
created_date: '2026-11-16T00:33:00Z'
labels:
  - size-3
milestone: m-3
dependencies:
  - TASK-1.45
  - TASK-1.84
parent_task_id: TASK-1
project: bruk-io-steg
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found on 2026-11-15 from TASK-1.84's plan: no task owned the list and show methods of tab and pane beyond pane show, nor putting doc-11's status on the wire. dispatch/tab.rs answers tab list and tab show with NotImplementedYet citing TASK-1.39 and TASK-1.41, both Done; dispatch/pane.rs the same for pane list (TASK-1.76) and pane resize (TASK-1.42). The CLI (TASK-1.49) and the app's bar (TASK-1.54) both need them. doc-2 section 8, doc-11 R50, R82, R83, decision-7. The worker of TASK-1.84 recommended a second field on the pane record over changing the meaning of the first. If the plan finds the leftover methods are a separate change from the records, split them.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 tab list, tab show and pane list answer over the API with the records doc-2 section 8 gives, built from the kernel's tree: title, layout, active pane, size where known, and for a tab the status it shows (decision-7's ranking, already in the kernel)
- [ ] #2 A pane's record carries what steg knows about its program right now (doc-11: running, finished, failed with its exit code, or nothing for a shell idle at its prompt) as a field of its own beside the existing running-or-ended fact, in the sdk's Status type; the server reads its clock once per request and passes it to dispatch as a Moment, and the kernel reads none. The plan says whether the existing field keeps its name and meaning, for the engineer
- [ ] #3 The remaining session, tab and pane methods that dispatch still answers NotImplementedYet for are each either built here or named with the task that owns them: pane resize (the kernel's Tab::size exists since TASK-1.42), session focus, and tab move to another session (the kernel has no such operation). None is left pointing at a task that is Done
- [ ] #4 The description's fields for these records match what is returned, shown by the completeness test; each method has a test through Runtime::handle, and a typed command's status reaches pane show and tab show in a real-pty test
<!-- AC:END -->
