//! Idle-stop (doc-8 R99, TASK-1.86): the predicate decides, the checker
//! thread carries it out. Kept in its own module (`.claude/rules/rust.md`:
//! behaviour lives in named modules, not `lib.rs`) and `pub(crate)`
//! throughout - nothing outside this crate needs any of it; `listen::run`
//! is the one caller.
//!
//! A server started explicitly never idle-stops (R99: "stops only when
//! told to"); one started on demand does, after 30 seconds with no
//! sessions and no clients. [`ServerConfig::idle_timeout`] is the switch:
//! `None` for an explicit start, `Some(DEFAULT_IDLE_TIMEOUT)` for an
//! on-demand one - set explicitly by `crates/steg`'s own entry point, not
//! by a default shared between the two.

use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

use crate::connection::ConnectionEvent;
use crate::listen::Stopper;

/// What a caller should do next, given the counts and the time just read -
/// never computed on a schedule of its own, only in response to a real
/// event or a wait that has just elapsed ([`run_checker`]).
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

/// A pure function of counts and a given time (R99) - no clock, no
/// counter, of its own; [`IdlePredicate::wait`] is given both every time
/// it is asked anything, and never reads either on its own
/// (`.claude/rules/testing-rust.md`: "Time... come[s] in as
/// parameters... No test sleeps to wait for something").
pub(crate) struct IdlePredicate {
    live_clients: i64, // signed: a stray disconnect with no matching connect
    // must never panic this thread on underflow; harmless if
    // it goes negative, which nothing in normal operation does
    empty_since: Option<Instant>,
}

impl IdlePredicate {
    pub(crate) fn new() -> Self {
        Self {
            live_clients: 0,
            empty_since: None,
        }
    }

    /// A connection counts as a client from the moment it is accepted,
    /// not from a completed handshake (R99's own wording) - the caller
    /// passes this through on [`ConnectionEvent::Connected`], which
    /// `connection::handle` already sends at exactly that point.
    pub(crate) fn client_connected(&mut self) {
        self.live_clients += 1;
        self.empty_since = None; // a client just arrived; not idle right now
    }

    /// `empty_since` is (re)started on the next [`IdlePredicate::wait`]
    /// call, not here, so a session that is still open at this instant
    /// correctly keeps it unset.
    pub(crate) fn client_disconnected(&mut self) {
        self.live_clients -= 1;
    }

    /// `now` and `session_count` are both given, never read by this
    /// function. Called whenever the caller wakes - an event arrived, or a
    /// previous [`Wait::Until`] elapsed - never on its own schedule.
    pub(crate) fn wait(
        &mut self,
        now: Instant,
        session_count: usize,
        idle_timeout: Duration,
    ) -> Wait {
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

/// Runs until it decides to stop the server (`Wait::Expired`) or the
/// channel disconnects (every sender gone - `listen::run`'s own join
/// bound is derived against exactly that, not a poll interval). The one
/// production entry point - `run_checker_with_observer` with a no-op
/// observer, a zero-capture closure the compiler inlines away, not a
/// per-iteration indirection in the shipped server.
pub(crate) fn run_checker(
    rx: Receiver<ConnectionEvent>,
    stop: Stopper,
    session_count: impl Fn() -> usize,
    idle_timeout: Duration,
) {
    run_checker_with_observer(rx, stop, session_count, idle_timeout, |_wait| {});
}

/// `observe` is called once per loop iteration with the [`Wait`] just
/// computed, before acting on it - production's own [`run_checker`] passes
/// a no-op; a test passes a recording closure instead, so "how many times
/// did this block with no timeout, how many times with a bounded one, in
/// what order" is counted through a real hook, never inferred from
/// wall-clock timing. Private: an integration test under `tests/` cannot
/// reach it, so its own test lives in this module's `#[cfg(test)]` below,
/// not in a separate file.
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
            // a future addition from elsewhere) just wakes this loop to
            // re-evaluate on the next `wait()` call - the deliberate trade
            // this catch-all makes: a variant that should have updated the
            // predicate directly but doesn't get a named arm here still
            // gets picked up correctly, because `session_count()` is
            // re-read fresh on every `wait()` call regardless of which
            // event woke it. Nothing here depends on knowing every
            // variant's exact name to stay correct.
            Err(mpsc::RecvTimeoutError::Timeout) => {} // loop back - `wait()` will see
            // `Expired` next time, since at least `remaining` has now passed
            Err(mpsc::RecvTimeoutError::Disconnected) => return, // every sender into
                                                                 // `rx` is gone - `run()` has already ended
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{IdlePredicate, Wait, run_checker_with_observer};
    use crate::connection::ConnectionEvent;
    use crate::listen::Stopper;
    use std::sync::mpsc;
    use std::time::{Duration, Instant};

    fn base() -> Instant {
        Instant::now()
    }

    #[test]
    fn wait_at_29_30_and_31_seconds_of_idleness() {
        let idle_timeout = Duration::from_secs(30);
        let t0 = base();
        let mut predicate = IdlePredicate::new();
        // Establishes `empty_since = t0` - a fresh predicate's own `wait()`
        // seeds its baseline from whatever `now` it is first given, so
        // "29 seconds idle" has to be two calls: one to set the baseline,
        // one to measure against it - never one call at `t0 + 29s` alone,
        // which would measure zero elapsed from a baseline of its own.
        predicate.wait(t0, 0, idle_timeout);
        assert_eq!(
            predicate.wait(t0 + Duration::from_secs(29), 0, idle_timeout),
            Wait::Until(Duration::from_secs(1))
        );
        assert_eq!(
            predicate.wait(t0 + Duration::from_secs(30), 0, idle_timeout),
            Wait::Expired
        );
        assert_eq!(
            predicate.wait(t0 + Duration::from_secs(31), 0, idle_timeout),
            Wait::Expired
        );
    }

    #[test]
    fn a_connected_client_with_no_matching_disconnect_never_fires_no_matter_how_far_time_moves() {
        let idle_timeout = Duration::from_secs(30);
        let t0 = base();
        let mut predicate = IdlePredicate::new();
        predicate.client_connected();
        assert_eq!(
            predicate.wait(t0 + Duration::from_secs(10_000), 0, idle_timeout),
            Wait::Forever
        );
    }

    #[test]
    fn a_nonzero_session_count_never_fires_regardless_of_clients() {
        let idle_timeout = Duration::from_secs(30);
        let t0 = base();
        let mut predicate = IdlePredicate::new();
        assert_eq!(
            predicate.wait(t0 + Duration::from_secs(10_000), 1, idle_timeout),
            Wait::Forever
        );
    }

    #[test]
    fn a_connect_then_disconnect_within_one_tick_restarts_the_full_period() {
        let idle_timeout = Duration::from_secs(30);
        let t0 = base();
        let mut predicate = IdlePredicate::new();
        predicate.client_connected();
        predicate.client_disconnected();
        assert_eq!(
            predicate.wait(t0 + Duration::from_millis(1), 0, idle_timeout),
            Wait::Until(idle_timeout),
            "the clock restarts from this `wait()` call, a full period - not a \
             partial remainder from when the client connected"
        );
    }

    #[test]
    fn a_client_reconnecting_every_29_seconds_never_fires() {
        let idle_timeout = Duration::from_secs(30);
        let t0 = base();
        let mut predicate = IdlePredicate::new();
        for n in 0..5u32 {
            let connect_at = t0 + Duration::from_secs(29 * u64::from(n));
            predicate.client_connected();
            let disconnect_at = connect_at + Duration::from_secs(1);
            predicate.client_disconnected();
            // Right after each disconnect, a fresh full period - never
            // cumulative, and the next connect (29s later) always arrives
            // comfortably inside it.
            assert_eq!(
                predicate.wait(disconnect_at, 0, idle_timeout),
                Wait::Until(idle_timeout)
            );
        }
    }

    #[test]
    fn empty_from_construction_and_waiting_past_idle_timeout_expires() {
        let idle_timeout = Duration::from_secs(30);
        let t0 = base();
        let mut predicate = IdlePredicate::new();
        assert_eq!(
            predicate.wait(t0, 0, idle_timeout),
            Wait::Until(idle_timeout),
            "an untouched predicate (a just-started, genuinely idle server) \
             starts its idle clock from the first call, not from some later event"
        );
        assert_eq!(
            predicate.wait(t0 + idle_timeout, 0, idle_timeout),
            Wait::Expired
        );
    }

    #[test]
    fn wait_until_is_exact_not_approximate() {
        let idle_timeout = Duration::from_secs(30);
        let t0 = base();
        let mut predicate = IdlePredicate::new();
        predicate.wait(t0, 0, idle_timeout); // establishes the baseline, see above
        assert_eq!(
            predicate.wait(t0 + Duration::from_secs(5), 0, idle_timeout),
            Wait::Until(Duration::from_secs(25))
        );
        assert_eq!(
            predicate.wait(t0 + Duration::from_secs(17), 0, idle_timeout),
            Wait::Until(Duration::from_secs(13))
        );
    }

    /// Drives [`run_checker_with_observer`] directly - no thread, no real
    /// server, no process - synchronised entirely through the recording
    /// channel itself (never a sleep): the feeder only sends its next
    /// event once it has actually seen the observation that should
    /// precede it.
    #[test]
    fn the_checker_blocks_with_no_timeout_while_ineligible_and_waits_exactly_once_while_eligible() {
        let idle_timeout = Duration::from_millis(50);
        let (signal_tx, signal_rx) = mpsc::channel::<ConnectionEvent>();
        let (observed_tx, observed_rx) = mpsc::channel::<Wait>();
        let stop = Stopper::new();

        let checker = std::thread::spawn({
            let stop = stop.clone();
            move || {
                run_checker_with_observer(
                    signal_rx,
                    stop,
                    || 0,
                    idle_timeout,
                    move |wait| {
                        let _ = observed_tx.send(*wait);
                    },
                );
            }
        });

        // First observation: eligible from construction (no clients, no
        // sessions) - `Wait::Until(idle_timeout)`.
        assert_eq!(
            observed_rx.recv().expect("the first observation"),
            Wait::Until(idle_timeout)
        );
        // Send `Connected` now that the checker is confirmed blocked on
        // its own bounded `recv_timeout` - it will wake, update, and loop
        // back to `Wait::Forever`.
        signal_tx
            .send(ConnectionEvent::Connected)
            .expect("the checker is still running");
        assert_eq!(
            observed_rx.recv().expect("the second observation"),
            Wait::Forever
        );
        // Now disconnect - the checker wakes from its untimed `recv()`,
        // updates, and loops back to a *fresh* full `Wait::Until`.
        signal_tx
            .send(ConnectionEvent::Disconnected)
            .expect("the checker is still running");
        assert_eq!(
            observed_rx.recv().expect("the third observation"),
            Wait::Until(idle_timeout)
        );
        // No further event: the bounded wait above times out, `wait()` is
        // called a fourth time and returns `Expired`, the checker calls
        // `stop.stop()` and returns.
        assert_eq!(
            observed_rx.recv().expect("the fourth observation"),
            Wait::Expired
        );
        assert!(
            observed_rx.recv().is_err(),
            "exactly four observations for this script - a fifth would mean an \
             extra, unexplained wake-up"
        );

        checker.join().expect("the checker thread should not panic");
    }

    /// TASK-1.98's own addition to this shared channel (section 5 of this
    /// task's plan, "coordination") - an accept retried after a
    /// recoverable error is not a client arriving, and this catch-all's
    /// own reasoning (section 5) says it should be treated the same as
    /// any other event it does not specifically care about: wake the
    /// loop, update nothing on the predicate, let `session_count()` be
    /// re-read fresh on the next `wait()`. Checked directly, not assumed.
    #[test]
    fn a_recoverable_accept_retry_does_not_count_as_a_client_arriving() {
        let idle_timeout = Duration::from_millis(50);
        let (signal_tx, signal_rx) = mpsc::channel::<ConnectionEvent>();
        let (observed_tx, observed_rx) = mpsc::channel::<Wait>();
        let stop = Stopper::new();

        let checker = std::thread::spawn({
            let stop = stop.clone();
            move || {
                run_checker_with_observer(signal_rx, stop, || 0, idle_timeout, move |wait| {
                    let _ = observed_tx.send(*wait);
                });
            }
        });

        assert_eq!(
            observed_rx.recv().expect("the first observation"),
            Wait::Until(idle_timeout)
        );
        signal_tx
            .send(ConnectionEvent::AcceptRetried(
                crate::listen::RecoverableAcceptError::DescriptorsExhausted,
            ))
            .expect("the checker is still running");
        // Woken, but still eligible, with the *same* clock still running -
        // the catch-all this event falls into (`run_checker_with_observer`'s
        // own `Ok(_) => {}` arm) updates nothing on the predicate at all, so
        // this is `Wait::Until` with a little *less* remaining than before
        // (time has passed, nothing reset it), never `Wait::Forever` (what a
        // real client arriving would have produced instead, test 2's own
        // shape) and never a fresh full period either (what `Connected`
        // then `Disconnected` produces, test 4's own shape) - a retry is
        // neither.
        match observed_rx.recv().expect("the second observation") {
            Wait::Until(remaining) => assert!(
                remaining < idle_timeout,
                "the clock should have kept running, not reset: {remaining:?} should be \
                 less than {idle_timeout:?}"
            ),
            other => panic!("expected Wait::Until with some time still left, got {other:?}"),
        }

        drop(signal_tx);
        checker.join().expect("the checker thread should not panic");
    }
}
