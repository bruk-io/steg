//! Events: broadcasting a `TreeEvent` to subscribers, and applying one
//! real [`PaneEvent`] to the tree (TASK-1.131 seam, not named in the
//! task's own list - the one real cross-cutting concern left once the
//! others are split out).

use std::time::Instant;

use steg_kernel::{CommandEnded, CommandOutputStarted, ExitOutcome, Moment};
use steg_sdk::{Connection, PaneId, SessionId};

use super::State;
use super::tree::session_owning_tab;
use crate::connection::ConnectionEvent;
use crate::dispatch::{self, Context};
use crate::pane::PaneEvent;

/// Emits `pane.status_changed` (§5 of this task's own plan): the one
/// event site for every way a typed command's status actually changes
/// (its own callers already decide *whether* anything changed - a no-op
/// mark is never passed here at all). Silently does nothing if the pane
/// cannot be scoped to a session (the pane is already gone from the tree,
/// §5's own documented race) - there is no subscriber whose filter could
/// meaningfully match it at that point anyway.
fn emit_pane_status_changed(state: &State, pane: PaneId, status: Option<steg_sdk::Status>) {
    let Some(scope) = state
        .tree
        .tab_of_pane(pane)
        .map(|tab| session_owning_tab(&state.tree, tab))
    else {
        return;
    };
    broadcast(
        state,
        &steg_sdk::TreeEvent::PaneStatusChanged { pane, status },
        scope,
    );
}

/// `tab.focus`/`pane.focus` (§5 of this task's own plan): emits the whole
/// path (decision-5: "one event reports the whole path, so no client ever
/// sees the two halves apart"), re-derived from the tree *after* the
/// mutation rather than threading the kernel's own `Focused`/`FocusMove`
/// back out of `dispatch::tab::focus`/`pane::focus` (both still return
/// `Outcome::Nothing`, unchanged) - correct regardless of which of the
/// two actually moved anything, or which neighbour `pane.focus
/// --direction` landed on: re-resolving the *original* target after the
/// fact still finds the same tab (focusing never moves a pane to another
/// tab), and that tab's own `current()` is whichever pane the operation
/// actually left focused, direction or not. Every other successful
/// method is a no-op here (`None` match arm) - nothing else changes focus.
///
/// **`session.focused` is never emitted here, named as a gap rather than
/// guessed at**: decision-5 gives a session no "current" of its own at the
/// tree level at all ("each session has a current tab, and each tab has a
/// current pane" - focus is a path *down* the tree; a session is the
/// path's root, not a level something else makes current within it).
/// Nothing in this task's own criteria or in decision-5 names what tree
/// mutation would produce it; `session.focus` (R87) is a different verb
/// entirely, addressed to a client, not a tree change, and owned by
/// TASK-1.109 under its own event name (`session.focus_requested`).
pub(super) fn emit_focus_event(state: &State, context: &Context, request: &steg_sdk::Request) {
    use steg_sdk::TreeEvent;

    let tab_id = match request.method.as_str() {
        "tab.focus" => dispatch::target_tab(&state.tree, context, request).ok(),
        "pane.focus" => dispatch::target_pane(&state.tree, context, request)
            .ok()
            .and_then(|pane| state.tree.tab_of_pane(pane)),
        _ => None,
    };
    let Some(tab_id) = tab_id else {
        return;
    };
    let session_id = session_owning_tab(&state.tree, tab_id);
    let Some(pane_id) = state
        .tree
        .session(session_id)
        .and_then(|session| session.tab(tab_id))
        .map(steg_kernel::Tab::current)
    else {
        return;
    };

    let event = match request.method.as_str() {
        "tab.focus" => TreeEvent::TabFocused {
            session: session_id,
            tab: tab_id,
            pane: pane_id,
        },
        "pane.focus" => TreeEvent::PaneFocused {
            session: session_id,
            tab: tab_id,
            pane: pane_id,
        },
        _ => unreachable!("already matched above"),
    };
    broadcast(state, &event, session_id);
}

/// Records that `apply_event_locked` has applied one of `pane`'s own
/// `CommandOutputStarted`/`CommandEnded` events (TASK-1.84 review): a
/// no-op in production, so the item-level `#[cfg(test)]` split, rather
/// than one on this call's own statement (unstable on expressions), is
/// what keeps `State::applied_marks` itself test-only.
#[cfg(test)]
fn note_mark_applied(state: &mut State, pane: PaneId) {
    *state.applied_marks.entry(pane).or_insert(0) += 1;
}

#[cfg(not(test))]
fn note_mark_applied(_state: &mut State, _pane: PaneId) {}

/// Applies one [`PaneEvent`] to the tree (§1), taking an already-locked
/// `State` - what the real drain thread calls in a loop, and what a test
/// can call directly with a hand-built [`PaneEvent`] for a deterministic,
/// synchronous proof of the cascade. No self-locking wrapper exists on
/// `Runtime` for this one (unlike `apply_size_changed`): nothing outside
/// the drain thread calls it yet, so there is nothing for one to serve
/// today; add one, the same shape as `apply_size_changed`'s, if a later
/// task needs to call this from outside an already-locked context.
pub(super) fn apply_event_locked(state: &mut State, event: PaneEvent) {
    match event {
        PaneEvent::Exited { pane, exit_code } => {
            // TASK-1.41: the one place the server's real clock becomes a
            // kernel `Moment` - `Instant::now()` against this `Runtime`'s
            // own `clock_origin`, never read by the kernel itself.
            let now = Moment::new(Instant::now().duration_since(state.clock_origin));
            // Captured *before* `record_exit` can close anything (the
            // same reasoning `close_and_dispatch` already uses, §5 of
            // this task's own plan): `pane.exited`'s own scope has to be
            // this pane's session even when the cascade closes it too, so
            // it is read from the tree while the pane is still in it,
            // not derived from `ClosedPane`'s own fields afterward (those
            // say *whether* the cascade reached the tab/session, not
            // which tab/session the pane was in when it did not).
            let scope = state
                .tree
                .tab_of_pane(pane)
                .map(|tab| session_owning_tab(&state.tree, tab));
            match state.tree.record_exit(pane, exit_code, now) {
                Ok(outcome) => {
                    if let Some(scope) = scope {
                        broadcast(
                            state,
                            &steg_sdk::TreeEvent::PaneExited { pane, exit_code },
                            scope,
                        );
                        if let ExitOutcome::Closed(closed) = &outcome {
                            super::close::emit_close_cascade(state, closed, scope);
                        }
                    }
                    if let ExitOutcome::Closed(closed) = outcome {
                        state.panes.remove(&closed.pane);
                        // TASK-1.86: this cascade (R94) can close a
                        // session with nobody connected at all - a shell
                        // pane whose program just exited, in a session
                        // no client is viewing. A request-driven close
                        // (`session.close`/`tab.close`/`pane.close`/
                        // `pane.move` over the wire, TASK-1.46) needs no
                        // notify of its own: each only ever runs on an
                        // already-accepted connection, and `connection::
                        // handle` sends `Disconnected` on every path out
                        // of itself, which already wakes the checker (and
                        // that connection's own `live_clients > 0` keeps
                        // the checker ineligible regardless of session
                        // count until it does). This path has no
                        // connection behind it at all, so it is the one
                        // case with nothing else to wake the checker if a
                        // session was all that was keeping it ineligible.
                        if closed.session.is_some()
                            && let Some(notify) = &state.notify_session_count_changed
                        {
                            let _ = notify.send(ConnectionEvent::SessionCountChanged);
                        }
                    }
                }
                Err(steg_kernel::Error::PaneNotFound(_)) => {
                    // TASK-1.78: this is now *exactly* what a forced close
                    // produces for its own grace window, every time, by
                    // design (`Runtime::close_and_dispatch` closes the
                    // tree *before* ending the program, §5) - not the
                    // "--force orphan" gap this arm was first written for.
                    // `state.panes` never has this pane's handle either by
                    // the time this runs (popped into `force_end`'s own
                    // local scope, never put back), so the `remove` below
                    // is a defensive no-op in practice, provably
                    // unreachable by any path this task's own design
                    // permits - kept rather than turned into an
                    // `unreachable!()`: a wrong "impossible" assumption
                    // panicking this live drain thread is worse than one
                    // harmless, documented, no-op branch.
                    state.panes.remove(&pane);
                }
                Err(other) => {
                    unreachable!(
                        "record_exit on a pane the tree still holds can only fail with \
                         ProgramNotStarted, and this task's own invariant (a program is \
                         always started immediately after minting, under the one lock) \
                         makes that unreachable here: {other}"
                    );
                }
            }
        }
        PaneEvent::CommandOutputStarted { pane } => {
            note_mark_applied(state, pane);
            match state.tree.note_command_output_started(pane) {
                Ok(CommandOutputStarted::Started) => {
                    emit_pane_status_changed(state, pane, Some(steg_sdk::Status::Running));
                }
                Ok(CommandOutputStarted::NotAShellPane) => {
                    // Nothing changed (the kernel's own doc: "a command
                    // pane's own program governs its status, never a
                    // mark") - R52's own correction (M1 for the events
                    // this task's own criteria reach) still means no
                    // event where nothing moved.
                }
                Err(steg_kernel::Error::PaneNotFound(_)) => {
                    // The mark is ignored - there is nothing else to do
                    // with one for a pane the tree no longer holds.
                    //
                    // Reachable by exactly one path, traced for TASK-
                    // 1.78's review: `close_and_dispatch` removes the
                    // pane from the tree *and* takes its `PaneHandle` out
                    // of `state.panes` under the same lock, before
                    // `force_end` has actually ended its program (§5) -
                    // a still-running program can emit an OSC 133 mark
                    // during that same grace window, forwarded by its
                    // relay/worker thread exactly as it would have been
                    // before the close. By the time this event reaches
                    // the control thread, the handle is already with
                    // `force_end`, not `state.panes` - this arm was never
                    // at risk of dropping its only `Sender` clone on this
                    // path, since there is no handle here to drop either
                    // way. `force_end` is what ends the program and
                    // joins its threads, on its own stated bound; this
                    // mark is simply overtaken by it. Not a gap
                    // close-first left open.
                }
                Err(other) => unreachable!(
                    "Tree::note_command_output_started documents only Error::PaneNotFound \
                     as a possible error; {other} is not that"
                ),
            }
        }
        PaneEvent::CommandEnded { pane, exit_code } => {
            note_mark_applied(state, pane);
            let now = Moment::new(Instant::now().duration_since(state.clock_origin));
            match state.tree.note_command_ended(pane, exit_code, now) {
                Ok(CommandEnded::Ended(status)) => {
                    emit_pane_status_changed(state, pane, Some(status));
                }
                Ok(CommandEnded::EndedWithNoExitCode) => {
                    // "the status is cleared, back to idle - not
                    // replaced with anything" (the kernel's own doc) -
                    // still a real change, reported the same as any
                    // other: `None`, not skipped.
                    emit_pane_status_changed(state, pane, None);
                }
                Ok(CommandEnded::Ignored) => {
                    // Nothing changed (an end with no start, or a second
                    // end in a row) - no event for a no-op.
                }
                Err(steg_kernel::Error::PaneNotFound(_)) => {
                    // Same reasoning as `CommandOutputStarted`'s own arm.
                }
                Err(other) => unreachable!(
                    "Tree::note_command_ended documents only Error::PaneNotFound as a \
                     possible error; {other} is not that"
                ),
            }
        }
        PaneEvent::OpenRetried { .. } => {
            // Observability only (TASK-1.112): no kernel state changes -
            // a retried-but-eventually-successful open is not a program
            // exit or a command mark. Same gap as `CommandOutputStarted`/
            // `CommandEnded` above: nothing forwards this to a client
            // event stream yet, because none exists yet.
        }
    }
}

/// Delivers one event to every subscriber whose own filter matches it
/// (TASK-1.46 §5/§6/§7) - called from every tree-changing site in this
/// file, still under the lock that site already holds, with `scope` the
/// session the event belongs to (every part-A event has one). `event`'s
/// own `to_wire` is the one place its name and body are derived - the
/// same definition a subscriber's own `TreeEvent::parse` reads back.
/// `enqueue_control` is lock-free and does no I/O (`outbound.rs`'s own
/// doc) - but a failed one *does* call `shutdown(Both)` on this
/// subscriber's own cloned socket, below, which is a real syscall made
/// while `state`'s lock is still held (non-blocking on a Unix socket in
/// practice, the same cross-thread-shutdown idiom `ConnectionCleanup`
/// already uses, not a bug - just not "nothing writes to a socket," which
/// an earlier draft of this comment overstated). AC #3's own answer to a
/// slow or closed subscriber is exactly that shutdown, called from here,
/// not a resend and not left to the writer thread to notice on its own.
pub(super) fn broadcast(state: &State, event: &steg_sdk::TreeEvent, scope: SessionId) {
    let (name, body) = event.to_wire();
    for subscriber in state.subscribers.values() {
        let Some(filter) = &subscriber.filter else {
            continue;
        };
        if !filter.matches(&name, scope) {
            continue;
        }
        let bytes = Connection::emit_event(&name, body.clone());
        if subscriber.outbound.enqueue_control(bytes).is_err() {
            // AC #3: a subscriber that falls behind is closed, not
            // resynced. Shutting its own socket down (not just its
            // `Outbound`) is what actually ends the connection: it
            // unblocks a writer stuck mid-write on a full OS buffer *and*
            // this connection's own reader thread, parked in a plain
            // `read()` with no idle timeout (TASK-1.45 §4) - nothing else
            // reaches that thread from here. `ConnectionCleanup::drop`
            // runs once the reader notices and returns, which is what
            // removes this entry from `subscribers` in turn.
            let _ = subscriber.shutdown.shutdown(std::net::Shutdown::Both);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::io::Read;
    use std::os::unix::net::UnixStream;
    use std::sync::Arc;
    use std::sync::mpsc;
    use std::thread;
    use std::time::{Duration, Instant};

    use steg_kernel::Moment;
    use steg_sdk::{ClientId, Program, ProgramStatus, Status, TreeEvent};

    use super::{State, apply_event_locked, broadcast};
    use crate::outbound::{Outbound, spawn_writer};
    use crate::pane::PaneEvent;
    use crate::pty::RealLauncher;
    use crate::runtime::testing::{
        TEST_GRACE, TEST_TICK, fresh_state_with_one_pane, test_base_env, test_state,
    };
    use crate::runtime::close::force_end;
    use crate::runtime::{Filter, LoginShell, Runtime, Subscriber};

    /// Registers a subscriber in `state` with a real writer thread
    /// actually draining its queue onto a real, paired `UnixStream` - so
    /// `broadcast`'s own calls (`apply_event_locked`'s real code path, no
    /// doubles) can be observed from the test's own end of the pair,
    /// exactly as a real connection's reader would. `filter` is the same
    /// `Some`/`None` distinction a real connection has before/after
    /// `event.watch` - a test that wants to prove a registered-but
    /// -unfiltered subscriber receives nothing passes `None`.
    fn register_test_subscriber(
        state: &mut State,
        filter: Option<Filter>,
    ) -> (UnixStream, Arc<Outbound>, thread::JoinHandle<()>) {
        let (read_end, write_end) = UnixStream::pair().unwrap();
        let outbound = Arc::new(Outbound::new());
        let writer = spawn_writer(Arc::clone(&outbound), write_end.try_clone().unwrap());
        state.subscribers.insert(
            ClientId::from_raw(1),
            Subscriber {
                outbound: Arc::clone(&outbound),
                filter,
                shutdown: write_end,
            },
        );
        (read_end, outbound, writer)
    }

    /// Registers a subscriber with *no* writer thread at all - nothing
    /// ever drains its `Outbound`, so `CONTROL_CAP` overflows
    /// deterministically at a known call count, with no dependence on a
    /// real OS socket buffer's own size or a real writer thread's own
    /// scheduling (the real-socket version of this proof, tried first in
    /// `tests/events.rs`, was flaky for exactly that reason - named
    /// there, not hidden).
    fn register_slow_subscriber(state: &mut State, filter: Option<Filter>) -> UnixStream {
        let (read_end, write_end) = UnixStream::pair().unwrap();
        let outbound = Arc::new(Outbound::new());
        state.subscribers.insert(
            ClientId::from_raw(1),
            Subscriber {
                outbound,
                filter,
                shutdown: write_end,
            },
        );
        read_end
    }

    /// Reads exactly one event off `stream` as a `TreeEvent`, bounded -
    /// never a bare `read` with no deadline. `connection` is the caller's
    /// own, carried across every call for the same stream - a fresh
    /// `Connection` per call would discard any bytes a single `read`
    /// happened to over-fetch (more than one queued frame can arrive in
    /// one `read()`), silently losing the next event this same test still
    /// expects; found exactly that way while building this test.
    fn read_one_event(stream: &mut UnixStream, connection: &mut steg_sdk::Connection) -> TreeEvent {
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        loop {
            match connection.poll() {
                Some(steg_sdk::Incoming::Event { name, body }) => {
                    return TreeEvent::parse(&name, body).expect("a described event parses");
                }
                Some(other) => panic!("expected an event, got {other:?}"),
                None => {}
            }
            let mut buf = [0_u8; 4096];
            let n = stream.read(&mut buf).expect("reading the broadcast event");
            assert!(n > 0, "the writer closed before sending an event");
            connection.receive(&buf[..n]);
        }
    }

    /// Asserts nothing arrives on `stream` within `bound` - the no-poll
    /// proof's own negative space: a quiet window with nothing broadcast
    /// produces nothing read, not a timeout racing a real event.
    fn assert_nothing_arrives_within(stream: &mut UnixStream, bound: Duration) {
        stream.set_read_timeout(Some(bound)).unwrap();
        let mut buf = [0_u8; 16];
        match stream.read(&mut buf) {
            Ok(0) => {}
            Ok(n) => panic!("expected nothing, got {n} bytes"),
            Err(err) => assert_eq!(
                err.kind(),
                std::io::ErrorKind::WouldBlock,
                "expected a plain timeout, got {err:?}"
            ),
        }
    }

    /// Bounded, never a bare `recv` - the no-sleep rule's own reasoning
    /// for every real-process wait in this crate's tests.
    const EVENT_TIMEOUT: Duration = Duration::from_secs(20);

    /// AC #2's cascade, proven deterministically: a hand-built
    /// `PaneEvent::Exited` against a `Tree` already holding a started
    /// pane, with no thread, no channel and no timing at all - the thing
    /// the real drain thread calls in a loop is exactly this function.
    #[test]
    fn apply_event_records_an_exit_and_leaves_a_command_pane_in_place() {
        let (mut state, pane) = fresh_state_with_one_pane();

        apply_event_locked(&mut state, PaneEvent::Exited { pane, exit_code: 3 });

        let found = state
            .tree
            .session(state.tree.sessions()[0].id())
            .unwrap()
            .tabs()[0]
            .pane(pane)
            .unwrap();
        assert_eq!(
            found.program_status(),
            Some(ProgramStatus::Ended { exit_code: 3 })
        );
    }

    #[test]
    fn apply_event_closes_a_shell_pane_through_the_same_cascade_as_a_manual_close() {
        let mut tree = steg_kernel::Tree::new();
        let session = tree.new_session(None).unwrap();
        let pane = tree.session(session).unwrap().tabs()[0].panes()[0].id();
        tree.start_program(
            pane,
            Program::Shell {
                cwd: "/tmp".to_string(),
            },
        )
        .unwrap();
        let mut state = test_state(tree, HashMap::new());

        // This pane is its tab's only pane, and its tab is its session's
        // only tab (R94) - a shell pane's own exit must cascade all the
        // way to the session closing, in one `apply_event` call, with no
        // further request issued.
        apply_event_locked(&mut state, PaneEvent::Exited { pane, exit_code: 0 });

        assert!(state.tree.sessions().is_empty());
    }

    /// AC #2: a command pane's own real exit, the `Stayed` outcome, emits
    /// `pane.exited` to a subscriber - the exact production path
    /// `apply_event_locked` is, not a synthetic call to `broadcast`.
    /// Negative control: comment out the one `broadcast` call this test
    /// depends on in `apply_event_locked`'s `Exited` arm - this test then
    /// times out waiting for an event that never arrives, proving it
    /// actually exercises that call.
    #[test]
    fn a_command_panes_exit_emits_pane_exited() {
        let (mut state, pane) = fresh_state_with_one_pane();
        let (mut stream, outbound, writer) = register_test_subscriber(
            &mut state,
            Some(Filter {
                name: None,
                session: None,
            }),
        );
        let mut connection =
            steg_sdk::Connection::new(steg_sdk::WireRole::Client, steg_sdk::description());

        apply_event_locked(&mut state, PaneEvent::Exited { pane, exit_code: 3 });

        assert_eq!(
            read_one_event(&mut stream, &mut connection),
            TreeEvent::PaneExited { pane, exit_code: 3 }
        );

        outbound.close();
        writer.join().unwrap();
    }

    /// AC #2: a shell pane's own exit, which cascades all the way to its
    /// session closing (R94), emits all four events in the stated order -
    /// `pane.exited` first (the causal fact), then the closure cascade
    /// innermost first. Negative control: the same comment-out-the-call
    /// proof as the test above, applied to `emit_close_cascade`'s own
    /// three calls instead - this test then sees fewer than four events
    /// and times out waiting for the rest.
    #[test]
    fn a_shell_panes_exit_emits_the_whole_cascade_in_order() {
        let mut tree = steg_kernel::Tree::new();
        let session = tree.new_session(None).unwrap();
        let tab = tree.session(session).unwrap().tabs()[0].id();
        let pane = tree.session(session).unwrap().tabs()[0].panes()[0].id();
        tree.start_program(
            pane,
            Program::Shell {
                cwd: "/tmp".to_string(),
            },
        )
        .unwrap();
        let mut state = test_state(tree, HashMap::new());
        let (mut stream, outbound, writer) = register_test_subscriber(
            &mut state,
            Some(Filter {
                name: None,
                session: None,
            }),
        );
        let mut connection =
            steg_sdk::Connection::new(steg_sdk::WireRole::Client, steg_sdk::description());

        apply_event_locked(&mut state, PaneEvent::Exited { pane, exit_code: 0 });

        assert_eq!(
            read_one_event(&mut stream, &mut connection),
            TreeEvent::PaneExited { pane, exit_code: 0 }
        );
        assert_eq!(
            read_one_event(&mut stream, &mut connection),
            TreeEvent::PaneClosed { id: pane }
        );
        assert_eq!(
            read_one_event(&mut stream, &mut connection),
            TreeEvent::TabClosed { id: tab }
        );
        assert_eq!(
            read_one_event(&mut stream, &mut connection),
            TreeEvent::SessionClosed { id: session }
        );

        outbound.close();
        writer.join().unwrap();
    }

    /// AC #2: a typed command's start and end (TASK-1.84's own marks)
    /// each emit `pane.status_changed`, with the status the kernel's own
    /// `CommandOutputStarted`/`CommandEnded` computed - the same
    /// `apply_event_locked` path a real OSC 133 mark drives, proven here
    /// deterministically with no pty, the same reasoning this module's
    /// own existing `apply_event` tests already give.
    #[test]
    fn a_typed_commands_start_and_end_emit_pane_status_changed() {
        let mut tree = steg_kernel::Tree::new();
        let session = tree.new_session(None).unwrap();
        let pane = tree.session(session).unwrap().tabs()[0].panes()[0].id();
        tree.start_program(
            pane,
            Program::Shell {
                cwd: "/tmp".to_string(),
            },
        )
        .unwrap();
        let mut state = test_state(tree, HashMap::new());
        let (mut stream, outbound, writer) = register_test_subscriber(
            &mut state,
            Some(Filter {
                name: None,
                session: None,
            }),
        );
        let mut connection =
            steg_sdk::Connection::new(steg_sdk::WireRole::Client, steg_sdk::description());

        apply_event_locked(&mut state, PaneEvent::CommandOutputStarted { pane });
        assert_eq!(
            read_one_event(&mut stream, &mut connection),
            TreeEvent::PaneStatusChanged {
                pane,
                status: Some(Status::Running)
            }
        );

        apply_event_locked(
            &mut state,
            PaneEvent::CommandEnded {
                pane,
                exit_code: Some(0),
            },
        );
        assert_eq!(
            read_one_event(&mut stream, &mut connection),
            TreeEvent::PaneStatusChanged {
                pane,
                status: Some(Status::Finished)
            }
        );

        outbound.close();
        writer.join().unwrap();
    }

    /// A subscriber registered but never having called `event.watch`
    /// (`filter: None`) receives nothing - never everything unfiltered.
    /// Negative control: pass `Some(Filter{name: None, session: None})`
    /// instead (every other test above already does, and sees events) -
    /// this test would then fail by finding an event where it expects
    /// silence, proving the assertion is real.
    #[test]
    fn an_unfiltered_subscriber_receives_nothing() {
        let (mut state, pane) = fresh_state_with_one_pane();
        let (mut stream, outbound, writer) = register_test_subscriber(&mut state, None);

        apply_event_locked(&mut state, PaneEvent::Exited { pane, exit_code: 0 });

        assert_nothing_arrives_within(&mut stream, Duration::from_millis(200));

        outbound.close();
        writer.join().unwrap();
    }

    /// AC #3: a subscriber whose own queue is full is closed, not
    /// resynced and not blocked on. With no writer thread draining it at
    /// all, `broadcast`'s own `enqueue_control` calls must eventually
    /// return `Full` - once they do, this test's own read end must see
    /// the connection close (a clean `Ok(0)`, since `shutdown(Both)` on
    /// the paired end is exactly what `UnixStream::pair` makes visible
    /// that way), not keep accepting data forever. Negative control:
    /// remove `broadcast`'s own `if ... is_err() { shutdown }` arm,
    /// replacing it with the silent `let _ = ...` it used to be - this
    /// test then fails, reading bytes indefinitely instead of a close,
    /// since nothing ever tells this connection's own (nonexistent, in
    /// this test) reader to stop.
    #[test]
    fn a_slow_subscribers_connection_is_shut_down_once_its_queue_is_full() {
        let (mut state, _pane) = fresh_state_with_one_pane();
        let mut stream = register_slow_subscriber(
            &mut state,
            Some(Filter {
                name: None,
                session: None,
            }),
        );

        // Every `session.new`'s own three creation events, repeated well
        // past `CONTROL_CAP` (256) - nothing drains this subscriber's own
        // queue at all, so this is guaranteed to overflow, not merely
        // likely to.
        for _ in 0..200 {
            let session = state.tree.new_session(None).unwrap();
            let tab = state.tree.session(session).unwrap().tabs()[0].id();
            let pane = state.tree.session(session).unwrap().tabs()[0].panes()[0].id();
            broadcast(
                &state,
                &TreeEvent::SessionCreated {
                    id: session,
                    label: None,
                },
                session,
            );
            broadcast(
                &state,
                &TreeEvent::TabCreated {
                    id: tab,
                    session,
                    label: None,
                },
                session,
            );
            broadcast(
                &state,
                &TreeEvent::PaneCreated {
                    id: pane,
                    tab,
                    label: None,
                },
                session,
            );
        }

        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut buf = [0_u8; 65536];
        loop {
            match stream.read(&mut buf) {
                Ok(0) => break, // the connection closed, as expected
                Ok(_) => {}     // still draining what was queued before the shutdown
                Err(err) => panic!("expected a clean close, got {err:?}"),
            }
        }
    }

    #[test]
    fn apply_event_on_a_pane_the_tree_no_longer_holds_removes_the_stale_handle_without_panicking() {
        // The orphan a forced close leaves (§5, until the task that ends
        // a running program on `--force`): the tree has already lost the
        // pane (simulated here directly, standing in for that future
        // close), but a `PaneHandle` for it is still in `panes` - exactly
        // what happens when that program later exits on its own.
        let tree = steg_kernel::Tree::new();
        let gone = steg_sdk::PaneId::from_raw(999);
        let (events_tx, _events_rx) = mpsc::channel();
        let handle = crate::pane::start(
            crate::pty::LaunchSpec {
                argv: vec!["sh".to_string(), "-c".to_string(), "sleep 1".to_string()],
                cwd: "/tmp".to_string(),
                env: test_base_env(),
                size: (80, 24),
            },
            gone,
            std::sync::Arc::new(RealLauncher::default()),
            events_tx,
            TEST_TICK,
        )
        .unwrap();
        let mut state = test_state(tree, HashMap::from([(gone, handle)]));

        apply_event_locked(
            &mut state,
            PaneEvent::Exited {
                pane: gone,
                exit_code: 0,
            },
        );

        assert!(
            !state.panes.contains_key(&gone),
            "the stale handle must be removed, not leaked"
        );
    }

    /// Proves the plumbing *before* `apply_event` is real - a real
    /// process's real exit reaching a real channel - the same shape
    /// `crates/server/src/pane.rs`'s own tests already use to bypass
    /// `PaneHandle` for a focused check, bypassing `Runtime`'s own drain
    /// thread too so this stays deterministic and bounded.
    #[test]
    fn a_real_exit_event_drives_the_same_cascade_apply_event_runs() {
        let mut tree = steg_kernel::Tree::new();
        let session = tree.new_session(None).unwrap();
        let pane = tree.session(session).unwrap().tabs()[0].panes()[0].id();
        tree.start_program(
            pane,
            Program::Shell {
                cwd: "/tmp".to_string(),
            },
        )
        .unwrap();

        let (events_tx, events_rx) = mpsc::channel();
        let handle = crate::pane::start(
            crate::pty::LaunchSpec {
                argv: vec!["sh".to_string(), "-c".to_string(), "exit 0".to_string()],
                cwd: "/tmp".to_string(),
                env: test_base_env(),
                size: (80, 24),
            },
            pane,
            std::sync::Arc::new(RealLauncher::default()),
            events_tx,
            TEST_TICK,
        )
        .unwrap();

        let event = events_rx
            .recv_timeout(EVENT_TIMEOUT)
            .expect("the real shell's own exit, within the bound");

        let mut state = test_state(tree, HashMap::from([(pane, handle)]));
        apply_event_locked(&mut state, event);

        assert!(state.tree.sessions().is_empty());
        assert!(!state.panes.contains_key(&pane));
    }

    // --- typed command marks (TASK-1.84) ----------------------------------
    //
    // Every test below drives a real `Runtime` - its own drain thread and
    // event channel, not a hand-built `apply_event_locked` call - with a
    // real pty program that prints real OSC 133 sequences directly (no
    // shell integration exists yet; that is TASK-1.85's own job, so each
    // script here stands in for a shell that already has it). Every wait
    // is bounded; nothing here adds a new thread or a new process of its
    // own (AC #3) - marks ride the `events` channel and drain thread
    // `Runtime::new` already starts, and every test lets `Drop` (or an
    // explicit `stop()`) end whatever real program it started.

    const MARK_TIMEOUT: Duration = Duration::from_secs(20);

    fn moment_now(state: &State) -> Moment {
        Moment::new(Instant::now().duration_since(state.clock_origin))
    }

    fn find_pane(tree: &steg_kernel::Tree, pane: steg_sdk::PaneId) -> Option<&steg_kernel::Pane> {
        tree.sessions()
            .iter()
            .find_map(|session| session.tabs().iter().find_map(|tab| tab.pane(pane)))
    }

    fn pane_status(runtime: &Runtime, pane: steg_sdk::PaneId) -> Option<Status> {
        let state = runtime.state.lock().unwrap();
        let now = moment_now(&state);
        find_pane(&state.tree, pane).and_then(|p| p.status(now))
    }

    fn pane_program_status(runtime: &Runtime, pane: steg_sdk::PaneId) -> Option<ProgramStatus> {
        let state = runtime.state.lock().unwrap();
        find_pane(&state.tree, pane).and_then(steg_kernel::Pane::program_status)
    }

    fn tree_next_status_deadline(runtime: &Runtime) -> Option<Moment> {
        let state = runtime.state.lock().unwrap();
        let now = moment_now(&state);
        state.tree.next_status_deadline(now)
    }

    /// Bounded, polling (never a bare wait): there is no event a test can
    /// wait on instead - a status read is a value, not an event stream
    /// (R52's own gap, TASK-1.46) - the same reasoning every other real-
    /// process wait in this crate already gives.
    fn wait_for_pane_status(runtime: &Runtime, pane: steg_sdk::PaneId, expected: Option<Status>) {
        let start = Instant::now();
        loop {
            let found = pane_status(runtime, pane);
            if found == expected {
                return;
            }
            assert!(
                start.elapsed() < MARK_TIMEOUT,
                "pane {pane} never reached status {expected:?}; last seen {found:?}"
            );
            thread::sleep(Duration::from_millis(20));
        }
    }

    /// Bounded, polling the one fact (TASK-1.84 review) that is true
    /// exactly when `apply_event_locked` has applied `expected` of this
    /// pane's own `CommandOutputStarted`/`CommandEnded` events - read
    /// under the same lock `apply_event_locked` itself holds while
    /// updating it, so observing `expected` here means that call has
    /// already fully returned, not merely that a sleep has elapsed.
    fn wait_for_applied_mark_count(runtime: &Runtime, pane: steg_sdk::PaneId, expected: u64) {
        let start = Instant::now();
        loop {
            let found = {
                let state = runtime.state.lock().unwrap();
                state.applied_marks.get(&pane).copied().unwrap_or(0)
            };
            if found >= expected {
                return;
            }
            assert!(
                start.elapsed() < MARK_TIMEOUT,
                "pane {pane} never reached {expected} applied marks; last seen {found}"
            );
            thread::sleep(Duration::from_millis(5));
        }
    }

    fn send_input(runtime: &Runtime, pane: steg_sdk::PaneId, bytes: &[u8]) {
        let state = runtime.state.lock().unwrap();
        state.panes[&pane].send_input(bytes.to_vec()).unwrap();
    }

    /// Builds a fresh session/tab/pane on `runtime`'s own tree, started as
    /// `program` (a `Program::Shell` or `Program::Command` - R97's own
    /// hinge on whether a typed-command mark can ever change this pane's
    /// status at all), and spawns `argv` as its *real* process through
    /// `runtime`'s own event channel and drain thread - not a hand-built
    /// `apply_event_locked` call, so every test using this is exercising
    /// the real wiring §1's own ordering argument describes, not a
    /// simulation of it. `program` and `argv` are independent on purpose,
    /// the same decoupling `fresh_state_with_one_pane` already relies on
    /// elsewhere in this file: the kernel's own `Program` is bookkeeping
    /// metadata, never read by anything that decides what real process
    /// `crate::pane::start` actually runs.
    fn start_real_pane(
        runtime: &Runtime,
        program: Program,
        argv: Vec<&str>,
    ) -> (steg_sdk::SessionId, steg_sdk::TabId, steg_sdk::PaneId) {
        let (session_id, tab_id, pane) = {
            let mut state = runtime.state.lock().unwrap();
            let session_id = state.tree.new_session(None).unwrap();
            let pane = state.tree.session(session_id).unwrap().tabs()[0].panes()[0].id();
            let tab_id = state.tree.tab_of_pane(pane).unwrap();
            state.tree.start_program(pane, program).unwrap();
            (session_id, tab_id, pane)
        };
        let events = runtime
            .events
            .clone()
            .expect("runtime just built for this test");
        let handle = crate::pane::start(
            crate::pty::LaunchSpec {
                argv: argv.into_iter().map(str::to_string).collect(),
                cwd: "/tmp".to_string(),
                env: test_base_env(),
                size: (80, 24),
            },
            pane,
            std::sync::Arc::new(RealLauncher::default()),
            events,
            TEST_TICK,
        )
        .unwrap();
        runtime.state.lock().unwrap().panes.insert(pane, handle);
        (session_id, tab_id, pane)
    }

    fn login_for_marks() -> LoginShell {
        LoginShell {
            argv0: "/bin/sh".to_string(),
            home: "/tmp".to_string(),
            user: None,
            base_env: test_base_env(),
        }
    }

    /// Criterion 2's first shape, and criterion 1's own wording: the
    /// kernel's status for the pane, and the tab's shown status read
    /// through the kernel, both reflect a typed command that succeeds.
    #[test]
    fn a_typed_command_that_succeeds_is_running_then_finished_and_the_tab_shows_it() {
        let runtime = Runtime::new(
            std::sync::Arc::new(RealLauncher::default()),
            login_for_marks(),
            TEST_GRACE,
            TEST_TICK,
        );
        let (_, tab_id, pane) = start_real_pane(
            &runtime,
            Program::Shell {
                cwd: "/tmp".to_string(),
            },
            vec![
                "sh",
                "-c",
                "printf '\\033]133;C\\007'; sleep 0.5; printf '\\033]133;D;0\\007'; sleep 100",
            ],
        );

        wait_for_pane_status(&runtime, pane, Some(Status::Running));
        wait_for_pane_status(&runtime, pane, Some(Status::Finished));

        let state = runtime.state.lock().unwrap();
        let now = moment_now(&state);
        assert_eq!(
            state.tree.tab_shown_status(tab_id, now).unwrap(),
            Some(pane),
            "the tab's shown status must reflect the typed command, read through the kernel \
             (criterion 1)"
        );
    }

    #[test]
    fn a_typed_command_that_fails_is_failed_with_its_exit_code() {
        let runtime = Runtime::new(
            std::sync::Arc::new(RealLauncher::default()),
            login_for_marks(),
            TEST_GRACE,
            TEST_TICK,
        );
        let (_, _, pane) = start_real_pane(
            &runtime,
            Program::Shell {
                cwd: "/tmp".to_string(),
            },
            vec![
                "sh",
                "-c",
                "printf '\\033]133;C\\007'; sleep 0.5; printf '\\033]133;D;3\\007'; sleep 100",
            ],
        );

        wait_for_pane_status(&runtime, pane, Some(Status::Running));
        wait_for_pane_status(&runtime, pane, Some(Status::Failed { exit_code: 3 }));
    }

    #[test]
    fn a_typed_command_still_running_stays_running_until_told_otherwise() {
        let runtime = Runtime::new(
            std::sync::Arc::new(RealLauncher::default()),
            login_for_marks(),
            TEST_GRACE,
            TEST_TICK,
        );
        let (_, _, pane) = start_real_pane(
            &runtime,
            Program::Shell {
                cwd: "/tmp".to_string(),
            },
            vec![
                "sh",
                "-c",
                "printf '\\033]133;C\\007'; read line; printf '\\033]133;D;0\\007'; sleep 100",
            ],
        );

        wait_for_pane_status(&runtime, pane, Some(Status::Running));

        // This half is what makes the assertion above not vacuous: a
        // still-running test that only ever checks `Running` cannot tell
        // "the `CommandOutputStarted` arm works" from "nothing on this
        // path was ever reached". Telling the script to finish, from
        // this test, and watching the status actually move away from
        // `Running`, proves the wiring is live.
        send_input(&runtime, pane, b"\n");

        wait_for_pane_status(&runtime, pane, Some(Status::Finished));
    }

    #[test]
    fn a_shell_with_no_marks_leaves_every_status_field_unchanged_until_one_arrives() {
        let runtime = Runtime::new(
            std::sync::Arc::new(RealLauncher::default()),
            login_for_marks(),
            TEST_GRACE,
            TEST_TICK,
        );
        let (_, _, pane) = start_real_pane(
            &runtime,
            Program::Shell {
                cwd: "/tmp".to_string(),
            },
            vec![
                "sh",
                "-c",
                "read line; printf '\\033]133;C\\007'; sleep 100",
            ],
        );

        // Bounded, not a race against nothing: every field R82/R83 govern
        // stays exactly as a shell idling at its own prompt starts out,
        // for as long as nothing has reported a mark.
        let start = Instant::now();
        while start.elapsed() < Duration::from_millis(300) {
            assert_eq!(
                pane_program_status(&runtime, pane),
                Some(ProgramStatus::Running),
                "a shell pane's own program status is unaffected either way"
            );
            assert_eq!(
                pane_status(&runtime, pane),
                None,
                "idle at its own prompt (R82/R83's own baseline): no mark has arrived yet"
            );
            assert_eq!(
                tree_next_status_deadline(&runtime),
                None,
                "nothing is scheduled to end on its own either"
            );
            thread::sleep(Duration::from_millis(20));
        }

        // The control named in the plan: the same harness, the same
        // pane, does detect a change once a mark actually arrives - so
        // the three "unchanged" assertions above were reading a
        // mechanism that can change, not one this test could never have
        // failed regardless of whether marks were wired up at all.
        send_input(&runtime, pane, b"\n");
        wait_for_pane_status(&runtime, pane, Some(Status::Running));
    }

    #[test]
    fn a_mark_then_the_shells_own_real_exit_cascades_with_no_panic() {
        let runtime = Runtime::new(
            std::sync::Arc::new(RealLauncher::default()),
            login_for_marks(),
            TEST_GRACE,
            TEST_TICK,
        );
        let (session_id, _, pane) = start_real_pane(
            &runtime,
            Program::Shell {
                cwd: "/tmp".to_string(),
            },
            vec!["sh", "-c", "printf '\\033]133;C\\007'; sleep 0.3; exit 7"],
        );

        wait_for_pane_status(&runtime, pane, Some(Status::Running));

        // The shell pane's own real exit, right behind the mark on the
        // same channel (§1's own ordering argument: the worker drains a
        // mark from `PtyBytes` before it ever reaches a later `Exited`
        // command, so both land on `events` in that order too) - R97's
        // cascade (a shell pane closes when its program exits) must run
        // exactly as `apply_event_closes_a_shell_pane_through_the_same_
        // cascade_as_a_manual_close` already proves with no mark at all,
        // not be disturbed by a `CommandOutputStarted` just ahead of it.
        let start = Instant::now();
        loop {
            let gone = {
                let state = runtime.state.lock().unwrap();
                state.tree.session(session_id).is_none() && !state.panes.contains_key(&pane)
            };
            if gone {
                break;
            }
            assert!(
                start.elapsed() < MARK_TIMEOUT,
                "the session never closed through the real exit cascade"
            );
            thread::sleep(Duration::from_millis(20));
        }
    }

    #[test]
    fn a_mark_for_a_pane_the_tree_no_longer_holds_does_nothing_and_does_not_panic() {
        // Same shape as `apply_event_on_a_pane_the_tree_no_longer_holds_
        // removes_the_stale_handle_without_panicking`: the tree never
        // gets this pane at all, standing in for the one path this arm
        // is actually reachable by (TASK-1.78's review traced it): a
        // forced close's own grace window, where `close_and_dispatch`
        // has already removed the pane from the tree, but `force_end`
        // has not yet ended its still-running program, which can still
        // emit a mark in the meantime. On the real path the handle
        // itself has already moved into `force_end`'s own local scope,
        // out of `panes` entirely - kept in `panes` here only to prove
        // this arm does not touch it (`assert!(state.panes.contains_
        // key(...))` below), not to claim the real path ever has it
        // there too.
        let tree = steg_kernel::Tree::new();
        let gone = steg_sdk::PaneId::from_raw(999);
        let (events_tx, _events_rx) = mpsc::channel();
        let handle = crate::pane::start(
            crate::pty::LaunchSpec {
                argv: vec!["sh".to_string(), "-c".to_string(), "sleep 100".to_string()],
                cwd: "/tmp".to_string(),
                env: test_base_env(),
                size: (80, 24),
            },
            gone,
            std::sync::Arc::new(RealLauncher::default()),
            events_tx,
            TEST_TICK,
        )
        .unwrap();
        let mut state = test_state(tree, HashMap::from([(gone, handle)]));

        apply_event_locked(&mut state, PaneEvent::CommandOutputStarted { pane: gone });
        apply_event_locked(
            &mut state,
            PaneEvent::CommandEnded {
                pane: gone,
                exit_code: Some(0),
            },
        );

        assert!(
            state.panes.contains_key(&gone),
            "unlike `Exited`, a mark's `PaneNotFound` must not remove the handle - the \
             program it owns may still be running"
        );

        // The handle this test built is real (a real spawned `sleep
        // 100`) - ending it here is the same cleanup path the orphan
        // would get for real, either from its own eventual exit (the
        // sibling test's own `Exited`-driven removal) or from
        // `Runtime::stop` ending every pane when the whole server
        // stops; this test has neither a tree pane nor a `Runtime`
        // around it to give it one automatically. `force_end` (TASK-
        // 1.78) is that same cleanup path's own real implementation now
        // that `PaneHandle::stop` no longer exists.
        let handle = state.panes.remove(&gone).unwrap();
        force_end(vec![(gone, handle)], TEST_GRACE);
    }

    /// A command pane's own program printing OSC 133 (R97): both marks
    /// are applied by the runtime - waited on through `applied_marks`,
    /// never a sleep or a different pane's own marks - and change
    /// nothing, since the `D` mark's own exit code (3) differs from the
    /// real exit this script ends with (0): a wrongly-applied mark would
    /// read `Failed { exit_code: 3 }`, a value the real exit path alone
    /// can never produce. Only the later, real exit drives `Finished`.
    #[test]
    fn a_command_panes_own_program_printing_marks_does_not_change_its_status() {
        let runtime = Runtime::new(
            std::sync::Arc::new(RealLauncher::default()),
            login_for_marks(),
            TEST_GRACE,
            TEST_TICK,
        );

        let script = "printf '\\033]133;C\\007'; sleep 0.3; printf '\\033]133;D;3\\007'; \
                       sleep 0.3; exit 0";
        let (_, _, pane) = start_real_pane(
            &runtime,
            Program::Command {
                argv: vec!["sh".to_string(), "-c".to_string(), script.to_string()],
                cwd: "/tmp".to_string(),
            },
            vec!["sh", "-c", script],
        );

        // `start_program` already set this the moment the program did
        // (kernel `Pane::start_program`'s own doc) - independent of
        // anything the real process has printed yet.
        assert_eq!(pane_status(&runtime, pane), Some(Status::Running));

        // Both of this pane's own marks (`C`, then `D;3`) have now been
        // applied by the runtime's drain thread.
        wait_for_applied_mark_count(&runtime, pane, 2);

        assert_eq!(
            pane_status(&runtime, pane),
            Some(Status::Running),
            "a command pane's own program always wins over a mark (R97)"
        );
        assert_eq!(
            pane_program_status(&runtime, pane),
            Some(ProgramStatus::Running)
        );

        // Only the program's own real exit (`record_exit`), never `note_
        // command_ended`, moves it from here - and it reads `Finished`
        // (the real exit's own code, 0), never `Failed` (the mark's own
        // code, 3).
        wait_for_pane_status(&runtime, pane, Some(Status::Finished));
    }

    /// A pane.show/tab.show request for `pane`'s own context, over the
    /// public `Runtime::handle` - the same entry point a real connection
    /// reaches (`connection.rs`). Panics with the response's own error on
    /// anything but `Response::Ok`, since every call site below already
    /// knows the method it names must succeed.
    fn show_pane_and_tab(
        runtime: &Runtime,
        session: steg_sdk::SessionId,
        tab: steg_sdk::TabId,
        pane: steg_sdk::PaneId,
    ) -> (serde_json::Value, serde_json::Value) {
        let description = steg_sdk::description();
        let context = crate::dispatch::Context {
            default_session: Some(session),
            default_tab: Some(tab),
            default_pane: Some(pane),
        };
        let pane_record = match runtime.handle(
            &description,
            &context,
            &steg_sdk::Request {
                method: "pane.show".to_string(),
                params: serde_json::Map::new(),
            },
        ) {
            steg_sdk::Response::Ok(value) => value,
            steg_sdk::Response::Error(error) => panic!("pane.show failed: {error:?}"),
        };
        let tab_record = match runtime.handle(
            &description,
            &context,
            &steg_sdk::Request {
                method: "tab.show".to_string(),
                params: serde_json::Map::new(),
            },
        ) {
            steg_sdk::Response::Ok(value) => value,
            steg_sdk::Response::Error(error) => panic!("tab.show failed: {error:?}"),
        };
        (pane_record, tab_record)
    }

    /// AC #4's "a typed command's status reaching pane show and tab show
    /// through a real pty" (TASK-1.84's own marks), proven through the
    /// public `Runtime::handle` - not just the kernel's own `Pane::status`
    /// the way this module's sibling tests above already do (criterion 1
    /// of TASK-1.84's own plan). The kernel-level wait
    /// (`wait_for_pane_status`) is what makes the one `Runtime::handle`
    /// call below deterministic: by the time it returns, the drain thread
    /// has already applied both marks under the same lock `dispatch`
    /// itself takes.
    #[test]
    fn a_typed_commands_status_reaches_pane_show_and_tab_show_through_runtime_handle() {
        let runtime = Runtime::new(
            std::sync::Arc::new(RealLauncher::default()),
            login_for_marks(),
            TEST_GRACE,
            TEST_TICK,
        );
        let (session, tab_id, pane) = start_real_pane(
            &runtime,
            Program::Shell {
                cwd: "/tmp".to_string(),
            },
            vec![
                "sh",
                "-c",
                "printf '\\033]133;C\\007'; sleep 0.5; printf '\\033]133;D;3\\007'; sleep 100",
            ],
        );

        wait_for_pane_status(&runtime, pane, Some(Status::Running));
        let (pane_record, tab_record) = show_pane_and_tab(&runtime, session, tab_id, pane);
        assert_eq!(pane_record["status"], serde_json::json!("running"));
        assert_eq!(
            tab_record["shown_status"],
            serde_json::json!(pane.to_string())
        );

        wait_for_pane_status(&runtime, pane, Some(Status::Failed { exit_code: 3 }));
        let (pane_record, tab_record) = show_pane_and_tab(&runtime, session, tab_id, pane);
        assert_eq!(
            pane_record["status"],
            serde_json::json!({"failed": {"exit_code": 3}})
        );
        assert_eq!(
            tab_record["shown_status"],
            serde_json::json!(pane.to_string())
        );
    }

    /// AC #4's "a command pane's status from its exit", through
    /// `Runtime::handle` - `program_status` and `status` agree here (both
    /// move the instant the real program exits), unlike the typed-command
    /// case above, where only `status` is driven by a mark.
    #[test]
    fn a_command_panes_exit_reaches_pane_show_through_runtime_handle() {
        let runtime = Runtime::new(
            std::sync::Arc::new(RealLauncher::default()),
            login_for_marks(),
            TEST_GRACE,
            TEST_TICK,
        );
        let (session, tab_id, pane) = start_real_pane(
            &runtime,
            Program::Command {
                argv: vec!["sh".to_string(), "-c".to_string(), "exit 7".to_string()],
                cwd: "/tmp".to_string(),
            },
            vec!["sh", "-c", "exit 7"],
        );

        wait_for_pane_status(&runtime, pane, Some(Status::Failed { exit_code: 7 }));

        let (pane_record, _tab_record) = show_pane_and_tab(&runtime, session, tab_id, pane);
        assert_eq!(
            pane_record["status"],
            serde_json::json!({"failed": {"exit_code": 7}})
        );
        assert_eq!(pane_record["program_status"], serde_json::json!("ended"));
        assert_eq!(pane_record["exit_code"], serde_json::json!(7));
    }
}
