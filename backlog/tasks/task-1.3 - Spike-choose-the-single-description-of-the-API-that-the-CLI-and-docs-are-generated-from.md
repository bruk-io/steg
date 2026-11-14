---
id: TASK-1.3
title: >-
  Spike: choose the single description of the API that the CLI and docs are
  generated from
status: In Progress
assignee:
  - '@bruk.w2'
created_date: '2026-11-13T22:52:00Z'
updated_date: '2026-11-14T04:17:00Z'
labels:
  - size-2
milestone: m-0
dependencies: []
documentation:
  - doc-2
  - doc-4
parent_task_id: TASK-1
type: spike
project: bruk-io-steg
asdlc_accepted_from: DRAFT-4
asdlc_branch: task-1.3-spike-choose-the-single-description-of
asdlc_plan_approved_by: bruk
asdlc_plan_approved_hash: '7891230c821a'
asdlc_plan_approved_size: size-2
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Design decision 3 in the spec. One source has to yield the server's methods, the self-description a client can ask for (R37), the CLI's commands (R32) and later the reference docs (R62).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A board document records the format, with one method carried end to end: defined once, served, self-described, and callable from a generated CLI command
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
## What this spike decides, and what it must not

Decides: where the one definition of steg's API lives, in what format, and how each
consumer gets it, so that the server's methods (TASK-1.10), the self-description a
client asks for (R37), the CLI's commands (R32, TASK-1.13) and the reference docs (R62)
all come from that one place. `.claude/rules/architecture.md` line 13 is the bar: a
method written by hand in the CLI, or documented by hand, is a second definition.

Must not decide, and the format has to leave room for all three:

- Framing and how terminal output reaches a client (TASK-1.2). The spike needs a
  transport to carry one call, so it uses newline-delimited JSON over a Unix socket and
  the document marks it a stand-in, not a choice. What the spike does fix is that the
  **payload** of the self-description, the JSON a client reads, is the contract; the
  Rust-side definition is an implementation detail behind it. Plugins are separate
  Python processes on the public API (R42, R75), so the payload is what they see.
- Command names, flags, context variables and exit statuses (TASK-1.4, Bruk's to
  decide). The document says instead **where** those names live: whether the format
  carries a naming slot per method and per parameter that TASK-1.4 fills, or the
  generator applies a mechanical transform (dots to subcommands, snake_case to
  kebab-case) that TASK-1.4 overrides. TASK-1.13's criterion 1, a test that fails when
  the API has a method the CLI does not expose, cannot be written until that is pinned.
- A Python client library (a later release, design decision 5 in the spec). The format
  has to be consumable from outside Rust with no Rust-side knowledge; the spike proves
  that with a script, not a library.

## The two axes, and the candidates

The decision has two independent axes:

1. Where the single definition lives: in the Rust source, or in a schema file the Rust
   is generated from.
2. When a client learns the API: at build time (generated code) or at run time (the
   server's self-description, which is how Rex's CLI and scripting API work, through its
   `help.all` method).

Two candidates are built, sharing one definition so the comparison isolates axis 2:

- **A. Definition in Rust, CLI generated at build time.** One definition in Rust yields
  the server's dispatch, the self-description payload, and a clap command tree compiled
  into the binary.
- **B. Definition in Rust, CLI built at run time** from the self-description the server
  returns, Rex-style.

One candidate is weighed on paper and not built:

- **C. A schema file as the definition** (an OpenRPC document, or JSON Schema, or an IDL
  such as Smithy or protobuf), with the Rust server and the CLI both generated from it.
  Judged on what exists on crates.io today, with crate names, versions and the date
  read; on whether the Rust types stay single-sourced or are a second definition in
  practice; and on build complexity. If the paper case beats A and B, the document says
  so and says a second round is needed rather than pretending it was tried.

### The discriminator

R61 ships the server and the CLI as one binary. So the test that decides axis 2 is
narrow and mechanical, not a trade-off table: with no server running, can the CLI print
`--help` for a command, emit shell completions, and can R62's reference docs be
generated? Measure, for each candidate: offline `--help` (works or not), CLI start-up
cost over repeated runs, and whether the docs step needs a running server.

Second discriminator, for axis 1: how a method's and a parameter's documentation reaches
the payload. R37 asks for the documentation and R62 generates from it, so whether Rust
doc comments can carry it, or the text must live in a schema file, separates A and B
from C.

## Files that change

In the worktree, `/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.3`.
Throwaway code, committed so the reviewer can re-run it, and deleted by TASK-1.10 when
the real thing lands; the README says so.

- `spikes/api-description/Cargo.toml`: the spike crate. It carries a bare `[workspace]`
  table, which makes it its own workspace root. TASK-1.5 adds the repository's root
  `Cargo.toml` in parallel, and without that table cargo would refuse this crate once
  the two branches meet. Dependencies pinned, each one's licence read by hand.
- `spikes/api-description/Cargo.lock`: committed, so the reviewer builds the versions
  that were measured.
- `spikes/api-description/README.md`: what the spike answers, how to run it, that it is
  throwaway, and who removes it.
- `spikes/api-description/src/api.rs`: the single definition. The one method with its
  parameters and documentation, plus one event record and one error record, so the
  format is shown to cover events (R38, TASK-1.11) and errors (R34's exit statuses), not
  methods alone. Only the method is carried end to end, as the acceptance criterion asks.
- `spikes/api-description/src/description.rs`: the self-description payload built from
  the definition: the typed structs a client reads, with serde derives.
- `spikes/api-description/src/server.rs`: the toy server. A Unix socket, newline-delimited
  JSON, dispatching the one method and answering the describe call. Stand-in framing.
- `spikes/api-description/src/cli_build.rs`: candidate A, the command tree generated from
  the definition at build time.
- `spikes/api-description/src/cli_runtime.rs`: candidate B, the command tree built at run
  time from the payload the server returns.
- `spikes/api-description/src/docs.rs`: the reference page for the method rendered as
  Markdown from the payload (R62).
- `spikes/api-description/src/bin/spike_server.rs`, `src/bin/spike_cli_a.rs`,
  `src/bin/spike_cli_b.rs`, `src/bin/spike_docs.rs`: the binaries the script below runs.
- `spikes/api-description/tests/drift.rs`: the drift tests (see Proof).
- `spikes/api-description/client.py`: a PEP 723 script that asks for the description and
  then calls the method knowing nothing else, standing in for a plugin (R42, R75).
- `spikes/api-description/carry-one-method`: an executable script that carries the method
  end to end in one command and prints the measurements. This is what the acceptance
  criterion is recorded against.

Not touched, deliberately: the repository's root `Cargo.toml` and `Cargo.lock` (TASK-1.5
owns them), the `justfile` (TASK-1.5 is editing it, and `steg-affected` treats it as a
file that affects every crate), `tools/`, and `.gitignore` (its bare `target/` already
matches at any depth).

Board artefacts, created with `asdlc` from the tasks clone and so not in this branch's
diff:

- A new document from `asdlc board doc create`, type `specification`: the deliverable.
  Linked to the task with `asdlc board task edit TASK-1.3 --add-doc <doc-id>`.
- The task's notes: what was measured, and the departures named under Risks.

No `asdlc board decision` record: the answer is a format TASK-1.10 builds from, so it
belongs in one document a builder reads, not split across two. Say so if you want an ADR
beside it.

## The method carried end to end

`pane.split`, chosen by what it has to exercise rather than by name: a target parameter
whose resolution follows R10 (full ID, label, position or ID suffix), a closed set as an
enum (the split's direction), two optional parameters (the command to run, the working
directory), a typed return (the new pane's ID, a type of its own, not a bare string, per
`.claude/rules/rust.md`), documentation on the method and on each parameter, and one
documented error (an ambiguous target, which R10 says is an error and never a guess).

End to end means, in one run: defined once, served by the toy server, returned by the
describe call, called through candidate A's generated command, called through candidate
B's generated command, called from `client.py` using only the description, and rendered
as a Markdown reference page.

## What the document must record

1. The decision: where the definition lives, in what format, and how each of the four
   consumers gets it.
2. The format itself, in enough detail for TASK-1.10 to build it: what a method record
   holds (name, summary, parameters with type, optionality and documentation, return
   type, the errors it can return), what an event record holds, and how the API's
   version is carried for R39.
3. The measurements from the discriminator above, per candidate.
4. Candidate C weighed, with the crates and versions read and the date.
5. The licence of every dependency the chosen approach needs (R59: MIT, nothing GPL or
   AGPL). `cargo deny check licenses` cannot see this crate, since it is outside the
   workspace and `just deny` skips it while there is no root manifest, so each licence
   field is read by hand and listed, as TASK-1.1's criterion 3 does.
6. The three lines left to others: the stand-in framing (TASK-1.2), where CLI names live
   (TASK-1.4), and the shape of TASK-1.13's no-drift test.

## Order of work

1. Record `just check` as the starting point, before anything changes.
2. Create the spike crate: manifest with the bare `[workspace]` table, README, and a
   build that compiles. Confirm `just affected` still answers `none` for a change under
   `spikes/`, so this branch does not change what the repository's checks run.
3. Write the definition in `api.rs` (method, event, error) and the payload in
   `description.rs`.
4. The toy server: dispatch the method, answer the describe call.
5. Candidate A: the build-time command tree, with `--help` working with no server.
6. Candidate B: the run-time command tree from the payload.
7. `client.py` and `docs.rs`, both from the payload alone.
8. The drift tests.
9. The `carry-one-method` script, and take the measurements.
10. Weigh candidate C; read each named crate's licence.
11. Write the document, link it to the task, and write the notes.

Steps 3 to 7 are the throwaway core; if step 5 or 6 proves far harder than the other,
that asymmetry is itself the finding and goes in the document.

## Risks

- **Collision with TASK-1.5**, which creates the Rust workspace in another worktree on
  the same repository. Handled by touching no root file and by the bare `[workspace]`
  table in the spike's manifest. Both check commands are run again after the default
  branch is merged at finish, which is when the collision would show.
- **A proc macro eats the whole spike.** The pattern can be proved without one: a
  `macro_rules!` table, or a plain table of method records beside a serde-derived
  parameter struct, is enough to show one definition yielding four consumers. The
  document says what a production macro would add. Timebox: if the spike needs a proc
  macro to answer the question, stop and say so rather than build one.
- **Candidate C's tooling may be too immature to judge from documentation alone.** Then
  the document says the evidence is thin and what a second round would cost, rather than
  rejecting it on a guess.
- **The honest answer may be a hybrid** (one definition in Rust, served at run time for
  R37 and plugins, generated at build time for the CLI and the docs). That is one
  decision with one artefact, and the document must show it as one source, not two.
- **Over-reach into TASK-1.4.** Mitigated by recording where names live rather than what
  they are, and by the carried command using the mechanical transform under protest, with
  the transform named as a placeholder in the document.
- **A departure from `.claude/rules/testing-rust.md`**, which says to run tests with
  `just test` and never `cargo test` directly. That rule governs workspace crates under
  `just`'s control; this crate is outside the workspace on purpose, and adding a justfile
  recipe would both collide with TASK-1.5 and make `steg-affected` select every crate.
  The spike's tests are run with `cargo test --manifest-path`, recorded on the task so
  the evidence still comes from the toolchain. Noted in the task's notes.

## Proof

Each recorded with `asdlc record-check TASK-1.3 --in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.3 --as bruk.w2 -- <command>`:

- `just check` at step 1 and again at finish: the repository's own bar, unchanged by this
  branch. Expect the Rust steps to skip while there is no workspace, the tools' tests to
  pass, and `just test` to print its skip line because `spikes/` owns no crate.
- `cargo test --manifest-path spikes/api-description/Cargo.toml`: the spike's tests. The
  load-bearing ones are the drift tests, not the happy path:
  - adding a parameter to the definition in `api.rs` changes what the server accepts,
    what the describe call returns, both CLIs' help and the generated Markdown, with no
    hand-written line edited anywhere. The test asserts the parameter appears in each of
    the four outputs, so deleting the generation makes it fail.
  - a method present in the definition with no command in the generated CLI fails the
    test. This is the shape TASK-1.13's criterion 1 inherits.
- `spikes/api-description/carry-one-method`: acceptance criterion 1 in one command.
  Defines `pane.split` once, starts the toy server, prints the describe call's payload,
  calls the method through candidate A's command and through candidate B's command, calls
  it from `client.py`, renders the Markdown page, and prints the measurements: offline
  `--help` for each candidate, and start-up cost over repeated runs.
- The document exists, is linked to the task, and records all six items under "What the
  document must record". That, with the recorded script output it quotes, is what the
  acceptance criterion is judged on.

## Where the evidence is thin, on purpose

Axis 2 (build time or run time) is answered by measurement: both candidates are built.
Axis 1 (Rust source or a schema file) is answered on paper only. Building a schema-file
candidate means a second code generation path and roughly doubles the spike, which a
size-2 task does not hold. So the document must say which of its two answers rests on a
measurement and which on a reading, and name what would overturn the paper one. If the
reading leaves axis 1 genuinely open, the honest outcome is to say so and ask for a
second round, not to pick from a guess.

Two consequences for the deliverable:

- Acceptance criterion 1 asks for one generated CLI command. Both candidates' runs are
  recorded, but the document names which candidate's run is the proof: the one the
  measurements choose. The other run is evidence for the comparison, not the criterion.
- The start-up measurement has no `hyperfine` on this machine. The README states the
  method it uses instead, so the number can be reproduced: how many runs, what is timed
  (process start to exit), which clock, and whether the server was already up.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Onboarding skipped by @bruk.o: bruk-io-steg has no repository plugin yet and no code to read layers or verify commands from. TASK-1.24 onboards it once the workspace exists. The repository's own rules are in .claude/rules/ and its commands are in the justfile.

Plan checked against doc-1 by @bruk.o: ok. It serves R32, R36, R37 and R62, leaves framing to TASK-1.2 and command names to TASK-1.4 (the mechanical name transform is a flagged placeholder), keeps its throwaway code under spikes/ in its own workspace root so it cannot collide with TASK-1.5, and its proof covers the acceptance criterion with drift tests. Two points put to Bruk with the plan: where CLI names live borders TASK-1.4, and the schema-file option is weighed on paper only. The plan is large for size-2; the worker's timebox on a proc macro is the control.

For phase 2, from Bruk on 2026-11-13: 'For the CLI we should keep the grammar unixy.' That bears on where CLI names live. A purely mechanical transform gives noun-verb commands (pane split); short unix-style verbs (split, ls, send) need the format to carry a name, or aliases, per method. Weigh that when the document says where names live. The names themselves stay TASK-1.4's. Recorded by @bruk.o.

Supersedes the earlier note on CLI names. Bruk chose noun-verb grammar on 2026-11-13, so a command is the API method's name mechanically (pane.split is steg pane split, the target parameter is the one positional, every other parameter is a flag, a boolean is a switch): see section 2 of the CLI specification (doc-2). The mechanical transform is therefore the design, not a placeholder, and the format needs no per-method name or alias slot. It does need to say which parameter is the target and which are booleans. Recorded by @bruk.o.

For phase 2, from TASK-1.4's review of doc-2: its section 5 says --help contacts no server, and the cli-design skill says help never starts or contacts a service. That bears directly on build time against run time: a command tree built at run time cannot print help with nothing running unless the description is cached or compiled in. Treat it as a requirement the candidates are measured against, not a preference. Recorded by @bruk.o.

Check passed: `just check` exited 0 at 2026-11-14T03:57:06+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.3 on task-1.3-spike-choose-the-single-description-of at 17e1d355fc71:

```text
skip    format check: no Rust workspace yet
skip    lint: no Rust workspace yet
skip    licences: no Rust workspace yet
skip    tests: no Rust workspace yet
ok      tool tests (0.4s)
```

Check passed: `cargo test --manifest-path spikes/api-description/Cargo.toml` exited 0 at 2026-11-14T04:11:18+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.3 on task-1.3-spike-choose-the-single-description-of at 17e1d355fc71 with uncommitted changes, last 40 of 73 lines:

```text

     Running unittests src/bin/spike_cli_b.rs (spikes/api-description/target/debug/deps/spike_cli_b-8244f07e05531ca7)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running unittests src/bin/spike_docs.rs (spikes/api-description/target/debug/deps/spike_docs-4af73df422e352d8)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running unittests src/bin/spike_server.rs (spikes/api-description/target/debug/deps/spike_server-a5735e7f6df91073)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests/drift.rs (spikes/api-description/target/debug/deps/drift-46d16e224c3291cb)

running 10 tests
test a_parameter_added_to_the_definition_reaches_the_cli_the_server_the_docs_and_the_payload ... ok
test a_parameter_the_description_does_not_declare_is_refused_by_the_running_server ... ok
test candidate_a_prints_help_for_a_command_with_no_server_running ... ok
test an_ambiguous_target_exits_with_the_status_the_description_gives_it ... ok
test the_json_output_carries_the_api_field_names ... ok
test every_method_in_the_description_can_be_called_through_its_generated_command ... ok
test a_method_prints_what_the_description_says_it_returns ... ok
test candidate_b_cannot_print_help_with_no_server_running ... ok
test both_candidates_generate_the_same_command_from_the_same_description ... ok
test the_docs_can_be_generated_from_the_servers_own_answer ... ok

test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.14s

   Doc-tests steg_spike_api_description

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

Check passed: `cargo clippy --manifest-path spikes/api-description/Cargo.toml --all-targets` exited 0 at 2026-11-14T04:11:24+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.3 on task-1.3-spike-choose-the-single-description-of at 17e1d355fc71 with uncommitted changes:

```text
    Checking steg-spike-api-description v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.3/spikes/api-description)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.39s
```

Check passed: `spikes/api-description/carry-one-method` exited 0 at 2026-11-14T04:11:36+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.3 on task-1.3-spike-choose-the-single-description-of at 17e1d355fc71 with uncommitted changes, last 40 of 467 lines:

````text
      "lowest_ms": 4.33,
      "median_ms": 6.93,
      "p95_ms": 12.31
    },
    "candidate_b": {
      "lowest_ms": 6.33,
      "median_ms": 8.32,
      "p95_ms": 12.7
    }
  },
  "version": {
    "candidate_a": {
      "lowest_ms": 4.17,
      "median_ms": 7.77,
      "p95_ms": 13.13
    },
    "candidate_b": {
      "exit_status": 4,
      "lowest_ms": 3.81,
      "median_ms": 6.24,
      "p95_ms": 10.64
    }
  },
  "a_call_against_a_live_server": {
    "candidate_a": {
      "lowest_ms": 4.29,
      "median_ms": 6.35,
      "p95_ms": 9.67
    },
    "candidate_b": {
      "lowest_ms": 5.36,
      "median_ms": 9.46,
      "p95_ms": 16.23
    }
  }
}

=== done
one method, defined once, served, self-described, called from two generated CLIs,
called from Python, and documented.
````

Check passed: `spikes/api-description/carry-one-method` exited 0 at 2026-11-14T04:12:28+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.3 on task-1.3-spike-choose-the-single-description-of at 17e1d355fc71 with uncommitted changes, last 40 of 477 lines:

````text
      "lowest_ms": 3.43,
      "median_ms": 3.73,
      "p95_ms": 4.26
    }
  },
  "version": {
    "candidate_a": {
      "lowest_ms": 2.99,
      "median_ms": 3.44,
      "p95_ms": 3.99
    },
    "candidate_b": {
      "exit_status": 4,
      "lowest_ms": 2.96,
      "median_ms": 3.51,
      "p95_ms": 3.99
    }
  },
  "a_call_against_a_live_server": {
    "candidate_a": {
      "lowest_ms": 3.16,
      "median_ms": 3.61,
      "p95_ms": 4.03
    },
    "candidate_b": {
      "lowest_ms": 2.97,
      "median_ms": 3.13,
      "p95_ms": 3.47
    }
  },
  "load_average_after": [
    9.94,
    6.59,
    4.47
  ]
}

=== done
one method, defined once, served, self-described, called from two generated CLIs,
called from Python, and documented.
````

Check passed: `just check` exited 0 at 2026-11-14T04:16:53+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.3 on task-1.3-spike-choose-the-single-description-of at 84c91a29dde2:

```text
skip    format check: no Rust workspace yet
skip    lint: no Rust workspace yet
skip    licences: no Rust workspace yet
skip    tests: no Rust workspace yet
ok      tool tests (0.1s)
```

Check passed: `cargo test --manifest-path spikes/api-description/Cargo.toml` exited 0 at 2026-11-14T04:17:01+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.3 on task-1.3-spike-choose-the-single-description-of at 84c91a29dde2, last 40 of 74 lines:

```text

     Running unittests src/bin/spike_cli_b.rs (spikes/api-description/target/debug/deps/spike_cli_b-8244f07e05531ca7)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running unittests src/bin/spike_docs.rs (spikes/api-description/target/debug/deps/spike_docs-4af73df422e352d8)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running unittests src/bin/spike_server.rs (spikes/api-description/target/debug/deps/spike_server-a5735e7f6df91073)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests/drift.rs (spikes/api-description/target/debug/deps/drift-46d16e224c3291cb)

running 10 tests
test a_parameter_added_to_the_definition_reaches_the_cli_the_server_the_docs_and_the_payload ... ok
test a_parameter_the_description_does_not_declare_is_refused_by_the_running_server ... ok
test candidate_b_cannot_print_help_with_no_server_running ... ok
test candidate_a_prints_help_for_a_command_with_no_server_running ... ok
test the_json_output_carries_the_api_field_names ... ok
test an_ambiguous_target_exits_with_the_status_the_description_gives_it ... ok
test both_candidates_generate_the_same_command_from_the_same_description ... ok
test every_method_in_the_description_can_be_called_through_its_generated_command ... ok
test a_method_prints_what_the_description_says_it_returns ... ok
test the_docs_can_be_generated_from_the_servers_own_answer ... ok

test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.20s

   Doc-tests steg_spike_api_description

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```
<!-- SECTION:NOTES:END -->

## Comments

<!-- COMMENTS:BEGIN -->
author: @bruk
created: 2026-11-14T03:56:00Z

---

Plan approved by @bruk (7891230c821a, size-2): its worker may build it.

---
<!-- COMMENTS:END -->
