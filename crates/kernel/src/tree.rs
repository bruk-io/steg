//! The tree: every session, tab and pane a server owns (doc-15, R85).

use steg_sdk::{PaneId, PaneKind, SessionId, TabId};

use crate::colour::next_colour;
use crate::error::Error;
use crate::ids::Ids;
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
    pub fn new_session(&mut self, label: Option<String>) -> SessionId {
        let colour = next_colour(self.sessions.iter().map(Session::colour));

        let id = self.ids.next_session();
        let first_tab = self.ids.next_tab();
        let first_pane = self.ids.next_pane();

        let mut session = Session::new(id, colour, first_tab, first_pane, PaneKind::Terminal);
        session.set_label(label);
        self.sessions.push(session);

        id
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
    /// with that id.
    pub fn new_tab(
        &mut self,
        session_id: SessionId,
        label: Option<String>,
    ) -> Result<TabId, Error> {
        let tab_id = self.ids.next_tab();
        let pane_id = self.ids.next_pane();

        let session = self
            .session_mut(session_id)
            .ok_or(Error::SessionNotFound(session_id))?;
        let tab = session.add_tab(tab_id, pane_id, PaneKind::Terminal);
        tab.set_label(label);

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

    /// Adds a new pane to the tab with this id.
    ///
    /// # Errors
    ///
    /// Returns [`Error::TabNotFound`] if no session in the tree holds a
    /// tab with that id.
    ///
    /// # Panics
    ///
    /// Never in practice: the session this just found to hold `tab_id`
    /// still holds it on the next line, since nothing else runs while this
    /// method holds `&mut self`.
    pub fn new_pane(&mut self, tab_id: TabId, kind: PaneKind) -> Result<PaneId, Error> {
        let pane_id = self.ids.next_pane();

        let session = self
            .session_owning_tab_mut(tab_id)
            .ok_or(Error::TabNotFound(tab_id))?;
        let tab = session
            .tab_mut(tab_id)
            .expect("session_owning_tab_mut only returns a session that holds this tab");
        tab.add_pane(pane_id, kind);

        Ok(pane_id)
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
                let id = tree.new_session(None);
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
            tree.new_session(None);
        }

        let sixth = tree.new_session(None);

        assert_eq!(tree.session(sixth).unwrap().colour(), Colour::Violet);
    }

    #[test]
    fn closing_a_sessions_only_tabs_only_pane_ends_the_session() {
        let mut tree = Tree::new();
        let session_id = tree.new_session(None);
        let pane_id = tree.session(session_id).unwrap().tabs()[0].panes()[0].id();

        let closed = tree.close_pane(pane_id).unwrap();

        assert_eq!(closed.pane, pane_id);
        assert!(closed.tab.is_some());
        assert_eq!(closed.session, Some(session_id));
        assert!(tree.session(session_id).is_none());
        assert!(tree.sessions().is_empty());
    }

    #[test]
    fn closing_one_of_several_panes_cascades_no_further_than_it_should() {
        let mut tree = Tree::new();
        let session_id = tree.new_session(None);
        let tab_id = tree.session(session_id).unwrap().tabs()[0].id();
        let second_pane = tree.new_pane(tab_id, steg_sdk::PaneKind::Terminal).unwrap();

        let closed = tree.close_pane(second_pane).unwrap();

        assert_eq!(closed.pane, second_pane);
        assert!(closed.tab.is_none());
        assert!(closed.session.is_none());
        assert!(tree.session(session_id).is_some());
    }

    #[test]
    fn closing_one_of_several_tabs_cascades_no_further_than_it_should() {
        let mut tree = Tree::new();
        let session_id = tree.new_session(None);
        let second_tab = tree.new_tab(session_id, None).unwrap();

        let closed = tree.close_tab(second_tab).unwrap();

        assert!(!closed.session_now_empty);
        assert!(tree.session(session_id).is_some());
    }

    #[test]
    fn new_tab_adds_a_labelled_tab_with_one_pane_already_in_it() {
        let mut tree = Tree::new();
        let session_id = tree.new_session(None);

        let tab_id = tree.new_tab(session_id, Some("logs".to_string())).unwrap();

        let session = tree.session(session_id).unwrap();
        assert_eq!(session.tabs().len(), 2);
        let tab = session.tab(tab_id).unwrap();
        assert_eq!(tab.label(), Some("logs"));
        assert_eq!(tab.panes().len(), 1);
    }

    #[test]
    fn new_pane_adds_a_pane_to_an_existing_tab() {
        let mut tree = Tree::new();
        let session_id = tree.new_session(None);
        let tab_id = tree.session(session_id).unwrap().tabs()[0].id();

        let pane_id = tree.new_pane(tab_id, steg_sdk::PaneKind::Terminal).unwrap();

        let tab = tree.session(session_id).unwrap().tab(tab_id).unwrap();
        assert_eq!(tab.panes().len(), 2);
        assert!(tab.pane(pane_id).is_some());
    }

    #[test]
    fn move_tab_reorders_a_sessions_tabs() {
        let mut tree = Tree::new();
        let session_id = tree.new_session(None);
        let first_tab = tree.session(session_id).unwrap().tabs()[0].id();
        let second_tab = tree.new_tab(session_id, None).unwrap();

        tree.move_tab(second_tab, 0).unwrap();

        let session = tree.session(session_id).unwrap();
        assert_eq!(session.tabs()[0].id(), second_tab);
        assert_eq!(session.tabs()[1].id(), first_tab);
    }

    #[test]
    fn move_session_reorders_and_rejects_an_out_of_range_position() {
        let mut tree = Tree::new();
        let first = tree.new_session(None);
        let second = tree.new_session(None);

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
            tree.new_pane(unknown_tab, steg_sdk::PaneKind::Terminal),
            Err(crate::error::Error::TabNotFound(unknown_tab))
        );
        assert_eq!(
            tree.move_tab(unknown_tab, 0),
            Err(crate::error::Error::TabNotFound(unknown_tab))
        );
    }
}
