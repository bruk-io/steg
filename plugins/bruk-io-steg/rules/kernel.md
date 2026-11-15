---
layer: kernel
paths:
  - "crates/kernel/**"
---
# The kernel

Applies to code in crates/kernel.

- What this package is and how to test it: `crates/kernel/CLAUDE.md`. The judgement its boundary needs: `.claude/rules/kernel.md`. Both load when you work in these files, so they are not repeated here.
- The terminal engine is ghostty's VT engine, vendored under `third_party/` and built by a pinned Zig (`third_party/README.md`). Never run Zig by hand inside `third_party/`: a direct build there writes into the vendored tree. Build through `just`.
- What the kernel holds and the rules between those things are doc-15 on the board, and its words are the names to use. Read it with `asdlc board doc view doc-15 --plain`.
