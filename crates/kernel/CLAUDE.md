# steg-kernel

Steg's kernel (doc-16): session, tab and pane state, layout, focus and target resolution -
logic only.

- May depend on: `steg-sdk`, without its optional parts (`boundaries.toml`'s `[steg-kernel]`).
- May not depend on: anything else, workspace or external. `#![forbid(unsafe_code)]` is at the
  top of `src/lib.rs`.
- Tests: `just test` runs this crate's tests when a change touches it; `just check-packages` builds
  and tests every package alone, including this one.
- Full spec: `backlog/docs/doc-16`.
