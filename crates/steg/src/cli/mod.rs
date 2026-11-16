//! The CLI's whole command tree, built at run time from the API
//! description (decision-1, doc-2 section 2) - nothing here names a
//! method, a flag or a value kind by hand (R32). This is TASK-1.49's own
//! part of the split: the tree itself, `--help`, `version` and
//! `completion`, and the drift test (`tests/cli_drift.rs`). Calling a
//! method for real, the exit-status mapping, and connecting to a server
//! are TASK-1.49b's; [`run`] below is this part's own placeholder seam,
//! replaced outright there.

mod params;
mod run;
mod tree;
mod values;

pub use params::extract_params;
pub use run::run;
pub use tree::{NON_METHOD_COMMANDS, build};
