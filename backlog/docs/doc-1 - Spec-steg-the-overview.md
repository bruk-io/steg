---
id: doc-1
title: 'Spec: steg, the overview'
type: specification
created_date: '2026-11-13T22:34:00Z'
updated_date: '2026-11-15T02:21:00Z'
---
- Intent: TASK-1, "A terminal like Rex, built in Rust and GPUI and extendable through Python".
- This is the overview. It says what steg is, fixes the words, lists the specs, and says which requirement is in which release. The requirements themselves are in the specs listed in section 5.
- Status: draft. Reorganised on 2026-11-15 from one long spec into this overview and nine area specs. Requirement numbers are unchanged, and no requirement was reworded in the move except to repoint references to documents since retired.
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

## 5. The specs

Each has the same shape: scope, requirements, open questions, out of scope. Requirement numbers are steg-wide and never reused.

| Spec | Covers | Requirements |
|---|---|---|
| doc-15, domain | The things steg is made of and the rules between them: structure, layout, focus, clients, a pane's program | R8-R12, R78, R85-R91, R93-R97, R100 |
| doc-8, server and sessions | The server as a running thing, and the API | R1-R7, R36-R39, R98, R99, R101 |
| doc-14, wire protocol | What travels between the server and a client | Serves the above |
| doc-9, terminal | Emulation, scrollback, speed | R13-R19, R92 |
| doc-10, app | The window, appearance, keyboard; points at the design artifact | R20-R31, R79-R81 |
| doc-2, CLI | The `steg` command | R32-R35 |
| doc-11, program status | What steg knows about the program in a pane | R49-R52, R82-R84 |
| doc-12, Python config and plugins | Config, plugins, agents, showing things beside the terminal | R40-R48, R53-R54, R63-R77 |
| doc-13, remote hosts and distribution | Other machines, and readiness to distribute | R55-R62 |

The decisions these rest on are decision records, decision-1 to decision-10.

## 6. Releases

| Release | What it proves | Requirements |
|---|---|---|
| M1: local daily driver | Steg can replace the terminal on one Mac. Sessions survive the app | R1-R6, R8-R10, R12-R17, R19-R26, R28, R29, R32-R39, R58-R61, R78-R80, R82, R83, R85-R101, and R50 for running, finished and failed |
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
| Server | The kernel plus its local socket, in a package no client depends on (R98). Starts on demand and outlives the app. Ships in one binary with the CLI (R61) |
| The steg library | How a Rust program is a client: the API description, the wire protocol's types, and the code to start, connect to and use a server (R101) |
| CLI | A thin client, built on the steg library, whose commands come from the API description |
| Steg.app (GPUI) | Renders panes, handles keys, runs the palette |
| Plugins | Python programs, one process each, beside the server and using the public API |

The repository is a monorepo for now (Bruk, 2026-11-15): the server, the CLI and the app are separate packages in one repository, kept apart so that any of them could move to a repository of its own.

### The intent's open questions

- **Carries forward intent question 1** (what Rex does not do for you beyond Python):
  only Bruk can answer it. No requirement depends on the answer, but it could add some.
- **Answers intent question 2** (where Python runs): beside the server, never inside
  it, with each plugin in a process of its own speaking the public API. Bruk decided
  this on 2026-11-13. A stuck or crashed plugin then cannot freeze a terminal or disturb
  another plugin, any library works, and each plugin has its own dependencies. The cost
  is one hop between the server and Python for each custom action, and the memory of
  one Python process per plugin (section 8).
- **Intent question 3** (terminal emulation library): decided 2026-11-15, ghostty's VT
  engine (R92, decision-3).
- **Answers intent question 4** (remote sessions): a later release, M3 (R55-R57). This is
  an assumption to confirm at acceptance.
- **Answers intent question 5** (matching Rex's names): no. Similar shape, steg's own
  names. OSC 7501 is the only compatibility promise. An assumption to confirm.
- **Answers intent question 6** (surviving a server restart or reboot): not in v1; see
  section 10. An assumption to confirm.

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
