//! The tree and its lookups (TASK-1.131 seam: "the tree and its
//! lookups").

use steg_kernel::Tree;
use steg_sdk::{ApiError, Description, PaneId, Request, SessionId, validate};

use super::Runtime;
use crate::dispatch::{self, Context};

impl Runtime {
    /// Resolves `request`'s target as a pane, with no mutation at all
    /// (TASK-1.47) - `connection.rs`'s own `pane.detach` uses this to turn
    /// a target into a `PaneId` before it can look its own stream up by
    /// that id (its attachment bookkeeping is keyed by `PaneId`, not by a
    /// target string), the one place this needs to be callable apart from
    /// [`Runtime::attach_pane`], which resolves the same way internally.
    ///
    /// # Errors
    /// Whatever `validate` or `dispatch::target_pane` refuse with
    /// (`BadRequest`, `NoSuchTarget`, `AmbiguousTarget`).
    pub(crate) fn resolve_pane_target(
        &self,
        description: &Description,
        context: &Context,
        request: &Request,
    ) -> Result<PaneId, ApiError> {
        validate(description, request)?;
        let state = self.lock();
        dispatch::target_pane(&state.tree, context, request)
    }
}

/// The session that owns `tab` - the same walk `rollback_mint`'s own
/// `session.new` arm already does inline, pulled out so the creation
/// -events helper below can use it for all three starting methods, not
/// only that one.
pub(super) fn session_owning_tab(tree: &Tree, tab: steg_sdk::TabId) -> SessionId {
    tree.sessions()
        .iter()
        .find(|session| session.tab(tab).is_some())
        .map(steg_kernel::Session::id)
        .expect("the tab just minted is still in the tree")
}
