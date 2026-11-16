---
id: TASK-1.47
title: >-
  Server: attach to a pane, stream its screen, and restore it after a client
  goes away
status: In Progress
assignee:
  - '@bruk.w4'
created_date: '2026-11-15T01:33:00Z'
updated_date: '2026-11-16T12:47:00Z'
labels:
  - size-5
milestone: m-3
dependencies:
  - TASK-1.40
  - TASK-1.42
  - TASK-1.45
  - TASK-1.74
  - TASK-1.76
  - TASK-1.46
parent_task_id: TASK-1
type: task
project: bruk-io-steg
asdlc_accepted_from: DRAFT-17
asdlc_branch: task-1.47-server-attach-to-a-pane-stream-its
asdlc_plan_approved_by: bruk
asdlc_plan_approved_hash: '9cff507ccc3f'
asdlc_plan_approved_size: size-5
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
doc-8 R2, doc-9 R15, doc-15 R91. decision-2, sections 3, 5 and 6, including its two ordering rules and its back-pressure policy.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Attaching returns a snapshot, and updates follow it in order with none lost and none sent for a stream the client has not been told about
- [ ] #2 A client that falls behind gets one fresh screen-sized snapshot, without scrollback; the program is never blocked
- [ ] #3 Attaching is offered by the steg library, and this task's tests go through it over a real socket with a real pty (R101)
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
## For the engineer — decisions made (2026-11-16), recorded here as settled

The engineer decided both open questions from this plan's first draft, as
recommended by the lead:

1. **Attach is a described API method**, sent as an ordinary request:
   `pane.attach`, whose reply carries the stream id and the opening
   snapshot. Decision-2's four control types (request, response, error,
   event) stay four — no new wire primitive. `detach` is the mirror:
   `pane.detach`, a plain request, `Returns::Nothing` — and, now that the
   routing is worked out (below), its target is an ordinary pane path like
   every other `pane.*` verb, not a stream id: the connection already has
   to keep its own `PaneId -> stream` bookkeeping to route input once
   TASK-1.114 lands, so `pane.detach`'s dispatch looks that stream up
   locally rather than asking the client to name it, and doc-2's target
   grammar needs no exception after all (an earlier draft of this
   paragraph proposed one; dropped once the routing was settled).
   **Why "closing the stream" alone, the other option the engineer named,
   is not enough**: `Connection::close_stream` (sdk) is purely local
   bookkeeping on the caller's own `open_streams` set — it tells nobody on
   the other end anything. The pane's worker thread (where the fan-out
   list lives) would never learn to stop sending that attachment frames,
   so the server would keep computing and queuing updates for a stream the
   client has already stopped reading — exactly the unbounded-queue-growth
   shape decision-2 §6 forbids. A message the server acts on is required;
   `pane.detach` is that message.
   The updates that follow attach (`Delta`/`Full`/`Unchanged`), input, and a
   declared size stay protocol shapes in the sdk, undecoded by `Control` —
   exactly as this plan's first draft had them, and TASK-1.114's to build
   for input/size. `pane.attach` itself takes no size parameter: declaring
   one is `Tree::declare_size`, a separate kernel call TASK-1.114 wires in,
   not part of opening the stream.
2. **The tick is `ServerConfig`, not a published setting.** `crates/server/
   src/config.rs` already holds exactly this pattern
   (`DEFAULT_HANDSHAKE_TIMEOUT`, `DEFAULT_ARBITRATION_TIMEOUT`, ... each a
   `pub const Duration` plus a field on `ServerConfig`, injectable in
   tests, threaded through `listen.rs`'s one call to `Runtime::new`). This
   task adds `DEFAULT_ATTACH_TICK: Duration = Duration::from_millis(8)` and
   an `attach_tick: Duration` field the same way, threaded into `Runtime::
   new` (which already takes `force_grace: Duration` as its own injected
   duration, post-TASK-1.78) and from there into `pane::start` so each
   pane's worker thread is built knowing its own tick.

Two corrections to this plan's own first draft, both the lead's:

3. **A pane's worker thread must not wake on a timer when idle.** The
   earlier draft's `recv_timeout` on every iteration, unconditionally,
   would wake roughly 125 times a second per pane with nobody attached —
   `worker_loop` keeps `commands_rx.recv()` (no timeout, no wake-up at all)
   whenever there is no armed deadline, and arms one (`Instant::now() +
   attach_tick`) only when a command that could change the screen
   (`PtyBytes`, `Resize`) is handled *while at least one attachment is
   registered and no deadline is already armed* — one arm per dirty window,
   not one per byte, which is also what keeps the tick's own coalescing
   real rather than nominal. The deadline clears the moment a tick fires;
   it is never re-armed except by a later dirtying command. An idle pane
   (nothing printed, nothing attached) and an unattached pane with real
   output both call `recv()` with no timeout, for ever, until a command
   arrives - zero tick-body calls, by construction, not by a cheap tick.
4. **A new attachment is caught up at the snapshot's own sequence, in the
   same step that reads it** - not "caught up starting next tick." `pane.
   attach`'s dispatch handler sends the worker a command that, in one step,
   (a) reads `terminal.screen()` for the reply, (b) registers the
   attachment in the worker's own fan-out list as `CaughtUp(tracked_seq)`
   - the worker's currently-tracked point, the same one `changes_since`
   reads against - fixing it so the stream it is about to start receiving
   resumes at exactly `tracked_seq + 1`, as decision-2 §3 requires, and (c)
   only then is the reply queued / the attachment becomes visible to the
   writer thread. The ordering rule (§1 below) governs *that* visibility
   question - when the writer may start delivering frames for this stream -
   not the attachment's own caught-up bookkeeping, which is correct from
   the moment it is written, under the one lock the worker thread already
   serialises everything through (it has no other state to race against:
   it is the only thread that ever touches `attachments` or `tracked_seq`).

## The method vocabulary, aligned to TASK-1.46's final plan

The names changed once more since the previous revision of this section;
building against the final ones, below, not the earlier `Snapshot`/
`stream_item` guess (withdrawn along with every reference to it). TASK-1.46's
own "For the engineer" point 1 settles two separate things, because a
method's reply and what follows it are two different questions:

- **A nested reply.** `Returns` gains `Nested { of: String, summary: String }`
  - `of` names the reply's own sdk type (`"TreeSnapshot"` for `event.watch`),
  the same role `Identifier{of}` already plays for naming a kind of id,
  applied to naming a kind of value instead. `api show` prints "a nested
  value of type `<of>` - `<summary>`" in place of a fields table.
- **What follows.** `Method` gains `follows: Follows`, a closed enum:
  `Nothing` (every method but two); `Events(Returns)` - id-less items on the
  *same* control stream the reply travelled on (`event.watch`'s case);
  `Attachment { item: Returns, stream_id_field: String }` - id-less items on
  a *new* stream, whose id is one named field of the reply itself. The third
  variant is mine, and it is what answers the routing question this plan's
  previous revision raised and left open: there is no separate channel
  marker to invent - `stream_id_field` *is* the answer, naming exactly which
  field of the `Nested` reply the client reads the new stream's id from.
  **Confirmed, not a gap**: `Follows::Attachment` expresses everything this
  task needs - the item's shape (`item: Returns`) and which stream it
  travels on (`stream_id_field`) - so there is nothing further to raise.

Applied to this task's two methods (names chosen here, since 1.46's own plan
left them to this task to confirm, offering only placeholders):

- `pane.attach`: `returns: Nested{of: "PaneAttached", summary: "the new
  stream's id, and the pane's screen as it was at the moment of attaching"}`,
  `follows: Follows::Attachment{item: Nested{of: "ScreenUpdate", summary:
  "the rows that changed, or a whole screen when a client fell behind"},
  stream_id_field: "stream"}`.
- `pane.detach`: `returns: Nothing`, `follows: Follows::Nothing`.

Two new, named sdk types this task adds (`crates/sdk/src/screen.rs`), the
reply's own real shape, matching `of` exactly - `PaneAttached`'s own
`stream_id_field` is literally the name of one of its fields, not a separate
string that could drift from the type:

```rust
/// `pane.attach`'s own reply (decision-2 section 3): the new stream's id,
/// and the pane's screen at the moment of attaching.
pub struct PaneAttached {
    pub stream: u32,
    pub snapshot: Screen,
}
```

`Update` (this crate's own existing `Delta`/`Full`/`Unchanged` shape,
`crates/sdk/src/screen.rs`) is renamed to `ScreenUpdate` to match the `of`
name above and to read unambiguously in `api show`'s own output, where `of`
is a bare type name with no module path in front of it to disambiguate a
plain "Update" from doc-14's own general use of the word for layout/session/
tab changes elsewhere in the API - a small, mechanical rename with nothing
else depending on the old name yet (confirmed: `steg-kernel`'s `terminal.rs`
is this type's only other user today, and only ever by its Rust name, never
serialised under the old one anywhere).

**The description-level check, proposed as this task's own test, as
invited** (TASK-1.46 builds the general `of`-names-a-real-type allow-list
check; this task adds its own two entries to that allow-list, and a second,
narrower check specific to `Follows::Attachment`): for every method whose
`follows` is `Attachment{..}`, `stream_id_field` names a field that really
exists on that method's own `Nested` reply type, checked at run time, not
only by eye - serialise a real `PaneAttached` value and assert its own JSON
object contains a key equal to `stream_id_field`'s declared string. Concrete,
not aspirational: a typo in `stream_id_field` (or a later rename of
`PaneAttached::stream` that forgets this string) fails the build. Today this
only has one case to check (`pane.attach`); written generically enough that
the next method to use `Follows::Attachment` is covered by the same test,
not a new one.

**Doc-2 §8's table and the completeness test, the exact count and the exact
lines, for the lead to add** (specs are the lead's, not mine, to edit - this
is the proposed wording, not a change I am making). Verified against the
worktree after merging `origin/main` (TASK-1.78/1.87/1.89 all in): the test
is still named `the_method_set_is_exactly_doc_2_section_8s_42_pairs` and
still asserts `len() == 42` - TASK-1.89 enriched existing records, it did not
add methods - so this task's own baseline is the real, current one, not a
stale pre-merge guess:

- The table grows by two rows, both noun `pane`, both an ordinary pane
  target like every other `pane.*` verb (no exception to doc-2 §4's target
  grammar - see "detach... its target" above):
  `pane attach` | pane | "Opens a stream to the pane's screen" | the stream id
  and the opening snapshot, as JSON
  `pane detach` | pane | "Ends this connection's attachment to the pane" |
  nothing
- `methods.rs`'s `the_method_set_is_exactly_doc_2_section_8s_42_pairs` moves
  to 44 (`("pane","attach")`, `("pane","detach")` added) - the test's own
  name needs the new number, same mechanical update TASK-1.89 and TASK-1.109
  each already make for their own additions.

`api show pane.attach` needs no renderer change beyond what 1.46 already
adds for `Nested`: the summary string carries what this task needs to say.
A future Python client (M2) needs nothing special either - R106/decision-2
§8's own gate already proved a dependency-free client reads any method's
JSON reply and then demuxes frames off the same connection by stream id,
which is exactly what `pane.attach`'s reply plus the existing wire framing
gives it, with no generated code.

## Scope, narrowed by the split the lead has already filed

This task (TASK-1.47) now covers AC #1, both halves of AC #3, and AC #7 for
attaching only. TASK-1.114 (depends on this task) covers AC #4 - input and a
declared size on an attached client's stream. TASK-1.115 (depends on this
task) covers AC #2, #5, #6 - scrollback paging, the byte-for-byte restore
proof, and the spike's deletion (after its oracle is ported). TASK-1.109
(not this task's) owns `pane.cwd_changed`/`pane.title_changed`, flagged by
TASK-1.46 as something this task's own per-tick terminal read could have
absorbed - noted there already, not duplicated here.

## 1. Opening an attachment, and the snapshot it starts from (AC #1, #7)

**`pane.attach`/`pane.detach` cannot be plain `dispatch::dispatch` arms, and
this plan's own first draft wrote it as if they could be - corrected here.**
`dispatch::dispatch` is a pure function over `&mut Tree` (`rules/rust.md`'s
"core and shell": no I/O in its signature), so it has no way to reach
`State.panes` for a `PaneHandle`, and neither it nor `Runtime::handle` can
reach a connection's `Arc<Outbound>` at all - `Outbound` is created inside
`connection.rs`'s own `run_post_handshake`, after the handshake, and never
handed anywhere `Runtime` can see. This is the exact structural gap
TASK-1.46 §5 already documents for its own `register_subscriber` ("`Runtime`
has no way to reach any connection's writer today... fixed by `run_post_
handshake` calling a new `Runtime::register_subscriber`... right after
`spawn_writer`"), and this task's answer is the same shape:

- `connection.rs`'s read loop (`run_post_handshake`'s inner `match
  connection.poll()`) gains its own arm, *before* falling through to the
  existing `Incoming::Request { .. } => runtime.handle(...)` path: when
  `request.method` (after `api.call` resolution, the same cheap second
  resolution TASK-1.46 §4 already does for `event.watch`, for the same
  reason - a client reaching `pane.attach` *through* `api.call` must be
  routed here too) is `"pane.attach"` or `"pane.detach"`, call a new
  `Runtime::attach_pane(&self, description, context, request, outbound:
  &Arc<Outbound>, stream: u32) -> Result<(), ApiError>` / `Runtime::
  detach_pane(&self, description, context, request) -> Result<(), ApiError>`,
  passing the `Arc<Outbound>` this function already holds and a stream id
  from a small per-connection counter kept beside it (not in the kernel,
  not in `Runtime` - a connection concern). Every other request keeps going
  through `runtime.handle(...)` unchanged. On `Err`, `connection.rs` builds
  and enqueues the ordinary error reply itself, exactly as TASK-1.46's own
  `subscribe` path does on its own validation failure.
- **`Runtime::attach_pane` builds and enqueues its own reply, the same
  shape as `Runtime::subscribe`, not split across the `Runtime`/
  `connection.rs` boundary the way this plan's previous revision had it.**
  Lock held only for the part that needs it: takes the lock, validates,
  resolves the target the same way `dispatch/pane.rs`'s own target
  -resolution helpers do (widened from `pub(super)` to `pub(crate)` so
  `runtime.rs` can reuse them rather than duplicate target parsing - a
  files-changed item, not a design decision), calls `tree.view_pane(client,
  pane)` (kernel, already built - `crates/kernel/src/tree.rs`), sends the
  pane's `PaneHandle` a new `PaneCommand::Attach { stream: u32, reply:
  Sender<Screen> }` and waits on the bounded reply (the worker handles it
  in one step, §"For the engineer" point 4: read `terminal.screen()`,
  register the attachment as `CaughtUp(tracked_seq)` - but *not* yet handed
  the `Outbound`, see below - and reply), releases the lock, then - lock
  -free from here - builds `Response::Ok({"stream", "snapshot"})`, calls
  `outbound.enqueue_control(bytes)`, and **only once that succeeds** hands
  the worker's attachment entry the `Arc<Outbound>` it needs to actually
  send anything (one more short message, `PaneCommand::ArmAttachment
  { stream, outbound: Arc<Outbound> }`, or the `Outbound` handle is simply
  not read by the tick body for an attachment until this message arrives -
  either shape works, decided during implementation) and calls `Outbound::
  open_stream(stream)`. This is decision-2 §3's ordering rule applied
  exactly: "queue the attach result first, register the attachment's queue
  second" - here, "register the attachment's queue" is deliberately two
  things kept apart: the worker's own internal bookkeeping (`CaughtUp`/
  `NeedsResync`, safe to set immediately, nothing reads it until armed) and
  the `Outbound`/writer-visible registration (`open_stream`, *and* handing
  the worker the `Arc<Outbound>` it needs to call `enqueue_data` at all) -
  only the second one is what decision-2's rule is actually about, and it
  is what happens last.
- **The race this closes, stated precisely, not just asserted**: without
  the worker-side split above, a tick could fire between the worker
  registering the attachment and `connection.rs`'s thread calling
  `open_stream` (two different threads; the worker's own `recv_timeout`
  cadence does not wait for anything outside itself), and would otherwise
  need the `Arc<Outbound>` to attempt `enqueue_data` early. Not handing it
  the `Outbound` until after `open_stream` has run makes that attempt
  structurally impossible rather than merely unlikely. As a second,
  independent line of defence (belt and suspenders, not load-bearing on its
  own - the same relationship `outbound.rs`'s existing `Closed` check has
  to `Outbound::close()`): if a tick ever did reach `enqueue_data` for a
  stream `Outbound::open_stream` has not yet registered, `EnqueueDataError::
  NotOpen` comes back, and the tick body (§2) treats that exactly like
  `Full` - mark `NeedsResync`, skip, retry next tick - so even a timing
  assumption proven wrong costs one tick's delay, never a silently dropped
  frame or an `unknown_stream_frames` count above zero.
- `Runtime::detach_pane` takes the lock, resolves the target pane, looks up
  which stream this connection attached it on (the connection's own
  `PaneId -> stream` map, not a parameter the client supplies - see "For
  the engineer" point 1's revision above), calls `tree.stop_viewing(client,
  pane)`, and sends the pane's worker a `PaneCommand::Detach(stream)` to
  drop it from the fan-out list, enqueues `Response::Ok(())`, then calls
  `Outbound::close_stream(stream)` (order reversed from attach, on purpose:
  here it is safe, even preferable, to stop the writer delivering to a
  stream the client is about to be told is done, before the reply
  confirming that is even on the wire - there is no "told about a stream
  too early" hazard symmetric to attach's, since closing never surprises a
  client with data it wasn't expecting).
- **Named, not silently left**: this task's own change to `connection.rs`'s
  match also has to stop it closing the connection on an `Incoming::Data`
  for a now-open stream (today, every arm except `Incoming::Request` and
  `None` ends the connection outright - confirmed by reading the current
  match). Input only has a real destination once TASK-1.114 lands; until
  then, a client that attaches and then sends input before 1.114 merges
  gets its connection closed on the first such frame - a real, temporary
  gap this task hands to 1.114 explicitly, not a silent regression, since
  nothing in this task's own acceptance criteria sends input on an
  attachment's stream.

**Why this does not contradict TASK-1.46's own ordering rule, stated in one
place rather than left for a reviewer to work out**: 1.46's rule protects
atomicity between `event.watch`'s *own* snapshot and the tree-change events
that follow it, all on stream 0, interleaved with everything else that
connection does - the hazard is a *different* request's reply or an
unrelated broadcast landing out of order *relative to that one subscription's
own promise*. Mine protects atomicity between `pane.attach`'s own snapshot
and the screen updates that follow it, on the attachment's *own*, separate
stream - a completely different pair of frames on a completely different
queue. Nothing stops an `event.watch` reply and a `pane.attach` reply on the
*same* connection interleaving with each other in either order (they are
unrelated requests, matched by id, with no ordering promise between them to
begin with); each rule only ever has to hold *within its own pair*, and nei-
ther rule reads or writes the other's state (`State.subscribers` vs. a
pane's own `PaneHandle`/its worker's `attachments` list).

Expect the predicted merge conflict with TASK-1.46 to land exactly in
`connection.rs`'s read-loop match: both tasks add a `Runtime` method called
directly from `run_post_handshake`'s reader loop rather than from
`dispatch`, for the identical structural reason, and both touch the same
`match connection.poll()`.

`Connection` (sdk) gains a client-side `attach` convenience mirroring
`call` (same validate-then-encode shape, same `(id, bytes)` return) - not
because the wire needs anything `call` cannot already do (a client could
equally call `call("pane.attach", ...)` by hand), but because AC #7 asks
for attaching to be "offered by the steg library," and a named method is
what a caller reaches for instead of hand-building the request. Once the
reply arrives, the caller reads `stream`/`snapshot` from it and calls
`Connection::open_stream(stream)` (already built, `crates/sdk/src/wire/
connection.rs`) itself - the client's own half of the ordering rule: it
only ever opens a stream id the server's reply just gave it, never one it
invented ahead of the reply.

## 2. The tick and the fan-out (AC #1, #3)

`worker_loop`'s command loop gains `next_tick: Option<Instant>` (point 3,
above) and, on every iteration: if `next_tick` is `Some(t)`, receive with
`commands_rx.recv_timeout(t.saturating_duration_since(Instant::now()))`
(recomputed each call, so a flood of arriving commands before `t` still
converges on firing at `t`, not later); else `commands_rx.recv()`, blocking
with no wake-up at all. `PtyBytes`/`Resize` arm `next_tick` (if unarmed and
`!attachments.is_empty()`) after doing what they already do. A timeout
clears `next_tick` and runs the tick body:

1. `terminal.changes_since(tracked_seq)` once (never per-attachment -
   `terminal.rs`'s own doc: "exactly one current point per terminal, not
   one per caller").
2. `Unchanged`: nothing to send, `tracked_seq` unchanged.
3. `Delta`/`Full`: every `CaughtUp(tracked_seq)` attachment gets the
   (re-encoded, see below) delta/full update and moves to `CaughtUp(new
   seq)`; every `NeedsResync` attachment gets a fresh `terminal.screen()`
   (a second, independent, cheap call - viewport-only, per `terminal.rs`'s
   own measurements) tagged with the same new `seq`, and only moves to
   `CaughtUp(new seq)` if `Outbound::enqueue_data` actually queues it.
4. `Outbound::enqueue_data` returning `Full` (decision-2 §6's placeholder,
   already built in `outbound.rs`, waiting on this task to use it for
   real) **or `NotOpen`** (§1's own belt-and-suspenders case - an
   attachment the worker knows about but `Outbound` has not yet been told
   to open, or has already closed) moves that attachment to `NeedsResync`
   and skips it for the rest of this tick - the next tick tries a fresh
   snapshot again, never a delta, until one actually queues. This is the
   whole of AC #3's second half ("a client that falls behind gets one
   fresh screen-sized snapshot, without scrollback"): no scrollback is
   ever part of a resync, by construction, since `terminal.screen()` never
   reads it.

`PaneHandle` gains no new public method for the tick itself - it is internal
to the worker's own loop body.

## 3. Back-pressure never blocks the program (AC #3, first half)

True by construction once §2 is built, not new machinery:
`Outbound::enqueue_data` (`crates/server/src/outbound.rs`) never blocks and
never grows past `DATA_CAP` - it returns `Full` and §2's tick loop treats
that as "mark `NeedsResync`, move on," never as a reason to wait. The flood
test in Proof is what demonstrates this holds under real contention, not a
read of the code.

**Why this differs from TASK-1.46's own back-pressure answer for its control
queue (close the connection), stated so a reviewer does not read it as an
inconsistency**: a control queue's backlog (replies, events) has no single
compacted replacement, so closing is the only cheap answer there; a data
queue's backlog is always exactly "one screen behind," which is what makes
"send one fresh snapshot and skip ahead" both correct and decision-2's own
measured choice (§6: "a client that falls behind gets one fresh
screen-sized snapshot"). Both tasks are consistent with decision-2 §6's
underlying principle (never grow a queue without bound, never block the
producer); they reach different mechanisms because the two queues hold
different kinds of backlog.

## Serialisation: what crosses the wire, and where it lives

`crates/sdk/src/screen.rs`'s own module doc already names this task as the
one that adds it ("whichever task puts these on the wire adds serialisation
at that point - R106"): `#[derive(Serialize, Deserialize)]` on `Screen`,
`Row`/`Run`/`CellStyle`, `Rgb`, `Underline`, `Wide`, `Cursor`, `CursorShape`,
and `ScreenUpdate` (renamed from `Update`, above - the `Delta`/`Full`/
`Unchanged` shape the attachment's own stream carries after the snapshot),
plus the two new reply types `PaneAttached` (above). `boundaries.toml`'s
existing allowance for `serde`/`serde_json` in `steg-sdk` already covers
this (R106) - no new dependency. `RowId`/`Page` stay TASK-1.115's own
(scrollback paging), not touched here. The updates on an attachment's own
non-zero stream are `Kind::Control` frames whose payload is `ScreenUpdate`'s
own JSON, not a `Control` value - `crates/sdk/src/wire/control.rs`'s module
doc already anticipates this ("a screen-mode attachment's update ... no new
`Kind` is needed for that, only a non-zero stream id"), and `Connection::
poll` already hands these back undecoded as `Incoming::StreamControl {
stream, payload }` for whichever code owns the attachment to decode - this
task is simply that code, encoding/decoding `ScreenUpdate` directly rather
than wrapping it in `Control`.

## Files that change

- `crates/server/src/config.rs`: `DEFAULT_ATTACH_TICK`, `ServerConfig::
  attach_tick`.
- `crates/server/src/listen.rs`: thread `config.attach_tick` into its one
  `Runtime::new` call.
- `crates/server/src/runtime.rs`: `Runtime::new` gains `attach_tick:
  Duration`, threaded to every `pane::start` call; two new public methods,
  `attach_pane`/`detach_pane` (§1), called only from `connection.rs` (not
  from `dispatch::dispatch`, which cannot reach `Outbound` - §1's own
  correction), each building and enqueueing its own reply on the `Arc<
  Outbound>` it is given and returning `Result<(), ApiError>` - the same
  shape TASK-1.46's own `Runtime::subscribe` uses, not a `Response` handed
  back for `connection.rs` to enqueue (the previous revision's mistake).
- `crates/server/src/dispatch/mod.rs` or `lookup.rs`: widen `target_pane`
  (and whichever sibling helper resolves a plain pane target) from
  `pub(super)` to `pub(crate)`, so `runtime.rs` can reuse the same target
  -resolution logic `dispatch/pane.rs` already has, rather than duplicate
  it - the one visibility change this task needs outside `runtime.rs`
  itself.
- `crates/server/src/pane.rs`: `recv_timeout`-driven `worker_loop` with
  `next_tick`; `PaneCommand::Attach`/`Detach`; the per-attachment fan-out
  list (`CaughtUp(seq) | NeedsResync`) and the tick-body algorithm (§2);
  `PaneHandle::attach`/`detach` alongside the existing `send_input`/
  `resize`/`snapshot`/`cwd`.
- `crates/server/src/connection.rs`: the read loop's new arm, intercepting
  `request.method == "pane.attach" | "pane.detach"` (after `api.call`
  resolution) *before* the existing `Incoming::Request { .. } => runtime.
  handle(...)` fallthrough; a per-connection stream-id counter and a
  `PaneId -> stream` map beside `Outbound` (so `pane.detach`'s plain pane
  target can be turned into a stream id locally, §"For the engineer" point
  1); on `Err` from either `Runtime` call, building and enqueueing the
  ordinary error reply itself (the `Ok` case needs nothing further -
  `Runtime::attach_pane`/`detach_pane` already enqueued it, §1); the
  match's catch-all arm narrowed so an `Incoming::Data`/`StreamControl`
  frame for a now-open attachment stream does not close the connection
  outright the way every non-`Request` variant does today (named
  limitation for TASK-1.114, above).
- `crates/sdk/src/description/methods.rs`: the two new `Method` entries -
  `pane.attach` (`returns: Nested{of: "PaneAttached", summary: "..."}`,
  `follows: Follows::Attachment{item: Nested{of: "ScreenUpdate", summary:
  "..."}, stream_id_field: "stream"}`) and `pane.detach` (`returns:
  Nothing`, `follows: Follows::Nothing`) - both exactly TASK-1.46's own
  vocabulary (`Returns::Nested`, `Method.follows`/`Follows`), no field of
  this task's own invention; this task's own description-level test (the
  `stream_id_field` check, above), added beside 1.46's `of`-names-a-real
  -type allow-list, with this task's two type names added to it;
  `the_method_set_is_exactly_doc_2_section_8s_42_pairs`'s own count,
  bumped to 44. **`Method::opens_stream`, `Returns::Value` and
  `Returns::Snapshot`/`Method.stream_item`, named in earlier revisions of
  this plan, are all withdrawn** - superseded by 1.46's final
  `Nested`/`Follows`, not layered on top of them.
- `crates/sdk/src/screen.rs`: the serde derives, `PaneAttached`, and the
  `Update` -> `ScreenUpdate` rename (above).
- `crates/kernel/src/terminal.rs`: the one other place `Update` is named
  (`Terminal::changes_since`'s own return type) - updated to `ScreenUpdate`,
  a rename with no behaviour change; its own tests' imports follow.
- `crates/sdk/src/wire/connection.rs`: `Connection::attach` (client-side
  convenience, mirroring `call` - §1).
- `crates/server/src/outbound.rs`: remove the five `#[cfg_attr(not(test),
  expect(dead_code, ...))]` markers on `DATA_CAP`, `EnqueueDataError`,
  `open_stream`, `close_stream`, `enqueue_data` - this task is their first
  real, non-test caller, and an `#[expect]` that stops firing is itself a
  build error (`rules/rust.md`), so these come out as part of wiring attach
  in, not as a separate cleanup step.
- Test-only: new integration tests under `crates/server/tests/` (this
  crate's own convention for anything driving a real `Runtime` + socket)
  and `crates/sdk/tests/` for `Connection::attach`'s own round trip.

## Order of work

1. `crates/sdk/src/screen.rs` derives, proven by a plain round-trip test, no
   server change yet.
2. `ServerConfig::attach_tick`, threaded through `Runtime::new`/`listen.rs`
   with no behavioural change yet (`worker_loop` still ignores it) -
   proven by construction only (the field exists, defaults correctly).
3. `recv_timeout`-driven `worker_loop`, `next_tick`, `PaneCommand::Attach`/
   `Detach`, the per-attachment list - proven first with no real fan-out
   logic beyond "an attachment is registered and later detached," the same
   register-first-prove-later order TASK-1.46's own plan used for its
   subscriber table. This is also where the idle/no-wake-up proof belongs
   (point 3's own test), before anything else depends on the loop's shape.
4. `pane.attach`/`pane.detach` dispatch + the sdk's description entries +
   `Connection::attach` - once TASK-1.46's `Returns::Nested`/`Method.
   follows`/`Follows` have merged; proven by a plain attach/detach round
   trip with no tick activity yet.
   **What this task builds against in the meantime (steps 1-3 do not wait
   on 1.46 at all)**: `PaneCommand::Attach`/`Detach`, the per-attachment
   list, the tick body and its back-pressure are all exercised directly -
   a test constructs `PaneCommand::Attach` by hand and sends it on a
   `PaneHandle`'s own channel, bypassing `Runtime::attach_pane`/the
   description entirely, the same way this crate's own tests already drive
   `PaneCommand`s directly below the dispatch layer elsewhere. Only step 4
   itself - the method wiring, `connection.rs`'s new arm, and `Connection::
   attach` - is blocked on 1.46 merging.
5. The tick body itself (§2): one attachment, no contention, a known write
   producing a known delta; then the new-attachment-caught-up-at-snapshot
   -seq test (point 4).
6. The ordering rule's negative control (§1) before this is called done,
   not after.
7. Back-pressure and the flood test (Proof), last, since it exercises every
   piece at once.
8. `just check` at each numbered step.

## Proof

Built over `steg-sdk`'s own `Connection` driving a real `Runtime` behind a
real `UnixStream::pair()` or socket, a real pty where a pane needs one,
every wait bounded on a real event per `testing-rust.md` - never a sleep for
the tick's own cadence; a test waits for the frame carrying the expected
bytes, or for a counted event, not for "N ms to have passed." Each test's
own recorded failing run (its mechanism disabled or reverted) is captured
on the task, against the test as it stands at that point, before this is
offered as done, per `team:testing`.

- **Attaching returns a snapshot that matches the pane's real screen at
  that moment.** Write known bytes through the existing dispatch path,
  attach, assert the reply's snapshot equals a direct `PaneHandle::
  snapshot()` read taken immediately after. Negative control: have the
  attach reply read a stale cached screen instead of a fresh `terminal.
  screen()` call - fails once a write lands between the two reads.
- **A new attachment is caught up at its own snapshot's sequence, not the
  next tick's.** Attach, then immediately (no tick elapsed) write one more
  distinguishable chunk; assert the very next update this attachment
  receives is a correctly-numbered continuation (`seq` one past the
  snapshot's own), never a duplicate of content already in the snapshot and
  never a gap. Negative control: register the attachment as `NeedsResync`
  instead of `CaughtUp(snapshot.seq)` on registration - the test sees an
  extra, redundant full-screen resync where a plain delta was expected.
- **Updates follow the snapshot in order, with nothing lost and nothing
  doubled.** Attach, then write a sequence of distinguishable chunks (e.g.
  numbered lines faster than the tick); assert every line appears exactly
  once, in order. Negative control: skip a tick's `seq` increment - the
  test catches a dropped or duplicated line.
- **The ordering rule, by a real race.** From a second thread, write output
  to the pane concurrently with the attach call (a `Barrier`, the pattern
  `outbound.rs`'s own flood test already uses), so output can genuinely
  land between "queue the reply" and "open the stream for the writer."
  Assert `unknown_stream_frames == 0` on the client side (surfaced today as
  `Incoming::UnknownStream`) across many repeated runs. Negative control:
  swap the two steps (`Outbound::open_stream` before the reply is queued) -
  decision-2 §3 records this lost 12 lines the first time it was built this
  way; this negative control reproduces that loss directly.
- **No wake-up for an idle or unattached pane (point 3).** Primary
  assertion is a state check, not a timing one: a test-only accessor
  reading whether `next_tick` is armed. Drive one pane with no attachment
  through real output (a real write, awaited the normal way - e.g. via a
  `PaneHandle::snapshot()` read confirming the bytes landed) and assert
  `next_tick` is still `None` afterward; drive a second, attached pane
  through one real tick (so a deadline was armed and fired) and assert
  `next_tick` is `None` again immediately after that tick's own update is
  observed, i.e. it was cleared and never re-armed by the absence of
  further output - never inferred from elapsed wall time. A test-only
  tick-body counter is kept as the secondary assertion (it should read 0
  for the unattached pane across the whole test, and exactly 1 for the
  attached one after its single tick). Negative control: revert to the
  first draft's unconditional `recv_timeout` on every iteration - the
  unattached pane's `next_tick` is never `None` (always freshly armed) and
  its counter climbs past 1 well before the attached pane's own single
  tick is observed.
- **A genuine flood never blocks the program, and a slow attachment gets a
  fresh snapshot, not the backlog.** A real pty flood to a pane with one
  slow attachment (reads deliberately paused) and one healthy one. Assert:
  the healthy attachment sees no gap; the slow one, once resumed, receives
  a full screen (not a backlog of deltas) and then resumes normal deltas;
  the flood's own completion time stays close to an unattached run's
  (proving the program was never blocked on the slow reader). Negative
  control: make `enqueue_data`'s `Full` case block instead of returning an
  error (reverting `outbound.rs`'s existing non-blocking design) - the
  flood's completion time blows out.
- **No thread left.** Every test above checks the pane's worker thread and
  (where a real program ran) its relay thread end cleanly on `PaneHandle::
  stop`/`shutdown`, joined with a bound - the tick's own `recv_timeout`
  loop is the same thread as the worker's existing command loop, so it
  needs no separate join, only the existing one to still succeed with the
  new loop shape. Negative control: call `PaneHandle::stop`/`shutdown`
  while `next_tick` is armed (a deadline pending) rather than only when
  idle - if the new loop shape ever left the command loop unable to notice
  `Shutdown`/`Stop` while blocked in `recv_timeout`, this is the case that
  would hang; reverting the bound join back to a bare `join()` on this
  specific case is the control that would make a real hang show up as a
  failure instead of the test itself timing out silently.
- **The wire shapes round-trip.** `crates/sdk/tests/`: `Screen`/
  `ScreenUpdate`/`PaneAttached` encode and decode back to the same value,
  including a value exercising every field (every `Underline`/`Wide`/
  `CursorShape` variant, `fg`/`bg`/`underline_colour` both `Some` and
  `None`) so a forgotten `#[derive]` on a nested type fails here rather than
  silently serialising as `null`; `Connection::attach`'s own request/reply
  round-trips against a `Description` carrying the new methods. Negative
  control: drop the `#[derive(Deserialize)]` from one nested type (e.g.
  `CellStyle`) - the crate fails to compile, which is itself the proof the
  derive is load-bearing rather than redundant; for the `Connection::attach`
  round trip, temporarily have it skip `validate` - a request naming an
  unknown field or missing a required one then round-trips anyway, which
  the un-reverted test would otherwise never catch.
- **`stream_id_field` names a real field of its own reply type.** Serialise
  a `PaneAttached` value; assert the JSON object has a key equal to
  `"stream"` (the declared `stream_id_field`). Negative control: rename
  `PaneAttached::stream` without updating the description's own string -
  the test fails, proving it actually reads the live struct rather than
  echoing the same literal back at itself.

## Risks / for the engineer

- This task's own dependency on TASK-1.46 is scoped exactly to step 4 of
  "order of work" (the method wiring) - steps 1-3 (the worker loop, the
  tick, back-pressure) build and prove against `PaneCommand` directly, with
  no sdk/description dependency at all - named so the whole task is not
  mistakenly read as blocked.
- The stream-routing question this plan raised in its previous revision is
  now settled by TASK-1.46's own final vocabulary: `Follows::Attachment`'s
  `stream_id_field` names it directly. Nothing left open on that point.
- `connection.rs` and `runtime.rs` are both being touched by TASK-1.46,
  1.98 and 1.86 at the same time (listen.rs, connection.rs's event enum,
  runtime.rs, per the lead's own note) - expect a real merge conflict in
  `State`'s struct literal, `Runtime::new`'s signature, and `connection.
  rs`'s read-loop match (both this task and 1.46 add a `Runtime` method
  called directly from there, §1's own note), not a design problem; this
  task should rebase onto whichever merges first.
- TASK-1.114 and TASK-1.115 both depend on this task merging first; neither
  can start their own build until the attachment list/`PaneCommand::Attach`
  shape here is final, since both add to the same per-attachment state.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Carried from finished tasks (2026-11-15), for this task's plan: (1) TASK-1.74 gave RowId, Update and Page no wire form, like the rest of the screen shapes in the sdk; this task is where they first travel, so it decides the wire form for the whole family. (2) TASK-1.74: changes_since can be called correctly once per tick per terminal; this task fans one tick's Update out to each attachment's own queue. (3) TASK-1.42: client labels are not validated, and ClientNotViewingPane/ClientNotViewingTab map to BadRequest only because no client method existed; decide their codes when a method can return them. (4) TASK-1.76 gives Runtime::apply_size_changed for when a client declares a size or the latest client changes.

Plan (9891d485ef9c) checked against decision-2, doc-2, doc-8 R36, doc-9, doc-15 R91 and doc-16 R101 by @bruk.o, 2026-11-16: not yet ok. (1) For the engineer: the plan makes attach a new kind of control message outside the API description; decision-2 lists four control types (request, response, error, event) and speaks of the attach result, and R36, R42 and R75 say the app uses no surface a plugin cannot see, so whether attach is a described method (with a stated exception to R32 for the CLI) or protocol outside the description is the engineer's decision; the orchestrator recommends a described method sent as an ordinary request. (2) Must change: the worker loop wakes every tick for every pane for ever, attached or not, changed or not; it must block with no timeout when there is no attachment or nothing has changed since the last tick, and arm one deadline when output arrives. (3) Must be stated: the state a new attachment is registered in (caught up at the snapshot's sequence), so it is not sent a second snapshot on the next tick. (4) For the engineer: the tick as a published user setting or a server configuration value until the app measures it; the orchestrator recommends the latter. The split into three is taken (drafts created). Held for the engineer's answers, then one rewrite.

Plan (9cff507ccc3f) checked against the three acceptance criteria as narrowed, decision-2 sections 3 and 6, doc-8 R36, doc-9, doc-16 R101, TASK-1.46's plan (4ee3881365b0) and the team's testing rules by @bruk.o, 2026-11-16: ok. Revised to the engineer's decisions (attach and detach as described methods; the tick 8 ms in ServerConfig) and at the orchestrator's request (no wake-up for an idle or unattached pane, with a counted proof; a new attachment registered as caught up at its snapshot's sequence; the reply queued in the same step that registers the attachment, as Runtime::subscribe does). Uses TASK-1.46's vocabulary: returns Nested{of: PaneAttached}, follows Attachment{item: Nested{of: ScreenUpdate}, stream_id_field: stream}. Depends on TASK-1.46 merging first for that vocabulary; the worker loop, tick and back-pressure are built and proven below the method layer meanwhile. doc-2 section 8 gains two rows when the plan is approved.
<!-- SECTION:NOTES:END -->

## Comments

<!-- COMMENTS:BEGIN -->
created: 2026-11-16T11:54:00Z

---

2026-11-16, @bruk.o: the engineer decided as recommended: attach is a described API method sent as an ordinary request and marked as opening a stream (an exception to R32 for the generated CLI); the tick is 8 ms as a server configuration value, not a published setting. The plan is being rewritten once; doc-2 gains a line when the plan is approved.

---

created: 2026-11-16T12:05:00Z

---

2026-11-16, @bruk.o: criteria narrowed to match the split the engineer accepted (TASK-1.114 has input and the declared size; TASK-1.115 has scrollback pages, the restore proof and the spike's deletion). Depends on TASK-1.46, which decides how the API description states a reply that carries a nested record and a method that opens a stream.

---

author: @bruk
created: 2026-11-16T12:47:00Z

---

Plan approved by @bruk (9cff507ccc3f, size-5): its worker may build it.

---
<!-- COMMENTS:END -->
