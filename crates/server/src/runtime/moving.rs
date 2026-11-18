//! `pane.move` (TASK-1.131: a seam not named in the task's own list,
//! added because it shares close.rs's own shape - a cascade that can
//! close a tab/session - but is a different dispatch method with its own
//! event emission; isolating it here keeps TASK-1.111's own "move a tab
//! to another session" from landing inside close.rs instead). Not named
//! `move.rs`: that word is reserved.

use std::time::Instant;

use steg_kernel::Moment;
use steg_sdk::{Description, Request, Response};

use super::tree::session_owning_tab;
use super::{Runtime, State, broadcast, reapply_every_sized_tab};
use crate::dispatch::{self, Context, Outcome};

impl Runtime {
    /// `pane.move` (TASK-1.46, found one review round after `close_and_
    /// dispatch` was built the same way): `Tree::move_pane_to_tab` can
    /// cascade into closing the pane's own source tab and, if that was
    /// the session's last tab, the source session too (R94, the same
    /// cascade `close_pane` has) - the generic `_` arm in `handle` above
    /// has no way to read that cascade back out (it only ever sees a
    /// `Response`, already erased to a `Value`), so this method, like
    /// `close_and_dispatch`, calls `dispatch_raw` directly for the typed
    /// `Outcome::PaneMoved` and emits from it.
    pub(super) fn move_and_dispatch(
        &self,
        description: &Description,
        context: &Context,
        request: &Request,
    ) -> Response {
        use steg_sdk::TreeEvent;

        let mut state = self.lock();

        // Captured before the move can cascade the source tab (and its
        // session) away - the same reason `capture_close_pre` reads its
        // own scope before `dispatch_raw` runs.
        let pane = match dispatch::target_pane(&state.tree, context, request) {
            Ok(pane) => pane,
            Err(error) => return Response::Error(error),
        };
        let source_tab = state
            .tree
            .tab_of_pane(pane)
            .expect("target_pane just resolved this pane; it is in the tree");
        let source_session = session_owning_tab(&state.tree, source_tab);

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
        let outcome = match outcome {
            Ok(outcome) => outcome,
            Err(error) => return Response::Error(error),
        };

        let Outcome::PaneMoved(moved) = &outcome else {
            unreachable!("pane.move always produces Outcome::PaneMoved on success")
        };
        // Innermost first, same rule as every close path: the tab before
        // the session it was the last tab of.
        if let Some(tab) = moved.source_tab_closed {
            broadcast(&state, &TreeEvent::TabClosed { id: tab }, source_session);
        }
        if let Some(session) = moved.source_session_closed {
            broadcast(
                &state,
                &TreeEvent::SessionClosed { id: session },
                source_session,
            );
        }

        let response = Response::Ok(outcome.into_value());
        reapply_every_sized_tab(&mut state);
        response
    }
}
