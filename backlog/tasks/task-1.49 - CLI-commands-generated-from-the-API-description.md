---
id: TASK-1.49
title: 'CLI: commands generated from the API description, in the steg package'
status: In Progress
assignee:
  - '@bruk.w5'
created_date: '2026-11-15T01:33:00Z'
updated_date: '2026-11-16T13:55:00Z'
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
asdlc_plan_approved_by: bruk
asdlc_plan_approved_hash: '37c4dd0b7f74'
asdlc_plan_approved_size: size-4
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
doc-2, sections 2 to 6 and 8: R32, R34. decision-4. Nothing in the CLI names a method by hand.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Every API method has a command and every command an API method, checked by a test that fails when they drift
- [ ] #2 The CLI lives in the `steg` package and reaches the server only through the sdk (doc-16 R98, R101)
- [ ] #3 steg version, steg completion <shell> and --help at every level work with no server running and without starting one
- [ ] #4 A command that parses and validates but cannot yet connect exits 1 with a message naming the method, until TASK-1.119 replaces that; an invalid one exits 2
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
This task is now part 1 of the split (the command tree generated from the API description, help, version and completion, the drift test, the boundaries edit). Parts 2 ("CLI: connecting to the server, calling a method, and the exit statuses") and 3 ("CLI: output formats: plain text, --json and -q") are their own tasks; their designs are kept below as an appendix for whoever plans them, not built here.

## For the engineer

Five decisions, each with a recommendation; decision 6 (splitting) is already taken.

1. **`steg` and `steg <noun>` alone: list (status 0) or fail (status 2)?** doc-2 section 2 says plainly both list; decision-1 section 7 records that the TASK-1.4 spike instead made both fail, status 2, and flags this as doc-2's to pin. **Recommendation: follow doc-2's literal text — both succeed, status 0.** **This plan is built against that recommendation** (noted below, wherever it matters); if overruled, it is a small, localised change to the tree-building step, not a redesign.
2. **`api.show`'s target is optional in the description, required-reading in doc-2.** `description::methods::api_methods` already declares it `required: false` ("with none, the whole description"), and dispatch already implements that. **Recommendation: doc-2 section 8 gains a line for the no-target form** — a documentation catch-up; the generator follows whichever `required` the description declares either way, so this never blocks the build.
3. **`unset --label` takes a value it discards.** Taken into this task's own build, conditional on the engineer's agreement — see "The `unset --label` switch" below, its own marked step.
4. **`pane.move` has no `direction` parameter; dispatch defaults to the right.** Nothing for this task to do; the generator only ever reflects the description's own params. Flagged here only because it was carried onto this task's planning.
5. **A nested value's plain-text rendering** (TASK-1.46's unmerged `Returns::Nested{of, summary}`). Not this task's concern directly — part 1's own renderer doesn't exist yet (that's part 3) — but recorded here since the recommendation travels with the other four: `--json` carries it whole, plain text prints only the record's scalar fields (the same treatment `tab_fields`'s own doc comment already gives `layout`/`shown_status`). Repeated in the appendix for part 3's own plan to start from.

## What this plan is built against, and what changes under it

`main` today has 42 methods, no `Method.follows` field, no `Returns::Nested` variant, no `pane.attach`/`pane.detach`. TASK-1.49's own dependency graph (`TASK-1.45`, `TASK-1.77`) does not include TASK-1.46 or TASK-1.47, so this plan builds and proves itself against today's shapes. Nothing here hard-codes 42: the drift test (criterion 1) is a set comparison over whatever `description().methods` holds at build time.

Two things 1.46 adds that this design must not be surprised by, so a later rebase costs no redesign:

- **`Method.follows` (1.46): `Nothing`, `Events(Returns)`, or `Attachment{item, stream_id_field}`.** Until it lands, every method's follow-on behaviour is `Nothing` by construction. Once it lands: a method whose `follows` is `Attachment` (`pane.attach`) is the stated exception to criterion 1 (R32: listed and described, reachable through `api call`, no rendering of the stream as a command) — the drift test's set comparison must then carry an explicit, narrow carve-out for exactly the methods whose `follows` is `Attachment`, itself asserted to be exactly that set. A method whose `follows` is `Events` (`event.watch`) still gets a generated command (noun, verb, flags, target, validated request) here; this task's own one-shot seam (below) covers it the same as any other method until it exists — TASK-1.50 is the one that reads the stream of events that follow and prints one record per line.
- **`Returns::Nested` (1.46).** Not this task's renderer to write (part 3's), named here only so whoever rebases this task's own drift-test code isn't surprised that `Returns` grows a variant later.

## The design: a clap `Command` tree built at run time from `Description`

Confirmed in Cargo.lock: no `clap` in the workspace today (`pico-args`/`arg_enum_proc_macro` are present only transitively, through gpui's own tree, unrelated). `deny.toml` already allows `Unicode-3.0`, the licence decision-1 flagged as needed for `syn` via clap's derive feature — not used here, since this uses clap's **builder** API, never `#[derive(Parser)]`: a derive names every command in a struct literal, one hand-written type per method, exactly what "nothing names a method by hand" forbids. The generator walks `description().methods()` in a loop building `Command`/`Arg` values from data.

**`boundaries.toml`'s `[steg]` entry needs editing first**: add `"clap"` and `"clap_complete"` to its `allow` list — both are needed in this task, since `completion <shell>` (built here, not in part 2 or 3) is what uses `clap_complete` — with a reason citing decision-1 and this task. `just check-boundaries` fails on an unlisted dependency otherwise — a first-step edit, not a cleanup. **Negative control for this check:** run `just check-boundaries` with the `[steg]` edit reverted, confirming it fails — proves the check is actually what blocks an unlisted dependency, not merely silent.

**Pin `clap = "=4.6.7"` exactly** (not a range) — the version decision-1's own licence table measured, and the one this plan's own verification below was run against.

**Verified, correcting decision-1 section 4's own finding:** checked directly against clap 4.6.7's published API (docs.rs, not assumed): `Command::new(name: impl Into<Str>)`, `Arg::new(id: impl Into<Id>)`, and `PossibleValue::new(name: impl Into<Str>)` each take `impl Into<...>`, and `clap::builder::Str` and `clap::Id` **both implement `From<String>`** in this exact version — confirmed by reading their generated trait-implementation lists, not inferred. decision-1's claim that "its `Str` and `Id` have no `From<String>`" does not hold for 4.6.7 as published; whatever the spike actually observed, this plan is built on the checked-today fact, not the carried claim. **Consequence: no leaking to `'static` anywhere in the generator.** Every `Command`/`Arg`/`PossibleValue` is built from owned `String`s cloned out of the `Description` (built once — a `std::sync::OnceLock<Description>` or simply constructed once in `main` and passed down by reference is enough; nothing needs `Box::leak`). This removes a whole category of "why this leak is sound" reasoning the earlier draft of this plan carried; nothing in this task's build needs it.

**Mapping the description to a tree, mechanically** (decision-1 section 2's own table; unchanged from the earlier draft):
- One `Command` per noun (`description().nouns()`, first-appearance order), one subcommand per verb (`description().verbs_of(noun)`), named exactly the method's noun/verb.
- `summary` → `about`; `details` → `long_about`.
- `Role::Target` → the one positional `Arg`, required per the param's own `required`.
- `Role::Flag` → `--name-with-hyphens <VALUE>` (`Param::flag()`, already on the type), required per the param.
- `Role::Switch` → two boolean args, `--name` and `--no-name` (`Param::negated_flag()`, already on the type), grouped so the last one given wins and both are always accepted (doc-2 section 5).
- `Role::Program` → a trailing variadic arg consuming everything after a literal `--` (doc-2 section 9).
- `ValueKind::Choice(words)` → a `PossibleValuesParser` built from the field's own words — one list, used for CLI validation, help text and (already) the server's own validation.
- `ValueKind::Integer`/`Boolean`/`Text`/`Path`/`Target`/`Argv` → clap's own value parsers (`Path` as `PathBuf`; `Target`/`Text` as plain strings — target *resolution* is server-side, R10).
- `api list`/`api show`/`api call` are generated exactly like every other method — no special case in tree-building. `api call`'s own stdin-JSON behaviour belongs to part 2, not this task.
- `version` and `completion <shell>` are added once, by name, outside the loop — the drift test asserts the top-level command set is exactly `description().nouns() ∪ {"version", "completion"}`, so a third hand-added command fails it.
- `steg` alone and `steg <noun>` alone: built against decision 1's recommendation (list, exit 0) — each prints its own list (nouns; that noun's verbs) explicitly, rather than relying on clap's own "missing subcommand" behaviour (which defaults to exit 2 and would need overriding either way this decision goes).

## What part 1 proves about a command it cannot yet run

There is no connecting logic in this task (that is part 2's). A parsed, locally-valid command needs somewhere to go that is not silently nothing, so this task defines one seam, meant to be replaced outright by part 2, not built around:

```
fn run(method: &Method, params: Map<String, Value>) -> ExitCode
```

implemented here as a fixed placeholder: prints nothing on stdout, prints `steg: {noun} {verb} is valid, but connecting to a server is not built yet (TASK-1.49b)` on stderr, and **exits 1**. Reasoning for 1, not 0 or 2: nothing succeeded (ruling out 0), and the command and its arguments were entirely valid — local parsing and validation against the description's own value kinds (closed sets, required-ness, switch pairs, the trailing program) already ran and passed (ruling out 2, "bad usage"). 1 ("the command ran and failed") is doc-2 section 6's literal fit for "parsed and validated, then could not proceed."

**This is what makes part 1 a reviewable whole rather than a tree nobody can exercise, and it is tested as such:**
- A test enumerating every described method (the same set the drift test already computes) builds one valid invocation per method (using each param's own declared kind to synthesise a legal value — a `Choice`'s first word, `0` for `Integer`, `true` for `Boolean`, a throwaway path for `Path`, an empty program for `Argv`) and asserts: exit 1, the placeholder message naming that exact method, nothing on stdout.
- The same test, with one param deliberately wrong per kind (a word outside a `Choice`'s set, a required flag left out), asserts clap's own usage error instead: exit 2, nothing on stdout — proving local validation actually runs before the placeholder is ever reached, not just that the placeholder always fires.
- **Negative control:** run both halves of this test against a version of the placeholder that returns 0 unconditionally — the first half must then fail (it no longer sees exit 1), proving the test actually distinguishes "reached the placeholder" from "exited 0 for any reason."

## The `unset --label` switch (conditional on the engineer's agreement)

Checked, not assumed: `crates/server/src/dispatch/{session,tab,pane}.rs`'s own `unset` functions already use `param_present(request, "label")` — presence alone, regardless of value — to decide whether to clear the label. `pane.rs`'s `unset` carries this exact comment already, for its `zoomed` switch: *"`unset` always clears, regardless of the switch's own boolean value - the same rule governs `unset --label` above."* **So dispatch needs no change at all** — changing `label`'s role from `Flag` to `Switch` in the description is the whole fix; `param_present` already generalises correctly, by design, to a switch whose value is discarded either way (`--label` or `--no-label` on `unset` both clear it, exactly like `--zoomed`/`--no-zoomed` already do). The earlier draft of this plan assumed a dispatch change would be needed; it would not have been, and is not part of this step.

- **Files:** `crates/sdk/src/description/methods.rs` — `session_methods()`, `tab_methods()`, `pane_methods()`'s own `unset`: `flag("label", ValueKind::Text, false, ...)` → `switch("label", ...)`. No change to `crates/server/src/dispatch/*.rs`.
- **Expected mechanical conflict:** TASK-1.46's unmerged branch adds a field (`follows`) to every `Method` literal in this same file. Whichever of the two tasks merges second resolves an ordinary field-by-field conflict there — not a design collision, named so it isn't mistaken for one.
- **Tests:** a sdk-crate test asserting `session.unset`/`tab.unset`/`pane.unset`'s `label` param has `role: Role::Switch` (not `Flag`) — parametrised over the three methods, one assertion body. A dispatch-crate test (already exercisable without this task's own tree) proving `--label` and `--no-label` both clear the label on `unset`, and neither flag given leaves it unchanged, for each of session/tab/pane — this test passes already today against the *current* `Flag`-typed param if the request is built by hand with `"label": true`/`"label": false"/absent (since dispatch doesn't care about the value's type, only presence), so it is written and run **before** the description edit, confirmed passing, then re-run unchanged **after** the edit — the same request shapes, now reachable through the real generated `--label`/`--no-label` switches instead of only by hand — proving the description change didn't move the goalposts.
- **If overruled:** skip this step; `label` stays a value-taking `Flag` on `unset`, the generator reflects that unchanged, and nothing else in this task's design is affected.

## Tests, generally

Every test proving a criterion runs the real `steg` binary (once `main.rs` is wired to the generator) or the generator's own library code directly where no process is needed (the drift test, the placeholder-seam test, the switch-shape test). No test sleeps to wait for an event. A control proves the test it was run against, in the form that test had then (the team testing rule) — if any of this task's own tests is edited later (a fix, a rewritten fixture), its control is re-run against the edited version before being cited again, not assumed to still hold.

Named controls, in addition to the two already given above:
- The drift test: run with a method manually left out of a cloned tree, and with an extra bare subcommand added — both must fail, proving it catches drift rather than vacuously passing.
- `--no-server` / `version` / `completion` / `--help` tests: run once with `STEG_SOCKET_PATH` pointed at a path that *would* answer if touched (a throwaway listening socket), confirming the test would have failed had any of these commands actually reached for a connection — proving "no server running" wasn't simply untested.

Items named as dropped, not silently skipped: the `--host` flag and `config`/`plugin`/`action`/`status`/`host` nouns (M2/M3) — none are in `description()` today, so the drift test has nothing to check them against. TASK-1.51's own context variables (`STEG_SESSION`/`STEG_TAB`/`STEG_PANE`) are explicitly that task's; this task wires no context, so every target left blank is simply absent, left for the server (or TASK-1.51's own "no target, no context" check) to answer.

## Appendix: parts 2 and 3, moved to their own tasks — for their own planning to start from, not built here

### Part 2 — CLI: connecting to the server, calling a method, and the exit statuses
Replaces this task's own `run(method, params) -> ExitCode` placeholder outright with the real path: `steg_sdk::local_socket::connect`, the real hello (`ClientKind::Cli`), `Connection::call`, reading the reply, and — per the engineer's own addition to the split — **printing the server's reply unchanged with `--json` as part of this task**, so the CLI is usable by hand the moment it lands, before the output-formats task.
- **Criteria:** the `--autostart`/`--no-autostart` half of #4; #3 (every exit status in doc-2 section 6, each produced by a test); #7 (a connection accepted and then closed before a hello arrives treated as no server there, arbitrated once more, shown with real processes).
- **Where AC7's logic lives:** the sdk's `local_socket::connect` (R101: no client holds protocol code of its own; the app needs this too). Extend it to read the first frame after a successful connect and, on EOF before a complete hello, treat that identically to "nothing was listening," retrying the whole find-or-start arbitration **exactly once**, bounded by the type, not an open loop.
- **The fixture needed:** independent of TASK-1.86 (in review, PR #60) merging first — a small fixture that accepts a connection and closes it immediately, with no hello, reproduces the client-side condition on its own. If 1.86 has merged by build time, its real server-stopping scenario is additional, stronger end-to-end evidence, not a substitute.
- **Exit status mapping, never a hand-written table:** `ApiError.code` looked up against `description().errors`'s own `ErrorRecord.exit_status`. CLI-local failures (clap's own usage errors, `--no-autostart` with nothing running) map to 2/4 directly, never through a server round trip. 130 is a SIGINT handler installed once at start.
- **A natural status-1 test, with no dependency on unbuilt methods:** any of the 42 described-but-undispatched methods (for example `pane resize`, TASK-1.110) already answers `ErrorCode::NotImplementedYet` (exit 1) from today's dispatch — named so a later reviewer doesn't read the test as depending on 1.88/1.90/1.110/1.111 landing.
- **Files:** `crates/sdk/src/local_socket.rs` (extend `connect`); `crates/steg/src/cli/connect.rs` (new); `crates/steg/src/cli/exit.rs` (new); `crates/steg/src/cli/api_call.rs` (new — `api call`'s stdin-JSON behaviour); `crates/steg/tests/cli_exit_statuses.rs`, `crates/steg/tests/cli_connection_arbitration.rs` (new). Test helpers (`unique_socket_path`, `KillOnDrop`, `wait_for_socket`, `wait_bounded`, `read_one_line_bounded`) lifted from `crates/steg/tests/server_start.rs` into `crates/steg/tests/support/mod.rs` rather than duplicated.

### Part 3 — CLI: output formats: plain text, `--json` and `-q`
- **Criteria:** #2 (the flags every command has, `--json`, and `-q` printing identifiers only); #5 (a list is tab-separated, full identifiers, no header, no alignment, identical piped or not).
- **Design:** one renderer, matched exhaustively on `Returns` — `Nothing` prints nothing; `Identifier` prints the one id alone; `Record`/`Records` print tab-separated fields in the description's own order, with no branch on whether stdout is a terminal (section 1's principle, applied literally — never conditionally suppressed); `Text` as-is. `--json` serialises the raw reply unchanged — already partly done by part 2's own addition. `-q` prints each record's full identifier alone, or the one new identifier unchanged; with `--json` together, the identifiers become one JSON array (decision-4's orchestrator-filled answer, settled). A nested value (TASK-1.46's `Returns::Nested`, see "For the engineer" item 5 above): `--json` carries it whole, plain text prints the record's scalar fields only.
- **Files:** `crates/steg/src/cli/output.rs` (new); `crates/steg/tests/cli_output_formats.rs` (new).
- **Risk to carry forward:** proving "identical piped or not" needs a real pty, not two `Command::output()` calls (both already "not a terminal"). `steg-server` already depends on `portable-pty`; add it as a **dev-dependency only** to `steg` to drive the CLI under a real pty and compare bytes against the same command piped.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Dependency added by @bruk.o 2026-11-15: TASK-1.77 adds the steg entry point for server start and starting a server on demand, which the generated CLI needs to reach a server at all and must keep as its one hand-written case.

Carried from TASK-1.75's build (2026-11-15), for this task's plan to settle against doc-2: (1) pane.move calls Tree::move_pane_to_tab, which needs a direction that doc-2 section 8 gives pane move no parameter for; dispatch defaults to the right. (2) unset --label requires a value to be sent that dispatch then discards (the description declares the parameter with a value). (3) the api.call target collision is its own draft task.

Carried from TASK-1.77's planning (2026-11-15): when a server is stopping, a connection the accept loop takes in the instant it sees the stop flag is dropped with no reply, so the client sees connect succeed and then end of file, a third outcome beside refused and not found. The CLI's client must treat end of file before a server hello as no server there, and start one, exactly as it treats a refusal; say whether that belongs in the sdk's local-socket provider (which today only waits for the socket) or in the handshake's caller.

TASK-1.89 (tab list, tab show, pane list) and TASK-1.88 (server and client methods) were accepted on 2026-11-15 for methods the generated commands will call; this task's own tests should use methods that exist when it is built and need not wait for those.

Plan (fdbb9acfc676) checked against doc-2, decision-1, decision-4, doc-16 and the seven criteria by @bruk.o, 2026-11-16: sound; the split into three is taken (two drafts created: connecting, calling and exit statuses; output formats). This task keeps the generated command tree, help, version and completion. The orchestrator asked for one change to the split: the connecting task prints the server's reply with --json, so the CLI is usable by hand when it lands, before the output formats. For the engineer: the plan's five small decisions, all recommended as the worker does (steg and steg <noun> alone list and exit 0, as doc-2 says; doc-2 gains a line for api show with no target; unset --label becomes a switch in the description; pane move's missing direction is left; a nested value prints whole with --json and as its scalar fields in plain text).

Plan (37c4dd0b7f74) as rewritten for the command tree alone, checked by @bruk.o, 2026-11-16: ok. Covers criterion 1 (the drift test in both directions), the no-server half of criterion 4, and criterion 6; criteria 2, 3, 5, 7 and the rest of 4 are the two split-off tasks'. Includes, as a marked step, the description change making unset --label a switch (description only; dispatch already acts on presence). A command that parses and validates but cannot yet connect exits 1 with a named message until the connecting task replaces that. clap pinned at 4.6.7, builder only.

Check passed: `just check` exited 0 at 2026-11-16T13:55:27+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.49 on task-1.49-cli-commands-generated-from-the-api at 0fde255eb1c0:

```text
ok      format check (0.2s)
ok      lint (90.8s)
ok      boundaries (0.2s)
ok      foundations (0.3s)
ok      foundations public surface (0.1s)
ok      design pages (0.1s)
ok      licences (1.1s)
skip    tests: this change touches no crate
ok      build (steg-sdk) (3.6s)
ok      test (steg-sdk) (2.3s)
ok      test (steg-sdk, local-socket) (2.1s)
ok      build (steg-kernel) (46.6s)
ok      test (steg-kernel) (1.5s)
ok      build (steg-server) (44.0s)
ok      test (steg-server) (12.3s)
ok      test (steg-server, race-fixture) (13.2s)
ok      build (steg) (1.9s)
ok      test (steg) (0.4s)
ok      test (steg, fixtures) (4.4s)
ok      build (steg-ui) (19.8s)
ok      test (steg-ui) (1.2s)
ok      build (steg-app) (2.2s)
ok      test (steg-app) (0.9s)
ok      tool tests (12.7s)
ok      architecture (0.7s)
ok      recipe names (0.1s)
ok      ui levels (0.1s)
ok      ui colours (0.1s)
```
<!-- SECTION:NOTES:END -->

## Comments

<!-- COMMENTS:BEGIN -->
author: @bruk
created: 2026-11-16T13:50:00Z

---

Plan approved by @bruk (37c4dd0b7f74, size-4): its worker may build it.

---

created: 2026-11-16T13:51:00Z

---

2026-11-16, @bruk.o: criteria narrowed to the split the engineer accepted. TASK-1.119 has connecting, calling, the exit statuses and the connection closed before a hello; TASK-1.120 has the output formats. The approved plan calls them parts 2 and 3 (and TASK-1.49b in one message): cite TASK-1.119 and TASK-1.120 in code and notes.

---
<!-- COMMENTS:END -->
