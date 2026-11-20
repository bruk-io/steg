//! Input and declared size (TASK-1.131 seam: "input and declared size").

use std::time::Instant;

use steg_kernel::{Moment, SizeChanged};
use steg_sdk::{
    ApiError, ClientId, Description, ErrorCode, PaneId, Request, Response, Size, validate,
};

use super::attach::map_pane_error;
use super::{Runtime, State};
use crate::dispatch::{self, Context};
use crate::pane::CAPTURE_TIMEOUT;

impl Runtime {
    /// `pane.send` (TASK-1.90, AC #1): validates and resolves the target
    /// through `dispatch::dispatch_raw`'s own pure, I/O-free arm
    /// (`dispatch::pane::send`), under the lock, then - only on success -
    /// delivers the bytes to the pane's own worker. The same shape as
    /// `start_and_dispatch`/`close_and_dispatch`/`move_and_dispatch`, for
    /// the same reason they exist: `pane.send` has no kernel-tree effect
    /// at all (unlike those three), only a `PaneHandle` effect dispatch's
    /// own `&mut Tree` signature can never reach - `dispatch_raw`'s role
    /// here is purely the validation/refusal half (target resolution,
    /// `text`-and-`key`-together, `ProgramNotRunning`).
    pub(super) fn send_and_dispatch(
        &self,
        description: &Description,
        context: &Context,
        request: &Request,
    ) -> Response {
        let mut state = self.lock();
        let outcome = {
            // See `handle`'s own comment: one explicit deref so the two
            // `&mut` arguments below project to disjoint fields of a
            // plain `&mut State`.
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
        if let Err(error) = outcome {
            return Response::Error(error);
        }

        // Re-resolves the same target `dispatch_raw` just validated,
        // still under the one lock it used - cheaper than threading a
        // `PaneId` out of `Outcome`, which carries nothing for this
        // method (`Returns::Nothing`).
        let Ok(pane) = dispatch::target_pane(&state.tree, context, request) else {
            // Never in practice: `dispatch_raw` just resolved this exact
            // target successfully, under this same lock, with nothing in
            // between that could make it fail now.
            return Response::Ok(serde_json::Value::Null);
        };

        // Deliberately not `Tree::note_input`: this is the unattached,
        // one-shot path (AC #1 - "without a client attached"), and
        // `note_input`'s own `ClientNotViewingPane` precondition would
        // refuse a client that is not viewing the pane anyway. No
        // "latest client"/size reapplication and no status-clearing
        // happen for this call.
        if let Some(bytes) = send_bytes(request)
            && let Some(handle) = state.panes.get(&pane)
        {
            // A plain, non-blocking `mpsc::Sender::send` - never waits on
            // the pane's own worker thread, whatever it is doing.
            let _ = handle.send_input(bytes);
        }

        Response::Ok(serde_json::Value::Null)
    }
}

/// The bytes [`Runtime::send_and_dispatch`] delivers, from `text` or
/// `key` - `dispatch::pane::send`'s own arm already refused both being
/// given at once, so this never has to choose between them. `None` when
/// neither is given (nothing to send - still a success: doc-2 names no
/// refusal for this, and a well-behaved CLI always supplies one or the
/// other, reading standard input into `text` when neither was given on
/// the command line) or when `key` somehow names nothing
/// [`steg_sdk::named_key_bytes`] maps (never in practice once `key`'s own
/// `ValueKind::Choice` has already refused an unlisted name during
/// `validate`, inside `dispatch_raw`, above).
fn send_bytes(request: &Request) -> Option<Vec<u8>> {
    if let Some(text) = dispatch::param_str(request, "text") {
        return Some(text.as_bytes().to_vec());
    }
    if let Some(key) = dispatch::param_str(request, "key") {
        return steg_sdk::named_key_bytes(key, false);
    }
    None
}

impl Runtime {
    /// `pane.capture` (TASK-1.139, AC #1/#2): validates and resolves the
    /// target through `dispatch::dispatch_raw`'s own pure, I/O-free arm
    /// (`dispatch::pane::capture` - target resolution only, deliberately
    /// no "must be running" refusal), clones the target's own
    /// `CaptureSender` under the same lock, then releases the lock before
    /// reading the terminal on the pane's own worker thread - the same
    /// hand-off shape `Runtime::attach_pane` already uses (resolve and
    /// clone a sender all under one lock acquisition, then call the
    /// worker unlocked), but bounded (`CAPTURE_TIMEOUT`), where attach's
    /// own wait is deliberately not: attach must answer with the pane's
    /// real screen, with no stream to recover staleness on afterward,
    /// where capture has no such constraint and AC #2 asks for a bound by
    /// name, so a pane whose worker is busy delays only its own capture.
    pub(super) fn capture_and_dispatch(
        &self,
        description: &Description,
        context: &Context,
        request: &Request,
    ) -> Response {
        let (capture_sender, scrollback) = {
            let mut state = self.lock();
            let outcome = {
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
            if let Err(error) = outcome {
                return Response::Error(error);
            }

            // Re-resolves the same target `dispatch_raw` just validated,
            // still under the one lock it used - the same reasoning
            // `send_and_dispatch` gives.
            let Ok(pane) = dispatch::target_pane(&state.tree, context, request) else {
                // Never in practice: see `send_and_dispatch`'s own
                // identical comment.
                return Response::Ok(serde_json::Value::Null);
            };
            let Some(handle) = state.panes.get(&pane) else {
                // Every pane in the tree has a worker, started in the
                // same step it was minted (the same reasoning `attach_
                // pane`'s own doc gives for the identical defensive
                // check) - never in practice, not a case doc-2 names.
                return Response::Error(ApiError::new(
                    ErrorCode::NoSuchTarget,
                    "this pane has no running worker to capture",
                ));
            };
            let scrollback = dispatch::param_bool(request, "scrollback").unwrap_or(false);
            (handle.capture_sender(), scrollback)
        };

        match capture_sender.capture(scrollback, CAPTURE_TIMEOUT) {
            Ok(text) => Response::Ok(serde_json::Value::String(text)),
            Err(error) => Response::Error(map_pane_error(error)),
        }
    }
}

impl Runtime {
    /// Applies a kernel [`SizeChanged`] to every pane it names (TASK-1.76
    /// §4) - the seam a later task's own `client.*` methods call directly
    /// once they declare a size or change who is latest (decision-6),
    /// rather than reapplying it themselves. Self-locking, for a caller
    /// that does not already hold this `Runtime`'s lock; [`Runtime::handle`]
    /// itself calls the already-locked [`apply_size_changed_locked`]
    /// instead, to avoid re-locking a `Mutex` it is already holding.
    pub fn apply_size_changed(&self, change: &SizeChanged) {
        let mut state = self.lock();
        apply_size_changed_locked(&mut state, change);
    }

    /// Bytes `client` sent on an attachment's own stream (TASK-1.114, AC
    /// #1): `Tree::note_input` makes `client` the new latest in `pane`'s
    /// session (R91's other half of "latest" - decision-6) and reapplies
    /// whatever size it last declared, if that now differs from the tab's
    /// current size; then the bytes themselves are handed to the pane's
    /// worker. Self-locking, the same shape as [`Runtime::arm_attachment`].
    ///
    /// Never blocks on the program: `PaneHandle::send_input` only enqueues
    /// onto the pane's own unbounded command channel (a non-blocking
    /// `mpsc::Sender::send`) - the pty write itself happens later, on that
    /// pane's own worker thread, not this one. `Tree::note_input`'s `Err`
    /// (`ClientNotFound`/`ClientNotViewingPane`) is never reached in
    /// practice - `connection.rs`'s own `stream_panes` only ever names a
    /// pane this connection's `client` is currently attached to - but is
    /// tolerated the same way `detach_pane`/`forget_attachment` already
    /// tolerate a stale pane: there is nothing sane to do about a state
    /// inconsistency here except not fail the bytes over it.
    pub(crate) fn send_input(&self, client: ClientId, pane: PaneId, bytes: Vec<u8>) {
        let mut state = self.lock();
        if let Ok(Some(change)) = state.tree.note_input(client, pane) {
            apply_size_changed_locked(&mut state, &change);
        }
        if let Some(handle) = state.panes.get(&pane) {
            let _ = handle.send_input(bytes);
        }
    }

    /// `tab.declare-size` (TASK-1.114, AC #2): `client` declares its own
    /// size for `tab`, in whole cells. `columns`/`rows` are already
    /// range-checked by `connection.rs` (both `1..=u16::MAX`) before this
    /// is ever called - the kernel's own `Tree::declare_size` applies
    /// whatever size it is given with no floor of its own (R100's last
    /// clause: a window may shrink a pane under R100's 20x4 floor, cropped
    /// rather than refused), so a degenerate `0` has to be refused before
    /// it ever reaches the kernel or the engine, not after.
    ///
    /// # Errors
    ///
    /// Whatever `validate`/`dispatch::target_tab`/`Tree::declare_size`
    /// refuse with (`BadRequest`, `NoSuchTarget`, `AmbiguousTarget`, or -
    /// `Tree::declare_size`'s own `ClientNotViewingTab` - `BadRequest`,
    /// via [`dispatch::map_kernel_error`]).
    pub(crate) fn declare_size(
        &self,
        description: &Description,
        context: &Context,
        request: &Request,
        client: ClientId,
        columns: u16,
        rows: u16,
    ) -> Result<(), ApiError> {
        validate(description, request)?;
        let mut state = self.lock();
        let tab = dispatch::target_tab(&state.tree, context, request)?;
        let change = state
            .tree
            .declare_size(client, tab, Size::new(columns, rows))
            .map_err(dispatch::map_kernel_error)?;
        apply_size_changed_locked(&mut state, &change);
        Ok(())
    }
}

/// The core of [`Runtime::apply_size_changed`] (§4), taking an already-
/// locked `State`.
pub(super) fn apply_size_changed_locked(state: &mut State, change: &SizeChanged) {
    for &(pane, size) in &change.panes {
        if let Some(handle) = state.panes.get(&pane) {
            let _ = handle.resize(size.columns(), size.rows());
        }
    }
}

/// `Runtime::handle`'s general step (§4): after any successful request,
/// reapplies every currently-sized tab's sizes to its panes - cheap
/// (empty today, since no `client` dispatch method exists to ever
/// declare one), and correct regardless of which verb just ran, since it
/// does not need to know which ones can change a layout.
pub(super) fn reapply_every_sized_tab(state: &mut State) {
    for tab in state.tree.sized_tabs() {
        if let Some(change) = state.tree.tab_sizes_now(tab) {
            apply_size_changed_locked(state, &change);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc;
    use std::time::Duration;

    use steg_sdk::Size;

    use super::Runtime;
    use crate::pty::RealLauncher;
    use crate::runtime::testing::{
        TEST_GRACE, TEST_TICK, fresh_state_with_one_pane, login, test_base_env,
    };

    #[test]
    fn apply_size_changed_resizes_only_the_panes_the_change_names() {
        let (mut state, pane) = fresh_state_with_one_pane();
        let (events_tx, _events_rx) = mpsc::channel();
        let handle = crate::pane::start(
            crate::pty::LaunchSpec {
                argv: vec!["sh".to_string(), "-c".to_string(), "sleep 1".to_string()],
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
        state.panes.insert(pane, handle);

        let change = steg_kernel::SizeChanged {
            tab: state.tree.sessions()[0].tabs()[0].id(),
            size: Size::new(40, 10),
            panes: vec![(pane, Size::new(40, 10))],
            viewers: Vec::new(),
        };
        super::apply_size_changed_locked(&mut state, &change);

        let screen = state.panes[&pane].snapshot().unwrap();
        assert_eq!(screen.rows.len(), 10);
    }

    /// Review finding 2: the plan's own §4 proof promised `pane.split`,
    /// `pane.resize`, `pane.close` and a swap, each driven through
    /// `Runtime::handle`, with both the terminal and the pty checked
    /// after each - and that the test would actually fail without the
    /// general step. `pane.resize` is still `not_implemented_yet`
    /// (TASK-1.42's own gap, not this task's to build), so this covers
    /// `pane.split` (both directions), `pane.close` and `pane.swap`, the
    /// three of the four this task's own dispatch arms serve.
    ///
    /// Every probe pane runs a tight polling loop (`while :; do stty
    /// size; sleep 0.05; done`), not a `SIGWINCH` trap: a signal-based
    /// first attempt at this test raced the trap's own installation
    /// against the one-time automatic resize the general step performs,
    /// which is a real, reproducible flake (recorded on this task) a
    /// polling loop inside the child does not have, since `stty size`
    /// only ever reads the pty's *current* winsize, with nothing to miss.
    ///
    /// The terminal side's row count is read via `Screen::rows.len()`;
    /// `Screen` has no column-count field at all (confirmed: an empty
    /// row is one zero-length run, carrying no width information), so
    /// the terminal's own column width is instead proven once, directly,
    /// right after the first resize, by printing exactly the expected
    /// number of characters and confirming none of them wrap to a
    /// second row - the same `Terminal::resize` call path every later
    /// step in this test also goes through, so this is not re-proven at
    /// every step, only established once.
    #[test]
    fn the_general_sizing_step_reaches_every_pane_through_split_close_and_swap() {
        use steg_sdk::{ClientKind, Request, SessionId, Size, TabId};

        let runtime = Runtime::new(
            std::sync::Arc::new(RealLauncher::default()),
            login(),
            TEST_GRACE,
            TEST_TICK,
        );
        let description = steg_sdk::description();
        let no_context = crate::dispatch::Context::none();

        fn polling_program() -> Vec<&'static str> {
            vec!["sh", "-c", "while :; do stty size; sleep 0.05; done"]
        }

        /// Like `polling_program`, but prints exactly 61 `x`s once,
        /// first - used only for `first_pane`'s own program, to prove
        /// the terminal's column width directly (see this test's own
        /// module doc), before the polling loop's own output has a
        /// chance to scroll that line away.
        fn polling_program_with_column_marker() -> Vec<&'static str> {
            vec![
                "sh",
                "-c",
                "printf '%s\n' \"$(printf 'x%.0s' $(seq 1 61))\"; \
                 while :; do stty size; sleep 0.05; done",
            ]
        }

        fn screen_text(runtime: &Runtime, pane: steg_sdk::PaneId) -> String {
            let state = runtime.state.lock().unwrap();
            let screen = state.panes[&pane].snapshot().unwrap();
            screen
                .rows
                .iter()
                .map(|row| row.iter().map(|run| run.text.as_str()).collect::<String>())
                .collect::<Vec<_>>()
                .join(
                    "
",
                )
        }

        /// Polls (bounded, never a bare wait; review nit) until `pane`'s
        /// own polling loop reports exactly `(cols, rows)` - proving the
        /// pty side. There is no event to wait on instead: `PaneEvent`
        /// only ever carries `Exited`, and an event for a program's own
        /// output landing is TASK-1.46's own gap, not this task's to
        /// close, so the only alternative to polling the snapshot is
        /// blocking forever on text that may never appear.
        fn wait_for_reported_size(runtime: &Runtime, pane: steg_sdk::PaneId, expected: (u16, u16)) {
            let wanted = format!("{} {}", expected.1, expected.0);
            let start = std::time::Instant::now();
            loop {
                let text = screen_text(runtime, pane);
                if text.lines().any(|line| line.trim() == wanted) {
                    return;
                }
                assert!(
                    start.elapsed() < Duration::from_secs(20),
                    "pane {pane} never reported pty size {wanted}; last screen: {text:?}"
                );
                std::thread::sleep(Duration::from_millis(20));
            }
        }

        /// The terminal side's row count, directly.
        fn terminal_rows(runtime: &Runtime, pane: steg_sdk::PaneId) -> usize {
            let state = runtime.state.lock().unwrap();
            state.panes[&pane].snapshot().unwrap().rows.len()
        }

        /// Every pane `tab_sizes_now` currently reports for `tab`, read
        /// under the lock - the one source of truth every assertion
        /// below checks against, never a size this test computed itself.
        fn expected_sizes(
            runtime: &Runtime,
            tab: TabId,
        ) -> std::collections::HashMap<steg_sdk::PaneId, Size> {
            let state = runtime.state.lock().unwrap();
            state
                .tree
                .tab_sizes_now(tab)
                .expect("this test always declares a size before checking")
                .panes
                .into_iter()
                .collect()
        }

        // One pane, its own session.
        let first = match runtime.handle(
            &description,
            &no_context,
            &Request {
                method: "session.new".to_string(),
                params: serde_json::json!({"program": polling_program_with_column_marker()})
                    .as_object()
                    .unwrap()
                    .clone(),
            },
        ) {
            steg_sdk::Response::Ok(value) => value,
            steg_sdk::Response::Error(error) => panic!("session.new failed: {error:?}"),
        };
        let session_id: SessionId = first["id"].as_str().unwrap().parse().unwrap();
        let (first_pane, tab_id) = {
            let state = runtime.state.lock().unwrap();
            let tab = &state.tree.session(session_id).unwrap().tabs()[0];
            (tab.panes()[0].id(), tab.id())
        };

        // Declared directly on the kernel - the only way to get a tab
        // sized at all today (no `client` dispatch method exists) - then
        // applied through the public `apply_size_changed` once, by hand,
        // exactly as a future `client.*` dispatch method would: nothing
        // about *this* first application goes through the general step,
        // since nothing has gone through `Runtime::handle` yet to trigger
        // it. Everything from here on on does.
        let initial_change = {
            let mut state = runtime.state.lock().unwrap();
            let client = state.tree.connect_client(ClientKind::App, None);
            state.tree.view_pane(client, first_pane).unwrap();
            state
                .tree
                .declare_size(client, tab_id, Size::new(61, 20))
                .unwrap()
        };
        runtime.apply_size_changed(&initial_change);

        // The terminal's own column width, proven once, checked first
        // and with a short bound: the polling loop's own output scrolls
        // this marker line away within about a second (20 rows at one
        // line every 50ms), so this has to win that race, not the other
        // way around. Bounded sleep-polling, not an event wait, for the
        // same reason as `wait_for_reported_size` above: no event exists
        // for a snapshot's text changing.
        {
            let start = std::time::Instant::now();
            loop {
                let text = screen_text(&runtime, first_pane);
                if let Some(row) = text.lines().find(|l| l.starts_with('x')) {
                    assert_eq!(
                        row.trim_end(),
                        "x".repeat(61),
                        "61 columns must fit exactly"
                    );
                    break;
                }
                assert!(
                    start.elapsed() < Duration::from_millis(800),
                    "never saw the 61-column marker row before it would have scrolled away"
                );
                std::thread::sleep(Duration::from_millis(5));
            }
        }

        wait_for_reported_size(&runtime, first_pane, (61, 20));
        assert_eq!(terminal_rows(&runtime, first_pane), 20);

        let context = crate::dispatch::Context {
            default_session: Some(session_id),
            default_tab: Some(tab_id),
            default_pane: Some(first_pane),
        };

        // Split right: two panes.
        let split_right = match runtime.handle(
            &description,
            &context,
            &Request {
                method: "pane.split".to_string(),
                params: serde_json::json!({"direction": "right", "program": polling_program()})
                    .as_object()
                    .unwrap()
                    .clone(),
            },
        ) {
            steg_sdk::Response::Ok(value) => value,
            steg_sdk::Response::Error(error) => panic!("pane.split (right) failed: {error:?}"),
        };
        let second_pane: steg_sdk::PaneId = split_right["id"].as_str().unwrap().parse().unwrap();

        let expected = expected_sizes(&runtime, tab_id);
        assert_eq!(expected.len(), 2, "two panes after one split");
        for &pane in &[first_pane, second_pane] {
            let size = expected[&pane];
            wait_for_reported_size(&runtime, pane, (size.columns(), size.rows()));
            assert_eq!(terminal_rows(&runtime, pane), usize::from(size.rows()));
        }

        // Split the first pane down too: three panes.
        let split_down = match runtime.handle(
            &description,
            &crate::dispatch::Context {
                default_session: Some(session_id),
                default_tab: Some(tab_id),
                default_pane: Some(first_pane),
            },
            &Request {
                method: "pane.split".to_string(),
                params: serde_json::json!({"direction": "down", "program": polling_program()})
                    .as_object()
                    .unwrap()
                    .clone(),
            },
        ) {
            steg_sdk::Response::Ok(value) => value,
            steg_sdk::Response::Error(error) => panic!("pane.split (down) failed: {error:?}"),
        };
        let third_pane: steg_sdk::PaneId = split_down["id"].as_str().unwrap().parse().unwrap();

        let expected = expected_sizes(&runtime, tab_id);
        assert_eq!(expected.len(), 3, "three panes after the second split");
        for &pane in &[first_pane, second_pane, third_pane] {
            let size = expected[&pane];
            wait_for_reported_size(&runtime, pane, (size.columns(), size.rows()));
            assert_eq!(terminal_rows(&runtime, pane), usize::from(size.rows()));
        }

        // Close the third pane: its space returns to the first (R95).
        match runtime.handle(
            &description,
            &context,
            &Request {
                method: "pane.close".to_string(),
                params: serde_json::json!({
                    "target": third_pane.to_string(),
                    "force": true,
                })
                .as_object()
                .unwrap()
                .clone(),
            },
        ) {
            steg_sdk::Response::Ok(_) => {}
            steg_sdk::Response::Error(error) => panic!("pane.close failed: {error:?}"),
        }

        let expected = expected_sizes(&runtime, tab_id);
        assert_eq!(expected.len(), 2, "back to two panes after the close");
        for &pane in &[first_pane, second_pane] {
            let size = expected[&pane];
            wait_for_reported_size(&runtime, pane, (size.columns(), size.rows()));
            assert_eq!(terminal_rows(&runtime, pane), usize::from(size.rows()));
        }
        let before_swap = expected;

        // Swap the two remaining panes.
        match runtime.handle(
            &description,
            &context,
            &Request {
                method: "pane.swap".to_string(),
                params: serde_json::json!({"with": second_pane.to_string()})
                    .as_object()
                    .unwrap()
                    .clone(),
            },
        ) {
            steg_sdk::Response::Ok(_) => {}
            steg_sdk::Response::Error(error) => panic!("pane.swap failed: {error:?}"),
        }

        let after_swap = expected_sizes(&runtime, tab_id);
        assert_eq!(after_swap.len(), 2);
        assert_ne!(
            after_swap, before_swap,
            "the swap must actually change which pane has which size,              or this test cannot tell a correct swap from a no-op"
        );
        for &pane in &[first_pane, second_pane] {
            let size = after_swap[&pane];
            wait_for_reported_size(&runtime, pane, (size.columns(), size.rows()));
            assert_eq!(terminal_rows(&runtime, pane), usize::from(size.rows()));
        }
    }

    /// R100's own last clause: a tab shrinking through a later
    /// declaration can push a pane under the 20x4 floor, refusing
    /// nothing and cropping instead - proven through `apply_size_changed`
    /// (the public method), to show `Runtime` applies whatever the
    /// kernel computes faithfully, with no floor logic of its own that
    /// could get it wrong.
    #[test]
    fn apply_size_changed_lets_a_pane_go_under_the_floor_when_the_kernel_says_so() {
        let (mut state, pane) = fresh_state_with_one_pane();
        let (events_tx, _events_rx) = mpsc::channel();
        let handle = crate::pane::start(
            crate::pty::LaunchSpec {
                argv: vec!["sh".to_string(), "-c".to_string(), "sleep 100".to_string()],
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
        state.panes.insert(pane, handle);

        let tab_id = state.tree.sessions()[0].tabs()[0].id();
        let client = state.tree.connect_client(steg_sdk::ClientKind::App, None);
        state.tree.view_pane(client, pane).unwrap();
        // Below R100's 20x4 floor on purpose - `declare_size` allows this
        // "when a window grows smaller" (R100's own last clause), and
        // `tab_sizes_now` reports exactly that, unclamped.
        let change = state
            .tree
            .declare_size(client, tab_id, Size::new(5, 1))
            .unwrap();
        assert_eq!(change.panes, vec![(pane, Size::new(5, 1))]);

        super::apply_size_changed_locked(&mut state, &change);

        let screen = state.panes[&pane].snapshot().unwrap();
        assert_eq!(
            screen.rows.len(),
            1,
            "the terminal was cropped, not refused"
        );
    }

    /// AC #2: `capture_and_dispatch` releases `Runtime`'s state lock
    /// before waiting on the pane's own worker, so a pane whose worker is
    /// busy delays only its own capture, never an unrelated request on a
    /// healthy pane - modelled directly on `runtime::attach::tests::
    /// attach_pane_on_a_busy_panes_own_worker_does_not_stall_an_
    /// unrelated_requests_lock` (that test's own doc has the full
    /// reasoning for the flood size: 70MB x 15 sends, the number that
    /// file's own worker found necessary - a smaller flood let the
    /// worker catch up before the real check ran, passing for the wrong
    /// reason, so this reuses the already-measured number rather than
    /// re-guessing it).
    ///
    /// Negative control, run by hand against this exact test (not
    /// committed as a toggle): editing `capture_and_dispatch` to call
    /// `CaptureSender::capture` while still holding `state`'s lock (never
    /// releasing it first) made the assertion below fail (the unrelated
    /// `pane.capture` on the healthy pane did not answer within its
    /// bound) - recorded on the task; reverted before this test was
    /// committed.
    #[test]
    fn capture_and_dispatch_on_a_busy_panes_own_worker_does_not_stall_an_unrelated_requests_lock() {
        use std::sync::Arc;
        use std::thread;

        use steg_sdk::Request;

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

        // Wedges the busy pane's own worker inside a real `write_all`
        // (see this test's own doc for the flood size's own provenance).
        {
            let state = runtime.state.lock().unwrap();
            let handle = state.panes.get(&busy_pane).unwrap();
            let chunk = vec![b'x'; 70 * 1024 * 1024];
            for _ in 0..15 {
                let _ = handle.send_input(chunk.clone());
            }
        }

        // The wedged pane's own capture, on its own thread - never
        // joined (there is no bound on how long a program that never
        // reads its own stdin keeps a `write_all` blocked), abandoned
        // once this test's own process ends.
        let stuck_runtime = Arc::clone(&runtime);
        let stuck_description = description.clone();
        let stuck_context = context;
        let stuck_request = Request {
            method: "pane.capture".to_string(),
            params: serde_json::json!({"target": busy_target})
                .as_object()
                .unwrap()
                .clone(),
        };
        thread::spawn(move || {
            let _ = stuck_runtime.handle(&stuck_description, &stuck_context, &stuck_request);
        });

        // The real check: an unrelated `pane.capture`, on the healthy
        // pane, run on its own thread so a real bug (the lock held
        // across the wait) hangs this thread, not the test's own,
        // answered through a bounded `recv_timeout`.
        let capture_request = Request {
            method: "pane.capture".to_string(),
            params: serde_json::json!({"target": other_target})
                .as_object()
                .unwrap()
                .clone(),
        };
        let (capture_tx, capture_rx) = mpsc::channel();
        let capture_runtime = Arc::clone(&runtime);
        let capture_description = description.clone();
        let capture_context = context;
        thread::spawn(move || {
            let response =
                capture_runtime.handle(&capture_description, &capture_context, &capture_request);
            let _ = capture_tx.send(response);
        });
        let response = capture_rx
            .recv_timeout(Duration::from_secs(2))
            .unwrap_or_else(|_| {
                panic!(
                    "an unrelated pane.capture must answer within a bound even while a \
                     different pane's own worker is stuck on a real write_all"
                )
            });
        assert!(
            matches!(response, steg_sdk::Response::Ok(_)),
            "{response:?}"
        );
    }
}
