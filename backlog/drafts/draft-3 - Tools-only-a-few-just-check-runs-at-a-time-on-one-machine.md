---
id: DRAFT-3
title: 'Tools: only a few just check runs at a time on one machine'
status: Draft
assignee: []
created_date: '2026-11-18T02:09:00Z'
labels:
  - size-2
milestone: m-3
dependencies: []
parent_task_id: TASK-1
project: bruk-io-steg
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Agreed with the engineer on 2026-11-17. Several workers run just check on this machine at once, each with its own target directory, and the timing-sensitive server tests then fail for no reason of their own: on 2026-11-17 the login_env grandchild test failed in 4 of 6 runs with two workers checking, and a stuck-pane test and a terminal-scrolling test failed once each under the same load. This is the ceiling on how many workers can usefully run. tools/src/steg_tools/quiet.py already wraps every step and is the natural place. TASK-1.123 bounds the waits; this task lowers the load that makes them matter.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The heavy steps of just check and just test take a machine-wide slot before they run; at most two run at once by default, and an environment variable sets the number
- [ ] #2 A run that is waiting says so in one line, naming what it waits for, and waits on the lock itself with no polling or timer
- [ ] #3 A run that crashes or is killed frees its slot without anyone cleaning up, because the operating system holds the lock
- [ ] #4 CI is unaffected; the tools' own tests cover two runs contending for one slot, a freed slot being taken, and the variable, each with a recorded failing control
<!-- AC:END -->
