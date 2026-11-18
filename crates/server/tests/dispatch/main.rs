//! Functional tests for `dispatch`: a real `Tree`, no doubles
//! (`.claude/rules/testing-rust.md`), driven only through the public API
//! (`steg_server::{dispatch, Context, Outcome}`), the way TASK-1.45 will
//! call it.

// `clippy.toml`'s `allow-unwrap-in-tests` only recognises `#[cfg(test)]`
// unit test modules, not a file under `tests/` - the whole of which is
// already test-only, with no production code for a stray `.unwrap()` to
// reach. rust.md's rule ("no unwrap outside tests") is still satisfied.
#![expect(
    clippy::unwrap_used,
    reason = "this whole file is a test; see the comment above"
)]

mod api;
mod cross_cutting;
mod fixtures;
mod pane;
mod session;
mod setting;
mod tab;
