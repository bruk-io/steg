# steg

The CLI and the entry point that starts a server, shipped as one binary with the server
(doc-16, R61, R98).

- May depend on: `steg-sdk`, `steg-server` (`boundaries.toml`'s `[steg]`).
- May not depend on: anything that isn't one of those two; it holds no server logic of its own.
- Tests: `just test` runs this crate's tests when a change touches it; `just check-packages` builds
  and tests every package alone, including this one.
- Full spec: `backlog/docs/doc-16`.
