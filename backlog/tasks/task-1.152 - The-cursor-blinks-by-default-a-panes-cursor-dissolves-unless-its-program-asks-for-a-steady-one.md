---
id: TASK-1.152
title: >-
  The cursor blinks by default: a pane's cursor dissolves unless its program
  asks for a steady one
status: To Do
assignee: []
created_date: '2026-11-22T04:53:00Z'
labels:
  - size-2
milestone: m-4
dependencies:
  - TASK-1.127
parent_task_id: TASK-1
project: bruk-io-steg
asdlc_accepted_from: DRAFT-2
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Decided by the engineer on 22 November 2026 after the live run of TASK-1.127 (steg PR 95): the dissolve should be on by default. TASK-1.127 as accepted runs it only when the program has asked for a blinking cursor, so in an ordinary shell nothing moves until a sequence such as CSI 1 SP q is sent. This changes the starting state only: what a pane's cursor is before its program says anything. The plan finds where that default lives (the terminal state in the server, or the app's reading of it) and says so; it must not make the app ignore a program's explicit request for a steady cursor. TASK-1.144 (the setting for which animation, including none) builds on this default and is where a user turns it off.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 In a new pane whose program has said nothing about the cursor, the block cursor dissolves and re-forms while it waits for a key, with no escape sequence typed first
- [ ] #2 A program that asks for a steady cursor gets a steady one, and one that asks for blinking gets the dissolve; a test covers each of the three cases, with a recorded control for the default
- [ ] #3 Every bound of TASK-1.127 still holds: nothing wakes the app when the window is not focused, the cursor is hidden, Reduce Motion is on, or the idle time has passed
<!-- AC:END -->
