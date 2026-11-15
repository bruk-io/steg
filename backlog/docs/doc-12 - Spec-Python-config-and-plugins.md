---
id: doc-12
title: 'Spec: Python config and plugins'
type: specification
created_date: '2026-11-15T01:13:00Z'
updated_date: '2026-11-15T01:24:00Z'
---
- Intent: TASK-1, "A terminal like Rex, built in Rust and GPUI and extendable through Python".
- One of steg's specs. doc-1 is the overview: it has the vocabulary, the release table and the list of specs. Requirement numbers are steg-wide and never reused, so R19 means the same thing in every document.
- Rests on: decision-9 (settings are overrides; the Python config is the only file).
- Status: draft. Reorganised from the single spec on 2026-11-15; every requirement here was moved without rewording, except that references to documents since retired now point at the decision records and at doc-10.

## Scope

Everything Python: the config file, plugins, agents, and showing things beside the terminal. All of it is M2. It also covers the one piece of settings state M1 has before the config exists.

## Requirements

### Python: config and plugins

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

### Agents

- R53 (M2). Steg ships a skill for Claude Code that teaches the CLI, and a session-start
  hook that tells the agent which session and pane it is in. The skill covers setting the
  session's colour, so a running agent can change the colour of its own session.
- R54 (M2). Using only the CLI, an agent can open a tab, split a pane, run a command in
  another pane, read that pane's screen, and wait for the command to finish.

### Showing things beside the terminal

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

## Open questions

- [ ] Python on the inside: whether steg offers a client library, the fields of the config dataclass, and how the config file is evaluated, most simply a short-lived process at load and reload that hands its data to the server.
- [ ] R80 has the server remembering settings overrides in M1, before the config exists. This spec does not yet say where that state lives or how it meets the config when the config arrives.
- [ ] Intent question 1, what Rex does not do for you beyond Python, is Bruk's to answer. No requirement depends on it, but it could add some.

## Out of scope

- Sandboxing plugins. Python runs with the user's full privileges, which is acceptable for a personal tool and needs revisiting before distribution.
- A pane kind the app draws. A viewer is a terminal program in an ordinary pane.
