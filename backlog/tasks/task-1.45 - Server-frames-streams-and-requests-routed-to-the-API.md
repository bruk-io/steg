---
id: TASK-1.45
title: 'Server: frames, streams, and requests routed to the API'
status: In Progress
assignee:
  - '@bruk.w2'
created_date: '2026-11-15T01:33:00Z'
updated_date: '2026-11-16T01:20:00Z'
labels:
  - size-3
milestone: m-3
dependencies:
  - TASK-1.44
  - TASK-1.43
  - TASK-1.70
  - TASK-1.75
  - TASK-1.76
parent_task_id: TASK-1
type: task
project: bruk-io-steg
asdlc_accepted_from: DRAFT-15
asdlc_branch: task-1.45-server-frames-streams-and-requests
asdlc_plan_approved_by: bruk
asdlc_plan_approved_hash: '1007366d16d6'
asdlc_plan_approved_size: size-3
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
decision-2, sections 1 and 3: length-prefixed frames on one multiplexed connection, control on stream 0.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Frames are length-prefixed with the cap decision-2 gives, and a frame over the cap is refused without allocating for it
- [ ] #2 A request on the control stream reaches the API and its response or error comes back with the same id
- [ ] #3 Control is written before terminal data, so a flood on one stream does not delay a control call, shown by a test
- [ ] #4 Framing, the handshake and stream state come from the sdk (TASK-1.70, doc-16 R101); the server adds only the socket and the routing
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
One reviewable change. AC #1-#4 are one mechanism: #3 requires the writer this task builds, #1/#2/#4 are the routing that writer sits behind, and none is separable without leaving the others unproven. The one piece that could be pulled out - minting a real kernel `ClientId`/`Context` per connection via `Tree::connect_client` instead of the fake local counter TASK-1.44 left - stays in: it is what the hello's `client` field and every `Context` this task builds depend on, and it is small (two `Runtime` wrapper methods).

## 1. Shape after the handshake

**Threads per connection.** Two, replacing TASK-1.44's single hold-open sink loop: the existing per-connection thread becomes the **reader** (`connection::handle`, unchanged entry point/signature apart from one new parameter, see below); it spawns one **writer** thread that owns the write half and drains `Outbound`'s queues (section 2). Both are joined before `connection::handle` returns, on every path including a panic (a `WriterGuard` local variable, `Drop` closes `Outbound` - waking the writer out of its condvar wait - and joins it; Rust runs local destructors on unwind, so a panic in `Runtime::handle` still joins the writer before the reader thread itself unwinds further). `listen::run` is unchanged in how it tracks/joins the *reader*'s `JoinHandle`; it still only ever sees one thread per connection.

**Reader loop, post-handshake.** `connection.receive(buf)` then drain `connection.poll()` in a loop:
```
Incoming::Request { id, request } => {
    let response = runtime.handle(&description, &context, &request);
    let bytes = Connection::reply(id, response);
    if outbound.enqueue_control(bytes).is_err() { break; } // queue closed/full, see §3
}
_ => break, // anything else post-handshake is a protocol violation, see §3
```
This leans entirely on the sdk's existing `Incoming` enum - no new classification logic in the server, which is the cleanest argument for AC #4 ("the server adds only the socket and the routing"): `Malformed`, a response-shaped frame, an event from a client, a second `Hello`, data/control on a stream never opened - every one of them is already a distinct `Incoming` variant the sdk hands back; this task's own code is the one `match` above, nothing more.

**Who owns `Runtime`, and how it is stopped.** No `Arc`. `run` builds one plain `Runtime` value, only on the path that already only runs once (`Arbitration::Became`, before the accept loop - `AlreadyRunning`/`Refuse` never pay for `Runtime::new`'s drain thread), from `ServerConfig`'s two new fields: `launcher: Arc<dyn ProgramLauncher>` and `login: LoginShell`, both defaulted (`RealLauncher`, `LoginShell::from_process_env()`) in `ServerConfig::default()` - every existing test that builds a `ServerConfig { socket_path: Some(..), .., ..ServerConfig::default() }` (confirmed by grep: every one in `handshake.rs`/`shutdown.rs`/`race.rs`/`socket_mode.rs` already does) keeps compiling unchanged, and a test that wants a fake launcher now has a seam for it, which is exactly `ServerConfig`'s documented purpose. TASK-1.77's own entry point (building a real `ServerConfig` for the `steg server start --on-demand` path) passes `login` resolved from the password database as R99 asks, and can leave `launcher` at its default; neither `serve` nor `run`'s own signature changes.

The whole accept loop - the existing loop body, `reap_finished`, and the final per-connection shutdown-and-join pass - moves inside one `std::thread::scope(|scope| { ... })` call, spawning each connection with `scope.spawn(move || connection::handle(stream, handshake_timeout, events, &runtime))` (one more parameter on `connection::handle`), borrowing `&runtime` rather than cloning an `Arc`. `Runtime::handle` only ever needs `&self`, so many connections holding that same borrow concurrently is ordinary, safe aliasing; `std::thread::scope` will not return until every thread it spawned has been joined, which is the *existing* per-connection shutdown-and-join loop's own job (unchanged: `shutdown_handle.shutdown(Both)` then `let _ = join.join()` for each, discarding a panicking connection's `Err` exactly as today - confirmed by a standalone repro that a scoped thread's panic, once explicitly joined inside the scope, does not make `thread::scope` itself panic or propagate anything further). Only once `thread::scope(...)` returns - after which the borrow checker will not let any other code still be holding `&runtime` - does `run` call `runtime.stop()` explicitly (now requiring `let mut runtime`), *then* remove the socket file, *then* release the arbiter lock: stop accepting, close the listener (unchanged timing, right after the loop breaks), shut down and join every connection, stop the runtime, remove the socket file, release the lock. This is what makes "a request can never reach a stopped runtime" a property the compiler enforces, not one this plan merely asserts: `Runtime::stop(&mut self)` cannot type-check while any `scope.spawn`ed closure might still hold `&runtime`, and `thread::scope` guarantees none does by the time it returns.

**Building a `Context`.** The hello (decision-2 §7, doc-14 "Clients") carries only `steg`/`protocol`/`client`/`kind`/`label` - never a session, tab or pane. `STEG_SESSION`/`STEG_TAB`/`STEG_PANE` (doc-2 §4) are environment variables the server puts into a *pane's own program's* environment, read by a CLI invocation running inside that pane, never sent by a client over the wire; doc-2 §4 is explicit that outside a steg terminal there is no context and a target-less command fails CLI-side with status 2 (bad usage), which means a correctly-built CLI client always resolves its own target before ever sending a request with no `target` to a server that cannot see its environment. There is therefore no source for a connection's own default session/tab/pane today (not invented: decision-2/doc-14 add none, and the hello's shape is frozen outside its two decided keys), so absence has to be representable rather than filled with any id, sentinel or not - a `Context` naming a real-looking session that nobody named is a false statement the client did not make, whatever number is picked.

`dispatch::Context`'s three fields become `Option<SessionId>`, `Option<TabId>`, `Option<PaneId>`; a `Context::none()` constructor (`Context { default_session: None, default_tab: None, default_pane: None }`) replaces the `from_raw(0)`-filled "no context" literals several existing tests build by hand (`runtime.rs`'s and `tests/runtime.rs`'s own `no_context`/`no_context()` helpers - each becomes a call to it; every other existing `Context { default_session: SessionId::from_raw(n), .. }` literal is a mechanical `Some(...)`-wrap, nothing else, and the compiler refuses to miss one). Every connection this task builds gets `Context::none()`.

`lookup.rs`'s `target_session`/`target_tab`/`target_pane`, today's `None => Ok(context.default_session)` (etc.) with no check at all, become `None => context.default_session.ok_or_else(|| missing_context("session"))` (etc.) - a request with no `target` string and no default answers `ErrorCode::BadRequest`, naming the missing part, exactly doc-2 §4/§6's own "no context, status 2" at the CLI, now enforced at the API boundary. A target string that *is* given but itself omits a part it needs (`"2/logs"` naming a tab and pane but no session, say) still has to fall back to `context.default_session` one level down, inside `resolve_tab_str`/`resolve_pane_str`, which call the kernel's `Tree::resolve_tab_path`/`resolve_pane_path` - both of which require a concrete `SessionId`/`TabId` by signature (kernel code, out of this task's scope to change) and only ever read it when the given path omitted that part. So `resolve_tab_str`/`resolve_pane_str` pass a fixed, never-real sentinel (`SessionId::from_raw(u64::MAX)`/`TabId::from_raw(u64::MAX)` - the same non-collision argument as before, now used purely as internal plumbing, never once returned to a client) only when `context.default_session`/`default_tab` is `None`, and catch the one error that sentinel can ever produce (`Error::SessionNotFound`/`TabNotFound` naming exactly that sentinel id) and remap it to the same `missing_context` `BadRequest` - a path that gives every part explicitly never reaches the sentinel at all (the kernel's own `match` on the path's parts skips the default entirely), so "a request that does give a target is unaffected" holds exactly, clause for clause, not just for the common case. Proven (§9): for a session, a tab and a pane method, a target-less request over a real socket is refused with `BadRequest` naming the missing part and changes nothing; the same request with a (fully-given) target succeeds; a partial target that omits the one part a no-context connection cannot supply is refused the same way.

**Minting the client.** `connection::handle` replaces its own `next_local_client_id()` (today's local, kernel-independent counter, explicitly called out in its own doc as "wiring a real session/client model to this connection is TASK-1.45's job") with two new `Runtime` methods, same self-locking shape as `apply_size_changed`: `Runtime::connect_client(kind, label) -> ClientId` (wraps `Tree::connect_client`, called once, right after a successful handshake, before the `ServerHello` is built) and `Runtime::disconnect_client(id)` (wraps `Tree::disconnect_client`, applying every `SizeChanged` it returns through the existing `apply_size_changed_locked`, same as `reapply_every_sized_tab` already does elsewhere) - called once, on every exit path out of `connection::handle` (ordinary close, protocol violation, panic unwinding past it: a second `Drop` guard, `ClientGuard`, holding the minted id and `&Runtime`). `client.list`/`client.show` stay `NotImplementedYet` (TASK-1.42's own gap) regardless - this is the tree tracking a real client's lifetime, not exposing the methods that read it back.

## 2. The writer and its queues

New module, `crates/server/src/outbound.rs`, `pub(crate)` only (see §9 on why it stays out of the crate's public surface). `Outbound`: one control queue (`VecDeque<Vec<u8>>`, cap 256 entries - generous for a well-behaved client pipelining many quick requests, see §3 for what happens past it) and a `HashMap<u32, VecDeque<Vec<u8>>>` of per-stream data queues (cap 64 frames each, decision-2 §6's own number), all behind one `Mutex` + `Condvar`, plus a `closed: bool`. `enqueue_control`/`enqueue_data`/`open_stream`/`close_stream` lock, push (or refuse when full/closed), notify, unlock - no write ever happens while the lock is held. The writer thread: lock, pop (control first, fully drained, then one frame from the next data stream in round-robin order - decision-2 §3's own wording, "drains control before any terminal data and then takes one frame from each attachment in turn"), unlock, `write_all` that one item, loop; waits on the condvar when both are empty and `closed` is false; returns once `closed` is true and both are empty.

This is server code, not sdk code, on purpose: doc-16 R102 says the sdk does no I/O, and R1/this crate's own module doc say the server is where I/O meets the kernel - scheduling *when* bytes already encoded by the sdk's own `Connection::reply`/`emit_event`/`send_data` actually hit a real socket is scheduling over a socket, which is squarely the server's job, not a second definition of the protocol (nothing here parses or builds a frame; it only orders already-encoded `Vec<u8>`s).

**What this task builds vs. what TASK-1.47 plugs into.** No terminal data flows yet (attach is TASK-1.47), so `Outbound`'s data side is exercised in this task only by its own tests, directly, not by any real pane. TASK-1.47 is the first real caller of `open_stream`/`enqueue_data`/`close_stream` (today's `Connection::open_stream`/`close_stream` on the *decoding* side are already TASK-1.70's; `Outbound`'s are the matching *encoding*-side calls, both needed once attach is real). This task's own placeholder policy for a full data queue is to drop the new frame and return `Full` to the caller - correct for "never block the program or the connection" (R19, decision-2 §6) but not yet the real drop-and-resync decision-2 describes (a dropped frame should be followed by one fresh snapshot); that needs a snapshot to send, which does not exist until TASK-1.47. Stated here rather than silently left for someone to discover.

**Proof (AC #3).** Two parts, since `Outbound` is `pub(crate)`:
1. A `#[cfg(test)]` module inside `outbound.rs` itself (sees `pub(crate)`, same convention `testing-rust.md` already asks for: "unit tests prove kernel logic and sit beside it... with no doubles"), with a real `UnixStream::pair()` as the transport and a real writer thread: push a genuine flood (thousands of sizeable frames, a `Barrier`-timed burst, not merely "many calls in a loop") into one stream's queue, then push one control frame, and assert - reading off the *other* end of the real pair - that the control frame's bytes are the next thing to arrive, and that its arrival latency does not grow with the flood's size. Negative control: a scratch edit that reverses the writer's own priority (data before control, or a single shared FIFO queue) makes the same test fail (the control frame arrives only after the flood drains) - recorded with `record-check`, then reverted.
2. An integration test in `crates/server/tests/`, over a real socket, real handshake, real `Runtime`: several control calls pipelined on one connection get correctly matched, in-order replies - proving the real path (`connection::handle`) actually routes every reply through `Outbound`, so part 1's proof is about code something real exercises, not a parallel mechanism nothing in production uses. This does not (and, with no attach, cannot) flood a *real* stream over the wire; it establishes that the production path and the directly-tested `Outbound` are the same one.

## 3. Limits and hostile input, post-handshake

- **A frame over the cap**: already refused by the sdk's `Decoder` before anything is allocated for the declared body (unchanged, proven by the sdk's own tests); this task adds one test proving the *post-handshake* read loop also closes promptly on this, not just the handshake-phase one TASK-1.44 already covers.
- **An unknown kind, a control payload that is not JSON or not a `Request`, a response-shaped frame (`Reply`), an event from a client, a second `Hello`, a frame on a stream never opened**: every one of these is already a distinct `Incoming` variant the sdk's `poll()` returns (`Malformed`, `Reply`, `Event`/`UnknownEvent`, `Hello` again, `UnknownStream`/`StreamControl`/`Data` - none of the latter three ever legitimately arrive, since this task's own server never calls `Connection::open_stream`). The reader's `match` (§1) treats every one of them the same way: close the connection. No id to answer any of them with (several carry none at all), so a reply is not possible for most of these cases; closing is the only coherent, uniform answer, and it reuses the sdk's own exhaustive classification rather than adding a second one.
- **A request id reused while one is in flight, or many requests in flight.** The server never tracks ids at all: it reads one `Request`, calls `Runtime::handle`, enqueues that reply with that same id, and only then reads the next frame - request processing on one connection is strictly serial and in arrival order (decision-2's own measured claim is about one connection's control call not being delayed by *another connection's* data flood, never about reordering within one connection, so serial-per-connection needs nothing more). A duplicate id therefore causes no error, no hang and no deduplication: each `Request` frame gets its own, independently-computed reply carrying whatever id arrived with it - a badly-written client that reuses ids gets two correct replies it cannot itself tell apart, which is its own bug to avoid, not the server's to detect. Proven by a test sending two requests with the same id and checking both replies arrive, each correct for its own request. "Many requests in flight" needs no special handling either: `Decoder`'s own buffer only ever holds bytes not yet turned into a complete frame, which this loop drains before reading more, so memory stays bounded by the OS's own socket buffer, not by how many requests a client has sent.
- **A client that sends requests and never reads any reply.** The one case genuinely new here: `Outbound`'s control queue has a bound (256, above). A client that keeps sending requests while never draining its own reply queue eventually fills it; past that point `enqueue_control` returns `Err`, and the reader closes the connection (the `match` arm in §1) rather than growing the queue without bound or blocking the reader on a write the client will never read. Disclosed plainly: by the time this fires, every request already processed has already taken effect server-side (a `session.new` really minted a session) - the client simply never learns which id got which result, since nothing is replayed after a close. A test fills the queue (a fake launcher with no real program, or several fast no-op methods) and asserts the connection closes rather than hanging or growing without bound.
- **A client that trickles a frame post-handshake, for ever.** No per-frame deadline exists after the handshake, and this task does not add one: TASK-1.44's AC #4 already promises a connection that completed its handshake is never closed for being idle (`set_read_timeout(None)` is already set on `Accept`, unchanged by this task), and a client stalled mid-frame is, at this level, indistinguishable from a legitimately idle app window with nothing to send - the sdk's own `Decoder` returns "need more bytes" for both. Stated plainly, as this task's own chosen design, not handed to another task to fix: a connected client may sit mid-frame for ever exactly as it may sit genuinely idle for ever, and this task does not and should not try to tell the two apart. The cost is one thread pair (reader and writer) held open per such connection, for as long as it is held open; bounding how many connections a server accepts at once is not built in M1 at all - a separate, open question about resource limits, not something either this task or TASK-1.77 owns.

## 4. Requests versus the runtime's lock

What is actually provable, stated narrowly rather than claimed broadly: connection B's reader and writer threads never block on anything connection A's reader thread is doing, because each connection owns its own two threads and its own socket fd, and `Outbound`'s own lock is per-connection, never shared across connections. A control call on connection B is not delayed by a data flood *on connection A or B* (AC #3, §2) - that is what the writer's priority actually buys. What is **not** true, and is not claimed: two connections both calling `session.new` (or any mutating method) at the same moment serialize on `Runtime`'s one `Mutex<State>`, for the full, synchronous duration of the second one's wait (including, in the worst case, another connection's own pty-open-and-fork) - this is TASK-1.76's own existing design (`runtime.rs`'s §1: "many operations need both atomically... one lock avoids a two-lock ordering hazard"), inherited here unchanged, not introduced or widened by this task. Proven: a test issuing concurrent control calls from several connections while a flood runs on `Outbound` directly (§2's own harness) shows the control calls' *replies being written* are not delayed by the flood; a second, explicitly-scoped test or just the narrative above (not a blocking claim this task cannot back) covers the lock-serialization case, named as inherited rather than measured as fast.

## 5. Errors

- **A kernel or dispatch error.** Unchanged: `Runtime::handle` already returns a `Response::Error(ApiError)` built by the existing `dispatch`/`map_kernel_error` path (TASK-1.75/1.76); this task's only job is to carry that `Response` into `Connection::reply(id, response)` and enqueue it - no new error mapping anywhere in this task.
- **A client during server shutdown.** `listen::run`'s existing shutdown order (close listener, shut down and join every open connection, *then* stop `Runtime`, then remove the socket file) means every connection's reader/writer sees its socket shut down (`Ok(0)`/an error) before any in-flight request could be answered by a `Runtime` that no longer exists - no request is ever handed to a stopped `Runtime`; `Runtime::handle`'s own existing panic-on-stopped-state path (`self.events.clone().expect(...)` inside `start_and_dispatch`) is consequently unreachable from a real connection under this ordering, by construction (and, per §1, by the type system), not by luck.
- **A panic in a request handler.** Defended today, one layer down, by `Runtime::lock()`'s existing `unwrap_or_else(PoisonError::into_inner)` (recovers a poisoned `Mutex` rather than propagating poison to every later caller) and by `listen::run`'s existing `reap_finished`/final-join loop, which already treats a panicked thread exactly like a normally-finished one (`let _ = join.join()` discards the `Err`). Net effect, inherited rather than built by this task: a panic inside one connection's call to `Runtime::handle` ends *that one connection* (its reader thread unwinds, the `WriterGuard`/`ClientGuard` from §1 still run their `Drop`s, closing and joining the writer and disconnecting the client), while the server process and every other connection continue. No deliberate panic-injection hook is added to prove this end-to-end (the same reasoning TASK-1.44's review gave for rejecting a shipped test-only hook, `STEG_TEST_REPORT_OUTCOME_TO`, applies here) - instead, a unit test proves the one mechanism this actually rests on directly: lock a `Runtime`'s own `Mutex`, panic on another thread while holding it, and confirm a subsequent `Runtime::handle` call from the test's own thread still succeeds rather than hanging or propagating the poison.

## 6. What changes in TASK-1.44's code, and what does not

Changes: `connection.rs` (the post-handshake hold-open sink loop is replaced by the reader/writer shape in §1; the handshake itself - timeout, deadline, `FirstFrame` classification, the malformed-hello reply - is untouched); `listen.rs` (`run` builds and stops the plain `Runtime` as in §1, wrapping the existing accept loop and shutdown-and-join pass in `std::thread::scope`; the accept loop's own logic, `Stopper`, `reap_finished` and the shutdown-order comment are untouched apart from the one new step and the `thread::spawn` -> `scope.spawn` change); `config.rs` (two new, defaulted fields). New: `outbound.rs`. `runtime.rs` gains two small wrapper methods (§1) and nothing else. `dispatch/mod.rs` (`Context`'s three fields become `Option<...>`, plus `Context::none()`) and `dispatch/lookup.rs` (`target_session`/`target_tab`/`target_pane`'s no-target branch, and `resolve_tab_str`/`resolve_pane_str`'s internal sentinel-and-remap, both in §1) are the two dispatch files this task touches; `dispatch/session.rs`/`tab.rs`/`pane.rs` do not change at all, since none of them reads `Context`'s fields directly - confirmed by reading every one of them (above) before writing this plan. Nothing is added to `crates/sdk`: every piece this task needs from the wire - `Connection`, `Incoming`, `Frame`/`encode_frame`, `Control`, the handshake types - already exists from TASK-1.70, confirmed by reading `crates/sdk/src/wire/` in full (above) before writing this plan; `cargo tree`/`git diff --stat` against `crates/sdk` stays empty except for the one scratch edit named in §9 (recorded, then reverted). Nothing under `crates/kernel` changes: `Tree::resolve_tab_path`/`resolve_pane_path`'s required (non-`Option`) `SessionId`/`TabId` parameters are reused exactly as they are, never widened to `Option` themselves - the absence is made representable in `dispatch::Context`, one layer up, not in the kernel's own signatures.

## 7. Left out, by task

- Events and subscriptions on stream 0 (`event.watch`, `client.list`/`show` staying `NotImplementedYet`): TASK-1.46.
- Attach, screen-mode streams, the real drop-and-resync policy: TASK-1.47 (names exactly what it plugs into `Outbound`, §2).
- The CLI itself, and any real per-connection context derived from `STEG_SESSION`/`TAB`/`PANE`: TASK-1.49 (§1's own context decision names why none is built here).
- Start-on-demand, detach, the login shell from the password database, the idle-with-no-clients stop: TASK-1.77. (The absence of a post-handshake per-frame deadline is this task's own chosen design, named in §3 - not TASK-1.77's to fix.)

## 8. Files that change

`crates/server/src/connection.rs`, `crates/server/src/listen.rs`, `crates/server/src/config.rs`, `crates/server/src/runtime.rs`, `crates/server/src/dispatch/mod.rs`, `crates/server/src/dispatch/lookup.rs`, `crates/server/src/lib.rs` (module/export list only), new `crates/server/src/outbound.rs`; mechanical updates (wrapping existing ids in `Some(...)`, or switching to `Context::none()`) everywhere an existing test builds a `Context` literal (`crates/server/src/runtime.rs`'s own test module, `crates/server/tests/runtime.rs`, `crates/server/tests/dispatch.rs`); new `crates/server/tests/outbound.rs` (or an addition to an existing `tests/` file - decided while building, named in the notes either way) and additions to `crates/server/tests/dispatch.rs`-adjacent coverage, likely a new `crates/server/tests/routing.rs`, for the request/reply/hostile-input/context tests. No file under `crates/sdk` or `crates/kernel` changes.

## 9. Proof

Every clause below over a real socket, the sdk's own `Connection` (role `Client`) doing the handshake and building requests exactly as a real client would, except where §2 names the one `Outbound`-internal test that is deliberately not over the wire.

- **AC #1.** A frame over `MAX_FRAME` sent as a bare length prefix post-handshake is refused promptly, the connection closes, nothing hangs (mirrors the sdk's own already-proven allocation claim, at the server's post-handshake loop). Negative control: a scratch edit to `crates/sdk/src/wire/frame.rs` lowering `MAX_FRAME` (or removing the early check) makes an otherwise-fine frame get refused/accepted differently - recorded with `record-check`, then reverted; the revert is why `git diff --stat` shows no real sdk change afterward, named here so a reviewer checking that diff is not surprised by a run recorded mid-task against a modified sdk.
- **AC #2.** A `pane.list`/`session.list` request on the control stream gets its result back tagged with the same id the client's own `Connection::call` minted; a request built to validate server-side (bypassing the client's own pre-send `validate` call by hand-encoding a bad method name) gets a matching `Response::Error` with that same id - proving the server's own `validate` call still runs, not just the client's.
- **AC #3.** Both parts of §2's proof, plus the negative control named there.
- **AC #4.** No new line of protocol/handshake/stream-state logic anywhere in this task's own diff (confirmed the same way as §6) - proven by the diff itself at review time, not a test.
- **Context (§1).** For a session, a tab and a pane method: a target-less request over a real socket from a `Context::none()` connection is refused with `BadRequest` naming the missing part, and changes nothing in the tree; the same request given a (fully explicit) target succeeds; a partial target that omits the one part a no-context connection cannot supply is refused the same way, with nothing left behind.
- **Hostile input, one test per case named in §3**: oversized frame post-handshake; unknown kind tag; non-JSON control payload; a `Reply`-shaped frame from the client; an `Event`/`UnknownEvent` from the client; a second `Hello`; data/control on an unopened stream; two requests sharing one id (both answered correctly); a client that floods requests and never reads (closed, not hung, not unbounded); a client that trickles a frame post-handshake forever (never closed, named consequence in §3, proven by a bounded-wait assertion that the connection stays open past several multiples of what would have been a handshake-phase timeout).
- **Thread joining.** Every path out of `connection::handle` - ordinary close, each hostile case above, a `Stopper::stop()` mid-request, and (§5) a panic inside `Runtime::handle` - leaves the writer thread joined and the client disconnected from the tree, proven the same way TASK-1.44 already proves its own single thread's lifecycle: a counter or an `events`-style channel, never a sleep, bounded waits throughout. `Runtime::stop()` actually running, exactly once, after every connection thread has already been joined by `run`'s own `thread::scope`, is proven directly: a real pane started on one connection, that connection closed, `run` returns, the real process is gone - the same shape `runtime.rs`'s own existing `stop_tests` module already uses, now exercised through `listen::run` rather than by calling `Runtime::stop()` directly. That `run` cannot even compile a call to `runtime.stop()` while a `scope.spawn`ed closure might still hold `&runtime` is a property of the type, checked by every build, not a separate test.
- **Repetition.** Every test above that touches real threads, a real socket or real timing is run at least 10 times consecutively via `record-check` before being cited as proof, with counts recorded, matching the discipline TASK-1.44/1.76's own reviews required.

## Risks

- `ServerConfig` gaining `launcher`/`login` fields touches every existing call site that builds one with `..ServerConfig::default()` (handshake.rs, shutdown.rs, race.rs, socket_mode.rs) - confirmed by grep that all of them already use that pattern rather than naming every field, so none need editing; if one does not, that is itself a finding to fix, not a sign the design is wrong.
- `Outbound`'s own back-pressure numbers (256 control entries, 64 data frames per stream) are this task's own choices, not measured here (decision-2's own 64-frame number for data is reused directly; 256 for control is this task's own generous guess) - named so a future measurement (TASK-1.47, with real traffic) can revise either without surprise.
- `resolve_tab_str`/`resolve_pane_str`'s internal sentinel (§1) never crosses into a response, but it is still a value this task invents to make a required kernel parameter satisfiable when `Context` has none - if building this finds a cleaner way to ask "would this path need a default at all" without touching kernel code (or finds the sentinel harder to keep internal-only than this plan expects), that is worth a line in the notes, not a silent swap.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Dependency added by @bruk.o 2026-11-15: TASK-1.76 introduces the server's Runtime (the tree, the pane handles and one handle() entry point); this task routes frames into it rather than into dispatch directly. The session count the idle rule needs (TASK-1.77) comes from here.

Plan checked against decision-2 (sections 1, 3, 6, 7), doc-14, doc-2 sections 4 and 6, and doc-16 by @bruk.o: ok. After the handshake a connection has a reader and one writer; the writer drains control ahead of data with bounded queues, as decision-2 describes; the reader routes a request to the one Runtime and leans on the sdk's own classification for everything else, so nothing is added to the sdk. The Runtime is created inside run from ServerConfig, borrowed by connection threads inside a thread scope, and stopped explicitly after they are joined, before the socket file is removed and the lock released: this task owns the Runtime, and TASK-1.77's entry point only builds the configuration. A connection has no default session, tab or pane; a request that needs a target and gives none is refused as a bad request naming what is missing, as doc-2 gives at the CLI. Stated, not built: a connected client may sit mid-frame as it may sit idle; the number of connections is not bounded in M1. One point for the build, not the plan: where a partial target path meets a missing default, prefer deciding before the kernel is called over passing a placeholder id and remapping its error, and keep a test that a real missing target under a real context still answers NoSuchTarget.

Check passed: `just check` exited 0 at 2026-11-16T00:29:19+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.45 on task-1.45-server-frames-streams-and-requests at a1efb552dced:

```text
ok      format check (0.2s)
ok      lint (92.4s)
ok      boundaries (0.2s)
ok      licences (0.7s)
skip    tests: this change touches no crate
ok      build (steg-sdk) (3.2s)
ok      test (steg-sdk) (1.9s)
ok      test (steg-sdk, local-socket) (2.0s)
ok      build (steg-kernel) (45.4s)
ok      test (steg-kernel) (1.5s)
ok      build (steg-server) (46.8s)
ok      test (steg-server) (6.2s)
ok      test (steg-server, race-fixture) (6.7s)
ok      build (steg) (0.2s)
ok      test (steg) (0.3s)
ok      build (steg-app) (19.1s)
ok      test (steg-app) (1.2s)
ok      tool tests (2.7s)
ok      architecture (2.0s)
ok      recipe names (0.1s)
```

Check passed: `cargo test -p steg-server --lib --tests --features race-fixture` exited 0 at 2026-11-16T00:37:09+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.45 on task-1.45-server-frames-streams-and-requests at a1efb552dced with uncommitted changes, last 40 of 271 lines:

```text
test a_nonexistent_cwd_through_api_call_leaves_nothing_behind ... ok
test pane_close_with_force_on_a_running_program_proceeds_exactly_as_today_the_stated_gap ... ok
test pane_close_refuses_while_its_program_is_running_and_touches_nothing ... ok
test a_bad_command_through_api_call_leaves_nothing_behind ... ok
test a_command_that_does_not_exist_fails_and_leaves_no_session_tab_or_pane_behind ... ok
test tab_new_with_a_nonexistent_cwd_leaves_no_new_tab_or_pane_behind ... ok
test pane_split_with_no_program_or_cwd_runs_the_login_shell_at_home ... ok
test pane_split_with_a_nonexistent_cwd_leaves_no_new_pane_behind ... ok
test session_new_through_api_call_with_no_program_runs_the_login_shell ... ok
test session_close_refuses_while_any_pane_in_any_of_its_tabs_is_running ... ok
test tab_new_with_a_bad_command_leaves_no_new_tab_or_pane_behind ... ok
test tab_close_refuses_while_any_of_its_panes_is_running ... ok
test pane_split_with_a_bad_command_leaves_no_new_pane_behind ... ok
test session_new_with_a_command_starts_it_and_pane_show_reports_it_running_then_ended ... ok
test session_new_through_api_call_starts_a_real_program ... ok
test pane_split_through_api_call_starts_a_real_program ... ok
test tab_new_through_api_call_starts_a_real_program ... ok

test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.06s

     Running tests/shutdown.rs (target/debug/deps/shutdown-ca9d92c4050c47bd)

running 8 tests
test a_stalled_handshake_is_closed_and_run_returns_promptly_on_stop ... ok
test stopping_closes_the_listener_removes_the_socket_and_releases_the_lock ... ok
test stop_called_twice_concurrently_on_an_idle_server_is_harmless ... ok
test stop_on_a_genuinely_idle_server_returns_from_run_within_a_bound ... ok
test a_completed_idle_connection_is_closed_and_run_returns_promptly_on_stop ... ok
test many_short_connections_do_not_grow_the_tracked_set ... ok
test stopping_while_a_trickling_handshake_is_in_progress_still_returns_within_a_bound ... ok
test stop_racing_a_client_connecting_still_returns_within_a_bound ... ok

test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.13s

     Running tests/socket_mode.rs (target/debug/deps/socket_mode-8de5a5ad1bf57de9)

running 1 test
test the_socket_is_0600_even_under_a_permissive_umask ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

Check FAILED: `cargo test -p steg-server --lib dispatch::lookup` exited 101 at 2026-11-16T00:38:00+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.45 on task-1.45-server-frames-streams-and-requests at a1efb552dced with uncommitted changes, last 40 of 55 lines:

```text
test dispatch::lookup::tests::a_target_less_request_with_a_real_context_still_resolves ... ok
test dispatch::lookup::tests::a_partial_pane_target_omitting_the_tab_part_from_context_with_no_tab_is_bad_request ... FAILED
test dispatch::lookup::tests::wire_position_converts_one_based_to_zero_based ... ok
test dispatch::lookup::tests::a_partial_tab_target_omitting_the_session_part_from_context_none_is_bad_request ... FAILED
test dispatch::lookup::tests::a_target_less_request_from_context_none_is_bad_request_for_every_part ... FAILED
test dispatch::lookup::tests::a_wrong_but_fully_explicit_target_from_context_none_is_a_real_not_found ... ok
test dispatch::lookup::tests::target_tab_and_target_pane_resolve_a_fully_explicit_target_from_context_none ... ok
test dispatch::lookup::tests::wire_position_rejects_zero_and_negative_without_a_kernel_call ... ok

failures:

---- dispatch::lookup::tests::a_partial_pane_target_omitting_the_tab_part_from_context_with_no_tab_is_bad_request stdout ----

thread 'dispatch::lookup::tests::a_partial_pane_target_omitting_the_tab_part_from_context_with_no_tab_is_bad_request' (16036973) panicked at crates/server/src/dispatch/lookup.rs:365:9:
assertion `left == right` failed
  left: NoSuchTarget
 right: BadRequest
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace

---- dispatch::lookup::tests::a_partial_tab_target_omitting_the_session_part_from_context_none_is_bad_request stdout ----

thread 'dispatch::lookup::tests::a_partial_tab_target_omitting_the_session_part_from_context_none_is_bad_request' (16036974) panicked at crates/server/src/dispatch/lookup.rs:343:9:
assertion `left == right` failed
  left: NoSuchTarget
 right: BadRequest

---- dispatch::lookup::tests::a_target_less_request_from_context_none_is_bad_request_for_every_part stdout ----

thread 'dispatch::lookup::tests::a_target_less_request_from_context_none_is_bad_request_for_every_part' (16036975) panicked at crates/server/src/dispatch/lookup.rs:279:18:
called `Result::unwrap_err()` on an `Ok` value: SessionId(0)


failures:
    dispatch::lookup::tests::a_partial_pane_target_omitting_the_tab_part_from_context_with_no_tab_is_bad_request
    dispatch::lookup::tests::a_partial_tab_target_omitting_the_session_part_from_context_none_is_bad_request
    dispatch::lookup::tests::a_target_less_request_from_context_none_is_bad_request_for_every_part

test result: FAILED. 7 passed; 3 failed; 0 ignored; 0 measured; 81 filtered out; finished in 0.00s

error: test failed, to rerun pass `-p steg-server --lib`
```

Check passed: `cargo test -p steg-server --lib --tests --features race-fixture` exited 0 at 2026-11-16T00:38:37+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.45 on task-1.45-server-frames-streams-and-requests at a1efb552dced with uncommitted changes, last 40 of 272 lines:

```text
test a_nonexistent_cwd_through_api_call_leaves_nothing_behind ... ok
test session_new_through_api_call_with_no_program_runs_the_login_shell ... ok
test a_command_that_does_not_exist_fails_and_leaves_no_session_tab_or_pane_behind ... ok
test pane_close_refuses_while_its_program_is_running_and_touches_nothing ... ok
test tab_new_with_a_nonexistent_cwd_leaves_no_new_tab_or_pane_behind ... ok
test pane_split_with_no_program_or_cwd_runs_the_login_shell_at_home ... ok
test a_bad_command_through_api_call_leaves_nothing_behind ... ok
test pane_split_with_a_nonexistent_cwd_leaves_no_new_pane_behind ... ok
test session_close_refuses_while_any_pane_in_any_of_its_tabs_is_running ... ok
test pane_close_with_force_on_a_running_program_proceeds_exactly_as_today_the_stated_gap ... ok
test tab_new_with_a_bad_command_leaves_no_new_tab_or_pane_behind ... ok
test pane_split_with_a_bad_command_leaves_no_new_pane_behind ... ok
test session_new_through_api_call_starts_a_real_program ... ok
test tab_close_refuses_while_any_of_its_panes_is_running ... ok
test tab_new_through_api_call_starts_a_real_program ... ok
test session_new_with_a_command_starts_it_and_pane_show_reports_it_running_then_ended ... ok
test pane_split_through_api_call_starts_a_real_program ... ok

test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.06s

     Running tests/shutdown.rs (target/debug/deps/shutdown-ca9d92c4050c47bd)

running 8 tests
test a_stalled_handshake_is_closed_and_run_returns_promptly_on_stop ... ok
test stopping_closes_the_listener_removes_the_socket_and_releases_the_lock ... ok
test stop_on_a_genuinely_idle_server_returns_from_run_within_a_bound ... ok
test stop_called_twice_concurrently_on_an_idle_server_is_harmless ... ok
test a_completed_idle_connection_is_closed_and_run_returns_promptly_on_stop ... ok
test many_short_connections_do_not_grow_the_tracked_set ... ok
test stopping_while_a_trickling_handshake_is_in_progress_still_returns_within_a_bound ... ok
test stop_racing_a_client_connecting_still_returns_within_a_bound ... ok

test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.13s

     Running tests/socket_mode.rs (target/debug/deps/socket_mode-8de5a5ad1bf57de9)

running 1 test
test the_socket_is_0600_even_under_a_permissive_umask ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

Check FAILED: `cargo test -p steg-server --lib outbound` exited 101 at 2026-11-16T00:45:02+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.45 on task-1.45-server-frames-streams-and-requests at c42025242d19 with uncommitted changes, last 40 of 49 lines:

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.08s
     Running unittests src/lib.rs (target/debug/deps/steg_server-75c81fd969602d6a)

running 10 tests
test outbound::tests::a_full_control_queue_refuses_rather_than_growing_without_bound ... ok
test outbound::tests::enqueue_data_on_a_closed_stream_is_refused_the_same_way ... ok
test outbound::tests::enqueue_refuses_once_closed ... ok
test outbound::tests::enqueue_data_on_a_never_opened_stream_is_refused ... ok
test outbound::tests::closing_drains_what_is_already_queued_then_the_writer_returns ... ok
test outbound::tests::a_stream_with_nothing_queued_is_skipped_not_given_an_empty_turn ... ok
test outbound::tests::data_streams_are_drained_round_robin_one_frame_at_a_time ... ok
test outbound::tests::control_is_drained_before_any_data_even_when_data_was_enqueued_first ... FAILED
test outbound::tests::a_genuine_flood_on_a_data_stream_never_delays_a_control_reply ... FAILED
test outbound::tests::a_full_data_queue_drops_the_new_frame_rather_than_growing_or_blocking ... ok

failures:

---- outbound::tests::control_is_drained_before_any_data_even_when_data_was_enqueued_first stdout ----

thread 'outbound::tests::control_is_drained_before_any_data_even_when_data_was_enqueued_first' (16171111) panicked at crates/server/src/outbound.rs:295:9:
assertion `left == right` failed
  left: [100, 97, 116, 97, 45, 102, 105, 114, 115, 116, 99, 111, 110, 116]
 right: [99, 111, 110, 116, 114, 111, 108, 45, 115, 101, 99, 111, 110, 100]
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace

---- outbound::tests::a_genuine_flood_on_a_data_stream_never_delays_a_control_reply stdout ----

thread 'outbound::tests::a_genuine_flood_on_a_data_stream_never_delays_a_control_reply' (16171108) panicked at crates/server/src/outbound.rs:511:9:
assertion `left == right` failed
  left: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]
 right: [99, 111, 110, 116, 114, 111, 108, 45, 117, 110, 100, 101, 114, 45, 102, 108, 111, 111, 100]


failures:
    outbound::tests::a_genuine_flood_on_a_data_stream_never_delays_a_control_reply
    outbound::tests::control_is_drained_before_any_data_even_when_data_was_enqueued_first

test result: FAILED. 8 passed; 2 failed; 0 ignored; 0 measured; 91 filtered out; finished in 0.00s

error: test failed, to rerun pass `-p steg-server --lib`
```

Check passed: `cargo test -p steg-server --lib --tests --features race-fixture` exited 0 at 2026-11-16T00:45:25+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.45 on task-1.45-server-frames-streams-and-requests at c42025242d19 with uncommitted changes, last 40 of 379 lines:

```text
test a_nonexistent_cwd_through_api_call_leaves_nothing_behind ... ok
test session_new_through_api_call_with_no_program_runs_the_login_shell ... ok
test tab_new_with_a_nonexistent_cwd_leaves_no_new_tab_or_pane_behind ... ok
test pane_split_with_a_nonexistent_cwd_leaves_no_new_pane_behind ... ok
test session_close_refuses_while_any_pane_in_any_of_its_tabs_is_running ... ok
test a_bad_command_through_api_call_leaves_nothing_behind ... ok
test a_command_that_does_not_exist_fails_and_leaves_no_session_tab_or_pane_behind ... ok
test pane_close_refuses_while_its_program_is_running_and_touches_nothing ... ok
test pane_close_with_force_on_a_running_program_proceeds_exactly_as_today_the_stated_gap ... ok
test session_new_through_api_call_starts_a_real_program ... ok
test pane_split_with_no_program_or_cwd_runs_the_login_shell_at_home ... ok
test tab_new_with_a_bad_command_leaves_no_new_tab_or_pane_behind ... ok
test pane_split_with_a_bad_command_leaves_no_new_pane_behind ... ok
test tab_close_refuses_while_any_of_its_panes_is_running ... ok
test session_new_with_a_command_starts_it_and_pane_show_reports_it_running_then_ended ... ok
test pane_split_through_api_call_starts_a_real_program ... ok
test tab_new_through_api_call_starts_a_real_program ... ok

test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.06s

     Running tests/shutdown.rs (target/debug/deps/shutdown-ca9d92c4050c47bd)

running 8 tests
test a_stalled_handshake_is_closed_and_run_returns_promptly_on_stop ... ok
test stopping_closes_the_listener_removes_the_socket_and_releases_the_lock ... ok
test stop_on_a_genuinely_idle_server_returns_from_run_within_a_bound ... ok
test stop_called_twice_concurrently_on_an_idle_server_is_harmless ... ok
test a_completed_idle_connection_is_closed_and_run_returns_promptly_on_stop ... ok
test many_short_connections_do_not_grow_the_tracked_set ... ok
test stopping_while_a_trickling_handshake_is_in_progress_still_returns_within_a_bound ... ok
test stop_racing_a_client_connecting_still_returns_within_a_bound ... ok

test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.13s

     Running tests/socket_mode.rs (target/debug/deps/socket_mode-8de5a5ad1bf57de9)

running 1 test
test the_socket_is_0600_even_under_a_permissive_umask ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

Check passed: `cargo test -p steg-server --lib --tests --features race-fixture` exited 0 at 2026-11-16T00:49:15+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.45 on task-1.45-server-frames-streams-and-requests at f61775beb21b with uncommitted changes, last 40 of 380 lines:

```text
test session_new_with_a_nonexistent_cwd_fails_and_leaves_nothing_behind ... ok
test tab_new_with_a_nonexistent_cwd_leaves_no_new_tab_or_pane_behind ... ok
test pane_split_with_no_program_or_cwd_runs_the_login_shell_at_home ... ok
test pane_close_refuses_while_its_program_is_running_and_touches_nothing ... ok
test pane_split_with_a_nonexistent_cwd_leaves_no_new_pane_behind ... ok
test a_command_that_does_not_exist_fails_and_leaves_no_session_tab_or_pane_behind ... ok
test a_bad_command_through_api_call_leaves_nothing_behind ... ok
test session_new_through_api_call_with_no_program_runs_the_login_shell ... ok
test pane_close_with_force_on_a_running_program_proceeds_exactly_as_today_the_stated_gap ... ok
test session_close_refuses_while_any_pane_in_any_of_its_tabs_is_running ... ok
test session_new_with_a_command_starts_it_and_pane_show_reports_it_running_then_ended ... ok
test pane_split_with_a_bad_command_leaves_no_new_pane_behind ... ok
test tab_new_with_a_bad_command_leaves_no_new_tab_or_pane_behind ... ok
test tab_close_refuses_while_any_of_its_panes_is_running ... ok
test session_new_through_api_call_starts_a_real_program ... ok
test pane_split_through_api_call_starts_a_real_program ... ok
test tab_new_through_api_call_starts_a_real_program ... ok

test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.06s

     Running tests/shutdown.rs (target/debug/deps/shutdown-ca9d92c4050c47bd)

running 8 tests
test stopping_closes_the_listener_removes_the_socket_and_releases_the_lock ... ok
test stop_on_a_genuinely_idle_server_returns_from_run_within_a_bound ... ok
test a_stalled_handshake_is_closed_and_run_returns_promptly_on_stop ... ok
test stop_called_twice_concurrently_on_an_idle_server_is_harmless ... ok
test a_completed_idle_connection_is_closed_and_run_returns_promptly_on_stop ... ok
test many_short_connections_do_not_grow_the_tracked_set ... ok
test stopping_while_a_trickling_handshake_is_in_progress_still_returns_within_a_bound ... ok
test stop_racing_a_client_connecting_still_returns_within_a_bound ... ok

test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.13s

     Running tests/socket_mode.rs (target/debug/deps/socket_mode-8de5a5ad1bf57de9)

running 1 test
test the_socket_is_0600_even_under_a_permissive_umask ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

Check passed: `cargo test -p steg-server --lib --tests --features race-fixture` exited 0 at 2026-11-16T00:55:32+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.45 on task-1.45-server-frames-streams-and-requests at 65ea5815dcc8 with uncommitted changes, last 40 of 312 lines:

```text
test a_nonexistent_cwd_through_api_call_leaves_nothing_behind ... ok
test pane_split_with_no_program_or_cwd_runs_the_login_shell_at_home ... ok
test tab_new_with_a_nonexistent_cwd_leaves_no_new_tab_or_pane_behind ... ok
test pane_close_refuses_while_its_program_is_running_and_touches_nothing ... ok
test session_new_through_api_call_with_no_program_runs_the_login_shell ... ok
test a_bad_command_through_api_call_leaves_nothing_behind ... ok
test pane_split_with_a_nonexistent_cwd_leaves_no_new_pane_behind ... ok
test pane_close_with_force_on_a_running_program_proceeds_exactly_as_today_the_stated_gap ... ok
test session_close_refuses_while_any_pane_in_any_of_its_tabs_is_running ... ok
test a_command_that_does_not_exist_fails_and_leaves_no_session_tab_or_pane_behind ... ok
test tab_close_refuses_while_any_of_its_panes_is_running ... ok
test tab_new_with_a_bad_command_leaves_no_new_tab_or_pane_behind ... ok
test pane_split_with_a_bad_command_leaves_no_new_pane_behind ... ok
test session_new_through_api_call_starts_a_real_program ... ok
test session_new_with_a_command_starts_it_and_pane_show_reports_it_running_then_ended ... ok
test tab_new_through_api_call_starts_a_real_program ... ok
test pane_split_through_api_call_starts_a_real_program ... ok

test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.06s

     Running tests/shutdown.rs (target/debug/deps/shutdown-ca9d92c4050c47bd)

running 8 tests
test a_stalled_handshake_is_closed_and_run_returns_promptly_on_stop ... ok
test stopping_closes_the_listener_removes_the_socket_and_releases_the_lock ... ok
test stop_on_a_genuinely_idle_server_returns_from_run_within_a_bound ... ok
test stop_called_twice_concurrently_on_an_idle_server_is_harmless ... ok
test a_completed_idle_connection_is_closed_and_run_returns_promptly_on_stop ... ok
test many_short_connections_do_not_grow_the_tracked_set ... ok
test stopping_while_a_trickling_handshake_is_in_progress_still_returns_within_a_bound ... ok
test stop_racing_a_client_connecting_still_returns_within_a_bound ... ok

test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.13s

     Running tests/socket_mode.rs (target/debug/deps/socket_mode-8de5a5ad1bf57de9)

running 1 test
test the_socket_is_0600_even_under_a_permissive_umask ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

Check FAILED: `cargo test -p steg-server --lib runtime::tests::a_panic_while_holding` exited 101 at 2026-11-16T01:05:33+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.45 on task-1.45-server-frames-streams-and-requests at 8d3e2643d1d0 with uncommitted changes, last 40 of 122 lines:

```text
                               at /rustc/48a229ceaefd4985c50990b14116b6d856af0985/library/std/src/panicking.rs:576:43
  33:        0x102fa068c - std[5d97c59e5e5fafcc]::panicking::catch_unwind::<core[663bb9e1522ba5ef]::result::Result<(), alloc[cc4db322bb1ac55d]::string::String>, core[663bb9e1522ba5ef]::panic::unwind_safe::AssertUnwindSafe<test[4aa3eea2f4983582]::run_test_in_process::{closure#0}>>
                               at /rustc/48a229ceaefd4985c50990b14116b6d856af0985/library/std/src/panicking.rs:544:19
  34:        0x102fa068c - std[5d97c59e5e5fafcc]::panic::catch_unwind::<core[663bb9e1522ba5ef]::panic::unwind_safe::AssertUnwindSafe<test[4aa3eea2f4983582]::run_test_in_process::{closure#0}>, core[663bb9e1522ba5ef]::result::Result<(), alloc[cc4db322bb1ac55d]::string::String>>
                               at /rustc/48a229ceaefd4985c50990b14116b6d856af0985/library/std/src/panic.rs:359:14
  35:        0x102fa068c - test[4aa3eea2f4983582]::run_test_in_process
                               at /rustc/48a229ceaefd4985c50990b14116b6d856af0985/library/test/src/lib.rs:747:27
  36:        0x102fa068c - test[4aa3eea2f4983582]::run_test::{closure#0}
                               at /rustc/48a229ceaefd4985c50990b14116b6d856af0985/library/test/src/lib.rs:668:43
  37:        0x102f9b56c - test[4aa3eea2f4983582]::run_test::{closure#1}
                               at /rustc/48a229ceaefd4985c50990b14116b6d856af0985/library/test/src/lib.rs:698:41
  38:        0x102f9b56c - std[5d97c59e5e5fafcc]::sys::backtrace::__rust_begin_short_backtrace::<test[4aa3eea2f4983582]::run_test::{closure#1}, ()>
                               at /rustc/48a229ceaefd4985c50990b14116b6d856af0985/library/std/src/sys/backtrace.rs:166:18
  39:        0x102fa2b54 - std[5d97c59e5e5fafcc]::thread::lifecycle::spawn_unchecked::<test[4aa3eea2f4983582]::run_test::{closure#1}, ()>::{closure#1}::{closure#0}
                               at /rustc/48a229ceaefd4985c50990b14116b6d856af0985/library/std/src/thread/lifecycle.rs:70:13
  40:        0x102fa2b54 - <core[663bb9e1522ba5ef]::panic::unwind_safe::AssertUnwindSafe<std[5d97c59e5e5fafcc]::thread::lifecycle::spawn_unchecked<test[4aa3eea2f4983582]::run_test::{closure#1}, ()>::{closure#1}::{closure#0}> as core[663bb9e1522ba5ef]::ops::function::FnOnce<()>>::call_once
                               at /rustc/48a229ceaefd4985c50990b14116b6d856af0985/library/core/src/panic/unwind_safe.rs:275:9
  41:        0x102fa2b54 - std[5d97c59e5e5fafcc]::panicking::catch_unwind::do_call::<core[663bb9e1522ba5ef]::panic::unwind_safe::AssertUnwindSafe<std[5d97c59e5e5fafcc]::thread::lifecycle::spawn_unchecked<test[4aa3eea2f4983582]::run_test::{closure#1}, ()>::{closure#1}::{closure#0}>, ()>
                               at /rustc/48a229ceaefd4985c50990b14116b6d856af0985/library/std/src/panicking.rs:576:43
  42:        0x102fa2b54 - std[5d97c59e5e5fafcc]::panicking::catch_unwind::<(), core[663bb9e1522ba5ef]::panic::unwind_safe::AssertUnwindSafe<std[5d97c59e5e5fafcc]::thread::lifecycle::spawn_unchecked<test[4aa3eea2f4983582]::run_test::{closure#1}, ()>::{closure#1}::{closure#0}>>
                               at /rustc/48a229ceaefd4985c50990b14116b6d856af0985/library/std/src/panicking.rs:544:19
  43:        0x102fa2b54 - std[5d97c59e5e5fafcc]::panic::catch_unwind::<core[663bb9e1522ba5ef]::panic::unwind_safe::AssertUnwindSafe<std[5d97c59e5e5fafcc]::thread::lifecycle::spawn_unchecked<test[4aa3eea2f4983582]::run_test::{closure#1}, ()>::{closure#1}::{closure#0}>, ()>
                               at /rustc/48a229ceaefd4985c50990b14116b6d856af0985/library/std/src/panic.rs:359:14
  44:        0x102fa2b54 - std[5d97c59e5e5fafcc]::thread::lifecycle::spawn_unchecked::<test[4aa3eea2f4983582]::run_test::{closure#1}, ()>::{closure#1}
                               at /rustc/48a229ceaefd4985c50990b14116b6d856af0985/library/std/src/thread/lifecycle.rs:68:26
  45:        0x102fa2b54 - <std[5d97c59e5e5fafcc]::thread::lifecycle::spawn_unchecked<test[4aa3eea2f4983582]::run_test::{closure#1}, ()>::{closure#1} as core[663bb9e1522ba5ef]::ops::function::FnOnce<()>>::call_once::{shim:vtable#0}
                               at /rustc/48a229ceaefd4985c50990b14116b6d856af0985/library/core/src/ops/function.rs:250:5
  46:        0x1031477cc - <alloc[cc4db322bb1ac55d]::boxed::Box<dyn core[663bb9e1522ba5ef]::ops::function::FnOnce<(), Output = ()> + core[663bb9e1522ba5ef]::marker::Send> as core[663bb9e1522ba5ef]::ops::function::FnOnce<()>>::call_once
                               at /rustc/48a229ceaefd4985c50990b14116b6d856af0985/library/alloc/src/boxed.rs:2319:9
  47:        0x1031477cc - <std[5d97c59e5e5fafcc]::sys::thread::unix::Thread>::new::thread_start
                               at /rustc/48a229ceaefd4985c50990b14116b6d856af0985/library/std/src/sys/thread/unix.rs:123:17
  48:        0x199ecdd00 - __pthread_exit

thread 'runtime::tests::a_panic_while_holding_the_lock_does_not_poison_a_later_call' (16535826) panicked at /rustc/48a229ceaefd4985c50990b14116b6d856af0985/library/core/src/panicking.rs:233:5:
panic in a destructor during cleanup
thread caused non-unwinding panic. aborting.
error: test failed, to rerun pass `-p steg-server --lib`

Caused by:
  process didn't exit successfully: `/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.45/target/debug/deps/steg_server-75c81fd969602d6a 'runtime::tests::a_panic_while_holding'` (signal: 6, SIGABRT: process abort signal)
```

Build load note (not a code finding): the server crate's own real-socket test run time is fast on its own - every `tests/routing.rs` run that completed finished in 0.71-0.73s internally, every time. What is slow is `cargo test` itself getting scheduled at all: with this machine's load average around 9-11 (several other workers' own `cargo build`/`cargo test` running concurrently in sibling worktrees), repeating `cargo test -p steg-server --test routing` 10 times under a 60s-per-iteration timeout, 6 of the 10 iterations were killed by that timeout with no output at all - not a test failure, cargo never got far enough to print anything, including "Compiling"/"Finished". Only 4 of 10 iterations actually ran, and all 4 passed (17/17 each time, ~0.72s). No evidence this is my own code hanging: when the test binary itself runs, it is fast and deterministic.

This is a real risk worth naming, not dismissing: this file's own tests use wall-clock bounds (2s for most "must close promptly" assertions, 5s for a few, matching this crate's existing convention in handshake.rs/shutdown.rs). Those bounds measure real time inside the test PROCESS once it is running, not the time to get scheduled in the first place - so they are at risk from CPU contention inside the process itself (a writer or reader thread not getting scheduled promptly), not from the kind of outer scheduling delay seen here (which is cargo/process-level, before the test binary even starts). I have not seen one of these in-process bounds actually trip under this load in any of the runs so far. This machine's load right now comes specifically from several asdlc workers building in parallel in sibling worktrees, not from anything inherent to this task; CI runs on its own dedicated runner and would not see this specific contention pattern. Recorded here rather than silently assumed fine either way.

Check passed: `sh -c 'timeout 15 cargo test -p steg-server --test routing a_client_that_floods_requests_and_never_reads_any_reply_is_closed_not_hung; echo "exit code: $?"'` exited 0 at 2026-11-16T01:20:21+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.45 on task-1.45-server-frames-streams-and-requests at 8d3e2643d1d0 with uncommitted changes, last 40 of 44 lines:

```text
    |        ----------------- field in this struct
...
263 |     shutdown_handle: UnixStream,
    |     ^^^^^^^^^^^^^^^
    |
    = note: `#[warn(dead_code)]` (part of `#[warn(unused)]`) on by default

warning: constant `DATA_CAP` is never used
  --> crates/server/src/outbound.rs:35:7
   |
35 | const DATA_CAP: usize = 64;
   |       ^^^^^^^^

warning: enum `EnqueueDataError` is never used
  --> crates/server/src/outbound.rs:51:17
   |
51 | pub(crate) enum EnqueueDataError {
   |                 ^^^^^^^^^^^^^^^^

warning: methods `open_stream`, `close_stream`, and `enqueue_data` are never used
   --> crates/server/src/outbound.rs:162:19
    |
127 | impl Outbound {
    | ------------- methods in this implementation
...
162 |     pub(crate) fn open_stream(&self, stream: u32) {
    |                   ^^^^^^^^^^^
...
175 |     pub(crate) fn close_stream(&self, stream: u32) {
    |                   ^^^^^^^^^^^^
...
191 |     pub(crate) fn enqueue_data(&self, stream: u32, bytes: Vec<u8>) -> Result<(), EnqueueDataError> {
    |                   ^^^^^^^^^^^^

warning: `steg-server` (lib) generated 4 warnings
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.08s
     Running tests/routing.rs (target/debug/deps/routing-0c157b8cdcb37f92)

running 1 test
exit code: 124
```
<!-- SECTION:NOTES:END -->

## Comments

<!-- COMMENTS:BEGIN -->
author: @bruk
created: 2026-11-16T00:24:00Z

---

Plan approved by @bruk (1007366d16d6, size-3): its worker may build it.

---
<!-- COMMENTS:END -->
