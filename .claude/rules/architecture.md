---
paths:
  - "**/*.rs"
  - "**/Cargo.toml"
---
# Architecture boundaries

Applies to all Rust code and crate manifests in steg. The reasons are in the spec (`backlog/docs/doc-1`), by requirement number.

- The kernel is logic only. It takes data and returns data, and does no I/O of its own: no sockets, no files, no clock, no process spawning except behind an interface the server supplies. That is what lets its tests run with no doubles, and it is how the server stays the only owner of state (R1).
- The server owns every session, tab, pane and running program. A client asks the server and draws what it reports; it never keeps a copy it treats as the truth (R1, R2).
- Every client uses the one public API and nothing else. The app gets no private channel, so anything the app can do, the CLI and a plugin can do too (R36). A feature the app needs is added to the API first.
- The API is defined once, in the description the CLI and the docs are generated from. A method written by hand in the CLI, or documented by hand, is a second definition (R32, R37).
- A pane has a kind. Layout, targeting and the API never assume a pane is a terminal (R12).
- A key press never waits on anything outside the app and the server. Bindings are data the app already holds (R29, R73).
- No Python runs inside the server or the app, and neither depends on Python being installed (R47, R72).
- A dependency from one crate on another is a statement about these boundaries. Adding one that crosses them, such as the kernel depending on an async runtime or the app depending on the kernel's internals, needs the plan to say why.
