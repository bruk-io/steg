//! The tree: every session, tab and pane a server owns (doc-15, R85).

use steg_sdk::{Direction, PaneId, PaneKind, Ratio, SessionId, Size, TabId};

use crate::colour::next_colour;
use crate::error::Error;
use crate::ids::Ids;
use crate::label;
use crate::session::{Session, TabClosed};
use crate::tab::Tab;

/// What happened when a pane was closed, and what else closed because of
/// it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClosedPane {
    /// The pane that was closed.
    pub pane: PaneId,
    /// The tab that was also closed, if closing the pane left it empty
    /// (R94).
    pub tab: Option<TabId>,
    /// The session that was also closed, if closing the tab left it empty
    /// (R94).
    pub session: Option<SessionId>,
}

/// What happened when a pane moved to another tab (R9).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MovedPane {
    /// The pane that moved.
    pub pane: PaneId,
    /// The tab it moved to.
    pub to: TabId,
    /// The source tab that was also closed, if the move left it empty
    /// (R94) — the same cascade `close_pane` runs.
    pub source_tab_closed: Option<TabId>,
    /// The source session that was also closed, if closing the source
    /// tab left it empty (R94).
    pub source_session_closed: Option<SessionId>,
}

/// The tree: every session, tab and pane a server owns.
///
/// This is the whole structure doc-15 calls "a server's tree" (R85) and
/// later tasks call simply "the tree" (TASK-1.46's title).
#[derive(Debug)]
pub struct Tree {
    ids: Ids,
    sessions: Vec<Session>,
}

impl Default for Tree {
    fn default() -> Self {
        Self::new()
    }
}

impl Tree {
    /// An empty tree, with no sessions.
    #[must_use]
    pub fn new() -> Self {
        Self {
            ids: Ids::new(),
            sessions: Vec::new(),
        }
    }

    /// Every session in the tree, in order.
    #[must_use]
    pub fn sessions(&self) -> &[Session] {
        &self.sessions
    }

    /// The session with this id, if the tree holds it.
    #[must_use]
    pub fn session(&self, id: SessionId) -> Option<&Session> {
        self.sessions.iter().find(|session| session.id() == id)
    }

    /// The session with this id, if the tree holds it.
    pub fn session_mut(&mut self, id: SessionId) -> Option<&mut Session> {
        self.sessions.iter_mut().find(|session| session.id() == id)
    }

    /// Creates a new session, with one tab and one pane already in it
    /// (R93, R94), and the next colour not in use (R26, R109).
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidLabel`] if `label` is invalid (doc-2 section
    /// 4). The label is validated before anything is minted, so a failed
    /// call never consumes an id (see `new_tab`'s comment below).
    ///
    /// # Panics
    ///
    /// Never in practice: `label` was already validated above, so setting
    /// it on the session just built cannot fail.
    pub fn new_session(&mut self, label: Option<String>) -> Result<SessionId, Error> {
        let label = label::validate(label)?;

        let colour = next_colour(self.sessions.iter().map(Session::colour));

        let id = self.ids.next_session();
        let first_tab = self.ids.next_tab();
        let first_pane = self.ids.next_pane();

        let mut session = Session::new(id, colour, first_tab, first_pane, PaneKind::Terminal);
        session.set_label(label).expect("already validated above");
        self.sessions.push(session);

        Ok(id)
    }

    /// Ends a session outright. There is no cascade above a session.
    ///
    /// # Errors
    ///
    /// Returns [`Error::SessionNotFound`] if the tree holds no session
    /// with that id.
    pub fn close_session(&mut self, id: SessionId) -> Result<(), Error> {
        let position = self
            .sessions
            .iter()
            .position(|session| session.id() == id)
            .ok_or(Error::SessionNotFound(id))?;

        self.sessions.remove(position);

        Ok(())
    }

    fn session_id_owning_tab(&self, id: TabId) -> Option<SessionId> {
        self.sessions
            .iter()
            .find(|session| session.tab(id).is_some())
            .map(Session::id)
    }

    fn session_owning_tab_mut(&mut self, id: TabId) -> Option<&mut Session> {
        self.sessions
            .iter_mut()
            .find(|session| session.tab(id).is_some())
    }

    /// Closes the tab with this id, ending its session too if that tab
    /// was its last (R94).
    ///
    /// # Errors
    ///
    /// Returns [`Error::TabNotFound`] if no session in the tree holds a
    /// tab with that id.
    ///
    /// # Panics
    ///
    /// Never in practice: the session this just found to hold `id` cannot
    /// have been removed from the tree in between, since nothing else runs
    /// while this method holds `&mut self`.
    pub fn close_tab(&mut self, id: TabId) -> Result<TabClosed, Error> {
        let session = self
            .session_owning_tab_mut(id)
            .ok_or(Error::TabNotFound(id))?;
        let session_id = session.id();
        let closed = session.close_tab(id)?;

        if closed.session_now_empty {
            self.close_session(session_id)
                .expect("the session that just reported itself empty is still in the tree");
        }

        Ok(closed)
    }

    fn tab_owning_pane_mut(&mut self, id: PaneId) -> Option<&mut Tab> {
        self.sessions
            .iter_mut()
            .flat_map(Session::tabs_mut)
            .find(|tab| tab.pane(id).is_some())
    }

    fn tab_by_id_mut(&mut self, id: TabId) -> Option<&mut Tab> {
        self.sessions
            .iter_mut()
            .flat_map(Session::tabs_mut)
            .find(|tab| tab.id() == id)
    }

    /// Closes the pane with this id, cascading up through its tab and its
    /// session if either was left with nothing in it (R94). This is a
    /// decision function: it returns what closed, and does none of the
    /// I/O that turns that into an event (R38) — that is the server's job.
    ///
    /// # Errors
    ///
    /// Returns [`Error::PaneNotFound`] if no tab in the tree holds a pane
    /// with that id.
    ///
    /// # Panics
    ///
    /// Never in practice: the tab this just found to hold `id` cannot have
    /// gone anywhere in between, since nothing else runs while this method
    /// holds `&mut self`.
    pub fn close_pane(&mut self, id: PaneId) -> Result<ClosedPane, Error> {
        let tab_id = self
            .tab_owning_pane_mut(id)
            .ok_or(Error::PaneNotFound(id))?
            .id();

        // Captured before any cascade can remove the session that holds it.
        let session_id = self.session_id_owning_tab(tab_id);

        let tab = self
            .tab_owning_pane_mut(id)
            .expect("just found this pane's tab above");
        let pane_closed = tab.close_pane(id)?;

        if !pane_closed.tab_now_empty {
            return Ok(ClosedPane {
                pane: pane_closed.pane,
                tab: None,
                session: None,
            });
        }

        let tab_closed = self
            .close_tab(tab_id)
            .expect("the tab that just reported itself empty is still in the tree");

        Ok(ClosedPane {
            pane: pane_closed.pane,
            tab: Some(tab_closed.tab),
            session: if tab_closed.session_now_empty {
                session_id
            } else {
                None
            },
        })
    }

    /// Creates a new tab in the session with this id, with one pane
    /// already in it (R94).
    ///
    /// # Errors
    ///
    /// Returns [`Error::SessionNotFound`] if the tree holds no session
    /// with that id, or [`Error::InvalidLabel`] if `label` is invalid
    /// (doc-2 section 4). The label is validated before anything is
    /// minted, same as the target-exists check below, so neither kind of
    /// failure ever consumes an id.
    ///
    /// # Panics
    ///
    /// Never in practice: `label` was already validated above, so setting
    /// it on the tab just added cannot fail.
    pub fn new_tab(
        &mut self,
        session_id: SessionId,
        label: Option<String>,
    ) -> Result<TabId, Error> {
        let label = label::validate(label)?;

        // Check the target exists before minting anything: an id only
        // ever names something actually being created, never a failed
        // call (step 10 of the plan). Destructuring `self` into its
        // fields, rather than calling `self.session_mut(...)`, is what
        // lets `ids` be minted from while `session` (borrowed from
        // `sessions`) is still held below.
        let Self { ids, sessions } = self;
        let session = sessions
            .iter_mut()
            .find(|session| session.id() == session_id)
            .ok_or(Error::SessionNotFound(session_id))?;

        let tab_id = ids.next_tab();
        let pane_id = ids.next_pane();

        let tab = session.add_tab(tab_id, pane_id, PaneKind::Terminal);
        tab.set_label(label).expect("already validated above");

        Ok(tab_id)
    }

    /// Moves the tab with this id to a new position among its session's
    /// other tabs (R8: "reordered").
    ///
    /// # Errors
    ///
    /// Returns [`Error::TabNotFound`] if no session in the tree holds a
    /// tab with that id, or [`Error::PositionOutOfRange`] if
    /// `to_position` is not a valid index among its siblings.
    pub fn move_tab(&mut self, id: TabId, to_position: usize) -> Result<(), Error> {
        let session = self
            .session_owning_tab_mut(id)
            .ok_or(Error::TabNotFound(id))?;
        session.move_tab(id, to_position)
    }

    /// Splits `target`'s leaf, in `direction`, with a newly-minted pane
    /// of this `kind` on `direction`'s side, and makes the new pane the
    /// tab's current pane (AC #1).
    ///
    /// # Errors
    ///
    /// Returns [`Error::PaneNotFound`] if no tab in the tree holds a pane
    /// with that id, or [`Error::SplitTooSmall`] if either resulting
    /// pane would be under R100's floor (only possible once the tab's
    /// size is known — [`Tree::set_tab_size`]).
    ///
    /// # Panics
    ///
    /// Never in practice: the tab this just found to hold `target`
    /// cannot have gone anywhere in between, since nothing else runs
    /// while this method holds `&mut self`.
    pub fn split_pane(
        &mut self,
        target: PaneId,
        direction: Direction,
        kind: PaneKind,
    ) -> Result<PaneId, Error> {
        // Check the target exists, and that the split clears the floor,
        // before minting anything (see `new_tab`'s comment): a refused
        // split must consume no id.
        if self.tab_owning_pane_mut(target).is_none() {
            return Err(Error::PaneNotFound(target));
        }

        let Self { ids, sessions } = self;
        let tab = sessions
            .iter_mut()
            .flat_map(Session::tabs_mut)
            .find(|tab| tab.pane(target).is_some())
            .expect("just found this pane's tab above");

        let new_id = ids.next_pane();
        tab.split_pane(target, direction, new_id, kind)?;

        Ok(new_id)
    }

    /// Changes the ratio of the split bordering `pane` on `direction`'s
    /// side (AC #3), clamping at R100's floor rather than refusing.
    /// Returns the ratio actually applied.
    ///
    /// # Errors
    ///
    /// Returns [`Error::PaneNotFound`] if no tab in the tree holds a pane
    /// with that id, or [`Error::NoSplitInDirection`] if it has no split
    /// bordering it on that side.
    pub fn resize_pane(
        &mut self,
        pane: PaneId,
        direction: Direction,
        ratio: Ratio,
    ) -> Result<Ratio, Error> {
        let tab = self
            .tab_owning_pane_mut(pane)
            .ok_or(Error::PaneNotFound(pane))?;
        tab.resize_pane(pane, direction, ratio)
    }

    /// Exchanges the positions of two panes in the same tab's layout
    /// (AC #3).
    ///
    /// # Errors
    ///
    /// Returns [`Error::PaneNotFound`] if either pane is in no tab, or
    /// [`Error::PanesNotInSameTab`] if they are in different tabs.
    ///
    /// # Panics
    ///
    /// Never in practice: the tab this just found to hold `a` cannot
    /// have gone anywhere in between, since nothing else runs while this
    /// method holds `&mut self`.
    pub fn swap_panes(&mut self, a: PaneId, b: PaneId) -> Result<(), Error> {
        let tab_a = self.tab_owning_pane_mut(a).ok_or(Error::PaneNotFound(a))?;
        if tab_a.pane(b).is_none() {
            // `b` might still be a real pane, just in a different tab;
            // that's `PanesNotInSameTab`, not `PaneNotFound`.
            if self.tab_owning_pane_mut(b).is_none() {
                return Err(Error::PaneNotFound(b));
            }
            return Err(Error::PanesNotInSameTab { a, b });
        }

        let tab = self
            .tab_owning_pane_mut(a)
            .expect("just confirmed both panes are in this tab");
        tab.swap_panes(a, b)
    }

    /// Sets which pane fills its tab (R96), replacing any previous fill.
    ///
    /// # Errors
    ///
    /// Returns [`Error::PaneNotFound`] if no tab in the tree holds a pane
    /// with that id.
    pub fn fill_pane(&mut self, pane: PaneId) -> Result<(), Error> {
        let tab = self
            .tab_owning_pane_mut(pane)
            .ok_or(Error::PaneNotFound(pane))?;
        tab.fill_pane(pane)
    }

    /// Clears a tab's fill, if it has one.
    ///
    /// # Errors
    ///
    /// Returns [`Error::TabNotFound`] if the tree holds no tab with that
    /// id.
    pub fn unfill_pane(&mut self, tab: TabId) -> Result<(), Error> {
        let tab = self.tab_by_id_mut(tab).ok_or(Error::TabNotFound(tab))?;
        tab.unfill();
        Ok(())
    }

    /// Stores a tab's current size in whole cells (AC #6: unconditional
    /// — no floor check, ever; a shrink "refuses and closes nothing").
    ///
    /// # Errors
    ///
    /// Returns [`Error::TabNotFound`] if the tree holds no tab with that
    /// id.
    pub fn set_tab_size(&mut self, tab: TabId, size: Size) -> Result<(), Error> {
        let tab = self.tab_by_id_mut(tab).ok_or(Error::TabNotFound(tab))?;
        tab.set_size(size);
        Ok(())
    }

    /// Moves `pane` out of its own tab and into `target_tab`, splitting
    /// `target_tab`'s current pane in `direction` to make room for it
    /// (R9: "moved to another tab"), becoming `target_tab`'s new current
    /// pane. The source tab (and, cascading, its session) closes if
    /// `pane` was its last, exactly as [`Tree::close_pane`]'s cascade
    /// does. Atomic: the floor is checked in `target_tab` *before*
    /// anything is removed from the source tab, so a refusal here leaves
    /// both tabs untouched.
    ///
    /// # Errors
    ///
    /// Returns [`Error::PaneNotFound`] if `pane` is in no tab,
    /// [`Error::TabNotFound`] if `target_tab` doesn't exist,
    /// [`Error::AlreadyInTab`] if `target_tab` is already `pane`'s own
    /// tab, or [`Error::SplitTooSmall`] if the floor would be violated.
    ///
    /// # Panics
    ///
    /// Never in practice: the tabs this just found to hold `pane` and
    /// `target_tab` cannot have gone anywhere in between, since nothing
    /// else runs while this method holds `&mut self`.
    pub fn move_pane_to_tab(
        &mut self,
        pane: PaneId,
        target_tab: TabId,
        direction: Direction,
    ) -> Result<MovedPane, Error> {
        let source_tab_id = self
            .tab_owning_pane_mut(pane)
            .ok_or(Error::PaneNotFound(pane))?
            .id();

        if source_tab_id == target_tab {
            return Err(Error::AlreadyInTab {
                pane,
                tab: target_tab,
            });
        }

        let target = self
            .tab_by_id_mut(target_tab)
            .ok_or(Error::TabNotFound(target_tab))?;
        let target_current = target.current();
        if target.would_violate_floor_inserting(target_current, direction, pane) {
            return Err(Error::SplitTooSmall {
                pane: target_current,
            });
        }

        // Captured before any cascade can remove the session that holds
        // the source tab (mirrors `close_pane`'s own comment).
        let source_session_id = self.session_id_owning_tab(source_tab_id);

        let source_tab = self
            .tab_owning_pane_mut(pane)
            .expect("just found this pane's tab above, and nothing else ran meanwhile");
        let (removed_pane, pane_closed) = source_tab.remove_pane_for_move(pane);

        let (source_tab_closed, source_session_closed) = if pane_closed.tab_now_empty {
            let tab_closed = self
                .close_tab(source_tab_id)
                .expect("the tab that just reported itself empty is still in the tree");
            (
                Some(tab_closed.tab),
                if tab_closed.session_now_empty {
                    source_session_id
                } else {
                    None
                },
            )
        } else {
            (None, None)
        };

        let target = self
            .tab_by_id_mut(target_tab)
            .expect("confirmed present above, and nothing else ran meanwhile");
        target.insert_pane_for_move(direction, removed_pane);

        Ok(MovedPane {
            pane,
            to: target_tab,
            source_tab_closed,
            source_session_closed,
        })
    }

    /// Moves the session with this id to a new position among its
    /// siblings (R6: "reordered").
    ///
    /// # Errors
    ///
    /// Returns [`Error::SessionNotFound`] if the tree holds no session
    /// with that id, or [`Error::PositionOutOfRange`] if `to_position` is
    /// not a valid index among the sessions.
    pub fn move_session(&mut self, id: SessionId, to_position: usize) -> Result<(), Error> {
        let from = self
            .sessions
            .iter()
            .position(|session| session.id() == id)
            .ok_or(Error::SessionNotFound(id))?;

        if to_position >= self.sessions.len() {
            return Err(Error::PositionOutOfRange {
                len: self.sessions.len(),
                position: to_position,
            });
        }

        let session = self.sessions.remove(from);
        self.sessions.insert(to_position, session);

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::Tree;
    use steg_sdk::Colour;

    #[test]
    fn new_session_gives_the_first_five_sessions_distinct_colours_in_order() {
        let mut tree = Tree::new();

        let colours: Vec<Colour> = (0..5)
            .map(|_| {
                let id = tree.new_session(None).unwrap();
                tree.session(id).unwrap().colour()
            })
            .collect();

        assert_eq!(
            colours,
            vec![
                Colour::Violet,
                Colour::Pink,
                Colour::Orange,
                Colour::Blue,
                Colour::Green,
            ]
        );
    }

    #[test]
    fn the_sixth_concurrent_session_repeats_the_first_colour() {
        let mut tree = Tree::new();
        for _ in 0..5 {
            tree.new_session(None).unwrap();
        }

        let sixth = tree.new_session(None).unwrap();

        assert_eq!(tree.session(sixth).unwrap().colour(), Colour::Violet);
    }

    #[test]
    fn close_session_removes_it_directly() {
        let mut tree = Tree::new();
        let session_id = tree.new_session(None).unwrap();

        tree.close_session(session_id).unwrap();

        assert!(tree.session(session_id).is_none());
        assert!(tree.sessions().is_empty());
    }

    #[test]
    fn closing_a_sessions_only_tabs_only_pane_ends_the_session() {
        let mut tree = Tree::new();
        let session_id = tree.new_session(None).unwrap();
        let pane_id = tree.session(session_id).unwrap().tabs()[0].panes()[0].id();

        let closed = tree.close_pane(pane_id).unwrap();

        assert_eq!(closed.pane, pane_id);
        assert!(closed.tab.is_some());
        assert_eq!(closed.session, Some(session_id));
        assert!(tree.session(session_id).is_none());
        assert!(tree.sessions().is_empty());
    }

    #[test]
    fn closing_a_tabs_last_pane_closes_the_tab_without_ending_a_session_that_has_others() {
        let mut tree = Tree::new();
        let session_id = tree.new_session(None).unwrap();
        let first_tab = tree.session(session_id).unwrap().tabs()[0].id();
        let first_pane = tree.session(session_id).unwrap().tabs()[0].panes()[0].id();
        let second_tab = tree.new_tab(session_id, None).unwrap();

        let closed = tree.close_pane(first_pane).unwrap();

        assert_eq!(closed.tab, Some(first_tab));
        assert_eq!(closed.session, None);
        let session = tree.session(session_id).unwrap();
        assert_eq!(session.tabs().len(), 1);
        assert_eq!(session.tabs()[0].id(), second_tab);
    }

    #[test]
    fn closing_one_of_several_panes_cascades_no_further_than_it_should() {
        let mut tree = Tree::new();
        let session_id = tree.new_session(None).unwrap();
        let first_pane = tree.session(session_id).unwrap().tabs()[0].panes()[0].id();
        let second_pane = tree
            .split_pane(
                first_pane,
                steg_sdk::Direction::Right,
                steg_sdk::PaneKind::Terminal,
            )
            .unwrap();

        let closed = tree.close_pane(second_pane).unwrap();

        assert_eq!(closed.pane, second_pane);
        assert!(closed.tab.is_none());
        assert!(closed.session.is_none());
        assert!(tree.session(session_id).is_some());
    }

    #[test]
    fn closing_one_of_several_tabs_cascades_no_further_than_it_should() {
        let mut tree = Tree::new();
        let session_id = tree.new_session(None).unwrap();
        let second_tab = tree.new_tab(session_id, None).unwrap();

        let closed = tree.close_tab(second_tab).unwrap();

        assert!(!closed.session_now_empty);
        assert!(tree.session(session_id).is_some());
    }

    #[test]
    fn new_tab_adds_a_labelled_tab_with_one_pane_already_in_it() {
        let mut tree = Tree::new();
        let session_id = tree.new_session(None).unwrap();

        let tab_id = tree.new_tab(session_id, Some("logs".to_string())).unwrap();

        let session = tree.session(session_id).unwrap();
        assert_eq!(session.tabs().len(), 2);
        let tab = session.tab(tab_id).unwrap();
        assert_eq!(tab.label(), Some("logs"));
        assert_eq!(tab.panes().len(), 1);
    }

    #[test]
    fn a_failed_new_tab_does_not_consume_an_id() {
        let mut tree = Tree::new();
        let session_id = tree.new_session(None).unwrap(); // mints raw ids 0 (session), 1 (tab), 2 (pane)
        let unknown_session = steg_sdk::SessionId::from_raw(999);

        assert!(tree.new_tab(unknown_session, None).is_err());

        // Had the failed call minted a tab id and a pane id before
        // checking, this would mint raw 5, not 3.
        let tab_id = tree.new_tab(session_id, None).unwrap();
        assert_eq!(tab_id.into_raw(), 3);
    }

    #[test]
    fn a_failed_split_pane_does_not_consume_an_id() {
        let mut tree = Tree::new();
        let session_id = tree.new_session(None).unwrap(); // mints raw ids 0, 1, 2
        let first_pane = tree.session(session_id).unwrap().tabs()[0].panes()[0].id();
        let unknown_pane = steg_sdk::PaneId::from_raw(999);

        assert!(
            tree.split_pane(
                unknown_pane,
                steg_sdk::Direction::Right,
                steg_sdk::PaneKind::Terminal
            )
            .is_err()
        );

        // Had the failed call minted a pane id before checking, this
        // would mint raw 4, not 3.
        let pane_id = tree
            .split_pane(
                first_pane,
                steg_sdk::Direction::Right,
                steg_sdk::PaneKind::Terminal,
            )
            .unwrap();
        assert_eq!(pane_id.into_raw(), 3);
    }

    #[test]
    fn split_pane_adds_a_pane_to_an_existing_tab() {
        let mut tree = Tree::new();
        let session_id = tree.new_session(None).unwrap();
        let first_pane = tree.session(session_id).unwrap().tabs()[0].panes()[0].id();

        let pane_id = tree
            .split_pane(
                first_pane,
                steg_sdk::Direction::Right,
                steg_sdk::PaneKind::Terminal,
            )
            .unwrap();

        let tab_id = tree.session(session_id).unwrap().tabs()[0].id();
        let tab = tree.session(session_id).unwrap().tab(tab_id).unwrap();
        assert_eq!(tab.panes().len(), 2);
        assert!(tab.pane(pane_id).is_some());
    }

    #[test]
    fn move_tab_reorders_a_sessions_tabs() {
        let mut tree = Tree::new();
        let session_id = tree.new_session(None).unwrap();
        let first_tab = tree.session(session_id).unwrap().tabs()[0].id();
        let second_tab = tree.new_tab(session_id, None).unwrap();

        tree.move_tab(second_tab, 0).unwrap();

        let session = tree.session(session_id).unwrap();
        assert_eq!(session.tabs()[0].id(), second_tab);
        assert_eq!(session.tabs()[1].id(), first_tab);
    }

    #[test]
    fn session_mut_lets_a_caller_change_a_sessions_label_and_colour() {
        let mut tree = Tree::new();
        let session_id = tree.new_session(None).unwrap();

        let session = tree.session_mut(session_id).unwrap();
        session.set_label(Some("work".to_string())).unwrap();
        session.set_colour(Colour::Green);

        let session = tree.session(session_id).unwrap();
        assert_eq!(session.label(), Some("work"));
        assert_eq!(session.colour(), Colour::Green);
    }

    #[test]
    fn new_session_sets_the_label_given_to_it() {
        let mut tree = Tree::new();

        let session_id = tree.new_session(Some("work".to_string())).unwrap();

        assert_eq!(tree.session(session_id).unwrap().label(), Some("work"));
    }

    #[test]
    fn new_session_rejects_an_invalid_label_and_mints_no_id() {
        let mut tree = Tree::new();

        assert_eq!(
            tree.new_session(Some("123".to_string())),
            Err(crate::error::Error::InvalidLabel("123".to_string()))
        );
        assert!(tree.sessions().is_empty());

        // Had the failed call minted ids before validating the label,
        // this session would start at raw id 3, not 0.
        let session_id = tree.new_session(None).unwrap();
        assert_eq!(session_id.into_raw(), 0);
    }

    #[test]
    fn new_tab_rejects_an_invalid_label_and_mints_no_id() {
        let mut tree = Tree::new();
        let session_id = tree.new_session(None).unwrap(); // mints raw ids 0, 1, 2

        assert_eq!(
            tree.new_tab(session_id, Some("a/b".to_string())),
            Err(crate::error::Error::InvalidLabel("a/b".to_string()))
        );
        assert_eq!(tree.session(session_id).unwrap().tabs().len(), 1);

        // Had the failed call minted a tab id and a pane id before
        // validating the label, this would mint raw 5, not 3.
        let tab_id = tree.new_tab(session_id, None).unwrap();
        assert_eq!(tab_id.into_raw(), 3);
    }

    #[test]
    fn move_session_reorders_and_rejects_an_out_of_range_position() {
        let mut tree = Tree::new();
        let first = tree.new_session(None).unwrap();
        let second = tree.new_session(None).unwrap();

        tree.move_session(second, 0).unwrap();

        assert_eq!(tree.sessions()[0].id(), second);
        assert_eq!(tree.sessions()[1].id(), first);

        assert_eq!(
            tree.move_session(first, 5),
            Err(crate::error::Error::PositionOutOfRange {
                len: 2,
                position: 5
            })
        );
    }

    #[test]
    fn a_not_found_id_at_any_level_is_the_matching_error() {
        let mut tree = Tree::new();

        let unknown_session = steg_sdk::SessionId::from_raw(999);
        let unknown_tab = steg_sdk::TabId::from_raw(999);
        let unknown_pane = steg_sdk::PaneId::from_raw(999);

        assert_eq!(
            tree.close_session(unknown_session),
            Err(crate::error::Error::SessionNotFound(unknown_session))
        );
        assert_eq!(
            tree.close_tab(unknown_tab),
            Err(crate::error::Error::TabNotFound(unknown_tab))
        );
        assert_eq!(
            tree.close_pane(unknown_pane),
            Err(crate::error::Error::PaneNotFound(unknown_pane))
        );
        assert_eq!(
            tree.new_tab(unknown_session, None),
            Err(crate::error::Error::SessionNotFound(unknown_session))
        );
        assert_eq!(
            tree.split_pane(
                unknown_pane,
                steg_sdk::Direction::Right,
                steg_sdk::PaneKind::Terminal
            ),
            Err(crate::error::Error::PaneNotFound(unknown_pane))
        );
        assert_eq!(
            tree.move_tab(unknown_tab, 0),
            Err(crate::error::Error::TabNotFound(unknown_tab))
        );
    }

    #[test]
    fn move_pane_to_tab_leaves_both_tabs_with_a_valid_tree() {
        let mut tree = Tree::new();
        let session_id = tree.new_session(None).unwrap();
        let first_tab = tree.session(session_id).unwrap().tabs()[0].id();
        let first_pane = tree.session(session_id).unwrap().tabs()[0].panes()[0].id();
        let second_pane = tree
            .split_pane(
                first_pane,
                steg_sdk::Direction::Right,
                steg_sdk::PaneKind::Terminal,
            )
            .unwrap();
        let second_tab = tree.new_tab(session_id, None).unwrap();

        let moved = tree
            .move_pane_to_tab(second_pane, second_tab, steg_sdk::Direction::Right)
            .unwrap();

        assert_eq!(moved.pane, second_pane);
        assert_eq!(moved.to, second_tab);
        assert_eq!(moved.source_tab_closed, None);
        assert_eq!(moved.source_session_closed, None);

        let source = tree.session(session_id).unwrap().tab(first_tab).unwrap();
        assert_eq!(source.panes().len(), 1);
        assert!(source.pane(first_pane).is_some());

        let target = tree.session(session_id).unwrap().tab(second_tab).unwrap();
        assert_eq!(target.panes().len(), 2);
        assert!(target.pane(second_pane).is_some());
        assert_eq!(target.current(), second_pane);
    }

    #[test]
    fn move_pane_to_tab_cascades_closing_the_source_tab_when_it_was_the_last_pane() {
        let mut tree = Tree::new();
        let session_id = tree.new_session(None).unwrap();
        let first_tab = tree.session(session_id).unwrap().tabs()[0].id();
        let first_pane = tree.session(session_id).unwrap().tabs()[0].panes()[0].id();
        let second_tab = tree.new_tab(session_id, None).unwrap();

        let moved = tree
            .move_pane_to_tab(first_pane, second_tab, steg_sdk::Direction::Right)
            .unwrap();

        assert_eq!(moved.source_tab_closed, Some(first_tab));
        assert_eq!(moved.source_session_closed, None);
        let session = tree.session(session_id).unwrap();
        assert_eq!(session.tabs().len(), 1);
        assert_eq!(session.tabs()[0].id(), second_tab);
    }

    #[test]
    fn move_pane_to_tab_refuses_a_panes_own_tab() {
        let mut tree = Tree::new();
        let session_id = tree.new_session(None).unwrap();
        let first_tab = tree.session(session_id).unwrap().tabs()[0].id();
        let first_pane = tree.session(session_id).unwrap().tabs()[0].panes()[0].id();

        assert_eq!(
            tree.move_pane_to_tab(first_pane, first_tab, steg_sdk::Direction::Right),
            Err(crate::error::Error::AlreadyInTab {
                pane: first_pane,
                tab: first_tab,
            })
        );
    }

    #[test]
    fn move_pane_to_tab_is_refused_when_the_target_tabs_floor_would_be_violated_and_nothing_changes()
     {
        let mut tree = Tree::new();
        let session_id = tree.new_session(None).unwrap();
        let first_tab = tree.session(session_id).unwrap().tabs()[0].id();
        let first_pane = tree.session(session_id).unwrap().tabs()[0].panes()[0].id();
        let second_tab = tree.new_tab(session_id, None).unwrap();
        let second_tab_pane = tree
            .session(session_id)
            .unwrap()
            .tab(second_tab)
            .unwrap()
            .panes()[0]
            .id();
        // 39 columns halves to 19 and 19 — under the 20-column floor.
        tree.set_tab_size(second_tab, steg_sdk::Size::new(39, 10))
            .unwrap();

        // The whole of both tabs, before the refused move — `Tab`
        // derives `PartialEq`, so this one comparison covers layout,
        // pane order, current pane and fill together, not just whatever
        // a hand-picked set of assertions happened to check.
        let source_before = tree
            .session(session_id)
            .unwrap()
            .tab(first_tab)
            .unwrap()
            .clone();
        let target_before = tree
            .session(session_id)
            .unwrap()
            .tab(second_tab)
            .unwrap()
            .clone();

        let result = tree.move_pane_to_tab(first_pane, second_tab, steg_sdk::Direction::Right);

        assert_eq!(
            result,
            Err(crate::error::Error::SplitTooSmall {
                pane: second_tab_pane
            })
        );
        assert_eq!(
            tree.session(session_id).unwrap().tab(first_tab).unwrap(),
            &source_before
        );
        assert_eq!(
            tree.session(session_id).unwrap().tab(second_tab).unwrap(),
            &target_before
        );
    }

    #[test]
    fn panes_are_in_layout_order_after_a_move() {
        let mut tree = Tree::new();
        let session_id = tree.new_session(None).unwrap();
        let first_pane = tree.session(session_id).unwrap().tabs()[0].panes()[0].id();
        let second_tab = tree.new_tab(session_id, None).unwrap();
        let second_tab_first_pane = tree
            .session(session_id)
            .unwrap()
            .tab(second_tab)
            .unwrap()
            .panes()[0]
            .id();

        tree.move_pane_to_tab(first_pane, second_tab, steg_sdk::Direction::Left)
            .unwrap();

        let target = tree.session(session_id).unwrap().tab(second_tab).unwrap();
        // Direction::Left puts the moved pane first.
        assert_eq!(
            target
                .panes()
                .iter()
                .map(crate::pane::Pane::id)
                .collect::<Vec<_>>(),
            vec![first_pane, second_tab_first_pane]
        );
    }

    #[test]
    fn swap_panes_is_refused_when_the_two_panes_are_in_different_tabs() {
        let mut tree = Tree::new();
        let session_id = tree.new_session(None).unwrap();
        let first_pane = tree.session(session_id).unwrap().tabs()[0].panes()[0].id();
        let second_tab = tree.new_tab(session_id, None).unwrap();
        let second_tab_pane = tree
            .session(session_id)
            .unwrap()
            .tab(second_tab)
            .unwrap()
            .panes()[0]
            .id();

        assert_eq!(
            tree.swap_panes(first_pane, second_tab_pane),
            Err(crate::error::Error::PanesNotInSameTab {
                a: first_pane,
                b: second_tab_pane,
            })
        );
    }

    #[test]
    fn set_tab_size_smaller_always_succeeds_and_closes_nothing() {
        let mut tree = Tree::new();
        let session_id = tree.new_session(None).unwrap();
        let tab_id = tree.session(session_id).unwrap().tabs()[0].id();

        tree.set_tab_size(tab_id, steg_sdk::Size::new(5, 1))
            .unwrap();

        assert!(tree.session(session_id).is_some());
        let tab = tree.session(session_id).unwrap().tab(tab_id).unwrap();
        assert_eq!(tab.panes().len(), 1);
    }
}
