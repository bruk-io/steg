---
id: decision-6
title: The latest client sets a terminal's size
date: '2026-11-15T01:11:00Z'
status: accepted
---
## Context

A terminal has one size in rows and columns, because the program in it is told one size. The spec allowed more than one window in the first release and put two clients on one session in the third, and left undecided which size wins when they differ. Under decision-5, two windows on one session are already two clients viewing it. Bruk raised the case of a client on another machine viewing the same session, which showed that stopping one app from opening two windows on a session would only postpone the question.

## Decision

Decided by Bruk on 2026-11-15: the latest client wins. When more than one client views a session, each terminal takes the size it would have in the client most recently active in that session, meaning the last one to send input or be resized. Every other client draws the terminal at that size, anchored top left, padded where its own pane is larger and cropped where it is smaller. Text is never rescaled. The proportions of the pane layout stay shared by all clients.

## Consequences

- One rule serves two windows on one machine in the first release and two machines in the third.
- The server has to remember which client was last active in each session.
- The other candidates were the smallest client winning, which shrinks the terminal on the machine being used when a smaller one attaches, and the largest winning, which can crop the client being used.
- That "active" includes resizing a window, and not only typing, was the orchestrator's wording and is Bruk's to change.
