---
id: DRAFT-3
title: 'Decide where focus lives: server or client'
status: Draft
assignee: []
created_date: '2026-11-14T17:29:00Z'
labels:
  - size-1
milestone: m-0
dependencies: []
parent_task_id: TASK-1
project: bruk-io-steg
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
## Problem

The spec and CLI spec rely on a 'current' or 'focused' session, tab and pane, but nothing says who holds it when more than one client or window is attached.

- doc-2 has `session focus`, `tab focus` and `pane focus`, and resolves partial targets against 'the current tab'.
- doc-1 lists 'focused' among the events (R38), shows status only for unfocused panes (R50), notifies only for unfocused panes (R51), and restores where the person was (R2) while the app holds no session state.
- doc-1 R20 allows more than one window. doc-5 (wire protocol) does not mention focus.

## Proposal to confirm or change

- Focused pane within a tab: held by the server, part of the tab's layout.
- Focused tab within a session: held by the server.
- Which session a window shows: held by the app, per window.
- Consequence: two windows on the same session show the same tab. A per-client override is deferred to M3 (R7, two clients on one session).

## Open questions

- [ ] Is one focused tab per session acceptable for M1, or must each window keep its own?
- [ ] Does 'unfocused' in R50 and R51 mean unfocused on the server, or not visible in any attached window?
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 doc-1 states which focus the server holds and which the app holds
- [ ] #2 doc-5 says how focus is carried in the snapshot on attach and in events
- [ ] #3 doc-2's 'current tab' and the focus commands are defined against that
<!-- AC:END -->
