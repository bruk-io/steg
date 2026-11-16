---
id: TASK-1.121
title: 'App: typing and resizing in the bare window'
status: In Progress
assignee:
  - '@bruk.w6'
created_date: '2026-11-16T15:26:00Z'
updated_date: '2026-11-16T16:48:00Z'
labels:
  - size-2
milestone: m-4
dependencies:
  - TASK-1.117
  - TASK-1.114
parent_task_id: TASK-1
project: bruk-io-steg
asdlc_accepted_from: DRAFT-2
asdlc_branch: task-1.121-app-typing-and-resizing-in-the-bare
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Split from TASK-1.117 by its plan on 2026-11-16, under the engineer's standing decision to split by default: the window first shows a live terminal read-only (TASK-1.117), then this adds input and size. doc-9; doc-10 R20, R22; decision-2 section 3. Needs TASK-1.114 (input on the attachment's stream; tab declare-size; the size on an update). TASK-1.117's plan holds the worked design for the key mapping, the input handler and the resize. Left out, for TASK-1.56 or later: application cursor mode, bracketed paste, the mouse, the Kitty keyboard protocol; so full-screen programs are not fully usable yet. Carries TASK-1.117's original second and third criteria and the recording half of its sixth.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Typing reaches the program and its output is drawn: a shell prompt accepts a command and shows the result; keys that are text, Enter, Backspace, Tab, Escape, the arrows and Control combinations arrive as the bytes a terminal sends, from a pure mapping with a table and tests; text composed by the input method arrives as text
- [ ] #2 The window declares its size on opening and when resized, coalesced so a drag does not flood the server, and draws the terminal at the size the server reports, filling the window
- [ ] #3 Closing the window leaves the session running on the server, and running the app again shows the same screen, including what was typed before
- [ ] #4 The pull request carries a short recording of typing a command and resizing; what can be tested without a window has tests with recorded controls; nothing in just check or CI opens a window
<!-- AC:END -->
