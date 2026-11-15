---
id: TASK-1.79
title: >-
  Status for a command typed at a shell prompt: get its exit code out of the
  terminal engine
status: In Progress
assignee:
  - '@bruk.w1'
created_date: '2026-11-15T19:43:00Z'
updated_date: '2026-11-15T21:27:00Z'
labels:
  - size-5
milestone: m-4
dependencies:
  - TASK-1.41
parent_task_id: TASK-1
project: bruk-io-steg
asdlc_accepted_from: DRAFT-6
asdlc_branch: task-1.79-status-for-a-command-typed-at-a-shell
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Split from TASK-1.41 (2026-11-15). doc-11: R50, R82, R83; decision-7; doc-9 for prompt marks. TASK-1.41's worker read the exact dependency: ghostty-vt-sys 0.8 carries GHOSTTY_TERMINAL_OPT_SEMANTIC_PROMPT with has_exit_code and exit_code on the command-end step, but ghostty-vt 0.8's safe Terminal has no event for it, and its get/set are pub(crate). The server cannot supply the code either: the command is a child of the shell, not of steg. This is the status most of daily use depends on, since most commands are typed at a prompt, so it belongs in M1. Unknowns dominate the first half of this task; if the plan finds the route needs upstream work that cannot land in time, say so and propose the stopgap rather than building one quietly.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A command typed at a shell prompt that reports prompt marks (OSC 133) shows running while it runs and finished or failed from its exit code when it ends (R50, R82), in a shell pane whose shell never exits between commands
- [ ] #2 How the exit code leaves the engine is decided in the plan with the alternatives weighed: a change contributed to or carried against the ghostty-vt crate so its safe API surfaces the semantic-prompt callback; a small foreign-interface module outside the kernel over ghostty-vt-sys; or another route found by reading the engine. A second parser over the pty bytes kept in step with the engine is the last resort and needs the engineer's word
- [ ] #3 The kernel still has no unsafe code and no I/O; anything foreign lives in one small module behind a safe API in a crate allowed to hold it (doc-16, .claude/rules/rust.md), and any change to vendored or patched dependencies is recorded where TASK-1.34 recorded the build
- [ ] #4 A shell that reports no prompt marks shows no finished or failed status for typed commands and nothing is guessed; the ending rules of TASK-1.41 apply unchanged to statuses that arrive this way
<!-- AC:END -->
