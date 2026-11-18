---
id: TASK-1.131
title: 'Server: runtime.rs split into modules by concern'
status: In Progress
assignee:
  - '@bruk.w7'
created_date: '2026-11-18T02:09:00Z'
updated_date: '2026-11-18T05:18:00Z'
labels:
  - size-3
milestone: m-3
dependencies: []
parent_task_id: TASK-1
project: bruk-io-steg
asdlc_accepted_from: DRAFT-2
asdlc_branch: task-1.131-server-runtime-rs-split-into-modules-by
asdlc_plan_approved_by: bruk
asdlc_plan_approved_hash: '6b3f731e72f1'
asdlc_plan_approved_size: size-3
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Agreed with the engineer on 2026-11-17 to let more server tasks run at once. runtime.rs was touched by 12 of the last 40 merges, the most of any file. This task only moves code. It must run while no other task is editing crates/server/src/runtime.rs, and it can run beside the split of the API description and the dispatch tests. Whether pane.rs (2,577 lines) and connection.rs (1,314) deserve the same is for the plan to say, as a follow-up and not as part of this task.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 crates/server/src/runtime.rs (4,936 lines) becomes a module with one file per concern, such as the tree and its lookups, attach and arm, input and declared size, close and force_end, start and stop; the plan names the split and why each seam is there
- [ ] #2 Runtime's public surface is unchanged, and the tests move with the code they test
- [ ] #3 No function body changes beyond imports and visibility, shown mechanically: a recorded comparison of function bodies or of moved lines before and after
- [ ] #4 The list of test names that run is identical before and after, none ignored; just check passes
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
Move-only. `git mv crates/server/src/runtime.rs crates/server/src/runtime/mod.rs` first (keeps git's rename detection), then split `mod.rs`'s own content into one file per concern under `crates/server/src/runtime/`. No behaviour, no renames, no content change anywhere outside this module beyond the `mod` declarations and `use` lines the move forces.

## Seams and layout

`runtime/mod.rs` keeps only what every seam needs and nothing seam-specific: the module doc comment, imports, `mod` declarations, `struct State`, `struct Subscriber`, `Runtime` itself, `FORCE_GRACE_DEFAULT`, `Runtime::new`, `Runtime::handle` (the dispatch entry and its generic `_` arm), `Runtime::lock`, `impl Drop for Runtime`, and `pub(crate) use` re-exports (below). Two unit tests stay here too: `a_panic_while_holding_the_lock_does_not_poison_a_later_call` and `runtime_new_does_not_panic_and_starts_empty` - both exercise construction/`lock` directly, nothing seam-specific.

Everything else moves to a sibling file, named for the task's own seam list where it gives one:

- **`runtime/login.rs`** - "login shell identity." `LoginShell`, its `Default` impl, `LoginShell::resolve_identity`, `build_login_shell`. No later task in the eight read below touches this - pure identity/environment capture, never dispatch.
- **`runtime/tree.rs`** - "the tree and its lookups." `session_owning_tab` (free fn), `Runtime::resolve_pane_target`. Read by close/moving/events/start below.
- **`runtime/start.rs`** - "starting panes." `Runtime::start_and_dispatch`, `resolve_default_cwd`, `launch_spec`, `PANE_TERM`, `new_pane_from`, `rollback_mint`, `emit_creation_events`. TASK-1.124 (fd inheritance on spawn) is the only later task that touches program-starting at all, and from what AC #1-#3 of it describe (reading portable-pty's own spawn, a descriptor-count test), the fix lands in `pty.rs`'s `RealLauncher::spawn`, not here - flagged as "probably clear," not certain, since the notes it asks for could still turn up something `launch_spec` itself has to pass through.
- **`runtime/attach.rs`** - "attach and arm." `Runtime::attach_pane`, `arm_attachment`, `detach_pane`, `forget_attachment`, `register_subscriber`, `deregister_subscriber`, `subscribe`, `attachment_count_for_test`, and `map_pane_error` (used only by `attach_pane`).
- **`runtime/input.rs`** - "input and declared size." `Runtime::send_input`, `declare_size`, `apply_size_changed` (the public, self-locking wrapper), `apply_size_changed_locked`, `reapply_every_sized_tab`.
- **`runtime/close.rs`** - "close and force_end with its numbered passes" - the task's own pairing, not mine: `Runtime::close_and_dispatch`, `ClosePre`, `capture_close_pre`, `emit_close_events`, `emit_close_cascade`, `force_end` (and its two consts, `ESCALATE_CONFIRM_TIMEOUT`/`FORCE_JOIN_TIMEOUT`, used only inside it).
- **`runtime/moving.rs`** - `Runtime::move_and_dispatch` (`pane.move`). Not named in the task's own seam list; added because it shares `close.rs`'s own shape (a cascade that can close a tab/session) but is a different dispatch method with its own event emission, and because TASK-1.111 (move a *tab* to another session) is likely to extend exactly this shape - isolating it now keeps that extension out of `close.rs`. Not `move.rs`: that word is reserved.
- **`runtime/events.rs`** - not named in the task's own list either, added because it is the one real cross-cutting concern left: `emit_pane_status_changed`, `emit_focus_event`, `apply_event_locked`, both `note_mark_applied` cfg variants, `broadcast`. TASK-1.109 (new events) and TASK-1.116 (status expiry) both live here - see Collisions.
- **`runtime/lifecycle.rs`** - "start and stop and idle," the task's own second use of "start" (Runtime's own construction/teardown, not program-starting): `Runtime::connect_client`, `disconnect_client`, `session_counter`, `set_idle_notifier`, `stop`.
- **`runtime/testing.rs`** (`#[cfg(test)]` only) - the helpers today's `mod tests` builds once and every one of its tests reuses: `login`, `test_base_env`, `test_state`, `fresh_state_with_one_pane`, `TEST_GRACE`, `TEST_TICK`, `register_test_subscriber`, `register_slow_subscriber`, `read_one_event`, `assert_nothing_arrives_within`, `EVENT_TIMEOUT`, `moment_now`, `find_pane`, `pane_status`, `pane_program_status`, `tree_next_status_deadline`, `wait_for_pane_status`, `wait_for_applied_mark_count`, the `send_input` test helper, `start_real_pane`, `login_for_marks`, `show_pane_and_tab`. Marked `pub(super)` so every seam's own `mod tests` can `use super::super::testing::*;`. Named `testing`, not `test_support`, so it is never confused with the crate-root `crates/server/src/test_support.rs` (a different, narrower helper: `run_bounded`), which this does not touch.

  `mod stop_tests` keeps its own, separately-defined `login`/`test_base_env`/`TEST_GRACE`/`TEST_TICK` inside `lifecycle.rs`, duplicated rather than folded into `testing.rs` - read both copies (lines 2264-2291 and 4662-4692 of today's file): they are identical in content, but that file's own doc comment at line 4662 says the duplication across test modules is this crate's deliberate, existing style, not an oversight. Deleting one copy to share it would be a real content change (removing a function definition), which this task's own "out of scope" line does not allow; the task asks for a move, not a cleanup.

## Why each is a real boundary - the eight later tasks, read in full

| Task | What it needs | Where it lands | Collides with |
|---|---|---|---|
| 1.88 (server show/stop, client list/show) | `stop`, `session_counter`, `connect_client`/`disconnect_client` for a server's own uptime/session/client accounting | `lifecycle.rs` | 1.116 (both touch `stop`'s own body - see below) |
| 1.90 (send input, capture, wait) | `send_input` as it stands today | `input.rs` | Possibly 1.110, see below - `send_input` itself is untouched by 1.110, but both are size/pane-dispatch adjacent |
| 1.109 (layout/session/tab/client events, `session.focus`) | every dispatch path that changes a layout, a session/tab record, or who is connected/viewing | `events.rs` primarily, plus a call site in `start.rs` (`emit_creation_events`'s own sibling), `close.rs`, `moving.rs`, and `input.rs` (a layout change from resizing) for the new `tab.layout_changed`/`session.changed` calls; `connect_client`/`disconnect_client` in `lifecycle.rs` for `client.connected`/`client.left`; `attach_pane`/`detach_pane` in `attach.rs` for `client.started_viewing`/`stopped_viewing` | 1.116 on `events.rs`; 1.88 on `lifecycle.rs` (both touch `connect_client`/`disconnect_client`); itself touches five of the nine files - the split narrows its own blast radius but does not remove it, named below |
| 1.115 (scrollback pages) | reads `PaneHandle`'s own screen/scrollback, the same way `attach_pane` already does | `attach.rs`, lightly, if anything - most of its own work is in `pane.rs` and the sdk, not here |
| 1.116 (status expiry thread) | a new thread beside the drain thread (`Runtime::new`, `mod.rs`), its own join on every path out (`stop`, `lifecycle.rs`), and reuses `emit_pane_status_changed` (`events.rs`) at its own deadline | `mod.rs` (construction, a new field) + `lifecycle.rs` (`stop`) + `events.rs` | 1.109 on `events.rs`; 1.88 on `lifecycle.rs`'s own `stop` |
| 1.110 (pane resize) | "proved through `Runtime::handle`" - its own AC names no new `Runtime` method; most plausibly reaches the kernel through the already-generic `_` arm in `mod.rs`, which already reapplies every sized tab unconditionally. Flagged, not certain: if the design settles on a dedicated reapply (the way `declare_size` has one), it touches `input.rs` instead | `mod.rs`, probably untouched beyond the existing generic arm; `input.rs` only if the design needs its own call | None confidently; `input.rs` with 1.90 only if it does land there |
| 1.111 (move a tab to another session) | the kernel operation is new (`crates/kernel`), but `tab.move --to`, "proved through `Runtime::handle`," needs the same cascade-aware event emission `move_and_dispatch` already has for `pane.move` | `moving.rs` | None of the other seven |
| 1.124 (fd inheritance on spawn) | reading `portable-pty`'s own spawn and, if needed, closing/marking descriptors before/around it | `pty.rs` (`RealLauncher::spawn`), not `runtime/` at all, on the evidence read so far | None |

**Pairs that still collide after the split**, stated plainly rather than claimed away: **1.109 × 1.116 on `events.rs`**; **1.109 × 1.88 on `lifecycle.rs`** (`connect_client`/`disconnect_client`); **1.116 × 1.88 on `lifecycle.rs`** (`stop`'s own body, from two directions - one adds a thread to join, the other rewrites what stopping means for an explicit server). 1.109 is the task the split does not fix: its own AC #1 ("every dispatch path... found by reading and listed in the plan one by one") means its plan will touch most of this module's new files for one call site each, even though each individual file's own existing code stays small. The split still helps it: each of those touches is a one- or two-line addition at an already-small, already-understood call site, not a scroll through a 4,900-line file to find it.

## Visibility: methods vs free functions, and the full pub(super) list

Inherent methods on `Runtime` resolve by type, not by path: moving `attach_pane` out of the old single file costs it nothing but the `pub(super)` bump below (if it needs one) - no caller anywhere writes a new `use` line for it, because `self.attach_pane(...)` already finds it through `Runtime` wherever `Runtime` is in scope (already true everywhere today). The only things that force a `use`-line change are **free functions** called from a file other than the one they are defined in - each such change is exactly one import line moving from `use super::foo;` to `use super::some_module::foo;` (or the reverse, while `foo` still lives in `mod.rs`), already allowed by AC #3's own wording ("no function body changed beyond imports and visibility").

Rust's own privacy rule does most of the work for free: an item with no modifier, defined in `mod.rs`, is visible to every submodule underneath it (descendants see an ancestor's private items) - so `State`'s fields, `Subscriber`, `Runtime::lock`, and `FORCE_GRACE_DEFAULT` need **no** visibility change at all, wherever they are called from, as long as they stay in `mod.rs`. The reverse direction - a submodule's own item, needed by `mod.rs` or by a sibling submodule - is the only case that needs `pub(super)` (visible from the parent down, which covers every sibling too, exactly matching the item's old, single-file-private visibility; never `pub(crate)`, which would leak it past `runtime` itself).

Full list, each a free function or method defined in one new file and called from another:

| Item | Defined in | Called from | Kind |
|---|---|---|---|
| `session_owning_tab` | `tree.rs` | `start.rs`, `close.rs`, `moving.rs`, `events.rs` | free fn - `use` line at each caller |
| `start_and_dispatch` | `start.rs` | `mod.rs` (`handle`) | method - no `use` needed |
| `close_and_dispatch` | `close.rs` | `mod.rs` (`handle`) | method |
| `move_and_dispatch` | `moving.rs` | `mod.rs` (`handle`) | method |
| `force_end` | `close.rs` | `lifecycle.rs` (`stop`) | free fn - `use` line |
| `emit_close_cascade` | `close.rs` | `events.rs` (`apply_event_locked`) | free fn - `use` line |
| `apply_size_changed_locked` | `input.rs` | `lifecycle.rs` (`disconnect_client`), `attach.rs` (`detach_pane`) | free fn - `use` line at each |
| `reapply_every_sized_tab` | `input.rs` | `mod.rs` (`handle`), `start.rs`, `close.rs`, `moving.rs` | free fn - `use` line at each |
| `broadcast` | `events.rs` | `start.rs` (`emit_creation_events`), `close.rs`, `moving.rs` | free fn - `use` line at each |
| `emit_focus_event` | `events.rs` | `mod.rs` (`handle`) | free fn - `use` line |
| `apply_event_locked` | `events.rs` | `mod.rs` (`Runtime::new`'s drain-thread closure) | free fn - `use` line |

Eleven items get `pub(super)`; nothing gets `pub` or `pub(crate)` that wasn't already. Confirmed, not assumed: of the eleven, only `Attaching` (unrelated to this table - a type, not a function, moved for its own reason below) already carried `pub(crate)` before this task; all eleven in the table above are private today, each moving to exactly `pub(super)` and no wider. `Attaching` moves to `attach.rs` (its only real user) and keeps its existing `pub(crate)` - `mod.rs` re-exports it (`pub(crate) use attach::Attaching;`) so `connection.rs`'s own `crate::runtime::Attaching` keeps resolving unchanged. `LoginShell`/`build_login_shell` move to `login.rs` and keep their existing `pub` - `mod.rs` re-exports both (`pub use login::{LoginShell, build_login_shell};`) for the same reason, feeding straight into `lib.rs`'s own `pub use runtime::{FORCE_GRACE_DEFAULT, LoginShell, Runtime, build_login_shell};`, which does not change at all. The crisp, cheap version of "the crate-external surface is unchanged": **`git diff origin/main -- crates/server/src/lib.rs` is empty**, and so is the diff on every file that names `crate::runtime::*` today (`config.rs`, `connection.rs`, `listen.rs`, `pty.rs`'s doc comment, `env.rs`'s doc comment) - verified in the proof below, not assumed. Proof (a)'s own second part (the grep-based pub-item listing, restricted to lines still saying `pub ` or `pub(crate) ` after stripping `pub(super) `) is exactly what would catch any of the eleven going wider than planned - if one of them ended up `pub(crate)` or bare `pub` instead of `pub(super)`, that line would appear in the "after" listing with no match in "before," failing (a) directly; this is also (a)'s own control already described below (widening `broadcast` to plain `pub` in a scratch edit), re-used rather than duplicated.

## Mechanical proofs, each recorded with `asdlc record-check`, each with a control

**(a) Runtime's public surface is unchanged.** Two parts, both cheap and both real:
1. `git diff origin/main -- crates/server/src/lib.rs crates/server/src/config.rs crates/server/src/connection.rs crates/server/src/listen.rs` is empty except for the merge of `origin/main` the task's own commits bring in anyway - these five are the only files outside `runtime/` naming `crate::runtime::*` today (confirmed by `git grep -n "crate::runtime\|runtime::" crates/server/src`). No import in any of them should need to change, since every path they use (`Runtime`, `LoginShell`, `build_login_shell`, `Attaching`, `FORCE_GRACE_DEFAULT`) is re-exported from `mod.rs` at its original visibility.
2. A grep-based before/after listing as a second, independent check: `grep -nE '^\s*(pub(\(\w+\))?\s+)?(fn|struct|enum|const)\s' crates/server/src/runtime.rs` (before, from `origin/main`) vs the same over every file under `crates/server/src/runtime/` (after), each line stripped of its leading visibility keyword and sorted, then diffed restricted to lines that still say `pub ` or `pub(crate) ` after stripping only `pub(super) ` - i.e. "is anything that was not already crate-visible now crate-visible, or vice versa." Expected: empty.

Control for (2): temporarily widen one `pub(super)` item (say `broadcast`) to plain `pub` in a scratch edit, rerun, confirm the script reports it; `git checkout` the file back.

**(b) The same test names run, none ignored.** `cargo test -p steg-server --lib -- --list --format=terse` is a narrower command than `just test` on purpose: `just test`'s own output is pass/fail per crate, not a structured list of test names, and a listing is exactly what this proof needs. Run once before any edit (baseline, alongside the first `record-check`) and once after the last commit. Normalise each line by stripping everything up to and including the last `runtime` segment of its module path (so `runtime::tests::foo` and `runtime::events::tests::foo` both become `foo`), since the module path prefix is expected to change and is not what this proves; sort both lists; diff. Expected: empty, and the raw (pre-strip) counts equal too, so a test silently gaining an `#[ignore]` would not hide inside the stripped comparison.

Control: add `#[ignore]` to one test in a scratch edit, confirm the raw count changes and is caught; revert.

**(c) No function body changed beyond imports and visibility.** Not a hand-rolled Rust parser - this file's own `serde_json::json!({...})` calls and string literals full of braces would make a brace-matching splitter unreliable, and debugging the prover is not this task's job. Instead: normalise both sides by stripping every `//`/`///`/`//!` comment line, every blank line, every bare `use ...;` line, every `mod ...;` line, and the leading `pub `/`pub(crate) `/`pub(super) ` token from any line that also contains `fn `/`struct `/`enum `/`impl`; concatenate the "after" set of files in `mod.rs`'s own declared order; `sort` both normalised streams; `diff`. A pure move produces two identical multisets - the "numbers" the task asks for are the line counts on each side (equal) and the diff's own line count (zero). Keep the script in the session scratchpad (`task-1131-body-diff.sh`), not committed - a one-off verification, not a repository recipe; run it through `asdlc record-check <task> --in <worktree> -- sh task-1131-body-diff.sh` (no `cd`, the script referenced by its path inside the worktree).

Control: in a scratch edit, swap one `if`/`else` branch's two bodies in, say, `force_end`'s pass 3; rerun; confirm the diff is non-empty and names lines from that function; revert.

**What the sorted-line multiset in (c) cannot catch, and what closes the gap.** Sorting throws line order away within the whole concatenated stream, not just within one function - two functions trading bodies wholesale (every line of A's body becomes B's and vice versa), or one line moved out of the function it was in and into a different function that happens to contain an identical line already, would still produce two identical multisets and a clean diff, because the proof never checks *which* function a line ended up in. Two things close this, both already planned for other reasons, not new work: (1) proof (b), the identical test-name list - a function whose body actually changed either fails a test that exercised the old behaviour or leaves an existing test applying to a now-wrong body, and either shows up as a test result changing, which (b) alone does not check (it checks names, not outcomes) but `just check`/`just test` run on every commit does, recorded the same way; (2) `git diff --color-moved=blocks --color-moved-ws=allow-indentation-change` run over the same before/after pair (not normalised, the real diff), read for its own line-level classification: every added/removed line in that diff must be coloured as a moved block, or be one of a `mod`/`use`/visibility-keyword line - recorded as two counts, "lines git classifies as moved" and "lines added or removed that are not a move," expected non-zero for the first and, beyond `mod`/`use`/visibility insertions, zero for the second. A line silently reassigned to a different function's body (same text, wrong new home) would still pass the sorted-multiset check but would very likely break `--color-moved`'s own block grouping (the line's new neighbours differ) or a test's result - this is a belt-and-suspenders pair, not a single bulletproof proof, named here rather than overclaimed.

Control for this addition: in a scratch edit, move one line verbatim from `force_end`'s pass 2 into pass 4 (identical text, wrong function) without changing either body's meaning in a way a test would catch on its own; confirm the sorted-multiset diff in (c) stays clean (showing the gap is real) while `git diff --color-moved` on the same edit shows that line's old neighbours broken out of its moved block; revert.

## Order of commits

One seam per commit, each leaving the whole crate compiling and `cargo test -p steg-server --lib -- --list` unchanged in content, chosen so a submodule never needs a `pub(super)` item that does not exist yet (anything still resident in `mod.rs` is already visible to every submodule for free, by the descendant rule above; only a *moved* item needs the bump, made in the same commit that moves it):

1. `git mv` to `runtime/mod.rs`; no other change - proves the rename alone compiles and the baseline proofs (a)/(b)/(c) are run against this starting point.
2. `login.rs` - no cross-file dependency either way.
3. `testing.rs` - the shared test helpers, extracted with no consumer yet pointed at it (dead code allowed under `#[cfg(test)]` only transiently, within this one commit - every later commit's own test-module move starts consuming it).
4. `tree.rs` - `session_owning_tab` gains `pub(super)` now, ahead of its own consumers, which is fine: nothing stops a `pub(super)` item from sitting unused by a sibling for a few commits.
5. `events.rs` - gains `pub(super)` on `broadcast`, `emit_focus_event`, `apply_event_locked`; `mod.rs`'s own `Runtime::new`/`handle` update their `use` lines to point at it. Tests move, consuming `testing.rs`.
6. `attach.rs` - `Attaching` moves here with its `pub(crate)` kept; `mod.rs` gains the re-export line. Tests move.
7. `input.rs` - `apply_size_changed_locked`/`reapply_every_sized_tab` gain `pub(super)`; every caller still in `mod.rs` or already-moved files (`attach.rs`) updates its `use` line. Tests move.
8. `lifecycle.rs` - consumes `input.rs`'s now-`pub(super)` `apply_size_changed_locked`. Tests move (`disconnect_client_reapplies_the_next_latest_declared_size`, the whole `mod stop_tests`, with its own independent helpers kept exactly as they are today).
9. `start.rs` - `start_and_dispatch` gains `pub(super)`; `mod.rs`'s `handle` updates its call. Consumes `tree.rs`'s `session_owning_tab` and `events.rs`'s `broadcast`/`reapply_every_sized_tab` via `use`. Tests move.
10. `close.rs` - `close_and_dispatch` gains `pub(super)`; `force_end`/`emit_close_cascade` gain theirs; `mod.rs`'s `handle` and `events.rs`'s `apply_event_locked` (import only) and `lifecycle.rs`'s `stop` (import only) update their `use` lines. Tests move.
11. `moving.rs` - `move_and_dispatch` gains `pub(super)`; `mod.rs`'s `handle` updates its call. No tests to move (`pane.move` is exercised only by `crates/server/tests/{dispatch,events,runtime}.rs`, confirmed by grep - none of it is inside today's unit-test modules).
12. Final tidy commit: confirm `mod.rs` now holds exactly the list given above, re-run every proof ((a), (b), (c), plus the `--color-moved` pair and the test-outcome run that close (c)'s own gap) end to end, run `just check`.
13. Stale cross-reference commit, comment-only, last: reword the eight files' own `runtime.rs`-naming comments to name their new file, as described above; `crates/server/tests/dispatch.rs` untouched. Record its own check (the non-comment-line diff is empty) separately from (a)/(b)/(c), since this commit is the one place in the task that touches content outside `runtime/` on purpose.

## If `origin/main` moves underneath this

Before starting and before every `record-check`, fetch and check `git log origin/main --oneline -- crates/server/src/runtime.rs crates/server/src/runtime/` since the branch point. Confirmed with the engineer: TASK-1.129 builds in `crates/app` only, TASK-1.119 in `crates/steg`/`crates/sdk`, TASK-1.130 in `crates/sdk/src/description/methods.rs` and `crates/server/tests/dispatch.rs` - none of the three names `crates/server/src/runtime.rs`, and the engineer is holding every other server task until this one merges, so nothing else is expected to touch it at all. If the log is non-empty anyway, stop rather than merge: a move-only refactor merging against a real logic change in the same file is exactly the conflict this task exists to prevent, and resolving it by hand would risk exactly the silent body change the proofs above are built to catch. Flag it as a plan/contract risk for the engineer instead. Otherwise, for the ordinary case (`origin/main` moved elsewhere in the crate, not in this file), rebase before continuing, since the branch has not been pushed yet at that point.

## Stale doc-comment cross-references this move creates - fixed in this task, as its own final commit

`crates/server/src/runtime.rs` is named by path, or `runtime.rs`'s own `stop_tests`/test module is named by name, in doc comments outside this file: `crates/server/src/config.rs:95`, `crates/server/src/pane.rs:2026,2105`, `crates/server/tests/attach.rs:1587,1633,1679,1686`, `crates/server/tests/events.rs:883`, `crates/server/tests/real_pty.rs:352,355`, `crates/server/tests/routing.rs:847,861,945,1136`, `crates/server/tests/runtime.rs:4,302,388,964` (this last one is the crate's own separate integration-test binary, `tests/runtime.rs` - a different file from `src/runtime.rs`/`src/runtime/mod.rs`, untouched by this move; its own doc comments about `src/runtime.rs`'s internals go stale the same way the others do). Checked: `crates/server/tests/dispatch.rs` has no reference to `runtime.rs` at all (`git grep -n "runtime\.rs" -- crates/server/tests/dispatch.rs` is empty) - nothing to hand to TASK-1.130's own notes.

Reworded in a dedicated, final, comment-only commit (step 13 below): each reference above gets its new file name in place of `runtime.rs` (for example "arm_attachment (runtime.rs)" becomes "arm_attachment (runtime/attach.rs)"; "runtime.rs's own stop_tests module" becomes "runtime/lifecycle.rs's own stop_tests module"), nothing else on the line. `crates/server/tests/dispatch.rs` is left untouched, as a sibling task's own file.

This commit sits outside proof (c)'s own claim - (c) strips comment lines before comparing, by design, specifically so a commit like this one can exist without failing it; (c)'s multiset over the *code* lines of every touched file stays identical whether this commit has landed or not. Its own, separate check: `git diff --unified=0 -- <each of the eight files above, excluding dispatch.rs>` restricted to non-comment lines is empty - i.e. every changed line in the commit's own diff starts with `//`, `///`, or `//!`, and nothing else in any of those files moves. A reviewer can see this directly in the commit's own diff (every `+`/`-` line is a comment line) without needing a separate script.

## Architecture checks: confirmed, not inferred

Read `boundaries.toml` and `.claude/rules/architecture.md`: both operate at the crate level (`[steg-server]`'s own dependency list; `just check-boundaries`/`check-architecture` check which crate depends on which, and the LikeC4 model's own components are packages, doc-16's own level 3). Nothing in either names a module inside a crate. Reorganising `crates/server/src/runtime.rs` into `crates/server/src/runtime/*.rs` changes no crate dependency and adds no external dependency, so `just check-boundaries`, `just check-architecture`, and `just deny` are expected to pass unchanged and need no new entry in either file.

## pane.rs and connection.rs: recommend a follow-up for pane.rs, not connection.rs yet

`pane.rs` (about 2,600 lines) is a good candidate for the same treatment now, on two independent signals read above: TASK-1.90's own capture/wait and TASK-1.115's own scrollback paging both read `PaneHandle`'s own screen/terminal state the same way `attach_pane` already does, and are likely to land inside it, in roughly the same area, at close to the same time. `connection.rs` (about 1,300 lines) is smaller and its own merge-churn was not read here (no per-file touch count was pulled the way runtime.rs's "12 of the last 40 merges" was given in this task's own description) - recommend checking that count before committing to a split, rather than assuming one is warranted from size alone; TASK-1.109's own `client.connected`/`client.left`/`client.started_viewing`/`client.stopped_viewing` events are the one concrete signal that touches it, and that alone may not be enough to warrant a split.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Plan checked against the task's four criteria by @bruk.o: ok. Move only: runtime.rs becomes runtime/mod.rs plus login, tree, start, attach, input, close, moving, events and lifecycle, with test helpers in a test-only module; eleven private items widen to pub(super) and no further. Proofs with controls: public surface unchanged; test list identical apart from module prefixes; a sorted normalised-line comparison of bodies, with moved-line counts recorded to cover what that comparison cannot see. Stale comments naming runtime.rs elsewhere are fixed in a final comments-only commit. Collisions that remain after the split are stated: TASK-1.109 touches most of the new files and pairs with 1.116 and 1.88.

Check passed: `just check` exited 0 at 2026-11-18T02:42:33+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.131 on task-1.131-server-runtime-rs-split-into-modules-by at c7330d672fc4:

```text
ok      format check (0.3s)
ok      lint (90.8s)
ok      boundaries (0.2s)
ok      foundations (0.2s)
ok      foundations public surface (0.1s)
ok      design pages (0.1s)
ok      licences (1.2s)
skip    tests: this change touches no crate
ok      build (steg-sdk) (3.4s)
ok      test (steg-sdk) (2.6s)
ok      test (steg-sdk, local-socket) (2.9s)
ok      build (steg-kernel) (45.1s)
ok      test (steg-kernel) (2.3s)
ok      build (steg-server) (47.0s)
ok      test (steg-server) (54.5s)
ok      test (steg-server, race-fixture) (137.4s)
ok      test (steg-server, pty-race-fixture) (143.6s)
ok      test (steg-server, accept-backoff-fixture) (116.8s)
ok      build (steg) (2.5s)
ok      test (steg) (2.0s)
ok      test (steg, fixtures) (6.5s)
ok      build (steg-ui) (20.4s)
ok      test (steg-ui) (1.6s)
ok      build (steg-app) (2.6s)
ok      test (steg-app) (2.2s)
ok      tool tests (14.6s)
ok      architecture (0.8s)
ok      recipe names (0.1s)
ok      ui levels (0.1s)
ok      ui colours (0.1s)
```

Check passed: `sh -c 'cargo build -p steg-server && cargo test -p steg-server --lib -- --list --format=terse'` exited 0 at 2026-11-18T04:28:23+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.131 on task-1.131-server-runtime-rs-split-into-modules-by at 3d143d5e1761 with uncommitted changes, last 40 of 212 lines:

```text
runtime::stop_tests::stop_ends_a_silent_program: test
runtime::stop_tests::stop_is_a_no_op_for_a_pane_whose_program_has_already_ended: test
runtime::stop_tests::stop_with_no_panes_at_all_returns_at_once: test
runtime::tests::a_command_panes_exit_emits_pane_exited: test
runtime::tests::a_command_panes_exit_reaches_pane_show_through_runtime_handle: test
runtime::tests::a_command_panes_own_program_printing_marks_does_not_change_its_status: test
runtime::tests::a_mark_for_a_pane_the_tree_no_longer_holds_does_nothing_and_does_not_panic: test
runtime::tests::a_mark_then_the_shells_own_real_exit_cascades_with_no_panic: test
runtime::tests::a_panic_while_holding_the_lock_does_not_poison_a_later_call: test
runtime::tests::a_plain_close_of_an_already_ended_pane_removes_its_idle_handle: test
runtime::tests::a_real_exit_event_drives_the_same_cascade_apply_event_runs: test
runtime::tests::a_shell_pane_not_only_a_command_pane_gets_the_same_composed_environment: test
runtime::tests::a_shell_panes_exit_emits_the_whole_cascade_in_order: test
runtime::tests::a_shell_with_no_marks_leaves_every_status_field_unchanged_until_one_arrives: test
runtime::tests::a_slow_subscribers_connection_is_shut_down_once_its_queue_is_full: test
runtime::tests::a_typed_command_still_running_stays_running_until_told_otherwise: test
runtime::tests::a_typed_command_that_fails_is_failed_with_its_exit_code: test
runtime::tests::a_typed_command_that_succeeds_is_running_then_finished_and_the_tab_shows_it: test
runtime::tests::a_typed_commands_start_and_end_emit_pane_status_changed: test
runtime::tests::a_typed_commands_status_reaches_pane_show_and_tab_show_through_runtime_handle: test
runtime::tests::an_unfiltered_subscriber_receives_nothing: test
runtime::tests::apply_event_closes_a_shell_pane_through_the_same_cascade_as_a_manual_close: test
runtime::tests::apply_event_on_a_pane_the_tree_no_longer_holds_removes_the_stale_handle_without_panicking: test
runtime::tests::apply_event_records_an_exit_and_leaves_a_command_pane_in_place: test
runtime::tests::apply_size_changed_lets_a_pane_go_under_the_floor_when_the_kernel_says_so: test
runtime::tests::apply_size_changed_resizes_only_the_panes_the_change_names: test
runtime::tests::arm_attachment_answers_false_once_the_pane_has_already_closed: test
runtime::tests::attach_pane_on_a_busy_panes_own_worker_does_not_stall_an_unrelated_requests_lock: test
runtime::tests::disconnect_client_reapplies_the_next_latest_declared_size: test
runtime::tests::login_shell_default_is_a_fixed_zero_io_value: test
runtime::tests::login_shell_resolve_identity_never_panics_and_is_never_empty: test
runtime::tests::pane_split_falls_back_to_home_when_the_inherited_live_cwd_no_longer_exists: test
runtime::tests::pane_split_inherits_the_live_cwd_of_the_pane_it_split_from_not_its_started_in_cwd: test
runtime::tests::runtime_new_does_not_panic_and_starts_empty: test
runtime::tests::steg_vars_and_term_always_win_over_a_conflicting_value_from_the_login_shells_own_base_env: test
runtime::tests::the_general_sizing_step_reaches_every_pane_through_split_close_and_swap: test
signal::tests::send_reaches_a_real_process_with_the_named_signal: test
signal::tests::send_reaches_the_whole_process_group_not_only_the_leader: test
test_support::tests::a_body_that_never_returns_fails_fast_naming_what_it_waited_for: test
test_support::tests::a_body_that_returns_promptly_is_not_delayed: test
```

Check passed: `cargo test -p steg-server --lib` exited 0 at 2026-11-18T04:43:31+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.131 on task-1.131-server-runtime-rs-split-into-modules-by at 492967d4e605 with uncommitted changes, last 40 of 216 lines:

```text
kill: 58341: No such process
test runtime::stop_tests::stop_ends_a_program_producing_output ... ok
test runtime::tests::disconnect_client_reapplies_the_next_latest_declared_size ... ok
test signal::tests::send_reaches_a_real_process_with_the_named_signal ... ok
test runtime::tests::steg_vars_and_term_always_win_over_a_conflicting_value_from_the_login_shells_own_base_env ... ok
test runtime::tests::pane_split_falls_back_to_home_when_the_inherited_live_cwd_no_longer_exists ... ok
test test_support::tests::a_body_that_returns_promptly_is_not_delayed ... ok
kill: 58524: No such process
test signal::tests::send_reaches_the_whole_process_group_not_only_the_leader ... ok
test pane::attach_tests::an_attachment_gets_nothing_before_it_is_armed_even_if_output_happens_first ... ok
test runtime::tests::pane_split_inherits_the_live_cwd_of_the_pane_it_split_from_not_its_started_in_cwd ... ok
test pane::attach_tests::attaching_to_a_pane_that_already_ended_gets_a_final_full_then_ended ... ok
test runtime::tests::attach_pane_on_a_busy_panes_own_worker_does_not_stall_an_unrelated_requests_lock ... ok
test login_env::tests::capture_login_environment_falls_back_on_a_shell_that_never_finishes_in_time ... ok
test pane::attach_tests::a_program_that_exits_on_its_own_sends_ended_with_its_real_exit_code_after_its_last_content ... ok
test pane::attach_tests::attaching_returns_the_panes_current_screen_and_updates_follow_it ... ok
test runtime::events::tests::an_unfiltered_subscriber_receives_nothing ... ok
test pane::attach_tests::mark_closing_sent_before_a_programs_own_exit_is_the_reason_ended_reports ... ok
test pane::ending_tests::await_exit_is_not_delayed_by_a_backlog_on_the_workers_own_queue ... ok
kill: 58343: No such process
test runtime::stop_tests::stop_ends_a_program_that_traps_the_polite_signal ... ok
test test_support::tests::a_body_that_never_returns_fails_fast_naming_what_it_waited_for - should panic ... ok
test runtime::tests::the_general_sizing_step_reaches_every_pane_through_split_close_and_swap ... ok
test pane::ending_tests::signal_hangup_reaches_a_backgrounded_grandchild_too ... ok
test idle::tests::the_checker_blocks_with_no_timeout_while_ineligible_and_waits_exactly_once_while_eligible ... ok
test runtime::events::tests::a_mark_then_the_shells_own_real_exit_cascades_with_no_panic ... ok
test runtime::events::tests::a_shell_with_no_marks_leaves_every_status_field_unchanged_until_one_arrives ... ok
test login_env::tests::capture_login_environment_falls_back_on_a_shell_that_exits_non_zero ... ok
test pane::ending_tests::signal_kill_ends_a_program_that_traps_the_polite_signal ... ok
test runtime::events::tests::a_typed_commands_status_reaches_pane_show_and_tab_show_through_runtime_handle ... ok
test runtime::events::tests::a_typed_command_that_succeeds_is_running_then_finished_and_the_tab_shows_it ... ok
test runtime::events::tests::a_typed_command_that_fails_is_failed_with_its_exit_code ... ok
test runtime::events::tests::a_command_panes_own_program_printing_marks_does_not_change_its_status ... ok
test login_env::tests::capture_login_environment_prints_noise_before_the_marker_without_corrupting_the_dump ... ok
kill: 58646: No such process
test login_env::tests::capture_login_environment_kills_a_backgrounded_grandchild_that_held_the_pipe_open ... ok
kill: 58322: No such process
test pane::tests::a_forward_failing_once_the_worker_is_gone_still_reaps_a_program_that_ends_on_its_own ... ok

test result: ok. 203 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.05s
```

Check passed: `cargo test -p steg-server --lib` exited 0 at 2026-11-18T04:47:41+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.131 on task-1.131-server-runtime-rs-split-into-modules-by at 991b0560a49a with uncommitted changes, last 40 of 216 lines:

```text
test runtime::tests::runtime_new_does_not_panic_and_starts_empty ... ok
test runtime::tests::a_shell_pane_not_only_a_command_pane_gets_the_same_composed_environment ... ok
test pane::attach_tests::an_attachment_gets_nothing_before_it_is_armed_even_if_output_happens_first ... ok
test signal::tests::send_reaches_a_real_process_with_the_named_signal ... ok
test pane::attach_tests::attaching_to_a_pane_that_already_ended_gets_a_final_full_then_ended ... ok
test runtime::tests::steg_vars_and_term_always_win_over_a_conflicting_value_from_the_login_shells_own_base_env ... ok
test test_support::tests::a_body_that_returns_promptly_is_not_delayed ... ok
test runtime::tests::pane_split_falls_back_to_home_when_the_inherited_live_cwd_no_longer_exists ... ok
kill: 69327: No such process
test signal::tests::send_reaches_the_whole_process_group_not_only_the_leader ... ok
test runtime::tests::pane_split_inherits_the_live_cwd_of_the_pane_it_split_from_not_its_started_in_cwd ... ok
test runtime::attach::tests::attach_pane_on_a_busy_panes_own_worker_does_not_stall_an_unrelated_requests_lock ... ok
test login_env::tests::capture_login_environment_falls_back_on_a_shell_that_never_finishes_in_time ... ok
test pane::attach_tests::a_program_that_exits_on_its_own_sends_ended_with_its_real_exit_code_after_its_last_content ... ok
test pane::attach_tests::attaching_returns_the_panes_current_screen_and_updates_follow_it ... ok
test pane::attach_tests::mark_closing_sent_before_a_programs_own_exit_is_the_reason_ended_reports ... ok
test runtime::events::tests::an_unfiltered_subscriber_receives_nothing ... ok
test pane::ending_tests::await_exit_is_not_delayed_by_a_backlog_on_the_workers_own_queue ... ok
kill: 69145: No such process
test runtime::stop_tests::stop_ends_a_program_that_traps_the_polite_signal ... ok
test runtime::tests::the_general_sizing_step_reaches_every_pane_through_split_close_and_swap ... ok
test test_support::tests::a_body_that_never_returns_fails_fast_naming_what_it_waited_for - should panic ... ok
test pane::ending_tests::signal_hangup_reaches_a_backgrounded_grandchild_too ... ok
test idle::tests::the_checker_blocks_with_no_timeout_while_ineligible_and_waits_exactly_once_while_eligible ... ok
test runtime::events::tests::a_mark_then_the_shells_own_real_exit_cascades_with_no_panic ... ok
test runtime::events::tests::a_shell_with_no_marks_leaves_every_status_field_unchanged_until_one_arrives ... ok
test login_env::tests::capture_login_environment_falls_back_on_a_shell_that_exits_non_zero ... ok
test pane::ending_tests::signal_kill_ends_a_program_that_traps_the_polite_signal ... ok
test runtime::events::tests::a_typed_command_that_succeeds_is_running_then_finished_and_the_tab_shows_it ... ok
test runtime::events::tests::a_typed_commands_status_reaches_pane_show_and_tab_show_through_runtime_handle ... ok
test runtime::events::tests::a_typed_command_that_fails_is_failed_with_its_exit_code ... ok
test login_env::tests::capture_login_environment_prints_noise_before_the_marker_without_corrupting_the_dump ... ok
test runtime::events::tests::a_command_panes_own_program_printing_marks_does_not_change_its_status ... ok
test outbound::tests::a_genuine_flood_on_a_data_stream_never_delays_a_control_reply ... ok
kill: 69411: No such process
test login_env::tests::capture_login_environment_kills_a_backgrounded_grandchild_that_held_the_pipe_open ... ok
kill: 69121: No such process
test pane::tests::a_forward_failing_once_the_worker_is_gone_still_reaps_a_program_that_ends_on_its_own ... ok

test result: ok. 203 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.05s
```

Check passed: `cargo test -p steg-server --lib` exited 0 at 2026-11-18T04:51:10+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.131 on task-1.131-server-runtime-rs-split-into-modules-by at 01c8d55e2d8e with uncommitted changes, last 40 of 216 lines:

```text
test runtime::tests::disconnect_client_reapplies_the_next_latest_declared_size ... ok
test runtime::tests::runtime_new_does_not_panic_and_starts_empty ... ok
test pane::attach_tests::an_attachment_gets_nothing_before_it_is_armed_even_if_output_happens_first ... ok
test signal::tests::send_reaches_a_real_process_with_the_named_signal ... ok
test runtime::tests::a_shell_pane_not_only_a_command_pane_gets_the_same_composed_environment ... ok
test pane::attach_tests::attaching_to_a_pane_that_already_ended_gets_a_final_full_then_ended ... ok
test test_support::tests::a_body_that_returns_promptly_is_not_delayed ... ok
kill: 73121: No such process
test signal::tests::send_reaches_the_whole_process_group_not_only_the_leader ... ok
test runtime::tests::steg_vars_and_term_always_win_over_a_conflicting_value_from_the_login_shells_own_base_env ... ok
test runtime::tests::pane_split_falls_back_to_home_when_the_inherited_live_cwd_no_longer_exists ... ok
test runtime::tests::pane_split_inherits_the_live_cwd_of_the_pane_it_split_from_not_its_started_in_cwd ... ok
test runtime::attach::tests::attach_pane_on_a_busy_panes_own_worker_does_not_stall_an_unrelated_requests_lock ... ok
test login_env::tests::capture_login_environment_falls_back_on_a_shell_that_never_finishes_in_time ... ok
test pane::attach_tests::a_program_that_exits_on_its_own_sends_ended_with_its_real_exit_code_after_its_last_content ... ok
test pane::attach_tests::attaching_returns_the_panes_current_screen_and_updates_follow_it ... ok
test runtime::events::tests::an_unfiltered_subscriber_receives_nothing ... ok
test pane::attach_tests::mark_closing_sent_before_a_programs_own_exit_is_the_reason_ended_reports ... ok
test pane::ending_tests::await_exit_is_not_delayed_by_a_backlog_on_the_workers_own_queue ... ok
test runtime::input::tests::the_general_sizing_step_reaches_every_pane_through_split_close_and_swap ... ok
kill: 72939: No such process
test runtime::stop_tests::stop_ends_a_program_that_traps_the_polite_signal ... ok
test test_support::tests::a_body_that_never_returns_fails_fast_naming_what_it_waited_for - should panic ... ok
test pane::ending_tests::signal_hangup_reaches_a_backgrounded_grandchild_too ... ok
test runtime::events::tests::a_mark_then_the_shells_own_real_exit_cascades_with_no_panic ... ok
test idle::tests::the_checker_blocks_with_no_timeout_while_ineligible_and_waits_exactly_once_while_eligible ... ok
test runtime::events::tests::a_shell_with_no_marks_leaves_every_status_field_unchanged_until_one_arrives ... ok
test pane::ending_tests::signal_kill_ends_a_program_that_traps_the_polite_signal ... ok
test login_env::tests::capture_login_environment_falls_back_on_a_shell_that_exits_non_zero ... ok
test login_env::tests::capture_login_environment_prints_noise_before_the_marker_without_corrupting_the_dump ... ok
test runtime::events::tests::a_typed_command_that_fails_is_failed_with_its_exit_code ... ok
test runtime::events::tests::a_typed_command_that_succeeds_is_running_then_finished_and_the_tab_shows_it ... ok
test runtime::events::tests::a_typed_commands_status_reaches_pane_show_and_tab_show_through_runtime_handle ... ok
test runtime::events::tests::a_command_panes_own_program_printing_marks_does_not_change_its_status ... ok
kill: 73208: No such process
test login_env::tests::capture_login_environment_kills_a_backgrounded_grandchild_that_held_the_pipe_open ... ok
kill: 72912: No such process
test pane::tests::a_forward_failing_once_the_worker_is_gone_still_reaps_a_program_that_ends_on_its_own ... ok

test result: ok. 203 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.05s
```

Check passed: `cargo test -p steg-server --lib` exited 0 at 2026-11-18T04:55:54+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.131 on task-1.131-server-runtime-rs-split-into-modules-by at 75ffe8998acd with uncommitted changes, last 40 of 216 lines:

```text
test pane::attach_tests::attaching_to_a_pane_that_already_ended_gets_a_final_full_then_ended ... ok
test runtime::tests::runtime_new_does_not_panic_and_starts_empty ... ok
test runtime::lifecycle::tests::disconnect_client_reapplies_the_next_latest_declared_size ... ok
test signal::tests::send_reaches_a_real_process_with_the_named_signal ... ok
test runtime::tests::a_plain_close_of_an_already_ended_pane_removes_its_idle_handle ... ok
kill: 84294: No such process
test signal::tests::send_reaches_the_whole_process_group_not_only_the_leader ... ok
test test_support::tests::a_body_that_returns_promptly_is_not_delayed ... ok
test runtime::tests::a_shell_pane_not_only_a_command_pane_gets_the_same_composed_environment ... ok
test runtime::tests::steg_vars_and_term_always_win_over_a_conflicting_value_from_the_login_shells_own_base_env ... ok
test runtime::tests::pane_split_falls_back_to_home_when_the_inherited_live_cwd_no_longer_exists ... ok
test runtime::attach::tests::attach_pane_on_a_busy_panes_own_worker_does_not_stall_an_unrelated_requests_lock ... ok
test runtime::tests::pane_split_inherits_the_live_cwd_of_the_pane_it_split_from_not_its_started_in_cwd ... ok
test login_env::tests::capture_login_environment_falls_back_on_a_shell_that_never_finishes_in_time ... ok
test pane::attach_tests::attaching_returns_the_panes_current_screen_and_updates_follow_it ... ok
test pane::attach_tests::a_program_that_exits_on_its_own_sends_ended_with_its_real_exit_code_after_its_last_content ... ok
test pane::attach_tests::mark_closing_sent_before_a_programs_own_exit_is_the_reason_ended_reports ... ok
test runtime::events::tests::an_unfiltered_subscriber_receives_nothing ... ok
test pane::ending_tests::await_exit_is_not_delayed_by_a_backlog_on_the_workers_own_queue ... ok
test runtime::input::tests::the_general_sizing_step_reaches_every_pane_through_split_close_and_swap ... ok
kill: 84119: No such process
test runtime::lifecycle::stop_tests::stop_ends_a_program_that_traps_the_polite_signal ... ok
test pane::ending_tests::signal_hangup_reaches_a_backgrounded_grandchild_too ... ok
test test_support::tests::a_body_that_never_returns_fails_fast_naming_what_it_waited_for - should panic ... ok
test idle::tests::the_checker_blocks_with_no_timeout_while_ineligible_and_waits_exactly_once_while_eligible ... ok
test runtime::events::tests::a_mark_then_the_shells_own_real_exit_cascades_with_no_panic ... ok
test runtime::events::tests::a_shell_with_no_marks_leaves_every_status_field_unchanged_until_one_arrives ... ok
test pane::ending_tests::signal_kill_ends_a_program_that_traps_the_polite_signal ... ok
test login_env::tests::capture_login_environment_falls_back_on_a_shell_that_exits_non_zero ... ok
test login_env::tests::capture_login_environment_prints_noise_before_the_marker_without_corrupting_the_dump ... ok
test runtime::events::tests::a_typed_command_that_succeeds_is_running_then_finished_and_the_tab_shows_it ... ok
test runtime::events::tests::a_typed_command_that_fails_is_failed_with_its_exit_code ... ok
test runtime::events::tests::a_typed_commands_status_reaches_pane_show_and_tab_show_through_runtime_handle ... ok
test runtime::events::tests::a_command_panes_own_program_printing_marks_does_not_change_its_status ... ok
kill: 84410: No such process
test login_env::tests::capture_login_environment_kills_a_backgrounded_grandchild_that_held_the_pipe_open ... ok
kill: 84087: No such process
test pane::tests::a_forward_failing_once_the_worker_is_gone_still_reaps_a_program_that_ends_on_its_own ... ok

test result: ok. 203 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.06s
```

Check passed: `cargo test -p steg-server --lib` exited 0 at 2026-11-18T05:01:39+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.131 on task-1.131-server-runtime-rs-split-into-modules-by at f4678565daba with uncommitted changes, last 40 of 216 lines:

```text
test pane::attach_tests::attaching_to_a_pane_that_already_ended_gets_a_final_full_then_ended ... ok
test runtime::tests::a_panic_while_holding_the_lock_does_not_poison_a_later_call ... ok
test runtime::tests::runtime_new_does_not_panic_and_starts_empty ... ok
test signal::tests::send_reaches_a_real_process_with_the_named_signal ... ok
test runtime::start::tests::a_shell_pane_not_only_a_command_pane_gets_the_same_composed_environment ... ok
kill: 3191: No such process
test signal::tests::send_reaches_the_whole_process_group_not_only_the_leader ... ok
test test_support::tests::a_body_that_returns_promptly_is_not_delayed ... ok
test runtime::start::tests::steg_vars_and_term_always_win_over_a_conflicting_value_from_the_login_shells_own_base_env ... ok
test runtime::tests::a_plain_close_of_an_already_ended_pane_removes_its_idle_handle ... ok
test runtime::start::tests::pane_split_falls_back_to_home_when_the_inherited_live_cwd_no_longer_exists ... ok
test runtime::attach::tests::attach_pane_on_a_busy_panes_own_worker_does_not_stall_an_unrelated_requests_lock ... ok
test runtime::start::tests::pane_split_inherits_the_live_cwd_of_the_pane_it_split_from_not_its_started_in_cwd ... ok
test login_env::tests::capture_login_environment_falls_back_on_a_shell_that_never_finishes_in_time ... ok
test pane::attach_tests::attaching_returns_the_panes_current_screen_and_updates_follow_it ... ok
test pane::attach_tests::a_program_that_exits_on_its_own_sends_ended_with_its_real_exit_code_after_its_last_content ... ok
test pane::attach_tests::mark_closing_sent_before_a_programs_own_exit_is_the_reason_ended_reports ... ok
test runtime::events::tests::an_unfiltered_subscriber_receives_nothing ... ok
test pane::ending_tests::await_exit_is_not_delayed_by_a_backlog_on_the_workers_own_queue ... ok
test runtime::input::tests::the_general_sizing_step_reaches_every_pane_through_split_close_and_swap ... ok
kill: 3014: No such process
test runtime::lifecycle::stop_tests::stop_ends_a_program_that_traps_the_polite_signal ... ok
test pane::ending_tests::signal_hangup_reaches_a_backgrounded_grandchild_too ... ok
test test_support::tests::a_body_that_never_returns_fails_fast_naming_what_it_waited_for - should panic ... ok
test idle::tests::the_checker_blocks_with_no_timeout_while_ineligible_and_waits_exactly_once_while_eligible ... ok
test runtime::events::tests::a_mark_then_the_shells_own_real_exit_cascades_with_no_panic ... ok
test runtime::events::tests::a_shell_with_no_marks_leaves_every_status_field_unchanged_until_one_arrives ... ok
test pane::ending_tests::signal_kill_ends_a_program_that_traps_the_polite_signal ... ok
test login_env::tests::capture_login_environment_falls_back_on_a_shell_that_exits_non_zero ... ok
test login_env::tests::capture_login_environment_prints_noise_before_the_marker_without_corrupting_the_dump ... ok
test runtime::events::tests::a_typed_command_that_succeeds_is_running_then_finished_and_the_tab_shows_it ... ok
test runtime::events::tests::a_typed_command_that_fails_is_failed_with_its_exit_code ... ok
test runtime::events::tests::a_typed_commands_status_reaches_pane_show_and_tab_show_through_runtime_handle ... ok
test runtime::events::tests::a_command_panes_own_program_printing_marks_does_not_change_its_status ... ok
kill: 3281: No such process
test login_env::tests::capture_login_environment_kills_a_backgrounded_grandchild_that_held_the_pipe_open ... ok
kill: 2981: No such process
test pane::tests::a_forward_failing_once_the_worker_is_gone_still_reaps_a_program_that_ends_on_its_own ... ok

test result: ok. 203 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.06s
```

Check passed: `cargo test -p steg-server --lib` exited 0 at 2026-11-18T05:06:15+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.131 on task-1.131-server-runtime-rs-split-into-modules-by at c8ed7dbad242 with uncommitted changes, last 40 of 216 lines:

```text
test pane::attach_tests::an_attachment_gets_nothing_before_it_is_armed_even_if_output_happens_first ... ok
test runtime::tests::a_panic_while_holding_the_lock_does_not_poison_a_later_call ... ok
test runtime::tests::runtime_new_does_not_panic_and_starts_empty ... ok
test runtime::start::tests::a_shell_pane_not_only_a_command_pane_gets_the_same_composed_environment ... ok
test signal::tests::send_reaches_a_real_process_with_the_named_signal ... ok
test pane::attach_tests::attaching_to_a_pane_that_already_ended_gets_a_final_full_then_ended ... ok
test test_support::tests::a_body_that_returns_promptly_is_not_delayed ... ok
test runtime::start::tests::pane_split_falls_back_to_home_when_the_inherited_live_cwd_no_longer_exists ... ok
test runtime::start::tests::steg_vars_and_term_always_win_over_a_conflicting_value_from_the_login_shells_own_base_env ... ok
kill: 12921: No such process
test signal::tests::send_reaches_the_whole_process_group_not_only_the_leader ... ok
test runtime::start::tests::pane_split_inherits_the_live_cwd_of_the_pane_it_split_from_not_its_started_in_cwd ... ok
test runtime::attach::tests::attach_pane_on_a_busy_panes_own_worker_does_not_stall_an_unrelated_requests_lock ... ok
test login_env::tests::capture_login_environment_falls_back_on_a_shell_that_never_finishes_in_time ... ok
test pane::attach_tests::attaching_returns_the_panes_current_screen_and_updates_follow_it ... ok
test pane::attach_tests::a_program_that_exits_on_its_own_sends_ended_with_its_real_exit_code_after_its_last_content ... ok
test pane::attach_tests::mark_closing_sent_before_a_programs_own_exit_is_the_reason_ended_reports ... ok
test pane::ending_tests::await_exit_is_not_delayed_by_a_backlog_on_the_workers_own_queue ... ok
test runtime::events::tests::an_unfiltered_subscriber_receives_nothing ... ok
test runtime::input::tests::the_general_sizing_step_reaches_every_pane_through_split_close_and_swap ... ok
kill: 12744: No such process
test runtime::lifecycle::stop_tests::stop_ends_a_program_that_traps_the_polite_signal ... ok
test test_support::tests::a_body_that_never_returns_fails_fast_naming_what_it_waited_for - should panic ... ok
test pane::ending_tests::signal_hangup_reaches_a_backgrounded_grandchild_too ... ok
test runtime::events::tests::a_mark_then_the_shells_own_real_exit_cascades_with_no_panic ... ok
test idle::tests::the_checker_blocks_with_no_timeout_while_ineligible_and_waits_exactly_once_while_eligible ... ok
test runtime::events::tests::a_shell_with_no_marks_leaves_every_status_field_unchanged_until_one_arrives ... ok
test login_env::tests::capture_login_environment_falls_back_on_a_shell_that_exits_non_zero ... ok
test pane::ending_tests::signal_kill_ends_a_program_that_traps_the_polite_signal ... ok
test runtime::events::tests::a_typed_command_that_succeeds_is_running_then_finished_and_the_tab_shows_it ... ok
test login_env::tests::capture_login_environment_prints_noise_before_the_marker_without_corrupting_the_dump ... ok
test runtime::events::tests::a_typed_command_that_fails_is_failed_with_its_exit_code ... ok
test runtime::events::tests::a_typed_commands_status_reaches_pane_show_and_tab_show_through_runtime_handle ... ok
test runtime::events::tests::a_command_panes_own_program_printing_marks_does_not_change_its_status ... ok
kill: 13003: No such process
test login_env::tests::capture_login_environment_kills_a_backgrounded_grandchild_that_held_the_pipe_open ... ok
kill: 12709: No such process
test pane::tests::a_forward_failing_once_the_worker_is_gone_still_reaps_a_program_that_ends_on_its_own ... ok

test result: ok. 203 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.05s
```

Check passed: `cargo test -p steg-server --lib` exited 0 at 2026-11-18T05:08:03+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.131 on task-1.131-server-runtime-rs-split-into-modules-by at c16123d9994b with uncommitted changes, last 40 of 216 lines:

```text
test runtime::start::tests::steg_vars_and_term_always_win_over_a_conflicting_value_from_the_login_shells_own_base_env ... ok
test runtime::tests::a_panic_while_holding_the_lock_does_not_poison_a_later_call ... ok
test runtime::tests::runtime_new_does_not_panic_and_starts_empty ... ok
test pane::attach_tests::an_attachment_gets_nothing_before_it_is_armed_even_if_output_happens_first ... ok
test signal::tests::send_reaches_a_real_process_with_the_named_signal ... ok
test runtime::start::tests::a_shell_pane_not_only_a_command_pane_gets_the_same_composed_environment ... ok
test test_support::tests::a_body_that_returns_promptly_is_not_delayed ... ok
test pane::attach_tests::attaching_to_a_pane_that_already_ended_gets_a_final_full_then_ended ... ok
kill: 14222: No such process
test signal::tests::send_reaches_the_whole_process_group_not_only_the_leader ... ok
test runtime::start::tests::pane_split_falls_back_to_home_when_the_inherited_live_cwd_no_longer_exists ... ok
test runtime::start::tests::pane_split_inherits_the_live_cwd_of_the_pane_it_split_from_not_its_started_in_cwd ... ok
test runtime::attach::tests::attach_pane_on_a_busy_panes_own_worker_does_not_stall_an_unrelated_requests_lock ... ok
test login_env::tests::capture_login_environment_falls_back_on_a_shell_that_never_finishes_in_time ... ok
test pane::attach_tests::attaching_returns_the_panes_current_screen_and_updates_follow_it ... ok
test pane::attach_tests::a_program_that_exits_on_its_own_sends_ended_with_its_real_exit_code_after_its_last_content ... ok
test pane::attach_tests::mark_closing_sent_before_a_programs_own_exit_is_the_reason_ended_reports ... ok
test runtime::events::tests::an_unfiltered_subscriber_receives_nothing ... ok
test pane::ending_tests::await_exit_is_not_delayed_by_a_backlog_on_the_workers_own_queue ... ok
test runtime::input::tests::the_general_sizing_step_reaches_every_pane_through_split_close_and_swap ... ok
kill: 14042: No such process
test runtime::lifecycle::stop_tests::stop_ends_a_program_that_traps_the_polite_signal ... ok
test test_support::tests::a_body_that_never_returns_fails_fast_naming_what_it_waited_for - should panic ... ok
test pane::ending_tests::signal_hangup_reaches_a_backgrounded_grandchild_too ... ok
test idle::tests::the_checker_blocks_with_no_timeout_while_ineligible_and_waits_exactly_once_while_eligible ... ok
test runtime::events::tests::a_mark_then_the_shells_own_real_exit_cascades_with_no_panic ... ok
test runtime::events::tests::a_shell_with_no_marks_leaves_every_status_field_unchanged_until_one_arrives ... ok
test pane::ending_tests::signal_kill_ends_a_program_that_traps_the_polite_signal ... ok
test login_env::tests::capture_login_environment_falls_back_on_a_shell_that_exits_non_zero ... ok
test login_env::tests::capture_login_environment_prints_noise_before_the_marker_without_corrupting_the_dump ... ok
test runtime::events::tests::a_typed_command_that_fails_is_failed_with_its_exit_code ... ok
test runtime::events::tests::a_typed_commands_status_reaches_pane_show_and_tab_show_through_runtime_handle ... ok
test runtime::events::tests::a_typed_command_that_succeeds_is_running_then_finished_and_the_tab_shows_it ... ok
test runtime::events::tests::a_command_panes_own_program_printing_marks_does_not_change_its_status ... ok
kill: 14306: No such process
test login_env::tests::capture_login_environment_kills_a_backgrounded_grandchild_that_held_the_pipe_open ... ok
kill: 14014: No such process
test pane::tests::a_forward_failing_once_the_worker_is_gone_still_reaps_a_program_that_ends_on_its_own ... ok

test result: ok. 203 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.05s
```

Check FAILED: `sh -c 'diff <(bash /private/tmp/claude-501/-Users-brukhabtu-src-github-com-bruk-io-steg/a318fbf0-a659-4ad6-9bfc-86a79ba9cc76/scratchpad/task-1131-pubsurface.sh /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.131 origin/main before) <(bash /private/tmp/claude-501/-Users-brukhabtu-src-github-com-bruk-io-steg/a318fbf0-a659-4ad6-9bfc-86a79ba9cc76/scratchpad/task-1131-pubsurface.sh /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.131 origin/main after); echo CONTROL-widened-pub-surface-must-fail'` exited 2 at 2026-11-18T05:09:34+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.131 on task-1.131-server-runtime-rs-split-into-modules-by at cdbc6bb5e16a with uncommitted changes:

```text
sh: -c: line 0: syntax error near unexpected token `('
sh: -c: line 0: `diff <(bash /private/tmp/claude-501/-Users-brukhabtu-src-github-com-bruk-io-steg/a318fbf0-a659-4ad6-9bfc-86a79ba9cc76/scratchpad/task-1131-pubsurface.sh /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.131 origin/main before) <(bash /private/tmp/claude-501/-Users-brukhabtu-src-github-com-bruk-io-steg/a318fbf0-a659-4ad6-9bfc-86a79ba9cc76/scratchpad/task-1131-pubsurface.sh /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.131 origin/main after); echo CONTROL-widened-pub-surface-must-fail'
```

Check FAILED: `bash /private/tmp/claude-501/-Users-brukhabtu-src-github-com-bruk-io-steg/a318fbf0-a659-4ad6-9bfc-86a79ba9cc76/scratchpad/task-1131-proof-a.sh /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.131 origin/main` exited 1 at 2026-11-18T05:09:52+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.131 on task-1.131-server-runtime-rs-split-into-modules-by at cdbc6bb5e16a with uncommitted changes:

```text
2a3
> pub fn broadcast(state: &State, event: &steg_sdk::TreeEvent, scope: SessionId) {
```

Check passed: `bash /private/tmp/claude-501/-Users-brukhabtu-src-github-com-bruk-io-steg/a318fbf0-a659-4ad6-9bfc-86a79ba9cc76/scratchpad/task-1131-proof-a.sh /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.131 origin/main` exited 0 at 2026-11-18T05:10:07+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.131 on task-1.131-server-runtime-rs-split-into-modules-by at cdbc6bb5e16a:

```text

```

Check passed: `git diff --exit-code origin/main -- crates/server/src/lib.rs crates/server/src/config.rs crates/server/src/connection.rs crates/server/src/listen.rs` exited 0 at 2026-11-18T05:10:15+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.131 on task-1.131-server-runtime-rs-split-into-modules-by at cdbc6bb5e16a:

```text

```

Check FAILED: `bash /private/tmp/claude-501/-Users-brukhabtu-src-github-com-bruk-io-steg/a318fbf0-a659-4ad6-9bfc-86a79ba9cc76/scratchpad/task-1131-proof-b.sh /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.131 /private/tmp/claude-501/-Users-brukhabtu-src-github-com-bruk-io-steg/a318fbf0-a659-4ad6-9bfc-86a79ba9cc76/scratchpad/task-1131-tests-before-raw.txt /private/tmp/claude-501/-Users-brukhabtu-src-github-com-bruk-io-steg/a318fbf0-a659-4ad6-9bfc-86a79ba9cc76/scratchpad/task-1131-tests-before.txt` exited 1 at 2026-11-18T05:11:46+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.131 on task-1.131-server-runtime-rs-split-into-modules-by at cdbc6bb5e16a with uncommitted changes:

```text
raw count before: 203
raw count after:  203
--- normalised name diff (empty below this line = identical) ---
--- running: ignored count must be exactly 0 ---

FAIL: expected 0 ignored tests
```

Check FAILED: `bash /private/tmp/claude-501/-Users-brukhabtu-src-github-com-bruk-io-steg/a318fbf0-a659-4ad6-9bfc-86a79ba9cc76/scratchpad/task-1131-proof-b.sh /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.131 /private/tmp/claude-501/-Users-brukhabtu-src-github-com-bruk-io-steg/a318fbf0-a659-4ad6-9bfc-86a79ba9cc76/scratchpad/task-1131-tests-before-raw.txt /private/tmp/claude-501/-Users-brukhabtu-src-github-com-bruk-io-steg/a318fbf0-a659-4ad6-9bfc-86a79ba9cc76/scratchpad/task-1131-tests-before.txt` exited 1 at 2026-11-18T05:12:05+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.131 on task-1.131-server-runtime-rs-split-into-modules-by at cdbc6bb5e16a:

```text
raw count before: 203
raw count after:  203
--- normalised name diff (empty below this line = identical) ---
--- running: ignored count must be exactly 0 ---

FAIL: expected 0 ignored tests
```

Check passed: `bash /private/tmp/claude-501/-Users-brukhabtu-src-github-com-bruk-io-steg/a318fbf0-a659-4ad6-9bfc-86a79ba9cc76/scratchpad/task-1131-proof-b.sh /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.131 /private/tmp/claude-501/-Users-brukhabtu-src-github-com-bruk-io-steg/a318fbf0-a659-4ad6-9bfc-86a79ba9cc76/scratchpad/task-1131-tests-before-raw.txt /private/tmp/claude-501/-Users-brukhabtu-src-github-com-bruk-io-steg/a318fbf0-a659-4ad6-9bfc-86a79ba9cc76/scratchpad/task-1131-tests-before.txt` exited 0 at 2026-11-18T05:13:23+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.131 on task-1.131-server-runtime-rs-split-into-modules-by at cdbc6bb5e16a:

```text
raw count before: 203
raw count after:  203
--- normalised name diff (empty below this line = identical) ---
--- running: ignored count must be exactly 0 ---
test result: ok. 203 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.05s
```

Check FAILED: `bash /private/tmp/claude-501/-Users-brukhabtu-src-github-com-bruk-io-steg/a318fbf0-a659-4ad6-9bfc-86a79ba9cc76/scratchpad/task-1131-proof-c.sh /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.131 origin/main` exited 1 at 2026-11-18T05:18:13+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.131 on task-1.131-server-runtime-rs-split-into-modules-by at cdbc6bb5e16a with uncommitted changes, last 40 of 64 lines:

```text
> fn emit_close_cascade(
1439a1441
> fn login() -> crate::runtime::LoginShell {
1441d1442
< fn login() -> super::LoginShell {
1487c1488
< fn test_state(tree: steg_kernel::Tree, panes: HashMap<steg_sdk::PaneId, PaneHandle>) -> State {
---
> fn test_state(
1514d1514
< force_end,
1555a1556
> if !stragglers.contains(pane) {
1584d1584
< if stragglers.contains(pane) {
2261a2262
> panes: HashMap<steg_sdk::PaneId, PaneHandle>,
2356d2356
< Response, SessionId, Size, validate,
2395a2396
> scope: SessionId,
2516a2518
> state: &State,
2680c2682
< super::LoginShell {
---
> super::close::emit_close_cascade(state, closed, scope);
2718a2721,2722
> TEST_GRACE, TEST_TICK, fresh_state_with_one_pane, login, test_base_env,
> TEST_GRACE, TEST_TICK, fresh_state_with_one_pane, test_base_env, test_state,
2764a2769
> tree: steg_kernel::Tree,
2795c2800,2801
< use steg_kernel::{
---
> use crate::runtime::testing::{
> use crate::runtime::testing::{
2797d2802
< use super::{
FAIL: the body-diff output no longer matches the accounted-for baseline
```
<!-- SECTION:NOTES:END -->

## Comments

<!-- COMMENTS:BEGIN -->
author: @bruk
created: 2026-11-18T02:30:00Z

---

Plan approved by @bruk (6b3f731e72f1, size-3): its worker may build it.

---
<!-- COMMENTS:END -->
