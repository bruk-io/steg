---
id: TASK-1.79
title: >-
  Terminal: surface a prompt-marked command's start and end, with its exit code,
  from the engine
status: In Review
assignee:
  - '@bruk.w1'
created_date: '2026-11-15T19:43:00Z'
updated_date: '2026-11-15T22:30:00Z'
labels:
  - size-5
  - review-passed
milestone: m-4
dependencies:
  - TASK-1.41
references:
  - task-1.79-status-for-a-command-typed-at-a-shell
  - https://github.com/bruk-io/steg/pull/41
parent_task_id: TASK-1
project: bruk-io-steg
asdlc_accepted_from: DRAFT-6
asdlc_branch: task-1.79-status-for-a-command-typed-at-a-shell
asdlc_plan_approved_by: bruk
asdlc_plan_approved_hash: '8ee9fd39c481'
asdlc_plan_approved_size: size-5
asdlc_review_kind: same-machine
asdlc_review_findings: 'important=0,nit=1'
asdlc_plan_matched: no
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Split from TASK-1.41 (2026-11-15). doc-11: R50, R82, R83; decision-7; doc-9 for prompt marks. TASK-1.41's worker read the exact dependency: ghostty-vt-sys 0.8 carries GHOSTTY_TERMINAL_OPT_SEMANTIC_PROMPT with has_exit_code and exit_code on the command-end step, but ghostty-vt 0.8's safe Terminal has no event for it, and its get/set are pub(crate). The server cannot supply the code either: the command is a child of the shell, not of steg. This is the status most of daily use depends on, since most commands are typed at a prompt, so it belongs in M1. Unknowns dominate the first half of this task; if the plan finds the route needs upstream work that cannot land in time, say so and propose the stopgap rather than building one quietly.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The kernel's Terminal reports, as data, when a prompt-marked command starts producing output and when it ends with its exit code (OSC 133 C and D), from real escape sequences fed to a real terminal; a sequence with no exit code, a malformed one, and marks arriving out of order are each handled and tested
- [ ] #2 The route is the one the plan weighed and chose: the safe ghostty-vt crate patched to wire the engine's semantic-prompt callback, carried in this repository so the build stays offline (R92), with the alternatives and why they fail recorded; a second parser over the pty bytes is not built
- [ ] #3 The kernel still has no unsafe code and no I/O; the patch is as small as it can be, its licence and origin are recorded where TASK-1.34 recorded the build (third_party/README.md), the build from a clean checkout with no network still works, and how to drop the patch when upstream has the change is written down
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
## Decision this plan asks the engineer to confirm at approval

**The route is (a): patch the safe `ghostty-vt` crate's `terminal.rs` to wire
`GHOSTTY_TERMINAL_OPT_SEMANTIC_PROMPT`, vendored under `third_party/` as a `path`
dependency through `[patch.crates-io]`, same crate name and version as crates.io's
0.8.0.** I built this, not just sketched it (below). The two-line alternative: a pinned
`git` dependency instead of a vendored `path` one - simpler to update when the patch is
re-derived against a new upstream version, but it is a new network source at `cargo
fetch` time (crates.io's registry already is one for every dependency; this would add
GitHub specifically). It does not reopen R92's closed surface (`tip.files.ghostty.org`
etc.) either way, since the patch never touches `ghostty-vt-sys`'s own build. I recommend
the vendored path dependency, both for consistency with how `ghostty`'s own C/Zig source
is already vendored here and because it is the form I actually built and tested below -
nothing stops the engineer from choosing `git` instead; the patch content is identical.

## What I built, not inferred

All of the following was run in this worktree and then fully reverted (`git checkout --
Cargo.toml Cargo.lock crates/kernel/src/terminal.rs`; `git status --porcelain` empty
afterward - nothing of this is committed):

1. Copied the real vendored `ghostty-vt` 0.8.0 source
   (`~/.cargo/registry/src/*/ghostty-vt-0.8.0+*`) to a scratch directory, dropped the
   registry-only files that never belong in a vendored copy (`Cargo.toml.orig`,
   `Cargo.lock`, `.cargo-ok`, `.cargo_vcs_info.json` - none of these exist in, e.g., a
   `cargo vendor` tree either), and applied the patch below to its one file,
   `src/terminal.rs`.
2. Pointed this worktree's root `Cargo.toml` at the scratch copy via
   `[patch.crates-io]` with a `path` entry, keeping the crate's own `Cargo.toml` (name
   `ghostty-vt`, version `0.8.0+ghostty.1.4.0-main.83fb956`, licence `Apache-2.0`)
   byte-for-byte unchanged from the registry's own normalized copy - confirmed by `diff`.
   **One change this experiment left out that the real build needs**: `crates/kernel/
   Cargo.toml`'s dependency line, `ghostty-vt = "0.8"`, is a caret range - if an
   unpatched `0.8.1` (or any later `0.8.x`) ever publishes, it satisfies that range too,
   and `[patch.crates-io]`'s pin to the exact `0.8.0+ghostty.1.4.0-main.83fb956` build
   would then mismatch and either stop applying or fail to resolve. The real patch
   tightens that line to `ghostty-vt = "=0.8.0+ghostty.1.4.0-main.83fb956"`, with a
   comment saying why the pin is load-bearing (the vendored patch is derived from, and
   only verified against, exactly that source) - the one-word difference between "a
   fork we carry" and "a fork that silently stops being used."
3. `just build`: the whole pipeline - `ghostty-vt-sys`'s Zig/C build against the
   vendored `third_party/ghostty` tree (untouched by this patch), through the patched
   `ghostty-vt`, through `steg-kernel` - succeeded.
4. Added a temporary test directly in `crates/kernel/src/terminal.rs` (same module as
   the private `engine` field, so it could call `term.engine.vt_write` and
   `term.engine.events()` directly - the real public surface this task adds,
   `take_command_marks`, does not exist yet) covering every case AC #1 names, each with
   real bytes:
   - `C` then `D;0` → `OutputStart`, then `CommandEnd{has_exit_code: true, exit_code: 0}`.
   - A bare `D` (no code at all) → `CommandEnd{has_exit_code: false, exit_code: 0}`
     (`exit_code`'s value is garbage/default when `has_exit_code` is `false` - never
     read without checking the flag first, which is exactly why the kernel's own type,
     below, makes it an `Option` rather than carrying both fields separately).
   - A malformed code, `D;abc` → **also** `CommandEnd{has_exit_code: false, exit_code:
     0}` - the engine's own OSC 133 parser degrades a value it can't parse the same way
     it handles no value at all. Verified, not assumed: this means the kernel needs no
     extra validation of its own for this case - trusting `has_exit_code` is already
     correct.
   - `D;5` with no `C` beforehand at all (a fresh terminal) → fires anyway,
     `CommandEnd{has_exit_code: true, exit_code: 5}`. The engine enforces no ordering
     ("Shells differ in what they report... handle each event on its own" - the sys
     crate's own doc comment on the callback type, read directly).
   - Two `C`s in a row, then `D;7` → **two** separate `OutputStart` events, not deduped,
     then `CommandEnd{has_exit_code: true, exit_code: 7}` ("a shell may start the same
     prompt more than once... treat a repeated `PROMPT_START` as harmless" - the same
     doc comment, and this test shows the rule extends to `OUTPUT_START` too). Passing
     each one through rather than deduping is this plan's choice - idempotency, if
     wanted, belongs to whatever reads `take_command_marks` next, not to this crate.
   - `D;9` split across two separate `vt_write` calls (`"\x1b]133;D;"` then `"9\x07"`) →
     still correctly reassembled into `CommandEnd{has_exit_code: true, exit_code: 9}` -
     the real engine's own OSC parser carries the partial sequence's state across the
     two calls. This is the concrete case for why route (d), a second byte-scanner over
     the raw pty stream, would have to reimplement this buffering itself rather than
     getting it free from the real parser.
5. **Negative control**: commented out the one new `install!(SEMANTIC_PROMPT, ...)`
   line (nothing else changed) and reran the same test - it failed immediately (`case 1
   (C then D;0): []` - no events at all, the first assertion's `panicked at ...
   assertion failed`). Restored the line; reran; passing again. This is the proof that
   the test is actually exercising the wiring, not something else.
6. **Clean-checkout, no-network re-proof**: `cargo clean -p ghostty-vt-sys` (removing
   595.8 MiB of build output, forcing a full rebuild) then `env
   GHOSTTY_SOURCE_URL=unreachable just build` - passing in 39.6s, exactly TASK-1.34's
   own proof shape, now re-run with the patch present. Confirms the patch (Rust-only,
   one file in the *safe* crate) does not reopen the network surface `ghostty-vt-sys`'s
   build already closed - it was never touched.
7. `just check` - format, lint, boundaries, licences, every package built and tested
   alone, the architecture model, recipe names - **all passing** with the patch in
   place (`ok` on every one of its 19 lines, re-run in full, not inferred from a
   partial run). This is the direct answer to "what do `boundaries.toml`, `deny.toml`
   and the licence check need": **nothing** - the `licences` step passed outright,
   because the crate's name, version and licence are identical to the unpatched 0.8.0,
   so `[steg-kernel]`'s existing `allow = [..., "ghostty-vt"]` in `boundaries.toml` and
   `cargo-deny`'s existing Apache-2.0 allow-list in `deny.toml` both already cover a
   path-dependency copy of the same crate without any edit to either file.

## The patch itself (one file, +49 lines, 0 deletions - measured with `diff -u` against
## the pristine registry copy)

In `install_callbacks`, one more line in the existing macro list:
```rust
install!(
    SEMANTIC_PROMPT,
    ffi::TerminalSemanticPromptFn,
    cb_semantic_prompt
);
```
A new callback, next to `cb_pwd_changed`, following that function's exact shape:
```rust
unsafe extern "C" fn cb_semantic_prompt(
    _t: ffi::Terminal,
    ud: *mut c_void,
    event: *const ffi::TerminalSemanticPrompt,
) {
    let shared = unsafe { shared_from_userdata(ud) };
    // SAFETY: libghostty guarantees a valid, non-null event for the
    // callback's duration - the same contract `cb_clipboard_write`'s
    // `write` parameter already relies on, stated on the sys crate's own
    // callback-type doc comment.
    let event = unsafe { &*event };
    let kind = match event.kind {
        ffi::SemanticPromptKind::GHOSTTY_SEMANTIC_PROMPT_OUTPUT_START => SemanticPromptKind::OutputStart,
        ffi::SemanticPromptKind::GHOSTTY_SEMANTIC_PROMPT_COMMAND_END => SemanticPromptKind::CommandEnd,
        ffi::SemanticPromptKind::GHOSTTY_SEMANTIC_PROMPT_PROMPT_START => SemanticPromptKind::PromptStart,
        ffi::SemanticPromptKind::GHOSTTY_SEMANTIC_PROMPT_INPUT_START => SemanticPromptKind::InputStart,
        _ => return, // GHOSTTY_SEMANTIC_PROMPT_INVALID or a future value.
    };
    shared.events.push(Event::SemanticPrompt(SemanticPromptEvent {
        kind,
        has_exit_code: event.has_exit_code,
        exit_code: event.exit_code,
    }));
}
```
and a new `Event::SemanticPrompt(SemanticPromptEvent)` variant plus the small
`SemanticPromptEvent`/`SemanticPromptKind` types it carries - `kind`, `has_exit_code`
(`bool`), `exit_code` (`i32`) only; `command` and `error` (the struct's other two
fields, both FFI-owned strings) are left off entirely, since nothing in this task reads
them and leaving them out means this patch never has to reason about their ownership or
lifetime at all. `kind`/`has_exit_code`/`exit_code` are plain `Copy` scalars - no
pointer, no slice, no string handling anywhere in this patch.

**Why this adds no unsafe code to *our* crates, not just "not much":**
`ghostty-vt` is a dependency, not a workspace member (`Cargo.toml`'s `[workspace]
members` list does not name it), so none of our own lints apply to it -
`undocumented_unsafe_blocks`, `crates/kernel/clippy.toml`'s I/O bans, `#![forbid(
unsafe_code)]` in `crates/kernel/src/lib.rs` - the same as today, for the unpatched
0.8.0, which already does plenty of unsafe FFI work throughout this exact file. This
patch adds four lines of `unsafe` (`shared_from_userdata(ud)` and `&*event`), both
already-established patterns in the same file (`cb_pwd_changed` for the first;
`cb_clipboard_write`'s `&*write` for the second), with a `// SAFETY:` comment on the one
dereference, matching that crate's own existing convention. The kernel's own new code
(below) is plain safe Rust - no `unsafe` anywhere, so `#![forbid(unsafe_code)]` is
unaffected, confirmed by it building at all (that attribute makes any violation a
compile error, not a lint).

## Vendoring and the licence record (AC #3)

`third_party/ghostty-vt-patched/` (new): the vendored crate source (508 KB on disk,
measured - trivial next to `third_party/ghostty/`'s 23 MB and `ghostty-vt-zig-deps/`'s
7.1 MB), plus a committed unified diff,
`third_party/ghostty-vt-patched/semantic-prompt.patch`, generated with
`diff -u <pristine 0.8.0>/src/terminal.rs third_party/ghostty-vt-patched/src/terminal.rs`
against the exact registry copy named below - this is the "mechanical way to check it's
identical but for the patch" asked for: anyone can re-fetch `ghostty-vt =
"=0.8.0+ghostty.1.4.0-main.83fb956"` into a scratch directory with `cargo vendor` or by
reading it straight out of `~/.cargo/registry/src/*/`, diff it against the committed
tree, and confirm the only difference is exactly that one patch file's content applied
to exactly that one file. I'll write this as a documented command in
`third_party/README.md`, in the same section as the record below, rather than a script
that itself needs maintaining - `diff -rq` between the two trees, ignoring the
registry's own build-metadata files, is the whole check, and `diff -rq` already proved
it during this plan's own verification (one file differs: `src/terminal.rs`; nothing
else).

`third_party/README.md` gains a new section, in the same place TASK-1.34's own record
lives, stating: the upstream crate's name, version and licence (unchanged: `ghostty-vt`,
`0.8.0+ghostty.1.4.0-main.83fb956`, Apache-2.0, carrying the same NOTICE obligation
decision-3 section 6 already names); that the patch is `semantic-prompt.patch`, applied
to `src/terminal.rs` only, and what it wires (one callback, `GHOSTTY_TERMINAL_OPT_
SEMANTIC_PROMPT`); the exact `diff -rq` command above to re-verify; and the drop
procedure below.

**Why 0.8.0, not the newer 0.9.0 (checked, not assumed):** I downloaded and extracted
`ghostty-vt` 0.9.0 (`+ghostty-vt.1.0.1`, released 2026-11-14, from crates.io - needs a
`User-Agent` header on the request or it 403s, no credentials involved) and its matching
`ghostty-vt-sys`. `GHOSTTY_TERMINAL_OPT_SEMANTIC_PROMPT` is still unwired there too -
same gap, same patch would be needed. But 0.9.0 also switches `ghostty-vt-sys`'s whole
vendoring model, from "download Ghostty's own C/Zig source and build the engine with
Zig" (what TASK-1.34 built and `third_party/` already holds) to "link a prebuilt
`libghostty-vt` release artifact, falling back to a source build of a different,
narrower release tarball." Adopting 0.9.0 would mean redoing TASK-1.34's whole
offline-build apparatus for no benefit to this task - the gap this patch closes exists
identically in both versions. Staying on 0.8.0 is a deliberate scope decision; a later
upgrade is its own, unrelated task.

## The kernel side (AC #1, point 4)

`crates/kernel/src/terminal.rs` gains one new, small, kernel-owned type - never the
engine's own `ghostty_vt::terminal::SemanticPromptEvent`/`SemanticPromptKind` crossing
out of this module, so nothing downstream has to depend on `ghostty-vt`'s own shapes to
read this (the same reasoning `steg_sdk::Status` already follows for not being
`ghostty_vt`'s types, one layer further in):

```rust
/// What a shell reported about a prompt-marked command, from OSC 133 (doc-9
/// R14; TASK-1.79). Only `C` (output started) and `D` (command ended) -
/// this task's own scope; `A`/`B` (prompt/input start) are read by
/// `command_mark_from` and dropped, since nothing yet needs them. Adding a
/// variant for them later is additive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandMark {
    /// OSC 133 `C`: the command started producing output.
    OutputStarted,
    /// OSC 133 `D`: the command ended. `None` when the shell reported no
    /// exit code (a bare `D`) or one the engine's own OSC parser could not
    /// read (checked by test: both report the same way) - never guessed.
    Ended { exit_code: Option<i32> },
}
```

`Terminal` gains one new private field, `command_marks: Vec<CommandMark>`, and
`drain_events` gains one more arm:
```rust
Event::SemanticPrompt(prompt) => {
    if let Some(mark) = command_mark_from(prompt) {
        self.command_marks.push(mark);
    }
}
```
with a small free function:
```rust
fn command_mark_from(event: ghostty_vt::terminal::SemanticPromptEvent) -> Option<CommandMark> {
    use ghostty_vt::terminal::SemanticPromptKind;
    match event.kind {
        SemanticPromptKind::OutputStart => Some(CommandMark::OutputStarted),
        SemanticPromptKind::CommandEnd => Some(CommandMark::Ended {
            exit_code: event.has_exit_code.then_some(event.exit_code),
        }),
        SemanticPromptKind::PromptStart | SemanticPromptKind::InputStart => None,
    }
}
```
and a new public, draining accessor - a queue, not continuous state like `cwd`/`title`,
because more than one mark can arrive from a single `write` (case 5 above: two
`OutputStarted`s), and none should be lost between two callers' reads:
```rust
/// Every `CommandMark` queued since the last call (doc-9 R14, OSC 133;
/// TASK-1.79). Drains what it returns - nothing here is continuous state
/// like `cwd`/`title`; it is a sequence of discrete occurrences.
///
/// Unbounded until a caller exists: nothing in this task calls this, so
/// in the repository's shipped state after this task merges,
/// `command_marks` grows for the whole life of any pane whose shell
/// emits OSC 133 - one `CommandMark` (8 bytes: a discriminant plus an
/// `Option<i32>`) per `C` or `D`, bounded in practice by how fast a
/// person can type commands, not by anything this method enforces. A
/// cap would be dead code with nothing to exercise it before the next
/// draft (the kernel operations, or the server's wiring) calls this and
/// actually drains it - named here deliberately, not added, so a
/// reviewer sees it was decided rather than missed.
pub fn take_command_marks(&mut self) -> Vec<CommandMark> {
    std::mem::take(&mut self.command_marks)
}
```
This is the whole new kernel surface for this task - no change to `Pane`/`Tree`/the
server (those are the other, now-separate drafts); `take_command_marks` is called by
nothing yet, same as `cwd()`/`title()` had no caller the task that added them built
either.

## Proof (AC #1, #3; point 5)

Each of the six cases above becomes its own `#[test]` in `crates/kernel/src/terminal.rs`
's existing test module, through the real public surface (`term.write(bytes)`, which
calls `vt_write` and `drain_events` together, then `term.take_command_marks()`), not
the private `engine` field the scratch verification used directly - that shortcut only
existed because `take_command_marks` doesn't exist yet, and the real test's sequence
(one call that writes-and-drains, then a separate call that takes the queue) is a
translation of what was verified, not a literal copy of the scratch code (which called
`vt_write` and `events()` separately, with nothing draining into a kernel-owned queue
in between). Concretely:
`output_started_mark_on_osc_133_c`, `command_ended_with_its_exit_code_on_osc_133_d`,
`command_ended_with_no_exit_code_reports_none` (bare `D`),
`command_ended_with_a_malformed_exit_code_reports_none` (`D;abc`),
`command_ended_fires_with_no_preceding_output_started` (`D;5` alone),
`two_output_started_marks_in_a_row_are_both_kept` (`C`, `C`, `D;7` → two marks, not one),
`a_mark_split_across_two_writes_still_resolves` (`D;` / `9\x07` across two `write` calls).
Each feeds the exact byte sequences already verified above, so the test's expected
value is something this plan already ran against the real engine, not a guess.

**Negative control, recorded on the task, not just described here**: build once with
the `install!(SEMANTIC_PROMPT, ...)` line commented out - every one of the above tests
that expects a mark fails with an empty `Vec`; restore the line - all pass. This is the
same proof already run during this plan's own verification (above), re-run once more
against the real test names once they exist, and both runs' `just test` output recorded
with `asdlc record-check`.

**Clean, no-network build**, recorded the same way TASK-1.34's own AC #1 proof was:
`cargo clean -p ghostty-vt-sys && env GHOSTTY_SOURCE_URL=unreachable just build` -
already run once during this plan's verification (39.6s, passing); re-run and recorded
once the real patch is committed.

**`just check`** (format, lint, boundaries, licences, every package's own tests, the
architecture model, recipe names) - already run in full with the patch in place during
this plan's own verification, 19/19 `ok`, proving AC #3's "no unsafe code and no I/O"
in our own crates holds (the kernel still builds under `#![forbid(unsafe_code)]`;
`crates/kernel/clippy.toml`'s I/O bans see nothing new to flag, since
`take_command_marks`/`command_mark_from` do no I/O of their own) and that
`boundaries.toml`/`deny.toml` need no edit. Re-run and recorded with `asdlc
record-check` once the real patch and tests are committed, not just the scratch copy.

## Files

- `third_party/ghostty-vt-patched/` (new: the vendored, patched crate source, ~508 KB)
  and `third_party/ghostty-vt-patched/semantic-prompt.patch` (the diff, for mechanical
  re-verification).
- `third_party/README.md` (new section, same place TASK-1.34's record lives).
- Workspace root `Cargo.toml` (`[patch.crates-io]`).
- `crates/kernel/src/terminal.rs` (`CommandMark`, the new `command_marks` field,
  `drain_events`'s new arm, `command_mark_from`, `take_command_marks`, the seven tests).

## Upstream (AC #2's "contributed to"; raised, not opened)

The change to propose to `prabirshrestha/gpui-cn` (the `ghostty-vt` maintainer) is
exactly the patch above - small, additive, no behavioural change to anything existing,
following the crate's own established per-callback pattern to the letter. I'm not
opening an issue or a PR; that's the engineer's to do or to ask for, as the team's rule
on raising things upstream says. **Drop procedure, once upstream carries it**: bump
`ghostty-vt` past whatever version wires `SEMANTIC_PROMPT` itself, delete `[patch.
crates-io]`'s entry and `third_party/ghostty-vt-patched/`, and update
`command_mark_from` to match the real `ghostty_vt::terminal::Event`'s new variant (its
exact shape may not match this patch's `SemanticPromptEvent`/`SemanticPromptKind`
verbatim - that's exactly why `CommandMark` is our own type and `command_mark_from` is
the one function that would need to change, not anything downstream of
`take_command_marks`).

## Risk

**Upstream never lands it, or lands a differently-shaped version.** Named above; the
patch is small enough (one file, +49 lines, no Zig/C) that carrying it indefinitely is
cheap, but every future bump of `ghostty-vt` needs the patch re-applied and re-verified
against the new source by hand (the `diff -rq` check above) until it's dropped.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Split on 2026-11-15 under the engineer's standing decision to split by default, along the five pieces the worker's plan proposed. This task is now piece one: the patch to ghostty-vt and the kernel's Terminal surfacing the event. Drafts under TASK-1 take the rest: status from a typed command in the kernel; the server passing the event to the kernel; shell integration so the marks are emitted. The worker's fourth piece (a loop turning a pane event into a kernel call with a clock reading) is not a draft: TASK-1.76, in progress, builds exactly that (Runtime::apply_event), so the server piece depends on it. The plan on this task was written for all five and is to be rewritten for piece one.

Rewritten plan for piece one checked against doc-9, doc-11 (R50, R82), decision-3, doc-16 and .claude/rules/rust.md by @bruk.o: ok. The route is a 49-line addition to one file of the safe ghostty-vt crate (0.8.0, Apache-2.0), vendored under third_party/ghostty-vt-patched (508 KB) through [patch.crates-io] with the diff committed beside it and the kernel's dependency pinned to the exact version so the patch cannot silently stop applying; the worker built it, fed real OSC 133 sequences through a real kernel Terminal, ran a negative control, the offline build and just check, on a scratch copy, before writing the plan. Our crates gain no unsafe code: the kernel surfaces its own CommandMark type and a draining accessor. Nothing is opened upstream; the proposal and the steps to drop the patch are written down. Decisions for the engineer at approval: the path dependency over a git dependency. Noted, not a fault: the queue of marks has no consumer until the kernel status task (a draft) drains it.

Check FAILED: `bash -lc 'export ZIG="$(mise which zig 2>/dev/null)"; just test'` exited 101 at 2026-11-15T22:00:37+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.79 on task-1.79-status-for-a-command-typed-at-a-shell at 9f68d4a4688b with uncommitted changes, last 40 of 64 lines:

```text
   --> crates/kernel/src/terminal.rs:884:18
    |
 33 | pub struct Terminal {
    | ------------------- method `take_command_marks` not found for this struct
...
884 |             term.take_command_marks(),
    |                  ^^^^^^^^^^^^^^^^^^ method not found in `terminal::Terminal`

error[E0599]: no method named `take_command_marks` found for struct `terminal::Terminal` in the current scope
   --> crates/kernel/src/terminal.rs:894:26
    |
 33 | pub struct Terminal {
    | ------------------- method `take_command_marks` not found for this struct
...
894 |         let first = term.take_command_marks();
    |                          ^^^^^^^^^^^^^^^^^^ method not found in `terminal::Terminal`

error[E0599]: no method named `take_command_marks` found for struct `terminal::Terminal` in the current scope
   --> crates/kernel/src/terminal.rs:895:27
    |
 33 | pub struct Terminal {
    | ------------------- method `take_command_marks` not found for this struct
...
895 |         let second = term.take_command_marks();
    |                           ^^^^^^^^^^^^^^^^^^ method not found in `terminal::Terminal`

error[E0599]: no method named `take_command_marks` found for struct `terminal::Terminal` in the current scope
   --> crates/kernel/src/terminal.rs:910:25
    |
 33 | pub struct Terminal {
    | ------------------- method `take_command_marks` not found for this struct
...
910 |         assert_eq!(term.take_command_marks(), vec![]);
    |                         ^^^^^^^^^^^^^^^^^^ method not found in `terminal::Terminal`

Some errors have detailed explanations: E0004, E0432, E0599.
For more information about an error, try `rustc --explain E0004`.
error: could not compile `steg-kernel` (lib test) due to 12 previous errors
full output: .quiet/tests-every-crate.log
error: recipe `test` failed with exit code 101
```

Check passed: `bash -lc 'export ZIG="$(mise which zig 2>/dev/null)"; just test'` exited 0 at 2026-11-15T22:01:52+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.79 on task-1.79-status-for-a-command-typed-at-a-shell at 9f68d4a4688b with uncommitted changes:

```text
ok      tests (every crate) (4.5s)
```

Check FAILED: `bash -lc 'export ZIG="$(mise which zig 2>/dev/null)"; just test'` exited 101 at 2026-11-15T22:02:08+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.79 on task-1.79-status-for-a-command-typed-at-a-shell at 9f68d4a4688b with uncommitted changes, last 40 of 64 lines:

```text
  left: []
 right: [OutputStarted, Ended { exit_code: Some(0) }]

---- terminal::tests::output_started_mark_on_osc_133_c stdout ----

thread 'terminal::tests::output_started_mark_on_osc_133_c' (14347606) panicked at crates/kernel/src/terminal.rs:856:9:
assertion `left == right` failed
  left: []
 right: [OutputStarted]

---- terminal::tests::command_ended_with_a_malformed_exit_code_reports_none stdout ----

thread 'terminal::tests::command_ended_with_a_malformed_exit_code_reports_none' (14347592) panicked at crates/kernel/src/terminal.rs:899:9:
assertion `left == right` failed
  left: []
 right: [Ended { exit_code: None }]

---- terminal::tests::two_output_started_marks_in_a_row_are_both_kept stdout ----

thread 'terminal::tests::two_output_started_marks_in_a_row_are_both_kept' (14347618) panicked at crates/kernel/src/terminal.rs:931:9:
assertion `left == right` failed
  left: []
 right: [OutputStarted, OutputStarted, Ended { exit_code: Some(7) }]


failures:
    terminal::tests::a_mark_split_across_two_writes_still_resolves
    terminal::tests::command_ended_fires_with_no_preceding_output_started
    terminal::tests::command_ended_with_a_malformed_exit_code_reports_none
    terminal::tests::command_ended_with_its_exit_code_on_osc_133_d
    terminal::tests::command_ended_with_no_exit_code_reports_none
    terminal::tests::output_started_mark_on_osc_133_c
    terminal::tests::take_command_marks_drains_what_it_returns
    terminal::tests::two_output_started_marks_in_a_row_are_both_kept

test result: FAILED. 272 passed; 8 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.06s

error: test failed, to rerun pass `-p steg-kernel --lib`
full output: .quiet/tests-every-crate.log
error: recipe `test` failed with exit code 101
```

Check passed: `bash -lc 'export ZIG="$(mise which zig 2>/dev/null)"; just test'` exited 0 at 2026-11-15T22:02:27+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.79 on task-1.79-status-for-a-command-typed-at-a-shell at 9f68d4a4688b with uncommitted changes:

```text
ok      tests (every crate) (4.2s)
```

Check passed: `bash -lc 'export ZIG="$(mise which zig 2>/dev/null)"; env GHOSTTY_SOURCE_URL=unreachable just build'` exited 0 at 2026-11-15T22:03:23+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.79 on task-1.79-status-for-a-command-typed-at-a-shell at 9f68d4a4688b with uncommitted changes:

```text
ok      build (39.2s)
```

Check passed: `bash -lc 'export ZIG="$(mise which zig 2>/dev/null)"; just check'` exited 0 at 2026-11-15T22:06:13+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.79 on task-1.79-status-for-a-command-typed-at-a-shell at 9f68d4a4688b with uncommitted changes:

```text
ok      format check (0.1s)
ok      lint (39.6s)
ok      boundaries (0.1s)
ok      licences (0.7s)
ok      tests (every crate) (32.3s)
ok      build (steg-sdk) (0.4s)
ok      test (steg-sdk) (1.3s)
ok      test (steg-sdk, local-socket) (1.6s)
ok      build (steg-kernel) (39.2s)
ok      test (steg-kernel) (1.4s)
ok      build (steg-server) (39.2s)
ok      test (steg-server) (2.6s)
ok      build (steg) (0.2s)
ok      test (steg) (0.4s)
ok      build (steg-app) (0.6s)
ok      test (steg-app) (0.7s)
ok      tool tests (1.3s)
ok      architecture (1.0s)
ok      recipe names (0.0s)
```

Check passed: `bash -lc 'export ZIG="$(mise which zig 2>/dev/null)"; just check'` exited 0 at 2026-11-15T22:10:32+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.79 on task-1.79-status-for-a-command-typed-at-a-shell at da963c8efdb6:

```text
ok      format check (0.1s)
ok      lint (0.4s)
ok      boundaries (0.1s)
ok      licences (0.6s)
ok      tests (every crate) (1.7s)
ok      build (steg-sdk) (0.1s)
ok      test (steg-sdk) (0.2s)
ok      test (steg-sdk, local-socket) (0.4s)
ok      build (steg-kernel) (0.1s)
ok      test (steg-kernel) (0.2s)
ok      build (steg-server) (0.1s)
ok      test (steg-server) (1.4s)
ok      build (steg) (0.1s)
ok      test (steg) (0.1s)
ok      build (steg-app) (0.3s)
ok      test (steg-app) (0.2s)
ok      tool tests (1.2s)
ok      architecture (1.1s)
ok      recipe names (0.0s)
```

Check passed: `bash -lc 'export ZIG="$(mise which zig 2>/dev/null)"; just check'` exited 0 at 2026-11-15T22:11:01+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.79 on task-1.79-status-for-a-command-typed-at-a-shell at 32ab0d9176f5:

```text
ok      format check (0.1s)
ok      lint (1.1s)
ok      boundaries (0.1s)
ok      licences (0.6s)
ok      tests (every crate) (4.2s)
ok      build (steg-sdk) (0.1s)
ok      test (steg-sdk) (0.2s)
ok      test (steg-sdk, local-socket) (0.3s)
ok      build (steg-kernel) (0.6s)
ok      test (steg-kernel) (0.8s)
ok      build (steg-server) (0.8s)
ok      test (steg-server) (2.6s)
ok      build (steg) (0.2s)
ok      test (steg) (0.4s)
ok      build (steg-app) (0.3s)
ok      test (steg-app) (0.2s)
ok      tool tests (1.2s)
ok      architecture (1.1s)
ok      recipe names (0.0s)
```

Built per the approved plan, in its order: brought in origin/main (TASK-1.41, TASK-1.81, both confirmed not touching crates/kernel/src/terminal.rs - checked by git log on that path, not assumed); derived third_party/ghostty-vt-patched/ from the exact registry copy of ghostty-vt 0.8.0; applied the one-file patch; committed semantic-prompt.patch beside it; pinned crates/kernel/Cargo.toml's dependency to the exact patched version; added the workspace root [patch.crates-io] entry; wrote third_party/README.md's new section (provenance, maintainer/repository confirmed via the crate's own Cargo.toml and crates.io's owners API - prabirshrestha/gpui-cn - the re-verification commands, the Apache-2.0 changed-file notice, and the drop procedure); built the kernel's CommandMark type, drain_events's new arm, command_mark_from, and Terminal::take_command_marks, test-first (the nine tests landed before the implementation and failed to compile, recorded).

Checks recorded on this task (asdlc record-check), in order:
1. just test - FAILED (E0599/E0432, CommandMark/take_command_marks don't exist yet) - the nine tests landing before the implementation.
2. just test - passing, once the kernel implementation was added.
3. Negative control on the real committed patch: the one new install!(SEMANTIC_PROMPT, ...) line commented out (nothing else changed) - just test FAILED, all eight marks-expecting tests with an empty queue (the ninth, no-prompt-marks-at-all, correctly still passed). Restored; just test passing again.
4. cargo clean -p ghostty-vt-sys, then env GHOSTTY_SOURCE_URL=unreachable just build - passing (39.2s), re-confirming TASK-1.34's offline-build proof holds with the patch present.
5. just check - passing in full (format, lint, boundaries, licences, every package built/tested alone, architecture, recipe names) at an intermediate head.
6. just check - passing again after rebasing onto origin/main a second time (a few more board-only commits landed) and once more at the final pushed head.
7. GitHub: PR #41's own CI run (43767882924, at 32ab0d9) green, 3m53s (the one-time Zig-link cost on a clean runner, consistent with TASK-1.34's own notes on this).

Mechanical re-verification run on the real committed files, not described secondhand: diff -rq between a pristine ~/.cargo/registry copy of ghostty-vt 0.8.0 and third_party/ghostty-vt-patched/ shows only src/terminal.rs differs (plus the registry-only files never vendored, and the added semantic-prompt.patch itself); applying that committed patch file to a fresh pristine copy with patch -p0 reproduces the vendored src/terminal.rs byte for byte (diff -u, empty output) - run twice, once right after writing the patch and once again after the negative-control round-trip, both clean.

## Coverage, clause by clause

AC #1 ("The kernel's Terminal reports, as data, when a prompt-marked command starts producing output and when it ends with its exit code... a sequence with no exit code, a malformed one, and marks arriving out of order are each handled and tested"): crates/kernel/src/terminal.rs's new CommandMark enum (OutputStarted / Ended { exit_code: Option<i32> }) and Terminal::take_command_marks(), fed real OSC 133 bytes through Terminal::write in nine tests: output_started_mark_on_osc_133_c (C alone); command_ended_with_its_exit_code_on_osc_133_d (C then D;0); command_ended_with_no_exit_code_reports_none (a bare D); command_ended_with_a_malformed_exit_code_reports_none (D;abc - verified the engine's own OSC parser degrades this the same way it handles no code at all, has_exit_code: false either way, so no extra validation was needed); command_ended_fires_with_no_preceding_output_started (D;5 alone - marks arriving out of order, the engine enforces no ordering); two_output_started_marks_in_a_row_are_both_kept (two Cs, each kept, not deduped); a_mark_split_across_two_writes_still_resolves (D; / 9\x07 across two separate write calls - the real engine's own parser buffers the split); take_command_marks_drains_what_it_returns; a_shell_with_no_prompt_marks_produces_no_command_marks (plain text, no OSC 133 at all, produces an empty queue - nothing guessed). Two more tests than the plan's own six cases named (the drain-itself and no-marks-at-all cases), additive, not a reduction.

AC #2 ("The route is the one the plan weighed and chose... with the alternatives and why they fail recorded; a second parser over the pty bytes is not built"): the plan on this task records the chosen route (patching the safe ghostty-vt crate) and why the two other named alternatives fail - a foreign-interface module outside the kernel over ghostty-vt-sys collapses into the same route as patching ghostty-vt itself, since the safe Terminal's raw handle is pub(crate) with no cross-crate accessor in either 0.8.0 or the newer 0.9.0 (checked both); reading engine state after each write (row semantic-prompt marks) cannot answer this at all, since RowSemanticPrompt carries no exit code and no Command/Output state anywhere in that path (checked by reading screen.rs). No second parser over the pty bytes was built - the one parser touched is the real engine's own, inside the patch.

AC #3 ("The kernel still has no unsafe code and no I/O; the patch is as small as it can be, its licence and origin are recorded where TASK-1.34 recorded the build... the build from a clean checkout with no network still works, and how to drop the patch... is written down"): no unsafe code or I/O was added to any of steg's own crates - crates/kernel/src/lib.rs's #![forbid(unsafe_code)] is unaffected (confirmed by it building at all, since that attribute makes a violation a compile error), and crates/kernel/clippy.toml's I/O bans see nothing new to flag, since take_command_marks/command_mark_from do no I/O of their own; ghostty-vt is a dependency, not a workspace member, so none of those lints ever reach the four lines of already-established-pattern unsafe the patch itself adds (shared_from_userdata, already used by every other callback in that file; &*event, the same shape cb_clipboard_write's &*write already uses, with a // SAFETY: comment following that crate's own convention). The patch is one file, +49 lines, 0 deletions (measured with diff -u against the pristine registry copy) - the smallest shape that wires one callback following the crate's own existing per-callback pattern. Licence and origin: third_party/README.md's new ghostty-vt-patched/ section records the crate's name/version/licence (Apache-2.0, unchanged), its maintainer and repository (prabirshrestha/gpui-cn, read from the crate's own Cargo.toml and confirmed against crates.io's owners API, not assumed), the Apache-2.0 Section 4(b) changed-file notice (both in that README section and at the top of the patched file itself), and why 0.8.0 rather than the newer 0.9.0 (checked directly: same gap exists there too, and 0.9.0 switches the whole vendoring model, which would mean redoing TASK-1.34's apparatus for no benefit to this task). The clean-checkout, no-network build (cargo clean -p ghostty-vt-sys; GHOSTTY_SOURCE_URL=unreachable; just build) passes, recorded above. The drop procedure is written down in third_party/README.md's new section: bump the pin past whatever version wires the callback itself, delete the vendored directory and the [patch.crates-io] entry, and update command_mark_from to match the real upstream Event's shape (CommandMark, steg's own type, insulates everything downstream of take_command_marks from needing to change). Proposing this upstream was raised on the task as the engineer's to do or ask for; nothing was opened.

Departure from the plan, worth naming: nine tests were built, not the plan's own six named cases - two more (the drain-itself test, and the no-prompt-marks-at-all test) were added because they were cheap and directly provable once the other seven existed, not because anything in the plan was wrong.
<!-- SECTION:NOTES:END -->

## Comments

<!-- COMMENTS:BEGIN -->
created: 2026-11-15T21:55:00Z

---

Plan rewritten for piece one (ghostty-vt patch + kernel Terminal event surfacing only). Scratch verification run during planning, outside the normal record-check flow since it was against a scratch copy of the crate, not this worktree's own committed code: applied the patch to a scratch copy of the real vendored ghostty-vt 0.8.0, pointed this worktree's Cargo.toml at it via [patch.crates-io], and ran (1) just build - full Zig/C + kernel pipeline passing; (2) a temporary test feeding the six real byte sequences AC #1 names (C then D;0; bare D; malformed D;abc; D with no preceding C; two Cs then D; a mark split across two vt_write calls) - all six behaved as the rewritten plan now states, including that a malformed exit code degrades the same way a missing one does (has_exit_code: false); (3) a negative control - commented out the one new install! line, the same test failed with zero events, restored it, passing again; (4) cargo clean -p ghostty-vt-sys && GHOSTTY_SOURCE_URL=unreachable just build - passing in 39.6s, re-confirming TASK-1.34's offline-build proof still holds with the patch present; (5) just check in full - 19/19 ok, confirming boundaries.toml and deny.toml need no edit. All experimental edits (Cargo.toml, Cargo.lock, crates/kernel/src/terminal.rs) were reverted with git checkout -- after each round; the worktree was clean (git status --porcelain empty) before and after.

---

author: @bruk
created: 2026-11-15T21:56:00Z

---

Plan approved by @bruk (8ee9fd39c481, size-5): its worker may build it.

---

author: @bruk.r1
created: 2026-11-15T22:16:00Z

---

@bruk.r1 is reviewing TASK-1.79: a same-machine review: it waited 0m with no other engineer's reviewer, longer than review_fallback_after (0m), so a fresh reviewer on its author's machine takes it.

---

author: @bruk.r1
created: 2026-11-15T22:30:00Z

---

Review by @bruk.r1 (same-machine): clean. It stays In Review for a person to merge.

Findings: 0 Important, 1 nit. The diff does not match the plan.

Reviewed at head 32ab0d9176f51c62d96f3b8bdbb977fd9ffbdc0a (PR #41, CI SUCCESS on this same OID).

Verified independently, not just read:
- Vendored tree vs. registry: diff -rq between third_party/ghostty-vt-patched/ and the exact registry copy (~/.cargo/registry/src/*/ghostty-vt-0.8.0+ghostty.1.4.0-main.83fb956/) shows exactly one source difference, src/terminal.rs, plus the expected registry-only files never vendored (.cargo-ok, .cargo_vcs_info.json, Cargo.lock, Cargo.toml.orig) and the added semantic-prompt.patch. Cargo.toml, LICENSE, LICENSE-GHOSTTY and build.rs are byte-identical to the registry copy (diff empty). File permissions match (all 644, no executable bit added). Committed patch applied with `patch -p0` to a fresh pristine copy reproduces the vendored terminal.rs byte for byte (diff -u empty).
- The patch (crate's terminal.rs, +49 lines): cb_semantic_prompt's signature matches ffi::TerminalSemanticPromptFn exactly (terminal: Terminal, userdata: *mut c_void, event: *const TerminalSemanticPrompt, returning unit); the kind match is exhaustive with an explicit `_ => return` only for GHOSTTY_SEMANTIC_PROMPT_INVALID/future values; install!(SEMANTIC_PROMPT, ...) wires Opt::SEMANTIC_PROMPT (=42), matching the sys crate's own doc comment for that slot. No catch_unwind exists anywhere in this crate's terminal.rs (checked by grep), so the new callback doesn't change the crate's existing no-panic-guard posture; it does strictly less than its neighbours (three Copy scalars, one Vec::push, no UTF-8 decoding, no unwrap/indexing) so it adds no panic path the crate doesn't already accept. The deref `&*event` and its SAFETY comment follow cb_clipboard_write's own existing pattern. `command`/`error` (the struct's FFI string fields) are correctly left unread, avoiding any string-lifetime question. The struct's `size` field needs checking only for fields added after the struct's introduction; kind/has_exit_code/exit_code have been present since introduction per the sys crate's own doc, so not checking it is sanctioned, not an omission.
- Our crates: `#![forbid(unsafe_code)]` unchanged in crates/kernel/src/lib.rs; boundaries.toml, deny.toml and crates/kernel/clippy.toml show zero diff against origin/main. Cargo.lock's ghostty-vt entry carries no `source =`/`checksum` line (resolved as a path dependency), and `cargo check -p steg-kernel` printed `Compiling ghostty-vt v0.8.0+... (.../third_party/ghostty-vt-patched)`, confirming the vendored path copy is what actually builds. Removing the `[patch.crates-io]` entry and rebuilding reproduces exactly the compile errors the notes describe (E0432/E0425/E0599, SemanticPromptKind/SemanticPromptEvent/Event::SemanticPrompt missing) against the real unpatched registry crate - the patch is load-bearing, not cosmetic.
- Kernel side: CommandMark is the kernel's own enum, never ghostty_vt's SemanticPromptEvent/SemanticPromptKind crossing out of terminal.rs (command_mark_from is private); drain_events's match on Event stays an explicit enumeration (no `_` arm), so PwdChanged/TitleChanged keep their order and nothing is dropped - rust.md's wildcard_enum_match_arm forced the new arm to exist rather than silently ignoring it. All nine recorded tests pass; I additionally wrote and ran two tests not in the diff (a mark during the alternate screen, and a mark after a resize, both named in this review's brief) directly against the real engine - both produced the expected marks, confirming the behaviour the brief asked about, then discarded the scratch tests (not committed).
- Negative control reproduced independently on the real committed crate file (not the scratch copy): commenting out the one `install!(SEMANTIC_PROMPT, ...)` line and running `cargo test -p steg-kernel --lib` fails the same 8 tests the task's own notes name (empty-queue assertions), restoring it passes all 280. Offline build reproduced independently: `cargo clean -p ghostty-vt-sys && GHOSTTY_SOURCE_URL=unreachable just build` passes in 40.8s. `just check` reproduced independently: 19/19 ok at this head.
- Licence/provenance: third_party/ghostty-vt-patched/Cargo.toml's `repository = "https://github.com/prabirshrestha/gpui-cn"` and `license = "Apache-2.0"` match third_party/README.md's claims exactly. The changed-file notice (Apache-2.0 S4(b)) is present at the top of the patched file. Size: du -sh reports 512K for third_party/ghostty-vt-patched/ (README states ~500 KB/508 KB - consistent, naming the measured number here).

Finding:
[Compliance, Nit] crates/kernel/Cargo.toml:24-31 and third_party/README.md's ghostty-vt-patched section both say the exact-version pin (`ghostty-vt = "=0.8.0+ghostty.1.4.0-main.83fb956"`) is "specifically so this patch can never silently stop applying" against a newer 0.8.x. Cargo discards semver build-metadata in a version requirement: removing the `[patch.crates-io]` entry and re-resolving against this same manifest line prints "version requirement '=0.8.0+ghostty.1.4.0-main.83fb956' for dependency 'ghostty-vt' includes semver metadata which will be ignored" - the real constraint cargo enforces is `=0.8.0`, not the full build-tagged string. The protective intent still holds in practice (`=0.8.0` already excludes an unpatched 0.8.1, the stated risk, and the vendored copy's own version satisfies `=0.8.0` too), so this isn't a behavioural bug, but the comment and README overstate what the pin mechanically does, which could mislead whoever next re-derives this patch against a different 0.8.x into thinking the full build tag is what's being matched.

plan-matched: no, for the record, not as a fault - two departures the worker's own notes name or that are a direct, undisputed consequence of what the plan specified: (1) notes head "Departure from the plan, worth naming: nine tests were built, not the plan's own six[/seven] named cases," explained in the notes; (2) the diff touches crates/kernel/src/lib.rs (`pub use terminal::{CommandMark, Terminal}`), which the plan's Files list does not itemise, but is the mechanical re-export of the public type the plan did specify.

No Important finding. Point-by-point verification (vendoring, the patch itself, our crates' unsafe/IO posture, the kernel's own type and drain_events, evidence, licence/record, offline build size) all checked out against the primary sources, not just the notes.

---
<!-- COMMENTS:END -->
