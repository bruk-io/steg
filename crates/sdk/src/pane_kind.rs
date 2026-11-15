//! What kind of thing a pane is running.

use serde::{Deserialize, Serialize};

/// What a pane runs.
///
/// A pane has a kind, and v1 ships only one: a terminal (R12). Nothing in
/// the kernel, the layout or the API may assume a pane is a terminal, so
/// this stays a plain enum — not `#[non_exhaustive]` — and adding a variant
/// forces every match on it, across every crate in this repository, to be
/// revisited (rust.md: "reach for `#[non_exhaustive]` only on types other
/// crates match on", and every consumer of this crate is this repository's
/// own).
///
/// Crosses the API as its own word (`"terminal"`), same as [`crate::Colour`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PaneKind {
    /// A pane running a terminal.
    Terminal,
}

#[cfg(test)]
mod tests {
    use super::PaneKind;

    #[test]
    fn serde_round_trips_through_its_own_word() {
        let json = serde_json::to_string(&PaneKind::Terminal).unwrap();
        assert_eq!(json, "\"terminal\"");
        assert_eq!(
            serde_json::from_str::<PaneKind>(&json).unwrap(),
            PaneKind::Terminal
        );
    }
}
