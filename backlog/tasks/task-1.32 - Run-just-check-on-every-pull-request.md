---
id: TASK-1.32
title: Run just check on every pull request
status: In Progress
assignee:
  - '@bruk.w5'
created_date: '2026-11-15T01:32:00Z'
updated_date: '2026-11-15T02:30:00Z'
labels:
  - size-2
milestone: m-0
dependencies: []
parent_task_id: TASK-1
type: task
project: bruk-io-steg
asdlc_accepted_from: DRAFT-2
asdlc_branch: task-1.32-run-just-check-on-every-pull-request
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The repository's one check command has to run where nobody can forget it. doc-1, section 7.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A pull request and a push to main each run `just check`, and a failing step fails the run
- [ ] #2 The toolchains are pinned in the workflow, so a run today and a run next month use the same ones
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
## Files that change
- `.github/workflows/ci.yml` (new): the only file this task adds. No edits to `justfile`, `commands.toml`, or any other repo file — `just check` already needs nothing from CI but a few tools on PATH, so the whole task is additive.

## Order of work
1. Before writing the workflow, check what the chosen macOS runner image actually ships (GitHub's published list of software for the hosted `macos-NN` label, not memory): confirm whether `just`, `uv`, and `rustup` are preinstalled. Pick a specific pinned image label (e.g. `macos-15`), never `macos-latest` — that tag's meaning changes over time, which is exactly what AC #2 rules out.
2. Write the workflow skeleton: `on: pull_request` (branches: `[main]`) and `on: push` (branches: `[main]`); `permissions: contents: read`; a `concurrency` block (`group: ci-${{ github.workflow }}-${{ github.ref }}`, `cancel-in-progress: true`) — macOS runner minutes are billed at a premium and a cold workspace build is the dominant cost here, so a superseded run should not keep paying for one.
3. `actions/checkout@v<pinned>` with `fetch-depth: 0`. `steg-affected`'s selection diffs against `origin/main` (`merge-base`), so the checkout needs full history or that ref resolves to nothing.
4. Rust toolchain: `rust-toolchain.toml` already pins channel `1.98.1` plus `rustfmt`/`clippy`, and rustup auto-installs an override toolchain's declared components when one is missing. If step 1 confirms rustup is preinstalled, add one step that runs `rustc --version && cargo fmt --version && cargo clippy --version` right after checkout — it forces the lazy install and turns "the right toolchain is present" into a checked fact in the run log instead of an assumption. If step 1 finds rustup absent, install it with a pinned action instead (and say so in the task's notes, since that changes this plan).
5. `uv`: not expected on the image. Add `astral-sh/setup-uv@v<pinned>` with its exact version pinned (its `version:` input) and Python pinned too (confirm the current input name for pinning the interpreter it provisions, e.g. `python-version:`, against the action's own docs at build time, not from memory) — `tools/pyproject.toml` requires `>=3.15` and has no `.python-version`, so without this the interpreter uv picks is whatever it defaults to that month.
6. `just` and `cargo-deny`: neither ships on the image, and `just check`'s `deny` step needs `cargo-deny` on PATH (it does not call `just setup`). Install both as prebuilt pinned binaries in one step with `taiki-e/install-action@v<pinned>` (`tool: just@<version>,cargo-deny@<version>`) rather than `cargo install --locked` (slow, and `--locked` pins cargo-deny's *dependencies*, not its own version) and never `brew install` (floats, defeating AC #2).
7. `Swatinem/rust-cache@v<pinned>`, keyed on `Cargo.lock`, before running checks — justified by the same cold-build cost as step 2's concurrency guard: the workspace links `gpui`, and an uncached `cargo clippy --workspace --all-targets` plus `cargo test` rebuild it every run otherwise.
8. One step: `run: just check`. Nothing else — this is the whole of AC #1, and running anything in addition would make CI and a worktree diverge (the thing `just check` exists to prevent, per CLAUDE.md).
9. Pin every action to an exact tag (and prefer a commit SHA where the action's own docs recommend it) current at build time — not a remembered one; a stale number here is worse than an unresolved one.

## Risks
- **The runner image may already carry `just`/`uv`/`rustup`, making an install step redundant, or may lack one of them, making this plan's step numbers wrong.** Step 1 resolves this before anything is written, against the current published image contents, not assumption.
- **`push` to `main` always selects `test: none`.** `steg-affected` diffs `origin/main...HEAD`; on a push to `main`, `HEAD` *is* `origin/main` by the time the workflow runs, so the merge-base diff is empty and `cargo test` never runs on `main` through this workflow. `commands.toml` already documents this ("on the default branch it runs no crate tests"); AC #1 asks for `just check` literally on both events, so this plan follows that wording rather than second-guessing it, but it's named here so the engineer isn't surprised that merging to `main` never exercises `cargo test`.
- **This task's own diff doesn't prove `cargo test` runs at all.** `.github/workflows/ci.yml` is not in `steg-affected`'s `GLOBAL_FILES`/`GLOBAL_FOLDERS` and owns no crate, so the real PR this task opens will itself select `test: none`. `just lint`'s `cargo clippy --workspace --all-targets` still compiles and links every crate, including the `gpui`-linked app crate, regardless of that selection — so a green first run does prove the workspace builds on the runner — but it never executes a test binary. The Proof section below forces one real `cargo test` run before merging to close that gap, then reverts the forcing change.
- **Floating action tags.** `@v4`-style major-version tags still drift within a major version. Pin to the exact release each action recommends (tag or SHA) and record the chosen versions in the workflow itself, so "pinned" is checkable by reading the file.

## Proof
- `just check` passes locally in the worktree first, unchanged by this task (sanity that the new workflow isn't compensating for something already broken).
- Open the PR and confirm the new workflow appears as a status check on it (`pull_request` trigger) and read back its log to confirm the resolved versions — macOS image, rustc, uv, just, cargo-deny — match what the workflow declares (closes AC #2).
- In a scratch commit on the same branch, deliberately break one step `just check` runs (e.g. an unformatted line, or a clippy violation) and push; confirm the run turns red and the PR shows a failing status check; then revert the scratch commit (closes AC #1's "a failing step fails the run").
- In a second scratch commit, force the `test` step's selection away from `none` (e.g. touch a file in `GLOBAL_FILES`/`GLOBAL_FOLDERS`, or temporarily swap the `test` recipe's call for `test-all`) and push; confirm `cargo test` actually executes against the `gpui`-linked workspace on the runner and passes; then revert the scratch commit so the merged diff is only `.github/workflows/ci.yml`.
- Record each of the above runs with `asdlc record-check TASK-1.32 --in <worktree> --as bruk.w5 -- <command>` where there's a local command to run (the GitHub-side confirmations — status check present, run log, forced-red, forced-test — are read from the Actions UI/`gh run view` and reported in the task's notes instead, since they can't run inside the worktree).
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Plan checked against doc-1 section 7 by @bruk.o: ok against the acceptance criteria. One thing for the engineer to decide: just check selects tests by diff against origin/main, so on a push to main it runs no crate tests. The orchestrator suggests the push-to-main run also does just test-all.
<!-- SECTION:NOTES:END -->
