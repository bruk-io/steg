# steg-app

Steg.app (doc-16): the GPUI window. A client of the server like any other, through the sdk and
the public API.

- May depend on: `steg-sdk`, `gpui` (`boundaries.toml`'s `[steg-app]`).
- May not depend on: `steg-server`, `steg-kernel`, or `steg`.
- Tests: `just test` runs this crate's tests when a change touches it; `just check-packages` builds
  and tests every package alone, including this one. `src/main.rs` cannot open a window in a
  test run.
- Full spec: `backlog/docs/doc-16`.
