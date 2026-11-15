---
id: doc-13
title: 'Spec: remote hosts and distribution'
type: specification
created_date: '2026-11-15T01:13:00Z'
updated_date: '2026-11-15T01:24:00Z'
---
- Intent: TASK-1, "A terminal like Rex, built in Rust and GPUI and extendable through Python".
- One of steg's specs. doc-1 is the overview: it has the vocabulary, the release table and the list of specs. Requirement numbers are steg-wide and never reused, so R19 means the same thing in every document.
- Rests on: decision-2 (the wire protocol was tested through a relay, not through a real SSH channel).
- Status: draft. Reorganised from the single spec on 2026-11-15; every requirement here was moved without rewording, except that references to documents since retired now point at the decision records and at doc-10.

## Scope

Reaching a steg server on another machine, and what has to be true before steg is given to anyone else. Remote hosts are M3.

## Requirements

### Remote hosts

- R55 (M3). The app and CLI connect to a server on another host over SSH, using the
  user's existing SSH config, keys, and agent. The remote host needs only the `steg`
  binary and no listening port.
- R56 (M3). Hosts are saved under a label. The app switches between hosts, and the CLI
  can target a host by its label.
- R57 (M3). When the connection drops, the remote sessions carry on. The app reconnects
  by itself and restores them.

### Ready to distribute later

These cost little now and a great deal to retrofit.

- R58 (M1). No telemetry, no account, no licence check, and no network call the user did
  not ask for.
- R59 (M1). MIT licensed, with no code taken from GPL sources. Zed's terminal crates are
  GPL, so the GPUI terminal view is written fresh. The licence check allows permissive
  licences, CDLA-Permissive-2.0 among them, and refuses GPL and AGPL.
- R60 (M1). Config, state, and sockets follow XDG paths. Nothing is
  tied to one person's machine.
- R61 (M1). The server and CLI ship as one binary with no runtime dependency beyond the
  operating system.
- R62 (M2). Reference docs for the CLI and the API are generated
  from the API description.

## Open questions

- [ ] Remote sessions being M3 is an assumption to confirm (intent question 4).
- [ ] Saved hosts in the CLI: whether `host` gets `add` and `remove` of its own (doc-2).
- [ ] Extensions on a remote host need uv and Python there. R47 keeps the server itself free of that dependency.

## Out of scope

- Port forwarding. SSH is the only remote transport.
