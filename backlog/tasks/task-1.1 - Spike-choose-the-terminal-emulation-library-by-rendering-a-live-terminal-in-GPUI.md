---
id: TASK-1.1
title: >-
  Spike: choose the terminal emulation library by rendering a live terminal in
  GPUI
status: In Progress
assignee:
  - '@bruk.w1'
created_date: '2026-11-13T22:52:00Z'
updated_date: '2026-11-14T01:10:00Z'
labels:
  - size-3
milestone: m-0
dependencies: []
parent_task_id: TASK-1
type: spike
project: bruk-io-steg
asdlc_accepted_from: DRAFT-2
asdlc_branch: task-1.1-spike-choose-the-terminal-emulation
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Answers intent question 3. Render one live shell in a GPUI window with libghostty-vt and with alacritty_terminal, and measure each against R19. Covers the risk in R14 and R19, and R59: no code from GPL sources, so nothing is copied from Zed's terminal crates. The code is throwaway; the answer is what this delivers.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A board document records the chosen library and why, with the measured 95th percentile typing latency and the result of printing a 100 MB file for each candidate
- [ ] #2 The document confirms or replaces the R19 numbers, which the spec marks as proposals
- [ ] #3 The document names the licence of every dependency the chosen approach needs
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
## What this delivers, and what it does not

The deliverable is one board document (AC #1-#3). The Rust code is a throwaway measuring
instrument that is kept only so the numbers can be re-run and checked. Nothing here is
production code, nothing is wired into the workspace TASK-1.5 creates, and no crate in
`spikes/` ever becomes a workspace member.

The spike answers intent question 3 with three kinds of evidence, because one number cannot
decide it:

- **A. Emulator alone, no GPUI.** Bytes in, screen read back (the technique
  `.claude/rules/testing-rust.md` names). This is where the candidates actually differ:
  parse throughput, screen-read cost, memory, and R14 feature coverage.
- **B. One live shell in a GPUI window, per candidate.** Confirms or replaces the R19
  numbers (AC #2) and proves each candidate can be driven from GPUI at all.
- **C. Fit with steg's architecture.** Thread-ownership shape, the extension points steg
  needs (unknown-OSC passthrough for OSC 7501, OSC 133, damage tracking, screen+scrollback
  snapshot for R2), build and distribution cost, licence (AC #3).

Measured-but-not-in-the-ACs items (C, and the R14 matrix) go in the document as the "why"
AC #1 asks for. They may well outweigh the milliseconds.

## Where the throwaway code lives

`spikes/task-1.1-terminal-library/`, inside this worktree, as its own Cargo workspace.

Two collisions make the location load-bearing:

- `justfile:12` turns every Rust step on with `path_exists("Cargo.toml")` at the repository
  root. A root manifest here would switch `fmt-check`, `lint`, `deny` and `test` on at once,
  and `deny` would fail because no `deny.toml` exists yet. **No root `Cargo.toml` is created
  by this task.**
- TASK-1.5 adds the real root workspace in another worktree, in parallel. So
  `spikes/task-1.1-terminal-library/Cargo.toml` carries its own `[workspace]` table, which
  makes cargo treat it as a separate workspace root and stops the future root manifest from
  trying to adopt it. After TASK-1.5 lands, `steg-affected` sees these files as belonging to
  no workspace crate, so `just test` selects nothing from them.

`target/` is already ignored at any depth by `.gitignore:5`. The 100 MB fixture is generated
under `$TMPDIR` at run time and is never committed.

## Files that change

Spike code (new, all under `spikes/task-1.1-terminal-library/`):

- `Cargo.toml`: the spike's own workspace, `[workspace]` with members `probe-core`,
  `probe-alacritty`, `probe-ghostty`, and `[profile.release]` settings used for every
  measurement.
- `Cargo.lock`: committed, so the measured versions are recorded exactly.
- `README.md`: how to run each measurement, what each number includes and excludes, and the
  machine it was measured on.
- `probe-core/Cargo.toml`: depends on `gpui = "=0.2.2"` and a pty crate (see step 4).
- `probe-core/src/lib.rs`: `mod`/`pub use` only, per `.claude/rules/rust.md`.
- `probe-core/src/backend.rs`: the `TerminalBackend` trait both adapters implement, and the
  plain-data snapshot types it returns (`Snapshot`, `Row`, `Cell`: char, fg, bg, flags).
  Deliberately **not** `Send`, so the libghostty adapter fits the same trait.
- `probe-core/src/pty.rs`: open a pty, spawn a program, read its output. The same pty code
  serves both candidates, so the comparison is of emulators and not of pty plumbing.
- `probe-core/src/element.rs`: the GPUI element and window, written once and shared, so both
  candidates are drawn by identical code. One shaped line per terminal row via
  `WindowTextSystem::shape_line` with a per-row cache keyed on the row's contents, painted
  with `ShapedLine::paint_background` then `ShapedLine::paint`, plus a cursor rectangle.
- `probe-core/src/latency.rs`: the keystroke driver and the percentile maths.
- `probe-core/src/flood.rs`: generate the 100 MB fixture and measure printing it.
- `probe-core/src/conformance.rs`: the R14 byte scripts and the expected screen for each.
- `probe-core/src/report.rs`: print every measurement as one table, so the document quotes
  tool output rather than prose.
- `probe-alacritty/Cargo.toml`, `probe-alacritty/src/adapter.rs`, `probe-alacritty/src/main.rs`:
  `alacritty_terminal = "0.26"` behind `TerminalBackend`, and the binary that runs the three
  measurements.
- `probe-ghostty/Cargo.toml`, `probe-ghostty/src/adapter.rs`, `probe-ghostty/src/main.rs`:
  the same for `libghostty-vt = "0.2"`.

Board (written by `asdlc`, never by hand):

- `backlog/docs/doc-<n> - *.md`: the answer, created with `asdlc board doc create` and filled
  with `asdlc board doc update --content`.
- `backlog/tasks/task-1.1 - *.md`: the document linked with
  `asdlc board task edit TASK-1.1 --add-doc doc-<n>`, plus notes and the recorded checks.

## Order of work

1. **Record the starting point.**
   `asdlc record-check TASK-1.1 --in <worktree> --as bruk.w1 -- just check`.
   Expect every Rust step to say it is skipped and the tool tests to pass; that is the
   honest baseline before anything is added.
2. **Prove GPUI works on this machine, standalone.** A scratch crate (outside the
   repository, under the scratchpad) with `gpui = "=0.2.2"`, built `--release`, opening a
   window that paints one shaped line, adapted from gpui 0.2.2's own `examples/` and its
   rustdoc. Record the first-build wall time. If the crates.io `gpui` does not build or run
   outside Zed's repository, that is the spike's first headline finding: write it down,
   switch to a pinned git dependency on `zed-industries/zed` (gpui itself is Apache-2.0),
   and record the change.
3. **Prove both emulators build and parse.** `probe-alacritty` and `probe-ghostty` with the
   smallest possible smoke test: feed `"hi\r\n"`, read the screen back, print it. For the
   ghostty side specifically record: the first-build wall time from a clean cargo cache,
   that `libghostty-vt-sys`'s `build.rs` clones ghostty at pinned commit
   `a887df42c56f6de86c0fe6da9c4eeca37931e083` over the network and builds it with Zig
   (0.15.2 is installed), whether a second build is cached, and whether
   `GHOSTTY_SOURCE_DIR` or `GHOSTTY_ZIG_SYSTEM_DIR` removes the network requirement. If this
   step fails outright, the spike still has an answer; report it rather than fighting it.
4. **`probe-core`: the common parts.** The `TerminalBackend` trait, the snapshot types, and
   the pty. Choose the pty crate here and record why: `portable-pty` 0.9 (MIT) or
   `rustix-openpty` 0.2 (Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT). Not
   `alacritty_terminal::tty`, which would drag one candidate's crate into the other's
   measurement.
5. **R14 feature matrix (evidence A).** For each item in R14 feed the bytes and read the
   screen or the reported event back: truecolour, mouse reporting, bracketed paste, the
   alternate screen, the kitty keyboard protocol, OSC 8, OSC 52, OSC 7, OSC 133. For each,
   record one of: the library implements it; the library reports it and the app acts; the
   app must implement it entirely. Add two rows steg needs beyond R14: an unknown OSC
   (`OSC 7501`, for R49) reaching steg rather than being dropped, and reflow on resize.
   This is where the two differ most, so it carries as much weight in the answer as the
   timings.
6. **Throughput with no rendering (evidence A).** Feed the 100 MB fixture straight into each
   emulator with a 10,000-line scrollback (R15) and no GPUI at all: MB/s, wall time, peak
   RSS. Run each three times and report all three, so the spread is visible.
7. **The GPUI terminal view (evidence B).** Finish `probe-core/src/element.rs` and wire both
   binaries to it: one live `$SHELL` in a window, keys to the pty, output to the screen,
   truecolour and the cursor. Written from gpui's own examples and rustdoc. **Zed's
   `terminal` and `terminal_view` crates are not opened at all**, and the document says so
   (R59, `.claude/rules/licence.md`).
8. **Typing latency (evidence B, AC #2).** Release builds. A driver enqueues a keystroke on
   the main thread at a human rate (about 12/s), the pty's own echo returns it, and the
   element reports the frame in which that byte first appears. N >= 2000 per candidate, the
   two runs interleaved on an otherwise idle machine. Report p50, p95, p99 and max for two
   stop points: the moment the paint is submitted, and the next-frame callback
   (`Window::on_next_frame`), which includes the vsync wait. The display here reports
   120.00 Hz, so one frame is about 8.3 ms and a 16 ms budget allows roughly two: the
   document must say that, because it decides whether 16 ms is a sane target at all. Say
   plainly what the measurement excludes: the path from the physical key to GPUI's
   dispatch. Rejected alternative: posting real key events with CGEvent, which needs
   accessibility permission and measures the window server more than it measures steg.
9. **Two panes (evidence B, the second half of R19).** Two ptys and two terminal views in
   one window. Pane A prints the 100 MB file while pane B is typed into by the same driver.
   Report pane B's p95, the longest frame interval during the flood, and how long pane A
   takes to finish. R19's "or delay input to other panes" is half the requirement, so it is
   measured here rather than deferred.
10. **Architecture fit (evidence C).** Prove the ownership shape steg needs: the terminal
    lives on its own thread, bytes go in over a channel, snapshots come out, and the GPUI
    side never touches the emulator directly. libghostty-vt's handles are `!Send + !Sync` by
    design, so this is the step that shows what that costs: one thread per pane, and what
    it means for `.claude/rules/architecture.md` ("one task owns a piece of state and others
    send it messages") and for TASK-1.8. Check rather than assume that
    `alacritty_terminal::Term` has no such restriction.
11. **Licence inventory (AC #3).** `cargo tree -e normal --format "{p} {l}"` in each probe
    crate, pasted into the document, plus the pieces no crate graph shows: ghostty's own Zig
    source (fetched by `build.rs` at the pinned commit; read its licence file, not its
    README) and Zig itself as a build-time tool. Note for TASK-1.5 which licences its
    `deny.toml` must allow, and that `alacritty_terminal` is Apache-2.0 only while
    `libghostty-vt` is MIT OR Apache-2.0: both are fine in an MIT project, but Apache-2.0
    carries an attribution obligation when steg is distributed. `cargo-deny` is not
    installed on this machine and there is no root workspace yet, so `just deny` cannot be
    the proof here; `cargo tree` is, and the document says which was used.
12. **Write the answer.** `asdlc board doc create` with the title "Decision: the terminal
    emulation library for steg" and `--type other`, then `asdlc board doc update <id>
    --content` with: the choice and the reasons in order of weight; the three measurement
    tables; the R14 matrix; the architecture-fit findings; the licence inventory; a section
    confirming or replacing each R19 number with the number measured, and what to change in
    the spec if it is replaced; and the findings other tasks need (TASK-1.5's `deny.toml`
    and pinned versions, TASK-1.8's thread ownership, TASK-1.16's drawing, TASK-1.23's CI if
    the ghostty build needs Zig and the network). Link it with
    `asdlc board task edit TASK-1.1 --add-doc <id>`.
13. **Finish.** Commit (trailer `Asdlc-Worker: bruk.w1`), `git fetch origin`, merge or rebase
    `origin/main` as the finish rules say, re-run `asdlc record-check ... -- just check`,
    push the branch, open the pull request on `bruk-io/steg`, and `asdlc finish` with the
    numbers in the notes.

## Risks

- **The end-to-end latency cannot tell the candidates apart.** Likely: both go through the
  same GPUI shape-and-paint path, which probably dominates the per-keystroke budget, so the
  two numbers may land inside the noise. This is why evidence A exists and is measured
  first; the end-to-end number's job is to confirm or replace R19 (AC #2), not to pick the
  winner.
- **The libghostty build needs the network and Zig at build time.** `build.rs` clones
  ghostty at a pinned commit and builds it with Zig. That affects `just check` on every pull
  request (TASK-1.23), offline builds, and `cargo vendor`. Step 3 measures it and tests the
  `GHOSTTY_SOURCE_DIR` escape hatch. It is a real cost to weigh, not a disqualification.
- **`!Send + !Sync` handles.** If a terminal cannot leave its thread, the server needs a
  thread per pane and a channel. Step 10 proves whether that shape works before the choice
  is made, because TASK-1.8 is built on it.
- **The bindings are a one-maintainer third-party crate at 0.2.x**
  (`github.com/uzaaft/libghostty-rs`), wrapping a pre-1.0 C API whose own README says it
  does not promise compatibility across revisions. `alacritty_terminal` is 0.26, pure Rust,
  and has Alacritty and Zed as users. The document weighs this as maintenance risk, with the
  evidence for both.
- **GPUI 0.2.2 may not work standalone.** Step 2 finds out before any terminal code exists.
  Fallback is a pinned git dependency; both are licence-safe, since gpui is Apache-2.0.
- **Measuring the wrong thing.** The latency number is not photon-to-glass. The document
  states the two stop points, the excluded segment, and the 8.3 ms frame interval, so nobody
  later reads 16 ms as something it is not.
- **Colliding with TASK-1.5.** Mitigated by touching nothing outside
  `spikes/task-1.1-terminal-library/` and the board, and by the spike's own `[workspace]`
  table. If the merge in step 13 conflicts at all, something went wrong: stop and say so.
- **Licence contamination.** Zed's terminal crates solve exactly this problem and are GPL.
  They are not opened during this task, and the document records that along with every
  source the probe code was written from.
- **Disk and time.** A 100 MB fixture plus two debug and release builds of gpui and ghostty;
  the fixture goes under `$TMPDIR`, and `target/` is already ignored.

## Proof

- **AC #1** (a document with the choice, the measured p95 typing latency, and the 100 MB
  print result for each candidate): the document exists and is linked on the task; the
  numbers in it are the tables `probe-alacritty` and `probe-ghostty` print, with the steps
  that produced them (8, 6, 9) recorded as runs of
  `asdlc record-check TASK-1.1 --in <worktree> --as bruk.w1 -- <probe command>`.
- **AC #2** (confirms or replaces the R19 numbers): a section of the document takes R19's
  two sentences in turn, puts the measured p50/p95/p99/max beside the 16 ms proposal and the
  flood result beside "does not freeze the app or delay input to other panes", and states
  for each whether it stands, with the number that replaces it if it does not. Evidence is
  the recorded output of steps 8 and 9.
- **AC #3** (the licence of every dependency the chosen approach needs): the document
  contains the `cargo tree -e normal --format "{p} {l}"` output for the chosen probe, run
  through `asdlc record-check`, plus the named non-crate pieces (ghostty's Zig source at its
  pinned commit and its licence, Zig as a build tool) and the licence list TASK-1.5's
  `deny.toml` must allow.
- **The repository's own check**: `asdlc record-check TASK-1.1 --in <worktree> --as bruk.w1
  -- just check`, once before any change (step 1) and once before finishing (step 13). It is
  expected to skip every Rust step both times, because this task adds no workspace crate;
  that expectation is itself a check that the spike stayed out of TASK-1.5's way, and the
  notes say so.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Onboarding skipped by @bruk.o: bruk-io-steg has no repository plugin yet and no code to read layers or verify commands from. TASK-1.24 onboards it once the workspace exists. The repository's own rules are in .claude/rules/ and its commands are in the justfile.

Facts verified by @bruk.w1 while planning (2026-11-13), so the builder does not rediscover them:

- This machine: macOS 27.0.1, arm64, rustc/cargo 1.98.1, just 1.59.0, zig 0.15.2 on PATH. The display reports 120.00 Hz, so one frame is about 8.3 ms and R19's 16 ms allows roughly two frames.
- Versions and licences on crates.io: `gpui` 0.2.2 (Apache-2.0, repository zed-industries/zed, has its own `examples/`), `alacritty_terminal` 0.26.0 (Apache-2.0 only, rust-version 1.85, and it ships its own `tty` and `event_loop` modules), `libghostty-vt` 0.2.2 and `libghostty-vt-sys` 0.2.2 (MIT OR Apache-2.0, repository github.com/uzaaft/libghostty-rs, rust-version 1.90).
- `libghostty-vt-sys`'s `build.rs` clones github.com/ghostty-org/ghostty at pinned commit a887df42c56f6de86c0fe6da9c4eeca37931e083 and builds it with Zig; `GHOSTTY_SOURCE_DIR` and `GHOSTTY_ZIG_SYSTEM_DIR` are its escape hatches. That source is not a crate, so no crate graph and no `cargo deny` run will ever list its licence: the document names it by hand.
- `libghostty-vt`'s README: handle types (`Terminal`, `RenderState`, `KeyEncoder`) are `!Send + !Sync` by design and must be driven from one thread.
- `cargo deny` is not installed and there is no root workspace, so the licence inventory uses `cargo tree -e normal --format "{p} {l}"`, which was tried and works on cargo 1.98.1.
- gpui 0.2.2 has `WindowTextSystem::shape_line`, `ShapedLine::paint` / `paint_background`, the three-method `Element` trait (request_layout/prepaint/paint), `Window::on_next_frame`, `Window::request_animation_frame` and `Window::refresh`.
- Pty crates and licences: `portable-pty` 0.9.0 (MIT), `rustix-openpty` 0.2.0 (Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT).
- `just check` in this worktree currently skips every Rust step, because `justfile:12` gates them on a root `Cargo.toml` that this task must not create.
<!-- SECTION:NOTES:END -->
