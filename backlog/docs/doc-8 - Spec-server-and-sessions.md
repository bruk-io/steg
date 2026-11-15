---
id: doc-8
title: 'Spec: server and sessions'
type: specification
created_date: '2026-11-15T01:13:00Z'
updated_date: '2026-11-15T01:15:00Z'
---
- Intent: TASK-1, "A terminal like Rex, built in Rust and GPUI and extendable through Python".
- One of steg's specs. doc-1 is the overview: it has the vocabulary, the release table and the list of specs. Requirement numbers are steg-wide and never reused, so R19 means the same thing in every document.
- Rests on: decision-1 (the API description), decision-5 (focus, clients and windows), decision-6 (terminal size). The decision records are in pull request 5 until it merges.
- Status: draft. Reorganised from the single spec on 2026-11-15; every requirement here was moved without rewording, except that references to documents since retired now point at the decision records and at doc-10.

## Scope

The server and what it owns: sessions, tabs and panes, their layout, focus, the clients connected to it, and the API every client uses. It does not cover what travels on the wire (doc-14), what a terminal does (doc-9), or the CLI's own rules (doc-2).

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
- R7 (M3). Two clients can attach to the same session at once, and each sees the other's
  changes as they happen.
- R85 (M1). Focus is a path down a server's tree, held by that server: each session has a
  current tab, and each tab has a current pane. Focusing a pane makes its tab the current
  tab of its session as well. There is no focus across servers.
- R86 (M1). A client is one connection to one server. Every client can learn of every
  session on that server and of changes to them. A client views zero or more sessions,
  which is what brings it their terminals' contents; the protocol sets no limit on how
  many.
- R87 (M1). A window belongs to the app, not to any server. It holds one client for each
  server it uses and shows one session at a time. Which session a window shows is held by
  the app. In M1 there is one server, and the app views exactly the session each window
  shows.
- R88 (M1). Focusing a pane or a tab never changes which sessions a client views or which
  session a window shows. Only the person at that window, or a command addressed to that
  client, does.
- R89 (M1). Two windows showing the same session show the same current tab, the same
  current pane and the same pane layout. Per-window focus is not provided.
- R90 (M1). A pane is in view when it is in the current tab of a session that a window is
  showing. That is the app's judgement from R85 and R87; a server does not track what is
  visible. R50's ring and bar, and R51's notifications, are decided by the app on this
  basis.
- R91 (M1). A terminal has one size. When more than one client views a session, each
  terminal in it takes the size it would have in the client most recently active in that
  session, meaning the last one to send input to it or be resized. Every other client
  draws the terminal at that size, anchored at its top left: with empty ground beyond it
  where that client's pane is larger, and cropped where it is smaller. Text is never
  rescaled. The pane layout's proportions are shared by all clients (R89); only the count
  of rows and columns follows the latest. This holds for two windows on one machine in M1
  and for clients on different machines in M3 alike.

### Layout

- R8 (M1). A session holds ordered tabs. A tab holds panes arranged in nested horizontal
  and vertical splits.
- R9 (M1). Panes can be split, closed, focused by direction, resized, swapped, moved to
  another tab, and zoomed to fill the tab and back.
- R10 (M1). Sessions, tabs, and panes can carry labels. Each is addressable by full ID,
  label, position, or an ID suffix of four or more characters. An ambiguous target is an
  error, never a guess.
- R11 (M3). A pane can float above a tab's splits, be moved and resized, and be minimised
  and restored.
- R12 (M1). The pane model allows kinds other than terminals, even though v1 ships only
  the terminal kind.
- R78 (M1). Where a split comes from, where a closed pane's space goes, the smallest a
  pane may be, and what happens when the window is resized follow the design artifact (doc-10).

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

- [ ] Server lifecycle on macOS: spawned by the first client or managed by launchd, and when an idle server stops.
- [ ] The smallest pane the server allows, in rows and columns. The app's design gives a smallest pane in pixels, which only the app knows; the server needs a floor of its own so the CLI cannot split a pane to nothing.
- [ ] Which crate holds the API description. Both the server and the CLI must be able to depend on it (decision-1).

## Out of scope

- Surviving a server restart or a reboot (doc-1, out of scope).
- Remote servers (doc-13).
