---
id: TASK-1.51
title: 'CLI: act on the terminal it runs in'
status: In Progress
assignee:
  - '@bruk.w1'
created_date: '2026-11-15T01:33:00Z'
updated_date: '2026-11-21T20:21:00Z'
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
## Scope correction, found by running the real binary before planning

Built `./target/debug/steg` and ran it against real on-demand servers (throwaway
sockets under `$TMPDIR`, never `/tmp` directly - `arbiter::attempt`'s own
`directory_is_safe` refuses `/tmp` on this machine since it is not owned safely).
Two of the three ACs are already true in the shipped binary; only AC #1 needs new
production behaviour. This changes what the task actually builds:

- **AC #2 already holds, unchanged.** `steg pane show` with no target, no `STEG_*`
  context, against a fresh server: exits 2 with `steg: no target given, and this
  connection has no pane context to fall back on`. `Context::none()` ->
  `missing_context` -> `ErrorCode::BadRequest` -> the description's own
  `exit_status` already gives 2 and already says so. Nothing to build; only a
  real-binary test is missing.
- **AC #3's own wording ("the steg a program finds") already holds.** Captured a
  real pane's screen (`session new --` followed by a one-line shell program,
  `echo BIN=$(command -v steg); ...`):
  `BIN=` printed the exact path of the running server's own binary. `launch_spec`'s
  `bin_dir` (`std::env::current_exe().parent()`) was already right - TASK-1.87 built
  it for `TERM`'s sake and it happens to be right for this too. Nothing to build for
  the PATH/`STEG_BIN_DIR` half; only a real-binary test is missing, with a negative
  control that breaks it on purpose (see the proof table).
- **What is genuinely broken, confirmed from the same capture:** the same pane
  printed `SRV=[]`, `SESS=00000000`, `TAB=00000000`, `PANE=00000000` - not the real
  session `00000001`/tab `00000002`/pane `00000003` the server had actually just
  minted. `runtime/start.rs::launch_spec` passes `pane_environment` a hardcoded
  empty server string and `SessionId::from_raw(0)`/`TabId::from_raw(0)`/
  `PaneId::from_raw(0)` - literal placeholders, not "whatever the real ids are".
  This is why AC #1 cannot work today even in principle: a CLI run inside a real
  pane would read `STEG_SESSION=0`/`STEG_TAB=0`/`STEG_PANE=0` off its own
  environment, which name nothing real.
- **The other half of AC #1's mechanism is also missing:** `crates/server/src/
  connection.rs::run_post_handshake` hard-codes `context: Context::none()` for
  every connection, always - nothing on the wire today carries a connecting
  client's own `STEG_SESSION`/`TAB`/`PANE` back to the server at all (the hello
  carries only `kind`/`label`, confirmed by reading `crates/sdk/src/wire/
  handshake.rs` and its own "nothing on the wire supplies a session, tab, or pane
  today" comment in `crates/server/src/dispatch/mod.rs`, which already names this
  as "the server's own remaining job"). This is this task's own job to finish.

So the real shape of the task: (1) make a pane's own `STEG_SERVER`/`SESSION`/
`TAB`/`PANE` carry real values instead of placeholders; (2) carry a connecting
CLI's own copy of those three back to the server, over the hello, so the server
can build a real `Context` instead of `Context::none()`; (3) have the CLI actually
use `STEG_SERVER` once it is real, rather than minting a correct value nothing
ever reads (if the CLI never reads it, a reviewer rightly flags it as a dead,
untested promise). AC #2 and most of AC #3 need only new tests, not new behaviour.

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
  `crates/steg/src/cli/connect.rs`.

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
  attachment/data-frame handling. This plan only touches `handle`'s hello-accept
  branch and `run_post_handshake`'s signature; it does not touch
  `handle_request`, `pane_streams`, or anything TASK-1.140 owns, but both
  changes land in the same file, so a conflict is likely and should be
  flagged, not silently merged**): in `handle`, the `Handshake::Accept { kind,
  label, .. }` match widens to `Handshake::Accept { kind, label, session, tab,
  pane, .. }` and builds `let context = Context::from_hello(session.as_deref(),
  tab.as_deref(), pane.as_deref());` right there (same place `kind`/`label` are
  already read). `run_post_handshake` gains a `context: Context` parameter
  (its one caller, `handle`, passes the new value); its own `ConnectionState`
  literal uses that parameter instead of the hard-coded `Context::none()`.
- `src/runtime/mod.rs`: `Runtime` gains one new private field,
  `server_address: Mutex<Option<String>>`, set to `Mutex::new(None)` inside
  `Runtime::new`'s existing struct literal (**`Runtime::new`'s own parameter
  list is not touched** - there are 43 `Runtime::new(` call sites across this
  crate's tests; adding a constructor parameter would touch all of them for no
  reason, since every one of those call sites runs with no socket at all).
  Two new `pub(crate)` methods, mirroring `listen.rs`'s own `Stopper::
  bind_socket_path`/its `Mutex<Option<PathBuf>>` field exactly (a value
  genuinely unknown until the socket is bound, set once, read later):
  `bind_server_address(&self, address: String)` and `server_address(&self) ->
  String` (empty string when unset - the same value every existing test already
  gets today, since none of them call `bind_server_address` and none of their
  assertions depend on it).
- `src/listen.rs`: in `run`, immediately after `Runtime::new(...)` and before
  the accept loop starts, `runtime.bind_server_address(socket_path.display()
  .to_string());` - `socket_path` is already a parameter of `run` at that point,
  so this is a one-line addition, no new plumbing needed to get the value there.
- `src/runtime/start.rs`: `start_and_dispatch` computes `tab_id =
  state.tree.tab_of_pane(new_pane).expect(...)` and `session_id =
  session_owning_tab(&state.tree, tab_id).expect(...)` (same helper already
  imported at the top of this file) right after minting `new_pane`, *before*
  calling `launch_spec` (today it is called after, inside `emit_creation_events`,
  too late for `launch_spec`'s own needs). `launch_spec`'s signature gains
  `pane: PaneId, tab: TabId, session: SessionId`; its body passes
  `&self.server_address()`, `session`, `tab`, `pane` to `pane_environment`
  instead of `""`/three `from_raw(0)` placeholders. Updates this function's own
  doc comment, which currently states the placeholder is deliberate pending
  "TASK-1.44/1.45" - that task is this one. New test:
  `a_panes_own_steg_session_tab_and_pane_match_its_real_tree_ids` (Adapter: real
  pty, through `Runtime::handle`'s public surface, the same shape this file's
  own `steg_vars_and_term_always_win...` test already uses) - echoes `$STEG_
  SESSION`/`$STEG_TAB`/`$STEG_PANE` from a real pane and compares against the
  real ids read back off `state.tree` for that same pane.

### `crates/steg`
- `src/cli/connect.rs`: `connect_and_handshake` reads `STEG_SESSION`/`STEG_TAB`/
  `STEG_PANE` from its own process environment (`std::env::var(...).ok()`) and
  passes them through to `connect_handshaken`. It also reads `STEG_SERVER`
  (`std::env::var_os`) and, when present, sets `ConnectOptions.socket_path` to
  it directly, in place of today's hard-coded `None` - otherwise this task mints
  a correct `STEG_SERVER` that nothing ever reads, which is worse than not
  minting one (flagged by review before being built, not found by one after).
  Outside a pane, both are simply absent and behaviour is unchanged (confirmed:
  every existing `crates/steg/tests/*.rs` fixture already runs with `.env_clear()`
  and never sets either).
- New test file `crates/steg/tests/cli_terminal_context.rs` (End to end, real
  binary, real server, real socket - `support::ScopedTempDir`, the same
  `run_steg`-with-`env_clear()` pattern `cli_exit_statuses.rs` already uses):
  AC #1 and AC #2 together, described in the proof table.

## Order of work

1. `steg_sdk` wire additions (`handshake.rs`, `connection.rs`, `local_socket.rs`)
   with their own unit tests - no other crate depends on anything else yet, and
   this is the one part with the most call-site surface to get right first.
2. `crates/server`: `dispatch/mod.rs`'s `Context::from_hello` and its unit
   tests, in isolation (no socket, no `Runtime`).
3. `crates/server`: `runtime/mod.rs`'s `server_address`/`bind_server_address`
   and `listen.rs`'s one-line call - small, mechanical, testable with a trivial
   unit test (set then get).
4. `crates/server`: `runtime/start.rs`'s `launch_spec` real-ids fix and its new
   adapter test, with the negative control (revert to placeholders, confirm the
   new test fails for the right reason, then restore).
5. `crates/server`: `connection.rs`'s `handle`/`run_post_handshake` wiring,
   last within this crate, since it is the one piece that depends on both (2)
   and needs (3)/(4) to matter for anything observable end to end.
6. `crates/steg`: `connect.rs`'s env-reading and `STEG_SERVER` preference.
7. `crates/steg`: the new end-to-end test file, with both negative controls
   (described below), run against the final head.
8. Housekeeping: correct every doc comment this task's own code makes stale
   (`dispatch/mod.rs`, `Context`, `launch_spec`, `pane_environment` if its own
   doc still references "no real source yet") - `.claude/rules/rust.md`/the
   code-design reminder: what the code says must stay true.

## Risks

- **Scope for a size-2 label.** Even with AC #2/#3 reduced to test-only, this
  still touches three crates (`sdk` wire shape, `server` Context + `Runtime` +
  `launch_spec`, `steg` CLI) and adds a new end-to-end test file. Flagging this
  plainly rather than understating it: if the engineer judges this too big for
  size-2 once the plan is read, the cut that actually works is **steps 3-4
  (the server writes real `STEG_*` into a pane's own environment, with its own
  adapter test, provable with no CLI or wire change at all) versus steps 1-2
  and 5-7 (the wire additions, `Context::from_hello`, the CLI's own env-reading
  and `STEG_SERVER` preference, and the end-to-end tests)** - not "1-4 vs. 5-7"
  as an earlier draft of this plan had it: the wire additions in step 1 have no
  consumer until step 6, so splitting there would ship a first half with dead,
  untested fields on the wire. Steps 3-4 stand alone and are independently
  provable; steps 1-2 and 5-7 are one coherent piece that depends on them.
- **`crates/server/src/connection.rs` overlap with TASK-1.140.** Named above,
  at the point it matters. This plan's only edits there are `handle`'s
  hello-accept branch and `run_post_handshake`'s signature/`ConnectionState`
  literal - not `handle_request`, not the attachment/data-frame handling
  TASK-1.140 owns - but both land in the same file, so expect a merge conflict,
  not a silent clean merge.
- **Hand-verified behaviour, not yet a committed test.** The AC #2/#3 "already
  works" findings above came from running the real binary by hand during
  planning (recorded above with the exact commands and output), not from an
  existing automated test - that gap is exactly what step 7's new test file
  closes; until it lands, "already works" is this plan's own claim, not the
  repository's.

## Proof table

The strongest proof for AC #1's pane clause, AC #3, and the `STEG_SERVER`
preference is one **chain test**: a real pane whose own program is the real
`steg` CLI itself, run with nothing faked and nothing hand-set. `session new --`
runs a one-line shell program that calls `steg pane show --json`, redirects its
output to a file (so the test can read it back after the pane's own program
exits - a command pane stays once its program ends, section 6), then the test
reads that file and asserts the printed `id` equals the pane the test created.
This exercises the *whole* chain at once: the server writes real `STEG_*` into
the pane (not hand-set by the test); the pane's own `PATH` is what resolves
`steg` to the running server's own binary (AC #3); that `steg` resolves its
socket from `STEG_SERVER` alone, since a pane's own environment never carries
`STEG_SOCKET_PATH` (confirmed from the `SESS=00000000`/`SRV=[]` capture above -
`RealLauncher::spawn`'s `.env_clear()` plus the login capture plus the five
`STEG_*` overrides is the pane's *entire* environment); and it resolves the
target-less `pane show` to exactly that pane (AC #1's pane clause).

A second, hand-set-environment test (STEG_SESSION/TAB/PANE given directly to a
`steg` subprocess the test spawns itself, pointed at the server over
`STEG_SOCKET_PATH`) stays alongside it for the tab and session clauses and
as an isolating tool: if the chain test ever fails, the hand-set test says
whether the hello-to-`Context` half is the broken half or not.

| AC | Clause | Mechanism | Test | Kind | Negative control |
|---|---|---|---|---|---|
| #1 | no target, inside a pane, resolves to that pane | `Context::from_hello` + hello session/tab/pane fields + real `STEG_*` in the pane's own env + the CLI preferring `STEG_SERVER` | The chain test above: a real pane's own program is `steg pane show --json`, output captured to a file; asserts the printed `id` is that pane's own real id | End to end | Temporarily restore `context: Context::none()` in `run_post_handshake`'s `ConnectionState` literal; rerun; confirm the pane's own `steg pane show` now prints "no target given, and this connection has no pane context to fall back on" to the captured output (today's real message) instead of succeeding; revert |
| #1 | …or that tab | hand-set-environment test: real `steg` CLI subprocess with `STEG_SESSION`/`TAB`/`PANE` set to a real pane's own ids, `STEG_SOCKET_PATH` set; `pane list` with no target (target: tab, doc-2 §8 - resolves through `default_tab`, unlike `tab list`, whose own target is a *session*), `--json`; asserts it lists exactly that pane, not a decoy pane created in a second session/tab | End to end | same revert/rerun as the chain test's control (one shared mechanism) |
| #1 | …or that session | same hand-set test, `session show` with no target (target: session, resolves through `default_session`), `--json`; asserts the id matches, not the decoy session's | End to end | same revert/rerun |
| #2 | no target, outside a pane, status 2 and says so | `Context::none()` (today's existing default, unchanged) | `cli_terminal_context.rs`: `pane show` with no target, no `STEG_*` vars set, real server; asserts status 2 and the stderr message | End to end | None needed - today's existing, unchanged behaviour; the test exists to pin it, not to prove a new mechanism (the empirical run above is this test's own expected output, already captured once by hand) |
| #3 | the `steg` a program finds belongs to its own server | `STEG_BIN_DIR` first on `PATH` (`launch_spec`'s `bin_dir`, unchanged by this task) | Covered by the chain test above (`command -v steg` is implicit in which `steg` actually answers `pane show`); kept as its own assertion too, not only inferred: the chain test's pane program also echoes `command -v steg` into the same captured file, compared against `CARGO_BIN_EXE_steg`'s own path | End to end | Hand `pane_environment` a deliberately wrong `bin_dir` for one run (not two real servers - one server is enough, doc-1 M1), confirm the captured `command -v steg` output no longer matches, revert |
| n/a (feeds #1 and #3's own context table, not a numbered AC by itself) | a pane's own `STEG_SESSION`/`TAB`/`PANE`/`SERVER` are real, not placeholders | `launch_spec` passing real ids + `Runtime::server_address()` | `runtime/start.rs::a_panes_own_steg_session_tab_and_pane_match_its_real_tree_ids` | Adapter (real pty, through `Runtime::handle`) | Revert `launch_spec` to the `from_raw(0)`/`""` placeholders, confirm the echoed values read back `0`/`0`/`0`/empty instead of the real tree ids, revert back |
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
- **`STEG_SERVER`'s value and the CLI preferring it.** Recommendation above
  (read it, use it as `ConnectOptions.socket_path` when present): without this,
  the task mints a variable nothing reads, which is the shape of gap a review
  finds. Flagging because it is a connection-resolution behaviour change, even
  though it is a no-op in M1 (one server per host/user, so `STEG_SERVER` and
  the default XDG-based resolution always agree today).
- **Exact new field names on `ClientHello`/`Handshake::Accept`**: `session`,
  `tab`, `pane` (bare, matching the `STEG_SESSION`/`STEG_TAB`/`STEG_PANE`
  variables they carry, without the `STEG_` prefix the wire's own `kind`/
  `label` fields also don't carry). Low-risk naming choice, named here so it is
  not decided silently.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Split by @bruk.o on 2026-11-21: the server half of the plan (real STEG_SESSION, STEG_TAB, STEG_PANE and STEG_SERVER in a pane's environment) is TASK-1.146; this task keeps the hello's context, the server's use of it, the CLI's reading of the environment and the end-to-end tests, and builds once TASK-1.146 is merged.
<!-- SECTION:NOTES:END -->
