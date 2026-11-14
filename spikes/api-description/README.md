# Spike: one description of the API (TASK-1.3)

**Throwaway code.** It exists so the spike's answer rests on something that ran, and so a
reviewer can re-run it. TASK-1.10 builds the real thing and deletes this folder; nothing
outside `spikes/` depends on it.

The question: where does steg's API get defined, once, so that the server's methods, the
self-description a client asks for (R37), the CLI's commands (R32) and the reference docs
(R62) all come from that one place? `.claude/rules/architecture.md` sets the bar: a method
written by hand in the CLI, or documented by hand, is a second definition.

The answer, and the evidence behind it, are in the spike's board document. This file says
how to run the code.

## Run it

From the repository root:

```sh
spikes/api-description/carry-one-method                       # one method, end to end
cargo test --manifest-path spikes/api-description/Cargo.toml  # the drift tests
```

`just test` does not run this crate, by design: it is outside steg's workspace, so
`steg-affected` sees no crate under `spikes/` and the repository's checks are untouched by
it. The crate's manifest carries a bare `[workspace]` table, which makes it its own
workspace root and is what keeps it from colliding with the real workspace TASK-1.5 adds
at the repository root.

## What is where

| File | What it is |
|---|---|
| `src/api.rs` | **The one definition.** Three methods, one event, four errors, as data |
| `src/description.rs` | The shape of the description: the payload a client reads |
| `src/server.rs` | The toy server, which dispatches and validates against the description |
| `src/cli_build.rs` | The CLI generator: a description in, the whole command tree out |
| `src/cli_runtime.rs` | Candidate B, which is the generator fed by the server |
| `src/docs.rs` | The reference docs, rendered from the description (R62) |
| `client.py` | A plugin's view: Python calling the API knowing only the description |
| `tests/drift.rs` | What this has to prove, against the real binaries and a real socket |
| `carry-one-method` | The acceptance criterion in one command |
| `measure.py` | The timings, with the method written down |

The two candidates differ in one line, in `src/bin/spike_cli_a.rs` and
`src/bin/spike_cli_b.rs`: where the description comes from. Everything after that is the
same code, so the comparison is about when a client learns the API and nothing else.

## What the framing here is not

Newline-delimited JSON over a Unix socket is a stand-in, chosen because it is the least
that carries one call. **TASK-1.2 chooses the real framing.** What this spike fixes is the
layer above it: the description's JSON payload is the contract, so TASK-1.2 only has to be
able to carry it.

Target resolution in `src/server.rs` is likewise the thinnest thing that shows R10's
ambiguity rule. TASK-1.7 owns the real one.

## How the timings were taken

`measure.py` says it in full. In short: release builds, 50 runs each after 5 discarded,
`time.perf_counter()` around `subprocess.run` so the whole process is timed, reported as
the lowest, median and 95th percentile in milliseconds, with the machine and the Rust
version printed beside them. `hyperfine` is not installed on this machine, which is why
the loop is written out rather than borrowed.
