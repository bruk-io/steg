---
id: TASK-1.88
title: 'Server: server show, server stop, and the client methods'
status: In Progress
assignee:
  - '@bruk.w6'
created_date: '2026-11-16T00:01:00Z'
updated_date: '2026-11-18T14:10:00Z'
labels:
  - size-3
milestone: m-3
dependencies:
  - TASK-1.45
  - TASK-1.77
  - TASK-1.78
parent_task_id: TASK-1
project: bruk-io-steg
asdlc_accepted_from: DRAFT-1
asdlc_branch: task-1.88-server-server-show-server-stop-and-the
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found on 2026-11-15 while checking TASK-1.77's plan: no task owned the server noun's methods. Dispatch answers every server method with NotImplementedYet, and TASK-1.77 adds only the steg entry point that starts a server process, so nothing could tell an explicitly started server to stop except killing it. doc-2 sections 8 and 9 (server show, server stop, --force), doc-8 R3 and R99. TASK-1.77's plan considered and set aside a signal handler for an explicit server (it needs unsafe or a new dependency, and none of its criteria asked for one): this task decides it. Depends on TASK-1.78 for ending programs and on TASK-1.45 for requests reaching the server at all.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 server show answers over the API with the server's version, how long it has been up, and its sessions and clients, as doc-2 section 8 gives; time is read by the server and reaches anything pure as data
- [ ] #2 server stop stops the server: it refuses with the ProgramRunning code while any session exists unless --force is given (doc-2 sections 8 and 9), answers the caller before the connection closes, then stops accepting, ends every program by the forced-close mechanism of TASK-1.78, joins every thread, removes the socket file and releases the lock; afterwards a client finds no server there
- [ ] #3 This is how a server started explicitly is told to stop (R99: it stops only when told to). The plan says what an interrupt or a termination signal from the terminal or a supervisor does to an explicit server: whether it is handled as a stop or left to end the process, what each leaves behind (the socket file, running programs), and if handling it needs unsafe code or a new dependency, that is named for the engineer
- [ ] #4 server start through the API on a running server answers as doc-2 gives for a server already running; the three server methods leave the not-yet list in dispatch and the completeness test covers them
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
RECOMMENDATION: SPLIT. The server half (server.show/start/stop, the Stopper wiring, the Outbound flush primitive, ServerConfig.force_grace, signals) and the client half (client.list/show, Tree::resolve_client, ClientRecord, last_active) touch disjoint production files and have independent proofs. They only share mechanical, minimal edits to dispatch/mod.rs's two test-only lists, methods/tests.rs (no change needed - see below) and record.rs/Outcome's variant lists. This plan covers both halves so the engineer can see the whole shape before deciding; split into two child tasks once approved.

Re-checked against the tree as it stands (not the task's own, now-stale wording):
- crates/sdk/src/description/methods/server.rs and client.rs are ALREADY FULLY BUILT (by whoever split methods into files) - server.show's fields (version/uptime/sessions/clients, all scalar), server.start's on_demand switch, server.stop's force switch, client.list's all switch, client.show's required target - all exist today. methods/tests.rs's doc_2_section_8() and doc_2_section_8_targets() already list and check all five methods correctly (target "none" for server.*/client.list, required Target for client.show). THIS TASK ADDS NO NEW DESCRIPTION AND NEEDS NO CHANGE TO methods/*.rs - only the dispatch/Runtime/record-building side.
- dispatch/mod.rs's NOT_YET_NOUNS currently lists all three server verbs and both client verbs as not-yet. INTERCEPTED_BEFORE_DISPATCH lists pane.attach/detach and tab.declare-size. event.watch is STILL in NOT_YET_NOUNS even though Runtime::subscribe already answers it for real (stale test-only labelling, pre-existing, not this task's bug) - moving it to INTERCEPTED_BEFORE_DISPATCH is a one-line, safe, disclosed drive-by fix while this task is already editing these exact lists; skip it if it causes any doubt.
- runtime/mod.rs's State already has notify_session_count_changed: Option<Sender<ConnectionEvent>>, set once, unconditionally-when-idle-checking-is-on, by set_idle_notifier - the exact pattern this plan reuses for reaching a Stopper.
- Runtime::stop(&mut self) (lifecycle.rs) already does force_end-every-pane + join-drain, and listen::run's shutdown sequence already does, in order: loop breaks on stop -> drop(listener) -> shutdown+join every open connection (inside the same thread::scope) -> runtime.stop() -> join the idle checker -> remove the socket file -> drop the lock. ALL OF THIS ALREADY EXISTS AND IS UNCHANGED BY THIS TASK. server.stop's whole job is: refuse-check, reply, flush, then call the one existing Stopper::stop() - nothing else to build on the shutdown side.

====================================================================
SERVER HALF
====================================================================

A. crates/kernel/src/moment.rs - add `pub const fn since_origin(&self) -> Duration { self.0 }`. Moment's own doc argues against reading it as a wall clock; this is the one legitimate exception the doc itself implies ("a point in time the caller... chose to represent as a duration since whatever origin it picked" - the caller already knows what the origin means; uptime IS that duration, by definition). Doc comment on the new method says so explicitly, and names the one honesty gap: clock_origin is `Runtime::new`'s own construction time, not the process's. In practice `Runtime::new` runs a few ms after the arbiter already bound the socket (login identity resolved, environment captured) - negligible, but stated, not silently assumed exact. One new test: `since_origin_returns_exactly_what_new_was_given`.

B. crates/server/src/dispatch/server.rs (new file) - plain dispatch_raw arm, no interception, mirrors session.rs's shape:
   - HANDLED: [("server","show"), ("server","start")]
   - dispatch(tree: &Tree, now: Moment, verb: &str) -> Result<Outcome, ApiError>
     - "show" => Ok(Outcome::ServerRecord(ServerRecord { version: SERVER_VERSION.to_string(), uptime: now.since_origin().as_secs(), sessions: tree.sessions().len(), clients: tree.clients().len() }))
     - "start" => Ok(Outcome::Nothing) - always, regardless of on_demand: criterion 4's answer. Over the API you are, by definition, already talking to a running server; doc-2 names no AlreadyRunning error for this method (its errors list is BadRequest only), and run_server_start's own CLI-level equivalent already treats "already running" as success (exit 0, "a server is already running"). Ok(null) is the plain, consistent reading - not invented.
   - SERVER_VERSION: a new `pub(crate) const SERVER_VERSION: &str = concat!("steg/", env!("CARGO_PKG_VERSION"));` - lives once (propose crates/server/src/lib.rs, top-level), replacing connection.rs's own inline `let server_version = concat!(...)` in the hello reply. One definition instead of two; small, disclosed, in-scope dedup since we're already touching the server-version string for server.show.
   - dispatch_raw's match (mod.rs) gets a new guard before the `("server", _)` catch-all: `(noun, verb) if server::HANDLED.contains(&(noun, verb)) => server::dispatch(tree, now, verb)`. The remaining catch-all becomes `("server", "stop") => Err(not_implemented_yet("connection.rs's own interception (TASK-1.88) - needs a connection's own Arc<Outbound> and the server's Stopper, neither reachable from this function's signature"))` - same shape and same reasoning pane.attach's own arm already states, just for this method.
   - Test file: crates/server/tests/dispatch/server.rs (new; add `mod server;` to tests/dispatch/main.rs) - real Tree, no doubles, same fixtures (fresh/request/moment/ok) session.rs already uses. Cases: show's four fields match the description exactly (declared_fields comparison, same as session_show_matches_exactly_the_fields_the_description_declares); show's sessions/clients counts track a tree with 0, 1 and 2 of each; show's uptime equals exactly what moment(n) was given (the pure pass-through, proven directly - this is the one place a wrong wiring would silently read 0 forever); start always answers Ok(null) with on_demand true, false, and omitted.

C. crates/sdk/src/record.rs - add `ServerRecord { version: String, uptime: u64, sessions: usize, clients: usize }`, Serialize/Deserialize, with the same per-field doc-comment-and-test shape every existing record has (serde round-trip; matches_the_fields_server_show_declares, comparing against server.show's own declared fields - server.start returns Nothing, so it has no record to check). Export it from crates/sdk/src/lib.rs's `pub use record::{...}` list and from Outcome (outcome.rs): new variant `ServerRecord(steg_sdk::ServerRecord)`, into_value arm `serde_json::to_value(record).expect(...)`.

D. crates/server/src/config.rs - add `pub force_grace: Duration` to ServerConfig, defaulted to the existing `crate::runtime::FORCE_GRACE_DEFAULT` constant (kept, now also the documented default rather than the only value) in `Default::default()`; add it to the `Debug` impl's field list. This is the task's own historical note (TASK-1.78's merge, 2026-11-15): the grace period belongs in ServerConfig beside handshake_timeout/arbitration_timeout, settable like them. Touches this task because server.stop's own force_end call uses exactly this value.

E. crates/server/src/listen.rs - two small, mechanical changes to `run`:
   - `Runtime::new(Arc::clone(&config.launcher), config.login.clone(), config.force_grace, config.attach_tick)` - reads the new config field instead of the constant directly.
   - Immediately after constructing `runtime` (before `build_idle_wiring`, unconditionally - NOT gated on `config.idle_timeout`, unlike idle-wiring): `runtime.set_stopper(stop.clone());`. Unlike the idle notifier, an EXPLICIT start still needs this - R99's other half ("a server started explicitly stops only when told to") is answered by server.stop, which needs to reach the same Stopper every server (explicit or on-demand) is already given by its caller (run_server_start constructs one and passes `&stopper` into `steg_server::run` on both paths today).

F. crates/server/src/runtime/mod.rs - add `stopper: Option<crate::listen::Stopper>` to `State` (initialised `None` in `Runtime::new`). Same-crate, cross-module `use crate::listen::Stopper;` is fine (doc-16's package boundaries are crate-level, not intra-crate - listen.rs already imports `crate::runtime::Runtime`, so this is the existing bidirectional module relationship, nothing new in kind).

G. crates/server/src/runtime/lifecycle.rs - three small additions, right beside the existing session_counter/set_idle_notifier (same seam, same file, per TASK-1.131's own notes):
   - `pub(crate) fn set_stopper(&self, stopper: crate::listen::Stopper) { self.lock().stopper = Some(stopper); }`
   - `pub(crate) fn session_count(&self) -> usize { self.lock().tree.sessions().len() }` - the refusal check's own read; a plain one-off call, not session_counter()'s long-lived closure (that closure exists for the idle checker's thread, which outlives any `&Runtime` borrow - a different need).
   - `pub(crate) fn stopper(&self) -> Option<crate::listen::Stopper> { self.lock().stopper.clone() }` - `Stopper` is already `Clone`.
   Flag for TASK-1.116/TASK-1.109 (named by the orchestrator's brief): both touch lifecycle.rs's stop/connect_client/disconnect_client area later; this task adds three small, independent methods beside them, not inside them - low collision risk, but naming it as asked.
   New unit tests (same file's existing `#[cfg(test)] mod tests`): set_stopper then stopper() returns a clone that (Stopper has no public is-it-set probe beyond what `.stop()` does, so this is proven indirectly, by behaviour, in the socket-level test below, not here) - at minimum, a direct test that `session_count()` tracks 0 -> 1 -> 2 sessions created through the existing `runtime.handle(...)` path, and back to 0 after close.

H. crates/server/src/outbound.rs - the one genuinely new mechanism. `Outbound::enqueue_control` only queues; `ConnectionCleanup::drop` does `close()` -> `shutdown(Both)` -> `writer.join()`, back to back, which can race an in-flight `write_all` on a real socket. AC #2 needs "the reply is written and flushed to the caller BEFORE its connection is closed" to be an actual guarantee, not a hope.
   - Change `Queues.control` from `VecDeque<Vec<u8>>` to `VecDeque<ControlItem>`, where `struct ControlItem { bytes: Vec<u8>, sent: Option<Sender<()>> }` (a plain pair, named rather than a bare tuple). `enqueue_control` pushes `ControlItem { bytes, sent: None }`, unchanged otherwise.
   - `Queues::pop_next`'s return type becomes `Option<(Vec<u8>, Option<Sender<()>>)>` - the control branch returns the item's own `sent`; the data-stream branch (unchanged logic) returns `(bytes, None)`.
   - `run_writer`: after `sink.write_all(&bytes)` returns `Ok`, if the popped item carried `Some(ack)`, `let _ = ack.send(());` before looping back to pop the next item - the ack fires strictly after this exact frame's bytes left `write_all`, never before, never for any other frame.
   - New method: `pub(crate) fn enqueue_control_and_wait_until_written(&self, bytes: Vec<u8>, bound: Duration) -> Result<(), EnqueueControlError>` - enqueues exactly like `enqueue_control` but with a fresh one-shot channel as `sent`, then blocks the CALLING thread on `ack_rx.recv_timeout(bound)` before returning `Ok(())` regardless of whether the receive timed out, errored (writer ended - broken socket, queue closed) or succeeded: in every case there is nothing further to wait for (either it was written, or the connection is already gone and there is nothing left to lose). Only the enqueue step itself can fail (`Full`/`Closed`), returned as today.
   - Bound: a new, local constant in connection.rs (not threaded through ServerConfig - this is a safety margin on an operation that should be near-instant, not a tunable production timeout), e.g. `STOP_REPLY_FLUSH_TIMEOUT = Duration::from_secs(5)`, the same order of magnitude as FORCE_JOIN_TIMEOUT/ESCALATE_CONFIRM_TIMEOUT elsewhere in this crate.
   - THE KEY FACT TO VERIFY EMPIRICALLY DURING BUILD, NOT ASSUME: once `write_all` returns on one end of an AF_UNIX socket pair, does a later `shutdown(Both)`/close on either end ever discard those already-written bytes before the peer reads them? A throwaway two-line probe (write_all then immediately shutdown, read from the other end) settles this directly, the same empirical discipline this crate's own comments already model (`outbound.rs`'s own `AlwaysFailsToWrite` test doc: "confirmed directly rather than assumed"). If confirmed (expected - the bytes are already in the kernel's socket buffer once write_all returns), flush-then-stop is sufficient and nothing about the client having READ the reply matters. Record the probe's result in the task's build notes either way.
   - New tests (outbound.rs's own `#[cfg(test)] mod tests`): using the existing `StepWriter` shape (already in this file, built for exactly this kind of deterministic ordering proof) - `enqueue_control_and_wait_until_written_does_not_return_until_the_write_call_itself_has_started/returned` (pins the ordering, not timing); `it_returns_promptly_once_already_closed_with_nothing_queued` (Closed/no-ack-ever-coming case does not hang past the bound); existing `control_is_drained_before_any_data_even_when_data_was_enqueued_first` and the other pop_next-shape tests re-run unchanged (the type change must not alter drain order).

I. crates/server/src/connection.rs - one new match arm in `handle_request` (same tier as pane.attach/pane.detach/tab.declare-size/event.watch - needs Outbound and, now, the Stopper), plus two new imports (`program_running`, `param_bool`) from `crate::dispatch`:
   ```
   "server.stop" => {
       let force = param_bool(&resolved, "force").unwrap_or(false);
       if !force && runtime.session_count() > 0 {
           let bytes = Connection::reply(id, Response::Error(program_running()));
           return outbound.enqueue_control(bytes).is_ok();
       }
       let bytes = Connection::reply(id, Response::Ok(serde_json::Value::Null));
       let _ = outbound.enqueue_control_and_wait_until_written(bytes, STOP_REPLY_FLUSH_TIMEOUT);
       if let Some(stopper) = runtime.stopper() {
           stopper.stop();
       }
       false // this connection's own job is done; `_cleanup` runs now rather
             // than waiting for listen::run's own shutdown pass to reach it
   }
   ```
   THE REFUSAL PREDICATE, NAMED EXPLICITLY (doc-2's own three statements disagree slightly - implementing the task's own criterion, not silently picking a different one): criterion 2 says "refuses... while any session exists unless --force"; doc-2 section 9 says "refuse while programs are still running"; doc-2 section 8's table says "--force if sessions exist". This plan implements "any session exists" (criterion 2, matching section 8's table and R99's own "a server that has a session never stops by itself") - a session with every program already ended still counts, deliberately different from pane.close/tab.close/session.close's own `any_pane_running` check. Proposing, as part of this task's own small cleanup, to reword doc-2 section 9's one sentence about server stop to match ("any session exists", not "programs are still running") - a wording fix to an already-settled rule, not a contract change; flag for the engineer to confirm or veto in review.
   `false` is a deliberate choice, not the only option: it lets `_cleanup`'s `Drop` clean this connection up immediately (close outbound, shut down its own socket, join its writer, disconnect the client) rather than waiting for listen::run's own shutdown pass to reach it in `connections`' iteration - both are safe (listen::run's later `shutdown()+join()` on an already-finished connection is a harmless no-op), `false` is just tidier and slightly faster.
   New tests, in connection.rs's own `#[cfg(test)] mod tests`, same in-process style `finish_attach`'s own tests already use (a real Runtime, a real Outbound/writer pair over `UnixStream::pair()`, no real listener): the refusal path (a session exists, no force -> ProgramRunning reply, Stopper not consulted at all - provable by never calling `set_stopper` at all and confirming no panic/no observable attempt); the success path with a real Stopper wired in top of a real Runtime, confirming (via the socket-level test below, since Stopper has no private-state probe reachable from here) the reply bytes a real client reads back are exactly `{"id":..,"result":null}` with no truncation.

J. crates/server/tests/server_stop.rs (new) - the authoritative, socket-level proof for AC #2 and #3's "what is left behind", over a real `listen::run` in a background thread and real `UnixStream` clients (same shape idle_stop.rs/handshake.rs already use):
   - Refuses with ProgramRunning, no --force, while a session exists (a real session created through a real connection first) - the server keeps running, provable by a second request succeeding afterward.
   - With --force, or with no sessions at all: the caller's own connection reads the full, correct Ok(null) reply (asserted byte-exact, not just "didn't hang") BEFORE the server tears anything down - this is the test that would have caught a lost reply if the flush-then-stop ordering were wrong; include a NEGATIVE CONTROL that removes the wait (call plain `enqueue_control` + immediate `stopper.stop()` in a scratch-reverted edit) and shows the reply is lost or truncated at least sometimes under this file's own load, recorded and reverted - matching testing.md's own "a test with its mechanism removed must actually fail" rule.
   - A session's real, running program (not just a shell sitting idle) is actually ended by the stop with --force - reuse the same trapping-program shape lifecycle.rs's own stop_tests module already proves Runtime::stop with, through the real server.stop request this time.
   - After the reply, a second client's `connect()` sees no server (refused, or once the socket file is gone, not found) - exactly the outcome listen::run's own doc already documents for this window; proven here through the real server.stop path, not only through a test-driven `Stopper::stop()` call as in listen.rs's own existing tests.
   - The socket file is gone and the process's own thread (`run`'s own return) actually returns within a bound.
   - Every thread this test's own server spawned is joined; no leftover process from this worktree before or after (this crate's own existing convention, every file).

K. crates/steg/tests/ (one new, cheap end-to-end case, as the brief asks) - extend the real-binary pattern server_start.rs already uses: start a real on-demand `steg server start --on-demand` process, run `steg server show` against it through the real CLI binary and assert exit 0 with version/uptime/sessions/clients in the JSON (`--json`), then `steg server stop` and assert exit 0, then assert the server process itself has actually exited (bounded wait on the child) and a further `steg server show --no-autostart` exits 4 (no server there). No change needed to crates/steg's own source beyond this test - connect.rs's generic `run()` already handles server.stop like any other generated command (confirmed: `server.start` is the one hand-written case in main.rs; `server.stop`/`server.show` are not, and need no entry there).

SIGNALS (criterion 3) - presented as a question, not decided here:
   Today, an explicitly-started server (`steg server start`, foreground, no `--on-demand`) installs no signal handler at all. SIGINT (Ctrl-C from the controlling terminal) and SIGTERM (a supervisor's ordinary "please stop") both run their OS default disposition - the process is terminated outright, mid-whatever-it-was-doing, with none of `run`'s own teardown executing: the socket file is left behind as a stale file (recovered automatically - TASK-1.77's own `a_fresh_start_replaces_the_socket_of_a_server_killed_outright` already proves the next `server start` cleans it up), and the arbiter's own flock is released by the OS the instant the process's fd table is torn down (nothing stale there). What happens to already-running pane programs needs verifying empirically before being written down as fact, not assumed: if they already run in their own session (pty slaves typically become session leaders of a new controlling terminal, which TASK-1.39/1.40's own pty setup likely already gives them, for ordinary job-control reasons unrelated to this task), a SIGINT delivered to the foreground process group would not reach them and they would keep running orphaned to pid 1 - consistent with R2's intent even though R2 is phrased about the app, not the server; this needs a direct probe (send SIGINT to a real server with a real pane running underneath it, check the pane's own program is still alive afterward), not an assumption, before the task's notes state it as fact.
   Handling them (treating SIGINT/SIGTERM as a request to stop, through the same Stopper mechanism server.stop already uses) needs either `unsafe` (installing a raw sigaction by hand) or a new dependency. `nix`, already allowed in boundaries.toml for this crate, has a `signal` feature with `pthread_sigmask`/`SigSet::wait` - a dedicated thread blocked in `sigwait` for SIGINT/SIGTERM, then calling the same `Stopper::stop()`, needs zero new dependency and zero new `unsafe` (sigwait is a safe blocking call once the signal is masked on every thread, which must happen before any other thread is spawned - itself a small, named risk: getting the mask timing wrong silently leaves it unhandled rather than crashing, so this needs its own test proving the signal is actually caught, not just "didn't crash").
   RECOMMENDATION: leave them unhandled for M1, as TASK-1.77's own plan already set aside once, and document precisely what is left behind (above) rather than build new signal-handling machinery this task's own criteria do not actually ask for. If the engineer wants handling, the `sigwait`-thread approach above is the smallest option with no new dependency and no unsafe - present it as the alternative, not silently build it.

====================================================================
CLIENT HALF
====================================================================

L. crates/kernel/src/target.rs - add `Tree::resolve_client`, mirroring `resolve_session` exactly (same file, same generic `resolve`/`to_result` helpers, already parameterised over `id_of`/`label_of`):
   ```
   pub fn resolve_client(&self, part: &str) -> Result<ClientId, Error> {
       to_result(part, resolve(part, self.clients(), Client::id, Client::label))
   }
   ```
   `Client::id`/`Client::label` already have exactly the right signatures (confirmed by reading client.rs). This is a small, disclosed addition to crates/kernel, outside this task's own package but necessary - doc-2 section 4's R10 (full id, label, position, suffix) is the only sensible way to resolve `client.show`'s target, and there is no other place this logic can live without duplicating `target.rs`'s own generic matcher. New tests mirror whatever test shape `resolve_session` already has in this file (by id, by label case-insensitively, by one-based position in connection order, by a four-or-more-character suffix, ambiguous, not found).

M. crates/server/src/dispatch/client.rs (new file) - plain dispatch_raw arm, no interception (needs only `&Tree`, nothing Runtime-only once last_active is settled as below):
   - HANDLED: [("client","list"), ("client","show")]
   - dispatch(tree: &Tree, request: &Request, verb: &str) -> Result<Outcome, ApiError>
     - "list" => Ok(list(tree, request)) - `--all` (param_bool, default false) includes every kind; without it, excludes ClientKind::Cli only (doc-2 section 8: "Command line clients only with --all") - App and Plugin are always included.
     - "show" => resolve the required `target` via `tree.resolve_client`, map_kernel_error on failure, then the same record-building as list's own per-client step.
   - A shared `fn record_for(tree: &Tree, client: &Client) -> ClientRecord` builds id/kind/label straight from the `Client`, `sessions` from `tree.sessions_viewed(client.id())` (already built by TASK-1.42/1.47 - `.expect(...)`, never actually fails here since the client was just read from this same tree), and `last_active: None` (see below).
   - dispatch_raw's `("client", _)` catch-all is removed entirely (replaced by the HANDLED guard - unlike server, nothing client-shaped needs interception, so there is no remaining not-yet arm to keep).
   - Test file: crates/server/tests/dispatch/client.rs (new; add `mod client;` to tests/dispatch/main.rs). Cases: list's field-set matches the description exactly; list excludes Cli by default and includes it with --all; list's order is connection order; show resolves by full id, by label, by one-based position, by a 4+-char suffix, and refuses ambiguous/not-found with the right codes and candidates; show's sessions field reflects a real `tree.view_pane` call (attach already landed - this reports real data, not a placeholder, satisfying criterion 5's "say what these report now" instruction directly); last_active is always null.

N. crates/sdk/src/record.rs - add `ClientRecord { id: ClientId, kind: ClientKind, label: Option<String>, sessions: Vec<SessionId>, last_active: Option<String> }`, Serialize/Deserialize, same per-field doc/test shape as every other record (serde round-trip; matches_the_fields_client_show_and_client_list_declare, the same comparison-by-key-name every other record's own test already uses - this is exactly what makes last_active: None/null satisfy that test without inventing any wire shape: PaneRecord::title and TabRecord::title already set the precedent of "always None, named as a documented, honest gap, not fabricated", and the field-match tests compare key names only, never value types). Export from crates/sdk/src/lib.rs's record re-export list and from Outcome: two new variants, `ClientRecord(steg_sdk::ClientRecord)` and `ClientRecords(Vec<steg_sdk::ClientRecord>)`, into_value arms exactly like PaneRecord/PaneRecords.
   `sessions`'s own wire shape: `client_fields()` already declares it `ValueKind::Text` with the doc "as a JSON array of identifiers" - this is the SAME placeholder every other non-scalar field in this codebase already uses (`ValueKind` has no array/object variant at all; `TabRecord.layout`, `TabRecord.shown_status` and `PaneRecord.status`/`size` are all declared `ValueKind::Text` too, with pane.rs's own comment stating exactly this: "status/size stay ValueKind::Text placeholders... the same choice for layout/shown_status"). `sessions: Vec<SessionId>` serialises to a genuine JSON array of id strings, matching the field's own doc literally - nothing invented, just following the precedent this file's own neighbouring methods already set.

LAST_ACTIVE (criterion 5 doesn't actually demand this field be populated - it only names "what each client is and the sessions it views" - but the description already commits to the field, so it must serialise to *something* consistent):
   The kernel's `Client` holds `activity: Vec<(SessionId, u64)>` - a monotonic tick per session, never a wall-clock reading (doc-16 R105: the kernel does no I/O, never reads a clock) - and no `chrono`/`time` dependency exists anywhere in this workspace. Three options, for the engineer:
   (a) RECOMMENDED. `last_active: None` always, for M1 - documented precisely as above, with exactly the precedent PaneRecord::title/TabRecord::title already set (a field the description commits to, honestly null until a later task wires real data through). Smallest, no new state anywhere, no new dependency, no Runtime-level interception needed for client.list/show at all (keeps them pure dispatch_raw arms, as designed above).
   (b) Track wall-clock activity at connect time only, in a new `HashMap<ClientId, SystemTime>` on Runtime's `State`, updated once in `connect_client` (lifecycle.rs, this task's own file) and read back formatted through a small, hand-rolled, dependency-free ISO-8601-UTC function added to the sdk (no existing one exists anywhere in this workspace to reuse). This makes client.list/show Runtime-level interceptions (losing the clean "plain dispatch_raw arm" shape above) for a value that would only ever show "when this connection was accepted", never real activity (updating it on every request would need threading a ClientId into Runtime::handle's own signature, or updating it from connection.rs's handle_request directly on every request - a wider, riskier change this task's criteria do not ask for).
   (c) Add a time-formatting dependency (chrono or time) to the sdk, update boundaries.toml and check its licence against deny.toml - the only way to get real per-request activity timestamps cheaply, but a new dependency for a field no criterion requires be real.
   This plan builds (a) and treats (b)/(c) as a named follow-up if the engineer wants real data later.

====================================================================
SHARED, MECHANICAL EDITS (both halves touch these; keep each minimal)
====================================================================
- crates/server/src/dispatch/mod.rs: `handled()`'s test-only lists - move server.show/start and client.list/show out of NOT_YET_NOUNS into their new modules' HANDLED; keep server.stop (with a reworded message) and optionally event.watch (drive-by, disclosed, skippable) in/moved appropriately; declare the two new `mod server;`/`mod client;` and their `pub(crate) use` re-exports the same way every existing noun module is wired in.
- crates/sdk/src/description/methods/tests.rs: re-verified above - NO CHANGE NEEDED. Both halves should re-run its tests to confirm, not just trust this plan's own reading.
- crates/sdk/src/record.rs / crates/sdk/src/lib.rs / crates/server/src/dispatch/outcome.rs: each half adds its own types/variants; no shared lines to conflict on if each sticks to appending at the end of the relevant list/match, which both should do.
- crates/server/tests/dispatch/main.rs: each half adds its own `mod` line.

COLLISIONS NAMED BY THE ORCHESTRATOR'S BRIEF
- TASK-1.90 (pane send/capture/wait): runtime/input.rs, pane.rs, dispatch/pane.rs - this task touches none of those three; no overlap found.
- TASK-1.123 (bounding test waits): edits test files across crates/server broadly - this task adds new test files rather than editing existing ones in most places (server_stop.rs, dispatch/server.rs, dispatch/client.rs are new), but DOES edit outbound.rs's and connection.rs's existing test modules - flag this pairing explicitly on the task.
- methods/tests.rs and dispatch/mod.rs's lists: shared with every other method-adding task, by design (per the orchestrator's brief) - kept to minimal, additive edits as above.
- TASK-1.116 (status expiry) and TASK-1.109 (more events): both later touch lifecycle.rs's stop/connect_client/disconnect_client - this task adds set_stopper/session_count/stopper() beside them, not inside them; named for both as asked.

====================================================================
ORDER OF WORK
====================================================================
1. Kernel: Moment::since_origin (A), Tree::resolve_client (L) - small, independent, unlocks everything else.
2. sdk: ServerRecord/ClientRecord in record.rs, lib.rs exports, Outcome's new variants (C, N).
3. Server, pure arms: dispatch/server.rs, dispatch/client.rs, mod.rs's list edits, dispatch/mod.rs's catch-all rewording (B, M, shared edits) - get tests/dispatch/{server,client}.rs green before touching anything connection-level.
4. Server, plumbing for stop: config.rs's force_grace (D), listen.rs's two edits (E), runtime/mod.rs's State field (F), lifecycle.rs's three methods (G).
5. Server, the flush primitive: outbound.rs's ControlItem/pop_next/enqueue_control_and_wait_until_written + its own tests, INCLUDING the empirical AF_UNIX probe (H) - do this before connection.rs's own arm, since it is the one piece everything else depends on being actually true.
6. Server, the interception: connection.rs's new "server.stop" arm and its own unit tests (I).
7. Socket-level proof: crates/server/tests/server_stop.rs, with its own negative control (J).
8. CLI end-to-end: one new crates/steg/tests case (K).
9. Signals: write up the probe's result and the recommendation in the task's notes; do not build handling unless the engineer asks for it after reading this plan.
10. just check at every step boundary above, and once more at the end; record each.

====================================================================
RISKS
====================================================================
- The AF_UNIX write-then-shutdown assumption (H) is the one load-bearing empirical fact the whole flush design rests on - verified by a direct probe before relying on it, and by server_stop.rs's own negative control, not merely argued.
- `false` from connection.rs's new arm self-terminates this connection concurrently with listen::run's own external shutdown pass reaching the same entry in `connections` - argued safe above (both paths are independently idempotent-ish: shutdown on an already-shut-down socket, join on an already-finished thread), but worth one dedicated check in server_stop.rs that this never panics or double-runs ConnectionCleanup's Drop (it structurally cannot - Drop runs exactly once per connection thread's own stack, listen::run never touches ConnectionCleanup at all).
- Moment::since_origin and the kernel's Tree::resolve_client are the two places this task reaches outside crates/server; both are small, mechanical, and exactly mirror an existing method in the same file - low risk, but named as a boundary crossing the brief did not mention.

====================================================================
PROOF, CRITERION BY CRITERION
====================================================================
#1 (server show) -> tests/dispatch/server.rs (counts, version, uptime pass-through) + the CLI end-to-end case (K).
#2 (server stop) -> connection.rs's own unit tests (refusal path) + server_stop.rs (the full sequence, the flush ordering with its negative control, the real force_end, the second-client-sees-nothing window, the socket file gone, every thread joined) + the CLI end-to-end case (K).
#3 (signals) -> written up as a question with a recommendation, not built, per the task's own wording ("the plan says what..."); a direct probe's result recorded either way.
#4 (server start on a running server) -> tests/dispatch/server.rs's start cases + doc-2's own already-complete description (no error invented).
#5 (client list/show) -> tests/dispatch/client.rs (filtering, resolution, real sessions-viewed data, last_active's documented null) + resolve_client's own kernel-level tests.

Each criterion above gets a failing-then-passing control recorded against the final tests (the new dispatch arm or interception absent/stubbed, test fails; restored, passes) - standard for every new arm added here, following this crate's own existing convention throughout runtime/close.rs and outbound.rs's own test suites.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
The client noun's two methods had no owner either (dispatch answers NotImplementedYet, citing TASK-1.42, which built the kernel side only); they are included here as the same kind of gap. If the plan finds the server methods and the client methods are two reviewable changes, split them.

From the independent read of TASK-1.78's merge (2026-11-15): the forced close's grace period (FORCE_GRACE_DEFAULT, 2s) is a constant read directly in listen.rs and is not a field of ServerConfig beside handshake_timeout and arbitration_timeout. server stop uses that mechanism: put the grace period in ServerConfig here, with its default, so it can be set like the other timeouts.

Split by @bruk.o on 2026-11-18, as the plan recommended: this task keeps the server noun (show, stop, start on a running server, and what a signal does); client list and client show moved to a draft task. The two share only additive edits to dispatch/mod.rs's lists and the record and outcome variant lists. The plan's three questions go to the engineer with the rewritten plan: signals left unhandled for M1; last_active (the client half's); and doc-2 section 9's wording against this task's second criterion.
<!-- SECTION:NOTES:END -->
