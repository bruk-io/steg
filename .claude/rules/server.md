---
paths:
  - "crates/server/**"
---
# The server

Applies to code in crates/server. `crates/server/CLAUDE.md` has what this package is, its
dependencies and its tests; this file is the judgement `boundaries.toml` can't make for you.

- The server owns every session, tab, pane and running program. A client asks the server and
  draws what it reports; it never keeps a copy it treats as the truth (R1, R2).
- How it starts, detaches and stops is doc-8 R99, and that code lives here, not in any client.
