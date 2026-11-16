---
id: doc-2
title: 'Spec: CLI'
type: specification
created_date: '2026-11-14T01:18:00Z'
updated_date: '2026-11-16T16:43:00Z'
---
- Intent: TASK-1, "A terminal like Rex, built in Rust and GPUI and extendable through Python".
- One of steg's specs. doc-1 is the overview: it has the vocabulary, the release table and the list of specs. Requirement numbers are steg-wide and never reused, so R19 means the same thing in every document.
- Rests on: decision-4 (steg's grammar; Rex where undecided), decision-1 (commands are generated from the API description), decision-5 (the client noun and the three focus commands).
- Status: draft. Reorganised from the single spec on 2026-11-15; every requirement here was moved without rewording, except that references to documents since retired now point at the decision records and at doc-10.

## Scope

The `steg` command: its grammar, nouns and verbs, targets, flags, output and exit statuses, and the first release's commands. Sections 1 to 10 below are the specification a builder reads.

## Requirements

These say what the CLI can do. Command names, flags, variable names and exit codes are
left to CLI design (section 7).

- R32 (M1). The `steg` CLI exposes everything the API exposes. Its commands are generated
  from the API description, so the two cannot drift. One kind of method is the stated
  exception: a method whose reply is followed by items on a stream of its own (`pane attach`;
  the description marks it) is listed and described by the CLI, and reachable through
  `api call`, but has no rendering of the stream as a command, because a terminal's screen
  updates are for a program to consume (decided by Bruk on 2026-11-16 with TASK-1.47's plan).
- R33 (M1). Inside a steg terminal the CLI acts on that session and pane by default,
  from context the terminal's environment provides. Anything else can be targeted
  explicitly.
- R34 (M1). Output is readable by a person by default and available as JSON. The exit
  status tells a script apart: success, failure, bad arguments, a missing or ambiguous
  target, an unreachable server, and an interruption.
- R35 (M1). The CLI can send text and keys to a pane, capture a pane's screen and
  scrollback as text, and wait for an event such as a program exiting.

The steg CLI is written in Rust and ships in the same binary as the server (R61). Its commands are generated from the API description (R32), never written by hand.

## 1. What it is designed for

In this order when they pull apart:

1. **Predictable.** One grammar, a closed set of verbs, no aliases, no behaviour that depends on whether a person is watching. Colour is the one exception (section 6).
2. **Composable.** One command does one thing. Results go to stdout in a form the next program can use, content comes in on stdin, and the exit status says what happened.
3. **Fast.** Quick enough to sit in a shell loop, a key binding, or an agent's inner loop.

Agents and scripts are the main users. A person at a prompt is served by the same rules, not by a second, friendlier mode.

## 2. Grammar

```
steg <noun> <verb> [target] [flags]
```

- Every command has a noun and a verb. `steg <noun>` alone lists that noun's verbs, and `steg` alone lists the nouns; both succeed, with status 0 (confirmed by Bruk on 2026-11-16 with TASK-1.49's plan: listing is what was asked for, so it is not a usage error).
- The one positional is the target: the thing the verb acts on. No command has a second, with one exception: a program to run is given after `--`, as in `steg pane split -- htop -d 5`.
- Everything else is a flag.
- There are no aliases and no abbreviations: `steg pane list`, never `steg ls`.
- Three commands are not API methods, work with no server running, and so have no noun: `steg version`, `steg completion <shell>`, and `--help` (`-h`) on the bare program or on any command. They are subcommands, as Rex has them. Nothing else is added to this list without changing this document.

### How a command comes from the API

A command is the API method of the same name, mechanically: the method `pane.split` is `steg pane split`, its target parameter is the positional, and each other parameter is the flag of the same name with underscores turned into hyphens. A boolean parameter is a switch. Nothing is renamed by hand, so there is no table of names to drift, and a method the CLI does not expose is a failing test (R32).

A new command is therefore a new API method, named so that its noun and verb obey this document.

The description describes itself: `api.list` and `api.show` are methods like any other, so `steg api list` prints every method, one per line, and `steg api show <method>` prints one in full, with its parameters and documentation (R37). `steg api call <method>` calls any method by name: its parameters are read as one JSON object on standard input, and its result is printed as JSON. It is for agents, and for a method a plugin adds before the CLI knows it. It is never the documented way to do something that has a command of its own.

## 3. Nouns and verbs

### Nouns

| Noun | What it is | Release |
|---|---|---|
| `session` | A named, long-lived workspace on one host | M1 |
| `tab` | An ordered page within a session | M1 |
| `pane` | A region of a tab running one thing | M1 |
| `server` | The steg server on this host | M1 |
| `event` | Something the server reports as it happens | M1 |
| `api` | The API's own description, and a way to call any method | M1 |
| `client` | One connection to the server: a window of the app, a CLI call, a plugin | M1 |
| `setting` | A value the server remembers, overriding its default until changed again or until the server stops (decision-9). Not the configuration file | M1 |
| `config` | The user's configuration | M2 |
| `plugin` | A Python plugin the config lists | M2 |
| `action` | A named operation, built in or from a plugin | M2 |
| `status` | A program status record from OSC 7501 | M2 |
| `host` | A saved steg server on another machine | M3 |

### The closed set of verbs

Each noun takes the verbs that apply to it. Section 8 is the authoritative set for the first release.

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
| `tab declare-size` | Declare the caller's own size for the target tab, in columns and rows | Nothing |
| `pane send` | Send text or keys to the program in the target | Nothing |
| `pane capture` | Print the target's screen, and scrollback when asked | The text |
| `pane attach` | Open a stream to the target pane's screen | The stream's identifier and the opening snapshot, as JSON |
| `pane detach` | End this connection's attachment to the target pane | Nothing |
| `config reload` | Apply the configuration | Nothing |
| `action run` | Run the named action | What the action returns, if anything |
| `api call` | Call the named API method with parameters from standard input | The method's result, as JSON |

Adding a verb to either table is a change to this document.

## 4. Targets

A target names one session, tab or pane. It is a path of up to three parts, most specific last:

```
[session/][tab/]pane        for a pane
[session/]tab               for a tab
session                     for a session
```

- Each part is a full identifier, a label, a position (1, 2, ...), or the last four or more characters of an identifier (R10). They are tried in that order, and the first kind that matches is the answer, as in Rex.
- A label is matched without regard to case.
- A part left out comes from the context: `logs` is the pane labelled logs in the current tab, `2/logs` the same in tab 2 of the current session, `work/2/logs` in the session labelled work.
- With no target at all, the command acts on where it was run: the current pane, tab or session.
- A target that matches nothing, or more than one thing, fails with status 3 and lists the candidates on stderr. Nothing is done.
- A label may not contain `/`, and may not be all digits, so a path always reads one way.

For `new`, the target is the parent the new thing is created in. `steg tab new work` creates a tab in the session labelled work.

Focus is a path down the server's tree (doc-1, R85): `pane focus` makes the pane current in its tab and its tab current in its session, and `tab focus` makes the tab current in its session. Neither changes what any client views or which session a window shows (R88).

`session focus` is different in kind: which session a window shows belongs to the app, so it is a request to one client to show that session. With `--client` it goes to that client. Without, it goes to the app's client when exactly one is connected, as in Rex; with none, or more than one, it fails with status 3 and lists them. A client is named like anything else: by identifier, label, position or identifier suffix.

One exception: with `--direction`, the target of `pane focus` is the pane the move starts from, and the pane that gains focus is its neighbour. At the edge of the layout focus does not move and the command exits 0; `--wrap` continues from the other side.

### Context

Inside a steg terminal the server sets these, and the CLI reads them. They carry identifiers, never labels.

| Variable | Holds |
|---|---|
| `STEG_SERVER` | How to reach the server this terminal belongs to |
| `STEG_SESSION` | The session this terminal is in |
| `STEG_TAB` | Its tab |
| `STEG_PANE` | Its pane |
| `STEG_BIN_DIR` | The directory holding the server's own `steg` binary |

The server also puts that directory at the front of `PATH` in every terminal it opens, so the `steg` a program finds is the one that matches its server.

Outside a steg terminal there is no context, so a command that needs a target and is given none fails with status 2 and says so.

## 5. Flags

- Long flags are lowercase words joined by hyphens.
- A boolean flag is a switch: `--zoomed`, never `--zoomed=true`. A state that can be on has a `--no-` flag to turn it off, and both are always accepted.
- A flag that takes a value always takes one. `--flag value` and `--flag=value` both work.
- Flags may come before or after the target. A bare `--` ends the flags.
- A flag means the same thing on every command that has it.
- A property is a flag named after the property, accepted by `set` and `unset`, and listed per noun in section 8: `--label`, `--zoomed` and `--no-zoomed`, and in M2 `--colour`.
- The only short flags are `-h`, `-q` and `-v`.

### Flags every command has

| Flag | Meaning |
|---|---|
| `-h`, `--help` | Usage for this exact command, on stdout, status 0. Contacts no server |
| `--json` | Machine-readable output (section 6) |
| `-q`, `--quiet` | Print less, as in Rex: identifiers only. Where a command prints records, `-q` prints each one's full identifier alone on a line; where it prints one new identifier, that is unchanged; where it prints nothing, nothing. With `--json`, the identifiers are one JSON array |
| `-v`, `--verbose` | Diagnostics on stderr |
| `--no-color` | No colour. Colour is also off when stdout is not a terminal or `NO_COLOR` is set |
| `--no-input` | Never read from the terminal. This is already the default |
| `--autostart`, `--no-autostart` | Start a server that is not running, which is the default, or exit 4 without starting one |
| `--host <label>` | Act on a saved host in place of the one in the context (M3) |

`steg version` prints the version and contacts no server.

### Flags that recur

| Flag | On | Meaning |
|---|---|---|
| `--label <text>` | `new`, `set` | The thing's label |
| `--label` | `unset` | Clears the label. A switch with no value, as `--zoomed` is on `unset` (decided by Bruk on 2026-11-16 with TASK-1.49's plan; it took a value that was discarded before) |
| `--position <n>` | `new`, `move` | Where it goes among its siblings |
| `--to <target>` | `move` | The new parent |
| `--direction <left, right, up or down>` | `pane split`, `pane resize`, `pane focus` | Which way |
| `--cwd <path>` | `session new`, `tab new`, `pane split` | The new program's working directory |
| `--text <text>` | `pane send` | Text to send, in place of standard input |
| `--key <name>` | `pane send` | A key to send, by name. May be given more than once |
| `--scrollback` | `pane capture` | Include scrollback, not just the screen |
| `--for` | `wait`, `event watch` | For `event watch`, which events to keep: the names come from the API description's event records (R38). For `pane wait`, what to wait for: in M1 the only condition is `exit`, the pane's program ending; it is a condition of its own and not an event name. Either way the CLI checks the word before any request, and an unknown one is status 2 |
| `--timeout <seconds>` | `wait`, `watch` | Give up after this long, with status 1 |
| `--by <cells>` | `pane resize` | How far to move the border, in character cells |
| `--with <target>` | `pane swap` | The pane to exchange places with |
| `--all` | `event watch`, `client list` | Every session on the host, not just the target's; or command line clients as well |
| `--force` | `close`, `server stop` | Do it even though programs are still running |
| `--focus`, `--no-focus` | `session new`, `tab new`, `pane split` | Whether the new thing takes focus. It does by default |
| `--wrap` | `pane focus` | With `--direction`, continue from the other side at the edge |
| `--client <target>` | `session focus` | The client that is asked to show the session |

## 6. Input, output and exit status

- **stdout is the result and nothing else.** Diagnostics, warnings and progress go to stderr.
- **An action prints nothing on success.** A command that creates something prints its full identifier alone on one line, so `steg pane split` can be captured into a variable.
- **A list is one record per line, tab-separated, in a fixed column order, with full identifiers.** No header line and no alignment, ever: its bytes are the same piped or not. This is the one place steg deliberately does not follow Rex, which prints a table meant for reading with short identifiers.
- **Colour is the only thing that depends on where output goes.** It is off when stdout is not a terminal, when `NO_COLOR` is set, or with `--no-color`.
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

A pane started with a program stays when the program ends: its screen can still be captured, and `pane show` reports the exit code. A pane started as a plain shell closes when the shell exits. Rex closes both unless `--keep-open` is given; steg differs because the app's design keeps the output in front of the person (the design artifact, doc-10) and because a pane that disappears makes `pane wait` a race.

Closing something that is already closed is status 3, like any target that matches nothing. This departs on purpose from the team cli-design skill's default of status 0: the server cannot tell a pane that is gone from a name that was never right, and returning 0 would hide a typo in a script.

## 7. Speed

These are proposals until measured. TASK-1.3 measures CLI start-up for both ways of building the command tree, and its numbers replace these.

- `--help` and `steg version`: under 10 ms, with no server running.
- A command against a local server that is already up: under 20 ms at the 95th percentile, from start to exit.
- One request per command where one will do.
- No config is read by a command that does not need it, and the Python plugins are never started by the CLI.

## 8. The first release's commands

Generated from the API, so this table describes the API as much as the CLI. Each takes the flags in section 5 that apply.

| Command | Target | Notes |
|---|---|---|
| `session list` | none | Every session on the host |
| `session show` | session | |
| `session new` | none | `--label`, `--cwd`, `--focus`, and a program after `--` |
| `session close` | session | `--force` if programs are running |
| `session move` | session | `--position` |
| `session set`, `session unset` | session | `--label` |
| `session focus` | session | A request to a client to show it; `--client` when more than one window is connected |
| `tab list` | session | The tabs of that session |
| `tab show` | tab | |
| `tab new` | session | `--label`, `--cwd`, `--position`, `--focus`, and a program after `--` |
| `tab close` | tab | `--force` if programs are running |
| `tab move` | tab | `--position`, `--to` another session |
| `tab set`, `tab unset` | tab | `--label` |
| `tab focus` | tab | Makes it the current tab of its session |
| `tab declare-size` | tab | `--columns`, `--rows`, both required and above zero. The size is the calling client's for that tab; the terminal's size follows the rule of doc-15 R91, and each attached client is told the size in force (decided by Bruk on 2026-11-16 with TASK-1.114's plan) |
| `pane list` | tab | The panes of that tab |
| `pane show` | pane | Kind, size, working directory, title, running program, and its exit code once it has ended |
| `pane split` | pane | `--direction`, `--cwd`, `--label`, `--focus`, and a program after `--` |
| `pane close` | pane | `--force` if a program is running |
| `pane move` | pane | `--to` another tab |
| `pane set`, `pane unset` | pane | `--label`, `--zoomed` |
| `pane focus` | pane | Or `--direction` from the target, with `--wrap` |
| `pane resize` | pane | `--direction`, `--by <cells>` |
| `pane swap` | pane | `--with <target>` |
| `pane send` | pane | stdin, or `--text`, `--key` |
| `pane capture` | pane | `--scrollback` |
| `pane wait` | pane | `--for exit`, `--timeout` |
| `pane attach` | pane | Opens a stream to the pane's screen: the reply carries the stream's identifier and a snapshot, and screen updates follow on that stream. The exception to R32 |
| `pane detach` | pane | Ends this connection's attachment to the pane |
| `server show` | none | Version, uptime, sessions, clients |
| `server start` | none | Stays up with no clients. With `--on-demand` it is the kind a client starts when it finds no server: it stops itself after 30 seconds with no sessions and no clients (doc-8 R99). `steg server start --on-demand` is what the sdk's local-socket provider runs |
| `server stop` | none | `--force` if sessions exist |
| `event watch` | session | `--for <event>` to filter on the server; all sessions with `--all`. The only stream in M1 |
| `client list` | none | Every client connected to the server. Command line clients only with `--all` |
| `client show` | client | What it is, and the sessions it views |
| `setting list` | none | Every setting, its current value and its default |
| `setting show` | setting | One setting: its name, current value and default |
| `setting set` | setting | `--value <text>`. The setting is named by the positional and its new value is the flag, since here the property is the thing addressed |
| `setting unset` | setting | Clears the override, so the setting returns to its default |
| `api list` | none | Every method, one per line |
| `api show` | method, or none | One method in full. With no target, the whole description |
| `api call` | method | Parameters as JSON on standard input |
| `version` | none | Not an API method; no server |
| `completion` | shell | Not an API method; no server |

Later releases add `config check`, `config reload`, `plugin list`, `action list`, `action run`, `status list`, `session set --colour`, promoting a pane's status to its tab (TASK-1.30), and the `host` noun.

Showing a file in a pane (R63 to R68, M2) adds no verb: `pane split --file <path>` runs the viewer the configuration maps to that file's type; `--tab` puts it in a new tab; `--no-focus` is the default when `--file` is given; `--reuse`, the default, prints the identifier of a pane in the same tab that already shows that file and leaves it alone; `pane set --file <path>` replaces a viewer's content; `--file -` reads the content from standard input, with `--type <name>` naming its type. `--file` and a program after `--` together are status 2.

## 9. Decided, and still to decide

Decided by Bruk on 2026-11-13:

- **A program to run is given after `--`.** `steg pane split -- htop -d 5`, and the same on `session new` and `tab new`. It is the one exception to a single positional, and follows `env`, `ssh` and `sudo`.
- **`close` and `server stop` refuse while programs are still running,** with status 1 and a message naming `--force`. With `--force` they go ahead.
- **A target's parts are separated by `/`,** as in `work/2/logs`.

Decided by Bruk on 2026-11-15. The rule: steg keeps its own grammar and words, and wherever this document had not decided, steg does what Rex does. decision-4 has each question in full.

- **Closing something already closed** is status 3 (section 6).
- **Lists** are tab-separated, with full identifiers, no header and no alignment, the same piped or not (section 6). Not as Rex.
- **Short flags** are `-h`, `-q` and `-v` only (section 5).
- **`-q` prints identifiers only** (section 5). As Rex.
- **`version` and `completion` are subcommands** (section 2). As Rex.
- **`api list`, `api show` and `api call`** exist (section 2). As Rex.
- **A target's parts are tried in a fixed order,** and labels ignore case (section 4). As Rex.
- **A new session, tab or pane takes focus,** with `--focus` and `--no-focus` (section 5).
- **`pane focus --direction` stays put at the edge,** with `--wrap` (section 4). `--wrap` as Rex.
- **A pane started with a program stays when it ends;** a plain shell pane closes (section 6). Not as Rex.
- **`event watch` is the only stream in M1** (section 8).
- **`--autostart` and `--no-autostart`** are on every command (section 5). As Rex.
- **The server puts its own directory first on `PATH`,** and sets `STEG_BIN_DIR` (section 4). As Rex.
- **Showing a file in a pane** is flags on `pane split` and `pane set` (section 8), in M2.
- **Saved hosts** are left for M3.
- **Focus is a path in the server's tree; a window is the app's** (section 4, and doc-1 R85 to R90). `pane focus` and `tab focus` set the path. `session focus` asks a client to show a session, and there is a `client` noun. As Rex, as far as its documentation shows: its focus commands name no client, and what it asks of the app is addressed to one.

Two details were filled in by the orchestrator to make the answers complete, and are Bruk's to change: that `-q` with `--json` prints the identifiers as a JSON array, and that `api call` takes its parameters as JSON on standard input.

## 10. Where this differs from Rex

Checked on 2026-11-14 against Rex's own documentation (the pages are listed in decision-4, part C), which describes a CLI in beta. Rows marked "the same" are where steg follows Rex.

| | Rex | steg | Why |
|---|---|---|---|
| Spellings | A full noun and verb tree, and short forms over it: `rex new` and `rex session new` are one command, as are `rex ls` and `rex session ls` | One spelling for each command | It is what lets the CLI be generated from the API with no table of names |
| Verb set | Two words for one meaning (`ls` and `list`, `kill` and `close`), plus `inspect`, `rename`, `zoom`, `types`, `add`, `remove` | One verb per meaning, from a closed set | The same reason |
| Toggles | `rex block zoom` toggles when neither `--on` nor `--off` is given | `pane set --zoomed` and `pane set --no-zoomed` | A toggle's result depends on state the script cannot see |
| Targets | A flag per level: `-s`, `-w`, `-b`, `-C` for a client, `-S` for a server | One path as the positional, with `--client` and `--host` | One way to write a target |
| Clients | `rex client ls`, `rex client inspect`; `-C` or `REX_CLIENT`; the app's client is used when exactly one is connected | `client list`, `client show`; `--client`; the same rule. No `STEG_CLIENT`: a pane is not tied to one client | |
| Target resolution | Identifier, label, position, then identifier suffix; labels ignore case | The same | |
| Words | `window` and `block` | `tab` and `pane` everywhere | The same words as the app (doc-1, section 4) |
| Scripting | `rex do` runs an action, a script or inline Lua | No scripting in the CLI; Python plugins use the API | Python runs beside the server, never inside the binary (R72) |
| Context | `REX_SESSION`, `REX_BLOCK`, `REX_SERVER`, `REX_CLIENT`, `REX_BIN_DIR` | `STEG_SESSION`, `STEG_TAB`, `STEG_PANE`, `STEG_SERVER`, `STEG_BIN_DIR` | Adds the tab, so a relative target never needs a lookup |
| Finding the binary | Its own directory first on `PATH` | The same | |
| Exit statuses | 0, 1, 2, 3, 4, 130 | The same | |
| Waiting | `rex wait` and `rex run --wait` exit with the program's own status; `--for DURATION` is the time limit | `pane wait` exits 0 and prints the program's code; `--for <event>` names the condition and `--timeout` is the limit | A script can tell "the wait failed" from "the program failed". The different meaning of `--for` is a trap for anyone porting a Rex script |
| Lists | A table meant for reading, short identifiers by default | Tab-separated, full identifiers, no header | The same bytes piped or not |
| `--json` | `rex ls --json` prints an object keyed `sessions` | `list` prints one array | |
| `-q` | Identifiers only | The same | |
| The API over the CLI | `rex api list`, `rex api describe`, `rex api call` | `api list`, `api show`, `api call` | `show` is steg's verb for one thing in full |
| Completion and version | `rex completion zsh`, `rex version` | The same | |
| Starting the server | `--autostart`, on by default | The same | |
| When a program ends | The pane closes unless `--keep-open` | A pane started with a program stays; a shell pane closes | The app's design, and `pane wait` |
| Capture | Screen and scrollback together, `--format` of text, html or vt, `--trim`, `--unwrap` | The screen as text, scrollback with `--scrollback` (R35) | R35 asks for text; the rest is M2 at the earliest |
| Events | `rex events` streams and the caller filters | `event watch --for <event>` filters on the server | Less traffic, and one fewer tool in an agent's loop |
| Pane kinds | `rex block new FLAVOR`, `rex block types` | Every pane is a terminal; a viewer is a program in one (doc-1 section 5.14) | One rendering path, and R12 keeps the door open |
| Port forwarding | `rex forward new, open, share, ls, close` | Out of scope (doc-1 section 10) | SSH is the only remote transport |

## Open questions

- [ ] `-q` together with `--json` printing the identifiers as a JSON array, and `api call` taking its parameters as JSON on standard input, were filled in by the orchestrator and are Bruk's to change.
- [ ] The speed numbers in section 7 are proposals until measured.
- [ ] Saved hosts (doc-13).

## Out of scope

- Scripting inside the CLI. Python plugins use the API (doc-12).
- Aliases for Rex's verbs. They can be added later as a layer if compatibility matters (decision-4).
