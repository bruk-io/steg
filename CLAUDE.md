# steg

Work is tracked on an asdlc board in this repository (`backlog/`). It is a shared board, so `asdlc board` commits each change and pushes it to `origin/main`; never edit `backlog/` by hand. This repository is registered as the project `bruk-io-steg`, and every task carries `--project bruk-io-steg`. asdlc refuses to work while this checkout has uncommitted changes, so code changes happen in a task's worktree.

Releases are Backlog.md milestones, defined in the spec (`doc-1`, section 6): `m-0` M1 Local daily driver, `m-1` M2 Extendable, `m-2` M3 Remote and shared. Give every new task the milestone of the release it belongs to (`-m <id>`); the intent has none because it spans all three.

## Commands

Run project commands through `just`, never cargo directly; `just` lists them.

- `just check` - what a change must pass before it is called done: format, lint, licences, the affected tests, and the tools' tests.
- `just test` - only the tests for the crates the change touches and the crates that depend on them. `tools/` chooses them from the diff against `origin/main`; do not choose tests by hand. `just test-all` runs everything and is for CI and for doubting the selection.
- Output is quiet: a passing step prints one line, a failing step prints the end of its output and the path of the full log under `.quiet/`. Read the log only when the tail is not enough. `STEG_VERBOSE=1` shows everything.
