---
layer: sdk
paths:
  - "crates/sdk/**"
---
# The sdk

Applies to code in crates/sdk.

- The sdk is steg's public interface and does no I/O of its own: a consumer supplies the transport (doc-16 on the board, R101 and R102). `boundaries.toml` gives `steg-sdk` its allow list and `just check-boundaries` fails on anything added.
- The one part that does I/O is the local-socket provider, behind the `local-socket` feature, which is off by default. Nothing outside that feature may need it, and the kernel never turns it on (R103, R105).
- A shape that crosses the API is defined here once and used by the kernel and every client. Behaviour that only the server needs, such as handing out a new identifier, is the kernel's.
- No GPUI type and no particular async runtime in a public signature, so the sdk stays usable from another language (R106).
