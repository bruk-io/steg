---
id: DRAFT-2
title: 'Kernel: running, finished and failed for a command typed at a shell prompt'
status: Draft
assignee: []
created_date: '2026-11-15T21:43:00Z'
labels:
  - size-3
milestone: m-4
dependencies:
  - TASK-1.79
parent_task_id: TASK-1
project: bruk-io-steg
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Piece two of the split of TASK-1.79 (2026-11-15). doc-11: R50, R82, R83; decision-7. The worker's plan on TASK-1.79 proposes new kernel operations (note_command_output_started, note_command_ended) factored beside record_exit, since a shell's own program never exits between commands. Read that plan and TASK-1.41's notes first. No I/O and no unsafe in the kernel.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A shell pane shows running from when a prompt-marked command starts producing output until it ends, then finished or failed from its exit code (R50, R82), while the shell's own program keeps running; a command pane's status from its program's exit is unchanged
- [ ] #2 The ending rules of TASK-1.41 apply unchanged to a status that arrives this way: finished ends after 10 minutes or on input, failed on input or when the person has dealt with it, opening a tab ends nothing; time arrives as a Moment
- [ ] #3 A shell that reports no prompt marks shows no finished or failed status for typed commands and nothing is guessed; a command end with no exit code, an end with no start, and a start followed by another start each have a stated result and a test
<!-- AC:END -->
