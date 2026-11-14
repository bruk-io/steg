---
id: TASK-1.5
title: Set up the Rust workspace, the licence and one check command
status: In Progress
assignee:
  - '@bruk.w3'
created_date: '2026-11-13T22:52:00Z'
updated_date: '2026-11-14T03:56:00Z'
labels:
  - size-2
milestone: m-0
dependencies: []
parent_task_id: TASK-1
type: task
project: bruk-io-steg
asdlc_accepted_from: DRAFT-6
asdlc_branch: task-1.5-set-up-the-rust-workspace-the-licence
asdlc_plan_approved_by: bruk
asdlc_plan_approved_hash: 'e0acb049d09a'
asdlc_plan_approved_size: size-2
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The frame everything else lands in: crates for the kernel, for the server and CLI binary, and for the app. R59 (MIT), R61 (the server and CLI are one binary), R58 (no telemetry, no network call the user did not ask for).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 One command runs formatting, lints and tests, and passes
- [ ] #2 The server and the CLI build as a single binary (R61)
- [ ] #3 The repository carries the MIT licence (R59)
- [ ] #4 The GPUI version is pinned
- [ ] #5 The check command refuses a dependency whose licence is not on an allow list that excludes GPL and AGPL (R59)
- [ ] #6 The check command refuses a crate dependency that crosses the boundaries in .claude/rules/architecture.md, starting with the kernel depending on anything that does I/O
- [ ] #7 The justfile's recipes (check, test, test-all, fmt, lint, deny, build, run) each do real work on the workspace, and none passes by doing nothing
- [ ] #8 just test runs only the crates a change touches and the crates that depend on them, chosen by steg-affected in tools/ and not by whoever runs it; a change to the workspace manifest, the lock file or the toolchain runs everything. The selection is proven against the real workspace, since tools/ has only been tested against made-up crates
- [ ] #9 Every recipe prints one line when it passes, and on failure prints the failure and the path of the full log (steg-quiet in tools/)
- [ ] #10 just setup installs or checks what the recipes need beyond Rust itself, cargo-deny first, and is safe to run again
- [ ] #11 The workspace is on the 2024 edition and sets its lints once in [workspace.lints], with warnings as errors, including the clippy lints .claude/rules/rust.md names as its enforcement: unwrap_used, missing_errors_doc, missing_panics_doc, undocumented_unsafe_blocks, wildcard_enum_match_arm, allow_attributes_without_reason and await_holding_lock; the kernel crate forbids unsafe code
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
Revision note (2026-11-14): Bruk decided the kernel boundary (AC#6) is enforced by an allow list built in tools/, not a cargo-deny deny list of crate names. This revision replaces that one mechanism and its probe; nothing else about the plan changed except where noted.

Files that change

Root (new):
- Cargo.toml - virtual workspace manifest. `resolver = "3"` (a virtual manifest does not inherit the resolver from `edition = "2024"`, so this is required, not decorative). `members = ["crates/kernel", "crates/server", "crates/cli", "crates/steg", "crates/app"]`. `[workspace.package]`: edition = "2024", license = "MIT", version = "0.1.0". `[workspace.dependencies]`: `gpui = { version = "=0.2.2" }` (pinned exact version - AC#4 - kept at the workspace level because it is shared by exactly the crates that will eventually need it, the usual reason for a workspace dependency; there is no `[bans]` section in this plan for it to satisfy). `[workspace.lints.clippy]`: unwrap_used, missing_errors_doc, missing_panics_doc, undocumented_unsafe_blocks, wildcard_enum_match_arm, allow_attributes_without_reason and await_holding_lock all set to "deny" directly - AC#11 says the workspace's lints are "warnings as errors" in `[workspace.lints]` itself, so the table is where that lives. This only takes effect under `cargo clippy`, not `cargo build`/`cargo test` (clippy lints are inert to rustc) - the proof below says so explicitly, rather than implying `cargo build` enforces them.
- clippy.toml - `allow-unwrap-in-tests = true`. Confirmed against the installed clippy's own lint configuration (clippy 0.1.98, `clippy_config::conf`): the key is `allow-unwrap-in-tests`, read from a `clippy.toml` at the workspace root, default `false`. Without this, `unwrap_used` set to "deny" fires inside `#[cfg(test)]` code too, which contradicts rust.md's "No `unwrap` outside tests."
- LICENSE - plain MIT text, copyright line matching the identity already used in this repository's commits (R59).
- rust-toolchain.toml - channel "1.98.1" (the machine's installed toolchain), components ["rustfmt", "clippy"], so the 2024-edition toolchain is pinned, not assumed.
- deny.toml - licences only: `[licenses] allow = [...]` seeded with the permissive licenses already known to appear in gpui's dependency tree (MIT, Apache-2.0, Apache-2.0 WITH LLVM-exception, BSD-2-Clause, BSD-3-Clause, ISC, Unicode-3.0, Zlib), expanded only with more permissive licenses once the real lockfile shows what's actually there. No `[bans]` section - the kernel/server boundary (AC#6) is enforced in tools/, not here.
- boundaries.toml - the one obvious file the allow list lives in, at the repository root beside deny.toml, justfile and Cargo.toml (not buried in tools/, since it is a project-wide architecture contract that `tools/` happens to be the thing that reads). One table per restricted crate; a crate with no table is unrestricted. Each table carries the dependencies it may use (workspace crates and external crates alike) and the reason, so the check's failure message can name the rule it enforces without hard-coding crate-specific text:
  ```toml
  [steg-kernel]
  reason = "the kernel does no I/O of its own (.claude/rules/architecture.md)"
  allow = []
  ```
  steg-kernel starts with zero dependencies, so its allow list starts empty too: the first dependency anyone adds to the kernel is necessarily a deliberate edit to this file, which is the point.
- Cargo.lock - generated by the first `cargo build --workspace`, committed (this ships binaries).

Five new crates, each a thin frame (no behaviour - that is TASK-1.6 onward). Every crate manifest sets `edition.workspace = true` and `license.workspace = true` explicitly (AC#3 depends on that inheritance actually being wired on each crate, not just declared once at the workspace):
- crates/kernel (package steg-kernel, lib): `#![forbid(unsafe_code)]` in src/lib.rs, `[lints] workspace = true`, no dependencies. One named `#[cfg(test)]` test, e.g. `fn a_test_may_unwrap()`, that calls `.unwrap()` on something trivial (`Some(1).unwrap()` is enough) - its only purpose is to exercise `allow-unwrap-in-tests` for real, since the crate otherwise has no behaviour to test yet.
- crates/server (package steg-server, lib): depends on steg-kernel via `path = "../kernel"`.
- crates/cli (package steg-cli, lib): no dependencies yet (the wire protocol and CLI design are open spikes - TASK-1.2, TASK-1.4).
- crates/steg (package steg, bin): depends on steg-server and steg-cli via path. This is the single binary that satisfies AC#2/R61. main.rs is a placeholder that does nothing yet.
- crates/app (package steg-app, bin): `gpui.workspace = true`. main.rs is a placeholder, kept to whatever is the smallest real call into gpui that proves the pin compiles and links (decided once gpui 0.2.2's actual exported API is read from docs.rs/source, not from memory, since GPUI is pre-1.0 and may have changed).

tools/ (existing package, no pyproject change expected for affected.py/quiet.py - testpaths already covers the whole tree; a new `[project.scripts]` entry is added for the boundary checker):
- tools/src/steg_tools/workspace.py (new) - a small shared helper, `repo_root()` and `cargo_metadata(root)`, factored out of affected.py's existing `main()` so both affected.py and the new boundaries.py run the same git-root-finding and `cargo metadata` subprocess logic instead of duplicating it. affected.py's `main()` is refactored to use it; its own tests are unaffected since `selection`/`owner`/`crates_from_metadata` keep their signatures.
- tools/src/steg_tools/affected.py - `Crate` gains a fourth field, `dependencies: frozenset[str]`, holding every direct dependency's name (workspace and external alike), separate from the existing `depends_on` (which stays workspace-only and keeps driving `with_dependents`/`selection` exactly as today). `crates_from_metadata` populates it from the same `package["dependencies"]` list, unfiltered. Also fixed wherever real `cargo metadata` output disagrees with the made-up-fixture unit tests' assumptions (AC#8's reason for existing) - specifically the `path` key's presence/format on dependency entries and how `manifest_path` resolves relative to the repo root for each of the five real crates.
- tools/src/steg_tools/boundaries.py (new) - the AC#6 check.
  - `BoundaryRule` (frozen dataclass): `allow: frozenset[str]`, `reason: str`.
  - `read_allow_table(path: Path) -> Mapping[str, BoundaryRule]` - thin I/O, parses boundaries.toml with the standard library's `tomllib`.
  - `Violation` (frozen dataclass): `crate: str`, `dependency: str`, `reason: str`.
  - `violations(crates: tuple[Crate, ...], rules: Mapping[str, BoundaryRule]) -> tuple[Violation, ...]` - pure: for every crate with an entry in `rules`, every one of its `dependencies` not in that entry's `allow` becomes a `Violation`. A crate absent from `rules` is unrestricted and never produces one. This is the testable decision; nothing here does I/O.
  - `unknown_rules(crates: tuple[Crate, ...], rules: Mapping[str, BoundaryRule]) -> frozenset[str]` - pure: the keys in `rules` that name no real crate. A typo in boundaries.toml (`steg-kernal`) would otherwise silently disable the check with no error; `main()` treats a non-empty result as a failure in its own right, printed before any violations.
  - `format_violation(v: Violation) -> str` - pure - `"{crate} depends on {dependency}, which is not on its allow list in boundaries.toml ({reason})."`, so the message always names the rule it enforces (architecture.md's text, carried through `reason`) without being specific to the kernel.
  - `main() -> int` - the thin shell: `workspace.repo_root()`, `workspace.cargo_metadata(root)`, `crates_from_metadata`, `read_allow_table(root / "boundaries.toml")`, then `unknown_rules(...)` and `violations(...)`; prints each with `format_violation` (or the unknown-crate message) and returns 1 if either is non-empty, else 0.
  - registered as the `steg-boundaries` console script in tools/pyproject.toml, next to steg-affected and steg-quiet.
- tools/tests/unit/test_boundaries.py (new) - `violations()` proven with made-up Crate tuples and rule tables, no doubles: a disallowed dependency on a restricted crate produces one Violation naming it; every dependency allowed produces none; a crate with no entry in the rules is never checked regardless of what it depends on; more than one restricted crate can each have their own violations at once. `unknown_rules()` proven to catch a rule naming a crate absent from the workspace. `read_allow_table` proven against a real TOML file in `tmp_path` (a temporary file is cheap and not a double, per testing-python.md) rather than mocking `tomllib`. `format_violation` proven to include the crate, the dependency and the reason text.
- tools/tests/adapter/test_affected_real_workspace.py (new) - runs real `cargo metadata --no-deps --format-version 1` against this repository's own workspace once it exists, feeds the JSON into `crates_from_metadata`, and asserts the five real crates and their real dependency edges (kernel -> server -> steg; cli -> steg; app standalone) come back correctly - the adapter proof AC#8 asks for, replacing "tested only against made-up crates." A second case feeds the same real crate tuple through `selection()` with synthetic changed-file lists (e.g. "crates/kernel/src/lib.rs" alone, "crates/app/src/main.rs" alone) and checks the expected `-p` sets - this is the part of the proof that doesn't depend on this branch's own diff (see Risks).
- tools/tests/unit/test_affected.py - updated for the new `dependencies` field on every `Crate(...)` construction (the shared `CRATES` fixture tuple, and the two expected `Crate` values in `test_crates_are_read_from_cargo_metadata`), extended so that fixture's expectations include a non-path, external dependency per crate (it already has one - `serde` on the kernel fixture, `tokio` alongside the path dependency on the server fixture - these just need to show up in the new field now).

justfile (existing, edited):
- `deny` recipe's command stays `cargo deny check licenses` (unchanged - no `bans`, since that mechanism moved to tools/).
- Add a `boundaries` recipe, following the same guarded, quiet-wrapped shape as `fmt`/`lint`/`deny`:
  ```
  # refuse a crate dependency that crosses the boundaries in .claude/rules/architecture.md (boundaries.toml)
  boundaries:
      @if [ "{{rust}}" = no ]; then echo "skip    boundaries: no Rust workspace yet"; else {{quiet}} "boundaries" -- uv run --quiet --project tools steg-boundaries; fi
  ```
- `check: fmt-check lint boundaries deny test test-tools` - `boundaries` added to the list.
- Add a `setup` recipe: checks `command -v cargo-deny`, and if absent runs `cargo install cargo-deny --locked` through `{{quiet}}`. Cargo-deny first and the only thing needed right now; safe to run again because it short-circuits when already installed. `check` is deliberately *not* given `setup` as a prerequisite: that would make the verification command install software as a side effect, and in CI (TASK-1.23) that is "build cargo-deny from source on every check run." If `cargo-deny` is missing, `deny`'s command fails and `{{quiet}}` prints the failure and the log path as it already does for any failing step - the log will show "command not found," enough to point someone at `just setup`.
No other justfile recipe needs to change for this task's acceptance criteria: `test`/`test-all`/`fmt`/`fmt-check`/`lint`/`build`/`affected` already do real work once `Cargo.toml` exists and already print one line through `{{quiet}}` on success (AC#9). `run` stays outside the quiet wrapper, as already settled (see Decisions standing below).

Order of work

1. Write the root Cargo.toml, clippy.toml, LICENSE, rust-toolchain.toml and the five crates' Cargo.toml/src files, including the kernel's `#[cfg(test)]` unwrap smoke test. No deny.toml or boundaries.toml yet - cargo-deny needs a resolvable lockfile to check against, and the allow list is easiest to write once the kernel's real, empty dependency list is confirmed.
2. `cargo build --workspace` for the first time. This is where gpui actually resolves and compiles (the slow, risky step - see Risks). Fix whatever the real GPUI 0.2.2 API disagrees with in app's placeholder main.rs. Commit the resulting Cargo.lock.
3. Install cargo-deny (`cargo install cargo-deny --locked`), write deny.toml, run `cargo deny check licenses` by hand and widen the `[licenses] allow` list to whatever permissive licenses the real graph actually has - stop and raise on the task rather than widen it if anything GPL/AGPL turns up.
4. Factor `tools/src/steg_tools/workspace.py` out of affected.py's existing `main()`, add the `dependencies` field to `Crate`/`crates_from_metadata`, update `test_affected.py`'s fixtures accordingly. Run `just test-tools` to confirm the existing unit suite still passes with the new field threaded through.
5. Write boundaries.toml (steg-kernel's empty allow list), `tools/src/steg_tools/boundaries.py`, and `tools/tests/unit/test_boundaries.py`. Register `steg-boundaries` in tools/pyproject.toml. Add `"boundaries.toml"` to affected.py's `GLOBAL_FILES` now that the file exists. Run `just test-tools` again.
6. Write the justfile's `boundaries` and `setup` recipes and the `check` edit described above.
7. `cargo fmt --all` once over the new files, then iterate `just check` until `fmt-check`, `lint`, `boundaries`, `deny`, `test`, `test-tools` all pass for real.
8. Prove AC#5 and AC#6 are real refusals, not configuration nobody exercises:
   - AC#5: temporarily remove "MIT" from deny.toml's `[licenses] allow`, run `cargo deny check licenses`, confirm it fails naming the MIT-licensed crates it now refuses, then restore it.
   - AC#6: temporarily add `tokio = "1"` as a direct dependency of crates/kernel/Cargo.toml (nothing in boundaries.toml's steg-kernel allow list permits it), run `cargo fetch` so the new dependency is actually resolvable before `cargo metadata` runs, then run `uv run --project tools steg-boundaries` (or `just boundaries`). Confirm the failure is the violation message naming steg-kernel, tokio and the kernel's reason text - not a `cargo metadata` error from an unresolved dependency, which would fail for the wrong reason and prove nothing. Then revert the Cargo.toml edit and re-run `cargo fetch`/`cargo metadata` once to leave the lockfile clean.
   Both probes are recorded as evidence on the task; neither survives into the committed diff.
9. Write `tools/tests/adapter/test_affected_real_workspace.py` (AC#8) against the now-real `cargo metadata` output, fixing `affected.py` wherever it disagrees. Run `just test-tools`.
10. Commit everything so far. Then prove `just test`'s selection against the real workspace with a genuinely isolated diff: this branch's own diff against `origin/main` touches `Cargo.toml`/`Cargo.lock`/`justfile`/`tools/`/`deny.toml`/`boundaries.toml`, all of which `steg-affected` already treats as global, so it cannot show per-crate selection from the branch diff itself. Instead run `uv run --quiet --project tools steg-affected --base HEAD` (empty `since`, so only an uncommitted touch counts) after touching crates/kernel/src/lib.rs alone, and again after touching crates/app/src/main.rs alone, and confirm the two cases select different, correct `-p` sets; then discard those probe touches.
11. Run `just test-all`, `just fmt`, `just fmt-check`, `just lint`, `just boundaries`, `just deny`, `just build`, `just setup` (twice, to show it is safe to run again) individually (AC#7, AC#10). Attempt `just run` under a timeout, reporting plainly whether a window was actually observed to open in this environment or only that the process started without crashing.
12. Final `just check`, review the full diff, commit.

Risks

- GPUI is pre-1.0 and changed how it's published since anything remembered about it: it is now a standalone crate on crates.io (0.2.2, Apache-2.0, edition 2024), not a git dependency into the zed monorepo as older memory would assume - confirmed directly against the crates.io API and its real dependency list before writing this plan, rather than assumed. Its wayland/x11 default features are gated to `cfg(target_os = "linux"/"freebsd")` in its own manifest, so they should not build on macOS, but that is confirmed for real only once `cargo build --workspace` runs.
- `steg-affected`'s own global-file list (Cargo.toml, Cargo.lock, justfile, tools/, deny.toml, now boundaries.toml) means this task's branch diff will always select "all." The real per-crate proof for AC#8 has to use `--base HEAD` against an isolated, later touch (step 10), not the branch's own history.
- The AC#6 probe (step 8) can fail for the wrong reason: `cargo metadata` needs the probe dependency actually resolvable, or the shell returns a metadata error rather than a boundary violation, proving nothing. The plan calls `cargo fetch` before the check for exactly this reason.
- cargo-deny is not installed on this machine; `just setup` builds it from source via `cargo install --locked`, which takes a few minutes the first time. Homebrew has a bottled formula that would be faster, but assumes Homebrew is present on whatever machine runs `just setup`; `cargo install` needs nothing beyond the Rust toolchain this task already requires, so it is the safer default.
- boundaries.toml only restricts crates it names - today, only steg-kernel. That is deliberately the whole of AC#6 ("starting with the kernel"); nothing stops a later task from giving steg-server, steg-app or steg-cli an undesirable dependency unless someone adds them to boundaries.toml too. `unknown_rules()` guards against a typo disabling the check silently, but not against a crate that was simply never added.
- `allow-unwrap-in-tests` is a `clippy.toml` configuration key, not a `[workspace.lints]` entry; it is easy to put it in the wrong file (Cargo.toml) and have it silently do nothing. Confirmed the key's name and file against the installed clippy's own configuration source (`clippy_config::conf`) before writing this plan, rather than from memory.
- The seven named clippy lints in `[workspace.lints.clippy]` are inert to `cargo build`/`cargo test` - rustc does not run clippy lints. They only fire under `cargo clippy`. The proof for AC#11 below is written against `cargo clippy`, not `cargo build`, so this plan does not claim an enforcement the table cannot give.
- `just run` launches a real GPUI window. This worker's environment may have no interactive window server; the proof for that one recipe may be limited to "the binary builds and starts without an immediate crash," reported honestly rather than claimed as an observed window if one can't actually be seen.
- `steg_tools.affected.crates_from_metadata` has never run against real `cargo metadata` output before this task; expect to fix small mismatches once tried for real - that is the reason AC#8 exists.

Decisions standing as planned (not reopened by this revision; Bruk confirmed both)

- `run` stays outside `{{quiet}}` and is treated as exempt from the AC#9 "one line" contract: wrapping a long-running interactive GPUI window in `{{quiet}}` would buffer its own stdout/stderr until the process exits, which is the wrong shape for a foreground app someone is looking at.
- The fifth `steg` bin crate carries the single binary (R61/AC#2), with steg-cli staying a plain library that a future Python client or tests can link without pulling in a `main`, rather than folding the binary into steg-cli directly.

Proof per acceptance criterion

#1 `just check` passes end to end after step 12.
#2 `cargo build --workspace` produces one `steg` binary that is `steg-server` plus `steg-cli` linked together; `ls target/debug/steg` after the build.
#3 root LICENSE file, MIT text; `license = "MIT"` on `[workspace.package]`, `license.workspace = true` on every crate.
#4 `gpui = { version = "=0.2.2" }` (exact pin) in `[workspace.dependencies]`, `gpui.workspace = true` in crates/app/Cargo.toml.
#5 the MIT-removal probe in step 8: `cargo deny check licenses` fails with "MIT" removed from the allow list, and passes once restored.
#6 `tools/tests/unit/test_boundaries.py` proves `violations()`'s and `unknown_rules()`'s logic directly, and the kernel/tokio probe in step 8 proves it for real: `steg-boundaries` (via `just boundaries`) fails naming steg-kernel, tokio and the architecture.md-citing reason with the temporary dependency present and `cargo fetch` already run, and passes once it is reverted.
#7 each of check/test/test-all/fmt/lint/boundaries/deny/build/run individually exercised in steps 7 and 11, with real output (not the justfile's current "skip ... no Rust workspace yet" path) since `Cargo.toml` now exists.
#8 the adapter test (step 9) plus the `--base HEAD` probes in step 10, run against the real workspace, not the made-up fixtures.
#9 steg-quiet already wraps every recipe but `run` (named as a standing exception above); its one-line pass/fail-plus-log-path output is visible in the output of every other step above.
#10 `just setup` run twice in a row in step 11: the first time installs cargo-deny, the second time reports it already installed and does nothing else.
#11 root Cargo.toml's `[workspace.lints.clippy]` table with the seven named lints set to "deny" directly, and `edition = "2024"`; `#![forbid(unsafe_code)]` in crates/kernel/src/lib.rs. `cargo clippy --workspace --all-targets` (no `-D warnings`) failing against a deliberately introduced violation, then passing once it is removed, is evidence the table itself enforces them, as distinct from the `lint` recipe's own `-D warnings` flag, which covers every other clippy lint the table doesn't name. The kernel's `#[cfg(test)]` unwrap test passing under `just lint` is the proof that `clippy.toml`'s `allow-unwrap-in-tests = true` is doing real work, not just declared.

Left unsettled for the engineer

- The exact smoke call in crates/app/src/main.rs depends on reading GPUI 0.2.2's real exported API at build time (step 2), not decided here.
- The final deny.toml allow-list contents are seeded, not final; they get fixed once the real lockfile's licenses are known (step 3).
- Whether `just run` can be observed to actually open a window in whatever environment builds this, or only confirmed to start the process (see Risks) - flagged rather than guessed at.
</content>
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Onboarding skipped by @bruk.o: bruk-io-steg has no repository plugin yet and no code to read layers or verify commands from. TASK-1.24 onboards it once the workspace exists. The repository's own rules are in .claude/rules/ and its commands are in the justfile.

Plan checked against doc-1 by @bruk.o: ok, with one point for phase 2. It delivers R58, R59 and R61 and each of the eleven acceptance criteria has a proof, including refusal probes for the licence and boundary checks and a real-workspace proof of the test selection. It builds only the frame and leaves the emulation library, wire protocol, API format and command names open. For phase 2: unwrap_used set to deny also fires in tests, and .claude/rules/rust.md allows unwrap there, so the workspace needs clippy's allow-unwrap-in-tests setting or the equivalent; confirm the setting's name against the installed clippy. Put to Bruk with the plan: the boundary check's mechanism (a crate-name deny list, or an allow-table of each crate's dependencies checked in tools/), just run left out of the quiet wrapper, and a fifth crate for the single binary.

Revised plan checked against doc-1 by @bruk.o: ok. The kernel boundary is now an allow list in boundaries.toml, checked by steg-boundaries in tools/ on the dependency edges cargo metadata reports, run from just check, with unit tests and a refusal probe; an unknown crate name in the table is itself a finding, so a typo cannot switch the check off. cargo deny is licences only. unwrap in tests is handled through clippy.toml. The other named decisions stand. This matches Bruk's decision of 2026-11-13.
<!-- SECTION:NOTES:END -->

## Comments

<!-- COMMENTS:BEGIN -->
created: 2026-11-14T01:54:00Z

---

Plan sent back by @bruk.o on Bruk's decision of 2026-11-13: enforce the kernel boundary with an allow list, not a deny list of crate names. Each crate's permitted dependencies are written down once, and the check fails on any dependency not in its list, so a new kernel dependency is a deliberate change and nothing has to be remembered. Build it in tools/ on the dependency edges crates_from_metadata already reads, with unit tests, and run it from just check. The kernel's list covers external crates as well as workspace ones. Keep cargo deny for licences. The other two named decisions stand as planned: just run stays outside the quiet wrapper, and the fifth steg crate carries the single binary. Also carry the earlier note: unwrap_used must not fire in tests.

---

created: 2026-11-14T01:59:00Z

---

Plan revised per @bruk.o's decision: AC#6 is now enforced by an allow list (boundaries.toml) checked by a new steg-boundaries tool in tools/ against crates_from_metadata's dependency edges, not a cargo-deny deny list. deny.toml stays licences-only. clippy.toml's allow-unwrap-in-tests = true confirmed by name against the installed clippy (0.1.98) for the unwrap_used-in-tests note. just run and the fifth steg crate stand as before.

---

author: @bruk
created: 2026-11-14T03:56:00Z

---

Plan approved by @bruk (e0acb049d09a, size-2): its worker may build it.

---
<!-- COMMENTS:END -->
