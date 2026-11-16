---
id: TASK-1.109
title: >-
  Server: events for layout, for a session or tab changing, for clients, and the
  one addressed to a single client
status: To Do
assignee: []
created_date: '2026-11-16T04:32:00Z'
labels:
  - size-3
milestone: m-3
dependencies:
  - TASK-1.46
  - TASK-1.89
parent_task_id: TASK-1
project: bruk-io-steg
asdlc_accepted_from: DRAFT-1
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Split from TASK-1.46 on 2026-11-15. TASK-1.46's plan (its sections 2 and 4) has the table of what the description lacks and where each event would be produced, and proposes names: tab.layout_changed; session.changed and tab.changed; client.connected, client.left, client.started_viewing, client.stopped_viewing; and session.focus_requested for the addressed one. doc-14, decision-2 section 3, decision-5, doc-8 R38 and R52. Owns the session focus method that dispatch still answers as not implemented (TASK-1.89 does not build it).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A change to a tab's layout (a split, a close, a move, a swap, a fill, a resize) is one event carrying the tab's layout whole; a change to a session's or a tab's own fields (label, position, colour) is an event carrying its record whole. Each is caused by every dispatch path that makes that change, found by reading and listed in the plan one by one
- [ ] #2 Clients connecting, leaving, and starting or stopping viewing a session are events, with the fields doc-14 gives for a client
- [ ] #3 session focus is built: it asks one client to show a session, as an event delivered to that client alone and to no other, never part of a subscription and never filtered; decision-5 says what focus means without a window, and the plan says what the server does when the client named is not connected
- [ ] #4 The API description gains these events under names the engineer has confirmed, the completeness tests cover them, and the event watch filter accepts them; doc-2 and doc-14 are brought up to date through the board where they list events
<!-- AC:END -->
