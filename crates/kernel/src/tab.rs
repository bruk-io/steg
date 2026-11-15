//! A tab: an ordered page within a session.

use steg_sdk::{PaneId, PaneKind, TabId};

use crate::error::Error;
use crate::label;
use crate::pane::Pane;

/// What happened when a pane was closed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PaneClosed {
    /// The pane that was closed.
    pub pane: PaneId,
    /// Whether the tab that held it has no panes left (R94: closing a
    /// tab's last pane closes the tab). The tab is left in place, empty,
    /// for `Tree` to close.
    pub tab_now_empty: bool,
}

/// A tab: an ordered page within a session, holding panes (doc-15).
///
/// `panes` is a placeholder for TASK-1.36's layout tree (R95): a tab's
/// panes are in creation order here, not yet arranged into splits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tab {
    id: TabId,
    label: Option<String>,
    panes: Vec<Pane>,
}

impl Tab {
    /// Builds a tab with exactly one pane already in it (R94: a tab is
    /// never created empty).
    pub(crate) fn new(id: TabId, first_pane: PaneId, first_pane_kind: PaneKind) -> Self {
        Self {
            id,
            label: None,
            panes: vec![Pane::new(first_pane, first_pane_kind)],
        }
    }

    /// This tab's id.
    #[must_use]
    pub const fn id(&self) -> TabId {
        self.id
    }

    /// This tab's label, if it has one.
    #[must_use]
    pub fn label(&self) -> Option<&str> {
        self.label.as_deref()
    }

    /// Sets or clears this tab's label.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidLabel`] if `label` contains `/`, or is made
    /// only of digits (doc-2 section 4).
    pub fn set_label(&mut self, label: Option<String>) -> Result<(), Error> {
        self.label = label::validate(label)?;
        Ok(())
    }

    /// This tab's panes, in order.
    #[must_use]
    pub fn panes(&self) -> &[Pane] {
        &self.panes
    }

    /// The pane with this id, if this tab holds it.
    #[must_use]
    pub fn pane(&self, id: PaneId) -> Option<&Pane> {
        self.panes.iter().find(|pane| pane.id() == id)
    }

    /// The pane with this id, if this tab holds it.
    pub fn pane_mut(&mut self, id: PaneId) -> Option<&mut Pane> {
        self.panes.iter_mut().find(|pane| pane.id() == id)
    }

    /// Adds a new pane to this tab, already minted (`Tree` mints; this
    /// method never does).
    pub(crate) fn add_pane(&mut self, id: PaneId, kind: PaneKind) -> &mut Pane {
        self.panes.push(Pane::new(id, kind));
        self.panes
            .last_mut()
            .expect("a pane was just pushed, so the list is not empty")
    }

    /// Closes the pane with this id.
    ///
    /// # Errors
    ///
    /// Returns [`Error::PaneNotFound`] if this tab holds no pane with that
    /// id.
    pub(crate) fn close_pane(&mut self, id: PaneId) -> Result<PaneClosed, Error> {
        let position = self
            .panes
            .iter()
            .position(|pane| pane.id() == id)
            .ok_or(Error::PaneNotFound(id))?;

        self.panes.remove(position);

        Ok(PaneClosed {
            pane: id,
            tab_now_empty: self.panes.is_empty(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::Tab;
    use crate::error::Error;
    use steg_sdk::{PaneId, PaneKind, TabId};

    fn tab_with_one_pane() -> (Tab, PaneId) {
        let first = PaneId::from_raw(1);
        (
            Tab::new(TabId::from_raw(1), first, PaneKind::Terminal),
            first,
        )
    }

    #[test]
    fn closing_a_pane_that_is_not_the_only_one_leaves_the_tab_open() {
        let (mut tab, first) = tab_with_one_pane();
        let second = tab.add_pane(PaneId::from_raw(2), PaneKind::Terminal).id();

        let closed = tab.close_pane(second).unwrap();

        assert_eq!(closed.pane, second);
        assert!(!closed.tab_now_empty);
        assert_eq!(tab.panes().len(), 1);
        assert!(tab.pane(first).is_some());
    }

    #[test]
    fn closing_the_only_pane_reports_the_tab_now_empty() {
        let (mut tab, first) = tab_with_one_pane();

        let closed = tab.close_pane(first).unwrap();

        assert!(closed.tab_now_empty);
        assert!(tab.panes().is_empty());
    }

    #[test]
    fn closing_an_unknown_pane_is_an_error() {
        let (mut tab, _first) = tab_with_one_pane();
        let unknown = PaneId::from_raw(999);

        assert_eq!(tab.close_pane(unknown), Err(Error::PaneNotFound(unknown)));
    }

    #[test]
    fn set_label_changes_it() {
        let (mut tab, _first) = tab_with_one_pane();
        assert_eq!(tab.label(), None);

        tab.set_label(Some("logs".to_string())).unwrap();

        assert_eq!(tab.label(), Some("logs"));
    }

    #[test]
    fn set_label_rejects_an_invalid_label() {
        let (mut tab, _first) = tab_with_one_pane();

        assert_eq!(
            tab.set_label(Some("123".to_string())),
            Err(Error::InvalidLabel("123".to_string()))
        );
        assert_eq!(tab.label(), None);
    }
}
