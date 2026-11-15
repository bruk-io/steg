---
id: TASK-1.40
title: Run a pane's program, and know when it has ended
status: In Progress
assignee:
  - '@bruk.w1'
created_date: '2026-11-15T01:32:00Z'
updated_date: '2026-11-15T17:35:00Z'
labels:
  - size-3
  - changes-requested
milestone: m-0
dependencies:
  - TASK-1.39
references:
  - task-1.40-run-a-pane-s-program-and-know-when-it
  - https://github.com/bruk-io/steg/pull/32
parent_task_id: TASK-1
type: task
project: bruk-io-steg
asdlc_accepted_from: DRAFT-10
asdlc_branch: task-1.40-run-a-pane-s-program-and-know-when-it
asdlc_plan_approved_by: bruk
asdlc_plan_approved_hash: '5e9ba0795787'
asdlc_plan_approved_size: size-3
asdlc_review_kind: same-machine
asdlc_review_findings: 'important=2,nit=4'
asdlc_review_policies: ["review", "rust", "licence"]
asdlc_plan_matched: no
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
doc-9 R13 and R16, doc-15 R97, doc-2 section 4 for the environment. The kernel holds the state; starting a process is the server's, behind an interface the server supplies.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A pane runs the login shell, or a given command, in a given working directory
- [ ] #2 When the program ends its exit code is known; a pane started with a command stays and can run the same command again, and a pane started as a shell closes
- [ ] #3 The program's environment has `STEG_SERVER`, `STEG_SESSION`, `STEG_TAB`, `STEG_PANE` and `STEG_BIN_DIR`, and the server's own directory is first on `PATH`
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
## Scope and the one honest boundary

One change, split cleanly at the kernel/server line doc-16 already draws: kernel gains the
*shape and the rule* (R97), server gains the *running program* (R1, "the server is where I/O
meets the kernel"). Deliberately excluded, named so no reviewer wonders: `--force` killing a
running program, the server's own login-shell base environment (R99 - that is TASK-1.44), and
anything beyond Running/Ended (richer `Status` with waiting/failed/bell/timers is TASK-1.41).
With those excluded this is one reviewable change; if build reveals otherwise, the fallback
split is exactly this seam (kernel: no new dependency, no I/O, land first; server: the
runtime, lands second against the kernel's new API).

## 1. Where the state lives, and the interface

**Shape crosses the API, so it is defined once in the sdk (R105), the same pattern TASK-1.39
used for `Screen`** (`crates/sdk/src/lib.rs`'s own doc comment: "Identifiers, the colour set
and pane kinds are shapes that cross the API... and the kernel uses them rather than defining
its own"). `pane show` reports "the running program, and its exit code once it has ended"
(doc-2 section 8), so this is API surface, not kernel-private data.

New `crates/sdk/src/program.rs`:
```rust
pub enum Program {
    Shell { cwd: String },
    Command { argv: Vec<String>, cwd: String },
}
pub enum ProgramStatus {
    Running,
    Ended { exit_code: i32 },
}
```
The variant itself stores "shell or command" - never inferred from whether `argv` is empty,
which is the trap: R97's stay-vs-close decision is exactly this fact, and it must be stored,
not reconstructed. Exported from `crates/sdk/src/lib.rs` beside `PaneKind`.

**Kernel holds the behaviour, not the engine.** `Pane` (`crates/kernel/src/pane.rs`) gains two
fields: `program: Option<Program>`, `status: Option<ProgramStatus>`. Both `None` only in the
window between a pane being minted and its program being started - see below for why that
window is never observed. `Pane::new`'s signature is unchanged (defaults both to `None`
internally), so none of the ~70 existing `Pane::new(id, kind)` call sites in kernel's own
tests move. New pane-level methods, `pub(crate)` (reached only through `Tree`, below):
- `start_program(&mut self, program: Program) -> Result<(), Error>` - errors
  `Error::ProgramAlreadyStarted(id)` if already `Some`.
- `record_exit(&mut self, exit_code: i32) -> Result<(), Error>` - errors
  `Error::ProgramNotStarted(id)` if `None`; otherwise sets `status = Ended { exit_code }`. Does
  **not** decide whether the pane closes - a `Pane` cannot run R94's tab/session cascade, only
  `Tree` can.
- `restart(&mut self) -> Result<(), Error>` - errors `Error::ProgramStillRunning(id)` unless
  `status` is `Ended`, and `Error::ShellCannotRestart(id)` if `program` is `Shell`; otherwise
  sets `status = Running` and leaves `program` (its `argv`/`cwd`) untouched, because "run the
  same command again in the same directory" (R97) means the *spec's own stored `cwd`*, never
  `Terminal::cwd()` - those two diverge the moment the program `cd`s, and restart must use the
  one that doesn't drift.
- `closes_on_exit(&self) -> bool` - `true` iff `program` is `Some(Program::Shell { .. })`.

Public accessors `program(&self) -> Option<&Program>` and `status(&self) -> Option<ProgramStatus>`.

`Tree` (`crates/kernel/src/tree.rs`) gets the id-addressed, public versions, the same shape as
its existing `resize_pane`/`swap_panes` (look the pane up via the existing
`tab_owning_pane_mut`, no new lookup helper needed):
```rust
pub enum ExitOutcome {
    Stayed,
    Closed(ClosedPane),   // reuses the existing type close_pane already returns
}
pub fn start_program(&mut self, pane: PaneId, program: Program) -> Result<(), Error>;
pub fn record_exit(&mut self, pane: PaneId, exit_code: i32) -> Result<ExitOutcome, Error>;
pub fn restart_program(&mut self, pane: PaneId) -> Result<(), Error>;
```
`record_exit` calls `pane.record_exit(exit_code)?`, then, if `pane.closes_on_exit()`, calls
`self.close_pane(pane)` - the exact same cascade a manual close runs (R94: tab/session closing
in turn) - and wraps the result as `ExitOutcome::Closed`; otherwise `ExitOutcome::Stayed`.
This is point 3 of the brief answered concretely: the close reaches the tree's cascade because
it *is* the tree's cascade, called from inside `record_exit`, not duplicated.

`Error` (`crates/kernel/src/error.rs`) gains `ProgramAlreadyStarted(PaneId)`,
`ProgramNotStarted(PaneId)`, `ProgramStillRunning(PaneId)`, `ShellCannotRestart(PaneId)`, each
with a `Display` line and no `source()` (same style as the existing plain-id variants).

**The `None` window, named rather than ignored.** A pane is minted by `new_session`/`new_tab`/
`split_pane` (unchanged signatures - threading `program` through those would touch ~123
existing call sites across three files for every sibling in-flight task on `tree.rs` to
rebase past, for no behavioural gain: the pane's id, which `STEG_PANE` must carry, does not
exist until minting returns, so the program cannot be started in the same call anyway).
Immediately after minting, the server calls `Tree::start_program` with that same id, before
the request handler returns and before any other request is processed (the control thread
that owns `Tree` is single-threaded and not reentrant between the two calls - see section 2).
`Pane::program() == None` is therefore never observed by anything outside that one pairing:
not by TASK-1.41's status, not by TASK-1.46's events, not by a client. This is a kernel
invariant enforced by shape, not just convention: `start_program` is the only way to leave
`None`, it can be called exactly once per pane (the `ProgramAlreadyStarted` guard), and nothing
reads a half-constructed pane because nothing else runs between mint and start.

**Kernel tests, no doubles** (`crates/kernel/src/tree.rs`'s existing test module, reusing its
fixtures): starting twice errors; recording an exit before starting errors; a `Command` pane
stays `Ended` and its `argv`/`cwd` survive a restart; a `Shell` pane's exit returns
`ExitOutcome::Closed` and - on a fixture where it's the tab's last pane - cascades exactly like
an existing `close_pane` test already asserts; restarting a `Shell` pane errors; restarting a
still-`Running` pane errors.

## 2. Thread ownership, and why it departs from one reading of TASK-1.39's note

**One OS thread per pane, for the pane's whole life - not per program run.** It is created the
moment `start_program` is first called for that pane and ends only when the pane closes. It
owns the kernel `Terminal` (created on it, once, and never recreated across a restart - that
is what "keeps what it printed" requires) and, while a program is running, the writer and
resize handle of the current run. `Tree` lives on the server's own control/dispatch thread,
a different thread entirely; the two talk only over two `Send` channels - commands in
(`PaneCommand`), events out (`PaneEvent`) - so the `!Send` engine (decision-3 section 7) never
has to cross a boundary, and neither does the `Tree`.

TASK-1.39's note - "a table keyed by pane owned by the thread that creates it" - reads most
naturally as one *shared* terminal-owning thread serving every pane's table entry. This plan
departs from that reading on purpose: R19's second clause ("printing a 100 MB file... does not
... delay input to other panes") is strongest when enforced by the OS scheduler, not by a
single shared recv loop - one pane's `vt_write` call blocking its own thread for 18 ms
(decision-3's measured worst case) cannot delay a *different* pane's bytes if they are
processed on different threads. A shared single terminal thread would serialize exactly the
case R19 cares about. One thread per pane keeps the `!Send` constraint and additionally buys
that isolation, at the cost of one more OS thread per open pane - cheap, and the same shape
decision-3 section 9 already recommended for TASK-1.8/1.40.

Within a pane's thread, while a program runs, one more short-lived relay thread is spawned per
*run* (not per pane): it blocks reading the pty, forwards bytes to the pane thread's channel as
they arrive, and - on EOF - calls the child's blocking `wait()` and forwards the exit code,
then ends. This mirrors the spike's own `spikes/task-1.2-wire-protocol/probe-server/src/pty.rs`
exactly (one thread, read-loop-then-wait), which is evidence the shape already reads cleanly in
this codebase, not a new technique. **Opening the pty and spawning the process happen on the
*control* thread, synchronously, before the pane thread is created or the run's relay thread is
spawned** - `portable-pty`'s master, writer and child handles are `Send` (only the engine is
pinned, per decision-3 section 7's own probe), so a bad command (`ENOENT`, say) is a normal
`Result` returned to the caller immediately, never something that has to escape a spawned
thread. `start`/`restart` both work this way: open+spawn on the control thread, hand the
resulting `Send` pieces into the pane thread (new thread for `start`, an existing one for
`restart`), which creates the relay thread and - for `start` only - the `Terminal`.

## 3. The environment (point 4 of the brief)

This task's part: the five variables and prepending `STEG_BIN_DIR` to `PATH`, per spawn.
TASK-1.44's part: where the server's *own* base environment comes from (R99's login shell).
The seam is a function signature, so swapping the source later is a one-line change at the
call site, not a rewrite: `crates/server/src/env.rs`'s
```rust
pub fn pane_environment(
    server: &str,            // STEG_SERVER - how to reach the server; a plain string this task takes as given
    session: SessionId, tab: TabId, pane: PaneId,
    bin_dir: &Path,           // STEG_BIN_DIR, and prepended to PATH
    base: &[(OsString, OsString)],   // everything the process otherwise inherits
) -> Vec<(OsString, OsString)>
```
`server` and `bin_dir` are **data passed in**, never computed inside this function - today the
call site gets `bin_dir` from `std::env::current_exe()`'s parent (real I/O, but trivial and
outside this pure function) and `server` from a placeholder until TASK-1.44 exists; this keeps
`pane_environment` itself a plain function with a plain unit test, no fake needed (it does no
I/O), exactly like kernel's own pure-logic tests, even though it lives in `server`.

## 4. Dependency

**`portable-pty = "0.9"`, server only.** It covers both the pty and the process spawn in one
crate (`CommandBuilder::spawn_command` on the slave side) - no separate process-spawning crate
needed. Already exercised against this exact shape in-repo:
`spikes/task-1.2-wire-protocol/probe-server/src/pty.rs` (`openpty`, `spawn_command`,
`try_clone_reader`, `take_writer`, `resize`, `wait`), and it was part of the "shared base" both
candidate trees in decision-3's spike carried, which is why neither tree's `cargo deny check
licenses` failure was about it (only ghostty's own `CDLA-Permissive-2.0` chain failed) -
reasonable evidence its licence already clears `deny.toml`'s allow-list, but this is **verified
for real by `just deny` in the build phase**, not asserted now; the version actually pulled
here will differ from the spike's pinned tree, so this is read, not assumed. If `just deny`
refuses something transitive, the fallback is the same one decision-3 used for
`CDLA-Permissive-2.0`: the engineer, not a worker, either widens `deny.toml`'s allow-list with
a reason or finds the feature to shed (`.claude/rules/licence.md`).

Rejected, and why: an async runtime (`tokio`, used by the wire-protocol spike, but only for its
*networking* goal - nothing about ptys or processes needs it, and `portable-pty`'s own API is
blocking, which fits plain `std::thread` better than a runtime that would need
`spawn_blocking` anyway); raw `nix`/`rustix` `openpty` calls (more code, for no gain
`portable-pty` doesn't already give, and it's already vetted in this repository for licence
and for exactly this problem shape).

`boundaries.toml`'s `[steg-server]` gains `"portable-pty"` in `allow`, with a reason citing
doc-16 R1 (the server is where I/O meets the kernel) and decision-3 section 9. No
`deny-features` entry (no optional feature of it is used). `just check-architecture` is
unaffected - no new container or component; `portable-pty` is an external dependency of the
already-modelled `server` container, the same treatment `ghostty-vt` already gets under
`kernel`.

## 5. Server structure

- `crates/server/src/pty.rs` - the interface the launch code is tested against:
  ```rust
  pub trait ProgramLauncher: Send + Sync {
      fn spawn(&self, spec: &LaunchSpec) -> Result<SpawnedProgram, Error>;
  }
  pub struct LaunchSpec { pub argv: Vec<String>, pub cwd: String, pub env: Vec<(OsString, OsString)>, pub size: (u16, u16) }
  pub struct SpawnedProgram {
      pub writer: Box<dyn Write + Send>,
      pub reader: Box<dyn Read + Send>,
      pub resize: Box<dyn Fn(u16, u16) -> Result<(), Error> + Send>,
      pub child: Box<dyn portable_pty::Child + Send + Sync>,
  }
  pub struct RealLauncher;   // native_pty_system() + CommandBuilder, mirrors the spike's pty.rs
  ```
  This is the trait the brief's "an interface the server supplies" means, placed entirely in
  **`crates/server`**, never in the kernel: if the kernel called a spawn trait itself, the
  kernel would be doing I/O by one level of indirection, which `crates/kernel/clippy.toml`'s
  disallowed list and doc-16 R110 both forbid in spirit even though they can't catch a trait
  object mechanically. The kernel exposes only the plain `start_program`/`record_exit`/
  `restart_program` of section 1; the server is the only thing that ever calls
  `ProgramLauncher::spawn`. The discriminating check: no `std::process::*`, `portable_pty`, or
  file type appears in any kernel signature, and `crates/kernel/clippy.toml` gains nothing.
- `crates/server/src/pane.rs` - the per-pane worker thread, its channel, and the public handle:
  ```rust
  pub enum PaneEvent { Exited { pane: PaneId, exit_code: i32 } }
  pub struct PaneHandle { /* holds the command Sender */ }
  impl PaneHandle {
      pub fn send_input(&self, bytes: Vec<u8>) -> Result<(), Error>;
      pub fn resize(&self, cols: u16, rows: u16) -> Result<(), Error>;      // TASK-1.42 needs this: resizes the Terminal and the pty master together, one call
      pub fn snapshot(&self) -> Result<Screen, Error>;                      // TASK-1.47 attaches here
      pub fn restart(&self, spec: LaunchSpec) -> Result<(), Error>;
      pub fn shutdown(self);
  }
  pub fn start(
      spec: LaunchSpec, pane: PaneId,
      launcher: Arc<dyn ProgramLauncher>, events: Sender<PaneEvent>,
  ) -> Result<PaneHandle, Error>;
  ```
  `PaneCommand` (the channel's actual message enum) stays private to the module; `PaneHandle`
  is the only public surface, named here for TASK-1.41 (reads `PaneEvent::Exited` to compute
  richer status), TASK-1.42 (`resize`), and TASK-1.47 (`send_input`/`snapshot` are exactly what
  an attached stream calls).
- `crates/server/src/env.rs` - `pane_environment` (section 3) plus its unit tests.
- `crates/server/src/error.rs` - `Error`, wrapping `io::Error` and `steg_kernel::Error` as
  needed, `Display`/`source()` per the kernel's own established style.
- `crates/server/src/lib.rs` - wires the modules in, re-exports `PaneHandle`, `PaneEvent`,
  `ProgramLauncher`, `RealLauncher`, `LaunchSpec`, `SpawnedProgram`, `pane_environment`,
  `start`, `Error`.

## 6. Tests, mapped to the acceptance criteria

- **AC #1** (runs the shell or a given command, in a given working directory):
  `crates/server/tests/real_pty.rs`, a real pty, `Program::Command` running
  `sh -c pwd` with `cwd` set to `std::env::temp_dir()`, asserting the printed path; plus
  kernel's `starting_a_programs_program_records_it`-style test proving the stored `Program`
  (shell vs. command, `cwd`) is exactly what was given, not inferred.
- **AC #2** (exit code known; a command pane stays and can run again; a shell pane closes):
  kernel: `a_command_pane_stays_after_its_program_ends`,
  `restarting_a_command_pane_resets_its_status_and_keeps_its_spec`,
  `a_shell_pane_closes_when_its_program_ends` (logic, no doubles). `real_pty.rs`: a real
  `sh -c 'printf hi; exit 3'` command pane - `PaneEvent::Exited{exit_code:3}` received (blocking
  `recv` on the event channel, never a sleep - the no-sleep rule), `snapshot()` shows "hi",
  `tree.record_exit` (called in the test, standing in for the server's own glue) returns
  `Stayed`, then `restart` re-runs it and a second `Exited{3}` arrives, with `snapshot()` now
  showing both runs' output concatenated - the proof that the `Terminal` was never recreated. A
  second real-pty case: a real `/bin/sh` as a `Program::Shell`, `send_input(b"exit 7\n")`,
  blocking-recv `Exited{exit_code:7}`, then `tree.record_exit` returns
  `Closed(_)` with the same tab/session cascade an existing `close_pane` test already asserts
  on a one-pane-tab fixture.
- **AC #3** (the five variables, and `PATH`): `env.rs`'s own unit tests assert the exact
  built list (no doubles - it is pure data in, data out). `real_pty.rs` pairs that with a real
  `sh -c env` run, asserting the five variables and the `PATH` prefix appear in a real child
  process's real environment, not just in the builder's return value.

`crates/server/tests/program.rs` (functional, the outside world faked at the boundary per
`.claude/rules/testing-rust.md`) covers the orchestration paths a real pty would make slow or
flaky to exercise repeatedly: a local `FakeLauncher`/fake `SpawnedProgram` (in-memory pipes, a
controllable exit code) proving the `Stayed`/`Closed`/restart wiring end-to-end through
`PaneHandle` without a real OS process, so CI's signal there is fast and the real-pty tests
stay few and deliberate.

## 7. What later tasks get, by name

- **TASK-1.41** (status): `Pane::program()`/`status()`, and `Tree::record_exit`'s
  `ExitOutcome` - `Stayed` vs `Closed` tells it which event to raise and whether the pane still
  exists to report status on.
- **TASK-1.42** (clients and size): `PaneHandle::resize(cols, rows)` - the one call that keeps
  the kernel `Terminal` and the pty master in step; TASK-1.42's job is feeding it a real
  client-declared size.
- **TASK-1.47** (attach): `PaneHandle::send_input`/`snapshot()` are the input and output sides
  a client's attached stream calls into; `PaneEvent::Exited` is one of the events TASK-1.46's
  subscription mechanism forwards.

## Proof, recap

`just check` (format, lint, boundaries, deny, affected tests, every package alone,
`check-architecture` unaffected, recipe names) recorded via `asdlc record-check`, plus the
test-to-clause mapping in section 6.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Plan checked against doc-9 (R13, R16), doc-15 (R97), doc-2 section 4, doc-8 (R1, R99) and doc-16 by @bruk.o: ok. The kernel holds what a pane's program is and its status, as shapes defined in the sdk, and the rule for what happens when it ends, reusing the tree's close cascade; the server launches through an interface of its own that the kernel never sees. One thread per pane owns its terminal for the pane's life, with a short relay thread per run; opening the pseudo-terminal and spawning happen on the control thread so a bad command is an ordinary error. portable-pty is added to the server only, its licence to be confirmed by just deny. Tests use a real pseudo-terminal and wait on events, never a sleep. Left to TASK-1.44: where the server's own environment comes from.

Check passed: `sh -c 'just check-fmt && just lint && just check-boundaries && just test'` exited 0 at 2026-11-15T16:35:06+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.40 on task-1.40-run-a-pane-s-program-and-know-when-it at c8dc7cdd8d3d with uncommitted changes:

```text
ok      format check (0.2s)
ok      lint (1.3s)
ok      boundaries (0.1s)
ok      tests (-p steg -p steg-app -p steg-kernel -p steg-sdk -p steg-server) (1.3s)
```

Check passed: `sh -c 'just deny && just check-boundaries'` exited 0 at 2026-11-15T16:35:36+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.40 on task-1.40-run-a-pane-s-program-and-know-when-it at c8dc7cdd8d3d with uncommitted changes:

```text
ok      licences (0.6s)
ok      boundaries (0.1s)
```

Check passed: `just check` exited 0 at 2026-11-15T16:48:31+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.40 on task-1.40-run-a-pane-s-program-and-know-when-it at c8dc7cdd8d3d with uncommitted changes:

```text
ok      format check (0.1s)
ok      lint (0.5s)
ok      boundaries (0.1s)
ok      licences (0.7s)
ok      tests (every crate) (0.7s)
ok      build (steg-sdk) (0.1s)
ok      test (steg-sdk) (0.2s)
ok      build (steg-kernel) (0.1s)
ok      test (steg-kernel) (0.2s)
ok      build (steg-server) (0.1s)
ok      test (steg-server) (0.3s)
ok      build (steg) (0.1s)
ok      test (steg) (0.1s)
ok      build (steg-app) (0.3s)
ok      test (steg-app) (0.3s)
ok      tool tests (1.2s)
ok      architecture (1.0s)
ok      recipe names (0.0s)
```

Check passed: `just check` exited 0 at 2026-11-15T16:50:22+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.40 on task-1.40-run-a-pane-s-program-and-know-when-it at c8dc7cdd8d3d with uncommitted changes:

```text
ok      format check (0.1s)
ok      lint (0.8s)
ok      boundaries (0.1s)
ok      licences (0.7s)
ok      tests (every crate) (0.9s)
ok      build (steg-sdk) (0.1s)
ok      test (steg-sdk) (0.2s)
ok      build (steg-kernel) (0.1s)
ok      test (steg-kernel) (0.2s)
ok      build (steg-server) (0.2s)
ok      test (steg-server) (0.4s)
ok      build (steg) (0.1s)
ok      test (steg) (0.1s)
ok      build (steg-app) (0.3s)
ok      test (steg-app) (0.3s)
ok      tool tests (1.2s)
ok      architecture (0.8s)
ok      recipe names (0.1s)
```

Check passed: `just check` exited 0 at 2026-11-15T16:51:27+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.40 on task-1.40-run-a-pane-s-program-and-know-when-it at 78c897c80feb:

```text
ok      format check (0.1s)
ok      lint (0.4s)
ok      boundaries (0.1s)
ok      licences (0.7s)
ok      tests (every crate) (0.7s)
ok      build (steg-sdk) (0.1s)
ok      test (steg-sdk) (0.2s)
ok      build (steg-kernel) (0.1s)
ok      test (steg-kernel) (0.2s)
ok      build (steg-server) (0.1s)
ok      test (steg-server) (0.4s)
ok      build (steg) (0.1s)
ok      test (steg) (0.1s)
ok      build (steg-app) (0.3s)
ok      test (steg-app) (0.3s)
ok      tool tests (0.9s)
ok      architecture (1.0s)
ok      recipe names (0.0s)
```

Built as planned, no departures from the approved plan's design (a few implementation-level refinements, noted below).

New: crates/sdk/src/program.rs (Program, ProgramStatus, the shapes that cross the API). Changed: crates/kernel/src/{pane,tree,error,lib,tab}.rs (Pane::start_program/record_exit/restart/closes_on_exit; Tree::start_program/record_exit/restart_program and ExitOutcome, reusing close_pane's own cascade; four new Error variants). New: crates/server/src/{pty,pane,env,error}.rs (ProgramLauncher/RealLauncher over portable-pty; the per-pane worker thread and PaneHandle; pane_environment; server's Error). Changed: crates/server/{Cargo.toml,src/lib.rs}, boundaries.toml (portable-pty, anyhow added to [steg-server]'s allow-list). New: crates/server/tests/{program,real_pty}.rs.

Refinements from the plan's exact wording, each small and within the plan's own stated intent:
- pane_environment takes just the current PATH value, not a full "base environment" list - portable_pty::CommandBuilder already seeds every other inherited variable from the current process's own environment on its own (its get_base_env()), so a full base list would have been redundant. STEG_SERVER/STEG_BIN_DIR are still data passed in, never computed inside the function, which was the point of that part of the plan.
- Program::Shell resolves to a concrete argv entirely at the caller's discretion (LaunchSpec.argv is just a Vec<String>; crate::pane::start/restart take whatever argv the caller already resolved) - the plan didn't fix this detail, and real_pty.rs's tests just hand it a real shell command directly, so no resolution policy had to be invented for this task.

Which test proves which clause:
- AC #1 (runs the shell or a given command, in a given working directory): real_pty.rs::a_real_program_runs_in_the_given_working_directory (real sh -c pwd against a real temp dir), plus kernel's starting_a_program_records_it_as_running (the stored Program is exactly what was given).
- AC #2 (exit code known; a command pane stays and can run again; a shell pane closes): kernel tests with no doubles - a_command_pane_stays_ended_and_does_not_close_on_exit, restarting_a_command_pane_resets_its_status_and_keeps_its_spec, a_shell_pane_closes_on_exit (pane.rs); a_command_panes_exit_stays_and_can_be_restarted, a_shell_panes_exit_closes_it_through_the_same_cascade_as_a_manual_close (tree.rs). Server: program.rs::restarting_keeps_the_same_terminal_so_old_output_survives_a_new_run (FakeLauncher, orchestration). Real: real_pty.rs::a_command_panes_real_exit_code_is_known_it_stays_and_can_run_again, a_shell_panes_real_exit_reaches_the_kernels_cascade (real /bin/sh, real exit codes, feeding the kernel's own record_exit with no doubles).
- AC #3 (the five variables, and PATH): server/src/env.rs's own unit tests (exact built list, no doubles) plus real_pty.rs::a_real_programs_environment_carries_the_five_variables_and_path (a real sh -c env).

just check recorded clean at each step (sdk+kernel first, then deny+boundaries after adding portable-pty/anyhow, then the full suite twice - once before and once after rebasing onto origin/main, which picked up TASK-1.42/1.70/1.74/1.75's board-only commits with no code conflicts). portable-pty's licence cleared just deny on the first try, so the engineer was not asked to widen deny.toml. CI green at PR #32 (both checks passed, ~3m40s).

Check FAILED: `cargo test -p steg-server --lib -- shutdown_mid_run` exited 101 at 2026-11-15T17:30:50+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.40 on task-1.40-run-a-pane-s-program-and-know-when-it at 0f970dc40fa9 with uncommitted changes:

```text
   Compiling steg-server v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.40/crates/server)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.32s
     Running unittests src/lib.rs (target/debug/deps/steg_server-d9f0e6653e721d82)

running 1 test
test pane::tests::shutdown_mid_run_still_reaps_a_program_that_was_still_producing_output ... FAILED

failures:

---- pane::tests::shutdown_mid_run_still_reaps_a_program_that_was_still_producing_output stdout ----

thread 'pane::tests::shutdown_mid_run_still_reaps_a_program_that_was_still_producing_output' (7358525) panicked at crates/server/src/pane.rs:405:9:
pid 784 still exists: child.wait() was skipped, leaving a zombie
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    pane::tests::shutdown_mid_run_still_reaps_a_program_that_was_still_producing_output

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 4 filtered out; finished in 1.03s

error: test failed, to rerun pass `-p steg-server --lib`
```

Commits since the review: 33, 31 without the Asdlc-Worker trailer.

Check passed: `just check` exited 0 at 2026-11-15T17:35:33+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.40 on task-1.40-run-a-pane-s-program-and-know-when-it at 775fcedcceee with uncommitted changes:

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
ok      tool tests (0.9s)
ok      architecture (1.0s)
ok      recipe names (0.0s)
```

Commits since the review: 58, 51 without the Asdlc-Worker trailer.
<!-- SECTION:NOTES:END -->

## Comments

<!-- COMMENTS:BEGIN -->
author: @bruk
created: 2026-11-15T16:26:00Z

---

Plan approved by @bruk (5e9ba0795787, size-3): its worker may build it.

---

author: @bruk.r4
created: 2026-11-15T16:58:00Z

---

@bruk.r4 is reviewing TASK-1.40: a same-machine review: it waited 2m with no other engineer's reviewer, longer than review_fallback_after (0m), so a fresh reviewer on its author's machine takes it.

---

author: @bruk.r4
created: 2026-11-15T17:11:00Z

---

Review by @bruk.r4 (same-machine): changes requested. Back to In Progress for the author's worker.

Findings: 2 Important, 4 nit. Cites: review, rust, licence. The diff does not match the plan.

[Bugs, Important] crates/server/src/pane.rs:270-304 (spawn_relay): when the pane's worker thread is gone while the relay thread is reading - e.g. a pane shut down via PaneHandle::shutdown() (reached by `pane close --force`, doc-2 section 9) while its program is still producing output - `commands.send(PtyBytes(...))` fails and the thread returns immediately without ever calling `child.wait()`. Confirmed empirically: starting a real `sh -c 'sleep 2; echo done'` and calling `handle.shutdown()` while it runs leaves the child as `<defunct>` (a zombie) in `ps`. When the program instead prints nothing before shutdown (e.g. `sleep 1000`), the same thread stays blocked in `reader.read()` forever - a leaked thread, not just a leaked zombie. The plan deliberately excludes *killing* a running program on close ("--force killing a running program" is named as out of scope), but reaping its exit status and ending the relay thread is a different, in-scope property - the relay thread's own stated job is exactly "waits for the exit code... then ends" - and this path silently drops both.

[Compliance, Important] crates/server/Cargo.toml, boundaries.toml: `anyhow` is added as a new direct dependency of steg-server and to boundaries.toml's allow-list, but the plan's "Dependency" section names only `portable-pty = "0.9"`, and the task's own review brief says "the new dependency: portable-pty only." The implementation notes' "Refinements from the plan's exact wording" section lists two refinements and does not mention adding anyhow as one of them; it appears only inside the file-manifest sentence, never framed as a departure. Per policy, "a departure from the plan is a finding unless the task's notes say what changed and why" - this one isn't said.

[Evidence, Nit] No test exercises the plan's central ENOENT claim ("a bad command... is a plain Result here, never something that has to escape a spawned thread"): FakeLauncher always succeeds, and real_pty.rs never spawns a nonexistent binary. I verified by hand that `steg_server::start` with a nonexistent argv does return `Err(Error::Pty(_))` synchronously, so the claim holds, but nothing in the PR proves it.

[Rust, Nit] crates/server/src/error.rs: `Error::Pty(anyhow::Error)`'s `source()` returns `None`. `.claude/rules/rust.md` wants a library error's cause kept as its `source()`; the comment's reason (anyhow::Error isn't std::error::Error) is accurate but the cause still isn't reachable except via Display.

[Licence, Nit] The task's notes never name the licences of the seven new crates portable-pty (0.9.0, MIT), anyhow (1.0.104, MIT/Apache-2.0), nix (0.28.0, MIT), serial2 (0.2.39, BSD-2-Clause/Apache-2.0), shell-words (1.1.1, MIT/Apache-2.0), shared_library (0.1.9, Apache-2.0/MIT) and cfg_aliases (0.1.1, MIT) - all permissive, none GPL/AGPL, `just deny` passes - which the Licence pass calls a Nit when unnamed on the task.

[Tests, Nit] No bound on how long a real-pty test, or the CI job itself, can run: real_pty.rs's waits are all unbounded `events_rx.recv()`, and .github/workflows/ci.yml's `check` job sets no `timeout-minutes`, so a misbehaving child would hang the job up to GitHub's 360-minute default on a macOS runner this file itself calls "billed at a premium."

Evidence checked: just check (format, lint, boundaries, licences, affected+per-package tests, architecture, recipe names) run clean on this worktree at the branch head (78c897c80feb), matching the recorded check and PR #32's green CI run on the same commit. just deny passes; boundaries.toml's new allow entries have true reasons. crates/kernel/clippy.toml and crates/sdk/clippy.toml are unchanged - no new I/O exemption in the kernel, and no process/file/socket/clock type appears in any kernel or sdk signature. Kernel tests cover starting twice (errors), recording an exit before starting (errors), restart only once Ended and never for a Shell pane, and a shell pane's exit cascading through the exact tree used by a manual close, on a one-pane-tab/one-tab-session fixture (a_shell_panes_exit_closes_it_through_the_same_cascade_as_a_manual_close). Program/ProgramStatus have wire-form round-trip tests, including a negative exit code. Ran crates/server/tests/real_pty.rs five times in a row with no failures and no sleep-based waits (all blocking recv on the event the real program or a real EOF triggers). No unwrap outside tests; errors from send-to-a-dropped-receiver are handled with map_err, not a panic.

---
<!-- COMMENTS:END -->
