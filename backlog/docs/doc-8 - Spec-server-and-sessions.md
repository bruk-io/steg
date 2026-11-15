---
id: doc-8
title: 'Spec: server and sessions'
type: specification
created_date: '2026-11-15T01:13:00Z'
updated_date: '2026-11-15T02:34:00Z'
---
- Intent: TASK-1, "A terminal like Rex, built in Rust and GPUI and extendable through Python".
- One of steg's specs. doc-1 is the overview: it has the vocabulary, the release table and the list of specs. Requirement numbers are steg-wide and never reused, so R19 means the same thing in every document.
- Rests on: decision-1 (the API description).
- Status: draft. R99 was added on 2026-11-15, when Bruk decided the server's lifecycle. How the server's code is kept apart from its clients, and the sdk, are doc-16 (R98, R101). Reorganised from the single spec on 2026-11-15; every requirement here was moved without rewording, except that references to documents since retired now point at the decision records and at doc-10.

## Scope

The server as a running thing: that it owns everything, how it starts and who can reach it, and the API every client uses. What the server holds, meaning sessions, tabs, panes, their layout, focus and clients, is the domain (doc-15). It does not cover what travels on the wire (doc-14), what a terminal does (doc-9), or the CLI's own rules (doc-2).

## Requirements

### Sessions and the server

- R1 (M1). A server process owns every session, tab, pane, and the programs running in
  them.
- R2 (M1). Quitting or crashing the app leaves every program running. Reopening the app
  restores each session's layout, screen contents, and scrollback.
- R3 (M1). The server starts on demand the first time the app or CLI needs it. It can
  also be started explicitly, in which case it stays up with no clients attached.
- R4 (M1). The server runs on macOS as the user's own account, with no root.
- R5 (M1). Local clients connect over a socket only the owning user can open. The server
  opens no network port.
- R6 (M1). Sessions can be created, labelled, reordered, listed, and destroyed.
- R99 (M1). A server started on demand detaches from the client that started it, takes
  its environment from a login shell and not from that client, and stops by itself
  after 30 seconds with no sessions and no clients. A server started explicitly stops
  only when told to. A server that has a session never stops by itself. Two clients
  that start a server at the same moment end up on the same one. All of this is the
  server's own code: a client runs the program and waits for the socket.
- R7 (M3). Two clients can attach to the same session at once, and each sees the other's
  changes as they happen.

### API

- R36 (M1). One documented API serves the app, the CLI, and Python. The app has no
  private channel.
- R37 (M1). The API describes itself: a client can ask for every method, its parameters,
  and its documentation.
- R38 (M1). Clients can subscribe to events: session, tab, and pane created, closed, and
  focused; terminal working directory, title, bell, exit, and program status changed.
- R39 (M1). The protocol is versioned. A mismatched client and server either work
  together or fail with a message naming both versions.

## Open questions

None. The server's lifecycle is R99: started by the first client, not by launchd. The API description lives in the sdk (doc-16, R101).

## Out of scope

- Surviving a server restart or a reboot (doc-1, out of scope).
- Remote servers (doc-13).
