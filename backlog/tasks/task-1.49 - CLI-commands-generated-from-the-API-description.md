---
id: TASK-1.49
title: 'CLI: commands generated from the API description, in the steg package'
status: In Progress
assignee:
  - '@bruk.w5'
created_date: '2026-11-15T01:33:00Z'
updated_date: '2026-11-16T13:36:00Z'
labels:
  - size-4
milestone: m-3
dependencies:
  - TASK-1.45
  - TASK-1.77
parent_task_id: TASK-1
type: task
project: bruk-io-steg
asdlc_accepted_from: DRAFT-19
asdlc_branch: task-1.49-cli-commands-generated-from-the-api
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
doc-2, sections 2 to 6 and 8: R32, R34. decision-4. Nothing in the CLI names a method by hand.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Every API method has a command and every command an API method, checked by a test that fails when they drift
- [ ] #2 The grammar, the flags every command has, `--json`, and `-q` printing identifiers only each behave as doc-2 says
- [ ] #3 Each exit status in doc-2 section 6 is produced by a test
- [ ] #4 `steg version`, `steg completion <shell>` and `--help` work with no server running; `--no-autostart` exits 4 without starting one
- [ ] #5 A list is tab-separated with full identifiers, no header and no alignment, the same piped or not
- [ ] #6 The CLI lives in the `steg` package and reaches the server only through the sdk (doc-16 R98, R101)
- [ ] #7 A client whose connection is accepted and then closed before a hello arrives treats that the same as no server being there, and arbitrates again once; shown with real processes against a server stopping as the client connects (moved from TASK-1.86's fourth criterion, whose plan the engineer approved on 2026-11-16: nothing built before this task reads a hello)
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
## For the engineer

Decisions that are yours, each with a recommendation. None blocks writing the generator; each is cheap to settle in either direction.

1. **`steg` and `steg <noun>` alone: list (status 0) or fail (status 2)?** doc-2 section 2 says plainly: "`steg <noun>` alone lists that noun's verbs, and `steg` alone lists the nouns" — no mention of failing. But decision-1 section 7 (TASK-1.4's note to this task) records that the spike's generator chose to make both print help and exit 2, "because no verb was given," and flags this as doc-2's to pin. **Recommendation: follow doc-2's literal text — both succeed, status 0, printing the noun list / that noun's verb list.** It is the document every builder reads, nothing supersedes it, and clap's builder makes this easy to implement as an explicit case rather than relying on clap's own "missing subcommand" error (which exits 2 by default and is the wrong status here regardless of which way this is decided).
2. **`api.show`'s target is optional in the description, required in doc-2.** `description::methods::api_methods` declares `api.show`'s target as `required: false` ("with none, the whole description") and dispatch already implements that. doc-2 section 8's row still reads only "One method in full" with target = method, no stated no-target form. **Recommendation: doc-2 section 8 gains a line for the no-target form**, since the description and dispatch already ship it — a documentation catch-up, not a CLI decision; the generator follows whatever `required` says either way, so this doesn't block the build.
3. **`unset --label` takes a value it discards.** Every `unset` method's `label` param is `role: Flag, value: Text, required: false` — a value-taking flag — carried from TASK-1.75's build note: "`unset --label` requires a value to be sent that dispatch then discards." The generated CLI will therefore accept `steg session unset --label anything`, sending and then throwing away whatever text follows. **Recommendation: change these params to `role: Switch`** in `description::methods` (a `steg-sdk` edit, not this task's to make unilaterally) so `unset --label` takes no value — cheaper for a script and a nicer generated command. Not blocking: the generator reflects whichever role is declared, so this can land before, during or after this task without changing the generator's code.
4. **`pane.move` has no `direction` parameter; dispatch defaults to the right.** Carried from TASK-1.75: `Tree::move_pane_to_tab` wants a direction doc-2 section 8 doesn't give `pane move`. Nothing for this task to do — the generator only ever reflects the description's own params, so if a `direction` flag is added to `pane.move` later, the generated command picks it up with no CLI code change. Flagged here only because it was carried onto this task's own planning.
5. **A nested value's plain-text rendering** (TASK-1.46's `Returns::Nested{of, summary}`, not yet on `main` — see "What TASK-1.46/1.47 add" below). doc-2 section 6 fixes every other `Returns` case's plain-text form but is silent on a nested one. **Recommendation: `--json` carries the nested value whole (unchanged, as every other `--json` output already does); plain text prints only the record's scalar fields** — the same treatment `tab_fields`'s own doc comment already gives `layout`/`shown_status` today ("not shown in a flat text table either way"). This keeps plain text readable without inventing a second structured-text format doc-2 never asked for.
6. **Splitting this task.** It is size-4 and holds three independently reviewable changes (the standing preference is to split rather than ask). Proposed below, in "The split."

## What this plan is built against, and what changes under it

`main` today has 42 methods, no `Method.follows` field, no `Returns::Nested` variant, and no `pane.attach`/`pane.detach`. TASK-1.49's own dependency graph (`TASK-1.45`, `TASK-1.77`) does not include TASK-1.46 or TASK-1.47, so this plan is written to build and prove itself against today's shapes, not against their unmerged branches. Nothing here hard-codes 42: the drift test (criterion 1) is a set comparison over whatever `description().methods` holds at build time, so it keeps working unchanged once 1.46/1.47 merge and the count moves to 44.

Two things those branches add that this design must not be surprised by, so a rebase onto them costs no redesign:

- **`Method.follows` (1.46): `Nothing`, `Events(Returns)`, or `Attachment{item, stream_id_field}`.** Until it lands, every method's follow-on behaviour is `Nothing` by construction (no such field exists yet), so the generator's "ordinary one-shot call" path is the only path there is. Once it lands: a method whose `follows` is `Attachment` (`pane.attach`) is the stated exception to criterion 1 (R32: "listed and described by the CLI, and reachable through `api call`, but has no rendering of the stream as a command") — the drift test's set comparison must then carry an explicit, narrow carve-out for exactly the methods whose `follows` is `Attachment`, asserted to be exactly that set (so the carve-out cannot silently swallow an unrelated method later). A method whose `follows` is `Events` (`event.watch`) still gets a generated command (noun, verb, flags, target, validated request) — this task's own one-shot executor sends the validated call and stops; it is TASK-1.50's to read the stream of `Incoming::Event`/`UnknownEvent` frames that follow and print one record per line until interrupted. The line: **this task wires the command and the single request; TASK-1.50 owns everything that happens after the first reply, for both `event.watch` and `pane.wait`.**
- **`Returns::Nested` (1.46).** Until it lands, the output renderer (phase/split 3, below) only ever matches `Nothing | Identifier | Record | Records | Text` and is written with an exhaustive match (`.claude/rules/rust.md`: match your own enums without a `_` arm) — so the match itself fails to compile the moment the variant is added, which is the forcing function for whoever rebases onto it to add engineer decision 5's handling, rather than it going unnoticed.

## The design: a clap `Command` tree built at run time from `Description`

Confirmed in Cargo.lock: no `clap` in the workspace today (`pico-args` and `arg_enum_proc_macro` are present only transitively, through gpui's own tree, and are not steg's). decision-1 sections 6 and 7 both assume `clap` + `clap_complete`; `deny.toml` already allows `Unicode-3.0`, the one licence decision-1 flagged as needed for `syn` (via `serde_derive`/clap's derive feature) — so adding them is licence-clean against the check that exists today, no `deny.toml` edit needed.

**Use clap's builder API (`clap::Command`/`clap::Arg`), never `#[derive(Parser)]`.** A derive names every command in a struct literal, one hand-written type per method — exactly what "nothing names a method by hand" forbids. The builder lets the generator walk `description().methods` in a loop and construct `Command`/`Arg` values from data: no method, flag or value kind is ever spelled out in Rust source. This is candidate A from decision-1's own spike (compiled-in description, tree built in-process at start-up), carried forward rather than re-measured — decision-1 section 4 found the round trip "lost in the noise of starting a process," so there's nothing to re-benchmark here; TASK-1.3's own speed numbers are what doc-2 section 7 already points at for the budget.

**`boundaries.toml`'s `[steg]` entry needs editing before anything else compiles**: add `"clap"` and `"clap_complete"` to its `allow` list, with a reason citing decision-1 (clap's builder, not derive — no `derive` feature turned on, so `deny-features` is not needed for it) and this task. `just check-boundaries` fails on an unlisted dependency, so this is a first-step edit, not a cleanup.

**Mapping the description to a tree, mechanically** (decision-1 section 2's own table):
- One `Command` per noun (`description().nouns()`, in first-appearance order), one subcommand per verb (`description().verbs_of(noun)`), each named exactly the method's noun/verb — never a literal string typed per method.
- A method's `summary` → the subcommand's `about`; `details` → `long_about`.
- `Role::Target` → the one positional `Arg`, required or not per the param's own `required`.
- `Role::Flag` → `--name-with-hyphens <VALUE>` (via `Param::flag()`, already on the type), required or not per the param.
- `Role::Switch` → two boolean args, `--name` and `--no-name` (via `Param::negated_flag()`, already on the type), in a clap `ArgGroup` so the last one given wins and both are always accepted (doc-2 section 5).
- `Role::Program` → a trailing variadic arg consuming everything after a literal `--` (clap's `trailing_var_arg`/`num_args(0..)`, matching doc-2 section 9's "after `env`, `ssh` and `sudo`").
- `ValueKind::Choice(words)` → a clap `PossibleValuesParser` built from the field's own words, so `--direction`'s accepted values, its help text, and the server's own validation come from the one list, with no duplicate written in the CLI.
- `ValueKind::Integer`/`Boolean`/`Text`/`Path`/`Target`/`Argv` → clap's own value parsers (`Path` as `PathBuf`; `Target` and `Text` both as plain strings at the parse stage — target *resolution* is server-side, R10, nothing the CLI parses locally beyond "a string was given").
- `api list`/`api show`/`api call` are generated exactly like every other method (they are methods, `description::methods::api_methods`) — no special case in the tree-building step. `api call`'s own behaviour (reading its forwarded parameters as one JSON object from stdin) is connecting/calling logic (split 2, below), not tree structure.
- `version` and `completion <shell>` are the two commands section 2 names as not API methods. They are added once, by name, outside the loop over `description().methods()` — and the drift test (below) asserts the top-level command set is exactly `description().nouns() ∪ {"version", "completion"}`, so a third hand-added command fails it immediately.

**Lifetime, per decision-1 section 4's own finding:** clap's builder takes `&'static str` for names, flags and placeholders. The compiled-in `Description` this binary carries (`steg_sdk::description::description()`) is owned, heap-allocated data, so building the tree from it needs the same move the spike made: leak the strings once, in one documented function, for this short-lived process's whole life. Not needed if the generator borrows a `Description` built once and kept alive for the process's duration and the tree is only ever built once per run (no repeated builds) — confirm which shape `clap::Command`'s builder actually needs (an owned `String` vs `&'static str`) against the clap version pinned, since this determines whether the leak is needed at all; name it as a risk to resolve empirically during the build, not guessed at here.

## The split

Three independently reviewable changes, each size 2, in this order:

### 1. `TASK-1.49a` — CLI: the command tree generated from the API description, with help, version and completion
- **Criteria:** #1 (the drift test, both directions, including the two named exemptions: `version`/`completion` as the only non-method top-level commands, asserted to be exactly those two; and, once `Method.follows` exists, the `Attachment` carve-out asserted to be exactly that set); the no-server half of #4 (`steg version`, `steg completion <shell>`, and `--help` at every level of the tree work with no server reachable, proven by pointing `STEG_SOCKET_PATH` at a path nothing listens on and asserting no connection is attempted — bounded run time, not just "didn't hang"); #6 (the crate boundaries: `boundaries.toml`'s `[steg]` edit, `just check-boundaries` passing, and this phase touching no server-reaching code at all).
- **Files:** `boundaries.toml` (`[steg]` edit); `crates/steg/Cargo.toml` (`clap`, `clap_complete`); `crates/steg/src/cli/mod.rs` (new — the tree builder); `crates/steg/src/cli/values.rs` (new — `ValueKind` → clap value parser mapping); `crates/steg/src/main.rs` (wire the generator in; `server start` stays the one hand-written case per `@bruk.o`'s dependency note); `crates/steg/tests/cli_drift.rs` (new, no server — unit-level over `description()` and the built `Command`, both directions); `crates/steg/tests/cli_no_server.rs` (new — real binary, `--help`/`version`/`completion` with nothing listening).
- **Risks:** the engineer decision above on `steg`/`steg <noun>` alone (list vs. fail) — written against "list, status 0" per the recommendation, a one-line change either way if overruled. clap's exact builder API for dynamic names (owned `String` vs `'static str`) is to be confirmed against the pinned clap version, not assumed.

### 2. `TASK-1.49b` — CLI: connecting to the server, and the exit statuses
- **Criteria:** the `--autostart`/`--no-autostart` half of #4 (`--no-autostart` exits 4 with nothing started); #3 (every exit status in doc-2 section 6 — 0, 1, 2, 3, 4, 130 — each produced by a test through the real binary); #7 (a connection accepted and then closed before a hello arrives is treated as no server there, and arbitrated once more, shown with real processes against a server stopping as the client connects).
- **Where AC7's logic lives:** the sdk's `local_socket::connect` (R101: no client holds protocol code of its own, and the app needs this exactly as much as the CLI does). Today `connect` only waits for the socket to accept; it never reads anything. This task extends it to read the first frame after a successful connect and, on EOF before a complete hello frame, treat that identically to the existing "nothing was listening" branch and retry the whole find-or-start arbitration **exactly once** — bounded by the type (a fixed retry count in the function, not a loop a future edit could widen, so "arbitrates again once" is structural). Checked against `.claude/rules/sdk.md` and R102/R110: this is the same category of I/O the module's existing `#![expect(clippy::disallowed_types, ...)]` already covers (R103's one sanctioned exception), so no new lint carve-out is needed beyond what the module already has.
- **The fixture needed, and what TASK-1.86 supplies:** TASK-1.86 (in review, PR #60) adds a fixture server with a short, settable idle timeout, for its own AC #4 — the *server's* side of this exact scenario. This task does not depend on it merging first: the honest, independent proof is a small fixture binary that accepts a connection and closes it immediately, writing no hello at all — reproducing the client-side condition exactly, with no dependency on 1.86's idle-stop mechanism, and useful regardless of when 1.86 lands. If 1.86 has merged by build time, its real server-stopping scenario is the stronger end-to-end proof and is used instead or in addition; if not, the accept-then-close fixture is the one this task's own criterion 7 is proven against, named as such rather than left implicit.
- **Exit status mapping, never a hand-written table:** a `CliOutcome`-to-status function looks up `ApiError.code` against `description().errors`'s own `ErrorRecord.exit_status` — the same generation decision-1 section 2 already describes for doc-2 section 6's table. CLI-local failures (an unknown flag, a missing value, clap's own parse errors, `--no-autostart` with nothing running, no target and no context once TASK-1.51 wires context) map to 2 or 4 directly in the CLI, never through a server round trip. `130` is a SIGINT handler installed once at start, independent of which command is running.
- **Files:** `crates/sdk/src/local_socket.rs` (extend `connect`; new unit tests with a same-module accept-then-close listener); `crates/steg/src/cli/connect.rs` (new — wraps `local_socket::connect`, sends the real hello with `ClientKind::Cli`, reads `ServerHello`/`Refused`, surfaces a version mismatch's message); `crates/steg/src/cli/exit.rs` (new — the status mapping; the SIGINT handler); `crates/steg/src/cli/api_call.rs` (new — `api call`'s stdin-JSON-to-params behaviour, since it is calling logic, not tree structure); `crates/steg/tests/cli_exit_statuses.rs` (new — one test per status, real binary, fixture server); `crates/steg/tests/cli_connection_arbitration.rs` (new — AC7's own test). Test helpers (`unique_socket_path`, `KillOnDrop`, `wait_for_socket`, `wait_bounded`, `read_one_line_bounded`) are lifted from `crates/steg/tests/server_start.rs` into a shared `crates/steg/tests/support/mod.rs` rather than duplicated — named here since `server_start.rs` doesn't currently expose them as a library.
- **Risks:** status 1 needs a method that genuinely fails at the application level without depending on TASK-1.88/1.90/1.110/1.111 landing first — any of the 42 described-but-undispatched methods (for example `pane resize`, not yet built per TASK-1.110) already answers `ErrorCode::NotImplementedYet` (exit 1) from today's dispatch, which is what this task's test uses; named so a later reviewer doesn't read it as accidentally depending on those tasks. Status 130 needs a fixture server that accepts a call and never answers it, so the real `steg` process is genuinely blocked when SIGINT is sent — a new, small fixture, not reused from elsewhere.

### 3. `TASK-1.49c` — CLI: output formats — plain text, `--json`, and `-q`
- **Criteria:** #2 (the flags every command has, `--json`, and `-q` printing identifiers only, each behaving as doc-2 says); #5 (a list is tab-separated, full identifiers, no header, no alignment, identical piped or not).
- **Design:** one renderer, matched exhaustively on `Returns` (so the match itself breaks when `Nested` is added, per engineer decision 5 above) — `Nothing` prints nothing; `Identifier` prints the one id alone; `Record`/`Records` print tab-separated fields in the description's own order, one record per line for `Records`, with **no branch on whether stdout is a terminal** (section 1's "no behaviour that depends on whether a person is watching," applied literally: alignment and a header are never produced, not conditionally suppressed); `Text` prints as-is. `--json` serialises the raw `Response::Ok(Value)` unchanged — the API's own field names, by construction, never a second list of them. `-q` prints each record's full identifier alone, one per line, or (unchanged) the one new identifier an action already prints; with `--json` together, the identifiers become one JSON array (decision-4's own orchestrator-filled answer, listed as settled, not open). Colour is the one place terminal-ness matters at all (section 6), and stays out of this split's own tests, since no command this task generates produces colour yet.
- **Files:** `crates/steg/src/cli/output.rs` (new — the renderer); `crates/steg/tests/cli_output_formats.rs` (new).
- **Risks:** proving "identical piped or not" needs a real pty, not just two `Command::output()` calls (which are both already "not a terminal"). `steg-server` already depends on `portable-pty`; add it as a **dev-dependency only** to `steg` (fixture binaries can't see dev-dependencies, but this is an ordinary `#[test]`, which can) to drive the CLI under a real pty and compare its bytes against the same command piped — named here since it is the one new test technique this plan introduces that nothing in the repository already does.

**Order:** 1 → 2 → 3. Both 2 and 3 build on 1's tree (there is nothing to attach a connection or a renderer to otherwise). 3's own tests need a working request/reply round trip to render anything real, which 2 builds — so while 2 and 3's *logic* don't depend on each other, 3's proof does, hence the strict order rather than running 2 and 3 in parallel.

## Tests, across all three

Every test proving a criterion runs the real `steg` binary against a real fixture server, never this machine's own server or start-up files — a throwaway socket (`unique_socket_path`, already in `server_start.rs`, kept short against `sockaddr_un`'s 104-byte budget) and, wherever a server needs to start a program or resolve an identity, a throwaway home (the pattern `login_environment.rs`/`fixture_server.rs` already establish). No test sleeps to wait for an event; every wait is bounded and polls the actual condition (`wait_for_socket`, `wait_bounded`), per the team testing rule and this crate's own existing exception for "a socket appearing at a path has no notification." Every process this plan's own tests start is ended and reaped on every path, via the existing `KillOnDrop`/`ServerAtSocket` patterns (the latter needed for anything reached only through the sdk's own `connect()`, which never hands back a `Child`).

Negative controls, named per the mechanism they prove:
- The drift test (1): run with a method manually left out of a cloned tree, and with an extra bare clap subcommand added — both must fail it, proving it actually catches drift rather than vacuously passing.
- The exit-status mapping (2): run against a cloned `description()` with one `ErrorRecord.exit_status` changed, confirming the mapped status changes with it (proves the mapping is read from the description, not hard-coded restating it).
- AC7's retry (2): confirmed to fail without the fix by running the identical accept-then-close fixture against today's unmodified `local_socket::connect`, which currently reports `NoServerRunning` only on a refused connection, never on an accepted-then-closed one — recorded before the fix lands, not asserted from memory.
- The pty byte-identity test (3): run once against a version that forgot to drop the "is this a terminal" branch, confirming it catches a reintroduced header/alignment.

Items this plan's proof would otherwise list but is naming as dropped, not silently skipped: the `--host` flag (M3, doc-2 section 5) and `config`/`plugin`/`action`/`status`/`host` nouns (M2/M3) — none are in `description()` today, so the drift test has nothing to check them against; TASK-1.51's own context variables (`STEG_SESSION`/`STEG_TAB`/`STEG_PANE`) are explicitly that task's, so "no target and no context" (doc-2 section 4, status 2) is tested here only as "no target, no context available at all" (since this task wires no context yet), not as "a context variable present but a target still missing."
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Dependency added by @bruk.o 2026-11-15: TASK-1.77 adds the steg entry point for server start and starting a server on demand, which the generated CLI needs to reach a server at all and must keep as its one hand-written case.

Carried from TASK-1.75's build (2026-11-15), for this task's plan to settle against doc-2: (1) pane.move calls Tree::move_pane_to_tab, which needs a direction that doc-2 section 8 gives pane move no parameter for; dispatch defaults to the right. (2) unset --label requires a value to be sent that dispatch then discards (the description declares the parameter with a value). (3) the api.call target collision is its own draft task.

Carried from TASK-1.77's planning (2026-11-15): when a server is stopping, a connection the accept loop takes in the instant it sees the stop flag is dropped with no reply, so the client sees connect succeed and then end of file, a third outcome beside refused and not found. The CLI's client must treat end of file before a server hello as no server there, and start one, exactly as it treats a refusal; say whether that belongs in the sdk's local-socket provider (which today only waits for the socket) or in the handshake's caller.

TASK-1.89 (tab list, tab show, pane list) and TASK-1.88 (server and client methods) were accepted on 2026-11-15 for methods the generated commands will call; this task's own tests should use methods that exist when it is built and need not wait for those.

Plan (fdbb9acfc676) checked against doc-2, decision-1, decision-4, doc-16 and the seven criteria by @bruk.o, 2026-11-16: sound; the split into three is taken (two drafts created: connecting, calling and exit statuses; output formats). This task keeps the generated command tree, help, version and completion. The orchestrator asked for one change to the split: the connecting task prints the server's reply with --json, so the CLI is usable by hand when it lands, before the output formats. For the engineer: the plan's five small decisions, all recommended as the worker does (steg and steg <noun> alone list and exit 0, as doc-2 says; doc-2 gains a line for api show with no target; unset --label becomes a switch in the description; pane move's missing direction is left; a nested value prints whole with --json and as its scalar fields in plain text).
<!-- SECTION:NOTES:END -->
