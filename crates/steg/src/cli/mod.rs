//! The CLI's whole command tree, built at run time from the API
//! description (decision-1, doc-2 section 2) - nothing here names a
//! method, a flag or a value kind by hand (R32). This is TASK-1.49's own
//! part of the split: the tree itself, `--help`, `version` and
//! `completion`, and the drift test (`tests/cli_drift.rs`). Calling a
//! method for real, the exit-status mapping, and connecting to a server
//! are TASK-1.119's (`connect.rs`, `exit.rs`, `api_call.rs`), which
//! replace that task's own placeholder (`run.rs`) outright.

pub mod api_call;
pub mod connect;
pub mod exit;
mod output;
mod params;
mod tree;
mod values;

pub use connect::run;
pub use params::extract_params;
pub use tree::{NON_METHOD_COMMANDS, build};
