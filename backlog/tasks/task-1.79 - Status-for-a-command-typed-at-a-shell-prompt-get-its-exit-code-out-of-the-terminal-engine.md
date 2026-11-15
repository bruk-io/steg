---
id: TASK-1.79
title: >-
  Terminal: surface a prompt-marked command's start and end, with its exit code,
  from the engine
status: In Progress
assignee:
  - '@bruk.w1'
created_date: '2026-11-15T19:43:00Z'
updated_date: '2026-11-15T22:10:00Z'
labels:
  - size-5
milestone: m-4
dependencies:
  - TASK-1.41
parent_task_id: TASK-1
project: bruk-io-steg
asdlc_accepted_from: DRAFT-6
asdlc_branch: task-1.79-status-for-a-command-typed-at-a-shell
asdlc_plan_approved_by: bruk
asdlc_plan_approved_hash: '8ee9fd39c481'
asdlc_plan_approved_size: size-5
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
<!-- COMMENTS:END -->
