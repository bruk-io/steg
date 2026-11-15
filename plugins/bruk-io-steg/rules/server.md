---
layer: server
paths:
  - "crates/server/**"
---
# The server

Applies to code in crates/server.

- The server depends on `steg-kernel` and `steg-sdk` and on no client; `boundaries.toml` holds the list (doc-16 on the board, R98 and R104).
- It is a consumer of the sdk like any client: framing, the handshake and the API description come from there, and the server adds the socket and the routing (R101).
- How it starts, detaches and stops is doc-8 R99, and that code lives here, not in any client.
