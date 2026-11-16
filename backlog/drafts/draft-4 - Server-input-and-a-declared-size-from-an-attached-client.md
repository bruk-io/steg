---
id: DRAFT-4
title: 'Server: input and a declared size from an attached client'
status: Draft
assignee: []
created_date: '2026-11-16T08:39:00Z'
labels:
  - size-3
milestone: m-3
dependencies:
  - TASK-1.47
parent_task_id: TASK-1
project: bruk-io-steg
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Split from TASK-1.47 by its plan (2026-11-16). doc-15 R91, doc-9, decision-2 sections 3 and 6. Covers TASK-1.47's fourth acceptance criterion. Not pane send (TASK-1.90, a one-shot request) and not pane resize (TASK-1.110, moving a layout border). The kernel's side exists (Tree::declare_size, view_pane, note_input; Runtime::apply_size_changed, from TASK-1.42 and TASK-1.76); this task calls them from the attachment's paths.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Bytes a client sends on the stream it attached with reach the pane's program, in order, and count as that client's input for the size rule (R91)
- [ ] #2 An attachment declares a size; the terminal takes the size the kernel's rule gives, and an update on every attachment's stream reports the terminal's size when it changes
- [ ] #3 Two clients attached with different sizes: the result is the kernel's rule and each is told the size in force; a client that detaches or dies releases its claim
- [ ] #4 Offered by the steg library, and the tests go through it over a real socket with a real pty (R101)
<!-- AC:END -->
