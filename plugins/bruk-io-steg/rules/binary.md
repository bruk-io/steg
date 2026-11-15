---
layer: binary
paths:
  - "crates/steg/**"
---
# The steg package

Applies to code in crates/steg.

- This package is the CLI and the entry point that starts a server, and it builds the one binary that ships (doc-13 R61, doc-16 R98). It holds no server logic: that goes in `steg-server`.
- The CLI reaches the server only through the sdk, even though both are in this binary.
- No command, flag or exit status is written by hand. They come from the API description (doc-2 on the board, decision-4); read doc-2 before changing how anything prints.
