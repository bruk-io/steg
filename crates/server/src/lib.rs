//! Steg's server: the kernel plus its local socket.
//!
//! Starts on demand and outlives the app (R1-R5).

mod dispatch;

pub use dispatch::{Context, Outcome, dispatch};
