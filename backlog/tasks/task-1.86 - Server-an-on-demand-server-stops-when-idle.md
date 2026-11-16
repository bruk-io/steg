---
id: TASK-1.86
title: 'Server: an on-demand server stops when idle'
status: In Progress
assignee:
  - '@bruk.w3'
created_date: '2026-11-15T23:40:00Z'
updated_date: '2026-11-16T08:43:00Z'
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
Read against doc-8 R99, doc-2 section 6/8, doc-16 (R101, R110), the three pull requests not yet on main (47/3e4594e, 56/040c5f0, 57/93973fd - this plan is written against their heads, not `origin/main`), and TASK-1.77's own carried design (its Implementation Notes, "For TASK-1.86"). No file is edited in this phase.

## For the engineer

**1. AC #4's "starts a fresh one" cannot be shown for the exact-instant race, because no production client exists yet to retry on it - recommend accepting a narrower reading.** TASK-1.77's notes gave this task two doors: close the window in the server, or prove the client's handling. I read both against the actual code (040c5f0's `listen::run`, `crates/sdk/src/local_socket.rs`), not assumed:

- *Closing the window further isn't available.* The listener is already dropped on the same thread, synchronously, immediately after the accept loop breaks - before any connection is joined (the ordering TASK-1.44/45's own reviews already settled, `listen.rs`'s own comment names why). The only remaining race is a client's `connect()` syscall completing into the kernel's backlog a few instructions of Rust before the stop flag is read back on the same iteration, plus whatever the kernel does with an already-queued connection underneath that - narrow, with no mechanism on either side to widen it or to observe it from a test (I'm not claiming a duration I have no way to measure), and not something I can close further without replacing blocking `accept()` with an evented/non-blocking loop, which is a much bigger change this task's three criteria don't ask for.
- *Proving the client's handling isn't available either, honestly.* `local_socket::connect` returns `Ok(stream)` the instant the OS-level `connect()` succeeds - it never reads a byte, so it cannot distinguish "healthy" from "accepted-then-dropped." The code that *would* notice (attempting a handshake, treating a bare EOF the same as refused) only runs in whatever drives `steg_sdk::Connection::new(Role::Client, ...)` for real, which has no production caller yet (TASK-1.49's CLI isn't built). Writing that retry logic only inside a test, to then assert it works, tests the test's own logic, not shipped behaviour - I'm not doing that.
- *What the ordinary case already gives, with zero new client code.* The overwhelming majority of "the instant it is stopping" - anything from the moment `stop()` is called up to the very edge of that one-iteration race - already gets `ECONNREFUSED` or, once the socket file is later removed, `ENOENT`. `local_socket::connect` already treats *any* `Err` from its own `UnixStream::connect` as "no server, autostart" - this needs no change. I prove this end to end, with real processes: an idle server stops itself, a client connecting afterward is refused, and a fresh on-demand start at the same path succeeds.
- *Recommendation:* accept AC #4 as proven by (a) that ordinary-timing real-process test, (b) the existing `stop_racing_a_client_connecting_still_returns_within_a_bound` (TASK-1.45, unchanged) as proof `run()` never hangs when a connect races `stop()` - the same `Stopper::stop()` function regardless of who calls it, so this task adds no new risk there - and (c) a note, carried forward the same way TASK-1.77 carried this one to me, handing "retry on an accepted-then-EOF outcome" to whichever task first builds a real handshake-driving client. If you want the exact-instant case proven rather than deferred, that's a different task (building a minimal real client ahead of TASK-1.49) - say so and I'll replan.

**2. AC #3's "ending its programs" is vacuous for idle-stop's *own* trigger in production, by construction - one test below deliberately builds the non-vacuous case anyway, to prove the sequence rather than assert it.** The predicate only calls `stop()` when `session_count == 0` (R99: "no sessions and no clients") - in normal operation idle-stop can never fire while any session, let alone a running program, exists (a session only disappears from the tree when explicitly closed, not merely because its one pane's program exited - doc-2: "a pane started with a program stays when the program ends"). `Runtime::stop()` ending real programs is already proven elsewhere (TASK-1.76/1.78's own tests) - this task doesn't need to re-prove that mechanism, only that this trigger reaches it. "A short injected grace period" (AC #3's wording) is satisfied by injecting a short `idle_timeout`, not `force_grace`. I did **not** touch `listen::run`'s hardcoded `crate::runtime::FORCE_GRACE_DEFAULT` (a line PR 47/TASK-1.78 just wrote) - no need to, for the reason above; a different test below (`a_session_that_exists_prevents_the_idle_stop`) does construct a real session, but sidesteps `force_grace` entirely by injecting a fake `ProgramLauncher` instead of touching that line (section 6).

**3. Not splitting.** I checked for a seam the way the standing default asks. Every piece here (the predicate, the checker thread, the `ServerConfig`/entry-point wiring) exists only to serve one behaviour (R99's idle stop) and none is independently reviewable or valuable alone - unlike TASK-1.77's three unrelated concerns. One task, one PR.

## Why this builds on three unmerged PRs, and the risk that carries

Read against PR 47 (3e4594e, TASK-1.78: `Runtime::new`'s third `force_grace` argument, `Runtime::stop` ending every pane within a bound), PR 56 (040c5f0, TASK-1.87: `ServerConfig.login`/`login_identity_timeout`/`login_environment_timeout`, the `fixtures`-gated `fixture_server` binary), and PR 57 (93973fd, TASK-1.89: a merge commit, not itself a feature I depend on for this task - `listen::run`'s shape I'm building against is 040c5f0's, which already contains PR 47 merged in). None of these are on `origin/main` yet. **Risk:** if any lands with a shape different from what's quoted below (especially `listen::run`'s body, or `ServerConfig`'s field list), my branch needs a rebase before `record-check` - I'll note the exact diff in this task's notes if that happens, not reopen this plan.

## 1. `IdlePredicate`: a pure function of counts and a given time

New module `crates/server/src/idle.rs` (not `listen.rs` or `lib.rs` - `.claude/rules/rust.md`: behaviour in named modules). Not exported from `lib.rs` (`pub(crate)` only) - nothing outside this crate needs it, and R101/R110 keep the sdk as the one public interface; `listen.rs` is the only caller.

```rust
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
                // empty_since is (re)started on the next tick(), not here.
            }
        }
    }

    /// `now` and `session_count` are both given, never read by this function.
    pub(crate) fn tick(&mut self, now: Instant, session_count: usize, idle_timeout: Duration) -> bool {
        if self.live_clients > 0 || session_count > 0 {
            self.empty_since = None;
            return false;
        }
        let since = *self.empty_since.get_or_insert(now);
        now.duration_since(since) >= idle_timeout
    }
}
```

**Correcting TASK-1.77's own carried claim about the arbiter probe, checked rather than repeated.** Its notes said a probe (`Connected` then an almost-immediate `Disconnected`, no hello) "costs at most a few seconds," resting on `get_or_insert`. Tracing it: `on_event(Connected)` sets `empty_since = None` unconditionally; `on_event(Disconnected)` does not set it back. So the *next* `tick()` call's `get_or_insert(now)` seeds it from whatever `now` happens to be at that call, not from any earlier value - the clock restarts from zero, not "close to where it was." **A probe costs a full idle period, not a few seconds.** Still compatible with AC #2 ("does not *by itself* keep an idle server alive" - one probe delays once, it doesn't hold indefinitely), but the plan and the unit test state the real cost, not the inherited one.

Unit tests (`#[cfg(test)]` inline, literal `Instant`s built from one fixed base plus `Duration` offsets, no sleeping):

1. `tick_at_29_30_and_31_seconds_of_idleness` - three literal cases on one otherwise-empty predicate: 29s → `false`, 30s → `true`, 31s → `true` (AC #1's own boundary wording).
2. `a_connected_with_no_matching_disconnected_never_fires_no_matter_how_far_time_moves` - `on_event(Connected, t0)`, then `tick` at `t0 + 10_000s` → `false` (AC #2: mid-handshake counts from acceptance).
3. `a_nonzero_session_count_never_fires_regardless_of_clients` - `tick(far_future, 1, idle_timeout)` → `false` even with `live_clients == 0`.
4. `a_connect_then_disconnect_within_one_tick_does_not_fire_early` - the arbiter-probe shape: `on_event(Connected, t0)`, `on_event(Disconnected, t0 + 1ms)`, `tick(t0 + 1ms + idle_timeout - 1ms, 0, idle_timeout)` → `false` (still short of a full period from the point the clock actually restarted), `tick(t0 + 1ms + idle_timeout, 0, idle_timeout)` → `true`. Named for what it actually proves (restarts from the tick, not the event) rather than the "few seconds" framing.
5. `a_client_reconnecting_every_29_seconds_never_fires` - five literal cycles of `Connected` at `t0 + 29*n`s, `Disconnected` at `t0 + 29*n + 1`s (modelling the handshake timeout closing it before the next arrives), with a `tick` between each pair never returning `true` - "stated and tested," AC #2's own open question, with no real time elapsed.
6. `empty_from_construction_and_ticking_past_idle_timeout_returns_true` - the base case the sketch named first.

**AC #2's arbiter-probe clause, through the real arbiter, needs no separate wiring-level test.** `arbiter::probe` (040c5f0's `arbiter.rs:177`) only runs on `ExistingFile::OursAndASocket` - a *contended* start, where a file already exists and another process checks whether it's a live one of ours - never on a solo start with nothing there yet. So a real probe is reachable only in the same multi-process race TASK-1.77's own `four_simultaneous_starts_converge_on_one_server` already exercises, not in any test this task adds. What a probe looks like to `run`'s own accept loop (`Connected` then an immediate `Disconnected`, no hello) is, by `connection.rs`'s own documented caveat, indistinguishable from *any* short-lived client that connects and leaves before a hello - the server has no signal to tell them apart, so there is nothing a probe-specific test could exercise that test 4 above (the general "connect then disconnect within one tick" shape) does not already cover identically. Named here so a reviewer checking this clause finds the reasoning rather than a gap.

No negative control for these: pure-function unit tests of logic with no mechanism behind them to disable (team testing skill: "an ordinary unit test of a pure function does not need one").

## 2. `ServerConfig.idle_timeout: Option<Duration>` - the on/off switch R99 needs

`None` means never idle-stop (an explicit start's default, unchanged); `Some(duration)` is the period. Added to `crates/server/src/config.rs` beside the existing injected-time fields, same pattern as `login_identity_timeout`:

```rust
pub const DEFAULT_IDLE_TIMEOUT: Duration = Duration::from_secs(30); // R99
```
```rust
pub idle_timeout: Option<Duration>,
```
`ServerConfig::default()` sets it to `None` - **not** `Some(DEFAULT_IDLE_TIMEOUT)`. `run_server_start`'s own `ServerConfig::default()` call is shared between an explicit and an on-demand start (section 5 of TASK-1.77's plan); defaulting to `Some` here would make an *explicit* start idle-stop too, which R99 forbids outright ("a server started explicitly stops only when told to"). The `on_demand` branch is what turns it on (section 4 below) - a one-line, explicit `Some`, not a default, the same reasoning TASK-1.87 gave for writing `config.login = ...` explicitly rather than leaving it to a default that happens to match.

`Debug for ServerConfig`'s manual impl gets one more `.field(...)` line (it's a plain `Option<Duration>`, no special handling needed, unlike `launcher`).

## 3. `Runtime::session_counter` - the seam into a non-`Arc` `Runtime`

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

## 4. Wiring `listen::run`: one internal channel, built only when idle-checking is on

No change to `connection::handle`'s signature. `connection::handle` already takes `events: Option<Sender<ConnectionEvent>>` and sends to whichever it's given; I don't touch that file. Instead, `run()` decides *which* sender to hand each connection:

```rust
// crates/server/src/listen.rs, inside run(), before `thread::scope`:
let (idle_tx, checker_handle) = match config.idle_timeout {
    Some(idle_timeout) => {
        let (tx, rx) = mpsc::channel();
        let stop = stop.clone();
        let session_count = runtime.session_counter();
        let forward_to = events.clone(); // may itself be None; unrelated test observer
        let handle = thread::Builder::new()
            .name("steg-idle-checker".to_string())
            .spawn(move || idle::run_checker(rx, stop, session_count, idle_timeout, forward_to))
            .expect("spawning the idle checker thread");
        (Some(tx), Some(handle))
    }
    None => (None, None),
};
```
Everything the `Some` arm needs - the `Duration`, the freshly built `Sender`/`Receiver` pair, the spawned handle - is produced and consumed inside that one arm, so there is no second place that has to agree `idle_tx` and `checker_handle` are both `Some` or both `None`; no invariant to restate with an `.expect()`. Inside the loop, the per-connection sender becomes `idle_tx.clone().or_else(|| events.clone())` in place of today's bare `events.clone()` - when idle-checking is off (`idle_tx` is `None`), this is `events.clone()`, byte-for-byte what runs today; no behaviour change for an explicit start or any existing test (none sets `idle_timeout`, so it stays `None`).

`idle_tx` (like `events` already does today) is captured by the `move |scope| { ... }` closure and never referenced again after it returns - its last clone (inside that closure) drops when `thread::scope` returns, the same implicit drop `events` already relies on. Combined with every per-connection clone already joined by then, the channel is fully disconnected the instant `thread::scope(...)` returns - before `runtime.stop()` runs, below.

`idle::run_checker` (new, in `idle.rs` beside the predicate):

```rust
pub(crate) fn run_checker(
    rx: Receiver<ConnectionEvent>,
    stop: Stopper,
    session_count: impl Fn() -> usize,
    idle_timeout: Duration,
    forward_to: Option<Sender<ConnectionEvent>>,
) {
    const POLL_INTERVAL: Duration = Duration::from_millis(100); // an implementation
        // detail, not test-configurable: small enough to notice an idle
        // `idle_timeout` of a few hundred ms promptly, cheap enough that
        // 30s of production polling (300 wakeups) costs nothing measurable.
    let mut predicate = IdlePredicate::new();
    loop {
        match rx.recv_timeout(POLL_INTERVAL) {
            Ok(event) => {
                if let Some(forward_to) = &forward_to {
                    let _ = forward_to.send(event);
                }
                predicate.on_event(event, Instant::now());
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => return, // run() has already
                // ended the accept loop and dropped every sender - nothing left to
                // check, whether or not this thread's own tick ever decided to stop
        }
        if predicate.tick(Instant::now(), session_count(), idle_timeout) {
            stop.stop();
            return; // this call already decided; no point looping further
        }
    }
}
```
No new public method on `Stopper` (its public surface was deliberately narrowed by a prior review - "there is now one public way to stop, and it always works" - adding a query method back would cut against that). Termination is guaranteed without one: once `stop()` is called by *any* caller (this checker or a later `server stop`), the accept loop's own existing logic ends it, `idle_tx`'s last clone drops, and this loop's `recv_timeout` sees `Disconnected` within one `POLL_INTERVAL` at the very most.

After `thread::scope(...)` returns (all connections already joined, listener already dropped) and before `runtime.stop()`:
```rust
if let Some(handle) = checker_handle {
    let _ = handle.join(); // bounded by construction: the channel this thread polls
        // is already disconnected by this point (see above), so `recv_timeout` returns
        // within one `POLL_INTERVAL` regardless of which path ended `run` - the same
        // "hard guarantee, not a real wait" reasoning `Runtime::stop`'s own drain-thread
        // join already uses.
}
```

**Exact lines this touches in 040c5f0's `listen.rs`, for TASK-1.98's ordering (same accept loop, different lines):** the `let (stream, _addr) = match listener.accept() ...` arm, `reap_finished(&mut connections)`, and the `if stop.should_stop() { drop(stream); break Ok(()); }` block are **untouched** by this task - those are TASK-1.98's. This task's only line inside the loop is the one computing the per-connection sender (today's bare `let events = events.clone();`, becoming the `idle_tx.clone().or_else(...)` line above); everything else is before the loop (constructing `idle_tx`/`checker_handle`) or after `thread::scope` returns (joining `checker_handle`, before `runtime.stop()`).

## 5. The entry point: on-demand turns idle-stop on, explicitly

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

## 6. Tests

**`crates/server/tests/idle_stop.rs` (new, in-process, real sockets and threads - no process spawn needed at this layer):** reuses `shutdown.rs`'s own `start_server`/`wait_until_accepting`/`join_within` shape (same crate, same file conventions) with `config.idle_timeout = Some(short)`.

- `a_genuinely_idle_server_stops_itself_within_a_bound`: `start_truly_idle_server`-style setup (drains the startup probe first, same reason `shutdown.rs` already gives), no client ever connects after that, `join_within(server_thread, idle_timeout + margin)`. **Negative control:** with `idle_timeout` set to `None` instead (scratch edit), the same server does *not* stop within the bound - recorded, reverted.
- `a_session_that_exists_prevents_the_idle_stop`: start a server with `config.launcher` overridden to a fake, in-memory `ProgramLauncher` (`runtime.rs`'s own tests already build one - reused, not reinvented) so `session.new` mints a real tree session with no real pty and no real program behind it - sidestepping `FORCE_GRACE_DEFAULT`/`ESCALATE_CONFIRM_TIMEOUT`/`FORCE_JOIN_TIMEOUT` entirely (several real seconds of teardown this task has no reason to inject a grace period to shorten, per "For the engineer" above), rather than widening the test's bound to cover them. Create the session over the wire (reusing `shutdown.rs`'s own handshake-completion helper plus a `session.new` call), confirm `run()` does **not** return within `idle_timeout + margin` even with the connection then closed. **Negative control:** same scenario with the session closed first (`session.close`) - now it *does* stop within the bound, proving the session, not the connection, is what was holding it open.
- `a_client_reconnecting_inside_the_handshake_timeout_keeps_the_server_up`: real repeated connects (no session), each closed before completing a hello, spaced inside `idle_timeout`; asserts `run()` is still alive past what would otherwise be the idle deadline. This is the one case in this file proving the wiring-level version of unit test 5 above, through real sockets rather than the pure predicate alone - distinct things, so both exist (team testing skill: "most logic should be pure... test the decision, then the behaviour through its entry point").
- `an_explicit_starts_runtime_never_idle_stops`: `serve`/`run` with `config.idle_timeout: None` (today's default, unchanged) and no client ever connecting; asserts `run()` has **not** returned after a short fixed bound (not tied to any `idle_timeout`, since there is none here - a plain "still alive after a few hundred ms" check, the same shape `explicit_start_stays_up_with_no_client_ever_connecting` (TASK-1.77) already uses at the real-process layer, written again here at the in-process layer because that is where `idle_timeout` itself lives). **Negative control:** the same setup with `idle_timeout: Some(short)` instead - now it does stop within the bound, proving this test would have caught the regression it's guarding against.

**`crates/steg/tests/idle_stop.rs` (new, `fixtures`-gated, real process - AC #1/#3's own "shown end to end with a real server process"):**

- `an_on_demand_server_with_no_clients_stops_itself_and_a_fresh_connect_afterward_starts_a_new_one`: spawns `fixture_server` with `STEG_TEST_IDLE_TIMEOUT_MS` set short plus the existing required `STEG_TEST_LOGIN_*` vars, waits for the socket to accept, confirms no client ever connects, bounded-polls (`try_wait`, never a bare `wait()`) for the process to exit on its own within `idle_timeout + margin`, asserts exit code 0. Then: a fresh `UnixStream::connect` to the same path fails (refused or not found - either is correct, same framing `stop_racing_a_client_connecting_still_returns_within_a_bound` already uses for this exact ambiguity), and spawning `fixture_server` again at the same path succeeds and prints "started a new server" (AC #4's ordinary-timing case, proven with real processes, zero new client code - the sdk's existing autostart already does this). **Negative control:** drop the `STEG_TEST_IDLE_TIMEOUT_MS`-reading block from `fixture_server.rs` in a scratch edit (so `config.idle_timeout` stays `None` even in the fixture) - the spawned process never exits, the bounded `try_wait` poll times out, the test fails loudly rather than hanging; recorded, reverted.

`an_explicit_start_never_idle_stops` is **not** a new real-process test here: a server-side override is awkward to add since an explicit start never touches `idle_timeout` by design, and `crates/server/tests/idle_stop.rs`'s own `an_explicit_starts_runtime_never_idle_stops` (above) already proves the same clause with its own negative control, at the layer where `idle_timeout` actually lives. TASK-1.77's existing `explicit_start_stays_up_with_no_client_ever_connecting` continues to pass unmodified at the real-process layer, which is what "the explicit path is unaffected" needs there - not a second, slower copy of the same proof.

**Processes and threads, named per the team's own policy:** every `Child` this task's own new tests spawn is wrapped in TASK-1.77's existing `KillOnDrop` (reused, not reinvented); the idle checker thread is joined on every path out of `run()` (section 4); no test sleeps for the idle timeout to elapse - `join_within`/`try_wait`/`recv_timeout` bound every wait, and the *only* literal `Instant::now()`-driven real time in this whole task is the checker's own production polling and the two out-of-process tests' injected short `idle_timeout` - never a test computing its own deadline by sleeping past one.

## Proof, by acceptance criterion

- **AC #1** (a server started on demand stops itself after 30s with no sessions/clients; one with a session, or started explicitly, never does; boundary at 29/30/31s with literal values): unit tests 1, 3, 6 above (the predicate itself) plus `a_genuinely_idle_server_stops_itself_within_a_bound`, `a_session_that_exists_prevents_the_idle_stop` and `an_explicit_starts_runtime_never_idle_stops` (the wiring, each with its own negative control) plus `an_on_demand_server_with_no_clients_stops_itself...` (a real process, with its own negative control).
- **AC #2** (counts from acceptance, not handshake completion; the arbiter probe does not by itself keep it alive; the 29-second reconnector is stated and tested): unit tests 2, 4 (corrected), 5 - `a_connected_with_no_matching_disconnected_never_fires...`, `a_connect_then_disconnect_within_one_tick_does_not_fire_early`, `a_client_reconnecting_every_29_seconds_never_fires` - plus `a_client_reconnecting_inside_the_handshake_timeout_keeps_the_server_up` through real sockets.
- **AC #3** (session count from `Runtime`; stop via the one `Stopper::stop` function, then the `Runtime`; shown end to end with real process, short injected grace, bounded waits): section 3's `session_counter` (reads `Tree::sessions().len()` through `Runtime`'s own state, nothing duplicated), section 4's `stop.stop()` call (the same public function every other caller uses), `an_on_demand_server_with_no_clients_stops_itself...` for the real-process, bounded-wait proof - with the "ending its programs" clause read as vacuous for this trigger and said so in "For the engineer" above, not glossed over.
- **AC #4** (a client connecting as the server stops sees what it would see with no server, and starts a fresh one; real processes): `an_on_demand_server_with_no_clients_stops_itself_and_a_fresh_connect_afterward_starts_a_new_one` for the ordinary-timing case through the real, unmodified sdk; `stop_racing_a_client_connecting_still_returns_within_a_bound` (TASK-1.45, unchanged, re-cited not re-proven) for "never hangs" under the exact-instant race; the exact-instant "starts a fresh one" clause handed forward in "For the engineer" above, with my recommendation.

## Files

- `crates/server/src/idle.rs` (new): `IdlePredicate`, `run_checker`, their unit tests.
- `crates/server/src/lib.rs`: `mod idle;` (no new `pub use` - internal only).
- `crates/server/src/config.rs`: `idle_timeout: Option<Duration>` field, `DEFAULT_IDLE_TIMEOUT` const, `Debug` impl's one new field line.
- `crates/server/src/runtime.rs`: `Runtime::session_counter`.
- `crates/server/src/listen.rs`: the conditional internal channel, checker spawn/join, per-connection sender line.
- `crates/server/tests/idle_stop.rs` (new).
- `crates/steg/src/main.rs`: one `if on_demand { config.idle_timeout = Some(...) }` line in `run_server_start`.
- `crates/steg/tests/fixtures/fixture_server.rs`: the same line, unconditionally, plus the `STEG_TEST_IDLE_TIMEOUT_MS` override.
- `crates/steg/Cargo.toml`: one new `[[test]]` entry (`idle_stop`, `required-features = ["fixtures"]`), same shape as `server_start`/`login_environment`.
- `crates/steg/tests/idle_stop.rs` (new).

No `Cargo.toml` dependency change, no `boundaries.toml` change (no new external dependency, no new package, all std), no `unsafe`.

## Order of work

1. `idle.rs` and its unit tests first (pure, no I/O, fast feedback) - `cargo test -p steg-server idle::` before touching anything else.
2. `ServerConfig.idle_timeout` and `Runtime::session_counter` (small, mechanical).
3. `listen::run`'s wiring, proven first in-process (`crates/server/tests/idle_stop.rs`) before touching `crates/steg` at all.
4. The entry point and fixture changes, then the out-of-process tests.
5. `just check` and the affected-crate tests (`just test`) last, recorded.

## Risks

- The three unmerged PRs this reads against (see above) - rebase risk, not a design risk, named there.
- `POLL_INTERVAL`'s cost at scale: not a risk in practice (one thread, one 100ms wakeup, for as long as a server is on-demand and idle-eligible - bounded, cheap, no different in kind from the drain thread already in `Runtime`).
- AC #4's exact-instant clause, per "For the engineer" above - the one open decision this plan surfaces rather than guesses at.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Carried from TASK-1.77's planning (2026-11-15): when a server is stopping, a connection the accept loop takes in the instant it sees the stop flag is dropped with no reply, so the client sees connect succeed and then end of file, a third outcome beside refused and not found. The idle stop is what makes that instant reachable in ordinary use: this task's fourth criterion (a client connecting as the server stops starts a fresh one) must cover it, either by closing that window in the server or by proving the client's handling.
<!-- SECTION:NOTES:END -->
