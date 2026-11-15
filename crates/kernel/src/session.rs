//! A session: a named, long-lived workspace.

use steg_sdk::{Colour, PaneId, PaneKind, TabId};

use crate::error::Error;
use crate::label;
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
    current_tab: TabId,
}

impl Session {
    /// Builds a session with exactly one tab, holding exactly one pane
    /// already in it (R93, R94: a session is never created empty, and
    /// neither is its first tab), and that tab as its current one (R85).
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
            current_tab: first_tab,
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
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidLabel`] if `label` contains `/`, or is made
    /// only of digits (doc-2 section 4).
    pub fn set_label(&mut self, label: Option<String>) -> Result<(), Error> {
        self.label = label::validate(label)?;
        Ok(())
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

    /// This session's current tab (doc-15 R85).
    #[must_use]
    pub const fn current_tab(&self) -> TabId {
        self.current_tab
    }

    /// Makes the tab with this id this session's current tab (R85). Used
    /// directly by `add_tab` (R115, unconditionally) and, through
    /// `Tree::focus_pane`/`Tree::focus_tab`, by the explicit focus
    /// operations.
    ///
    /// # Errors
    ///
    /// Returns [`Error::TabNotFound`] if this session holds no tab with
    /// that id.
    pub(crate) fn set_current_tab(&mut self, id: TabId) -> Result<(), Error> {
        if self.tab(id).is_none() {
            return Err(Error::TabNotFound(id));
        }
        self.current_tab = id;
        Ok(())
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
    /// already in it (`Tree` mints both ids; this method never does), and
    /// makes it this session's current tab unconditionally (R115: "a new
    /// tab becomes its session's current tab"). Creating something makes
    /// it current in its own parent only — this does not touch any pane's
    /// current; `--no-focus` (doc-2 section 5) is not a parameter here, but
    /// is given by a caller putting the previous focus back afterward
    /// (R115), which is `Tree::focus_tab`'s job, not this method's.
    pub(crate) fn add_tab(
        &mut self,
        id: TabId,
        first_pane: PaneId,
        first_pane_kind: PaneKind,
    ) -> &mut Tab {
        self.tabs.push(Tab::new(id, first_pane, first_pane_kind));
        self.current_tab = id;
        self.tabs
            .last_mut()
            .expect("a tab was just pushed, so the list is not empty")
    }

    /// Closes the tab with this id. If it was this session's current tab,
    /// repairs `current_tab` to the tab that takes its place in the order,
    /// or the last tab if the closed one was last (R114). Closing any other
    /// tab leaves `current_tab` as it was (R114's second sentence). If this
    /// was the session's only tab, `current_tab` is left stale (not
    /// repaired) — `Tree::close_session` removes this session in the same
    /// call, so nothing ever reads it.
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

        if self.current_tab == id
            && let Some(replacement) = self.tabs.get(position).or_else(|| self.tabs.last())
        {
            self.current_tab = replacement.id();
        }

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
    fn a_new_session_has_its_only_tab_as_current() {
        let (session, first_tab) = session_with_one_tab();

        assert_eq!(session.current_tab(), first_tab);
    }

    #[test]
    fn adding_a_tab_makes_it_current() {
        let (mut session, first_tab) = session_with_one_tab();
        assert_eq!(session.current_tab(), first_tab);

        let second_tab = session
            .add_tab(TabId::from_raw(2), PaneId::from_raw(2), PaneKind::Terminal)
            .id();

        // R115: a new tab becomes current unconditionally.
        assert_eq!(session.current_tab(), second_tab);
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
    fn closing_a_non_current_tab_leaves_current_tab_unchanged() {
        let (mut session, first_tab) = session_with_one_tab();
        let second_tab = session
            .add_tab(TabId::from_raw(2), PaneId::from_raw(2), PaneKind::Terminal)
            .id();
        // `add_tab` just made `second_tab` current (R115); close the
        // *other* one.
        assert_eq!(session.current_tab(), second_tab);

        session.close_tab(first_tab).unwrap();

        // R114's second sentence: closing a tab that is not current
        // leaves `current_tab` exactly as it was.
        assert_eq!(session.current_tab(), second_tab);
    }

    #[test]
    fn closing_the_current_tab_among_several_moves_current_to_the_tab_that_takes_its_place() {
        let (mut session, _first_tab) = session_with_one_tab();
        let second_tab = session
            .add_tab(TabId::from_raw(2), PaneId::from_raw(2), PaneKind::Terminal)
            .id();
        let third_tab = session
            .add_tab(TabId::from_raw(3), PaneId::from_raw(3), PaneKind::Terminal)
            .id();
        // Focus the middle tab, then close it.
        session.set_current_tab(second_tab).unwrap();

        session.close_tab(second_tab).unwrap();

        // R114: the tab now at `second_tab`'s old index (1) — which is
        // `third_tab`, promoted up — becomes current.
        assert_eq!(session.current_tab(), third_tab);
    }

    #[test]
    fn closing_the_current_tab_when_it_is_last_moves_current_to_the_new_last_tab() {
        let (mut session, first_tab) = session_with_one_tab();
        let second_tab = session
            .add_tab(TabId::from_raw(2), PaneId::from_raw(2), PaneKind::Terminal)
            .id();
        // `second_tab` is already current (R115) and is also the last tab.

        session.close_tab(second_tab).unwrap();

        // R114: no tab took its old index, so the new last tab (`first_tab`,
        // the only one left) becomes current.
        assert_eq!(session.current_tab(), first_tab);
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

    #[test]
    fn move_tab_leaves_current_tab_unchanged() {
        let (mut session, first_tab) = session_with_one_tab();
        let second_tab = session
            .add_tab(TabId::from_raw(2), PaneId::from_raw(2), PaneKind::Terminal)
            .id();
        // `second_tab` is current (R115).

        session.move_tab(first_tab, 1).unwrap();

        // R114's second sentence: reordering never invalidates
        // `current_tab`, since it names a tab by id, not by position.
        assert_eq!(session.current_tab(), second_tab);
    }

    #[test]
    fn set_current_tab_rejects_an_unknown_id() {
        let (mut session, _first_tab) = session_with_one_tab();
        let unknown = TabId::from_raw(999);

        assert_eq!(
            session.set_current_tab(unknown),
            Err(Error::TabNotFound(unknown))
        );
    }

    #[test]
    fn set_label_changes_it() {
        let (mut session, _first_tab) = session_with_one_tab();
        assert_eq!(session.label(), None);

        session.set_label(Some("work".to_string())).unwrap();

        assert_eq!(session.label(), Some("work"));
    }

    #[test]
    fn set_label_rejects_an_invalid_label() {
        let (mut session, _first_tab) = session_with_one_tab();

        assert_eq!(
            session.set_label(Some("work/logs".to_string())),
            Err(Error::InvalidLabel("work/logs".to_string()))
        );
        assert_eq!(session.label(), None);
    }

    #[test]
    fn set_colour_changes_it() {
        let (mut session, _first_tab) = session_with_one_tab();
        assert_eq!(session.colour(), Colour::Violet);

        session.set_colour(Colour::Green);

        assert_eq!(session.colour(), Colour::Green);
    }
}
