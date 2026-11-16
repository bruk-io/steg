---
id: TASK-1.46
title: 'Server: the tree, and events a client subscribes to'
status: In Progress
assignee:
  - '@bruk.w2'
created_date: '2026-11-15T01:33:00Z'
updated_date: '2026-11-16T13:01:00Z'
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
asdlc_plan_approved_by: bruk
asdlc_plan_approved_hash: '4ee3881365b0'
asdlc_plan_approved_size: size-3
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

Decisions made 2026-11-16, recorded as settled (unchanged from the previous
revision): the tree travels as `event.watch`'s own reply; the confirmed event
names (all TASK-1.109's, not this task's — see §5); a slow subscriber's
connection is closed; R52 corrected (M1 for status events). The pane-identity
question is answered in §2, not reopened.

Two more things, found while fixing the five points raised, need your
confirmation before I build against them (a third, on the same point, is
recorded below as a finding rather than a question — it was split off into
its own task):

1. **The description's vocabulary, named precisely for TASK-1.47 to cite (§2).**
   Two separate things, not one, because a method's own reply and what
   follows it are two different questions:
   - **A nested reply.** `Returns` gains one variant, naming the reply's own
     sdk type rather than describing nothing: `Nested { of: String, summary:
     String }` — `of` is the type's name (`"TreeSnapshot"`), the same role
     `Identifier{of}` already plays for naming what kind of id a reply is,
     applied here to naming what kind of value it is instead of a fields
     table (the same honesty `ValueKind::Text` already has for `layout`/
     `status`, one level up, but naming the real type instead of falling
     back to prose alone). `api show` prints "a nested value of type `<of>`
     — `<summary>`" in place of a fields table. A generated CLI's `--json`
     prints the server's own JSON unchanged — already well-formed, needing
     no reshaping, and its field names are the named sdk type's own serde
     names, the single real definition (decision-1), not re-declared here a
     second time. Plain-text rendering has no flat default for this case
     (the same shape of exception doc-2 §6 already allows for `Text`
     returns, e.g. `pane.capture`) — left to TASK-1.50 to decide, not
     settled here. `event.watch`'s own `returns` becomes `Nested{of:
     "TreeSnapshot", summary: "every session, with its tabs, each with its
     panes, as of the moment this subscription began"}`.
   - **A reply followed by a stream — a bare `opens_stream: bool` describes
     too little: it cannot say whether what follows is id-less events on
     this same control stream (decision-2 §3: "an event carries no id") or
     a brand-new stream whose id a generated client has to read out of the
     reply first (decision-2 §3: "every other stream id belongs to one
     attachment, and the server hands it out in the attach result") — and a
     generated client behaves differently in the two cases.** `Method`
     gains one field, `follows: Follows`, a closed enum:
     ```rust
     pub enum Follows {
         /// This reply is the end of it — every method but the two below.
         Nothing,
         /// Further items with no id, on the control stream this reply
         /// itself travelled on. `event.watch`'s own case.
         Events(Returns),
         /// Further items with no id, on a new stream, whose id is named
         /// by `stream_id_field` — one field of *this* reply. TASK-1.47's
         /// attach.
         Attachment { item: Returns, stream_id_field: String },
     }
     ```
     `event.watch`: `follows: Follows::Events(Returns::Records{fields:
     [event, data]})` — unchanged from what `returns` used to carry, moved
     to where it actually belongs, since it described the stream, not the
     reply. `Method` has no `Default`, so every one of the 41 other methods
     gains one mechanical line, `follows: Follows::Nothing,` — the same
     size of change TASK-1.47's own `opens_stream: bool` already costs,
     for a type that says enough to act on. Both additions break the
     existing exhaustive `match` over `Returns` in `record.rs`'s own tests
     (by design — `rust.md`'s own reason for refusing a `_` arm) — named in
     Files below, not hidden. TASK-1.47's own `pane.attach` is expected to
     use `Nested{of: "PaneScreen", ...}` and `Follows::Attachment{item:
     Nested{of: "ScreenUpdate", ...}, stream_id_field: "stream"}` (naming
     its own reply's field that carries the new stream's id) — its plan to
     confirm exactly, not this one's to invent for it.
2. **"A subscription... can be ended" (AC #3) reads as "ended by disconnecting,"
   with no separate end-subscription method.** The alternative: an explicit
   `event.unwatch`-shaped method that ends a subscription on a connection that
   stays open — which the app's own single, long-lived connection (TASK-1.52)
   might eventually want, if it ever needs to stop watching without dropping
   its only channel to the server. Nothing in doc-2's grammar or this task's
   own criteria asks for it, and the app's own plan (TASK-1.52) reads as
   subscribing once for the life of the connection, never pausing. My
   recommendation: disconnect-ends-it for M1, revisit if TASK-1.52 finds it
   actually needs to pause without reconnecting.
**Recorded finding, not a question — checked by the engineer directly and
settled, then split off:** the tension §6 found between your preferred fix
(carry a moment so a client derives expiry itself, no timer in the server)
and `crates/sdk/src/status.rs`'s own already-merged, deliberate decision
("nothing timing-shaped can ever appear here... never on the wire") is
resolved in favour of keeping that decision: an event fired by one armed
deadline, built from the kernel's own `Tree::next_status_deadline`, never a
poll. Building the deadline thread itself is **its own task** (DRAFT-1,
"Server: a status that expires is reported to subscribers at its deadline,"
depending on this one), because this plan had already grown past this task's
own size before this piece — the subscription and its atomic reply, typed sdk
shapes, new description vocabulary, cascades on every path, and a background
thread with its own lifecycle are five separable things, and the engineer's
own standing rule is to split rather than carry them as one. §6 below keeps
its full analysis, unbuilt here — DRAFT-1 is pointed at it directly rather
than repeating it.

## 1. What's merged since the first draft (unchanged from the previous revision)

TASK-1.45/1.78/1.87/1.89 merged; TASK-1.98/1.86 in progress, named by overlap in
§7; TASK-1.47 being planned, coordinating on §1 above and §8's back-pressure
wording. `dispatch/session.rs`'s own `"focus"` arm already cites TASK-1.109 by
name — the split held.

## 2. The tree snapshot: typed in the sdk, not a server-built `Value` (AC #4)

**Criterion 4 says the tree is offered through the sdk; a `serde_json::Value`
assembled in `dispatch` is neither defined once (R101) nor typed for a caller
outside this crate.** New sdk types, beside the existing records
(`crates/sdk/src/record.rs`, or a new `tree.rs` re-exported alongside them —
build-time choice, not a design one):

```rust
/// One session with its tabs embedded — the shape `event.watch`'s own
/// reply carries (AC #1), and the sdk's one definition of it (R101).
pub struct SessionTree {
    #[serde(flatten)]
    pub session: SessionRecord,
    pub tabs: Vec<TabTree>,
}

pub struct TabTree {
    #[serde(flatten)]
    pub tab: TabRecord,
    pub panes: Vec<PaneRecord>,
}

/// The whole tree, as `event.watch`'s own reply.
pub struct TreeSnapshot {
    pub sessions: Vec<SessionTree>,
}
```

Named `TreeSnapshot`, not `Tree` — `steg_kernel::Tree` already owns that name
for the live, mutable structure; this is its wire photograph, a different
thing, in a different crate, and a reviewer should never have to ask "which
Tree." Each gets the round-trip test the other records have; the
fields-versus-description cross-check the others have does **not** apply here
(§1's "For the engineer": a `Nested`-returning method names its own type by
`of`, the same exemption `api.show`'s own `Method`/`Description` values
already get, for the same reason — there is no flat field list to check
against; a completeness test instead confirms `of` names a real sdk type by
checking it against a small, explicit allow-list this task adds to, so a typo
in `of` fails the build rather than silently describing nothing).

**The pane-identity question, answered, not left open:** `TabRecord.layout`
(`crates/sdk/src/layout.rs`) is a tree of splits whose leaves are `PaneId` — it
says *where* each pane sits, a structural reference, the same role
`SessionRecord.current_tab`/`TabRecord.current_pane` already play: naming a
sibling by id, not embedding its data. `TabTree.panes` is the one place that
pane's actual data lives. A client resolves a `layout` leaf's id against that
same tab's own `panes`, exactly as it already resolves `current_pane`/
`current_tab` against sibling arrays today — one existing convention, one
level deeper, nothing stated twice.

**Who builds it:** `crates/server/src/dispatch/event.rs` (new), reusing the
record builders TASK-1.89 already wrote — `session::record_at`,
`tab::record_for`, `pane::record_for` — made `pub(super)` (no behaviour
change), never redefining any of the three records a second time.

**The sdk's own client side (R101, criterion 4: the CLI and the app read the
tree and its events through the sdk, not by hand):**

```rust
impl Connection {
    /// Subscribes (`event.watch`): the id to match against the reply, and
    /// the bytes to write — the same shape `call` already has. The caller
    /// reads the matching `Incoming::Reply`'s `Response::Ok` as a
    /// `TreeSnapshot` (`serde_json::from_value`), then every
    /// `Incoming::Event` afterward as a `TreeEvent` (below) — no further
    /// request needed to keep them coming.
    pub fn watch(&mut self, target: Option<&str>, for_: &[&str], all: bool)
        -> Result<(u64, Vec<u8>), ApiError> { /* = self.call("event.watch", ...) */ }
}

/// One of this task's own events, typed by name (TASK-1.109 adds its own
/// variants to this same enum, not a second one).
pub enum TreeEvent {
    SessionCreated { id: SessionId, label: Option<String> },
    SessionClosed { id: SessionId },
    SessionFocused { session: SessionId, tab: TabId, pane: PaneId },
    TabCreated { id: TabId, session: SessionId, label: Option<String> },
    TabClosed { id: TabId },
    TabFocused { session: SessionId, tab: TabId, pane: PaneId },
    PaneCreated { id: PaneId, tab: TabId, label: Option<String> },
    PaneClosed { id: PaneId },
    PaneFocused { session: SessionId, tab: TabId, pane: PaneId },
    PaneExited { pane: PaneId, exit_code: i32 },
    PaneStatusChanged { pane: PaneId, status: Option<Status> },
}
impl TreeEvent {
    pub fn parse(name: &str, body: Value) -> Result<Self, serde_json::Error> { /* match on name */ }
}
```

Fields exactly as `description/methods.rs`'s own `events()` already declares
them — mechanical, not new design; a completeness test checks every variant's
fields against its own `Event` record, the same cross-check the other records
get (unlike `TreeSnapshot`, event bodies stay flat, so this one *is*
checkable).

## 3. Subscribing, narrowing, ending (AC #1, #3)

`event.watch` is the subscribe operation; `--for`/`target`/`--all` are read
once, at subscribe time, and kept as this connection's own filter. Ending a
subscription is disconnecting (§1's "For the engineer", item 2 — a reading,
not yet confirmed).

## 4. Atomicity, fixed by construction (AC #1)

**The race your review found, read back precisely:** registering the filter
and reading the snapshot happen under the lock; the lock is released; *only
then*, back in `connection.rs`'s own reader loop, does this connection's
thread build and enqueue the reply. Between release and that enqueue, another
thread's mutation can take the lock, broadcast, and enqueue an event onto this
same connection's queue first — the event arrives on the wire before the
reply it is supposed to follow.

**Fix: the reply is enqueued inside the same locked section that registers the
filter**, not after the lock is released, since `enqueue_control` is
lock-free and does no I/O (`outbound.rs:164` — already true today, used the
same way by every `broadcast` call in §5). Concretely, `event.watch` does not
go through the generic `Runtime::handle`/`Connection::reply`-afterward path at
all:

- `connection.rs`'s own reader loop, on `Incoming::Request{id, request}`,
  resolves `api.call` wrapping first if present (`dispatch::resolve_api_call`,
  already `pub(crate)`, called here the same way `Runtime::handle` already
  calls it internally — a second, cheap, I/O-free resolution, the same
  "harmless, cheap: it has to run here first too" reasoning `start_and_
  dispatch`'s own existing comment already gives for its own second
  `validate` call) to find the *resolved* method name, **without** otherwise
  changing how the non-`event.watch` path is handled (`runtime.handle` is
  still called with the *original*, unresolved `request`, exactly as today,
  and resolves `api.call` again itself — redundant resolution, not redundant
  behaviour).
- If the resolved method is `"event.watch"`: a new `Runtime::subscribe(&self,
  description, context, request /* resolved */, client_id, request_id) ->
  Result<(), ApiError>`. Internally: lock, validate (on failure, return
  `Err` — nothing registered, nothing enqueued, the caller builds and
  enqueues the ordinary error reply, no race to guard since nothing changed);
  look up `state.subscribers[&client_id]` (present since `register_subscriber`
  already ran before any request could arrive, §7 — `.expect(...)`, not a
  possible `None`); set its filter from the request's params; build the
  `TreeSnapshot` (§2); encode `Connection::reply(request_id, Response::Ok(..))`;
  call `subscriber.outbound.enqueue_control(bytes)` directly, still under the
  lock (ignore its own `Err` — the connection is already tearing down
  concurrently if it fails, nothing more to do); return `Ok(())`.
- `connection.rs`'s caller: on `Ok(())`, nothing further — the reply is
  already on the wire. On `Err(error)`, build and enqueue the error reply the
  ordinary way.

A client reaching `event.watch` *through* `api.call` is still routed here,
since the resolution runs before the method-name check — the one path this
fix does not special-case differently is `api.call` itself failing to resolve
(malformed `method` selector), which already has its own error path untouched.

**Proof, sharpened per your own wording:** many repetitions of the race in
§9's own test, asserting **wire order** (the reply carrying the snapshot is
the first frame this subscribing connection ever reads, before any event, in
and across many repetitions) *and* **exactly-once** (the race's own mutation
is in the snapshot or is exactly one event, never both, never neither).
Negative control: revert to the previous shape (register under the lock,
build and enqueue the reply after it is released, the design this section
replaces) — the race test then shows an event arriving before the reply, or
the mutation doubled or dropped, across repetitions.

## 5. Where every event fires: one rule for cascades (AC #2)

**One rule, not two, as asked: everything created or closed gets its own
event, on every path, creation outermost-first, closure innermost-first,
all delivered inside the one lock acquisition that performs the change — no
other thread's broadcast can land inside this cascade, since nothing else can
take the lock while this thread holds it.** Every id needed is already
available with **no kernel change**: `ClosedPane{pane, tab, session}` (from
`close_pane`/`record_exit`'s own `ExitOutcome::Closed`) already gives the
whole cascade for the exit-driven path; for the request-driven paths,
`close_and_dispatch` captures what it needs *before* calling `dispatch::
dispatch` (the same pattern it already uses for `panes_in_close_scope`), and
confirms what actually cascaded *after*, by checking what the tree no longer
holds — never by threading a new return shape through `dispatch::tab::close`/
`session::close`, which stay exactly as TASK-1.89 left them.

**Creation** (`start_and_dispatch`, after `Response::Ok` is final, same
placement as before — after the real spawn, never at the earlier, still
-rollback-able mint):

| Request | Events, in order |
|---|---|
| `session.new` | `session.created{id,label}`, `tab.created{id,session,label}`, `pane.created{id,tab,label}` |
| `tab.new` | `tab.created{...}`, `pane.created{...}` |
| `pane.split` | `pane.created{...}` |

Ids for the new tab/pane come from the existing `new_pane_from`/`tree.tab_of_
pane` the function already has in scope; nothing new to look up.

**Closure** (`close_and_dispatch`, inside the lock, after `dispatch::dispatch`
returns `Ok`, before `force_end` runs — unchanged placement, consistent with
TASK-1.78's own plan, which already assumed this timing):

| Request | Captured *before* dispatch | Events *after* `Ok`, in order |
|---|---|---|
| `pane.close` | the target pane id (already resolved) | `pane.closed{id}` |
| `tab.close` | the target tab id; its owning session id (`target_tab`, then the session that holds it — both already resolvable the same way `panes_in_close_scope` resolves the tab); its panes, in layout order (`panes_in_close_scope`, reused, unchanged) | one `pane.closed{id}` per pane, in order; then `tab.closed{id}`; then — only if `state.tree.session(session_id).is_none()` afterward, confirming R94's own cascade actually ran — `session.closed{id}` |
| `session.close` | the target session id; its tabs, in order, each with its own panes, in order (one read of `tree.session(id).tabs()`, before the close removes it) | for each tab in order: each of its panes' `pane.closed{id}`, then that tab's `tab.closed{id}`; after every tab, `session.closed{id}` |

**Exit-driven** (`apply_event_locked`, `PaneEvent::Exited`): `pane.exited
{pane, exit_code}` always, first (it is the causal fact; the closure, if
any, is its consequence); then, from `ClosedPane` directly, the same
innermost-first order: `pane.closed{id: pane}`; if `tab.is_some()`,
`tab.closed{id: tab}`; if `session.is_some()`, `session.closed{id:
session}`. `CommandOutputStarted`/`CommandEnded` (unchanged from the previous
revision): `pane.status_changed`.

**Focus** (unchanged): `tab.focus`/`pane.focus` each emit their own
`*.focused`, whole path, from the kernel's own `Focused`/`FocusMove` — no
cascade question here, nothing else is created or closed.

**What's still explicitly not in this task**: layout, session/tab label or
position "changed," and every client event — TASK-1.109, unchanged from the
previous revision; `events()` is still exactly its original fourteen on
current `origin/main`, confirmed again on this re-read.

## 6. Status that changes with no request and no tree mutation (AC #2)

**This section is analysis, not this task's own build.** The mechanism below
is built by DRAFT-1, "Server: a status that expires is reported to
subscribers at its deadline," split off and depending on this task (§1's
"For the engineer" has the recorded finding and why). It is kept here in
full because that task is pointed at this section directly rather than
repeating the design. **Until DRAFT-1 lands, a subscribed client sees a
`Finished` status stay as last reported — it does not expire on its own from
that client's point of view — until the next event for that pane** (a fresh
command starting or ending, a close, or any other `pane.status_changed` this
task's own §5 already fires), which carries the true, freshly-computed value
and corrects it. Stated plainly rather than left implicit: this is the one
named gap this task's own plan leaves in AC #2's "a pane's status changing
are events," for the span between this task landing and DRAFT-1's own build.

**Only `Finished` ever has a timer (doc-11 R83: "waiting and failed never on
a timer; finished after 10 minutes... Opening a tab clears nothing") — `Failed`
has none, and "a key press or click" clearing either has no dispatch path to
reach yet at all** (`pane.send`/an acknowledge method are both still
`not_implemented_yet`; `Tree::acknowledge_pane`/`note_input` exist in the
kernel with no caller — the same shape of gap as `pane.cwd_changed`/
`pane.title_changed`, named, not built, left to whichever task wires input).
So the one real case this task's own criteria reach is: a `Finished` status
expiring on its own, ten minutes after it was set.

**Recommended, and settled per §1's "For the engineer" recorded finding: one
armed deadline, never a poll**, using the kernel's own
`Tree::next_status_deadline(now)` — built, by its own doc, for exactly this
("sleep until this moment, or until something else happens first, instead of
polling every pane on an interval"). DRAFT-1's own worker builds a new
background thread, owned by `Runtime`, the same shape as the existing `drain`
thread:

```
loop:
  now = clock
  deadline = { lock; tree.next_status_deadline(now) }   // released immediately after
  match deadline:
    None => block on wake_rx.recv() (no timer armed at all; woken only by a
            mutation that might have set one)
    Some(d) => match wake_rx.recv_timeout(d - now):
      Ok(())        => loop (woken early — something changed; recompute)
      Timeout        => { lock; diff every pane's status(now) against a
                           per-pane cache the thread itself owns, broadcast
                           pane.status_changed for each difference, update
                           the cache } ; loop (next iteration re-arms)
      Disconnected   => return (Runtime is stopping)
```

A `wake_tx: Sender<()>` clone is sent one `()` from every §5 call site that
can set or clear a status (command start/end, program exit, close) — cheap,
unit payloads, no backpressure concern; the loop's own re-entry recomputes
the deadline from scratch each time, so a stale wake is harmless. This is
**not** a periodic wake-up: every wait is either unbounded (nothing timed) or
bounded to the one real deadline the kernel itself computed: confirmed by the
proof in §10.

**Tab shown-status changing (R52 as corrected: "a pane's and a tab's shown
status changing is reported to subscribers")**: `Tab::shown_status` is a pure
function of its panes' own statuses, computed server-side specifically so "the
app, the CLI and a plugin cannot disagree" (doc-14) — a client must **not**
re-derive it locally from `pane.status_changed`, which would contradict that.
But reporting it needs `tab.changed` (or whichever event TASK-1.109 settles
on for "a tab's own fields changed"), which does not exist until that task
lands. **Named as a gap, the same honest shape as cwd/title**: every site in
this task that can move a tab's shown status (every §5/§6 status change) is
where TASK-1.109's own producer call belongs once it exists; until then, a
tab's shown status changing is not reported to a subscriber at all. Quoted in
full so it is not missed: doc-11 R83's own text is above; R52 now reads "a
pane's and a tab's shown status changing is reported to subscribers" per the
engineer's correction.

## 7. Delivering an event to every subscribed connection (unchanged design,
re-confirmed against current `origin/main`)

`Runtime::register_subscriber`/`deregister_subscriber`, `State.subscribers`,
`broadcast` — as the previous revision, re-read against the merged
`connection.rs`/`runtime.rs` and still accurate: `Outbound` is still created
inside `run_post_handshake`, still after `connect_client` already ran: the
structural gap is unchanged, and the fix is unchanged. Lock order unchanged
(`Runtime`'s state lock always before `Outbound`'s own).

## 8. Back-pressure for a slow subscriber (AC #3) — unchanged, stated beside
TASK-1.47's own data-queue policy as before: a full control queue closes the
connection; a full data queue (TASK-1.47's own, different queue) means a
fresh snapshot instead.

## 9. What's offered through the sdk vs. built in the server — updated for §2

`TreeSnapshot`/`SessionTree`/`TabTree`/`TreeEvent` and `Connection::watch` are
now real sdk additions (§2), not a details sentence. Everything else this
task itself builds — `State.subscribers`, `broadcast`, the registration
-timing fix — stays server-side scheduling, no I/O or runtime type crossing
into `steg-sdk`, re-checked against `.claude/rules/sdk.md`. (The status-expiry
thread, §6, is DRAFT-1's own build, not this task's — the same boundary
applies there too, unchanged, when that task plans it.)

## 10. Proof — carried forward from the previous revision, plus:

- **Wire order, not just exactly-once** (§4): sharpened as described there,
  with its own negative control.
- **Filter by session, not just by name**: subscribe two connections, one
  `--for pane.created` with `target` naming session A, one with `--all`;
  cause a `pane.created` in session A and another in session B; assert the
  first connection sees only A's, the second sees both. Negative control:
  remove the scope check inside `broadcast` — the first connection then also
  sees B's.
- **The cascade order, both paths, each with its own negative control**:
  drive `session.new` and assert the three creation events arrive in the
  stated order; drive `session.close` over two tabs with two panes each and
  assert every `pane.closed` precedes its own tab's `tab.closed`, and every
  `tab.closed` precedes `session.closed`; drive the shell-pane-exit cascade
  and assert `pane.exited` precedes the same ordered closure. Negative
  control for each: remove one level's own broadcast call (e.g. the per-pane
  loop) — the test then sees a cascade missing an event it asserted on.
The status-expiry thread's own proof (it fires once, at the real deadline,
and does not poll) is DRAFT-1's own, not built or tested here — §6 names what
a subscriber sees in the meantime instead.

Every item the brief's own proof list could ask for is a test above, in the
previous revision, or named as dropped (subscription narrowing after the
fact — no method exists, §1 item 2; cwd/title events and a key press/click
clearing a status — no dispatch producer exists yet, §5/§6; a tab's shown
status changing — TASK-1.109's, §6; a finished status expiring on its own —
DRAFT-1's, §6).

## Files that change

- `crates/server/src/dispatch/session.rs`, `tab.rs`, `pane.rs` — record
  builders made `pub(super)` (unchanged from before); no change to
  `Outcome`/return shapes for any close/new arm (§5 reads tree state before
  and after instead, see there for why).
- `crates/server/src/dispatch/event.rs` (new) — `TreeSnapshot` assembly
  (calling into the sdk type, §2), `event.watch`'s validation/filter
  resolution (called by `Runtime::subscribe`, not through the generic
  `dispatch::dispatch` match — `event.watch` is removed from any per-noun
  `HANDLED` list and handled entirely through the new path).
- `crates/server/src/dispatch/mod.rs` — `("event", _)` unaffected (still
  routes to `not_implemented_yet` for anything that is *not* `event.watch`,
  since `event.watch` itself no longer reaches `dispatch::dispatch` at all).
- `crates/server/src/runtime.rs` — `State.subscribers`; `register_subscriber`/
  `deregister_subscriber`; `broadcast`; `subscribe` (§4); one `broadcast`
  call added at each §5 site, still under the lock each site already holds.
  (The status-expiry thread and its own wake channel, §6, are DRAFT-1's own
  addition to this same file, not this task's.)
- `crates/server/src/connection.rs` — `run_post_handshake` calls
  `register_subscriber`, resolves `api.call` to check for `event.watch` and
  routes to `Runtime::subscribe` when it matches (§4); `ConnectionCleanup::
  drop` calls `deregister_subscriber`.
- `crates/sdk/src/description/shapes.rs` — `Returns::Nested{of, summary}`;
  `Follows` (`Nothing`/`Events`/`Attachment`); `Method.follows: Follows` (§1
  item 1).
- `crates/sdk/src/description/methods.rs` — every one of the 41 other
  methods gains `follows: Follows::Nothing,`; `event.watch`'s own `returns`
  becomes `Nested{..}` and gains `follows: Follows::Events(..)` (what
  `returns` used to carry) (§2).
- `crates/sdk/src/record.rs` (or a new `crates/sdk/src/tree.rs`,
  re-exported) — `TreeSnapshot`, `SessionTree`, `TabTree`; `crates/sdk/src/
  wire/connection.rs` — `Connection::watch`; a new small module or an
  addition to an existing one for `TreeEvent` and its `parse`.
- `crates/sdk/src/record.rs`'s own test module — every exhaustive match over
  `Returns` gains a `Nested{..}` arm (compiler-enforced by `rust.md`'s own
  no-wildcard rule, named here so it is expected, not a surprise diff); one
  new completeness test, that every `Nested{of, ..}` in the description
  names a real sdk type, against a small explicit allow-list this task adds
  `"TreeSnapshot"` to (§2).
- Test-only: new integration tests under `crates/server/tests/`.

## Order of work

1. `Returns::Nested`/`Follows`/`Method.follows` in the sdk, with the
   exhaustive-match fallout fixed at each of its own existing call sites and
   the one mechanical line added to every other method; `event.watch`'s own
   description entry updated. `just check` clean before anything else.
2. `TreeSnapshot`/`SessionTree`/`TabTree` in the sdk, with their round-trip
   tests; record builders made `pub(super)`; `dispatch/event.rs`'s
   `tree_snapshot` function, proven against a hand-built nested tree (no
   subscription machinery yet).
3. `State.subscribers`, `register_subscriber`/`deregister_subscriber`, wired
   into `connection.rs` — proven with a test-only accessor only.
4. `Runtime::subscribe` (§4), routed from `connection.rs` — proven first for
   the ordinary (no-race) case: a subscribe gets exactly the snapshot, in
   reply, with nothing else on the wire yet.
5. `broadcast`, one call per §5's creation/closure tables, each proven with
   its own cascade-order test before the next is added.
6. The race (§4) and filter-by-session tests (§10), last.
7. `Connection::watch`/`TreeEvent` in the sdk, with their own tests, used by
   every test above instead of raw `Connection::call`/`poll` once they exist
   (an ordering choice, not a correctness one — earlier steps can use the raw
   calls and be ported once the typed wrapper lands, whichever reads easier
   when building).
8. `just check` at each numbered step. (The status-expiry thread, §6, is
   DRAFT-1's own order of work, built once this task has merged.)

## Risks

- The status-expiry thread (§6) is split off to DRAFT-1 rather than built
  here — it was the single largest net-new mechanism this plan had added,
  a background thread with its own wake channel, separable from everything
  else in this task. Until it lands, a subscriber sees a `Finished` status
  stay as last reported past its own ten-minute deadline (§6's own stated
  gap) — named so this task is not mistaken for closing AC #2 completely on
  its own.
- `dispatch/event.rs`'s `tree_snapshot` still walks every session/tab/pane on
  every `event.watch` call (unchanged risk note from before) — fine at M1
  scale, named so it isn't mistaken for an oversight.
- `session.focus`'s dispatch arm is TASK-1.109's, confirmed by the merged
  code's own `not_implemented_yet` message.
- TASK-1.98/1.86 both touch `connection.rs`/`listen.rs`/`ConnectionEvent`
  concurrently with this task's own edits to `connection.rs` — different
  functions, no shared type (§7), a real textual merge expected, not a design
  conflict.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Plan (4ee3881365b0) checked against the four acceptance criteria, decision-2, doc-11 (R52 as corrected, R82, R83), doc-16 R101 and the team's testing rules by @bruk.o, 2026-11-16: ok. Revised at the orchestrator's request: the reply carrying the tree is queued inside the locked step that registers the subscription, so no event can precede it on the wire (tested for order as well as exactly-once); the tree and the events are typed shapes in the sdk; the description gains Returns::Nested and Method.follows (Nothing, Events, Attachment), which TASK-1.47 uses; one rule for cascades on every path; a test for the filter by session. The event at a finished status's expiry is split into its own task (drafted): status.rs keeps timing off the wire by a merged decision, so the server reports the expiry from one armed deadline. For the engineer: the description's new vocabulary, and reading 'a subscription can be ended' as ended by disconnecting.

Check FAILED: `just check` exited 101 at 2026-11-16T12:51:06+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.46 on task-1.46-server-the-tree-and-events-a-client at 27c15dbcef41, last 40 of 76 lines:

```text
error[E0063]: missing field `follows` in initializer of `Method`
    --> crates/sdk/src/description/methods.rs:1214:9
     |
1214 |         Method {
     |         ^^^^^^ missing `follows`

error[E0063]: missing field `follows` in initializer of `Method`
    --> crates/sdk/src/description/methods.rs:1229:9
     |
1229 |         Method {
     |         ^^^^^^ missing `follows`

error[E0063]: missing field `follows` in initializer of `Method`
    --> crates/sdk/src/description/methods.rs:1245:9
     |
1245 |         Method {
     |         ^^^^^^ missing `follows`

error[E0063]: missing field `follows` in initializer of `Method`
    --> crates/sdk/src/description/methods.rs:1281:9
     |
1281 |         Method {
     |         ^^^^^^ missing `follows`

error[E0063]: missing field `follows` in initializer of `Method`
    --> crates/sdk/src/description/methods.rs:1292:9
     |
1292 |         Method {
     |         ^^^^^^ missing `follows`

error[E0063]: missing field `follows` in initializer of `Method`
    --> crates/sdk/src/description/methods.rs:1311:9
     |
1311 |         Method {
     |         ^^^^^^ missing `follows`

For more information about this error, try `rustc --explain E0063`.
error: could not compile `steg-sdk` (lib) due to 42 previous errors
full output: .quiet/test-steg-kernel.log
error: recipe `check-packages` failed with exit code 101
```

Check passed: `just check` exited 0 at 2026-11-16T13:01:03+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.46 on task-1.46-server-the-tree-and-events-a-client at 27c15dbcef41 with uncommitted changes:

```text
ok      format check (0.2s)
ok      lint (0.4s)
ok      boundaries (0.1s)
ok      foundations (0.1s)
ok      foundations public surface (0.0s)
ok      design pages (0.1s)
ok      licences (0.7s)
ok      tests (-p steg -p steg-app -p steg-kernel -p steg-sdk -p steg-server) (11.6s)
ok      build (steg-sdk) (0.1s)
ok      test (steg-sdk) (0.2s)
ok      test (steg-sdk, local-socket) (0.4s)
ok      build (steg-kernel) (0.1s)
ok      test (steg-kernel) (0.2s)
ok      build (steg-server) (0.1s)
ok      test (steg-server) (11.0s)
ok      test (steg-server, race-fixture) (11.2s)
ok      build (steg) (0.1s)
ok      test (steg) (0.1s)
ok      test (steg, fixtures) (3.0s)
ok      build (steg-ui) (0.3s)
ok      test (steg-ui) (0.3s)
ok      build (steg-app) (0.3s)
ok      test (steg-app) (0.2s)
ok      tool tests (11.8s)
ok      architecture (0.7s)
ok      recipe names (0.0s)
ok      ui levels (0.0s)
ok      ui colours (0.1s)
```
<!-- SECTION:NOTES:END -->

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

author: @bruk
created: 2026-11-16T12:47:00Z

---

Plan approved by @bruk (4ee3881365b0, size-3): its worker may build it.

---

created: 2026-11-16T12:50:00Z

---

2026-11-16, @bruk.o: the task the plan calls DRAFT-1 (a status that expires is reported to subscribers at its deadline) was accepted as TASK-1.116. The plan's text is left as approved; cite TASK-1.116 in code comments and notes.

---

created: 2026-11-16T13:01:00Z

---

The first recorded check on this task (just check exited 101) raced my own in-progress edit to shapes.rs/methods.rs mid-run, not a real failure of the pre-existing codebase: the background process picked up a half-finished struct field addition while I was still editing. The next recorded check, after finishing that edit, is clean and is the true state at this step.

---
<!-- COMMENTS:END -->
