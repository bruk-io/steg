//! A pane: a region of a tab running one program.

use steg_sdk::{PaneId, PaneKind};

use crate::error::Error;
use crate::label;

/// A pane: a region of a tab running one program (doc-15).
///
/// Carries only its identity, its label and its kind. A pane's program and
/// its status (TASK-1.39, TASK-1.40, TASK-1.41) and its place in the tab's
/// layout (TASK-1.36) are added to this same struct by later tasks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pane {
    id: PaneId,
    label: Option<String>,
    kind: PaneKind,
}

impl Pane {
    pub(crate) fn new(id: PaneId, kind: PaneKind) -> Self {
        Self {
            id,
            label: None,
            kind,
        }
    }

    /// This pane's id.
    #[must_use]
    pub const fn id(&self) -> PaneId {
        self.id
    }

    /// This pane's label, if it has one.
    #[must_use]
    pub fn label(&self) -> Option<&str> {
        self.label.as_deref()
    }

    /// What this pane runs.
    ///
    /// Nothing that reads this may assume it is `Terminal` (R12): match it
    /// without a wildcard arm, so a new kind fails the build everywhere it
    /// matters.
    #[must_use]
    pub const fn kind(&self) -> PaneKind {
        self.kind
    }

    /// Sets or clears this pane's label.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidLabel`] if `label` contains `/`, or is made
    /// only of digits (doc-2 section 4).
    pub fn set_label(&mut self, label: Option<String>) -> Result<(), Error> {
        self.label = label::validate(label)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::Pane;
    use steg_sdk::{PaneId, PaneKind};

    #[test]
    fn a_new_pane_has_its_kind_and_no_label() {
        let pane = Pane::new(PaneId::from_raw(1), PaneKind::Terminal);

        assert_eq!(pane.kind(), PaneKind::Terminal);
        assert_eq!(pane.label(), None);
    }

    #[test]
    fn set_label_changes_it() {
        let mut pane = Pane::new(PaneId::from_raw(1), PaneKind::Terminal);

        pane.set_label(Some("logs".to_string())).unwrap();

        assert_eq!(pane.label(), Some("logs"));
    }

    #[test]
    fn set_label_rejects_an_invalid_label() {
        let mut pane = Pane::new(PaneId::from_raw(1), PaneKind::Terminal);

        assert_eq!(
            pane.set_label(Some("123".to_string())),
            Err(crate::error::Error::InvalidLabel("123".to_string()))
        );
        assert_eq!(pane.label(), None);
    }
}
