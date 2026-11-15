---
paths:
  - "**/*.rs"
  - "**/Cargo.toml"
---
# Architecture boundaries

Applies to all Rust code and crate manifests in steg. The reasons are in the specs (`backlog/docs/doc-1`, and `backlog/docs/doc-16` for the packages and the dependencies between them), by requirement number.

- The kernel is logic only. It takes data and returns data, and does no I/O of its own: no sockets, no files, no clock, no process spawning except behind an interface the server supplies. That is what lets its tests run with no doubles, and it is how the server stays the only owner of state (R1).
- The server owns every session, tab, pane and running program. A client asks the server and draws what it reports; it never keeps a copy it treats as the truth (R1, R2).
- Every client uses the one public API and nothing else. The app gets no private channel, so anything the app can do, the CLI and a plugin can do too (R36). A feature the app needs is added to the API first.
- The API is defined once, in the description the CLI and the docs are generated from. A method written by hand in the CLI, or documented by hand, is a second definition (R32, R37).
- The sdk is steg's public interface and does no I/O of its own (doc-16 R101, R102): a consumer supplies the transport. The CLI (inside `steg`), the server and the app are all consumers of it, and none holds protocol code of its own.
- The server's code is in packages no client depends on: the app never depends on the server or the kernel, and the kernel may depend on the sdk only without its optional parts (doc-16 R98, R105).
- A pane has a kind. Layout, targeting and the API never assume a pane is a terminal (R12).
- A key press never waits on anything outside the app and the server. Bindings are data the app already holds (R29, R73).
- No Python runs inside the server or the app, and neither depends on Python being installed (R47, R72).
- A dependency from one crate on another is a statement about these boundaries. Adding one that crosses them, such as the kernel depending on an async runtime or the app depending on the kernel's internals, needs the plan to say why. `boundaries.toml` holds doc-16's table of which package may depend on which, with a reason for each; `just check-boundaries` fails on anything added that isn't listed (doc-16 R104).
