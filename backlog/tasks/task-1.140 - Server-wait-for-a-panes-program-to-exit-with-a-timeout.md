---
id: TASK-1.140
title: 'Server: wait for a pane''s program to exit, with a timeout'
status: In Review
assignee:
  - '@bruk.w2'
created_date: '2026-11-18T14:09:00Z'
updated_date: '2026-11-21T22:09:00Z'
labels:
  - size-3
  - review-passed
milestone: m-3
dependencies:
  - TASK-1.90
references:
  - task-1.140-server-wait-for-a-pane-s-program-to-exit
  - https://github.com/bruk-io/steg/pull/92
parent_task_id: TASK-1
project: bruk-io-steg
asdlc_accepted_from: DRAFT-4
asdlc_branch: task-1.140-server-wait-for-a-pane-s-program-to-exit
asdlc_plan_approved_by: bruk
asdlc_plan_approved_hash: '83254935a0f3'
asdlc_plan_approved_size: size-3
asdlc_review_kind: same-machine
asdlc_review_findings: 'important=0,nit=1'
asdlc_review_policies: ["review", "testing-rust"]
asdlc_plan_matched: no
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Split from TASK-1.90 by its plan on 2026-11-18 (its third criterion). doc-2 sections 6 and 8: pane wait --for exit exits 0 when the program ended and prints the program's own exit code in its result; a wait that times out is status 1. TASK-1.90's plan holds the worked design as a starting point: pending waits kept in the runtime's state and resolved from the code paths that already hold the lock when a program exits or a pane is closed, with a timer only when a timeout is given that wakes at once on resolution. It needs a new module in crates/server/src/runtime/ that TASK-1.131's map did not anticipate, and probably a new error code for the timeout, which is a change to the API description for the engineer to agree. Built after pane send and pane capture: all three land in crates/server/src/dispatch/pane.rs.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 pane wait answers when the pane's program exits, with its exit code, or when the timeout given passes; a wait on a pane whose program has already ended answers at once; a pane closed while being waited on answers at once
- [ ] #2 A waiting request holds neither the runtime's lock nor a connection's reader; the plan says how the request is held and answered now that a reply is written by the connection's writer
- [ ] #3 A client that disconnects while waiting leaves no thread, timer or registration behind, shown by a test
- [ ] #4 pane wait leaves the not-yet list in dispatch; it is proved with a real program in a real pty and bounded waits, and what the timeout answers (its code and the CLI's exit status under doc-2 section 6) is settled with the engineer in the plan
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
REVISED 2026-11-21: the engineer answered the three open questions the previous version of this plan raised (recorded in this task's own notes by @bruk.o). What changed from that version: (1) `ErrorCode::TimedOut` is now unconditional, not contingent on agreement, and its message must name the expired timeout in seconds; (2) `--timeout 0` means "check now and answer at once" (a third branch, not the earlier plan's "clamp to an immediate timeout through the gate/timer"), and a negative `--timeout` is `BadRequest` (status 2), refused before the target is resolved or anything registered, not clamped; (3) the reply is a typed `steg_sdk` record, not inline JSON. `register_wait`'s own decision tree, the sdk file list, and the proof section are updated below to match; everything else (the core resolution mechanism, the other changed files, AC #2/#3's own tests) is unchanged and not re-derived. origin/main was merged into this branch first (clean, no conflicts) - unrelated work only; nothing in it touches any file this task changes.

Starting point: TASK-1.90's own stale plan (§4, written 2026-11-18, before the split) already worked this design out in detail and is re-checked against the tree today, not re-derived from scratch. Re-checked facts: pane.wait is already fully described (crates/sdk/src/description/methods/pane.rs - target, for=Choice(["exit"]) required, timeout: Integer (seconds, doc-2 section 5) optional, Returns::Record{exit_code}, errors today BadRequest/NoSuchTarget/AmbiguousTarget only - no TimedOut); dispatch/pane.rs's "wait" arm and pane::HANDLED still list it, still answering NotImplementedYet citing this task; cross_cutting.rs:90 still expects that; TASK-1.88 (server.*/client.*) has since landed, so the ErrorCode-enum collision the stale plan flagged is gone. pane.capture landed (TASK-1.139) with the exact hand-off shape (CaptureSender/CAPTURE_TIMEOUT) the stale plan predicted, confirming the sibling precedent is sound; wait touches none of that code.

Not a split: one coherent cross-cutting mechanism (new runtime state, three resolution sites, one connection-level interception); the acceptance criteria are already scoped to exactly this.

DOCUMENT CHANGE TO FLAG FOR THE ORCHESTRATOR (not edited here)

doc-2 section 5's `--timeout` row and section 6's exit-status table don't currently say what `0` or a negative value does for `pane.wait` specifically (section 5 only says "Give up after this long, with status 1"; section 6 names no error codes at all, only CLI exit statuses). Once this task lands, doc-2 is missing: (a) that `0` means "check now," (b) that a negative value is refused as bad usage, and (c) that the timeout case now has its own code (`TimedOut`) underneath status 1, the same way `ProgramNotRunning`/`ProgramRunning` already sit underneath status 1 without doc-2 naming them by code. Flagged for the orchestrator to add to doc-2 (or decide it doesn't need to) - not edited in this plan or by this task.

DESIGN

Core decision, unchanged from the stale plan: pending waits live in Runtime's own state, resolved from the code paths that already hold its lock when the outcome becomes known (a real exit, a close), with a timer thread only when a positive `--timeout` is given, woken early by a condition variable rather than ever sleeping out a timeout that is already answered. No worker-thread command, no polling loop, anywhere.

One deliberate simplification over the stale plan: its WaitGate carried the outcome through a `Mutex<Option<T>>`. It doesn't need to - whichever resolver fires first (the exit-event drain, the close path, or a real timeout) writes the real reply straight to the wait's own `Arc<Outbound>` and that is the only place the answer ever lives. The gate's only job is waking a sleeping timer thread early so it doesn't sit out the rest of a timeout whose answer is already known. So:

- `crates/server/src/runtime/wait.rs` (new - the seam TASK-1.131's own module map didn't anticipate, named as such in the task's own description):
  - `struct WaitGate { state: Mutex<bool>, changed: Condvar }` - `resolve()` sets `true` and notifies; `wait(timeout)` blocks on `wait_timeout_while(|done| !*done)` and returns nothing (the caller always re-checks the real state afterward - see below - so there is nothing useful to return here).
  - `struct PendingWait { request_id: u64, client_id: ClientId, outbound: Arc<Outbound>, gate: Option<Arc<WaitGate>> }` - `gate` is `Some` only when a positive `--timeout` was given.
  - `pub(super) type PendingWaits = HashMap<PaneId, Vec<PendingWait>>` - a `Vec` because more than one client can wait on the same pane, and the same client can issue more than one wait on it (each its own `request_id`).
  - `pub(super) fn resolve_pane_waits_locked(state: &mut State, pane: PaneId, outcome: Result<i32, ApiError>)` - called with `state` already locked: `remove`s every `PendingWait` for `pane`, and for each one builds `Response::Ok(serde_json::to_value(&WaitResult{exit_code: code}))` or `Response::Error(error.clone())` from `outcome`, writes it with `Connection::reply(request_id, ...)` onto `wait.outbound`, and calls `gate.resolve()` on every one that has a gate, so its timer thread wakes now instead of at its own deadline. Only ever called with `Ok(exit_code)` (the exit drain) or the close path's own `Err(NoSuchTarget)` - never a timeout, which is resolved separately (below). A failed `enqueue_control` here is best-effort and silently dropped (`let _ = ...`), the same tolerance `attach_pane` already documents - deliberately **not** `broadcast`'s own "shut the socket down" response to a full queue: `broadcast` fans one event out to every subscriber and must stop a slow one from piling up unboundedly; a wait's reply is a single, one-shot, point-to-point answer, so a failed enqueue here only means the connection's own outbound is already closed (nothing left to tell) or already full past `CONTROL_CAP` (256 other undelivered replies ahead of it - a connection in that state is not being read at all and is already on its way to being torn down by the existing stuck-writer/shutdown machinery `connection.rs`'s own doc describes, not something this mechanism needs to additionally detect).
  - `fn timed_out(timeout_secs: i64) -> ApiError` - the one place the `TimedOut` message is built: `ApiError::new(ErrorCode::TimedOut, format!("the wait's own {timeout_secs} second timeout passed before the condition was met"))`, naming the expired timeout per the engineer's answer. A pure function, not a resolver itself - it has two independent callers, below, one of which never touches `pending_waits` at all.
  - `fn timeout_wait_locked(state: &Arc<Mutex<State>>, pane: PaneId, request_id: u64, client_id: ClientId, timeout_secs: i64)` - locks, finds and removes the one `PendingWait` in `pending_waits[pane]` matching `(request_id, client_id)` (several waits can be pending on the same pane - matching both, not just the pane, is what keeps them independent), and **only if it was still there**, replies `timed_out(timeout_secs)` on its own outbound. If it is already gone (the exit/close path resolved it first, in the race between that path dropping the lock and this thread waking), this is a no-op - never a reply sent twice. This function's whole job is "find-and-remove, then maybe reply" - it is never called for `--timeout 0` (below), which never registered anything for it to find: that branch builds and sends `timed_out(0)` directly, with no lock-held removal step at all, since calling `timeout_wait_locked` there would find nothing and silently fail to answer.
  - `impl Runtime { fn register_wait(&self, description, context, request, waiting: Waiting, outbound: &Arc<Outbound>) }`, `Waiting { client: ClientId, id: u64 }` grouped the same way `Attaching` groups `attach_pane`'s own three identifying values, for the same clippy argument-count reason. Body, mirroring `attach_pane`'s own shape (validate, lock, resolve, release, no lock held across anything that can block), now a six-step decision tree:
    1. `validate(description, request)?` (as `ApiError`); on `Err`, reply it on `outbound` and return - the same self-enqueuing shape `attach_pane` already uses, so `connection.rs`'s own arm is a single call with nothing left to build.
    2. **Before anything else is touched** (the engineer's own wording: refused before the target is resolved or anything is registered): read `timeout_secs = dispatch::param_i64(request, "timeout")`. If `Some(secs) if secs < 0`, reply `ErrorCode::BadRequest` ("a wait's own --timeout must not be negative") on `outbound` and return - no lock taken, no target resolved. The same manual-range-check shape `connection.rs`'s own `"tab.declare-size"` arm already uses for `columns`/`rows` outside `validate` (`ValueKind::Integer` declares no min/max either way).
    3. Lock. `dispatch::target_pane(&state.tree, context, request)` - on `Err`, reply it and return (unlocked first).
    4. Read the pane's own `Pane::program_status()` the same way `dispatch::pane::show`'s `record_for` already does (via `tree.tab_of_pane` + `find_tab`/`tab.pane`, both already `pub(crate)` from `dispatch::lookup`).
    5. `Some(ProgramStatus::Ended{exit_code})`: reply `Ok(WaitResult{exit_code})` at once, no registration at all (AC #1's "already ended... answers at once") - reached regardless of what `--timeout` was given, including `0` or none.
    6. `Some(ProgramStatus::Running)` (the only other case reachable in practice - a pane with no program at all would mean `None`, and every pane in the tree has one started under the same lock it was minted, `close.rs`'s own documented invariant; kept, not special-cased, since nothing sane changes if it were somehow reached), by `timeout_secs`:
       - `None` (the flag was never given): register the pending wait, `gate: None`, no thread - waits indefinitely.
       - `Some(0)`: reply `timed_out(0)` directly on `outbound` - **no registration, no thread, no lock-held removal step** (there is nothing registered yet to remove) - the engineer's "check now and return at once."
       - `Some(secs)` where `secs > 0`: push `PendingWait { request_id: waiting.id, client_id: waiting.client, outbound: Arc::clone(outbound), gate: Some(Arc::new(WaitGate::new())) }` into `state.pending_waits.entry(pane).or_default()`, drop the lock, and `thread::spawn` a detached thread holding `Arc::clone(&self.state)` (the same `Arc<Mutex<State>>` clone the drain thread's own spawn already closes over - the precedented way this crate hands a background thread something Send+'static without handing it `&Runtime`, which the connection's own stack frame cannot promise outlives it) that calls `gate.wait(Duration::from_secs(secs as u64))` (safe: `secs > 0` is already established) then unconditionally calls `timeout_wait_locked(..., secs)` (itself the no-op-if-already-resolved check, so there is nothing to branch on here).
  - `impl Runtime { fn cancel_waits(&self, client_id: ClientId) }` - locks once, and for every pane's `Vec<PendingWait>` removes every entry whose `client_id` matches (via `retain`, calling `gate.resolve()` on each removed one with a gate before dropping it). No reply is ever sent here - the connection is already gone.
  - `pub(crate) fn pending_wait_count_for_test(&self, pane: PaneId) -> usize` - **not** `#[cfg(test)]`, the same way `attachment_count_for_test` already isn't: compiled into the crate unconditionally, visible to this crate's own `#[cfg(test)] mod tests` (e.g. in `connection.rs`), never exported to an external `tests/` integration binary, which only ever sees the public API.

- `crates/server/src/runtime/mod.rs`: `mod wait;` added to the module list; `pub(crate) use wait::Waiting;` added alongside the existing `pub(crate) use attach::Attaching;`. `State` gains `pending_waits: wait::PendingWaits`, initialised empty in `Runtime::new`.

- `crates/server/src/runtime/events.rs`, `apply_event_locked`'s `PaneEvent::Exited` arm: as the first statement inside the existing `Ok(outcome) => { ... }` block (ahead of the `broadcast`/cascade code, unchanged below it), `super::wait::resolve_pane_waits_locked(state, pane, Ok(exit_code))` - correct regardless of whether `outcome` is `Stayed` or `Closed`, since a wait only ever cares about the exit code, not the cascade. The `Err(PaneNotFound(_))` arm needs no change: by the time that race is reachable (a forced close's own grace window - see `close.rs` below), `close_and_dispatch` has already drained this pane's waits itself, under the lock it used to remove the pane from the tree, so there is nothing left here to resolve a second time.

- `crates/server/src/runtime/close.rs`, `close_and_dispatch`: right after `emit_close_events(...)` and before `scope.into_iter()` builds `to_force` (still under the lock, with `scope: Vec<PaneId>` still owned - only borrowed here), for `&pane in &scope { wait::resolve_pane_waits_locked(&mut state, pane, Err(pane_closed_while_waiting())) }` - a new small helper `fn pane_closed_while_waiting() -> ApiError` (`ErrorCode::NoSuchTarget` - already in `pane.wait`'s own `errors:` list today, so this one needs **no** description change at all, unlike the timeout case: doc-2's own answer for "the thing you asked about is gone," and `pane.wait`'s `Returns::Record{exit_code}` has nothing honest to put there for a pane that was closed rather than exited - the same reasoning `ClosedPane`/`EndReason::Closed` already carry no exit code for). Covers every path through this function - `pane.close`, `tab.close`, `session.close`, forced or plain - since `scope` is already exactly "every pane this close just removed from the tree," computed once above regardless of method. AC #1's "a pane closed while being waited on answers at once" - genuinely at once here: no extra thread, the same lock acquisition the close already needed, answered long before `force_end` (which runs after this, unlocked) would ever end the program.
  - `Runtime::stop` (server stop; `Drop for Runtime`) calls `force_end` directly against every pane, never through `close_and_dispatch`, so it never itself touches `pending_waits`. This is sound, not a gap: `server.stop`'s own connection returns `false` from its `handle_request` arm, running its own `ConnectionCleanup::drop` (which calls `cancel_waits`) before `Stopper::stop()` is even allowed to end the accept loop, and `listen::run`'s own existing shutdown pass then closes and joins every *other* connection the same way, each running its own `ConnectionCleanup::drop` in turn - so by the time `Runtime::stop`'s `force_end` actually runs, no connection with a pending wait on any pane is still attached to tell. Stated here so a reviewer does not have to re-derive it.

- `crates/server/src/connection.rs`:
  - `ConnectionCleanup::drop`: one new line, `self.runtime.cancel_waits(self.client_id);`, placed after the existing `pane_streams` sweep loop and before `deregister_subscriber`/`disconnect_client` - the same ordering reasoning already documented there (this client's id must still unambiguously mean this connection when it is cancelled). AC #3 in full: no reply sent (the connection is gone), the `PendingWait` entry is removed synchronously inside this one call (no polling needed to observe it gone), and any timer thread that exists for it wakes via its own gate and exits within its own bounded wait, never past it.
  - `handle_request`: one new arm, `"pane.wait" => { runtime.register_wait(description, context, &resolved, crate::runtime::Waiting { client: client_id, id }, outbound); true }` - placed with the other four interceptions (`pane.attach`/`pane.detach`/`tab.declare-size`/`server.stop`), for the identical structural reason stated on all of them: answering it needs this connection's own `Arc<Outbound>` and (new) the server's pending-wait state, neither reachable from `dispatch`'s plain `(&mut Tree, ...)` signature. `register_wait` already enqueues its own reply or registers and returns nothing further to send - `true` unconditionally, the same tolerance `pane.attach`'s own arm already has for a reply that failed to enqueue because the connection is concurrently tearing down.

- `crates/server/src/dispatch/pane.rs`: remove `("pane", "wait")` from `HANDLED`, and delete the `"wait" => Err(not_implemented_yet(...))` arm from this module's own `dispatch` match (now unreachable through this module at all, the same way `"send"`/`"capture"` stopped being matched here once their own tasks landed).

- `crates/server/src/dispatch/mod.rs`:
  - `dispatch_raw`: one new literal arm, next to `("pane", "attach" | "detach")`: `("pane", "wait") => Err(not_implemented_yet("connection.rs's own interception (TASK-1.140) - ... neither reachable from this function's signature"))` - the same "fully built, never reached this way in the real system" shape every other interception already documents.
  - `#[cfg(test)] const INTERCEPTED_BEFORE_DISPATCH`: add `("pane", "wait")`. `handled()`'s own completeness test needs no other change - `pane::HANDLED` losing the entry and `INTERCEPTED_BEFORE_DISPATCH` gaining it keeps the two lists' union exactly equal to `description().methods` either way.

- `crates/server/tests/dispatch/cross_cutting.rs:90`: the `("pane.wait", ..., "TASK-1.140")` row's expected substring becomes `"connection.rs's own interception"` - the same wording `"server.stop"`'s own row already uses, since `pane.wait` joins it as "described, fully built, never reached through `dispatch` directly."

sdk changes (now settled, not conditional):
- `crates/sdk/src/description/shapes.rs`: add `ErrorCode::TimedOut` ("A wait reached its own timeout before the condition was met").
- `crates/sdk/src/description/methods/errors.rs`: its `ErrorRecord { code: ErrorCode::TimedOut, summary: ..., exit_status: 1 }` (doc-2 section 6's own wording for this exact case).
- **CLI exit-status mapping, checked against the tree, not assumed - the merge above brought in `crates/steg/src/cli/exit.rs` and `crates/steg/tests/cli_exit_statuses.rs` (TASK-1.119), neither of which is in the previous version of this plan:** `exit.rs`'s `api_error_status` looks `code` up in `api.errors` by equality and returns that record's own `exit_status` - it is a table lookup, not a `match` over `ErrorCode`, so it needs no new arm for `TimedOut` and cannot be broken by adding one (`wildcard_enum_match_arm` does not apply - there is no match on the enum here). `cli_exit_statuses.rs`'s own `every_described_error_maps_to_its_own_exit_status` iterates `api.errors` the same generic way. **Neither file changes for this task** - named here so a reviewer checking the plan's file list against the real diff does not read their absence as an oversight.
- `crates/sdk/src/description/methods/pane.rs`: add `ErrorCode::TimedOut` to `pane.wait`'s own `errors:` list (`BadRequest` is already there, covering the negative-timeout refusal - no change needed for that one).
- `crates/sdk/src/description/methods/tests.rs`: `errors_has_thirteen_codes_with_their_exit_statuses` becomes fourteen (rename the fn), with one more `assert_eq!(status_of(ErrorCode::TimedOut), 1)`.
- `crates/sdk/src/record.rs`: new `pub struct WaitResult { pub exit_code: i32 }`, derived the same way every sibling record in that file is (`#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]`), added to the module's own enumerated doc comment (it already narrates each record's own arrival in order - `SessionRecord`, `SettingRecord`, `TabRecord`, `ClientRecord`, ... `WaitResult` (TASK-1.140) joins that list) - **not** `screen.rs` (that module's own doc scopes it to "the shapes a client draws a terminal's screen from," a wrong fit for a one-shot reply record) and **not** inline JSON anywhere a success reply is built.

OPEN QUESTIONS FOR THE ENGINEER

None blocking. The one informational note carried over: `WaitGate`'s simplification from the stale plan's `Option<T>`-carrying gate to a bare wake signal (above) - named so a reviewer comparing against TASK-1.90's old plan doesn't go looking for a dropped payload.

PROOF PER ACCEPTANCE CRITERION

- AC #1/#4: `crates/server/tests/pane_wait.rs` (new, real sockets, the same `support::{ScopedTempDir, scoped_socket_path}` + `start_server`/`stop_server` harness `pane_capture.rs`/`pane_send.rs` already use, real explicit pty programs, never `$SHELL`/`$HOME`, every wait bounded):
  - a wait on an already-ended pane (waited into that state via a real exit, never a sleep), with no `--timeout` given, answers at once with the real exit code.
  - a wait on a running pane answers once a real program really exits, with its real code.
  - a wait with a positive `--timeout` against a program that outlives it answers `TimedOut` within the bound, and the error's own `message` names the given seconds (e.g. `message.contains(&secs.to_string())`). Negative control: with `timeout_wait_locked`'s `format!` reverted to a plain, non-interpolated string, that assertion fails - recorded failing then passing.
  - `--timeout 0` against an already-ended pane answers `Ok` with the real exit code, exactly as with no timeout given - proving the "check now" branch does not disturb the ordinary already-ended path.
  - `--timeout 0` against a still-running pane answers `TimedOut` at once (bounded well under any real wait, e.g. well under 200ms, with no pty-exit or close event involved), with nothing registered. Negative control: with the `Some(0)` immediate-reply branch removed (falling through to "register indefinitely"), this case instead never answers until the test's own cleanup ends the program - recorded failing (hangs past a short bound) then passing.
  - a negative `--timeout` (e.g. `-1`) against a still-running pane answers `BadRequest` at once, and the pane is left exactly as before (checked via a later, ordinary `pane.wait` with no timeout against the same pane still working normally once it really exits). Negative control: with the negative check removed, `secs as u64` wraps a negative `i64` to a huge duration instead of refusing - this case instead hangs past the test's own bound rather than failing fast - recorded failing then passing.
  - closing the pane (or its tab/session, with `--force`) while a wait is pending answers `NoSuchTarget`, bounded well under `force_end`'s own grace window - proving the close-path resolution fires immediately under the lock, not only once the program is actually made to die.
  - the already-ended-pane success case (with or without `--timeout 0`) deserialises its reply into `steg_sdk::record::WaitResult`, not read as a bare `Value["exit_code"]`, proving the server actually serialises the typed record the engineer asked for, not a same-shaped coincidence.
  - negative controls carried over from the previous version: (exit) with `resolve_pane_waits_locked`'s call removed from the `Exited` arm, the plain-exit case hangs past a generous bound - recorded failing then passing; (close) with the same call removed from `close_and_dispatch`, the close-while-waiting case instead blocks for the close's own grace window - recorded failing then passing; (timer wake) with `gate.resolve()` removed from `resolve_pane_waits_locked`'s own loop (only meaningful if a timer thread for the same wait is concurrently sleeping - noted explicitly if the test as built never actually exercises that overlap).
- AC #2: same file - a `pane.wait` sent with no timeout against a running pane, then, on the very same connection, an unrelated request (e.g. `pane.list`) sent right behind it without waiting for the first reply: the second reply must arrive (bounded) while the first is still outstanding, proving `register_wait` released the lock and the connection's own reader never blocked on the pending wait. Negative control: temporarily holding `state`'s lock across a `thread::sleep` inside `register_wait` (scratch edit, reverted) stalls the second request past the bound - recorded, not committed. Unaffected by the three answers (uses no timeout at all).
- AC #3: a same-crate unit test (`crates/server/src/connection.rs`'s own `#[cfg(test)] mod tests`, the identical shape `connection_cleanups_drop_sweeps_every_attachment_it_still_holds` already uses for attachments): register a pending wait directly through `runtime.register_wait` against a real, still-running pane, with a short positive timeout so a timer thread genuinely exists; build a `ConnectionCleanup` around it and drop it; assert `runtime.pending_wait_count_for_test(pane) == 0` at once (synchronous - `cancel_waits` removes it under the same call, no polling needed) and that a *second*, fresh `pane.wait` against the same still-running pane (through `runtime.register_wait` again, a second connection's worth of identifying values) still registers and later resolves correctly once the pane really exits - proving the cancelled entry did not leave anything that would wedge a later wait, not merely that nothing crashed. Negative control: with the new `self.runtime.cancel_waits(self.client_id)` line removed from `ConnectionCleanup::drop`, `pending_wait_count_for_test(pane)` stays at 1 after the drop instead of reaching 0 - recorded failing then passing. Unaffected by the three answers (uses a positive timeout, the one branch that still registers and spawns a timer).

RISK

The only genuinely new, cross-cutting state in the server is `pending_waits` itself - no existing precedent beyond `RunGate`'s own narrower, one-gate-per-run shape. Mitigated by keeping every resolution site a plain, already-lock-holding function call (no second lock, no new thread except the one, bounded, early-wakeable timer per positive-timeout wait) and by the same real-pty/real-socket proof style this crate's own `attach.rs`/`pane_send.rs`/`pane_capture.rs` already established. The sdk change (`ErrorCode::TimedOut`, `WaitResult`) is the one part of this task that is really an API-description change wearing a server task's name, same as TASK-1.90 flagged for `ProgramNotRunning` - now agreed by the engineer, so no longer a blocking risk, but still the file set a reviewer should check most carefully, since every later task that adds a method or a record touches these same files.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Plan checked against doc-2 (sections 5, 6, 8) by @bruk.o on 2026-11-20: ok. It builds only pane.wait; exit 0 with the program's own code in the result and status 1 on timeout match section 6; a pane closed mid-wait answers NoSuchTarget, already in the method's errors; the wait is woken by the exit, the close or the disconnect, with one early-wakeable timer per timed wait and no polling. Three points wait on the engineer: a new ErrorCode::TimedOut (status 1) in the API description; a zero or negative --timeout; inline JSON or a typed result. Orchestrator's recommendations: add TimedOut; zero is an immediate timeout and a negative value is BadRequest (status 2) rather than clamped; typed struct.

Revised plan (83254935a0f3) checked against doc-2 (sections 5, 6) and the engineer's answers of 2026-11-21 by @bruk.o: ok. TimedOut is a new error code with exit status 1; --timeout 0 answers at once without registering a wait; a negative --timeout is BadRequest (status 2) before the target is resolved; the reply is the typed record WaitResult. One difference from the wording put to the engineer: the expired seconds are in the error's message, not a separate details field, because errors carry none; told to the engineer with the plan. doc-2 sections 5 and 6 do not yet state the 0 and negative cases; a document change for the engineer to agree.

Check FAILED: `just check` exited 101 at 2026-11-21T20:27:31+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.140 on task-1.140-server-wait-for-a-pane-s-program-to-exit at ccf5dfb2b3b9, last 40 of 81 lines:

```text
   --> crates/server/src/runtime/wait.rs:229:19
    |
219 | impl Runtime {
    | ------------ methods in this implementation
...
229 |     pub(crate) fn register_wait(
    |                   ^^^^^^^^^^^^^
...
327 |     pub(crate) fn cancel_waits(&self, client_id: ClientId) {
    |                   ^^^^^^^^^^^^
...
349 |     pub(crate) fn pending_wait_count_for_test(&self, pane: PaneId) -> usize {
    |                   ^^^^^^^^^^^^^^^^^^^^^^^^^^^

error: type `PendingWait` is private
   --> crates/server/src/runtime/mod.rs:107:5
    |
107 |     pending_waits: wait::PendingWaits,
    |     ^^^^^^^^^^^^^ private type

error: type `PendingWait` is private
   --> crates/server/src/runtime/mod.rs:209:28
    |
209 |             pending_waits: HashMap::new(),
    |                            ^^^^^^^^^^^^^^ private type

warning: `steg-server` (lib) generated 9 warnings
error: could not compile `steg-server` (lib) due to 2 previous errors; 9 warnings emitted
warning: build failed, waiting for other jobs to finish...
error[E0063]: missing field `pending_waits` in initializer of `State`
  --> crates/server/src/runtime/testing.rs:66:5
   |
66 |     State {
   |     ^^^^^ missing `pending_waits`

For more information about this error, try `rustc --explain E0063`.
warning: `steg-server` (lib test) generated 1 warning (1 duplicate)
error: could not compile `steg-server` (lib test) due to 1 previous error; 1 warning emitted
full output: .quiet/test-steg-server-accept-backoff-fixture.log
error: recipe `check-packages` failed with exit code 101
```

Check FAILED: `env ZIG=/Users/brukhabtu/.local/share/mise/installs/zig/0.17.1/bin/zig cargo test -p steg-server --lib connection::tests::connection_cleanups_drop_cancels_every_pending_wait_it_still_holds` exited 101 at 2026-11-21T20:33:25+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.140 on task-1.140-server-wait-for-a-pane-s-program-to-exit at ccf5dfb2b3b9 with uncommitted changes:

```text
   Compiling steg-server v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.140/crates/server)
warning: method `cancel_waits` is never used
   --> crates/server/src/runtime/wait.rs:331:19
    |
223 | impl Runtime {
    | ------------ method in this implementation
...
331 |     pub(crate) fn cancel_waits(&self, client_id: ClientId) {
    |                   ^^^^^^^^^^^^
    |
    = note: `#[warn(dead_code)]` (part of `#[warn(unused)]`) on by default

warning: `steg-server` (lib test) generated 1 warning
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.83s
     Running unittests src/lib.rs (target/debug/deps/steg_server-6d2f1d51fdd2e018)

running 1 test
test connection::tests::connection_cleanups_drop_cancels_every_pending_wait_it_still_holds ... FAILED

failures:

---- connection::tests::connection_cleanups_drop_cancels_every_pending_wait_it_still_holds stdout ----

thread 'connection::tests::connection_cleanups_drop_cancels_every_pending_wait_it_still_holds' (71648184) panicked at crates/server/src/connection.rs:1666:9:
assertion `left == right` failed: ConnectionCleanup::drop must cancel every pending wait it still holds
  left: 1
 right: 0
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    connection::tests::connection_cleanups_drop_cancels_every_pending_wait_it_still_holds

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 222 filtered out; finished in 0.01s

error: test failed, to rerun pass `-p steg-server --lib`
```

Check passed: `env ZIG=/Users/brukhabtu/.local/share/mise/installs/zig/0.17.1/bin/zig cargo test -p steg-server --lib connection::tests::connection_cleanups_drop_cancels_every_pending_wait_it_still_holds` exited 0 at 2026-11-21T20:33:39+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.140 on task-1.140-server-wait-for-a-pane-s-program-to-exit at ccf5dfb2b3b9 with uncommitted changes:

```text
   Compiling steg-server v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.140/crates/server)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.94s
     Running unittests src/lib.rs (target/debug/deps/steg_server-6d2f1d51fdd2e018)

running 1 test
test connection::tests::connection_cleanups_drop_cancels_every_pending_wait_it_still_holds ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 222 filtered out; finished in 0.01s
```

Check FAILED: `env ZIG=/Users/brukhabtu/.local/share/mise/installs/zig/0.17.1/bin/zig cargo test -p steg-server --test pane_wait wait_with_a_positive_timeout_on_an_outliving_program_answers_timed_out_naming_the_seconds` exited 101 at 2026-11-21T20:37:46+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.140 on task-1.140-server-wait-for-a-pane-s-program-to-exit at ccf5dfb2b3b9 with uncommitted changes, last 40 of 41 lines:

```text
   --> crates/server/src/runtime/wait.rs:128:14
    |
128 | fn timed_out(timeout_secs: i64) -> ApiError {
    |              ^^^^^^^^^^^^ help: if this is intentional, prefix it with an underscore: `_timeout_secs`
    |
    = note: `#[warn(unused_variables)]` (part of `#[warn(unused)]`) on by default

warning: method `pending_wait_count_for_test` is never used
   --> crates/server/src/runtime/wait.rs:353:19
    |
223 | impl Runtime {
    | ------------ method in this implementation
...
353 |     pub(crate) fn pending_wait_count_for_test(&self, pane: PaneId) -> usize {
    |                   ^^^^^^^^^^^^^^^^^^^^^^^^^^^
    |
    = note: `#[warn(dead_code)]` (part of `#[warn(unused)]`) on by default

warning: `steg-server` (lib) generated 2 warnings (run `cargo fix --lib -p steg-server` to apply 1 suggestion)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.09s
     Running tests/pane_wait.rs (target/debug/deps/pane_wait-0e1fe91e2aa055cd)

running 1 test
test wait_with_a_positive_timeout_on_an_outliving_program_answers_timed_out_naming_the_seconds ... FAILED

failures:

---- wait_with_a_positive_timeout_on_an_outliving_program_answers_timed_out_naming_the_seconds stdout ----

thread 'wait_with_a_positive_timeout_on_an_outliving_program_answers_timed_out_naming_the_seconds' (72011549) panicked at crates/server/tests/pane_wait.rs:360:5:
the message should name the expired timeout (1 second): "the wait's own timeout passed before the condition was met"
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    wait_with_a_positive_timeout_on_an_outliving_program_answers_timed_out_naming_the_seconds

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 15 filtered out; finished in 1.02s

error: test failed, to rerun pass `-p steg-server --test pane_wait`
```

Check passed: `env ZIG=/Users/brukhabtu/.local/share/mise/installs/zig/0.17.1/bin/zig cargo test -p steg-server --test pane_wait wait_with_a_positive_timeout_on_an_outliving_program_answers_timed_out_naming_the_seconds` exited 0 at 2026-11-21T20:38:01+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.140 on task-1.140-server-wait-for-a-pane-s-program-to-exit at ccf5dfb2b3b9 with uncommitted changes:

```text
   Compiling steg-server v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.140/crates/server)
warning: method `pending_wait_count_for_test` is never used
   --> crates/server/src/runtime/wait.rs:353:19
    |
223 | impl Runtime {
    | ------------ method in this implementation
...
353 |     pub(crate) fn pending_wait_count_for_test(&self, pane: PaneId) -> usize {
    |                   ^^^^^^^^^^^^^^^^^^^^^^^^^^^
    |
    = note: `#[warn(dead_code)]` (part of `#[warn(unused)]`) on by default

warning: `steg-server` (lib) generated 1 warning
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.74s
     Running tests/pane_wait.rs (target/debug/deps/pane_wait-0e1fe91e2aa055cd)

running 1 test
test wait_with_a_positive_timeout_on_an_outliving_program_answers_timed_out_naming_the_seconds ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 15 filtered out; finished in 1.02s
```

Check FAILED: `env ZIG=/Users/brukhabtu/.local/share/mise/installs/zig/0.17.1/bin/zig cargo test -p steg-server --test pane_wait timeout_zero_on_a_running_pane_answers_timed_out_at_once` exited 101 at 2026-11-21T20:39:00+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.140 on task-1.140-server-wait-for-a-pane-s-program-to-exit at ccf5dfb2b3b9 with uncommitted changes:

```text
warning: method `pending_wait_count_for_test` is never used
   --> crates/server/src/runtime/wait.rs:359:19
    |
223 | impl Runtime {
    | ------------ method in this implementation
...
359 |     pub(crate) fn pending_wait_count_for_test(&self, pane: PaneId) -> usize {
    |                   ^^^^^^^^^^^^^^^^^^^^^^^^^^^
    |
    = note: `#[warn(dead_code)]` (part of `#[warn(unused)]`) on by default

warning: `steg-server` (lib) generated 1 warning
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.11s
     Running tests/pane_wait.rs (target/debug/deps/pane_wait-0e1fe91e2aa055cd)

running 1 test
test timeout_zero_on_a_running_pane_answers_timed_out_at_once ... FAILED

failures:

---- timeout_zero_on_a_running_pane_answers_timed_out_at_once stdout ----

thread 'timeout_zero_on_a_running_pane_answers_timed_out_at_once' (72139961) panicked at crates/server/tests/pane_wait.rs:149:39:
reading the reply: Os { code: 35, kind: WouldBlock, message: "Resource temporarily unavailable" }
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    timeout_zero_on_a_running_pane_answers_timed_out_at_once

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 15 filtered out; finished in 15.01s

error: test failed, to rerun pass `-p steg-server --test pane_wait`
```

Check passed: `env ZIG=/Users/brukhabtu/.local/share/mise/installs/zig/0.17.1/bin/zig cargo test -p steg-server --test pane_wait timeout_zero_on_a_running_pane_answers_timed_out_at_once` exited 0 at 2026-11-21T20:39:14+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.140 on task-1.140-server-wait-for-a-pane-s-program-to-exit at ccf5dfb2b3b9 with uncommitted changes:

```text
   Compiling steg-server v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.140/crates/server)
warning: method `pending_wait_count_for_test` is never used
   --> crates/server/src/runtime/wait.rs:353:19
    |
223 | impl Runtime {
    | ------------ method in this implementation
...
353 |     pub(crate) fn pending_wait_count_for_test(&self, pane: PaneId) -> usize {
    |                   ^^^^^^^^^^^^^^^^^^^^^^^^^^^
    |
    = note: `#[warn(dead_code)]` (part of `#[warn(unused)]`) on by default

warning: `steg-server` (lib) generated 1 warning
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.84s
     Running tests/pane_wait.rs (target/debug/deps/pane_wait-0e1fe91e2aa055cd)

running 1 test
test timeout_zero_on_a_running_pane_answers_timed_out_at_once ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 15 filtered out; finished in 0.02s
```

Check FAILED: `env ZIG=/Users/brukhabtu/.local/share/mise/installs/zig/0.17.1/bin/zig cargo test -p steg-server --test pane_wait a_negative_timeout_on_a_running_pane_answers_bad_request_and_leaves_the_pane_untouched` exited 101 at 2026-11-21T20:39:43+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.140 on task-1.140-server-wait-for-a-pane-s-program-to-exit at ccf5dfb2b3b9 with uncommitted changes:

```text
warning: method `pending_wait_count_for_test` is never used
   --> crates/server/src/runtime/wait.rs:354:19
    |
223 | impl Runtime {
    | ------------ method in this implementation
...
354 |     pub(crate) fn pending_wait_count_for_test(&self, pane: PaneId) -> usize {
    |                   ^^^^^^^^^^^^^^^^^^^^^^^^^^^
    |
    = note: `#[warn(dead_code)]` (part of `#[warn(unused)]`) on by default

warning: `steg-server` (lib) generated 1 warning
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.08s
     Running tests/pane_wait.rs (target/debug/deps/pane_wait-0e1fe91e2aa055cd)

running 1 test
test a_negative_timeout_on_a_running_pane_answers_bad_request_and_leaves_the_pane_untouched ... FAILED

failures:

---- a_negative_timeout_on_a_running_pane_answers_bad_request_and_leaves_the_pane_untouched stdout ----

thread 'a_negative_timeout_on_a_running_pane_answers_bad_request_and_leaves_the_pane_untouched' (72222217) panicked at crates/server/tests/pane_wait.rs:462:9:
expected BadRequest, got Ok(Object {"exit_code": Number(2)})
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    a_negative_timeout_on_a_running_pane_answers_bad_request_and_leaves_the_pane_untouched

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 15 filtered out; finished in 0.33s

error: test failed, to rerun pass `-p steg-server --test pane_wait`
```

Check passed: `env ZIG=/Users/brukhabtu/.local/share/mise/installs/zig/0.17.1/bin/zig cargo test -p steg-server --test pane_wait a_negative_timeout_on_a_running_pane_answers_bad_request_and_leaves_the_pane_untouched` exited 0 at 2026-11-21T20:40:05+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.140 on task-1.140-server-wait-for-a-pane-s-program-to-exit at ccf5dfb2b3b9 with uncommitted changes:

```text
   Compiling steg-server v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.140/crates/server)
warning: method `pending_wait_count_for_test` is never used
   --> crates/server/src/runtime/wait.rs:353:19
    |
223 | impl Runtime {
    | ------------ method in this implementation
...
353 |     pub(crate) fn pending_wait_count_for_test(&self, pane: PaneId) -> usize {
    |                   ^^^^^^^^^^^^^^^^^^^^^^^^^^^
    |
    = note: `#[warn(dead_code)]` (part of `#[warn(unused)]`) on by default

warning: `steg-server` (lib) generated 1 warning
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.85s
     Running tests/pane_wait.rs (target/debug/deps/pane_wait-0e1fe91e2aa055cd)

running 1 test
test a_negative_timeout_on_a_running_pane_answers_bad_request_and_leaves_the_pane_untouched ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 15 filtered out; finished in 0.32s
```

Check FAILED: `env ZIG=/Users/brukhabtu/.local/share/mise/installs/zig/0.17.1/bin/zig cargo test -p steg-server --test pane_wait closing_the_pane_while_a_wait_is_pending_answers_no_such_target_bounded_under_the_close_grace` exited 101 at 2026-11-21T20:42:19+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.140 on task-1.140-server-wait-for-a-pane-s-program-to-exit at ccf5dfb2b3b9 with uncommitted changes:

```text
warning: function `pane_closed_while_waiting` is never used
   --> crates/server/src/runtime/wait.rs:180:15
    |
180 | pub(super) fn pane_closed_while_waiting() -> ApiError {
    |               ^^^^^^^^^^^^^^^^^^^^^^^^^
    |
    = note: `#[warn(dead_code)]` (part of `#[warn(unused)]`) on by default

warning: method `pending_wait_count_for_test` is never used
   --> crates/server/src/runtime/wait.rs:353:19
    |
223 | impl Runtime {
    | ------------ method in this implementation
...
353 |     pub(crate) fn pending_wait_count_for_test(&self, pane: PaneId) -> usize {
    |                   ^^^^^^^^^^^^^^^^^^^^^^^^^^^

warning: `steg-server` (lib) generated 2 warnings
   Compiling steg-server v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.140/crates/server)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.38s
     Running tests/pane_wait.rs (target/debug/deps/pane_wait-0e1fe91e2aa055cd)

running 1 test
test closing_the_pane_while_a_wait_is_pending_answers_no_such_target_bounded_under_the_close_grace ... FAILED

failures:

---- closing_the_pane_while_a_wait_is_pending_answers_no_such_target_bounded_under_the_close_grace stdout ----

thread 'closing_the_pane_while_a_wait_is_pending_answers_no_such_target_bounded_under_the_close_grace' (72298956) panicked at crates/server/tests/pane_wait.rs:149:39:
reading the reply: Os { code: 35, kind: WouldBlock, message: "Resource temporarily unavailable" }
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    closing_the_pane_while_a_wait_is_pending_answers_no_such_target_bounded_under_the_close_grace

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 15 filtered out; finished in 15.02s

error: test failed, to rerun pass `-p steg-server --test pane_wait`
```

Check passed: `env ZIG=/Users/brukhabtu/.local/share/mise/installs/zig/0.17.1/bin/zig cargo test -p steg-server --test pane_wait closing_the_pane_while_a_wait_is_pending_answers_no_such_target_bounded_under_the_close_grace` exited 0 at 2026-11-21T20:42:32+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.140 on task-1.140-server-wait-for-a-pane-s-program-to-exit at ccf5dfb2b3b9 with uncommitted changes:

```text
   Compiling steg-server v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.140/crates/server)
warning: method `pending_wait_count_for_test` is never used
   --> crates/server/src/runtime/wait.rs:353:19
    |
223 | impl Runtime {
    | ------------ method in this implementation
...
353 |     pub(crate) fn pending_wait_count_for_test(&self, pane: PaneId) -> usize {
    |                   ^^^^^^^^^^^^^^^^^^^^^^^^^^^
    |
    = note: `#[warn(dead_code)]` (part of `#[warn(unused)]`) on by default

warning: `steg-server` (lib) generated 1 warning
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.72s
     Running tests/pane_wait.rs (target/debug/deps/pane_wait-0e1fe91e2aa055cd)

running 1 test
test closing_the_pane_while_a_wait_is_pending_answers_no_such_target_bounded_under_the_close_grace ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 15 filtered out; finished in 0.06s
```

Check FAILED: `env ZIG=/Users/brukhabtu/.local/share/mise/installs/zig/0.17.1/bin/zig cargo test -p steg-server --test pane_wait wait_on_a_running_pane_answers_once_the_program_really_exits_with_its_real_code` exited 101 at 2026-11-21T20:43:22+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.140 on task-1.140-server-wait-for-a-pane-s-program-to-exit at ccf5dfb2b3b9 with uncommitted changes:

```text
warning: method `pending_wait_count_for_test` is never used
   --> crates/server/src/runtime/wait.rs:353:19
    |
223 | impl Runtime {
    | ------------ method in this implementation
...
353 |     pub(crate) fn pending_wait_count_for_test(&self, pane: PaneId) -> usize {
    |                   ^^^^^^^^^^^^^^^^^^^^^^^^^^^
    |
    = note: `#[warn(dead_code)]` (part of `#[warn(unused)]`) on by default

warning: `steg-server` (lib) generated 1 warning
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.09s
     Running tests/pane_wait.rs (target/debug/deps/pane_wait-0e1fe91e2aa055cd)

running 1 test
test wait_on_a_running_pane_answers_once_the_program_really_exits_with_its_real_code ... FAILED

failures:

---- wait_on_a_running_pane_answers_once_the_program_really_exits_with_its_real_code stdout ----

thread 'wait_on_a_running_pane_answers_once_the_program_really_exits_with_its_real_code' (72341338) panicked at crates/server/tests/pane_wait.rs:149:39:
reading the reply: Os { code: 35, kind: WouldBlock, message: "Resource temporarily unavailable" }
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    wait_on_a_running_pane_answers_once_the_program_really_exits_with_its_real_code

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 15 filtered out; finished in 15.01s

error: test failed, to rerun pass `-p steg-server --test pane_wait`
```

Check passed: `env ZIG=/Users/brukhabtu/.local/share/mise/installs/zig/0.17.1/bin/zig cargo test -p steg-server --test pane_wait wait_on_a_running_pane_answers_once_the_program_really_exits_with_its_real_code` exited 0 at 2026-11-21T20:43:52+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.140 on task-1.140-server-wait-for-a-pane-s-program-to-exit at ccf5dfb2b3b9 with uncommitted changes:

```text
   Compiling steg-server v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.140/crates/server)
warning: method `pending_wait_count_for_test` is never used
   --> crates/server/src/runtime/wait.rs:353:19
    |
223 | impl Runtime {
    | ------------ method in this implementation
...
353 |     pub(crate) fn pending_wait_count_for_test(&self, pane: PaneId) -> usize {
    |                   ^^^^^^^^^^^^^^^^^^^^^^^^^^^
    |
    = note: `#[warn(dead_code)]` (part of `#[warn(unused)]`) on by default

warning: `steg-server` (lib) generated 1 warning
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.86s
     Running tests/pane_wait.rs (target/debug/deps/pane_wait-0e1fe91e2aa055cd)

running 1 test
test wait_on_a_running_pane_answers_once_the_program_really_exits_with_its_real_code ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 15 filtered out; finished in 0.32s
```

Check FAILED: `env ZIG=/Users/brukhabtu/.local/share/mise/installs/zig/0.17.1/bin/zig cargo test -p steg-server --test pane_wait an_unrelated_request_behind_a_pending_wait_answers_first_proving_the_reader_was_never_blocked` exited 101 at 2026-11-21T20:44:48+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.140 on task-1.140-server-wait-for-a-pane-s-program-to-exit at ccf5dfb2b3b9 with uncommitted changes:

```text
warning: method `pending_wait_count_for_test` is never used
   --> crates/server/src/runtime/wait.rs:357:19
    |
223 | impl Runtime {
    | ------------ method in this implementation
...
357 |     pub(crate) fn pending_wait_count_for_test(&self, pane: PaneId) -> usize {
    |                   ^^^^^^^^^^^^^^^^^^^^^^^^^^^
    |
    = note: `#[warn(dead_code)]` (part of `#[warn(unused)]`) on by default

warning: `steg-server` (lib) generated 1 warning
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.08s
     Running tests/pane_wait.rs (target/debug/deps/pane_wait-0e1fe91e2aa055cd)

running 1 test
test an_unrelated_request_behind_a_pending_wait_answers_first_proving_the_reader_was_never_blocked ... FAILED

failures:

---- an_unrelated_request_behind_a_pending_wait_answers_first_proving_the_reader_was_never_blocked stdout ----

thread 'an_unrelated_request_behind_a_pending_wait_answers_first_proving_the_reader_was_never_blocked' (72397384) panicked at crates/server/tests/pane_wait.rs:587:47:
reading a reply: Os { code: 35, kind: WouldBlock, message: "Resource temporarily unavailable" }
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    an_unrelated_request_behind_a_pending_wait_answers_first_proving_the_reader_was_never_blocked

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 15 filtered out; finished in 15.01s

error: test failed, to rerun pass `-p steg-server --test pane_wait`
```

Check passed: `env ZIG=/Users/brukhabtu/.local/share/mise/installs/zig/0.17.1/bin/zig cargo test -p steg-server --test pane_wait an_unrelated_request_behind_a_pending_wait_answers_first_proving_the_reader_was_never_blocked` exited 0 at 2026-11-21T20:45:15+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.140 on task-1.140-server-wait-for-a-pane-s-program-to-exit at ccf5dfb2b3b9 with uncommitted changes:

```text
   Compiling steg-server v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.140/crates/server)
warning: method `pending_wait_count_for_test` is never used
   --> crates/server/src/runtime/wait.rs:353:19
    |
223 | impl Runtime {
    | ------------ method in this implementation
...
353 |     pub(crate) fn pending_wait_count_for_test(&self, pane: PaneId) -> usize {
    |                   ^^^^^^^^^^^^^^^^^^^^^^^^^^^
    |
    = note: `#[warn(dead_code)]` (part of `#[warn(unused)]`) on by default

warning: `steg-server` (lib) generated 1 warning
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.79s
     Running tests/pane_wait.rs (target/debug/deps/pane_wait-0e1fe91e2aa055cd)

running 1 test
test an_unrelated_request_behind_a_pending_wait_answers_first_proving_the_reader_was_never_blocked ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 15 filtered out; finished in 0.02s
```

Check FAILED: `just check` exited 1 at 2026-11-21T20:50:51+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.140 on task-1.140-server-wait-for-a-pane-s-program-to-exit at ccf5dfb2b3b9 with uncommitted changes, last 40 of 64 lines:

```text
(B[m 
     let start = Instant::now();
[31m-    let response = read_reply_for(&mut wait_stream, &mut wait_connection, wait_id, Duration::from_secs(5));
(B[m[32m+    let response = read_reply_for(
(B[m[32m+        &mut wait_stream,
(B[m[32m+        &mut wait_connection,
(B[m[32m+        wait_id,
(B[m[32m+        Duration::from_secs(5),
(B[m[32m+    );
(B[m     let elapsed = start.elapsed();
     assert!(
         elapsed < Duration::from_secs(1),
Diff in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.140/crates/server/tests/pane_wait.rs:565:
     let (mut stream, mut connection) = connect_and_handshake(&socket_path);
     let pane = new_pane_running(&mut stream, &mut connection, vec!["sh", "-c", "sleep 100"]);
 
[31m-    let wait_id = call_without_reading(&mut stream, &mut connection, "pane.wait", wait_params(&pane, None));
(B[m[32m+    let wait_id = call_without_reading(
(B[m[32m+        &mut stream,
(B[m[32m+        &mut connection,
(B[m[32m+        "pane.wait",
(B[m[32m+        wait_params(&pane, None),
(B[m[32m+    );
(B[m     let mut show_params = Map::new();
     show_params.insert("target".to_string(), json!(pane));
     let show_id = call_without_reading(&mut stream, &mut connection, "pane.show", show_params);
Diff in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.140/crates/server/tests/pane_wait.rs:596:
         "the pending pane.wait must not answer before the unrelated pane.show sent right \
          behind it"
     );
[31m-    assert!(matches!(show_response, Response::Ok(_)), "{show_response:?}");
(B[m[32m+    assert!(
(B[m[32m+        matches!(show_response, Response::Ok(_)),
(B[m[32m+        "{show_response:?}"
(B[m[32m+    );
(B[m 
     stop_server(&stop, handle);
 }
full output: .quiet/format-check.log
error: recipe `check-fmt` failed on line 163 with exit code 1
```

Check FAILED: `just check` exited 101 at 2026-11-21T20:52:08+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.140 on task-1.140-server-wait-for-a-pane-s-program-to-exit at ccf5dfb2b3b9 with uncommitted changes, last 40 of 71 lines:

```text
test cli::exit::tests::every_described_error_maps_to_its_own_exit_status ... ok
test cli::output::tests::nested_plain_and_json_are_identical_bytes ... FAILED
test cli::output::tests::record_plain_is_one_tab_separated_line_in_fields_order ... ok
test cli::output::tests::a_null_field_renders_as_an_empty_column_never_a_dropped_one ... ok
test cli::output::tests::records_plain_is_one_line_per_record_nothing_for_zero ... ok
test cli::output::tests::a_tab_newline_and_carriage_return_inside_a_field_become_single_spaces_in_plain_text ... ok
test cli::output::tests::a_field_declared_text_but_really_structured_renders_its_own_compact_json ... FAILED
test cli::tree::tests::autostart_defaults_to_true_with_neither_flag_given ... ok
test cli::tree::tests::autostart_given_after_no_autostart_wins ... ok
test cli::tree::tests::json_is_read_from_the_leaf_subcommand ... ok
test cli::tree::tests::no_autostart_is_read_whether_given_before_or_after_the_subcommand ... ok
test cli::tree::tests::quiet_is_read_from_the_leaf_subcommand_either_spelling ... ok

failures:

---- cli::output::tests::nested_plain_and_json_are_identical_bytes stdout ----

thread 'cli::output::tests::nested_plain_and_json_are_identical_bytes' (72975788) panicked at crates/steg/src/cli/output.rs:708:9:
assertion `left == right` failed
  left: "{\"stream\":\"st1\",\"snapshot\":{\"rows\":[]}}\n"
 right: "{\"snapshot\":{\"rows\":[]},\"stream\":\"st1\"}\n"
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace

---- cli::output::tests::a_field_declared_text_but_really_structured_renders_its_own_compact_json stdout ----

thread 'cli::output::tests::a_field_declared_text_but_really_structured_renders_its_own_compact_json' (72975780) panicked at crates/steg/src/cli/output.rs:593:9:
assertion `left == right` failed
  left: "t1\t{\"split\":\"horizontal\",\"panes\":[\"p1\",\"p2\"]}\n"
 right: "t1\t{\"panes\":[\"p1\",\"p2\"],\"split\":\"horizontal\"}\n"


failures:
    cli::output::tests::a_field_declared_text_but_really_structured_renders_its_own_compact_json
    cli::output::tests::nested_plain_and_json_are_identical_bytes

test result: FAILED. 35 passed; 2 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

error: test failed, to rerun pass `-p steg --lib`
full output: .quiet/tests-p-steg-p-steg-app-p-steg-kernel-p-steg-sdk-p-steg-server.log
error: recipe `test` failed with exit code 101
```

Built and committed locally (not pushed): sdk ErrorCode::TimedOut + WaitResult (crates/sdk/src/description/{shapes.rs,methods/errors.rs,methods/pane.rs,methods/tests.rs}, crates/sdk/src/record.rs, crates/sdk/src/lib.rs); new crates/server/src/runtime/wait.rs (WaitGate, PendingWait, register_wait, cancel_waits, pending_wait_count_for_test); hooks in runtime/events.rs (exit) and runtime/close.rs (close-while-waiting); connection.rs's pane.wait interception and ConnectionCleanup::drop's cancel_waits call; pane.wait moved out of dispatch/pane.rs's HANDLED into dispatch/mod.rs's own not-yet arm; cross_cutting.rs's row updated; new crates/server/tests/pane_wait.rs (9 tests) plus one same-crate unit test in connection.rs for AC #3.

Departures from the plan, each small: runtime/testing.rs needed pending_waits added to its own State literal (not named in the plan, a necessary consequence of the new field); lib.rs needed WaitResult re-exported alongside the other record types (same reason); pending_wait_count_for_test ended up #[cfg(test)] (matching attachment_count_for_test's real precedent exactly - the plan's own text describing it as 'not #[cfg(test)]' was a misreading of that precedent, corrected in the doc comment); crates/steg/src/cli/exit.rs's own doc comment got a one-line rename (the test name it cites changed from thirteen to fourteen) - no logic or test changed in that file, matching the plan's own claim that it needs no change in substance.

Every production-code negative control recorded as a failing+passing pair at this commit: the Some(0) branch, the negative-timeout check, the TimedOut message's own seconds, the close-path drain, the exit-path drain (events.rs), and a scratch lock-held sleep in register_wait for AC #2 - plus AC #3's cancel_waits control (recorded before this commit, same mechanism).

just check at this commit: every step passes (format, lint, boundaries, foundations, design pages, licences) except the tests step, which fails on exactly the two tests named in my brief (cli::output::tests::nested_plain_and_json_are_identical_bytes, cli::output::tests::a_field_declared_text_but_really_structured_renders_its_own_compact_json) - the known main-is-red JSON key-order issue (TASK-1.145), reached because this task's own sdk change pulls steg-app into the same test command. Not pushing per instructions; waiting for TASK-1.145's fix to land on main.

Check passed: `just check` exited 0 at 2026-11-21T21:40:42+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.140 on task-1.140-server-wait-for-a-pane-s-program-to-exit at a8edfb54f3de:

```text
ok      format check (0.3s)
ok      lint (2.5s)
ok      boundaries (0.1s)
ok      foundations (0.1s)
ok      foundations public surface (0.0s)
ok      design pages (0.1s)
ok      licences (0.7s)
ok      tests (-p steg -p steg-app -p steg-kernel -p steg-sdk -p steg-server) (84.9s)
ok      build (steg-sdk) (2.6s)
ok      test (steg-sdk) (2.7s)
ok      test (steg-sdk, local-socket) (3.2s)
ok      build (steg-kernel) (1.7s)
ok      test (steg-kernel) (1.5s)
ok      build (steg-server) (1.5s)
ok      test (steg-server) (63.5s)
ok      test (steg-server, race-fixture) (116.0s)
ok      test (steg-server, pty-race-fixture) (106.6s)
ok      test (steg-server, accept-backoff-fixture) (98.3s)
ok      build (steg) (2.9s)
ok      test (steg) (8.0s)
ok      test (steg, fixtures) (13.2s)
ok      build (steg-ui) (15.8s)
ok      test (steg-ui) (1.6s)
ok      build (steg-app) (1.6s)
ok      test (steg-app) (2.3s)
ok      tool tests (40.8s)
ok      architecture (0.7s)
ok      recipe names (0.2s)
ok      ui levels (0.1s)
ok      ui colours (0.1s)
```

Built pane.wait per the approved, revised plan: pending waits in Runtime's own state (new crates/server/src/runtime/wait.rs), resolved from the lock-holding exit (events.rs), close (close.rs) and disconnect (connection.rs) paths, one early-wakeable timer per positive --timeout wait. --timeout 0 checks now; a negative one is BadRequest before anything is touched; TimedOut names the expired seconds; the reply is the typed steg_sdk::WaitResult. pane.wait moved into connection.rs's own interception (dispatch/pane.rs, dispatch/mod.rs, cross_cutting.rs updated to match).

Checks: just check recorded green at the pre-merge head and again at the final merged head (a8edfb54, PR CI also green there). Every production-code negative control (the --timeout 0 branch, the negative-timeout check, the TimedOut message's own seconds, the close-path drain, the exit-path drain, and a scratch lock-held sleep proving the lock is released before a pending wait) recorded as a failing-then-passing pair, plus AC #3's cancel_waits control from a same-crate connection.rs unit test - all at the pre-merge head. None re-recorded after merging origin/main: the merge touched only Cargo.toml/boundaries.toml (workspace feature wiring for TASK-1.145's own fix), crates/steg/src/cli/output.rs (that fix itself, unrelated to pane.wait), and docs - none of wait.rs/close.rs/events.rs/connection.rs or the two test files this task added/touched, so no control's own mechanism or test changed at the final head; just check's own full test run passed cleanly there regardless.

Departures from the plan, each small: runtime/testing.rs's State literal and lib.rs's record re-export both needed one-line additions the plan's own new field/type implied but didn't name; pending_wait_count_for_test ended up #[cfg(test)] rather than plain pub(crate) (matching attachment_count_for_test's real precedent - the plan's own prose calling it 'not #[cfg(test)]' was a misreading, now corrected in the doc comment); crates/steg/src/cli/exit.rs got a one-line doc-comment rename only (a test name it cites, renamed for the error count going 13->14), no logic or test change there.

For a reviewer: start with crates/server/src/runtime/wait.rs (the whole mechanism in one file) and its three call sites in events.rs/close.rs/connection.rs; the six-step decision tree in register_wait is where the three engineer-decided behaviours (TimedOut, --timeout 0/negative, typed WaitResult) actually live.
<!-- SECTION:NOTES:END -->

## Comments

<!-- COMMENTS:BEGIN -->
author: @bruk
created: 2026-11-21T20:19:00Z

---

Plan approved by @bruk (83254935a0f3, size-3): its worker may build it.

---

author: @bruk.r1
created: 2026-11-21T21:58:00Z

---

@bruk.r1 is reviewing TASK-1.140: a same-machine review: it waited 0m with no other engineer's reviewer, longer than review_fallback_after (0m), so a fresh reviewer on its author's machine takes it.

---

author: @bruk.r1
created: 2026-11-21T22:09:00Z

---

Review by @bruk.r1 (same-machine): clean. It stays In Review for a person to merge.

Findings: 0 Important, 1 nit. Cites: review, testing-rust. The diff does not match the plan.

[Tests, Nit] crates/server/tests/pane_wait.rs:226 wait_for_program_status polls pane.show with a 20ms sleep between tries to establish an already-ended precondition for two tests, instead of waiting on an event (event.watch --for pane.exited exists for exactly this). This is a departure from .claude/rules/testing-rust.md ("No test sleeps to wait for something; it waits on the event"); the doc comment's claim that "there is no event at this level to wait on instead" is not accurate. Kept as a Nit, not Important: it is precondition setup only, bounded at 20s, fails loudly on timeout rather than passing spuriously, and the assertions it feeds (real exit codes) are sound either way. (The 5ms sleep in start_server() polling for the socket to accept, pane_wait.rs:75, is verbatim precedent already used in pane_capture.rs/pane_send.rs and is not a departure this diff introduces.)

No other findings. What I checked:

Thread/handle lifecycle for the one new kind of background thread (a timer per positive --timeout wait), every path that can end it or drop its last handle:
- Real exit (events.rs's Exited arm): resolve_pane_waits_locked removes the PendingWait under lock, sends the real reply, calls gate.resolve() if a gate exists. The timer thread's gate.wait wakes at once (Condvar notified), re-locks in timeout_wait_locked, finds nothing at (request_id, client_id), no-op, thread exits. No double reply.
- Close (close_and_dispatch): same resolve_pane_waits_locked call, still under the lock the close already holds, before force_end runs - same effect on the timer thread as above.
- Real timeout elapsing with nothing else having resolved it: the timer thread's gate.wait returns on the real Duration, timeout_wait_locked locks, finds-and-removes the entry (still present), replies TimedOut, drops the lock, thread exits. Bounded by its own `secs`, never past it.
- Client disconnect (ConnectionCleanup::drop -> cancel_waits): retains non-matching entries, calling gate.resolve() on every removed one with a gate, no reply (correct - the connection is gone). The timer thread wakes via the gate, finds nothing in timeout_wait_locked, no-op, exits. Matches AC #3.
- The push-then-spawn race (a PendingWait is pushed under lock, the lock is dropped, then the thread is spawned): the Arc<WaitGate> clone stored in the PendingWait is already resolvable before the thread exists: if exit/close/cancel fires in that window, the gate is already marked done, so the thread's own wait() returns immediately instead of sleeping, and the already-gone entry makes timeout_wait_locked a no-op. No reply is ever sent twice, and no thread blocks past its bound.
- Server stop (Runtime::stop/Drop, which calls force_end directly, bypassing close_and_dispatch): the plan rests this on every connection's ConnectionCleanup::drop (hence cancel_waits) running before Stopper::stop() lets the accept loop end, pre-existing listen.rs/lifecycle.rs machinery this task does not touch, already relied on for forget_attachment in the identical Drop. I did not re-derive that shutdown ordering myself; even if it were wrong, the timer thread is still bounded by its own timeout and does not outlive the process, so this would not leave a thread blocked for good.
I also repeated the --timeout 0 negative control myself, in a scratch worktree off the branch's own head (a8edfb54): with the Some(0) immediate-reply branch in register_wait replaced with indefinite registration, timeout_zero_on_a_running_pane_answers_timed_out_at_once fails exactly as described (times out reading the reply); reverted, it passes again. Confirms the control is real and still valid at the final head, not just at the pre-merge commit it was recorded against.

Evidence: the seven production-code negative controls (plus AC #3's cancel_waits control) were all recorded as failing-then-passing asdlc record-check pairs before the merge of origin/main (at ccf5dfb2b3b9), not re-recorded at the final head (a8edfb54). I checked what that merge actually changed: Cargo.toml/boundaries.toml (serde_json preserve_order wiring), crates/steg/src/cli/output.rs (the unrelated TASK-1.145 fix), board docs, and plugins/team/rules/testing.md (adding the "every control is a recorded asdlc record-check pair" rule) - none of wait.rs/close.rs/events.rs/connection.rs or the two test files these controls exercise. Per that same newly-added testing.md line ("when a test is edited after its control was recorded ... run the control again before citing it"), since none of those files were edited by the merge, the pre-merge pairs still count; I also re-ran one of them myself at the final head above and got the described result. One wrinkle: a later just check at ccf5dfb2b3b9 shows rustfmt reflowing two call sites in pane_wait.rs after the controls were recorded against that commit - whitespace-only (line-wrapping multi-arg calls), not a behavioural edit, so it does not invalidate the pairs either. The final just check is recorded directly against the PR head (a8edfb54), green across the whole suite including steg-server's race/pty-race/accept-backoff fixtures.

Plan-matched: no, even though each departure is explained in the notes and I found all of them benign - runtime/testing.rs's and lib.rs's one-line additions (both forced by the new field/type, not named in the plan), pending_wait_count_for_test ending up #[cfg(test)] rather than plain pub(crate) (verified true: attachment_count_for_test, the precedent the notes cite, is #[cfg(test)] too, at attach.rs:207), and crates/steg/src/cli/exit.rs's doc-comment-only rename (no logic or test change, confirmed by the diff).

Compliance: all four acceptance criteria have a test or recorded run behind each clause (AC #1: already-ended/real-exit/timeout/closed-while-waiting all in pane_wait.rs; AC #2: the lock/reader-not-blocked test; AC #3: connection.rs's own unit test plus its control; AC #4: pane.wait removed from dispatch/pane.rs's HANDLED and real-pty/bounded proof throughout). doc-2 sections 5/6/8 match the built behaviour exactly (TimedOut status 1, --timeout 0 checks now, negative --timeout is BadRequest status 2 before the target is resolved, typed WaitResult). Plan approval present and current (83254935a0f3, size-3, "Plan approved by @bruk").

---
<!-- COMMENTS:END -->
