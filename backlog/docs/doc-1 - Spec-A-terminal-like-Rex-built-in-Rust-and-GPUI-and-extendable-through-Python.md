---
id: doc-1
title: >-
  Spec: A terminal like Rex, built in Rust and GPUI and extendable through
  Python
type: specification
created_date: '2026-11-13T22:34:00Z'
updated_date: '2026-11-15T00:50:00Z'
---
- Intent: "A terminal like Rex, built in Rust and GPUI and extendable through Python" (DRAFT-1 until accepted)
- Status: draft. Nothing here is approved yet.
- Revised 2026-11-15 for the app's design (doc-7) and the four decisions Bruk made that day: program status is split across M1 and M2; session colour follows the design; settings are overrides and the Python config is never written to; each status ends by its own rule. The changes are R21, R26, R50 and R51 reworded, R78 to R84 added, and the release table.
- Revised again 2026-11-15 for where focus lives (TASK-1.29): the vocabulary gains window and focus, the client is defined more exactly, and R85 to R91 are added. R91 settles the size a terminal takes when more than one client views it.
- Reference product: [Rex](https://www.superlogical.com/rex/docs) by Superlogical.

## 1. What steg is

Steg is a terminal workspace where the work outlives the window. A server owns the
sessions and the programs in them. A desktop app and a CLI are clients of that server,
and Python extends all three.

Superlogical describes the idea as "a durable session around the work itself: one that
can span applications and environments, provide relevant context by default, expose
structured data and actions, preserve history, and be driven by software while remaining
visible and controllable by people." Steg takes that sentence as its brief, scoped to one
person's machines.

### How steg differs from Rex

| | Rex | Steg |
|---|---|---|
| Extension language | Lua 5.1 | Python 3.14+ |
| Desktop UI | Native Mac and iOS apps | Rust with GPUI, macOS first |
| Audience | Teams, multi-user servers | One person, distributable later |
| Program status | OSC 7501 | The same protocol, unchanged |
| Liquid Glass | Yes | No |

## 2. Who it is for

- **Now:** Bruk, on a Mac, with sessions on that Mac. Other Macs reached over SSH come
  in a later release.
- **Coding agents:** Claude Code and similar tools running inside steg terminals, driving
  the workspace through the CLI.
- **Later:** other developers who install a release build. Nothing in v1 is built for
  them, but nothing in v1 should have to be redone for them (section 5.13).

## 3. Principles

1. **The server is the product.** The app and the CLI hold no session state. Anything the
   app can do, the CLI and Python can do, through the same API.
2. **Works with no configuration.** Config exists for when there is something to change.
3. **Driven by software, visible to people.** Every automated change shows up in the UI,
   and every UI action has an API equivalent.
4. **An extension cannot hurt a session.** Python failing never ends, freezes, or delays
   a terminal.
5. **Match existing protocols before inventing new ones.** OSC 7501, OSC 133, OSC 7,
   SSH, XDG paths.

## 4. Vocabulary

One set of words across the API, CLI, Python, and UI. Rex says "window" and "block" in
its API and "tab" and "pane" in its app; steg uses the UI words everywhere.

| Term | Meaning |
|---|---|
| Host | A machine running a steg server |
| Session | A named, long-lived workspace on one host |
| Tab | An ordered page within a session |
| Pane | A region of a tab running one thing. In v1 that thing is always a terminal |
| Client | One connection to one server. A CLI call and a Python extension are clients; so is each connection a window of the app holds |
| Window | What the app draws. It holds one client for each server it uses, and shows one session at a time, from any of them |
| Focus | A path down one server's tree: each session's current tab, and each tab's current pane |
| Action | A named operation that can be bound to a key, shown in the palette, or invoked |
| Event | A notification from the server that something changed |

## 5. Requirements

Each requirement is tagged with the release it belongs to (section 6).

### 5.1 Sessions and the server

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

### 5.2 Layout

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
  pane may be, and what happens when the window is resized follow doc-7, D45 to D51.

### 5.3 Terminal

- R13 (M1). A pane runs the user's login shell, or a given command, with a given working
  directory and environment.
- R14 (M1). Full-screen programs render correctly: vim, htop, tmux, and Claude Code are
  the acceptance set. Supported features include truecolour, mouse reporting, bracketed
  paste, the alternate screen, the kitty keyboard protocol, OSC 8 links, OSC 52
  clipboard, OSC 7 working directory, and OSC 133 prompt marks.
- R15 (M1). The server keeps scrollback per pane up to a configurable limit, 10,000 lines
  by default.
- R16 (M1). The server knows each terminal's working directory, title, and its program's
  exit code, and reports changes as events.
- R17 (M1). The app supports text selection, copy, paste, find in scrollback, and
  clickable links.
- R18 (M2). The app can jump between shell prompts in scrollback.
- R19 (M1). Typing in a local pane paints within 16 ms at the 95th percentile. Printing a
  100 MB file does not freeze the app or delay input to other panes. These numbers are
  proposals to confirm once a spike has measured a baseline.

### 5.4 Desktop app

- R20 (M1). A macOS app built with GPUI. A window shows a session switcher, the current
  session's tabs, and the current tab's panes. More than one window can be open.
- R21 (M1). A command palette goes to any session, tab or pane, and lists every action,
  searchable by title. Its modes, steps and keys follow doc-7, D79 to D88.
- R22 (M1). The app is fully usable with no config file.
- R23 (M1). The terminal font, size, line height, ligatures, and fallback fonts are
  configurable.
- R79 (M1). What the window looks like and how its parts behave is doc-7, "Spec: the
  app's design": its named values, and its requirements D1 to D110. App tasks cite those
  numbers. Where doc-7 marks a value as proposed, it is not settled until Bruk says so.
- R80 (M1). There is no settings window. A setting is changed from the palette, applies
  at once, and is an override the server remembers; steg never writes to the config file
  (doc-7, D101 and D106 to D108).

### 5.5 Appearance

- R24 (M1). The app follows the system's light and dark setting and can be pinned to
  either.
- R25 (M1). The default dark look is a near-black base with warm gradient accents
  (violet through pink to orange), in the spirit of the Superlogical site and early Arc.
  No Liquid Glass and no dependence on macOS 26 materials.
- R26 (M1). Each session carries one of five colours (violet, pink, orange, blue, green),
  so sessions are told apart at a glance. A new session takes the next colour not in use,
  and the palette changes it. Where the colour shows follows doc-7, D9. A change shows in
  every attached client at once.
- R81 (M2). A session's colour can also be any colour or gradient, set from the CLI and
  Python like any other session property (doc-7, D10).
- R27 (M2). Terminal colour themes are selectable separately for light and dark.

### 5.6 Keyboard

- R28 (M1). Every action can be bound to a key. Sensible defaults ship with the app.
- R29 (M1). A key that is not bound reaches the terminal unchanged.
- R30 (M2). Bindings support key sequences, such as `g` then `g`.
- R31 (M2). Bindings support named key modes, such as a resize mode in which arrow keys
  resize panes. Escape leaves a mode.

### 5.7 CLI

These say what the CLI can do. Command names, flags, variable names and exit codes are
left to CLI design (section 7).

- R32 (M1). The `steg` CLI exposes everything the API exposes. Its commands are generated
  from the API description, so the two cannot drift.
- R33 (M1). Inside a steg terminal the CLI acts on that session and pane by default,
  from context the terminal's environment provides. Anything else can be targeted
  explicitly.
- R34 (M1). Output is readable by a person by default and available as JSON. The exit
  status tells a script apart: success, failure, bad arguments, a missing or ambiguous
  target, an unreachable server, and an interruption.
- R35 (M1). The CLI can send text and keys to a pane, capture a pane's screen and
  scrollback as text, and wait for an event such as a program exiting.

### 5.8 API

- R36 (M1). One documented API serves the app, the CLI, and Python. The app has no
  private channel.
- R37 (M1). The API describes itself: a client can ask for every method, its parameters,
  and its documentation.
- R38 (M1). Clients can subscribe to events: session, tab, and pane created, closed, and
  focused; terminal working directory, title, bell, exit, and program status changed.
- R39 (M1). The protocol is versioned. A mismatched client and server either work
  together or fail with a message naming both versions.

### 5.9 Python: config and plugins

Python is kept open-ended. Inside its own process a plugin may use any library and be
written any way. The requirements below fix only the boundary between Python and steg:
what a plugin can do, and what it cannot. Bruk agreed these limits on 2026-11-13.

**Config**

- R40 (M2). Configuration is a Python file in steg's config directory that builds a
  config object, a dataclass. It is data: key bindings, key modes, which viewer handles
  which file type, and the plugins to run.
- R48 (M2). The config can be validated without applying it, and reloaded without
  restarting the server or ending a session. A config with errors leaves the previous
  one in force.

**Plugins**

- R41 (M2). A plugin is a Python program whose entry point is `__main__`. Steg runs
  each plugin the config lists, in a process of its own, and no others: nothing is discovered or
  started by itself.
- R42 (M2). A plugin can call anything in the public API, the same surface the CLI has,
  and can subscribe to any event and keep running to handle it.
- R43 (M2). A plugin can register actions and key bindings. They exist while the plugin
  runs: the actions appear in the palette, can be bound to keys, and can be invoked from
  the CLI.
- R44 (M2). When a plugin stops or dies, everything it registered disappears and nothing
  else is affected.
- R45 (M2). A plugin can declare third-party dependencies, resolved by uv. Each
  plugin has its own environment, apart from other plugins' and from any virtual
  environment active in a pane.

**Limits**

- R46 (M2). An exception, hang, or crash in Python does not end a session, block input to
  a terminal, or stop built-in key bindings. The user sees the error and its traceback.
- R47 (M2). The server, the app, and built-in bindings work on a host with no Python
  installed. Python is needed only once there is a config file or a plugin.
- R72 (M2). No Python runs inside the server or the app.
- R73 (M2). A key press never waits on Python. Bindings are resolved from the config's
  data, and a plugin cannot sit in the path of typing or of terminal output. It can read
  a screen and send input through the API.
- R74 (M2). A plugin cannot draw in the app's own interface. What it shows is a terminal
  pane, a session colour, a label, a status, or a notification.
- R75 (M2). A plugin reaches nothing the public API does not offer. There are no private
  hooks.
- R76 (M2). A plugin cannot remove or alter built-in behaviour. It can add actions, and
  keys can be bound or rebound to them.
- R77 (M2). Plugins are isolated from one another. A plugin cannot read or change
  another plugin's state, call into its code, or invoke its actions. This protects
  plugins from each other's mistakes; it is not a security sandbox (section 8).

### 5.10 Program status

- R49 (M2). Steg implements the Program Status Protocol (OSC 7501) as Superlogical
  publishes it: grammar, keys, size limits, whole-record replacement, the `id` hierarchy,
  record lifetime, the feature detection reply, and the security rules.
- R82 (M1). Without the protocol, steg still shows what it can know by itself about a
  pane's foreground program: running while it runs, and finished or failed from its exit
  status. Progress, waiting for a person, and the bell as a status need R49.
- R50 (M1 for running, finished and failed; M2 for the rest). A pane's status is shown as
  a ring on its tab while the pane is out of view, and as a bar in the pane's own header
  while it is in view. A tab shows one ring, for the most pressing of its panes. The
  selected tab shows none, and a session never shows one. Shapes, colours, order and
  motion follow doc-7, D52 to D78.
- R83 (M1). Each status ends by its own rule: waiting and failed never on a timer;
  finished after 10 minutes and the bell after 30 seconds, or sooner on a key press or
  click in that pane. Opening a tab clears nothing (doc-7, D109 and D110).
- R84 (M2). A tab holds the current status of each of its panes as a set: a new report
  replaces only that pane's last one, and the tab shows the most pressing, then the next
  when one ends. A pane's status can be promoted to its tab from the CLI; a pane that is
  waiting or has failed still takes the ring. The command and its protocol form are
  decided in TASK-1.30.
- R51 (M3). A waiting or failed status can raise a macOS notification naming the session
  and pane, only while steg is in the background and not more than once per pane per
  state. Finished does not notify. Each is switchable on its own (doc-7, D71).
- R52 (M2). Status changes are events, and the CLI can list a pane's status records.

### 5.11 Agents

- R53 (M2). Steg ships a skill for Claude Code that teaches the CLI, and a session-start
  hook that tells the agent which session and pane it is in. The skill covers setting the
  session's colour, so a running agent can change the colour of its own session.
- R54 (M2). Using only the CLI, an agent can open a tab, split a pane, run a command in
  another pane, read that pane's screen, and wait for the command to finish.

### 5.12 Remote hosts

- R55 (M3). The app and CLI connect to a server on another host over SSH, using the
  user's existing SSH config, keys, and agent. The remote host needs only the `steg`
  binary and no listening port.
- R56 (M3). Hosts are saved under a label. The app switches between hosts, and the CLI
  can target a host by its label.
- R57 (M3). When the connection drops, the remote sessions carry on. The app reconnects
  by itself and restores them.

### 5.13 Ready to distribute later

These cost little now and a great deal to retrofit.

- R58 (M1). No telemetry, no account, no licence check, and no network call the user did
  not ask for.
- R59 (M1). MIT licensed, with no code taken from GPL sources. Zed's terminal crates are
  GPL, so the GPUI terminal view is written fresh.
- R60 (M1). Config, state, and sockets follow XDG paths. Nothing is
  tied to one person's machine.
- R61 (M1). The server and CLI ship as one binary with no runtime dependency beyond the
  operating system.
- R62 (M2). Reference docs for the CLI and the API are generated
  from the API description.

### 5.14 Showing things beside the terminal

Claude Desktop shows documents and previews in a side panel and decides what to show and
when. Steg gives an agent, or a person, the same ability from the CLI. A viewer is a
terminal program that steg runs in an ordinary pane, so every pane stays a
terminal. Bruk decided this on 2026-11-13, over panes the app draws itself.

- R63 (M2). One CLI command shows a file in a new pane beside the pane the command ran
  in, choosing a viewer from the file's type. It can be put in a new tab instead.
- R64 (M2). Opening a pane this way leaves keyboard focus where it was unless focus is
  asked for, so an agent showing something does not interrupt typing.
- R65 (Later). Steg packages its own viewers, Markdown and diffs first. None is built
  in M1 to M3; until then R63 runs whatever program the config maps to the file's
  type (R69).
- R66 (Later). A packaged viewer follows its file: when the file changes on disk, the
  pane shows the new content within a second and keeps its scroll position. A program
  mapped in the config follows its file only if that program does.
- R67 (M2). Opening a file already shown in the same tab reuses that pane. From the CLI, a
  viewer's content can be replaced with another file and the pane closed.
- R68 (M2). Content can be given on standard input in place of a file, with its type
  named, so an agent can show something that is not saved anywhere.
- R69 (M2). Which viewer handles which file type is set in the config (R40), so a
  viewer can be added or replaced without changing steg.
- R70 (M2). The Claude Code skill (R53) teaches the agent to open a viewer for a document
  it has changed or wants the user to read.
- R71 (M2). Viewers are read-only.

## 6. Releases

| Release | What it proves | Requirements |
|---|---|---|
| M1: local daily driver | Steg can replace the terminal on one Mac. Sessions survive the app | R1-R6, R8-R10, R12-R17, R19-R26, R28, R29, R32-R39, R58-R61, R78-R80, R82, R83, R85-R91, and R50 for running, finished and failed |
| M2: extendable | Python, the status protocol, and agents make it more than a terminal | R18, R27, R30, R31, R40-R49, R52-R54, R62-R64, R67-R77, R81, R84, and the rest of R50 |
| M3: remote and shared | The same workspace on other hosts, and from two clients at once | R7, R11, R51, R55-R57 |

Two spikes come before M1, because they carry the most risk:

1. A GPUI window rendering one live terminal, measured against R19.
2. A server holding a session that the app detaches from and reattaches to (R2).

## 7. Design

This is a product spec first, so the design is deliberately thin: the parts, the intent's
questions, and the decisions still to make.

### Parts

| Part | Role |
|---|---|
| Kernel (Rust library) | Session, tab, and pane state; terminals; the event stream; target resolution |
| Server | The kernel plus its local socket. Starts on demand and outlives the app |
| CLI | A thin client whose commands come from the API description |
| Steg.app (GPUI) | Renders panes, handles keys, runs the palette |
| Plugins | Python programs, one process each, beside the server and using the public API |

### The intent's open questions

- **Carries forward intent question 1** (what Rex does not do for you beyond Python):
  only Bruk can answer it. No requirement depends on the answer, but it could add some.
- **Answers intent question 2** (where Python runs): beside the server, never inside
  it, with each plugin in a process of its own speaking the public API. Bruk decided
  this on 2026-11-13. A stuck or crashed plugin then cannot freeze a terminal or disturb
  another plugin, any library works, and each plugin has its own dependencies. The cost
  is one hop between the server and Python for each custom action, and the memory of
  one Python process per plugin (section 8).
- **Carries forward intent question 3** (terminal emulation library): `libghostty-vt` or
  `alacritty_terminal`, decided by the first spike in section 6.
- **Answers intent question 4** (remote sessions): a later release, M3 (R55-R57). This is
  an assumption to confirm at acceptance.
- **Answers intent question 5** (matching Rex's names): no. Similar shape, steg's own
  names. OSC 7501 is the only compatibility promise. An assumption to confirm.
- **Answers intent question 6** (surviving a server restart or reboot): not in v1; see
  section 10. An assumption to confirm.

### Decisions still to make

The requirements hold whichever way these go.

1. **CLI design.** Command names, flags, the environment variables that carry context,
   and exit codes. Deliberately not decided in this spec.
2. **Wire protocol.** Message framing, and how terminal output reaches a client: raw
   bytes replayed by a client-side emulator, or screen updates computed by the server.
3. **API description.** The single source that generates the CLI, any Python client library,
   and the docs.
4. **Server lifecycle on macOS.** Spawned by the first client or managed by launchd, and
   when an idle server stops.
5. **Python on the inside.** Whether steg offers a client library, the fields of the
   config dataclass, and how the config file is evaluated: most simply a short-lived
   process at load and reload that hands its data to the server.
6. **Two clients, one pane.** Decided 2026-11-15: the latest client wins (R91). The same
   rule serves two windows on one machine in M1 and two machines in M3.

## 8. Areas of concern

- **GPUI is pre-1.0.** Its API changes between releases, so the version is pinned and
  upgrades are deliberate work.
- **Extensions are not sandboxed.** Python runs with the user's full privileges. That is
  acceptable for a personal tool and needs revisiting before distribution.
- **OSC 7501 belongs to Superlogical and Rex is in beta.** The protocol may change, and
  steg follows it rather than forking it.
- **Many plugin processes.** Each plugin is a Python process, so memory grows with the
  number of plugins. An idle plugin should cost no CPU, and its memory needs measuring
  before a budget is set. If it proves heavy, plugins that only provide actions can be
  started when first used instead of at launch.
- **Python on remote hosts.** Extensions on a remote host need uv and Python there. R47
  keeps the server itself free of that dependency.

## 9. Open questions the intent did not ask

None.

## 10. Out of scope

- Liquid Glass.
- Linux, iOS, Windows, and FreeBSD, for the server and the app alike. Remote hosts
  (R55-R57) are other Macs.
- Shared and login servers, and anything multi-user.
- Network listeners and built-in Tailscale. SSH is the only remote transport.
- Pane kinds other than terminals. R12 keeps the door open.
- Building the packaged viewers (R65). Markdown and diffs are noted as the first two.
- Editing in a viewer.
- Lua, and compatibility with Rex's API, CLI, or config.
- Sessions surviving a server restart, an upgrade, or a reboot.
- Signed installers, auto-update, and a Homebrew formula, until distribution is decided.
- A web client and live session sharing with other people.
