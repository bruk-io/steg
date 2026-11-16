---
id: TASK-1.47
title: >-
  Server: attach to a pane, stream its screen, and restore it after a client
  goes away
status: In Progress
assignee:
  - '@bruk.w4'
created_date: '2026-11-15T01:33:00Z'
updated_date: '2026-11-16T11:54:00Z'
labels:
  - size-5
milestone: m-3
dependencies:
  - TASK-1.45
  - TASK-1.42
  - TASK-1.40
  - TASK-1.74
  - TASK-1.76
parent_task_id: TASK-1
type: task
project: bruk-io-steg
asdlc_accepted_from: DRAFT-17
asdlc_branch: task-1.47-server-attach-to-a-pane-stream-its
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
doc-8 R2, doc-9 R15, doc-15 R91. decision-2, sections 3, 5 and 6, including its two ordering rules and its back-pressure policy.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Attaching returns a snapshot, and updates follow it in order with none lost and none sent for a stream the client has not been told about
- [ ] #2 Scrollback is fetched in pages by absolute row identity
- [ ] #3 A client that falls behind gets one fresh screen-sized snapshot, without scrollback; the program is never blocked
- [ ] #4 Input arrives on the stream the client attached with; each attachment declares a size, and an update reports the terminal's size when it changes
- [ ] #5 A client killed mid-output and a new one attaching afterwards ends up with the server's screen and scrollback byte for byte, as an automated test
- [ ] #6 `spikes/task-1.2-wire-protocol/` is deleted
- [ ] #7 Attaching, input, scrollback pages and declaring a size are offered by the steg library, and this task's tests go through it (R101)
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
## Split this into three reviewable changes — proposed at the top, not asked

This task's own AC #1–7 need three things that can be built and proven in order,
not one: (A) the attach mechanism itself — a stream, a snapshot, a per-pane tick
that calls `Terminal::changes_since` once and fans the one result out, and the
non-blocking back-pressure decision-2 §6 already measured; (B) input and a
declared size arriving on that stream, which only means anything once (A)
exists; (C) scrollback paged by absolute row identity, the real (not
screen-sized-only) resync policy once paging exists, and the byte-for-byte
restore proof that needs paging to compare scrollback at all. Recommended
split, filed as two drafts once this plan is approved:

- **TASK-1.47 (this task, narrowed to AC #1, both halves of AC #3, AC #7 for
  attach only):** opening an attachment, the snapshot-then-updates sequence
  and its ordering rule, the tick, fan-out to every attachment on a pane, and
  back-pressure down to "the program is never blocked" and "a client that
  falls behind gets one fresh screen-sized snapshot" (no scrollback involved
  in a resync, so this half needs nothing from paging).
- **TASK-1.47b (new, depends on 1.47): input and size from an attached
  client (AC #4).** Bytes arriving on the attachment's stream reach the
  program; a declared size reaches the kernel's existing `Tree::declare_size`/
  `note_input` (already built, see below) and is reapplied. Boundary stated
  explicitly so nobody re-decides it: this is *not* `pane.send` (TASK-1.90,
  a one-shot request/reply with no attachment at all) and declaring an
  attachment's size is *not* `pane.resize` (moving a layout border,
  TASK-1.110).
- **TASK-1.47c (new, depends on 1.47, probably 1.47b): scrollback paging,
  the restore proof, and the spike's deletion (AC #2, #5, #6).** `Terminal::
  page`/`oldest_row` already exist (TASK-1.74); this task wires them onto the
  wire and writes the killed-client/reattach oracle this task's own AC #5
  demands. AC #6 (deleting `spikes/task-1.2-wire-protocol/`) waits for this
  piece specifically: decision-2 §5 says the oracle there was "written before
  the run... code, not judgement," and it is the direct source for this
  task's own restore proof — deleting it before porting what it checks loses
  the reference. Also: decision-2's own record cites that path by name, which
  the deletion makes stale; TASK-1.82 ("stale decision text") already owns
  fixing decision prose, so this task's plan does not touch it, only names
  the dependency.

If the engineer would rather keep it as one task, say so and I build it as
one; every design decision below still applies, just not as three PRs.

## For the engineer

Four decisions this plan needs settled before any code is written — three are
genuinely open, one is a confirmation that something already decided by an
earlier task's own code stays decided.

### 1. Where the tick lives, and its starting value

`Terminal::changes_since` must run on the pane's own worker thread (the engine
is `!Send`; `crates/kernel/src/terminal.rs`'s own doc: "its job is to call
this once per tick and fan the one result out"). Today `worker_loop`
(`crates/server/src/pane.rs`) is a blocking `for command in commands_rx` with
no cadence of its own. Streaming needs a clock inside that loop: swapping the
blocking receive for `commands_rx.recv_timeout(tick)`, running the tick body
(call `changes_since` once, fan out, see §3) whenever that unblocks, whether
by a command arriving or by timing out.

The tick's *value* is explicitly unsettled in two places this task rests on:
doc-9's open questions ("8 ms leaves too little of R19's budget... wants a
real window in front of it") and decision-2 §9 ("the answer wants a GPUI
client in front of it, which is TASK-1.16" — M1b, not this milestone). This
plan cannot settle the number, only avoid blocking on it: make it a server
setting (`Settings`/`steg_sdk::settings()`, the mechanism decision-9 and
TASK-1.48 already built — `pub fn new(cols...)`-style, no wiring needed
beyond one schema entry), default **8 ms**, decision-2 §4.1's own measured,
non-flood-prone number, and leave it to the app's own task to lower it once a
real window is measuring frames. **My recommendation:** 8 ms as the default,
settable, revisited in M1b — not 1 ms (255 resyncs under the spike's own
flood) and not left hard-coded (doc-9's own open question is explicit that
this wants revisiting).

### 2. Whether attach, input and a declared size are API methods at all

They are not, and I recommend building them that way on purpose, for a reason
that is architectural rather than a vocabulary gap: `Response::Ok(Value)` is
already arbitrary JSON (`crates/sdk/src/description/request.rs`), so nothing
about the *wire* stops a method's reply from carrying a nested snapshot — the
blocker is R32 ("the CLI exposes everything the API exposes... mechanically")
and the `Returns` enum that mechanism reads (`Nothing | Identifier | Record |
Records | Text`), which only exists to tell the generated CLI how to print a
result. Attach has no sensible one-shot CLI rendering to generate in the
first place — it is inherently a long-lived stream, and neither TASK-1.50 nor
TASK-1.90 (the CLI/server one-shot `pane send`/`capture`/`wait` tasks; see §4)
asks for a `pane attach` command. Making attach a described method would force
doc-2 to grow a verb no CLI user calls, and `methods.rs`'s own
`the_method_set_is_exactly_doc_2_section_8s_42_pairs` completeness test would
need an exception carved into it anyway.

So: attach, the updates that follow it, input and a declared size are
protocol logic (`crates/sdk/src/wire/`, R101: "the protocol's logic... each
stream's state"), not API methods — the same reasoning that already put
`Control::Event` outside the method/`Description`/`validate` system
entirely. Concretely:

- `Control` (`crates/sdk/src/wire/control.rs`) gains new tagged variants —
  `Attach { id, pane, size }`, `Attached { id, stream, snapshot }`,
  `AttachRefused { id, error: ApiError }` — decoded on stream 0 exactly like
  `Request`/`Reply` are today, but never touching `Description`/`validate`/
  `dispatch`. `Connection::poll` gains matching `Incoming` variants.
  `Connection` gains `attach`/`detach`-building methods on the client side,
  mirroring `call`/`reply`.
- The updates that follow (`Update::Delta`/`Full`/`Unchanged`, a `Screen`
  snapshot) are a new small wire shape of their own
  (`crates/sdk/src/wire/attach.rs` or similar), carried as `Kind::Control`
  frames on the attachment's own non-zero stream — `wire/control.rs`'s
  existing doc comment already names this shape ("no new `Kind` is needed
  for that, only a non-zero stream id") and `Connection::poll` already
  returns these undecoded, as `Incoming::StreamControl { stream, payload }`
  — this task only has to define and decode the payload.
- The sdk types in `crates/sdk/src/screen.rs` (`Screen`, `Row`, `Run`,
  `CellStyle`, `Cursor`, `RowId`, `Update`, `Page`, ...) get
  `#[derive(Serialize, Deserialize)]`: the module's own doc comment already
  names this as the next task's job ("Whichever task puts these on the wire
  adds serialisation at that point"), and `boundaries.toml`'s existing
  allowance for `serde`/`serde_json` in `steg-sdk` already covers it (R106).
- Still discoverable and still one API (R36): nothing here hides the
  mechanism from a plugin reading the protocol directly — R42/R75's
  dependency-free Python client reads `steg-sdk`'s wire shapes the same way
  it reads the hello today, with no generated code either way.

**This overlaps TASK-1.46 exactly once.** Its own plan hits the identical
problem for `event.watch`'s reply (a nested tree snapshot that does not fit
`Returns` either) and proposes extending `Returns` with a new vocabulary
instead of stepping outside the method system. The two problems look alike
but are not the same shape: `event.watch` *is* a CLI command today (`steg
event watch`, doc-2 §8) that genuinely wants its one-time reply pretty-printed
somehow, so staying inside the method system and growing `Returns` is the
right call there. Attach is not a CLI command and never claims to be. I am
not asking 1.46 to change its own approach — only naming that the two tasks
solve a superficially similar problem differently, for a reviewer who reads
both plans back to back, so neither is mistaken for copying the other
inconsistently.

### 3. Fan-out and back-pressure, per attachment, each tick

Already measured and already partly built; stated here as the mechanism this
plan uses, not as a question. Per pane, the worker thread holds
`attachments: Vec<Attachment>`, each an `Arc<Outbound>` (the connection's
existing writer queues, `crates/server/src/outbound.rs`), its stream id, and
one of two states: `CaughtUp(seq)` or `NeedsResync`. Each tick:

1. Call `terminal.changes_since(tracked_seq)` once (the worker's own tracked
   point, not one per attachment — `terminal.rs`'s own doc is explicit that
   there is "exactly one current point per terminal, not one per caller").
2. `Unchanged`: nothing to send.
3. `Delta { rows, cursor, seq }`: every `CaughtUp(tracked_seq)` attachment is
   sent this exact delta (`Outbound::enqueue_data`) and moves to
   `CaughtUp(seq)`; every `NeedsResync` attachment is sent a fresh screen
   (`terminal.screen()`, a second, independent call — cheap, viewport-only,
   confirmed by `terminal.rs`'s own measurements) tagged with the same `seq`,
   and moves to `CaughtUp(seq)` only if that send actually queues (see below).
4. `Full { screen, seq }` (the engine itself resynchronised, e.g. after a
   resize): every attachment gets the full screen and moves to
   `CaughtUp(seq)`.
5. A brand-new attachment (registered since the last tick, so its state is
   `NeedsResync` from the start) gets a fresh `terminal.screen()` on the very
   next tick, by the same path as 3's resync branch — this is what makes the
   *attach* snapshot itself (the `Attached` reply's own payload) and the
   first screen update consistent: the reply's snapshot is built from the
   same `terminal.screen()` call, under the same per-pane worker-thread
   serialisation, as the ordering rule in §5 below requires.
6. `Outbound::enqueue_data` returning `Full` (decision-2 §6, already the
   policy `outbound.rs` has a placeholder for: "nothing more is queued for
   that attachment until one fresh snapshot is") moves that attachment to
   `NeedsResync` and skips it for the rest of this tick — the next tick tries
   a fresh snapshot again, never a delta, until one actually queues.

**This is not the same back-pressure outbound.rs's own doc already names for
TASK-1.46's control queue** (close the connection on `Full`) **and that is
correct, not an inconsistency to flag**: a control queue's backlog (replies,
events) has no single compacted replacement, so closing is the only answer;
a data queue's backlog is exactly one screen behind, which is what makes
"send one fresh snapshot and skip ahead" cheap and already decision-2's own
measured answer. The plan states the divergence here so a reviewer comparing
the two tasks does not read it as copied wrong.

### 4. Confirmed, not reopened: size is declared per tab, and nothing is torn down with no client attached

Already decided by TASK-1.42's own kernel code, not this task's to re-litigate:
`Tree::declare_size(client, tab, size)`'s own doc comment states plainly that
"this task's plan settles doc-14's per-attachment wording as the report
direction, not the accept direction: what a client declares in is the tab
total, what it is told is each pane's size through `SizeChanged::panes`."
`Tree::view_pane`/`stop_viewing`/`declare_size`/`note_input` and
`Runtime::apply_size_changed` already exist and are already proven by
`crates/kernel/src/tree.rs`'s and `crates/server/src/runtime.rs`'s own tests;
this task (well, 1.47b) only has to call them from the attach/detach/input
paths, not design them. `Runtime` itself has no `view/declare_size/note_input`
wrapper methods yet (only `apply_size_changed`/`connect_client`/
`disconnect_client` do) — those three thin wrappers are 1.47b's own addition,
same shape as the existing ones.

"What is kept when no client is attached, and what restore means" is also
already true by construction, not machinery this task adds: a pane's worker
thread and its one `Terminal` live for the pane's whole life regardless of
any attachment (`crates/server/src/pane.rs`'s own module doc), so there is
nothing to tear down and nothing to restore beyond the stream, the per
-attachment queue and that attachment's own `seq` tracking — all of which are
connection-scoped and already gone once the connection is. AC #5 falls out of
this design rather than needing its own state machine; TASK-1.47c's job is
proving it with a real kill-and-reattach, not building a restore path.

### Declined, and named so it is not silently dropped

TASK-1.46's own plan names this task "the natural owner of `pane.cwd_changed`/
`pane.title_changed` detection... it already reads live terminal state every
tick for the screen." No acceptance criterion here asks for it, and this task
is already a size-5 split three ways; I am not absorbing it. Flagging it back
to the orchestrator to file as its own small task once 1.46 lands, rather than
letting it quietly vanish between the two tasks' plans.

## 1. Opening an attachment, and the snapshot it starts from (AC #1, #7)

`Connection::attach` (client side) sends `Control::Attach { id, pane, size }`
on stream 0 and returns the id to match against `Incoming::Attached`/
`AttachRefused`. Server side, `connection.rs`'s own read loop gets a new arm:
look up the pane's `PaneHandle` under `Runtime`'s lock, call
`tree.view_pane(client, pane)` (kernel; errors map to `ApiError` the same way
`dispatch::map_kernel_error` already does elsewhere), mint the next stream id
for this connection (a small per-connection counter beside `Outbound`, not a
kernel concern), send a new `PaneCommand::Attach(Arc<Outbound>, stream)` to
the pane's worker thread and wait for its reply (a `terminal.screen()` read,
the same shape `PaneCommand::Snapshot` already uses). **The two-step ordering
rule decision-2 §3 already names, learned the hard way once** ("queue the
attach result first, register the attachment's queue second"): the
`Attached` reply is queued on `Outbound` *before* the pane's worker thread
registers this attachment in its own fan-out list — concretely, the worker's
reply to `PaneCommand::Attach` includes the snapshot and the stream id, the
connection enqueues `Attached` on `Outbound`'s control queue, *then* a second,
separate step tells the worker "now start fanning updates to this
attachment" (or: the worker registers the attachment only after sending its
own reply, inside the same command handler — either shape works, chosen
during implementation, but the test in §9 is what proves whichever one is
built actually holds the rule).

## 2. The tick and the fan-out (AC #1, #3)

`worker_loop`'s blocking `for command in commands_rx` becomes a
`recv_timeout(tick)` loop; §"For the engineer" §1/§3 above give the value and
the per-attachment algorithm. `PaneHandle` gains no new public method for
this — it is entirely internal to the worker's own loop body, run after every
command *and* on every timeout, not only after `PtyBytes`.

## 3. Back-pressure never blocks the program (AC #3, first half)

Already true by construction once §2 is built: `Outbound::enqueue_data`
(`crates/server/src/outbound.rs`) never blocks and never grows past
`DATA_CAP` — it returns `Full` and the tick loop above treats that as "mark
`NeedsResync`, move on," never as a reason to wait. The flood test in §9 is
what proves this holds under real contention, not a read of the code.

## Files that change

- `crates/server/src/pane.rs`: `recv_timeout`-driven `worker_loop`; a tick
  constant/parameter threaded in from `Runtime`; `PaneCommand::Attach`/
  `Detach`; the per-attachment fan-out list and algorithm (§"For the
  engineer" §3); `PaneHandle::attach`/`detach` (or equivalent) alongside the
  existing `send_input`/`resize`/`snapshot`/`cwd`.
- `crates/server/src/connection.rs`: a new arm in the read loop for
  `Incoming::Request`'s sibling, whatever `Connection::poll` returns for the
  new `Control::Attach` (name depends on the exact `Incoming` variant chosen
  in §"For the engineer" §2); a per-connection stream-id counter; the
  ordering-rule sequencing itself.
- `crates/sdk/src/wire/control.rs`: new `Control` variants (`Attach`/
  `Attached`/`AttachRefused`).
- `crates/sdk/src/wire/connection.rs`: matching `Incoming` variants; `poll`
  wiring; `Connection::attach`-building method (client side), mirroring
  `call`.
- `crates/sdk/src/screen.rs`: `#[derive(Serialize, Deserialize)]` on every
  shape that crosses the wire (`Screen`, `Row`/`Run`/`CellStyle`, `Rgb`,
  `Underline`, `Wide`, `Cursor`, `CursorShape`, `Update`, `RowId` - the last
  only if this task, rather than 1.47c, ends up needing it for the snapshot's
  own `rows_ever`/cursor; `Page` stays 1.47c's).
- `crates/server/src/outbound.rs`: remove the five `#[cfg_attr(not(test),
  expect(dead_code, ...))]` markers on `DATA_CAP`, `EnqueueDataError`,
  `open_stream`, `close_stream`, `enqueue_data` — this task is their first
  real, non-test caller, and an `#[expect]` that stops firing is itself a
  build error (`rules/rust.md`), so these come out as part of wiring attach
  in, not as a separate cleanup step.
- `crates/sdk/src/setting.rs`/`steg_sdk::settings()`: one new setting schema
  entry for the tick, default `8` (ms).
- Test-only: new integration tests under `crates/server/tests/` (this crate's
  own convention for anything driving a real `Runtime` + socket) and
  `crates/sdk/tests/` for the new `Control`/`Connection` wire shapes in
  isolation.

## Order of work

1. Confirm "For the engineer" §1 (tick value/location) and §2 (attach as
   protocol logic, not a method) with the engineer before writing code —
   both change the shape of everything after them, and §2 especially is not
   reversible cheaply once `Control`'s wire shape ships.
2. `crates/sdk/src/screen.rs` derives, and the new `Control` variants plus
   `Incoming` wiring in isolation (`crates/sdk/tests/`), with no server
   change yet — the wire shape round-trips before anything depends on it.
3. `recv_timeout`-driven `worker_loop`, `PaneCommand::Attach`/`Detach`, the
   per-attachment list — proven first with no real fan-out logic beyond
   "an attachment is registered and later unregistered," the same
   register-first-prove-later order TASK-1.46's own plan used for its
   subscriber table.
4. The tick body (§2 above): one attachment, no contention, a known write
   producing a known delta.
5. The ordering rule (§1): the negative control in §9 before this is called
   done, not after.
6. Back-pressure and the flood test (§9), last, since it exercises every
   piece at once.
7. `just check` at each numbered step.

## Proof

Built over `steg-sdk`'s own `Connection` driving a real `Runtime` behind a
real `UnixStream::pair()` or socket, a real pty where a pane needs one, every
wait bounded on a real event per `testing-rust.md` — never a sleep for the
tick itself; a test waits for the *frame carrying the expected bytes*, not
for "N ms to have passed." Each test's own recorded failing run (its
mechanism disabled or reverted) is captured on the task before this is
offered as done, per `team:testing`.

- **Attaching returns a snapshot that matches the pane's real screen at that
  moment.** Start a pane, write known bytes to it through the existing
  dispatch path, attach, assert the `Attached` reply's snapshot equals a
  direct `PaneHandle::snapshot()` read taken immediately after. Negative
  control: have the attach reply read a stale cached screen instead of a
  fresh `terminal.screen()` call — the test fails once a write lands between
  the two reads.
- **Updates follow the snapshot in order, with nothing lost and nothing
  doubled.** Attach, then write a sequence of distinguishable chunks (e.g.
  numbered lines faster than the tick) and assert every line appears exactly
  once, in order, across the delta stream — the same "no gap, no repeat"
  oracle decision-2 §5 already used. Negative control: skip a tick's `seq`
  increment (simulate the bug the terminal module's own doc warns against,
  more than one independent caller of `changes_since`) — the test catches a
  dropped or duplicated line.
- **The ordering rule, by a real race.** From a second thread, write output
  to the pane concurrently with the attach call (a `Barrier`, the same
  pattern `outbound.rs`'s own flood test already uses), so output can
  genuinely land between "queue the reply" and "register the attachment."
  Assert `unknown_stream_frames == 0` on the client side (surfaced today as
  `Incoming::UnknownStream` — decision-2 §3's own invariant) across many
  repeated runs. Negative control: swap the two steps (register the
  attachment before queuing the reply) — decision-2 §3 records this lost 12
  lines when it was built this way the first time; this test's negative
  control reproduces that loss directly, with `unknown_stream_frames > 0` or
  a dropped line, not inferred.
- **A genuine flood never blocks the program, and a slow attachment gets a
  fresh snapshot, not the backlog.** A real pty flood (decision-2's own
  100 MB-class shape, scaled to what the test budget allows) to a pane with
  one slow attachment (reads deliberately paused) and one healthy one.
  Assert: the healthy attachment sees no gap; the slow one, once resumed,
  receives a full screen (not a backlog of deltas) and then resumes normal
  deltas; the flood itself completes in wall-clock time close to an
  unattached run (proving the program was never blocked waiting on the slow
  reader). Negative control: make `enqueue_data`'s `Full` case block instead
  of returning an error (reverting `outbound.rs`'s existing non-blocking
  design) — the flood's own completion time blows out.
- **No thread or child left.** Every test above checks the pane's worker
  thread, its relay thread (if a real program ran) and the tick's own
  `recv_timeout` loop all end cleanly on `PaneHandle::stop`/`shutdown` —
  joined with a bound, per this crate's existing convention, never a bare
  `join()` and never a sleep-based poll.
- **The wire shapes round-trip.** `crates/sdk/tests/`: every new `Control`
  variant and the attachment-stream update payload encode and decode back to
  the same value, including a payload fed one byte at a time and in one
  piece (mirroring `probe-wire/tests/framing.rs`'s own style), proving the
  framing layer still never looks inside the payload.

## Risks / for the engineer

- §"For the engineer" §1 and §2 are the two decisions that have to be made
  before any code is written; everything else here is written to be correct
  either way those land, except the wire shape itself, which §2 commits to
  concretely once approved.
- The split (top of this plan) is proposed, not assumed; if rejected,
  1.47b/1.47c's scope becomes this task's own continuation instead, same
  content, same order of work.
- `connection.rs`'s read loop and `runtime.rs`'s `State` are both being
  touched by TASK-1.46 at the same time (its own plan already names
  `runtime.rs`'s `State` as edited by three tasks at once) — expect a real
  merge conflict in both files, not a design problem; this task should
  rebase onto whichever of 1.46/1.78/1.87 merges first, per their own plans'
  stated order.
- `pane.cwd_changed`/`pane.title_changed` (declined above) should not be
  forgotten once this task merges — flagging for the orchestrator to file,
  not for this task to build.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Carried from finished tasks (2026-11-15), for this task's plan: (1) TASK-1.74 gave RowId, Update and Page no wire form, like the rest of the screen shapes in the sdk; this task is where they first travel, so it decides the wire form for the whole family. (2) TASK-1.74: changes_since can be called correctly once per tick per terminal; this task fans one tick's Update out to each attachment's own queue. (3) TASK-1.42: client labels are not validated, and ClientNotViewingPane/ClientNotViewingTab map to BadRequest only because no client method existed; decide their codes when a method can return them. (4) TASK-1.76 gives Runtime::apply_size_changed for when a client declares a size or the latest client changes.

Plan (9891d485ef9c) checked against decision-2, doc-2, doc-8 R36, doc-9, doc-15 R91 and doc-16 R101 by @bruk.o, 2026-11-16: not yet ok. (1) For the engineer: the plan makes attach a new kind of control message outside the API description; decision-2 lists four control types (request, response, error, event) and speaks of the attach result, and R36, R42 and R75 say the app uses no surface a plugin cannot see, so whether attach is a described method (with a stated exception to R32 for the CLI) or protocol outside the description is the engineer's decision; the orchestrator recommends a described method sent as an ordinary request. (2) Must change: the worker loop wakes every tick for every pane for ever, attached or not, changed or not; it must block with no timeout when there is no attachment or nothing has changed since the last tick, and arm one deadline when output arrives. (3) Must be stated: the state a new attachment is registered in (caught up at the snapshot's sequence), so it is not sent a second snapshot on the next tick. (4) For the engineer: the tick as a published user setting or a server configuration value until the app measures it; the orchestrator recommends the latter. The split into three is taken (drafts created). Held for the engineer's answers, then one rewrite.
<!-- SECTION:NOTES:END -->

## Comments

<!-- COMMENTS:BEGIN -->
created: 2026-11-16T11:54:00Z

---

2026-11-16, @bruk.o: the engineer decided as recommended: attach is a described API method sent as an ordinary request and marked as opening a stream (an exception to R32 for the generated CLI); the tick is 8 ms as a server configuration value, not a published setting. The plan is being rewritten once; doc-2 gains a line when the plan is approved.

---
<!-- COMMENTS:END -->
