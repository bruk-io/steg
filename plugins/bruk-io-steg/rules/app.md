---
layer: app
paths:
  - "crates/app/**"
---
# The app

Applies to code in crates/app.

- The app depends on `steg-sdk` and never on the server or the kernel; `boundaries.toml` holds the list (doc-16 on the board, R98 and R104).
- `gpui` is pinned to an exact version in the workspace's root `Cargo.toml`. Changing the pin is its own change, never a side effect of another one.
- How the app looks and behaves is the design artifact, and doc-10 on the board says which page is the source for what. Do not invent a colour, a size or a motion.
- `crates/app/src/main.rs` cannot open a window in a test run, so what can be decided without a window is a plain function tested without one.
