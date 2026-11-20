---
id: TASK-1.120
title: 'CLI: output formats: plain text, --json and -q'
status: In Progress
assignee:
  - '@bruk.w8'
created_date: '2026-11-16T13:35:00Z'
updated_date: '2026-11-20T17:25:00Z'
labels:
  - size-2
milestone: m-3
dependencies:
  - TASK-1.49
  - TASK-1.119
parent_task_id: TASK-1
project: bruk-io-steg
asdlc_accepted_from: DRAFT-3
asdlc_branch: task-1.120-cli-output-formats-plain-text-json-and-q
asdlc_plan_approved_by: bruk
asdlc_plan_approved_hash: 'e49be2b088c1'
asdlc_plan_approved_size: size-2
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Split from TASK-1.49 by its plan on 2026-11-16. doc-2 sections 1, 5 and 6; decision-4. TASK-1.49's plan, section 'The split', part 3, holds the worked design. Carries TASK-1.49's criteria 2 and 5. Builds on the task that connects and calls (the draft beside this one): set that dependency when both have ids.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The flags every command has, --json, and -q printing identifiers only each behave as doc-2 says, for every kind of result the API description declares, matched without a catch-all so a new kind of result fails the build until it is handled
- [ ] #2 A list is tab-separated with full identifiers, no header and no alignment, and is byte for byte the same on a terminal and piped, shown by running the real binary under a real pty and through a pipe
- [ ] #3 A nested result is printed whole with --json; its plain-text form is as the engineer decided with TASK-1.49's plan
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
## Summary

One new module (`crates/steg/src/cli/output.rs`): a pure function, matched exhaustively on `steg_sdk::Returns` (no catch-all), from a method's declared `Returns` plus the raw `serde_json::Value` reply to the bytes doc-2 section 6 wants - no server, no process, unit-tested directly. `connect.rs::run` calls it in place of today's `if json { println!("{value}") }`. One new global flag (`-q`/`--quiet`, `tree.rs`, same shape as `--json`/`--autostart`). One new test file (`tests/cli_output_formats.rs`) proving criteria 1-3 through the real binary. One existing test's docstring/scope corrected (`cli_exit_statuses.rs`). No split: one module, one test file, one flag, one small edit to an existing test - smaller than TASK-1.49's own single-task scope.

**Revision note:** this revises the plan reported earlier against the engineer's six answers (below, "Decisions"). Branch rebased onto `origin/main` first (`git rebase origin/main`, clean, no conflicts - this branch had no commits of its own yet, so it was a no-op fast-forward). Re-checked after the rebase: `pane_fields`/`server_fields`/`client_fields`/`session_fields`/`setting_fields`/`api_method_fields` and every `Returns` variant are unchanged from what this plan was built against (same six `Returns` kinds, same field lists) - server show/stop, client list/show and pane capture moving onto main changed their *dispatch*, not the *description* this task reads from, so nothing in the table below needed a shape correction, only the six decisions below.

## The renderer: one pure function

```rust
// crates/steg/src/cli/output.rs
pub fn render(returns: &Returns, value: &Value, json: bool, quiet: bool) -> Result<Vec<u8>, RenderError>
```

No I/O, no clap, no sdk connection types - `Returns` and `serde_json::Value` in, bytes out. `connect.rs::run` writes the bytes to stdout (or nothing, for an empty `Vec`) and keeps its own `eprintln!`/exit-status job for the `Err` case. Matched exhaustively on `Returns`'s six variants - the workspace already denies a wildcard match arm, so a seventh `Returns` variant fails the build with no action needed from this task to make that true (negative control: add a dummy seventh variant to a scratch copy of `shapes.rs`, confirm `cargo build -p steg` fails on `output.rs`'s own match, revert).

A field's own value is rendered by matching `serde_json::Value`'s variants directly, never `ValueKind` - this is the one design choice that removes a whole class of special-casing. `tab_fields`'s `layout`/`shown_status` and `pane_fields`'s `status`/`size` are declared `ValueKind::Text` but their real JSON value is an object, an array, or null (their own doc comments say so: "both stay `ValueKind::Text` placeholders"). Matching the real `Value` instead of the declared `ValueKind` renders every one of these correctly with no per-field-name special case:

| `Value` at that field | Printed in that column |
|---|---|
| `String(s)` | `s`, with any `\t`/`\n`/`\r` replaced by a single space (decision 6) |
| `Number(n)` | its decimal text |
| `Bool(b)` | `true`/`false` |
| `Null`, or the key absent entirely | empty (the column stays; count never changes - see "missing field" below) |
| `Array`/`Object` | the value's own compact JSON text (`serde_json::to_string`, never pretty), same control-character replacement applied to the result |

`--json` never applies this replacement - it keeps the exact value, serde's own escaping of `\t`/`\n` inside a JSON string being that format's own business, not this renderer's (decision 6).

## The table: every `Returns` kind, its three forms, tied to doc-2 and the six decisions

| `Returns` | Plain text | `--json` | `-q` | doc-2 / decision |
|---|---|---|---|---|
| `Nothing` | Nothing | **Nothing** (decision 3) | Nothing | §6 "An action... is silent"; §5 `-q` row, third clause |
| `Identifier { of }` | The `"id"` field's string, alone on a line | The reply object unchanged (`{"id": "..."}`) | Same as plain - "unchanged" (§5) | §6 "prints its full identifier alone"; §5 `-q` row, second clause |
| `Record { fields }` | One line, each `fields`-order column tab-separated | The reply object unchanged | `fields[0]`'s value alone; nothing if `fields` is empty (decision 4) | §6 third bullet; §5 `-q` row, first clause |
| `Records { fields }` | One tab-separated line per array element, `fields` order, nothing printed for zero elements | The reply array unchanged | `fields[0]`'s value per line (nothing per record with zero fields - decision 4); `--json` together: one JSON array of them (§5's own orchestrator-filled note) | same, plural |
| `Text` | The string, unchanged | The reply value (a JSON string) unchanged | **Unchanged** - no reduction (decision 5) | §6 names `Text` as an exception already |
| `Nested { of, summary }` | **The reply, as JSON** - unconditionally, same bytes as `--json` (decision 1) | The reply unchanged (so identical to the plain-text column for this one kind) | No reduction - a direct consequence of decision 1: once plain text already *is* the whole JSON reply, there is no record to cut down to an identifier from, the same shape decision 5 gives `Text` | doc-2 §3's `pane attach` row already said this for one `Nested` method ("the reply carries the stream's identifier and a snapshot, as JSON", not gated on `--json`) - decision 1 generalises it to every `Nested` reply (`event.watch`'s `TreeSnapshot` included) |

**`fields[0]` is not a new lookup rule - it coincides with every record's own `"id"` field.** Checked directly (session/tab/pane/client's own `_fields()` functions): `"id"` is declared first in every one of them. `setting_fields`/`api_method_fields`/`server_fields`/`pane.wait`'s own `Record{fields:[exit_code]}` have no `"id"` at all, and decision 4 settles exactly those: `-q setting list` prints each setting's `name`; `-q api list` prints each method's dotted name; `-q server show` prints the version string alone; `-q pane wait` prints the exit code alone. One index, no per-method table, no `Field`-level marker needed.

A reply whose actual shape doesn't match its method's declared `Returns` (for instance `Records` declared but the server answered an object) is `Err(RenderError::Shape)`, never a panic - `connect.rs` maps it through `connect_error_status(&ConnectError::Io(...))`, reusing the bucket `block_until_reply`'s own `Incoming::Malformed` arm already uses for "the server said something that isn't a valid reply," exit 4. Proved as a defensive case with a hand-built mismatched `Value` in `output.rs`'s own unit tests, not as a reachable scenario through a real server (one `Description` compiled into both sides).

### Missing field, not missing column

`pane_fields.exit_code` is absent from a running pane's own JSON reply (the field only exists once the program has ended). Byte-identical plain text needs a fixed column count regardless, so a missing key renders as an empty column between its two tabs, never a dropped column.

## Flags: what this task reads, and what it leaves alone

`tree.rs` today has `--autostart`/`--no-autostart`/`--json`, each `.global(true)`. This task adds `-q`/`--quiet` the same way (`QUIET_ID`, `ArgAction::SetTrue`, `.short('q')`, a `quiet_flag(matches)` reader beside `json_flag`) - doc-2 §5 restricts short flags to `-h`/`-q`/`-v`, so `-q` is the only short form this task introduces.

Criterion #1's own text is "**the flags every command has, `--json`, and `-q`** ... each behave as doc-2 says" - read as naming exactly those two, not every row of §5's seven-flag table. The other four:

- **`-v`/`--verbose`** ("diagnostics on stderr"): nothing in this task's own design has diagnostics to gate - `eprintln!`'s already in `connect.rs`/`api_call.rs` are unconditional error messages, not the extra diagnostics `-v` is for. Not added here.
- **`--no-color`**: no colour exists anywhere in the CLI yet. Out of scope by construction - belongs to whichever task first colours something.
- **`--no-input`** ("already the default"): true by construction already; no flag needed to make a true statement true.
- **`--host`**: doc-2's own "Decided... Saved hosts are left for M3" - out of scope by the document's own text.

Leaving these four unregistered with clap means giving one today is a clap usage error (exit 2) until whichever task adds it - the existing state before this task too, unchanged by it.

## `api call`'s own exceptions

`api_call.rs` already documents why its own output is never gated on `--json`. `-q` gets the same treatment, for the same structural reason: `api.call`'s target method may be one "a plugin adds before the CLI knows it" (`api.rs`'s own doc comment), so there may be no `Method`/`Returns` at all to read `fields[0]` from. `-q` is read and silently ignored on `api call`, exactly like `--json` already is - one added line to `api_call.rs`'s module doc, no behavioural code change.

## Where rendering lives, and the three tasks that touch this crate's `cli/` module

- `crates/steg/src/cli/output.rs` - new, this task's own, no server/clap/sdk-connection types in its signature.
- `crates/steg/src/cli/tree.rs` - one new global flag + reader, same shape as the two already there.
- `crates/steg/src/cli/connect.rs` - the `if json {...}` block replaced by a call to `output::render` and a write of its bytes (or none); the error arm maps through `exit.rs`.
- `crates/steg/src/cli/api_call.rs` - one doc-comment line, no behaviour change.
- `crates/steg/src/cli/exit.rs` - no new function needed (reusing `connect_error_status`), one added sentence on `ConnectError::Io`'s own doc comment naming the new caller.

TASK-1.51 (acting on the terminal the CLI runs in) and TASK-1.50 (send/capture/wait/watch) are both still `mod.rs`-level siblings; neither needs a file this task touches, and this task adds no file either of them would need. The one shared surface is `connect.rs::run`'s single call site - TASK-1.50's own event-stream loop (the items that follow `event.watch`'s one reply) should treat `output::render`'s call as the "one reply, then handed to the caller" boundary, not re-implement it.

## Criterion #2's proof: the system `script` utility, a real pty, kept to one test

**No new dependency.** Per the engineer's decision 2, this uses the `script` command already on the machine (BSD/macOS `script(1)`), never `portable-pty` - no `boundaries.toml`/`Cargo.toml` edit anywhere in this task, and the dependency question this plan raised earlier is withdrawn.

**What kind of test this is, stated plainly (per the engineer's instruction):** the real built `steg` binary, invoked under a real pty that the OS actually allocates (`script`'s own child runs on a pty slave, not a pipe or an in-process fake), against a real server listening on a real Unix socket started the same way `server_start.rs`'s own tests already start one - nothing doubled, nothing simulated. Kept to **one** such test (`cli_output_formats.rs::a_non_empty_list_is_byte_identical_through_a_pty_and_a_pipe`): `session.new --label <fixed> --json` then `session.list` (non-empty - an empty list's bytes are indistinguishable from "nothing rendered," too weak a proof, the same reason `cli_exit_statuses.rs`'s own empty-list test isn't strong enough either, below), run once piped and once through `script`, bytes compared.

**Verified directly before writing this down** (not assumed): `script -q <typescript> sh -c 'stty raw -echo; exec "$@"' sh <program> <args...>` - `-q` suppresses `script`'s own "Script started/done" banner lines; the `sh -c` wrapper puts the pty into raw, no-echo mode (`stty raw -echo`) *before* `exec`-ing the real program, so the pty's own line-discipline (which otherwise turns every `\n` the program writes into `\r\n`, and would echo input) stops rewriting the child's bytes - confirmed with `/bin/echo`, including a case with embedded tabs and one with an argument containing spaces: after that point, the captured typescript is byte-for-byte what a plain pipe captures. Also confirmed: this works with the calling process's own stdin/stdout/stderr already redirected away from any tty (as `cargo test` itself runs), since `script` allocates its own pty regardless of whether its caller has one.

**One fixed artefact, measured at test time, not hard-coded:** even with `-q` and raw mode, macOS's `script` writes a short, constant byte sequence to the pty *before* the child's own first byte (confirmed: identical four bytes - `^D` followed by two backspaces - whether the child is `/bin/echo hello`, `/bin/echo` with no output at all, or anything else tried). This is `script`'s own pty-setup noise, not the CLI's output, and it is deterministic for a given OS/`script` build but not promised to stay exactly four bytes forever. The test measures it itself each run - one calibration call, `script -q <path> sh -c 'stty raw -echo; true'`, whose captured length is the preamble to skip on every subsequent capture in the same test - rather than a magic number that would silently misbehave on a different machine or a future macOS. Every wait (the server's socket appearing, `script`'s own child exiting) is bounded by an explicit `Duration`, the same shape `server_start.rs`'s `wait_bounded`/`wait_for_socket` already use, rewritten locally in the new test file rather than imported (the brief's own warning: the existing duplicated helpers across test files have already collided across worktrees).

A fresh, file-local `unique_path(label)` rooted at `std::env::var("CARGO_TARGET_TMPDIR")` (per-target-directory, worktree-unique by construction) gives the server its throwaway socket path and the typescript its own file, in a per-test temporary directory; the server is started with an explicit `server.start` invocation, never a pane's own login shell, so this test has no account-database dependency at all.

**Scope of the calibration, named plainly:** the calibration call measures a fixed *prefix* `script` writes before the child's first byte - it was checked against a short, single-write child (`/bin/echo`, `true`), not against the multi-line, multi-write output this test's own real comparison produces (`session.list`'s several tab-separated lines). If some `script` build also interleaves or appends bytes of its own *after* the child starts writing (not observed on this machine, not assumed absent either), the calibration step would not catch that - but the test's own `assert_eq!` on the full raw byte vectors would, failing the test rather than passing a wrong comparison. Named here so that failure mode reads as anticipated, not discovered.

**This test's own `sh -c` is a child process the test spawns, not a check command.** `asdlc record-check`'s own ban on `sh -c`/a pipe/`tail` governs how *checks are recorded* in step 6 below (every one of them runs `cargo test ...`/`just check` directly, no shell wrapper) - it says nothing about what a test may itself `Command::new` as the program under test. `script`'s own `sh -c 'stty raw -echo; exec "$@"'` argument is exactly that: the thing being tested (the real binary, run under a pty), not a check being graded.

## The one existing test to change, and why; and the ones that look like they might need to but don't

**Must change:** `cli_exit_statuses.rs::without_json_a_successful_command_prints_nothing_on_stdout` (around line 246). It calls `session list` with no sessions created and asserts empty stdout, with a docstring reading "plain-text rendering is TASK-1.120's, not reused early here." After this task, that is no longer why it passes - `session.list` is `Returns::Records`, and this task's renderer prints one tab-separated line *per record*; the assertion still holds only because the list is empty (zero records -> zero lines -> empty stdout), a degenerate case, not a demonstration that plain-text rendering does nothing. **Fix:** reword the docstring to say plainly that this is the empty-list degenerate case of `Records`, point to `cli_output_formats.rs` for the real (non-empty) proof, and add one more line to the same test: create a session first, call `session list` again with no flags, and assert the plain line equals the expected tab-separated row built from `Returns::Records`'s own `fields` - so the test still earns its name once this task exists.

**Don't need to change, named so a reviewer doesn't have to re-derive it:**
- `cli_exit_statuses.rs:291`, `session show deadbeef00000000` without `--json` - the target never resolves, so execution never reaches the renderer at all. Unaffected.
- `cli_exit_statuses.rs:412`, `session close <id>` without `--json` - `Returns::Nothing`; this task's renderer prints nothing for `Nothing` exactly as the pre-this-task code already did by accident. Unaffected.
- Every other `run_steg` call in `cli_exit_statuses.rs`, `cli_api_call.rs`, and `cli_connection_arbitration.rs` passes `--json` already (grepped: every site but the three above does) - this task's `--json` path for `Identifier`/`Records` is "the reply unchanged," identical to today's code, so none of those assertions change.
- `cli_no_server.rs`'s bare-`steg`/bare-noun listing tests print the noun/verb lists `main.rs` builds directly from `description().nouns()`/`verbs_of()`, never through `output::render` - a different code path entirely, untouched.

## Decisions (the engineer's six answers, applied above)

1. **Nested results print as JSON in plain-text mode.** Applied to the table's `Nested` row - also settles, as a direct consequence, that `-q` has nothing to reduce on a `Nested` reply (no new question).
2. **Pty proof via the system `script` command, not `portable-pty`.** Applied in "Criterion #2's proof" above - no dependency, no boundaries question, one test, kept to the kind of test named explicitly.
3. **`--json` on a `Nothing`-returning command prints nothing.** Applied to the table's `Nothing` row.
4. **`-q` on a record with no `id` field prints the first declared field; nothing if the record declares no fields.** Applied via `fields[0]`, shown above to coincide with `"id"` wherever that field exists, and settling `setting`/`api`/`server.show`/`pane.wait` explicitly without inventing a fallback.
5. **`-q` on `Text` leaves it unchanged.** Applied to the table's `Text` row.
6. **Tabs and newlines in plain-text fields become spaces; `--json` keeps exact values.** Applied in the renderer's own field-value table above.

## Files, order of work, and proof

Every step's own check is recorded with `asdlc record-check <task> --in <worktree> --as <worker> -- <command>`, the command run directly, never through `sh -c`, a pipe, or `tail` (the engineer's own rule for *recording* a check - distinct from what a test itself spawns as the program under test, named above).

| Order | File | What | Recorded check |
|---|---|---|---|
| 1 | `crates/steg/src/cli/tree.rs` | `-q`/`--quiet` global flag + `quiet_flag()` reader, two tests mirroring `json_flag`'s existing pair | `cargo test -p steg --lib` (the module's own `mod tests`) |
| 2 | `crates/steg/src/cli/output.rs` (new) | `render()`, `RenderError`; unit tests for every `Returns` variant x every flag combination, all pure (hand-built `Returns`/`Value`, no process) - including the zero-fields `Record`/`Records` case from decision 4, which no described method reaches today, so this is the *only* layer that proves it | `cargo test -p steg --lib` |
| 3 | `crates/steg/src/cli/exit.rs` | one sentence added to `ConnectError::Io`'s own doc comment naming the new caller; no new function | none (comment only) |
| 4 | `crates/steg/src/cli/connect.rs` | swap the `if json {...}` block for `output::render(...)` + write/err-map | `cargo test -p steg --test cli_exit_statuses` (the existing suite, to catch a regression before the new one exists) |
| 5 | `crates/steg/src/cli/api_call.rs` | one doc-comment line (`-q` ignored, same reasoning as `--json`) | `cargo test -p steg --test cli_api_call` |
| 6 | `crates/steg/tests/cli_output_formats.rs` (new) | criteria #1-#3 through the real binary: every `Returns` kind x plain/`--json`/`-q`; the one `script`-based pty-vs-pipe byte comparison; `pane attach`'s `Nested` reply as JSON unconditionally | `cargo test -p steg --test cli_output_formats` |
| 7 | `crates/steg/tests/cli_exit_statuses.rs` | reword + extend `without_json_a_successful_command_prints_nothing_on_stdout` per above | `cargo test -p steg --test cli_exit_statuses` |
| 8 | whole-task close | the repository's own full gate, once, on the final head | `just check` |

**Proof, criterion by criterion:**
- **#1** (flags behave as doc-2 says, no catch-all): `output.rs`'s own exhaustive match, proved by the dummy-seventh-variant negative control (build fails, reverted); every cell of the table above as a passing test, including each of the six decisions as its own named test case.
- **#2** (byte-identical list, pty vs pipe): the one `script`-based test, non-empty list, bytes compared with `assert_eq!` on the raw `Vec<u8>` (not `str`, so a stray byte would actually be caught); the preamble-measurement step is itself asserted non-empty and consistent across two calibration calls in the same test run, so a flaky measurement fails the test rather than silently mis-slicing.
- **#3** (`Nested` whole with `--json`; plain text as decided): decision 1 settles this outright - plain text and `--json` are the same bytes for every `Nested` method, tested for both `pane.attach` and `event.watch`'s own one-shot reply.
- **Negative controls**, each a failing-then-passing `asdlc record-check` pair: the dummy `Returns` variant (criterion #1); `output::render`'s `Nothing` arm changed to pass the raw value through unconditionally with `--json`, confirming the reworded `cli_exit_statuses.rs` test (and a new `output.rs` unit test) actually fails without decision 3's own default; the `script`-based comparison run once with the pty side's `stty raw -echo` step removed, confirming the test as written would then fail on the `\r\n`/echo difference - proving the pty is load-bearing and the raw-mode step is doing real work, not decorative.

## Risks carried forward

None blocking - both items this plan previously flagged as needing the engineer (the `Nested` plain-text form, and the pty dependency) are now settled by the six decisions above. The one residual risk worth naming: the `script`-based test depends on the exact macOS `script(1)` build's own pty-setup behaviour (confirmed deterministic on this machine, four bytes, for any child); its own calibration step measures this at run time rather than assuming it, so a future `script` build that behaves differently fails the test loudly (a wrong comparison) rather than silently passing a wrong one.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Revised plan checked against doc-2 (sections 1, 3, 5, 6) and decision-4 by @bruk.o on 2026-11-20: ok. All six of the engineer's answers of 2026-11-20 are applied: nested results print as JSON in plain text; the pty proof uses the system script command with no new dependency, as one test named for what it is (real binary, real pty, real server); --json on a nothing-returning command prints nothing; -q prints a record's first declared field and nothing where none is declared; -q leaves text unchanged; tabs and newlines in plain-text fields become spaces. It builds only the output module and its tests.
<!-- SECTION:NOTES:END -->

## Comments

<!-- COMMENTS:BEGIN -->
author: @bruk
created: 2026-11-20T17:25:00Z

---

Plan approved by @bruk (e49be2b088c1, size-2): its worker may build it.

---
<!-- COMMENTS:END -->
