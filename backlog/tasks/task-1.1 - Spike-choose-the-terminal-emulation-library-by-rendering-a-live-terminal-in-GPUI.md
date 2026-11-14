---
id: TASK-1.1
title: >-
  Spike: choose the terminal emulation library by rendering a live terminal in
  GPUI
status: In Progress
assignee:
  - '@bruk.w1'
created_date: '2026-11-13T22:52:00Z'
updated_date: '2026-11-14T05:19:00Z'
labels:
  - size-3
milestone: m-0
dependencies: []
documentation:
  - doc-6
parent_task_id: TASK-1
type: spike
project: bruk-io-steg
asdlc_accepted_from: DRAFT-2
asdlc_branch: task-1.1-spike-choose-the-terminal-emulation
asdlc_plan_approved_by: bruk
asdlc_plan_approved_hash: 'b1e716d2ff94'
asdlc_plan_approved_size: size-3
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

## How the choice is made, fixed before any measurement exists

Written here, before the numbers, so the answer is not a weighting chosen to fit the data.

1. **Gates.** A candidate is out, whatever its speed, if either fails: its licence and every
   licence under it cannot be cleared for an MIT project that will be distributed (R59), or
   it cannot be driven in the ownership shape step 10 builds (the terminal owned by one
   thread, bytes in on a channel, snapshots out) without `Arc<Mutex<_>>` shared across
   tasks, which `.claude/rules/rust.md` names a last resort.
2. **R19 is a threshold, not a score.** A candidate that clears R19's two sentences clears
   it; being faster than the other is worth nothing on its own. If both clear it, the
   timings do not decide.
3. **Then the work each one leaves to steg decides.** Every R14 item (and OSC 7501, and
   reflow on resize) the library drops on the app is costed in days of work and summed. The
   candidate that leaves steg less to write wins. The kitty keyboard protocol, OSC 133 and
   unknown-OSC passthrough are the heavy items.
4. **Tie-break, in this order.** Build and release cost (network and Zig at build time
   against pure Rust, and what that does to TASK-1.23); then maintenance risk (version,
   number of users, whether the API promises stability); then how much of the GPUI drawing
   code each one makes steg write.
5. **If neither clears R19**, the spike's answer is a replacement number with the reason,
   plus the cheaper candidate on rule 3 (AC #2 allows exactly this).

## Rules the probe binaries follow

`asdlc record-check` runs these with nobody present, and the reviewer re-runs them, so:

- Each binary takes its sample count and which measurement to run as arguments, runs it,
  prints the table to stdout, and then quits the app itself (`cx.quit()`). No measurement
  ever waits for a window to be closed by hand.
- Every measurement is deterministic apart from the timings: the same byte scripts, the same
  fixture, the same terminal size, the same sample count.
- The binary prints the machine, the display's refresh rate, the crate versions and the
  build profile above each table, so a number can never be read out of its context.
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

Plan checked against doc-1 by @bruk.o: ok. It serves R14, R19 and R59 and answers intent question 3; the decision rule is fixed before any measurement (licence and thread ownership are gates, R19 is a threshold, then the work each library leaves steg decides); throwaway code stays under spikes/ in its own workspace root, clear of TASK-1.5; nothing is taken from Zed's GPL terminal crates; its proof covers all three acceptance criteria. Put to Bruk with the plan: what acceptance criterion 1 means if the libghostty build cannot be made to work here, and that the plan grew (the R14 matrix, the two-pane flood, the architecture-fit step) while staying size-3.

Decided by Bruk on 2026-11-13, in answer to the worker's question on acceptance criterion 1: if the libghostty-vt candidate cannot be built on this machine after a real attempt, the spike may report that it could not be measured and why, with what was tried. A build that fragile is itself part of the answer. The criterion's wording is unchanged; read it with this. Recorded by @bruk.o.

Check passed: `just check` exited 0 at 2026-11-14T03:56:50+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.1 on task-1.1-spike-choose-the-terminal-emulation at f5ccc602052e:

```text
skip    format check: no Rust workspace yet
skip    lint: no Rust workspace yet
skip    licences: no Rust workspace yet
skip    tests: no Rust workspace yet
ok      tool tests (0.4s)
```

Check passed: `sh -c 'spikes/task-1.1-terminal-library/target/release/probe-alacritty conformance'` exited 0 at 2026-11-14T04:24:16+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.1 on task-1.1-spike-choose-the-terminal-emulation at f5ccc602052e with uncommitted changes:

```text
## alacritty_terminal (0.26.0, Apache-2.0, pure Rust)

- machine: macOS 27.0.1, Apple M3 Ultra, 28 cores
- display: UI Looks like: 3840 x 1080 @ 120.00Hz
- rustc: rustc 1.98.1 (48a229cea 2026-09-01)
- gpui: 0.2.2 (crates.io)
- build: release, opt-level 3, thin LTO

### R14 feature matrix

| feature | requirement | alacritty_terminal | detail |
|---|---|---|---|
| truecolour (24-bit SGR) | R14 | handled | fg #ff6400, bg #001e3c for SGR 38;2;255;100;0 on 48;2;0;30;60 |
| double-width characters | R14 | handled | cursor at column 9 (9 expected), row reads "日本語abc" |
| alternate screen (1049) | R14 | handled | alt screen shows "alternate", primary restored to "primary", flag reported: true |
| mouse reporting (1000/1006) | R14 | handled | after enabling: true |
| bracketed paste (2004) | R14 | handled | after enabling: true |
| kitty keyboard protocol | R14 | handled | after CSI > 1 u: true |
| OSC 8 hyperlinks | R14, R17 | handled | url on the first cell: https://example.com/steg |
| OSC 52 clipboard | R14 | handled | writes: ["steg"] |
| OSC 7 working directory | R14, R16 | not reported | pwd: not reported |
| OSC 133 prompt marks | R14, R18 | not reported | prompt marks seen: the library exposes nothing |
| unknown OSC reaches the app (OSC 7501) | R49 | not reported | payload handed over: dropped by the library |
| window title (OSC 0/2) | R16 | handled | title: steg title |
| reflow on resize | R8, R20 | handled | 100 x's wrapped at 80 then widened to 120: first row holds 100 of them |
| scrollback honours its limit | R15 | handled | a 50-line limit: 50 rows held after 200 lines, 50 after 5,000 - pruned to the line count asked for, as R15 is written |

14 checks: 11 handled, 0 done differently, 3 left to steg, 0 wrong
```

Check passed: `sh -c 'spikes/task-1.1-terminal-library/target/release/probe-ghostty conformance'` exited 0 at 2026-11-14T04:24:23+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.1 on task-1.1-spike-choose-the-terminal-emulation at f5ccc602052e with uncommitted changes:

```text
## ghostty-vt (0.8.0 bindings (Apache-2.0) over ghostty 1.4.0-main+83fb956 (MIT), built with zig 0.17.1)

- machine: macOS 27.0.1, Apple M3 Ultra, 28 cores
- display: UI Looks like: 3840 x 1080 @ 120.00Hz
- rustc: rustc 1.98.1 (48a229cea 2026-09-01)
- gpui: 0.2.2 (crates.io)
- build: release, opt-level 3, thin LTO

### R14 feature matrix

| feature | requirement | ghostty-vt | detail |
|---|---|---|---|
| truecolour (24-bit SGR) | R14 | handled | fg #ff6400, bg #001e3c for SGR 38;2;255;100;0 on 48;2;0;30;60 |
| double-width characters | R14 | handled | cursor at column 9 (9 expected), row reads "日本語abc" |
| alternate screen (1049) | R14 | handled | alt screen shows "alternate", primary restored to "primary", flag reported: true |
| mouse reporting (1000/1006) | R14 | handled | after enabling: true |
| bracketed paste (2004) | R14 | handled | after enabling: true |
| kitty keyboard protocol | R14 | handled | after CSI > 1 u: true |
| OSC 8 hyperlinks | R14, R17 | handled | url on the first cell: https://example.com/steg |
| OSC 52 clipboard | R14 | handled | writes: ["steg"] |
| OSC 7 working directory | R14, R16 | handled | pwd: file://localhost/private/tmp/steg |
| OSC 133 prompt marks | R14, R18 | handled | prompt marks seen: true |
| unknown OSC reaches the app (OSC 7501) | R49 | not reported | payload handed over: dropped by the library |
| window title (OSC 0/2) | R16 | handled | title: steg title |
| reflow on resize | R8, R20 | handled | 100 x's wrapped at 80 then widened to 120: first row holds 100 of them |
| scrollback honours its limit | R15 | differs | a 50-line limit: 191 rows held after 200 lines, 854 after 5,000 - bounded, but not by the line count: this library's limit is a byte estimate, so steg must convert R15's 10,000 lines into bytes |

14 checks: 12 handled, 1 done differently, 1 left to steg, 0 wrong
```

Check passed: `sh -c 'spikes/task-1.1-terminal-library/target/release/probe-alacritty flood 100 3'` exited 0 at 2026-11-14T04:25:17+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.1 on task-1.1-spike-choose-the-terminal-emulation at f5ccc602052e with uncommitted changes:

```text
## alacritty_terminal (0.26.0, Apache-2.0, pure Rust)

- machine: macOS 27.0.1, Apple M3 Ultra, 28 cores
- display: UI Looks like: 3840 x 1080 @ 120.00Hz
- rustc: rustc 1.98.1 (48a229cea 2026-09-01)
- gpui: 0.2.2 (crates.io)
- build: release, opt-level 3, thin LTO

### printing 100 MB with no renderer

fixture: /var/folders/rf/f34xmczn4659gm98hrsprydc0000gn/T/steg-spike-flood-104857600.txt

| run | seconds | MB/s | slowest 64 KB chunk ms | peak RSS MB | scrollback rows |
|---|---|---|---|---|---|
| run 1 | 0.5 | 214 | 0.58 | 40.5 | 10000 |
| run 2 | 0.5 | 213 | 0.47 | 40.6 | 10000 |
| run 3 | 0.5 | 211 | 0.38 | 40.6 | 10000 |

| one 64 KB chunk | samples | p50 ms | p95 ms | p99 ms | max ms |
|---|---|---|---|---|---|
| feed | 1600 | 0.29 | 0.32 | 0.34 | 0.38 |
| read the screen back | 200 | 0.01 | 0.01 | 0.02 | 0.02 |
```

Check passed: `sh -c 'spikes/task-1.1-terminal-library/target/release/probe-ghostty flood 100 3'` exited 0 at 2026-11-14T04:25:21+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.1 on task-1.1-spike-choose-the-terminal-emulation at f5ccc602052e with uncommitted changes:

```text
## ghostty-vt (0.8.0 bindings (Apache-2.0) over ghostty 1.4.0-main+83fb956 (MIT), built with zig 0.17.1)

- machine: macOS 27.0.1, Apple M3 Ultra, 28 cores
- display: UI Looks like: 3840 x 1080 @ 120.00Hz
- rustc: rustc 1.98.1 (48a229cea 2026-09-01)
- gpui: 0.2.2 (crates.io)
- build: release, opt-level 3, thin LTO

### printing 100 MB with no renderer

fixture: /var/folders/rf/f34xmczn4659gm98hrsprydc0000gn/T/steg-spike-flood-104857600.txt

| run | seconds | MB/s | slowest 64 KB chunk ms | peak RSS MB | scrollback rows |
|---|---|---|---|---|---|
| run 1 | 0.1 | 1036 | 0.11 | 20.3 | 10995 |
| run 2 | 0.1 | 982 | 0.11 | 20.5 | 10995 |
| run 3 | 0.1 | 1041 | 0.12 | 20.5 | 10995 |

| one 64 KB chunk | samples | p50 ms | p95 ms | p99 ms | max ms |
|---|---|---|---|---|---|
| feed | 1600 | 0.06 | 0.06 | 0.07 | 0.12 |
| read the screen back | 200 | 0.09 | 0.10 | 0.11 | 0.12 |
```

Check passed: `sh -c 'spikes/task-1.1-terminal-library/target/release/probe-alacritty latency 2000'` exited 0 at 2026-11-14T04:59:45+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.1 on task-1.1-spike-choose-the-terminal-emulation at f5ccc602052e with uncommitted changes:

```text
## alacritty_terminal (0.26.0, Apache-2.0, pure Rust)

- machine: macOS 27.0.1, Apple M3 Ultra, 28 cores
- display: UI Looks like: 3840 x 1080 @ 120.00Hz
- rustc: rustc 1.98.1 (48a229cea 2026-09-01)
- gpui: 0.2.2 (crates.io)
- build: release, opt-level 3, thin LTO


### the run, block by block

| samples | wall s | paint p50 ms | paint p95 ms | frame p50 ms | frame p95 ms | dropped so far |
|---|---|---|---|---|---|---|
| 0..248 | 22.3 | 2.65 | 10.41 | 10.55 | 18.46 | 2 |
| 248..496 | 22.3 | 2.67 | 10.53 | 10.63 | 18.62 | 4 |
| 496..744 | 22.6 | 2.92 | 10.73 | 10.79 | 18.52 | 6 |
| 744..992 | 22.9 | 9.29 | 10.91 | 17.29 | 18.57 | 8 |
| 992..1016 | 133.5 | 3.24 | 10.78 | 11.12 | 18.44 | 234 |
| 1016..1016 | 145.3 | - | - | - | - | 484 |
| 1016..1016 | 145.4 | - | - | - | - | 734 |
| 1016..1016 | 145.3 | - | - | - | - | 984 |

### typing into one pane

| stop point | samples | p50 ms | p95 ms | p99 ms | max ms |
|---|---|---|---|---|---|
| key to paint | 1016 | 2.84 | 10.77 | 11.01 | 11.88 |
| key to next frame | 1016 | 10.71 | 18.54 | 18.69 | 18.80 |
| interval between frames | 2000 | 92.12 | 583.80 | 584.20 | 591.30 |

- dropped samples (never drawn within the deadline): 984
- within R19's 16 ms, to the next frame: 671 of 1016 (66.0%)
```

Check passed: `sh -c 'spikes/task-1.1-terminal-library/target/release/probe-alacritty latency 2000'` exited 0 at 2026-11-14T05:03:52+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.1 on task-1.1-spike-choose-the-terminal-emulation at f5ccc602052e with uncommitted changes:

```text
## alacritty_terminal (0.26.0, Apache-2.0, pure Rust)

- machine: macOS 27.0.1, Apple M3 Ultra, 28 cores
- display: UI Looks like: 3840 x 1080 @ 120.00Hz
- rustc: rustc 1.98.1 (48a229cea 2026-09-01)
- gpui: 0.2.2 (crates.io)
- build: release, opt-level 3, thin LTO


### the run, block by block

| samples | wall s | paint p50 ms | paint p95 ms | frame p50 ms | frame p95 ms | dropped so far |
|---|---|---|---|---|---|---|
| 0..250 | 21.3 | 2.54 | 10.34 | 10.52 | 18.34 | 0 |
| 250..500 | 21.5 | 2.60 | 10.48 | 10.53 | 18.17 | 0 |
| 500..750 | 21.7 | 2.40 | 10.59 | 10.24 | 18.09 | 0 |
| 750..1000 | 21.8 | 2.72 | 10.32 | 10.66 | 18.33 | 0 |
| 1000..1250 | 22.1 | 3.23 | 10.77 | 10.80 | 18.39 | 0 |
| 1250..1500 | 21.7 | 2.64 | 10.57 | 10.53 | 18.38 | 0 |
| 1500..1750 | 22.0 | 2.95 | 10.70 | 10.80 | 18.46 | 0 |
| 1750..2000 | 21.7 | 2.64 | 10.57 | 10.53 | 18.44 | 0 |

### typing into one pane

| stop point | samples | p50 ms | p95 ms | p99 ms | max ms |
|---|---|---|---|---|---|
| key to paint | 2000 | 2.67 | 10.57 | 10.92 | 11.29 |
| key to next frame | 2000 | 10.54 | 18.38 | 18.65 | 18.82 |
| interval between frames | 2050 | 83.57 | 91.92 | 116.78 | 510.88 |

- dropped samples (never drawn within the deadline): 0
- within R19's 16 ms, to the next frame: 1403 of 2000 (70.2%)
```

Check passed: `sh -c 'spikes/task-1.1-terminal-library/target/release/probe-ghostty latency 2000'` exited 0 at 2026-11-14T05:06:52+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.1 on task-1.1-spike-choose-the-terminal-emulation at f5ccc602052e with uncommitted changes:

```text
## ghostty-vt (0.8.0 bindings (Apache-2.0) over ghostty 1.4.0-main+83fb956 (MIT), built with zig 0.17.1)

- machine: macOS 27.0.1, Apple M3 Ultra, 28 cores
- display: UI Looks like: 3840 x 1080 @ 120.00Hz
- rustc: rustc 1.98.1 (48a229cea 2026-09-01)
- gpui: 0.2.2 (crates.io)
- build: release, opt-level 3, thin LTO


### the run, block by block

| samples | wall s | paint p50 ms | paint p95 ms | frame p50 ms | frame p95 ms | dropped so far |
|---|---|---|---|---|---|---|
| 0..250 | 21.3 | 2.23 | 9.87 | 10.01 | 17.73 | 0 |
| 250..500 | 21.3 | 2.07 | 9.73 | 9.65 | 17.10 | 0 |
| 500..750 | 21.6 | 2.24 | 9.94 | 9.91 | 17.46 | 0 |
| 750..1000 | 21.7 | 2.01 | 9.88 | 9.34 | 17.28 | 0 |
| 1000..1250 | 21.6 | 1.97 | 9.95 | 9.44 | 17.17 | 0 |
| 1250..1500 | 21.5 | 2.19 | 10.01 | 9.89 | 17.51 | 0 |
| 1500..1750 | 21.5 | 2.03 | 9.92 | 9.39 | 17.27 | 0 |
| 1750..2000 | 21.6 | 2.11 | 10.02 | 9.83 | 17.37 | 0 |

### typing into one pane

| stop point | samples | p50 ms | p95 ms | p99 ms | max ms |
|---|---|---|---|---|---|
| key to paint | 2000 | 2.12 | 9.91 | 10.47 | 12.51 |
| key to next frame | 2000 | 9.67 | 17.33 | 18.29 | 19.23 |
| interval between frames | 2050 | 83.46 | 91.90 | 116.84 | 514.31 |

- dropped samples (never drawn within the deadline): 0
- within R19's 16 ms, to the next frame: 1615 of 2000 (80.8%)
```

Check passed: `sh -c 'spikes/task-1.1-terminal-library/target/release/probe-alacritty flood-ui 100 300'` exited 0 at 2026-11-14T05:07:33+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.1 on task-1.1-spike-choose-the-terminal-emulation at f5ccc602052e with uncommitted changes:

```text
## alacritty_terminal (0.26.0, Apache-2.0, pure Rust)

- machine: macOS 27.0.1, Apple M3 Ultra, 28 cores
- display: UI Looks like: 3840 x 1080 @ 120.00Hz
- rustc: rustc 1.98.1 (48a229cea 2026-09-01)
- gpui: 0.2.2 (crates.io)
- build: release, opt-level 3, thin LTO

### typing in one pane while the other prints 100 MB

fixture: /var/folders/rf/f34xmczn4659gm98hrsprydc0000gn/T/steg-spike-flood-104857600.txt


### the run, block by block

| samples | wall s | paint p50 ms | paint p95 ms | frame p50 ms | frame p95 ms | dropped so far |
|---|---|---|---|---|---|---|
| 0..250 | 22.3 | 9.31 | 10.85 | 16.60 | 17.95 | 0 |
| 250..300 | 4.5 | 9.64 | 10.84 | 16.92 | 17.89 | 0 |

### typing while the other pane floods

| stop point | samples | p50 ms | p95 ms | p99 ms | max ms |
|---|---|---|---|---|---|
| key to paint | 300 | 9.40 | 10.85 | 11.39 | 11.62 |
| key to next frame | 299 | 16.64 | 17.95 | 18.60 | 18.74 |
| interval between frames | 311 | 91.06 | 92.40 | 130.75 | 227.31 |

- dropped samples (never drawn within the deadline): 0
- within R19's 16 ms, to the next frame: 125 of 299 (41.8%)

- the typing run took 26.8s; the flooding pane had finished by then
```

Check passed: `sh -c 'spikes/task-1.1-terminal-library/target/release/probe-alacritty flood-ui 100 300'` exited 0 at 2026-11-14T05:08:41+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.1 on task-1.1-spike-choose-the-terminal-emulation at f5ccc602052e with uncommitted changes:

```text
## alacritty_terminal (0.26.0, Apache-2.0, pure Rust)

- machine: macOS 27.0.1, Apple M3 Ultra, 28 cores
- display: UI Looks like: 3840 x 1080 @ 120.00Hz
- rustc: rustc 1.98.1 (48a229cea 2026-09-01)
- gpui: 0.2.2 (crates.io)
- build: release, opt-level 3, thin LTO

### typing in one pane while the other prints 100 MB

fixture: /var/folders/rf/f34xmczn4659gm98hrsprydc0000gn/T/steg-spike-flood-104857600.txt


### the run, block by block

| samples | wall s | paint p50 ms | paint p95 ms | frame p50 ms | frame p95 ms | dropped so far |
|---|---|---|---|---|---|---|
| 0..250 | 22.3 | 7.79 | 10.86 | 16.03 | 18.40 | 0 |
| 250..300 | 4.5 | 9.60 | 10.76 | 16.65 | 18.38 | 0 |

### typing while the other pane floods

| stop point | samples | p50 ms | p95 ms | p99 ms | max ms |
|---|---|---|---|---|---|
| key to paint | 300 | 9.01 | 10.78 | 11.47 | 57.20 |
| key to next frame | 299 | 16.40 | 18.40 | 18.79 | 64.37 |
| interval between frames | 313 | 90.65 | 92.45 | 117.37 | 186.76 |

- dropped samples (never drawn within the deadline): 0
- within R19's 16 ms, to the next frame: 140 of 299 (46.8%)

- the typing run took 26.7s; the flooding pane had finished by then
- printing the file in the pane took 0.08s
```

Check passed: `sh -c 'spikes/task-1.1-terminal-library/target/release/probe-alacritty flood-ui 100 300'` exited 0 at 2026-11-14T05:10:16+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.1 on task-1.1-spike-choose-the-terminal-emulation at f5ccc602052e with uncommitted changes:

```text
## alacritty_terminal (0.26.0, Apache-2.0, pure Rust)

- machine: macOS 27.0.1, Apple M3 Ultra, 28 cores
- display: UI Looks like: 3840 x 1080 @ 120.00Hz
- rustc: rustc 1.98.1 (48a229cea 2026-09-01)
- gpui: 0.2.2 (crates.io)
- build: release, opt-level 3, thin LTO

### typing in one pane while the other prints 100 MB

fixture: /var/folders/rf/f34xmczn4659gm98hrsprydc0000gn/T/steg-spike-flood-104857600.txt


### the run, block by block

| samples | wall s | paint p50 ms | paint p95 ms | frame p50 ms | frame p95 ms | dropped so far |
|---|---|---|---|---|---|---|
| 0..250 | 22.3 | 9.31 | 10.88 | 16.71 | 18.11 | 0 |
| 250..300 | 4.4 | 2.86 | 10.41 | 10.67 | 17.37 | 0 |

### typing while the other pane floods

| stop point | samples | p50 ms | p95 ms | p99 ms | max ms |
|---|---|---|---|---|---|
| key to paint | 300 | 9.16 | 10.81 | 11.42 | 11.86 |
| key to next frame | 299 | 16.45 | 18.03 | 18.63 | 18.68 |
| interval between frames | 311 | 90.81 | 92.45 | 125.76 | 244.03 |

- dropped samples (never drawn within the deadline): 0
- within R19's 16 ms, to the next frame: 143 of 299 (47.8%)

- the typing run took 26.7s; the flooding pane had finished by then
- the pane fed the whole file to its emulator in 0.05s (1962 MB/s through the UI)
- the flooding pane: 5 pumps, 101.4 MB fed, largest pump 24404 KB, slowest pump 148.5 ms
```

Check passed: `sh -c 'spikes/task-1.1-terminal-library/target/release/probe-ghostty flood-ui 100 300'` exited 0 at 2026-11-14T05:10:58+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.1 on task-1.1-spike-choose-the-terminal-emulation at f5ccc602052e with uncommitted changes:

```text
## ghostty-vt (0.8.0 bindings (Apache-2.0) over ghostty 1.4.0-main+83fb956 (MIT), built with zig 0.17.1)

- machine: macOS 27.0.1, Apple M3 Ultra, 28 cores
- display: UI Looks like: 3840 x 1080 @ 120.00Hz
- rustc: rustc 1.98.1 (48a229cea 2026-09-01)
- gpui: 0.2.2 (crates.io)
- build: release, opt-level 3, thin LTO

### typing in one pane while the other prints 100 MB

fixture: /var/folders/rf/f34xmczn4659gm98hrsprydc0000gn/T/steg-spike-flood-104857600.txt


### the run, block by block

| samples | wall s | paint p50 ms | paint p95 ms | frame p50 ms | frame p95 ms | dropped so far |
|---|---|---|---|---|---|---|
| 0..250 | 22.5 | 9.38 | 10.43 | 15.82 | 16.76 | 0 |
| 250..300 | 4.5 | 9.43 | 10.38 | 15.81 | 16.52 | 0 |

### typing while the other pane floods

| stop point | samples | p50 ms | p95 ms | p99 ms | max ms |
|---|---|---|---|---|---|
| key to paint | 300 | 9.39 | 10.40 | 10.93 | 11.41 |
| key to next frame | 299 | 15.81 | 16.76 | 17.39 | 18.09 |
| interval between frames | 362 | 90.98 | 92.25 | 117.43 | 124.84 |

- dropped samples (never drawn within the deadline): 0
- within R19's 16 ms, to the next frame: 177 of 299 (59.2%)

- the typing run took 27.1s; the flooding pane had finished by then
- the pane fed the whole file to its emulator in 0.10s (986 MB/s through the UI)
- the flooding pane: 287 pumps, 101.4 MB fed, largest pump 14512 KB, slowest pump 18.2 ms
```

Check passed: `sh -c 'spikes/task-1.1-terminal-library/target/release/probe-alacritty ownership && spikes/task-1.1-terminal-library/target/release/probe-ghostty ownership'` exited 0 at 2026-11-14T05:11:09+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.1 on task-1.1-spike-choose-the-terminal-emulation at f5ccc602052e with uncommitted changes:

```text
## alacritty_terminal (0.26.0, Apache-2.0, pure Rust)

- machine: macOS 27.0.1, Apple M3 Ultra, 28 cores
- display: UI Looks like: 3840 x 1080 @ 120.00Hz
- rustc: rustc 1.98.1 (48a229cea 2026-09-01)
- gpui: 0.2.2 (crates.io)
- build: release, opt-level 3, thin LTO

### one terminal, one thread, driven over a channel

- the first row read back from the thread: Some("owned by its own thread")
- bytes fed over the channel: 25
- so the shape works for alacritty_terminal: the emulator is created on the thread that owns it, and callers send messages (no Arc<Mutex<_>> across tasks)
## ghostty-vt (0.8.0 bindings (Apache-2.0) over ghostty 1.4.0-main+83fb956 (MIT), built with zig 0.17.1)

- machine: macOS 27.0.1, Apple M3 Ultra, 28 cores
- display: UI Looks like: 3840 x 1080 @ 120.00Hz
- rustc: rustc 1.98.1 (48a229cea 2026-09-01)
- gpui: 0.2.2 (crates.io)
- build: release, opt-level 3, thin LTO

### one terminal, one thread, driven over a channel

- the first row read back from the thread: Some("owned by its own thread")
- bytes fed over the channel: 25
- so the shape works for ghostty-vt: the emulator is created on the thread that owns it, and callers send messages (no Arc<Mutex<_>> across tasks)
```

Check passed: `sh -c 'spikes/task-1.1-terminal-library/target/release/probe-ghostty window 6'` exited 0 at 2026-11-14T05:11:48+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.1 on task-1.1-spike-choose-the-terminal-emulation at f5ccc602052e with uncommitted changes:

```text
## ghostty-vt (0.8.0 bindings (Apache-2.0) over ghostty 1.4.0-main+83fb956 (MIT), built with zig 0.17.1)

- machine: macOS 27.0.1, Apple M3 Ultra, 28 cores
- display: UI Looks like: 3840 x 1080 @ 120.00Hz
- rustc: rustc 1.98.1 (48a229cea 2026-09-01)
- gpui: 0.2.2 (crates.io)
- build: release, opt-level 3, thin LTO

### a live shell, closing itself after 6s

| ➜  ~
```

Check passed: `sh -c 'spikes/task-1.1-terminal-library/target/release/probe-alacritty program /usr/bin/vim && spikes/task-1.1-terminal-library/target/release/probe-ghostty program /usr/bin/vim'` exited 0 at 2026-11-14T05:12:57+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.1 on task-1.1-spike-choose-the-terminal-emulation at f5ccc602052e with uncommitted changes, last 40 of 92 lines:

```text
- build: release, opt-level 3, thin LTO

### /usr/bin/vim on a pseudo-terminal

4437 bytes of output, 120x32 screen

| 
| ~
| ~
| ~
| ~
| ~
| ~
| ~
| ~
| ~
| ~                                                  VIM - Vi IMproved
| ~
| ~                                                   version 9.1.1752
| ~                                               by Bram Moolenaar et al.
| ~                                     Vim is open source and freely distributable
| ~
| ~                                            Help poor children in Uganda!
| ~                                    type  :help iccf<Enter>       for information
| ~
| ~                                    type  :q<Enter>               to exit
| ~                                    type  :help<Enter>  or  <F1>  for on-line help
| ~                                    type  :help version9<Enter>   for version info
| ~
| ~
| ~
| ~
| ~
| ~
| ~
| ~
| ~
| 

cursor at column 0, row 0
```

Check passed: `sh -c 'spikes/task-1.1-terminal-library/target/release/probe-alacritty program /opt/homebrew/bin/btop && spikes/task-1.1-terminal-library/target/release/probe-ghostty program /opt/homebrew/bin/tmux'` exited 0 at 2026-11-14T05:13:19+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.1 on task-1.1-spike-choose-the-terminal-emulation at f5ccc602052e with uncommitted changes, last 40 of 92 lines:

```text
- build: release, opt-level 3, thin LTO

### /opt/homebrew/bin/tmux on a pseudo-terminal

0 bytes of output, 120x32 screen

| 
| 
| 
| 
| 
| 
| 
| 
| 
| 
| 
| 
| 
| 
| 
| 
| 
| 
| 
| 
| 
| 
| 
| 
| 
| 
| 
| 
| 
| 
| 
| 

cursor at column 0, row 0
```

Check passed: `sh -c 'cargo tree --manifest-path spikes/task-1.1-terminal-library/Cargo.toml -p probe-alacritty -e normal --format "{p} {l}"'` exited 0 at 2026-11-14T05:13:42+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.1 on task-1.1-spike-choose-the-terminal-emulation at f5ccc602052e with uncommitted changes, last 40 of 1137 lines:

```text
    │   ├── usvg v0.45.1 Apache-2.0 OR MIT (*)
    │   ├── uuid v1.29.2 Apache-2.0 OR MIT
    │   │   ├── getrandom v0.4.3 MIT OR Apache-2.0 (*)
    │   │   ├── serde_core v1.0.229 MIT OR Apache-2.0
    │   │   └── sha1_smol v1.0.1 BSD-3-Clause
    │   ├── waker-fn v1.2.0 Apache-2.0 OR MIT
    │   └── zed-font-kit v0.14.1-zed MIT OR Apache-2.0
    │       ├── bitflags v2.13.2 MIT OR Apache-2.0 (*)
    │       ├── byteorder v1.5.0 Unlicense OR MIT
    │       ├── core-foundation v0.10.0 MIT OR Apache-2.0 (*)
    │       ├── core-graphics v0.24.0 MIT OR Apache-2.0 (*)
    │       ├── core-text v21.0.0 MIT OR Apache-2.0 (*)
    │       ├── dirs v5.0.1 MIT OR Apache-2.0
    │       │   └── dirs-sys v0.4.1 MIT OR Apache-2.0
    │       │       ├── libc v0.2.191 MIT OR Apache-2.0
    │       │       └── option-ext v0.2.0 MPL-2.0
    │       ├── float-ord v0.3.2 MIT / Apache-2.0
    │       ├── lazy_static v1.5.1 MIT OR Apache-2.0
    │       ├── libc v0.2.191 MIT OR Apache-2.0
    │       ├── log v0.4.35 MIT OR Apache-2.0 (*)
    │       ├── pathfinder_geometry v0.5.1 MIT/Apache-2.0 (*)
    │       ├── pathfinder_simd v0.5.6 MIT OR Apache-2.0
    │       └── walkdir v2.5.0 Unlicense/MIT (*)
    ├── libc v0.2.191 MIT OR Apache-2.0
    └── portable-pty v0.9.0 MIT
        ├── anyhow v1.0.104 MIT OR Apache-2.0
        ├── downcast-rs v1.2.1 MIT/Apache-2.0
        ├── filedescriptor v0.8.3 MIT
        │   ├── libc v0.2.191 MIT OR Apache-2.0
        │   └── thiserror v1.0.69 MIT OR Apache-2.0 (*)
        ├── libc v0.2.191 MIT OR Apache-2.0
        ├── log v0.4.35 MIT OR Apache-2.0 (*)
        ├── nix v0.28.0 MIT
        │   ├── bitflags v2.13.2 MIT OR Apache-2.0 (*)
        │   ├── cfg-if v1.0.5 MIT OR Apache-2.0
        │   └── libc v0.2.191 MIT OR Apache-2.0
        ├── serial2 v0.2.39 BSD-2-Clause OR Apache-2.0
        │   ├── cfg-if v1.0.5 MIT OR Apache-2.0
        │   └── libc v0.2.191 MIT OR Apache-2.0
        └── shell-words v1.1.1 MIT/Apache-2.0
```

Check passed: `sh -c 'cargo tree --manifest-path spikes/task-1.1-terminal-library/Cargo.toml -p probe-ghostty -e normal --format "{p} {l}"'` exited 0 at 2026-11-14T05:13:45+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.1 on task-1.1-spike-choose-the-terminal-emulation at f5ccc602052e with uncommitted changes, last 40 of 1125 lines:

```text
    │   ├── usvg v0.45.1 Apache-2.0 OR MIT (*)
    │   ├── uuid v1.29.2 Apache-2.0 OR MIT
    │   │   ├── getrandom v0.4.3 MIT OR Apache-2.0 (*)
    │   │   ├── serde_core v1.0.229 MIT OR Apache-2.0
    │   │   └── sha1_smol v1.0.1 BSD-3-Clause
    │   ├── waker-fn v1.2.0 Apache-2.0 OR MIT
    │   └── zed-font-kit v0.14.1-zed MIT OR Apache-2.0
    │       ├── bitflags v2.13.2 MIT OR Apache-2.0
    │       ├── byteorder v1.5.0 Unlicense OR MIT
    │       ├── core-foundation v0.10.0 MIT OR Apache-2.0 (*)
    │       ├── core-graphics v0.24.0 MIT OR Apache-2.0 (*)
    │       ├── core-text v21.0.0 MIT OR Apache-2.0 (*)
    │       ├── dirs v5.0.1 MIT OR Apache-2.0
    │       │   └── dirs-sys v0.4.1 MIT OR Apache-2.0
    │       │       ├── libc v0.2.191 MIT OR Apache-2.0
    │       │       └── option-ext v0.2.0 MPL-2.0
    │       ├── float-ord v0.3.2 MIT / Apache-2.0
    │       ├── lazy_static v1.5.1 MIT OR Apache-2.0
    │       ├── libc v0.2.191 MIT OR Apache-2.0
    │       ├── log v0.4.35 MIT OR Apache-2.0 (*)
    │       ├── pathfinder_geometry v0.5.1 MIT/Apache-2.0 (*)
    │       ├── pathfinder_simd v0.5.6 MIT OR Apache-2.0
    │       └── walkdir v2.5.0 Unlicense/MIT (*)
    ├── libc v0.2.191 MIT OR Apache-2.0
    └── portable-pty v0.9.0 MIT
        ├── anyhow v1.0.104 MIT OR Apache-2.0
        ├── downcast-rs v1.2.1 MIT/Apache-2.0
        ├── filedescriptor v0.8.3 MIT
        │   ├── libc v0.2.191 MIT OR Apache-2.0
        │   └── thiserror v1.0.69 MIT OR Apache-2.0 (*)
        ├── libc v0.2.191 MIT OR Apache-2.0
        ├── log v0.4.35 MIT OR Apache-2.0 (*)
        ├── nix v0.28.0 MIT
        │   ├── bitflags v2.13.2 MIT OR Apache-2.0
        │   ├── cfg-if v1.0.5 MIT OR Apache-2.0
        │   └── libc v0.2.191 MIT OR Apache-2.0
        ├── serial2 v0.2.39 BSD-2-Clause OR Apache-2.0
        │   ├── cfg-if v1.0.5 MIT OR Apache-2.0
        │   └── libc v0.2.191 MIT OR Apache-2.0
        └── shell-words v1.1.1 MIT/Apache-2.0
```

Check passed: `sh -c 'spikes/task-1.1-terminal-library/target/release/probe-ghostty flood 100 3'` exited 0 at 2026-11-14T05:19:00+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.1 on task-1.1-spike-choose-the-terminal-emulation at f5ccc602052e with uncommitted changes:

```text
## ghostty-vt (0.8.0 bindings (Apache-2.0) over ghostty 1.4.0-main+83fb956 (MIT), built with zig 0.17.1)

- machine: macOS 27.0.1, Apple M3 Ultra, 28 cores
- display: UI Looks like: 3840 x 1080 @ 120.00Hz
- rustc: rustc 1.98.1 (48a229cea 2026-09-01)
- gpui: 0.2.2 (crates.io)
- build: release, opt-level 3, thin LTO

### printing 100 MB with no renderer

fixture: /var/folders/rf/f34xmczn4659gm98hrsprydc0000gn/T/steg-spike-flood-104857600.txt

| run | seconds | MB/s | slowest 64 KB chunk ms | peak RSS MB | scrollback rows |
|---|---|---|---|---|---|
| run 1 | 0.1 | 1041 | 0.12 | 20.3 | 10995 |
| run 2 | 0.1 | 1050 | 0.11 | 20.6 | 10995 |
| run 3 | 0.1 | 1049 | 0.10 | 20.6 | 10995 |

| one 64 KB chunk | samples | p50 ms | p95 ms | p99 ms | max ms |
|---|---|---|---|---|---|
| feed | 1600 | 0.06 | 0.06 | 0.06 | 0.10 |
| read the screen back | 200 | 0.10 | 0.10 | 0.10 | 0.11 |
```

Check passed: `sh -c 'spikes/task-1.1-terminal-library/target/release/probe-ghostty conformance'` exited 0 at 2026-11-14T05:19:08+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.1 on task-1.1-spike-choose-the-terminal-emulation at f5ccc602052e with uncommitted changes:

```text
## ghostty-vt (0.8.0 bindings (Apache-2.0) over ghostty 1.4.0-main+83fb956 (MIT), built with zig 0.17.1)

- machine: macOS 27.0.1, Apple M3 Ultra, 28 cores
- display: UI Looks like: 3840 x 1080 @ 120.00Hz
- rustc: rustc 1.98.1 (48a229cea 2026-09-01)
- gpui: 0.2.2 (crates.io)
- build: release, opt-level 3, thin LTO

### R14 feature matrix

| feature | requirement | ghostty-vt | detail |
|---|---|---|---|
| truecolour (24-bit SGR) | R14 | handled | fg #ff6400, bg #001e3c for SGR 38;2;255;100;0 on 48;2;0;30;60 |
| double-width characters | R14 | handled | cursor at column 9 (9 expected), row reads "日本語abc" |
| alternate screen (1049) | R14 | handled | alt screen shows "alternate", primary restored to "primary", flag reported: true |
| mouse reporting (1000/1006) | R14 | handled | after enabling: true |
| bracketed paste (2004) | R14 | handled | after enabling: true |
| kitty keyboard protocol | R14 | handled | after CSI > 1 u: true |
| OSC 8 hyperlinks | R14, R17 | handled | url on the first cell: https://example.com/steg |
| OSC 52 clipboard | R14 | handled | writes: ["steg"] |
| OSC 7 working directory | R14, R16 | handled | pwd: file://localhost/private/tmp/steg |
| OSC 133 prompt marks | R14, R18 | handled | prompt marks seen: true |
| unknown OSC reaches the app (OSC 7501) | R49 | not reported | payload handed over: dropped by the library |
| window title (OSC 0/2) | R16 | handled | title: steg title |
| reflow on resize | R8, R20 | handled | 100 x's wrapped at 80 then widened to 120: first row holds 100 of them |
| scrollback honours its limit | R15 | differs | a 50-line limit: 191 rows held after 200 lines, 854 after 5,000 - bounded, but not by the line count: this library's limit is a byte estimate, so steg must convert R15's 10,000 lines into bytes |

14 checks: 12 handled, 1 done differently, 1 left to steg, 0 wrong
```

Check passed: `sh -c 'spikes/task-1.1-terminal-library/target/release/probe-alacritty flood 100 3'` exited 0 at 2026-11-14T05:19:13+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.1 on task-1.1-spike-choose-the-terminal-emulation at f5ccc602052e with uncommitted changes:

```text
## alacritty_terminal (0.26.0, Apache-2.0, pure Rust)

- machine: macOS 27.0.1, Apple M3 Ultra, 28 cores
- display: UI Looks like: 3840 x 1080 @ 120.00Hz
- rustc: rustc 1.98.1 (48a229cea 2026-09-01)
- gpui: 0.2.2 (crates.io)
- build: release, opt-level 3, thin LTO

### printing 100 MB with no renderer

fixture: /var/folders/rf/f34xmczn4659gm98hrsprydc0000gn/T/steg-spike-flood-104857600.txt

| run | seconds | MB/s | slowest 64 KB chunk ms | peak RSS MB | scrollback rows |
|---|---|---|---|---|---|
| run 1 | 0.5 | 218 | 0.51 | 40.5 | 10000 |
| run 2 | 0.5 | 220 | 0.48 | 40.6 | 10000 |
| run 3 | 0.5 | 220 | 0.39 | 40.6 | 10000 |

| one 64 KB chunk | samples | p50 ms | p95 ms | p99 ms | max ms |
|---|---|---|---|---|---|
| feed | 1600 | 0.28 | 0.29 | 0.32 | 0.39 |
| read the screen back | 200 | 0.01 | 0.01 | 0.01 | 0.01 |
```
<!-- SECTION:NOTES:END -->

## Comments

<!-- COMMENTS:BEGIN -->
author: @bruk
created: 2026-11-14T03:56:00Z

---

Plan approved by @bruk (b1e716d2ff94, size-3): its worker may build it.

---
<!-- COMMENTS:END -->
