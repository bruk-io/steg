//! `pane.wait` (TASK-1.140 - the cross-cutting seam TASK-1.131's own module
//! map did not anticipate, named as such on the task itself).
//!
//! Pending waits live in [`super::State`] (`pending_waits`), resolved from
//! the code paths that already hold its lock when the outcome becomes
//! known - a real exit ([`super::events::apply_event_locked`]'s own
//! `Exited` arm), a close ([`super::close::close_and_dispatch`]) or a
//! client disconnecting ([`crate::connection::ConnectionCleanup`]'s own
//! `Drop`) - plus one early-wakeable timer thread per positive-`--timeout`
//! wait, never a polling loop. [`WaitGate`]'s only job is waking that
//! timer thread early so it never sits out the rest of a timeout whose
//! answer is already known - the real answer always goes straight to the
//! wait's own [`Outbound`], never carried through the gate itself.

use std::collections::HashMap;
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::thread;
use std::time::Duration;

use steg_kernel::Pane;
use steg_sdk::{
    ApiError, ClientId, Connection, Description, ErrorCode, PaneId, ProgramStatus, Request,
    Response, WaitResult, validate,
};

use super::{Runtime, State};
use crate::dispatch::{self, Context};
use crate::outbound::Outbound;

/// A sleeping timer thread's own wake signal (TASK-1.140's own design
/// simplification over TASK-1.90's stale plan, which carried the outcome
/// itself through a `Mutex<Option<T>>`): the real reply is always written
/// straight to a [`PendingWait`]'s own `outbound` by whichever resolver
/// gets there first, so this only ever needs to wake a sleeping
/// `wait(timeout)` call early, never to carry a value.
struct WaitGate {
    state: Mutex<bool>,
    changed: Condvar,
}

impl WaitGate {
    fn new() -> Self {
        Self {
            state: Mutex::new(false),
            changed: Condvar::new(),
        }
    }

    fn lock(&self) -> MutexGuard<'_, bool> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Marks this gate resolved and wakes anyone waiting on it. A no-op if
    /// already resolved (set once, never un-set).
    fn resolve(&self) {
        let mut done = self.lock();
        *done = true;
        self.changed.notify_all();
    }

    /// Blocks up to `timeout`, returning early the instant [`Self::resolve`]
    /// is called from another thread. The caller always re-checks the real
    /// state afterward (`pending_waits`'s own entry, under the lock) - there
    /// is nothing useful for this to return itself.
    fn wait(&self, timeout: Duration) {
        let guard = self.lock();
        let _ = self
            .changed
            .wait_timeout_while(guard, timeout, |done| !*done);
    }
}

/// One client's own outstanding `pane.wait` (TASK-1.140): what to reply to
/// (`request_id`/`outbound`), who to clear it for on disconnect
/// (`client_id`), and the timer it wakes early, if `--timeout` was positive.
/// `pub(super)` only because [`PendingWaits`] (visible to `runtime`, for
/// `State`'s own field) cannot otherwise name a type `runtime` itself has
/// no way to see - nothing outside this module ever touches a `PendingWait`
/// value or its fields directly.
pub(super) struct PendingWait {
    request_id: u64,
    client_id: ClientId,
    outbound: Arc<Outbound>,
    gate: Option<Arc<WaitGate>>,
}

/// Every pane's own outstanding waits, kept in [`super::State`].
pub(super) type PendingWaits = HashMap<PaneId, Vec<PendingWait>>;

/// The three identifying values one `pane.wait` call needs beyond its own
/// request/description/context - grouped the same way [`super::Attaching`]
/// groups `attach_pane`'s own three, for the same clippy argument-count
/// reason.
pub(crate) struct Waiting {
    /// The connection waiting.
    pub(crate) client: ClientId,
    /// The request's own id, to match the reply.
    pub(crate) id: u64,
}

/// Writes `response` for `request_id` onto `outbound` - best-effort, the
/// same tolerance `attach_pane`'s own reply already documents: a failed
/// enqueue here only means the connection's own outbound is already closed
/// (nothing left to tell) or already full past `CONTROL_CAP` (a connection
/// in that state is not being read at all and is already on its way to
/// being torn down by the existing stuck-writer/shutdown machinery
/// `connection.rs`'s own doc describes) - deliberately not `broadcast`'s own
/// "shut the socket down" response to a full queue, which exists to stop a
/// slow *fan-out* subscriber from piling up unboundedly; a wait's reply is
/// a single, one-shot, point-to-point answer, not a stream.
fn reply(request_id: u64, response: Response, outbound: &Outbound) {
    let bytes = Connection::reply(request_id, response);
    let _ = outbound.enqueue_control(bytes);
}

/// `pane.wait`'s own success reply - the typed [`WaitResult`], not inline
/// JSON.
fn ok_response(exit_code: i32) -> Response {
    Response::Ok(
        serde_json::to_value(WaitResult { exit_code })
            .expect("WaitResult always serialises to JSON"),
    )
}

/// The one place `ErrorCode::TimedOut`'s message is built, naming the
/// timeout that expired, in seconds - called both by the `--timeout 0`
/// immediate branch and by [`timeout_wait_locked`]'s own real timeout, so
/// the wording is never duplicated.
fn timed_out(timeout_secs: i64) -> ApiError {
    ApiError::new(
        ErrorCode::TimedOut,
        format!("the wait's own {timeout_secs} second timeout passed before the condition was met"),
    )
}

/// `pane`'s own current program status, read the same way
/// `dispatch::pane::show`'s own `record_for` already does (via
/// `Tree::tab_of_pane` + `find_tab`/`Tab::pane`) - `None` only in the
/// window between a pane being minted and its program being started,
/// never observed in practice once a wait can even resolve the target at
/// all (every pane in the tree has a program started under the same lock
/// it was minted, `close.rs`'s own documented invariant).
fn program_status_of(state: &State, pane: PaneId) -> Option<ProgramStatus> {
    let tab_id = state.tree.tab_of_pane(pane)?;
    dispatch::find_tab(&state.tree, tab_id)
        .and_then(|tab| tab.pane(pane))
        .and_then(Pane::program_status)
}

/// Resolves every `PendingWait` on `pane` with `outcome` (TASK-1.140,
/// AC #1) - called with `state` already locked, from the two sites that
/// already hold it when the outcome becomes known: a real exit
/// (`Ok(exit_code)`) or a close (`Err(ErrorCode::NoSuchTarget)` - never a
/// timeout, which [`timeout_wait_locked`] resolves separately, since a
/// timeout is never "the condition `pane` itself reached," only this one
/// wait's own clock running out.
pub(super) fn resolve_pane_waits_locked(
    state: &mut State,
    pane: PaneId,
    outcome: Result<i32, ApiError>,
) {
    let Some(waits) = state.pending_waits.remove(&pane) else {
        return;
    };
    for wait in waits {
        let response = match &outcome {
            Ok(exit_code) => ok_response(*exit_code),
            Err(error) => Response::Error(error.clone()),
        };
        reply(wait.request_id, response, &wait.outbound);
        if let Some(gate) = &wait.gate {
            gate.resolve();
        }
    }
}

/// `pane_closed_while_waiting`'s own error (TASK-1.140, AC #1 - "a pane
/// closed while being waited on answers at once"): `ErrorCode::NoSuchTarget`
/// is already in `pane.wait`'s own `errors:` list today, so this needs no
/// description change at all - doc-2's own answer for "the thing you asked
/// about is gone," and `pane.wait`'s `Returns::Record{exit_code}` has
/// nothing honest to put there for a pane that was closed rather than
/// exited (the same reasoning `ClosedPane`/`EndReason::Closed` already
/// carry no exit code for).
pub(super) fn pane_closed_while_waiting() -> ApiError {
    ApiError::new(
        ErrorCode::NoSuchTarget,
        "the pane was closed while this wait was pending",
    )
}

/// The one timer thread a positive `--timeout` spawns calls this once it
/// wakes, whether woken early by [`WaitGate::resolve`] or by a real
/// timeout: finds and removes the one `PendingWait` on `pane` matching
/// `(request_id, client_id)` (several waits can be pending on the same
/// pane - matching both, not just the pane, is what keeps them
/// independent), and **only if it was still there**, replies
/// [`timed_out`]. If it is already gone - the exit/close path resolved it
/// first, in the race between that path dropping the lock and this thread
/// waking - this is a no-op: never a reply sent twice. Never called for
/// `--timeout 0`, which never registers anything for this to find -
/// `register_wait`'s own zero branch replies directly instead.
fn timeout_wait_locked(
    state: &Arc<Mutex<State>>,
    pane: PaneId,
    request_id: u64,
    client_id: ClientId,
    timeout_secs: i64,
) {
    let mut guard = state.lock().unwrap_or_else(PoisonError::into_inner);
    let Some(waits) = guard.pending_waits.get_mut(&pane) else {
        return;
    };
    let Some(index) = waits
        .iter()
        .position(|wait| wait.request_id == request_id && wait.client_id == client_id)
    else {
        return;
    };
    let wait = waits.remove(index);
    if waits.is_empty() {
        guard.pending_waits.remove(&pane);
    }
    drop(guard);
    reply(
        request_id,
        Response::Error(timed_out(timeout_secs)),
        &wait.outbound,
    );
}

impl Runtime {
    /// `pane.wait` (TASK-1.140, AC #1/#2): validates and resolves the
    /// target, then answers from the pane's own current program status -
    /// at once if it has already ended, at once (refused) if `--timeout`
    /// is negative, at once (`TimedOut`) if `--timeout` is exactly `0`
    /// ("check now and answer at once" - the engineer's own answer), or
    /// registered as a [`PendingWait`] otherwise, with a timer thread only
    /// when a positive `--timeout` was given. Self-enqueuing, the same
    /// shape `attach_pane` already uses - `connection.rs`'s own `"pane.
    /// wait"` arm is a single call with nothing left to build.
    pub(crate) fn register_wait(
        &self,
        description: &Description,
        context: &Context,
        request: &Request,
        waiting: Waiting,
        outbound: &Arc<Outbound>,
    ) {
        if let Err(error) = validate(description, request) {
            reply(waiting.id, Response::Error(error), outbound);
            return;
        }

        // Before anything else is touched (the engineer's own wording: a
        // negative --timeout is refused before the target is resolved or
        // anything is registered) - the same manual-range-check shape
        // `connection.rs`'s own `"tab.declare-size"` arm already uses for
        // `columns`/`rows` outside `validate` (`ValueKind::Integer`
        // declares no min/max either way).
        let timeout_secs = dispatch::param_i64(request, "timeout");
        if let Some(secs) = timeout_secs
            && secs < 0
        {
            reply(
                waiting.id,
                Response::Error(ApiError::new(
                    ErrorCode::BadRequest,
                    "a wait's own --timeout must not be negative",
                )),
                outbound,
            );
            return;
        }

        let mut state = self.lock();
        let pane = match dispatch::target_pane(&state.tree, context, request) {
            Ok(pane) => pane,
            Err(error) => {
                drop(state);
                reply(waiting.id, Response::Error(error), outbound);
                return;
            }
        };

        if let Some(ProgramStatus::Ended { exit_code }) = program_status_of(&state, pane) {
            drop(state);
            reply(waiting.id, ok_response(exit_code), outbound);
            return;
        }

        // `ProgramStatus::Running`, or (never in practice, see
        // `program_status_of`'s own doc) `None` - either way, nothing has
        // ended yet, so this is answered by `--timeout`.
        match timeout_secs {
            None => {
                state
                    .pending_waits
                    .entry(pane)
                    .or_default()
                    .push(PendingWait {
                        request_id: waiting.id,
                        client_id: waiting.client,
                        outbound: Arc::clone(outbound),
                        gate: None,
                    });
            }
            Some(0) => {
                drop(state);
                reply(waiting.id, Response::Error(timed_out(0)), outbound);
            }
            Some(secs) => {
                // `secs > 0` here: `0` and negative are both handled above.
                let gate = Arc::new(WaitGate::new());
                state
                    .pending_waits
                    .entry(pane)
                    .or_default()
                    .push(PendingWait {
                        request_id: waiting.id,
                        client_id: waiting.client,
                        outbound: Arc::clone(outbound),
                        gate: Some(Arc::clone(&gate)),
                    });
                drop(state);

                let state_handle = Arc::clone(&self.state);
                let client_id = waiting.client;
                let request_id = waiting.id;
                #[expect(
                    clippy::cast_sign_loss,
                    reason = "secs > 0 is already established by this match arm"
                )]
                let timeout = Duration::from_secs(secs as u64);
                thread::spawn(move || {
                    gate.wait(timeout);
                    timeout_wait_locked(&state_handle, pane, request_id, client_id, secs);
                });
            }
        }
    }

    /// Removes every `PendingWait` belonging to `client_id`, across every
    /// pane, waking (never replying to - the connection is already gone)
    /// each one's own timer thread, if it has one (TASK-1.140, AC #3).
    /// Called from `ConnectionCleanup::drop`, on every path out of a
    /// connection.
    pub(crate) fn cancel_waits(&self, client_id: ClientId) {
        let mut state = self.lock();
        for waits in state.pending_waits.values_mut() {
            waits.retain(|wait| {
                if wait.client_id == client_id {
                    if let Some(gate) = &wait.gate {
                        gate.resolve();
                    }
                    false
                } else {
                    true
                }
            });
        }
        state.pending_waits.retain(|_, waits| !waits.is_empty());
    }

    /// How many `PendingWait`s `pane` currently has (TASK-1.140, AC #3's
    /// own test) - `#[cfg(test)]`, the same way `attachment_count_for_test`
    /// already is: its only caller is this crate's own `#[cfg(test)] mod
    /// tests` (in `connection.rs`), which this function is only ever
    /// compiled alongside.
    #[cfg(test)]
    pub(crate) fn pending_wait_count_for_test(&self, pane: PaneId) -> usize {
        let state = self.lock();
        state.pending_waits.get(&pane).map_or(0, Vec::len)
    }
}
