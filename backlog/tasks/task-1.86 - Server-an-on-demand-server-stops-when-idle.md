---
id: TASK-1.86
title: 'Server: an on-demand server stops when idle'
status: In Progress
assignee:
  - '@bruk.w3'
created_date: '2026-11-15T23:40:00Z'
updated_date: '2026-11-16T11:48:00Z'
labels:
  - size-3
milestone: m-3
dependencies:
  - TASK-1.77
  - TASK-1.45
parent_task_id: TASK-1
project: bruk-io-steg
asdlc_accepted_from: DRAFT-1
asdlc_branch: task-1.86-server-an-on-demand-server-stops-when
asdlc_plan_approved_by: bruk
asdlc_plan_approved_hash: '7161104a6d7b'
asdlc_plan_approved_size: size-3
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Second piece of the split of TASK-1.77 (2026-11-15). doc-8 R99. The worker's plan on TASK-1.77, section 3, is the starting design: a pure predicate, a checker thread fed by ConnectionEvent, the arbiter-probe caveat carried from TASK-1.44's review. Depends on TASK-1.45 because requests over the socket are what create sessions on a real server; if the plan finds the idle rule can be built and proven without it, say so and the dependency can be dropped.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A server started on demand stops by itself after 30 seconds with no sessions and no clients; one with a session, or one started explicitly, never does (R99). The rule is a pure function of counts and a time passed in as data, with the boundary tested at 29, 30 and 31 seconds with literal values
- [ ] #2 A connection counts as a client from the moment it is accepted, not from a completed handshake, so a client mid-handshake cannot have the server stopped under it; the arbiter's liveness probe, which appears as a client arriving and leaving, does not by itself keep an idle server alive, and what a client that reconnects every 29 seconds does is stated and tested
- [ ] #3 The session count comes from the server's Runtime, and stopping uses the listener's one stop function and then stops the Runtime, ending its programs; shown end to end with a real server process, a short injected grace period and bounded waits
- [ ] #4 A client that connects in the instant the server is stopping sees what it would see with no server there and starts a fresh one; shown with real processes
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
Read against doc-8 R99, doc-2 section 6/8, doc-15 (R94, R97), doc-16 (R101, R110), the three pull requests not yet on main (47/3e4594e, 56/040c5f0, 57/93973fd - this plan is written against their heads, not `origin/main`), and TASK-1.77's own carried design (its Implementation Notes, "For TASK-1.86"). No file is edited in this phase.

**Revision note (second round):** the engineer asked for one simplification - one thread and one channel instead of two, since the relay thread existed only to convert between two message types so `connection.rs` stayed untouched, and the deadlock-avoidance I had to design around came from that extra hop. This version drops the relay and `IdleSignal` entirely: `ConnectionEvent` itself (`connection.rs`'s existing enum) gains one new variant, `connection::handle`'s own signature does not change at all, and the join bound is re-derived from scratch against every sender and every path out of `run`. Everything else from the first-reviewed version is unchanged.

## For the engineer

**1. AC #4's "starts a fresh one" cannot be shown for the exact-instant race, because no production client exists yet to retry on it - recommend a criterion on TASK-1.49 rather than prose.** TASK-1.77's notes gave this task two doors: close the window in the server, or prove the client's handling. I read both against the actual code (040c5f0's `listen::run`, `crates/sdk/src/local_socket.rs`), not assumed:

- *Closing the window further isn't available.* The listener is already dropped on the same thread, synchronously, immediately after the accept loop breaks - before any connection is joined (the ordering TASK-1.44/45's own reviews already settled, `listen.rs`'s own comment names why). The only remaining race is a client's `connect()` syscall completing into the kernel's backlog a few instructions of Rust before the stop flag is read back on the same iteration, plus whatever the kernel does with an already-queued connection underneath that - narrow, with no mechanism on either side to widen it or to observe it from a test, and not something I can close further without replacing blocking `accept()` with an evented/non-blocking loop, which is a much bigger change this task's three criteria don't ask for.
- *Proving the client's handling isn't available either, honestly.* `local_socket::connect` returns `Ok(stream)` the instant the OS-level `connect()` succeeds - it never reads a byte, so it cannot distinguish "healthy" from "accepted-then-dropped." The code that *would* notice (attempting a handshake, treating a bare EOF the same as refused) only runs in whatever drives `steg_sdk::Connection::new(Role::Client, ...)` for real, which has no production caller yet (TASK-1.49's CLI isn't built). Writing that retry logic only inside a test, to then assert it works, tests the test's own logic, not shipped behaviour - I'm not doing that.
- *What the ordinary case already gives, with zero new client code.* The overwhelming majority of "the instant it is stopping" - anything from the moment `stop()` is called up to the very edge of that one-iteration race - already gets `ECONNREFUSED` or, once the socket file is later removed, `ENOENT`. `local_socket::connect` already treats *any* `Err` from its own `UnixStream::connect` as "no server, autostart" - this needs no change. I prove this end to end, with real processes: an idle server stops itself, a client connecting afterward is refused, and a fresh on-demand start at the same path succeeds.
- *The residual window, named precisely, not left implied.* What a client sees today, in the narrow instant its `connect()` lands in the same accept-loop iteration the stop flag has just become true: `UnixStream::connect` returns `Ok` - a real, connected socket from the client's point of view - and a subsequent read on it sees end-of-file with zero bytes: not a decodable frame, not a protocol error, not `ConnectionRefused`/`NotFound` - a clean close with nothing ever written. Nothing built yet performs that subsequent read as part of becoming a client, so nothing in the repository today can observe this outcome at all, let alone react to it.
- *Recommendation:* accept AC #4 as proven by (a) that ordinary-timing real-process test, (b) the existing `stop_racing_a_client_connecting_still_returns_within_a_bound` (TASK-1.45, unchanged) as proof `run()` never hangs when a connect races `stop()` - the same `Stopper::stop()` function regardless of who calls it, so this task adds no new risk there - and (c) a criterion added to **TASK-1.49** ("CLI: commands generated from the API description, in the steg package" - confirmed, that is the task with no real client-handshake caller yet): *a client whose connection is accepted and then closed before a hello arrives treats that the same as no server being there, and arbitrates again once.* Not prose on this task - an acceptance criterion on that one, worded so its own worker has to build and prove it. If you'd rather this task prove the exact-instant case directly, that needs a different, smaller task first (a minimal real client ahead of TASK-1.49) - say so and I'll replan.

**2. AC #3's "ending its programs" is vacuous for idle-stop's own trigger in production, by construction - one test below deliberately builds the non-vacuous case anyway, to prove the sequence rather than assert it.** The predicate only calls `stop()` when `session_count == 0` (R99: "no sessions and no clients") - in normal operation idle-stop can never fire while any session, let alone a running program, exists. `Runtime::stop()` ending real programs is already proven elsewhere (TASK-1.76/1.78's own tests) - this task doesn't need to re-prove that mechanism, only that this trigger reaches it. "A short injected grace period" is satisfied by injecting a short `idle_timeout`, not `force_grace`. I did **not** touch `listen::run`'s hardcoded `FORCE_GRACE_DEFAULT`; the one test that does construct a real session sidesteps `force_grace` with a fake `ProgramLauncher` instead. I *did* add two small, necessary lines inside `Runtime::stop()` itself (section 4) - flagged there, not hidden.

**3. Not splitting.** Every piece here (the predicate, the one new thread, the `ServerConfig`/entry-point wiring) exists only to serve one behaviour (R99's idle stop) and none is independently reviewable or valuable alone.

**4. This round's simplification, summarised.** The previous version used two threads and two channel types - a relay converting `ConnectionEvent` into a separate `IdleSignal` wrapper purely so `connection.rs` stayed untouched, and a three-step join order (relay, then `runtime.stop()`, then checker) to avoid a deadlock that shape itself created. Simplified below to one thread and one channel: `ConnectionEvent` (already defined in `connection.rs`) gains one new variant, `SessionCountChanged`, so the exact same type flows from `connection::handle`, from `Runtime`'s own notifier, and - once it lands - from TASK-1.98's own addition to the accept loop. `connection::handle`'s signature does not change at all. The join bound is re-derived from the actual set of senders and every path out of `run`, below (section 6) - it turns out to need exactly one join, in one place, not three.

**5. Coordination with TASK-1.98.** That task adds its own `ConnectionEvent` variant (an accept retried after a recoverable error) sent from the accept loop, and also touches `listen::run` - the same enum and the same function this task now touches, though different lines of each (section 6 says exactly which). Per the engineer: it lands first, this task rebases onto it. My own diff to `ConnectionEvent` is one more variant added alongside whatever TASK-1.98 adds - nothing about my own variant's meaning or the checker's handling of it depends on knowing TASK-1.98's exact name or payload (the checker's own match treats any variant it doesn't specifically care about identically - section 5).

## Why this builds on three unmerged PRs, and the risk that carries

Read against PR 47 (3e4594e, TASK-1.78: `Runtime::new`'s third `force_grace` argument, `Runtime::stop` ending every pane within a bound), PR 56 (040c5f0, TASK-1.87: `ServerConfig.login`/`login_identity_timeout`/`login_environment_timeout`, the `fixtures`-gated `fixture_server` binary), and PR 57 (93973fd, TASK-1.89: a merge commit, not itself a feature I depend on for this task - `listen::run`'s shape I'm building against is 040c5f0's, which already contains PR 47 merged in). None of these are on `origin/main` yet, and TASK-1.98 (section 5 above) is a fourth, still-being-planned point of the same kind. **Risk:** if any lands with a shape different from what's quoted below (especially `listen::run`'s body, `Runtime::stop`'s body, `ConnectionEvent`'s own variants, or `ServerConfig`'s field list), my branch needs a rebase before `record-check` - I'll note the exact diff in this task's notes if that happens, not reopen this plan.

## 1. `IdlePredicate` and `Wait`: a pure function of counts and a given time, decoupled from `ConnectionEvent`'s own type

New module `crates/server/src/idle.rs` (not `listen.rs` or `lib.rs` - `.claude/rules/rust.md`: behaviour in named modules). Not exported from `lib.rs` (`pub(crate)` only) - nothing outside this crate needs it.

**Decoupled from `ConnectionEvent` on purpose, this round.** Now that `ConnectionEvent` is a channel two tasks independently widen (section 5), the predicate's own public shape shouldn't depend on its exact variant list growing over time. Two plain methods instead of one that takes the enum:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Wait {
    /// Not idle-eligible (a client is connected, or a session exists):
    /// block on the next real event with no timeout at all.
    Forever,
    /// Idle-eligible, with this much of `idle_timeout` left: block for
    /// exactly this long, or until the next real event, whichever first.
    Until(Duration),
    /// Idle-eligible and `idle_timeout` has already elapsed: decide now.
    Expired,
}

pub(crate) struct IdlePredicate {
    live_clients: i64, // signed: a stray disconnect with no matching connect
                       // must never panic this thread on underflow; harmless if
                       // it goes negative, which nothing in normal operation does
    empty_since: Option<Instant>,
}

impl IdlePredicate {
    pub(crate) fn new() -> Self {
        Self { live_clients: 0, empty_since: None }
    }

    pub(crate) fn client_connected(&mut self) {
        self.live_clients += 1;
        self.empty_since = None; // AC #2: counts from acceptance - no `now` needed,
                                 // since resetting to `None` doesn't record a value
    }

    pub(crate) fn client_disconnected(&mut self) {
        self.live_clients -= 1;
        // empty_since is (re)started on the next wait(), not here.
    }

    /// `now` and `session_count` are both given, never read by this function.
    /// Called whenever the caller wakes - an event arrived, or a previous
    /// `Wait::Until` elapsed - never on its own schedule.
    pub(crate) fn wait(&mut self, now: Instant, session_count: usize, idle_timeout: Duration) -> Wait {
        if self.live_clients > 0 || session_count > 0 {
            self.empty_since = None;
            return Wait::Forever;
        }
        let since = *self.empty_since.get_or_insert(now);
        let elapsed = now.duration_since(since);
        if elapsed >= idle_timeout {
            Wait::Expired
        } else {
            Wait::Until(idle_timeout - elapsed)
        }
    }
}
```
**Correcting TASK-1.77's own carried claim about the arbiter probe, checked rather than repeated.** Its notes said a probe (connect then an almost-immediate disconnect, no hello) "costs at most a few seconds," resting on `get_or_insert`. Tracing it: `client_connected` sets `empty_since = None` unconditionally; `client_disconnected` does not set it back. So the *next* `wait()` call's `get_or_insert(now)` seeds it from whatever `now` happens to be at that call, not from any earlier value - the clock restarts from zero, not "close to where it was." **A probe costs a full idle period, not a few seconds**, and `Wait::Until`'s own returned `Duration` after such a pair is exactly `idle_timeout`, not a partial remainder - checkable directly. Still compatible with AC #2 ("does not *by itself* keep an idle server alive").

Unit tests (`#[cfg(test)]` inline, literal `Instant`s built from one fixed base plus `Duration` offsets, no sleeping):

1. `wait_at_29_30_and_31_seconds_of_idleness` - three literal cases on one otherwise-empty predicate: 29s → `Wait::Until(Duration::from_secs(1))` (exact), 30s → `Wait::Expired`, 31s → `Wait::Expired` (AC #1's own boundary wording).
2. `a_connected_client_with_no_matching_disconnect_never_fires_no_matter_how_far_time_moves` - `client_connected()`, then `wait` at `t0 + 10_000s` → `Wait::Forever` (AC #2: mid-handshake counts from acceptance).
3. `a_nonzero_session_count_never_fires_regardless_of_clients` - `wait(far_future, 1, idle_timeout)` → `Wait::Forever` even with `live_clients == 0`.
4. `a_connect_then_disconnect_within_one_tick_restarts_the_full_period` - the arbiter-probe shape: `client_connected()`, `client_disconnected()`, `wait(t0 + 1ms, 0, idle_timeout)` → `Wait::Until(idle_timeout)` *exactly* - named for what it actually proves, not the "few seconds" framing TASK-1.77 inherited.
5. `a_client_reconnecting_every_29_seconds_never_fires` - five literal cycles of connect (`client_connected()`) then disconnect (`client_disconnected()`) at `t0 + 29*n`s and `t0 + 29*n + 1`s; a `wait` call right after each disconnect returns `Wait::Until(idle_timeout)` (a fresh full period each time) and the next connect always arrives before that period could mature into `Expired`.
6. `empty_from_construction_and_waiting_past_idle_timeout_expires` - an untouched predicate returns `Wait::Until(idle_timeout)` from its very first call, then `Wait::Expired` once that long has passed - R99's own "stops by itself" timing starts from server start. The client that *caused* the on-demand start is itself racing that same 30s clock while it polls for the socket (`local_socket::connect`'s own 20ms retry loop, far below 30s by default) - harmless by construction, named so a reviewer doing the same arithmetic finds the answer rather than redoing it.
7. `wait_until_is_exact_not_approximate` - literal elapsed values (5s, 17s against a 30s `idle_timeout`) each return `Wait::Until(idle_timeout - elapsed)` to the exact `Duration`, not rounded or padded.

**AC #2's arbiter-probe clause, through the real arbiter, needs no separate wiring-level test.** `arbiter::probe` only runs on `ExistingFile::OursAndASocket` - a *contended* start - never on a solo start with nothing there yet, so a real probe is reachable only in the same multi-process race TASK-1.77's own `four_simultaneous_starts_converge_on_one_server` already exercises. What a probe looks like to `run`'s own accept loop is, by `connection.rs`'s own documented caveat, indistinguishable from any short-lived client that connects and leaves before a hello - test 4 above already covers that shape identically.

No negative control for these seven: pure-function unit tests of logic with no mechanism behind them to disable.

## 2. Why the session count can change with no client connected, and where `Runtime` tells the checker

**Checked against the kernel, not assumed.** R94 (doc-15): "Closing a tab's last pane closes the tab, and closing a session's last tab ends the session." R97: "A pane started as a plain shell closes when the shell exits." `Tree::record_exit` runs exactly this cascade when a pane's program ends: `pane.closes_on_exit()` (true for a plain shell pane) calls `self.close_pane(id)`, whose return type `ClosedPane { pane, tab: Option<TabId>, session: Option<SessionId> }` reports `session: Some(id)` *exactly* when this one exit closed a session - precise, not a guess. `apply_event_locked`'s own `PaneEvent::Exited` arm already calls `record_exit`; none of this runs through any client connection - it is driven by the drain thread reading a real pty's event, which can arrive at any time, client or no client. So the session count can drop to zero with nobody connected, and the checker has no other way to learn that, since it never reads the tree on a schedule of its own (section 5's whole point).

**Why only the `PaneEvent::Exited` arm, not `close_and_dispatch`'s own request-driven `session.close`/`tab.close`/`pane.close` path.** That path can also close a session (R94), but it only ever runs as the result of a request on an already-accepted, already-handshaken connection - and `connection::handle` sends `Disconnected` on every path out of itself, including a panic unwinding past it (its own doc comment already says so). So a request-driven close is always followed, sooner or later, by that same connection's own `Disconnected`, which already wakes the checker and makes it re-read the live count. Adding a second notify call on that path would be correct but redundant. The pty-driven path is different in kind - no connection and no `Disconnected` behind it at all - so it is the one case with nothing else to wake the checker.

## 3. `connection.rs`'s one change: a new `ConnectionEvent` variant, nothing else

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionEvent {
    Connected,
    Disconnected,
    /// A pane's exit just closed a session with no client connected to it
    /// (R94/R97's own cascade) - carries no id: a caller that cares always
    /// re-reads the live count (`Runtime::session_counter`), never caches
    /// one against this event. Sent by `Runtime`, never by `connection::
    /// handle` - the one variant here not about one connection's own
    /// lifecycle (TASK-1.98 adds another non-connection-specific one,
    /// an accept retried; this channel is becoming the server's own
    /// internal "something happened" stream, not strictly per-connection,
    /// and keeping one shared enum rather than each task inventing its
    /// own wrapper is what keeps the two edits from colliding).
    SessionCountChanged,
}
```

**`connection::handle`'s own signature does not change, and neither does either of its two existing send call sites.** It already takes `events: Option<Sender<ConnectionEvent>>` and already sends `Connected`/`Disconnected` to it; both are untouched by this task. The only edit inside `connection.rs` is the one variant above and its doc comment.

`runtime.rs` gains: one field on `State` (`notify_session_count_changed: Option<Sender<ConnectionEvent>>`, `None` by construction); one setter, self-locking like `connect_client`/`disconnect_client` already are:

```rust
impl Runtime {
    /// Told once, by `listen::run`, only when idle-checking is on (section
    /// 6) - every pane-exit cascade that closes a session (R94) sends on
    /// this afterward.
    pub(crate) fn set_idle_notifier(&self, notify: Sender<ConnectionEvent>) {
        self.lock().notify_session_count_changed = Some(notify);
    }
}
```

and one addition inside the *existing* `PaneEvent::Exited` arm of `apply_event_locked`, right after `state.panes.remove(&closed.pane)`:

```rust
if closed.session.is_some() {
    if let Some(notify) = &state.notify_session_count_changed {
        let _ = notify.send(ConnectionEvent::SessionCountChanged);
    }
}
```

**Why calling `set_idle_notifier` after `Runtime::new` is safe, not racy.** `listen::run` calls it immediately after constructing `runtime`, before the accept loop starts accepting anything - no pane can exist yet, so no `PaneEvent::Exited` can arrive before the notifier is set.

**The matching addition to `Runtime::stop()` (PR 47/TASK-1.78's own method).** `stop()` already takes every `Option`-wrapped field and empties it (`self.events.take()`, `self.drain.take()`) so a second call is a no-op; this field joins that list for the same reason, and for a second one - section 6's checker, when blocked in `Wait::Forever`, can only be woken by the channel disconnecting if every sender into it, including `Runtime`'s own, is actually dropped:

```rust
state.notify_session_count_changed = None; // drops this Runtime's own sender clone -
    // one of the senders section 6's join bound is derived against
```

## 4. `ServerConfig.idle_timeout: Option<Duration>` - the on/off switch R99 needs

`None` means never idle-stop (an explicit start's default, unchanged); `Some(duration)` is the period:

```rust
pub const DEFAULT_IDLE_TIMEOUT: Duration = Duration::from_secs(30); // R99
```
```rust
pub idle_timeout: Option<Duration>,
```
`ServerConfig::default()` sets it to `None` - **not** `Some(DEFAULT_IDLE_TIMEOUT)`: `run_server_start`'s own `ServerConfig::default()` call is shared between an explicit and an on-demand start; defaulting to `Some` here would make an *explicit* start idle-stop too, which R99 forbids outright. The `on_demand` branch turns it on (section 7) - a one-line, explicit `Some`, not a default.

`Debug for ServerConfig`'s manual impl gets one more `.field(...)` line.

## 5. `Runtime::session_counter` and `idle::run_checker`: exact waits, no polling

```rust
// crates/server/src/runtime.rs
impl Runtime {
    /// A cheap, repeatable way to read the live session count, usable from a
    /// thread that outlives any borrow of `self`. `pub(crate)`: nothing
    /// outside this crate needs it.
    pub(crate) fn session_counter(&self) -> impl Fn() -> usize + Send + 'static {
        let state = Arc::clone(&self.state);
        move || {
            state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .tree
                .sessions()
                .len()
        }
    }
}
```
`listen::run` owns `Runtime` by value (so `runtime.stop()` typechecks only once every connection thread, scoped to `runtime`'s own borrow, has already been joined - TASK-1.45's own property); the checker has to outlive that borrow, so it is given this owned `Arc` clone, never `&Runtime` itself.

```rust
// crates/server/src/idle.rs
pub(crate) fn run_checker(
    rx: Receiver<ConnectionEvent>,
    stop: Stopper,
    session_count: impl Fn() -> usize,
    idle_timeout: Duration,
) {
    run_checker_with_observer(rx, stop, session_count, idle_timeout, |_wait| {});
}

/// `observe` is called once per loop iteration with the `Wait` just
/// computed, before acting on it - production's own `run_checker` passes a
/// no-op (a zero-capture closure the compiler inlines, not a per-iteration
/// indirection in the shipped server); a test passes a recording closure
/// instead, so "how many times did this block with no timeout, how many
/// times with a bounded one, in what order" is counted through a real
/// hook, never inferred from wall-clock timing.
fn run_checker_with_observer(
    rx: Receiver<ConnectionEvent>,
    stop: Stopper,
    session_count: impl Fn() -> usize,
    idle_timeout: Duration,
    mut observe: impl FnMut(&Wait),
) {
    let mut predicate = IdlePredicate::new();
    loop {
        let wait = predicate.wait(Instant::now(), session_count(), idle_timeout);
        observe(&wait);
        let recv_result = match wait {
            Wait::Forever => rx.recv().map_err(|_| mpsc::RecvTimeoutError::Disconnected),
            Wait::Until(remaining) => rx.recv_timeout(remaining),
            Wait::Expired => {
                stop.stop();
                return;
            }
        };
        match recv_result {
            Ok(ConnectionEvent::Connected) => predicate.client_connected(),
            Ok(ConnectionEvent::Disconnected) => predicate.client_disconnected(),
            Ok(_) => {} // any other `ConnectionEvent` variant (`SessionCountChanged`,
                // TASK-1.98's own addition, and anything future) just wakes this loop
                // to re-evaluate on the next `wait()` call - the deliberate trade this
                // catch-all makes: a variant that should have updated the predicate
                // directly but doesn't get a named arm here still gets picked up
                // correctly, because `session_count()` is re-read fresh on every
                // `wait()` call regardless of which event woke it. Nothing here
                // depends on knowing every variant's exact name to stay correct
            Err(mpsc::RecvTimeoutError::Timeout) => {} // loop back - `wait()` will see
                // `Expired` next time, since at least `remaining` has now passed
            Err(mpsc::RecvTimeoutError::Disconnected) => return, // every sender into
                // `rx` is gone (section 6) - run() has already ended
        }
    }
}
```
No new public method on `Stopper` (its public surface was deliberately narrowed by a prior review). Termination is guaranteed without one, by the channel disconnecting - section 6's own derivation, not a poll interval.

## 6. Wiring `listen::run`: one thread, one channel, and the join bound derived from every sender and every exit path

```rust
// crates/server/src/listen.rs
struct IdleWiring {
    checker_tx: Sender<ConnectionEvent>, // cloned per-connection and used by the
                                         // accept loop's own sends (TASK-1.98's own
                                         // addition reads this same variable)
    checker_handle: thread::JoinHandle<()>,
}

fn build_idle_wiring(
    idle_timeout: Option<Duration>,
    runtime: &Runtime,
    stop: &Stopper,
) -> Option<IdleWiring> {
    let idle_timeout = idle_timeout?; // `None` in, `None` out, nothing built -
        // this one line is the whole of "an explicit start spawns no thread"

    let (checker_tx, checker_rx) = mpsc::channel();
    runtime.set_idle_notifier(checker_tx.clone());

    let stop = stop.clone();
    let session_count = runtime.session_counter();
    let checker_handle = thread::Builder::new()
        .name("steg-idle-checker".to_string())
        .spawn(move || idle::run_checker(checker_rx, stop, session_count, idle_timeout))
        .expect("spawning the idle checker thread");

    Some(IdleWiring { checker_tx, checker_handle })
}
```
Called from `run()`, before `thread::scope`, as `build_idle_wiring(config.idle_timeout, &runtime, &stop)`, then split:
```rust
let (checker_tx, checker_handle) = match build_idle_wiring(config.idle_timeout, &runtime, &stop) {
    Some(w) => (Some(w.checker_tx), Some(w.checker_handle)),
    None => (None, None),
};
```
Inside the loop, the per-connection sender becomes `checker_tx.clone().or_else(|| events.clone())` in place of today's bare `events.clone()` - when idle-checking is off (`checker_tx` is `None`), this is `events.clone()`, byte-for-byte what runs today; no behaviour change for an explicit start or any existing test. `checker_tx` is captured by the `move |scope| { ... }` closure exactly as `events` already is.

**Dropped this round: the external-observer-while-idle-checking capability the relay used to carry, and why that's fine.** None of this task's own tests need both `events` (a test observer) and `idle_timeout` set on the same server at once - checked against every test in section 8 before dropping it, not assumed. When `idle_timeout` is `Some`, `checker_tx` is what every connection gets; `events`, if also given, is not read by the per-connection wiring. If a later test genuinely needs both, the fix is for `connection::handle`'s own two existing send call sites to send to a second, external sender too, inline - not a thread brought back to adapt between two types that no longer exist.

**The join bound, re-derived against every sender of `checker_tx` and every path out of `run`, not assumed from the previous round's shape.** Three kinds of sender exist once idle-checking is on:

- **(a) The one `checker_tx` captured by `run`'s own `move |scope| { ... }` closure** - the value every per-connection clone and the accept loop's own send (TASK-1.98's) are made from. Dropped the instant that closure itself returns, i.e. when `thread::scope(...)` returns - the same guarantee `events` already relies on today, unchanged by this task.
- **(b) Each per-connection clone**, moved into its own `scope.spawn(move || connection::handle(...))` closure. Dropped when that thread ends - an ordinary close, a handshake timeout, a protocol violation, *or a panic*: Rust runs a thread's own local destructors during an ordinary unwind regardless of whether `thread::scope`'s own `let _ = join.join();` later discards the panic result, so this clone is gone either way. Already-joined by the existing connections-join loop, on every path out of the loop (a clean stop, a fatal `accept()` error - both break the same loop the same way).
- **(c) `Runtime`'s own one clone**, held in `State.notify_session_count_changed` until `Runtime::stop()` drops it (section 3's one new line).

**Every path out of `run` reaches the same three steps in the same order - a clean stop (the idle checker's own `stop.stop()`, or a later `server stop`), and a fatal accept error alike, since both break the same loop the same way, and a connection panicking changes nothing about (b)'s timing (above):** `drop(listener)` → join every still-open connection (clears every live (b)) → the scope closure returns (clears (a)) → `runtime.stop()` runs (clears (c)). **Only after all three are gone** does joining the checker have a bound that comes from the channel disconnecting rather than a guess:
```rust
runtime.stop();
if let Some(checker_handle) = checker_handle {
    let _ = checker_handle.join(); // (a), every (b), and (c) are all gone by this
        // line - whichever `Wait` state the checker was in, `Forever` or mid-
        // `Until`, its blocking call now sees `Disconnected` and returns.
}
```
One join, in one place, after `runtime.stop()` - not three, and not before it: joining the checker *before* `runtime.stop()` would be the deadlock the previous round's relay design needed two extra steps to avoid, because (c) would still be alive and nothing else would ever wake a `Wait::Forever` with no more events coming.

**Exact lines this touches in 040c5f0's `listen.rs`, for TASK-1.98's ordering (same function, different lines, and the same enum from a different angle - section 5 above):** the `let (stream, _addr) = match listener.accept() ...` arm, `reap_finished(&mut connections)`, and the `if stop.should_stop() { drop(stream); break Ok(()); }` block are **untouched** by this task - those are TASK-1.98's, including wherever it sends its own new `ConnectionEvent` variant from inside that loop (it reads the same `checker_tx`/`events`-effective variable this task's own per-connection line computes, with no new sender of its own to track). This task's only line inside the loop is the per-connection sender computation (today's `let events = events.clone();`, becoming `checker_tx.clone().or_else(...)`); everything else is before the loop (`build_idle_wiring`) or after `thread::scope` returns (`runtime.stop()`, then one join).

## 7. The entry point: on-demand turns idle-stop on, explicitly

`crates/steg/src/main.rs`'s `run_server_start`, in the `Became` branch, after the existing `detach()` call and before constructing `Stopper`:
```rust
let mut config = config;
if on_demand {
    config.idle_timeout = Some(steg_server::DEFAULT_IDLE_TIMEOUT);
}
```
No test-only hook: the same shape as the existing `println!`/`detach()` lines, gated on the same `on_demand` the CLI already parses.

`crates/steg/tests/fixtures/fixture_server.rs` (always mirrors the on-demand path) gets the same line unconditionally, plus one test-only override:
```rust
if let Ok(millis) = std::env::var("STEG_TEST_IDLE_TIMEOUT_MS") {
    config.idle_timeout = Some(Duration::from_millis(
        millis.parse().expect("STEG_TEST_IDLE_TIMEOUT_MS should be a plain integer"),
    ));
} else {
    config.idle_timeout = Some(steg_server::DEFAULT_IDLE_TIMEOUT);
}
```

## 8. Tests

**Two tests stay inside the crate, not in `tests/idle_stop.rs`, because what they call is private.** `build_idle_wiring` (section 6) and `run_checker_with_observer` (section 5) are both plain, unexported `fn`s - keeping them that way, rather than widening either to `pub(crate)` just so an integration test under `tests/` could reach them, is the same narrow-surface choice this plan already makes everywhere else. An integration test is a separate crate and can only call `pub` items, so these two live in the module they test, in its own `#[cfg(test)]` block:

- `crates/server/src/listen.rs`'s own test module gains `build_idle_wiring_spawns_no_thread_when_idle_timeout_is_none`: calls `build_idle_wiring` directly with `idle_timeout: None`, asserts the result is `None` - a structural fact about the one gate the thread is built behind, not a timing inference. No negative control needed - a direct check of a guard clause, not a mechanism that could silently not run.
- `crates/server/src/idle.rs`'s own test module (beside `IdlePredicate`'s unit tests, section 1) gains `the_checker_blocks_with_no_timeout_while_ineligible_and_waits_exactly_once_while_eligible`: calls `run_checker_with_observer` directly, with a `session_count` closure fixed at `0` and a recording `observe` closure. A feeder, synchronised through the same recording channel (never a sleep), sends `Connected` once the first `Wait::Until` has been observed, then `Disconnected` once `Wait::Forever` has been observed. Asserts the exact observed sequence: `[Until(idle_timeout), Forever, Until(idle_timeout), Expired]` - four entries, not an unbounded or periodic number. **Negative control:** temporarily reverting the loop to the earlier, rejected 100ms-poll design (a scratch edit) makes the observed sequence dozens of entries long for the same script; recorded, reverted.

**`crates/server/tests/idle_stop.rs` (new, in-process, real sockets and threads - no process spawn needed at this layer):** reuses `shutdown.rs`'s own `start_server`/`wait_until_accepting`/`join_within` shape with `config.idle_timeout = Some(short)`.

- `a_genuinely_idle_server_stops_itself_within_a_bound`: `wait_until_accepting`, no client ever connects, `join_within(server_thread, idle_timeout + margin)` - `margin` must exceed one connect/accept cycle (a few milliseconds), not just "be generous": it covers the startup probe's own effect on the deadline, which shifts `empty_since` to just after the probe's own disconnect is processed, stated as a deliberate, sized margin rather than drained away as `shutdown.rs`'s older tests do, since nothing here needs the first `accept()` to have definitely cycled past it the way `stop()`'s own wake-up guarantee did. **Negative control:** `idle_timeout: None` instead (scratch edit) - the same server does not stop within the bound; recorded, reverted.
- `a_session_that_exists_prevents_the_idle_stop`: a fake, in-memory `ProgramLauncher` (`runtime.rs`'s own tests already build one) so `session.new` mints a real tree session with no real pty behind it - sidestepping `force_grace` entirely rather than widening the bound to cover it. Create the session over the wire, confirm `run()` does **not** return within `idle_timeout + margin` even with the connection then closed. **Negative control:** the session closed first instead - now it does stop within the bound.
- `closing_a_shell_panes_last_session_with_no_client_connected_still_wakes_the_checker`: the case section 2 exists for - a fake launcher whose fake program can be told to "exit" from the test, a real shell-kind session, a client that creates it and **disconnects** before the program "exits." Only after the connection is gone does the test trigger the exit, cascading (R94) to close the session with zero clients attached - `join_within(server_thread, idle_timeout + margin)` then proves the checker noticed via `ConnectionEvent::SessionCountChanged`, not by ever being woken by a client. **Negative control:** the same scenario with the one new line inside `Runtime::stop()` removed (scratch edit) - the server does not stop within the bound; recorded, reverted.
- `every_sender_dropping_is_what_lets_run_return_after_stop`: a fake launcher, a real session created so the checker is genuinely blocked in `Wait::Forever` (section 6's own hardest case), then the test calls `stop.stop()` directly - an externally-triggered stop, the same shape a later `server stop` command would produce, decoupled from idle-timing entirely. Asserts `run()` returns within a bound (`join_within`). **Negative control, the one this round's review asked for by name:** remove the `state.notify_session_count_changed = None;` line from `Runtime::stop()` (a scratch edit) and rerun the identical script - `join_within`'s own bounded wait now fails (a panic from the bound, not a hang that takes the whole suite down with it), because `Runtime`'s own sender clone is still alive, the channel never disconnects, and the checker's `Wait::Forever` block never wakes; recorded, reverted. This is the direct proof that the join bound in section 6 comes from the senders this task tracked, not from something else making it look safe by coincidence.
- `an_explicit_starts_runtime_behaves_correctly_through_a_full_lifecycle`: `config.idle_timeout: None`, a fake launcher, a full real lifecycle with no delay anywhere - connect, create a session, close it, disconnect - then a fresh connect to the same socket still succeeds, with no wait at all.

**`crates/steg/tests/idle_stop.rs` (new, `fixtures`-gated, real process - AC #1/#3's own "shown end to end with a real server process"):**

- `an_on_demand_server_with_no_clients_stops_itself_and_a_fresh_connect_afterward_starts_a_new_one`: spawns `fixture_server` with `STEG_TEST_IDLE_TIMEOUT_MS` set short plus the existing required `STEG_TEST_LOGIN_*` vars, waits for the socket to accept, confirms no client ever connects, bounded-polls (`try_wait`, never a bare `wait()`) for the process to exit on its own within `idle_timeout + margin`, asserts exit code 0. Then: a fresh connect to the same path fails, and spawning `fixture_server` again at the same path succeeds and prints "started a new server". **Negative control:** drop the `STEG_TEST_IDLE_TIMEOUT_MS`-reading block from `fixture_server.rs` (scratch edit) - the spawned process never exits, the bounded `try_wait` poll times out rather than hanging; recorded, reverted.

`listen.rs`'s own `build_idle_wiring_spawns_no_thread_when_idle_timeout_is_none` (above) is what proves "an explicit start never idle-stops," structurally. TASK-1.77's existing `explicit_start_stays_up_with_no_client_ever_connecting` continues to pass unmodified at the real-process layer.

**Processes and threads, named per the team's own policy:** every `Child` this task's own new tests spawn is wrapped in TASK-1.77's existing `KillOnDrop`; the one checker thread is joined on every path out of `run` (section 6), with the join bound re-derived against every sender rather than assumed; no test sleeps for the idle timeout to elapse - `join_within`/`try_wait`/`recv_timeout`/the observer-driven synchronisation bound every wait.

## Proof, by acceptance criterion

- **AC #1**: unit tests 1, 3, 6 (the predicate) plus `a_genuinely_idle_server_stops_itself_within_a_bound`, `a_session_that_exists_prevents_the_idle_stop`, `closing_a_shell_panes_last_session_with_no_client_connected_still_wakes_the_checker` (each with its own negative control), `build_idle_wiring_spawns_no_thread_when_idle_timeout_is_none` and `an_explicit_starts_runtime_behaves_correctly_through_a_full_lifecycle` plus `an_on_demand_server_with_no_clients_stops_itself...` (a real process, with its own negative control).
- **AC #2**: unit tests 2, 4 (corrected), 5.
- **AC #3**: section 5's `session_counter`, section 6's `stop.stop()` call (the same public function every other caller uses), `an_on_demand_server_with_no_clients_stops_itself...` for the real-process bounded-wait proof, `closing_a_shell_panes_last_session_with_no_client_connected_still_wakes_the_checker` for the one case where "ending its programs" is not vacuous, and `every_sender_dropping_is_what_lets_run_return_after_stop` for the join bound itself, with its own negative control.
- **AC #4**: `an_on_demand_server_with_no_clients_stops_itself_and_a_fresh_connect_afterward_starts_a_new_one` for the ordinary-timing case; `stop_racing_a_client_connecting_still_returns_within_a_bound` (TASK-1.45, unchanged, re-cited not re-proven) for "never hangs"; the exact-instant "starts a fresh one" clause handed to a proposed TASK-1.49 criterion.
- **The checker's own exact-wait property and its join bound** (load-bearing for all four above, not itself a numbered AC): `the_checker_blocks_with_no_timeout_while_ineligible_and_waits_exactly_once_while_eligible` and `every_sender_dropping_is_what_lets_run_return_after_stop`, each with its own negative control.

## Files

- `crates/server/src/idle.rs` (new): `IdlePredicate`, `Wait`, `run_checker`/`run_checker_with_observer`, their unit tests including `the_checker_blocks_with_no_timeout_while_ineligible_and_waits_exactly_once_while_eligible` (private items, tested in this module's own `#[cfg(test)]` block, not from `tests/`).
- `crates/server/src/lib.rs`: `mod idle;` (no new `pub use` - internal only).
- `crates/server/src/connection.rs`: one new `ConnectionEvent` variant, `SessionCountChanged`, and its doc comment widened; nothing else in this file changes.
- `crates/server/src/config.rs`: `idle_timeout: Option<Duration>` field, `DEFAULT_IDLE_TIMEOUT` const, `Debug` impl's one new field line.
- `crates/server/src/runtime.rs`: `Runtime::session_counter`, `Runtime::set_idle_notifier`, `State.notify_session_count_changed`, one new arm's worth of lines inside `apply_event_locked`'s existing `PaneEvent::Exited` match, one new line inside the existing `Runtime::stop()`.
- `crates/server/src/listen.rs`: `build_idle_wiring`, the per-connection sender line, one join after `runtime.stop()`, and `build_idle_wiring_spawns_no_thread_when_idle_timeout_is_none` in this module's own `#[cfg(test)]` block (same reason as `idle.rs`'s own test, above).
- `crates/server/tests/idle_stop.rs` (new, five tests).
- `crates/steg/src/main.rs`: one `if on_demand { config.idle_timeout = Some(...) }` line in `run_server_start`.
- `crates/steg/tests/fixtures/fixture_server.rs`: the same line, unconditionally, plus the `STEG_TEST_IDLE_TIMEOUT_MS` override.
- `crates/steg/Cargo.toml`: one new `[[test]]` entry (`idle_stop`, `required-features = ["fixtures"]`).
- `crates/steg/tests/idle_stop.rs` (new, one test).

No `Cargo.toml` dependency change, no `boundaries.toml` change, no `unsafe`.

## Order of work

1. `idle.rs`'s pure pieces first - `IdlePredicate`/`Wait`, their unit tests - before touching anything else.
2. `idle::run_checker`/`run_checker_with_observer` and `the_checker_blocks_with_no_timeout_while_ineligible_and_waits_exactly_once_while_eligible` (`idle.rs`'s own test module), still with no real server involved.
3. `connection.rs`'s one new variant, `ServerConfig.idle_timeout`, `Runtime::session_counter`, `Runtime::set_idle_notifier` and the `apply_event_locked`/`Runtime::stop` additions - check each against PR 47's and TASK-1.98's actual merged shape if either has landed by then.
4. `listen::run`'s wiring, proven in-process (`crates/server/tests/idle_stop.rs`'s own five tests, plus `build_idle_wiring_spawns_no_thread_when_idle_timeout_is_none` in `listen.rs`'s own test module) before touching `crates/steg` at all.
5. The entry point and fixture changes, then the out-of-process test.
6. `just check` and the affected-crate tests (`just test`) last, recorded.

## Risks

- The three unmerged PRs this reads against, plus TASK-1.98 (section 5) - rebase risk, not a design risk.
- AC #4's exact-instant clause, per "For the engineer" above - the one open decision this plan surfaces rather than guesses at.
- One always-blocking (never-polling) thread per on-demand server (the checker) - bounded, cheap, no different in kind from the drain thread `Runtime` already has; its join bound (section 6) is derived against the current shutdown sequence, and a future change to that sequence should re-check the same three senders rather than assume the ordering still holds.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Carried from TASK-1.77's planning (2026-11-15): when a server is stopping, a connection the accept loop takes in the instant it sees the stop flag is dropped with no reply, so the client sees connect succeed and then end of file, a third outcome beside refused and not found. The idle stop is what makes that instant reachable in ordinary use: this task's fourth criterion (a client connecting as the server stops starts a fresh one) must cover it, either by closing that window in the server or by proving the client's handling.

Plan (7161104a6d7b) checked against the four acceptance criteria, doc-8 R3 and R99, doc-16 and the team's testing rules by @bruk.o, 2026-11-16: ok, with one decision for the engineer. Revised twice at the orchestrator's request: no periodic wake-up (the checker blocks while a client or a session exists and waits exactly to the deadline otherwise); one thread and one channel (the Runtime notifies the checker when a session closes with no client connected, which the kernel's cascade on a pane's exit can do); the join bound derived from where each sender is dropped, with a test and its control. For the engineer: criterion 4's exact-instant case (a connection accepted and then closed before a hello) cannot be observed by anything built yet; the plan proves the ordinary case with real processes and proposes the rest as a criterion on TASK-1.49, the first client that reads a hello. Nit for the build: SessionCountChanged is not a connection's event; a name for the enum that covers both is worth a line.

Check passed: `just check` exited 0 at 2026-11-16T11:39:12+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.86 on task-1.86-server-an-on-demand-server-stops-when at 73e8491d45df:

```text
ok      format check (0.3s)
ok      lint (92.6s)
ok      boundaries (0.2s)
ok      licences (0.8s)
skip    tests: this change touches no crate
ok      build (steg-sdk) (3.1s)
ok      test (steg-sdk) (1.8s)
ok      test (steg-sdk, local-socket) (2.1s)
ok      build (steg-kernel) (43.6s)
ok      test (steg-kernel) (1.6s)
ok      build (steg-server) (48.6s)
ok      test (steg-server) (10.7s)
ok      test (steg-server, race-fixture) (10.7s)
ok      build (steg) (2.1s)
ok      test (steg) (0.4s)
ok      test (steg, fixtures) (3.5s)
ok      build (steg-ui) (24.9s)
ok      test (steg-ui) (1.1s)
ok      build (steg-app) (1.7s)
ok      test (steg-app) (0.8s)
ok      tool tests (12.8s)
ok      architecture (0.7s)
ok      recipe names (0.1s)
ok      ui levels (0.1s)
ok      ui colours (0.1s)
```

Check passed: `just test-all` exited 0 at 2026-11-16T11:40:42+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.86 on task-1.86-server-an-on-demand-server-stops-when at 313e4e8d7699:

```text
ok      tests (every crate) (58.2s)
```

Check passed: `cargo test -p steg-server --lib idle::` exited 0 at 2026-11-16T11:46:17+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.86 on task-1.86-server-an-on-demand-server-stops-when at 313e4e8d7699 with uncommitted changes, last 40 of 41 lines:

```text
  --> crates/server/src/config.rs:51:11
   |
51 | pub const DEFAULT_IDLE_TIMEOUT: Duration = Duration::from_secs(30);
   |           ^^^^^^^^^^^^^^^^^^^^
   |
   = note: `#[warn(dead_code)]` (part of `#[warn(unused)]`) on by default

warning: function `run_checker` is never used
  --> crates/server/src/idle.rs:95:15
   |
95 | pub(crate) fn run_checker(
   |               ^^^^^^^^^^^

warning: methods `session_counter` and `set_idle_notifier` are never used
   --> crates/server/src/runtime.rs:421:19
    |
236 | impl Runtime {
    | ------------ methods in this implementation
...
421 |     pub(crate) fn session_counter(&self) -> impl Fn() -> usize + Send + 'static {
    |                   ^^^^^^^^^^^^^^^
...
442 |     pub(crate) fn set_idle_notifier(&self, notify: Sender<ConnectionEvent>) {
    |                   ^^^^^^^^^^^^^^^^^

warning: `steg-server` (lib test) generated 3 warnings
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.11s
     Running unittests src/lib.rs (target/debug/deps/steg_server-6d2f1d51fdd2e018)

running 8 tests
test idle::tests::empty_from_construction_and_waiting_past_idle_timeout_expires ... ok
test idle::tests::a_nonzero_session_count_never_fires_regardless_of_clients ... ok
test idle::tests::a_connected_client_with_no_matching_disconnect_never_fires_no_matter_how_far_time_moves ... ok
test idle::tests::a_client_reconnecting_every_29_seconds_never_fires ... ok
test idle::tests::a_connect_then_disconnect_within_one_tick_restarts_the_full_period ... ok
test idle::tests::wait_at_29_30_and_31_seconds_of_idleness ... ok
test idle::tests::wait_until_is_exact_not_approximate ... ok
test idle::tests::the_checker_blocks_with_no_timeout_while_ineligible_and_waits_exactly_once_while_eligible ... ok

test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 145 filtered out; finished in 0.06s
```

Check FAILED: `cargo test -p steg-server --lib --test shutdown` exited 101 at 2026-11-16T11:47:41+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.86 on task-1.86-server-an-on-demand-server-stops-when at 313e4e8d7699 with uncommitted changes, last 40 of 191 lines:

```text
test runtime::tests::disconnect_client_reapplies_the_next_latest_declared_size ... ok
test runtime::tests::a_typed_command_still_running_stays_running_until_told_otherwise ... ok
test runtime::tests::steg_vars_and_term_always_win_over_a_conflicting_value_from_the_login_shells_own_base_env ... ok
test runtime::tests::pane_split_falls_back_to_home_when_the_inherited_live_cwd_no_longer_exists ... ok
test runtime::tests::pane_split_inherits_the_live_cwd_of_the_pane_it_split_from_not_its_started_in_cwd ... ok
test login_env::tests::capture_login_environment_falls_back_on_a_shell_that_never_finishes_in_time ... ok
test pane::ending_tests::await_exit_is_not_delayed_by_a_backlog_on_the_workers_own_queue ... ok
kill: 6795: No such process
test runtime::stop_tests::stop_ends_a_program_that_traps_the_polite_signal ... ok
test login_env::tests::capture_login_environment_falls_back_on_a_shell_that_exits_non_zero ... ok
test login_env::tests::capture_login_environment_prints_noise_before_the_marker_without_corrupting_the_dump ... ok
test pane::ending_tests::signal_hangup_reaches_a_backgrounded_grandchild_too ... ok
test runtime::tests::the_general_sizing_step_reaches_every_pane_through_split_close_and_swap ... ok
test runtime::tests::a_mark_then_the_shells_own_real_exit_cascades_with_no_panic ... ok
test runtime::tests::a_shell_with_no_marks_leaves_every_status_field_unchanged_until_one_arrives ... ok
test pane::ending_tests::signal_kill_ends_a_program_that_traps_the_polite_signal ... ok
test runtime::tests::a_typed_command_that_fails_is_failed_with_its_exit_code ... ok
test runtime::tests::a_typed_command_that_succeeds_is_running_then_finished_and_the_tab_shows_it ... ok
test runtime::tests::a_typed_commands_status_reaches_pane_show_and_tab_show_through_runtime_handle ... ok
test runtime::tests::a_command_panes_own_program_printing_marks_does_not_change_its_status ... ok
kill: 7073: No such process
test login_env::tests::capture_login_environment_kills_a_backgrounded_grandchild_that_held_the_pipe_open ... ok
kill: 6791: No such process
test pane::tests::a_forward_failing_once_the_worker_is_gone_still_reaps_a_program_that_ends_on_its_own ... ok

failures:

---- arbiter::tests::attempt_takes_over_a_stale_socket_left_by_a_dead_server stdout ----

thread 'arbiter::tests::attempt_takes_over_a_stale_socket_left_by_a_dead_server' (26979889) panicked at crates/server/src/arbiter.rs:416:17:
expected Became, got something else: AlreadyRunning
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    arbiter::tests::attempt_takes_over_a_stale_socket_left_by_a_dead_server

test result: FAILED. 153 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.06s

error: test failed, to rerun pass `-p steg-server --lib`
```

Check passed: `cargo test -p steg-server --lib` exited 0 at 2026-11-16T11:48:06+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.86 on task-1.86-server-an-on-demand-server-stops-when at 313e4e8d7699 with uncommitted changes, last 40 of 176 lines:

```text
test runtime::stop_tests::stop_is_a_no_op_for_a_pane_whose_program_has_already_ended ... ok
test runtime::tests::login_shell_resolve_identity_never_panics_and_is_never_empty ... ok
test runtime::tests::runtime_new_does_not_panic_and_starts_empty ... ok
test runtime::tests::a_real_exit_event_drives_the_same_cascade_apply_event_runs ... ok
test runtime::tests::a_command_panes_exit_reaches_pane_show_through_runtime_handle ... ok
test signal::tests::send_reaches_a_real_process_with_the_named_signal ... ok
kill: 8853: No such process
test signal::tests::send_reaches_the_whole_process_group_not_only_the_leader ... ok
test runtime::tests::a_plain_close_of_an_already_ended_pane_removes_its_idle_handle ... ok
test runtime::tests::a_shell_pane_not_only_a_command_pane_gets_the_same_composed_environment ... ok
test runtime::tests::apply_event_on_a_pane_the_tree_no_longer_holds_removes_the_stale_handle_without_panicking ... ok
test runtime::tests::apply_size_changed_lets_a_pane_go_under_the_floor_when_the_kernel_says_so ... ok
test pane::ending_tests::restarting_then_forcing_ends_the_new_program_as_a_plain_regression ... ok
test runtime::tests::apply_size_changed_resizes_only_the_panes_the_change_names ... ok
test runtime::tests::disconnect_client_reapplies_the_next_latest_declared_size ... ok
test runtime::tests::steg_vars_and_term_always_win_over_a_conflicting_value_from_the_login_shells_own_base_env ... ok
test runtime::tests::a_typed_command_still_running_stays_running_until_told_otherwise ... ok
test runtime::tests::pane_split_falls_back_to_home_when_the_inherited_live_cwd_no_longer_exists ... ok
test runtime::tests::pane_split_inherits_the_live_cwd_of_the_pane_it_split_from_not_its_started_in_cwd ... ok
test login_env::tests::capture_login_environment_falls_back_on_a_shell_that_never_finishes_in_time ... ok
test pane::ending_tests::await_exit_is_not_delayed_by_a_backlog_on_the_workers_own_queue ... ok
kill: 8677: No such process
test runtime::stop_tests::stop_ends_a_program_that_traps_the_polite_signal ... ok
test login_env::tests::capture_login_environment_prints_noise_before_the_marker_without_corrupting_the_dump ... ok
test login_env::tests::capture_login_environment_falls_back_on_a_shell_that_exits_non_zero ... ok
test pane::ending_tests::signal_hangup_reaches_a_backgrounded_grandchild_too ... ok
test runtime::tests::the_general_sizing_step_reaches_every_pane_through_split_close_and_swap ... ok
test runtime::tests::a_mark_then_the_shells_own_real_exit_cascades_with_no_panic ... ok
test runtime::tests::a_shell_with_no_marks_leaves_every_status_field_unchanged_until_one_arrives ... ok
test pane::ending_tests::signal_kill_ends_a_program_that_traps_the_polite_signal ... ok
test runtime::tests::a_typed_command_that_fails_is_failed_with_its_exit_code ... ok
test runtime::tests::a_typed_command_that_succeeds_is_running_then_finished_and_the_tab_shows_it ... ok
test runtime::tests::a_typed_commands_status_reaches_pane_show_and_tab_show_through_runtime_handle ... ok
test runtime::tests::a_command_panes_own_program_printing_marks_does_not_change_its_status ... ok
kill: 8956: No such process
test login_env::tests::capture_login_environment_kills_a_backgrounded_grandchild_that_held_the_pipe_open ... ok
kill: 8675: No such process
test pane::tests::a_forward_failing_once_the_worker_is_gone_still_reaps_a_program_that_ends_on_its_own ... ok

test result: ok. 154 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.06s
```
<!-- SECTION:NOTES:END -->

## Comments

<!-- COMMENTS:BEGIN -->
author: @bruk
created: 2026-11-16T11:33:00Z

---

Plan approved by @bruk (7161104a6d7b, size-3): its worker may build it.

---
<!-- COMMENTS:END -->
