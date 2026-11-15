---
paths:
  - "**/*.rs"
  - "**/Cargo.toml"
---
# Architecture boundaries

Applies to all Rust code and crate manifests in steg. The reasons are in the specs (`backlog/docs/doc-1`, and `backlog/docs/doc-16` for the packages and the dependencies between them), by requirement number.

- The kernel and the sdk do no I/O of their own: `clippy::disallowed_types` and `clippy::disallowed_methods`, configured per crate in `crates/kernel/clippy.toml` and `crates/sdk/clippy.toml`, refuse a file, a socket, a process or the clock (doc-16 R102, R110). The sdk's `local-socket` feature is its one exception (R103) - see `.claude/rules/sdk.md`; how the kernel reaches the outside world when it must is `.claude/rules/kernel.md`.
- Every client uses the one public API and nothing else. The app gets no private channel, so anything the app can do, the CLI and a plugin can do too (R36). A feature the app needs is added to the API first.
- The API is defined once, in the description the CLI and the docs are generated from. A method written by hand in the CLI, or documented by hand, is a second definition (R32, R37).
- The sdk is steg's public interface: a consumer supplies the transport. The CLI (inside `steg`), the server and the app are all consumers of it, and none holds protocol code of its own (doc-16 R101).
- A pane has a kind. Layout, targeting and the API never assume a pane is a terminal (R12).
- A key press never waits on anything outside the app and the server. Bindings are data the app already holds (R29, R73).
- No Python runs inside the server or the app, and neither depends on Python being installed (R47, R72).
- A dependency from one crate on another is a statement about these boundaries. Adding one that crosses them, such as the kernel depending on an async runtime or the app depending on the kernel's internals, needs the plan to say why. `boundaries.toml` holds doc-16's table of which package may depend on which and which of an allowed dependency's features it may turn on, each with a reason; `just check-boundaries` fails on anything added that isn't listed, such as the kernel declaring the sdk's `local-socket` feature in its own `Cargo.toml` (doc-16 R104, R110). `just check-packages` (part of `just check`) builds and tests each package alone, with no other member's features unified in, so a kernel change that only *compiles* against that feature because some other package turned it on in the same `--workspace` run fails there instead of passing silently.
