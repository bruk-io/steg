---
id: decision-1
title: The one description steg's API is defined in
date: '2026-11-15T01:11:00Z'
status: proposed
---
## Context

steg's CLI commands, its self-description, its reference documentation and what a Python plugin sees all have to agree (the spec's requirements on the CLI being generated from the API, one API for every client, and the API describing itself). A spike, the archived TASK-1.3, built and measured the candidates on 2026-11-14. This was moved here from the board document doc-4 on 2026-11-15, unchanged.

## Decision

steg's API is defined once as data in Rust, as one value in one module. That value is compiled into the single binary, and the server also serves it unchanged to anyone who asks. Every other consumer reads it; nothing downstream names a method, a parameter, a flag or an exit status of its own.

Status: proposed. The spike's document asks for Bruk's approval of the decision and the format, and that has not been recorded.

## Consequences

- The CLI's command tree, the self-description and the reference docs cannot drift from the server, because none of them is written by hand.
- Which crate holds the value is not decided here; both the server and the CLI must be able to depend on it.
- The full reasoning, the format and the measurements are in the record below.

## The record, as the spike wrote it

- Spike: TASK-1.3. Design decision 3 in the spec (doc-1, section 7).
- Serves R32 (the CLI's commands are generated from the API), R36 (one API for every
  client), R37 (the API describes itself) and R62 (reference docs generated from the API
  description).
- Reads with: doc-1 (the spec) and doc-2 (the CLI specification). Where doc-2 fixes the
  CLI's grammar, this document says what the description has to carry for that grammar to
  be generated rather than written.
- Status: the decision and the format are proposed for approval. Everything in section 4
  was measured; what rests on reading rather than running is marked as such.

## 1. The decision

**steg's API is defined once as data in Rust, as one value in one module. That value is
compiled into the single binary, and the server also serves it unchanged to anyone who
asks for it. Every other consumer reads it: nothing downstream names a method, a
parameter, a flag, or an exit status of its own.**

| Consumer | How it gets the description | When |
|---|---|---|
| The server's methods | Dispatches and validates against it | Compiled in |
| The self-description a client asks for (R37) | Serialized and served as the answer to one call | At the call |
| The CLI's commands (R32) | Builds its whole command tree from it | Compiled in, built in-process at start-up |
| The reference docs (R62) | Rendered from it by a build step | Compiled in |
| A Python plugin (R42, R75) | The served payload, as JSON | At the call |

**Which crate that module belongs to is not decided here.** The spike shows only that one
value serves all four consumers; it had a single crate and so proves nothing about steg's
layout. The constraint it does place is that both the server and the CLI read the value
directly, so they must both be able to depend on wherever it lives. TASK-1.5 is laying the
crates out, and `.claude/rules/architecture.md` asks a plan to say why when a crate
dependency crosses its boundaries, so TASK-1.5 and TASK-1.10 settle that between them.

Two things follow from the decision, and they are the parts that matter:

1. **The serialized payload is the contract, not the Rust types.** A plugin is a separate
   Python process on the public API, so what it reads is JSON. The Rust definition is the
   single source; the payload is the single interface. Section 2 is therefore written as
   the shape of the payload, and the Rust types are its mirror.
2. **The CLI does not ask the server what the API is.** It already has it. The server is
   asked only for things only the server knows: sessions, panes, and whatever a plugin
   registered while running.

### What was rejected

- **The CLI learning the API from the server at run time**, which is how Rex does it
  ("the CLI and scripting API derive dynamically from the server API it is connected to",
  through its `help.all` method). It cannot answer `--help` or `--version` with no server
  running, which doc-2 section 5 requires and the team's CLI design policy states as a
  rule. Measured in section 4.
- **A schema file as the definition** (an OpenRPC document, JSON Schema, or an IDL such as
  Smithy), with the Rust generated from it. Weighed in section 5, on reading rather than
  running, and rejected on the state of the tooling today.
- **Code generation at build time** from the definition, emitting the clap builder calls
  as Rust source. Nothing to buy: building the command tree from the compiled-in value
  costs no measurable time (section 4), and a build script or a proc macro would add a
  machinery nobody has to read today.

## 2. The format

What a client receives from the describe call, and so what TASK-1.10 builds. Field names
are as they appear in the payload.

### The description

| Field | Holds |
|---|---|
| `api_version` | `{major, minor}`. R39's version check compares `major` |
| `methods` | Every method |
| `events` | Every event a client can subscribe to (R38) |
| `errors` | Every error the API answers with, and the exit status each one becomes |

Events and errors are in the description for the same reason methods are. `event watch
--for <event>` takes one of those names, and doc-2 section 6's table of exit statuses is
otherwise written a second time in the CLI. TASK-1.11 adds to `events`; it does not start
a list of its own.

### A method

| Field | Holds |
|---|---|
| `name` | `{noun, verb}`, not a dotted string. `pane.split` is `{noun: "pane", verb: "split"}` |
| `summary` | One line. It becomes the command's `about` and the docs' table entry |
| `details` | The prose. It becomes `long_about` and the docs' body |
| `params` | The parameters, in the order they are shown |
| `returns` | What the method answers with, and so what the CLI prints |
| `errors` | The error codes this method can answer with |
| `since` | The API version the method appeared in |

The noun and the verb are separate fields rather than a name to be split on a dot. That
is what makes doc-2's grammar structural: a method that does not read as a noun and a verb
cannot be written down, `steg` alone can list the nouns, and `steg <noun>` alone can list
that noun's verbs, all without a second table.

### A parameter, and its role

| Field | Holds |
|---|---|
| `name` | The field name on the wire, in snake_case. The flag is this with hyphens |
| `role` | `target`, `flag`, `switch` or `program`: where the CLI puts it |
| `value` | `{kind}`, and `{kind: "choice", of: [...]}` for a closed set |
| `required` | Whether a call without it is a bad request |
| `summary` | One line, which reaches both `--help` and the docs |

`role` is the one field doc-2's grammar needs and a plain parameter list does not have.
Bruk's note on this task is that the mechanical transform is the design and the format
needs no per-method name or alias slot, but it does need to say which parameter is the
target and which are booleans. Four roles cover it:

| Role | What the CLI does with it | doc-2 |
|---|---|---|
| `target` | The one positional. At most one per method | Sections 2 and 4 |
| `flag` | `--name-with-hyphens <VALUE>` | Section 5 |
| `switch` | `--name` and `--no-name`, both always accepted | Section 5 |
| `program` | The program and its arguments, after `--`. At most one per method | Section 9 |

`value.kind` is `text`, `integer`, `boolean`, `choice`, `target`, `path` or `argv`. A
`choice` carries its words, which become the CLI's accepted values, the docs' list, and
the server's validation, from the one place.

### What a method returns, and what the CLI prints

`returns` is a closed set, and each case fixes the output doc-2 section 6 asks for:

| `returns` | The CLI prints |
|---|---|
| `nothing` | Nothing. An action that worked is silent |
| `identifier`, with `of` | The identifier alone on one line, so a shell can capture it |
| `record`, with `fields` | One record, the fields in the description's order |
| `records`, with `fields` | One record per line, tab-separated, that same order |
| `text` | The text |

`--json` prints the payload as it is, under the API's own field names, which is how doc-2
section 6's promise that `--json` uses the field names of the API is kept without a second
list of them.

### Errors

Each error record is `{code, summary, exit_status}`. The CLI looks the status up by code,
so doc-2 section 6's table is generated too: `ambiguous_target` is 3 because the
description says 3. TASK-1.13's third acceptance criterion, that each exit status is
produced by a test, is then a test over the description rather than over a hand-written
table.

### What the format deliberately does not carry

- **No framing.** Nothing about message boundaries, request identifiers, batching or
  streaming. TASK-1.2 decides those, and only has to be able to carry this payload.
- **No command names.** There is no name or alias slot: a command is its method, by the
  transform in doc-2 section 2. A new command is a new API method, named so that its noun
  and verb obey that document.
- **No CLI-only flags.** `--json`, `--quiet`, `--verbose`, `--no-color`, `--no-input`,
  `--help` and `--host` belong to the CLI, not to the API, and the generator adds them to
  every command in one place.

## 3. What was built to decide it

Throwaway code, in the task's branch under `spikes/api-description/`, as its own cargo
workspace so it cannot collide with the workspace TASK-1.5 creates. `pane.split` is
carried end to end, with `pane.set` and `pane.list` beside it so that a switch, a record
list, and a second verb under one noun are all exercised.

Two CLIs were built from one definition, differing in a single line: where the description
comes from. Candidate A has it compiled in; candidate B asks the server for it on every
run and then calls exactly the same generator. Everything after that line is shared code,
so the comparison is about when a client learns the API and nothing else.

`spikes/api-description/carry-one-method` runs the whole thing in one command: the
definition, the server, the describe call's payload, the method called through both
generated CLIs, the method called from Python knowing only the payload, an ambiguous target
exiting 3, the reference docs, `--help` with no server for both candidates, and the
timings.

**Which run is the acceptance criterion's proof.** The criterion asks for one method
defined once, served, self-described, and callable from a generated CLI command. That is
**candidate A's run**, the candidate this decision chooses: in the script, the step
"called through candidate A's generated command". Candidate B's run beside it is evidence
for the comparison, not the criterion.

## 4. The evidence

### The requirement that decides it

doc-2 section 5: `-h`, `--help` gives "usage for this exact command, on stdout, status 0.
Contacts no server", and `steg --version` "contacts no server". Measured, with no server
listening:

| | `pane split --help` | `--version` |
|---|---|---|
| Candidate A, compiled in | **exit status 0**, the full usage text on stdout | **0** |
| Candidate B, from the server | **exit status 4**, nothing on stdout | **4** |

Candidate B fails the same way whatever is asked of it, because the description has to
arrive before anything can be parsed. A cached copy on disk would answer help, at the cost
of a staleness bug of its own to own: the cache has to be invalidated when the server's
version changes (R39), which is the compiled-in description with extra steps.

### Speed does not decide it

Release builds, 50 runs each after 5 discarded, whole process timed from spawn to exit,
reported in milliseconds. Both runs below are recorded on the task. This machine was also
building other tasks' worktrees, so the absolute numbers are the honest cost on a busy
developer machine, and the comparison between the candidates is within one run. Each run
records its own load average.

| Measured, run of 04:12 UTC | Candidate A | Candidate B |
|---|---|---|
| `pane split --help`, server up | 3.52 median, 3.93 at the 95th | 3.73, 4.26 |
| `pane split`, server up | 3.61 median, 4.03 at the 95th | 3.13, 3.47 |
| `--version` | 3.44 median, 3.99 at the 95th | fails: status 4 |

**The round trip is lost in the noise of starting a process.** In the run above candidate B
was faster than candidate A on a real call, which is nothing but run-to-run variation; in
the earlier recorded run, taken while a larger build was running, A's median was 6.35 ms
against B's 9.46 ms. Neither difference is a reason to choose. The requirement is.

Two things from this are worth carrying:

- **doc-2 section 7's proposals hold, with room, on a machine that is not busy building.**
  In the recorded run above, `--help` was 3.52 ms median and 3.93 ms at the 95th percentile
  against its proposed 10 ms, and a call against a live server 3.61 ms median and 4.03 ms
  at the 95th against its proposed 20 ms. In the earlier recorded run, with a large build
  running, `--help` reached 11.02 ms at the 95th percentile, past the proposal. So the
  10 ms figure should say that it is for a machine that is not compiling, or be raised, and
  since this is a toy server with no real work to do, the figure for a call will only grow.
  doc-2 section 7 says this spike's numbers replace its proposals; changing that document
  is not this spike's to do, so the figures above are offered for whoever owns it.
- **Building the command tree from the description at start-up costs nothing measurable.**
  That is what removes any reason to generate CLI code at build time: there is no machinery
  to add, because there is nothing to speed up.

### The description's size, for anyone tempted by the run-time path later

The three-method description serializes to 6,053 bytes, about 2,018 bytes a method. doc-2
section 8 lists 30 commands for the first release, so a full description is of the order of
60 KB. That is an extrapolation from a measurement, not a measurement. It is small enough
that fetching it would stay cheap, which is the point: the run-time path was not rejected
for its cost.

### One finding from the build worth passing on

clap's builder takes `&'static str` for a command's name, a flag, and a value's
placeholder: its `Str` and `Id` have no `From<String>`. A description compiled into the
binary can hand it string literals; a description that arrives over a socket is owned data,
so its names have to be leaked to get a `'static` lifetime. The spike does exactly that, in
one documented function, and it is sound in a short-lived CLI process. It is one more small
cost that falls only on the run-time path.

## 5. The schema file, weighed and not built

Could the definition live outside Rust, in a file the Rust is generated from? That is the
one axis this spike did not measure: building a second code generation path would have
doubled it. What follows is crates.io read on 2026-11-14, not crates tried.

| Crate | Version | Direction | Bearing |
|---|---|---|---|
| `schemars` | 1.2.3 | Rust types to JSON Schema | Mature, and points the way this decision already goes |
| `typed-openrpc` | 0.1.1 | Rust definitions to an OpenRPC document | Same direction, early |
| `openrpc` | 0.1.0 | OpenRPC types | Early |
| `typify` | 0.10.0-alpha.3 | JSON Schema to Rust types | The schema-first direction, in alpha |
| `aws-smithy-*` | 0.65 to 1.2 | Smithy runtime crates | Smithy's generator is a JVM build step; the crates here are its runtime, not a Rust-native IDL compiler |
| `jsonrpsee` | 0.26.1 | A JSON-RPC framework with `#[rpc]` macros | Would give dispatch, but produces no machine-readable description, so R37 would still be unserved |

The pattern is the finding: **the mature tooling runs Rust-first.** It takes Rust
definitions and emits a schema. The schema-first direction, a checked-in document that Rust
is generated from, rests on a crate in alpha or on putting a JVM in steg's build. Choosing
it would mean owning that generator for a one-person project whose whole API is 30 methods.

This costs steg nothing, because the direction the ecosystem is mature in is the direction
that also serves R37: an OpenRPC document or a JSON Schema can be emitted from the Rust
definition whenever something outside wants a standard artefact. If a Python client library
is written later (doc-1, design decision 5), that is how it should get its types, and the
decision here does not have to be revisited for it.

What would overturn this: `typify` reaching a stable release, or a second language needing
generated types badly enough to pay for the pipeline. Neither is true for the first
release.

## 6. Licences (R59)

Every dependency the chosen approach needs, with the licence read from the crate's own
metadata rather than by eye:

| Crate | Licence |
|---|---|
| `clap` 4.6.7, `clap_builder`, `clap_lex`, `anstream`, `anstyle*`, `colorchoice`, `is_terminal_polyfill` | MIT OR Apache-2.0 |
| `serde` 1.0.229, `serde_core`, `serde_derive`, `serde_json` 1.0.154, `itoa`, `proc-macro2`, `quote`, `syn` | MIT OR Apache-2.0 |
| `memchr` | Unlicense OR MIT |
| `strsim`, `zmij` | MIT |
| `utf8parse` | Apache-2.0 OR MIT |
| `unicode-ident` | (MIT OR Apache-2.0) AND Unicode-3.0 |

Nothing GPL or AGPL, and nothing taken from any other project: the format is steg's own,
and Rex is a reference for behaviour only.

**For TASK-1.5:** `unicode-ident` is reached through `syn`, which `serde_derive` and clap's
derive feature need, so **Unicode-3.0 has to be on the allow list** the check command uses,
beside MIT, Apache-2.0 and Unlicense. Without it `just deny` will refuse a tree that every
crate in steg will have.

## 7. What this leaves to others

- **TASK-1.2 (the wire protocol).** The framing is untouched. The spike used
  newline-delimited JSON over a Unix socket as a stand-in. All this decision asks of
  TASK-1.2 is that the transport can carry the description payload in the answer to one
  call, and that the request carries a method name and parameters by name.
- **TASK-1.4 (the CLI design).** Names stay doc-2's. This decision says only that the
  format carries no name of its own, which is what makes doc-2 section 2's transform the
  whole mapping. One detail doc-2 does not pin and the generator had to choose: `steg` and
  `steg <noun>` alone print help and exit 2, because no verb was given. If a bare listing
  should instead succeed, that is a line for doc-2 and a change to the generator, not to
  the format.
- **TASK-1.5 (the workspace).** Where the description's module lives, and whether the CLI
  depending on it crosses a boundary worth stating, as section 1 says. Also the licence
  allow list in section 6.
- **TASK-1.10 (the server's methods and its self-description).** Section 2 is what to
  build. Two tests from the spike are worth keeping: every method in the description has a
  body and every body is in the description, and a request naming a parameter the
  description does not declare is a bad request.
- **TASK-1.11 (events).** Add to the description's `events`. The shape is in section 2.
- **TASK-1.13 (the CLI).** The no-drift test its first acceptance criterion asks for is a
  function over the description and the command tree: every method the description declares
  must have a command. The spike has it, with its negative case, and it needs no running
  server.
- **Later, for M2 plugins (R43).** Actions a plugin registers while running cannot be in a
  compiled-in description, and they do not need to be: doc-2 already reaches them through
  `action list` and `action run <name>`, where the name is a value at run time rather than
  a generated subcommand. The static API is compiled in; the dynamic surface is data behind
  a stable command. Nothing about this decision has to change when plugins arrive.
- **For doc-2's open question 4 (shell completion).** It falls out of this choice:
  candidate A holds the whole command tree with no server, and `clap_complete` 4.6.11
  (MIT OR Apache-2.0) generates completions from exactly that. A run-time CLI could not
  have done it without a server.

## 8. Rough edges in the spike, and what a production version adds

Named so they are not mistaken for parts of the decision:

- An `identifier` result is carried as an object with one field called `id`, which the CLI
  and the handler both assume. The real format should name that field in the description.
- The spike's parameter validation covers unknown parameters, missing required ones, and
  words outside a closed set. Types are not checked beyond that.
- Target resolution in the spike is the thinnest thing that shows R10's ambiguity rule, on
  panes only. TASK-1.7 owns the real one, and the format says nothing about how a target
  resolves, only that a parameter is one.
- The definition is a plain table of records built by functions. At 30 methods that reads
  well; if it stops reading well, a declarative macro over the same data is the next step,
  and it changes nothing in the payload. A proc macro was deliberately not written: the
  question did not need one.
- The spike had one crate, so it says nothing about steg's crate layout, and its
  `[workspace]` table has not yet met the real workspace: TASK-1.5 was still in progress
  when this branch was pushed, so the collision it guards against has not been exercised.

## 9. Evidence

Recorded on TASK-1.3 by `asdlc record-check`, each from the task's worktree:

- `just check` before anything was written, and again at the end. While there is no Rust
  workspace its Rust steps print a skip line, so that check shows the tools' tests passing
  and nothing more.
- `cargo test --manifest-path spikes/api-description/Cargo.toml`: 31 tests. The ones that
  carry the argument are the drift tests in `tests/drift.rs`, which run the real binaries
  against a real socket: a parameter added to the definition reaches the CLI, the server,
  the docs and the payload with nothing hand-written edited; a method with no command is
  named by the check; candidate A prints help with no server and candidate B exits 4; both
  candidates generate the same command from the same description; an ambiguous target exits
  3; and the docs come out the same whether rendered from the definition or from the
  server's own answer.
- `cargo clippy --manifest-path spikes/api-description/Cargo.toml --all-targets`, with the
  lints `.claude/rules/rust.md` names set to deny in the spike's manifest.
- `spikes/api-description/carry-one-method`: the acceptance criterion, in one command, with
  the measurements quoted in section 4.
