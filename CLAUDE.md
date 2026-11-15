# steg

Work is tracked on an asdlc board in this repository (`backlog/`). It is a shared board, so `asdlc board` commits each change and pushes it to `origin/main`; never edit `backlog/` by hand. This repository is registered as the project `bruk-io-steg`, and every task carries `--project bruk-io-steg`. asdlc refuses to work while this checkout has uncommitted changes, so code changes happen in a task's worktree.

Releases are Backlog.md milestones, defined in the spec (`doc-1`, section 6): `m-0` M1 Local daily driver, `m-1` M2 Extendable, `m-2` M3 Remote and shared. M1 is reached through two stopping points with milestones of their own: `m-3` M1a Works from the command line, then `m-4` M1b A window to work in; `m-0` holds what is left of M1 after them. Give every new task the milestone it belongs to (`-m <id>`), the stopping point's when it is M1 work one of them needs; the intent has none because it spans all three releases. Work the board in that order: `m-3` before `m-4` before `m-0`.

## Commands

Run project commands through `just`, never cargo directly; `just` lists them.

- `just check` - what a change must pass before it is called done: format, lint, crate boundaries, licences, the affected tests, the tools' own tests, the architecture model, and the justfile's own recipe names.
- `just test` - only the tests for the crates the change touches and the crates that depend on them. `tools/` chooses them from the diff against `origin/main`; do not choose tests by hand. `just test-all` runs everything and is for CI and for doubting the selection.
- Output is quiet: a passing step prints one line, a failing step prints the end of its output and the path of the full log under `.quiet/`. Read the log only when the tail is not enough. `STEG_VERBOSE=1` shows everything.
