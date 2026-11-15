//! Steg's kernel: session, tab and pane state, and target resolution.
//!
//! The kernel is logic only. It takes data and returns data, and does no I/O
//! of its own (`.claude/rules/architecture.md`). It depends on the sdk,
//! without the sdk's optional parts (R105), for the shapes that cross the
//! API — identifiers, the colour set and pane kinds (`steg_sdk`) — and
//! defines its own behaviour on top of them: minting an identifier,
//! choosing a session's colour, and the tree of sessions, tabs and panes
//! with the rules between them (doc-15).
#![forbid(unsafe_code)]

mod colour;
mod error;
mod ids;
mod label;
mod pane;
mod session;
mod tab;
mod target;
mod tree;

pub use error::Error;
pub use pane::Pane;
pub use session::{Session, TabClosed};
pub use tab::{DirectionalFocus, PaneClosed, Tab};
pub use target::Candidate;
pub use tree::{ClosedPane, FocusMove, Focused, MovedPane, Tree};
