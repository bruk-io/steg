# TASK-1.2: the wire protocol spike

Throwaway code. It exists so the answer in the board document can be re-run and checked, and
**TASK-1.12 deletes this whole directory** when the real attach path lands. Nothing here is
production code, nothing is a workspace member, and no root file was touched: the `[workspace]`
table in `Cargo.toml` makes this its own workspace root so the repository's real manifest
(TASK-1.5) does not adopt it.

The question it answers: how a byte on the wire is delimited, how control calls and terminal
output share one connection, and what the server puts on the wire so that a client killed
mid-output is replaced by a new one showing the same screen and scrollback (R1, R2, R36, R39).

## The three scripts

Each takes its arguments, runs, prints its own verdict and exits by itself, so `asdlc
record-check` and a reviewer can run them unattended.

```sh
./reattach-demo [--mode bytes|deltas|screen] [--framing length|ndjson] [--lines N] [--interval-us U]
./gates         [--framing length|ndjson]
./measure       [--mode M] [--framing F] [--mega-bytes N] [--samples N] [--overflow resync|block] [--tick-ms N]
```

- `reattach-demo` is acceptance criterion 2. A pane prints numbered lines past the 10,000-line
  scrollback limit; a client is **SIGKILLed** mid-output; nothing is attached for a while; a new
  client attaches; the program finishes; then `probe-client verify` compares that client's screen
  and scrollback against the server's own capture byte for byte, checks the numbered lines run
  with no gap and no repeat, and checks that every line printed while nothing was attached is
  there. It also attaches two clients of different sizes to one pane and kills one (R7).
- `gates` runs the decision rule's gates: a dependency-free Python client (`client.py`), the
  protocol over a pipe pair through a relay process and over SSH when one answers, and the
  handshake's answers to a mismatched version and to a first frame it cannot read.
- `measure` runs the numbers: keystroke-to-screen latency, the cost of attaching with a full
  scrollback, and a 100 MB flood in one pane with a slow client attached while a control call and
  a keystroke in another pane are timed.

`client.py` needs no third-party package and no Rust. It is run directly with `python3` rather
than through `uv run`, which is the repository's usual way: the point of that file is that a
plugin can speak this protocol with whatever Python is already there, so the lowest floor is the
strongest evidence. Its PEP 723 block declares an empty dependency list for the same reason.

## What each number includes and excludes

- **Latency** is a byte into the pty, to that byte appearing on the client's own copy of the
  screen. It stops at the client's grid: the drawing half is TASK-1.1's measurement, and R19's
  16 ms has to cover both halves added together. It is not photon-to-glass.
- The machine, the crate versions, the build profile and the tick are printed above every table,
  so a number cannot be read out of its context. Measurements were taken on macOS 27.0.0 arm64,
  28 cpus, rustc 1.98.1, release profile, opt-level 3, lto off.
- `measure` is timing-sensitive by nature: a paced program, a wall-clock kill, a deliberately slow
  reader. Its oracle fails loudly rather than passing by luck, so re-run it rather than trusting
  one run.

## The crates

- `probe-wire`: the protocol. The two framings, the handshake and its version rule, the control
  envelope, the snapshot and the delta types. Its tests (`cargo test --manifest-path
  probe-wire/Cargo.toml`) are the mechanical half of the spike: framing at arbitrary chunk
  boundaries, the frame cap, and every handshake refusal.
- `probe-server`: a pty per pane, one thread owning each terminal, attachments with bounded
  queues, and a writer that puts control frames ahead of terminal data.
- `probe-client`: the subcommands the scripts drive, including both client shapes (an emulator
  fed raw bytes, and plain rows fed deltas), the verify oracle, and `emit`, the deterministic
  program the panes run.

`alacritty_terminal` is here as an **instrument**, not as a choice: TASK-1.1 chooses the
emulation library. The document says which findings are independent of that choice.

The tests run with `cargo test --manifest-path ...` rather than `just test`, a stated departure
from `.claude/rules/testing-rust.md`: this crate is deliberately outside the workspace, and a
justfile recipe would collide with TASK-1.5 and make `steg-affected` select every crate. Every
run is recorded on the task through `asdlc record-check`.
