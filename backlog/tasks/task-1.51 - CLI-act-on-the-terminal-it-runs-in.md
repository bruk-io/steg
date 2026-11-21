---
id: TASK-1.51
title: 'CLI: act on the terminal it runs in'
status: In Progress
assignee:
  - '@bruk.w1'
created_date: '2026-11-15T01:33:00Z'
updated_date: '2026-11-21T20:24:00Z'
labels:
  - size-2
milestone: m-3
dependencies:
  - TASK-1.146
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
"Server: a pane's environment carries its real session, tab, pane and server"
(draft, waiting for the engineer to accept) - `Runtime::bind_server_address`/
`server_address`, `listen.rs`'s one-line call, and `runtime/start.rs`'s
`launch_spec` real-ids fix, with its own adapter test. This task now **depends
on that task and builds on it once merged**: the chain test below (AC #1's pane
clause and AC #3) only passes once both halves exist, since it needs a real
pane to actually carry real `STEG_*` values. Everything else below is this
task's own: the wire additions, `Context::from_hello`, `connection.rs`'s
wiring, the CLI's env-reading, the socket-resolution precedence, and the
end-to-end tests.

## Scope correction, found by running the real binary before planning

Built `./target/debug/steg` and ran it against real on-demand servers (throwaway
sockets under `$TMPDIR`, never `/tmp` directly - `arbiter::attempt`'s own
`directory_is_safe` refuses `/tmp` on this machine since it is not owned safely).
Two of the three ACs are already true in the shipped binary; only AC #1 needs new
production behaviour in this task (the split-out task supplies the other half).

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
  for `TERM`'s sake and it happens to be right for this too, and the split-out
  task does not change it. Nothing to build for the PATH/`STEG_BIN_DIR` half;
  only a real-binary test is missing, with a negative control that breaks it on
  purpose (see the proof table).
- **What is genuinely broken, confirmed from the same capture (the split-out
  task's own job):** the same pane printed `SRV=[]`, `SESS=00000000`,
  `TAB=00000000`, `PANE=00000000` - not the real session `00000001`/tab
  `00000002`/pane `00000003` the server had actually just minted.
  `runtime/start.rs::launch_spec` passes `pane_environment` a hardcoded empty
  server string and `SessionId::from_raw(0)`/`TabId::from_raw(0)`/
  `PaneId::from_raw(0)` - literal placeholders, not "whatever the real ids
  are". This is why AC #1 cannot work today even in principle: a CLI run
  inside a real pane would read `STEG_SESSION=0`/`STEG_TAB=0`/`STEG_PANE=0`
  off its own environment, which name nothing real.
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

### `crates/sdk` (wire format and socket resolution - additive, no existing call site breaks)
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
- `src/local_socket.rs`: two separate changes, since they answer two separate
  questions (what context a connection carries, and which socket it dials) -
  called out separately so neither reads as a side effect of the other:
  - `connect_handshaken`/`one_handshake_attempt` gain 3 new trailing
    `Option<&str>` params (`session`, `tab`, `pane`), threaded to
    `Connection::send_hello_with_context` in place of `send_hello`. Only 4 call
    sites total: 3 in this file's own test module (pass `None, None, None` -
    unrelated to context, retry-logic tests) and the one real caller,
    `crates/steg/src/cli/connect.rs`.
  - `resolve_path` gains a new fallback tier, **`STEG_SERVER`, between
    `STEG_SOCKET_PATH` and the `XDG_RUNTIME_DIR`/`TMPDIR`/`USER`-derived
    default** - the architecture rule (`.claude/rules/architecture.md`: "the
    sdk is steg's public interface: a consumer supplies the transport... none
    [of the CLI, server, app] holds protocol code of its own") puts socket
    resolution here, not in the CLI, so it is one decision in one place. Full
    order after this change: `options.socket_path` (an explicit override, e.g.
    a test's own throwaway path) > `STEG_SOCKET_PATH` (today's existing
    explicit override) > `STEG_SERVER` (this task's addition - the context a
    real pane's own program was given) > the `XDG_RUNTIME_DIR`/`TMPDIR`/`USER`
    default. `STEG_SOCKET_PATH` outranks `STEG_SERVER` because it is the
    narrower, more deliberately-set override (used throughout this
    workspace's own tests to point at a throwaway socket) - a value aimed
    specifically at redirecting a connection should not be shadowed by one a
    pane's own program merely inherited. `connect_and_handshake` (CLI, below)
    needs no change at all for this part: it already passes
    `socket_path: None`, so `resolve_path`'s own new tier is reached exactly
    when nothing more specific overrides it, with no CLI-side plumbing.
    `ConnectOptions.socket_path`'s own doc comment updated to name the new
    tier. New test, described in the proof table (two distinct real
    `UnixListener`s standing in for two distinct sockets - this needs real
    processes, not a bare unit test over `resolve_path` in isolation, since
    `resolve_path` reads live process environment directly via `env::var_os`,
    and this crate's own convention is never to mutate that process-wide
    state from a test, `std::env::set_var` being unsafe and racy under
    parallel `cargo test`).

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
  passes them through to `connect_handshaken`. It does **not** touch
  `ConnectOptions.socket_path` or read `STEG_SERVER` itself - that resolution
  now lives entirely in `steg_sdk::local_socket::resolve_path` (above), so the
  CLI stays a consumer of the sdk's public interface, never a second place
  that decides which socket to dial. Outside a pane, `STEG_SESSION`/`TAB`/
  `PANE` are simply absent and behaviour is unchanged (confirmed: every
  existing `crates/steg/tests/*.rs` fixture already runs with `.env_clear()`
  and never sets them).
- New test file `crates/steg/tests/cli_terminal_context.rs` (End to end, real
  binary, real server, real socket - `support::ScopedTempDir`, the same
  `run_steg`-with-`env_clear()` pattern `cli_exit_statuses.rs` already uses,
  and the same explicit discipline for every subprocess this file spawns:
  `.env_clear()` first, then exactly the variables each test names, never this
  test process's own ambient environment - SAFETY: this machine holds real
  credentials): AC #1, AC #2, AC #3, and the `STEG_SOCKET_PATH`/`STEG_SERVER`
  precedence, all described in the proof table.

## Order of work

1. `steg_sdk` wire additions (`handshake.rs`, `connection.rs`,
   `local_socket.rs`'s `connect_handshaken`/`one_handshake_attempt`) with their
   own unit tests.
2. `steg_sdk`: `local_socket.rs`'s `resolve_path` precedence change and its
   own real-listener test - independent of (1), can be built and tested in
   either order, grouped here because it is the same file.
3. `crates/server`: `dispatch/mod.rs`'s `Context::from_hello` and its unit
   tests, in isolation (no socket, no `Runtime`).
4. `crates/server`: `connection.rs`'s `handle`/`run_post_handshake` wiring -
   merge `origin/main` first (TASK-1.140's own likely landing), per the note
   above.
5. `crates/steg`: `connect.rs`'s env-reading.
6. `crates/steg`: the new end-to-end test file - **this is the point where
   the split-out server task must already be merged**, since the chain test
   (AC #1's pane clause, AC #3) needs a real pane to actually carry real
   `STEG_*` values. If that task is not yet merged when this step is reached,
   stop and say so rather than building against a still-placeholder
   environment.
7. Housekeeping: correct every doc comment this task's own code makes stale
   (`dispatch/mod.rs`, `Context`, `ConnectOptions.socket_path`) -
   `.claude/rules/rust.md`/the code-design reminder: what the code says must
   stay true.

## Risks

- **Depends on the split-out server task.** Named above and in step 6. If
  that task's plan changes what a pane's `STEG_*` values look like (field
  names, or whether `STEG_SERVER` is the bare socket path vs. something
  wrapped), this task's chain test and `Context::from_hello`'s parsing both
  need to agree with whatever it actually ships, not with this plan's own
  assumption of "the bare socket path, `SessionId`/`TabId`/`PaneId`'s own
  `Display` strings."
- **`crates/server/src/connection.rs` overlap with TASK-1.140.** Named above,
  at the point it matters, with the concrete action (merge `origin/main`
  before and after editing this file).
- **Hand-verified behaviour, not yet a committed test.** The AC #2/#3 "already
  works" findings above came from running the real binary by hand during
  planning (recorded above with the exact commands and output), not from an
  existing automated test - that gap is exactly what step 6's new test file
  closes; until it lands, "already works" is this plan's own claim, not the
  repository's.

## Proof table

The strongest proof for AC #1's pane clause and AC #3 is one **chain test**: a
real pane whose own program is the real `steg` CLI itself, run with nothing
faked and nothing hand-set. `session new --` runs a one-line shell program
that calls `steg pane show --json`, redirects its output to a file (so the
test can read it back after the pane's own program exits - a command pane
stays once its program ends, section 6), then the test reads that file and
asserts the printed `id` equals the pane the test created. This exercises the
*whole* chain at once: the server writes real `STEG_*` into the pane (not
hand-set by the test, and only possible once the split-out task is merged);
the pane's own `PATH` is what resolves `steg` to the running server's own
binary (AC #3); that `steg` resolves its socket from `STEG_SERVER` alone,
through `resolve_path`'s new tier, since a pane's own environment never
carries `STEG_SOCKET_PATH` (confirmed from the `SESS=00000000`/`SRV=[]`
capture above - `RealLauncher::spawn`'s `.env_clear()` plus the login capture
plus the five `STEG_*` overrides is the pane's *entire* environment); and it
resolves the target-less `pane show` to exactly that pane (AC #1's pane
clause).

A second, hand-set-environment test (`STEG_SESSION`/`TAB`/`PANE` given
directly to a `steg` subprocess the test spawns itself, over `STEG_SOCKET_PATH`
rather than `STEG_SERVER`) stays alongside it for the tab and session clauses
and as an isolating tool: if the chain test ever fails, the hand-set test says
whether the hello-to-`Context` half is the broken half or not, independent of
anything the split-out task built.

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
| #1 | no target, inside a pane, resolves to that pane | `Context::from_hello` + hello session/tab/pane fields + real `STEG_*` in the pane's own env (split-out task) + `resolve_path`'s `STEG_SERVER` tier | The chain test above: a real pane's own program is `steg pane show --json`, output captured to a file; asserts the printed `id` is that pane's own real id | End to end | Both controls above (server-side and CLI-side), each run against this exact test |
| #1 | …or that tab | hand-set-environment test: real `steg` CLI subprocess with `STEG_SESSION`/`TAB`/`PANE` set to a real pane's own ids, `STEG_SOCKET_PATH` set, env built from nothing (`.env_clear()`) plus exactly those four variables; `pane list` with no target (target: tab, doc-2 §8 - resolves through `default_tab`, unlike `tab list`, whose own target is a *session*), `--json`; asserts it lists exactly that pane, not a decoy pane created in a second session/tab | End to end | Both controls above, run against this test |
| #1 | …or that session | same hand-set test, `session show` with no target (target: session, resolves through `default_session`), `--json`; asserts the id matches, not the decoy session's | End to end | Both controls above, run against this test |
| #2 | no target, outside a pane, status 2 and says so | `Context::none()` (today's existing default, unchanged) | `cli_terminal_context.rs`: `pane show` with no target, no `STEG_*` vars set, real server; asserts status 2 and the stderr message | End to end | None needed - today's existing, unchanged behaviour; the test exists to pin it, not to prove a new mechanism (the empirical run above is this test's own expected output, already captured once by hand) |
| #3 | the `steg` a program finds belongs to its own server | `STEG_BIN_DIR` first on `PATH` (`launch_spec`'s `bin_dir`, unchanged by this task, built by the split-out task) | Covered by the chain test above; kept as its own assertion too, not only inferred: the chain test's pane program also echoes `command -v steg` into the same captured file, compared against `CARGO_BIN_EXE_steg`'s own path | End to end | Hand `pane_environment` a deliberately wrong `bin_dir` for one run (one real server is enough, doc-1 M1 - not two), confirm the captured `command -v steg` output no longer matches, revert |
| n/a (socket resolution, feeds AC #1's own chain test) | `STEG_SOCKET_PATH` outranks `STEG_SERVER`, which outranks the default | new test in `cli_terminal_context.rs` (or a sibling file, if `resolve_path`'s own test needs a different fixture shape): two real `UnixListener`s at two different throwaway paths; run the real `steg` binary with both `STEG_SOCKET_PATH` and `STEG_SERVER` set, to different sockets, and confirm only the `STEG_SOCKET_PATH` one is ever accepted; then with only `STEG_SERVER` set, confirm that one is accepted instead | End to end | The test *is* two cases proving each other's absence (`STEG_SOCKET_PATH` wins when both are set; `STEG_SERVER` is reached only when it is not) - no separate revert needed, since both branches are exercised directly |
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
- **`STEG_SOCKET_PATH`/`STEG_SERVER` precedence**: recommendation above -
  explicit `STEG_SOCKET_PATH` wins, `STEG_SERVER` otherwise, then the default,
  decided once in `steg_sdk::local_socket::resolve_path`. Named here because
  it is a connection-resolution behaviour change, even though it is a no-op in
  M1 (one server per host/user, so `STEG_SERVER` and the default XDG-based
  resolution always agree today, and nothing in this workspace's own tests
  sets `STEG_SERVER` at all, so no existing test can disagree with this order).
- **Exact new field names on `ClientHello`/`Handshake::Accept`**: `session`,
  `tab`, `pane` (bare, matching the `STEG_SESSION`/`STEG_TAB`/`STEG_PANE`
  variables they carry, without the `STEG_` prefix the wire's own `kind`/
  `label` fields also don't carry). Low-risk naming choice, named here so it is
  not decided silently.

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
<!-- SECTION:NOTES:END -->
