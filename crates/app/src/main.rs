//! Steg.app: a GPUI window with the session switcher, tabs and split panes.
//!
//! The window itself is TASK-1.15. This crate is the frame; the call below
//! is deliberately not a full `Application::run` (which opens a real window
//! and needs a window server) - it is the smallest real use of the pinned
//! `gpui` crate that proves the pin resolves, compiles and links.

fn main() {
    let width: gpui::Pixels = 800.0_f32.into();
    println!("steg-app: not yet implemented (gpui pinned, width = {width:?})");
}
