---
id: DRAFT-13
title: >-
  Server: attach to a terminal, stream it, and restore it after the client goes
  away
status: Draft
assignee: []
created_date: '2026-11-13T22:52:00Z'
updated_date: '2026-11-14T00:05:00Z'
labels:
  - size-5
milestone: m-0
dependencies:
  - DRAFT-10
  - DRAFT-9
parent_task_id: DRAFT-1
type: task
project: bruk-io-steg
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
R2: quitting or crashing the app leaves every program running, and reopening restores each session's layout, screen contents and scrollback. Carries terminal output to a client and input back, the way the wire protocol spike decided.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A test kills a client while a program is printing, attaches a new one, and finds the same screen and scrollback (R2)
- [ ] #2 Input typed in one attached client reaches the program
- [ ] #3 A resized client resizes the terminal
<!-- AC:END -->
