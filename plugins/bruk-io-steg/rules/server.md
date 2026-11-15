---
layer: server
paths:
  - "crates/server/**"
---
# The server

Applies to code in crates/server.

- The server depends on `steg-kernel` and never on `steg-cli` or `steg-app`: a client reaches it through the socket and the public API only (doc-8 on the board, R36 and R98). `boundaries.toml` does not check this yet; TASK-1.69 adds it.
- How it starts, detaches and stops is doc-8 R99, and that code lives here, not in any client.
