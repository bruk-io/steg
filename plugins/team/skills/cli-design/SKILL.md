---
description: How the team designs a command-line interface - noun-verb grammar, a closed set of verbs, one positional for the primary target, boolean flags as switches with --no- pairs, universal flags, data-only stdout, and never prompting. Load this before designing, adding, changing, reviewing or advising on any CLI command, flag, output format or exit status, even a single flag. Without it, agents invent a new verb for an action that already has one, add positionals for things that should be flags, make booleans take values, print decoration on stdout, and add a confirmation prompt that hangs a script.
---

# CLI design

- Owner: Bruk Habtu
- Source: Bruk's direction of 2026-11-13, and each project's own CLI specification where it has one (in steg, the "CLI specification" document on the board).
- Enforced by: advisory, except where a project generates its commands from one description and tests that nothing is written by hand. A project's CLI specification outranks this skill where the two differ.

A CLI is used by scripts and agents far more often than by a person at a prompt. So it is designed for three things, and when they pull apart they rank in this order:

1. **Predictable.** The same grammar everywhere, so a command that was never read can be guessed. No behaviour that depends on who is watching.
2. **Composable.** Each command does one thing, takes its input and gives its output in a form the next program can use.
3. **Fast.** It starts and answers quickly enough to sit in a loop or a key binding.

## Grammar

The shape is always: program, noun, verb, then the target, then flags.

- **Noun, then verb.** The noun is the kind of thing; the verb is what is done to it. Every command has both. A bare noun lists that noun's verbs.
- **Nouns are singular** and name something the user already has a word for.
- **Verbs come from a closed set.** One verb per meaning, used for every noun it applies to. Before adding a verb, look for the one that already means it. The set the team starts from:

  | Verb | Meaning |
  |---|---|
  | `list` | Every one of the noun, one per line |
  | `show` | One of the noun, in full |
  | `new` | Create one |
  | `close` | End one, and whatever runs in it |
  | `rename` | Change its label |
  | `move` | Change its position or its parent |
  | `set`, `unset` | Change or clear one named property |
  | `start`, `stop` | A long-running thing's lifecycle |
  | `check` | Validate without changing anything |
  | `watch` | Stream changes until interrupted |
  | `wait` | Block until a named condition, then exit |

  Never `ls` beside `list`, `create` beside `new`, or `rm`, `delete` and `kill` beside `close`. A verb that only makes sense for one noun is allowed, listed in the project's specification, and still follows every rule here.
- **No aliases and no abbreviations.** Two spellings of one command are two things to learn and to keep working.

## Arguments

- **The one positional is the primary target:** the thing the verb acts on. A command has at most one.
- **The target is optional when the context supplies it,** such as the thing the command was run inside. An explicit target always wins over the context.
- **Everything else is a flag:** modifiers, options, and any second value. If a command seems to need two positionals, the second is a flag.
- **Content comes from standard input,** not from a positional. A flag may supply short content inline. A lone `-` where a file is expected means standard input.
- **A target that matches nothing, or more than one thing, is an error.** Never guess, and never act on the first match.

## Flags

- **Long flags are lowercase words joined by hyphens.** A short form exists only for the universal flags below and for a handful used constantly.
- **A boolean flag is a switch.** Its presence means yes: `--force`, never `--force=true`.
- **A default that is on is turned off with a `--no-` flag:** `--no-color`, `--no-input`. Both spellings of a pair are accepted, so a script can state the default explicitly.
- **A flag that takes a value always takes one.** No flag has an optional value. Both `--flag value` and `--flag=value` work.
- **Flags may come in any order, before or after the target.** A bare `--` ends the flags.
- **The same flag means the same thing on every command.** Check the project's specification before naming one.

Every command accepts these:

| Flag | Meaning |
|---|---|
| `-h`, `--help` | Usage for this exact command, on stdout, exit 0. Makes no connection and changes nothing |
| `--version` | The program's version, on the program itself |
| `--json` | Machine-readable output, in the documented shape |
| `-q`, `--quiet` | No output on success |
| `-v`, `--verbose` | More diagnostics, on stderr |
| `--no-color` | No colour. Also off when stdout is not a terminal or `NO_COLOR` is set |
| `--no-input` | Never read from the terminal. This is the default; the flag exists so a script can say so |

## Output

- **stdout carries the result and nothing else.** Diagnostics, progress and warnings go to stderr.
- **An action that succeeds prints nothing,** unless it made something, in which case it prints that thing's identifier alone on one line so it can be captured.
- **A list is one record per line with stable columns,** in a stable order. The shape does not change when output is piped; only colour and alignment do.
- **`--json` is a contract:** full identifiers, no abbreviation, fields only ever added. A single result is one document; a stream is one document per line.
- **Times are ISO 8601 in UTC.** Sizes and durations carry their unit.
- **No pager, no spinner, no banner, no hint about a newer version.**

## Behaviour

- **Never prompt.** A command that would need to ask fails, saying which flag answers the question.
- **Destructive commands act on exactly the named target.** `--force` is for the case where a safe default refuses, and the refusal says so.
- **Exit status is part of the interface.** A script must be able to tell success, failure, bad usage, a missing or ambiguous target, an unreachable service and an interruption apart without reading text.
- **An error says what was wrong and what to do,** on one line where it can, naming the argument at fault.
- **Running a command twice should be safe where it can be:** closing what is already closed is not an error worth failing a script over, unless the project's specification says otherwise.
- **Context comes from documented environment variables,** never from hidden state.

## Speed

- Help, version and completion never start or contact a service.
- A command does one round trip where one will do.
- Nothing is loaded that the command does not use: no network call, no update check, no config parsed for a command that needs none.
- The project's specification gives the numbers. Measure against them when a command's start-up path changes.

## Checking it

For a new or changed command, confirm each of these, and say on the task which you checked and how:

- Its noun and verb are in the project's specification, or the change adds them there.
- It has at most one positional, and that is the target.
- Every boolean is a switch, and any default-on behaviour has its `--no-` flag.
- `--help` works with nothing else running, and `--json` output matches the documented shape.
- Run it with stdout piped to a file: the file holds only the result.
- Run it with a target that matches nothing and one that matches two things: each fails with the documented status.
