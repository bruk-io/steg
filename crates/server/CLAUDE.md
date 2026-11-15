# steg-server

The server (doc-16): the kernel plus sockets, pseudo-terminals, processes and the clock. One
process per person per machine.

- May depend on: `steg-kernel`, `steg-sdk` (`boundaries.toml`'s `[steg-server]`).
- May not depend on: `steg` or `steg-app`; no client reaches it except through the socket and
  the public API.
- Tests: `just test` runs this crate's tests when a change touches it; `just packages` builds
  and tests every package alone, including this one.
- Full spec: `backlog/docs/doc-16`.
