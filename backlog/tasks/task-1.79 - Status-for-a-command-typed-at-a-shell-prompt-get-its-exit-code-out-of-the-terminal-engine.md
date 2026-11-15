---
id: TASK-1.79
title: >-
  Status for a command typed at a shell prompt: get its exit code out of the
  terminal engine
status: In Progress
assignee:
  - '@bruk.w1'
created_date: '2026-11-15T19:43:00Z'
updated_date: '2026-11-15T21:42:00Z'
labels:
  - size-5
milestone: m-4
dependencies:
  - TASK-1.41
parent_task_id: TASK-1
project: bruk-io-steg
asdlc_accepted_from: DRAFT-6
asdlc_branch: task-1.79-status-for-a-command-typed-at-a-shell
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Split from TASK-1.41 (2026-11-15). doc-11: R50, R82, R83; decision-7; doc-9 for prompt marks. TASK-1.41's worker read the exact dependency: ghostty-vt-sys 0.8 carries GHOSTTY_TERMINAL_OPT_SEMANTIC_PROMPT with has_exit_code and exit_code on the command-end step, but ghostty-vt 0.8's safe Terminal has no event for it, and its get/set are pub(crate). The server cannot supply the code either: the command is a child of the shell, not of steg. This is the status most of daily use depends on, since most commands are typed at a prompt, so it belongs in M1. Unknowns dominate the first half of this task; if the plan finds the route needs upstream work that cannot land in time, say so and propose the stopgap rather than building one quietly.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A command typed at a shell prompt that reports prompt marks (OSC 133) shows running while it runs and finished or failed from its exit code when it ends (R50, R82), in a shell pane whose shell never exits between commands
- [ ] #2 How the exit code leaves the engine is decided in the plan with the alternatives weighed: a change contributed to or carried against the ghostty-vt crate so its safe API surfaces the semantic-prompt callback; a small foreign-interface module outside the kernel over ghostty-vt-sys; or another route found by reading the engine. A second parser over the pty bytes kept in step with the engine is the last resort and needs the engineer's word
- [ ] #3 The kernel still has no unsafe code and no I/O; anything foreign lives in one small module behind a safe API in a crate allowed to hold it (doc-16, .claude/rules/rust.md), and any change to vendored or patched dependencies is recorded where TASK-1.34 recorded the build
- [ ] #4 A shell that reports no prompt marks shows no finished or failed status for typed commands and nothing is guessed; the ending rules of TASK-1.41 apply unchanged to statuses that arrive this way
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
## Recommendation up front

Route **(a)**: patch the safe `ghostty-vt` crate (Rust-only, ~25 lines in one file) to wire
`GHOSTTY_TERMINAL_OPT_SEMANTIC_PROMPT`, carried as a pinned fork via `[patch.crates-io]`
under the *same* crate name/version so no other file in the repo (`boundaries.toml`,
every `Cargo.toml`) has to change. **Built, not just sketched**: I applied this exact
patch to a scratch copy of the real vendored 0.8.0 source, pointed this worktree's
`Cargo.toml` at it via `[patch.crates-io]` with a `path` dependency, and ran `just build`
- the full pipeline (`ghostty-vt-sys`'s Zig/C build against the vendored `third_party/`
tree, unchanged, through `ghostty-vt`'s own build, through `steg-kernel`) succeeded.
I then added a temporary test directly in `crates/kernel/src/terminal.rs` that feeds the
literal bytes `\x1b]133;D;3\x07` into a real `Terminal` and asserts the patched crate's
`Event::SemanticPrompt` comes back with `has_exit_code: true, exit_code: 3` - it passed
(`cargo test -p steg-kernel --lib scratch_semantic_prompt_event_reaches_the_engine`, 1
passed). All experimental edits (`Cargo.toml`, `Cargo.lock`, `crates/kernel/src/
terminal.rs`) were then reverted with `git checkout --`; the worktree is clean and
nothing of this experiment is committed. Open an issue/PR against upstream
(`prabirshrestha/gpui-cn`) at the same time; if it never lands, the fork is maintained
indefinitely at this size and cost - a real but small ongoing cost, named as a risk below,
not hidden.

**AC #1 cannot be satisfied by this task alone, and I'm saying so rather than building
around it quietly.** `record_exit`/`PaneEvent::Exited` already has no call site that
turns it into a `Tree::record_exit` call anywhere in this repo - that loop is TASK-1.76's
own, still To Do, and its own module comment calls it "a known gap, unreachable by any
client today." The two new `PaneEvent` variants this task adds have exactly the same
gap: nothing today reads `PaneEvent` and calls `Tree` with a real `Moment`. AC #1 asks
for an end-to-end, observable "shows running... finished or failed," and that cannot be
demonstrated without that loop existing somewhere. See piece 4 and the split below for
the engineer's two choices.

**Engineer decisions this plan surfaces, not mine to make:**
1. Fork-and-patch `ghostty-vt` now (route a, built and proven above), vendored as a
   `path` dependency under `third_party/` (consistent with how `ghostty`'s own source is
   vendored, and the form I actually tested) or pulled via a pinned `git` dependency
   (simpler to update, but a new network source at `cargo fetch` time -
   crates.io/github.com, not `tip.files.ghostty.org`, so it does not reopen R92's closed
   surface, but it is a new one). I recommend the vendored path dependency, both for
   consistency with this repo's existing policy and because it's the form I verified.
2. Whether steg ships/sources ghostty's own shell-integration scripts (zsh/bash/fish/
   elvish; **nushell's ghostty integration never emits OSC 133 at all** - checked by
   reading it, not assumed) automatically for every shell pane, or leaves that to the
   person's own shell config. `ghostty-vt` already carries these scripts
   (`shell_integration::resources_dir()`) and the exit code is really in them
   (`\e]133;D;$status\a`, read directly out of the vendored zsh/bash/fish/elvish scripts).
   This is R82/decision-7's whole premise ("depend on the shell reporting prompt marks")
   and AC #4 is explicitly fine with "no marks, no guess" - so this is a UX/adoption
   decision, not a correctness one, and I'm not deciding it here. One constraint is not
   optional, though: `resources_dir()` writes files to a cache directory (its own doc
   comment says so), which is real file I/O - `crates/kernel/clippy.toml`'s
   `disallowed-methods` already forbids exactly this shape (`std::fs::write`,
   `create_dir_all`, etc., "doc-16 R110: no file I/O in the kernel"), so whatever calls it
   has to live in the server (or `steg`), never the kernel, wherever `LaunchSpec`'s
   environment is already built.
3. If (a) falls through (upstream refuses a patch of this shape, or the engineer declines
   to carry a fork at all): the stopgap is **(d)**, scoped as tightly as I can state it -
   see "If route (a) fails" below. I recommend against building it pre-emptively.
4. The split below (five tasks). Only the first two need no new decision from TASK-1.76.
   The third is independent too. The fourth is where AC #1's gap above lives - whether to
   fold the `PaneEvent` → `Tree` loop into this task's own scope (duplicating or
   preempting part of TASK-1.76) or to accept that AC #1 is only demonstrable end to end
   once TASK-1.76's loop exists, with this task building everything short of that.

## What I verified by reading or running, vs. what I infer

**Read directly, this session**, against the real vendored sources on this machine
(`~/.cargo/registry/src/*/ghostty-vt-sys-0.8.0+*/src/bindings.rs`,
`~/.cargo/registry/src/*/ghostty-vt-0.8.0+*/src/{terminal,screen,osc,shell_integration}.rs`,
and the shell-integration scripts the crate embeds), and against this repo's own
`crates/kernel/src/{pane,terminal}.rs`, `crates/kernel/src/tree.rs`,
`crates/server/src/pane.rs`, `third_party/README.md`, `.cargo/config.toml`,
`boundaries.toml`, `deny.toml`, doc-9, doc-11, doc-16, decision-3, decision-7,
`.claude/rules/rust.md`, and TASK-1.34/1.41's own notes and review comments:

- `TerminalSemanticPrompt` (`ghostty-vt-sys` 0.8.0, `bindings.rs:3291-3330`) carries
  `kind` (`PROMPT_START`/`INPUT_START`/`OUTPUT_START`/`COMMAND_END`), `has_exit_code`,
  `exit_code` (i32, can be negative), `command` (the about-to-run command line, set on
  `OUTPUT_START` only) and `error` (rarely sent). The callback type
  `TerminalSemanticPromptFn` and the option tag `TerminalOption::SEMANTIC_PROMPT = 42`
  both exist in the sys crate today.
- `ghostty-vt` 0.8.0's safe `Terminal::install_callbacks` (`terminal.rs:945-985`) wires
  eleven callbacks through one macro (`install!(OPT, FnType, cb_fn)`) and
  **`SEMANTIC_PROMPT` is not among them** - confirmed by reading the whole function, not
  by its absence from a grep. `Event` (`terminal.rs:1223-1254`) has no variant for it
  either. The pattern every existing callback follows (e.g. `cb_pwd_changed`,
  `terminal.rs:1042-1046`) is three lines: read the `shared` state from userdata, pull
  the C struct's fields into an owned `Event::Variant`, push it.
- The handle's internals are `pub(crate)`: `Terminal.inner: NonNull<ffi::TerminalImpl>`
  (`pub(crate)` even in the newer 0.9.0, checked both), `shared: NonNull<Shared>`
  (private), `fn as_raw(&self) -> ffi::Terminal` (`pub(crate)`), `fn set`/`set_ptr`
  (`pub(crate)`). **There is no accessor at all that crosses the crate boundary with the
  raw pointer or the installed-callback state.** This is why I'm folding the task's
  routes (a) and (b) into one finding below rather than weighing them as genuinely
  separate options.
- `RowSemanticPrompt` (`screen.rs:398-411`, `RowData::SEMANTIC_PROMPT`) - the one piece
  of OSC 133 state the safe API *does* expose today - has exactly three values: `None`,
  `Prompt`, `Continuation`. No `Command`/`Output` state, no exit code, nowhere in that
  path. Confirms route (c) cannot answer AC #1 at all, not just imperfectly.
- Checked the newest release on crates.io, not assumed stale: `ghostty-vt` 0.9.0
  (`+ghostty-vt.1.0.1`, published 2026-11-14, one day before this task) and its matching
  `ghostty-vt-sys` 0.9.0. Downloaded and extracted both (crates.io's download endpoint
  needs a `User-Agent` header or it 403s; no credentials involved). Same finding:
  `SEMANTIC_PROMPT` still unwired, `inner`/`as_raw` still `pub(crate)`. 0.9.0 also
  switches `ghostty-vt-sys`'s vendoring model from "download Ghostty's own source and
  build the whole engine with Zig" to "link a prebuilt `libghostty-vt` release artifact
  (an xcframework/static-lib), falling back to a source build" - a materially different
  vendoring shape from what TASK-1.34 built and documented, so upgrading to 0.9.0 later
  is its own task, independent of this one, and not something I'm recommending here.
- Checked upstream's own issue tracker (`prabirshrestha/gpui-cn`, the `ghostty-vt`
  maintainer - this is their own GPUI UI-kit application; `ghostty-vt` is a sub-crate of
  it, not a standalone terminal-engine project) via the GitHub API: no open or closed
  issue mentions `semantic_prompt` or program-status wiring. The repo is active (25
  issues, most closed, a release as recent as yesterday), so a PR has a real chance of
  review, but nothing says it will land on M1's timeline.
- Confirmed the exit code really is on the wire: read the vendored zsh, bash, fish and
  elvish integration scripts byte for byte. zsh: `print -nu $_ghostty_fd
  '\e]133;D;'$cmd_status'\a'`. bash: `printf "\e]133;D;%s;aid=%s\a" "$ret" "$BASHPID"`.
  fish: `echo -en "\e]133;D;$status\a"`. elvish: `printf "\e]133;D;"$exit-status";aid="$pid"\a"`.
  All four also have a bare-`D`-with-no-code path (an unclosed prior mark being closed),
  so `has_exit_code` genuinely varies even with real integration - never assume it's
  always true for `COMMAND_END`.
- Read `crates/kernel/src/pane.rs` (TASK-1.41, on `origin/main`) and `crates/server/src/
  pane.rs` (TASK-1.40) in full. **The kernel's `Terminal` (the ghostty-vt wrapper) is not
  owned by `Pane`/`Tree` at all** - it lives on the server's per-pane worker thread
  (`crates/server/src/pane.rs::worker_loop`), which already drains `ghostty_vt::Event`
  inside `Terminal::write` (title/pwd today) on every `PtyBytes` command, and already has
  exactly the channel shape this task needs: `PaneEvent` (`Send`, crosses to wherever
  `Tree` lives) is how `Exited{pane, exit_code}` already travels, for TASK-1.76 to one
  day call `Tree::record_exit` with a `Moment` it reads itself. That call site does not
  exist yet (`record_exit` has no caller anywhere in `crates/server/src/*.rs` except
  tests) - TASK-1.76 is still To Do. This is the key architectural fact the split below
  is built on.

**Inferred / judgement, flagged as such:**
- That vendoring the patched `ghostty-vt` as a `path` dependency (not `git`) is the
  better fit for this repo - inferred from decision-3/R92's existing preference for
  vendoring over any new network source, not read as a rule that forbids `git` deps.
- That `note_command_output_started`/`note_command_ended` (names below) should accept
  any pane kind, not just `Program::Shell` - my reading of "harmless either way", open to
  the engineer scoping it to shells only.
- Everything about whether this is too small/large a set of tasks, and the exact task
  boundaries - judgement, see the split.

## Route (a)/(b), merged: the actual patch

`terminal.rs`, inside `install_callbacks`'s macro list, one more line:
```rust
install!(SEMANTIC_PROMPT, ffi::TerminalSemanticPromptFn, cb_semantic_prompt);
```
and, alongside `cb_pwd_changed`, a new extern "C" fn of the same shape:
```rust
unsafe extern "C" fn cb_semantic_prompt(
    _t: ffi::Terminal,
    ud: *mut c_void,
    event: *const ffi::TerminalSemanticPrompt,
) {
    let shared = unsafe { shared_from_userdata(ud) };
    let event = unsafe { &*event };
    shared.events.push(Event::SemanticPrompt(SemanticPromptEvent {
        kind: semantic_prompt_kind_from(event.kind),
        has_exit_code: event.has_exit_code,
        exit_code: event.exit_code,
    }));
}
```
plus a new, small public enum/struct beside `Event` (`command`/`error` left off for M1 -
nothing downstream needs them yet, and the struct can grow non-breakingly):
```rust
pub struct SemanticPromptEvent {
    pub kind: SemanticPromptKind,
    pub has_exit_code: bool,
    pub exit_code: i32,
}
pub enum SemanticPromptKind { PromptStart, InputStart, OutputStart, CommandEnd }
```
and one new `Event::SemanticPrompt(SemanticPromptEvent)` variant. This is the whole
patch: no `unsafe` beyond what every other callback in this file already does (each with
its own `// SAFETY:` comment, same pattern), no Zig/C change, no build-script change.
`GHOSTTY_SEMANTIC_PROMPT_INVALID` maps to... nothing sensible; I'd make
`semantic_prompt_kind_from` return `Option<SemanticPromptKind>` and drop the event
silently on `INVALID` (the sys crate's own doc never says a real shell sends it).

**Vendoring it:** copy the crate's source (Apache-2.0, same licence as the unpatched
0.8.0; carries the same NOTICE obligation already named in decision-3 section 6) into
`third_party/ghostty-vt-patched/` (or similar), apply the diff above, and point
`crates/kernel/Cargo.toml`'s existing `ghostty-vt = "0.8"` at it via
`[patch.crates-io]` in the workspace root `Cargo.toml` - keeping the package name and
version compatible so `boundaries.toml`'s `[steg-kernel] allow = [..., "ghostty-vt"]`
and every other reference to the crate name needs no change. `third_party/README.md`
gets a new section recording this exactly where TASK-1.34 recorded the Zig/C vendoring -
the diff, the upstream issue/PR link once filed, the licence (unchanged, Apache-2.0), and
the maintenance plan (re-diff against each upstream release; drop the patch the day
upstream wires it itself).

**If route (a) fails** (upstream refuses, or the engineer declines to carry a fork): the
stopgap is a parser that recognises *only* the literal `ESC ] 1 3 3 ; D ( ; <digits> )?
(BEL | ESC \)` byte sequence in the raw bytes a shell pane's `PtyBytes` carries, run
*alongside* (never replacing) the real engine write, feeding a result into the same
`Tree::note_command_ended` this plan builds either way. Named risks, stated plainly
because decision-3's own argument for choosing ghostty over alacritty was exactly "a
second parser over every byte of output on the hot path" (section 4) - this reopens
precisely that cost, for one sequence: it has no VT/string-literal state, so a program
that `cat`s a file containing that literal byte sequence as *data* (not a real mark)
would misfire; a mark split across two `PtyBytes` chunks needs its own small buffering,
duplicating what the real engine's OSC parser already does correctly; and it is a
second thing to keep in step with ghostty's own grammar if it ever changes. This needs
the engineer's explicit word before being built, as the task's own AC #2 says, and I'm
not building any of it now.

## The kernel operations (point 4)

`record_exit` as it stands is for a pane's *own* program ending (`ProgramStatus`,
`closes_on_exit`'s cascade) - a shell pane's program is the shell itself, which does not
exit between typed commands, so reusing `record_exit` here is wrong, exactly as the task
brief warns. Two new, narrower operations instead, modelled on `start_program`/
`record_exit`'s own split but touching only `status`, never `program_status`:

`crates/kernel/src/pane.rs`:
- `pub(crate) fn note_command_output_started(&mut self, tick: u64)` - sets
  `status = Some(StatusRecord { kind: StatusKind::Running, tick })`, nothing else. Fires
  on OSC 133 `C` (`OutputStart`).
- `pub(crate) fn note_command_ended(&mut self, exit_code: i32, now: Moment, tick: u64)` -
  sets `status` to `Finished`/`Failed` exactly as `record_exit` already decides
  (factor that one `if exit_code == 0 {..} else {..}` out of `record_exit` into a small
  private `fn status_kind_for_exit(exit_code: i32, now: Moment) -> StatusKind` both call,
  so the two can never silently diverge). Fires on OSC 133 `D` *only when
  `has_exit_code`* - when it isn't, nothing is called at all (AC #4's "nothing is
  guessed" extends to a bare `D` too, which real shells do send sometimes, per the
  script excerpts above).

`crates/kernel/src/tree.rs`:
- `pub fn note_command_output_started(&mut self, pane: PaneId) -> Result<(), Error>` -
  looks the pane up (`Error::PaneNotFound`, the one guard every other pane op has),
  takes the tree's own `tick()` the same way `start_program` does, delegates.
- `pub fn note_command_ended(&mut self, pane: PaneId, exit_code: i32, now: Moment) ->
  Result<(), Error>` - same guard, delegates. No `ExitOutcome`, no cascade: this never
  closes a pane.

Both accept any pane kind unconditionally (judgement call above) rather than checking
`Program::Shell` - open to the engineer narrowing it.

Between commands (OSC 133 `A`/`B`, prompt/input start): nothing happens. R82 is scoped to
"running while it runs" and M1 has no `Waiting`; a shell sitting idle at its own prompt,
with the previous command's status already cleared by a keystroke
(`Tree::note_input`, unchanged), correctly shows no status at all until the next command's
`C` arrives - not a gap, the right answer for "nothing is running."

## Wiring it live (point 4, "what the server passes in")

Exactly where `PaneEvent::Exited` already sits, next to it:
```rust
pub enum PaneEvent {
    Exited { pane: PaneId, exit_code: i32 },
    CommandOutputStarted { pane: PaneId },
    CommandEnded { pane: PaneId, exit_code: i32 },
}
```
`worker_loop`'s `PtyBytes` arm, right after `terminal.write(&bytes)`, drains whatever new
semantic-prompt events that write queued (same shape `Terminal::drain_events` already
uses for title/pwd - a new `Terminal::take_semantic_prompt_events(&mut self) ->
Vec<SemanticPromptEvent>` or similar) and sends the matching `PaneEvent`s. A `Moment` is
never read in this module - exactly as today, it has no clock and sends raw facts; the
real clock reading happens where `Tree` lives, which is the same place TASK-1.76 will
(or does) read a `Moment` before calling `record_exit`. That call site does not exist in
this repo today for *any* `PaneEvent`, which is the reason this plan does not try to
build "wire `CommandEnded` into a live `Tree`" as part of this task - see the split.

## Splitting (point 5)

Not one reviewable change - five pieces, three genuinely independent of each other and
two coupled to TASK-1.76's own unbuilt wiring:

1. **Patch `ghostty-vt`, vendor it, surface `SemanticPromptEvent` from the kernel's
   `Terminal`.** AC #2, half of AC #3 (the vendoring record). Proof: real OSC 133 byte
   sequences (`A`, `B`, `C`, bare `D`, `D;0`, `D;3`, a mark split across two `write`
   calls, and a mark with no shell integration active at all producing nothing) fed
   straight into `Terminal::new`/`write`, asserting on the drained events - no server, no
   real shell, no pty. Independent of every other piece.
2. **The kernel operations** (`note_command_output_started`/`note_command_ended`,
   `status_kind_for_exit` factored out). Rest of AC #3 (no unsafe, no I/O - this is pure
   `Pane`/`Tree` logic). Proof: literal `Moment`s and ticks, mirroring `record_exit`'s
   own existing tests one for one, plus the "ends with no exit code does nothing" case
   and "AC #4 guessed-nothing" case at the pane level. Independent of piece 1 - can build
   against hand-constructed `SemanticPromptEvent` values without the real patch existing
   yet, though it reads more honestly built after piece 1 lands.
3. **Wire pieces 1+2 together inside the server's per-pane worker thread**
   (`PaneEvent::CommandOutputStarted`/`CommandEnded`, the `PtyBytes` arm). Depends on 1
   and 2. Proof: a real pty, a real shell with ghostty's shipped integration sourced
   (zsh, bash and fish at minimum - elvish/nushell if the engineer wants the full set),
   asserting the right `PaneEvent`s come out of `events_rx`, the same style
   `real_pty.rs` already uses for `Exited`. **Still does not call `Tree`** - there is
   nowhere to call it from yet, same as `Exited` today.
4. **The call site that turns a `PaneEvent` into a `Tree` call with a real `Moment` -
   where AC #1's gap, named above, actually lives.** This is the same gap
   `PaneEvent::Exited` already has, and TASK-1.76 is where it's named as a "known gap,
   unreachable by any client today." Without this loop existing somewhere, AC #1 is not
   demonstrable end to end, no matter how well 1-3 and 5 land. Two choices, the
   engineer's:
   - **(i)** Fold one small control-thread loop - reading `PaneEvent`, reading a real
     clock once per event, calling `Tree::record_exit`/`note_command_output_started`/
     `note_command_ended` - into *this* task's own scope, ahead of TASK-1.76, which then
     reuses it for `Exited` when it lands. This is the only way AC #1 gets a real,
     end-to-end proof inside TASK-1.79 itself.
   - **(ii)** Leave it to TASK-1.76, and accept that TASK-1.79 proves AC #1's *logic*
     fully (pieces 1-3, each demonstrated independently, as precisely as AC #2-#4 ask)
     but its end-to-end, observable claim waits until TASK-1.76 merges. State that
     explicitly on this task rather than marking AC #1 done on partial evidence.
5. **Shell integration itself** (decision #2 above) - whether/how steg gets
   `GHOSTTY_RESOURCES_DIR` pointed at `shell_integration::resources_dir()` for a shell
   pane, or documents it for the person to opt into. Depends on nothing above
   technically (it's independent of whether the kernel can *read* the marks), but is
   pointless to ship before piece 1 exists and this task's own AC #1 needs it to be true
   for the end-to-end proof. I'd sequence it after 1-3, built once there's something for
   it to feed.

What proves what, by AC: AC #1 needs 1+2+3+4+5 together (end to end) but the *logic* is
fully proven by 1+2 alone, each in isolation, before 3/4/5 exist. AC #2 is this plan.
AC #3 is 1 (vendoring record) + 2 (kernel purity). AC #4 is provable at both the piece-1
level (no marks in -> nothing out) and the piece-2 level (no exit code -> no call) without
needing a real shell at all, plus once more end to end with a real shell that has no
integration (`sh` with nothing sourced).

## Files

- `third_party/ghostty-vt-patched/` (new, vendored fork) or a pinned `git` dependency -
  engineer's call (point 1 above); workspace root `Cargo.toml` (`[patch.crates-io]`);
  `third_party/README.md` (new section, same place TASK-1.34's record lives).
- `crates/kernel/src/terminal.rs` (patch content lives in the vendored crate, not here;
  this file's `drain_events`/a new `take_semantic_prompt_events` accessor - piece 1).
- `crates/kernel/src/pane.rs` (`note_command_output_started`, `note_command_ended`,
  `status_kind_for_exit` factored out of `record_exit` - piece 2).
- `crates/kernel/src/tree.rs` (the two matching public methods - piece 2).
- `crates/server/src/pane.rs` (`PaneEvent` gains two variants, `worker_loop`'s `PtyBytes`
  arm drains and forwards them - piece 3).
- Wherever TASK-1.76 puts its `PaneEvent` → `Tree` loop (piece 4, raised with that task's
  planner rather than claimed here).
- Shell-integration sourcing, file(s) TBD by the engineer's decision #2 (piece 5).

## Risks

- **Upstream never lands it.** Named above; the fork is small (~25 lines, Rust-only,
  no Zig/C) so carrying it indefinitely is cheap, but it is a real, standing maintenance
  item - every future `ghostty-vt` bump needs the patch re-applied by hand until dropped.
- **`has_exit_code` is sometimes false even with real integration** (the bare-`D` case,
  found in all four scripts read) - `note_command_ended` must never be called for that
  case, or AC #4's "nothing is guessed" is silently violated for a real shell, not just
  an unintegrated one.
- **Piece 4's ownership.** If TASK-1.76 is already past its own planning by the time this
  lands, its plan won't mention `CommandOutputStarted`/`CommandEnded` - whoever reviews
  either task should check the other.
- **`ghostty-vt` 0.9.0 exists and is newer.** Not adopting it here (different vendoring
  shape entirely) is a deliberate scope decision, not an oversight - named so a later
  reader doesn't "fix" this plan by bumping the version and finding the same gap.
<!-- SECTION:PLAN:END -->
