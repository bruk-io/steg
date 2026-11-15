# TASK-1.1: which terminal emulation library

Throwaway measuring instrument. It exists so the numbers in the board document can be re-run
and checked, not because any of it ships. The answer is the document; this is the evidence.

## Why it lives here and not in the workspace

`justfile:12` turns every Rust step on as soon as a `Cargo.toml` exists at the repository root,
and TASK-1.5 is adding the real root workspace in another worktree at the same time. So this
folder is its own Cargo workspace: the `[workspace]` table in `Cargo.toml` makes cargo treat it
as a separate root, and the future root manifest will not adopt these crates as members.
`just check` in the repository skips every Rust step while TASK-1.1 is the only change, and
that is the expected result.

## Running it

```sh
cd spikes/task-1.1-terminal-library
cargo build --release                       # alacritty_terminal only
ZIG=<zig 0.17 binary> cargo build --release # also ghostty-vt, which builds its own C library

./target/release/probe-alacritty <mode>
./target/release/probe-ghostty  <mode>
```

Every mode runs to completion, prints a Markdown table and quits by itself, because
`asdlc record-check` runs these with nobody present to close a window.

| mode | what it measures | GPUI |
|---|---|---|
| `smoke` | bytes in, screen back. The cheapest proof a candidate works | no |
| `conformance` | the R14 feature matrix, plus OSC 7501 and reflow | no |
| `flood [MB] [runs]` | printing a 100 MB file with no renderer | no |
| `latency [samples]` | typing latency in one pane | yes |
| `flood-ui [MB] [samples]` | typing in one pane while the other prints the file | yes |
| `window [seconds]` | a live shell to look at, which closes itself | yes |
| `ownership` | the emulator owned by one thread, driven over a channel | no |

## What the latency numbers include, and what they do not

The probe writes a byte to the pseudo-terminal, the tty's own echo brings it back, the pane
feeds it to the emulator and asks for a repaint. Two clocks stop:

- **key to paint**: the byte has been drawn into the frame gpui is building.
- **key to next frame**: gpui's next-frame callback has run. On macOS that callback is driven by
  the display link, so this includes a wait for the display's refresh. At 120 Hz one refresh is
  8.3 ms, which is half of R19's 16 ms budget before steg does anything at all.

The truth for "on the glass" sits between the two. Neither number includes the path from a
physical key to gpui's dispatch: posting real key events with CGEvent needs accessibility
permission and would measure the window server more than it measures steg.

`cat` is the program, not a shell, so the sample holds no prompt redraw or completion
machinery. Arrival is detected by the cursor moving, not by looking for the letter, because a
scrolling screen drops older copies of the same letter and that would lose samples.

The one unavoidable addition is polling: gpui's contexts may only be touched from the main
thread, so the pane looks for new output every 200 microseconds instead of being woken. That
adds a uniform 0 to 200 microseconds, the same for both candidates.

## The two candidates

| | crate | licence | built how |
|---|---|---|---|
| alacritty | `alacritty_terminal` 0.26 | Apache-2.0 | pure Rust |
| ghostty | `ghostty-vt` 0.8 over ghostty `1.4.0-main+83fb956` | Apache-2.0 bindings, MIT C library | Zig 0.17 at build time |

The task named `libghostty-vt` 0.2.2 for the ghostty side. That crate cannot be built on this
machine: it pins ghostty `a887df42`, whose `build.zig` calls `requireZig("0.15.2")`, and Zig
0.15.2 cannot link anything against the macOS 27 SDK, where `libSystem.B.tbd` lists only
`x86_64`, `arm64e` and `arm64e.x1` targets and no plain `arm64-macos`. Even `zig build-exe` of
an empty `main` fails. `ghostty-vt` 0.8 is an explicit fork of those bindings on a newer
ghostty commit built with Zig 0.17, which does link, so the engine is measured through it. The
document keeps the two findings apart: what the engine does, and the state of the bindings.
