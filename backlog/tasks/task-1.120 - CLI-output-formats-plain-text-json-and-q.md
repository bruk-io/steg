---
id: TASK-1.120
title: 'CLI: output formats: plain text, --json and -q'
status: In Progress
assignee:
  - '@bruk.w8'
created_date: '2026-11-16T13:35:00Z'
updated_date: '2026-11-18T14:01:00Z'
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

One new module (`crates/steg/src/cli/output.rs`): a pure function, matched exhaustively on `steg_sdk::Returns` (no catch-all), from a method's declared `Returns` plus the raw `serde_json::Value` reply to the bytes doc-2 section 6 wants - no server, no process, unit-tested directly. `connect.rs::run` calls it in place of today's `if json { println!("{value}") }`. One new global flag (`-q`/`--quiet`, `tree.rs`, same shape as `--json`/`--autostart`). One new test file (`tests/cli_output_formats.rs`) proving criteria 1-3 through the real binary. One existing test's docstring/scope corrected (`cli_exit_statuses.rs`). One boundaries question (a pty crate as a steg dev-dependency, for criterion #2's byte-identical proof). No split: this is one module, one test file, one flag, one small edit to an existing one - smaller than TASK-1.49's own single-task scope.

**Design found already decided, as asked:** TASK-1.49's plan body (hash `fdbb9acfc676`, "For the engineer", item 5, repeated at "Part 3" in the appendix) carries the **worker's own recommendation**: "`--json` carries it whole, plain text prints only the record's scalar fields". The orchestrator (@bruk.o) checked that plan as "sound" (note at 2026-11-16, under the plan). But @bruk's own recorded approval (comment #1, "Plan approved by @bruk (37c4dd0b7f74, size-4): its worker may build it") is of a **different, later plan hash** - the one "rewritten for the command tree alone" (the note directly under it: "criteria 2, 3, 5, 7 and the rest of 4 are the two split-off tasks'"). I read every comment on TASK-1.49 (14 of them) and found no comment where @bruk decided this. **So: there is no recorded engineer decision for criterion #3 - only a worker's recommendation that the orchestrator called sound and that was never put to @bruk.** That recommendation also cannot be applied literally: `Returns::Nested { of, summary }` (`shapes.rs`) carries no `fields` at all, so "the record's scalar fields" has nothing to read from the description for a `Nested` reply - it could only mean walking the raw reply JSON's own top-level keys at run time, a second rendering mechanism beside the Record/Records one. Section "3. Open questions, each with a recommended default" below, question 1, is this - framed as a question for the engineer, not asserted as settled.

## The renderer: one pure function

```rust
// crates/steg/src/cli/output.rs
pub fn render(returns: &Returns, value: &Value, json: bool, quiet: bool) -> Result<Vec<u8>, RenderError>
```

No I/O, no clap, no sdk connection types - `Returns` and `serde_json::Value` in, bytes out. `connect.rs::run` writes the bytes to stdout (or nothing, for an empty `Vec`) and keeps its own `eprintln!`/exit-status job for the `Err` case. Matched exhaustively on `Returns`'s six variants, `#[allow(clippy::wildcard_enum_match_arm)]` nowhere - the workspace already denies a wildcard arm, so a seventh `Returns` variant fails the build with no action needed from this task to make that true (negative control: add a dummy seventh variant to a scratch copy of `shapes.rs`, confirm `cargo build -p steg` fails on `output.rs`'s own match, revert).

A field's own value is rendered by matching `serde_json::Value`'s variants directly, never `ValueKind` - this is the one design choice that removes a whole class of special-casing. `tab_fields`'s `layout`/`shown_status` and `pane_fields`'s `status`/`size` are declared `ValueKind::Text` but their real JSON value is an object, an array, or null (their own doc comments say so: "both stay `ValueKind::Text` placeholders"). Matching the real `Value` instead of the declared `ValueKind` renders every one of these correctly with no per-field-name special case:

| `Value` at that field | Printed in that column |
|---|---|
| `String(s)` | `s`, with any `\t`/`\n`/`\r` replaced by a single space (question 5) |
| `Number(n)` | its decimal text |
| `Bool(b)` | `true`/`false` |
| `Null`, or the key absent entirely | empty (the column stays; count never changes - see "missing field" below) |
| `Array`/`Object` | the value's own compact JSON text (`serde_json::to_string`, never pretty), same control-character replacement applied to the result |

## The table: every `Returns` kind, its three forms, tied to doc-2

| `Returns` | Plain text | `--json` | `-q` | doc-2 |
|---|---|---|---|---|
| `Nothing` | Nothing | **Nothing** (see question 2 - not today's raw passthrough) | Nothing | §6 "An action... is silent"; §5 `-q` row, third clause |
| `Identifier { of }` | The `"id"` field's string, alone on a line | The reply object unchanged (`{"id": "..."}`) | Same as plain - "unchanged" (§5) | §6 "prints its full identifier alone"; §5 `-q` row, second clause |
| `Record { fields }` | One line, each `fields`-order column tab-separated (one record, so no trailing newline logic beyond the one line) | The reply object unchanged | The `"id"`-named field's value alone, if `fields` has one; else see question 3 | §6 third bullet (tab-separated, fixed order, full identifiers); §5 `-q` row, first clause |
| `Records { fields }` | One tab-separated line per array element, `fields` order, nothing printed for zero elements | The reply array unchanged | One `"id"`-named field's value per line; `--json` together: one JSON array of them (§5's own "decided... orchestrated-filled" note) | same, plural |
| `Text` | The string, unchanged (`pane capture`'s screen text, `api.call`'s own reply before `api_call.rs`'s separate exception) | The reply value (a JSON string) unchanged | No defined reduction - left unchanged (question 4) | §6 `Text` is named as the one exception already; nothing about `-q` on it |
| `Nested { of, summary }` | **Carved out, not scalar fields** - see questions 1 and 1a | The reply unchanged | No defined reduction - left unchanged, same reasoning as `Text` | §3's `pane attach` row: "as JSON" unconditionally - see below |

A reply whose actual shape doesn't match its method's declared `Returns` (for instance `Records` declared but the server answered an object) is `Err(RenderError::Shape)`, never a panic - `connect.rs` maps it through `connect_error_status(&ConnectError::Io(...))`, reusing the bucket `block_until_reply`'s own `Incoming::Malformed` arm already uses for "the server said something that isn't a valid reply," exit 4. This should never happen in practice (one `Description` compiled into both sides), so it's proved as a defensive case with a hand-built mismatched `Value` in `output.rs`'s own unit tests, not as a reachable scenario through a real server.

### Missing field, not missing column

`pane_fields.exit_code` is absent from a running pane's own JSON reply (TASK-1.89's own shape: the field only exists once the program has ended). Byte-identical plain text needs a fixed column count regardless, so a missing key renders as an empty column between its two tabs, never a dropped column - `pane list` on a tab with one running and one ended pane produces two lines with the same number of tab-separated fields.

## Flags: what this task reads, and what it leaves alone

`tree.rs` today has `--autostart`/`--no-autostart`/`--json`, each `.global(true)`. This task adds `-q`/`--quiet` the same way (`QUIET_ID`, `ArgAction::SetTrue`, `.short('q')`, a `quiet_flag(matches)` reader beside `json_flag`) - doc-2 §5 restricts short flags to `-h`/`-q`/`-v`, so `-q` is the only short form this task introduces.

Criterion #1's own text is "**the flags every command has, `--json`, and `-q`** ... each behave as doc-2 says" - read as naming exactly those two (the comma-list), not every row of §5's seven-flag table. The other four:

- **`-v`/`--verbose`** ("diagnostics on stderr"): nothing in this task's own design has diagnostics to gate - `eprintln!`'s already in `connect.rs`/`api_call.rs` are unconditional error messages, not the extra diagnostics `-v` is for. Not added here; whichever task first writes a diagnostic belongs with it.
- **`--no-color`**: no colour exists anywhere in the CLI yet (nothing in `tree.rs`, `connect.rs`, or this task's own renderer touches an ANSI code). Out of scope by construction - "where colour belongs" is whichever task first colours something, reading `NO_COLOR`/`--no-color`/`is_terminal()` at that point, not this one inventing a flag with nothing to turn off.
- **`--no-input`** ("already the default"): true by construction already (nothing in this crate reads from the terminal); no flag needed to make a true statement true.
- **`--host`**: doc-2's own "Decided... Saved hosts are left for M3" - out of scope by the document's own text.

Leaving these four unregistered with clap means giving one today (`steg session list --no-color`) is a clap usage error (exit 2) until whichever task adds it. That is the existing state before this task too (checked: `tree.rs` registers none of the seven today beside the three already named) - this task changes nothing about that, only adds `-q`.

## `api call`'s own two exceptions, now three lines in one comment

`api_call.rs` already documents why its own output is never gated on `--json` ("never the documented way... its own output is JSON by its nature"). `-q` gets the same treatment, for the same structural reason the brief already names: `api.call`'s target method may be one "a plugin adds before the CLI knows it" (`api.rs`'s own doc comment), so there may be no `Method`/`Returns` at all to find an `"id"` field in. Recommendation: `-q` is read and silently ignored on `api call`, exactly like `--json` already is - one added line to `api_call.rs`'s module doc, no behavioural code change (its `println!("{value}")` already does the one thing it does).

## Where rendering lives, and the three tasks that touch this crate's `cli/` module

- `crates/steg/src/cli/output.rs` - new, this task's own, no server/clap/sdk-connection types in its signature.
- `crates/steg/src/cli/tree.rs` - one new global flag + reader, same shape as the two already there.
- `crates/steg/src/cli/connect.rs` - the `if json {...}` block replaced by a call to `output::render` and a write of its bytes (or none); the error arm maps through `exit.rs`.
- `crates/steg/src/cli/api_call.rs` - one doc-comment line, no behaviour change.
- `crates/steg/src/cli/exit.rs` - no new function needed (reusing `connect_error_status`), but its own doc comment for `ConnectError::Io`'s case ("grouping every 'could not finish talking to a server' case under one status") gets one added sentence naming the new caller.

TASK-1.51 (acting on the terminal the CLI runs in - context variables, no-target resolution) and TASK-1.50 (send/capture/wait/watch) are both still `mod.rs`-level siblings; neither needs a file this task touches, and this task adds no file either of them would need to touch - `output.rs`'s signature takes a `Returns`/`Value` pair it already has in hand, nothing about a pane, a target, or a stream. The one shared surface is `connect.rs::run`, already a single call site both this task and TASK-1.50 (the event-stream loop, once `follows: Events`/`Attachment` items start arriving after the one reply this task renders) will touch - TASK-1.50's own plan should treat `output::render`'s call as the "one reply, then handed to the caller" boundary, not something it re-implements.

## Criterion #2's proof: a real pty, byte-identical against a pipe

Two `Command::output()` calls are "both already not a terminal" (TASK-1.49's plan, carried forward) - no proof at all that colour/formatting doesn't silently differ. This task's test drives the real `steg` binary once through a real pty and once through a plain pipe, same args, same server, and diffs the bytes.

**Boundary question, raised rather than assumed:** `crates/steg`'s `boundaries.toml` entry allows exactly `["steg-sdk", "steg-server", "serde_json", "clap", "clap_complete"]` today - no pty crate. `steg-server` already depends on `portable-pty = "0.9"` (`[server]`'s own allow-list, `boundaries.toml:30`). Adding it to `crates/steg`'s `[dev-dependencies]` (a new section - none exists there today) still trips `check-boundaries`: confirmed by reading `tools/src/steg_tools/affected.py:110-118`, which builds `Crate.dependencies` from `cargo metadata`'s own `package["dependencies"]` with no kind filter, so a dev-only dependency is checked the same as an ordinary one. **Recommendation:** add `portable-pty = "0.9"` to `crates/steg`'s new `[dev-dependencies]` section and `"portable-pty"` to `boundaries.toml`'s `[steg].allow`, with a reason citing this task and TASK-1.49's own precedent for adding a dependency there (clap/clap_complete) - no new entry in `Cargo.lock` either way, since `portable-pty` is already resolved through `steg-server`. This is the one dependency change in the whole task and it needs the engineer's sign-off before it's built, per this task's own brief.

**The test itself** (`tests/cli_output_formats.rs`): a fresh, file-local `unique_path(label)` rooted at `std::env::var("CARGO_TARGET_TMPDIR")` (per-target-directory, so worktree-unique by construction) - not the four already-duplicated `unique_socket_path`/`steg_binary`/`run_steg` helpers in `cli_exit_statuses.rs`/`cli_api_call.rs`/`cli_connection_arbitration.rs`, which the brief says have already collided across worktrees; this task writes its own rather than touching or further duplicating theirs. An autostarted server (explicit `server.start` program, an explicit socket path in a per-test temp directory - never a pane's own shell, so no login-shell/account-database dependency at all for this particular test, since it only exercises `session.new`/`session.list`). `portable-pty`'s `native_pty_system().openpty(...)` opens the master/slave pair; the `steg` binary runs as the slave's child with stdout(slave-clone) and stdin/stderr elsewhere; a plain `std::process::Command` with piped stdout runs the identical args against the identical server for the pipe half. Every wait (the server socket appearing, the child exiting, the pty read reaching EOF) is bounded with an explicit `Duration`, following `wait_for_socket`/`wait_bounded`'s own pattern in `server_start.rs` - rewritten locally, not imported, for the same worktree-collision reason as the path helper. The comparison method is `session.new --label <fixed> --json` then `session.list` (non-empty, since an empty list's plain text is indistinguishable from "nothing rendered yet" - too weak a proof, named as the reason `cli_exit_statuses.rs`'s existing empty-list test is not strong enough either, below).

## The one existing test to change, and why; and the ones that look like they might need to but don't

**Must change:** `cli_exit_statuses.rs::without_json_a_successful_command_prints_nothing_on_stdout` (around line 246). It calls `session list` with no sessions created and asserts empty stdout, with a docstring reading "plain-text rendering is TASK-1.120's, not reused early here." After this task, that is no longer why it passes - `session.list` is `Returns::Records`, and this task's renderer prints one tab-separated line *per record*; the assertion still holds only because the list is empty (zero records -> zero lines -> empty stdout), a degenerate case, not a demonstration that plain-text rendering does nothing. Left unchanged, a reviewer reads this as still proving "no renderer yet," which is false the moment this task lands. **Fix:** reword the docstring to say plainly that this is the empty-list degenerate case of `Records`, point to `cli_output_formats.rs` for the real (non-empty) proof, and add one more line to the same test: create a session first (`--json`, read its id back, as the test two functions above it already does), call `session list` again with no flags, and assert the plain line equals the expected tab-separated row built from the same fields `Returns::Records`'s own `fields` declares - so the test still earns its name once this task exists, rather than quietly becoming vacuous.

**Don't need to change, named so a reviewer doesn't have to re-derive it:**
- `cli_exit_statuses.rs:291`, `session show deadbeef00000000` without `--json` - the target never resolves, so execution never reaches the renderer at all; stdout stays empty for an unrelated reason (the error path, unchanged by this task).
- `cli_exit_statuses.rs:412`, `session close <id>` without `--json` - `Returns::Nothing`; this task's renderer prints nothing for `Nothing` exactly as the placeholder/pre-this-task code already did by accident. Unaffected.
- Every other `run_steg` call in `cli_exit_statuses.rs`, `cli_api_call.rs`, and `cli_connection_arbitration.rs` passes `--json` already (grepped: every site but the three above does) - this task's `--json` path for `Identifier`/`Records` is "the reply unchanged," identical to today's code, so none of those assertions change.
- `cli_no_server.rs`'s bare-`steg`/bare-noun listing tests print the noun/verb lists `main.rs` builds directly from `description().nouns()`/`verbs_of()`, never through `output::render` - a different code path entirely, untouched.

## 3. Open questions, each with a recommended default

doc-2 is silent on all five; none is invented past without saying so.

1. **A `Nested` reply's plain-text form - no recorded engineer decision exists** (see "Summary" above for the full trail). `Returns::Nested` has no `fields` to pull "scalar fields" from, so the worker's own recommendation on TASK-1.49 can't be applied literally to this variant. Two of the three described `Nested` methods are **both stream-starting methods**: `pane.attach` and `event.watch`. doc-2 §3's own table already settles `pane.attach` explicitly and unconditionally: "the reply carries the stream's identifier and a snapshot, **as JSON**" - not gated on `--json`, the same shape `api.call`'s own exception already has. **Recommendation:** treat every `Nested` reply the same way, as a third stated exception beside `api call`: prints the reply as JSON unconditionally, regardless of `--json`/`-q` - mechanical (one arm in the match, no per-method special case), consistent with the one `Nested` method doc-2 does pin down, and avoids inventing an undefined "walk the raw reply's own top-level keys for scalars" mechanism for `event.watch`'s `TreeSnapshot` (whose own top-level shape is arrays of sessions/tabs/panes, not scalars at all - "scalar fields only" would print nothing useful for it regardless). **Flagged for the engineer because it overrides, not merely fills in, the recommendation TASK-1.49's plan carried forward - worth a direct decision, not a quiet default.**
   - 1a. **Transitional note, not a defect this task owns:** `event.watch` is a `follows: Events` method; before TASK-1.50 lands, `steg event watch` will connect, render this one `Nested` reply (as JSON, per the recommendation above), and exit - it will not yet keep reading the event stream. That is TASK-1.50's own job to add (a loop that keeps reading after this task's one `render` call returns), not something this task needs to simulate or test for.
2. **`Nothing` with `--json`: nothing, or the server's raw reply value (today's accidental behaviour)?** Before this task, `connect.rs::run` does `if json { println!("{value}") }` with no knowledge of `Returns` at all - so `pane close --json` today would print whatever JSON the server answers an action with (`null`, going by dispatch's own `Outcome::into_value` for a no-result outcome), untested by any existing test (grepped: no test calls an action method with `--json`). **Recommendation:** `Nothing` prints nothing in every combination of `--json`/`-q`, reading doc-2 §6's "an action... is silent" as unconditional rather than carved out by `--json`. This is a one-line behaviour change from today's code, but to an untested, accidental case, not a locked-in contract - flagged so it isn't read as silently redefining something proven.
3. **A `Record`/`Records` whose `fields` has no field named `id`: a no-op, or a different fallback?** `setting_fields` uses `"name"`; `api_method_fields` uses `"method"`; `server_fields` (singular, no list) and `pane.wait`'s own `Record { fields: [exit_code] }` have **no identifying field at all**. `Field` carries no `Role`-style marker the way `Param` does, so there is nothing in the description to read this off mechanically beyond string-matching the literal name `"id"`. **Recommendation:** match only the literal field name `"id"`; where absent (`setting list`/`show`, `api list`/`show`, `server show`, `pane wait`), `-q` is a no-op - the plain-text row prints unchanged. Simple, auditable (one string), and honestly incomplete rather than silently inventing that `"name"`/`"method"` are identifiers-in-fact; a follow-up could add a `Field`-level marker if this is wrong.
4. **`-q` on `Text`** (`pane capture -q`): doc-2's own `-q` row names three cases (records, one new identifier, nothing) and leaves `Text` out entirely. **Recommendation:** unchanged - `-q` has no defined reduction for free text, so it is a no-op here too, by the same reasoning as question 3's fallback.
5. **A field value containing a literal tab or newline** (`pane_fields.title` comes from the program; nothing bounds it). **Recommendation:** replace `\t`/`\n`/`\r` with a single space when placing a value in a plain-text column, so "one record per line" and "tab-separated" hold unconditionally rather than merely in the common case; `--json` is unaffected (JSON already escapes these natively). A label containing a tab would already be a pre-existing, separate risk (nothing stops one being set) - named, not newly introduced by this task.

## Files, order of work, and proof

| Order | File | What |
|---|---|---|
| 1 | `crates/steg/src/cli/tree.rs` | `-q`/`--quiet` global flag + `quiet_flag()` reader, two tests mirroring `json_flag`'s existing pair |
| 2 | `crates/steg/src/cli/output.rs` (new) | `render()`, `RenderError`; unit tests for every `Returns` variant x every flag combination x the five open-question cases, all pure (hand-built `Returns`/`Value`, no process) |
| 3 | `crates/steg/src/cli/exit.rs` | one sentence added to `ConnectError::Io`'s own doc comment naming the new caller; no new function |
| 4 | `crates/steg/src/cli/connect.rs` | swap the `if json {...}` block for `output::render(...)` + write/err-map |
| 5 | `crates/steg/src/cli/api_call.rs` | one doc-comment line (`-q` ignored, same reasoning as `--json`) |
| 6 | `boundaries.toml`, `crates/steg/Cargo.toml` | `portable-pty` dev-dependency + allow-list entry - **pending the engineer's sign-off on the question above** |
| 7 | `crates/steg/tests/cli_output_formats.rs` (new) | criteria #1-#3 through the real binary: every `Returns` kind x plain/`--json`/`-q`; the pty-vs-pipe byte comparison; `pane attach`'s `Nested` reply as JSON unconditionally |
| 8 | `crates/steg/tests/cli_exit_statuses.rs` | reword + extend `without_json_a_successful_command_prints_nothing_on_stdout` per above |

**Proof, criterion by criterion:**
- **#1** (flags behave as doc-2 says, no catch-all): `output.rs`'s own exhaustive match, proved by the dummy-seventh-variant negative control (build fails, reverted); every cell of the table above as a passing test; the three open-question defaults (3, 4, 5) each as their own named test case, not left implicit.
- **#2** (byte-identical list, pty vs pipe): the new pty test, non-empty list, bytes compared with `assert_eq!` on the raw `Vec<u8>` (not `str`, so a stray CR from the pty side would actually be caught rather than trimmed away by a string comparison).
- **#3** (`Nested` whole with `--json`; plain text as decided): since there is no recorded decision (question 1), this criterion's own text is unsatisfiable as literally written until the engineer answers question 1 - built against the recommended default (JSON unconditionally, every `Nested` method) with that default's own test, and flagged in the final notes as resting on an un-ratified default rather than a settled decision, so review doesn't mistake it for one.
- **Negative controls**, each a failing-then-passing `asdlc record-check` pair: the dummy `Returns` variant (criterion #1); `output::render`'s `Nothing` arm changed to pass the raw value through unconditionally, confirming the reworded `cli_exit_statuses.rs` test (and a new `output.rs` unit test) actually fails without the question-2 default; the pty/pipe comparison run once with the pty side swapped for a second `Command::output()` call (both "not a terminal"), confirming the test as originally feared would pass vacuously that way and only the real pty catches a real difference - proving the pty is load-bearing, not decorative.

## Risks carried forward, not resolved here

- Question 1 is the one risk big enough to block: if the engineer's answer differs from the recommended default, the `Nested` arm (and its one test) is a small, localised rewrite, not a redesign - nothing else in `output.rs`'s structure depends on which way it goes.
- The `portable-pty` dev-dependency needs sign-off before step 6/7 can build; if refused, criterion #2 as literally written ("a real pty") has no alternative proof this task can substitute without the same sign-off question recurring under a different crate name.
<!-- SECTION:PLAN:END -->
