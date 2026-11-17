//! Steg.app: a GPUI window with the session switcher, tabs and split panes.
//!
//! The window itself is TASK-1.15. This crate is the frame; the call below
//! is deliberately not a full `Application::run` (which opens a real window
//! and needs a window server) - it is the smallest real use of the pinned
//! `gpui` crate that proves the pin resolves, compiles and links.

// TASK-1.121: these modules are pure functions and state with their own tests, built ahead
// of the window plumbing (`terminal_element.rs`/`client.rs`, TASK-1.117's own branch, not yet
// merged here) that will call them. `#[cfg(test)]` while that wiring doesn't exist yet, so
// `cargo build`'s own real binary target carries no dead code to warn about; dropped once
// `on_key_event`/`paint`/the resize observer call them for real.
#[cfg(test)]
mod ime;
#[cfg(test)]
mod keys;
#[cfg(test)]
mod size;

fn main() {
    let width: gpui::Pixels = 800.0_f32.into();
    println!("steg-app: not yet implemented (gpui pinned, width = {width:?})");
}
