---
id: TASK-1.46
title: 'Server: the tree, and events a client subscribes to'
status: In Progress
assignee:
  - '@bruk.w2'
created_date: '2026-11-15T01:33:00Z'
updated_date: '2026-11-16T11:58:00Z'
labels:
  - size-3
milestone: m-3
dependencies:
  - TASK-1.45
  - TASK-1.89
parent_task_id: TASK-1
type: task
project: bruk-io-steg
asdlc_accepted_from: DRAFT-16
asdlc_branch: task-1.46-server-the-tree-and-events-a-client
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
doc-8 R38 and doc-14. A client asks for the tree once and then receives changes.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 One request returns the tree: sessions, tabs and panes with their focus fields, layout and status, as one coherent read; a client that subscribes gets that tree and then every event after it, with nothing lost or doubled between the two
- [ ] #2 Events report sessions, tabs and panes created and closed; a focus change is one event carrying the whole path; a pane's program exiting and a pane's status changing are events. Each is caused by every place the tree changes that way, whether from a request, a program's exit or a typed command's start and end, and reaches every subscribed client in order
- [ ] #3 A subscription can be narrowed to named events on the server, and ended; a client that subscribes and does not read is dealt with as decision-2 and the plan say, without delaying any other client or holding the runtime's lock while writing
- [ ] #4 Subscribing, the tree and the events are offered through the sdk, so the CLI and the app do not each write them (R101), and this task's tests go through it over a real socket
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
## For the engineer

Rewritten for part A alone, over `origin/main` as of 2026-11-16 (TASK-1.45, 1.78,
1.87, 1.89 all merged since the first draft; TASK-1.98 and TASK-1.86 are being
built now and are named by overlap below, not merged). TASK-1.109 holds part B
(layout/session/tab-changed/client events, `session.focus`) and already cites this
task's old plan for its event names; `dispatch/session.rs`'s own `"focus"` arm now
points at TASK-1.109 by name, confirming the split landed as proposed.

Decisions made 2026-11-16, recorded here as settled, not reopened below:

1. The tree travels as the `Response` to `event.watch` (§1(a) of the first draft).
2. Event names for part B were confirmed as proposed (`tab.layout_changed`,
   `session.changed`, `tab.changed`, the four `client.*` events,
   `session.focus_requested`) — nothing part A adds a new `Event` record for
   (part A's events — created/closed/focused/exited/status_changed — are already
   all fourteen of the description's existing M1 events).
3. A subscriber that does not read has its connection closed (§7 below).
4. Status-change events are M1; R52 in doc-11 is corrected through the board
   (M1 for the events, M2 only for the CLI's own listing of status records).

One thing new since the first draft, not yet asked of the engineer, resolved
below rather than carried as open: TASK-1.89's reviewer asked whether the tree
snapshot states a pane's identity twice, since `TabRecord.layout` already names
every pane in the tab. §2 answers it: no, by construction, not by a new rule.

## 1. What's merged since the first draft, read before writing any of this

- **TASK-1.89**: `steg_sdk::{TabRecord, PaneRecord}` now exist in full
  (`crates/sdk/src/record.rs`), with `PaneRecord.{kind, program_status, status,
  size}` and `TabRecord.{layout, shown_status}`. `dispatch::dispatch` and every
  per-noun `dispatch`/`new`/`show`/`list` now take `now: steg_kernel::Moment`,
  read once by the caller and passed down, never read by the kernel. The record
  builders this task needs already exist, one per noun, each private to its own
  module: `session::record_at(sessions, index)`, `tab::record_for(tree, tab,
  position, now)`, `pane::record_for(tree, tab_id, pane, now)`. §2 makes each
  `pub(super)` rather than writing a second copy.
- **TASK-1.78**: `Runtime::handle` now intercepts `pane.close`/`tab.close`/
  `session.close` into `close_and_dispatch` (`runtime.rs:587`), which closes the
  tree **first**, under the lock, then drops the lock and ends any running
  program afterward (`force_end`). Confirmed by reading the merged code: the
  close event for part A (`pane.closed`/`tab.closed`/`session.closed`, via
  `apply_event_locked`'s cascade, and the direct close arms) fires at the tree
  mutation, inside `close_and_dispatch`'s own locked section, before `force_end`
  ever runs — exactly the timing TASK-1.78's own plan already assumed for this
  task ("such an event would correctly describe tree state... not process
  lifecycle").
- **TASK-1.87**: `Runtime::new` gained a `force_grace: Duration` parameter
  (TASK-1.78) and `LoginShell` gained `user`/`base_env` (TASK-1.87) — neither
  touches anything this task reads or writes. Named only because `runtime.rs`'s
  `State`/`Runtime::new` are now a well-travelled seam; this task adds one field
  (`subscribers`) and two methods to `State`/`Runtime`, nothing to their existing
  fields or constructor signature.
- **TASK-1.98** (in progress): adds `ConnectionEvent::AcceptRetried(...)`, an
  additive variant on the enum `connection.rs` already defines, sent from the
  accept loop in `listen.rs`. This task never matches on `ConnectionEvent`
  (confirmed: `grep -rn "ConnectionEvent" crates/server/src` today has no
  `match` on it anywhere this task touches), so there is nothing to rebase
  around beyond the ordinary textual merge in `listen.rs`, which this task does
  not edit at all.
- **TASK-1.86** (in progress): adds `ConnectionEvent::SessionCountChanged`, a
  *different* "event" from the ones this task builds — an internal liveness
  signal from `Runtime`/`connection::handle` to `listen::run`'s own idle-checker
  thread, carried on `Sender<ConnectionEvent>`, never seen by any API client.
  Named explicitly so a reviewer does not conflate it with `pane.closed`/etc:
  this task's own events are wire-level, carried on a subscribed connection's
  `Outbound` as `Control::Event` frames (decision-2 §3); `ConnectionEvent` is
  server-internal bookkeeping with its own, separate channel. No shared code,
  no shared type, no ordering dependency between them.
- **TASK-1.47** (being planned): relies on two of this task's own decisions and
  needs them stated side by side, not contradicted later — a *control*-queue
  overflow (this task: a slow event subscriber) closes the connection; a
  *data*-queue overflow (TASK-1.47: a slow screen-attachment subscriber) means a
  fresh snapshot is queued instead, per decision-2 §6. Two different queues
  (`outbound.rs`'s `control`/`data`), two different remedies, both already true
  of `Outbound` today — this task does not change that module at all, only adds
  callers of `enqueue_control`.

## 2. The tree snapshot (AC #1): shape, and the pane-identity question

Nested, one level at a time: the snapshot is every session (via
`session::record_at`, made `pub(super)`), each carrying its tabs (via
`tab::record_for`, made `pub(super)`), each carrying its panes (via
`pane::record_for`, made `pub(super)`) — reusing the exact record builders
`session.list`/`tab.list`/`pane.list` already use, not a second definition of
any of the three records. Concretely, a new `dispatch/event.rs` function:

```rust
fn tree_snapshot(tree: &Tree, now: Moment) -> Value // one JSON value, embedding
// SessionRecord[] -> each with tabs: TabRecord[] -> each with panes: PaneRecord[]
```

**The pane-identity question, answered, not left open:** `TabRecord.layout` is a
tree of splits whose leaves are `PaneId` (`crates/sdk/src/layout.rs`) — it says
*where* each pane sits in the tab's split tree, a structural reference, the same
role `SessionRecord.current_tab: TabId` and `TabRecord.current_pane: PaneId`
already play elsewhere in the flat API: naming a sibling by id, not embedding its
data. The snapshot's own `panes: Vec<PaneRecord>` under each tab is the one place
that pane's actual data (status, program, size...) lives; a client resolves a
`layout` leaf's id against that same tab's own `panes` array, exactly as it
already must resolve `current_pane`/`current_tab` against sibling arrays today.
Nothing new is stated twice — `layout` and `panes` answer two different
questions ("what shape" and "what is each one"), and removing either loses
information the other doesn't carry. No new resolution rule for a client to
learn; the existing "an id names something in the array next to it" convention
already covers this one level deeper.

`event.watch`'s own `details` gains one sentence: the method's `Response` is
this snapshot, taken under the same lock the subscription is registered in (§4);
everything that follows on the connection afterward is an event, carrying no id.
No `Returns` variant changes shape — `Returns::Records{fields:[event,data]}`
still describes the id-less events that follow, which is the only thing a
generated CLI ever has to print one-record-per-line; the snapshot is the
one-time reply to the request itself, printed once, before that loop starts
(TASK-1.50's own problem to solve, not this task's).

## 3. Subscribing, narrowing, ending (AC #1, #3)

`event.watch` *is* the subscribe operation (doc-2 §9: "the only stream in M1");
there is no separate subscribe/unsubscribe method. `--for` (already
`ValueKind::Choice` of the fourteen event names) and `--all`/`target` are read
once, at the moment `event.watch` is dispatched, and kept as this connection's
own filter for every event delivered afterward. **"A subscription... can be
ended" (AC #3)**: ending is disconnecting — closing the connection. Nothing in
doc-2's grammar or this task's own criteria describes narrowing or cancelling a
subscription without a new connection, so none is built; stated plainly rather
than silently assumed, since AC #3 uses the word "ended" and I want the reading
on record.

## 4. Atomicity: nothing lost or doubled between the snapshot and the events (AC #1)

Registering the subscription and reading the snapshot happen inside one lock
acquisition of `Runtime`'s existing `Mutex<State>` — the same lock every
mutating dispatch call already holds (`Runtime::handle`). `event.watch`'s own
dispatch arm: (1) registers this connection's `Outbound` handle and filter in
`State.subscribers` (§5), (2) builds the snapshot (§2), (3) the lock is released
when the function returns, (4) the snapshot becomes the `Response`. Because
every mutating call also takes this same lock, step (1) happening before step
(2) is what makes "included in the snapshot" and "delivered as an event after
it" mutually exclusive and jointly exhaustive: a change that reaches the lock
before this request does is serialised before it and lands in the snapshot; one
that reaches the lock after is excluded from the snapshot (already taken) and
is broadcast to the now-registered subscriber like any other. No second
ordering rule is needed (unlike decision-2 §3's "queue the result first,
register the queue second" for attach, which exists because that attach result
and that queue registration are two separate calls racing a second thread) —
here, both steps are statements inside one function, already serialised by the
lock, not two calls a writer thread could reorder.

## 5. Where each part-A event is produced (every site, read one by one)

Every row below runs with `Runtime`'s lock already held — the event is
*computed* there (the typed value, e.g. `Focused`, is already in hand from the
kernel call) and *enqueued* there (`Outbound::enqueue_control`, lock-free, no
I/O — `outbound.rs:164`), and only *written* later, off-lock, by each
connection's own writer thread. No polling, no periodic tick, anywhere in this
design: every broadcast call is driven by a real state change at the moment it
happens, never by a loop that wakes up to check.

| Call site | Change | Event(s) |
|---|---|---|
| `dispatch/session.rs: new` (via `Runtime::start_and_dispatch`) | mints session+tab+pane | `session.created` |
| `dispatch/session.rs: close` (via `Runtime::close_and_dispatch`) | closes session, cascading tabs/panes in one kernel call | `session.closed`, naming the session only (§5a) |
| `dispatch/tab.rs: new` (via `start_and_dispatch`) | mints tab+pane | `tab.created` |
| `dispatch/tab.rs: close` (via `close_and_dispatch`) | closes tab, cascading panes | `tab.closed` |
| `dispatch/tab.rs: focus` | sets `current_tab` | `tab.focused`, whole path |
| `dispatch/pane.rs: split` (via `start_and_dispatch`) | mints pane | `pane.created` |
| `dispatch/pane.rs: close` (via `close_and_dispatch`) | closes pane | `pane.closed` |
| `dispatch/pane.rs: focus` | sets `current_pane` (+`current_tab` if it moved, decision-5) | `pane.focused`, whole path, one event regardless of whether the session's own current tab also moved |
| `Runtime::start_and_dispatch` (`runtime.rs`, the real mint for `session.new`/`tab.new`/`pane.split`) | the actual program starts | emitted **after** `Response::Ok` is final — after the real spawn succeeds, never at the earlier, still-rollback-able `dispatch::dispatch` mint (`rollback_mint` undoes that one on a spawn failure a few lines later) |
| `apply_event_locked`, `PaneEvent::Exited` | `record_exit` → `ExitOutcome::Closed`/`Stayed` | `pane.exited` always; `Closed` additionally cascades `pane.closed`/`tab.closed`/`session.closed` as above — one real exit can produce up to four events (the shell-pane-is-its-tab's-only-pane-is-its-session's-only-tab case already proven in `runtime.rs`'s own existing test) |
| `apply_event_locked`, `CommandOutputStarted`/`CommandEnded` | `note_command_output_started`/`note_command_ended` → pane status | `pane.status_changed` — these two arms' own existing comments ("no client event stream exists yet") are exactly what this task closes |
| `dispatch/pane.rs`, `show`/`list` reading status directly (no mutation) | — | nothing — read-only, confirmed no event needed |

**§5a, `session.close`'s cascade is one event, not three per level**, because
`Tree::close_session` removes every tab and pane in one kernel call with no
observable intermediate state, and `event.watch`'s own target model has no way
to subscribe below the session level in the first place (`--for` filters by
event *name*, `target`/`--all` by *session* — doc-2 §8) — so no subscriber can
be watching a tab without also watching its session, and a synthetic
`tab.closed`/`pane.closed` manufactured after the fact would tell a subscriber
nothing `session.closed` doesn't already.

**What is explicitly not in this task**: layout changes (`split`/`move`/`swap`/
`set --zoomed`/`resize` on a pane; `pane.move`), session/tab label or position
changes (`set`/`unset`/`move`), and every client event — all TASK-1.109, all
needing `Event` records that don't exist yet (confirmed unchanged on
`origin/main`: `events()` in `description/methods.rs` is still exactly the
fourteen it was). `pane.cwd_changed`/`pane.title_changed` have no detection
mechanism anywhere in the server today and are TASK-1.109's own open question
(its comment #1 already names this, not re-litigated here).

## 6. Delivering an event to every subscribed connection (AC #1, #3)

**Structural gap, first step of the build, unchanged from the first draft and
still true on current `origin/main`:** `Runtime` has no way to reach any
connection's writer. `Outbound` is created inside `connection::handle`'s
`run_post_handshake` (`connection.rs:318`), never handed to `Runtime`.
`Runtime::connect_client` is called at `connection.rs:402`, **before**
`run_post_handshake` (and its `Outbound`) exists at all — confirmed still true
after the TASK-1.98/1.86 work in flight, since neither touches this ordering.
Fixed by:

- `run_post_handshake` calls a new `Runtime::register_subscriber(client_id,
  outbound: Arc<Outbound>)` right after `spawn_writer` returns, before entering
  the read loop, and `ConnectionCleanup::drop` (which already calls
  `outbound.close()`/`runtime.disconnect_client`) gains one more call,
  `runtime.deregister_subscriber(client_id)` — so a subscriber is registered
  for the whole of its usable life and removed on every exit path this guard
  already covers (ordinary close, a protocol violation, a panic unwinding past
  it), with no new exit path to reason about.
- Registering (fan-out eligibility) is separate from subscribing (`event.watch`,
  §3/§4): every connected client is registered the moment it is usable, so a
  client that never calls `event.watch` is registered but has no filter — it
  receives nothing, since `broadcast` (below) skips a subscriber with no
  declared filter, never delivers unfiltered.
- `State` gains `subscribers: HashMap<ClientId, Subscriber>`, where
  `Subscriber { outbound: Arc<Outbound>, filter: Option<Filter> }` —
  `filter: None` until `event.watch` runs, `Filter` holding the target
  session(s)/`--all`/`--for` set `event.watch` declared.
- One new function, `fn broadcast(state: &State, name: &str, body: Value, scope:
  SessionId)`, called from every row in §5's table, walks `subscribers`,
  applies each one's own `filter` against `name`/`scope`, and calls
  `enqueue_control` on every match — still under the same lock every §5 call
  site already holds (`enqueue_control` does no I/O, `outbound.rs:164`), so
  this adds no new lock anywhere.
- **Lock order, stated as an invariant, unchanged by this task**: `Runtime`'s
  state lock is always taken before `Outbound`'s own internal lock (inside
  `enqueue_control`, itself called from inside `broadcast`), never the
  reverse — a writer thread never touches `State` at all, so there is no
  second path to check.

## 7. Back-pressure for a slow subscriber (AC #3; decision-2 §6's principle)

decision-2 §6 is written for attachment (data-stream) back-pressure and does
not itself answer this. An event is a control-kind frame on stream 0
(`Connection::emit_event`/`Control::Event`, already built in the sdk, no change
needed), so it goes through `Outbound::enqueue_control`, capped at
`CONTROL_CAP` (256). **Decision, confirmed by the engineer 2026-11-16: the
connection is closed** — the same outcome TASK-1.45 §3 already chose for a
client that floods requests and never reads a reply, reusing the existing cap
and the existing closing behaviour rather than inventing a resync-style
recovery (there is no cheaper compacted replacement for a backlog of distinct
event names the way a fresh screen snapshot replaces a backlog of screen
deltas — decision-2 §6's own reasoning for *why* a resync is cheap there
doesn't carry over to a heterogeneous event queue). Stated beside TASK-1.47's
own, different policy for its *data* queue (§1 above) so neither plan
contradicts the other.

## 8. What's offered through the sdk (R101) vs. built in the server

No sdk wire-layer change: `Connection::emit_event`, `Incoming::Event`/
`UnknownEvent`, `Control::Event` already exist and are unchanged by this task.
One doc-only change to `event.watch`'s `details` (§2). Everything else —
`State.subscribers`, `broadcast`, the registration-timing fix — is server-side
scheduling over a socket it owns, the same boundary `outbound.rs`'s own module
doc already draws between protocol code (sdk) and I/O scheduling (server).
Checked against doc-16 R101/R102 and `.claude/rules/sdk.md` again on this
re-read: no I/O, no GPUI type, no runtime type crosses into `steg-sdk`.

## 9. Proof (every item below gets a test, named as such, or is named as dropped)

Built over `steg-sdk`'s own `Connection` driving a real `Runtime` behind a real
socket, no mocking crate, every wait bounded on a real event, per
`testing-rust.md`. Each test's own negative control is named below, to be
recorded against the test as it stands at the end of the build, per
`team:testing` — none of these have been run yet; this is still the plan.

- **Each part-A event, caused by a real change, observed by the test client**:
  one test per row in §5's table (`session.created`/`closed`,
  `tab.created`/`closed`/`focused`, `pane.created`/`closed`/`focused`/`exited`/
  `status_changed`) — drive the real request or real exit, assert the exact
  event name and body arrive on `Connection::poll()`. **Negative control**:
  comment out that row's own `broadcast` call — the event does not arrive
  within the bound.
- **The snapshot carries exactly the tree at the moment of subscribing, nothing
  missing, nothing duplicated across sessions/tabs/panes**: build a tree with
  two sessions, each with two tabs, each with two panes, in varied states
  (one closed pane, one with a status); subscribe; assert the snapshot's own
  nested shape matches a hand-built expected value exactly, field by field.
  **Negative control**: drop one level of nesting (e.g., return `panes: []`
  unconditionally) — the test fails on the missing panes, proving it actually
  checks the nesting, not just that *a* response arrived.
- **Snapshot-then-events, nothing lost or doubled, under a real race**:
  subscribe on one connection while, from a second thread synchronised with a
  `Barrier` (the same pattern `outbound.rs`'s own flood test already uses), a
  tree-mutating request races the subscribe call; assert the mutation appears
  **exactly once** — in the snapshot, or as exactly one event after it, never
  both, never neither. **Negative control**: swap §4's own ordering (read the
  snapshot, then register) — the test flakes, occasionally losing the race's
  own mutation entirely.
- **`event.watch`'s own `--for` filter**: subscribe with `--for
  session.created` only, cause a `tab.created` too; assert only the first
  arrives. **Negative control**: remove the filter check inside `broadcast` —
  the second event arrives too.
- **A slow subscriber is closed, not blocked on, not resynced**: a connection
  that never reads, fed real events from a real flood of real tree changes
  until `CONTROL_CAP` is reached; assert the connection closes (the next
  `poll`/the writer's own exit) and the thread producing the flood is never
  blocked waiting on it. **Negative control**: raise `CONTROL_CAP` to
  something enormous for this one test — it then fails to close within the
  bound, proving the test exercises the real cap, not an unrelated timeout.
- **Ending a subscription is disconnecting**: close a subscribed connection;
  assert no later `broadcast` call panics or blocks on it, and
  `State.subscribers` no longer holds its entry (a test-only accessor, matching
  this crate's existing style, e.g. `outbound.rs`'s own dead-handle tests).
- **A disconnect while events are queued**: queue several events on a
  connection mid-flood, then disconnect it before they are written; assert the
  writer thread ends and nothing panics (same shape as
  `a_broken_write_closes_the_queue_so_the_readers_next_enqueue_fails`,
  `outbound.rs`).
- **No thread or child left, on every path above**: checked via this crate's
  existing join-and-bound convention on every test above; no new process class
  is introduced here (events carry no pty).
- **No periodic wake-up**: proven negatively, not by inspection alone — a test
  that subscribes and then asserts nothing arrives for a bounded, generous
  wait with no tree change at all, then a change immediately produces exactly
  one event right after that wait ends. **Negative control**: this is itself
  the negative control for "is anything polling" — if a poll loop existed, an
  event (or at least a wake-visible side effect) would appear during the quiet
  window; dropped as its own separate test since the design has no poll loop
  to begin with (checked by reading: `broadcast`'s only callers are the §5
  table's own sites, nothing scheduled).

Every item the brief's proof list could ask for is above as a test, except:
narrowing an existing subscription after the fact (no method exists, §3 — named
as dropped, not built) and cwd/title events (TASK-1.109, §5's own "explicitly
not in this task" — named as dropped here too, for the same reason).

## Files that change

- `crates/server/src/dispatch/session.rs`, `tab.rs`, `pane.rs` — make
  `record_at`/`record_for` (session/tab/pane) `pub(super)`; no behaviour change
  to any of the three.
- `crates/server/src/dispatch/event.rs` (new) — `tree_snapshot`, the
  `event.watch` dispatch arm, `Filter`.
- `crates/server/src/dispatch/mod.rs` — `("event", _) => not_implemented_yet`
  replaced for `event.watch` only; wired into the existing per-noun dispatch
  shape.
- `crates/server/src/runtime.rs` — `State.subscribers`; `Runtime::
  register_subscriber`/`deregister_subscriber`; `broadcast`; one `broadcast`
  call added at each §5 site, still under the lock each site already holds.
- `crates/server/src/connection.rs` — `run_post_handshake` calls
  `register_subscriber`; `ConnectionCleanup::drop` calls
  `deregister_subscriber`.
- `crates/sdk/src/description/methods.rs` — one sentence added to
  `event.watch`'s `details` (§2); no new `Event`, no `Returns` change.
- Test-only: new integration tests under `crates/server/tests/`, per
  `testing-rust.md`'s own convention for anything driving a real `Runtime` and
  socket.

## Order of work

1. Merge `origin/main` (done for this plan's own re-read; re-check at build
   time for anything newer than today).
2. Make the three record builders `pub(super)`; no other change; `just check`
   clean.
3. `State.subscribers`, `register_subscriber`/`deregister_subscriber`, wired
   into `connection.rs` — proven with a test-only accessor only, no events yet.
4. `event.watch`'s dispatch arm and `tree_snapshot` — proven against the
   nested-shape test (§9) before anything broadcasts.
5. `broadcast`, and one call per §5 row, in the table's own order, each with
   its own test (§9) before the next row is added.
6. The race, filter, slow-subscriber, disconnect, and no-poll tests (§9), last,
   since they exercise every call site at once.
7. `just check` at each numbered step.

## Risks

- `dispatch/event.rs`'s own `tree_snapshot` walks every session, tab and pane
  on every `event.watch` call — fine for M1's scale (decision-2 §4.2 measured
  a *screen* snapshot at ~40ms for 10,000 scrollback rows; this snapshot
  carries none of that, only flat records), named so nobody is surprised it
  isn't paginated.
- `session.focus`'s own dispatch arm is confirmed, not just planned, to belong
  to TASK-1.109 — `dispatch/session.rs`'s own `not_implemented_yet` message
  already names that task.
- TASK-1.98/1.86 are both touching `connection.rs`/`listen.rs`/`ConnectionEvent`
  concurrently with this task's own edit to `connection.rs`
  (`run_post_handshake`/`ConnectionCleanup`) — different functions within the
  same file, no shared type, but a real textual merge is likely; named so it
  isn't mistaken for a design conflict when it happens.
<!-- SECTION:PLAN:END -->

## Comments

<!-- COMMENTS:BEGIN -->
created: 2026-11-16T04:32:00Z

---

Split on 2026-11-15 by @bruk.o under the engineer's standing decision to split by default: the worker's plan found two changes. This task keeps the tree, subscribing, delivery to every subscribed client, narrowing, and the events that already have a producer in the tree (created, closed, focused, exited, status changed). The events the API description does not have yet (a tab's layout changed, a session or tab changed, clients connecting, leaving and viewing, and the one event addressed to a single client for session focus) moved to a new draft, with session focus itself. Criteria reworded to match. Depends on TASK-1.89 for the tab record.

---

created: 2026-11-16T11:54:00Z

---

2026-11-16, @bruk.o: the engineer decided all four as recommended: the tree is the reply to event watch; the event names as proposed; a subscriber that does not read has its connection closed; status-change events are M1 (R52's label corrected in doc-11). The plan is being rewritten once for part A.

---
<!-- COMMENTS:END -->
