---
layer: sdk
paths:
  - "crates/sdk/**"
---
# The sdk

Applies to code in crates/sdk.

- What this package is and how to test it: `crates/sdk/CLAUDE.md`. The judgement its boundary needs: `.claude/rules/sdk.md`. Both load when you work in these files, so they are not repeated here.
- The specs it serves are on the board, not in the repository: doc-16 (R101 to R106) for the sdk's place in the architecture, and doc-14 for the wire protocol. Read them with `asdlc board doc view <id> --plain`.
