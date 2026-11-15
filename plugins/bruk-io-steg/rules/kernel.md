---
layer: kernel
paths:
  - "crates/kernel/**"
---
# The kernel

Applies to code in crates/kernel.

- The kernel has no dependencies, workspace or external: `boundaries.toml` gives `steg-kernel` an empty allow list and `just boundaries` fails on anything added. A new dependency is a deliberate edit to `boundaries.toml` with its reason, and the task's plan says why.
- `#![forbid(unsafe_code)]` is at the top of `crates/kernel/src/lib.rs`. Code that needs `unsafe` goes in another crate, behind a safe interface.
- What the kernel holds and the rules between those things are the domain spec, doc-15 on the board. Use its words for names.
