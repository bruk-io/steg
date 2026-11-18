---
id: TASK-1.88
title: 'Server: server show, server stop, and the client methods'
status: In Progress
assignee:
  - '@bruk.w6'
created_date: '2026-11-16T00:01:00Z'
updated_date: '2026-11-18T23:03:00Z'
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
asdlc_plan_approved_by: bruk
asdlc_plan_approved_hash: 'c42e82b251d6'
asdlc_plan_approved_size: size-3
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
REWRITE (server noun only - client.list/client.show, Tree::resolve_client and last_active are now a separate draft task carrying that design; removed here). This is the one sanctioned edit to this plan after reporting it.

Re-checked against the tree as it stands (not the task's own, now-stale wording): crates/sdk/src/description/methods/server.rs is already fully built (version/uptime/sessions/clients, all scalar fields; server.start's on_demand switch; server.stop's force switch) and methods/tests.rs's doc_2_section_8()/doc_2_section_8_targets() already list and check all three server methods correctly. THIS TASK ADDS NO NEW DESCRIPTION AND TOUCHES NO FILE UNDER crates/sdk/src/description/ - only the dispatch/Runtime/record-building side, plus one small kernel crossing (below). It adds no new ErrorCode and does not touch crates/sdk/src/description/shapes.rs.

====================================================================
ONE KERNEL CROSSING, NAMED
====================================================================
crates/kernel/src/moment.rs - add `pub const fn since_origin(&self) -> Duration { self.0 }`. server.show needs to turn `now: Moment` (already threaded into dispatch_raw, already built as `Instant::now().duration_since(state.clock_origin)` by Runtime::handle's generic arm) into a plain number of seconds; `Moment` has no public accessor for its own duration today. This is the one legitimate exception to Moment's own doc ("never read as a wall-clock value... only asks is this one later, this one ten minutes on, never what time is it") - the caller already knows what the origin means (Runtime construction), and uptime literally IS that duration; the doc comment on the new method says so explicitly, and names the honesty gap: clock_origin is Runtime::new's own construction time, a few ms after the arbiter actually bound the socket, not the process's start. One new test: `since_origin_returns_exactly_what_new_was_given`.

====================================================================
B. crates/server/src/dispatch/server.rs (new file)
====================================================================
Plain dispatch_raw arm, no interception at all for show/start - mirrors session.rs's shape exactly:
- HANDLED: [("server","show"), ("server","start")]
- dispatch(tree: &Tree, now: Moment, verb: &str) -> Result<Outcome, ApiError>
  - "show" => Ok(Outcome::ServerRecord(ServerRecord { version: SERVER_VERSION.to_string(), uptime: now.since_origin().as_secs(), sessions: tree.sessions().len(), clients: tree.clients().len() }))
  - "start" => Ok(Outcome::Nothing) - always, regardless of on_demand. Over the API you are, by definition, already talking to a running server; doc-2 names no AlreadyRunning error for this method (its errors list is BadRequest only), and run_server_start's own CLI-level equivalent already treats "already running" as success (exit 0, "a server is already running"). Ok(null) is the plain, consistent reading, not invented - this is criterion 4 in full.
- SERVER_VERSION: `pub(crate) const SERVER_VERSION: &str = concat!("steg/", env!("CARGO_PKG_VERSION"));`, lives once (crates/server/src/lib.rs), replacing connection.rs's own inline `let server_version = concat!(...)` in the hello reply - one definition instead of two, small disclosed dedup since this task is already adding this exact string for server.show.
- dispatch/mod.rs's match gets a new guard before the `("server", _)` catch-all: `(noun, verb) if server::HANDLED.contains(&(noun, verb)) => server::dispatch(tree, now, verb)`. The remaining catch-all narrows to `("server", "stop") => Err(not_implemented_yet("connection.rs's own interception (TASK-1.88) - needs the connection's own Arc<Outbound> and the server's Stopper, neither reachable from this function's signature"))`, the same shape pane.attach's own arm already has.
- Test file: crates/server/tests/dispatch/server.rs (new; add `mod server;` to tests/dispatch/main.rs). Cases: show's four fields match the description exactly (declared_fields comparison, the pattern session_show_matches_exactly_the_fields_the_description_declares already uses); show's sessions/clients counts track a tree with 0, 1, 2 of each; show's uptime equals exactly what moment(n) is given (the pure pass-through - the one place a wrong wiring would silently read 0 forever, proven directly, not inferred); start always answers Ok(null) with on_demand true, false and omitted.

====================================================================
C. crates/sdk/src/record.rs / lib.rs / outcome.rs
====================================================================
Add `ServerRecord { version: String, uptime: u64, sessions: usize, clients: usize }`, Serialize/Deserialize, same per-field doc-comment-and-test shape every existing record has (serde round-trip; matches_the_fields_server_show_declares against server.show's own declared fields - server.start returns Nothing, so it has no record to check). Export from crates/sdk/src/lib.rs's `pub use record::{...}` list. Outcome (dispatch/outcome.rs) gets one new variant, `ServerRecord(steg_sdk::ServerRecord)`, with an into_value arm identical in shape to every existing record variant.

====================================================================
D-G. Plumbing: ServerConfig.force_grace, the Stopper reaching a request handler
====================================================================
D. crates/server/src/config.rs - add `pub force_grace: Duration` to ServerConfig, defaulted to the existing `crate::runtime::FORCE_GRACE_DEFAULT` constant (kept, now the documented default rather than the only value) in `Default::default()`, added to the `Debug` impl's field list. The task's own historical note (TASK-1.78's merge, 2026-11-15): this belongs in ServerConfig beside handshake_timeout/arbitration_timeout, settable the same way. Relevant here because server.stop's own force_end call uses exactly this value.

E. crates/server/src/listen.rs - two mechanical edits inside `run`:
   - `Runtime::new(Arc::clone(&config.launcher), config.login.clone(), config.force_grace, config.attach_tick)` - reads the new field instead of the constant directly.
   - Immediately after constructing `runtime` (before build_idle_wiring, UNCONDITIONALLY - not gated on config.idle_timeout the way idle-wiring is): `runtime.set_stopper(stop.clone());`. An explicit start never gets idle-wiring (R99: it never idle-stops by itself), but it still needs this - R99's other half, "a server started explicitly stops only when told to", is answered by server.stop, which needs the same Stopper every server (explicit or on-demand) is already given by its own caller today (run_server_start constructs one and passes `&stopper` into steg_server::run on both paths, already, with nothing downstream able to reach it - the task's description names this exact gap).

F. crates/server/src/runtime/mod.rs - add `stopper: Option<crate::listen::Stopper>` to `State` (`None` in Runtime::new's construction). Same-crate, cross-module `use crate::listen::Stopper` - listen.rs already imports `crate::runtime::Runtime`, so this is the existing bidirectional module relationship, nothing new in kind; doc-16's package boundaries are crate-level, not intra-crate.

G. crates/server/src/runtime/lifecycle.rs - three small additions beside the existing session_counter/set_idle_notifier (same seam, same file, per TASK-1.131's own notes):
   - `pub(crate) fn set_stopper(&self, stopper: crate::listen::Stopper) { self.lock().stopper = Some(stopper); }`
   - `pub(crate) fn session_count(&self) -> usize { self.lock().tree.sessions().len() }` - the refusal check's own read; a plain one-off call, not session_counter()'s long-lived closure (that one exists for the idle checker's thread, which outlives any `&Runtime` borrow - a different need, unrelated here).
   - `pub(crate) fn stopper(&self) -> Option<crate::listen::Stopper> { self.lock().stopper.clone() }` (Stopper is already Clone).
   New unit test here: session_count() tracks 0 -> 1 -> 2 sessions created through runtime.handle(...), back to 0 after close.
   Flag for TASK-1.116/TASK-1.109 (named by the orchestrator's brief): both later touch lifecycle.rs's stop/connect_client/disconnect_client area; this task adds three small, independent methods beside them, not inside them.

====================================================================
H. THE FLUSH PRIMITIVE - crates/server/src/outbound.rs
====================================================================
Compared directly against TASK-1.128's Outbound::end_stream, as asked, before adding anything new:

end_stream solves a DIFFERENT problem. It defers a STREAM's own internal bookkeeping teardown (removing it from `data`/`order`/`closing`) until Queues::pop_next has actually popped that stream's final frame - a purely internal sequencing fix, with no caller-side wait at all (finish_attach calls `end_stream` then `connection.close_stream` immediately after, with nothing in between). It works with no wait because nothing EXTERNAL races it: the connection's own socket stays open and in ordinary use afterward - an attach ending does not tear the whole connection down, so there is no competing "shut this socket down now" action for end_stream's own deferred teardown to lose a race against.

server.stop needs something end_stream's mechanism cannot give: an ACTIVE confirmation, readable by a caller on another thread, that a specific control frame's bytes have actually left `write_all`, before that caller deliberately does something external - here, calling `Stopper::stop()` and returning `false` to let `ConnectionCleanup::drop` run `shutdown(Both)` on this same socket. `Outbound::close()`'s own doc already promises "a reply enqueued just before this is called still gets written... as long as the socket itself is still good" - but that promise is about `close()`'s own internal behaviour (it drains, it does not drop the queue); it says nothing about a CONCURRENT `shutdown(Both)` on a different fd of the same underlying socket, which `shutdown_handle.shutdown(Both)` genuinely is (a `try_clone`'d duplicate shares the same kernel socket object, not an independent one - confirmed by this same file's own doc on why `shutdown` is needed at all to unstick a blocked writer: it "forces that blocked write to fail promptly", which only works because it acts on the shared socket, not the fd). Calling `close()` then `shutdown(Both)` back to back, with no wait, is exactly today's `ConnectionCleanup::drop` sequence - fine for an ordinary disconnect (nobody is waiting on that specific in-flight frame), not fine for server.stop, where this task's own criterion explicitly requires the reply to survive. So: a new, active primitive is needed; it is not a second copy of end_stream's idea, it is one step stronger, for the one case that needs it.

Design:
- `struct ControlItem { bytes: Vec<u8>, sent: Option<Sender<()>> }`. `Queues.control` becomes `VecDeque<ControlItem>` (was `VecDeque<Vec<u8>>`).
- `enqueue_control` pushes `ControlItem { bytes, sent: None }` - unchanged behaviour otherwise.
- `Queues::pop_next`'s return type becomes `Option<(Vec<u8>, Option<Sender<()>>)>`: the control branch returns the popped item's own `sent`; the data-stream branch (logic unchanged) returns `(bytes, None)`.
- `run_writer`: after `sink.write_all(&bytes)` returns `Ok`, `if let Some(ack) = ack { let _ = ack.send(()); }`, before looping back to pop the next item - the ack fires strictly after THIS frame's own bytes left `write_all`, never before, never for any other frame.
- New method: `pub(crate) fn enqueue_control_and_wait_until_written(&self, bytes: Vec<u8>, bound: Duration) -> Result<(), EnqueueControlError>` - enqueues exactly like `enqueue_control` but with a fresh one-shot channel as `sent`, then blocks the calling thread on `ack_rx.recv_timeout(bound)` before returning `Ok(())` regardless of whether that receive succeeded, timed out, or the channel disconnected (writer ended - broken socket, already closed): in every one of those outcomes there is nothing further worth waiting for (either it was written, or the connection is already gone and there is nothing left to lose). Only the enqueue step itself can fail (Full/Closed), surfaced exactly as `enqueue_control` already does.
- Bound: a new, local constant in connection.rs - `STOP_REPLY_FLUSH_TIMEOUT = Duration::from_secs(5)`, the same order of magnitude as FORCE_JOIN_TIMEOUT/ESCALATE_CONFIRM_TIMEOUT elsewhere in this crate. Not threaded through ServerConfig: a safety margin on an operation expected to be near-instant, not a tunable production timeout.

THE PROBE, run first, before anything else in this section is built: on a real `UnixStream::pair()`, call `write_all` on one end with a few bytes, THEN immediately call `shutdown(Shutdown::Both)` on a `try_clone()`'d duplicate of that SAME end (not the writer's own handle - the exact shape `shutdown_handle` is in production), then read from the OTHER end and confirm the bytes still arrive. This settles, directly rather than by reasoning, whether a write that has already returned `Ok` from the kernel is safe against a racing shutdown on a cloned fd of the same socket - expected yes (the bytes are already in the kernel's own socket buffer once `write_all` returns), which is exactly what makes "wait for the write to return, then shut down" sufficient, with no need for the client to have READ anything. IF THE PROBE SHOWS OTHERWISE: the fallback is to delay `shutdown(Both)` itself, not just the ack-wait - e.g. a short, bounded grace sleep after the ack before `_cleanup` runs, or reading one byte of TCP/Unix-level acknowledgement is not available over AF_UNIX at all, so the real fallback would be to accept that server.stop's own self-cleanup (step 5 below) cannot call shutdown immediately and must instead let the connection close only once listen::run's own external pass reaches it naturally (removing the `false`-return self-cleanup entirely, falling back to `true` and trusting the ordinary, slower path) - named here so a surprising probe result does not stall the task, but not expected to be needed.

New tests (outbound.rs's own `#[cfg(test)] mod tests`), using the existing `StepWriter` shape already in this file (built for exactly this kind of deterministic-ordering proof, not a timing-based one): `enqueue_control_and_wait_until_written_does_not_return_until_the_write_call_itself_has_returned` (pins the ordering via `StepWriter`'s own proceed-channel, not a sleep); `it_returns_promptly_once_the_queue_is_already_closed_with_nothing_coming` (the Closed/no-ack-ever case does not hang past the bound). Every existing pop_next-shape test (`control_is_drained_before_any_data_even_when_data_was_enqueued_first` etc.) re-run unchanged - the type change must not alter drain order.

====================================================================
I. THE INTERCEPTION - crates/server/src/connection.rs
====================================================================
One new match arm in `handle_request`, same tier as pane.attach/pane.detach/tab.declare-size/event.watch (needs Outbound and, now, the Stopper), plus two new imports from `crate::dispatch` (`program_running`, `param_bool`):

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
    false // this connection's own job is done - `_cleanup` runs now
}
```

THE REFUSAL PREDICATE, doc-2's own three statements compared directly (not silently reconciled):
- Criterion 2 (this task's own brief): "refuses with the ProgramRunning code while any session exists unless --force is given."
- doc-2 section 8's table: "server stop | none | --force if sessions exist" - agrees with the criterion.
- doc-2 section 9's prose: "close and server stop refuse while programs are still running, with status 1 and a message naming --force" - disagrees: "programs still running" is a narrower, different condition than "any session exists" (a session whose sole pane's program has already exited still counts under the criterion's own wording, and under R99's "a server that has a session never stops by itself", but would NOT trigger a refusal under section 9's literal wording).
THE CODE FOLLOWS THE CRITERION (and section 8's table): `!force && runtime.session_count() > 0`, not `any_pane_running`. This is deliberately different from pane.close/tab.close/session.close's own refusal check, which really is about running programs. Proposed, not made here: reword doc-2 section 9's one sentence about server stop specifically to "any session exists" rather than "programs are still running", to match section 8 and this task's own criterion - a spec change, which is the engineer's (Bruk's) to make, not this plan's; named as question 2 below.

`false`, not `true`: lets `_cleanup`'s Drop clean this connection up immediately (outbound.close() -> shutdown(Both) on its own socket -> join its own writer, which has nothing left queued by this point and returns at once -> forget attachments -> deregister subscriber -> disconnect_client -> send Disconnected) rather than waiting for listen::run's own external shutdown pass to reach this same connection in its later iteration over `connections`. Both are safe (listen::run's later `shutdown()+join()` on an already-finished thread is a harmless no-op - shutdown on an already-shut-down socket, join on an already-returned thread); `false` is simply tidier and does not depend on the accept loop noticing in time.

New unit tests (connection.rs's own `#[cfg(test)] mod tests`, same in-process style finish_attach's own tests already use - a real Runtime, a real Outbound/writer pair over `UnixStream::pair()`, no real listener): the refusal path (a session exists, no force -> ProgramRunning reply, over a Runtime that never had `set_stopper` called at all - proving the Stopper is never even consulted when refusing); the success path confirms the reply bytes a client reads back are exactly `{"id":...,"result":null}` with no truncation, with a real Stopper wired in (confirming it actually gets called is left to the socket-level test below, since `Stopper` exposes no private-state probe reachable from this file).

====================================================================
THE FULL ORDERED SEQUENCE, WITH BOUNDS - server.stop end to end
====================================================================
This task's own new work is steps 1-5. Steps 6-13 are listen::run's EXISTING, unconditional shutdown sequence (unchanged by this task) - the SAME sequence that already runs when the idle checker (TASK-1.86) decides to stop an on-demand server, since `idle::run_checker` also just calls `Stopper::stop()` on the same `Stopper`. ONE MECHANISM, not two: server.stop's entire job is to validate, reply, flush, and call the one existing `Stopper::stop()` - nothing downstream is re-implemented.

1. Validate: `!force && session_count() > 0` -> refuse with ProgramRunning, stop here. [Instant - a lock, a length check.]
2. Build the Ok(null) reply's bytes.
3. `enqueue_control_and_wait_until_written(bytes, STOP_REPLY_FLUSH_TIMEOUT)` - blocks THIS connection's own reader thread until the writer thread has actually called `write_all` for these exact bytes and it returned, or the queue reports closed, or 5s passes. [Bounded by STOP_REPLY_FLUSH_TIMEOUT=5s; expected near-instant in practice - nothing else should be contending for this connection's own Outbound.]
4. `stopper.stop()` - sets the atomic flag and wakes the accept loop's blocked `accept()` with a throwaway connect to the same socket (existing mechanism, `wake_accept`). [Instant to call; the wake itself depends on step 6 below.]
5. Return `false` -> `run_post_handshake` returns -> `_cleanup`'s Drop runs for THIS connection: outbound.close(), shutdown(Both) on its own socket, join its own writer (nothing left queued - returns at once), forget attachments, deregister subscriber, disconnect_client, send Disconnected. [All existing, all fast; writer.join() is bounded because by this point its queue is empty and closed.]
6. (Triggered by step 4's wakeup, on the accept-loop thread) The blocked `accept()` call returns; the loop checks `stop.should_stop()` -> true -> breaks with `Ok(())`. [Deterministic via `wake_accept`, not a poll - effectively instant.]
7. `drop(listener)` - no new connection is ever accepted again; a connect() landing from this instant on is refused outright (ECONNREFUSED), never queued into the backlog. [Instant.]
8. `for (shutdown_handle, join) in connections { shutdown(Both); join(); }` - every still-open connection (this one, if listen::run's pass reaches it before step 5 finished on its own - safe either order, as above) is forced closed and its thread joined. [Bounded by each thread noticing its own shutdown and returning - already proven elsewhere (`a_completed_idle_connection_is_closed_and_run_returns_promptly_on_stop`) to be prompt, since shutdown forces a blocked read to see Ok(0) at once.]
9. `runtime.stop()` - force_end's six passes over every pane still tracked (mark closing; signal_hangup to all; wait on a shared deadline up to `force_grace`; signal_kill the stragglers; confirm within `ESCALATE_CONFIRM_TIMEOUT`; settle+join every worker within `FORCE_JOIN_TIMEOUT`), then drop every clone of `events` and join the one drain thread within `FORCE_JOIN_TIMEOUT`. [Bounded overall by `force_grace + ESCALATE_CONFIRM_TIMEOUT + FORCE_JOIN_TIMEOUT` regardless of pane count - the existing, documented guarantee, unchanged.]
10. Join the idle checker thread, if any. [Bounded - its channel disconnects the instant every sender, including this Runtime's own clone from step 9, is gone.]
11. Remove the socket file. From this instant, a connecting client sees "no such file" (not found) rather than "refused".
12. `drop(lock)` - releases the arbiter's flock; a FUTURE `server start` can now succeed.
13. `run()` returns `Ok(())` -> the process exits 0 (main.rs's own existing `Ok(()) => 0` mapping).

WHAT A SECOND CLIENT CONNECTING DURING THE STOP SEES, by window:
- Before step 7: the loop has already broken by construction before this code ever accepts a connection again (listen.rs's own existing comment explains exactly why the listener is dropped immediately rather than after the join below), so a `connect()` landing here is queued into the kernel's own backlog and then reset once the listener drops - never reaches a real reply.
- Between steps 7 and 11: `ECONNREFUSED` - refused outright, nobody listening on that path.
- After step 11: "no such file" - not found.
Both of the latter two are the exact outcomes the sdk's local-socket provider already treats identically ("try to start one", R99's own autostart story) - nothing new, confirmed here through server.stop's own real trigger (the new socket-level test, below), not only through a test harness calling `Stopper::stop()` directly as existing tests do.

====================================================================
J. crates/server/tests/server_stop.rs (new) - the authoritative socket-level proof
====================================================================
Mirrors idle_stop.rs's own shape (a real `listen::run` on a background thread, real `UnixStream` clients, no process spawn needed at this layer) rather than a shared cross-worktree helper: a local `fn unique_socket_path()` identical in shape to idle_stop.rs's own (`std::process::id()` + a per-file `static AtomicU64` counter + `std::env::temp_dir()`, named with this file's own suffix) - the same pattern every sibling test file in this crate already uses independently (this crate's own convention: "an integration test file is its own crate; there is no shared test-support module"), written so TASK-1.123's planned consolidation into one shared helper is a drop-in swap later (same signature, same semantics), not a redesign.

Cases:
- Refuses with ProgramRunning, no --force, while a real session exists (created over a real connection first) - the server keeps running afterward, proven by a second request succeeding.
- With --force, or with no sessions at all: the caller's own connection reads the FULL, BYTE-EXACT Ok(null) reply before the server tears anything down - the direct proof of the ordering AC #2 asks for. NEGATIVE CONTROL: a scratch-reverted edit that calls plain `enqueue_control` + immediate `stopper.stop()` (skipping the wait) and shows the reply is lost or truncated at least some of the time under this file's own load - recorded, then reverted, matching testing.md's "a test whose mechanism is removed must actually fail" rule.
- A session's real, running program (not an idle shell) is actually ended by the stop, with --force - reuses the trapping-program shape lifecycle.rs's own `stop_tests` module already proves `Runtime::stop` with, through the real `server.stop` request this time, not a direct `Runtime::stop()` call.
- After the reply: a second client's `connect()` sees no server, matching the two windows named above - proven through this real trigger, not only through listen.rs's own existing `Stopper::stop()`-driven tests.
- The socket file is gone; `run()`'s own background thread actually returns within a bound; the arbiter's lock is releasable again (a fresh `attempt()` against the same path succeeds).
- Every thread and every real process this test's own server or client fixtures start is joined/reaped on every path (success, refusal, and the negative control above); no leftover process from this worktree before or after.

====================================================================
K. crates/steg/tests/ - one cheap end-to-end case through the real binary
====================================================================
Extends server_start.rs's own real-process pattern: start a real on-demand `steg server start --on-demand` process, run `steg server show --json` against it through the real CLI binary and assert exit 0 with version/uptime/sessions/clients present; then `steg server stop` and assert exit 0; then assert the server process itself has actually exited (a bounded wait on the child, the same `wait_bounded` helper server_start.rs's own tests already added); then `steg server show --no-autostart` exits 4 (no server there). No change needed to crates/steg's own source beyond this test - `server.stop`/`server.show` are not main.rs's one hand-written case (`server.start` is); connect.rs's generic `run()` already routes them like any other generated command, confirmed by reading main.rs's own `dotted == "server.start"` special-case, which names nothing else.

====================================================================
QUESTIONS FOR THE ENGINEER
====================================================================

1. SIGNALS (criterion 3). Default proposed: leave SIGINT/SIGTERM unhandled for M1.
   Cost of the default: a hard-killed explicit server leaves a stale socket file (self-healing - TASK-1.77's own `a_fresh_start_replaces_the_socket_of_a_server_killed_outright` already proves the next `server start` cleans it up) and, to be confirmed by a direct probe before being written down as fact (send a real SIGINT to a real server process with a real pane running underneath it, check the pane's own program afterward), likely leaves running panes orphaned-but-alive (if pty slaves already become their own session leaders, which TASK-1.39/1.40's own pty setup likely already gives them for ordinary job-control reasons, a SIGINT to the server's own foreground process group would not reach them) - consistent with R2's intent even though R2 is phrased about the app, not the server. No graceful-shutdown feedback for a supervised (systemd/launchd) explicit server; an operator's Ctrl-C or `kill` gets no "stopped cleanly" confirmation, nor does the socket file or any in-progress program get the chance server.stop itself would give them.
   Alternative (cheaper than TASK-1.77's own plan assumed - that plan set this aside believing it needed `unsafe` or a new dependency; it needs neither): a `sigwait`-thread. Before any other thread exists (`main()`, or the very top of `run_server_start`, strictly before `Runtime::new` is ever called - signal masks are per-thread and inherited only by threads spawned AFTER the mask is set, so this is a real ordering constraint, not a detail), mask SIGINT and SIGTERM on the whole process with `nix::sys::signal::pthread_sigmask(SIG_BLOCK, ...)` (no thread then handles them by default disposition any more), then spawn one small, dedicated thread that blocks in `nix::sys::signal::sigwait` (a safe, ordinary blocking call - not an async-signal-context handler, so nothing async-signal-unsafe is ever run) for that same signal set, and on return calls `stopper.stop()` once. Roughly 15-25 lines, one new small module or a few lines in `main.rs`/`listen.rs`; zero new dependency (`nix`'s `signal` feature is already allowed for steg-server in boundaries.toml); zero `unsafe` in this crate's own code. Risk: getting the mask-before-spawn ordering wrong fails silently (reverts to "unhandled", not a crash) - needs its own test sending a real signal to a real server process and confirming a clean exit (status 0, socket removed), not just "didn't crash".
   This plan builds neither by default; signals are written up, not handled, unless the engineer asks for the sigwait-thread option after reading this.

2. doc-2 section 9's wording. Quoted above in full, against criterion 2 and section 8's table. The code follows criterion 2/section 8 ("any session exists"), not section 9's literal "programs are still running". Proposed: reword section 9's one sentence about server stop to match ("close and server stop refuse while [close: programs are still running / server stop: any session exists], with status 1..."). This is Bruk's/the engineer's spec change to make, not this plan's or this task's own code's.

3. Nothing else server-only found worth raising as a separate question - the version-string dedup (SERVER_VERSION) and the ServerConfig.force_grace addition are both small, mechanical, and already named above with their own reasons.

====================================================================
COLLISIONS
====================================================================
- TASK-1.90 (now pane send only: dispatch/pane.rs, runtime/input.rs; may add a new ErrorCode to crates/sdk/src/description/shapes.rs) - no overlap: this task touches neither of those two files, and adds no ErrorCode of its own (server.show/start/stop's errors - BadRequest, ProgramRunning, NotImplementedYet - all already exist in shapes.rs today).
- TASK-1.123 (replacing the duplicated unique-socket-path helpers across sixteen test files with one shared helper, plus a lock around attach.rs's flood tests) - server_stop.rs (new) mirrors the exact existing per-file pattern (idle_stop.rs's own shape) rather than inventing a stronger, divergent one, specifically so TASK-1.123's later swap-in is a drop-in replacement of the same signature, not a redesign.
- methods/tests.rs and dispatch/mod.rs's two test-only lists are shared with the separate client-methods task (client.list/client.show) by design - this task's own edits there are additive only (one new HANDLED guard, one narrowed catch-all arm for server.stop), and the client task's edits are independent additions to the same lists.
- TASK-1.116/TASK-1.109 (both later touch lifecycle.rs's stop/connect_client/disconnect_client) - named above at G; this task's three new methods sit beside, not inside, that area.

====================================================================
ORDER OF WORK
====================================================================
1. Kernel: Moment::since_origin (small, independent, unlocks server.show).
2. sdk: ServerRecord in record.rs, lib.rs export, Outcome's new variant.
3. Server, pure arms: dispatch/server.rs, mod.rs's list/guard/catch-all edits - get tests/dispatch/server.rs green before touching anything connection-level.
4. Server, plumbing: config.rs's force_grace, listen.rs's two edits, runtime/mod.rs's State field, lifecycle.rs's three methods.
5. THE PROBE (write_all then shutdown on a real UnixStream pair) - run and record its result before writing the flush primitive.
6. Server, the flush primitive: outbound.rs's ControlItem/pop_next/enqueue_control_and_wait_until_written + its own tests.
7. Server, the interception: connection.rs's new "server.stop" arm and its own unit tests.
8. Socket-level proof: crates/server/tests/server_stop.rs, with its own negative control.
9. CLI end-to-end: one new crates/steg/tests case.
10. Write up the signals probe's result and the recommendation in the task's own notes; do not build handling unless the engineer asks for it after reading this plan.
11. `just check` at every step boundary above, and once more at the end; record each.

====================================================================
PROOF, CRITERION BY CRITERION, EACH WITH A FAILING-THEN-PASSING CONTROL
====================================================================
#1 (server show): tests/dispatch/server.rs (counts, version, uptime pass-through - each field's own test fails against the not-yet-built arm, then passes once B lands) + the CLI end-to-end case (K).
#2 (server stop): connection.rs's own unit tests (the refusal path, run against the not-yet-built arm first, fails as NotImplementedYet, then passes) + server_stop.rs's full sequence including its own negative control on the flush ordering (the one case this plan names as a real, recorded failure-then-fix, not merely a construction argument) + the real force_end proof + the second-client-sees-nothing windows + the socket file gone and the lock releasable + the CLI end-to-end case (K).
#3 (signals): written up as a question with a recommendation and a probed, recorded fact about panes, not built, per the task's own wording ("the plan says what...").
#4 (server start on a running server): tests/dispatch/server.rs's start cases (fails against the not-yet-built arm, then Ok(null) always) + doc-2's own already-complete description (no error invented) + the three server methods confirmed off dispatch/mod.rs's NOT_YET_NOUNS (show/start into server::HANDLED; stop into INTERCEPTED_BEFORE_DISPATCH with its own real connection.rs arm), proven by dispatch/mod.rs's own existing completeness test (`every_described_method_has_exactly_one_handler`) passing unchanged.

`just check` (or the affected subset) recorded at each numbered step above and once more at the end, per the repository's own testing convention; every real process or thread this task's own tests start is joined/reaped on every path, with no leftover from this worktree before or after.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
The client noun's two methods had no owner either (dispatch answers NotImplementedYet, citing TASK-1.42, which built the kernel side only); they are included here as the same kind of gap. If the plan finds the server methods and the client methods are two reviewable changes, split them.

From the independent read of TASK-1.78's merge (2026-11-15): the forced close's grace period (FORCE_GRACE_DEFAULT, 2s) is a constant read directly in listen.rs and is not a field of ServerConfig beside handshake_timeout and arbitration_timeout. server stop uses that mechanism: put the grace period in ServerConfig here, with its default, so it can be set like the other timeouts.

Split by @bruk.o on 2026-11-18, as the plan recommended: this task keeps the server noun (show, stop, start on a running server, and what a signal does); client list and client show moved to a draft task. The two share only additive edits to dispatch/mod.rs's lists and the record and outcome variant lists. The plan's three questions go to the engineer with the rewritten plan: signals left unhandled for M1; last_active (the client half's); and doc-2 section 9's wording against this task's second criterion.

Plan checked against doc-2 sections 8 and 9, doc-8 R99 and the task's four criteria by @bruk.o: ok, subject to the engineer's answers. server show and server start are plain dispatch arms in a new dispatch/server.rs; only server stop is intercepted, because it needs the connection's outbound queue and the server's stopper. Its reply is written before the connection is torn down through a new acknowledged write on the control stream - a different need from TASK-1.128's end_stream, which defers a stream's own teardown and has no caller waiting - preceded by a probe of write-then-shutdown on a Unix socket. Everything after the signal to stop is the existing shutdown path the idle stop already drives: one mechanism. Two questions go to the engineer: signals (default: leave them unhandled for M1 and document what is left behind), and doc-2 section 9's wording against the second criterion. Orchestrator's caution on the signal alternative, for whoever takes it up: blocking signals for a sigwait thread changes the mask that pane programs inherit, so a shell in a pane could start with interrupt blocked unless the mask is reset at spawn; 'no unsafe, no dependency' needs that checked before it is believed.

Check passed: `cargo test -p steg-sdk -p steg-kernel` exited 0 at 2026-11-18T22:22:14+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.88 on task-1.88-server-server-show-server-stop-and-the at a44e55e51cd6 with uncommitted changes, last 40 of 539 lines:

```text
test wire::frame::tests::two_frames_back_to_back_decode_in_order ... ok
test wire::handshake::tests::a_client_newer_than_the_server_is_refused_naming_both_versions ... ok
test wire::handshake::tests::a_first_frame_that_is_not_json_is_malformed ... ok
test wire::handshake::tests::a_matching_version_is_accepted ... ok
test wire::handshake::tests::a_missing_required_field_is_malformed ... ok
test wire::handshake::tests::a_server_newer_than_the_client_is_refused_naming_both_versions ... ok
test wire::handshake::tests::a_wrong_magic_is_malformed_naming_the_server_version ... ok
test wire::handshake::tests::an_unknown_field_is_ignored ... ok
test wire::handshake::tests::kind_and_label_round_trip_through_the_hello ... ok
test wire::handshake::tests::parse_server_hello_reads_an_accepted_hello ... ok
test wire::handshake::tests::parse_server_hello_surfaces_a_refusals_message ... ok
test screen::tests::pane_attached_round_trips_and_carries_both_fields_in_one_object ... ok

test result: ok. 175 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests/connection.rs (target/debug/deps/connection-5aaf1820065b9c7d)

running 2 tests
test a_version_mismatch_is_refused_naming_both_versions ... ok
test a_client_and_server_connection_shake_hands_and_exchange_a_call_in_memory ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests/local_socket.rs (target/debug/deps/local_socket-b8a3fda317535471)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests steg_kernel

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests steg_sdk

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

Check passed: `sh -c 'cargo test -p steg-server --lib dispatch:: && cargo test -p steg-server --test dispatch'` exited 0 at 2026-11-18T22:26:43+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.88 on task-1.88-server-server-show-server-stop-and-the at a44e55e51cd6 with uncommitted changes, last 40 of 168 lines:

```text
test api::api_show_with_no_target_returns_the_whole_description ... ok
test setting::setting_show_an_unknown_name_is_no_such_target ... ok
test tab::tab_list_with_an_ambiguous_session_target_is_ambiguous_target ... ok
test tab::tab_move_with_position_reorders ... ok
test setting::setting_show_returns_one_setting_in_full ... ok
test setting::setting_unset_an_unknown_name_is_no_such_target ... ok
test tab::tab_new_makes_the_new_tab_current_by_default ... ok
test server::server_show_matches_exactly_the_fields_the_description_declares ... ok
test setting::setting_unset_reverts_to_the_default ... ok
test tab::tab_new_with_no_position_appends_at_the_end_as_before ... ok
test api::api_call_of_a_request_that_fails_validation_gives_the_same_error_a_direct_call_would ... ok
test tab::tab_new_with_no_focus_restores_a_current_tab_that_is_not_the_first ... ok
test tab::tab_new_with_a_position_past_the_end_and_no_focus_creates_nothing_and_restores_focus ... ok
test tab::tab_new_with_no_focus_restores_the_previous_current_tab ... ok
test tab::tab_show_and_tab_list_shown_status_follow_the_kernels_ranking_not_tab_order ... ok
test tab::tab_show_returns_the_tabs_record ... ok
test tab::tab_new_with_position_zero_is_refused_and_creates_nothing ... ok
test tab::tab_list_with_an_unknown_session_is_no_such_target ... ok
test tab::tab_close_removes_it ... ok
test server::server_start_always_answers_nothing_regardless_of_on_demand ... ok
test setting::setting_set_changes_the_value_the_next_read_sees ... ok
test server::server_show_reports_exactly_the_moment_its_given_as_uptime ... ok
test api::api_call_reaches_setting_unset_with_its_required_target ... ok
test tab::tab_show_with_an_unknown_target_is_no_such_target ... ok
test tab::tab_new_with_a_middle_position_places_the_new_tab_there ... ok
test pane::pane_split_with_no_focus_restores_the_previous_current_pane ... ok
test server::server_show_counts_every_session_and_client_the_tree_holds ... ok
test pane::pane_split_with_an_invalid_label_leaves_the_tree_exactly_as_it_was ... ok
test api::api_call_reaches_setting_set_with_its_required_target ... ok
test tab::tab_show_with_an_ambiguous_target_is_ambiguous_target ... ok
test tab::tab_new_with_position_one_places_the_new_tab_first ... ok
test api::api_call_of_an_unknown_method_is_refused_the_same_way_a_direct_call_would_be ... ok
test api::api_call_reaches_setting_show_with_its_required_target ... ok
test setting::an_override_is_still_there_for_a_fresh_tree_and_context_over_the_same_settings ... ok
test tab::tab_new_with_a_position_past_the_end_behaves_like_tab_move_and_creates_nothing ... ok
test cross_cutting::every_not_yet_scenario_answers_not_implemented_yet_and_changes_nothing ... ok
test cross_cutting::every_described_method_gets_a_response_with_no_panic ... ok
test api::api_call_matches_a_direct_call_for_every_served_method_with_its_own_target_given ... ok

test result: ok. 82 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
```

Check passed: `cargo test -p steg-server --lib runtime::` exited 0 at 2026-11-18T22:28:32+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.88 on task-1.88-server-server-show-server-stop-and-the at a44e55e51cd6 with uncommitted changes, last 40 of 59 lines:

```text
test runtime::login::tests::login_shell_default_is_a_fixed_zero_io_value ... ok
test runtime::lifecycle::tests::stopper_is_none_until_set_then_some ... ok
test runtime::login::tests::login_shell_resolve_identity_never_panics_and_is_never_empty ... ok
test runtime::events::tests::a_slow_subscribers_connection_is_shut_down_once_its_queue_is_full ... ok
test runtime::events::tests::a_real_exit_event_drives_the_same_cascade_apply_event_runs ... ok
test runtime::tests::a_panic_while_holding_the_lock_does_not_poison_a_later_call ... ok
test runtime::tests::runtime_new_does_not_panic_and_starts_empty ... ok
test runtime::events::tests::a_mark_for_a_pane_the_tree_no_longer_holds_does_nothing_and_does_not_panic ... ok
test runtime::events::tests::apply_event_on_a_pane_the_tree_no_longer_holds_removes_the_stale_handle_without_panicking ... ok
test runtime::input::tests::apply_size_changed_resizes_only_the_panes_the_change_names ... ok
test runtime::input::tests::apply_size_changed_lets_a_pane_go_under_the_floor_when_the_kernel_says_so ... ok
test runtime::events::tests::a_command_panes_exit_reaches_pane_show_through_runtime_handle ... ok
test runtime::attach::tests::arm_attachment_answers_false_once_the_pane_has_already_closed ... ok
kill: 31365: No such process
test runtime::lifecycle::stop_tests::stop_is_a_no_op_for_a_pane_whose_program_has_already_ended ... ok
test runtime::lifecycle::tests::disconnect_client_reapplies_the_next_latest_declared_size ... ok
test runtime::lifecycle::tests::session_count_tracks_sessions_created_and_closed_through_handle ... ok
kill: 31366: No such process
test runtime::close::tests::a_plain_close_of_an_already_ended_pane_removes_its_idle_handle ... ok
test runtime::lifecycle::stop_tests::stop_ends_a_silent_program ... ok
test runtime::events::tests::a_typed_command_still_running_stays_running_until_told_otherwise ... ok
kill: 31364: No such process
test runtime::lifecycle::stop_tests::stop_ends_a_program_producing_output ... ok
test runtime::start::tests::steg_vars_and_term_always_win_over_a_conflicting_value_from_the_login_shells_own_base_env ... ok
test runtime::start::tests::a_shell_pane_not_only_a_command_pane_gets_the_same_composed_environment ... ok
test runtime::start::tests::pane_split_falls_back_to_home_when_the_inherited_live_cwd_no_longer_exists ... ok
test runtime::start::tests::pane_split_inherits_the_live_cwd_of_the_pane_it_split_from_not_its_started_in_cwd ... ok
test runtime::attach::tests::attach_pane_on_a_busy_panes_own_worker_does_not_stall_an_unrelated_requests_lock ... ok
test runtime::events::tests::an_unfiltered_subscriber_receives_nothing ... ok
test runtime::input::tests::the_general_sizing_step_reaches_every_pane_through_split_close_and_swap ... ok
kill: 31368: No such process
test runtime::lifecycle::stop_tests::stop_ends_a_program_that_traps_the_polite_signal ... ok
test runtime::events::tests::a_mark_then_the_shells_own_real_exit_cascades_with_no_panic ... ok
test runtime::events::tests::a_shell_with_no_marks_leaves_every_status_field_unchanged_until_one_arrives ... ok
test runtime::events::tests::a_typed_command_that_fails_is_failed_with_its_exit_code ... ok
test runtime::events::tests::a_typed_commands_status_reaches_pane_show_and_tab_show_through_runtime_handle ... ok
test runtime::events::tests::a_typed_command_that_succeeds_is_running_then_finished_and_the_tab_shows_it ... ok
test runtime::events::tests::a_command_panes_own_program_printing_marks_does_not_change_its_status ... ok

test result: ok. 40 passed; 0 failed; 0 ignored; 0 measured; 169 filtered out; finished in 0.63s
```

Check passed: `cargo test -p steg-server --lib outbound::` exited 0 at 2026-11-18T22:32:11+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.88 on task-1.88-server-server-show-server-stop-and-the at a44e55e51cd6 with uncommitted changes:

```text
warning: unused import: `server::SERVER_VERSION`
  --> crates/server/src/dispatch/mod.rs:33:16
   |
33 | pub(crate) use server::SERVER_VERSION;
   |                ^^^^^^^^^^^^^^^^^^^^^^
   |
   = note: `#[warn(unused_imports)]` (part of `#[warn(unused)]`) on by default

warning: `steg-server` (lib test) generated 1 warning (run `cargo fix --lib -p steg-server --tests` to apply 1 suggestion)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.09s
     Running unittests src/lib.rs (target/debug/deps/steg_server-6d2f1d51fdd2e018)

running 16 tests
test outbound::tests::a_full_control_queue_refuses_rather_than_growing_without_bound ... ok
test outbound::tests::a_broken_write_closes_the_queue_so_the_readers_next_enqueue_fails ... ok
test outbound::tests::closing_drains_what_is_already_queued_then_the_writer_returns ... ok
test outbound::tests::a_stream_with_nothing_queued_is_skipped_not_given_an_empty_turn ... ok
test outbound::tests::a_full_data_queue_drops_the_new_frame_rather_than_growing_or_blocking ... ok
test outbound::tests::enqueue_control_and_wait_until_written_returns_promptly_once_the_queue_is_already_closed ... ok
test outbound::tests::control_is_drained_before_any_data_even_when_data_was_enqueued_first ... ok
test outbound::tests::data_streams_are_drained_round_robin_one_frame_at_a_time ... ok
test outbound::tests::enqueue_data_on_a_closed_stream_is_refused_the_same_way ... ok
test outbound::tests::enqueue_data_on_a_never_opened_stream_is_refused ... ok
test outbound::tests::end_stream_still_delivers_the_final_frame_when_the_queue_was_already_full ... ok
test outbound::tests::enqueue_control_and_wait_until_written_delivers_the_frame_over_a_real_writer ... ok
test outbound::tests::end_stream_tears_the_stream_down_only_once_the_writer_actually_drains_it ... ok
test outbound::tests::enqueue_refuses_once_closed ... ok
test outbound::tests::a_genuine_flood_on_a_data_stream_never_delays_a_control_reply ... ok
test outbound::tests::enqueue_control_and_wait_until_written_does_not_return_until_write_all_itself_returns ... ok

test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 196 filtered out; finished in 0.36s
```

Check passed: `cargo test -p steg-server --lib` exited 0 at 2026-11-18T22:34:57+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.88 on task-1.88-server-server-show-server-stop-and-the at a44e55e51cd6 with uncommitted changes, last 40 of 227 lines:

```text
test pane::attach_tests::attaching_to_a_pane_that_already_ended_gets_a_final_full_then_ended ... ok
test runtime::tests::a_panic_while_holding_the_lock_does_not_poison_a_later_call ... ok
test runtime::tests::runtime_new_does_not_panic_and_starts_empty ... ok
test signal::tests::send_reaches_a_real_process_with_the_named_signal ... ok
test runtime::start::tests::a_shell_pane_not_only_a_command_pane_gets_the_same_composed_environment ... ok
test runtime::attach::tests::attach_pane_on_a_busy_panes_own_worker_does_not_stall_an_unrelated_requests_lock ... ok
test test_support::tests::a_body_that_returns_promptly_is_not_delayed ... ok
test runtime::start::tests::pane_split_inherits_the_live_cwd_of_the_pane_it_split_from_not_its_started_in_cwd ... ok
test runtime::start::tests::pane_split_falls_back_to_home_when_the_inherited_live_cwd_no_longer_exists ... ok
kill: 35232: No such process
test runtime::start::tests::steg_vars_and_term_always_win_over_a_conflicting_value_from_the_login_shells_own_base_env ... ok
test signal::tests::send_reaches_the_whole_process_group_not_only_the_leader ... ok
test login_env::tests::capture_login_environment_falls_back_on_a_shell_that_never_finishes_in_time ... ok
test pane::attach_tests::attaching_returns_the_panes_current_screen_and_updates_follow_it ... ok
test pane::attach_tests::a_program_that_exits_on_its_own_sends_ended_with_its_real_exit_code_after_its_last_content ... ok
test pane::attach_tests::mark_closing_sent_before_a_programs_own_exit_is_the_reason_ended_reports ... ok
test pane::ending_tests::await_exit_is_not_delayed_by_a_backlog_on_the_workers_own_queue ... ok
test runtime::events::tests::an_unfiltered_subscriber_receives_nothing ... ok
test runtime::input::tests::the_general_sizing_step_reaches_every_pane_through_split_close_and_swap ... ok
kill: 35049: No such process
test runtime::lifecycle::stop_tests::stop_ends_a_program_that_traps_the_polite_signal ... ok
test test_support::tests::a_body_that_never_returns_fails_fast_naming_what_it_waited_for - should panic ... ok
test pane::ending_tests::signal_hangup_reaches_a_backgrounded_grandchild_too ... ok
test runtime::events::tests::a_mark_then_the_shells_own_real_exit_cascades_with_no_panic ... ok
test idle::tests::the_checker_blocks_with_no_timeout_while_ineligible_and_waits_exactly_once_while_eligible ... ok
test outbound::tests::enqueue_control_and_wait_until_written_does_not_return_until_write_all_itself_returns ... ok
test runtime::events::tests::a_shell_with_no_marks_leaves_every_status_field_unchanged_until_one_arrives ... ok
test pane::ending_tests::signal_kill_ends_a_program_that_traps_the_polite_signal ... ok
test runtime::events::tests::a_typed_command_that_fails_is_failed_with_its_exit_code ... ok
test runtime::events::tests::a_typed_command_that_succeeds_is_running_then_finished_and_the_tab_shows_it ... ok
test runtime::events::tests::a_typed_commands_status_reaches_pane_show_and_tab_show_through_runtime_handle ... ok
test login_env::tests::capture_login_environment_falls_back_on_a_shell_that_exits_non_zero ... ok
test login_env::tests::capture_login_environment_prints_noise_before_the_marker_without_corrupting_the_dump ... ok
test runtime::events::tests::a_command_panes_own_program_printing_marks_does_not_change_its_status ... ok
kill: 35312: No such process
test login_env::tests::capture_login_environment_kills_a_backgrounded_grandchild_that_held_the_pipe_open ... ok
kill: 35019: No such process
test pane::tests::a_forward_failing_once_the_worker_is_gone_still_reaps_a_program_that_ends_on_its_own ... ok

test result: ok. 214 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.05s
```

Check passed: `cargo test -p steg-server --test server_stop` exited 0 at 2026-11-18T22:38:46+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.88 on task-1.88-server-server-show-server-stop-and-the at a44e55e51cd6 with uncommitted changes:

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.09s
     Running tests/server_stop.rs (target/debug/deps/server_stop-3b19889852620df0)

running 11 tests
test support::tests::candidate_name_does_not_collide_under_the_same_forced_pid_and_counter ... ok
test support::tests::the_old_pid_and_counter_only_scheme_collides_when_two_processes_share_a_pid ... ok
test support::tests::a_fresh_scoped_temp_dir_is_0700 ... ok
test support::tests::path_panics_rather_than_hand_back_something_over_the_sun_path_budget - should panic ... ok
test support::tests::scoped_temp_dir_exists_after_new_and_is_gone_after_drop ... ok
test support::tests::create_unique_dir_skips_a_name_that_already_exists ... ok
test support::tests::two_scoped_temp_dirs_made_back_to_back_are_different ... ok
test server_stop_with_no_sessions_flushes_the_reply_then_stops_and_a_second_client_sees_no_server ... ok
test server_stop_without_force_while_a_session_exists_refuses_and_the_server_keeps_running ... ok
kill: 51379: No such process
test server_stop_with_force_ends_a_real_running_programs_process ... ok
test support::tests::wait_child_bounded_kills_and_reaps_a_child_that_outlives_the_bound ... ok

test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.31s
```

Check passed: `cargo test -p steg --test server_show_stop` exited 0 at 2026-11-18T22:41:10+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.88 on task-1.88-server-server-show-server-stop-and-the at a44e55e51cd6 with uncommitted changes:

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.09s
     Running tests/server_show_stop.rs (target/debug/deps/server_show_stop-412642b217cdfff8)

running 9 tests
test support::tests::candidate_name_does_not_collide_under_the_same_forced_pid_and_counter ... ok
test support::tests::the_old_pid_and_counter_only_scheme_collides_when_two_processes_share_a_pid ... ok
test support::tests::a_fresh_scoped_temp_dir_is_0700 ... ok
test support::tests::one_scoped_temp_dir_can_hand_out_two_different_paths ... ok
test support::tests::path_panics_rather_than_hand_back_something_over_the_sun_path_budget - should panic ... ok
test support::tests::scoped_temp_dir_exists_after_new_and_is_gone_after_drop ... ok
test support::tests::two_scoped_temp_dirs_made_back_to_back_are_different ... ok
test support::tests::create_unique_dir_skips_a_name_that_already_exists ... ok
test server_show_and_server_stop_work_through_the_real_binary ... ok

test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.34s
```

Check passed: `just check` exited 0 at 2026-11-18T22:57:02+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.88 on task-1.88-server-server-show-server-stop-and-the at 2542e1145dc1:

```text
ok      format check (0.3s)
ok      lint (0.4s)
ok      boundaries (0.1s)
ok      foundations (0.1s)
ok      foundations public surface (0.0s)
ok      design pages (0.0s)
ok      licences (0.7s)
ok      tests (-p steg -p steg-app -p steg-kernel -p steg-sdk -p steg-server) (56.6s)
ok      build (steg-sdk) (0.1s)
ok      test (steg-sdk) (0.2s)
ok      test (steg-sdk, local-socket) (0.4s)
ok      build (steg-kernel) (0.1s)
ok      test (steg-kernel) (0.2s)
ok      build (steg-server) (0.1s)
ok      test (steg-server) (52.4s)
ok      test (steg-server, race-fixture) (53.4s)
ok      test (steg-server, pty-race-fixture) (65.3s)
ok      test (steg-server, accept-backoff-fixture) (85.0s)
ok      build (steg) (0.8s)
ok      test (steg) (4.0s)
ok      test (steg, fixtures) (8.4s)
ok      build (steg-ui) (14.5s)
ok      test (steg-ui) (1.2s)
ok      build (steg-app) (1.4s)
ok      test (steg-app) (2.1s)
ok      tool tests (40.6s)
ok      architecture (0.7s)
ok      recipe names (0.1s)
ok      ui levels (0.1s)
ok      ui colours (0.1s)
```

Check FAILED: `just check` exited 101 at 2026-11-18T23:03:51+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.88 on task-1.88-server-server-show-server-stop-and-the at 285d5adf822b, last 40 of 80 lines:

```text
test a_dropped_connection_releases_its_claim_the_same_way ... ok
test two_connections_attached_to_one_pane_each_get_their_own_updates_and_a_detach_does_not_touch_the_other ... ok
test repeated_attach_and_detach_against_a_flooding_pane_never_sees_an_unknown_stream ... ok
test support::tests::wait_child_bounded_kills_and_reaps_a_child_that_outlives_the_bound ... ok
test attaching_to_a_pane_whose_program_already_ended_gets_a_final_full_then_ended ... ok
test pane_close_force_reports_ended_closed_not_the_signals_own_exit_code ... ok
test a_program_that_exits_on_its_own_sends_ended_with_its_real_exit_code ... ok
test detach_ends_the_stream_even_though_the_program_keeps_producing_output ... ok
test two_clients_attached_to_one_pane_both_get_ended_for_the_same_exit ... ok
test detaching_releases_the_claim_and_the_survivor_returns_to_its_own_declared_size ... FAILED
test a_declared_size_reaches_the_pty_and_is_reported_on_every_attached_stream ... FAILED
test a_second_connections_control_call_is_not_blocked_by_a_stuck_panes_lock_held_attach ... ok
test a_pane_that_closes_between_a_queued_attach_and_its_arm_leaves_nothing_to_detach ... ok
test pane_close_force_returns_within_its_bound_even_while_the_panes_own_worker_is_stuck ... ok
test stopping_the_server_returns_within_its_bound_even_while_a_panes_worker_is_stuck ... ok
test an_unresponsive_panes_own_commands_do_not_stall_the_connection_or_another_pane has been running for over 60 seconds
test an_unresponsive_panes_own_commands_do_not_stall_the_connection_or_another_pane ... ok

failures:

---- detaching_releases_the_claim_and_the_survivor_returns_to_its_own_declared_size stdout ----

thread 'detaching_releases_the_claim_and_the_survivor_returns_to_its_own_declared_size' (63185864) panicked at crates/server/tests/attach.rs:464:17:
no update arrived on stream 1 within 5s

---- a_declared_size_reaches_the_pty_and_is_reported_on_every_attached_stream stdout ----

thread 'a_declared_size_reaches_the_pty_and_is_reported_on_every_attached_stream' (63185851) panicked at crates/server/tests/attach.rs:464:17:
no update arrived on stream 1 within 5s


failures:
    a_declared_size_reaches_the_pty_and_is_reported_on_every_attached_stream
    detaching_releases_the_claim_and_the_survivor_returns_to_its_own_declared_size

test result: FAILED. 29 passed; 2 failed; 0 ignored; 0 measured; 0 filtered out; finished in 60.27s

error: test failed, to rerun pass `-p steg-server --test attach`
full output: .quiet/test-steg-server-pty-race-fixture.log
error: recipe `check-packages` failed with exit code 101
```
<!-- SECTION:NOTES:END -->

## Comments

<!-- COMMENTS:BEGIN -->
author: @bruk
created: 2026-11-18T22:17:00Z

---

Plan approved by @bruk (c42e82b251d6, size-3): its worker may build it.

---
<!-- COMMENTS:END -->
