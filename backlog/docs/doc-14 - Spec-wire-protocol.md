---
id: doc-14
title: 'Spec: wire protocol'
type: specification
created_date: '2026-11-15T01:13:00Z'
updated_date: '2026-11-15T01:23:00Z'
---
- Intent: TASK-1, "A terminal like Rex, built in Rust and GPUI and extendable through Python".
- One of steg's specs. doc-1 is the overview: it has the vocabulary, the release table and the list of specs. Requirement numbers are steg-wide and never reused, so R19 means the same thing in every document.
- Rests on: decision-2 (framing, streams, snapshots and updates, the version handshake), decision-5 (focus and clients), decision-6 (terminal size), decision-7 (status). The decision records are in pull request 5 until it merges.
- Status: draft. Reorganised from the single spec on 2026-11-15; every requirement here was moved without rewording, except that references to documents since retired now point at the decision records and at doc-10.

## Scope

What travels between the server and a client. The frame format, the streams, the attach sequence, the snapshot and the updates that follow it, back-pressure and the version handshake are decided and measured in decision-2 and are not restated here. This spec adds what the protocol carries beyond terminals: the tree, focus, clients, sizes, status and layout. Nothing in it was measured, and it adds no kind of frame. Method and event names are placeholders; the real ones come from the API description (decision-1, R38).

## Requirements

The requirements the protocol serves are R1, R2, R36 and R39 in doc-8, and R83 to R91.
### Two ways a client relates to a session

doc-1 R86 says a client knows about every session on its server and views zero or more of them.
On the wire those are two different things, and the attachments of decision-2 are the second.

- **Knowing about sessions** is a subscription on stream 0. A client asks for the tree and then
  receives events as it changes. This is what the app's bar and palette are drawn from, and it
  costs no terminal traffic.
- **Viewing** is holding attachments (decision-2, section 3) to panes. A client views a session when it is
  attached to at least one of that session's panes. In M1 the app attaches to the panes of the tab
  it is showing and to nothing else, so a tab that is not on screen streams nothing.

So "which sessions does this client view" is never sent by the client; the server reads it off
the attachments.

### The tree, and the focus path in it

The tree snapshot, returned by one request and kept current by events:

```
session  id, label, position, colour, current_tab
tab      id, label, title, position, current_pane, layout, shown_status
pane     id, label, title, cwd, program, exit_code, status
```

- **Focus is two fields:** a session's `current_tab` and a tab's `current_pane` (R85). Nothing
  else on the wire says "focused".
- A request to focus a pane sets the pane's tab's `current_pane` and, if that tab was not
  current, its session's `current_tab`, in one step on the server. One event reports the whole
  path, `{session, tab, pane}`, so no client ever sees the two halves apart.
- Focusing never touches attachments (R88).

### Clients

- The hello (decision-2, section 7) gains two keys, which is allowed because unknown keys are ignored: `kind`,
  one of `app`, `cli` or `plugin`, and an optional `label`. The server's reply gains `client`, the
  identifier it has given this connection.
- `client.list` returns each client's identifier, kind, label, the sessions it views (from its
  attachments), and when it was last active. Events report a client connecting, leaving, and
  starting or stopping viewing a session.
- **Asking a client to show a session** (doc-2, `session focus`) is the one message addressed to a
  single client. It is an event on stream 0 delivered only to that client, carrying the session.
  The server does not wait for it to be obeyed: which session a window shows is the app's (R87).
  The caller's request succeeds once the event is queued.

### The size of a terminal

The spike left this open: sizes are carried per attachment, and which wins was undecided. R91
decides it.

- Each attachment still declares its own size, at attach and whenever the client's pane changes
  size. That is the size the terminal would have in that client.
- The server remembers, per session, the **latest client**: the last one to send input on an
  attachment in that session, or to declare a new size on one.
- A terminal takes the size the latest client has declared for it. If the latest client has no
  attachment to that pane, its size does not change.
- When a terminal's size changes, every attachment to it is told. The snapshot already carries a
  size (decision-2, section 3); an update gains an optional `size` for the same purpose. A client draws at
  that size, top left, padded or cropped (R91).

### Status

- A pane's `status` is one record or nothing: `kind`, `progress` where known, `since`, and
  `ends_at` where the kind has a lifetime (R83). In M1 the kinds are running, finished and failed,
  set by the server from the program's exit status (R82). M2 adds what the status protocol
  reports.
- **The server owns the lifetimes.** It clears a finished status after 10 minutes and a bell after
  30 seconds, and it clears finished, failed and the bell when input arrives for that pane. A
  click is not input, so the app sends one small request saying the person has dealt with that
  pane. Opening a tab sends nothing and clears nothing.
- **The server chooses what a tab shows.** A tab's `shown_status` names the pane whose status the
  tab's ring stands for, by the order in the design artifact (doc-10). It is computed once, on the server, so the
  app, the CLI and a plugin cannot disagree. Promotion (R84, M2) is the server choosing
  differently; no client needs to know why.
- Status changes are events, each carrying the pane and its new record or nothing.

### Pane layout

- A tab's `layout` is a tree: a node is either a pane, or a split with a direction, a ratio
  between 0 and 1, and two children. A tab also has `zoomed`, a pane or nothing, for fill.
- Ratios, not sizes, are what the server holds, which is why every client shares the proportions
  (R89) while each has its own pixels.
- Splitting, closing, resizing, swapping and filling are requests that change this tree, and each
  is reported as one event carrying the tab's new layout whole. A layout is small, and sending it
  whole means a client never has to apply a change to a tree it might have wrong.

## Open questions

- [ ] The smallest pane in rows and columns, so the CLI cannot split a pane to nothing (also in doc-8).
- [ ] Whether telling the server a person has dealt with a pane is its own request or a flag on focusing it. This spec assumes its own.
- [ ] An event addressed to one client is new. Every other event goes to every subscriber.
- [ ] A real SSH channel has not been tested, only a relay over two pipes.

## Out of scope

- Raw terminal bytes replayed by an emulator in the client. Measured and set aside in decision-2.
- A second data connection per pane. Not needed on the evidence in decision-2.
