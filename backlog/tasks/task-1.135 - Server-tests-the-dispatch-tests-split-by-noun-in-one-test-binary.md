---
id: TASK-1.135
title: 'Server tests: the dispatch tests split by noun, in one test binary'
status: In Progress
assignee:
  - '@bruk.w9'
created_date: '2026-11-18T02:25:00Z'
updated_date: '2026-11-18T02:26:00Z'
labels:
  - size-2
milestone: m-3
dependencies: []
parent_task_id: TASK-1
project: bruk-io-steg
asdlc_accepted_from: DRAFT-1
asdlc_branch: task-1.135-server-tests-the-dispatch-tests-split-by
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Split from TASK-1.130 on 2026-11-18, as its plan recommended (the default is to split): TASK-1.130 keeps the sdk description, this task takes the server dispatch tests. Different crates, no shared file, independent proofs, so the two can run side by side. It must run while no other task is editing crates/server/tests/dispatch.rs. Agreed with the engineer on 2026-11-17 as part of letting more server tasks run at once: 9 of the last 40 merges touched this file (2,978 lines).

The dispatch half of TASK-1.130 first plan, kept as a starting point, not as an approved plan:

### Half B: server dispatch tests (do second)
1. `git mv crates/server/tests/dispatch.rs crates/server/tests/dispatch/main.rs`. Pure move. Commit alone. Record proof (b) right after this commit too: the binary name and every bare test name must already be identical to the pre-move baseline, since nothing but the path changed.
2. Extract `dispatch/support.rs`. `main.rs` adds `mod support;` and the explicit imports each remaining test still needs.
3. Extract `dispatch/cross_cutting.rs` (least entangled with the rest; validates the `support` split works across unrelated test groups early).
4. Extract `dispatch/session.rs`.
5. Extract `dispatch/tab.rs`.
6. Extract `dispatch/pane.rs`.
7. Extract `dispatch/api.rs`.
8. Extract `dispatch/setting.rs`. `main.rs` now holds only its doc comment, the `#![expect]`, and `mod` lines.
9. Record proof (b) and (c) for this half, fix anything they catch.

## Why one binary (`dispatch/main.rs` + modules), not one binary per noun file

The task names both options. One binary wins, for reasons sharper than raw link time:

- **`crates/server/tests/support/mod.rs`'s own doc comment (lines 20-33) is the decisive evidence, already in this repo.** It records that cross-binary shared helpers forced a downgrade from `#[expect(dead_code)]` to `#[allow(dead_code, reason = "...")]`, because "an unfulfilled lint expectation has no cfg this module could gate on per binary" when not every binary that includes the shared module calls every helper in it. `dispatch.rs`'s own `#![expect(clippy::unwrap_used, ...)]` (line 11) has exactly this shape: split into N independent binaries, every one of them needs its own copy of that attribute, and the `support`-style shared-helpers file risks the same unfulfilled-expectation problem the moment one binary's subset of tests doesn't happen to call every shared helper (already true for `minimal_value`, used only by two of the eight groups). One binary with ordinary child modules keeps one `#![expect]`, with real, unconditional visibility (`pub(super)`) between modules - no lint-suppression dance to design around.
- **Test identity is close to free this way.** `git mv dispatch.rs dispatch/main.rs` keeps the compiled binary named `dispatch`. The before/after `--list` comparison (proof b) then only has to account for `module::` prefixes being added to each bare test name inside one stable binary - not binary names and hash suffixes changing on top of that.
- **Measured, not assumed, link cost.** From a warm `target/` (dependencies already built - the normal case for a `just test`/`just check` run, since most changes don't touch `steg-kernel`/`steg-sdk`), relinking the existing single `dispatch` binary after a no-op touch took 0.33s wall. The real comparison recorded as this half's own first proof step is: `cargo clean -p steg-server && time cargo test -p steg-server --no-run` on the commit right before this task's first commit, versus the same command on the commit after every `dispatch/*.rs` extraction lands - the actual delta of linking one binary against eight (support.rs itself produces no binary; `main.rs` is the one and only target) versus one. One binary gives a delta of zero by construction; this run is what proves it, rather than assuming it.
- **`just test`'s selection is unaffected either way** - confirmed from `tools/src/steg_tools/affected.py`'s own code, not measured: `owner()` maps a changed path to the crate whose folder is the longest prefix (`crates/server/` either way), and `selection()` emits `-p steg-server`; `cargo test -p steg-server` then runs every target under that crate regardless of how many `tests/*.rs` top-level files or `tests/<name>/main.rs` directories it has. Splitting a file inside `crates/server/tests/` can never change which crates `just test` selects, nor whether that selection differs between the one-binary and N-binary layouts.
- Cargo runs integration test *targets* sequentially (only the tests inside one binary run in parallel); eight small binaries of fast, in-memory, allocation-only tests is likely slower end to end than one, on top of paying eight separate process-start costs every run.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 crates/server/tests/dispatch.rs becomes dispatch/main.rs with one module per noun and one for shared helpers, still one test binary named dispatch
- [ ] #2 The list of test names that run is identical before and after apart from the module prefix, none ignored that was not ignored before, shown by a recorded comparison with a control that a renamed test breaks it
- [ ] #3 No test body changes beyond imports and visibility, shown mechanically with a control that a one-character body edit breaks the comparison
- [ ] #4 just check passes; the notes give the measured build and link time of the dispatch tests before and after, and say which file a new method's dispatch tests now go in
<!-- AC:END -->
