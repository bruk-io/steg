# steg

Work is tracked on an asdlc board in this repository (`backlog/`). It is a shared board, so `asdlc board` commits each change and pushes it to `origin/main`; never edit `backlog/` by hand. This repository is registered as the project `bruk-io-steg`, and every task carries `--project bruk-io-steg`. asdlc refuses to work while this checkout has uncommitted changes, so code changes happen in a task's worktree.

Releases are Backlog.md milestones, defined in the spec (`doc-1`, section 6): `m-0` M1 Local daily driver, `m-1` M2 Extendable, `m-2` M3 Remote and shared. Give every new task the milestone of the release it belongs to (`-m <id>`); the intent has none because it spans all three.
