---
id: DRAFT-12
title: 'Server: events that clients subscribe to'
status: Draft
assignee: []
created_date: '2026-11-13T22:52:00Z'
updated_date: '2026-11-14T00:05:00Z'
labels:
  - size-3
milestone: m-0
dependencies:
  - DRAFT-11
  - DRAFT-9
parent_task_id: DRAFT-1
type: task
project: bruk-io-steg
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
R38: session, tab and pane created, closed and focused; terminal working directory, title, bell and exit changed. R16: the terminal's facts are reported as events. Program status events come with OSC 7501 in the second release.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A subscribed client receives each event in R38, apart from program status, in a test through the socket
- [ ] #2 A client that subscribes to one session receives nothing from another
<!-- AC:END -->
