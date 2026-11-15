//! What kind of thing a pane is running.

/// What a pane runs.
///
/// A pane has a kind, and v1 ships only one: a terminal (R12). Nothing in
/// the kernel, the layout or the API may assume a pane is a terminal, so
/// this stays a plain enum — not `#[non_exhaustive]` — and adding a variant
/// forces every match on it, across every crate in this repository, to be
/// revisited (rust.md: "reach for `#[non_exhaustive]` only on types other
/// crates match on", and every consumer of this crate is this repository's
/// own).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PaneKind {
    /// A pane running a terminal.
    Terminal,
}
