//! Steg's server: the kernel plus its local socket.
//!
//! Starts on demand and outlives the app (R1-R5).
//!
//! TASK-1.40 lands this crate's first behaviour: running a pane's program
//! (`pane`, `pty`) and its environment (`env`). The kernel holds the
//! state (`steg_kernel::Tree::start_program`/`record_exit`/
//! `restart_program`); this crate is where that state meets real I/O
//! (doc-16 R1). TASK-1.75 adds `dispatch`, which routes a request to the
//! kernel and maps its errors.
//!
//! TASK-1.44 (part A) adds the socket, the one-server-per-socket
//! arbiter, and the handshake: `config`, `paths`, `bind`, `arbiter`,
//! `connection`, `listen`. [`serve`] runs in the foreground of whichever
//! process calls it, with no detaching - a server that starts on demand,
//! detaches, captures a login-shell environment, and stops itself after
//! an idle period is part B's, a separate task.

mod arbiter;
mod bind;
mod config;
mod connection;
mod detach;
mod dispatch;
mod env;
mod error;
mod listen;
mod pane;
mod paths;
mod program_param;
mod pty;
mod runtime;
mod signal;

pub use arbiter::{Arbitration, Reason as ArbitrationReason, ServerHandle, attempt};
pub use config::{DEFAULT_ARBITRATION_TIMEOUT, DEFAULT_HANDSHAKE_TIMEOUT, ServerConfig};
pub use connection::ConnectionEvent;
pub use detach::detach;
pub use dispatch::{Context, Outcome, dispatch};
pub use env::pane_environment;
pub use error::Error;
pub use listen::{ServeOutcome, Stopper, resolve_socket_path, run, serve};
pub use pane::{PaneEvent, PaneHandle, start};
pub use paths::Reason as PathReason;
pub use program_param::program_from_request;
pub use pty::{LaunchSpec, ProgramLauncher, RealLauncher, SpawnedProgram};
pub use runtime::{FORCE_GRACE_DEFAULT, LoginShell, Runtime};
