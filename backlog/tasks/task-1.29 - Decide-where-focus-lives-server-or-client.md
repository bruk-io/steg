---
id: TASK-1.29
title: 'Decide where focus lives: server or client'
status: To Do
assignee: []
created_date: '2026-11-14T17:29:00Z'
updated_date: '2026-11-15T00:50:00Z'
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

- [ ] Is one focused tab per session acceptable for M1, or must each window keep its own? - Each winndow should keep its own
- [ ] Does 'unfocused' in R50 and R51 mean unfocused on the server, or not visible in any attached window? Unsure
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 doc-1 states which focus the server holds and which the app holds
- [ ] #2 doc-5 says how focus is carried in the snapshot on attach and in events
- [ ] #3 doc-2's 'current tab' and the focus commands are defined against that
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Decided 2026-11-15 (Bruk). (1) Focus is a path down one server's tree, held by that server: each session has a current tab and each tab a current pane, and focusing a pane sets every level above it. (2) A client is one connection to one server; it can know about every session there and views zero or more of them, with no limit in the protocol. (3) A window belongs to the app: it holds one client per server it uses and shows one session at a time; which session it shows is the app's, and cannot live on a server because there can be more than one server. (4) Focusing a pane or tab never changes what a client views or a window shows; 'session focus' is a request to a client. (5) Two windows on one session mirror each other; per-window focus is not provided. (6) 'In view' is the app's judgement; a server does not track visibility. Both open questions here are answered by (5) and (6). Written into doc-1 as R85 to R90 and the vocabulary, and into doc-2 as the client noun and the meaning of the three focus commands. Criterion 2 (doc-5) is not yet done.

Decided 2026-11-15 (Bruk): when more than one client views a session, each terminal takes the size it would have in the client most recently active in that session; other clients draw it at that size, anchored top left, padded or cropped, never rescaled. Split proportions stay shared. One rule for two windows in M1 and two machines in M3. Written into doc-1 as R91, and doc-1's 'two clients, one pane' decision is marked settled.
<!-- SECTION:NOTES:END -->
