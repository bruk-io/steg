---
id: TASK-1.69
title: 'Workspace: the five packages of doc-16, and the check that holds them apart'
status: In Progress
assignee:
  - '@bruk.w4'
created_date: '2026-11-15T01:44:00Z'
updated_date: '2026-11-15T02:34:00Z'
labels:
  - size-2
milestone: m-0
dependencies: []
parent_task_id: TASK-1
type: task
project: bruk-io-steg
asdlc_accepted_from: DRAFT-39
asdlc_branch: task-1.69-workspace-the-server-s-code-kept-apart
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
## Decisions the plan makes

1. **Library crate name: `steg-client`, at `crates/client`.** `steg` is already the
   binary package (crates/steg). Naming follows the existing pattern (steg-kernel,
   steg-server, steg-cli, steg-app): short, prefixed with the role, R101's own words
   ("how a Rust program is a client"). Rejected: renaming the binary package and
   keeping its `steg` output via `[[bin]] name = "steg"`, so the library could be
   called `steg` - this would touch layers.toml, rules/binary.md and the orchestrator's
   brief, all of which fix `crates/steg` as the binary, for no gain.

2. **No change to `tools/src/steg_tools/boundaries.py`.** It already does the job: a
   restricted crate's *allow list* is read and anything not on it is a violation
   (`violations()`), and `format_violation` already prints the dependency and the
   reason - that is AC#4's "refuses each dependency... with its reason." A `deny` key
   would be strictly weaker (an app denied `steg-server` could still pick up
   `steg-kernel` tomorrow with nothing to catch it), and boundaries.toml's own header
   already states the mechanism: a crate restricted gets a full allow list, workspace
   and external dependencies alike; a crate with no table is unrestricted. Extending
   the tool and its tests for a weaker check is unnecessary machinery for a size-2 task.

3. **AC#1 and AC#2 are already true today** (steg-cli has no dependencies; steg-app
   depends only on gpui). This task does not restructure either crate; the allow lists
   below make what is already true into something `just boundaries` enforces, so a
   later task can't quietly add the dependency doc-8 R98 forbids.

## boundaries.toml - the full table after this task

```toml
[steg-kernel]
reason = "the kernel does no I/O of its own (.claude/rules/architecture.md)"
allow = []

[steg-server]
reason = "the server owns every session; no client reaches it except through its socket and the public API, and it may not depend on either client crate (doc-8 R98)"
allow = ["steg-kernel"]

[steg-client]
reason = "the steg library is how a Rust program is a client (doc-8 R101); it holds no server logic and must not depend on steg-server or steg-kernel - cli and app inherit whatever this crate allows itself, since the check only ever looks at direct dependencies"
allow = []

[steg-cli]
reason = "the CLI is a client built on the steg library; it holds no protocol code of its own and does not depend on steg-server or steg-kernel (doc-8 R98, R101)"
allow = ["steg-client"]

[steg-app]
reason = "the app is a client like any other, with no private channel to the server (doc-8 R98, R101); it holds no server code and does not depend on steg-server or steg-kernel"
allow = ["steg-client", "gpui"]

[steg]
reason = "the steg binary only puts the server and the CLI together; it holds nothing else (doc-13 R61)"
allow = ["steg-server", "steg-cli"]
```

`cargo metadata --no-deps` gives `violations()` each crate's *direct* dependencies
only - it never walks a transitive edge. That is why `steg-client`'s own list matters
as much as cli's and app's: if the library crate ever took a dependency on
`steg-kernel`, cli and app would inherit that path with nothing in their own entries
to catch it. The `steg-client` reason says this explicitly so the next task that adds
a dependency to it reads why the list is empty, not just that it is.

## Files, in order

1. **`Cargo.toml`** (workspace root) - add `"crates/client"` to `[workspace] members`.

2. **`crates/client/Cargo.toml`** - new package `steg-client`: `[package]` with
   `edition.workspace = true`, `license.workspace = true`, `version.workspace = true`,
   `[lints] workspace = true`, no `[dependencies]` table (nothing to depend on yet).

3. **`crates/client/src/lib.rs`** - a frame, in the voice of `crates/kernel/src/lib.rs`
   and `crates/server/src/lib.rs`: a module doc comment saying this is the steg
   library (R101) - the API description, the wire protocol's types, and the code to
   find or start a server, connect, call a method, subscribe to events and attach to a
   pane - that it holds no server logic and depends on neither `steg-server` nor
   `steg-kernel`, and that TASK-1.43 adds the API description and draft-1 the
   connection code. No `#![forbid(unsafe_code)]` (that is kernel.md's rule, not stated
   for this crate) and no test module (cli's and server's frames have none either;
   kernel's dummy test exists only to exercise `clippy.toml`'s
   `allow-unwrap-in-tests`, which this crate has no other reason to need yet).

4. **`crates/cli/Cargo.toml`** - add `steg-client = { path = "../client" }` under
   `[dependencies]`, in the same inline-path style the server's and binary's manifests
   already use. Not consumed by any code yet (same as `steg-server`'s unused
   `steg-kernel` dependency today); that is expected until TASK-1.43 and later tasks
   give the CLI something to call through it.

5. **`crates/app/Cargo.toml`** - add `steg-client = { path = "../client" }` beside the
   existing `gpui.workspace = true`.

6. **`boundaries.toml`** - replace the single `[steg-kernel]` table with the six-table
   version above, in crate order (kernel, server, client, cli, app, steg).

7. **`.claude/rules/architecture.md`** - add two bullets after the existing kernel
   bullet, each naming where it is enforced, not just stating it (so the
   rule-invariants check reads them as judgement, not a restatable lint):
   - The server is its own package; no client's package depends on it, and it reaches
     clients only through its socket and the public API (doc-8 R98). `boundaries.toml`
     holds the allow list that enforces this; `just boundaries` fails on anything
     added that isn't on it.
   - The steg library is how a Rust program is a client (doc-8 R101): the CLI and the
     app depend on it, never on the server or the kernel directly, and it holds no
     server logic of its own. The same `boundaries.toml` allow lists enforce this.

8. **`plugins/bruk-io-steg/layers.toml`** - add:
   ```toml
   [layers.client]
   paths = ["crates/client/**"]
   summary = "steg-client: the steg library. How a Rust program is a client of the server (R101); the CLI and the app depend on it."
   ```
   and add `depends_on = ["client"]` to the existing `[layers.cli]` and
   `[layers.app]` tables, matching how `[layers.server]` already names
   `depends_on = ["kernel"]`.

9. **`plugins/bruk-io-steg/rules/client.md`** (new) - one rule file for the new layer,
   frontmatter `layer: client`, `paths: ["crates/client/**"]`, in the shape of
   `rules/kernel.md`: states that `steg-client` holds no server logic and must not
   depend on `steg-server` or `steg-kernel` (R101), that `boundaries.toml` gives it an
   allow list enforcing that, and that the API description (TASK-1.43) and the
   connection code (draft-1) land here, not in the server or the CLI.

10. **`plugins/bruk-io-steg/rules/server.md`** and **`rules/cli.md`** - both currently
    end their boundary bullet with "`boundaries.toml` does not check this yet;
    TASK-1.69 adds it." That sentence becomes false once step 6 lands; replace it with
    "`boundaries.toml` gives this crate an allow list; `just boundaries` fails on
    anything added that isn't on it." `cli.md` also gains the word that the CLI is
    built on `steg-client`, not just that it avoids the server and the kernel.

11. **`plugins/bruk-io-steg/rules/app.md`** - add one bullet: the app depends on
    `steg-client`, never on `steg-server` or `steg-kernel`, and `boundaries.toml`
    enforces it (R98, R101).

These plugin edits are hand-written here rather than through `asdlc-plugin
verify-command`/generation, because they are the same architectural fact
`.claude/rules/architecture.md` states, restated for the plugin's layer rules - not a
new command or layer map discovered from the code. Flagged here so the engineer can
say no to hand-editing the plugin before it happens.

## Risks

- **`steg-affected` can't narrow this change.** `Cargo.toml`, `Cargo.lock` and
  `boundaries.toml` are all in `affected.py`'s `GLOBAL_FILES`, so `steg-affected`
  prints `all` and `just test` (inside `just check`) builds and tests every crate,
  including `steg-app`, which links `gpui` against macOS frameworks. Give
  `record-check` calls the long timeout for this reason.
- **The friction this task adds is deliberate and lasting.** Every later task that
  gives `steg-cli`, `steg-app`, `steg-client`, `steg-server` or `steg` (the binary) a
  new dependency - workspace or external - must add it to that crate's `allow` list in
  `boundaries.toml` with a reason, the same way `steg-kernel`'s empty list already
  works. This is the point of the check, not a gap in it.
- **`steg-client`'s empty allow list is a promise about today, not a lock.** TASK-1.43
  (the API description) and draft-1 (the connection code) will each need to decide
  whether they add a dependency to it; this task does not pre-empt that, it only
  leaves the crate with nothing to depend on yet.

## Proof

1. `asdlc record-check` of `just check` on the tree as found, before any edit, so the
   board shows the starting point.
2. After steps 1-11: `just check` (fmt, lint, `boundaries`, `deny`, the affected
   tests - which is every crate's, per the risk above - and the tools' own tests).
   Record it.
3. Individually, per AC#5: `cargo build -p steg-client`, `cargo build -p steg-cli`,
   `cargo build -p steg-app`, `cargo build -p steg-server`, and the matching
   `cargo test -p <name>` for each, confirming every one builds and tests standing
   alone.
4. **A live demonstration of AC#4, not a new fixture test** (the check logic itself is
   already covered by `tools/tests/unit/test_boundaries.py`, which does not change
   here): with `just boundaries` passing from step 2, add
   `steg-server = { path = "../server" }` to `crates/app/Cargo.toml`'s
   `[dependencies]` only (no code change; `cargo metadata --no-deps` reads the
   manifest), run `just boundaries` again and record that it fails, naming
   `steg-app`, `steg-server` and the reason string from the `[steg-app]` table above.
   Then revert that manifest edit before committing anything - the bad dependency
   never lands in a commit.
5. `uv run --project tools pytest -q tools/tests` (also inside `just check`, called out
   because AC#4 is about this suite's subject) - unchanged, since `boundaries.py` and
   its tests are not touched by this task.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Plan held by @bruk.o, not sent for approval: the engineer is settling the repository's architecture (an SDK with no I/O that holds the API description, the CLI merged into the steg binary crate, the kernel allowed to depend on the SDK). This plan was written against the earlier shape (a steg-client crate, a separate steg-cli). It is to be rewritten once the architecture spec is on the board. Also: the plugin under plugins/bruk-io-steg is changed through asdlc-plugin, not in a task's worktree.
<!-- SECTION:NOTES:END -->
