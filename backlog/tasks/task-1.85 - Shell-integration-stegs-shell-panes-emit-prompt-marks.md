---
id: TASK-1.85
title: 'Shell integration: steg''s shell panes emit prompt marks'
status: To Do
assignee: []
created_date: '2026-11-15T21:43:00Z'
labels:
  - size-3
milestone: m-4
dependencies:
  - TASK-1.79
  - TASK-1.76
parent_task_id: TASK-1
project: bruk-io-steg
asdlc_accepted_from: DRAFT-4
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Piece five of the split of TASK-1.79 (2026-11-15). Whether steg sources shell integration automatically is the engineer's decision; the worker's plan on TASK-1.79 lays out what ghostty ships (zsh, bash, fish and elvish scripts; nushell's never emits OSC 133) and that its resources_dir lookup does file I/O. Without marks, the status for typed commands never shows, so this is what makes the feature work for a person who has configured nothing.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A shell pane started by steg in zsh, bash or fish emits OSC 133 marks with the exit code, by the means the engineer chose in this task's plan (sourcing ghostty's own integration scripts for that shell automatically, or documenting what a person adds to their shell configuration), and a setting or environment variable turns it off
- [ ] #2 Where the scripts come from, their licence, and how they are found at run time from the app bundle and from a development build are stated; reading them is done by the server or the steg program, never the kernel
- [ ] #3 A person's own shell configuration is not edited, and a shell steg does not know is started untouched
<!-- AC:END -->
