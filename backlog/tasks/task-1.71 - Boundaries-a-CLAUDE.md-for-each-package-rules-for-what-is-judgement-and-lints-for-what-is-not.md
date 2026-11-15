---
id: TASK-1.71
title: >-
  Boundaries: a CLAUDE.md for each package, rules for what is judgement, and
  lints for what is not
status: In Review
assignee:
  - '@bruk.w4'
created_date: '2026-11-15T02:35:00Z'
updated_date: '2026-11-15T03:47:00Z'
labels:
  - size-3
milestone: m-0
dependencies:
  - TASK-1.69
references:
  - task-1.71-boundaries-a-claude-md-for-each-package
  - https://github.com/bruk-io/steg/pull/17
parent_task_id: TASK-1
type: task
project: bruk-io-steg
asdlc_accepted_from: DRAFT-2
asdlc_branch: task-1.71-boundaries-a-claude-md-for-each-package
asdlc_plan_approved_by: bruk
asdlc_plan_approved_hash: 'd8f3885f55ed'
asdlc_plan_approved_size: size-3
asdlc_review_kind: same-machine
asdlc_review_findings: 'important=1,nit=0'
asdlc_review_policies: ["review"]
asdlc_plan_matched: yes
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

**Engineer confirmed (yes): `just lint` runs with `--all-features` on, so the
sdk's `local-socket` module is linted too, not silently skipped.** Checked
before relying on it, in the real worktree, not a scratch copy:
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
  passes cleanly today (0.55s warm, zero errors). `cargo metadata --no-deps`
  shows only `steg-sdk` declares any feature at all (`local-socket`); no
  other crate's dependency graph has a feature `--all-features` could flip
  on that changes what gets compiled or fetched. The lone warning present
  (`block`/`proc-macro-error2` "rejected by a future version of Rust") is
  pre-existing and appears identically with plain `cargo clippy --all-targets
  -- -D warnings` too - not something `--all-features` introduces.
- Re-ran the kernel-isolation check (first done in a scratch workspace)
  against the real crates: `cargo build -p steg-kernel -v
  --target-dir <scratch>` and grepped its `rustc --crate-name steg_sdk`
  invocation - no `--cfg feature="local-socket"` is passed. Kernel, built
  alone exactly as `just packages` will build it, still never gets the
  feature even once `--all-features` is added elsewhere. The two flags don't
  interact because `just packages`'s `cargo build -p "$pkg"` /
  `cargo test -p "$pkg"` calls (see AC#2 section below) carry no
  `--all-features` of their own and never will - that omission is itself
  the control this plan relies on, so name it as a constraint, not an
  oversight, if anyone edits `packages` later.

Keeping the exemption from spreading, now that `--all-features` makes the
local-socket module visible to every lint pass, not just a feature-flagged
build someone remembers to run: the `clippy.toml` disallowed lists are a
closed, named set (section 1 below) that applies to the whole crate by
default, so any use of a listed type or method anywhere in `crates/sdk`
is flagged unless silenced. The only silencer is `#[expect(...)]` (never
`#[allow]`, per `rust.md`'s existing convention) on the local-socket module
and its test module specifically - nowhere else. Two properties make this
self-enforcing rather than just stated: an `#[expect]` is itself an error if
the lint never actually fires in that scope, so if the local-socket code
stops doing real I/O (refactored away, say) the `#[expect]` breaks the build
and demands cleanup instead of silently going stale like `#[allow]` would;
and `.claude/rules/sdk.md` (section 4) states as a rule, for a human or
reviewing agent, that this crate has exactly one home for that annotation,
so a diff adding a second one anywhere else in `crates/sdk` has something
concrete to point at.

## 1. `crates/sdk/clippy.toml` (new) and `crates/kernel/clippy.toml` (new)

Each file repeats `allow-unwrap-in-tests = true` (required - see above, and
confirmed still needed going forward: TASK-1.34's approved plan, adding
`ghostty-vt` as a kernel dependency, replaces the kernel's placeholder
`#[cfg(test)] mod tests` with its own AC#3 test and explicitly says it will
"keep one `.unwrap()` in the new test's setup rather than drop the only thing
proving that `clippy.toml` line still does something" - i.e. that task is
already written assuming this one lands `allow-unwrap-in-tests` into
`crates/kernel/clippy.toml`. Whichever of the two tasks merges second should
expect to carry this file forward unchanged into its own rebase) plus:

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
`ghostty-vt`, TASK-1.34's new kernel dependency, needs no entry either way:
its own plan states its Rust API does no I/O at runtime (feeding bytes in,
reading cells back is pure computation; the network fetch and Zig link are
build-time) and gives `boundaries.toml`'s reason for it on that basis -
consistent with, not an exception to, this list. List is deliberately not
exhaustive of every std I/O function that exists; `.claude/rules/sdk.md` and
`kernel.md` say so, so nobody goes hunting for the one syscall wrapper that
slipped through instead of trusting the mechanism.

The sdk's `local-socket` provider (future work, TASK-1.70/1.44) is this
crate's one permitted exception (R103). It and its own test module (clippy
sees test code too under `--all-targets`, and real I/O against a temp
listener is this crate's planned test technique per TASK-1.70's notes) each
carry their own
`#[expect(clippy::disallowed_types, clippy::disallowed_methods, reason = "R103: the local-socket provider is the sdk's one permitted I/O")]`,
scoped to that module only - nowhere else in the crate (see "Keeping the
exemption from spreading" above).

## 2. AC#2: `tools/src/steg_tools/affected.py`, `boundaries.py`, and `just packages` joining `just check`

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
this is the right, static, unification-immune signal for catching the
**declaration** (kernel's own Cargo.toml asking for the feature).

That static check cannot catch the **use** half of the same hole: verified in
the real repository, not just the scratch workspace, that `cargo build -p
steg-kernel -v` never passes `--cfg feature="local-socket"` when compiling
`steg_sdk` - kernel built alone is correctly pure - but `cargo build
--workspace` (what `just lint`/`just test`/`just build`/`just check` all run)
unifies features across every member in one invocation, so once some other
member (the app or `steg`, as R103 expects) turns `local-socket` on, the
*same* `--workspace` build compiles kernel against an sdk instance that
textually *could* satisfy a call into that module if kernel's own source
ever added one - and `cargo build --workspace` would let that compile
silently, with nothing in `just check` today that would fail. This was
proven directly with a throwaway workspace reproducing exactly this shape
before writing the first version of this plan.

**Engineer confirmed (yes): add `just packages` (already exists, builds and
tests every crate alone with `-p`, with no `--all-features` of its own - see
AC#1's note above on why that omission has to stay) to `just check`.** This
is what closes the *use* half: because `just packages`'s per-crate `-p`
invocations never unify with any other member, kernel built that way can
never successfully compile a call into the sdk's optional parts, so a
mistake there fails `just check` the same way a normal compile error would -
rather than only failing accidentally, depending on which recipe happens to
run `-p` versus `--workspace` that day.

**Cost, measured warm in the real worktree** (today's five frame crates, no
real behaviour yet, no `ghostty-vt`): `just packages` alone, fully warm
(second consecutive run), is 1.7s wall; `just check` alone, fully warm, is
15.4s wall (13.5s of that is `lint`). Adding `packages` to `check` today is
roughly an 11% addition to a warm run. Caveat for the engineer: this is
measured before TASK-1.34 lands `ghostty-vt` in the kernel; once it does,
`just packages`'s standalone `cargo build -p steg-kernel` pays its own link
step against the Zig-built `libghostty-vt` artifacts in addition to the
`--workspace` build's. Cargo's build cache is keyed the same way regardless
of `-p` vs `--workspace` for a given target dir and feature set, so this
should stay a relink rather than a recompile, but that hasn't been measured
since `ghostty-vt` isn't in this worktree yet - flag it as something to
re-measure once TASK-1.34 merges, not something this task should guess at.

Changes to the check tooling itself:

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
  existing `reason` string - no new reason field needed). Note for whoever
  rebases against TASK-1.34: that task edits the same `[steg-kernel]` table
  to extend `allow` with `"ghostty-vt"` and reword `reason`; this task only
  adds the new `deny-features` key, so the two edits should merge cleanly,
  but check by hand on rebase since both touch the same TOML table.
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
    any feature of that dependency is denied) that closes a gap the advisor
    flagged: today sdk has no `default` feature, so this never fires, but it
    catches one being added later without anyone revisiting this check.
    kernel's Cargo.toml already sets `default-features = false`, so this
    costs it nothing today.
- `format_feature_violation(violation)`: names the crate, the dependency, the
  feature, and the reason (AC#3).
- `main()`: also runs `feature_violations`, prints each with
  `format_feature_violation`, and fails (exit 1) if either list is non-empty.
- Edit the stale comment in `crates/kernel/Cargo.toml`
  ("`just boundaries`... cannot check that `local-socket` stays off") to name
  the new check instead.
- `justfile`: add `packages` to `check`'s recipe line
  (`check: fmt-check lint boundaries deny test test-tools packages` - or
  after `test-tools`, order doesn't matter since each step is independent
  and a failure anywhere already stops the chain under `set -euo pipefail`).

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
  convention (including its test module, exactly it and nowhere else) and
  that the list in `crates/sdk/clippy.toml` isn't exhaustive by design.
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
- The dependency-table bullet gains the feature half of R110, and now the
  `just packages`-in-`just check` half too: "... `boundaries.toml` holds
  doc-16's table of which package may depend on which and which of an
  allowed dependency's features it may turn on, each with a reason; `just
  boundaries` fails on anything added that isn't listed, such as the kernel
  declaring the sdk's `local-socket` feature in its own `Cargo.toml` (doc-16
  R104, R110). `just packages` (part of `just check`) builds and tests each
  package alone, with no other member's features unified in, so a kernel
  change that only *compiles* against that feature because some other
  package turned it on in the same `--workspace` run fails there instead of
  passing silently."
- The sdk-is-the-public-interface bullet stays (it's about steg/server/app as
  consumers together, genuinely cross-package) but drops "does no I/O of its
  own" (now stated once, in the merged bullet above) - keeps "a consumer
  supplies the transport... none holds protocol code of its own (R101)".

## 5. AC#6: what the plugin's layer rules should become (for the orchestrator,
   through `asdlc-plugin` - not edited here)

Re-read against `origin/main`, not the stale snapshot the first version of
this plan was written against: `plugins/bruk-io-steg/rules/cli.md` is already
gone and `binary.md` already covers `crates/steg` alone, `sdk.md` already
exists, and `kernel.md`/`server.md` no longer say "TASK-1.69 adds it" - all
of that was fixed when the plugin was republished after TASK-1.69. Drop the
earlier version of this section's claims about `cli.md`/stale TASK-1.69 notes
entirely; they no longer hold.

What's still true, read against the current content: the plugin's
`sdk.md`/`kernel.md`/`server.md` already state, in their own words, several
of the same facts this task's new `.claude/rules/{sdk,kernel,server}.md` and
`crates/{sdk,kernel,server}/CLAUDE.md` state (for example, plugin `sdk.md`'s
"The one part that does I/O is the local-socket provider... the kernel never
turns it on (R103, R105)" versus this task's own sdk rule and CLAUDE.md
saying the same thing from two more angles). A kernel-file touch would then
load three carriers - the plugin's `layer: kernel` rule, `.claude/rules/
architecture.md` plus the new `.claude/rules/kernel.md`, and `crates/kernel/
CLAUDE.md` - some of them saying the identical sentence, which is exactly
what R111 refuses ("nothing is stated in... two rules").

Recommend the orchestrator trim each plugin layer rule to a pointer at this
repository's own carriers, keeping only whatever is genuinely asdlc/board-
specific and has no other home:
- `kernel.md`, `server.md`, `sdk.md`, `app.md`, `binary.md`: replace the
  restated boundary content with one line - "see `crates/<pkg>/CLAUDE.md`
  for what this package is and its tests, and `.claude/rules/<pkg>.md`
  (where one exists) for the judgement calls `boundaries.toml` and the lint
  can't make for you." `app.md`'s gpui-pin/design-artifact/untestable-main
  bullets have no other home today (there is no new `.claude/rules/app.md`
  from this task), so they stay; `binary.md`'s "no command/flag/exit status
  written by hand" bullet likewise has no other home and stays.

This is a judgement call about where the line sits between "this repository's
own memory" and "the board's view of this repository," left for the
orchestrator and @bruk to decide, not something this plan forces.

## Order of work

1. `crates/sdk/clippy.toml`, `crates/kernel/clippy.toml`.
2. `affected.py`/`boundaries.py`/`boundaries.toml`/`justfile` changes, with
   the unit tests, before touching any rule/CLAUDE.md prose (so the mechanism
   named in the plan is real before the prose points at it).
3. The five `CLAUDE.md` files.
4. `.claude/rules/sdk.md`, `kernel.md`, `server.md` (new); trim
   `architecture.md`.
5. Fix `crates/kernel/Cargo.toml`'s stale comment.
6. Write up the AC#6 section above into the task's notes/comments for the
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
- After `justfile`'s `check` recipe gains `packages`: add a kernel-only
  function that calls through to one of the sdk's (hypothetical, reverted)
  `local-socket`-gated items directly in source (not via Cargo.toml) while
  some other crate's Cargo.toml turns the feature on - record that `just
  lint`/`just build` (both `--workspace`) pass while `just packages` fails on
  that one line alone, demonstrating the gap this half of AC#2 closes; revert
  both edits afterward.
- `asdlc record-check ... -- just test-tools` for the new Python unit tests.
- Final `asdlc record-check ... -- just check` once every file change is in
  place, as the task's closing evidence.

## Risks / things the engineer already confirmed, and what's left

- `just lint` runs `--all-features` (confirmed yes) - verified safe against
  the real worktree, no feature anywhere currently changes what builds or
  fetches; the sdk's local-socket module needs its own `#[expect(...)]` once
  real code lands there (TASK-1.70/1.44), named in `.claude/rules/sdk.md`.
- `just packages` joins `just check` (confirmed yes) - warm cost measured at
  ~1.7s against a ~15.4s warm `just check`, ~11% today; re-measure once
  `ghostty-vt` lands (TASK-1.34), since this plan hasn't measured that.
- The disallowed list is a curated, non-exhaustive set of `std` paths, not a
  provable total ban on I/O (nothing in clippy's config language expresses
  "every function under `std::fs`"); `.claude/rules/sdk.md`/`kernel.md` say so
  explicitly so a reviewer doesn't read the lint as a stronger guarantee than
  it is.
- Two tasks (`TASK-1.34`, building `ghostty-vt` into the kernel) touch the
  same two files this task does: `crates/kernel/Cargo.toml` (its comment) and
  `boundaries.toml`'s `[steg-kernel]` table (its `allow`/`reason` versus this
  task's new `deny-features`). Flagged above at each touch point; whichever
  lands second resolves the merge by hand, not blocking either plan.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Plan checked against doc-16 (R104, R110, R111) by @bruk.o: ok. Per-package clippy configuration refuses I/O in the sdk and the kernel, with the reason in the message; a deny-features table in boundaries.toml refuses the kernel turning on the sdk's socket provider; just lint runs with all features and the provider's module carries the one reasoned exemption; just packages joins just check to catch Cargo's feature unification. Each statement has one home: lint, a path-scoped rule, or a package's CLAUDE.md. After it merges the orchestrator trims the plugin's layer rules to pointers through asdlc-plugin. The cost of just packages in check is to be measured again once the kernel builds ghostty's engine.

Check passed: `just check` exited 0 at 2026-11-15T03:25:10+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.71 on task-1.71-boundaries-a-claude-md-for-each-package at 1c5c1365a62c:

```text
ok      format check (0.1s)
ok      lint (0.3s)
ok      boundaries (0.1s)
ok      licences (0.6s)
skip    tests: this change touches no crate
ok      tool tests (0.2s)
ok      architecture (0.8s)
```

Check passed: `just lint` exited 0 at 2026-11-15T03:25:36+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.71 on task-1.71-boundaries-a-claude-md-for-each-package at 1c5c1365a62c with uncommitted changes:

```text
ok      lint (0.4s)
```

Check passed: `just check` exited 0 at 2026-11-15T03:26:22+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.71 on task-1.71-boundaries-a-claude-md-for-each-package at 1c5c1365a62c with uncommitted changes:

```text
ok      format check (0.1s)
ok      lint (0.3s)
ok      boundaries (0.1s)
ok      licences (0.6s)
ok      tests (every crate) (0.3s)
ok      build (steg-sdk) (0.1s)
ok      test (steg-sdk) (0.1s)
ok      build (steg-kernel) (0.1s)
ok      test (steg-kernel) (0.1s)
ok      build (steg-server) (0.1s)
ok      test (steg-server) (0.1s)
ok      build (steg) (0.1s)
ok      test (steg) (0.1s)
ok      build (steg-app) (0.2s)
ok      test (steg-app) (0.2s)
ok      tool tests (0.2s)
ok      architecture (1.0s)
```

Check passed: `sh -c '! just lint'` exited 0 at 2026-11-15T03:26:34+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.71 on task-1.71-boundaries-a-claude-md-for-each-package at 1c5c1365a62c with uncommitted changes, last 40 of 41 lines:

```text
    Checking steg-kernel v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.71/crates/kernel)
error: docs for function which may panic missing `# Panics` section
  --> crates/kernel/src/lib.rs:14:1
   |
14 | pub fn demo_disallowed_io(path: &str) -> std::fs::File {
   | ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
   |
note: first possible panic found here
  --> crates/kernel/src/lib.rs:15:5
   |
15 |     std::fs::File::open(path).expect("demo only")
   |     ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
   = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.98.0/index.html#missing_panics_doc
   = note: requested on the command line with `-D clippy::missing-panics-doc`

error: use of a disallowed type `std::fs::File`
  --> crates/kernel/src/lib.rs:14:42
   |
14 | pub fn demo_disallowed_io(path: &str) -> std::fs::File {
   |                                          ^^^^^^^^^^^^^
   |
   = note: doc-16 R110: no file I/O in the kernel
   = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.98.0/index.html#disallowed_types
   = note: `-D clippy::disallowed-types` implied by `-D warnings`
   = help: to override `-D warnings` add `#[allow(clippy::disallowed_types)]`

error: use of a disallowed type `std::fs::File`
  --> crates/kernel/src/lib.rs:15:5
   |
15 |     std::fs::File::open(path).expect("demo only")
   |     ^^^^^^^^^^^^^
   |
   = note: doc-16 R110: no file I/O in the kernel
   = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.98.0/index.html#disallowed_types

error: could not compile `steg-kernel` (lib) due to 3 previous errors
warning: build failed, waiting for other jobs to finish...
error: could not compile `steg-kernel` (lib test) due to 3 previous errors
full output: .quiet/lint.log
error: recipe `lint` failed on line 70 with exit code 101
```

Check passed: `just lint` exited 0 at 2026-11-15T03:26:43+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.71 on task-1.71-boundaries-a-claude-md-for-each-package at 1c5c1365a62c with uncommitted changes:

```text
ok      lint (0.4s)
```

Check passed: `sh -c '! just lint'` exited 0 at 2026-11-15T03:26:53+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.71 on task-1.71-boundaries-a-claude-md-for-each-package at 1c5c1365a62c with uncommitted changes, last 40 of 41 lines:

```text
    Checking steg-sdk v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.71/crates/sdk)
error: docs for function which may panic missing `# Panics` section
  --> crates/sdk/src/lib.rs:15:1
   |
15 | pub fn demo_disallowed_io(path: &str) -> std::fs::File {
   | ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
   |
note: first possible panic found here
  --> crates/sdk/src/lib.rs:16:5
   |
16 |     std::fs::File::open(path).expect("demo only")
   |     ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
   = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.98.0/index.html#missing_panics_doc
   = note: requested on the command line with `-D clippy::missing-panics-doc`

error: use of a disallowed type `std::fs::File`
  --> crates/sdk/src/lib.rs:15:42
   |
15 | pub fn demo_disallowed_io(path: &str) -> std::fs::File {
   |                                          ^^^^^^^^^^^^^
   |
   = note: doc-16 R110: no file I/O in the sdk
   = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.98.0/index.html#disallowed_types
   = note: `-D clippy::disallowed-types` implied by `-D warnings`
   = help: to override `-D warnings` add `#[allow(clippy::disallowed_types)]`

error: use of a disallowed type `std::fs::File`
  --> crates/sdk/src/lib.rs:16:5
   |
16 |     std::fs::File::open(path).expect("demo only")
   |     ^^^^^^^^^^^^^
   |
   = note: doc-16 R110: no file I/O in the sdk
   = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.98.0/index.html#disallowed_types

error: could not compile `steg-sdk` (lib) due to 3 previous errors
warning: build failed, waiting for other jobs to finish...
error: could not compile `steg-sdk` (lib test) due to 3 previous errors
full output: .quiet/lint.log
error: recipe `lint` failed on line 70 with exit code 101
```

Check passed: `just lint` exited 0 at 2026-11-15T03:27:04+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.71 on task-1.71-boundaries-a-claude-md-for-each-package at 1c5c1365a62c with uncommitted changes:

```text
ok      lint (0.5s)
```

Check passed: `just test-tools` exited 0 at 2026-11-15T03:28:52+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.71 on task-1.71-boundaries-a-claude-md-for-each-package at 1c5c1365a62c with uncommitted changes:

```text
ok      tool tests (0.2s)
```

Check passed: `just boundaries` exited 0 at 2026-11-15T03:28:56+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.71 on task-1.71-boundaries-a-claude-md-for-each-package at 1c5c1365a62c with uncommitted changes:

```text
ok      boundaries (0.1s)
```

Check passed: `sh -c '! just boundaries'` exited 0 at 2026-11-15T03:29:05+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.71 on task-1.71-boundaries-a-claude-md-for-each-package at 1c5c1365a62c with uncommitted changes:

```text
FAILED  boundaries (exit 1, 0.1s)
steg-kernel turns on steg-sdk's "local-socket" feature, which its rule in boundaries.toml denies (the kernel does no I/O and depends on the sdk only for the shapes that cross the API, never for its optional parts (doc-16 R105)).
full output: .quiet/boundaries.log
error: recipe `boundaries` failed on line 78 with exit code 1
```

Check passed: `just boundaries` exited 0 at 2026-11-15T03:29:16+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.71 on task-1.71-boundaries-a-claude-md-for-each-package at 1c5c1365a62c with uncommitted changes:

```text
ok      boundaries (0.1s)
```

Check passed: `just build` exited 0 at 2026-11-15T03:30:05+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.71 on task-1.71-boundaries-a-claude-md-for-each-package at 1c5c1365a62c with uncommitted changes:

```text
ok      build (0.3s)
```

Check passed: `sh -c '! just packages'` exited 0 at 2026-11-15T03:30:08+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.71 on task-1.71-boundaries-a-claude-md-for-each-package at 1c5c1365a62c with uncommitted changes:

```text
ok      build (steg-sdk) (0.1s)
ok      test (steg-sdk) (0.3s)
FAILED  build (steg-kernel) (exit 101, 0.1s)
   Compiling steg-kernel v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.71/crates/kernel)
error[E0425]: cannot find function `demo_local_socket_only_fn` in crate `steg_sdk`
  --> crates/kernel/src/lib.rs:19:15
   |
19 |     steg_sdk::demo_local_socket_only_fn()
   |               ^^^^^^^^^^^^^^^^^^^^^^^^^ not found in `steg_sdk`
   |
note: found an item that was configured out
  --> crates/sdk/src/lib.rs:16:8
   |
15 | #[cfg(feature = "local-socket")]
   |       ------------------------ the item is gated behind the `local-socket` feature
16 | pub fn demo_local_socket_only_fn() -> i32 {
   |        ^^^^^^^^^^^^^^^^^^^^^^^^^

For more information about this error, try `rustc --explain E0425`.
error: could not compile `steg-kernel` (lib) due to 1 previous error
full output: .quiet/build-steg-kernel.log
error: recipe `packages` failed with exit code 101
```

Check passed: `just check` exited 0 at 2026-11-15T03:30:35+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.71 on task-1.71-boundaries-a-claude-md-for-each-package at 1c5c1365a62c with uncommitted changes:

```text
ok      format check (0.1s)
ok      lint (0.5s)
ok      boundaries (0.1s)
ok      licences (0.6s)
ok      tests (every crate) (1.2s)
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
ok      tool tests (0.2s)
ok      architecture (0.8s)
```

Check passed: `just check` exited 0 at 2026-11-15T03:31:49+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.71 on task-1.71-boundaries-a-claude-md-for-each-package at 1c5c1365a62c with uncommitted changes:

```text
ok      format check (0.1s)
ok      lint (0.4s)
ok      boundaries (0.1s)
ok      licences (0.7s)
ok      tests (every crate) (0.4s)
ok      build (steg-sdk) (0.1s)
ok      test (steg-sdk) (0.2s)
ok      build (steg-kernel) (0.2s)
ok      test (steg-kernel) (0.2s)
ok      build (steg-server) (0.2s)
ok      test (steg-server) (0.3s)
ok      build (steg) (0.2s)
ok      test (steg) (0.2s)
ok      build (steg-app) (0.6s)
ok      test (steg-app) (0.5s)
ok      tool tests (0.5s)
ok      architecture (2.1s)
```

Check passed: `just check` exited 0 at 2026-11-15T03:33:35+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.71 on task-1.71-boundaries-a-claude-md-for-each-package at 9f9585ebdc03:

```text
ok      format check (0.1s)
ok      lint (0.5s)
ok      boundaries (0.1s)
ok      licences (0.6s)
ok      tests (every crate) (0.4s)
ok      build (steg-sdk) (0.1s)
ok      test (steg-sdk) (0.1s)
ok      build (steg-kernel) (0.1s)
ok      test (steg-kernel) (0.1s)
ok      build (steg-server) (0.1s)
ok      test (steg-server) (0.1s)
ok      build (steg) (0.1s)
ok      test (steg) (0.1s)
ok      build (steg-app) (0.3s)
ok      test (steg-app) (0.3s)
ok      tool tests (0.2s)
ok      architecture (0.8s)
```

Built per the approved plan, with two things not in the plan's own file-change
list but confirmed by the plan's own prose (both engineer-approved): the
justfile `lint` recipe gained `--all-features`, and `packages` joined `check`.
Both were measured and demonstrated before relying on them, as recorded below.

AC#1 (sdk/kernel I/O fails just lint, mechanism named, reason in the message):
proved by two recorded demonstrations, each added-then-reverted - a deliberate
`std::fs::File` use in crates/kernel/src/lib.rs, and the same in
crates/sdk/src/lib.rs. Each failed `! just lint` with clippy's
`disallowed_types` message naming "doc-16 R110: no file I/O in the
<crate>", then passed `just lint` again after revert. The mechanism
(`clippy::disallowed_types`/`disallowed_methods`, configured in
`crates/{kernel,sdk}/clippy.toml`) also covers socket, process and clock
paths, visible directly in those two files - not separately demonstrated
live, since it's the same clippy mechanism on a different path each time.
"An async runtime" is covered by the pre-existing `boundaries.toml`
dependency-name check (neither crate may depend on `tokio` at all), unchanged
by this task, so not re-demonstrated here.

AC#2 (kernel turning on the sdk's optional part fails just boundaries, with a
tools/tests test): the declaration half - recorded `! just boundaries` failing
when `crates/kernel/Cargo.toml` adds `features = ["local-socket"]` to its
`steg-sdk` dependency, naming doc-16 R105 in the message, then passing again
after revert. Unit tests: `test_a_denied_feature_declared_on_the_edge_is_a_violation`,
`test_default_features_on_a_denied_dependency_is_a_violation`, and others in
`tools/tests/unit/test_boundaries.py`, run via recorded `just test-tools`.
The complementary use half (kernel's own source compiling against the sdk's
optional parts only because some other workspace member turned the feature
on, which the declared-edge check can't see) - recorded `just build` passing
and `! just packages` failing on exactly that, reverted after. `just packages`
now joins `just check`.

AC#3 (every refusal names the requirement and why): visible in every
recorded failure above - clippy prints the configured `reason` string
("doc-16 R110: ..."), and `just boundaries`'s `FeatureViolation`/`Violation`
messages print the crate's `reason` from `boundaries.toml` ("doc-16 R105
..."). `format_feature_violation`/`format_violation` have their own unit
tests asserting the crate, dependency, feature and reason are all present.

AC#4 (CLAUDE.md per package, none restates a lint): `crates/{sdk,kernel,
server,steg,app}/CLAUDE.md` added - what/depends/tests/doc-16 pointer each.
Tests are stated as `just test`/`just packages`, not `cargo test -p`, per
root CLAUDE.md's "commands go through just, never cargo directly" - a
departure from the plan's literal wording, not its intent. Checked with
`grep -rn -i "clippy\|disallowed" crates/*/CLAUDE.md`: no match, so none
restates a lint.

AC#5 (.claude/rules/ carries judgement, scoped by paths; lint-covered lines
replaced in architecture.md): new `.claude/rules/{sdk,kernel,server}.md`,
each `paths`-scoped to its one package. `architecture.md`'s old "no sockets,
no files, no clock, no process spawning" enumeration is replaced by a bullet
naming `clippy::disallowed_types`/`disallowed_methods` and the two
`clippy.toml` paths; the dependency-table bullet names `just packages` as
the use-side half of the same boundary. Recorded `just check` passing after
this edit.

AC#6 (plan lists what the plugin's layer rules should become): on the task's
plan already (re-verified against origin/main's current, already-republished
plugin content, not the stale snapshot the plan's first draft was written
against). Nothing in `plugins/bruk-io-steg` touched in this worktree -
confirmed with `git diff --stat HEAD -- plugins/` showing no changes.

Known consequence worth flagging, not fixed here: moving "the server... never
keeps a copy it treats as the truth" out of `architecture.md` (into the new
`server.md`) changes which file holds it. An active decision record
(decision-2) and an archived spike (task-1.2) cite
"`.claude/rules/architecture.md:11`" by file and line for that exact
sentence. The sentence still exists verbatim, now in `.claude/rules/
server.md` (also, coincidentally, line 11), but the citations' file path is
now wrong. Decision records are only hand-edited through their own PR per
team convention, so not changed from this worktree - flagging for whoever
next touches decision-2.

Every check above was recorded through `asdlc record-check`, including the
demonstrations that were reverted. `just check` recorded clean before any
change, after the clippy.toml/justfile changes, after the Python/boundaries.toml
changes, after the CLAUDE.md and rules changes, and once more after rebasing
onto origin/main just before pushing.

Check passed: `just check` exited 0 at 2026-11-15T03:41:21+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.71 on task-1.71-boundaries-a-claude-md-for-each-package at ada4026cced6 with uncommitted changes:

```text
ok      format check (0.1s)
ok      lint (1.3s)
ok      boundaries (0.1s)
ok      licences (0.8s)
ok      tests (every crate) (2.3s)
ok      build (steg-sdk) (0.2s)
ok      test (steg-sdk) (0.1s)
ok      build (steg-kernel) (0.1s)
ok      test (steg-kernel) (0.1s)
ok      build (steg-server) (0.1s)
ok      test (steg-server) (0.1s)
ok      build (steg) (0.2s)
ok      test (steg) (0.1s)
ok      build (steg-app) (0.7s)
ok      test (steg-app) (0.3s)
ok      tool tests (0.3s)
ok      architecture (1.0s)
```

Commits since the review: 25, 22 without the Asdlc-Worker trailer.

Fixed the one Important finding and checked for the same mistake elsewhere.

crates/sdk/CLAUDE.md:6-8 duplicated .claude/rules/sdk.md:11-13's R106 content
verbatim ("a required dependency is for data and serialisation only", the
GPUI/async-runtime ban). Removed from CLAUDE.md. While there, corrected the
remaining dependency line to match boundaries.toml's actual `[steg-sdk]`
table (an empty allow list, so "nothing" is accurate; the sdk may not depend
on anything, steg's or external, not only "any other steg package" - the
narrower phrasing I'd had was itself slightly wrong, independent of the
duplication).

Read the other four CLAUDE.md files against every rule that applies to their
package (the new .claude/rules/{sdk,kernel,server}.md plus the repo-wide
architecture.md, grepped for shared multi-word phrases, not just eyeballed)
and found two more of the same mistake:
- crates/app/CLAUDE.md's "with no private channel" restated
  architecture.md's R36 bullet ("The app gets no private channel, so
  anything the app can do, the CLI and a plugin can do too"). Removed.
- crates/server/CLAUDE.md's "owns every session, tab, pane and running
  program" restated .claude/rules/server.md's R1/R2 bullet verbatim.
  Removed.
crates/kernel/CLAUDE.md and crates/steg/CLAUDE.md had no overlapping phrase
with kernel.md, architecture.md, or (for steg, which has no new rule of its
own from this task) architecture.md - left unchanged. Confirmed with a grep
for each phrase judged ("no private channel", "owns every session", "GPUI
type", "async runtime", "data and serialisation", "server logic", "never
keeps a copy", "optional parts", "no I/O") across all five CLAUDE.md files
and the four rule files together - each now appears in exactly one file.

PR 18 (the just-recipe rename) was still open, not merged, when checked
(`gh pr view 18` and `git show origin/main:justfile`) - left every recipe
name as it was (`packages`, `boundaries`, `fmt-check`, `architecture`,
`affected`), matching the instruction for that case.

Brought in origin/main with a merge (the branch was already pushed): PR 15
(TASK-1.35) landed real code in crates/kernel/src (pane, session, tab, tree,
colour, error, ids) and crates/sdk/src (colour, ids, pane_kind) since this
task's last review. No conflicts. Recorded `just check` clean against that
real code with the clippy.toml/boundaries.toml changes from this task still
in place - the new disallowed-types/methods lists don't false-positive on
any of it.

Pull request (unchanged): https://github.com/bruk-io/steg/pull/17
<!-- SECTION:NOTES:END -->

## Comments

<!-- COMMENTS:BEGIN -->
author: @bruk
created: 2026-11-15T03:24:00Z

---

Plan approved by @bruk (d8f3885f55ed, size-3): its worker may build it.

---

author: @bruk.r1
created: 2026-11-15T03:34:00Z

---

@bruk.r1 is reviewing TASK-1.71: a same-machine review: it waited 0m with no other engineer's reviewer, longer than review_fallback_after (0m), so a fresh reviewer on its author's machine takes it.

---

author: @bruk.r1
created: 2026-11-15T03:37:00Z

---

Review by @bruk.r1 (same-machine): changes requested. Back to In Progress for the author's worker.

Findings: 1 Important, 0 nit. Cites: review. The diff matches the plan.

[Compliance, Important] crates/sdk/CLAUDE.md:6-8 and .claude/rules/sdk.md:11-13 state the same fact twice: "a required dependency is for data and serialisation only" and the GPUI/async-runtime ban appear verbatim in both the package's CLAUDE.md and the new path-scoped rule. R111 says each package's CLAUDE.md holds "what it may and may not depend on" at the boundaries.toml/dependency-table level, and the rule holds "what is left to judgement about a boundary" - R106's "no GPUI type, no particular async runtime, data/serialisation-only dependency" is explicitly judgement (sdk.md's own text calls it out: "this is about what the sdk's own public types expose, which a dependency boundary can't see"), so it belongs in sdk.md only. Folding it into CLAUDE.md's "may not depend on" line duplicates it, which is exactly what R111 refuses ("nothing is stated in... two rules") when the two carriers are read together as this review required.

Everything else checked out:
- Both per-package clippy.toml files correctly repeat allow-unwrap-in-tests (clippy does not merge clippy.toml files; verified directly, and confirmed by the plan's own recorded demonstration) - nothing from the root file is silently lost, and every disallowed-types/methods entry's reason names doc-16 R110 and the crate.
- just lint runs --all-features (confirmed in the justfile diff), so the sdk's local-socket module is linted too, and that module's own #[expect(...)] is the one named exemption (.claude/rules/sdk.md).
- just packages genuinely builds/tests each package alone with cargo -p and no --all-features, is joined into just check's recipe line, and is exactly what closes the feature-unification gap AC#2 describes; boundaries.toml's new deny-features table and its conservative default-features rule are implemented and covered by tools/tests/unit/test_boundaries.py (declared-feature violation, no-violation, default-features violation, format, and read_allow_table parsing cases).
- Every refusal's message names its requirement and why: clippy's disallowed_types/methods print "doc-16 R110: ... in the <crate>"; format_feature_violation and the existing format_violation print boundaries.toml's reason string, which names R105/R104.
- architecture.md's two lint-covered bullets (kernel/sdk I/O, the dependency table) are replaced by bullets naming the lint and pointing at the new per-package rules; the kernel-is-logic-only and server-owns-every-session bullets were moved whole into the new kernel.md/server.md, not left duplicated in architecture.md.
- CI (the "check" workflow) is green on the pull request's head commit (9f9585e).
- AC#6 is answered in the plan/notes for the orchestrator, and plugins/ is untouched in this branch, confirmed with git diff --stat.
- The just-recipes-not-cargo-test-p departure in the CLAUDE.md files matches root CLAUDE.md's own instruction and is explained in the notes.

---

created: 2026-11-15T03:47:00Z

---

From @bruk.o: commits after the hand-back. The worker handed this back to review at an earlier head, then, on the orchestrator's instruction and before any reviewer claimed it, merged origin/main (pull requests 15 and 18) and switched every old just recipe name to the new one. The board takes no recorded check on a task that is In Review, so the check at the new head 5ba2098 is pull request 17's CI run 43673858878 (success), plus a just check the worker ran unrecorded. Conflicts resolved in .claude/rules/architecture.md, crates/kernel/Cargo.toml and the justfile. The reviewer is asked to review at 5ba2098 and to confirm no old recipe name remains in this change's files.

---
<!-- COMMENTS:END -->
