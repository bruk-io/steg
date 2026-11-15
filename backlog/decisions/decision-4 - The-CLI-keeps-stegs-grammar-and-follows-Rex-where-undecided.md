---
id: decision-4
title: The CLI keeps steg's grammar and follows Rex where undecided
date: '2026-11-15T01:11:00Z'
status: accepted
---
## Context

The CLI specification left sixteen points open, some of them its own and some found when it was reviewed against the spec and against Rex's documentation. A spike, the archived TASK-1.4, wrote them up for Bruk on 2026-11-14. Bruk considered making the CLI the same as Rex's and set that aside. This was moved here from the board document doc-3 on 2026-11-15, unchanged; the answers are at its top.

## Decision

Decided by Bruk on 2026-11-15: steg keeps its own grammar and its own words (a noun then a verb, tab and pane, one path as the target), and wherever the CLI specification had not decided, steg does what Rex does. Under that rule thirteen of the sixteen points were settled at once, and Bruk answered the other three: lists are not as Rex's (tab-separated, full identifiers, no header); `-q` is as Rex's (identifiers only); and there is an `api call`, as Rex has.

## Consequences

- Every answer is applied to the CLI specification, which is the only document a builder reads.
- A copy of Rex's interface would have undone the generated grammar, put "block" in the CLI while the app says "pane", and needed Lua. A layer of aliases mapping Rex's verbs onto steg's commands can be added later if compatibility matters.
- Two details were filled in by the orchestrator and are Bruk's to change: `-q` with `--json` prints the identifiers as a JSON array, and `api call` takes its parameters as JSON on standard input.

## The record: what was asked, and the answers

- Task: TASK-1.4, "Spike: propose the CLI design for Bruk to decide".
- Spec: doc-1 (R32 to R35, R63 to R68). CLI specification: doc-2. Policy: the team's
  cli-design skill (`plugins/team/skills/cli-design/`).
- Status: a proposal against doc-2, not a second specification. Part A is for Bruk to
  answer; part B is the edit list the orchestrator then applies to doc-2. Once applied,
  this document is the record of what was asked and why, and doc-2 remains the only
  place a builder reads.

doc-2 already carries the grammar, the verbs, the targets, the context variables, the
flags, the output rules, the exit statuses, the first release's commands and the Rex
comparison, so this task is the remainder: the points doc-2 leaves open, the points its
review turned up, and the Rex comparison checked against Rex's own documentation.

## Answers, 2026-11-15

Bruk set one rule on 2026-11-15: steg keeps its own grammar and its own words (noun then
verb, tab and pane, one path as the target), and **wherever doc-2 has not decided, steg
does what Rex does.** A 1:1 copy of Rex's interface was considered and set aside, because
it would undo the generated grammar (doc-4), put "block" in the CLI while the app says
"pane", and need Lua for `rex do`. A set of aliases that maps Rex's verbs onto steg's
commands can be added later if compatibility matters.

@bruk.o applied that rule to the sixteen points below. Thirteen are settled by it, or by
the app design Bruk accepted the same day (doc-7). Three are left for Bruk because the
rule and doc-2's own principles pull in different directions.

| Point | Answer | Settled by |
|---|---|---|
| A1 | Status 3 when closing something already closed, as doc-2 proposes | doc-2 had proposed it; Rex's behaviour is not documented here |
| A3 | `-h`, `-q` and `-v` only | steg's design: Rex's other short flags belong to its flag-per-level targets, which steg does not have |
| A4 | Subcommands, as Rex has them: `steg completion <shell>` and `steg version`, with `--help` as the one bare flag. Section 2 states the exception: these are not API methods and work with no server | Rex |
| A5 | As recommended: `pane split --file`, `--tab`, `--focus`, `--reuse`, `pane set --file`, `--file -` with `--type`. No `view` noun and no pane flavour | steg's design (doc-1 section 5.14); M2 |
| A6 | Deferred to M3 | doc-2 |
| B1 | A new pane, tab or session takes focus; a viewer does not; `--focus` and `--no-focus` on all three | The app design, doc-7 D45 |
| B3 | Rex's order: full identifier, then label, then position, then an identifier suffix of four or more characters, stopping at the first kind that matches. Labels match without regard to case, as Rex's do. The ban on all-digit labels stays | Rex |
| B5 | `event watch` is the only stream in M1, with `--for` and a target | doc-3's recommendation; Rex's behaviour is not documented here |
| B6 | `pane focus --direction <left, right, up or down>`. At the edge focus does not move and the command exits 0. `--wrap` continues from the other side, as Rex's does | steg's grammar for the shape; Rex for `--wrap` |
| B7 | A pane started with a command stays when the command ends, and `pane show` reports its exit code. A pane started as a plain shell closes when the shell exits. This differs from Rex, which closes unless `--keep-open` is given | The app design, doc-7 D91 to D93 |
| B8 | `--autostart` and `--no-autostart` on every command, `--autostart` the default; `--no-autostart` exits 4 | Rex |
| B9 | The server puts the directory of its own binary at the front of PATH in every terminal it opens, and sets `STEG_BIN_DIR` | Rex |

Answered by Bruk on 2026-11-15:

| Point | Answer |
|---|---|
| A2 and B10 | Not as Rex: a list is tab-separated, with full identifiers, no header and no alignment, the same piped or not |
| B2 | As Rex: `-q` prints identifiers only, one per line |
| B4 | As Rex: `api list`, `api show` (steg's verb for Rex's `describe`), and `api call` |

All sixteen answers, and the ten edits of part B, were applied to doc-2 on 2026-11-15. Three of part B's edits changed with the answers: edit 5 (`-q` with `--json` now prints the identifiers as a JSON array, since `-q` no longer means silence), edit 8 (section 10 is part C brought up to date with these answers), and edit 10 (`version` and `completion` are subcommands, so only `--help` is a flag).

## A. Decisions for Bruk

Each one: the question, what is recommended, the alternative, and what it costs to
choose wrong. Answering "all as recommended except A2 and B7" is enough.

A1 to A6 are doc-2 section 9's own open points. B1 to B10 came out of reviewing doc-2
against doc-1 and against Rex, and have not been put to you before.

### A1. Closing something that is already closed

- **Recommended:** status 3, as doc-2 proposes. The server cannot tell a pane that is
  gone from a name that was never right, so returning 0 would hide a typo in a script.
- **Alternative:** status 0, which is the team cli-design skill's default ("closing what
  is already closed is not an error worth failing a script over, unless the project's
  specification says otherwise"). The skill permits this override; doc-2 has to say that
  it is one, and why. That is edit 6.
- **Cost of the wrong choice:** low either way. A script that wants idempotence ignores
  status 3 in one line.
- **Blocks:** TASK-1.13 acceptance criterion 3, which tests every exit status.

### A2. A header line on lists

- **Recommended:** never print one. doc-2 section 1 defines predictable as "no behaviour
  that depends on whether a person is watching", and a header that appears only on a
  terminal is exactly that. Never printing one makes a list's bytes identical piped or
  not, and a person who wants names has `--json` or `show`.
- **Alternative:** doc-2's proposal, a header only when stdout is a terminal. More
  readable at a prompt, and it contradicts the document's own first principle.
- **Cost of the wrong choice:** every script that reads a list has to know whether to
  skip a line, and the answer depends on where it runs. That is the expensive mistake,
  so it argues for never.
- **Note:** Rex does the readable thing: "Most commands print a table meant for reading",
  and its lists show short IDs by default. steg's section 6 chooses tab-separated full
  identifiers instead, which already favours the script over the reader.
- **Related:** B10, whether columns are aligned on a terminal. Answer them together.

### A3. Short flags

- **Recommended:** `-h`, `-q` and `-v` only, as doc-2 proposes, which is what the skill
  allows. Everything else is long.
- **Alternative:** add a short form for the flags used constantly. Rex went much
  further: `-s`, `-w`, `-b`, `-C`, `-S`, `-A`, a short form for `--cwd`, and `-q`, `-v`.
- **Cost of the wrong choice:** adding one later is a doc-2 edit and a new generated
  alias; removing one is a break. So starting small is the reversible direction.

### A4. Shell completion

- **Recommended:** `steg --completion <shell>`, as doc-2 proposes, and close the set of
  bare-program flags at three: `--help`, `--version`, `--completion`. The reason is
  worth writing into section 2: these are the only commands that are **not** API
  methods, and they must work with no server, so they cannot be a noun and a verb
  without inventing a server method for shell completion.
- **Alternative:** Rex's shape, which makes them subcommands: `rex completion bash`,
  `rex completion zsh`, `rex completion fish`, `rex completion powershell`,
  `rex completion nu`, and `rex version`. It reads better and it breaks doc-2's rule that
  a command is an API method, so section 2 needs an exception clause either way.
- **Cost of the wrong choice:** cosmetic, but it sets the precedent for every future
  local-only command (a doctor command, a terminfo installer, an agent-skill installer:
  Rex has all three).

### A5. Showing a file in a pane (R63)

doc-2's one-line proposal (`steg pane split --file <path>`) carries R63 and none of the
requirements around it. R64 says focus stays where it was unless asked for; R67 says
opening a file already shown in the same tab reuses that pane, that a viewer's content
can be replaced with another file, and that the pane can be closed; R68 says content can
come in on stdin with its type named.

- **Recommended**, adding no verb:
  - `steg pane split --file <path>` runs the viewer the configuration maps to that
    file's type (R69) in the new pane. The target is the pane to split, so with no
    target it is the pane the command ran in, which is what R63 asks for.
  - `--tab` puts it in a new tab instead (R63).
  - `--focus` and `--no-focus`, with `--no-focus` the default when `--file` is given, so
    an agent showing a document does not interrupt typing (R64). See B1: the default for
    an ordinary split is a separate question.
  - `--reuse` and `--no-reuse`, with `--reuse` the default: if a pane in the same tab
    already shows that file, print its identifier and leave it alone (R67).
  - `steg pane set --file <path>` replaces a viewer's content, because the file a viewer
    shows is a property of the pane, which is what `set` is for (R67). `steg pane close`
    closes it; no new verb for that either.
  - `--file -` reads the content from stdin, with `--type <name>` naming its type (R68),
    following the skill's rule that a lone `-` where a file is expected means stdin.
  - `--file` and a program after `--` are mutually exclusive, status 2: they are two ways
    of saying what the pane runs.
- **Alternative:** a `view` noun of its own, or a pane kind the app draws. doc-1 section
  5.14 already rejected the second ("a viewer is a terminal program that steg runs in an
  ordinary pane"), and the first duplicates `pane`, since a viewer is a pane.
- **Cost of the wrong choice:** this is M2, and R65 to R71 are built on top of it. Six
  flags on verbs that already exist is the cheapest thing to change later.
- **Difference from Rex:** Rex would do this as a block flavour (`rex block new FLAVOR`,
  `rex block types`). steg keeps every pane a terminal.

### A6. Saved hosts (M3)

- **Recommended:** defer, as doc-2 does, but the question is narrower than it looks. R55
  says a remote host is reached "using the user's existing SSH config, keys, and agent",
  so steg may not create a host at all, only label one. If labelling is all it does,
  `host list`, `host show`, `host set` and `host check` cover it and the closed verb set
  survives untouched.
- **Alternative:** Rex's shape, `rex hosts add LABEL ENDPOINT`, `rex hosts remove LABEL`,
  `rex hosts edit LABEL`, `rex hosts check`, `rex hosts list`, with a plural noun and two
  verbs steg does not have.
- **Cost of the wrong choice:** none now. It is one row of section 3 at M3.

### B1. Does a new pane take focus?

doc-2 says nothing about it, for `pane split` or for `tab new`, and R64 depends on the
answer for viewers.

- **Recommended:** a split takes focus (it is what a person at a prompt means), a viewer
  does not (R64), and both are controllable with `--focus` and `--no-focus` on
  `pane split`, `tab new` and `session new`.
- **Alternative:** never take focus unless asked. More predictable for scripts, more
  surprising for a person.
- **Cost of the wrong choice:** an agent that splits a pane steals the user's keyboard
  mid-sentence, which is the thing R64 exists to prevent.
- **Blocks:** TASK-1.13, TASK-1.15.

### B2. Does `-q` suppress a created identifier?

doc-2 section 5 says `--quiet` prints "nothing on success, not even a new identifier";
section 6 makes that identifier the point of `steg pane split`. Both sentences cannot
stand.

- **Recommended:** keep section 5. `-q` means nothing on stdout, and a script that wants
  the identifier simply does not pass `-q`. One rule, no exceptions to remember.
- **Alternative:** Rex's meaning, where `-q` prints *less*: "Print IDs only, one per
  line" on `rex ls`, and "Print method names only" on `rex api list`. That makes `-q` a
  second output format beside `--json` rather than a silence switch.
- **Cost of the wrong choice:** small, but it is a break to change later: a script
  written against one meaning breaks silently under the other, since both exit 0.
- **Note:** if the answer is Rex's, `-q` needs a documented shape per command, which is
  a third output contract beside the table and `--json`. That argues for the
  recommendation.

### B3. How is a target part resolved when it could be two things?

doc-2 section 4 says a part is "a full identifier, a label, a position (1, 2, ...), or
the last four or more characters of an identifier", and bans all-digit labels so a label
cannot look like a position. It does not say what happens when a four-character
identifier suffix is all digits, which can also look like a position. R10 says an
ambiguous target is an error and never a guess.

- **Recommended:** adopt the order Rex documents, which is the obvious one, and write it
  into section 4: a full identifier, then a label, then a position, then an identifier
  suffix of four or more characters, stopping at the first kind that matches. Then the
  ban on all-digit labels is no longer needed for disambiguation, though it is still
  worth keeping so a path reads one way to a person. Also say whether a label is matched
  case-insensitively; Rex's is.
- **Alternative:** keep the ban and make any part that could be read two ways status 3.
  Safer, and it fails on input a person reasonably expects to work.
- **Cost of the wrong choice:** this is the rule TASK-1.7 implements and TASK-1.13 tests.
  Getting it wrong means a script that worked yesterday acts on the wrong pane today,
  which is the worst failure in this document.
- **Blocks:** TASK-1.7, TASK-1.13.

### B4. Is there a command that prints the API's own description?

R37 says a client can ask for every method, its parameters and its documentation; R32
says the CLI exposes everything the API exposes; R62 generates the reference docs from
the description. No command in doc-2 section 8 does it, and `server show` reports only
version, uptime, sessions and clients.

- **Recommended:** add the noun `api` with `api list` (every method, one per line) and
  `api show <method>` (one method in full), generated from the description like
  everything else. It is how the docs are generated (R62), how a plugin author reads the
  surface (R42, R75), and it is the one thing that proves R32 to a person. Both are API
  methods like any other, `api.list` and `api.show`, so the description describes itself.
  That is normal, it is what Rex does, and it is the one place where section 2's rule
  that a command is a method is worth saying out loud.
- **Alternative:** state the exemption in section 2, that the description method has no
  command. Then R32's "everything the API exposes" needs the word "except".
- **Cost of the wrong choice:** TASK-1.13's drift test has to carry a special case, which
  is exactly the kind of hand-written exception section 2 exists to avoid.
- **Difference from Rex:** Rex has all three: `rex api list`, `rex api describe METHOD`,
  `rex api call METHOD`. steg should not copy `api call`: it is a hole through the
  generated grammar.
- **Blocks:** TASK-1.10, TASK-1.13.

### B5. Can one session's or one pane's changes be streamed, or only `event watch`?

Section 8 gives `watch` to `event` alone, so an agent watching one pane filters the
stream itself.

- **Recommended:** keep `event watch` as the only stream in M1, with `--for <event>` and
  a target. One streaming command, filtered server-side, is less to build and less to
  learn, and the target already narrows it to a session.
- **Alternative:** `session watch`, `tab watch` and `pane watch` as well, which is what
  section 3's table implies today.
- **Cost of the wrong choice:** TASK-1.13's drift test needs to know which method and
  command pairs must exist. Without an answer it cannot be written.
- **Blocks:** TASK-1.11, TASK-1.13.

### B6. `pane focus --direction`, and what happens at the edge

Section 4 says the target is the thing the verb acts on, but focusing by direction acts
on the target's *neighbour*. R9 asks for focus by direction, so the question is only how
doc-2 writes it down. Nothing says what happens at the edge of a layout.

- **Recommended:** keep `pane focus --direction <left, right, up or down>`, and write the
  exception into section 4 in one line: with `--direction`, the target is the pane the
  move starts from, and the pane that gains focus is its neighbour. At the edge, focus
  does not move and the command exits 0. No wrapping in M1.
- **Alternative:** Rex's shape, `rex focus TARGET|DIRECTION`, where one positional is
  either, plus `--wrap` to continue from the other side. It needs no exception clause and
  it makes the positional two kinds of thing, which doc-2's grammar forbids.
- **Cost of the wrong choice:** low. Wrapping can be added later as `--wrap`.

### B7. Does a pane close when its program exits?

Nothing in doc-1 or doc-2 says. It decides what `steg pane split -- make test` leaves
behind, whether `pane wait` can still read an exit code, and whether a viewer's pane
survives its viewer (R67's reuse depends on the pane still being there).

- **Recommended:** the pane stays until it is closed, and `pane show` reports the
  program's exit code (R16 already requires the server to know it). An agent that wants
  the output can still capture the screen after the program ended.
- **Alternative:** the pane closes when its program exits, with a flag to keep it. Rex
  does it this way, with `--keep-open` ("Keep the terminal open after its command
  exits"), and its own documentation shows the cost: `rex wait` "returns 1" when "a
  terminal closes before reporting a child exit".
- **Cost of the wrong choice:** a disappearing pane loses the output an agent was about
  to read, and makes `pane wait` racy. That is the reason for the recommendation.
- **Blocks:** TASK-1.8, TASK-1.14.

### B8. May a command refuse to start the server?

R3 starts the server on demand, and status 4 covers "the server could not be reached or
started". There is no way to ask a question without starting one, so a script that only
wants to know whether steg is running starts it by asking.

- **Recommended:** add `--autostart` and `--no-autostart` to the universal flags, with
  `--autostart` the default. `--no-autostart` exits 4 instead of starting a server.
- **Alternative:** leave it, and let `server show` be the one command that does not start
  a server. That is a special case in section 5 rather than a flag.
- **Cost of the wrong choice:** small, but it bites in exactly the place steg cares about:
  a shell prompt or a key binding that asks about steg on a machine where it is not
  running.
- **Difference from Rex:** Rex has it as a global option, `--autostart`, "Start a missing
  local server. Defaults to true."

### B9. How does `steg` get onto PATH inside a steg terminal?

R33 says the CLI acts on the terminal's own session and pane "from context the
terminal's environment provides", and doc-2 section 4 lists the four `STEG_` variables.
Nothing says the binary is reachable, and an agent in a pane cannot use a CLI it cannot
find.

- **Recommended:** the server puts the directory holding its own binary at the front of
  PATH in every terminal it opens, and sets `STEG_BIN_DIR` to that directory, so a
  mismatch can be diagnosed. One row in section 4's table and one line in doc-2 beside
  R33.
- **Alternative:** leave it to the user's own PATH. Then a steg built from source, or two
  versions on one machine, silently drives the wrong server (R39's version check would
  catch it at least).
- **Difference from Rex:** Rex does exactly the recommended thing, with `REX_BIN_DIR`,
  and documents the failure mode at length, including shells that rebuild PATH from a
  fixed list.
- **Blocks:** TASK-1.8 or TASK-1.9, whichever sets a terminal's environment.

### B10. Are list columns aligned when stdout is a terminal?

The skill says only colour and alignment change when output is piped. doc-2 section 6
says a list is tab-separated and that "piping changes nothing else", and never mentions
alignment.

- **Recommended:** no alignment, ever. Tab-separated in both cases, so `cut`, `awk` and a
  person all see the same thing, and the only terminal-dependent behaviour left in the
  CLI is colour.
- **Alternative:** align on a terminal. Then two of the three terminal-dependent
  behaviours stay, and section 1's principle needs the exception written down twice.
- **Answer with A2.**

## B. Edits to doc-2 that need no decision

Each is a defect in doc-2 against its own rules, doc-1, or the cli-design skill. They are
listed in doc-2's section order, and the ones that depend on an answer above say so.

1. **Section 3, the heading "Verbs every noun shares".** It is not true and not what the
   skill says: `wait` is `pane`'s alone, `watch` is `event`'s, `start` and `stop` are
   `server`'s, `check` is M2's. Retitle it "The closed set of verbs" and add one
   sentence: each noun takes the verbs that apply to it, and section 8 is the
   authoritative set for the first release. The mechanical check in the appendix confirms
   every command in section 8 already has a noun and a verb section 3 lists, so this is
   wording, not coverage.
2. **Section 5, property flags.** `--zoomed`, `--no-zoomed` and (M2) `--colour` are used
   in sections 3 and 8 and defined in neither table (appendix, check 2). Add a sentence:
   a property is a flag named after the property, accepted by `set` and `unset`, and
   listed per noun in section 8. `--file` and `--completion` are correctly undefined
   until A5 and A4 are answered.
3. **Section 1 and section 6, the colour exception.** Section 1 forbids behaviour that
   depends on whether a person is watching; section 6 makes colour depend on it. Write
   colour down as the one exception, and name the convention it follows (`NO_COLOR`, and
   off when stdout is not a terminal). With A2 and B10 answered as recommended, it
   becomes the only one.
4. **Section 5, `--for <event>`.** No event name appears anywhere in doc-2. Say that the
   names come from the API description's event records (R38), that the CLI validates the
   name against them before any request, and that an unknown name is status 2. Nothing is
   named by hand, which is section 2's rule. Depends on TASK-1.3's format and TASK-1.11's
   events for the list itself.
5. **Section 6, `--json` with `--quiet`.** Undefined today. Say `--quiet` wins and
   nothing is printed, which follows from "print nothing on success".
6. **Section 9 point 1.** Mark it as a deliberate departure from the cli-design skill and
   give the reason, whichever way A1 goes. It reads today as though no policy applies.
7. **Section 4, case.** Say whether a label is matched case-insensitively. Rex's is. Part
   of B3.
8. **Section 10.** Replace with part C, which is checked against Rex's documentation.
9. **Section 7.** It already names TASK-1.3; add that TASK-1.3's measured start-up numbers
   replace the first two bullets, and that until then they are proposals, as doc-1 R19
   does for the paint budget.
10. **Section 2.** Add the sentence A4 needs either way: `--help`, `--version` and (if A4
    goes as recommended) `--completion` are the only commands that are not API methods,
    which is why they are flags on the bare program.

## C. Where this differs from Rex, checked against Rex's documentation

Checked on 2026-11-14 against Rex's own pages, fetched as Markdown:

- `https://www.superlogical.com/rex/docs/reference/cli.md` (every public command and flag)
- `https://www.superlogical.com/rex/docs/automate/cli.md` (targets, context, exit statuses)
- `https://www.superlogical.com/rex/docs/reference/events.md` (event names)
- `https://www.superlogical.com/rex/docs/reference/api.md`

Rex's install pages are behind a Discord login; none of the pages above is, so every row
below rests on text anyone can read. This is Rex's documented CLI on that date, in beta.

### The rows doc-2 section 10 has today

| Row | Verdict |
|---|---|
| Shape | **Misleading, replace.** Rex is not verb-first. It has a full noun-verb tree *and* a layer of short shortcuts over it: `rex new` and `rex session new` carry the same description and the same flags, as do `rex ls` and `rex session ls`, `rex capture` and `rex block capture`, and the same holds for `send`, `zoom`, `move`, `swap` and `kill`. The difference is not the shape; it is that Rex has two spellings for the common commands and steg has one |
| Targets | **Confirmed.** "To act somewhere else, specify it with a flag": `-s`, `-w`, `-b`, `-C` for a client, `-S` for a server. Add that Rex has a `client` noun as a target and steg has none |
| Words | **Confirmed.** Rex's taxonomy is session, window, block, client, and its commands are `rex window ...` and `rex block ...` |
| Scripting | **Confirmed.** `rex do` runs an action, a script or inline code, and "it has a complete, powerful Lua scripting interface" |
| Context | **Confirmed but incomplete.** Rex sets `REX_SESSION`, `REX_BLOCK`, `REX_SERVER`, and also `REX_CLIENT` and `REX_BIN_DIR` (see B9) |
| Exit statuses | **Wrong as stated.** The table does match, 0, 1, 2, 3, 4 and 130, but "Commands that wait for a program, such as `rex wait` and `rex run --wait`, exit with that program's status instead", and steg's section 6 deliberately does the opposite. Split into two rows: the table is the same, waiting is not |
| `--json` | **Confirmed in part.** Both have it, and the shapes differ: `rex ls --json` prints an object keyed `sessions`, steg's `list` prints one array. Rex's readable output is a table with short IDs by default (`--short`, defaults to true); steg prints tab-separated full identifiers |

### The rows to add

| | Rex | steg | Why |
|---|---|---|---|
| Verb set | Two spellings for one meaning: `ls` and `list`, `kill` and `close`; plus `inspect`, `rename`, `zoom`, `types`, `add`, `remove` | One verb per meaning, from a closed set, no aliases | The skill's rule, and it is what lets the CLI be generated from the API with no table of names |
| Toggles | `rex block zoom` toggles when neither `--on` nor `--off` is given | `pane set --zoomed` and `pane set --no-zoomed`; no toggle | A toggle's result depends on state the script cannot see |
| The API over the CLI | `rex api list`, `rex api describe METHOD`, `rex api call METHOD` | B4 is open; `api call` is deliberately not copied | A generic call command is a hole through the generated grammar |
| Completion and version | Subcommands: `rex completion zsh`, `rex version` | Flags on the bare program (A4) | They are the commands that are not API methods |
| Capture | Screen and scrollback together, `--format` of text, html or vt, plus `--trim` and `--unwrap` | The screen as text, scrollback with `--scrollback` (R35) | R35 asks for text; the rest is M2 at the earliest |
| Events | `rex events` streams and the caller filters (its own examples pipe through `jq`) | `event watch --for <event>` filters on the server | Less traffic, and one fewer tool in an agent's loop |
| Waiting | `--for DURATION` is the time limit | `--for <event>` names the condition, `--timeout <seconds>` is the limit | The same flag with a different meaning: a trap for anyone porting a Rex script |
| Starting the server | `--autostart`, defaults to true, can be turned off | B8 is open | A script needs to ask without starting one |
| Pane kinds | `rex block new FLAVOR`, `rex block types` | Every pane is a terminal; a viewer is a program in one (doc-1 section 5.14, R65) | Keeps one rendering path, and R12 keeps the door open |
| Port forwarding | `rex forward new, open, share, ls, close` | Out of scope (doc-1 section 10) | SSH is the only remote transport |

## D. The acceptance criteria, read against doc-2

- **#1, a board document lists every proposed command with its flags, the context
  variables and the exit statuses.** Met by doc-2: commands in section 8, flags in
  section 5, context variables in section 4, exit statuses in section 6, and it is linked
  to this task. The appendix's check now reports only four flags section 5 does not
  define, two of which are undecided proposals (`--file`, `--completion`) and two of
  which are edit 2 (`--no-zoomed`, and `--colour` at M2). For Bruk: "every proposed
  command" is read here as the first release's set plus R63, not every command of every
  release. Say if you meant wider.
- **#2, the document marks each point where Rex's CLI does it differently.** Section 10
  does it in form, but it had never been checked against Rex's documentation, and the
  check found one row misleading (Shape), one row wrong in an important way (Exit
  statuses), two rows incomplete (Context, `--json`), and ten differences missing
  entirely. Part C is the checked replacement. Closing this criterion on doc-2 as it
  stands today would close it on an unverified table; closing it after edit 8 is applied
  closes it on a sourced one.
- **#3, Bruk has recorded his decision on the document.** Not met, and not a worker's to
  meet. It closes when Bruk answers part A and the orchestrator records the answers in
  doc-2 section 9, in the form it already used for the three decisions of 2026-11-13.

No accepted criterion is changed by this document.

## E. What waits, and for whom

Two items in this task's notes depend on TASK-1.3, which is planned but not built. They
are not done here, and neither is lost:

1. **The speed numbers in doc-2 section 7.** TASK-1.3's plan measures CLI start-up for a
   build-time and a run-time command tree, which is what section 7's first two bullets
   want. Transcribing them is one doc-2 edit once TASK-1.3 lands. Edit 9 makes section 7
   say so in the meantime.
2. **Checking section 8's commands against the API description.** The mechanical form,
   every API method has a command, is TASK-1.13's acceptance criterion 1, the drift test,
   which TASK-1.3's plan already shapes. What this task owes instead is the shape the
   description has to carry: which parameter is the target (the positional), which
   parameters are booleans (switches, with `--no-` pairs), that a program to run is a
   trailing list after `--`, that a parameter may repeat (`--key`), and that event names
   come from the event records (edit 4). doc-2 section 2 states the first two; edit 4 and
   B4's answer complete it.

**One consequence TASK-1.3 should know.** doc-2 section 5 says `--help` "contacts no
server", section 7 wants it under 10 ms with no server running, and the skill says help,
version and completion never contact a service. That is policy, so it decides TASK-1.3's
second axis: a command tree built at run time from the server's self-description cannot
print help with nothing running unless it caches the description. TASK-1.3's document
should answer that rather than this one.

**Noted, and no decision needed now.** Each is something Rex's CLI has and steg's does
not, which is a choice rather than an oversight, and none belongs in M1:

- `--format` of html or vt, and `--trim` and `--unwrap`, on capture. R35 asks for text.
  An agent reading a screen would use `--trim` and `--unwrap` often; worth TASK-1.25.
- `--all` on `session list`, `tab list` and `pane list` (Rex has `-A` on `rex block ls`).
  steg's lists are scoped to a parent, so an agent that wants every pane loops.
- How a program given after `--` is run: Rex has `--shell` and `--interactive`, and
  documents that a command runs with the login shell's environment but not its
  interactive startup files. doc-1 R13 says "a given command, with a given working
  directory and environment" and no more. This is TASK-1.8's to pin.
- A `client` noun. doc-1 section 4 defines the word, R7 has two clients on one session at
  M3, and `server show` reports a count. Listing or disconnecting one has no command.
- `--timeout` as a universal flag. Rex limits every connection and non-streaming request,
  10 seconds by default; steg's is on `wait` and `watch` only.

## Appendix. The consistency check

Throwaway, kept here rather than in the repository so anyone can re-run it against
doc-2. It reports every command in section 8 whose noun or verb section 3 does not list,
every long flag the document uses and section 5 does not define, and Rex's verbs against
steg's set. Save it, then pass it doc-2's Markdown and, optionally, Rex's CLI reference.

Output on 2026-11-14, against doc-2 as updated at 02:46 UTC that day:

```
1. Commands in section 8 with a noun or verb section 3 does not list
   none

2. Long flags the document uses and section 5 does not define
   --colour: in section 3, 8
   --completion: in section 9
   --file: in section 9
   --no-zoomed: in section 3

3. Rex's verbs against steg's verb set
   steg's set:            capture check close focus list move new reload resize run send
                          set show split start stop swap unset wait watch
   Rex noun-verb, shared: capture check close focus list move new reload run send set
                          show start stop swap unset
   Rex noun-verb, extra:  add bash call describe edit fish host inspect install kill logs
                          ls nu open powershell remove rename send-key setup share status
                          stream tailscale types uninstall zoom zsh
   Rex top-level words:   actions api block capture client completion config do events
                          focus forward home hosts keymap kill login-server ls move new
                          resize run send send-key server session skill split swap
                          terminfo version wait whoami window zoom
```

The script:

```python
#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.14"
# dependencies = []
# ///

"""Check the steg CLI specification (doc-2) against itself, and against Rex's verbs.

Usage: doc2-check <doc-2.md> [<rex-reference-cli.md>]
"""

import re
import sys
from pathlib import Path

SECTION = re.compile(r"^## (\d+)\.", re.MULTILINE)
SUBSECTION = re.compile(r"^### (.+)$", re.MULTILINE)
CODED = re.compile(r"`([^`]+)`")
FLAG = re.compile(r"--[a-z][a-z0-9-]*")
REX_HEADING = re.compile(r"^### `rex ([a-z-]+)(?: ([a-z-]+))?", re.MULTILINE)


def sections(text: str) -> dict[str, str]:
    """Split the document into its numbered sections."""
    marks = [(m.group(1), m.start()) for m in SECTION.finditer(text)]
    out: dict[str, str] = {}
    for i, (number, start) in enumerate(marks):
        end = marks[i + 1][1] if i + 1 < len(marks) else len(text)
        out[number] = text[start:end]
    return out


def first_column(section: str) -> list[str]:
    """Return the backticked tokens in the first column of every table row."""
    tokens: list[str] = []
    for line in section.splitlines():
        if not line.startswith("|") or set(line) <= set("|- "):
            continue
        cell = line.split("|")[1]
        tokens.extend(t.strip() for t in CODED.findall(cell))
    return tokens


def subsections(section: str) -> dict[str, str]:
    """Split one section into its ### subsections, keyed by heading."""
    marks = [(m.group(1).strip(), m.start()) for m in SUBSECTION.finditer(section)]
    out: dict[str, str] = {}
    for i, (heading, start) in enumerate(marks):
        end = marks[i + 1][1] if i + 1 < len(marks) else len(section)
        out[heading] = section[start:end]
    return out


def main(argv: list[str]) -> int:
    """Print the three set differences, and return 1 if any is non-empty."""
    if not 2 <= len(argv) <= 3:
        print(__doc__, file=sys.stderr)
        return 2
    text = Path(argv[1]).read_text()
    part = sections(text)
    three = subsections(part["3"])

    nouns = set(first_column(three["Nouns"]))
    verbs: set[str] = set()
    for token in first_column(three["Verbs every noun shares"]):
        verbs.update(v.strip() for v in token.split(","))
    # Section 3's own-verb table holds whole commands, not bare verbs.
    for token in first_column(three["Verbs one noun owns"]):
        verbs.add(token.strip().split()[-1])

    print("1. Commands in section 8 with a noun or verb section 3 does not list")
    bad = []
    for token in first_column(part["8"]):
        for command in token.split(","):
            words = command.strip().split()
            if len(words) != 2:
                continue
            noun, verb = words
            if noun not in nouns or verb not in verbs:
                bad.append(f"   {command.strip()}: noun={noun in nouns} verb={verb in verbs}")
    print("\n".join(bad) if bad else "   none")

    print("\n2. Long flags the document uses and section 5 does not define")
    defined = set(FLAG.findall(part["5"]))
    used: dict[str, set[str]] = {}
    for number, body in part.items():
        if number == "5":
            continue
        for flag in FLAG.findall(body):
            if flag == "--":
                continue
            used.setdefault(flag, set()).add(number)
    missing = {f: s for f, s in used.items() if f not in defined}
    for flag in sorted(missing):
        print(f"   {flag}: in section {', '.join(sorted(missing[flag]))}")
    if not missing:
        print("   none")

    print("\n3. Rex's verbs against steg's verb set")
    if len(argv) == 3:
        tree: set[str] = set()
        top_level: set[str] = set()
        for top, second in REX_HEADING.findall(Path(argv[2]).read_text()):
            if second:
                tree.add(second)
            else:
                top_level.add(top)
        print("   steg's set:            " + " ".join(sorted(verbs)))
        print("   Rex noun-verb, shared: " + " ".join(sorted(tree & verbs)))
        print("   Rex noun-verb, extra:  " + " ".join(sorted(tree - verbs)))
        print("   Rex top-level words:   " + " ".join(sorted(top_level)))
    else:
        print("   skipped: no Rex reference given")

    return 1 if bad or missing else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
```
