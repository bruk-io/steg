---
layer: binary
paths:
  - "crates/steg/**"
---
# The steg binary

Applies to code in crates/steg.

- This crate only puts `steg-server` and `steg-cli` together into the one program that ships (doc-13 on the board, R61). Behaviour goes in one of those two; `main.rs` chooses which runs and nothing else.
