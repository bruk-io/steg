---
id: TASK-1.51
title: 'CLI: act on the terminal it runs in'
status: In Progress
assignee:
  - '@bruk.w1'
created_date: '2026-11-15T01:33:00Z'
updated_date: '2026-11-21T20:27:00Z'
labels:
  - size-2
milestone: m-3
dependencies:
  - TASK-1.40
  - TASK-1.49
  - TASK-1.119
  - TASK-1.146
  - TASK-1.147
parent_task_id: TASK-1
type: task
project: bruk-io-steg
asdlc_accepted_from: DRAFT-21
asdlc_branch: task-1.51-cli-act-on-the-terminal-it-runs-in
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
doc-2 R33 and section 4.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Inside a steg terminal a command with no target acts on that pane, tab or session
- [ ] #2 Outside one, a command that needs a target and is given none fails with status 2 and says so
- [ ] #3 The `steg` a program finds in a pane is the one that belongs to that pane's server
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
## Split, and what this task now depends on

The orchestrator split this plan's earlier steps 3-4 into their own task,
**TASK-1.146**, "Server: a pane's environment carries its real session, tab,
pane and server" - `Runtime::bind_server_address`/`server_address`,
`listen.rs`'s one-line call, and `runtime/start.rs`'s `launch_spec` real-ids
fix, with its own adapter test.

Separately, the engineer decided there is **one** variable, not two: doc-2
section 4 defines `STEG_SERVER`; `STEG_SOCKET_PATH` is code-only and grew up
in `crates/sdk/src/local_socket.rs`/`paths.rs`, `crates/server/src/config.rs`/
`listen.rs`, and about fourteen test/fixture files. **TASK-1.147** renames
`STEG_SOCKET_PATH` to `STEG_SERVER` everywhere, with a deterministic tool,
changing no behaviour. This removes the precedence question an earlier draft
of this plan raised and designed a rule for: once the rename lands there is
one name, so there is nothing to rank against it, and `resolve_path`'s own
logic (below) needs no change from this task at all.

This task now **depends on both TASK-1.146 and TASK-1.147, and builds on
each once merged**: the chain test below (AC #1's pane clause and AC #3) only
passes once TASK-1.146 gives a pane real `STEG_*` values; the claim "a pane's
`steg` reaches its own server through `STEG_SERVER` alone" only reads as
written once TASK-1.147 lands (before it, the identical mechanism exists
under the name `STEG_SOCKET_PATH`, which this plan's own empirical capture
below used, since it was run before either task existed). Everything else
below is this task's own: the wire additions, `Context::from_hello`,
`connection.rs`'s wiring, the CLI's env-reading, and the end-to-end tests.

## Scope correction, found by running the real binary before planning

Built `./target/debug/steg` and ran it against real on-demand servers (throwaway
sockets under `$TMPDIR`, never `/tmp` directly - `arbiter::attempt`'s own
`directory_is_safe` refuses `/tmp` on this machine since it is not owned safely).
Two of the three ACs are already true in the shipped binary; only AC #1 needs new
production behaviour in this task (TASK-1.146 supplies the other half).

- **AC #2 already holds, unchanged.** `steg pane show` with no target, no `STEG_*`
  context, against a fresh server: exits 2 with `steg: no target given, and this
  connection has no pane context to fall back on`. `Context::none()` ->
  `missing_context` -> `ErrorCode::BadRequest` -> the description's own
  `exit_status` already gives 2 and already says so. Nothing to build; only a
  real-binary test is missing.
- **AC #3's own wording ("the steg a program finds") already holds.** Captured a
  real pane's screen (`session new --` followed by a one-line shell program,
  `echo BIN=$(command -v steg); ...`): `BIN=` printed the exact path of the
  running server's own binary. `launch_spec`'s `bin_dir`
  (`std::env::current_exe().parent()`) was already right - TASK-1.87 built it
  for `TERM`'s sake and it happens to be right for this too, and TASK-1.146
  does not change it. Nothing to build for the PATH/`STEG_BIN_DIR` half; only
  a real-binary test is missing, with a negative control that breaks it on
  purpose (see the proof table).
- **What is genuinely broken, confirmed from the same capture (TASK-1.146's own
  job):** the same pane printed `SRV=[]`, `SESS=00000000`, `TAB=00000000`,
  `PANE=00000000` - not the real session `00000001`/tab `00000002`/pane
  `00000003` the server had actually just minted. `runtime/start.rs::
  launch_spec` passes `pane_environment` a hardcoded empty server string and
  `SessionId::from_raw(0)`/`TabId::from_raw(0)`/`PaneId::from_raw(0)` - literal
  placeholders, not "whatever the real ids are". This is why AC #1 cannot
  work today even in principle: a CLI run inside a real pane would read
  `STEG_SESSION=0`/`STEG_TAB=0`/`STEG_PANE=0` off its own environment, which
  name nothing real.
- **The other half of AC #1's mechanism is this task's own job:**
  `crates/server/src/connection.rs::run_post_handshake` hard-codes
  `context: Context::none()` for every connection, always - nothing on the
  wire today carries a connecting client's own `STEG_SESSION`/`TAB`/`PANE`
  back to the server at all (the hello carries only `kind`/`label`, confirmed
  by reading `crates/sdk/src/wire/handshake.rs` and its own "nothing on the
  wire supplies a session, tab, or pane today" comment in `crates/server/src/
  dispatch/mod.rs`, which already names this as "the server's own remaining
  job"). This task finishes that job.

## A pre-existing finding from the reviewer, settled

"A bare pane id with no session/tab context does not resolve today" - confirmed,
but it is not a bug and not in this task's scope to change: `resolve_pane_str`
(`dispatch/lookup.rs`) treats a *single-segment* target string as needing
`default_tab`/`default_session` regardless of whether that one segment happens to
be a full, globally unique id, because doc-2 section 4's grammar is positional
(`[session/][tab/]pane`), not "try the tree-wide id first". AC #1 is about a
command given **no target at all** (the `target_pane`/`target_tab`/`target_session`
`None` branch in `dispatch/lookup.rs`, which never touches `resolve_pane_str`'s
segment counting), so this finding does not block AC #1. It will incidentally
start working too, once `Context` carries real defaults instead of `none()`,
since a bare id then also only has to supply the one segment it's missing - but
that is a side effect, not something this plan adds code for.

## Files touched

### `crates/sdk` (wire format - additive, no existing call site breaks)
- `src/wire/handshake.rs`: `ClientHello` gains `session`/`tab`/`pane`:
  `Option<String>`, `#[serde(default)]` each (forward/backward compatible, the
  same way `kind`/`label` were added - this file's own module doc already says
  so). `ClientHello::new`'s *signature* is untouched (still 4 params); its body
  sets the 3 new fields to `None`. New `ClientHello::with_context(client,
  protocol, kind, label, session, tab, pane)` constructor, used only by the one
  real caller that needs it (below). `Handshake::Accept` gains the same 3 fields;
  `check_hello` moves them off the deserialized `ClientHello` (no clone needed,
  same as `client`/`label` already are). Two of this file's own tests
  (`a_matching_version_is_accepted`, `kind_and_label_round_trip_through_the_hello`)
  assert a full `Handshake::Accept{...}` literal and need `session: None, tab:
  None, pane: None` added to the expected value - the only edits this file needs
  to its *existing* tests. New test: session/tab/pane round-trip through
  `check_hello` when the hello carries them.
- `src/wire/connection.rs`: `Connection::send_hello` is untouched (still calls
  `ClientHello::new` with 4 args, exactly as today) - every one of its ~10
  existing call sites across `app`/`server`/`steg` needs no edit. New
  `Connection::send_hello_with_context(client, kind, label, session, tab, pane)`,
  calling `ClientHello::with_context`, used only by `local_socket.rs` below.
- `src/local_socket.rs`: `connect_handshaken`/`one_handshake_attempt` gain 3 new
  trailing `Option<&str>` params (`session`, `tab`, `pane`), threaded to
  `Connection::send_hello_with_context` in place of `send_hello`. Only 4 call
  sites total: 3 in this file's own test module (pass `None, None, None` -
  unrelated to context, retry-logic tests) and the one real caller,
  `crates/steg/src/cli/connect.rs`. **Nothing else in this file changes.**
  `resolve_path`'s own explicit-override env-var read is `STEG_SOCKET_PATH`
  today and `STEG_SERVER` once TASK-1.147 merges - the identical single read,
  renamed, with no second tier and no precedence to design: an earlier draft
  of this plan proposed adding a `STEG_SERVER` fallback tier here itself, which
  the engineer's "one variable" decision makes unnecessary. `connect_and_handshake`
  (CLI, below) already passes `ConnectOptions.socket_path: None`, so once
  the rename lands, the pane's own `STEG_SERVER` reaches `resolve_path`'s
  existing single read with zero new code in either crate - checked directly
  against this file's own current body, not assumed.

### `crates/server`
- `src/dispatch/mod.rs`: new `Context::from_hello(session: Option<&str>, tab:
  Option<&str>, pane: Option<&str>) -> Self`, parsing each with `.and_then(|s|
  s.parse().ok())` (`SessionId`/`TabId`/`PaneId` already implement `FromStr`,
  confirmed in `crates/sdk/src/ids.rs`) - an unparsable or absent value degrades
  to `None` for that field alone, never refuses the handshake (same "absence is
  represented, not guessed" rule `Context::none()`'s own doc already states).
  Corrects this module's own now-stale doc comment ("nothing on the wire supplies
  a session, tab, or pane today... the server's own remaining job") and
  `Context`'s own doc comment, both of which this task finishes. New unit tests:
  all three present and well-formed; all three absent (equals `none()`); one
  present and malformed (that field alone is `None`, the others still resolve).
- `src/connection.rs` (**shares this file with TASK-1.140, which touches pane
  wait/close - `runtime/events.rs`, `runtime/close.rs`, `connection.rs`'s own
  attachment/data-frame handling, and is building now, likely to merge before
  this task does. This plan's only edits here are `handle`'s hello-accept
  branch and `run_post_handshake`'s signature/`ConnectionState` literal - not
  `handle_request`, not `pane_streams`, not anything TASK-1.140 owns - but both
  changes land in the same file, so expect a real merge conflict, not a clean
  one. Merge `origin/main` into this task's branch before starting any edit
  here, and again before finishing, exactly as the standard finish steps
  already require - calling it out here because this is the one file in this
  plan where that step is load-bearing, not routine**): in `handle`, the
  `Handshake::Accept { kind, label, .. }` match widens to `Handshake::Accept {
  kind, label, session, tab, pane, .. }` and builds `let context =
  Context::from_hello(session.as_deref(), tab.as_deref(), pane.as_deref());`
  right there (same place `kind`/`label` are already read). `run_post_handshake`
  gains a `context: Context` parameter (its one caller, `handle`, passes the
  new value); its own `ConnectionState` literal uses that parameter instead of
  the hard-coded `Context::none()`.

### `crates/steg`
- `src/cli/connect.rs`: `connect_and_handshake` reads `STEG_SESSION`/`STEG_TAB`/
  `STEG_PANE` from its own process environment (`std::env::var(...).ok()`) and
  passes them through to `connect_handshaken`. **No code is added for the
  server address itself** - `connect_and_handshake` already passes
  `ConnectOptions.socket_path: None` today, which already lets
  `steg_sdk::local_socket::resolve_path` read the explicit-override
  environment variable on its own (confirmed above); after TASK-1.147's
  rename that variable is `STEG_SERVER`, with no change needed on the CLI's
  side at all. This keeps the CLI a plain consumer of the sdk's public
  interface for connection resolution, never a second place that decides
  which socket to dial (`.claude/rules/architecture.md`). Outside a pane,
  `STEG_SESSION`/`TAB`/`PANE` are simply absent and behaviour is unchanged
  (confirmed: every existing `crates/steg/tests/*.rs` fixture already runs
  with `.env_clear()` and never sets them).
- New test file `crates/steg/tests/cli_terminal_context.rs` (End to end, real
  binary, real server, real socket - `support::ScopedTempDir`, the same
  `run_steg`-with-`env_clear()` pattern `cli_exit_statuses.rs` already uses,
  and the same explicit discipline for every subprocess this file spawns:
  `.env_clear()` first, then exactly the variables each test names, never this
  test process's own ambient environment - SAFETY: this machine holds real
  credentials). By the time this task builds, TASK-1.147 has already renamed
  `STEG_SOCKET_PATH` to `STEG_SERVER` everywhere, so this new file (and
  `cli_exit_statuses.rs`'s own pattern it borrows) sets `STEG_SERVER` on every
  subprocess it spawns, never the old name. AC #1, AC #2 and AC #3, all
  described in the proof table.

## Order of work

1. `steg_sdk` wire additions (`handshake.rs`, `connection.rs`,
   `local_socket.rs`'s `connect_handshaken`/`one_handshake_attempt`) with their
   own unit tests.
2. `crates/server`: `dispatch/mod.rs`'s `Context::from_hello` and its unit
   tests, in isolation (no socket, no `Runtime`).
3. `crates/server`: `connection.rs`'s `handle`/`run_post_handshake` wiring -
   merge `origin/main` first (TASK-1.140's own likely landing), per the note
   above.
4. `crates/steg`: `connect.rs`'s env-reading.
5. `crates/steg`: the new end-to-end test file - **this is the point where
   both TASK-1.146 and TASK-1.147 must already be merged**: the chain test
   (AC #1's pane clause, AC #3) needs a real pane to actually carry real
   `STEG_*` values (TASK-1.146), and the claim that a pane's `steg` reaches
   its own server through `STEG_SERVER` only holds once the rename has
   happened (TASK-1.147). If either is not yet merged when this step is
   reached, stop and say so rather than building against a still-placeholder
   environment or the old variable name.
6. Housekeeping: correct every doc comment this task's own code makes stale
   (`dispatch/mod.rs`, `Context`) - `.claude/rules/rust.md`/the code-design
   reminder: what the code says must stay true.

## Risks

- **Depends on TASK-1.146 and TASK-1.147.** Named above and in step 5. If
  TASK-1.146's plan changes what a pane's `STEG_*` values look like (field
  names, or whether `STEG_SERVER` is the bare socket path vs. something
  wrapped), this task's chain test and `Context::from_hello`'s parsing both
  need to agree with whatever it actually ships, not with this plan's own
  assumption of "the bare socket path, `SessionId`/`TabId`/`PaneId`'s own
  `Display` strings." If TASK-1.147 is still in flight, this task's new test
  file inherits whichever variable name is current at the time it is written
  - write it against `STEG_SERVER` only once TASK-1.147 is confirmed merged.
- **`crates/server/src/connection.rs` overlap with TASK-1.140.** Named above,
  at the point it matters, with the concrete action (merge `origin/main`
  before and after editing this file).
- **Hand-verified behaviour, not yet a committed test.** The AC #2/#3 "already
  works" findings above came from running the real binary by hand during
  planning (recorded above with the exact commands and output, using the then
  -current `STEG_SOCKET_PATH` name since this ran before TASK-1.147 existed),
  not from an existing automated test - that gap is exactly what step 5's new
  test file closes; until it lands, "already works" is this plan's own claim,
  not the repository's.

## Proof table

The strongest proof for AC #1's pane clause and AC #3 is one **chain test**: a
real pane whose own program is the real `steg` CLI itself, run with nothing
faked and nothing hand-set. `session new --` runs a one-line shell program
that calls `steg pane show --json`, redirects its output to a file (so the
test can read it back after the pane's own program exits - a command pane
stays once its program ends, section 6), then the test reads that file and
asserts the printed `id` equals the pane the test created. This exercises the
*whole* chain at once: the server writes real `STEG_*` into the pane (not
hand-set by the test, and only possible once TASK-1.146 is merged); the
pane's own `PATH` is what resolves `steg` to the running server's own binary
(AC #3); that `steg` resolves its socket from `STEG_SERVER` alone, through
`resolve_path`'s existing (by then renamed) read, since a pane's own
environment never carries any other way to reach the server (confirmed from
the `SESS=00000000`/`SRV=[]` capture above - `RealLauncher::spawn`'s
`.env_clear()` plus the login capture plus the five `STEG_*` overrides is the
pane's *entire* environment); and it resolves the target-less `pane show` to
exactly that pane (AC #1's pane clause).

A second, hand-set-environment test (`STEG_SESSION`/`TAB`/`PANE` given
directly to a `steg` subprocess the test spawns itself, with `STEG_SERVER`
set to point it at the test's own server) stays alongside it for the tab and
session clauses and as an isolating tool: if the chain test ever fails, the
hand-set test says whether the hello-to-`Context` half is the broken half or
not, independent of anything TASK-1.146 built.

Two negative controls prove AC #1's mechanism, deliberately kept separate so a
failure in "the server ignores the hello's context" and a failure in "the CLI
never sends it" are each seen failing on their own, not conflated into one
control that could pass for either reason:

- **Server-side control (`run_post_handshake`):** temporarily restore
  `context: Context::none()` in its `ConnectionState` literal, leaving
  `connect.rs`'s own env-reading untouched. Rerun the chain test and the
  hand-set test; confirm both now fail with "no target given, and this
  connection has no pane/tab/session context to fall back on" (today's real
  message) even though the CLI is still sending its context correctly. Revert.
- **CLI-side control (`connect.rs`):** temporarily pass `None, None, None` in
  place of the real `STEG_SESSION`/`TAB`/`PANE` reads, leaving
  `connection.rs`'s own `Context::from_hello` wiring untouched. Rerun the same
  two tests; confirm both now fail the same way, even though the server is
  still ready and willing to build a real `Context` - this is what proves the
  server-side control above was not merely a stand-in for a CLI-side bug, and
  vice versa. Revert.

| AC | Clause | Mechanism | Test | Kind | Negative control |
|---|---|---|---|---|---|
| #1 | no target, inside a pane, resolves to that pane | `Context::from_hello` + hello session/tab/pane fields + real `STEG_*` in the pane's own env (TASK-1.146) + `STEG_SERVER` (TASK-1.147) | The chain test above: a real pane's own program is `steg pane show --json`, output captured to a file; asserts the printed `id` is that pane's own real id | End to end | Both controls above (server-side and CLI-side), each run against this exact test |
| #1 | …or that tab | hand-set-environment test: real `steg` CLI subprocess with `STEG_SESSION`/`TAB`/`PANE` set to a real pane's own ids, `STEG_SERVER` set, env built from nothing (`.env_clear()`) plus exactly those four variables; `pane list` with no target (target: tab, doc-2 §8 - resolves through `default_tab`, unlike `tab list`, whose own target is a *session*), `--json`; asserts it lists exactly that pane, not a decoy pane created in a second session/tab | End to end | Both controls above, run against this test |
| #1 | …or that session | same hand-set test, `session show` with no target (target: session, resolves through `default_session`), `--json`; asserts the id matches, not the decoy session's | End to end | Both controls above, run against this test |
| #2 | no target, outside a pane, status 2 and says so | `Context::none()` (today's existing default, unchanged) | `cli_terminal_context.rs`: `pane show` with no target, no `STEG_*` vars set, real server; asserts status 2 and the stderr message | End to end | None needed - today's existing, unchanged behaviour; the test exists to pin it, not to prove a new mechanism (the empirical run above is this test's own expected output, already captured once by hand) |
| #3 | the `steg` a program finds belongs to its own server | `STEG_BIN_DIR` first on `PATH` (`launch_spec`'s `bin_dir`, unchanged by this task, built by TASK-1.146) | Covered by the chain test above; kept as its own assertion too, not only inferred: the chain test's pane program also echoes `command -v steg` into the same captured file, compared against `CARGO_BIN_EXE_steg`'s own path | End to end | Hand `pane_environment` a deliberately wrong `bin_dir` for one run (one real server is enough, doc-1 M1 - not two), confirm the captured `command -v steg` output no longer matches, revert |
| n/a | `Context::from_hello` itself: parses well-formed ids, degrades absent/malformed ones to `None` per field, never refuses | unit tests in `dispatch/mod.rs` | three unit tests (all present; all absent; one malformed) | Unit | A malformed-id test *is* its own negative control: asserts the malformed field alone is `None` while the other two still resolve, which only holds if parsing is independent per field |
| n/a | the hello itself carries session/tab/pane losslessly | unit test in `handshake.rs` | round-trip through `check_hello` | Unit | none needed - a pure serialisation round trip |

## Open questions for the engineer

- **Malformed or stale ids in the hello.** Recommendation above: degrade that one
  field to `None`, never refuse the whole handshake. The alternative (refuse the
  connection outright) seemed disproportionate - a value this far out of date
  can also just mean the pane's own session has since been closed, which is not
  a protocol error.
- **No cross-validation of session/tab/pane against the live tree at handshake
  time** (e.g. confirming the hello's tab really belongs to the hello's
  session). Recommendation: don't - resolution, and any `NotFound`, already
  happens correctly at the point a request is actually dispatched
  (`dispatch/lookup.rs`'s existing machinery), and the CLI only ever sends the
  three values straight from its own environment, which the server itself just
  set together as one unit (`pane_environment`), so they are never
  independently stale relative to each other in practice.
- **Exact new field names on `ClientHello`/`Handshake::Accept`**: `session`,
  `tab`, `pane` (bare, matching the `STEG_SESSION`/`STEG_TAB`/`STEG_PANE`
  variables they carry, without the `STEG_` prefix the wire's own `kind`/
  `label` fields also don't carry). Low-risk naming choice, named here so it is
  not decided silently.

(The `STEG_SOCKET_PATH`/`STEG_SERVER` precedence question an earlier draft of
this plan raised, with a recommended default, is withdrawn: the engineer
decided there is one variable, TASK-1.147 does the rename, and there is
nothing left to rank.)

## Housekeeping done during planning, for the record

Verifying AC #2/#3 by hand started several real on-demand servers and panes
under throwaway socket paths (never `/tmp` directly, since this machine's own
`/tmp` fails `directory_is_safe`'s ownership check - `$TMPDIR` sockets were
used instead, matching `ScopedTempDir`'s own reasoning). Checked afterward:
no `steg`/pane process from this task's own worktree or from any of these
throwaway sockets is still running (`ps`/`lsof` both confirm it; the one real
on-demand server left running on this machine, pid 79715, started two days
before this session and is unrelated to this task's own work - left alone,
never touched). Every throwaway socket, lock file and scratch directory this
planning created has been removed.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Split by @bruk.o on 2026-11-21: the server half of the plan (real STEG_SESSION, STEG_TAB, STEG_PANE and STEG_SERVER in a pane's environment) is TASK-1.146; this task keeps the hello's context, the server's use of it, the CLI's reading of the environment and the end-to-end tests, and builds once TASK-1.146 is merged.

Decided by the engineer on 2026-11-21: one variable. STEG_SOCKET_PATH is renamed to STEG_SERVER by TASK-1.147, which this task now also depends on; no precedence between two names is built here.

Plan (2220a19df068) checked against doc-2 R33 and section 4 by @bruk.o on 2026-11-21: ok. It builds only the context path: the hello carries the caller's session, tab and pane identifiers (never labels, as section 4 says), the server makes its dispatch context from them, and the CLI reads STEG_SESSION, STEG_TAB and STEG_PANE; no new flag, variable or exit status. AC 1 has a test per clause (pane by the chain test, tab and session by hand-set environment), with a server-side and a CLI-side control; AC 2 and AC 3 are pinned by end-to-end tests of behaviour that already holds. It adds no reading of the server address (TASK-1.147) and does not write the pane's environment (TASK-1.146). Three points for the engineer: a malformed or stale identifier in the hello drops that one field rather than refusing the connection; the hello's identifiers are not checked against the tree at handshake; the hello's new fields are named session, tab, pane.
<!-- SECTION:NOTES:END -->
