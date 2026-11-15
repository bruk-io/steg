---
id: TASK-1.71
title: >-
  Boundaries: a CLAUDE.md for each package, rules for what is judgement, and
  lints for what is not
status: In Progress
assignee:
  - '@bruk.w4'
created_date: '2026-11-15T02:35:00Z'
updated_date: '2026-11-15T03:15:00Z'
labels:
  - size-3
milestone: m-0
dependencies:
  - TASK-1.69
parent_task_id: TASK-1
type: task
project: bruk-io-steg
asdlc_accepted_from: DRAFT-2
asdlc_branch: task-1.71-boundaries-a-claude-md-for-each-package
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
doc-16: R104, R110, R111. Once the five packages exist, make their boundaries hard to cross by accident. What a program can check becomes a lint; what needs judgement becomes a rule scoped to the package; what an agent should know on opening a package is its CLAUDE.md. Today the only check is which crate depends on which, and the repository's rules, the plugin's layer rules and the specs say overlapping things.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Code in the sdk or the kernel that uses a file, a socket, a process, the clock or an async runtime fails `just lint`, shown by a recorded run for each package; the mechanism (for example clippy's disallowed types and methods, set per package) is named in the plan
- [ ] #2 The kernel depending on the sdk with an optional part turned on fails `just boundaries`, with a test in `tools/tests`
- [ ] #3 Every refusal's message names the requirement it enforces and why
- [ ] #4 Each of the five packages has a CLAUDE.md with what it is, what it may and may not depend on, how to run its tests, and a pointer to doc-16; none restates a lint
- [ ] #5 `.claude/rules/` has the boundary rules that need judgement, each scoped by `paths` to the package it is about; lines a lint now enforces are removed from `architecture.md` and replaced by the lint's name
- [ ] #6 The plan lists what the repository plugin's layer rules should become so that nothing is said twice; the orchestrator makes that change through asdlc-plugin
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
## Mechanism for AC#1 (confirmed empirically, not just from docs)

Clippy's own docs (fetched from doc.rust-lang.org/clippy/configuration.html):
clippy.toml lookup starts at CARGO_MANIFEST_DIR and walks up until it finds a
file; it does not merge multiple files, it reads the first one found. That by
itself doesn't prove per-crate files are honoured under `cargo clippy
--workspace` (CARGO_MANIFEST_DIR could resolve to the workspace root for the
whole invocation). Verified directly: built a throw-away two-crate workspace
with root `clippy.toml` (`allow-unwrap-in-tests = true`) and the real kernel
crate's own unwrap-in-test canary. Added `crates/kernel/clippy.toml`
*without* `allow-unwrap-in-tests`, ran `cargo clippy --workspace --all-targets
-- -D warnings` after `rm -rf target`: the kernel's canary test failed on
`unwrap_used`, proving the per-crate file is read instead of the root one.
Re-added `allow-unwrap-in-tests = true` to the per-crate file: passed again.
So per-crate `clippy.toml` files work as proposed, but **do not merge** with
the root one - each must repeat every option the root file sets that still
applies to that crate.

Separately confirmed `clippy::disallowed_types` / `clippy::disallowed_methods`
are warn-by-default (not nursery/restriction, need no group opt-in), so
`just lint`'s existing `-D warnings` already turns a hit into a failure, and
the configured `reason` string is what clippy prints - this is what satisfies
AC#3 for these two lints.

Async runtime is already fully covered by `boundaries.toml` (neither crate
may depend on `tokio` or anything else at all): don't add runtime paths to
the lint, that would restate a boundary already enforced one way (R111).

## 1. `crates/sdk/clippy.toml` (new) and `crates/kernel/clippy.toml` (new)

Each file repeats `allow-unwrap-in-tests = true` (required - see above, kernel
already has one test that needs it) plus:

```toml
disallowed-types = [
    { path = "std::fs::File", reason = "doc-16 R110: no file I/O in the <sdk|kernel>" },
    { path = "std::fs::OpenOptions", reason = "doc-16 R110: no file I/O in the <sdk|kernel>" },
    { path = "std::net::TcpStream", reason = "doc-16 R110: no socket I/O in the <sdk|kernel>" },
    { path = "std::net::TcpListener", reason = "doc-16 R110: no socket I/O in the <sdk|kernel>" },
    { path = "std::net::UdpSocket", reason = "doc-16 R110: no socket I/O in the <sdk|kernel>" },
    { path = "std::os::unix::net::UnixStream", reason = "doc-16 R110: no socket I/O in the <sdk|kernel>" },
    { path = "std::os::unix::net::UnixListener", reason = "doc-16 R110: no socket I/O in the <sdk|kernel>" },
    { path = "std::process::Command", reason = "doc-16 R110: no process spawning in the <sdk|kernel>" },
    { path = "std::process::Child", reason = "doc-16 R110: no process spawning in the <sdk|kernel>" },
]
disallowed-methods = [
    { path = "std::fs::read", reason = "doc-16 R110: no file I/O in the <sdk|kernel>" },
    { path = "std::fs::write", reason = "doc-16 R110: no file I/O in the <sdk|kernel>" },
    { path = "std::fs::read_to_string", reason = "doc-16 R110: no file I/O in the <sdk|kernel>" },
    { path = "std::fs::remove_file", reason = "doc-16 R110: no file I/O in the <sdk|kernel>" },
    { path = "std::fs::create_dir", reason = "doc-16 R110: no file I/O in the <sdk|kernel>" },
    { path = "std::fs::create_dir_all", reason = "doc-16 R110: no file I/O in the <sdk|kernel>" },
    { path = "std::fs::copy", reason = "doc-16 R110: no file I/O in the <sdk|kernel>" },
    { path = "std::fs::rename", reason = "doc-16 R110: no file I/O in the <sdk|kernel>" },
    { path = "std::thread::spawn", reason = "doc-16 R110: no runtime of its own in the <sdk|kernel>" },
    { path = "std::thread::sleep", reason = "doc-16 R110: no runtime of its own in the <sdk|kernel>" },
    { path = "std::time::Instant::now", reason = "doc-16 R110: no clock access in the <sdk|kernel>" },
    { path = "std::time::SystemTime::now", reason = "doc-16 R110: no clock access in the <sdk|kernel>" },
]
```

All paths are plain `std`, always resolvable, so no `allow-invalid` needed.
List is deliberately not exhaustive of every std I/O function that exists;
`.claude/rules/sdk.md` and `kernel.md` say so, so nobody goes hunting for the
one syscall wrapper that slipped through instead of trusting the mechanism.

The sdk's `local-socket` provider (future work, TASK-1.70/1.44) is this
crate's one permitted exception (R103). It and its own test module (clippy
sees test code too under `--all-targets`, and real I/O against a temp
listener is this crate's planned test technique per TASK-1.70's notes) each
carry their own
`#[expect(clippy::disallowed_types, clippy::disallowed_methods, reason = "R103: the local-socket provider is the sdk's one permitted I/O")]`,
scoped to that module only - nowhere else in the crate. This is the existing
house convention (rust.md: "Silence a lint with `#[expect(...)]`, never
`#[allow]`"), named in `.claude/rules/sdk.md` for whoever builds TASK-1.70 so
they don't invent their own way.

**Open question for the engineer, not blocking this task's build:**
`just lint` today runs `cargo clippy --workspace --all-targets -- -D
warnings` with default features, so the `local-socket` module (once it
exists) would never actually be compiled or linted by `just lint` at all -
not a false pass on I/O (there's no I/O rule to violate there once it has its
`#[expect]`), but a silent gap for every *other* lint in that module too.
Recommend widening the recipe to
`cargo clippy --workspace --all-targets --all-features -- -D warnings` now
(cheap today: the feature is declared and empty, so nothing changes), so the
gap doesn't have to be rediscovered when TASK-1.70 lands code there. Flagging
rather than doing it unasked since it's a repo-wide `justfile` change outside
this task's literal ACs; call it out to @bruk when approving.

## 2. AC#2: `tools/src/steg_tools/affected.py` and `boundaries.py`

`cargo metadata --no-deps` (already what `workspace.cargo_metadata` runs)
gives, per package, a `dependencies` array straight from Cargo.toml -
`features: [...]` and `uses_default_features: bool` per edge - independent of
feature *resolution*/unification. Verified this is untouched by unification:
built a 3-member throwaway workspace (sdk with a `local-socket` feature,
kernel depending on it with `default-features = false`, app depending on it
with the feature on). `cargo metadata` (full, resolved) shows local-socket
"on" for sdk globally once *any* member turns it on - exactly the gap this
task is fixing, and why a resolved-graph read would be useless here. But
`cargo metadata --no-deps`'s declared `dependencies` entry for kernel's edge
to sdk stays `"features": [], "uses_default_features": false` regardless -
this is the right, static, unification-immune signal.

Also verified empirically (`cargo build -p kernel -v`, rustc invocation) that
kernel built *alone* never gets `--cfg feature="local-socket"` even once app
turns it on - the real risk `just lint`/`just check`'s `--workspace` runs
carry is that *other* lints/tests silently pass against a kernel linked to a
non-pure sdk instance; AC#2 only asks for the declared-edge check, which
catches the case where kernel's own Cargo.toml is the one that turns the
feature on. Catching the workspace-unification case too would mean adding
`just packages` (already exists, builds/tests each crate alone with `-p`) to
`just check` - note this as a known, separate gap for the engineer to decide
on, not in this task's scope (AC#2 is about the declaration, not the build).

Changes:

- `Crate` dataclass (`affected.py`): two new fields, both defaulted so every
  existing call site (including `test_affected.py`'s fixtures) keeps
  compiling unchanged:
  `dependency_features: frozenset[tuple[str, str]] = frozenset()` (pairs of
  dependency name, feature name, from each edge's declared `features` list)
  and `default_features_on: frozenset[str] = frozenset()` (names of
  dependencies whose edge has `uses_default_features: true`).
- `crates_from_metadata`: populate both from `package["dependencies"]`'s
  `features` and `uses_default_features` fields.
- `boundaries.toml` schema: a new optional `deny-features` table per crate,
  e.g. for `[steg-kernel]`:
  `deny-features = { "steg-sdk" = ["local-socket"] }` (reuses the crate's
  existing `reason` string - no new reason field needed).
- `BoundaryRule` (`boundaries.py`): new field
  `deny_features: frozenset[tuple[str, str]] = frozenset()`, parsed in
  `read_allow_table` from the new table.
- New `FeatureViolation` dataclass (`crate`, `dependency`, `feature`,
  `reason`) and `feature_violations(crates, rules)`:
  - a violation when `(dep, feature)` is in both `rule.deny_features` and
    `crate.dependency_features`;
  - a violation (feature named `"default"`) when a dependency named in
    `rule.deny_features` is also in `crate.default_features_on` - a
    deliberately conservative rule (deny default features outright wherever
    any feature of that dependency is denied) that closes the gap the
    advisor flagged: today sdk has no `default` feature, so this never
    fires, but it catches one being added later without anyone revisiting
    this check. kernel's Cargo.toml already sets
    `default-features = false`, so this costs it nothing today.
- `format_feature_violation(violation)`: names the crate, the dependency, the
  feature, and the reason (AC#3).
- `main()`: also runs `feature_violations`, prints each with
  `format_feature_violation`, and fails (exit 1) if either list is non-empty.
- Edit the stale comment in `crates/kernel/Cargo.toml`
  ("`just boundaries`... cannot check that `local-socket` stays off") to name
  the new check instead.

Tests (`tools/tests/unit/test_boundaries.py`, same fixture style as the
existing `KERNEL`/`SERVER`/`APP` constants):
- a denied feature present on the edge is a violation
  (`test_a_denied_feature_is_a_violation`);
- an allowed dependency with no denied feature used is not
  (`test_an_allowed_dependency_with_no_denied_feature_is_no_violation`);
- a dependency with `uses_default_features: true` and a `deny-features` entry
  for it is a violation even with an empty `features` list
  (`test_default_features_on_a_denied_dependency_is_a_violation`);
- `format_feature_violation` names the crate, the dependency, the feature and
  the reason;
- `read_allow_table` parses a real `deny-features` table from a temp file.

## 3. AC#4: one `CLAUDE.md` per package (new files)

`crates/sdk/CLAUDE.md`, `crates/kernel/CLAUDE.md`, `crates/server/CLAUDE.md`,
`crates/steg/CLAUDE.md`, `crates/app/CLAUDE.md`. Each: one line on what the
package is, "may depend on" / "may not depend on" (names `boundaries.toml`'s
table entry, doesn't restate *why* - that's doc-16 and the rule), how to run
its tests (`cargo test -p <crate>`, and that `just packages` does every
package alone), and a pointer to `backlog/docs/doc-16`. None mentions
`disallowed-types`/`disallowed-methods` or `clippy.toml` by name - that's the
lint's job to speak for itself, this file is only "what/depends/tests/spec".
Subdirectory CLAUDE.md files load on demand (Read/Write/Edit under them) and
are not re-injected after `/compact` - fine for identity/pointer content with
nothing load-bearing left for a session to lose track of, per
authoring-memory's guidance.

## 4. AC#5: lint-enforced lines out of `.claude/rules/architecture.md`,
   judgement into new path-scoped files

New files (one topic, `paths` scoped to the one package each is about):

- `.claude/rules/sdk.md` (`paths: ["crates/sdk/**"]`): R106 (no GPUI type, no
  particular async runtime in its public interface, a required dependency is
  for data/serialisation only - this is about the sdk's own public API
  surface, which `boundaries.toml`'s dependency ban can't see, so it's not a
  restatement of that check); the `local-socket` module's `#[expect(...)]`
  convention (including its test module) and that the list in
  `crates/sdk/clippy.toml` isn't exhaustive by design.
- `.claude/rules/kernel.md` (`paths: ["crates/kernel/**"]`): when the kernel
  needs something from outside (spawning a program, reading the clock) it
  takes a trait or a callback the server supplies rather than reaching for it
  itself - the design pattern that satisfies the lint, not the lint's
  mechanism restated; this is what lets its tests run with no doubles.
- `.claude/rules/server.md` (`paths: ["crates/server/**"]`): the server owns
  every session, tab, pane and running program - a client asks and draws
  what it reports, never keeps a copy it treats as the truth (R1, R2); how it
  starts, detaches and stops is doc-8 R99 and lives here, not in any client.
  (Moved whole, not just trimmed, out of architecture.md - single-package
  judgement belongs in one home, not the repo-wide file.)

`architecture.md` keeps only genuinely cross-package judgement (app+CLI
client/API invariant, pane-kind, key-press latency, no-Python-in-process) and
replaces the two now-mechanically-checked bullets:

- The kernel/sdk "no sockets, no files, no clock, no process spawning"
  enumeration becomes one bullet naming the lint: "The kernel and the sdk do
  no I/O of their own: `clippy::disallowed_types` and
  `clippy::disallowed_methods`, configured per crate in
  `crates/kernel/clippy.toml` and `crates/sdk/clippy.toml`, refuse a file, a
  socket, a process or the clock (doc-16 R102, R110). The sdk's
  `local-socket` feature is its one exception (R103) - see
  `.claude/rules/sdk.md`; how the kernel reaches the outside world when it
  must is `.claude/rules/kernel.md`."
- The dependency-table bullet gains the feature half of R110: "...
  `boundaries.toml` holds doc-16's table of which package may depend on which
  and which of an allowed dependency's features it may turn on, each with a
  reason; `just boundaries` fails on anything added that isn't listed, such
  as the kernel turning on the sdk's `local-socket` feature (doc-16 R104,
  R110)."
- The sdk-is-the-public-interface bullet stays (it's about steg/server/app as
  consumers together, genuinely cross-package) but drops "does no I/O of its
  own" (now stated once, in the merged bullet above) - keeps "a consumer
  supplies the transport... none holds protocol code of its own (R101)".

## 5. AC#6: what the plugin's layer rules should become (for the orchestrator,
   through `asdlc-plugin` - not edited here)

Verified while reading them: `plugins/bruk-io-steg/rules/cli.md` is scoped
`paths: ["crates/cli/**"]` and `binary.md` says the `steg` crate combines
"`steg-server` and `steg-cli`" - but the workspace has no `crates/cli` at all
(`git ls-files crates/` lists only app, kernel, sdk, server, steg); `cli.md`
never loads. Both `cli.md` and `server.md` still say "`boundaries.toml` does
not check this yet; TASK-1.69 adds it" - false since TASK-1.69 shipped, and
now doubly false once this task's `deny-features` lands.

Recommended shape, so a kernel-file touch doesn't load three carriers saying
the same thing (plugin layer rule, `.claude/rules/architecture.md`, the new
crate `CLAUDE.md`) - assign each statement exactly one home: the plugin layer
rule becomes a short pointer to this repository's own carriers plus whatever
is genuinely asdlc/board-specific (if anything):

- `kernel.md`, `server.md`: replace the restated boundary content with "see
  `crates/<pkg>/CLAUDE.md` for what this package is and its tests, and
  `.claude/rules/<pkg>.md` for the judgement calls `boundaries.toml`/the lint
  can't make for you." Drop the stale "TASK-1.69 adds it" sentences.
- `app.md`: add the same pointer line; its existing three bullets (gpui pin,
  design-artifact source of truth, main.rs untestable without a window) are
  asdlc/judgement content with no other home today, so they stay as they are.
- `cli.md`: delete (dead path, `crates/cli` doesn't exist) - fold its still-true
  content (grammar/flags/output/exit statuses come from doc-2 and
  decision-4) into `binary.md`.
- `binary.md`: becomes the sole layer rule for `crates/steg`
  (`paths: ["crates/steg/**"]`), combining its own "glue, no behaviour of its
  own" line with `cli.md`'s surviving content, plus the pointer line above.
- `sdk.md` (new): no layer rule exists for `crates/sdk` today at all - add
  one (`paths: ["crates/sdk/**"]`) with the pointer line; whether it needs
  anything board/asdlc-specific beyond that is the orchestrator's call.

## Order of work

1. `crates/sdk/clippy.toml`, `crates/kernel/clippy.toml`.
2. `affected.py`/`boundaries.py`/`boundaries.toml` changes, with the unit
   tests, before touching any rule/CLAUDE.md prose (so the mechanism named in
   the plan is real before the prose points at it).
3. The five `CLAUDE.md` files.
4. `.claude/rules/sdk.md`, `kernel.md`, `server.md` (new); trim
   `architecture.md`.
5. Fix `crates/kernel/Cargo.toml`'s stale comment.
6. Write up the AC#6 list above into the task's notes/comments for the
   orchestrator (not into `plugins/` - that directory is orchestrator-only,
   never edited from a worktree).

## Proof (every step through `asdlc record-check`, demonstrations reverted)

- Baseline: `just check` passes before any lint change.
- `crates/kernel/clippy.toml` + `crates/sdk/clippy.toml` added, nothing else
  changed yet: `just lint` still passes (empty disallowed lists do nothing
  until misused).
- Add one line of real I/O (e.g. `std::fs::File::open` in a `#[cfg(test)]`-free
  function) to `crates/kernel/src/lib.rs`: record
  `asdlc record-check ... -- sh -c '! just lint'` (succeeds because the
  inner command correctly fails), then revert the line and record a plain
  `just lint` passing again. Repeat the same pair for `crates/sdk/src/lib.rs`.
- Add `features = ["local-socket"]` to `crates/kernel/Cargo.toml`'s `steg-sdk`
  dependency: record `asdlc record-check ... -- sh -c '! just boundaries'`,
  then revert and record a plain `just boundaries` passing.
- `asdlc record-check ... -- just test-tools` for the new Python unit tests.
- Final `asdlc record-check ... -- just check` once every file change is in
  place, as the task's closing evidence.

## Risks / things the engineer should confirm before approving

- The `just lint` `--all-features` widening (section 1) - recommended, not
  forced into the plan's scope; needs a yes/no.
- AC#2 only guards the Cargo.toml declaration, not a `--workspace` build's
  feature unification; closing that fully means adding `just packages` (which
  already exists) to `just check` - named as a gap, not undertaken here.
- The disallowed list is a curated, non-exhaustive set of `std` paths, not a
  provable total ban on I/O (nothing in clippy's config language expresses
  "every function under `std::fs`"); `.claude/rules/sdk.md`/`kernel.md` say so
  explicitly so a reviewer doesn't read the lint as a stronger guarantee than
  it is.
<!-- SECTION:PLAN:END -->
