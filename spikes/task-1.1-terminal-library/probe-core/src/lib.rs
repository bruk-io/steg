//! The parts of the TASK-1.1 spike that must be identical for both candidates: the backend
//! contract, the pseudo-terminal, the GPUI terminal view, and every measurement.

pub mod backend;
pub mod conformance;
pub mod element;
pub mod flood;
pub mod latency;
pub mod measure;
pub mod pty;
pub mod report;

pub use backend::{Cell, Facts, Rgb, Row, Snapshot, TerminalBackend};
pub use pty::Pty;
