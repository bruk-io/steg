//! What can go wrong asking the tree to do something.

use std::fmt;

use steg_sdk::{PaneId, SessionId, TabId};

/// An error from an operation on the tree.
///
/// One variant per thing a caller can tell apart (rust.md). Later kernel
/// tasks (the layout tree's split, resize and swap; target resolution's
/// ambiguous and not-found target) are expected to add their own variants
/// here rather than defining a second error type, since every one of them
/// touches this same crate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    /// No session in the tree has this id.
    SessionNotFound(SessionId),
    /// No tab in the tree has this id.
    TabNotFound(TabId),
    /// No pane in the tree has this id.
    PaneNotFound(PaneId),
    /// A move or reorder was asked to place something at a position beyond
    /// the end of its siblings.
    PositionOutOfRange {
        /// How many siblings there are to place it among.
        len: usize,
        /// The position that was asked for.
        position: usize,
    },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::SessionNotFound(id) => write!(f, "no session with id {id}"),
            Error::TabNotFound(id) => write!(f, "no tab with id {id}"),
            Error::PaneNotFound(id) => write!(f, "no pane with id {id}"),
            Error::PositionOutOfRange { len, position } => {
                write!(f, "position {position} is out of range for {len} siblings")
            }
        }
    }
}

impl std::error::Error for Error {}
