---
id: doc-6
title: 'Decision: the terminal emulation library for steg'
type: other
created_date: '2026-11-14T05:14:00Z'
updated_date: '2026-11-14T05:19:00Z'
---
- Task: TASK-1.1, the spike that answers intent question 3 for TASK-1.
- Spec: `doc-1`. Serves R14, R19 and R59.
- Measured by @bruk.w1 on 2026-11-13 and 2026-11-14. Every number below is the output of a
  command recorded on TASK-1.1 with `asdlc record-check`, on one machine: macOS 27.0.1, Apple
  M3 Ultra, 28 cores, a 3840x1080 display at **120.00 Hz**, rustc 1.98.1, gpui 0.2.2 from
  crates.io, release builds at opt-level 3 with thin LTO.
- The code that produced them is kept at `spikes/task-1.1-terminal-library/`, with its own
  `README.md` saying what each number includes. It is a measuring instrument, not production
  code, and it is not part of the Cargo workspace TASK-1.5 creates.

## 1. The answer

**Use ghostty's VT engine.** It leaves steg materially less to write, it is five times faster
at parsing, it uses half the memory, and its worst stall under a 100 MB print is eight times
shorter. The price is the build: a Zig toolchain, a source fetch, and a version constraint that
broke on this very machine while this spike was being run (section 8).

The decision rule was fixed on the task **before** any measurement existed, so the weighting
could not be chosen to fit the data. Applied to the evidence:

| rule | what it says | outcome |
|---|---|---|
| 1. Gates | licence must be clearable; the terminal must work owned by one thread | **both pass** (sections 6, 7) |
| 2. R19 is a threshold | clearing it is worth everything, being faster is worth nothing | **neither clears it as written** (section 5) |
| 3. Work left to steg decides | count what each library drops on the app | **ghostty**: 1 item against 3, plus the whole colour palette (section 4) |
| 4. Tie-breaks | build cost, then maintenance risk, then drawing code | **alacritty** on the first two, ghostty on the third (sections 7, 8) |
| 5. If neither clears R19 | replace the number, and take the cheaper candidate on rule 3 | **ghostty**, with R19 replaced (section 5) |

Rule 3 decides it and rule 5 confirms it. Rule 4 is the counterweight and it is a real one: if
the engineer weighs a pure-Rust build and a better-maintained dependency above feature coverage
and speed, the rule to change is rule 3's priority over rule 4, and the answer becomes
`alacritty_terminal`. That is a judgement about what steg's team wants to carry, not something
this spike can measure, so it is named here rather than decided quietly.

**What to depend on, concretely:** `ghostty-vt = "0.8"` (bindings, Apache-2.0) over ghostty
`1.4.0-main+83fb956` (the C library, MIT), built with Zig 0.17.x, with `GHOSTTY_SOURCE_DIR`
pointing at a vendored source tree in CI so no build reaches the network. Not
`libghostty-vt = "0.2.2"`, which the task named: it cannot be built here at all (section 8).

## 2. What was measured, and why in three parts

A single end-to-end latency number cannot tell these two apart: both go through the same GPUI
shape-and-paint path, and that path dominates. The measurements are therefore split, and the
split is what makes the answer legible:

- **A. The emulator alone, no GPUI.** Bytes in, screen read back. This is where they differ.
- **B. One live shell in a GPUI window.** Confirms or replaces R19, and proves each can be
  driven from GPUI at all.
- **C. Fit.** Thread ownership, build cost, maintenance, licence.

## 3. Evidence A: the emulator alone

Printing a 100 MB file straight into the emulator with a 10,000-row scrollback and no renderer.
Three runs each; the fixture is a deterministic mix of plain 80-column lines, truecolour SGR
every tenth line and double-width text every hundredth.

| | seconds | MB/s | slowest 64 KB chunk | peak RSS | scrollback held |
|---|---|---|---|---|---|
| `alacritty_terminal` 0.26 | 0.5 / 0.5 / 0.5 | 218 / 220 / 220 | 0.51 / 0.48 / 0.39 ms | 40.6 MB | 10,000 rows |
| `ghostty-vt` 0.8 | 0.1 / 0.1 / 0.1 | 1041 / 1050 / 1049 | 0.12 / 0.11 / 0.10 ms | 20.6 MB | 10,995 rows |

Per 64 KB chunk, and per read of the visible screen (which a renderer pays every frame):

| | feed p50 | feed p95 | feed max | read screen p50 | read screen p95 |
|---|---|---|---|---|---|
| alacritty | 0.28 ms | 0.29 ms | 0.39 ms | 0.01 ms | 0.01 ms |
| ghostty | 0.06 ms | 0.06 ms | 0.10 ms | 0.10 ms | 0.10 ms |

Ghostty parses about **4.8x faster** and holds a comparable scrollback in **half the memory**.
Alacritty reads the screen back about 10x faster, but at 0.1 ms against an 8.3 ms frame neither
figure is near mattering.

One measurement note, so the memory numbers are comparable at all: ghostty's `max_scrollback` is
a **byte** estimate, not a line count, so a request in lines has to be converted. The adapter
converts at 1,152 bytes a row, which is not a guess - it is what a row of the 100 MB fixture
costs at 120 columns, measured by giving the library a known byte budget and counting the rows
it kept. At that rate a 10,000-line request holds 10,995 rows, which is what the table reports
beside alacritty's 10,000. An undersized estimate holds proportionally fewer rows in
proportionally less memory, so getting this conversion right is work steg owns (section 4).

## 4. Evidence A: the R14 feature matrix

Every row is bytes fed in and the screen or the reported fact read back - never the emulator's
internals. "not reported" means steg has to implement it; "differs" means the library does it,
but not the way the requirement is written.

| feature | requirement | alacritty_terminal | ghostty-vt |
|---|---|---|---|
| truecolour (24-bit SGR) | R14 | handled | handled |
| double-width characters | R14 | handled | handled |
| alternate screen (1049) | R14 | handled | handled |
| mouse reporting (1000/1006) | R14 | handled | handled |
| bracketed paste (2004) | R14 | handled | handled |
| kitty keyboard protocol | R14 | handled | handled |
| OSC 8 hyperlinks | R14, R17 | handled | handled |
| OSC 52 clipboard | R14 | handled | handled |
| window title (OSC 0/2) | R16 | handled | handled |
| reflow on resize | R8, R20 | handled | handled |
| **OSC 7 working directory** | R14, R16 | **not reported** | handled |
| **OSC 133 prompt marks** | R14, R18 | **not reported** | handled |
| **unknown OSC reaches the app (OSC 7501)** | R49 | **not reported** | **not reported** |
| scrollback honours its limit | R15 | handled, in lines | differs: bounded in bytes |
| | | 11 handled, 3 left to steg | 12 handled, 1 left to steg, 1 differs |

What the table means in work:

- **OSC 7 and OSC 133 are steg's problem with alacritty.** `vte` 0.15's `osc_dispatch` handles a
  closed set - OSC 0/2, 4, 8, 10-12, 22, 50, 52, 104 - and writes anything else to a debug log
  (the `unhandled` helper in its `src/ansi.rs`). There is no hook. steg would have to pre-parse
  the byte stream for OSC 7 and OSC 133 before handing it to the emulator, which means a second
  parser over every byte of output on the hot path. Ghostty reports both: OSC 7 as a `PwdChanged`
  event, OSC 133 as per-row semantic-prompt state the renderer can read.
- **OSC 7501 is steg's problem either way** (R49, M2). Neither library hands an unrecognised OSC
  to the application: ghostty types it as `CommandType::Invalid` and drops it, vte logs and
  drops it. Rex's Program Status Protocol therefore needs steg's own OSC pre-parser whichever
  library is chosen. With ghostty that parser only has to look for 7501; with alacritty it has
  to look for 7501, 7 and 133.
- **The colour palette is the app's job with alacritty.** `Colors::default()` is 256 entries of
  `None`: the library hands back palette indices and named colours and leaves resolution to the
  caller. The probe's adapter ships its own xterm-256 table to make the comparison possible, and
  steg would ship that table too. Ghostty resolves fg and bg to RGB per cell through its own
  palette.
- **Scrollback differs rather than fails.** R15 says 10,000 lines. Alacritty prunes to a line
  count. Ghostty prunes to a byte estimate at page granularity: asked for 50 lines it held 191
  rows after 200 lines and 854 after 5,000, so it is bounded, just not by the number R15 names.
  steg converts lines to bytes, and either reports the real row count or stops promising lines.

Real programs, as a cross-check on the byte scripts: vim 9.1.1752 draws its splash screen
correctly on both, and btop draws its full-screen interface with box-drawing, braille sparklines
and colour correctly on both. tmux starts and runs a command. The acceptance set itself is
TASK-1.21's job, not this spike's.

## 5. Evidence B: GPUI, typing latency, and R19

One live `cat` on a pseudo-terminal in a GPUI window, 120x32, with a terminal view written for
this spike and shared by both candidates, so they are drawn by identical code. The probe writes
one byte, the tty's own echo brings it back, the pane feeds it to the emulator and asks for a
repaint. 2,000 samples each at about 12 keystrokes a second, no dropped samples, stable across
all eight blocks of 250 (the per-block tables are on the task).

Two clocks stop, and the difference between them is the finding:

- **key to paint**: the byte has been drawn into the frame gpui is building.
- **key to next frame**: gpui's next-frame callback has run. On macOS that callback is driven by
  the display link, so this includes waiting for the display's next refresh.

| | key to paint p50 | p95 | p99 | key to next frame p50 | p95 | p99 | within 16 ms |
|---|---|---|---|---|---|---|---|
| alacritty | 2.67 ms | 10.57 ms | 10.92 ms | 10.54 ms | 18.38 ms | 18.65 ms | 70.2% |
| ghostty | 2.12 ms | 9.91 ms | 10.47 ms | 9.67 ms | 17.33 ms | 18.29 ms | 80.8% |

As predicted, the gap between the candidates is about 1 ms at p95 - real, repeatable, and much
smaller than the gap between the two stop points. **The display is what dominates typing
latency, not the emulator.** At 120 Hz one refresh is 8.3 ms; the p50 of key-to-paint is 2.1-2.7
ms, and nearly all of the rest is waiting for a frame.

Typing in one pane while the other prints the 100 MB file, 300 samples:

| | key to paint p50 | p95 | max | key to next frame p95 | the print | worst single pump |
|---|---|---|---|---|---|---|
| alacritty | 9.16 ms | 10.81 ms | 11.86 ms | 18.03 ms | 0.05 s | **148.5 ms** (24 MB in one go) |
| ghostty | 9.39 ms | 10.40 ms | 11.41 ms | 16.76 ms | 0.10 s | **18.2 ms** (14.5 MB in one go) |

Neither pane was ever starved and no sample was dropped, but the worst-pump column is the
important one, and it is a finding about **steg's code, not the library's**: the probe's pane
feeds everything the pty has produced in one call, so one call handed the emulator 24 MB and
blocked the main thread for 148 ms. Ghostty survives the same mistake eight times better purely
because it parses faster. The fix belongs to TASK-1.16: cap the bytes fed per frame and carry
the rest over, so the UI thread never takes an unbounded parse.

### R19, confirmed or replaced (AC #2)

R19 says: *"Typing in a local pane paints within 16 ms at the 95th percentile. Printing a 100 MB
file does not freeze the app or delay input to other panes. These numbers are proposals to
confirm once a spike has measured a baseline."*

**First sentence: replace it.** 16 ms at p95 is not met by either candidate once the wait for the
display is counted (18.4 ms and 17.3 ms), and it is met comfortably by both if it is not (10.6 ms
and 9.9 ms). The number is not wrong so much as unstated: it does not say where the clock stops,
and on a 120 Hz display the refresh wait alone is 8.3 ms of a 16 ms budget. Proposed wording:

> R19 (revised). Typing in a local pane is drawn into the next frame gpui builds within **12 ms
> at the 95th percentile**, measured from the key event to the end of the paint. Including the
> wait for the display's refresh, the key-to-frame time is within **20 ms at the 95th
> percentile** on a 120 Hz display. Both are measured with the app's own instrumentation, not
> with a camera.

That is the measured baseline with a little headroom (ghostty: 9.9 ms and 17.3 ms today), and
unlike the original it says what is being timed. If a single number is wanted, "16 ms to paint"
is comfortably safe and 20 ms to frame is the honest user-visible figure.

**Second sentence: confirmed, with a condition.** Printing 100 MB neither froze the app nor
starved the other pane: no dropped samples, and p95 typing in the other pane stayed at 10.4-10.8
ms to paint. But it only holds if the app caps how much it feeds per frame. Unbounded, this
spike produced a 148 ms stall. Proposed addition: *"No single frame hands the emulator more than
a bounded amount of output; the rest is carried to later frames."*

## 6. Licences (AC #3)

Taken from `cargo tree -e normal --format "{p} {l}"` over each probe, plus the pieces no crate
graph shows. 435 crates in the alacritty tree and 436 in the ghostty tree; nearly all of them
are gpui's. The **full** per-crate list for each is the recorded output of these two commands on
TASK-1.1, which is where to look to check this section crate by crate:

```sh
cargo tree --manifest-path spikes/task-1.1-terminal-library/Cargo.toml -p probe-alacritty -e normal --format "{p} {l}"
cargo tree --manifest-path spikes/task-1.1-terminal-library/Cargo.toml -p probe-ghostty   -e normal --format "{p} {l}"
```

**What each candidate adds beyond the shared base (gpui, portable-pty):**

| candidate | crate | licence |
|---|---|---|
| alacritty | `alacritty_terminal` 0.26.0 | **Apache-2.0 only** |
| | `vte` 0.15.0 | Apache-2.0 OR MIT |
| | `cursor-icon` 1.2.0 | MIT OR Apache-2.0 OR Zlib |
| | `rustix-openpty` 0.2.0 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT |
| | `signal-hook` 0.4.6 | MIT OR Apache-2.0 |
| ghostty | `ghostty-vt` 0.8.0 | **Apache-2.0 only** |
| | `ghostty-vt-sys` 0.8.0 | **Apache-2.0 only** |
| | `dirs` 6.0.0, `dirs-sys` 0.5.0 | MIT OR Apache-2.0 |
| | `int-enum` 1.3.0 | MIT OR Apache-2.0 |
| | `proc-macro2-diagnostics` 0.11.0 | MIT/Apache-2.0 |

**Not a crate, so no licence check will ever see it:** ghostty's own Zig source, pinned in the
sys crate's `GHOSTTY.lock` as commit `83fb95624b0439ee660c2679b43b38c8b63769cd`, version
`1.4.0-main+83fb956`, fetched from `tip.files.ghostty.org` and verified against a sha256 in that
lock file. Its licence is **MIT** - read from the `LICENSE` file in the extracted source and from
`LICENSE-GHOSTTY` in the crate, not from a README. Zig itself is a build-time tool only (MIT) and
nothing of it ends up in steg's binary.

**Nothing in either tree is GPL or AGPL.** One weak-copyleft crate is present in **both** trees
and comes from gpui, not from either candidate: `option-ext` 0.2.0, **MPL-2.0**, reached through
`dirs-sys`. MPL-2.0 is file-level copyleft and an unmodified dependency is compatible with
shipping steg under MIT, but cargo-deny's usual allow-lists refuse it, so TASK-1.5 has to decide
it deliberately rather than discover it.

**For TASK-1.5's `deny.toml`**, the allow-list these trees need is: `Apache-2.0`,
`Apache-2.0 WITH LLVM-exception`, `MIT`, `MIT-0`, `BSD-2-Clause`, `BSD-3-Clause`, `ISC`, `Zlib`,
`0BSD`, `CC0-1.0`, `Unicode-3.0`, `Unlicense`, `BSL-1.0`, and a deliberate decision on
`MPL-2.0`. Note that `cargo-deny` is **not installed on this machine**, so `just deny` could not
be the evidence here; `cargo tree` was, and TASK-1.5 should make `just deny` real.

Both candidates are permissively licensed and clear R59's gate. Apache-2.0 carries an
attribution obligation (its NOTICE terms) when steg is distributed, and both candidates' own
crates are Apache-2.0 only, so steg's distribution will need a third-party licence file either
way.

**No GPL code was read while building this.** Zed's `terminal` and `terminal_view` crates solve
exactly this problem and are GPL; they were not opened. The GPUI terminal view in
`spikes/task-1.1-terminal-library/probe-core/src/element.rs` was written from gpui 0.2.2's own
`examples/` folder and its rustdoc, which are Apache-2.0.

## 7. Evidence C: fit with steg's architecture

**Thread ownership.** `.claude/rules/architecture.md` wants one task to own a piece of state and
others to send it messages. Both candidates work that way - proven, not assumed, by the
`ownership` mode: the emulator is created on its own thread, fed over a channel, and snapshots
come back over another, with no `Arc<Mutex<_>>` across tasks.

The difference is what the compiler allows:

- `alacritty_terminal::Term<VoidListener>` **is** `Send`. It compiles under an
  `fn assert_send<T: Send>()` probe.
- Ghostty's handles are **not**: the same probe fails with ``error[E0277]: `NonNull<TerminalImpl>`
  cannot be sent between threads safely``, because `Terminal`, `RenderState` and the iterators
  are `!Send + !Sync` by design.

So with ghostty a terminal must be *created* on the thread that owns it and may never move.
That is a constraint on TASK-1.8, not a blocker: one OS thread per pane, a channel in and a
channel out. It also rules out handing a terminal to a work-stealing async runtime, which the
kernel is not supposed to do anyway. Worth knowing before TASK-1.8 is designed, which is why it
is written here.

**Drawing.** Ghostty ships a render-state API built for this: a snapshot taken under one short
borrow of the terminal, global and per-row dirty flags, and resolved per-cell colours. Alacritty
offers `Term::damage()` with per-line damage bounds, which is the same idea with less of it, and
leaves colour resolution to the app. Both are enough for TASK-1.16; ghostty needs less code
around it.

**One thing both need from steg:** an OSC pre-parser for 7501 (R49), since neither hands an
unknown OSC to the application.

## 8. The bindings, which is where the real cost is

The task named `libghostty-vt` 0.2.2. **It cannot be built on this machine, and the reason is
worth recording because it will recur.**

- `libghostty-vt-sys` 0.2.2's `build.rs` clones ghostty at commit `a887df42` and runs
  `zig build`. That commit's `build.zig` calls `requireZig("0.15.2")`, and the check demands an
  exact major and minor, so only Zig 0.15.x with patch >= 2 is accepted. Zig 0.15.2 is the only
  such version available.
- Zig 0.15.2 cannot link **anything** against this machine's macOS 27 SDK. An empty
  `pub fn main() void {}` fails with every libSystem symbol undefined, because
  `libSystem.B.tbd` in both the Xcode and Command Line Tools SDKs lists only `x86_64-macos`,
  `arm64e-macos` and `arm64e.x1-macos` targets - no plain `arm64-macos` - and Zig's linker
  matches targets exactly. Apple's own linker does not care; Zig 0.15.2 does.
- Zig 0.17.1 links the same program fine. But ghostty `a887df42` refuses it:
  *"Your Zig version v0.17.1 does not meet the required build version of v0.15.2"*.

The two constraints are mutually exclusive, so that crate is unbuildable here. Ghostty's engine
was measured through `ghostty-vt` 0.8.0 instead, which is an explicit fork of those bindings
(it says so in its README) pinned to a newer ghostty and built with Zig 0.17, and which builds
in about 45 seconds. Zig 0.17.1 was installed with `mise install zig@0.17.1` alongside the
existing 0.15.2; nothing global was changed.

What this says about choosing ghostty:

- The ghostty path makes steg's build depend on **a Zig version that must satisfy both ghostty's
  own `requireZig` and the host's SDK at once**. That window closed on this machine today. It
  will close again on the next Xcode that changes a `.tbd`, and the fix is not in steg's hands.
- The bindings are third-party and plural: `libghostty-vt` (Uzaaft/pluiedev, 0.2.x),
  `ghostty-vt` (prabirshrestha/gpui-cn, 0.8.x), and others on crates.io. The C API is pre-1.0
  and its own README does not promise compatibility across revisions. Against that,
  `alacritty_terminal` is 0.26, pure Rust, with Alacritty and Zed as users.
- Mitigations, if ghostty is chosen: pin the Zig version in `mise.toml` so every machine and CI
  agree; vendor the ghostty source and set `GHOSTTY_SOURCE_DIR`, so `just check` never fetches
  anything and the pinned source is reviewable in-tree; and keep the adapter behind steg's own
  trait, so swapping to `alacritty_terminal` later is one crate, not a rewrite. The last point
  matters most: this spike's `TerminalBackend` trait was enough to hold both libraries behind
  identical measurement code, which is evidence that the swap stays cheap.

## 9. Findings other tasks need

- **TASK-1.5** (workspace, licence, check): the `deny.toml` allow-list in section 6, including a
  deliberate decision on `MPL-2.0` from gpui's `option-ext`; install `cargo-deny` so `just deny`
  stops being a skip; pin `gpui = "=0.2.2"` (it builds and runs standalone from crates.io - that
  was verified, not assumed); and if ghostty is chosen, pin Zig in `mise.toml`.
- **TASK-1.8** (kernel terminals): one thread per terminal, created on that thread, fed over a
  channel, with snapshots out. Required by ghostty's `!Send` handles and good practice either
  way. Also: scrollback for ghostty is a byte budget, so R15's line limit needs converting, and
  the kernel should report the real row count.
- **TASK-1.16** (terminal view, drawing and typing): cap the bytes fed to the emulator per
  frame; unbounded feeding gave a 148 ms main-thread stall on a 100 MB print. Shape one line per
  row with a per-row cache keyed on the row's contents - that is what the probe does and it is
  why an unchanged screen costs no shaping. Budget: at 120 Hz a frame is 8.3 ms, and the
  measured key-to-paint p50 is about 2 ms.
- **TASK-1.21** (full-screen programs): vim and btop already render correctly through both
  candidates, so the acceptance set starts from a working base.
- **TASK-1.23** (CI): if ghostty is chosen, `just check` needs Zig on the runner and a vendored
  ghostty source, or every pull request fetches a tarball.
- **R49 / program status (M2)**: steg needs its own OSC pre-parser for 7501 whichever library is
  chosen. Neither hands an unknown OSC to the application.

## 10. What this spike did not answer

- **Nothing here measures a physical key.** Both numbers start when the probe writes to the
  pseudo-terminal, so the path from a real key press through the window server and gpui's
  dispatch is excluded. Posting real events with CGEvent needs accessibility permission and
  would measure the window server more than steg.
- **One machine, one display, one font.** An M3 Ultra at 120 Hz is close to the best case. A
  60 Hz display doubles the refresh wait and would make the revised R19 wrong; if steg is ever
  measured on one, the key-to-frame number has to be re-stated.
- **No scrolled-back rendering, no selection, no reflow under a live window**, and no second
  client. Those belong to TASK-1.16, TASK-1.17 and TASK-1.12.
- **The acceptance set in R14** is not exercised in anger: vim, btop and a tmux session that
  starts, rather than a day's work in each. That is TASK-1.21.
