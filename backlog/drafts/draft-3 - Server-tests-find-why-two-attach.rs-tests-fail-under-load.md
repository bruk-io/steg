---
id: DRAFT-3
title: 'Server tests: find why two attach.rs tests fail under load'
status: Draft
assignee: []
created_date: '2026-11-22T20:04:00Z'
labels:
  - size-2
milestone: m-3
dependencies: []
parent_task_id: TASK-1
project: bruk-io-steg
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Split out of TASK-1.149 by its plan (22 November 2026). TASK-1.149's notes hold the two sightings; neither traces to TASK-1.149's mechanism (tests in one binary racing an fd count). Cause-finding first, no fix assumed. Both were seen while three heavy runs overlapped on one machine, so say plainly whether the answer is contention on the machine, a missing lock, or a real fault in the server.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The cause of each sighting is named with evidence, or shown not reproducible with the method stated: a_declared_size_reaches_the_pty_and_is_reported_on_every_attached_stream (holds neither flood lock) and an_unresponsive_panes_own_commands_do_not_stall_the_connection_or_another_pane (holds both and still failed once under three overlapping heavy runs)
- [ ] #2 Each cause is either fixed here with a control that fails without the fix, or drafted as its own task if it is a product fault or a larger change
- [ ] #3 No bound is lengthened and no retry added to make a test pass without the cause being named
<!-- AC:END -->
