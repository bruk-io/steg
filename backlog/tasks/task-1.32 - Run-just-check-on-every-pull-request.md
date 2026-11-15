---
id: TASK-1.32
title: Run just check on every pull request
status: In Progress
assignee:
  - '@bruk.w5'
created_date: '2026-11-15T01:32:00Z'
updated_date: '2026-11-15T02:40:00Z'
labels:
  - size-2
milestone: m-0
dependencies: []
parent_task_id: TASK-1
type: task
project: bruk-io-steg
asdlc_accepted_from: DRAFT-2
asdlc_branch: task-1.32-run-just-check-on-every-pull-request
asdlc_plan_approved_by: bruk
asdlc_plan_approved_hash: '80329a1be2b3'
asdlc_plan_approved_size: size-2
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
- `.github/workflows/ci.yml` (new): the only file this task adds. No edits to `justfile`, `commands.toml`, or any other repo file.

## Order of work
1. Before writing the workflow, check what the chosen macOS runner image actually ships (GitHub's published list of software for the hosted `macos-NN` label, not memory): confirm whether `just`, `uv`, and `rustup` are preinstalled. Pick a specific pinned image label (e.g. `macos-15`), never `macos-latest` — that tag's meaning changes over time, which is exactly what AC #2 rules out.
2. Write the workflow skeleton: `on: pull_request` (branches: `[main]`) and `on: push` (branches: `[main]`); `permissions: contents: read`; a `concurrency` block (`group: ci-${{ github.workflow }}-${{ github.ref }}`, `cancel-in-progress: true`) — macOS runner minutes are billed at a premium and a cold workspace build is the dominant cost here, so a superseded run should not keep paying for one.
3. `actions/checkout@v<pinned>` with `fetch-depth: 0`. `steg-affected`'s selection diffs against `origin/main` (`merge-base`), so the checkout needs full history or that ref resolves to nothing.
4. Rust toolchain: `rust-toolchain.toml` already pins channel `1.98.1` plus `rustfmt`/`clippy`, and rustup auto-installs an override toolchain's declared components when one is missing. If step 1 confirms rustup is preinstalled, add one step that runs `rustc --version && cargo fmt --version && cargo clippy --version` right after checkout — it forces the lazy install and turns "the right toolchain is present" into a checked fact in the run log instead of an assumption. If step 1 finds rustup absent, install it with a pinned action instead (and say so in the task's notes, since that changes this plan).
5. `uv`: not expected on the image. Add `astral-sh/setup-uv@v<pinned>` with its exact version pinned (its `version:` input) and Python pinned too (confirm the current input name for pinning the interpreter it provisions, e.g. `python-version:`, against the action's own docs at build time, not from memory) — `tools/pyproject.toml` requires `>=3.15` and has no `.python-version`, so without this the interpreter uv picks is whatever it defaults to that month.
6. `just` and `cargo-deny`: neither ships on the image, and `just check`'s `deny` step needs `cargo-deny` on PATH (it does not call `just setup`). Install both as prebuilt pinned binaries in one step with `taiki-e/install-action@v<pinned>` (`tool: just@<version>,cargo-deny@<version>`) rather than `cargo install --locked` (slow, and `--locked` pins cargo-deny's *dependencies*, not its own version) and never `brew install` (floats, defeating AC #2).
7. `Swatinem/rust-cache@v<pinned>`, keyed on `Cargo.lock`, before running checks — justified by the same cold-build cost as step 2's concurrency guard: the workspace links `gpui`, and an uncached `cargo clippy --workspace --all-targets` plus `cargo test`/`test-all` rebuild it every run otherwise.
8. One step, unconditional on both triggers: `run: just check`. This alone is AC #1 for a pull request.
9. A second step, gated `if: github.event_name == 'push'`: `run: just test-all`. The engineer decided (2026-11-15) that a push to `main` must run this in addition to `just check`, because `steg-affected` always selects `test: none` on `main` (its diff is against `origin/main`, which *is* `HEAD` by the time a push-triggered run starts) — so `just check` alone would mean `cargo test` never runs on the default branch. `just test-all` runs the whole workspace's tests regardless of selection, independent of a diff, which is exactly commands.toml's own description of what it's for. This step is additional to `just check`, not a replacement for it, and does not run on `pull_request`.
10. Pin every action to an exact tag (and prefer a commit SHA where the action's own docs recommend it) current at build time — not a remembered one; a stale number here is worse than an unresolved one.

## Risks
- **The runner image may already carry `just`/`uv`/`rustup`, making an install step redundant, or may lack one of them, making this plan's step numbers wrong.** Step 1 resolves this before anything is written, against the current published image contents, not assumption.
- **`just test-all` on push to `main` is new work beyond this task's literal acceptance criteria.** AC #1 only names `just check`; the engineer explicitly asked for `test-all` in addition on push, so it's in this plan as a deliberate instruction, recorded here with who decided it and why, not as this worker's own addition.
- **The `push`-triggered job, and therefore the `test-all` step, cannot be exercised on the feature branch before merge** — the trigger is scoped to `branches: [main]`, and a task's work happens on its own branch. Proof below covers this with a temporary, reverted change to the trigger rather than skipping the check.
- **This task's own PR diff doesn't exercise `cargo test` on the `pull_request` path.** `.github/workflows/ci.yml` is not in `steg-affected`'s `GLOBAL_FILES`/`GLOBAL_FOLDERS` and owns no crate, so the PR itself selects `test: none` under `just check`. `just lint`'s `cargo clippy --workspace --all-targets` still compiles and links every crate, including the `gpui`-linked app crate, regardless of that selection — so a green PR run does prove the workspace builds on the runner — but it never executes a test binary on the `pull_request` path. The Proof section forces one real `cargo test` run on that path too, then reverts the forcing change.
- **Floating action tags.** `@v4`-style major-version tags still drift within a major version. Pin to the exact release each action recommends (tag or SHA) and record the chosen versions in the workflow itself, so "pinned" is checkable by reading the file.

## Proof
- `just check` and `just test-all` both pass locally in the worktree first, unchanged by this task (sanity that the new workflow isn't compensating for something already broken).
- Open the PR and confirm the new workflow appears as a status check on it (`pull_request` trigger, running only `just check`) and read back its log to confirm the resolved versions — macOS image, rustc, uv, just, cargo-deny — match what the workflow declares (closes AC #2).
- In a scratch commit on the same branch, deliberately break one step `just check` runs (e.g. an unformatted line, or a clippy violation) and push; confirm the run turns red and the PR shows a failing status check; then revert the scratch commit (closes AC #1's "a failing step fails the run" for the `pull_request` path).
- In a second scratch commit, force the `test` step's selection away from `none` on the `pull_request` path (e.g. touch a file in `GLOBAL_FILES`/`GLOBAL_FOLDERS`, or temporarily swap `just check`'s `test` recipe for `test-all` in that run) and push; confirm `cargo test` actually executes against the `gpui`-linked workspace on the runner and passes; then revert the scratch commit so the merged diff is only `.github/workflows/ci.yml`.
- To exercise the `push`-to-`main` path (and its `test-all` step) before merging anything to `main`, temporarily add the task's own branch to the workflow's `push.branches` list in a scratch commit, push, confirm both `just check` and `just test-all` run and pass on that event; then remove the task branch from the list again, leaving only `main`, before the final push.
- Record each local command above with `asdlc record-check TASK-1.32 --in <worktree> --as bruk.w5 -- <command>`. The GitHub-side confirmations (status check present, run log contents, forced-red run, forced-test-run, push-path exercised) are read from the Actions UI/`gh run view` and reported in the task's notes instead, since they can't run inside the worktree.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Plan checked against doc-1 section 7 by @bruk.o: ok against the acceptance criteria. One thing for the engineer to decide: just check selects tests by diff against origin/main, so on a push to main it runs no crate tests. The orchestrator suggests the push-to-main run also does just test-all.

Revised plan checked by @bruk.o: ok. It adds just test-all on push to main, as the engineer decided on 2026-11-15, and proves the push path before merge.

Check passed: `just check` exited 0 at 2026-11-15T02:40:23+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.32 on task-1.32-run-just-check-on-every-pull-request at c492de56a0b8 with uncommitted changes:

```text
ok      format check (0.1s)
ok      lint (14.0s)
ok      boundaries (0.2s)
ok      licences (0.6s)
skip    tests: this change touches no crate
ok      tool tests (0.5s)
```
<!-- SECTION:NOTES:END -->

## Comments

<!-- COMMENTS:BEGIN -->
author: @bruk
created: 2026-11-15T02:36:00Z

---

Plan approved by @bruk (80329a1be2b3, size-2): its worker may build it.

---
<!-- COMMENTS:END -->
