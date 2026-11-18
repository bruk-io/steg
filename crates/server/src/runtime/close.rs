//! Close and force_end with its numbered passes (TASK-1.131 seam: "close
//! and force_end with its numbered passes" - the task's own pairing, not
//! a seam invented here).

use std::thread;
use std::time::{Duration, Instant};

use steg_kernel::{Moment, Tree};
use steg_sdk::{ApiError, Description, PaneId, Request, Response, SessionId};

use super::tree::session_owning_tab;
use super::{FORCE_JOIN_TIMEOUT, Runtime, State, broadcast, reapply_every_sized_tab};
use crate::dispatch::{self, Context, Outcome};
use crate::pane::PaneHandle;

/// How long [`force_end`] waits, after escalating, for a program that
/// ignored the polite signal to actually die (TASK-1.78 §2) - `SIGKILL`
/// cannot be trapped or blocked, so this bounds only OS scheduling and
/// reap latency, never a real wait for cooperation.
const ESCALATE_CONFIRM_TIMEOUT: Duration = Duration::from_secs(2);

impl Runtime {
    /// `pane.close`/`tab.close`/`session.close`: closes the tree *first*,
    /// under one lock acquisition, then ends every program that scope had
    /// running - the reverse of this task's own AC #1 wording, deliberate
    /// and explained in the task's own plan (§5/§6): the instant the lock
    /// below is released, the whole scope is already gone from the tree,
    /// so *every* other method - not only a second `--force` - gets the
    /// ordinary `PaneNotFound`/`TabNotFound`/`SessionNotFound` any stale
    /// target already gets, with nothing new threaded through `dispatch`'s
    /// own lookup helpers (AC #4, in full, not only force-vs-force).
    pub(super) fn close_and_dispatch(
        &self,
        description: &Description,
        context: &Context,
        request: &Request,
    ) -> Response {
        let mut state = self.lock();

        // Read *before* `dispatch_raw` runs: once a close succeeds, the
        // tree can no longer be asked which panes it used to cover.
        let scope = match dispatch::panes_in_close_scope(&state.tree, context, request) {
            Ok(scope) => scope,
            Err(error) => return Response::Error(error),
        };
        // Same reason, for this task's own closure events (§5): the one
        // thing every case needs that the kernel's own return value
        // cannot give (which session this cascade is scoped to, for
        // `broadcast`'s own filter - needed even when the cascade stops
        // at just the pane), plus, for `session.close` only, its tabs
        // grouped with their own panes (`Tree::close_session` reports no
        // structured cascade at all, unlike the other two methods - see
        // `emit_close_events`'s own doc for why this is the one case
        // still read entirely from the tree before the close).
        let pre = match capture_close_pre(&state.tree, context, request) {
            Ok(pre) => pre,
            Err(error) => return Response::Error(error),
        };

        let outcome = {
            // See `start_and_dispatch`'s own comment: one explicit deref
            // so the two `&mut` arguments below project to disjoint
            // fields of a plain `&mut State`.
            let state: &mut State = &mut state;
            let now = Moment::new(Instant::now().duration_since(state.clock_origin));
            dispatch::dispatch_raw(
                &mut state.tree,
                &mut state.settings,
                description,
                context,
                request,
                now,
            )
        };
        let outcome = match outcome {
            Ok(outcome) => outcome,
            Err(error) => {
                // The refusal (no `--force`, something running) or any
                // other error `dispatch_raw`'s own arm gives - nothing
                // changed, so nothing to pop or end.
                return Response::Error(error);
            }
        };

        // The close just ran, under the lock still held: the whole scope
        // is gone from the tree now, atomically - not partially, since
        // `close_pane`/`close_tab`/`close_session` are each one kernel
        // operation, never a loop this could interrupt midway. Emitted
        // before `force_end` runs (still under the lock, before it is
        // dropped a few lines down) - the close events describe the
        // tree, not the program's own lifecycle, and the tree has
        // already changed by this point (TASK-1.78's own plan already
        // assumed exactly this timing for this task). Reads `outcome`
        // itself (the kernel's own report of what actually closed, AC #2
        // and the review that found this task's first draft missed it
        // for `pane.close`/`tab.close`'s own cascades) before consuming
        // it into the reply's own value, below.
        emit_close_events(&state, request.method.as_str(), &pre, &scope, &outcome);

        let response = Response::Ok(outcome.into_value());

        let to_force: Vec<(PaneId, PaneHandle)> = scope
            .into_iter()
            .filter_map(|pane| state.panes.remove(&pane).map(|handle| (pane, handle)))
            .collect();
        reapply_every_sized_tab(&mut state);
        drop(state);

        // Lock-free: no other pane's request is ever stalled by this
        // (AC #4's first clause). `to_force` holds every popped handle
        // regardless of whether its own program was genuinely running -
        // `force_end` treats an already-ended one cheaply (its own
        // `signal_hangup`/`signal_kill` no-op, `await_exit` replies at
        // once), which is also this task's fix for the adjacent gap a
        // *plain* close of an already-ended pane used to leave behind
        // (its idle handle and worker thread, never cleaned up before
        // this task - not named by any of the five criteria, included
        // here because it falls out of this same pass at no extra cost).
        force_end(to_force, self.force_grace);

        response
    }
}

/// What `close_and_dispatch` needs captured *before* `dispatch_raw` runs
/// (§5 of this task's own plan, revised after a review found the first
/// version missed `pane.close`/`tab.close`'s own cascades): the session
/// every event in this close is scoped to - needed even when the kernel's
/// own cascade stops at just the pane, since nothing else still names it
/// once the pane is gone - and, for `session.close` alone, its tabs
/// grouped with their own panes. `pane.close`/`tab.close` need nothing
/// else pre-captured: what they actually closed comes back from the
/// kernel itself (`Outcome::PaneClosed`/`TabClosed`, read in
/// `emit_close_events` below), not from a second read of the tree after
/// the fact - the bug a review of this task found, and the reason this
/// is no longer named `CloseCascade`.
enum ClosePre {
    /// `pane.close`: just the session it was in.
    Pane { session: SessionId },
    /// `tab.close`: just the session it was in - its own panes are
    /// `close_and_dispatch`'s own `scope` (`panes_in_close_scope`),
    /// already exactly this tab's panes, reused rather than captured a
    /// second time here.
    Tab { session: SessionId },
    /// `session.close`: the session, and its tabs in order, each with its
    /// own panes in order - `Tree::close_session` reports no structured
    /// cascade of its own at all (unlike the other two), so this is the
    /// one case still read entirely from the tree before the close.
    Session {
        id: SessionId,
        tabs: Vec<(steg_sdk::TabId, Vec<PaneId>)>,
    },
}

/// Builds the [`ClosePre`] `close_and_dispatch` needs, before it closes
/// anything.
///
/// # Errors
/// Whatever resolving the request's own target gives - the same errors
/// `dispatch`'s own close arm would refuse with, caught here first since
/// this runs before it.
fn capture_close_pre(
    tree: &Tree,
    context: &Context,
    request: &Request,
) -> Result<ClosePre, ApiError> {
    match request.method.as_str() {
        "pane.close" => {
            let pane = dispatch::target_pane(tree, context, request)?;
            let tab = tree
                .tab_of_pane(pane)
                .expect("target_pane just resolved this pane; it is in the tree");
            Ok(ClosePre::Pane {
                session: session_owning_tab(tree, tab),
            })
        }
        "tab.close" => {
            let tab = dispatch::target_tab(tree, context, request)?;
            Ok(ClosePre::Tab {
                session: session_owning_tab(tree, tab),
            })
        }
        "session.close" => {
            let session = dispatch::target_session(tree, context, request)?;
            let tabs = tree
                .session(session)
                .map(|found| {
                    found
                        .tabs()
                        .iter()
                        .map(|tab| {
                            let panes = tab.panes().iter().map(steg_kernel::Pane::id).collect();
                            (tab.id(), panes)
                        })
                        .collect()
                })
                .unwrap_or_default();
            Ok(ClosePre::Session { id: session, tabs })
        }
        other => {
            unreachable!(
                "capture_close_pre is only ever called for the three close methods, not {other}"
            )
        }
    }
}

/// Emits one close's own events, innermost first (§5 of this task's own
/// plan): every pane, then its tab if the tab closed too, then the
/// session if that closed too - one rule, on every path, whichever of the
/// three methods caused it. `pane.close`/`tab.close` read what actually
/// closed from the kernel's own report (`outcome`); `session.close` reads
/// it from `pre` instead, since `Tree::close_session` reports none of its
/// own (`capture_close_pre`'s own doc says why). `tab_scope_panes` is
/// `close_and_dispatch`'s own `scope` (`panes_in_close_scope`) - for
/// `tab.close` this is already exactly that tab's own panes, so it is
/// reused here rather than captured a second time in `ClosePre`.
///
/// # Panics
/// Never in practice: `method` is always one of the three close verbs
/// (`close_and_dispatch`'s only caller), and `outcome`/`pre` are always
/// what that exact verb's own code just produced - the two are never
/// mismatched.
fn emit_close_events(
    state: &State,
    method: &str,
    pre: &ClosePre,
    tab_scope_panes: &[PaneId],
    outcome: &Outcome,
) {
    use steg_sdk::TreeEvent;

    match method {
        "pane.close" => {
            let ClosePre::Pane { session } = pre else {
                unreachable!("pane.close's own ClosePre is always ClosePre::Pane")
            };
            let Outcome::PaneClosed(closed) = outcome else {
                unreachable!("pane.close always produces Outcome::PaneClosed on success")
            };
            // Same shape `record_exit`'s own exit-driven cascade already
            // emits from a `ClosedPane` - one function, not two (the
            // second review round's own nit; this arm used to repeat
            // `emit_close_cascade`'s body verbatim).
            emit_close_cascade(state, closed, *session);
        }
        "tab.close" => {
            let ClosePre::Tab { session } = pre else {
                unreachable!("tab.close's own ClosePre is always ClosePre::Tab")
            };
            let Outcome::TabClosed(closed) = outcome else {
                unreachable!("tab.close always produces Outcome::TabClosed on success")
            };
            for pane in tab_scope_panes {
                broadcast(state, &TreeEvent::PaneClosed { id: *pane }, *session);
            }
            broadcast(state, &TreeEvent::TabClosed { id: closed.tab }, *session);
            if closed.session_now_empty {
                broadcast(state, &TreeEvent::SessionClosed { id: *session }, *session);
            }
        }
        "session.close" => {
            let ClosePre::Session { id, tabs } = pre else {
                unreachable!("session.close's own ClosePre is always ClosePre::Session")
            };
            for (tab_id, panes) in tabs {
                for pane in panes {
                    broadcast(state, &TreeEvent::PaneClosed { id: *pane }, *id);
                }
                broadcast(state, &TreeEvent::TabClosed { id: *tab_id }, *id);
            }
            broadcast(state, &TreeEvent::SessionClosed { id: *id }, *id);
        }
        other => unreachable!(
            "emit_close_events is only ever called for the three close methods, not {other}"
        ),
    }
}

/// Emits the closure half of a cascade `ClosedPane` already describes
/// (§5 of this task's own plan), innermost first: the pane always, then
/// its tab if the cascade also closed that (R94), then its session if
/// the cascade reached that far too. `scope` is the session every event
/// in the cascade is filtered against, even once `closed.session` itself
/// names it too - the same id, read once by the caller, since this
/// cascade can never reach a session other than the one the pane itself
/// was in.
pub(super) fn emit_close_cascade(state: &State, closed: &steg_kernel::ClosedPane, scope: SessionId) {
    use steg_sdk::TreeEvent;

    broadcast(state, &TreeEvent::PaneClosed { id: closed.pane }, scope);
    if let Some(tab) = closed.tab {
        broadcast(state, &TreeEvent::TabClosed { id: tab }, scope);
    }
    if let Some(session) = closed.session {
        broadcast(state, &TreeEvent::SessionClosed { id: session }, scope);
    }
}

/// Ends every program in `handles`, reaps each, joins every worker thread -
/// bounded by `grace` (shared across every pane in `handles`, not a fresh
/// window per pane) plus two small, fixed margins, regardless of how many
/// panes are given (TASK-1.78 §2). Used by both [`Runtime::stop`] (scope:
/// every pane) and [`crate::dispatch`]'s own close-with-`--force` wrapper
/// (scope: one pane, a tab's panes, or a session's tabs' panes) - one
/// shared primitive rather than two copies of "signal, wait, escalate,
/// join" that could drift apart. Never panics, never blocks past its own
/// stated bound; takes no lock of its own and must be called with none
/// held, since its own waits are real (bounded, but real).
pub(super) fn force_end(handles: Vec<(PaneId, PaneHandle)>, grace: Duration) {
    // Pass 0 (TASK-1.128): tells every pane's own worker it is being
    // closed, *before* anything below signals its program - over the
    // command queue (`PaneHandle::mark_closing`, not `RunGate`), so the
    // worker records `EndReason::Closed` ahead of the `Exited` the signal
    // below is about to cause, never behind it. This ordering holds
    // because this loop is a plain, synchronous send per handle that
    // fully returns before pass 1 even starts: the relay thread that
    // would send that `Exited` cannot observe a death the signal below
    // has not caused yet, and that signal is not sent until pass 1 runs.
    // A future change that parallelises this pass with pass 1 (the way
    // passes 2-6 already use `thread::scope`, for throughput) would
    // silently reopen the bug this pass exists to fix - keep it a plain,
    // sequential loop, strictly before pass 1's own loop starts.
    //
    // Covers every caller of `force_end`, both `close_and_dispatch`
    // (forced and plain closes alike - every popped handle lands here
    // regardless of whether its program was running) and `Runtime::stop`
    // (server stop) - one new pass fixes the reason for both, with
    // nothing to change at either call site.
    for (_, handle) in &handles {
        handle.mark_closing();
    }

    // Pass 1: signal every pane at once. `signal_hangup` goes straight
    // through `RunGate` (a `Mutex`, held only for the length of the
    // signal call itself - review finding 1), never through the
    // worker's own command queue, so this loop costs microseconds
    // regardless of how backed up any one pane's own worker is with its
    // own run's `PtyBytes`.
    for (_, handle) in &handles {
        handle.signal_hangup();
    }

    // Pass 2: wait against one shared deadline, not N deadlines - one
    // thread per pane so N panes share the deadline, not N * it.
    // `await_exit` is also `RunGate`-based (a `Condvar` wait, never the
    // worker's queue), which is what makes this a real "has the process
    // ended" check rather than "has the worker drained its backlog far
    // enough to notice" (the misclassification finding 1 found).
    let deadline = Instant::now() + grace;
    let stragglers: Vec<PaneId> = thread::scope(|scope| {
        let jobs: Vec<_> = handles
            .iter()
            .map(|(pane, handle)| {
                scope.spawn(move || {
                    let remaining = deadline.saturating_duration_since(Instant::now());
                    (*pane, handle.await_exit(remaining))
                })
            })
            .collect();
        jobs.into_iter()
            .map(|job| {
                job.join()
                    .expect("await_exit only waits on a Condvar with a bound, never panics")
            })
            .filter(|(_, ended)| !ended)
            .map(|(pane, _)| pane)
            .collect()
    });

    // Pass 3: escalate only the stragglers - same direct-`RunGate`
    // reasoning as pass 1.
    for (pane, handle) in &handles {
        if stragglers.contains(pane) {
            handle.signal_kill();
        }
    }

    // Pass 4: confirm the escalated ones actually died (`SIGKILL` cannot
    // be trapped, so this bound is a safety margin, not a real wait for
    // cooperation) - a pane that already ended during pass 2 is not
    // re-visited here.
    thread::scope(|scope| {
        for pane in &stragglers {
            if let Some((_, handle)) = handles.iter().find(|(p, _)| p == pane) {
                scope.spawn(|| {
                    handle.await_exit(ESCALATE_CONFIRM_TIMEOUT);
                });
            }
        }
    });

    // Pass 5: settle every worker thread *in parallel* - telling each
    // one its own run has ended, via `PaneHandle::settle_worker`, and
    // waiting for its reply, costs up to `FORCE_JOIN_TIMEOUT` in total
    // regardless of how many panes are in `handles`, because this runs
    // all of them at once rather than one after another. Folding this
    // into `join_worker` itself instead (settle-then-join, called once
    // per handle in the sequential pass below) was rejected during
    // review: that would have put the exact N-pane-multiplication risk
    // this task's own `force_end` exists to remove back into the one
    // place a worker's own queue can still be genuinely backed up (a
    // flooding run's `PtyBytes`) even after `RunGate` has already
    // confirmed, promptly, that the process itself is dead.
    thread::scope(|scope| {
        for (_, handle) in &handles {
            scope.spawn(|| {
                handle.settle_worker(FORCE_JOIN_TIMEOUT);
            });
        }
    });

    // Pass 6: join every worker thread, bounded - sequential, not
    // scoped: `join_worker` consumes `self` by value, and by this point
    // every handle's own worker has already been told to stop and
    // settled (pass 5), so each join is expected to return almost at
    // once, never block on another pane's own wait.
    for (_, handle) in handles {
        let _ = handle.join_worker(FORCE_JOIN_TIMEOUT);
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::Runtime;
    use crate::runtime::testing::{TEST_GRACE, TEST_TICK, login};
    use crate::pty::RealLauncher;

    /// The idle-handle fix itself (TASK-1.78 §5, item 2), proven where it
    /// can actually be observed - `state.panes`, private to this module,
    /// not through `Runtime::handle`'s own public responses, which never
    /// expose it either way (DRAFT-1: `tests/runtime.rs`'s own black-box
    /// sibling, `pane_close_on_an_already_ended_pane_succeeds_without_
    /// force`, would pass identically with or without this fix, so it is
    /// not offered as proof of it - this is).
    #[test]
    fn a_plain_close_of_an_already_ended_pane_removes_its_idle_handle() {
        let runtime = Runtime::new(
            std::sync::Arc::new(RealLauncher::default()),
            login(),
            TEST_GRACE,
            TEST_TICK,
        );
        let description = steg_sdk::description();
        let no_context = crate::dispatch::Context::none();

        let session = match runtime.handle(
            &description,
            &no_context,
            &steg_sdk::Request {
                method: "session.new".to_string(),
                params: serde_json::json!({"program": ["sh", "-c", "exit 0"]})
                    .as_object()
                    .unwrap()
                    .clone(),
            },
        ) {
            steg_sdk::Response::Ok(value) => value,
            steg_sdk::Response::Error(error) => panic!("session.new failed: {error:?}"),
        };
        let session_id: steg_sdk::SessionId = session["id"].as_str().unwrap().parse().unwrap();
        let (pane, tab_id) = {
            let state = runtime.state.lock().unwrap();
            let tab = &state.tree.session(session_id).unwrap().tabs()[0];
            (tab.panes()[0].id(), tab.id())
        };

        // Waits for the real exit first (R97: a command pane stays) by
        // polling the kernel's own status directly - bounded, no event
        // for this exists on the server side (`PaneEvent::Exited` only
        // reaches the drain thread, not a test holding the lock between
        // polls).
        let start = std::time::Instant::now();
        loop {
            let ended = {
                let state = runtime.state.lock().unwrap();
                matches!(
                    state
                        .tree
                        .session(session_id)
                        .and_then(|s| s.tabs()[0].pane(pane))
                        .and_then(steg_kernel::Pane::program_status),
                    Some(steg_sdk::ProgramStatus::Ended { .. })
                )
            };
            if ended {
                break;
            }
            assert!(
                start.elapsed() < Duration::from_secs(10),
                "the real `exit 0` never reached Ended within the bound"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(
            runtime.state.lock().unwrap().panes.contains_key(&pane),
            "the idle handle must still be there right after the program ends - \
             this is the gap's own starting condition, not yet the fix"
        );

        let context = crate::dispatch::Context {
            default_session: Some(session_id),
            default_tab: Some(tab_id),
            default_pane: Some(pane),
        };
        match runtime.handle(
            &description,
            &context,
            &steg_sdk::Request {
                method: "pane.close".to_string(),
                params: serde_json::json!({"target": pane.to_string()})
                    .as_object()
                    .unwrap()
                    .clone(),
            },
        ) {
            steg_sdk::Response::Ok(_) => {}
            steg_sdk::Response::Error(error) => panic!("pane.close failed: {error:?}"),
        }

        assert!(
            !runtime.state.lock().unwrap().panes.contains_key(&pane),
            "the plain close must remove the now-pointless idle handle, \
             not leave it (and its worker thread) running forever"
        );
    }
}
