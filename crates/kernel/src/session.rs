//! A session: a named, long-lived workspace.

use steg_sdk::{Colour, PaneId, PaneKind, TabId};

use crate::error::Error;
use crate::tab::Tab;

/// What happened when a tab was closed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TabClosed {
    /// The tab that was closed.
    pub tab: TabId,
    /// Whether the session that held it has no tabs left (R94: closing a
    /// session's last tab ends the session). The session is left in place,
    /// empty, for `Tree` to close.
    pub session_now_empty: bool,
}

/// A session: a named, long-lived workspace on one host (doc-15).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Session {
    id: steg_sdk::SessionId,
    label: Option<String>,
    colour: Colour,
    tabs: Vec<Tab>,
}

impl Session {
    /// Builds a session with exactly one tab, holding exactly one pane
    /// already in it (R93, R94: a session is never created empty, and
    /// neither is its first tab).
    pub(crate) fn new(
        id: steg_sdk::SessionId,
        colour: Colour,
        first_tab: TabId,
        first_pane: PaneId,
        first_pane_kind: PaneKind,
    ) -> Self {
        Self {
            id,
            label: None,
            colour,
            tabs: vec![Tab::new(first_tab, first_pane, first_pane_kind)],
        }
    }

    /// This session's id.
    #[must_use]
    pub const fn id(&self) -> steg_sdk::SessionId {
        self.id
    }

    /// This session's label, if it has one.
    #[must_use]
    pub fn label(&self) -> Option<&str> {
        self.label.as_deref()
    }

    /// Sets or clears this session's label.
    pub fn set_label(&mut self, label: Option<String>) {
        self.label = label;
    }

    /// This session's colour (R26).
    #[must_use]
    pub const fn colour(&self) -> Colour {
        self.colour
    }

    /// Changes this session's colour (AC #3: "its colour can be changed").
    pub fn set_colour(&mut self, colour: Colour) {
        self.colour = colour;
    }

    /// This session's tabs, in order.
    #[must_use]
    pub fn tabs(&self) -> &[Tab] {
        &self.tabs
    }

    /// The tab with this id, if this session holds it.
    #[must_use]
    pub fn tab(&self, id: TabId) -> Option<&Tab> {
        self.tabs.iter().find(|tab| tab.id() == id)
    }

    /// The tab with this id, if this session holds it.
    pub fn tab_mut(&mut self, id: TabId) -> Option<&mut Tab> {
        self.tabs.iter_mut().find(|tab| tab.id() == id)
    }

    /// This session's tabs, in order, mutably. For `Tree`'s own use in
    /// searching across every session for the tab that holds a pane.
    pub(crate) fn tabs_mut(&mut self) -> &mut [Tab] {
        &mut self.tabs
    }

    /// Adds a new tab to this session, already minted, with its first pane
    /// already in it (`Tree` mints both ids; this method never does).
    pub(crate) fn add_tab(
        &mut self,
        id: TabId,
        first_pane: PaneId,
        first_pane_kind: PaneKind,
    ) -> &mut Tab {
        self.tabs.push(Tab::new(id, first_pane, first_pane_kind));
        self.tabs
            .last_mut()
            .expect("a tab was just pushed, so the list is not empty")
    }

    /// Closes the tab with this id.
    ///
    /// # Errors
    ///
    /// Returns [`Error::TabNotFound`] if this session holds no tab with
    /// that id.
    pub(crate) fn close_tab(&mut self, id: TabId) -> Result<TabClosed, Error> {
        let position = self
            .tabs
            .iter()
            .position(|tab| tab.id() == id)
            .ok_or(Error::TabNotFound(id))?;

        self.tabs.remove(position);

        Ok(TabClosed {
            tab: id,
            session_now_empty: self.tabs.is_empty(),
        })
    }

    /// Moves the tab with this id to a new position among its siblings
    /// (R8: "reordered").
    ///
    /// # Errors
    ///
    /// Returns [`Error::TabNotFound`] if this session holds no tab with
    /// that id, or [`Error::PositionOutOfRange`] if `to_position` is not a
    /// valid index among the tabs.
    pub(crate) fn move_tab(&mut self, id: TabId, to_position: usize) -> Result<(), Error> {
        let from = self
            .tabs
            .iter()
            .position(|tab| tab.id() == id)
            .ok_or(Error::TabNotFound(id))?;

        if to_position >= self.tabs.len() {
            return Err(Error::PositionOutOfRange {
                len: self.tabs.len(),
                position: to_position,
            });
        }

        let tab = self.tabs.remove(from);
        self.tabs.insert(to_position, tab);

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::Session;
    use crate::error::Error;
    use steg_sdk::{Colour, PaneId, PaneKind, TabId};

    fn session_with_one_tab() -> (Session, TabId) {
        let first_tab = TabId::from_raw(1);
        (
            Session::new(
                steg_sdk::SessionId::from_raw(1),
                Colour::Violet,
                first_tab,
                PaneId::from_raw(1),
                PaneKind::Terminal,
            ),
            first_tab,
        )
    }

    #[test]
    fn closing_the_only_tab_reports_the_session_now_empty() {
        let (mut session, first_tab) = session_with_one_tab();

        let closed = session.close_tab(first_tab).unwrap();

        assert!(closed.session_now_empty);
        assert!(session.tabs().is_empty());
    }

    #[test]
    fn closing_one_of_several_tabs_leaves_the_session_open() {
        let (mut session, first_tab) = session_with_one_tab();
        let second_tab = session
            .add_tab(TabId::from_raw(2), PaneId::from_raw(2), PaneKind::Terminal)
            .id();

        let closed = session.close_tab(second_tab).unwrap();

        assert!(!closed.session_now_empty);
        assert_eq!(session.tabs().len(), 1);
        assert!(session.tab(first_tab).is_some());
    }

    #[test]
    fn move_tab_reorders_and_rejects_an_out_of_range_position() {
        let (mut session, first_tab) = session_with_one_tab();
        let second_tab = session
            .add_tab(TabId::from_raw(2), PaneId::from_raw(2), PaneKind::Terminal)
            .id();

        session.move_tab(second_tab, 0).unwrap();

        assert_eq!(session.tabs()[0].id(), second_tab);
        assert_eq!(session.tabs()[1].id(), first_tab);

        assert_eq!(
            session.move_tab(first_tab, 5),
            Err(Error::PositionOutOfRange {
                len: 2,
                position: 5
            })
        );
    }
}
