//! Reduce Motion (TASK-1.127's plan): a stub only, until TASK-1.63 gives it a real
//! platform read.
//!
//! gpui 0.2.2 exposes nothing of this: grepped its own source (`src/*.rs`,
//! `src/platform/**/*.rs`) for "reduce_motion"/"ReducedMotion"/"accessibility" and found
//! no trace of it. The real macOS read (`NSWorkspace.accessibilityDisplayShouldReduceMotion`)
//! needs an Objective-C bridge this crate does not depend on today (`futures`, `gpui`,
//! `serde_json`, `steg-sdk`, `steg-ui`, `unicode-segmentation` only) - the same shape of gap
//! TASK-1.126 found and declined to close for a one-off test (a new dependency and licence
//! surface unrelated to what it was building). TASK-1.63 ("App: keyboard focus, VoiceOver,
//! Reduce Motion and Increase Contrast") is where that read belongs.

/// Whether Reduce Motion is on. Always `false` ("not reduced" - motion always allowed)
/// until TASK-1.63 replaces this with a real read of the system setting.
#[must_use]
pub fn reduce_motion_enabled() -> bool {
    false
}
