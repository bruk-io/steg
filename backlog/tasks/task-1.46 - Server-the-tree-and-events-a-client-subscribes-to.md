---
id: TASK-1.46
title: 'Server: the tree, and events a client subscribes to'
status: In Progress
assignee:
  - '@bruk.w2'
created_date: '2026-11-15T01:33:00Z'
updated_date: '2026-11-16T04:32:00Z'
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
## Split this into two reviewable changes — proposed at the top, not asked

This task's own AC #1-5 need: (A) a snapshot-then-events mechanism (atomic subscribe,
ordering, fan-out, back-pressure) and (B) several API-description additions this
mechanism has no producer for today (layout changes, client events, the one-client
-addressed event). Building B's event *kinds* needs A's delivery machinery to exist
first, but A can ship and be proven complete against the handful of events that
already have a clear kernel-level producer (created/closed/focused/exited/
status_changed). Recommended split, filed as two drafts once this plan is approved:

- **TASK-1.46 (this task, narrowed to AC #1, #2's first half, #5):** the tree
  snapshot, `event.watch` as subscribe, atomicity, ordering, fan-out to every
  subscribed connection's writer, back-pressure, `--for` filtering. Proven against
  session/tab/pane created, closed, focused, pane.exited and pane.status_changed —
  every event already in the description with a real producer in the tree today.
- **TASK-1.46b (new, depends on 1.46):** AC #2's "layout... whole" clause, AC #3
  (client connect/leave/view events), AC #4 (the one-client-addressed event,
  `session.focus`'s own dispatch arm). Adds the new `Event` records this needs
  (none exist today — see "Events the description is missing" below), and wires
  every dispatch call site that changes a tab's layout or a client's view set.

If the engineer would rather keep it as one task, say so and I build it as one;
everything below still applies, just not as two PRs.

## 1. The tree snapshot (AC #1)

**Open design question, the engineer's to decide — I have not worked around it.**
doc-14 says "the tree snapshot, returned by one request." Read literally (and it
has to be literal: the atomicity this brief asks for — "a snapshot then events with
nothing lost between" — cannot be built across three separate round trips to
`session.list`/`tab.list`/`pane.list`, because the tree can change between them and
there is no lock to hold across a client's own three requests), this is a method no
`Returns` variant and no entry in doc-2 section 8's 42-method table currently covers:
`methods.rs`'s own `the_method_set_is_exactly_doc_2_section_8s_42_pairs` test fails
the moment anything new is added. Three ways to give AC #1 its literal reading,
none of which I am choosing silently:

- **(a) `event.watch`'s own `Response` carries the snapshot.** No new method —
  `event.watch` already exists, already is "the only stream in M1" (doc-2 §9), and
  decision-2 §3's reply/event split ("events carry no id... a reply... travels
  staying matched to its own id") already gives a natural home for a one-time,
  id-matched answer before the id-less events start. Cost: `event.watch`'s
  described `returns: Records{fields:[event,data]}` currently describes the
  *stream*, not this one-time reply — the CLI's own `event watch` prints "one
  record per line" (doc-2 §8), and a nested session/tab/pane tree does not fit a
  tab-separated row. The description's shape would need a dedicated field for the
  one-time reply, separate from what streams after it, which today's `Returns`
  enum has no vocabulary for either.
- **(b) A new method, e.g. a `tree` noun with `tree.show`.** Breaks the 42-pair
  completeness test on purpose, needs the engineer to accept it as doc-2 §3's
  table growing by one noun, and needs a `Returns` case for a nested value (every
  existing `Record`/`Records` variant is flat fields, matching the CLI's own
  tab-separated row model — the existing `tab.layout`/`pane.status` fields are
  already `ValueKind::Text` placeholders for "a JSON shape I can't declare
  structurally yet," precedent for doing the same here, not a clean fit).
- **(c) Keep `session.list`/`tab.list`/`pane.list` as the only way to read the
  tree, and make AC #1's "one request" mean "one coherent read," not "one wire
  round trip"** — i.e. loosen the literal reading. This is the one option that
  needs no sdk change, but it directly conflicts with the atomicity this brief
  asks the plan to settle, so I am not picking it either.

**My recommendation, for the engineer to confirm or overrule:** (a). It adds no
method and reuses the one stream doc-2 already fixed; the `Returns` wording gap is
small (one sentence saying the request's own reply is the tree, distinct from what
follows) and is a description-only change, not a protocol change.

Snapshot shape, regardless of which option is chosen: nested — every session with
its tabs embedded, every tab with its panes embedded (doc-14's three-row sketch,
nested rather than three flat lists the client has to stitch back together, which
is the whole reason "nothing lost between" needs one read, not three). Field names
match `SessionRecord`/`TabRecord`/`PaneRecord` exactly, so nothing here invents a
second vocabulary for the same data — which is why this task cannot start building
the snapshot's tab-level fields until **TASK-1.89 merges** (it is building
`TabRecord` and the richer `Status` shape right now; today there is no tab record
type in the sdk at all, confirmed by reading `crates/sdk/src/record.rs`). Flagged
as a hard dependency, not a nice-to-have: this plan does not duplicate `TabRecord`.

## 2. Events the description is missing (AC #2, #3, #4 — for the engineer)

`crates/sdk/src/description/methods.rs`'s `events()` has fourteen records, each
with its own doc comment: "doc-8 R38's M1 events... Fourteen records; TASK-1.46
fills in behaviour, not this list." That line is stale against this task's own
acceptance criteria and against doc-14, both written after it. Checked one by one
against every event this task's ACs and doc-14 actually need:

| What's needed | In the 14 today? | Producer in the tree today? |
|---|---|---|
| session/tab/pane created, closed | yes | yes (every `new_*`/`close_*` kernel call) |
| session/tab/pane focused (whole path) | yes | yes (`Focused { session, tab, pane }`, already carries all three — `tree.rs:37`) |
| pane cwd/title changed | yes | **no** — nothing in the server today notices a live cwd or title change to fire from; `PaneHandle::cwd()` is read on demand (`resolve_default_cwd`), never diffed against its last value. No AC of this task names cwd/title, so I am leaving this one unfired — flagged as a gap for whichever task adds per-tick terminal polling (TASK-1.47 is the natural owner: it already reads live terminal state every tick for the screen) rather than building a second polling loop here |
| pane exited | yes | yes (`ExitOutcome`/`record_exit`, in `apply_event_locked`) |
| pane status changed | yes | yes — `dispatch/pane.rs` already computes the two-word status per pane; this task adds the event, doesn't invent the fact |
| **a tab's layout changed, whole (AC #2)** | **no** | would be — `split_pane`/`close_pane`/`move_pane_to_tab`/`swap_panes`/`fill_pane`/`unfill_pane`/`resize_pane` all touch `Tab::layout()` |
| **a session or tab "changed" generally — label, position (AC #2's own word)** | **no** | would be — `session.set`/`unset`, `session.move`, `tab.set`/`unset`, `tab.move` |
| **client connected / left / started viewing / stopped viewing (AC #3)** | **no** | would be — `Runtime::connect_client`/`disconnect_client`, `Tree::view_pane`/`stop_viewing` (TASK-1.42's own, not yet reached by any dispatch method) |
| **one event addressed to a single client, for `session focus` (AC #4)** | **no, and no per-client delivery path exists at all** | would be — `session.focus`'s own dispatch arm, which is `not_implemented_yet` today (`dispatch/session.rs:39`) |

Every missing row is proposed above as **TASK-1.46b** (§0's split), not invented
silently. Names proposed for the engineer to confirm or rename, following the
existing `noun.past_participle` convention: `tab.layout_changed` (fields:
`tab`, `layout`); `session.changed`/`tab.changed` (fields: the full
`SessionRecord`/`TabRecord`, so a client never has to know which sub-field moved —
the same whole-record-not-a-diff reasoning decision-2 §3 already uses for
`tab.layout_changed`); `client.connected`/`client.left`/`client.started_viewing`/
`client.stopped_viewing` (fields per doc-14's "Clients" section: id, kind, label,
session); and for AC #4, **not an entry in `events` at all** — doc-14 says this one
"is an event on stream 0 delivered only to that client," i.e. it is never
subscribed to and never filtered by `--for`, so it does not belong in the same list
`event.watch`'s `--for` draws its closed set from (`event_watch_for_is_drawn_
from_the_event_names`, `methods.rs`). Proposed as its own thing: `Connection::
emit_event` already takes a bare name and body with no list membership check at
the `Connection` level (only `description.event(name)` classifies known/unknown for
*incoming* frames) — so the server can emit `session.focus_requested` (or
whatever name the engineer prefers) straight onto the one addressed connection's
queue with no description entry required, as long as a client reading it
unprompted doesn't choke (R38/decision-2's forward-compat promise: "an unknown
field in an otherwise valid hello is ignored" — the same tolerance doc-14 implies
for an unknown event name reaching `event.watch --for`-filtered clients that never
asked for it). Flagged for the engineer because it is the one event in this whole
set that is not really a subscription event at all, and §3/§4 below assume it is
not.

R52 (doc-11, "status changes are events" — labelled M2) versus R82/R83 (doc-11,
M1, running/finished/failed) and this task's own AC #2/#3 (which plainly want
`pane.status_changed` now): treated here as R52's M2 label being stale against the
task that was actually scoped after it — `pane.status_changed` ships in M1 as part
of this task, and I'm not blocking on it; said here so the conflict isn't silent.

## 3. Subscribing, narrowing, and what a client gets first (AC #1, #5)

No `event.subscribe`/`unsubscribe` method exists or is proposed: `event.watch`
*is* the subscribe operation, matching doc-2 §9's "`event watch` is the only
stream in M1." Unsubscribing is disconnecting — nothing in doc-2's grammar or this
task's ACs asks for narrowing an existing subscription without starting a new
connection, so none is built. `--for` (already described, `ValueKind::Choice` of
the event names) and `--all` (every session, not just the target's — `target` is
also already described) are read once, when the request arrives, and kept as the
connection's own filter for every event emitted afterward.

**Atomicity.** Registering the subscription and reading the snapshot happen in the
same critical section, under `Runtime`'s one `Mutex<State>` — the same lock every
mutation already takes (`runtime.rs`'s own `State`). Concretely: `event.watch`'s
dispatch arm takes the lock, (1) registers this connection's filter and `Outbound`
handle in a new table on `State` (§5), (2) reads the snapshot, (3) releases the
lock, (4) the snapshot becomes the `Response`. Step (1) before (2) is what decision
-2 §3's own ordering rule generalises to events: a change that lands on another
thread between (1) and (2) is *included* in the snapshot (the lock serialises it
before this thread's read), so there is no window where a change is neither in the
snapshot nor delivered as an event after it — every mutating dispatch call already
runs under the same lock (`Runtime::handle`), so "registered, then read" inside one
lock acquisition is sufficient; no second rule like decision-2's "queue the attach
result first, register the queue second" is needed here, because (unlike
`Outbound::open_stream`, which is a separate call the writer races against) steps
(1) and (2) are two statements inside one already-held lock, not two defeat
-able-by-reordering calls.

## 4. Where every event is produced (every tree-changing site, read one by one)

Everything below runs with `Runtime`'s lock already held — events are *computed*
there (cheap: the typed `Outcome`/kernel-return value is already in hand) and
*enqueued* there (`Outbound::enqueue_control`, which does no I/O — `outbound.rs:
164` — so this never blocks on a socket while holding the lock), and only
*written* later, off-lock, by each connection's own writer thread. This is what
makes "no runtime lock held while writing" true by construction, not by care.

| Call site | Tree change | Event(s) | Split |
|---|---|---|---|
| `dispatch/session.rs`: `new` | mints session+tab+pane | `session.created` | A (but see below: `session.new` is intercepted by `Runtime::start_and_dispatch`, not plain `dispatch`) |
| `dispatch/session.rs`: `close` | closes session, cascading tabs/panes | `session.closed` (one), **not** a `tab.closed`/`pane.closed` per child — see note below | A |
| `dispatch/session.rs`: `move`/`set`/`unset` | position/label | `session.changed` | B |
| `dispatch/session.rs`: `focus` (not yet built — also TASK-1.89's to resolve, see §6) | none to the tree; addresses one client | the one-client event | B |
| `dispatch/tab.rs`: `new` | mints tab+pane | `tab.created` | A (via `start_and_dispatch`) |
| `dispatch/tab.rs`: `close` | closes tab, cascading panes | `tab.closed` | A |
| `dispatch/tab.rs`: `move`/`set`/`unset` | position/label/session | `tab.changed` | B |
| `dispatch/tab.rs`: `focus` | sets `current_tab` | `tab.focused` | A |
| `dispatch/pane.rs`: `split` | mints pane, changes layout | `pane.created`, `tab.layout_changed` | A for created (via `start_and_dispatch`), B for layout |
| `dispatch/pane.rs`: `close` | closes pane, changes layout | `pane.closed`, `tab.layout_changed` | A for closed, B for layout |
| `dispatch/pane.rs`: `move`/`swap`/`set --zoomed`/`unset --zoomed` | layout | `tab.layout_changed` | B |
| `dispatch/pane.rs`: `set`/`unset` (label only) | — | nothing (no "pane.changed" exists either — not asked for by any AC; flagged, not built) | — |
| `dispatch/pane.rs`: `focus` | sets `current_pane`(+`current_tab`) | `pane.focused` (whole path, per decision-5 — same event whether or not the session's `current_tab` also moved) | A |
| `Runtime::start_and_dispatch` (`runtime.rs:332`) | the real mint for `session.new`/`tab.new`/`pane.split` | emitted **after** `Response::Ok` is final (after the real program starts, after the rollback-on-spawn-failure branch is past) — never at the pure-`dispatch` mint, which can still be rolled back (`rollback_mint`, `runtime.rs:556`) a few lines later | A |
| `apply_event_locked`, `PaneEvent::Exited` (`runtime.rs:607`) | `record_exit` → `ExitOutcome::Closed`/`Stayed` | `pane.exited` always; `pane.closed`/`tab.closed`/`session.closed` **additionally** when `Closed` cascades (one real exit can produce up to four events — the shell-pane-is-its-tab's-only-pane-is-its-session's-only-tab case, `runtime.rs:787`, closes pane+tab+session in one call) | A |
| `apply_event_locked`, `CommandOutputStarted`/`CommandEnded` (`runtime.rs:637`,`663`) | `note_command_output_started`/`note_command_ended` → pane status | `pane.status_changed` — these two arms' own comments ("no client event stream exists yet," "TASK-1.46's own gap") are exactly what this task closes | A |
| `Runtime::connect_client`/`disconnect_client` (`runtime.rs:254`,`271`) | client set | `client.connected`/`client.left` | B |
| `Tree::view_pane`/`stop_viewing` (no dispatch method reaches these yet — TASK-1.42's own gap, unchanged by this task) | viewing set | `client.started_viewing`/`client.stopped_viewing` — **no producer exists to call from today**; named so B doesn't silently skip it, but nothing to wire until a `client.*` dispatch method exists | B, blocked |

**`session.close`'s cascade emits one event, not three per level**, because
`Tree::close_session` already removes every tab and pane in one call with no
intermediate state a client could observe anyway — the choice is between "one
`session.closed` naming the session" (what's proposed) and "a `session.closed` plus
a synthetic `tab.closed`/`pane.closed` per child, manufactured after the fact." The
second gives a client that was *only* watching one of those child tabs (not
subscribed to the session) a way to notice without re-deriving it from
`session.closed` — but nothing in `event.watch`'s own target model subscribes
below the session level (`--for` filters by event *name*, `target`/`--all` by
*session*, doc-2 §8), so no subscriber can be watching a tab without watching its
session too. One event, named here so a reviewer doesn't wonder why the fan-out
code doesn't walk the closed subtree.

## 5. Delivering an event to every subscribed connection (AC #1, the brief's "how
events reach each client")

**Structural gap, first step of the build, before any event logic:** `Runtime` has
no way to reach any connection's writer today. `Outbound` is created inside
`connection::handle`'s `run_post_handshake` (`connection.rs:318`) and never handed
anywhere `Runtime` can see. `Runtime::connect_client` is called at `connection.rs:
402`, **before** `run_post_handshake` creates the `Outbound` at all — so client
registration (in the tree) and writer registration (for events) happen at two
different times, with a real window between them during which an event for this
newly-minted client would have nowhere to go. Fixed by:

- `run_post_handshake` (not `connection::handle`'s earlier hello-reply step) calls
  a new `Runtime::register_subscriber(client_id, outbound: Arc<Outbound>, filter)`
  **right after** `Arc::clone(&outbound)`/`spawn_writer`, before entering the
  read loop — and deregisters it in `ConnectionCleanup::drop` (the same guard that
  already calls `outbound.close()`/`runtime.disconnect_client`, extended with one
  more call), so a client is never fanned-out-to after its writer is gone and
  never fans out into a closed `Outbound` (whose own `enqueue_control` already
  refuses once closed, `outbound.rs:164` — belt and suspenders, not load-bearing on
  its own: deregistering is what stops the broadcast loop from doing pointless
  work, the existing `Closed` check is what stops it from being wrong if the
  timing is ever off).
- This is **not** the subscription itself (§3's `event.watch` request) — a
  connection is registered for fan-out the moment it is usable at all (every
  connected client can receive the one-client-addressed event or, later, be told
  about sessions it hasn't explicitly watched yet, matching decision-5's "a client
  can know about every session... and views zero or more of them"); the `--for`/
  target filter from `event.watch` narrows what actually reaches it, read at
  delivery time, not at registration time, so a client is never silently dropped
  from the registry — only ever filtered.
- `State` gains `subscribers: HashMap<ClientId, Subscriber>` where `Subscriber`
  holds the `Arc<Outbound>` and whatever `event.watch` last declared (target
  session(s)/`--for` set/`--all`) — `None` until `event.watch` is called at all,
  since a connection that has never called it is registered (for the one-client
  event) but filters out every subscribable event.
- A new `fn broadcast(state: &State, name: &str, body: Value, scope: Scope)` (one
  function, called from every site in §4's table) walks `subscribers`, applies
  each one's own filter against `scope` (which session(s) this event is "in," for
  the target/`--all` rule) and `name` (for `--for`), and calls `enqueue_control`
  on every match — still under the same lock every §4 call site already holds,
  since `enqueue_control` is lock-free I/O-free bookkeeping (§3's own argument,
  restated).
- **Lock order, stated as an invariant**: `Runtime`'s state lock is always taken
  before any `Outbound`'s own internal lock (`enqueue_control` takes `Outbound`'s
  lock *inside* `broadcast`, which already holds `State`'s) — never the reverse,
  and nothing in this plan ever needs the reverse, since a writer thread never
  touches `State` at all.

## 6. Overlaps, named explicitly

- **TASK-1.89** (in progress, `@bruk.w5`): owns `TabRecord`, the richer `Status`
  shape, and may build or re-point `session.focus`'s dispatch arm (its own AC #3
  lists "session focus" among the leftover `NotImplementedYet` methods it will
  "either build... or name with the task that owns them"). **Collision risk**:
  both tasks could try to implement `session.focus`. Flagging for the lead to
  settle before either build starts — my own reading is that TASK-1.46b should own
  it (the dispatch arm's only real behaviour *is* emitting the one-client event),
  and TASK-1.89 should name TASK-1.46b rather than building it. This task also
  cannot start building the snapshot's tab fields until TASK-1.89's `TabRecord`
  merges (§1).
- **TASK-1.78** (in review, `pull/47`): rewrites `Runtime::stop` and the close
  paths into `close_and_dispatch`, and its own plan (§6) already reasons through
  this exact task: "a future tree-change event (TASK-1.46) fire[s] before the
  program is dead... correctly, by construction" — confirms the close-event
  timing this plan uses (§4's table: emitted at the tree mutation, under the lock,
  before any forced-close signalling happens outside it) is exactly what TASK-1.78
  already assumes. No conflict; cited so the reviewer of this task doesn't have to
  re-derive it. One mechanical note: TASK-1.78 adds `Runtime::handle`'s own
  `"pane.close" | "tab.close" | "session.close"` intercepted branch
  (`close_and_dispatch`) — this task's close events fire *inside* that function
  now, not inside plain `dispatch::dispatch`'s callers, once 1.78 merges first (it
  is further along — in review — so this task should rebase onto it, not the
  reverse).
- **TASK-1.87** (in progress, `@bruk.w1`): changes `Runtime::new`'s own
  constructor (adds `base_env`) and touches `LoginShell`. No overlap in
  *behaviour*, but `runtime.rs` is being edited by three tasks at once
  (1.78's `force_grace`, 1.87's `base_env`, this task's `subscribers` field) —
  named so whichever merges last expects a real, substantive merge conflict in
  `State`'s own struct literal and `Runtime::new`'s signature, not a sign of a
  design problem.
- **TASK-1.47** (attach/screen streams, not started): the natural owner of
  `pane.cwd_changed`/`pane.title_changed` detection (§2) and of the per-tick
  cadence that would drive it; also the task whose own `Outbound::open_stream`/
  `enqueue_data` this task's `enqueue_control` sits beside — no shared state
  between them (data streams vs. the control stream), but worth the next worker
  reading §5 before building attach, since both tasks grow `Outbound`'s callers.
- **TASK-1.50/1.52**: CLI `event watch` and the app's own tree-holding are
  explicitly left to them — this task proves the mechanism with the sdk's own
  test client (per the brief), not a CLI or GPUI client.

## 7. Back-pressure for a slow subscriber (AC per the brief; decision-2 §6's
*principle*, not its mechanism)

decision-2 §6 is about attachment (data-stream) back-pressure and does not answer
this directly — stated plainly rather than claimed otherwise. An event is a
control-kind frame on stream 0 (`Connection::emit_event`/`Control::Event`,
already built in the sdk), so it goes through `Outbound::enqueue_control`, capped
at `CONTROL_CAP` (256, `outbound.rs:32`) — the same queue a reply shares. A
subscriber that falls behind fills that queue; the next `enqueue_control` (for an
event, or for that connection's own next request's reply) returns
`EnqueueControlError::Full`. **Decision: the connection is closed**, the same
outcome TASK-1.45 §3 already chose for "a client that keeps sending requests while
never reading any reply" — decision-2 §6's underlying *principle* (never grow a
queue without bound, never block the producer) is honoured by reusing the existing
cap and the existing closing behaviour, rather than inventing a resync-style
recovery for events the way §6's own mechanism does for a data stream (there is no
"fresh snapshot" to resend here that would be cheaper than what's already queued —
unlike a screen, an event queue's backlog has no single compacted replacement).
Proven, not just asserted (§9).

## 8. What is offered through the sdk (R101) vs. built in the server

Everything the brief lists already exists in `steg-sdk`'s `wire` module with no
change needed: `Connection::emit_event`, `Incoming::Event`/`UnknownEvent`,
`Control::Event`. This task adds **no new sdk wire-layer code** — only new `Event`
records in `description/methods.rs` (data, not logic; TASK-1.46b, §2) and, if
option (a) in §1 is chosen, a one-sentence doc change to `event.watch`'s own
`details`. Everything else (the subscriber table, filtering, broadcast, the
registration-timing fix) is server-side scheduling over a socket it owns, the same
boundary `outbound.rs`'s own module doc already draws for the reply/data queues —
confirmed against `doc-16` R101/R102 and `.claude/rules/sdk.md` before writing this
(no I/O, no GPUI type, no runtime type crosses into `steg-sdk`).

## 9. Proof (TASK-1.46, part A; TASK-1.46b gets its own proof once split off)

Built over `steg-sdk`'s own client machinery (`Connection`) driving a real
`Runtime` behind a real `UnixStream::pair()` or a real socket — no mocking crate,
real processes where a pane needs one, every wait bounded on a real event per
`testing-rust.md`. Each test below gets a recorded failing run (its own mechanism
disabled or reverted) before being offered as proof, per the team's testing rule;
listed here as what each negative control is, not yet run.

- **Each described event caused by a real change, observed by the test client**:
  one test per row in §4's table that is in split A (`session.created/closed`,
  `tab.created/closed/focused`, `pane.created/closed/focused/exited/
  status_changed`) — drive the real request or real exit, assert the exact event
  name and body arrive on the subscriber's `Connection::poll()`. Negative control:
  comment out the one `broadcast` call for that site — the test fails (no event
  arrives within the bound).
- **Snapshot-then-events, nothing lost or doubled, under a real race**: subscribe
  on one connection; from a second thread, race a tree-mutating request against
  the subscribe call (a `Barrier`, the same pattern `outbound.rs`'s own flood test
  already uses) so the mutation genuinely can land before, during or after
  registration; assert the mutation appears **exactly once** — either inside the
  snapshot (if the lock serialised it first) or as exactly one event after it
  (never both, never neither). Negative control: swap the registration/snapshot
  order in §3 (read snapshot, then register) — the test flakes, occasionally
  losing the race's own mutation entirely.
- **A slow subscriber**: a subscriber that never reads, fed events until
  `CONTROL_CAP` is reached by a real flood of real tree changes (not a synthetic
  queue fill — this has to go through real `broadcast` calls); assert the
  connection closes (reader's next `poll`/the writer's own exit) rather than
  blocking the thread that produced the flood. Negative control: raise
  `CONTROL_CAP` to something enormous for this one test — it then fails to close
  within the bound, proving the test is actually exercising the cap, not an
  unrelated timeout.
- **Unsubscribe == disconnect**: close a subscribed connection; assert no further
  `broadcast` call panics or blocks on it, and `subscribers` no longer holds it
  (a test-only accessor, or the absence of any observable effect — matching this
  crate's existing style, e.g. `outbound.rs`'s own dead-handle tests).
- **A disconnect while events are queued**: queue several events on a connection
  mid-flood, then disconnect it (drop the stream) before they are written; assert
  the writer thread ends and nothing panics (same shape as
  `a_broken_write_closes_the_queue_so_the_readers_next_enqueue_fails`,
  `outbound.rs`).
- **No thread or child left**: every test above checked for a leaked thread via
  this crate's existing join-and-bound convention; no new process class is
  introduced by this part of the task (events carry no pty).
- **A negative control for the filter (`--for`)**: subscribe with `--for
  session.created` only; cause a `tab.created` too; assert only the first
  arrives. Negative control: remove the filter check in `broadcast` — the second
  event arrives too, proving the test catches an unfiltered broadcast.

Each test's own recorded failing run (with its stated mechanism removed) is
captured on the task before this is offered as done, per `team:testing`.

## Files that change (part A)

- `crates/sdk/src/description/methods.rs` — `event.watch`'s `details` (if §1(a)),
  no new events in this part.
- `crates/server/src/runtime.rs` — `State.subscribers`; `Runtime::
  register_subscriber`/`deregister_subscriber`; `broadcast`; the `event.watch`
  dispatch arm itself (moved from `("event", _) => not_implemented_yet` into a
  real handler in `dispatch/mod.rs` or a new `dispatch/event.rs`, following the
  existing per-noun module shape); every §4 call site gains one `broadcast` call
  after its own mutation, still under the lock.
- `crates/server/src/connection.rs` — `run_post_handshake` calls
  `register_subscriber`; `ConnectionCleanup::drop` calls `deregister_subscriber`.
- `crates/server/src/dispatch/mod.rs` — the `("event", _)` arm's `NotImplemented
  Yet` citation removed for `event.watch` only (`client.*` stays pointed at
  TASK-1.42/TASK-1.46b as appropriate).
- Possibly `crates/server/src/outbound.rs` — no change expected (`enqueue_control`
  already does everything needed); named in case `broadcast`'s own filtering
  surfaces a need `pop_next`'s existing control-then-round-robin order doesn't
  already satisfy (it does, on current reading).
- Test-only: new integration tests under `crates/server/tests/` (this crate's own
  convention for anything driving a real `Runtime`+socket, per
  `testing-rust.md`).

## Order of work

1. Confirm §1's design choice and §2's split with the engineer before writing
   code (both change the shape of everything after them).
2. Rebase onto TASK-1.78 once it merges (close-path call sites move).
3. `State.subscribers`, `register_subscriber`/`deregister_subscriber`, wired into
   `connection.rs` — proven with no events yet (just that registration/
   deregistration happens at the right times, via a test-only accessor).
4. `broadcast`, and the `event.watch` dispatch arm (snapshot shape depends on
   TASK-1.89's `TabRecord` — block here if it hasn't merged, per §1).
5. One `broadcast` call per §4 row, in the order the table lists them, each with
   its own test before the next is added.
6. The slow-subscriber and race tests (§9), last, since they exercise every call
   site at once.
7. `just check` at each numbered step.

## Risks / for the engineer

- §1's design choice (how the snapshot travels) is the one decision that has to
  be made before any code is written — everything else in this plan is written
  to be correct either way, but the snapshot's own shape is not.
- The split (§0) is proposed, not assumed; if rejected, TASK-1.46b's scope above
  becomes this task's own §10-and-beyond instead, same content.
- `session.focus`'s dispatch arm is claimed for TASK-1.46b but TASK-1.89 is
  already in flight and may reach it first — needs the lead's call before either
  worker builds it, not discovered by a merge conflict.
- `pane.cwd_changed`/`pane.title_changed` are named as a real gap with no owner
  today (§2) — not blocking this task, but should not be forgotten once TASK-1.47
  starts.
<!-- SECTION:PLAN:END -->

## Comments

<!-- COMMENTS:BEGIN -->
created: 2026-11-16T04:32:00Z

---

Split on 2026-11-15 by @bruk.o under the engineer's standing decision to split by default: the worker's plan found two changes. This task keeps the tree, subscribing, delivery to every subscribed client, narrowing, and the events that already have a producer in the tree (created, closed, focused, exited, status changed). The events the API description does not have yet (a tab's layout changed, a session or tab changed, clients connecting, leaving and viewing, and the one event addressed to a single client for session focus) moved to a new draft, with session focus itself. Criteria reworded to match. Depends on TASK-1.89 for the tab record.

---
<!-- COMMENTS:END -->
