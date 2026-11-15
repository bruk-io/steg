---
id: decision-5
title: Focus is a path in the server's tree, and a window belongs to the app
date: '2026-11-15T01:11:00Z'
status: accepted
---
## Context

The specs used "focus" and "current" for three different things without saying who holds each, and allowed more than one window without saying what a window is. Bruk and the orchestrator worked it through on 2026-11-15; the model was corrected three times in that conversation, each time because something true of the first release had been written as if it were how the system works. Rex's documentation was read for hints: its focus commands name no client, and what it asks of the app is addressed to one.

## Decision

1. Focus is a path down one server's tree, held by that server: each session has a current tab, and each tab has a current pane. Focusing a pane sets every level above it.
2. A client is one connection to one server. It can know about every session there and views zero or more of them; the protocol sets no limit.
3. A window belongs to the app. It holds one client for each server it uses and shows one session at a time, from any of them. Which session a window shows is the app's, and cannot live on a server, because there can be more than one server.
4. Focusing a pane or a tab never changes what a client views or what a window shows. Asking a window to show a session is a request addressed to a client.
5. Two windows on one session show the same current tab, current pane and layout. Per-window focus is not provided.
6. Whether a pane is in view is the app's judgement from the above. A server does not track what is visible.

## Consequences

- The CLI gains a `client` noun, and `session focus` becomes a request to a client; `pane focus` and `tab focus` set the path.
- There is no `STEG_CLIENT` in a terminal's environment: a pane is not tied to one client.
- The first release has one server, so each window has one client, but the specs must not say a window is a client or that a client views one session.
- Viewing a session means being attached to its panes, which is where the size question in decision-6 comes from.
