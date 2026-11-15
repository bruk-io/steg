//! Steg's server: the kernel plus its local socket.
//!
//! Starts on demand and outlives the app (R1-R5).
//!
//! TASK-1.40 lands this crate's first behaviour: running a pane's program
//! (`pane`, `pty`) and its environment (`env`). The kernel holds the
//! state (`steg_kernel::Tree::start_program`/`record_exit`/
//! `restart_program`); this crate is where that state meets real I/O
//! (doc-16 R1).

mod env;
mod error;
mod pane;
mod pty;

pub use env::pane_environment;
pub use error::Error;
pub use pane::{PaneEvent, PaneHandle, start};
pub use pty::{LaunchSpec, ProgramLauncher, RealLauncher, SpawnedProgram};
