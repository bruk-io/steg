//! What can go wrong asking the tree to do something.

use std::fmt;

use steg_sdk::{PaneId, SessionId, TabId};

use crate::target::Candidate;

/// An error from an operation on the tree.
///
/// One variant per thing a caller can tell apart (rust.md). Later kernel
/// tasks (the layout tree's split, resize and swap) are expected to add
/// their own variants here rather than defining a second error type, since
/// every one of them touches this same crate.
///
/// Not `Copy` (the former derive, kept while every variant was a plain id
/// or two `usize`s): `InvalidLabel`, `InvalidTargetPath`, `TargetNotFound`
/// and `TargetAmbiguous` each own a `String` or a `Vec`, and nothing in
/// this crate relies on `Error` being `Copy` — every use is `?`,
/// `.ok_or(...)`, `.expect(...)` or `assert_eq!`.
#[derive(Debug, Clone, PartialEq, Eq)]
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
    /// A label contained `/`, or was made only of digits (doc-2 section 4;
    /// see `label::validate`).
    InvalidLabel(String),
    /// A target path had the wrong number of `/`-separated segments, or an
    /// empty one — bad usage (doc-2 section 6's status 2), never "matched
    /// nothing" (status 3): an empty segment can never come from a real
    /// id, label, position or suffix.
    InvalidTargetPath(String),
    /// A target's part matched nothing at that level (doc-15 R10). Carries
    /// every candidate at that level, so a caller can show what it could
    /// have meant.
    TargetNotFound {
        /// The part that was tried.
        part: String,
        /// Every candidate at that level.
        candidates: Vec<Candidate>,
    },
    /// A target's part matched more than one thing at the first kind that
    /// matched at all (doc-15 R10: "An ambiguous target is an error, never
    /// a guess"). Carries only the candidates that matched.
    TargetAmbiguous {
        /// The part that was tried.
        part: String,
        /// The candidates that matched.
        candidates: Vec<Candidate>,
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
            Error::InvalidLabel(label) => {
                write!(
                    f,
                    "{label:?} is not a valid label: a label may not contain '/' or be made only of digits"
                )
            }
            Error::InvalidTargetPath(path) => {
                write!(f, "{path:?} is not a valid target path")
            }
            Error::TargetNotFound { part, candidates } => {
                write!(
                    f,
                    "{part:?} matches nothing among {} candidate(s)",
                    candidates.len()
                )
            }
            Error::TargetAmbiguous { part, candidates } => {
                write!(
                    f,
                    "{part:?} matches {} candidates, not one",
                    candidates.len()
                )
            }
        }
    }
}

impl std::error::Error for Error {}
