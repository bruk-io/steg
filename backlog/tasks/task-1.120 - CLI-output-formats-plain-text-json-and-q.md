---
id: TASK-1.120
title: 'CLI: output formats: plain text, --json and -q'
status: In Progress
assignee:
  - '@bruk.w8'
created_date: '2026-11-16T13:35:00Z'
updated_date: '2026-11-20T18:21:00Z'
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

Check passed: `env ZIG=/Users/brukhabtu/.local/share/mise/installs/zig/0.17.1/bin/zig cargo test -p steg --lib` exited 0 at 2026-11-20T17:29:06+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.120 on task-1.120-cli-output-formats-plain-text-json-and-q at 22e089e3ff4c with uncommitted changes:

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.09s
     Running unittests src/lib.rs (target/debug/deps/steg-a2a09eaf6a06f7de)

running 8 tests
test cli::exit::tests::every_connect_error_maps_to_4 ... ok
test cli::exit::tests::every_described_error_maps_to_its_own_exit_status ... ok
test cli::exit::tests::api_error_status_follows_the_table_not_a_parallel_match ... ok
test cli::tree::tests::autostart_given_after_no_autostart_wins ... ok
test cli::tree::tests::autostart_defaults_to_true_with_neither_flag_given ... ok
test cli::tree::tests::no_autostart_is_read_whether_given_before_or_after_the_subcommand ... ok
test cli::tree::tests::json_is_read_from_the_leaf_subcommand ... ok
test cli::tree::tests::quiet_is_read_from_the_leaf_subcommand_either_spelling ... ok

test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

Check passed: `env ZIG=/Users/brukhabtu/.local/share/mise/installs/zig/0.17.1/bin/zig cargo test -p steg --lib` exited 0 at 2026-11-20T17:32:56+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.120 on task-1.120-cli-output-formats-plain-text-json-and-q at 22e089e3ff4c with uncommitted changes, last 40 of 42 lines:

```text
     Running unittests src/lib.rs (target/debug/deps/steg-a2a09eaf6a06f7de)

running 35 tests
test cli::output::tests::nothing_prints_nothing_in_every_combination ... ok
test cli::output::tests::record_quiet_with_json_prints_fields_zero_as_json ... ok
test cli::output::tests::identifier_quiet_is_the_same_as_without_quiet_either_way ... ok
test cli::output::tests::record_quiet_prints_fields_zero_alone ... ok
test cli::output::tests::nested_plain_and_json_are_identical_bytes ... ok
test cli::output::tests::record_plain_is_one_tab_separated_line_in_fields_order ... ok
test cli::output::tests::identifier_json_prints_the_object_unchanged ... ok
test cli::output::tests::nested_quiet_makes_no_difference ... ok
test cli::output::tests::records_json_prints_the_array_unchanged ... ok
test cli::output::tests::identifier_with_no_id_field_is_a_shape_error_not_a_panic ... ok
test cli::output::tests::record_json_prints_the_object_unchanged ... ok
test cli::output::tests::a_field_declared_text_but_really_structured_renders_its_own_compact_json ... ok
test cli::output::tests::record_quiet_with_no_id_field_prints_the_first_declared_field ... ok
test cli::output::tests::identifier_plain_prints_the_bare_id_alone ... ok
test cli::exit::tests::every_connect_error_maps_to_4 ... ok
test cli::output::tests::record_with_no_declared_fields_prints_nothing_in_every_combination ... ok
test cli::output::tests::records_quiet_with_json_is_one_json_array_of_fields_zero ... ok
test cli::output::tests::records_with_a_non_array_reply_is_a_shape_error_not_a_panic ... ok
test cli::output::tests::records_quiet_prints_fields_zero_per_line ... ok
test cli::output::tests::a_null_field_renders_as_an_empty_column_never_a_dropped_one ... ok
test cli::output::tests::records_with_a_non_object_element_is_a_shape_error_not_a_panic ... ok
test cli::output::tests::record_with_a_non_object_reply_is_a_shape_error_not_a_panic ... ok
test cli::output::tests::records_plain_is_one_line_per_record_nothing_for_zero ... ok
test cli::output::tests::records_with_no_declared_fields_prints_nothing_in_every_combination ... ok
test cli::output::tests::text_json_prints_the_string_value_unchanged ... ok
test cli::output::tests::text_plain_is_the_string_unaltered ... ok
test cli::output::tests::text_quiet_is_the_same_as_without_quiet_either_way ... ok
test cli::output::tests::text_with_a_non_string_reply_is_a_shape_error_not_a_panic ... ok
test cli::exit::tests::every_described_error_maps_to_its_own_exit_status ... ok
test cli::exit::tests::api_error_status_follows_the_table_not_a_parallel_match ... ok
test cli::tree::tests::autostart_defaults_to_true_with_neither_flag_given ... ok
test cli::tree::tests::autostart_given_after_no_autostart_wins ... ok
test cli::tree::tests::json_is_read_from_the_leaf_subcommand ... ok
test cli::tree::tests::no_autostart_is_read_whether_given_before_or_after_the_subcommand ... ok
test cli::tree::tests::quiet_is_read_from_the_leaf_subcommand_either_spelling ... ok

test result: ok. 35 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

Check passed: `env ZIG=/Users/brukhabtu/.local/share/mise/installs/zig/0.17.1/bin/zig cargo test -p steg --test cli_exit_statuses` exited 0 at 2026-11-20T17:33:58+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.120 on task-1.120-cli-output-formats-plain-text-json-and-q at 22e089e3ff4c with uncommitted changes:

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.09s
     Running tests/cli_exit_statuses.rs (target/debug/deps/cli_exit_statuses-a198bfba3dceb624)

running 18 tests
test support::tests::candidate_name_does_not_collide_under_the_same_forced_pid_and_counter ... ok
test support::tests::the_old_pid_and_counter_only_scheme_collides_when_two_processes_share_a_pid ... ok
test support::tests::one_scoped_temp_dir_can_hand_out_two_different_paths ... ok
test support::tests::a_fresh_scoped_temp_dir_is_0700 ... ok
test support::tests::scoped_temp_dir_exists_after_new_and_is_gone_after_drop ... ok
test support::tests::path_panics_rather_than_hand_back_something_over_the_sun_path_budget - should panic ... ok
test support::tests::create_unique_dir_skips_a_name_that_already_exists ... ok
test support::tests::two_scoped_temp_dirs_made_back_to_back_are_different ... ok
test malformed_stdin_on_api_call_exits_2 ... ok
test no_autostart_with_nothing_running_exits_4 ... ok
test oversized_stdin_on_api_call_exits_2_without_panicking ... ok
test sigint_during_a_hung_call_ends_the_process_by_signal ... ok
test without_json_a_successful_command_prints_nothing_on_stdout ... ok
test a_target_that_matches_nothing_exits_3 ... ok
test with_json_a_failed_command_still_prints_the_message_on_stderr_not_json ... ok
test a_real_round_trip_through_an_autostarted_server_prints_json_unchanged ... ok
test a_program_still_running_refuses_close_with_status_1 ... ok
test a_value_outside_a_closed_set_exits_2_with_no_server_contacted ... ok

test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.51s
```

Check passed: `env ZIG=/Users/brukhabtu/.local/share/mise/installs/zig/0.17.1/bin/zig cargo test -p steg --test cli_api_call` exited 0 at 2026-11-20T17:34:20+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.120 on task-1.120-cli-output-formats-plain-text-json-and-q at 22e089e3ff4c with uncommitted changes:

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.09s
     Running tests/cli_api_call.rs (target/debug/deps/cli_api_call-51a926aaf823770e)

running 10 tests
test support::tests::candidate_name_does_not_collide_under_the_same_forced_pid_and_counter ... ok
test support::tests::the_old_pid_and_counter_only_scheme_collides_when_two_processes_share_a_pid ... ok
test support::tests::path_panics_rather_than_hand_back_something_over_the_sun_path_budget - should panic ... ok
test support::tests::scoped_temp_dir_exists_after_new_and_is_gone_after_drop ... ok
test support::tests::one_scoped_temp_dir_can_hand_out_two_different_paths ... ok
test support::tests::a_fresh_scoped_temp_dir_is_0700 ... ok
test support::tests::create_unique_dir_skips_a_name_that_already_exists ... ok
test support::tests::two_scoped_temp_dirs_made_back_to_back_are_different ... ok
test api_call_prints_json_with_no_json_flag_given ... ok
test api_call_sends_stdin_params_flat_to_the_named_method ... ok

test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.40s
```

Check passed: `env ZIG=/Users/brukhabtu/.local/share/mise/installs/zig/0.17.1/bin/zig cargo test -p steg --test cli_output_formats` exited 0 at 2026-11-20T17:46:36+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.120 on task-1.120-cli-output-formats-plain-text-json-and-q at 22e089e3ff4c with uncommitted changes:

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.09s
     Running tests/cli_output_formats.rs (target/debug/deps/cli_output_formats-b7f07949c8f37e3f)

running 20 tests
test support::tests::candidate_name_does_not_collide_under_the_same_forced_pid_and_counter ... ok
test support::tests::the_old_pid_and_counter_only_scheme_collides_when_two_processes_share_a_pid ... ok
test support::tests::run_bounded_returns_the_bodys_value_when_it_finishes_in_time ... ok
test support::tests::scoped_temp_dir_exists_after_new_and_is_gone_after_drop ... ok
test support::tests::a_fresh_scoped_temp_dir_is_0700 ... ok
test support::tests::one_scoped_temp_dir_can_hand_out_two_different_paths ... ok
test support::tests::path_panics_rather_than_hand_back_something_over_the_sun_path_budget - should panic ... ok
test support::tests::create_unique_dir_skips_a_name_that_already_exists ... ok
test support::tests::wait_for_socket_returns_once_something_is_listening ... ok
test support::tests::two_scoped_temp_dirs_made_back_to_back_are_different ... ok
test support::tests::run_bounded_panics_when_the_body_does_not_finish_in_time - should panic ... ok
test support::tests::wait_for_socket_panics_when_nothing_ever_answers - should panic ... ok
test quiet_with_no_id_field_prints_the_first_declared_field ... ok
test text_renders_unaltered_plain_and_as_a_json_string ... ok
test records_render_plain_json_and_quiet_through_a_real_server ... ok
test identifier_renders_the_bare_id_plain_and_is_unchanged_by_quiet ... ok
test nested_pane_attach_renders_as_json_unconditionally_through_a_real_server ... ok
test nothing_prints_nothing_even_with_json_through_a_real_server ... ok
test a_non_empty_list_is_byte_identical_through_a_pty_and_a_pipe ... ok
test record_renders_plain_json_and_quiet_through_a_real_server ... ok

test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.51s
```

Check passed: `env ZIG=/Users/brukhabtu/.local/share/mise/installs/zig/0.17.1/bin/zig cargo test -p steg --test cli_exit_statuses` exited 0 at 2026-11-20T17:47:21+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.120 on task-1.120-cli-output-formats-plain-text-json-and-q at 22e089e3ff4c with uncommitted changes:

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.09s
     Running tests/cli_exit_statuses.rs (target/debug/deps/cli_exit_statuses-a198bfba3dceb624)

running 22 tests
test support::tests::candidate_name_does_not_collide_under_the_same_forced_pid_and_counter ... ok
test support::tests::the_old_pid_and_counter_only_scheme_collides_when_two_processes_share_a_pid ... ok
test support::tests::run_bounded_returns_the_bodys_value_when_it_finishes_in_time ... ok
test support::tests::one_scoped_temp_dir_can_hand_out_two_different_paths ... ok
test support::tests::wait_for_socket_returns_once_something_is_listening ... ok
test support::tests::scoped_temp_dir_exists_after_new_and_is_gone_after_drop ... ok
test support::tests::path_panics_rather_than_hand_back_something_over_the_sun_path_budget - should panic ... ok
test support::tests::a_fresh_scoped_temp_dir_is_0700 ... ok
test support::tests::two_scoped_temp_dirs_made_back_to_back_are_different ... ok
test support::tests::create_unique_dir_skips_a_name_that_already_exists ... ok
test support::tests::wait_for_socket_panics_when_nothing_ever_answers - should panic ... ok
test support::tests::run_bounded_panics_when_the_body_does_not_finish_in_time - should panic ... ok
test malformed_stdin_on_api_call_exits_2 ... ok
test no_autostart_with_nothing_running_exits_4 ... ok
test oversized_stdin_on_api_call_exits_2_without_panicking ... ok
test sigint_during_a_hung_call_ends_the_process_by_signal ... ok
test a_real_round_trip_through_an_autostarted_server_prints_json_unchanged ... ok
test a_target_that_matches_nothing_exits_3 ... ok
test with_json_a_failed_command_still_prints_the_message_on_stderr_not_json ... ok
test without_json_a_successful_command_prints_nothing_on_stdout ... ok
test a_program_still_running_refuses_close_with_status_1 ... ok
test a_value_outside_a_closed_set_exits_2_with_no_server_contacted ... ok

test result: ok. 22 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.49s
```

Check passed: `env ZIG=/Users/brukhabtu/.local/share/mise/installs/zig/0.17.1/bin/zig just --justfile /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.120/justfile --working-directory /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.120 check` exited 0 at 2026-11-20T18:02:45+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.120 on task-1.120-cli-output-formats-plain-text-json-and-q at 22e089e3ff4c with uncommitted changes:

```text
ok      format check (0.4s)
ok      lint (0.8s)
ok      boundaries (0.1s)
ok      foundations (0.1s)
ok      foundations public surface (0.1s)
ok      design pages (0.1s)
ok      licences (0.7s)
ok      tests (-p steg) (2.9s)
ok      build (steg-sdk) (0.1s)
ok      test (steg-sdk) (0.3s)
ok      test (steg-sdk, local-socket) (0.5s)
ok      build (steg-kernel) (0.1s)
ok      test (steg-kernel) (0.3s)
ok      build (steg-server) (0.1s)
ok      test (steg-server) (95.2s)
ok      test (steg-server, race-fixture) (61.1s)
ok      test (steg-server, pty-race-fixture) (62.3s)
ok      test (steg-server, accept-backoff-fixture) (193.1s)
ok      build (steg) (0.1s)
ok      test (steg) (2.1s)
ok      test (steg, fixtures) (9.1s)
ok      build (steg-ui) (20.4s)
ok      test (steg-ui) (1.2s)
ok      build (steg-app) (2.6s)
ok      test (steg-app) (2.1s)
ok      tool tests (41.1s)
ok      architecture (0.7s)
ok      recipe names (0.1s)
ok      ui levels (0.1s)
ok      ui colours (0.1s)
```

Check FAILED: `env ZIG=/Users/brukhabtu/.local/share/mise/installs/zig/0.17.1/bin/zig just --justfile /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.120/justfile --working-directory /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.120 check` exited 101 at 2026-11-20T18:07:03+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.120 on task-1.120-cli-output-formats-plain-text-json-and-q at 65b9e3561bac, last 40 of 80 lines:

```text
test pane::attach_tests::a_program_that_exits_on_its_own_sends_ended_with_its_real_exit_code_after_its_last_content ... ok
test pane::attach_tests::mark_closing_sent_before_a_programs_own_exit_is_the_reason_ended_reports ... ok
test runtime::events::tests::an_unfiltered_subscriber_receives_nothing ... ok
test pane::ending_tests::await_exit_is_not_delayed_by_a_backlog_on_the_workers_own_queue ... ok
test runtime::input::tests::the_general_sizing_step_reaches_every_pane_through_split_close_and_swap ... ok
test pane::ending_tests::signal_hangup_reaches_a_backgrounded_grandchild_too ... ok
kill: 60806: No such process
test runtime::lifecycle::stop_tests::stop_ends_a_program_that_traps_the_polite_signal ... ok
test idle::tests::the_checker_blocks_with_no_timeout_while_ineligible_and_waits_exactly_once_while_eligible ... ok
test outbound::tests::enqueue_control_and_wait_until_written_does_not_return_until_write_all_itself_returns ... ok
test test_support::tests::a_body_that_never_returns_fails_fast_naming_what_it_waited_for - should panic ... ok
test runtime::events::tests::a_mark_then_the_shells_own_real_exit_cascades_with_no_panic ... ok
test runtime::events::tests::a_shell_with_no_marks_leaves_every_status_field_unchanged_until_one_arrives ... ok
test pane::ending_tests::signal_kill_ends_a_program_that_traps_the_polite_signal ... ok
test runtime::events::tests::a_typed_commands_status_reaches_pane_show_and_tab_show_through_runtime_handle ... ok
test runtime::events::tests::a_typed_command_that_fails_is_failed_with_its_exit_code ... ok
test runtime::events::tests::a_typed_command_that_succeeds_is_running_then_finished_and_the_tab_shows_it ... ok
test runtime::events::tests::a_command_panes_own_program_printing_marks_does_not_change_its_status ... ok
kill: 60755: No such process
test pane::tests::a_forward_failing_once_the_worker_is_gone_still_reaps_a_program_that_ends_on_its_own ... ok
test login_env::tests::capture_login_environment_falls_back_on_a_shell_that_exits_non_zero ... ok
test login_env::tests::capture_login_environment_prints_noise_before_the_marker_without_corrupting_the_dump ... ok
test login_env::tests::capture_login_environment_kills_a_backgrounded_grandchild_that_held_the_pipe_open ... FAILED

failures:

---- login_env::tests::capture_login_environment_kills_a_backgrounded_grandchild_that_held_the_pipe_open stdout ----

thread 'login_env::tests::capture_login_environment_kills_a_backgrounded_grandchild_that_held_the_pipe_open' (68068266) panicked at crates/server/src/login_env.rs:675:13:
the backgrounded grandchild's own pid file never appeared


failures:
    login_env::tests::capture_login_environment_kills_a_backgrounded_grandchild_that_held_the_pipe_open

test result: FAILED. 220 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 30.01s

error: test failed, to rerun pass `-p steg-server --lib`
full output: .quiet/test-steg-server-pty-race-fixture.log
error: recipe `check-packages` failed with exit code 101
```

Check FAILED: `env ZIG=/Users/brukhabtu/.local/share/mise/installs/zig/0.17.1/bin/zig just --justfile /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.120/justfile --working-directory /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.120 check` exited 101 at 2026-11-20T18:07:47+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.120 on task-1.120-cli-output-formats-plain-text-json-and-q at 65b9e3561bac, last 40 of 78 lines:

```text
test pane::attach_tests::mark_closing_sent_before_a_programs_own_exit_is_the_reason_ended_reports ... ok
test pane::attach_tests::a_program_that_exits_on_its_own_sends_ended_with_its_real_exit_code_after_its_last_content ... ok
test runtime::events::tests::an_unfiltered_subscriber_receives_nothing ... ok
test pane::ending_tests::await_exit_is_not_delayed_by_a_backlog_on_the_workers_own_queue ... ok
test runtime::input::tests::the_general_sizing_step_reaches_every_pane_through_split_close_and_swap ... ok
kill: 66863: No such process
test runtime::lifecycle::stop_tests::stop_ends_a_program_that_traps_the_polite_signal ... ok
test pane::ending_tests::signal_hangup_reaches_a_backgrounded_grandchild_too ... ok
test idle::tests::the_checker_blocks_with_no_timeout_while_ineligible_and_waits_exactly_once_while_eligible ... ok
test outbound::tests::enqueue_control_and_wait_until_written_does_not_return_until_write_all_itself_returns ... ok
test runtime::events::tests::a_mark_then_the_shells_own_real_exit_cascades_with_no_panic ... ok
test test_support::tests::a_body_that_never_returns_fails_fast_naming_what_it_waited_for - should panic ... ok
test runtime::events::tests::a_shell_with_no_marks_leaves_every_status_field_unchanged_until_one_arrives ... ok
test pane::ending_tests::signal_kill_ends_a_program_that_traps_the_polite_signal ... ok
test runtime::events::tests::a_typed_command_that_succeeds_is_running_then_finished_and_the_tab_shows_it ... ok
test runtime::events::tests::a_typed_command_that_fails_is_failed_with_its_exit_code ... ok
test runtime::events::tests::a_typed_commands_status_reaches_pane_show_and_tab_show_through_runtime_handle ... ok
test runtime::events::tests::a_command_panes_own_program_printing_marks_does_not_change_its_status ... ok
kill: 66799: No such process
test pane::tests::a_forward_failing_once_the_worker_is_gone_still_reaps_a_program_that_ends_on_its_own ... ok
test login_env::tests::capture_login_environment_falls_back_on_a_shell_that_exits_non_zero ... ok
test login_env::tests::capture_login_environment_prints_noise_before_the_marker_without_corrupting_the_dump ... ok
test login_env::tests::capture_login_environment_kills_a_backgrounded_grandchild_that_held_the_pipe_open ... FAILED

failures:

---- login_env::tests::capture_login_environment_kills_a_backgrounded_grandchild_that_held_the_pipe_open stdout ----

thread 'login_env::tests::capture_login_environment_kills_a_backgrounded_grandchild_that_held_the_pipe_open' (68093928) panicked at crates/server/src/login_env.rs:675:13:
the backgrounded grandchild's own pid file never appeared


failures:
    login_env::tests::capture_login_environment_kills_a_backgrounded_grandchild_that_held_the_pipe_open

test result: FAILED. 220 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 30.02s

error: test failed, to rerun pass `-p steg-server --lib`
full output: .quiet/test-steg-server.log
error: recipe `check-packages` failed with exit code 101
```

Check FAILED: `env ZIG=/Users/brukhabtu/.local/share/mise/installs/zig/0.17.1/bin/zig cargo build -p steg` exited 101 at 2026-11-20T18:20:55+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.120 on task-1.120-cli-output-formats-plain-text-json-and-q at 65b9e3561bac with uncommitted changes:

```text
   Compiling steg-sdk v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.120/crates/sdk)
   Compiling steg-kernel v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.120/crates/kernel)
   Compiling steg-server v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.120/crates/server)
   Compiling steg v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.120/crates/steg)
error[E0004]: non-exhaustive patterns: `&Returns::ScratchControl` not covered
   --> crates/steg/src/cli/output.rs:49:11
    |
 49 |     match returns {
    |           ^^^^^^^ pattern `&Returns::ScratchControl` not covered
    |
note: `Returns` defined here
   --> crates/sdk/src/description/shapes.rs:206:1
    |
206 | pub enum Returns {
    | ^^^^^^^^^^^^^^^^
...
250 |     ScratchControl,
    |     -------------- not covered
    = note: the matched value is of type `&Returns`
help: ensure that all possible cases are being handled by adding a match arm with a wildcard pattern or an explicit pattern as shown
    |
 55 ~         Returns::Nested { .. } => render_nested(value),
 56 ~         &Returns::ScratchControl => todo!(),
    |

For more information about this error, try `rustc --explain E0004`.
error: could not compile `steg` (lib) due to 1 previous error
warning: build failed, waiting for other jobs to finish...
```

Check passed: `env ZIG=/Users/brukhabtu/.local/share/mise/installs/zig/0.17.1/bin/zig cargo build -p steg` exited 0 at 2026-11-20T18:21:10+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.120 on task-1.120-cli-output-formats-plain-text-json-and-q at 65b9e3561bac:

```text
   Compiling steg-sdk v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.120/crates/sdk)
   Compiling steg-kernel v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.120/crates/kernel)
   Compiling steg-server v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.120/crates/server)
   Compiling steg v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.120/crates/steg)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.46s
```
<!-- SECTION:NOTES:END -->

## Comments

<!-- COMMENTS:BEGIN -->
author: @bruk
created: 2026-11-20T17:25:00Z

---

Plan approved by @bruk (e49be2b088c1, size-2): its worker may build it.

---
<!-- COMMENTS:END -->
