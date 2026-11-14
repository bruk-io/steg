---
id: doc-2
title: CLI specification
type: specification
created_date: '2026-11-14T01:18:00Z'
updated_date: '2026-11-14T01:53:00Z'
---
- Intent: TASK-1, "A terminal like Rex, built in Rust and GPUI and extendable through Python"
- Spec: doc-1. This document is design decision 1 in its section 7, and gives R32 to R35 and R63 their concrete form.
- Status: draft. Sections 1 to 6 follow Bruk's direction of 2026-11-13. Section 9 lists what he has decided since and what is still his to decide.
- Policy: the team's CLI design skill (`plugins/team/skills/cli-design/`). Where this document is more specific, this document governs.

The steg CLI is written in Rust and ships in the same binary as the server (R61). Its commands are generated from the API description (R32), never written by hand.

## 1. What it is designed for

In this order when they pull apart:

1. **Predictable.** One grammar, a closed set of verbs, no aliases, no behaviour that depends on whether a person is watching.
2. **Composable.** One command does one thing. Results go to stdout in a form the next program can use, content comes in on stdin, and the exit status says what happened.
3. **Fast.** Quick enough to sit in a shell loop, a key binding, or an agent's inner loop.

Agents and scripts are the main users. A person at a prompt is served by the same rules, not by a second, friendlier mode.

## 2. Grammar

```
steg <noun> <verb> [target] [flags]
```

- Every command has a noun and a verb. `steg <noun>` alone lists that noun's verbs, and `steg` alone lists the nouns.
- The one positional is the target: the thing the verb acts on. No command has a second, with one exception: a program to run is given after `--`, as in `steg pane split -- htop -d 5`.
- Everything else is a flag.
- There are no aliases and no abbreviations: `steg pane list`, never `steg ls`.

### How a command comes from the API

A command is the API method of the same name, mechanically: the method `pane.split` is `steg pane split`, its target parameter is the positional, and each other parameter is the flag of the same name with underscores turned into hyphens. A boolean parameter is a switch. Nothing is renamed by hand, so there is no table of names to drift, and a method the CLI does not expose is a failing test (R32).

A new command is therefore a new API method, named so that its noun and verb obey this document.

## 3. Nouns and verbs

### Nouns

| Noun | What it is | Release |
|---|---|---|
| `session` | A named, long-lived workspace on one host | M1 |
| `tab` | An ordered page within a session | M1 |
| `pane` | A region of a tab running one thing | M1 |
| `server` | The steg server on this host | M1 |
| `event` | Something the server reports as it happens | M1 |
| `config` | The user's configuration | M2 |
| `plugin` | A Python plugin the config lists | M2 |
| `action` | A named operation, built in or from a plugin | M2 |
| `status` | A program status record from OSC 7501 | M2 |
| `host` | A saved steg server on another machine | M3 |

### Verbs every noun shares

| Verb | Meaning | Prints on success |
|---|---|---|
| `list` | Every one of the noun in scope, one per line | The records |
| `show` | One of the noun, in full | The record |
| `new` | Create one. The target is where it is created | The new identifier, alone on a line |
| `close` | End one, and whatever runs in it | Nothing |
| `move` | Change its position or its parent | Nothing |
| `set`, `unset` | Change or clear its properties, each a flag | Nothing |
| `focus` | Make it the one that receives input | Nothing |
| `start`, `stop` | A long-running thing's lifecycle | Nothing |
| `check` | Validate without changing anything | Findings, if any |
| `watch` | Stream changes until interrupted | One record per line |
| `wait` | Block until a named condition, then exit | The event that ended the wait |

A label, a colour and a zoomed state are properties, so they are `set` and `unset`, not verbs of their own: `steg pane set --label logs`, `steg session set --colour violet`, `steg pane set --zoomed` and `steg pane set --no-zoomed`. There is no `rename`, no `zoom` and no toggle.

### Verbs one noun owns

| Command | Meaning | Prints on success |
|---|---|---|
| `pane split` | Split the target pane and run a program in the new one | The new pane's identifier |
| `pane resize` | Move the border of the target pane | Nothing |
| `pane swap` | Exchange the target with the pane given by `--with` | Nothing |
| `pane send` | Send text or keys to the program in the target | Nothing |
| `pane capture` | Print the target's screen, and scrollback when asked | The text |
| `config reload` | Apply the configuration | Nothing |
| `action run` | Run the named action | What the action returns, if anything |

Adding a verb to either table is a change to this document.

## 4. Targets

A target names one session, tab or pane. It is a path of up to three parts, most specific last:

```
[session/][tab/]pane        for a pane
[session/]tab               for a tab
session                     for a session
```

- Each part is a full identifier, a label, a position (1, 2, ...), or the last four or more characters of an identifier (R10).
- A part left out comes from the context: `logs` is the pane labelled logs in the current tab, `2/logs` the same in tab 2 of the current session, `work/2/logs` in the session labelled work.
- With no target at all, the command acts on where it was run: the current pane, tab or session.
- A target that matches nothing, or more than one thing, fails with status 3 and lists the candidates on stderr. Nothing is done.
- A label may not contain `/`, and may not be all digits, so a path always reads one way.

For `new`, the target is the parent the new thing is created in. `steg tab new work` creates a tab in the session labelled work.

### Context

Inside a steg terminal the server sets these, and the CLI reads them. They carry identifiers, never labels.

| Variable | Holds |
|---|---|
| `STEG_SERVER` | How to reach the server this terminal belongs to |
| `STEG_SESSION` | The session this terminal is in |
| `STEG_TAB` | Its tab |
| `STEG_PANE` | Its pane |

Outside a steg terminal there is no context, so a command that needs a target and is given none fails with status 2 and says so.

## 5. Flags

- Long flags are lowercase words joined by hyphens.
- A boolean flag is a switch: `--zoomed`, never `--zoomed=true`. A state that can be on has a `--no-` flag to turn it off, and both are always accepted.
- A flag that takes a value always takes one. `--flag value` and `--flag=value` both work.
- Flags may come before or after the target. A bare `--` ends the flags.
- A flag means the same thing on every command that has it.

### Flags every command has

| Flag | Meaning |
|---|---|
| `-h`, `--help` | Usage for this exact command, on stdout, status 0. Contacts no server |
| `--json` | Machine-readable output (section 6) |
| `-q`, `--quiet` | Print nothing on success, not even a new identifier |
| `-v`, `--verbose` | Diagnostics on stderr |
| `--no-color` | No colour. Colour is also off when stdout is not a terminal or `NO_COLOR` is set |
| `--no-input` | Never read from the terminal. This is already the default |
| `--host <label>` | Act on a saved host in place of the one in the context (M3) |

`steg --version` prints the version and contacts no server.

### Flags that recur

| Flag | On | Meaning |
|---|---|---|
| `--label <text>` | `new`, `set`, `unset` | The thing's label |
| `--position <n>` | `new`, `move` | Where it goes among its siblings |
| `--to <target>` | `move` | The new parent |
| `--direction <left, right, up or down>` | `pane split`, `pane resize`, `pane focus` | Which way |
| `--cwd <path>` | `session new`, `tab new`, `pane split` | The new program's working directory |
| `--text <text>` | `pane send` | Text to send, in place of standard input |
| `--key <name>` | `pane send` | A key to send, by name. May be given more than once |
| `--scrollback` | `pane capture` | Include scrollback, not just the screen |
| `--for <event>` | `wait` | What to wait for |
| `--timeout <seconds>` | `wait`, `watch` | Give up after this long, with status 1 |
| `--force` | `close`, `server stop` | Do it even though programs are still running |

## 6. Input, output and exit status

- **stdout is the result and nothing else.** Diagnostics, warnings and progress go to stderr.
- **An action prints nothing on success.** A command that creates something prints its full identifier alone on one line, so `steg pane split` can be captured into a variable.
- **A list is one record per line, tab-separated, in a fixed column order,** with a header line only when stdout is a terminal. Piping changes nothing else.
- **`--json` is a contract.** Full identifiers, the field names of the API, fields only ever added. `list` prints one JSON array, `show` one object, and `watch` one object per line.
- **Content comes in on standard input.** `steg pane send` with no `--text` and no `--key` reads stdin to its end and sends it.
- **Times are ISO 8601 in UTC.**
- **The CLI never prompts,** never pages, and never checks for a newer version.

| Status | Meaning |
|---|---|
| 0 | Success |
| 1 | The command ran and failed, or a wait timed out |
| 2 | Bad usage: an unknown flag, a missing value, no target and no context |
| 3 | The target matched nothing, or more than one thing |
| 4 | The server could not be reached or started |
| 130 | Interrupted |

`steg pane wait --for exit` exits 0 when the program ended, and prints the program's own exit code in its result. It does not adopt that code as its own status, so a script can tell "the wait failed" from "the program failed".

## 7. Speed

These are proposals until measured. TASK-1.3 measures CLI start-up for both ways of building the command tree, and its numbers replace these.

- `--help` and `--version`: under 10 ms, with no server running.
- A command against a local server that is already up: under 20 ms at the 95th percentile, from start to exit.
- One request per command where one will do.
- No config is read by a command that does not need it, and the Python plugins are never started by the CLI.

## 8. The first release's commands

Generated from the API, so this table describes the API as much as the CLI. Each takes the flags in section 5 that apply.

| Command | Target | Notes |
|---|---|---|
| `session list` | none | Every session on the host |
| `session show` | session | |
| `session new` | none | `--label`, `--cwd`, and a program after `--` |
| `session close` | session | `--force` if programs are running |
| `session move` | session | `--position` |
| `session set`, `session unset` | session | `--label` |
| `session focus` | session | |
| `tab list` | session | The tabs of that session |
| `tab show` | tab | |
| `tab new` | session | `--label`, `--cwd`, `--position`, and a program after `--` |
| `tab close` | tab | `--force` if programs are running |
| `tab move` | tab | `--position`, `--to` another session |
| `tab set`, `tab unset` | tab | `--label` |
| `tab focus` | tab | |
| `pane list` | tab | The panes of that tab |
| `pane show` | pane | Kind, size, working directory, title, running program |
| `pane split` | pane | `--direction`, `--cwd`, `--label`, and a program after `--` |
| `pane close` | pane | `--force` if a program is running |
| `pane move` | pane | `--to` another tab |
| `pane set`, `pane unset` | pane | `--label`, `--zoomed` |
| `pane focus` | pane | Or `--direction` from the target |
| `pane resize` | pane | `--direction`, `--by <cells>` |
| `pane swap` | pane | `--with <target>` |
| `pane send` | pane | stdin, or `--text`, `--key` |
| `pane capture` | pane | `--scrollback` |
| `pane wait` | pane | `--for exit`, `--timeout` |
| `server show` | none | Version, uptime, sessions, clients |
| `server start` | none | Stays up with no clients |
| `server stop` | none | `--force` if sessions exist |
| `event watch` | session | `--for <event>` to filter; all sessions with `--all` |

Later releases add `config check`, `config reload`, `plugin list`, `action list`, `action run`, `status list`, `session set --colour`, the command that shows a file in a pane (R63), and the `host` noun.

## 9. Decided, and still to decide

Decided by Bruk on 2026-11-13:

- **A program to run is given after `--`.** `steg pane split -- htop -d 5`, and the same on `session new` and `tab new`. It is the one exception to a single positional, and follows `env`, `ssh` and `sudo`.
- **`close` and `server stop` refuse while programs are still running,** with status 1 and a message naming `--force`. With `--force` they go ahead.
- **A target's parts are separated by `/`,** as in `work/2/logs`.

Still Bruk's to decide:

1. **Closing something that is already closed.** Proposed: status 3, like any target that matches nothing, since a script that names a pane that is gone has usually made a mistake.
2. **A header line on lists.** Proposed: only when stdout is a terminal. The alternative is never, which is more predictable and less readable.
3. **Short flags.** Proposed: only `-h`, `-q` and `-v`. Every other flag is long.
4. **Shell completion.** It is not a noun and a verb. Proposed: `steg --completion <shell>` beside `--version`, as the second and last flag on the bare program.
5. **Showing a file in a pane (R63).** Proposed for M2: `steg pane split --file <path>`, which runs the viewer the config maps to that file's type in the new pane, so no new verb is needed.
6. **Saved hosts (M3).** `new` and `close` read oddly for an entry in a list. Whether `host` gets `add` and `remove` as verbs of its own is left for that release.

## 10. Where this differs from Rex

| | Rex | steg | Why |
|---|---|---|---|
| Shape | Verb first: `rex split`, `rex ls`, `rex send` | Noun then verb: `steg pane split` | One grammar that the API generates, and that can be guessed |
| Targets | A flag per level: `-s`, `-w`, `-b`, and `-S` for the host | One path as the positional, and `--host` | One way to write a target |
| Words | `window` and `block` in the CLI and API | `tab` and `pane` everywhere | The same words as the app (doc-1, section 4) |
| Scripting | `rex do` runs Lua in the CLI | No scripting in the CLI; Python plugins use the API | Python runs beside the server, never inside the binary (R72) |
| Context | `REX_SESSION`, `REX_BLOCK`, `REX_SERVER` | `STEG_SESSION`, `STEG_TAB`, `STEG_PANE`, `STEG_SERVER` | Adds the tab, so a relative target never needs a lookup |
| Exit statuses | 0, 1, 2, 3, 4, 130 | The same | No reason to differ |
| `--json` | Yes | Yes | The same |
