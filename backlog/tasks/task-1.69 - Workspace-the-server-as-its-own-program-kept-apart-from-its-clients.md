---
id: TASK-1.69
title: 'Workspace: the five packages of doc-16, and the check that holds them apart'
status: In Review
assignee:
  - '@bruk.w4'
created_date: '2026-11-15T01:44:00Z'
updated_date: '2026-11-15T02:48:00Z'
labels:
  - size-2
milestone: m-0
dependencies: []
references:
  - task-1.69-workspace-the-server-s-code-kept-apart
  - https://github.com/bruk-io/steg/pull/11
parent_task_id: TASK-1
type: task
project: bruk-io-steg
asdlc_accepted_from: DRAFT-39
asdlc_branch: task-1.69-workspace-the-server-s-code-kept-apart
asdlc_plan_approved_by: bruk
asdlc_plan_approved_hash: '3b5cfdb0a760'
asdlc_plan_approved_size: size-2
asdlc_reviewer: bruk.r1
asdlc_review_kind: same-machine
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
doc-16: R98, R101 to R105, R107; doc-13 R61. Today the workspace has kernel, server, cli, steg and app, there is no sdk, and `boundaries.toml` restricts only the kernel. Revise the plan to this description; the earlier plan was written before doc-16 existed.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The workspace's packages are sdk, kernel, server, `steg` and app; the CLI's package is merged into `steg`, which builds the one binary (R61, R98)
- [ ] #2 The sdk package exists with no behaviour yet, no I/O, and its local-socket provider declared as an optional part that is off by default (R102, R103)
- [ ] #3 `boundaries.toml` holds doc-16's table for every package, with a reason on each, and `just boundaries` fails when a dependency outside it is added, shown by a recorded run (R104)
- [ ] #4 The kernel may depend on the sdk without its optional parts and on nothing that does I/O (R105)
- [ ] #5 Each package builds and tests by itself (R107)
- [ ] #6 `.claude/rules/architecture.md` states these rules and cites doc-16, and the repository plugin's layers and rules are brought in line through `asdlc-plugin` by the orchestrator, not in this task's worktree
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
Rewritten against doc-16 (the architecture spec), which replaced the earlier plan's
shape: no `steg-client`, no separate `steg-cli`. The repository ends this task with
five packages - sdk, kernel, server, `steg`, app - matching doc-16's level-3 table
exactly.

## Decisions the plan makes

1. **sdk's crate name: `steg-sdk`, at `crates/sdk`.** Follows the convention every
   other package already has (`steg-kernel`, `steg-server`, `steg-app`; `steg` itself
   is the lone unprefixed one, because it is the binary's own name). doc-16 writes
   "sdk" as the component's short name in its tables, the same way it writes "kernel"
   and "server" for packages whose real names carry the prefix.

2. **`crates/cli` is deleted; its two files' content folds into `crates/steg`.**
   doc-16's components table is explicit: "`steg` | The CLI, and the entry point that
   starts the server". `crates/cli/src/lib.rs`'s doc comment (a thin client, commands
   come from the API description, R32) becomes part of `crates/steg/src/main.rs`'s,
   since that is now where the CLI's code will land (TASK-1.49 says so directly: "CLI:
   commands generated from the API description, in the steg package").

3. **No change to `tools/src/steg_tools/boundaries.py`.** Unchanged from the earlier
   plan's finding, for the same reason: `violations()` already reads a restricted
   crate's full allow list - workspace and external dependencies alike - and
   `format_violation` already prints the dependency and the reason, which is what
   AC#3 and R104 ask for. See "What `boundaries.py` can and cannot check" below for
   the one place this finding does *not* cover: R105's "without its optional parts."
   That gap is a separate task being drafted; this task leaves it as found and does
   not try to close it.

4. **Every edge doc-16's table allows is wired into the real `Cargo.toml` files now,
   even though nothing uses it yet.** This matches the repository's own existing
   practice: `steg-server` already depends on `steg-kernel` with no code in
   `crates/server/src/lib.rs` calling into it, and the old `crates/steg` depended on
   `steg-cli` the same way. Declaring the dependency ahead of the behaviour that uses
   it is how this workspace has done it so far, so kernel, server, `steg` and app each
   get a real (unused) dependency on sdk, not just a permission sitting in
   `boundaries.toml` with nothing to show for it. AC#5's "builds and tests by itself"
   is then proof against real manifests, not against a table nobody's Cargo.toml
   matches.

5. **A new `just packages` recipe, not raw `cargo build -p`/`cargo test -p`.** This
   repository's commands go through `just`, never cargo directly (CLAUDE.md). Step 13
   below adds a recipe that builds and tests every package by itself through
   `steg-quiet`, so AC#5 ("each package builds and tests by itself", R107) is
   something anyone can re-run later, not just a sequence of commands quoted in this
   plan's proof. It stays outside `check` (mirroring how `test-all` also stays outside
   `check`): `check` already runs every crate's tests through `test` whenever the
   selection is `all`, which this task's `Cargo.toml`/`boundaries.toml`/`justfile`
   changes make it anyway (see Risks), so running the same build and test a second
   time, per package, inside `check` would just repeat work `check` already does.

## doc-16's table, and `boundaries.toml` after this task

| Package | May depend on | Does I/O |
|---|---|---|
| sdk | nothing of steg's | No |
| kernel | sdk | No |
| server | kernel, sdk | Yes |
| `steg` | sdk, server | Yes |
| app | sdk | Yes |

```toml
[steg-sdk]
reason = "the sdk is steg's public interface and does no I/O of its own; it has nothing of steg's to depend on (doc-16 R101, R102)"
allow = []

[steg-kernel]
reason = "the kernel does no I/O and depends on the sdk only for the shapes that cross the API, never for its optional parts (doc-16 R105)"
allow = ["steg-sdk"]

[steg-server]
reason = "the server is the kernel plus sockets, pseudo-terminals, processes and the clock; no client's package depends on it (doc-16 R98)"
allow = ["steg-kernel", "steg-sdk"]

[steg]
reason = "the steg package holds the CLI and the entry point that starts a server, and ships them as one binary with the server (doc-16 R98, doc-13 R61); it holds no server logic of its own"
allow = ["steg-sdk", "steg-server"]

[steg-app]
reason = "the app is a client of the server like any other, through the sdk and the public API, with no private channel (doc-16 R98, R101)"
allow = ["steg-sdk", "gpui"]
```

## What `boundaries.py` can and cannot check

`crates_from_metadata` (`tools/src/steg_tools/affected.py`) builds each `Crate` from
`cargo metadata --no-deps`'s `package["dependencies"]`, keeping only each dependency's
*name*. `violations()` then asks only "is this name on the allow list" - it has no
access to a dependency's `features` or `default-features` table, because
`--no-deps` never resolves features in the first place; that needs the full resolve
graph (`cargo metadata` without `--no-deps`, or `cargo tree -e features`), which is a
different, heavier query than this tool runs today.

So `just boundaries` **can** enforce the crate-level half of R105: that
`steg-kernel`'s only workspace dependency is `steg-sdk`, and nothing that does I/O
(`steg-server`, `steg`, `steg-app`, or an I/O-bearing external crate) is on its list.
It **cannot yet** enforce the feature-level half - that the dependency on `steg-sdk`
specifically excludes the `local-socket` feature. That half is carried by
`crates/kernel/Cargo.toml` writing `default-features = false` on the dependency
(step 6 below) and by `.claude/rules/architecture.md` stating the rule for a reviewer
to check, not by any command. Closing this gap mechanically is already a separate
task being drafted; it is left exactly as found here.

## Files, in order

1. **`Cargo.toml`** (workspace root) - `[workspace] members` becomes
   `["crates/sdk", "crates/kernel", "crates/server", "crates/steg", "crates/app"]`:
   drop `crates/cli`, add `crates/sdk`.

2. **Delete `crates/cli/`** (`Cargo.toml` and `src/lib.rs`), via `git rm -r`.

3. **`crates/sdk/Cargo.toml`** - new package `steg-sdk`: `[package]` with
   `edition.workspace = true`, `license.workspace = true`, `version.workspace = true`,
   `[lints] workspace = true`, no `[dependencies]` (R102: no I/O, nothing of steg's to
   depend on), and:
   ```toml
   [features]
   local-socket = []
   ```
   Declared and empty (R103): no `default = [...]` table, so it is off unless a
   consumer asks for it, and no optional dependency or `cfg`-gated code sits behind it
   yet - that is TASK-1.44 and draft-1's to add.

4. **`crates/sdk/src/lib.rs`** - a frame, in the voice of `crates/kernel/src/lib.rs`:
   a module doc comment stating this is steg's public interface (R101) - the API
   description, the shapes that cross the API, and the protocol's logic - that it does
   no I/O of its own (R102), that a consumer supplies the transport, and that the
   `local-socket` feature is declared and empty today, filled in by TASK-1.44 (the
   server's local socket) and draft-1 (the client side of the connection). No
   `#![forbid(unsafe_code)]` (kernel's rule, not stated for this crate) and no test
   module (nothing to test yet).

5. **`crates/kernel/Cargo.toml`** - add:
   ```toml
   [dependencies]
   # R105: the sdk, never its optional parts. `just boundaries` checks that this
   # dependency exists and that nothing else does (boundaries.toml); it cannot check
   # that `local-socket` stays off (see the plan's note on this).
   steg-sdk = { path = "../sdk", default-features = false }
   ```

6. **`crates/kernel/src/lib.rs`** - add one sentence to the existing doc comment: the
   kernel depends on the sdk for the shapes that will cross the API (R105), unused
   today because the sdk has none yet.

7. **`crates/server/Cargo.toml`** - add `steg-sdk = { path = "../sdk" }` beside the
   existing `steg-kernel` dependency.

8. **`crates/steg/Cargo.toml`** - remove the `steg-cli` dependency; add
   `steg-sdk = { path = "../sdk" }` beside the existing `steg-server`.

9. **`crates/steg/src/main.rs`** - fold in the deleted `crates/cli/src/lib.rs`'s doc
   comment: this binary is now the server, the CLI, and the entry point that starts
   one (R61, R98); the CLI's commands come from the API description once TASK-1.43 and
   TASK-1.49 land; the crate holds no server logic of its own. Which of server or CLI
   a run acts as is still undecided here - unchanged from today's frame.

10. **`crates/app/Cargo.toml`** - add `steg-sdk = { path = "../sdk" }` beside the
    existing `gpui.workspace = true`.

11. **`boundaries.toml`** - replace the single `[steg-kernel]` table with the
    five-table version above (sdk, kernel, server, steg, app).

12. **`.claude/rules/architecture.md`** - update to cite doc-16 instead of doc-8/R36
    where this task's rules moved, and add what is new:
    - Replace the existing API-description bullet's implicit source with doc-16, and
      add: "The sdk is steg's public interface and does no I/O of its own (doc-16
      R101, R102): a consumer supplies the transport. The CLI (inside `steg`), the
      server and the app are all consumers of it, and none holds protocol code of its
      own."
    - Add: "The server's code is in packages no client depends on: the app never
      depends on the server or the kernel, and the kernel may depend on the sdk only
      without its optional parts (doc-16 R98, R105)."
    - Extend the existing dependency-boundary bullet: "`boundaries.toml` holds doc-16's
      table of which package may depend on which, with a reason for each; `just
      boundaries` fails on anything added that isn't listed (doc-16 R104)."

13. **`justfile`** - add a recipe that builds and tests every package by itself
    through `steg-quiet`, in the voice of the existing `test`/`test-tools` recipes:
    ```
    # build and test each package by itself (R107), so this is something anyone can run again
    packages:
        #!/usr/bin/env bash
        set -euo pipefail
        if [ "{{rust}}" = no ]; then echo "skip    packages: no Rust workspace yet"; exit 0; fi
        for pkg in steg-sdk steg-kernel steg-server steg steg-app; do
            {{quiet}} "build ($pkg)" -- cargo build -p "$pkg"
            {{quiet}} "test ($pkg)" -- cargo test -p "$pkg"
        done
    ```
    Not added to `check`'s recipe list (see decision 5): `check` already runs every
    crate's tests together whenever `steg-affected` says `all`, which this task's own
    changes to `Cargo.toml`/`boundaries.toml`/`justfile` guarantee it will (Risks,
    below), so `packages` running the same work again inside `check` would be pure
    repetition. It is its own recipe, run directly, the same way `test-all` is.

## For the orchestrator, through `asdlc-plugin` (not edited in this worktree)

AC#6 keeps `plugins/bruk-io-steg/` out of this task. What it will need, so the
orchestrator can apply it deliberately rather than guess from the diff:

- `layers.toml`: a new `[layers.sdk]` (`paths = ["crates/sdk/**"]`); the `[layers.cli]`
  table removed, since `crates/cli` no longer exists; `[layers.binary]`'s `paths`
  unchanged (`crates/steg/**`) but its `depends_on` becomes `["sdk", "server"]`;
  `[layers.kernel]` gains `depends_on = ["sdk"]`; `[layers.server]`'s `depends_on`
  becomes `["kernel", "sdk"]`; `[layers.app]` gains `depends_on = ["sdk"]`.
- `rules/cli.md` is deleted (its crate is gone).
- A new `rules/sdk.md`: no I/O, nothing of steg's to depend on, the `local-socket`
  feature declared and empty until TASK-1.44/draft-1 (doc-16 R101-R103).
- `rules/kernel.md`: add that it now depends on `steg-sdk`, never with
  `local-socket` (doc-16 R105), and that `boundaries.toml` checks the dependency's
  name but not yet the feature (the plan's note above, so the rule doesn't overclaim
  what's enforced).
- `rules/binary.md`: update "this crate only puts `steg-server` and `steg-cli`
  together" - `steg-cli` no longer exists; the CLI's code is now inside this crate
  (doc-16's components table), and the dependency is on `steg-sdk` and `steg-server`.
- `rules/app.md`: add that the app depends on `steg-sdk`, never on `steg-server` or
  `steg-kernel`, enforced by `boundaries.toml` (doc-16 R98, R101).
- `rules/server.md`: add `steg-sdk` beside `steg-kernel` as its allowed dependency.
- `commands.toml`'s `[test]` entry (`just test-all && just test-tools`) could add
  `just packages`, if the orchestrator wants the per-package build/test verified in
  CI too; left to that judgement, not assumed here.

## Risks

- **`steg-affected`'s `GLOBAL_FILES`** (`Cargo.toml`, `Cargo.lock`, `boundaries.toml`,
  and now `justfile` too, since step 13 edits it) all change here, so `steg-affected`
  prints `all` and `just check` builds and tests every crate, including `steg-app`
  linking `gpui` against macOS frameworks. Give `record-check` the long timeout.
- **Feature-off-by-default is a manifest convention, not a check**, as above: nothing
  stops a later task from writing `steg-sdk = { path = "../sdk" }` with no
  `default-features = false` on a workspace member that shouldn't get `local-socket`.
  Flagged in both the plan and the rule; a separate task is already being drafted to
  close it, not this one.
- **This is deliberate, lasting friction**: any later task giving `steg-sdk`,
  `steg-kernel`, `steg-server`, `steg` or `steg-app` a new dependency - workspace or
  external - edits that crate's `allow` list in `boundaries.toml` with a reason. That
  is the point of R104, not a gap in it.
- **R106** (the sdk stays usable from another language: no GPUI type, no particular
  async runtime, dependencies for data and serialisation only) has nothing to check
  yet, because the sdk has no dependencies at all after this task. It becomes a live
  constraint the first time something is added to `steg-sdk`'s allow list.

## Proof

1. `asdlc record-check` of `just check` on the tree as found, before any edit.
2. After steps 1-13: `just check` (fmt, lint, `boundaries`, `deny`, the affected
   tests - every crate's, per the risk above - and the tools' own tests). Record it.
3. Per AC#5: `just packages` (the new recipe), building and testing `steg-sdk`,
   `steg-kernel`, `steg-server`, `steg` and `steg-app` each by themselves through
   `steg-quiet`. Record it.
4. **Two live demonstrations of AC#3/R104, not new fixture tests** (the check logic
   itself is `tools/tests/unit/test_boundaries.py`'s subject, unchanged here):
   - A disallowed workspace dependency: add `steg-server = { path = "../server" }` to
     `crates/app/Cargo.toml`'s `[dependencies]` only, run `just boundaries`, record
     that it fails naming `steg-app`, `steg-server` and the `[steg-app]` reason.
   - A disallowed external dependency: add `tokio = "1"` to
     `crates/kernel/Cargo.toml`'s `[dependencies]`, run `just boundaries`, record that
     it fails naming `steg-kernel`, `tokio` and the `[steg-kernel]` reason.
   Revert both manifest edits before anything is committed - neither bad dependency
   ever lands.
5. `uv run --project tools pytest -q tools/tests` (also inside `just check`, called out
   because it is `boundaries.py`'s own suite) - unchanged, since the tool and its
   tests are not touched by this task.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Plan held by @bruk.o, not sent for approval: the engineer is settling the repository's architecture (an SDK with no I/O that holds the API description, the CLI merged into the steg binary crate, the kernel allowed to depend on the SDK). This plan was written against the earlier shape (a steg-client crate, a separate steg-cli). It is to be rewritten once the architecture spec is on the board. Also: the plugin under plugins/bruk-io-steg is changed through asdlc-plugin, not in a task's worktree.

Rewritten plan checked against doc-16 by @bruk.o: ok. Five packages with doc-16's table in boundaries.toml, the sdk empty with its local-socket feature off by default, the CLI merged into steg, each package built and tested by itself through a just recipe. Known gap, left to the boundaries task: the check cannot see the kernel turning on an sdk feature. The plugin changes it lists are the orchestrator's, through asdlc-plugin, after this merges.

Check passed: `just check` exited 0 at 2026-11-15T02:41:47+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.69 on task-1.69-workspace-the-server-s-code-kept-apart at dfebbf69b590:

```text
ok      format check (0.1s)
ok      lint (13.9s)
ok      boundaries (0.1s)
ok      licences (0.7s)
skip    tests: this change touches no crate
ok      tool tests (0.5s)
```

Check FAILED: `just check` exited 1 at 2026-11-15T02:43:17+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.69 on task-1.69-workspace-the-server-s-code-kept-apart at dfebbf69b590 with uncommitted changes, last 40 of 41 lines:

```text
ok      lint (0.5s)
ok      boundaries (0.1s)
ok      licences (0.6s)
ok      tests (every crate) (18.3s)
FAILED  tool tests (exit 1, 0.2s)
FF.............................                                          [100%]
=================================== FAILURES ===================================
__________ test_the_five_real_crates_and_their_real_dependency_edges ___________

    def test_the_five_real_crates_and_their_real_dependency_edges() -> None:
        by_name = {crate.name: crate for crate in real_crates()}
>       assert by_name.keys() == {"steg-kernel", "steg-server", "steg-cli", "steg", "steg-app"}
E       AssertionError: assert dict_keys(['s..., 'steg-app']) == {'steg', 'ste...'steg-server'}
E         
E         Extra items in the left set:
E         'steg-sdk'
E         Extra items in the right set:
E         'steg-cli'
E         Use -v to get more diff

tools/tests/adapter/test_affected_real_workspace.py:21: AssertionError
____________________ test_selection_against_the_real_crates ____________________

    def test_selection_against_the_real_crates() -> None:
        crates = real_crates()
        assert selection(("crates/kernel/src/lib.rs",), crates) == "-p steg -p steg-kernel -p steg-server"
        assert selection(("crates/app/src/main.rs",), crates) == "-p steg-app"
>       assert selection(("crates/cli/src/lib.rs",), crates) == "-p steg -p steg-cli"
E       AssertionError: assert 'none' == '-p steg -p steg-cli'
E         
E         - -p steg -p steg-cli
E         + none

tools/tests/adapter/test_affected_real_workspace.py:45: AssertionError
=========================== short test summary info ============================
FAILED tools/tests/adapter/test_affected_real_workspace.py::test_the_five_real_crates_and_their_real_dependency_edges
FAILED tools/tests/adapter/test_affected_real_workspace.py::test_selection_against_the_real_crates
2 failed, 29 passed in 0.11s
full output: .quiet/tool-tests.log
error: recipe `test-tools` failed on line 43 with exit code 1
```

Check passed: `just check` exited 0 at 2026-11-15T02:46:43+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.69 on task-1.69-workspace-the-server-s-code-kept-apart at dfebbf69b590 with uncommitted changes:

```text
ok      format check (0.1s)
ok      lint (0.4s)
ok      boundaries (0.1s)
ok      licences (0.6s)
ok      tests (every crate) (0.4s)
ok      tool tests (0.2s)
```

Check passed: `just packages` exited 0 at 2026-11-15T02:46:51+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.69 on task-1.69-workspace-the-server-s-code-kept-apart at dfebbf69b590 with uncommitted changes:

```text
ok      build (steg-sdk) (0.1s)
ok      test (steg-sdk) (0.1s)
ok      build (steg-kernel) (0.1s)
ok      test (steg-kernel) (0.1s)
ok      build (steg-server) (0.1s)
ok      test (steg-server) (0.1s)
ok      build (steg) (0.2s)
ok      test (steg) (0.1s)
ok      build (steg-app) (0.6s)
ok      test (steg-app) (0.3s)
```

Check passed: `uv run --project tools pytest -q tools/tests` exited 0 at 2026-11-15T02:47:22+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.69 on task-1.69-workspace-the-server-s-code-kept-apart at dfebbf69b590 with uncommitted changes:

```text
warning: `VIRTUAL_ENV=/Users/brukhabtu/.cache/uv/builds-v0/.tmp0dXsRU` does not match the project environment path `tools/.venv` and will be ignored; use `--active` to target the active environment instead
...............................                                          [100%]
31 passed in 0.09s
```

Check passed: `just check` exited 0 at 2026-11-15T02:47:30+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.69 on task-1.69-workspace-the-server-s-code-kept-apart at dfebbf69b590 with uncommitted changes:

```text
ok      format check (0.1s)
ok      lint (0.4s)
ok      boundaries (0.1s)
ok      licences (0.6s)
ok      tests (every crate) (0.3s)
ok      tool tests (0.2s)
```

Check passed: `just check` exited 0 at 2026-11-15T02:47:56+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.69 on task-1.69-workspace-the-server-s-code-kept-apart at df8f9a21d734:

```text
ok      format check (0.1s)
ok      lint (0.8s)
ok      boundaries (0.1s)
ok      licences (0.7s)
ok      tests (every crate) (1.6s)
ok      tool tests (0.3s)
```

Built exactly as planned, with one departure found while building: deleting
crates/cli broke tools/tests/adapter/test_affected_real_workspace.py, which
asserts crates_from_metadata's output against this repository's real cargo
metadata, hard-coded to the old five crate names and edges. just check caught
it; fixed in the same pass (the test's fixtures now describe the real
post-task workspace - steg-sdk replacing steg-cli, every package's
depends_on updated, the crates/cli selection case replaced with
crates/sdk/src/lib.rs expecting every package). Recorded as a comment on the
task, not a plan rewrite, since the plan itself still held.

Workspace now has five packages: steg-sdk (new, no I/O, local-socket feature
declared and empty), steg-kernel, steg-server, steg (binary, CLI merged in,
steg-cli deleted), steg-app. boundaries.toml holds a full allow-list table for
all five, each with a reason citing doc-16. .claude/rules/architecture.md
cites doc-16 for the rules that moved and the two that are new. A new
`just packages` recipe builds and tests each package standalone.
plugins/bruk-io-steg/ was not touched (AC#6); the plan's "For the
orchestrator" section lists what layers.toml and its rule files need.

Checks recorded on the task:
- `just check`: passing before any edit, after the edits, and again after
  rebasing onto origin/main.
- `just packages`: all five packages build and test standalone.
- `uv run --project tools pytest -q tools/tests`: 31 passed.
- Two live demonstrations of `just boundaries` refusing a disallowed
  dependency (steg-app -> steg-server, and steg-kernel -> tokio), each with
  its reason, reverted before commit (not recorded as a check, since nothing
  was committed in that state).

The boundaries.py feature-level gap (it cannot see whether a crate's sdk
dependency enables local-socket) is left exactly as found, per the plan - a
separate task is already being drafted for it.
<!-- SECTION:NOTES:END -->

## Comments

<!-- COMMENTS:BEGIN -->
author: @bruk
created: 2026-11-15T02:41:00Z

---

Plan approved by @bruk (3b5cfdb0a760, size-2): its worker may build it.

---

created: 2026-11-15T02:46:00Z

---

Small departure found while building, plan otherwise unchanged: deleting `crates/cli`
and adding `crates/sdk` (step 2) breaks
`tools/tests/adapter/test_affected_real_workspace.py`, which asserts
`crates_from_metadata`'s output against this repository's real `cargo metadata`,
hard-coded to the old five crate names and edges (`steg-kernel`, `steg-server`,
`steg-cli`, `steg`, `steg-app`). `just check` caught it: `test-tools` failed on this
file once the workspace changed. Fixed in the same step as everything else: the
test's fixtures (names, folders, `depends_on` sets, and one `selection()` case) now
describe the real post-task workspace - `steg-sdk` replacing `steg-cli`, every
package's `depends_on` updated for its real sdk edge, and the
`crates/cli/src/lib.rs` selection case replaced with `crates/sdk/src/lib.rs`,
expecting every package (`-p steg -p steg-app -p steg-kernel -p steg-sdk -p
steg-server`), since every package now depends on the sdk directly or transitively.
`crates_from_metadata` and `selection()` themselves are untouched; only the
real-world fixture this adapter test reads changed, which is what an adapter test is
for when the real thing moves.

---

author: @bruk.r1
created: 2026-11-15T02:48:00Z

---

@bruk.r1 is reviewing TASK-1.69: a same-machine review: it waited 0m with no other engineer's reviewer, longer than review_fallback_after (0m), so a fresh reviewer on its author's machine takes it.

---
<!-- COMMENTS:END -->
