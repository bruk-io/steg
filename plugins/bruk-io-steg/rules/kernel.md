---
layer: kernel
paths:
  - "crates/kernel/**"
---
# The kernel

Applies to code in crates/kernel.

- The kernel does no I/O and depends only on `steg-sdk`, with default features off (doc-16 on the board, R105). `boundaries.toml` holds its allow list and `just boundaries` fails on anything added; a new dependency is a deliberate edit there with its reason, and the task's plan says why.
- `#![forbid(unsafe_code)]` is at the top of `crates/kernel/src/lib.rs`. Code that needs `unsafe` goes in another crate, behind a safe interface.
- Shapes that cross the API come from the sdk; do not define a second copy here. What the kernel holds and the rules between those things are doc-15 on the board, and its words are the names to use.
