---
layer: cli
paths:
  - "crates/cli/**"
---
# The CLI

Applies to code in crates/cli.

- The CLI is a client: it does not depend on `steg-server` or `steg-kernel` (doc-8 on the board, R98). `boundaries.toml` does not check this yet; TASK-1.69 adds it.
- Its grammar, flags, output and exit statuses are doc-2 on the board, and decision-4 says they come from the API description. The crate is still a frame, so read both before adding the first command.
