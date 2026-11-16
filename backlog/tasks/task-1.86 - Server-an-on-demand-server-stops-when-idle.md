---
id: TASK-1.86
title: 'Server: an on-demand server stops when idle'
status: In Progress
assignee:
  - '@bruk.w3'
created_date: '2026-11-15T23:40:00Z'
updated_date: '2026-11-16T08:57:00Z'
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

**Revision note:** this replaces the plan's earlier version after the engineer's review of it. Three changes: (1) the checker now waits exactly, never on a timer, which needed working out whether the session count can change with no client connected (it can - R94/R97's cascade - and the plan now says where `Runtime` tells the checker); (2) AC #4's forward note is now a proposed criterion on TASK-1.49, with the residual window named precisely; (3) "an explicit start never idle-stops" now has a named, event-bounded test rather than a fixed-wait assertion. Nothing else changed from the reviewed version.

## For the engineer

**1. AC #4's "starts a fresh one" cannot be shown for the exact-instant race, because no production client exists yet to retry on it - recommend a criterion on TASK-1.49 rather than prose.** TASK-1.77's notes gave this task two doors: close the window in the server, or prove the client's handling. I read both against the actual code (040c5f0's `listen::run`, `crates/sdk/src/local_socket.rs`), not assumed:

- *Closing the window further isn't available.* The listener is already dropped on the same thread, synchronously, immediately after the accept loop breaks - before any connection is joined (the ordering TASK-1.44/45's own reviews already settled, `listen.rs`'s own comment names why). The only remaining race is a client's `connect()` syscall completing into the kernel's backlog a few instructions of Rust before the stop flag is read back on the same iteration, plus whatever the kernel does with an already-queued connection underneath that - narrow, with no mechanism on either side to widen it or to observe it from a test, and not something I can close further without replacing blocking `accept()` with an evented/non-blocking loop, which is a much bigger change this task's three criteria don't ask for.
- *Proving the client's handling isn't available either, honestly.* `local_socket::connect` returns `Ok(stream)` the instant the OS-level `connect()` succeeds - it never reads a byte, so it cannot distinguish "healthy" from "accepted-then-dropped." The code that *would* notice (attempting a handshake, treating a bare EOF the same as refused) only runs in whatever drives `steg_sdk::Connection::new(Role::Client, ...)` for real, which has no production caller yet (TASK-1.49's CLI isn't built). Writing that retry logic only inside a test, to then assert it works, tests the test's own logic, not shipped behaviour - I'm not doing that.
- *What the ordinary case already gives, with zero new client code.* The overwhelming majority of "the instant it is stopping" - anything from the moment `stop()` is called up to the very edge of that one-iteration race - already gets `ECONNREFUSED` or, once the socket file is later removed, `ENOENT`. `local_socket::connect` already treats *any* `Err` from its own `UnixStream::connect` as "no server, autostart" - this needs no change. I prove this end to end, with real processes: an idle server stops itself, a client connecting afterward is refused, and a fresh on-demand start at the same path succeeds.
- *The residual window, named precisely, not left implied.* What a client sees today, in the narrow instant its `connect()` lands in the same accept-loop iteration the stop flag has just become true: `UnixStream::connect` returns `Ok` - a real, connected socket from the client's point of view - and a subsequent read on it sees end-of-file with zero bytes: not a decodable frame, not a protocol error, not `ConnectionRefused`/`NotFound` - a clean close with nothing ever written. Nothing built yet performs that subsequent read as part of becoming a client, so nothing in the repository today can observe this outcome at all, let alone react to it.
- *Recommendation:* accept AC #4 as proven by (a) that ordinary-timing real-process test, (b) the existing `stop_racing_a_client_connecting_still_returns_within_a_bound` (TASK-1.45, unchanged) as proof `run()` never hangs when a connect races `stop()` - the same `Stopper::stop()` function regardless of who calls it, so this task adds no new risk there - and (c) a criterion added to **TASK-1.49** ("CLI: commands generated from the API description, in the steg package" - confirmed, that is the task with no real client-handshake caller yet): *a client whose connection is accepted and then closed before a hello arrives treats that the same as no server being there, and arbitrates again once.* Not prose on this task - an acceptance criterion on that one, worded so its own worker has to build and prove it, the same way TASK-1.77 carried design notes to me rather than a vague pointer. If you'd rather this task prove the exact-instant case directly, that needs a different, smaller task first (a minimal real client ahead of TASK-1.49) - say so and I'll replan.

**2. AC #3's "ending its programs" is vacuous for idle-stop's own trigger in production, by construction - one test below deliberately builds the non-vacuous case anyway, to prove the sequence rather than assert it.** The predicate only calls `stop()` when `session_count == 0` (R99: "no sessions and no clients") - in normal operation idle-stop can never fire while any session, let alone a running program, exists (a session only disappears from the tree when explicitly closed, not merely because its one pane's program exited - doc-2: "a pane started with a program stays when the program ends"). `Runtime::stop()` ending real programs is already proven elsewhere (TASK-1.76/1.78's own tests) - this task doesn't need to re-prove that mechanism, only that this trigger reaches it. "A short injected grace period" (AC #3's wording) is satisfied by injecting a short `idle_timeout`, not `force_grace`. I did **not** touch `listen::run`'s hardcoded `crate::runtime::FORCE_GRACE_DEFAULT` (a line PR 47/TASK-1.78 just wrote) - no need to; the one test that does construct a real session sidesteps `force_grace` entirely with a fake `ProgramLauncher` (section 9) rather than touching that line. I *did* end up adding two small, necessary lines inside `Runtime::stop()` itself (section 4, below) - flagged there, not hidden.

**3. Not splitting.** I checked for a seam the way the standing default asks. Every piece here (the predicate, the two new threads, the `ServerConfig`/entry-point wiring) exists only to serve one behaviour (R99's idle stop) and none is independently reviewable or valuable alone - unlike TASK-1.77's three unrelated concerns. One task, one PR.

**4. This round's required change, summarised.** The first version of this plan had the checker poll every 100ms for the whole life of every on-demand server. That's wrong for a background terminal server that may sit idle-eligible, or not, for days. Fixed below: the checker blocks with no timeout at all while not idle-eligible, and with a timeout that is exactly the time left to the deadline once it is - which needed confirming whether the session count can change with nobody connected. It can (R94/R97's own cascade: a shell pane's program exiting can close its pane, its tab, and its session, driven entirely by the pty, never by a client) - section 4 is where `Runtime` tells the checker.

## Why this builds on three unmerged PRs, and the risk that carries

Read against PR 47 (3e4594e, TASK-1.78: `Runtime::new`'s third `force_grace` argument, `Runtime::stop` ending every pane within a bound), PR 56 (040c5f0, TASK-1.87: `ServerConfig.login`/`login_identity_timeout`/`login_environment_timeout`, the `fixtures`-gated `fixture_server` binary), and PR 57 (93973fd, TASK-1.89: a merge commit, not itself a feature I depend on for this task - `listen::run`'s shape I'm building against is 040c5f0's, which already contains PR 47 merged in). None of these are on `origin/main` yet. **Risk:** if any lands with a shape different from what's quoted below (especially `listen::run`'s body, `Runtime::stop`'s body, or `ServerConfig`'s field list), my branch needs a rebase before `record-check` - I'll note the exact diff in this task's notes if that happens, not reopen this plan.

## 1. `IdlePredicate` and `Wait`: a pure function of counts and a given time, telling the caller exactly how long to wait rather than only whether to stop now

New module `crates/server/src/idle.rs` (not `listen.rs` or `lib.rs` - `.claude/rules/rust.md`: behaviour in named modules). Not exported from `lib.rs` (`pub(crate)` only) - nothing outside this crate needs it, and R101/R110 keep the sdk as the one public interface; `listen.rs` is the only caller.

```rust
/// What a caller should do next, given the counts and the time just read -
/// never computed on a schedule of its own, only in response to a real
/// event or a wait that has just elapsed (`listen::run_checker`, below).
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
    live_clients: i64, // signed: a stray Disconnected with no matching Connected
                       // must never panic this thread on underflow; harmless if
                       // it goes negative, which nothing in normal operation does
    empty_since: Option<Instant>,
}

impl IdlePredicate {
    pub(crate) fn new() -> Self {
        Self { live_clients: 0, empty_since: None }
    }

    pub(crate) fn on_event(&mut self, event: ConnectionEvent, now: Instant) {
        match event {
            ConnectionEvent::Connected => {
                self.live_clients += 1;
                self.empty_since = None; // AC #2: counts from acceptance
            }
            ConnectionEvent::Disconnected => {
                self.live_clients -= 1;
                // empty_since is (re)started on the next wait(), not here.
            }
        }
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

**Correcting TASK-1.77's own carried claim about the arbiter probe, checked rather than repeated.** Its notes said a probe (`Connected` then an almost-immediate `Disconnected`, no hello) "costs at most a few seconds," resting on `get_or_insert`. Tracing it: `on_event(Connected)` sets `empty_since = None` unconditionally; `on_event(Disconnected)` does not set it back. So the *next* `wait()` call's `get_or_insert(now)` seeds it from whatever `now` happens to be at that call, not from any earlier value - the clock restarts from zero, not "close to where it was." **A probe costs a full idle period, not a few seconds**, and `Wait::Until`'s own returned `Duration` after such a pair is exactly `idle_timeout`, not a partial remainder - checkable directly, not just inferred. Still compatible with AC #2 ("does not *by itself* keep an idle server alive" - one probe delays once, it doesn't hold indefinitely).

Unit tests (`#[cfg(test)]` inline, literal `Instant`s built from one fixed base plus `Duration` offsets, no sleeping):

1. `wait_at_29_30_and_31_seconds_of_idleness` - three literal cases on one otherwise-empty predicate: 29s → `Wait::Until(Duration::from_secs(1))` (exact), 30s → `Wait::Expired`, 31s → `Wait::Expired` (AC #1's own boundary wording).
2. `a_connected_with_no_matching_disconnected_never_fires_no_matter_how_far_time_moves` - `on_event(Connected, t0)`, then `wait` at `t0 + 10_000s` → `Wait::Forever` (AC #2: mid-handshake counts from acceptance).
3. `a_nonzero_session_count_never_fires_regardless_of_clients` - `wait(far_future, 1, idle_timeout)` → `Wait::Forever` even with `live_clients == 0`.
4. `a_connect_then_disconnect_within_one_tick_restarts_the_full_period` - the arbiter-probe shape: `on_event(Connected, t0)`, `on_event(Disconnected, t0 + 1ms)`, `wait(t0 + 1ms, 0, idle_timeout)` → `Wait::Until(idle_timeout)` *exactly* (not a partial remainder) - named for what it actually proves (restarts from the wait call, full period, not the "few seconds" framing TASK-1.77 inherited).
5. `a_client_reconnecting_every_29_seconds_never_fires` - five literal cycles of `Connected` at `t0 + 29*n`s, `Disconnected` at `t0 + 29*n + 1`s (modelling the handshake timeout closing it before the next arrives); a `wait` call right after each disconnect returns `Wait::Until(idle_timeout)` (a fresh full period each time, never cumulative) and the next `Connected` always arrives before that period could mature into `Expired` - "stated and tested," AC #2's own open question, with no real time elapsed.
6. `empty_from_construction_and_waiting_past_idle_timeout_expires` - the base case: an untouched predicate (matching a just-started, genuinely idle server) returns `Wait::Until(idle_timeout)` from its very first call, then `Wait::Expired` once that long has passed - R99's own "stops by itself" timing starts from server start, not from the first client leaving. This means the client that *caused* the on-demand start is itself racing that same 30s clock while it polls for the socket to accept (`local_socket::connect`'s own 20ms retry loop, `connect_timeout` far below 30s by default) - harmless by construction (the default `idle_timeout` is two orders of magnitude larger than a normal connect), named here so a reviewer doing the same arithmetic finds the answer rather than redoing it.
7. `wait_until_is_exact_not_approximate` - literal elapsed values (5s, 17s against a 30s `idle_timeout`) each return `Wait::Until(idle_timeout - elapsed)` to the exact `Duration`, not a rounded or padded one - backs "a timeout that is exactly the time left to the deadline" with a value a reviewer can check, not inferred from behaviour.

**AC #2's arbiter-probe clause, through the real arbiter, needs no separate wiring-level test.** `arbiter::probe` (040c5f0's `arbiter.rs:177`) only runs on `ExistingFile::OursAndASocket` - a *contended* start, where a file already exists and another process checks whether it's a live one of ours - never on a solo start with nothing there yet. So a real probe is reachable only in the same multi-process race TASK-1.77's own `four_simultaneous_starts_converge_on_one_server` already exercises, not in any test this task adds. What a probe looks like to `run`'s own accept loop (`Connected` then an immediate `Disconnected`, no hello) is, by `connection.rs`'s own documented caveat, indistinguishable from *any* short-lived client that connects and leaves before a hello - the server has no signal to tell them apart, so there is nothing a probe-specific test could exercise that test 4 above does not already cover identically. Named here so a reviewer checking this clause finds the reasoning rather than a gap.

No negative control for these seven: pure-function unit tests of logic with no mechanism behind them to disable (team testing skill: "an ordinary unit test of a pure function does not need one").

## 2. Why the session count can change with no client connected, and where `Runtime` tells the checker

**Checked against the kernel, not assumed.** R94 (doc-15): "Closing a tab's last pane closes the tab, and closing a session's last tab ends the session." R97: "A pane started as a plain shell closes when the shell exits." `Tree::record_exit` (`crates/kernel/src/tree.rs`) runs exactly this cascade when a pane's program ends: `pane.closes_on_exit()` (true for a plain shell pane) calls `self.close_pane(id)`, whose return type `ClosedPane { pane, tab: Option<TabId>, session: Option<SessionId> }` reports `session: Some(id)` *exactly* when this one exit closed a session - precise, not a guess. `apply_event_locked`'s own `PaneEvent::Exited` arm already calls `record_exit` and matches on `ExitOutcome::Closed(closed)`; none of this runs through any client connection at all - it is driven by the drain thread reading a real pty's `PaneEvent`, which can arrive at any time, client or no client. **So yes, the session count can drop to zero with nobody connected, and the checker has no other way to learn that**, since it never reads the tree on a schedule of its own (section 5's whole point).

`idle.rs` gains a second type, carried on the same channel the checker already reads:

```rust
pub(crate) enum IdleSignal {
    Connection(ConnectionEvent),
    /// A pane's exit just closed a session (`ClosedPane::session: Some(_)`) -
    /// carries no id: the checker only ever needs "re-read the count," never
    /// which session, and re-reading is already exact and cheap (`Runtime::
    /// session_counter`, section 4).
    SessionCountChanged,
}
```

`runtime.rs` gains: one field on `State` (`notify_session_count_changed: Option<Sender<idle::IdleSignal>>`, `None` by construction - nothing sets it until `listen::run` does, section 7); one setter, self-locking like `connect_client`/`disconnect_client` already are:

```rust
impl Runtime {
    /// Told once, by `listen::run`, only when idle-checking is on (section
    /// 7) - every pane-exit cascade that closes a session (R94) sends on
    /// this afterward, so the checker learns of it the instant it happens,
    /// never by polling the tree.
    pub(crate) fn set_idle_notifier(&self, notify: Sender<idle::IdleSignal>) {
        self.lock().notify_session_count_changed = Some(notify);
    }
}
```

and one addition inside the *existing* `PaneEvent::Exited` arm of `apply_event_locked`, right after `state.panes.remove(&closed.pane)`:

```rust
if closed.session.is_some() {
    if let Some(notify) = &state.notify_session_count_changed {
        let _ = notify.send(idle::IdleSignal::SessionCountChanged);
    }
}
```

**Why only the `PaneEvent::Exited` arm, not `close_and_dispatch`'s own request-driven `session.close`/`tab.close`/`pane.close` path.** That path can also close a session (R94), but it only ever runs as the result of a request on an already-accepted, already-handshaken connection - and `connection::handle` sends `Disconnected` on every path out of itself, including a panic unwinding past it (its own doc comment already says so). So a request-driven close is always followed, sooner or later, by that same connection's own `Disconnected` - which already wakes the checker (section 5) and makes it re-read the live count via `wait()`'s own `session_count()` call, correctly. Adding a second notifier call on that path would be correct but redundant - never the thing that makes the server idle-eligible when nothing else would have. The pty-driven path (`apply_event_locked`) is different in kind: it runs on the drain thread, with no connection and no `Disconnected` behind it at all, so it is the one case with nothing else to wake the checker - the only place this task adds a notifier call.

**Why calling `set_idle_notifier` after `Runtime::new` is safe, not racy.** `listen::run` calls it (section 7) immediately after constructing `runtime`, before the accept loop starts accepting anything - no pane can exist yet (a pane is only ever minted by a request, which needs an accepted, handshaken connection, which needs the accept loop already running), so no `PaneEvent::Exited` can arrive before the notifier is set. Stated plainly rather than left to be trusted.

**The matching addition to `Runtime::stop()` (PR 47/TASK-1.78's own method - a small, necessary touch, not avoided this time).** `stop()` already takes every `Option`-wrapped field and empties it (`self.events.take()`, `self.drain.take()`) so a second call is a no-op; this field joins that list for the same reason, and for a second one - section 7's checker thread, when blocked in `Wait::Forever`, can only be woken by the channel disconnecting if every sender into it, including `Runtime`'s own, is actually dropped. One line, inside the lock `stop()` already takes:

```rust
state.notify_session_count_changed = None; // drops this Runtime's own sender clone,
    // one of the last things keeping the checker's channel connected (section 7)
```

## 3. `ServerConfig.idle_timeout: Option<Duration>` - the on/off switch R99 needs

`None` means never idle-stop (an explicit start's default, unchanged); `Some(duration)` is the period. Added to `crates/server/src/config.rs` beside the existing injected-time fields, same pattern as `login_identity_timeout`:

```rust
pub const DEFAULT_IDLE_TIMEOUT: Duration = Duration::from_secs(30); // R99
```
```rust
pub idle_timeout: Option<Duration>,
```
`ServerConfig::default()` sets it to `None` - **not** `Some(DEFAULT_IDLE_TIMEOUT)`. `run_server_start`'s own `ServerConfig::default()` call is shared between an explicit and an on-demand start (section 5 of TASK-1.77's plan); defaulting to `Some` here would make an *explicit* start idle-stop too, which R99 forbids outright ("a server started explicitly stops only when told to"). The `on_demand` branch is what turns it on (section 8 below) - a one-line, explicit `Some`, not a default, the same reasoning TASK-1.87 gave for writing `config.login = ...` explicitly rather than leaving it to a default that happens to match.

`Debug for ServerConfig`'s manual impl gets one more `.field(...)` line (it's a plain `Option<Duration>`, no special handling needed, unlike `launcher`).

## 4. `Runtime::session_counter` - the seam into a non-`Arc` `Runtime`

`listen::run` owns `Runtime` by value (so `runtime.stop()` typechecks only once every connection thread, scoped to `runtime`'s own borrow, has already been joined - TASK-1.45's own property). The checker thread has to outlive that borrow, so it cannot hold `&Runtime` itself. `Runtime` already holds `state: Arc<Mutex<State>>` - the checker needs only a cheap, independently-cloneable read, so I give it an owned `Arc` clone, not a borrow:

```rust
// crates/server/src/runtime.rs
impl Runtime {
    /// A cheap, repeatable way to read the live session count, usable from a
    /// thread that outlives any borrow of `self` (`listen::run`'s idle
    /// checker - TASK-1.86). `pub(crate)`: nothing outside this crate needs
    /// it, same reasoning as every other internal seam here.
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
Built once in `listen::run`, before `thread::scope` - calling it needs only `&Runtime`, no conflict with the later `&mut self` `runtime.stop()` call, since the returned closure holds no borrow of `runtime` at all, only its own `Arc` clone.

## 5. `idle::run_checker`: exact waits, decided by an `IdleSignal`, no polling

```rust
pub(crate) fn run_checker(
    rx: Receiver<IdleSignal>,
    stop: Stopper,
    session_count: impl Fn() -> usize,
    idle_timeout: Duration,
) {
    // `|_wait| {}` is a zero-capture, zero-field closure - the compiler
    // monomorphises `run_checker_with_observer` for this exact type and
    // inlines the call, the same as every other no-op callback already in
    // this codebase (e.g. the sdk's own default handlers); not a per-
    // iteration indirection in the shipped server.
    run_checker_with_observer(rx, stop, session_count, idle_timeout, |_wait| {});
}

/// `observe` is called once per loop iteration, with the `Wait` just
/// computed, before acting on it - production's own `run_checker` passes a
/// no-op; a test passes a counting/recording closure instead, so "how many
/// times did this block with no timeout, how many times with a bounded
/// one, in what order" is counted through a real hook the loop itself
/// calls, never inferred from wall-clock timing (`crates/server/src/idle.
/// rs`'s own `#[cfg(test)]` module is the only other caller - `run_checker`
/// is still what every production path calls, unchanged).
fn run_checker_with_observer(
    rx: Receiver<IdleSignal>,
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
            Ok(IdleSignal::Connection(event)) => predicate.on_event(event, Instant::now()),
            Ok(IdleSignal::SessionCountChanged) => {} // nothing to update on the predicate
                // itself - session_count is read fresh on every `wait()` call, never
                // cached; this signal exists only to wake a `Wait::Forever` block
                // promptly instead of leaving a session-less, client-less server
                // waiting indefinitely for a `Connected` that may never come.
            Err(mpsc::RecvTimeoutError::Timeout) => {} // loop back - `wait()` will see
                // `Expired` next time, since at least `remaining` has now passed
            Err(mpsc::RecvTimeoutError::Disconnected) => return, // every sender into
                // `rx` is gone (section 7) - run() has already ended; nothing left to
                // decide, whether or not this thread's own wait ever matured
        }
    }
}
```
No new public method on `Stopper` (its public surface was deliberately narrowed by a prior review - "there is now one public way to stop, and it always works" - adding a query method back would cut against that). Termination is guaranteed without one, by the channel disconnecting - see section 7's own join ordering, not a poll interval.

## 6. `idle::run_connection_relay`: the one place `ConnectionEvent` becomes `IdleSignal`

No change to `connection::handle`'s signature, and no change to `connection.rs` at all - it keeps sending plain `ConnectionEvent`s to whatever `Sender<ConnectionEvent>` it's given, exactly as today. A small, always-blocking (never-polling) relay thread is what turns those into `IdleSignal` for the checker, and is also where forwarding to an external test-observer (`events`, the existing public parameter) happens when idle-checking is on - the checker itself (section 5) never needs to know `events` exists:

```rust
pub(crate) fn run_connection_relay(
    rx: Receiver<ConnectionEvent>,
    combined: Sender<IdleSignal>,
    forward_to: Option<Sender<ConnectionEvent>>,
) {
    for event in rx {
        if let Some(forward_to) = &forward_to {
            let _ = forward_to.send(event);
        }
        let _ = combined.send(IdleSignal::Connection(event));
    }
    // `rx`'s own iterator ends the instant every sender into it - every
    // per-connection clone (joined inside `thread::scope`) and this
    // function's one caller's own clone (dropped when that scope returns,
    // section 7) - is gone. No timeout anywhere in this function.
}
```

## 7. Wiring `listen::run`: two threads, built only when idle-checking is on, joined in the order their own inputs actually disconnect

Factored into its own function, rather than inlined where it's called, for one reason beyond readability: it is then directly, structurally testable - "does this spawn anything at all when `idle_timeout` is `None`" becomes a plain call and an `assert!`, not a timing-based inference (section 9).

```rust
// crates/server/src/listen.rs
struct IdleWiring {
    connection_relay_tx: Sender<ConnectionEvent>, // cloned per-connection, replacing
                                                   // today's bare `events.clone()`
    relay_handle: thread::JoinHandle<()>,
    checker_handle: thread::JoinHandle<()>,
}

fn build_idle_wiring(
    idle_timeout: Option<Duration>,
    runtime: &Runtime,
    stop: &Stopper,
    events: Option<Sender<ConnectionEvent>>,
) -> Option<IdleWiring> {
    let idle_timeout = idle_timeout?; // `None` in, `None` out, nothing built -
        // this one line is the whole of "an explicit start spawns neither thread"

    let (connection_relay_tx, connection_relay_rx) = mpsc::channel();
    let (combined_tx, combined_rx) = mpsc::channel();
    runtime.set_idle_notifier(combined_tx.clone());

    let relay_combined_tx = combined_tx.clone();
    let relay_handle = thread::Builder::new()
        .name("steg-idle-relay".to_string())
        .spawn(move || idle::run_connection_relay(connection_relay_rx, relay_combined_tx, events))
        .expect("spawning the idle connection-relay thread");
    drop(combined_tx); // this function's own clone - only the relay's (above) and
                       // `Runtime`'s own (`set_idle_notifier`, just above) remain live

    let stop = stop.clone();
    let session_count = runtime.session_counter();
    let checker_handle = thread::Builder::new()
        .name("steg-idle-checker".to_string())
        .spawn(move || idle::run_checker(combined_rx, stop, session_count, idle_timeout))
        .expect("spawning the idle checker thread");

    Some(IdleWiring { connection_relay_tx, relay_handle, checker_handle })
}
```
Called from `run()`, before `thread::scope`, as `build_idle_wiring(config.idle_timeout, &runtime, &stop, events.clone())`. Everything the `Some` path needs - the channels, both thread spawns - is produced inside this one function, so there is no second place that has to agree the pieces are all present or all absent; no invariant to restate with an `.expect()`.

Before `thread::scope`, split the result so only what the connection loop needs is moved into it:
```rust
let (connection_relay_tx, relay_handle, checker_handle) = match build_idle_wiring(config.idle_timeout, &runtime, &stop, events.clone()) {
    Some(w) => (Some(w.connection_relay_tx), Some(w.relay_handle), Some(w.checker_handle)),
    None => (None, None, None),
};
```
Inside the loop, the per-connection sender becomes `connection_relay_tx.clone().or_else(|| events.clone())` in place of today's bare `events.clone()` - when idle-checking is off (`connection_relay_tx` is `None`), this is `events.clone()`, byte-for-byte what runs today; no behaviour change for an explicit start or any existing test (none sets `idle_timeout`, so it stays `None`). `connection_relay_tx` is captured by the `move |scope| { ... }` closure exactly as `events` already is, and its last clone drops when `thread::scope` returns.

**Join order - the part this round's review asked to fix - each handle joined only once what it is blocked on is actually gone, never by a poll interval:**
```rust
// after `drop(listener)` and the existing connections-join loop, still inside
// `thread::scope`'s return, before `runtime.stop()`:
if let Some(relay_handle) = relay_handle {
    let _ = relay_handle.join(); // `connection_relay_rx` has already disconnected:
        // every per-connection clone is joined above, and this function's own
        // clone (captured by the scope's closure) just dropped when the scope
        // returned - nothing left to send, so the relay's `for event in rx`
        // already ended on its own.
}

runtime.stop(); // now also drops `state.notify_session_count_changed` (section 2) -
    // the last live sender into the checker's channel besides the relay's, which
    // is already gone (joined just above).

if let Some(checker_handle) = checker_handle {
    let _ = checker_handle.join(); // every sender into `combined_rx` is now gone
        // (the relay's, just joined; `Runtime`'s own, just dropped by `stop()`
        // above) - whichever `Wait` state the checker was in, `Forever` or
        // mid-`Until`, its blocking call now sees `Disconnected` and returns.
        // This bound comes from the channel disconnecting, not from any poll
        // interval - there is none left in this design.
}
```
**Why this exact order, not relay-then-checker-then-stop or some other arrangement.** If `runtime.stop()` ran *after* joining the checker, a checker genuinely blocked in `Wait::Forever` (a session still open when some other mechanism, not this task's, calls `stop()` directly - e.g. a later `server stop`) would deadlock: `Runtime`'s own sender clone is the last thing keeping that channel connected, and nothing else would ever wake a no-timeout `recv()`. Joining the relay before `runtime.stop()` matters for the same reason in miniature - it's one of the two senders the checker's disconnect depends on, and it is itself already guaranteed to end once the scope above returns, so there's no cost to doing it first.

**Exact lines this touches in 040c5f0's `listen.rs`, for TASK-1.98's ordering (same accept loop, different lines):** the `let (stream, _addr) = match listener.accept() ...` arm, `reap_finished(&mut connections)`, and the `if stop.should_stop() { drop(stream); break Ok(()); }` block are **untouched** by this task - those are TASK-1.98's. This task's only line inside the loop is the one computing the per-connection sender (today's bare `let events = events.clone();`, becoming the `connection_relay_tx.clone().or_else(...)` line above); everything else is before the loop (building `idle_wiring`) or after `thread::scope` returns (joining the relay, then `runtime.stop()`, then joining the checker).

## 8. The entry point: on-demand turns idle-stop on, explicitly

`crates/steg/src/main.rs`'s `run_server_start`, in the `Became` branch, after the existing `detach()` call and before constructing `Stopper`:
```rust
let mut config = config;
if on_demand {
    config.idle_timeout = Some(steg_server::DEFAULT_IDLE_TIMEOUT);
}
```
(`DEFAULT_IDLE_TIMEOUT` re-exported from `lib.rs` alongside the existing `DEFAULT_HANDSHAKE_TIMEOUT`/`DEFAULT_ARBITRATION_TIMEOUT`.) No test-only hook: this is the same shape as the existing `println!`/`detach()` lines, gated on the same `on_demand` the CLI already parses, nothing a test controls that production can't reach.

`crates/steg/tests/fixtures/fixture_server.rs` (always mirrors the on-demand path - its own doc comment already says so) gets the same line unconditionally, plus one test-only override so an out-of-process test can inject a short period without touching the shipped binary:
```rust
if let Ok(millis) = std::env::var("STEG_TEST_IDLE_TIMEOUT_MS") {
    config.idle_timeout = Some(Duration::from_millis(
        millis.parse().expect("STEG_TEST_IDLE_TIMEOUT_MS should be a plain integer"),
    ));
} else {
    config.idle_timeout = Some(steg_server::DEFAULT_IDLE_TIMEOUT);
}
```

## 9. Tests

**`crates/server/tests/idle_stop.rs` (new, in-process, real sockets and threads - no process spawn needed at this layer):** reuses `shutdown.rs`'s own `start_server`/`wait_until_accepting`/`join_within` shape (same crate, same file conventions) with `config.idle_timeout = Some(short)`.

- `a_genuinely_idle_server_stops_itself_within_a_bound`: `start_truly_idle_server`-style setup (drains the startup probe first, same reason `shutdown.rs` already gives), no client ever connects after that, `join_within(server_thread, idle_timeout + margin)`. **Negative control:** with `idle_timeout` set to `None` instead (scratch edit), the same server does *not* stop within the bound - recorded, reverted.
- `a_session_that_exists_prevents_the_idle_stop`: start a server with `config.launcher` overridden to a fake, in-memory `ProgramLauncher` (`runtime.rs`'s own tests already build one - reused, not reinvented) so `session.new` mints a real tree session with no real pty and no real program behind it - sidestepping `FORCE_GRACE_DEFAULT`/`ESCALATE_CONFIRM_TIMEOUT`/`FORCE_JOIN_TIMEOUT` entirely, rather than widening the test's bound to cover them. Create the session over the wire (reusing `shutdown.rs`'s own handshake-completion helper plus a `session.new` call), confirm `run()` does **not** return within `idle_timeout + margin` even with the connection then closed. **Negative control:** same scenario with the session closed first (`session.close`) - now it *does* stop within the bound, proving the session, not the connection, is what was holding it open.
- `closing_a_shell_panes_last_session_with_no_client_connected_still_wakes_the_checker`: the case section 2 exists for - a fake `ProgramLauncher` whose fake program can be told to "exit" from the test (the same shape `runtime.rs`'s own fake launcher tests already use to simulate a program ending), a real shell-kind session with no command (so it closes on exit, R97), a client that creates it, sends input or otherwise completes its own part, and **disconnects** before the program "exits." Only after the connection is gone does the test trigger the fake program's exit, cascading (R94) to close the session with zero clients attached - `join_within(server_thread, idle_timeout + margin)` then proves the checker noticed via `IdleSignal::SessionCountChanged`, not by ever being woken by a client. **Negative control:** the same scenario with `Runtime::set_idle_notifier` never called (scratch edit - skip that one line in `listen::run`) - the server does *not* stop within the bound, because nothing told the checker the session was gone while it was blocked in `Wait::Forever`; recorded, reverted.
- `build_idle_wiring_spawns_neither_thread_when_idle_timeout_is_none`: calls `listen::build_idle_wiring` directly (same crate, `pub(crate)` or private - no process, no real server, no timing of any kind) with `idle_timeout: None`, asserts the result is `None`. This, not a timing-bounded test, is **the** proof that an explicit start never idle-stops: it is a structural fact about the one gate both new threads are built behind (section 7's own `?` on the first line), checked directly rather than inferred from "didn't stop within however long we waited" - which would only ever show *slow*, never *never*, however long that bound was made. No negative control needed, for the same reason the pure predicate's own unit tests need none: a direct check of a guard clause, not a mechanism that could silently not run.
- `an_explicit_starts_runtime_behaves_correctly_through_a_full_lifecycle`: `config.idle_timeout: None`, a fake launcher, a full real lifecycle with no delay anywhere - connect, create a session, close it, disconnect - then, with **no wait at all**, a **fresh connect to the same socket still succeeds**. This is the behavioural complement to the structural test above, not a second proof of "never spawns a checker" - it shows the explicit path still works end to end with this task's changes present, through the one path (`connection_relay_tx` staying `None`, per `build_idle_wiring`) that keeps `connection::handle`'s per-connection sender exactly what it is today.
- `the_checker_blocks_with_no_timeout_while_ineligible_and_waits_exactly_once_while_eligible`: calls `idle::run_checker_with_observer` directly (both `pub(crate)`, same crate - no thread, no process, no real server needed for this one), with an injected `session_count` closure fixed at `0` and a recording `observe` closure. A feeder, synchronised through the same recording channel (never a sleep), sends `IdleSignal::Connection(Connected)` once the first `Wait::Until` has been observed, then `Connection(Disconnected)` once `Wait::Forever` has been observed. Asserts the exact observed sequence: `[Until(idle_timeout), Forever, Until(idle_timeout), Expired]` - four entries, not an unbounded or periodic number - which is this task's own direct proof of "no wake-up while ineligible, exactly one timed wait per eligible period," counted through the hook itself rather than inferred from timing. **Negative control:** temporarily reverting `run_checker_with_observer`'s loop to the earlier, rejected 100ms-poll design (a scratch edit) makes the observed sequence dozens of entries long for the same script, instead of four - recorded, reverted.

**`crates/steg/tests/idle_stop.rs` (new, `fixtures`-gated, real process - AC #1/#3's own "shown end to end with a real server process"):**

- `an_on_demand_server_with_no_clients_stops_itself_and_a_fresh_connect_afterward_starts_a_new_one`: spawns `fixture_server` with `STEG_TEST_IDLE_TIMEOUT_MS` set short plus the existing required `STEG_TEST_LOGIN_*` vars, waits for the socket to accept, confirms no client ever connects, bounded-polls (`try_wait`, never a bare `wait()`) for the process to exit on its own within `idle_timeout + margin`, asserts exit code 0. Then: a fresh `UnixStream::connect` to the same path fails (refused or not found - either is correct, same framing `stop_racing_a_client_connecting_still_returns_within_a_bound` already uses for this exact ambiguity), and spawning `fixture_server` again at the same path succeeds and prints "started a new server" (AC #4's ordinary-timing case, proven with real processes, zero new client code - the sdk's existing autostart already does this). **Negative control:** drop the `STEG_TEST_IDLE_TIMEOUT_MS`-reading block from `fixture_server.rs` in a scratch edit (so `config.idle_timeout` stays `None` even in the fixture) - the spawned process never exits, the bounded `try_wait` poll times out, the test fails loudly rather than hanging; recorded, reverted.

`crates/server/tests/idle_stop.rs`'s own `build_idle_wiring_spawns_neither_thread_when_idle_timeout_is_none` (above) is what proves "an explicit start never idle-stops," structurally, at the layer where `idle_timeout` actually lives. TASK-1.77's existing `explicit_start_stays_up_with_no_client_ever_connecting` continues to pass unmodified at the real-process layer - that is what "the explicit path is unaffected" needs there, not a second, slower copy of the same proof.

**Processes and threads, named per the team's own policy:** every `Child` this task's own new tests spawn is wrapped in TASK-1.77's existing `KillOnDrop` (reused, not reinvented); the relay and checker threads are each joined on every path out of `run()` (section 7), in the order their own disconnect condition is actually met, never by a poll interval; no test sleeps for the idle timeout to elapse - `join_within`/`try_wait`/`recv_timeout`/the observer-driven synchronisation bound every wait, and the *only* literal `Instant::now()`-driven real time in this whole task is the checker's own production clock reads and the two out-of-process tests' injected short `idle_timeout` - never a test computing its own deadline by sleeping past one.

## Proof, by acceptance criterion

- **AC #1** (a server started on demand stops itself after 30s with no sessions/clients; one with a session, or started explicitly, never does; boundary at 29/30/31s with literal values): unit tests 1, 3, 6 above (the predicate itself) plus `a_genuinely_idle_server_stops_itself_within_a_bound`, `a_session_that_exists_prevents_the_idle_stop`, `closing_a_shell_panes_last_session_with_no_client_connected_still_wakes_the_checker` (the wiring, each with its own negative control), `build_idle_wiring_spawns_neither_thread_when_idle_timeout_is_none` (the explicit path, structurally) and `an_explicit_starts_runtime_behaves_correctly_through_a_full_lifecycle` (the same path, behaviourally) plus `an_on_demand_server_with_no_clients_stops_itself...` (a real process, with its own negative control).
- **AC #2** (counts from acceptance, not handshake completion; the arbiter probe does not by itself keep it alive; the 29-second reconnector is stated and tested): unit tests 2, 4 (corrected), 5 - `a_connected_with_no_matching_disconnected_never_fires...`, `a_connect_then_disconnect_within_one_tick_restarts_the_full_period`, `a_client_reconnecting_every_29_seconds_never_fires`.
- **AC #3** (session count from `Runtime`; stop via the one `Stopper::stop` function, then the `Runtime`; shown end to end with real process, short injected grace, bounded waits): section 4's `session_counter` (reads `Tree::sessions().len()` through `Runtime`'s own state, nothing duplicated), section 7's `stop.stop()` call (the same public function every other caller uses), `an_on_demand_server_with_no_clients_stops_itself...` for the real-process, bounded-wait proof, and `closing_a_shell_panes_last_session_with_no_client_connected_still_wakes_the_checker` for the one case where "ending its programs" is not vacuous - with the general vacuity of that clause for idle-stop's own trigger still stated in "For the engineer" above, not glossed over.
- **AC #4** (a client connecting as the server stops sees what it would see with no server, and starts a fresh one; real processes): `an_on_demand_server_with_no_clients_stops_itself_and_a_fresh_connect_afterward_starts_a_new_one` for the ordinary-timing case through the real, unmodified sdk; `stop_racing_a_client_connecting_still_returns_within_a_bound` (TASK-1.45, unchanged, re-cited not re-proven) for "never hangs" under the exact-instant race; the exact-instant "starts a fresh one" clause handed to a proposed TASK-1.49 criterion, per "For the engineer" above, with the residual window named precisely there.
- **The checker's own exact-wait property** (this round's required change, not itself a numbered AC but load-bearing for all four above): `the_checker_blocks_with_no_timeout_while_ineligible_and_waits_exactly_once_while_eligible`, with its own negative control.

## Files

- `crates/server/src/idle.rs` (new): `IdlePredicate`, `Wait`, `IdleSignal`, `run_checker`/`run_checker_with_observer`, `run_connection_relay`, their unit tests.
- `crates/server/src/lib.rs`: `mod idle;` (no new `pub use` - internal only).
- `crates/server/src/config.rs`: `idle_timeout: Option<Duration>` field, `DEFAULT_IDLE_TIMEOUT` const, `Debug` impl's one new field line.
- `crates/server/src/runtime.rs`: `Runtime::session_counter`, `Runtime::set_idle_notifier`, `State.notify_session_count_changed`, one new arm's worth of lines inside `apply_event_locked`'s existing `PaneEvent::Exited` match, one new line inside the existing `Runtime::stop()`.
- `crates/server/src/listen.rs`: the `IdleWiring` construction, the per-connection sender line, the relay/checker join ordering.
- `crates/server/tests/idle_stop.rs` (new).
- `crates/steg/src/main.rs`: one `if on_demand { config.idle_timeout = Some(...) }` line in `run_server_start`.
- `crates/steg/tests/fixtures/fixture_server.rs`: the same line, unconditionally, plus the `STEG_TEST_IDLE_TIMEOUT_MS` override.
- `crates/steg/Cargo.toml`: one new `[[test]]` entry (`idle_stop`, `required-features = ["fixtures"]`), same shape as `server_start`/`login_environment`.
- `crates/steg/tests/idle_stop.rs` (new).

No `Cargo.toml` dependency change, no `boundaries.toml` change (no new external dependency, no new package, all std), no `unsafe`.

## Order of work

1. `idle.rs`'s pure pieces first - `IdlePredicate`/`Wait`, their unit tests (pure, no I/O, fast feedback) - `cargo test -p steg-server idle::` before touching anything else.
2. `idle::run_checker`/`run_checker_with_observer`/`run_connection_relay` and the observer-driven loop-shape test, still with no real server involved.
3. `ServerConfig.idle_timeout`, `Runtime::session_counter`, `Runtime::set_idle_notifier` and the `apply_event_locked`/`Runtime::stop` additions (small, mechanical, but check each against PR 47's actual merged shape if it has landed by then).
4. `listen::run`'s wiring, proven in-process (`crates/server/tests/idle_stop.rs`, all six tests) before touching `crates/steg` at all.
5. The entry point and fixture changes, then the out-of-process test.
6. `just check` and the affected-crate tests (`just test`) last, recorded.

## Risks

- The three unmerged PRs this reads against (see above) - rebase risk, not a design risk, named there; `Runtime::stop()`'s own body is now one of the places I touch, which is a second, small point of friction against PR 47 specifically, beyond the field additions already named.
- AC #4's exact-instant clause, per "For the engineer" above - the one open decision this plan surfaces rather than guesses at.
- Two new always-blocking (never-polling) threads per on-demand server (the relay, the checker) - bounded, cheap, no different in kind from the drain thread `Runtime` already has; their join ordering (section 7) is the one place a future change to `listen::run`'s shutdown sequence could reintroduce a deadlock if it's not kept in mind - named here so it's checked, not just once, correctly, now.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Carried from TASK-1.77's planning (2026-11-15): when a server is stopping, a connection the accept loop takes in the instant it sees the stop flag is dropped with no reply, so the client sees connect succeed and then end of file, a third outcome beside refused and not found. The idle stop is what makes that instant reachable in ordinary use: this task's fourth criterion (a client connecting as the server stops starts a fresh one) must cover it, either by closing that window in the server or by proving the client's handling.
<!-- SECTION:NOTES:END -->
