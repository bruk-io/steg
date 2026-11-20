//! Attach and arm (TASK-1.131 seam: "attach and arm").

use std::sync::Arc;
use std::time::Instant;

use steg_kernel::Moment;
use steg_sdk::{
    ApiError, ClientId, Connection, Description, ErrorCode, PaneId, Request, Response, validate,
};

use super::{Runtime, Subscriber, apply_size_changed_locked};
use crate::dispatch::{self, Context};
use crate::outbound::Outbound;
use crate::pane;
#[cfg(test)]
use crate::pane::PaneHandle;

/// The three identifying values one `pane.attach` call needs beyond its
/// own request/description/context (TASK-1.47) - grouped only so
/// [`Runtime::attach_pane`] stays under clippy's own argument-count lint,
/// not to claim these three belong together for any deeper reason.
pub(crate) struct Attaching {
    /// The connection attaching.
    pub(crate) client: ClientId,
    /// The request's own id, to match the reply.
    pub(crate) id: u64,
    /// The new attachment's stream id, already minted by the caller.
    pub(crate) stream: u32,
}

impl Runtime {
    /// `pane.attach` (TASK-1.47; AC #1, #7): resolves the target, attaches
    /// the kernel's own `Client` to it (`Tree::view_pane`), reads the
    /// pane's current screen and registers the attachment in its worker's
    /// own fan-out list, then builds and enqueues the reply itself - the
    /// same shape TASK-1.46's own `Runtime::subscribe` uses, not a
    /// `Response` handed back for `connection.rs` to enqueue. Returns the
    /// resolved `PaneId` on success so the caller can finish decision-2
    /// section 3's ordering rule (`Outbound::open_stream`, then
    /// [`Runtime::arm_attachment`]) and record it in its own, per
    /// -connection attachment map.
    ///
    /// # Errors
    /// Whatever `validate`/`dispatch::target_pane`/`Tree::view_pane`
    /// refuse with, or [`ErrorCode::NoSuchTarget`] if the pane has no
    /// running worker at all (never in practice: every pane in the tree
    /// has one, started in the same step it was minted) or
    /// [`ErrorCode::EngineFailed`] if reading its screen failed. Nothing
    /// is enqueued on `Err`: the caller builds and sends the ordinary
    /// error reply itself.
    pub(crate) fn attach_pane(
        &self,
        description: &Description,
        context: &Context,
        attaching: Attaching,
        request: &Request,
        outbound: &Arc<crate::outbound::Outbound>,
    ) -> Result<(PaneId, pane::AttachmentId), ApiError> {
        let Attaching { client, id, stream } = attaching;
        validate(description, request)?;
        // Resolves the target and clones a sender for its worker's own
        // command channel, all under the lock, then releases it before
        // `AttachSender::attach` ever waits on a reply (TASK-1.47
        // review): `PaneHandle::attach`'s own wait is unbounded (it must
        // answer with the pane's real screen, not a timeout's worth of a
        // stale one), so calling it while still holding this `Runtime`'s
        // one state lock would stall *every* other request on the server
        // behind a single busy pane's own worker - proven by
        // `attach_pane_on_a_busy_panes_own_worker_does_not_stall_an_
        // unrelated_requests_lock`, below.
        let (pane, attach_sender) = {
            let mut state = self.lock();
            let pane = dispatch::target_pane(&state.tree, context, request)?;
            state
                .tree
                .view_pane(client, pane)
                .map_err(dispatch::map_kernel_error)?;
            let handle = state.panes.get(&pane).ok_or_else(|| {
                ApiError::new(
                    ErrorCode::NoSuchTarget,
                    "this pane has no running program to attach to",
                )
            })?;
            (pane, handle.attach_sender())
        };
        let (attachment_id, screen) = attach_sender.attach(stream).map_err(map_pane_error)?;
        let attached = steg_sdk::PaneAttached {
            stream,
            snapshot: screen,
        };
        let value =
            serde_json::to_value(&attached).expect("PaneAttached always serialises to JSON");
        let bytes = steg_sdk::Connection::reply(id, Response::Ok(value));
        // Best-effort, the same reasoning TASK-1.46's own `subscribe`
        // gives for its own reply: if this fails, the connection is
        // already tearing down concurrently, and there is nothing more to
        // do about it here.
        let _ = outbound.enqueue_control(bytes);
        Ok((pane, attachment_id))
    }

    /// Hands an already-registered attachment the `Outbound` it sends
    /// updates to (TASK-1.47) - the second half of decision-2 section 3's
    /// ordering rule, called by `connection.rs` only after it has queued
    /// the attach reply and opened the stream with the writer. Self
    /// -locking, a no-op if `pane` no longer has a worker at all - never
    /// a stall-the-server hazard the way [`Runtime::attach_pane`]'s own
    /// wait was (TASK-1.47 review): `PaneHandle::arm` only sends on an
    /// unbounded `mpsc` channel, which returns the moment the message is
    /// queued, regardless of how backed up the worker reading it is -
    /// it never waits for a reply, so holding the lock across it costs
    /// nothing more than the lock already costs for any other request.
    ///
    /// Returns whether an attachment was actually armed - `false` for
    /// the race the same review found releasing `attach_pane`'s own
    /// lock opened: the pane (or its whole tab) can close in the now
    /// -unlocked window between `attach_pane` cloning this worker's own
    /// `AttachSender` and this call running, in which case `attach_pane`
    /// already replied with a real snapshot (truthfully - the pane was
    /// still there when it was read) and the worker already registered
    /// the attachment, but there is no `Outbound` left to ever send it
    /// anything: the worker's own copy sits inert until that worker
    /// -thread's own, unrelated shutdown (closing a pane ends its
    /// worker) drops it, never swept by anything attachment-specific.
    /// The caller must not record this in its own bookkeeping as a live
    /// attachment when this returns `false` - see `connection.rs`'s own
    /// `"pane.attach"` arm for what it does instead.
    pub(crate) fn arm_attachment(
        &self,
        pane: PaneId,
        attachment_id: pane::AttachmentId,
        outbound: Arc<crate::outbound::Outbound>,
    ) -> bool {
        let state = self.lock();
        match state.panes.get(&pane) {
            Some(handle) => handle.arm(attachment_id, outbound).is_ok(),
            None => false,
        }
    }

    /// `pane.detach` (TASK-1.47): releases the kernel's own viewing claim
    /// (`Tree::stop_viewing`), reapplying any `SizeChanged` it reports
    /// (decision-6: detaching can change who is latest), and tells the
    /// pane's own worker to drop the attachment. Takes an already-resolved
    /// `pane` and the caller's own `attachment_id` (`connection.rs` looks
    /// both up from its own bookkeeping before calling this - see
    /// [`Runtime::resolve_pane_target`]) rather than a `Request`, since
    /// "is this connection even attached to this pane" is connection
    /// -local state the kernel and this `Runtime` know nothing about. The
    /// worker's own `AttachmentId` (TASK-1.114), not the connection's own
    /// stream number, is what identifies the attachment to end - see
    /// `crate::pane::AttachmentId`'s own doc for why the stream number
    /// alone is not safe once a second connection can attach the same
    /// pane.
    ///
    /// Best-effort on both halves: `stop_viewing` failing (the client
    /// already gone, or never really viewing this pane) and the pane
    /// having no worker left are both silently tolerated - the caller
    /// already confirmed its own attachment was real before calling this,
    /// so either case only means the state this was meant to release is
    /// already gone. Like [`Runtime::arm_attachment`] (TASK-1.47 review),
    /// `PaneHandle::detach`'s own send never waits on the worker - this
    /// never risks the stall [`Runtime::attach_pane`]'s own wait did.
    pub(crate) fn detach_pane(
        &self,
        client: ClientId,
        pane: PaneId,
        attachment_id: pane::AttachmentId,
    ) {
        let mut state = self.lock();
        if let Ok(changes) = state.tree.stop_viewing(client, pane) {
            for change in &changes {
                apply_size_changed_locked(&mut state, change);
            }
        }
        if let Some(handle) = state.panes.get(&pane) {
            let _ = handle.detach(attachment_id);
        }
    }

    /// Tells `pane`'s own worker thread to drop one attachment, with no
    /// kernel interaction at all (TASK-1.47) - used only when a connection
    /// ends without having called `pane.detach` for every attachment it
    /// still held (`connection.rs`'s own `ConnectionCleanup`):
    /// `Tree::disconnect_client` already removes that connection's whole
    /// kernel-level `Client` row in one step, viewing list included, so
    /// calling `Tree::stop_viewing` here as well would at best be a no-op
    /// and at worst race whichever of the two cleanup calls runs first.
    /// Only the pane worker's own in-memory fan-out list, which the kernel
    /// knows nothing about, still needs telling. Same reasoning as
    /// [`Runtime::detach_pane`] (TASK-1.47 review): a non-waiting send,
    /// never a stall hazard.
    pub(crate) fn forget_attachment(&self, pane: PaneId, attachment_id: pane::AttachmentId) {
        let state = self.lock();
        if let Some(handle) = state.panes.get(&pane) {
            let _ = handle.detach(attachment_id);
        }
    }

    /// How many attachments `pane`'s own worker currently holds
    /// (TASK-1.47 review): test-only, reaching `PaneHandle::
    /// attachment_count` (itself `#[cfg(test)]`) for a connection.rs test
    /// that proves `ConnectionCleanup`'s own sweep, not merely this
    /// method, is what returns the count to what it was. `0` if `pane`
    /// has no running worker at all.
    #[cfg(test)]
    pub(crate) fn attachment_count_for_test(&self, pane: PaneId) -> usize {
        let state = self.lock();
        state
            .panes
            .get(&pane)
            .map_or(0, PaneHandle::attachment_count)
    }

    /// Registers a connection as eligible for broadcast (TASK-1.46 §7):
    /// called once, by `connection::handle`'s own `run_post_handshake`,
    /// right after its writer (`outbound`) is spawned and before the read
    /// loop that could bring in `event.watch` even starts - so a
    /// connection is never asked to subscribe before it is registered
    /// (`subscribe`'s own `.expect` relies on this). `filter: None` until
    /// `event.watch` runs: a registered-but-unfiltered connection receives
    /// nothing from `broadcast`, never everything.
    pub(crate) fn register_subscriber(
        &self,
        client_id: ClientId,
        outbound: Arc<Outbound>,
        shutdown: std::os::unix::net::UnixStream,
    ) {
        self.lock().subscribers.insert(
            client_id,
            Subscriber {
                outbound,
                filter: None,
                shutdown,
            },
        );
    }

    /// Deregisters a connection (TASK-1.46 §7): called on every exit path
    /// out of a connection (`connection::handle`'s own `ConnectionCleanup`
    /// guard), so `broadcast` never holds a handle to a writer that is
    /// gone. Idempotent: deregistering a connection that was never
    /// registered, or already was, does nothing further.
    pub fn deregister_subscriber(&self, client_id: ClientId) {
        self.lock().subscribers.remove(&client_id);
    }

    /// `event.watch` (AC #1, #3, #4): registers this connection's own
    /// filter and replies with the tree, atomically - both inside the one
    /// lock acquisition, so nothing broadcast by another thread between
    /// them can ever reach the wire ahead of this reply (TASK-1.46's own
    /// plan, section 4, the race its first draft missed). Bypasses the
    /// generic `Runtime::handle`/`Connection::reply`-afterward path
    /// entirely: `connection::handle` calls this directly (after
    /// resolving any `api.call` wrapping itself, so that path reaches
    /// here too) instead of calling `handle` and enqueueing its own
    /// reply - the reply is already on the wire by the time this returns
    /// `Ok`.
    ///
    /// On `Err`, nothing is registered or changed: the caller builds and
    /// enqueues the ordinary error reply itself, which needs no
    /// atomicity (nothing was broadcast as part of answering a request
    /// that never actually subscribed).
    ///
    /// # Errors
    /// The same validation and target-resolution errors `dispatch::
    /// dispatch` would give — a request `event.watch` does not declare, a
    /// malformed `for`/`all`, or no session to resolve (`resolve_filter`).
    ///
    /// # Panics
    /// Never in practice: `client_id` is only ever one `connection::
    /// handle` passed back, which always calls `register_subscriber`
    /// first - the same invariant `start_and_dispatch`'s own panics
    /// already rest on elsewhere in this file.
    pub fn subscribe(
        &self,
        description: &Description,
        context: &Context,
        request: &Request,
        client_id: ClientId,
        request_id: u64,
    ) -> Result<(), ApiError> {
        let mut state = self.lock();
        validate(description, request)?;
        let filter = dispatch::resolve_filter(&state.tree, context, request)?;
        let now = Moment::new(Instant::now().duration_since(state.clock_origin));
        let snapshot = dispatch::tree_snapshot(&state.tree, now);

        let subscriber = state.subscribers.get_mut(&client_id).expect(
            "register_subscriber already ran, before any request could arrive on this connection",
        );
        subscriber.filter = Some(filter);

        let value = serde_json::to_value(snapshot).expect("TreeSnapshot always serialises");
        let bytes = Connection::reply(request_id, Response::Ok(value));
        // A failed enqueue means this connection's queue is already
        // closed or full - it is tearing down concurrently (its own
        // `deregister_subscriber` is on its way), and there is nothing
        // further to do: the request still succeeded (the filter is set,
        // the tree was read), there is simply no one left to write the
        // reply to.
        let _ = subscriber.outbound.enqueue_control(bytes);
        Ok(())
    }
}

/// Converts this crate's own [`crate::error::Error`] (what a `PaneHandle`
/// call can fail with) into an [`ApiError`] (TASK-1.47's own
/// `PaneHandle::attach` is the first dispatch-adjacent caller that needs
/// this - no earlier task needed the conversion at all). `PaneGone` maps
/// to `NoSuchTarget`: from the caller's own point of view, a pane whose
/// worker thread is no longer there is indistinguishable from one that no
/// longer exists to attach to. Every other variant is this crate's own
/// I/O-adjacent failure: the engine, the pty, a read/write, and (TASK-1.139)
/// a bounded wait on the worker that ran out before it answered.
/// `pane.attach` never performs any of these itself, but `EngineFailed` is
/// the closest named code for any of them, named plainly rather than
/// invented. `Timeout` is deliberately not mapped to `NoSuchTarget`
/// alongside `PaneGone`: the pane is not gone, only busy, and telling a
/// caller "that target doesn't exist" for a pane that does would be a
/// false statement (see `Error::Timeout`'s own doc). `pub(super)`, not
/// private: `runtime::input`'s own `capture_and_dispatch` (TASK-1.139)
/// reuses this rather than duplicating it.
pub(super) fn map_pane_error(error: crate::error::Error) -> ApiError {
    match error {
        crate::error::Error::PaneGone => ApiError::new(ErrorCode::NoSuchTarget, error.to_string()),
        crate::error::Error::Kernel(_)
        | crate::error::Error::Pty(_)
        | crate::error::Error::Io(_)
        | crate::error::Error::Timeout => ApiError::new(ErrorCode::EngineFailed, error.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::mpsc;
    use std::thread;
    use std::time::Duration;

    use super::Runtime;
    use crate::outbound::Outbound;
    use crate::pty::RealLauncher;
    use crate::runtime::testing::{TEST_GRACE, TEST_TICK, login};

    /// TASK-1.47 review's own Important finding: `attach_pane` used to
    /// call `PaneHandle::attach` - an unbounded wait for the worker's own
    /// reply, since it must answer with the pane's real screen, never a
    /// stale one - while still holding this `Runtime`'s one state lock.
    /// A pane whose worker is busy (here: flooded with so much stdin
    /// that it blocks inside a real `write_all`, writing to a program
    /// that never reads it) held that lock hostage, which every other
    /// request on the server needs too - proven here directly on the
    /// lock, not through a real socket: the stuck attach runs on its own
    /// thread, deliberately never joined (even once fixed, there is no
    /// bound on how long a program that never reads its own stdin keeps
    /// a `write_all` blocked - only this test's own process exiting ever
    /// ends it, which abandoning the thread, rather than joining it,
    /// costs nothing for); what this asserts is that a completely
    /// unrelated request, on a healthy pane, answers within a short
    /// bound regardless.
    #[test]
    fn attach_pane_on_a_busy_panes_own_worker_does_not_stall_an_unrelated_requests_lock() {
        use steg_sdk::{ClientKind, Request};

        let runtime = Arc::new(Runtime::new(
            Arc::new(RealLauncher::default()),
            login(),
            TEST_GRACE,
            TEST_TICK,
        ));
        let description = steg_sdk::description();
        let context = crate::dispatch::Context::none();

        let busy_session = match runtime.handle(
            &description,
            &context,
            &Request {
                method: "session.new".to_string(),
                params: serde_json::json!({"program": ["sh", "-c", "exec sleep 100"]})
                    .as_object()
                    .unwrap()
                    .clone(),
            },
        ) {
            steg_sdk::Response::Ok(value) => value,
            steg_sdk::Response::Error(error) => panic!("session.new failed: {error:?}"),
        };
        let busy_session_id: steg_sdk::SessionId =
            busy_session["id"].as_str().unwrap().parse().unwrap();

        let other_session = match runtime.handle(
            &description,
            &context,
            &Request {
                method: "session.new".to_string(),
                params: serde_json::json!({"program": ["sh", "-c", "exec sleep 100"]})
                    .as_object()
                    .unwrap()
                    .clone(),
            },
        ) {
            steg_sdk::Response::Ok(value) => value,
            steg_sdk::Response::Error(error) => panic!("session.new failed: {error:?}"),
        };
        let other_target = {
            let state = runtime.state.lock().unwrap();
            let other_session_id: steg_sdk::SessionId =
                other_session["id"].as_str().unwrap().parse().unwrap();
            let tab = &state.tree.session(other_session_id).unwrap().tabs()[0];
            let pane = tab.panes()[0].id();
            format!("{other_session_id}/{}/{pane}", tab.id())
        };

        let (busy_pane, busy_target) = {
            let state = runtime.state.lock().unwrap();
            let tab = &state.tree.session(busy_session_id).unwrap().tabs()[0];
            let pane = tab.panes()[0].id();
            (pane, format!("{busy_session_id}/{}/{pane}", tab.id()))
        };

        // Floods the busy pane's own worker: `exec sleep 100` never
        // reads its stdin at all, so the pty's own kernel buffer fills
        // and the worker's `PaneCommand::Input` handler blocks inside a
        // real `write_all` - confirmed necessary by this test's own
        // negative control (reverting `attach_pane` to hold the lock
        // across its wait makes the assertion below fail, not hang: the
        // flood itself, independent of that revert, is what the stuck
        // -ness depends on). Sized generously (TASK-1.114's own worker
        // found, independently, that a much smaller flood - 70 MB - let
        // the worker catch up before the real check ran at all, passing
        // for the wrong reason) so the stuck window comfortably outlasts
        // this test's own bound, confirmed below rather than assumed.
        {
            let state = runtime.state.lock().unwrap();
            let handle = state.panes.get(&busy_pane).unwrap();
            let chunk = vec![b'x'; 70 * 1024 * 1024];
            for _ in 0..15 {
                let _ = handle.send_input(chunk.clone());
            }
        }

        let client = runtime.connect_client(ClientKind::Cli, None);
        let outbound = Arc::new(Outbound::new());
        let attach_request = Request {
            method: "pane.attach".to_string(),
            params: serde_json::json!({"target": busy_target})
                .as_object()
                .unwrap()
                .clone(),
        };
        let stuck_runtime = Arc::clone(&runtime);
        let stuck_description = description.clone();
        let stuck_context = context;
        // Signals when (if ever, within this test's own lifetime) the
        // stuck attach finally answers - not joined (there is no bound
        // on how long a program that never reads its own stdin keeps a
        // `write_all` blocked, even once this is fixed), but checked
        // below with a non-blocking `try_recv`, so this test confirms
        // the stuck attach is genuinely still unanswered at the moment
        // the real check runs, rather than assuming the flood above was
        // large enough.
        let (done_tx, done_rx) = mpsc::channel::<()>();
        thread::spawn(move || {
            let _ = stuck_runtime.attach_pane(
                &stuck_description,
                &stuck_context,
                super::Attaching {
                    client,
                    id: 1,
                    stream: 1,
                },
                &attach_request,
                &outbound,
            );
            let _ = done_tx.send(());
        });

        // `std::sync::Mutex::lock` has no bound of its own - if this
        // test called `runtime.handle` directly and the bug were real,
        // *this* call would then also block forever, hanging the test
        // instead of failing it (confirmed directly: an earlier version
        // did exactly that, found by actually running the control, not
        // assumed). Run on its own thread instead, answered through a
        // channel this test waits on with a real, bounded `recv_timeout`
        // - the one thing in `std` that can actually time out a wait on
        // another thread's own lock. (TASK-1.125 review's own nit: a
        // `done_rx` check right here, immediately after spawning, would
        // prove nothing - the spawned thread may not even have started
        // yet, so it could pass regardless of whether the flood below
        // ever stuck anything. The one check that matters is after the
        // real wait, below.)
        let show_request = Request {
            method: "pane.show".to_string(),
            params: serde_json::json!({"target": other_target})
                .as_object()
                .unwrap()
                .clone(),
        };
        let (show_tx, show_rx) = mpsc::channel();
        let show_runtime = Arc::clone(&runtime);
        let show_description = description.clone();
        let show_context = context;
        thread::spawn(move || {
            let response = show_runtime.handle(&show_description, &show_context, &show_request);
            let _ = show_tx.send(response);
        });
        let response = show_rx
            .recv_timeout(Duration::from_secs(2))
            .unwrap_or_else(|_| {
                panic!(
                    "an unrelated pane.show must answer within a bound even while a \
                 different pane's own attach is stuck on its worker"
                )
            });
        assert!(
            matches!(response, steg_sdk::Response::Ok(_)),
            "{response:?}"
        );
        assert!(
            done_rx.try_recv().is_err(),
            "the stuck attach answered during the real check - it was not actually \
             stuck, so this run proves nothing"
        );
    }

    /// TASK-1.47 review: releasing `attach_pane`'s own lock before
    /// waiting on the worker (this task's own fix for the review's
    /// Important finding) opened a window the old, lock-held code never
    /// had - the pane (or its tab) can now close between `attach_pane`
    /// cloning the worker's own `AttachSender` and `arm_attachment`
    /// running. No real race is needed to prove `arm_attachment`'s own
    /// answer in that case: calling it, by hand, after the pane is
    /// already gone, is the exact state a real race would produce.
    #[test]
    fn arm_attachment_answers_false_once_the_pane_has_already_closed() {
        use steg_sdk::{ClientKind, Request};

        let runtime = Runtime::new(
            Arc::new(RealLauncher::default()),
            login(),
            TEST_GRACE,
            TEST_TICK,
        );
        let description = steg_sdk::description();
        let context = crate::dispatch::Context::none();

        let session = match runtime.handle(
            &description,
            &context,
            &Request {
                method: "session.new".to_string(),
                params: serde_json::json!({"program": ["sh", "-c", "exec sleep 100"]})
                    .as_object()
                    .unwrap()
                    .clone(),
            },
        ) {
            steg_sdk::Response::Ok(value) => value,
            steg_sdk::Response::Error(error) => panic!("session.new failed: {error:?}"),
        };
        let (pane, target) = {
            let state = runtime.state.lock().unwrap();
            let session_id: steg_sdk::SessionId = session["id"].as_str().unwrap().parse().unwrap();
            let tab = &state.tree.session(session_id).unwrap().tabs()[0];
            let pane = tab.panes()[0].id();
            (pane, format!("{session_id}/{}/{pane}", tab.id()))
        };

        let client = runtime.connect_client(ClientKind::Cli, None);
        let outbound = Arc::new(Outbound::new());
        let attach_request = Request {
            method: "pane.attach".to_string(),
            params: serde_json::json!({"target": target.clone()})
                .as_object()
                .unwrap()
                .clone(),
        };
        let (_, attachment_id) = runtime
            .attach_pane(
                &description,
                &context,
                super::Attaching {
                    client,
                    id: 1,
                    stream: 1,
                },
                &attach_request,
                &outbound,
            )
            .expect("the pane is still there - this attach must succeed");

        // Simulates the window `attach_pane`'s own released lock opened:
        // the pane closes after a client is told it attached (the
        // reply above already went out) but before `arm_attachment`
        // ever runs - `--force`, since a shell left running counts as
        // still running from the kernel's own point of view.
        let close_request = Request {
            method: "pane.close".to_string(),
            params: serde_json::json!({"target": target, "force": true})
                .as_object()
                .unwrap()
                .clone(),
        };
        match runtime.handle(&description, &context, &close_request) {
            steg_sdk::Response::Ok(_) => {}
            steg_sdk::Response::Error(error) => panic!("pane.close failed: {error:?}"),
        }

        let armed = runtime.arm_attachment(pane, attachment_id, outbound);
        assert!(
            !armed,
            "arm_attachment must answer false once the pane it was meant for has \
             already closed, not arm a worker that no longer has anywhere to send \
             anything"
        );
    }
}
