# steg-kernel

Steg's kernel (doc-16): session, tab and pane state, layout, focus and target resolution -
logic only.

- May depend on: `steg-sdk`, without its optional parts, and `ghostty-vt` - the terminal
  engine doc-16 lists under the kernel (R92). Its Rust API does no I/O at runtime: feeding
  bytes to it and reading cells back is pure computation. The network fetch and the Zig link
  its build needs happen before any of this crate's own code runs, from a vendored copy
  (`mise.toml`, `.cargo/config.toml`, `third_party/README.md`), never from `just check`
  (`boundaries.toml`'s `[steg-kernel]`).
- May not depend on anything else, workspace or external. `#![forbid(unsafe_code)]` is at the
  top of `src/lib.rs`.
- Tests: `just test` runs this crate's tests when a change touches it; `just check-packages` builds
  and tests every package alone, including this one.
- Full spec: `backlog/docs/doc-16`.
