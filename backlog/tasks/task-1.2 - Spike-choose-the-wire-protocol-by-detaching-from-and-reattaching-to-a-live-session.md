---
id: TASK-1.2
title: >-
  Spike: choose the wire protocol by detaching from and reattaching to a live
  session
status: In Progress
assignee:
  - '@bruk.w4'
created_date: '2026-11-13T22:52:00Z'
updated_date: '2026-11-14T05:14:00Z'
labels:
  - size-3
milestone: m-0
dependencies: []
documentation:
  - doc-5
parent_task_id: TASK-1
type: spike
project: bruk-io-steg
asdlc_accepted_from: DRAFT-3
asdlc_branch: task-1.2-spike-choose-the-wire-protocol-by
asdlc_plan_approved_by: bruk
asdlc_plan_approved_hash: '4dfe21538510'
asdlc_plan_approved_size: size-3
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Design decision 2 in the spec. A server holds one terminal; a client is killed and a new one reattaches with the screen and scrollback intact (R1, R2). Decides message framing, and whether terminal output reaches a client as raw bytes or as screen updates. The code is throwaway.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A board document records the framing and how terminal output reaches a client, with the reason
- [ ] #2 The document shows a client killed mid-output and a new client restoring the same screen and scrollback
- [ ] #3 The document says how the protocol carries a version, for R39
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
## What this spike decides, and what it must not

Decides, as design decision 2 in the spec: how a byte on the wire is delimited, how control
calls and terminal output share one connection, and what the server puts on the wire so that
a client killed mid-output is replaced by a new one showing the same screen and scrollback
(R1, R2, R36, R39). The code is a throwaway instrument; the deliverable is one board document
with the recorded demonstration.

Must not decide, and the answer has to leave room for each:

- **The terminal emulation library** (TASK-1.1). This spike uses one as an instrument and
  says so; see "The emulator stand-in".
- **The API description** (TASK-1.3). It fixed that the **JSON payload** of the
  self-description is the contract and declared newline-delimited JSON a stand-in for
  framing. This spike replaces the framing and must say in one sentence whether its choice
  keeps that JSON payload contract intact.
- **Command names, flags and exit statuses** (TASK-1.4, doc-2). The spike's client is not a
  CLI proposal; its arguments are whatever is cheapest to drive.
- **The server's lifecycle on macOS** (decision 4, settled in TASK-1.9's plan). The spike
  starts its server by hand.
- **What size a terminal takes when two attached clients differ** (decision 6, R7, M3). The
  spike proves the protocol can carry two attachments and a per-attachment size; which size
  wins is not its question.

## What is already settled, so no candidate re-argues it

Before any candidate exists: **the server holds the emulator in every candidate.** This is
not this spike's choice, it is already required:

- R15 keeps 10,000 lines of scrollback per pane *on the server*.
- `pane capture` prints a pane's screen and scrollback as text (doc-2 section 8) with no
  client attached, and a Python plugin reaching it through the public API (R42, R75) cannot
  replay VT bytes.
- `.claude/rules/architecture.md:11`: a client "never keeps a copy it treats as the truth".
- AC #2's screen includes output produced while **no client was attached at all**.

So the open question is not who emulates. It is **what the server puts on the wire**: a
snapshot, a stream of screen deltas, raw bytes replayed onto a snapshot, or a mix. If the
measurements favour raw bytes after a snapshot, the document must reconcile that with
architecture.md:11 explicitly: the client's grid is then a derived cache, rebuilt from the
server's snapshot on demand, and the server stays the only truth. If that reconciliation
fails, the honest outcome is a `contract-change` comment on this task, not a workaround.

## The four axes

The task names two questions; there are four, and conflating them is how this spike goes
wrong.

1. **Byte framing.** How a frame's end is found. Candidates **F1** newline-delimited JSON
   (TASK-1.3's stand-in, with terminal bytes base64 in a JSON string) and **F2** a
   length-prefixed binary frame whose payload is either UTF-8 JSON (control) or opaque bytes
   (terminal data).
2. **Control message shape.** Whether TASK-1.3's JSON payload contract survives. F2 is
   designed to keep it: control payloads stay the same JSON bytes, only the delimiter
   changes. The document states this as a finding, not a hope, proved by `client.py`.
3. **Topology.** **T1** one multiplexed connection, every attachment a stream id within it,
   against **T2** a control connection plus a separate data connection per attachment,
   opened with a ticket the server issues (Rex's shape, named in the brief). What this
   decides is head-of-line blocking: whether a 100 MB flood in one pane delays a control
   call or input to another pane.
4. **Terminal output representation.** **D1** snapshot on attach, then raw pty bytes, with a
   client-side emulator. **D2** snapshot on attach, then screen deltas the server computes,
   with no emulator in the client. The snapshot is plain data in both (rows of cells: char,
   foreground, background, flags; cursor; modes; scrollback rows), so neither needs an
   emulator to *start*.

### Back-pressure is the finding, not a footnote

A client killed mid-output (AC #2) is the limiting case of a client that reads slowly, and
R19's second sentence ("does not freeze the app or delay input to other panes") forces an
answer: when an attachment's queue fills, what happens? The three possibilities are block
the pty reader (which delays the program, and other panes if one task serves many), grow the
queue without limit (memory), or discard and resynchronise. This is where D1 and D2 actually
differ:

- D2's deltas coalesce: on overflow, discard what is queued and enqueue one fresh snapshot.
- D1's bytes cannot be coalesced at all, so D1 must *also* implement discard-and-resnapshot
  to survive a slow client, which means D1 needs the whole snapshot path anyway plus an
  emulator in the client.

So the spike measures the overflow policy as a first-class result: bytes on the wire, peak
server memory per attachment, and how long a deliberately slow client takes to catch up after
a 100 MB flood.

### One correctness requirement both data candidates must prove

The snapshot and the stream that follows it must be atomic, or a reattach silently loses or
repeats output: the snapshot is taken at sequence N and the stream must resume at exactly
N+1. The thread that owns the terminal produces both in one step (which is the shape
`.claude/rules/rust.md:29` prescribes anyway: one task owns the state, others send it
messages). The spike proves this with the no-client-attached gap in the demonstration, and
the document records it as a requirement on TASK-1.12, not an implementation detail.

## The decision rule, fixed before any measurement exists

Written here, before the numbers, so the answer is not a weighting chosen to fit the data.

**Gates. A candidate is out whatever its speed if any fails.**

1. **Usable from outside Rust** (R42, R75): a PEP 723 Python script makes a control call and
   reads the description payload knowing nothing of Rust, and attaches to a pane. Proved by
   running it, not asserted.
2. **Survives a byte stream it does not control** (R5 now, R55 later): framing is
   self-delimiting over one bidirectional stream, with no dependence on datagram boundaries
   (`SOCK_SEQPACKET` is therefore out), no file-descriptor passing and no peer-credential
   check in the protocol itself. Proved by running the same client over a plain pipe pair
   through a proxy process, and by decoding a captured stream fed in one byte at a time.
3. **Leaves R7 open**: two clients attach to one pane at once, each with its own attachment
   id and its own declared size, and killing one does not disturb the other.
4. **Licence** (R59): MIT project, nothing from a GPL or AGPL source, every dependency's
   licence listed.
5. **Stays clear of the workspace**: no root `Cargo.toml` (`justfile:12` turns every Rust
   step on with `path_exists("Cargo.toml")`, and `just deny` would then fail for want of a
   `deny.toml`), nothing outside `spikes/`, the spike's own bare `[workspace]` table.

**Thresholds, not scores.**

6. Reattach is byte-for-byte correct (AC #2). A candidate that is not correct is out.
7. Under a 100 MB flood in one pane: a control call and a keystroke to another pane stay
   inside R19's 16 ms at the 95th percentile, and the flooded pane's program is not stalled
   by a slow or dead client. Being faster than the other candidate is worth nothing on its
   own.

**Then the work each one leaves steg to write decides**, in days: an emulator in every client
(including the app's second copy, which interacts with TASK-1.1's choice) and a resync path,
against the server-side delta computation and a cell-model contract that has to carry every
R14 feature the app draws. Tie-break, in order: how much of it a Python client can use
(R42/R75); what it costs over a single SSH stream (R55); then bytes on the wire.

## The emulator stand-in

`alacritty_terminal` 0.26.0 (Apache-2.0, pure Rust, no Zig and no network at build time, the
cheapest of the two candidates to build) is used **as an instrument**. TASK-1.1 owns the
library choice and this is not it. The document splits its evidence honestly:

- **Measured and emulator-independent**: framing sizes and decode correctness, head-of-line
  blocking, snapshot size and build time with 10,000 lines of scrollback, reattach
  correctness, overflow behaviour, Python usability.
- **Reasoned from TASK-1.1's recorded finding**: `libghostty-vt`'s handles are `!Send +
  !Sync`, so a pane's terminal lives on one thread with bytes in and snapshots out either
  way. The document states where the answer holds for both and where it does not.

Deltas (D2) are computed by diffing consecutive snapshots row by row, which any emulator
supports, rather than through one library's damage API. The document records separately what
a damage API would save, so D2's cost is not overstated for whichever library wins.

## Where the throwaway code lives

`spikes/task-1.2-wire-protocol/`, inside the worktree
`/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.2`, as its own Cargo
workspace. The directory name follows TASK-1.1's convention. Its `Cargo.toml` carries a bare
`[workspace]` table so cargo treats it as a separate root and the workspace TASK-1.5 adds in
parallel does not try to adopt it; after TASK-1.5 lands, `steg-affected` sees these files as
belonging to no workspace crate. `target/` is already ignored at any depth
(`.gitignore:5`). The 100 MB fixture is generated under `$TMPDIR` at run time and is never
committed. TASK-1.12 deletes the directory when the real thing lands; the README says so.

## Files that change

All new, all under `spikes/task-1.2-wire-protocol/`:

- `Cargo.toml`: the spike's own workspace, `[workspace]` with members `probe-wire`,
  `probe-server`, `probe-client`, and the `[profile.release]` settings every measurement uses.
- `Cargo.lock`: committed, so the measured versions are recorded exactly.
- `README.md`: what the spike answers, how to run each measurement, what each number includes
  and excludes, the machine it ran on, that it is throwaway, and who removes it.
- `probe-wire/Cargo.toml`: `serde` 1, `serde_json` 1, `base64` 0.23, `tokio-util` 0.7 with
  the `codec` feature, `tokio` 1 (`net`, `rt-multi-thread`, `macros`, `sync`, `time`, `io-util`).
- `probe-wire/src/lib.rs`: `mod` and `pub use` lines only (`.claude/rules/rust.md:23`).
- `probe-wire/src/frame.rs`: the two framings behind one trait: F1 newline-delimited JSON,
  F2 length-prefixed (`tokio_util::codec::LengthDelimitedCodec` with a 4-byte big-endian
  length) carrying a small header (stream id, kind) and then either JSON or opaque bytes.
- `probe-wire/src/hello.rs`: the handshake and the version rule (see below), including the
  error frame that names both versions.
- `probe-wire/src/control.rs`: the control envelope: a request with an id, a method name and
  JSON parameters; a response or an error; and a server-initiated event, so a long-lived
  subscription (`event watch`, doc-2 section 8) does not block requests on the same
  connection.
- `probe-wire/src/snapshot.rs`: the plain-data snapshot: `Snapshot`, `Row`, `Cell` (char,
  foreground, background, flags), cursor, modes, scrollback rows, and the sequence number the
  stream resumes from. Deliberately not `Send`-dependent, so it fits either emulator.
- `probe-wire/src/delta.rs`: the D2 messages: changed rows, scroll, scrollback append, cursor
  move, and the "resynchronise from this snapshot" message both candidates need.
- `probe-wire/tests/framing.rs`, `probe-wire/tests/version.rs`: the mechanical tests (see
  Proof).
- `probe-server/Cargo.toml`: `probe-wire`, `alacritty_terminal` 0.26, `portable-pty` 0.9
  (MIT, the same pty crate TASK-1.1 chose, so the two spikes do not disagree), `tokio`.
- `probe-server/src/pty.rs`: open a pty, spawn a program, read its output.
- `probe-server/src/term.rs`: one thread owns one terminal; bytes in over a channel,
  snapshots and deltas out, with the snapshot and the sequence number produced in one step.
- `probe-server/src/attach.rs`: attachments. A bounded queue per attachment, a round-robin
  writer with a maximum frame size (this is where topology T1's fairness lives), and the
  overflow policy per candidate.
- `probe-server/src/socket.rs`: a Unix listener, and the same service over a pipe pair for
  the stream-transport gate.
- `probe-server/src/main.rs`: the binary, taking the framing and data candidate as arguments.
- `probe-client/src/grid.rs`: a plain grid that applies a snapshot and D2's deltas, with no
  emulator.
- `probe-client/src/replay.rs`: D1's client-side emulator, applying raw bytes onto the
  snapshot.
- `probe-client/src/capture.rs`: render the client's screen and scrollback to text, so two
  clients can be compared byte for byte.
- `probe-client/src/main.rs`: attach, type, resize, read slowly on demand, dump the capture,
  exit.
- `client.py`: a PEP 723 script that makes a control call, reads the description payload, and
  attaches to a pane, knowing nothing of Rust (gate 1).
- `reattach-demo`: an executable script that runs AC #2's demonstration end to end in one
  command and prints its own verdict.
- `measure`: an executable script that runs the flood, latency and snapshot-size
  measurements for each candidate and prints them as tables.

Not touched, deliberately: the root (`Cargo.toml` and `Cargo.lock` are TASK-1.5's), the
`justfile` (TASK-1.5 is editing it, and `steg-affected` treats it as a file that affects every
crate), `tools/`, `.gitignore`, and `backlog/` by hand.

Board artefacts, created with `asdlc` from the tasks clone and so not in this branch's diff:
a new document from `asdlc board doc create` holding the answer, linked with `asdlc board task
edit TASK-1.2 --add-doc <doc-id>`, and the task's notes.

## AC #2's demonstration, and its oracle

Defined before it is built, so the comparison cannot be chosen to pass:

1. The server runs a live pty whose program prints continuously and deterministically:
   numbered lines at a steady rate, so a gap or a repeat is visible on sight. The run is long
   enough to push more than 10,000 lines through, so scrollback is full and its limit (R15) is
   exercised.
2. Client A attaches, is confirmed to be showing live output, and is **SIGKILLed** mid-output.
   Not a clean detach: an abrupt kill is what makes this a reattach test, and it also proves
   the server survives the broken pipe without dying or stalling the pty.
3. A gap with **no client attached at all** while the program keeps printing.
4. Client B attaches and reconstructs. The program is then quiesced (it stops printing and the
   server's queues drain).
5. The oracle: client B's rendered screen and scrollback are compared byte for byte against
   the server's own capture of the same pane at the same quiesced point, and the lines printed
   during the no-client gap are asserted present and unrepeated. A mismatch prints both texts
   and their first differing line.

One script, no manual steps, prints its own verdict and exits by itself, so the reviewer
re-runs it under `asdlc record-check`. Run for both data candidates, and the document shows
the transcript of each.

An extra case in the same script, for gate 3: two clients attached at once, each with its own
size, one killed, and the other asserted to carry on unaffected.

## AC #3: how the protocol carries a version

The discriminating constraint, which is easy to get wrong: **the first frame must be parseable
by every future version**, or TASK-1.9's "a message naming both versions" (its AC #4, R39)
becomes a parse error instead of a message. So the document states, and the spike proves:

- The handshake's wire shape, and what is frozen forever: the length prefix's width and
  endianness, that the first frame's payload is UTF-8 JSON, the two keys that must be in it
  (a magic and an integer protocol version), and that unknown keys are ignored.
- The rule for when a client and a server work together rather than fail: the proposal to
  test is that the integer must match exactly for framing, control shape and the data
  representation, because they ship in one binary (R61) and a mismatch is not partly
  recoverable; while the API's own version, carried in the self-description (TASK-1.3), is what
  lets a client be newer in methods without a framing mismatch. The document says whether the
  spike confirms or replaces that proposal.
- Whether the control protocol and the terminal data stream version together or separately,
  with the reason.

Proved by `probe-wire/tests/version.rs`: a client declaring the version above the server's
gets an error naming both and no hang; one below likewise; a first frame that is not valid
JSON, is truncated, or is enormous gets an error rather than a panic or an unbounded
allocation.

## Order of work

1. **Record the starting point.** `asdlc record-check TASK-1.2 --in
   /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.2 --as bruk.w4 -- just check`.
   Every Rust step is expected to say it is skipped and the tools' tests to pass; that
   expectation is itself the check that this branch stayed clear of TASK-1.5, and the notes
   say so. Confirm `just affected` still answers `none` for a change under `spikes/`.
2. **The spike workspace.** Manifests with the bare `[workspace]` table, README, a build that
   compiles, `Cargo.lock` committed.
3. **`probe-wire`: framing and the handshake.** F1 and F2 behind one trait, the control
   envelope, the hello, and the two test files. These are written and passing before anything
   touches a pty, because they are the only part with a mechanical right answer.
4. **The pty and the terminal thread.** One thread owning an `alacritty_terminal` instance,
   bytes in over a channel, and a snapshot with its sequence number out in one step.
5. **Attachments and the socket.** Bounded per-attachment queues, the round-robin writer with
   a maximum frame size, and the Unix listener; then the same service over a pipe pair.
6. **Candidate D1.** Snapshot on attach, raw bytes after, the client-side emulator, and the
   resync path a slow client forces.
7. **Candidate D2.** The same attach, row-diffed deltas after, and the plain grid in the
   client with no emulator.
8. **`reattach-demo`,** for both candidates, including the two-clients case. This is AC #2.
9. **`client.py`,** for gate 1, against the chosen framing and both data candidates as far as
   each allows; if a Python client cannot reasonably consume one of them, that is a finding
   against that candidate and is recorded as one.
10. **`measure`:** the 100 MB flood (bytes on the wire, control-call and other-pane keystroke
    latency during it, whether the program stalls behind a slow or dead client, client catch-up
    time, peak server memory per attachment), the attach cost with 10,000 lines of scrollback
    (snapshot bytes and build time, and the same for a paged scrollback request), and
    byte-into-pty-to-grid-updated latency as p50/p95/p99/max. Three runs of each, all three
    reported. F1 against F2 on the flood; D1 against D2 throughout.
11. **Topology T2 on paper.** Weigh the ticket-and-second-connection shape against T1's
    measured head-of-line numbers, what a second connection costs over one SSH stream (R55),
    and what a ticket's lifetime and second handshake add. Build it only if T1 misses
    threshold 7; say which happened.
12. **Licence inventory.** `cargo tree -e normal --format "{p} {l}"` in each probe crate,
    pasted into the document, with the licences TASK-1.5's `deny.toml` must allow. `cargo-deny`
    is not installed and there is no root workspace, so `just deny` cannot be the proof here and
    the document says which was used.
13. **Write the answer.** `asdlc board doc create`, then `asdlc board doc update <id>
    --content` with: the decision on each of the four axes and the reasons in order of weight;
    the measurement tables; the reattach transcripts; the back-pressure policy; the version
    handshake and what is frozen; the licence inventory; and what each sibling task inherits
    (below). Link it with `asdlc board task edit TASK-1.2 --add-doc <id>`.
14. **Finish.** Commit with the trailer `Asdlc-Worker: bruk.w4`, `git fetch origin`, rebase or
    merge the default branch as the finish rules say, re-run `just check` through
    `asdlc record-check`, push the branch, open the pull request on `bruk-io/steg`, and
    `asdlc finish` with the numbers in the notes.

## Risks

- **Four axes in a size-3 spike.** The control is in what is built and what is not: two
  framings (cheap, the payload is pluggable), two data candidates (the real question), one
  topology measured and one weighed on paper (step 11). If step 7's delta computation starts
  growing into a damage-tracking project, stop: the row diff is the whole of it, and what a
  damage API would save is a sentence in the document, not code.
- **The stand-in emulator biases D2.** Mitigated by diffing snapshots rather than using one
  library's damage API, and by stating which findings are emulator-independent.
- **Localhost says little about SSH.** Mitigated by the pipe-proxy run and the one-byte-at-a-time
  decode, which are what gate 2 actually tests. If `ssh localhost` works on this machine one run
  goes through it; if it does not, the document says so rather than implying it was tested.
- **The latency number is not photon-to-glass.** It stops at the client's grid, because the
  drawing half is TASK-1.1's measurement. The document says so beside the number, and says that
  R19's 16 ms has to cover both halves added together.
- **A dead client must not stall the pty.** A SIGKILL mid-write means an EPIPE in the middle of
  a frame. Step 8's kill covers it; if the server stalls or dies, that is the spike's first
  finding, not a bug to paper over.
- **An unbounded first frame.** A hello that declares a huge length would let any local process
  exhaust memory before the version is even read. The handshake caps it, and
  `probe-wire/tests/version.rs` proves the cap.
- **Colliding with the siblings.** TASK-1.1 and TASK-1.3 add their own directories under
  `spikes/` and TASK-1.5 adds the root workspace. Nothing here is shared, so the merge at step
  14 should be clean; if it conflicts at all, something went wrong, so stop and say so.
- **`tokio` in a spike.** The kernel must never depend on an async runtime
  (`.claude/rules/architecture.md:17`), and it does not: the runtime is in the shell that owns
  the socket and the pty, which is what `.claude/rules/rust.md:28` prescribes. The document
  repeats this so TASK-1.12 does not read the spike as licence to push async into the kernel.
- **A departure from `.claude/rules/testing-rust.md:15`**, which says to run tests through
  `just test` and never `cargo test` directly. That rule governs crates under `just`'s control;
  this crate is deliberately outside the workspace, and adding a justfile recipe would both
  collide with TASK-1.5 and make `steg-affected` select every crate. The spike's tests run with
  `cargo test --manifest-path`, recorded through `asdlc record-check` so the evidence still
  comes from the toolchain. Noted on the task, as TASK-1.3 did the same.
- **Licence contamination.** tmux, mosh and Zed's terminal crates solve this exact problem and
  are copyleft. They are not opened during this task; the document records every source the
  probe code was written from.

## Proof

Each recorded with `asdlc record-check TASK-1.2 --in
/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.2 --as bruk.w4 -- <command>`.

- **The repository's own check**, at step 1 and again at step 14: `just check`. Expected to
  skip every Rust step both times, because this task adds no workspace crate.
- **AC #1** (a document recording the framing and how terminal output reaches a client, with
  the reason): the document exists and is linked on the task, and decides each of the four
  axes. Its reasons are the recorded output of `measure` (step 10), the gates in the decision
  rule each proved by a run (`client.py`, the pipe-proxy run, the one-byte decode test, the
  two-clients case, `cargo tree`), and the paper case for T2 with what would overturn it.
- **AC #2** (a client killed mid-output and a new client restoring the same screen and
  scrollback): `reattach-demo`, run for each data candidate, with its transcript in the
  document. It passes only when client B's rendered screen and scrollback match the server's
  own capture byte for byte at a quiesced point and the no-client-attached lines are present
  and unrepeated. The kill is a SIGKILL; a clean detach does not count.
- **AC #3** (how the protocol carries a version, for R39): the document's handshake section,
  with `cargo test --manifest-path spikes/task-1.2-wire-protocol/Cargo.toml` covering
  `probe-wire/tests/version.rs`: higher and lower versions each get an error naming both
  versions, and a malformed, truncated or oversized first frame gets an error rather than a
  panic or an unbounded allocation.
- **The framing tests** in the same run: a captured stream decodes identically when fed one
  byte at a time, in random chunk sizes, and in one piece; and a frame at the size cap is
  accepted while one above it is refused. For TASK-1.3's payload contract the property tested
  is not byte equality between F1 and F2, which differ by F1's newline and so would prove
  nothing: it is that the JSON **object** a client parses out of a control frame is identical
  under both framings, and that `client.py` reads the same self-description payload through
  either one.

## Rules the probe binaries follow

`asdlc record-check` runs these with nobody present, and the reviewer re-runs them, so:

- Each binary and script takes what to run as arguments, runs it, prints its result, and exits
  by itself. Nothing waits to be stopped by hand, and every run has a timeout after which it
  fails loudly.
- Every measurement is deterministic apart from the timings: the same program in the pty, the
  same fixture, the same terminal size, the same sample count.
- Each prints the machine, the crate versions, the build profile and the candidate above its
  table, so a number cannot be read out of its context.

## What each sibling task inherits from the answer

Written into the document so nobody has to read this plan:

- **TASK-1.3**: the framing that replaces its newline-delimited JSON stand-in, and whether its
  JSON self-description payload survives unchanged under it.
- **TASK-1.9**: the handshake's wire shape, what is frozen forever, and the mismatch message
  its AC #4 has to produce.
- **TASK-1.12**: the attach sequence, the snapshot's contents and its sequence number, the
  atomicity requirement, the back-pressure policy, and what a client must implement.
- **TASK-1.5**: the licences its `deny.toml` must allow for whichever crates the answer needs.
- **TASK-1.1**: where the protocol's cost depends on the library choice, stated as a
  dependency rather than a preference.

## Left for the engineer

AC #1 reads as a binary choice ("raw bytes that a client-side emulator replays, or screen
updates the server computes"). AC #2 removes that choice from one half of it: output printed
while **no client was attached** has no byte stream left to replay, so a snapshot on attach is
forced, in every candidate, by the acceptance criteria themselves and not by a preference of
this spike's. AC #1's two options are therefore two choices of what **follows** the snapshot:
raw bytes onto it, or deltas against it. The plan is written that way, so the answer will be a
hybrid in AC #1's terms however the measurements fall.

What is asked of the engineer is only whether that satisfies AC #1 as worded, or whether the
criterion's text should be read with this note beside it, as TASK-1.1's AC #1 was. Nothing in
the plan changes either way.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Onboarding skipped by @bruk.o: bruk-io-steg has no repository plugin yet and no code to read layers or verify commands from. TASK-1.24 onboards it once the workspace exists. The repository's own rules are in .claude/rules/ and its commands are in the justfile.

Facts verified by @bruk.w4 while planning (2026-11-13/14), so the builder does not rediscover them:

- Crate versions and licences read from the crates.io API while planning: tokio 1.54.1 (MIT), tokio-util 0.7.20 (MIT, the `codec` feature holds LengthDelimitedCodec), serde 1.0.229 and serde_json 1.0.154 (MIT OR Apache-2.0), base64 0.23.1 (MIT OR Apache-2.0), rmp-serde 1.3.1 (MIT, only if MessagePack is weighed), portable-pty 0.9.0 (MIT, the same pty crate TASK-1.1 chose), alacritty_terminal 0.26.0 (Apache-2.0). All permissive; none GPL or AGPL (R59). The licence inventory at build time is still `cargo tree -e normal --format "{p} {l}"` read by hand, because cargo-deny is not installed and there is no root workspace yet.
- The server holds the emulator in every candidate, and this spike does not get to choose otherwise: R15 keeps 10,000 lines of scrollback on the server, `pane capture` (doc-2 section 8) renders a pane's screen and scrollback as text with no client attached, a Python plugin on the public API (R42, R75) cannot replay VT bytes, and .claude/rules/architecture.md:11 says a client never keeps a copy it treats as the truth. So the open question is what the server puts on the wire, not who emulates. This reading is @bruk.w4's and is in the plan for the engineer to check.
- doc-2 section 8 has `event watch` streaming one record per line and `pane capture` returning text, so the control protocol has to carry server-initiated messages and a long-lived subscription on the same connection as requests. That is a framing requirement, not a server detail.
- `just check` in this worktree skips every Rust step while there is no root Cargo.toml (justfile:12 gates them on `path_exists("Cargo.toml")`). This task must not create one: `just deny` would then run with no deny.toml and fail. TASK-1.5 owns the root manifest.
- The spike's tests run with `cargo test --manifest-path`, a stated departure from .claude/rules/testing-rust.md:15, for the same reason TASK-1.3 recorded: the crate is deliberately outside the workspace, and a justfile recipe would collide with TASK-1.5 and make steg-affected select every crate. Every run is recorded through `asdlc record-check`.

Plan checked against doc-1 by @bruk.o: ok. It serves R1, R2, R36 and R39, keeps R7 and R55 open through its gates (two clients attached at once, and a byte stream it does not control), and proves the protocol is usable from Python as R42 and R75 need. Its starting point, that the server holds the emulator in every candidate, follows from R15, R16, pane capture in doc-2 and .claude/rules/architecture.md, so the question is what goes on the wire. The decision rule and the reattach test are fixed before any measurement. It leaves the emulation library, the API description, the CLI and the server's lifecycle to their own tasks, using alacritty_terminal only as a declared instrument. Put to Bruk with the plan: acceptance criterion 1 reads as a choice between two, and the answer will be a snapshot on attach plus one of them, whichever way the measurements fall.

Check passed: `just check` exited 0 at 2026-11-14T03:56:54+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.2 on task-1.2-spike-choose-the-wire-protocol-by at e54403d1e1a6:

```text
skip    format check: no Rust workspace yet
skip    lint: no Rust workspace yet
skip    licences: no Rust workspace yet
skip    tests: no Rust workspace yet
ok      tool tests (0.4s)
```

Check passed: `cargo test --manifest-path spikes/task-1.2-wire-protocol/probe-wire/Cargo.toml` exited 0 at 2026-11-14T04:04:40+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.2 on task-1.2-spike-choose-the-wire-protocol-by at e54403d1e1a6 with uncommitted changes:

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.02s
     Running unittests src/lib.rs (spikes/task-1.2-wire-protocol/target/debug/deps/probe_wire-7a61279e5ecc4502)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests/framing.rs (spikes/task-1.2-wire-protocol/target/debug/deps/framing-fa3dd66e41e37a8c)

running 7 tests
test a_declared_length_over_the_cap_is_refused_without_allocating_it ... ok
test bytes_that_are_not_a_frame_are_refused_rather_than_guessed_at ... ok
test a_partial_frame_asks_for_more_rather_than_failing ... ok
test the_json_object_a_client_parses_is_the_same_under_both_framings ... ok
test a_stream_decodes_the_same_whatever_the_chunk_boundaries_are ... ok
test a_snapshot_round_trips_through_a_control_frame ... ok
test a_frame_at_the_cap_is_accepted_and_one_over_it_is_refused ... ok

test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.27s

     Running tests/version.rs (spikes/task-1.2-wire-protocol/target/debug/deps/version-3749561cbf83f3c3)

running 6 tests
test a_newer_client_is_refused_with_both_versions_named ... ok
test an_older_client_is_refused_with_both_versions_named ... ok
test a_refusal_travels_as_a_frame_a_client_of_any_version_can_read ... ok
test a_hello_that_cannot_be_read_is_an_error_naming_the_servers_version ... ok
test the_same_version_is_accepted ... ok
test an_unknown_field_in_a_hello_is_ignored_so_a_later_version_can_add_one ... ok

test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests probe_wire

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

Check passed: `spikes/task-1.2-wire-protocol/reattach-demo --mode deltas` exited 0 at 2026-11-14T04:32:01+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.2 on task-1.2-spike-choose-the-wire-protocol-by at e54403d1e1a6 with uncommitted changes:

```text
== reattach demonstration: mode=deltas framing=length lines=12000 interval=350us queue=64
Darwin 27.0.0
-- step 1: a pane prints 12000 numbered lines
handshake server=probe-server/0.0.0 protocol=1 api=0.1.0 framing=length
{"pane":1}
-- step 2: client A attaches and is killed mid-output
   client A ready: pane=1 attachment=1 mode=deltas seq=10 snapshot_bytes=3335 scrollback_rows=0
handshake server=probe-server/0.0.0 protocol=1 api=0.1.0 framing=length
   client A killed with SIGKILL while the pane was at line 3394
-- step 3: nothing is attached, and the program keeps printing
handshake server=probe-server/0.0.0 protocol=1 api=0.1.0 framing=length
   the pane reached line 6667 with no client attached
-- step 4: client B attaches
   client B ready: pane=1 attachment=2 mode=deltas seq=6698 snapshot_bytes=897920 scrollback_rows=6676
-- step 5: the program finishes, everything quiesces, and the two are compared
   client B: frames=296 bytes_read=1795397 resyncs=0 seq=11999 unknown_stream_frames=0
handshake server=probe-server/0.0.0 protocol=1 api=0.1.0 framing=length
capture pane=1 bytes=120277 lines=10024 last=12000
same  the reattached client and the server agree byte for byte (120277 bytes, 10024 lines)
whole 10023 numbered lines run from 1978 to 12000 with no gap and no repeat
last  the last line is 12000, as the program printed
gap   every line from 3394 to 6667, printed while no client was attached, is there
PASS
-- the server's counters
handshake server=probe-server/0.0.0 protocol=1 api=0.1.0 framing=length
{"panes":[{"attachments":0,"blocked_nanos":0,"overflows":0,"pane":1,"pty_bytes":156000,"resyncs":0,"ticks":1279,"wire_bytes":4108857,"wire_frames":498}]}
-- two clients on one pane at once, each with its own size, while output is flowing (R7 is left open)
handshake server=probe-server/0.0.0 protocol=1 api=0.1.0 framing=length
{"pane":2}
handshake server=probe-server/0.0.0 protocol=1 api=0.1.0 framing=length
{"attachments":[{"attachment":3,"cols":80,"rows":24},{"attachment":4,"cols":120,"rows":40}]}
   one of the two was killed with SIGKILL
   the client that was not killed: frames=310 bytes_read=1664258 resyncs=0 seq=4000 unknown_stream_frames=0
   it kept receiving output after the other was killed (310 frames)
handshake server=probe-server/0.0.0 protocol=1 api=0.1.0 framing=length
   and its screen and scrollback still match the server's
```

Check passed: `spikes/task-1.2-wire-protocol/reattach-demo --mode bytes` exited 0 at 2026-11-14T04:32:23+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.2 on task-1.2-spike-choose-the-wire-protocol-by at e54403d1e1a6 with uncommitted changes:

```text
== reattach demonstration: mode=bytes framing=length lines=12000 interval=350us queue=64
Darwin 27.0.0
-- step 1: a pane prints 12000 numbered lines
handshake server=probe-server/0.0.0 protocol=1 api=0.1.0 framing=length
{"pane":1}
-- step 2: client A attaches and is killed mid-output
   client A ready: pane=1 attachment=1 mode=bytes seq=8 snapshot_bytes=3333 scrollback_rows=0
handshake server=probe-server/0.0.0 protocol=1 api=0.1.0 framing=length
   client A killed with SIGKILL while the pane was at line 3412
-- step 3: nothing is attached, and the program keeps printing
handshake server=probe-server/0.0.0 protocol=1 api=0.1.0 framing=length
   the pane reached line 6664 with no client attached
-- step 4: client B attaches
   client B ready: pane=1 attachment=2 mode=bytes seq=6702 snapshot_bytes=898322 scrollback_rows=6679
-- step 5: the program finishes, everything quiesces, and the two are compared
   client B: frames=5298 bytes_read=116556 resyncs=0 seq=6702 unknown_stream_frames=0
handshake server=probe-server/0.0.0 protocol=1 api=0.1.0 framing=length
capture pane=1 bytes=120277 lines=10024 last=12000
same  the reattached client and the server agree byte for byte (120277 bytes, 10024 lines)
whole 10023 numbered lines run from 1978 to 12000 with no gap and no repeat
last  the last line is 12000, as the program printed
gap   every line from 3412 to 6664, printed while no client was attached, is there
PASS
-- the server's counters
handshake server=probe-server/0.0.0 protocol=1 api=0.1.0 framing=length
{"panes":[{"attachments":0,"blocked_nanos":0,"overflows":0,"pane":1,"pty_bytes":156000,"resyncs":0,"ticks":1277,"wire_bytes":1355746,"wire_frames":8732}]}
-- two clients on one pane at once, each with its own size, while output is flowing (R7 is left open)
handshake server=probe-server/0.0.0 protocol=1 api=0.1.0 framing=length
{"pane":2}
handshake server=probe-server/0.0.0 protocol=1 api=0.1.0 framing=length
{"attachments":[{"attachment":4,"cols":120,"rows":40},{"attachment":3,"cols":80,"rows":24}]}
   one of the two was killed with SIGKILL
   the client that was not killed: frames=3994 bytes_read=87868 resyncs=0 seq=6 unknown_stream_frames=0
   it kept receiving output after the other was killed (3994 frames)
handshake server=probe-server/0.0.0 protocol=1 api=0.1.0 framing=length
   and its screen and scrollback still match the server's
```

Check passed: `spikes/task-1.2-wire-protocol/reattach-demo --mode deltas --framing ndjson` exited 0 at 2026-11-14T04:32:42+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.2 on task-1.2-spike-choose-the-wire-protocol-by at e54403d1e1a6 with uncommitted changes:

```text
== reattach demonstration: mode=deltas framing=ndjson lines=12000 interval=350us queue=64
Darwin 27.0.0
-- step 1: a pane prints 12000 numbered lines
handshake server=probe-server/0.0.0 protocol=1 api=0.1.0 framing=ndjson
{"pane":1}
-- step 2: client A attaches and is killed mid-output
   client A ready: pane=1 attachment=1 mode=deltas seq=7 snapshot_bytes=3333 scrollback_rows=0
handshake server=probe-server/0.0.0 protocol=1 api=0.1.0 framing=ndjson
   client A killed with SIGKILL while the pane was at line 3380
-- step 3: nothing is attached, and the program keeps printing
handshake server=probe-server/0.0.0 protocol=1 api=0.1.0 framing=ndjson
   the pane reached line 6643 with no client attached
-- step 4: client B attaches
   client B ready: pane=1 attachment=2 mode=deltas seq=6685 snapshot_bytes=896044 scrollback_rows=6662
-- step 5: the program finishes, everything quiesces, and the two are compared
   client B: frames=301 bytes_read=1819477 resyncs=0 seq=12000 unknown_stream_frames=0
handshake server=probe-server/0.0.0 protocol=1 api=0.1.0 framing=ndjson
capture pane=1 bytes=120277 lines=10024 last=12000
same  the reattached client and the server agree byte for byte (120277 bytes, 10024 lines)
whole 10023 numbered lines run from 1978 to 12000 with no gap and no repeat
last  the last line is 12000, as the program printed
gap   every line from 3380 to 6643, printed while no client was attached, is there
PASS
-- the server's counters
handshake server=probe-server/0.0.0 protocol=1 api=0.1.0 framing=ndjson
{"panes":[{"attachments":0,"blocked_nanos":0,"overflows":0,"pane":1,"pty_bytes":156000,"resyncs":0,"ticks":1283,"wire_bytes":4139578,"wire_frames":504}]}
-- two clients on one pane at once, each with its own size, while output is flowing (R7 is left open)
handshake server=probe-server/0.0.0 protocol=1 api=0.1.0 framing=ndjson
{"pane":2}
handshake server=probe-server/0.0.0 protocol=1 api=0.1.0 framing=ndjson
{"attachments":[{"attachment":4,"cols":120,"rows":40},{"attachment":3,"cols":80,"rows":24}]}
   one of the two was killed with SIGKILL
   the client that was not killed: frames=311 bytes_read=1676942 resyncs=0 seq=4000 unknown_stream_frames=0
   it kept receiving output after the other was killed (311 frames)
handshake server=probe-server/0.0.0 protocol=1 api=0.1.0 framing=ndjson
   and its screen and scrollback still match the server's
```

Check passed: `spikes/task-1.2-wire-protocol/gates` exited 0 at 2026-11-14T04:35:27+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.2 on task-1.2-spike-choose-the-wire-protocol-by at e54403d1e1a6 with uncommitted changes:

```text
== gates: framing=length
Darwin 27.0.0
-- the socket is the owning user's only (R5), and there is no network port
   srw-------@ 1 brukhabtu  staff  0 Nov 13 23:35 /var/folders/rf/f34xmczn4659gm98hrsprydc0000gn/T//steg-gates.OitV5k/socket
   the server's open sockets:
   lsof is not available here
-- gate 1: Python, no Rust, no emulator
handshake server=probe-server/0.0.0 protocol=1 api=0.1.0
describe  protocol=1 api=0.1.0
describe  12 methods: describe, pane.open, pane.list, pane.capture, pane.scrollback, pane.attach, pane.detach, pane.input, pane.resize, pane.attachments, server.stats, server.shutdown
pane      1
attach    attachment=1 stream=1 seq=0 80x24 scrollback_rows=0
updates   deltas=189 resyncs=0 rows_held=3001
agree     36001 bytes, 3001 lines, byte for byte
   gate 1 passed
-- gate 2: the protocol over a pipe pair through a relay it does not control
   {"api":"0.1.0","methods":[{"name":"describe","params":[],"summary":"Every method, its parameters and its documentation"}
   a control call travelled over two pipes and a process in the middle
   and so did a whole attachment, snapshot and updates
-- gate 2, over SSH to this machine, when one answers
   not tested: ssh to localhost is not available here
   ssh: connect to host localhost port 22: Connection refused
   the pipe-pair run above is the evidence for R55's shape instead
-- AC #3: a mismatched protocol version
   client 2: probe-client: steg protocol mismatch: the client speaks version 2 and this server speaks version 1
   client 0: probe-client: steg protocol mismatch: the client speaks version 0 and this server speaks version 1
-- AC #3: a first frame the server cannot read
   not-json: {"steg":"steg","error":"malformed-hello","client_protocol":null,"server_protocol":1,"message":"steg could not read the client's hello (expected value at line 1 column 1); this server speaks protocol version 1 and the client's version is unknown"}
   wrong-magic: {"steg":"steg","error":"malformed-hello","client_protocol":null,"server_protocol":1,"message":"steg could not read the client's hello (the magic is \"ssh\", not \"steg\"); this server speaks protocol version 1 and the client's version is unknown"}
   not-hello: {"steg":"steg","error":"malformed-hello","client_protocol":null,"server_protocol":1,"message":"steg could not read the client's hello (the first frame is a Control frame, not a hello); this server speaks protocol version 1 and the client's version is unknown"}
   huge: the server closed the connection without answering
-- and the server is still serving after all of that
   handshake server=probe-server/0.0.0 protocol=1 api=0.1.0 framing=length
   {"panes":[{"attachments":0,"blocked_nanos":0,"overflows":0,"pane":1,"pty_bytes":39000,"resyncs":0,"ticks":405,"wire_bytes":15382
PASS  every gate
```

Check FAILED: `spikes/task-1.2-wire-protocol/gates --framing ndjson` exited 1 at 2026-11-14T04:35:52+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.2 on task-1.2-spike-choose-the-wire-protocol-by at e54403d1e1a6 with uncommitted changes:

```text
== gates: framing=ndjson
Darwin 27.0.0
-- the socket is the owning user's only (R5), and there is no network port
   srw-------@ 1 brukhabtu  staff  0 Nov 13 23:35 /var/folders/rf/f34xmczn4659gm98hrsprydc0000gn/T//steg-gates.fAMSjw/socket
   the server's open sockets:
   lsof is not available here
-- gate 1: Python, no Rust, no emulator
   skipped: client.py implements the length framing only, which is the candidate
handshake server=probe-server/0.0.0 protocol=1 api=0.1.0 framing=ndjson
-- gate 2: the protocol over a pipe pair through a relay it does not control
   {"api":"0.1.0","methods":[{"name":"describe","params":[],"summary":"Every method, its parameters and its documentation"}
   a control call travelled over two pipes and a process in the middle
   and so did a whole attachment, snapshot and updates
-- gate 2, over SSH to this machine, when one answers
   not tested: ssh to localhost is not available here
   ssh: connect to host localhost port 22: Connection refused
   the pipe-pair run above is the evidence for R55's shape instead
-- AC #3: a mismatched protocol version
   client 2: probe-client: steg protocol mismatch: the client speaks version 2 and this server speaks version 1
   client 0: probe-client: steg protocol mismatch: the client speaks version 0 and this server speaks version 1
-- AC #3: a first frame the server cannot read
   probe-client: not-json: the server never answered and never closed: it hung
FAIL  the server mishandled a not-json first frame
   probe-client: wrong-magic: the server never answered and never closed: it hung
FAIL  the server mishandled a wrong-magic first frame
   probe-client: not-hello: the server never answered and never closed: it hung
FAIL  the server mishandled a not-hello first frame
   probe-client: huge: the server never answered and never closed: it hung
FAIL  the server mishandled a huge first frame
-- and the server is still serving after all of that
   handshake server=probe-server/0.0.0 protocol=1 api=0.1.0 framing=ndjson
   {"panes":[{"attachments":0,"blocked_nanos":0,"overflows":0,"pane":1,"pty_bytes":39000,"resyncs":0,"ticks":2293,"wire_bytes":1051
FAIL  at least one gate
```

Check passed: `spikes/task-1.2-wire-protocol/gates` exited 0 at 2026-11-14T04:37:43+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.2 on task-1.2-spike-choose-the-wire-protocol-by at e54403d1e1a6 with uncommitted changes:

```text
== gates: framing=length
Darwin 27.0.0
-- the socket is the owning user's only (R5), and there is no network port
   srw-------@ 1 brukhabtu  staff  0 Nov 13 23:37 /var/folders/rf/f34xmczn4659gm98hrsprydc0000gn/T//steg-gates.vrGf7P/socket
   the server's open sockets:
   lsof is not available here
-- gate 1: Python, no Rust, no emulator
handshake server=probe-server/0.0.0 protocol=1 api=0.1.0
describe  protocol=1 api=0.1.0
describe  12 methods: describe, pane.open, pane.list, pane.capture, pane.scrollback, pane.attach, pane.detach, pane.input, pane.resize, pane.attachments, server.stats, server.shutdown
pane      1
attach    attachment=1 stream=1 seq=0 80x24 scrollback_rows=0
updates   deltas=190 resyncs=0 rows_held=3001
agree     36001 bytes, 3001 lines, byte for byte
   gate 1 passed
-- gate 2: the protocol over a pipe pair through a relay it does not control
   {"api":"0.1.0","methods":[{"name":"describe","params":[],"summary":"Every method, its parameters and its documentation"}
   a control call travelled over two pipes and a process in the middle
   and so did a whole attachment, snapshot and updates
-- gate 2, over SSH to this machine, when one answers
   not tested: ssh to localhost is not available here
   ssh: connect to host localhost port 22: Connection refused
   the pipe-pair run above is the evidence for R55's shape instead
-- AC #3: a mismatched protocol version
   client 2: probe-client: steg protocol mismatch: the client speaks version 2 and this server speaks version 1
   client 0: probe-client: steg protocol mismatch: the client speaks version 0 and this server speaks version 1
-- AC #3: a first frame the server cannot read
   not-json: {"steg":"steg","error":"malformed-hello","client_protocol":null,"server_protocol":1,"message":"steg could not read the client's hello (expected value at line 1 column 1); this server speaks protocol version 1 and the client's version is unknown"}
   wrong-magic: {"steg":"steg","error":"malformed-hello","client_protocol":null,"server_protocol":1,"message":"steg could not read the client's hello (the magic is \"ssh\", not \"steg\"); this server speaks protocol version 1 and the client's version is unknown"}
   not-hello: {"steg":"steg","error":"malformed-hello","client_protocol":null,"server_protocol":1,"message":"steg could not read the client's hello (the first frame is a Control frame, not a hello); this server speaks protocol version 1 and the client's version is unknown"}
   huge: the server closed the connection without answering
-- a finding, not a gate: a client speaking the other framing
   wrong-framing: the server closed the connection without answering
   whichever way that goes, the handshake needs a timeout of its own: TASK-1.9's to add
-- and the server is still serving after all of that
   handshake server=probe-server/0.0.0 protocol=1 api=0.1.0 framing=length
   {"panes":[{"attachments":0,"blocked_nanos":0,"overflows":0,"pane":1,"pty_bytes":39000,"resyncs":0,"ticks":409,"wire_bytes":15409
PASS  every gate
```

Check passed: `spikes/task-1.2-wire-protocol/gates --framing ndjson` exited 0 at 2026-11-14T04:37:54+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.2 on task-1.2-spike-choose-the-wire-protocol-by at e54403d1e1a6 with uncommitted changes:

```text
== gates: framing=ndjson
Darwin 27.0.0
-- the socket is the owning user's only (R5), and there is no network port
   srw-------@ 1 brukhabtu  staff  0 Nov 13 23:37 /var/folders/rf/f34xmczn4659gm98hrsprydc0000gn/T//steg-gates.LLi8Qi/socket
   the server's open sockets:
   lsof is not available here
-- gate 1: Python, no Rust, no emulator
   skipped: client.py implements the length framing only, which is the candidate
handshake server=probe-server/0.0.0 protocol=1 api=0.1.0 framing=ndjson
-- gate 2: the protocol over a pipe pair through a relay it does not control
   {"api":"0.1.0","methods":[{"name":"describe","params":[],"summary":"Every method, its parameters and its documentation"}
   a control call travelled over two pipes and a process in the middle
   and so did a whole attachment, snapshot and updates
-- gate 2, over SSH to this machine, when one answers
   not tested: ssh to localhost is not available here
   ssh: connect to host localhost port 22: Connection refused
   the pipe-pair run above is the evidence for R55's shape instead
-- AC #3: a mismatched protocol version
   client 2: probe-client: steg protocol mismatch: the client speaks version 2 and this server speaks version 1
   client 0: probe-client: steg protocol mismatch: the client speaks version 0 and this server speaks version 1
-- AC #3: a first frame the server cannot read
   not-json: the server closed the connection without answering
   wrong-magic: {"stream":0,"kind":"hello","json":{"client_protocol":null,"error":"malformed-hello","message":"steg could not read the client's hello (the magic is \"ssh\", not \"steg\"); this server speaks protocol version 1 and the client's version is unknown","server_protocol":1,"steg":"steg"}}
   not-hello: {"stream":0,"kind":"hello","json":{"client_protocol":null,"error":"malformed-hello","message":"steg could not read the client's hello (the first frame is a Control frame, not a hello); this server speaks protocol version 1 and the client's version is unknown","server_protocol":1,"steg":"steg"}}
   huge: the server closed the connection without answering
-- a finding, not a gate: a client speaking the other framing
   wrong-framing: the server neither answered nor closed within 5s
   whichever way that goes, the handshake needs a timeout of its own: TASK-1.9's to add
-- and the server is still serving after all of that
   handshake server=probe-server/0.0.0 protocol=1 api=0.1.0 framing=ndjson
   {"panes":[{"attachments":0,"blocked_nanos":0,"overflows":0,"pane":1,"pty_bytes":39000,"resyncs":0,"ticks":837,"wire_bytes":10526
PASS  every gate
```

Check passed: `cargo test --manifest-path spikes/task-1.2-wire-protocol/probe-wire/Cargo.toml` exited 0 at 2026-11-14T05:00:46+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.2 on task-1.2-spike-choose-the-wire-protocol-by at e54403d1e1a6 with uncommitted changes:

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.02s
     Running unittests src/lib.rs (spikes/task-1.2-wire-protocol/target/debug/deps/probe_wire-4bd05bd20cc34a73)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests/framing.rs (spikes/task-1.2-wire-protocol/target/debug/deps/framing-91f245836523c78f)

running 7 tests
test a_declared_length_over_the_cap_is_refused_without_allocating_it ... ok
test bytes_that_are_not_a_frame_are_refused_rather_than_guessed_at ... ok
test a_partial_frame_asks_for_more_rather_than_failing ... ok
test the_json_object_a_client_parses_is_the_same_under_both_framings ... ok
test a_stream_decodes_the_same_whatever_the_chunk_boundaries_are ... ok
test a_snapshot_round_trips_through_a_control_frame ... ok
test a_frame_at_the_cap_is_accepted_and_one_over_it_is_refused ... ok

test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.28s

     Running tests/version.rs (spikes/task-1.2-wire-protocol/target/debug/deps/version-3c1a4f6200ff98d5)

running 6 tests
test a_newer_client_is_refused_with_both_versions_named ... ok
test an_older_client_is_refused_with_both_versions_named ... ok
test a_refusal_travels_as_a_frame_a_client_of_any_version_can_read ... ok
test a_hello_that_cannot_be_read_is_an_error_naming_the_servers_version ... ok
test the_same_version_is_accepted ... ok
test an_unknown_field_in_a_hello_is_ignored_so_a_later_version_can_add_one ... ok

test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests probe_wire

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

Check passed: `spikes/task-1.2-wire-protocol/reattach-demo --mode bytes` exited 0 at 2026-11-14T05:01:00+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.2 on task-1.2-spike-choose-the-wire-protocol-by at e54403d1e1a6 with uncommitted changes:

```text
== reattach demonstration: mode=bytes framing=length lines=12000 interval=350us queue=64
Darwin 27.0.0
-- step 1: a pane prints 12000 numbered lines
handshake server=probe-server/0.0.0 protocol=1 api=0.1.0 framing=length
{"pane":1}
-- step 2: client A attaches and is killed mid-output
   client A ready: pane=1 attachment=1 mode=bytes seq=9 snapshot_bytes=3347 scrollback_rows=0 attach_ms=0.4
handshake server=probe-server/0.0.0 protocol=1 api=0.1.0 framing=length
   client A killed with SIGKILL while the pane was at line 3394
-- step 3: nothing is attached, and the program keeps printing
handshake server=probe-server/0.0.0 protocol=1 api=0.1.0 framing=length
   the pane reached line 6669 with no client attached
-- step 4: client B attaches
   client B ready: pane=1 attachment=2 mode=bytes seq=6705 snapshot_bytes=898741 scrollback_rows=6682 attach_ms=29.6
-- step 5: the program finishes, everything quiesces, and the two are compared
   quiet pane=1 pty_bytes=156000 after=3.5s mb_per_s=0.1
   client B: why="it was told to stop" frames=5295 bytes_read=116490 resyncs=0 seq=6705 unknown_stream_frames=0 fetched_scrollback_rows=0
handshake server=probe-server/0.0.0 protocol=1 api=0.1.0 framing=length
capture pane=1 bytes=120277 lines=10024 last=12000
same  the reattached client and the server agree byte for byte (120277 bytes, 10024 lines)
whole 10023 numbered lines run from 1978 to 12000 with no gap and no repeat
last  the last line is 12000, as the program printed
gap   every line from 3394 to 6669, printed while no client was attached, is there
PASS
-- the server's counters
handshake server=probe-server/0.0.0 protocol=1 api=0.1.0 framing=length
{"panes":[{"attachments":0,"blocked_nanos":0,"overflows":0,"pane":1,"pty_bytes":156000,"resyncs":0,"ticks":783}],"wire_bytes":1362493,"wire_frames":8743}
-- two clients on one pane at once, each with its own size, while output is flowing (R7 is left open)
handshake server=probe-server/0.0.0 protocol=1 api=0.1.0 framing=length
{"pane":2}
handshake server=probe-server/0.0.0 protocol=1 api=0.1.0 framing=length
{"attachments":[{"attachment":4,"cols":120,"dropped":0,"mode":"bytes","overflowed":false,"rows":40,"sent":94},{"attachment":3,"cols":80,"dropped":0,"mode":"bytes","overflowed":false,"rows":24,"sent":95}]}
   one of the two was killed with SIGKILL
   the client that was not killed: why="the duration ran out" frames=3994 bytes_read=87868 resyncs=0 seq=6 unknown_stream_frames=0 fetched_scrollback_rows=0
   it kept receiving output after the other was killed (3994 frames)
handshake server=probe-server/0.0.0 protocol=1 api=0.1.0 framing=length
   and its screen and scrollback still match the server's
```

Check passed: `spikes/task-1.2-wire-protocol/reattach-demo --mode deltas` exited 0 at 2026-11-14T05:01:14+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.2 on task-1.2-spike-choose-the-wire-protocol-by at e54403d1e1a6 with uncommitted changes:

```text
== reattach demonstration: mode=deltas framing=length lines=12000 interval=350us queue=64
Darwin 27.0.0
-- step 1: a pane prints 12000 numbered lines
handshake server=probe-server/0.0.0 protocol=1 api=0.1.0 framing=length
{"pane":1}
-- step 2: client A attaches and is killed mid-output
   client A ready: pane=1 attachment=1 mode=deltas seq=9 snapshot_bytes=3347 scrollback_rows=0 attach_ms=0.3
handshake server=probe-server/0.0.0 protocol=1 api=0.1.0 framing=length
   client A killed with SIGKILL while the pane was at line 3375
-- step 3: nothing is attached, and the program keeps printing
handshake server=probe-server/0.0.0 protocol=1 api=0.1.0 framing=length
   the pane reached line 6601 with no client attached
-- step 4: client B attaches
   client B ready: pane=1 attachment=2 mode=deltas seq=6631 snapshot_bytes=888825 scrollback_rows=6608 attach_ms=20.9
-- step 5: the program finishes, everything quiesces, and the two are compared
   quiet pane=1 pty_bytes=156000 after=3.6s mb_per_s=0.1
   client B: why="it was told to stop" frames=305 bytes_read=1842797 resyncs=0 seq=12000 unknown_stream_frames=0 fetched_scrollback_rows=0
handshake server=probe-server/0.0.0 protocol=1 api=0.1.0 framing=length
capture pane=1 bytes=120277 lines=10024 last=12000
same  the reattached client and the server agree byte for byte (120277 bytes, 10024 lines)
whole 10023 numbered lines run from 1978 to 12000 with no gap and no repeat
last  the last line is 12000, as the program printed
gap   every line from 3375 to 6601, printed while no client was attached, is there
PASS
-- the server's counters
handshake server=probe-server/0.0.0 protocol=1 api=0.1.0 framing=length
{"panes":[{"attachments":0,"blocked_nanos":0,"overflows":0,"pane":1,"pty_bytes":156000,"resyncs":0,"ticks":796}],"wire_bytes":4152062,"wire_frames":545}
-- two clients on one pane at once, each with its own size, while output is flowing (R7 is left open)
handshake server=probe-server/0.0.0 protocol=1 api=0.1.0 framing=length
{"pane":2}
handshake server=probe-server/0.0.0 protocol=1 api=0.1.0 framing=length
{"attachments":[{"attachment":3,"cols":80,"dropped":0,"mode":"deltas","overflowed":false,"rows":24,"sent":8},{"attachment":4,"cols":120,"dropped":0,"mode":"deltas","overflowed":false,"rows":40,"sent":7}]}
   one of the two was killed with SIGKILL
   the client that was not killed: why="the duration ran out" frames=313 bytes_read=1680791 resyncs=0 seq=4000 unknown_stream_frames=0 fetched_scrollback_rows=0
   it kept receiving output after the other was killed (313 frames)
handshake server=probe-server/0.0.0 protocol=1 api=0.1.0 framing=length
   and its screen and scrollback still match the server's
```

Check passed: `spikes/task-1.2-wire-protocol/reattach-demo --mode screen` exited 0 at 2026-11-14T05:01:28+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.2 on task-1.2-spike-choose-the-wire-protocol-by at e54403d1e1a6 with uncommitted changes:

```text
== reattach demonstration: mode=screen framing=length lines=12000 interval=350us queue=64
Darwin 27.0.0
-- step 1: a pane prints 12000 numbered lines
handshake server=probe-server/0.0.0 protocol=1 api=0.1.0 framing=length
{"pane":1}
-- step 2: client A attaches and is killed mid-output
   client A ready: pane=1 attachment=1 mode=screen seq=8 snapshot_bytes=3347 scrollback_rows=0 attach_ms=0.3
handshake server=probe-server/0.0.0 protocol=1 api=0.1.0 framing=length
   client A killed with SIGKILL while the pane was at line 3373
-- step 3: nothing is attached, and the program keeps printing
handshake server=probe-server/0.0.0 protocol=1 api=0.1.0 framing=length
   the pane reached line 6613 with no client attached
-- step 4: client B attaches
   client B ready: pane=1 attachment=2 mode=screen seq=6656 snapshot_bytes=892175 scrollback_rows=6633 attach_ms=12.4
-- step 5: the program finishes, everything quiesces, and the two are compared
   quiet pane=1 pty_bytes=156000 after=3.6s mb_per_s=0.1
   client B: why="it was told to stop" frames=309 bytes_read=2479311 resyncs=0 seq=11997 unknown_stream_frames=0 fetched_scrollback_rows=10000
handshake server=probe-server/0.0.0 protocol=1 api=0.1.0 framing=length
capture pane=1 bytes=120277 lines=10024 last=12000
same  the reattached client and the server agree byte for byte (120277 bytes, 10024 lines)
whole 10023 numbered lines run from 1978 to 12000 with no gap and no repeat
last  the last line is 12000, as the program printed
gap   every line from 3373 to 6613, printed while no client was attached, is there
PASS
-- the server's counters
handshake server=probe-server/0.0.0 protocol=1 api=0.1.0 framing=length
{"panes":[{"attachments":0,"blocked_nanos":0,"overflows":0,"pane":1,"pty_bytes":156000,"resyncs":0,"ticks":799}],"wire_bytes":4346757,"wire_frames":560}
-- two clients on one pane at once, each with its own size, while output is flowing (R7 is left open)
handshake server=probe-server/0.0.0 protocol=1 api=0.1.0 framing=length
{"pane":2}
handshake server=probe-server/0.0.0 protocol=1 api=0.1.0 framing=length
{"attachments":[{"attachment":3,"cols":80,"dropped":0,"mode":"screen","overflowed":false,"rows":24,"sent":8},{"attachment":4,"cols":120,"dropped":0,"mode":"screen","overflowed":false,"rows":40,"sent":7}]}
   one of the two was killed with SIGKILL
   the client that was not killed: why="the duration ran out" frames=312 bytes_read=1679645 resyncs=0 seq=4000 unknown_stream_frames=0 fetched_scrollback_rows=3977
   it kept receiving output after the other was killed (312 frames)
handshake server=probe-server/0.0.0 protocol=1 api=0.1.0 framing=length
   and its screen and scrollback still match the server's
```

Check passed: `spikes/task-1.2-wire-protocol/measure --mode bytes --mega-bytes 100 --samples 200` exited 0 at 2026-11-14T05:02:10+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.2 on task-1.2-spike-choose-the-wire-protocol-by at e54403d1e1a6 with uncommitted changes:

```text
== measurements
   machine: Darwin 27.0.0 arm64, 28 cpus
   rust:    rustc 1.98.1 (48a229cea 2026-09-01)
   build:   release, opt-level 3, lto off
   crates:  alacritty_terminal 0.26.0, tokio 1.x, tokio-util 0.7, portable-pty 0.9
   run:     framing=length mode=bytes overflow=resync tick=8ms fixture=100MB samples=200
handshake server=probe-server/0.0.0 protocol=1 api=0.1.0 framing=length
handshake server=probe-server/0.0.0 protocol=1 api=0.1.0 framing=length
handshake server=probe-server/0.0.0 protocol=1 api=0.1.0 framing=length
-- 1. a byte into the pty, to that byte on the client's own screen (quiet machine)
   latency mode=bytes n=200 p50=0.057ms p95=0.089ms p99=0.137ms max=0.255ms lost=0
-- 2. the cost of attaching with a full scrollback
   scrollback filled: quiet pane=3 pty_bytes=156000 after=0.5s mb_per_s=14.1
   attach pane=3 attachment=2 mode=bytes seq=12000 snapshot_bytes=1343355 scrollback_rows=10000 attach_ms=45.9
   page of 100 scrollback rows: call pane.scrollback n=20 p50=0.037ms p95=0.061ms p99=0.061ms max=0.061ms
   quiet server: call pane.list n=50 p50=0.019ms p95=0.030ms p99=0.042ms max=0.042ms
-- 3. the flood: 100MB printed in pane 2 with a slow client attached to it
   fixture /var/folders/rf/f34xmczn4659gm98hrsprydc0000gn/T//steg-measure.Hn21vv/flood lines=1310720
   slow client pane=2 attachment=3 mode=bytes seq=0 snapshot_bytes=3347 scrollback_rows=0 attach_ms=0.2
handshake server=probe-server/0.0.0 protocol=1 api=0.1.0 framing=length
   flood quiet pane=2 pty_bytes=104857609 after=2.5s mb_per_s=66.6
   control calls during the flood: call pane.list n=100 p50=0.031ms p95=0.067ms p99=0.084ms max=0.084ms
   keystrokes in another pane during the flood: latency mode=bytes n=100 p50=0.085ms p95=0.144ms p99=0.162ms max=0.162ms lost=0
-- 4. what the server had to do about the slow client
handshake server=probe-server/0.0.0 protocol=1 api=0.1.0 framing=length
   {"panes":[{"attachments":0,"blocked_nanos":0,"overflows":0,"pane":1,"pty_bytes":3760,"resyncs":0,"ticks":441},{"attachments":1,"blocked_nanos":0,"overflows":46,"pane":2,"pty_bytes":104857609,"resyncs":46,"ticks":500},{"attachments":0,"blocked_nanos":0,"overflows":0,"pane":3,"pty_bytes":156000,"resyncs":0,"ticks":520}],"wire_bytes":63060571,"wire_frames":1175}
   slow client why="the duration ran out" frames=369 bytes_read=61569903 resyncs=46 seq=102756 unknown_stream_frames=0 fetched_scrollback_rows=0
-- 5. and whether the slow client is still correct afterwards
   server capture pane=2 bytes=791818 lines=10024 last=0
     the reattached client and the server agree byte for byte (791818 bytes, 10024 lines)
   the slow client ended up with the same screen and scrollback as the server
```

Check passed: `spikes/task-1.2-wire-protocol/measure --mode deltas --mega-bytes 100 --samples 200` exited 0 at 2026-11-14T05:02:48+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.2 on task-1.2-spike-choose-the-wire-protocol-by at e54403d1e1a6 with uncommitted changes:

```text
== measurements
   machine: Darwin 27.0.0 arm64, 28 cpus
   rust:    rustc 1.98.1 (48a229cea 2026-09-01)
   build:   release, opt-level 3, lto off
   crates:  alacritty_terminal 0.26.0, tokio 1.x, tokio-util 0.7, portable-pty 0.9
   run:     framing=length mode=deltas overflow=resync tick=8ms fixture=100MB samples=200
handshake server=probe-server/0.0.0 protocol=1 api=0.1.0 framing=length
handshake server=probe-server/0.0.0 protocol=1 api=0.1.0 framing=length
handshake server=probe-server/0.0.0 protocol=1 api=0.1.0 framing=length
-- 1. a byte into the pty, to that byte on the client's own screen (quiet machine)
   latency mode=deltas n=200 p50=10.042ms p95=10.558ms p99=10.655ms max=10.796ms lost=0
-- 2. the cost of attaching with a full scrollback
   scrollback filled: quiet pane=3 pty_bytes=156000 after=0.5s mb_per_s=13.3
   attach pane=3 attachment=2 mode=deltas seq=12000 snapshot_bytes=1343355 scrollback_rows=10000 attach_ms=38.0
   page of 100 scrollback rows: call pane.scrollback n=20 p50=0.040ms p95=0.071ms p99=0.071ms max=0.071ms
   quiet server: call pane.list n=50 p50=0.024ms p95=0.049ms p99=0.050ms max=0.050ms
-- 3. the flood: 100MB printed in pane 2 with a slow client attached to it
   fixture /var/folders/rf/f34xmczn4659gm98hrsprydc0000gn/T//steg-measure.Vo3tbC/flood lines=1310720
   slow client pane=2 attachment=3 mode=deltas seq=0 snapshot_bytes=3347 scrollback_rows=0 attach_ms=0.1
handshake server=probe-server/0.0.0 protocol=1 api=0.1.0 framing=length
   flood quiet pane=2 pty_bytes=104857602 after=2.5s mb_per_s=68.4
   control calls during the flood: call pane.list n=100 p50=0.041ms p95=0.072ms p99=0.088ms max=0.088ms
   keystrokes in another pane during the flood: latency mode=deltas n=100 p50=10.088ms p95=10.184ms p99=10.208ms max=10.208ms lost=0
-- 4. what the server had to do about the slow client
handshake server=probe-server/0.0.0 protocol=1 api=0.1.0 framing=length
   {"panes":[{"attachments":0,"blocked_nanos":0,"overflows":0,"pane":1,"pty_bytes":3760,"resyncs":0,"ticks":486},{"attachments":1,"blocked_nanos":0,"overflows":24,"pane":2,"pty_bytes":104857602,"resyncs":24,"ticks":704},{"attachments":0,"blocked_nanos":0,"overflows":0,"pane":3,"pty_bytes":156000,"resyncs":0,"ticks":719}],"wire_bytes":110084688,"wire_frames":611}
   slow client why="the duration ran out" frames=124 bytes_read=140868274 resyncs=24 seq=102745 unknown_stream_frames=0 fetched_scrollback_rows=0
-- 5. and whether the slow client is still correct afterwards
   server capture pane=2 bytes=791818 lines=10024 last=0
     the reattached client and the server agree byte for byte (791818 bytes, 10024 lines)
   the slow client ended up with the same screen and scrollback as the server
```

Check passed: `spikes/task-1.2-wire-protocol/measure --mode screen --mega-bytes 100 --samples 200` exited 0 at 2026-11-14T05:03:25+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.2 on task-1.2-spike-choose-the-wire-protocol-by at e54403d1e1a6 with uncommitted changes:

```text
== measurements
   machine: Darwin 27.0.0 arm64, 28 cpus
   rust:    rustc 1.98.1 (48a229cea 2026-09-01)
   build:   release, opt-level 3, lto off
   crates:  alacritty_terminal 0.26.0, tokio 1.x, tokio-util 0.7, portable-pty 0.9
   run:     framing=length mode=screen overflow=resync tick=8ms fixture=100MB samples=200
handshake server=probe-server/0.0.0 protocol=1 api=0.1.0 framing=length
handshake server=probe-server/0.0.0 protocol=1 api=0.1.0 framing=length
handshake server=probe-server/0.0.0 protocol=1 api=0.1.0 framing=length
-- 1. a byte into the pty, to that byte on the client's own screen (quiet machine)
   latency mode=screen n=200 p50=9.940ms p95=10.535ms p99=10.719ms max=10.725ms lost=0
-- 2. the cost of attaching with a full scrollback
   scrollback filled: quiet pane=3 pty_bytes=156000 after=0.5s mb_per_s=14.1
   attach pane=3 attachment=2 mode=screen seq=12000 snapshot_bytes=1343355 scrollback_rows=10000 attach_ms=34.0
   page of 100 scrollback rows: call pane.scrollback n=20 p50=0.041ms p95=0.074ms p99=0.074ms max=0.074ms
   quiet server: call pane.list n=50 p50=0.023ms p95=0.035ms p99=0.051ms max=0.051ms
-- 3. the flood: 100MB printed in pane 2 with a slow client attached to it
   fixture /var/folders/rf/f34xmczn4659gm98hrsprydc0000gn/T//steg-measure.zdHnjP/flood lines=1310720
   slow client pane=2 attachment=3 mode=screen seq=0 snapshot_bytes=3347 scrollback_rows=0 attach_ms=0.1
handshake server=probe-server/0.0.0 protocol=1 api=0.1.0 framing=length
   flood quiet pane=2 pty_bytes=104857604 after=2.3s mb_per_s=78.6
   control calls during the flood: call pane.list n=100 p50=0.029ms p95=0.069ms p99=0.084ms max=0.084ms
   keystrokes in another pane during the flood: latency mode=screen n=100 p50=10.102ms p95=10.476ms p99=10.608ms max=10.608ms lost=0
-- 4. what the server had to do about the slow client
handshake server=probe-server/0.0.0 protocol=1 api=0.1.0 framing=length
   {"panes":[{"attachments":0,"blocked_nanos":0,"overflows":0,"pane":1,"pty_bytes":3760,"resyncs":0,"ticks":463},{"attachments":1,"blocked_nanos":0,"overflows":0,"pane":2,"pty_bytes":104857604,"resyncs":0,"ticks":701},{"attachments":0,"blocked_nanos":0,"overflows":0,"pane":3,"pty_bytes":156000,"resyncs":0,"ticks":698}],"wire_bytes":3139173,"wire_frames":666}
   slow client why="the duration ran out" frames=154 bytes_read=590411 resyncs=0 seq=102732 unknown_stream_frames=0 fetched_scrollback_rows=0
-- 5. and whether the slow client is still correct afterwards
   server capture pane=2 bytes=1818 lines=24 last=0
     the reattached client and the server agree byte for byte (1818 bytes, 24 lines)
   the slow client ended up with the same screen and scrollback as the server
```

Check passed: `spikes/task-1.2-wire-protocol/measure --mode screen --tick-ms 1 --mega-bytes 100 --samples 200` exited 0 at 2026-11-14T05:04:12+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.2 on task-1.2-spike-choose-the-wire-protocol-by at e54403d1e1a6 with uncommitted changes:

```text
== measurements
   machine: Darwin 27.0.0 arm64, 28 cpus
   rust:    rustc 1.98.1 (48a229cea 2026-09-01)
   build:   release, opt-level 3, lto off
   crates:  alacritty_terminal 0.26.0, tokio 1.x, tokio-util 0.7, portable-pty 0.9
   run:     framing=length mode=screen overflow=resync tick=1ms fixture=100MB samples=200
handshake server=probe-server/0.0.0 protocol=1 api=0.1.0 framing=length
handshake server=probe-server/0.0.0 protocol=1 api=0.1.0 framing=length
handshake server=probe-server/0.0.0 protocol=1 api=0.1.0 framing=length
-- 1. a byte into the pty, to that byte on the client's own screen (quiet machine)
   latency mode=screen n=200 p50=1.340ms p95=1.427ms p99=1.532ms max=1.560ms lost=0
-- 2. the cost of attaching with a full scrollback
   scrollback filled: quiet pane=3 pty_bytes=156000 after=0.5s mb_per_s=13.3
   attach pane=3 attachment=2 mode=screen seq=12000 snapshot_bytes=1343355 scrollback_rows=10000 attach_ms=30.1
   page of 100 scrollback rows: call pane.scrollback n=20 p50=0.050ms p95=0.102ms p99=0.102ms max=0.102ms
   quiet server: call pane.list n=50 p50=0.026ms p95=0.048ms p99=0.080ms max=0.080ms
-- 3. the flood: 100MB printed in pane 2 with a slow client attached to it
   fixture /var/folders/rf/f34xmczn4659gm98hrsprydc0000gn/T//steg-measure.ln6mrA/flood lines=1310720
   slow client pane=2 attachment=3 mode=screen seq=0 snapshot_bytes=3347 scrollback_rows=0 attach_ms=0.2
handshake server=probe-server/0.0.0 protocol=1 api=0.1.0 framing=length
   flood quiet pane=2 pty_bytes=104857605 after=3.7s mb_per_s=37.5
   control calls during the flood: call pane.list n=100 p50=0.025ms p95=0.062ms p99=0.254ms max=0.254ms
   keystrokes in another pane during the flood: latency mode=screen n=100 p50=1.365ms p95=1.426ms p99=1.442ms max=1.442ms lost=0
-- 4. what the server had to do about the slow client
handshake server=probe-server/0.0.0 protocol=1 api=0.1.0 framing=length
   {"panes":[{"attachments":0,"blocked_nanos":0,"overflows":0,"pane":1,"pty_bytes":3760,"resyncs":0,"ticks":4830},{"attachments":1,"blocked_nanos":0,"overflows":238,"pane":2,"pty_bytes":104857605,"resyncs":238,"ticks":4007},{"attachments":0,"blocked_nanos":0,"overflows":0,"pane":3,"pty_bytes":156000,"resyncs":0,"ticks":5064}],"wire_bytes":323505721,"wire_frames":1070}
   slow client why="the duration ran out" frames=556 bytes_read=320956716 resyncs=238 seq=102752 unknown_stream_frames=0 fetched_scrollback_rows=0
-- 5. and whether the slow client is still correct afterwards
   server capture pane=2 bytes=1818 lines=24 last=0
     the reattached client and the server agree byte for byte (1818 bytes, 24 lines)
   the slow client ended up with the same screen and scrollback as the server
```

Check passed: `spikes/task-1.2-wire-protocol/measure --mode screen --framing ndjson --mega-bytes 100 --samples 200` exited 0 at 2026-11-14T05:04:49+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.2 on task-1.2-spike-choose-the-wire-protocol-by at e54403d1e1a6 with uncommitted changes:

```text
== measurements
   machine: Darwin 27.0.0 arm64, 28 cpus
   rust:    rustc 1.98.1 (48a229cea 2026-09-01)
   build:   release, opt-level 3, lto off
   crates:  alacritty_terminal 0.26.0, tokio 1.x, tokio-util 0.7, portable-pty 0.9
   run:     framing=ndjson mode=screen overflow=resync tick=8ms fixture=100MB samples=200
handshake server=probe-server/0.0.0 protocol=1 api=0.1.0 framing=ndjson
handshake server=probe-server/0.0.0 protocol=1 api=0.1.0 framing=ndjson
handshake server=probe-server/0.0.0 protocol=1 api=0.1.0 framing=ndjson
-- 1. a byte into the pty, to that byte on the client's own screen (quiet machine)
   latency mode=screen n=200 p50=10.086ms p95=10.835ms p99=10.916ms max=10.927ms lost=0
-- 2. the cost of attaching with a full scrollback
   scrollback filled: quiet pane=3 pty_bytes=156000 after=0.5s mb_per_s=12.9
   attach pane=3 attachment=2 mode=screen seq=12000 snapshot_bytes=1343355 scrollback_rows=10000 attach_ms=83.9
   page of 100 scrollback rows: call pane.scrollback n=20 p50=0.041ms p95=0.063ms p99=0.063ms max=0.063ms
   quiet server: call pane.list n=50 p50=0.022ms p95=0.042ms p99=0.058ms max=0.058ms
-- 3. the flood: 100MB printed in pane 2 with a slow client attached to it
   fixture /var/folders/rf/f34xmczn4659gm98hrsprydc0000gn/T//steg-measure.gjn45C/flood lines=1310720
   slow client pane=2 attachment=3 mode=screen seq=0 snapshot_bytes=3347 scrollback_rows=0 attach_ms=0.2
handshake server=probe-server/0.0.0 protocol=1 api=0.1.0 framing=ndjson
   flood quiet pane=2 pty_bytes=104857604 after=2.3s mb_per_s=78.1
   control calls during the flood: call pane.list n=100 p50=0.041ms p95=0.070ms p99=0.086ms max=0.086ms
   keystrokes in another pane during the flood: latency mode=screen n=100 p50=10.124ms p95=10.413ms p99=10.782ms max=10.782ms lost=0
-- 4. what the server had to do about the slow client
handshake server=probe-server/0.0.0 protocol=1 api=0.1.0 framing=ndjson
   {"panes":[{"attachments":0,"blocked_nanos":0,"overflows":0,"pane":1,"pty_bytes":3760,"resyncs":0,"ticks":469},{"attachments":1,"blocked_nanos":0,"overflows":0,"pane":2,"pty_bytes":104857604,"resyncs":0,"ticks":707},{"attachments":0,"blocked_nanos":0,"overflows":0,"pane":3,"pty_bytes":156000,"resyncs":0,"ticks":704}],"wire_bytes":3162334,"wire_frames":667}
   slow client why="the duration ran out" frames=155 bytes_read=598751 resyncs=0 seq=102741 unknown_stream_frames=0 fetched_scrollback_rows=0
-- 5. and whether the slow client is still correct afterwards
   server capture pane=2 bytes=1818 lines=24 last=0
     the reattached client and the server agree byte for byte (1818 bytes, 24 lines)
   the slow client ended up with the same screen and scrollback as the server
```

Check passed: `spikes/task-1.2-wire-protocol/measure --mode screen --overflow block --mega-bytes 100 --samples 100` exited 0 at 2026-11-14T05:05:25+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.2 on task-1.2-spike-choose-the-wire-protocol-by at e54403d1e1a6 with uncommitted changes:

```text
== measurements
   machine: Darwin 27.0.0 arm64, 28 cpus
   rust:    rustc 1.98.1 (48a229cea 2026-09-01)
   build:   release, opt-level 3, lto off
   crates:  alacritty_terminal 0.26.0, tokio 1.x, tokio-util 0.7, portable-pty 0.9
   run:     framing=length mode=screen overflow=block tick=8ms fixture=100MB samples=100
handshake server=probe-server/0.0.0 protocol=1 api=0.1.0 framing=length
handshake server=probe-server/0.0.0 protocol=1 api=0.1.0 framing=length
handshake server=probe-server/0.0.0 protocol=1 api=0.1.0 framing=length
-- 1. a byte into the pty, to that byte on the client's own screen (quiet machine)
   latency mode=screen n=100 p50=9.881ms p95=10.236ms p99=10.407ms max=10.407ms lost=0
-- 2. the cost of attaching with a full scrollback
   scrollback filled: quiet pane=3 pty_bytes=156000 after=0.5s mb_per_s=11.5
   attach pane=3 attachment=2 mode=screen seq=12000 snapshot_bytes=1343355 scrollback_rows=10000 attach_ms=30.0
   page of 100 scrollback rows: call pane.scrollback n=20 p50=0.050ms p95=0.117ms p99=0.117ms max=0.117ms
   quiet server: call pane.list n=50 p50=0.022ms p95=0.051ms p99=0.070ms max=0.070ms
-- 3. the flood: 100MB printed in pane 2 with a slow client attached to it
   fixture /var/folders/rf/f34xmczn4659gm98hrsprydc0000gn/T//steg-measure.W8I7fI/flood lines=1310720
   slow client pane=2 attachment=3 mode=screen seq=0 snapshot_bytes=3347 scrollback_rows=0 attach_ms=0.1
handshake server=probe-server/0.0.0 protocol=1 api=0.1.0 framing=length
   flood quiet pane=2 pty_bytes=104857603 after=2.3s mb_per_s=78.2
   control calls during the flood: call pane.list n=100 p50=0.024ms p95=0.058ms p99=0.093ms max=0.093ms
   keystrokes in another pane during the flood: latency mode=screen n=100 p50=9.953ms p95=10.419ms p99=10.508ms max=10.508ms lost=0
-- 4. what the server had to do about the slow client
handshake server=probe-server/0.0.0 protocol=1 api=0.1.0 framing=length
   {"panes":[{"attachments":0,"blocked_nanos":110789,"overflows":0,"pane":1,"pty_bytes":2360,"resyncs":0,"ticks":371},{"attachments":1,"blocked_nanos":44627,"overflows":0,"pane":2,"pty_bytes":104857603,"resyncs":0,"ticks":526},{"attachments":0,"blocked_nanos":0,"overflows":0,"pane":3,"pty_bytes":156000,"resyncs":0,"ticks":526}],"wire_bytes":2748100,"wire_frames":567}
   slow client why="the duration ran out" frames=155 bytes_read=594094 resyncs=0 seq=102743 unknown_stream_frames=0 fetched_scrollback_rows=0
-- 5. and whether the slow client is still correct afterwards
   server capture pane=2 bytes=1818 lines=24 last=0
     the reattached client and the server agree byte for byte (1818 bytes, 24 lines)
   the slow client ended up with the same screen and scrollback as the server
```

Check passed: `spikes/task-1.2-wire-protocol/measure --mode screen --tick-ms 1 --mega-bytes 100 --samples 200` exited 0 at 2026-11-14T05:06:30+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.2 on task-1.2-spike-choose-the-wire-protocol-by at e54403d1e1a6 with uncommitted changes:

```text
== measurements
   machine: Darwin 27.0.0 arm64, 28 cpus
   rust:    rustc 1.98.1 (48a229cea 2026-09-01)
   build:   release, opt-level 3, lto off
   crates:  alacritty_terminal 0.26.0, tokio 1.x, tokio-util 0.7, portable-pty 0.9
   run:     framing=length mode=screen overflow=resync tick=1ms fixture=100MB samples=200
handshake server=probe-server/0.0.0 protocol=1 api=0.1.0 framing=length
handshake server=probe-server/0.0.0 protocol=1 api=0.1.0 framing=length
handshake server=probe-server/0.0.0 protocol=1 api=0.1.0 framing=length
-- 1. a byte into the pty, to that byte on the client's own screen (quiet machine)
   latency mode=screen n=200 p50=1.337ms p95=1.425ms p99=1.673ms max=1.714ms lost=0
-- 2. the cost of attaching with a full scrollback
   scrollback filled: quiet pane=3 pty_bytes=156000 after=0.5s mb_per_s=12.7
   attach pane=3 attachment=2 mode=screen seq=12000 snapshot_bytes=1343355 scrollback_rows=10000 attach_ms=39.6
   page of 100 scrollback rows: call pane.scrollback n=20 p50=0.042ms p95=0.079ms p99=0.079ms max=0.079ms
   quiet server: call pane.list n=50 p50=0.023ms p95=0.039ms p99=0.048ms max=0.048ms
-- 3. the flood: 100MB printed in pane 2 with a slow client attached to it
   fixture /var/folders/rf/f34xmczn4659gm98hrsprydc0000gn/T//steg-measure.QDs38o/flood lines=1310720
   slow client pane=2 attachment=3 mode=screen seq=0 snapshot_bytes=3347 scrollback_rows=0 attach_ms=0.1
handshake server=probe-server/0.0.0 protocol=1 api=0.1.0 framing=length
   flood quiet pane=2 pty_bytes=104857603 after=2.8s mb_per_s=54.4
   control calls during the flood: call pane.list n=100 p50=0.026ms p95=0.066ms p99=0.077ms max=0.077ms
   keystrokes in another pane during the flood: latency mode=screen n=100 p50=1.368ms p95=1.444ms p99=1.674ms max=1.674ms lost=0
-- 4. what the server had to do about the slow client
handshake server=probe-server/0.0.0 protocol=1 api=0.1.0 framing=length
   {"panes":[{"attachments":0,"blocked_nanos":0,"overflows":0,"pane":1,"pty_bytes":3760,"resyncs":0,"ticks":4179},{"attachments":1,"blocked_nanos":0,"overflows":255,"pane":2,"pty_bytes":104857603,"resyncs":255,"ticks":3989},{"attachments":0,"blocked_nanos":0,"overflows":0,"pane":3,"pty_bytes":156000,"resyncs":0,"ticks":4403}],"wire_bytes":4778709,"wire_frames":1121}
   slow client why="the duration ran out" frames=608 bytes_read=2229652 resyncs=255 seq=102760 unknown_stream_frames=0 fetched_scrollback_rows=0
-- 5. and whether the slow client is still correct afterwards
   server capture pane=2 bytes=1818 lines=24 last=0
     the reattached client and the server agree byte for byte (1818 bytes, 24 lines)
   the slow client ended up with the same screen and scrollback as the server
```

Check passed: `spikes/task-1.2-wire-protocol/measure --mode bytes --overflow block --mega-bytes 100 --samples 100` exited 0 at 2026-11-14T05:07:08+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.2 on task-1.2-spike-choose-the-wire-protocol-by at e54403d1e1a6 with uncommitted changes:

```text
== measurements
   machine: Darwin 27.0.0 arm64, 28 cpus
   rust:    rustc 1.98.1 (48a229cea 2026-09-01)
   build:   release, opt-level 3, lto off
   crates:  alacritty_terminal 0.26.0, tokio 1.x, tokio-util 0.7, portable-pty 0.9
   run:     framing=length mode=bytes overflow=block tick=8ms fixture=100MB samples=100
handshake server=probe-server/0.0.0 protocol=1 api=0.1.0 framing=length
handshake server=probe-server/0.0.0 protocol=1 api=0.1.0 framing=length
handshake server=probe-server/0.0.0 protocol=1 api=0.1.0 framing=length
-- 1. a byte into the pty, to that byte on the client's own screen (quiet machine)
   latency mode=bytes n=100 p50=0.057ms p95=0.086ms p99=0.225ms max=0.225ms lost=0
-- 2. the cost of attaching with a full scrollback
   scrollback filled: quiet pane=3 pty_bytes=156000 after=1.1s mb_per_s=0.2
   attach pane=3 attachment=2 mode=bytes seq=12000 snapshot_bytes=1343355 scrollback_rows=10000 attach_ms=35.1
   page of 100 scrollback rows: call pane.scrollback n=20 p50=0.037ms p95=0.070ms p99=0.070ms max=0.070ms
   quiet server: call pane.list n=50 p50=0.019ms p95=0.030ms p99=0.042ms max=0.042ms
-- 3. the flood: 100MB printed in pane 2 with a slow client attached to it
   fixture /var/folders/rf/f34xmczn4659gm98hrsprydc0000gn/T//steg-measure.fvejOk/flood lines=1310720
   slow client pane=2 attachment=3 mode=bytes seq=0 snapshot_bytes=3347 scrollback_rows=0 attach_ms=0.2
handshake server=probe-server/0.0.0 protocol=1 api=0.1.0 framing=length
   flood quiet pane=2 pty_bytes=104857606 after=32.0s mb_per_s=3.2
   control calls during the flood: call pane.list n=100 p50=0.025ms p95=0.065ms p99=0.097ms max=0.097ms
   keystrokes in another pane during the flood: latency mode=bytes n=100 p50=0.068ms p95=0.122ms p99=0.143ms max=0.143ms lost=0
-- 4. what the server had to do about the slow client
handshake server=probe-server/0.0.0 protocol=1 api=0.1.0 framing=length
   {"panes":[{"attachments":0,"blocked_nanos":99362,"overflows":0,"pane":1,"pty_bytes":2360,"resyncs":0,"ticks":3504},{"attachments":0,"blocked_nanos":29255157080,"overflows":0,"pane":2,"pty_bytes":104857606,"resyncs":0,"ticks":1714},{"attachments":0,"blocked_nanos":0,"overflows":0,"pane":3,"pty_bytes":156000,"resyncs":0,"ticks":3573}],"wire_bytes":10481371,"wire_frames":9375}
   slow client why="the duration ran out" frames=8753 bytes_read=9011712 resyncs=0 seq=0 unknown_stream_frames=0 fetched_scrollback_rows=0
-- 5. and whether the slow client is still correct afterwards
   server capture pane=2 bytes=791818 lines=10024 last=0
   FAIL  the client and the server differ first at line 1: client "0000101587 fill .............................................................." server "0001300697 fill .............................................................."
```

Check passed: `spikes/task-1.2-wire-protocol/measure --mode screen --mega-bytes 100 --samples 200` exited 0 at 2026-11-14T05:08:10+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.2 on task-1.2-spike-choose-the-wire-protocol-by at e54403d1e1a6 with uncommitted changes:

```text
== measurements
   machine: Darwin 27.0.0 arm64, 28 cpus
   rust:    rustc 1.98.1 (48a229cea 2026-09-01)
   build:   release, opt-level 3, lto off
   crates:  alacritty_terminal 0.26.0, tokio 1.x, tokio-util 0.7, portable-pty 0.9
   run:     framing=length mode=screen overflow=resync tick=8ms fixture=100MB samples=200
handshake server=probe-server/0.0.0 protocol=1 api=0.1.0 framing=length
handshake server=probe-server/0.0.0 protocol=1 api=0.1.0 framing=length
handshake server=probe-server/0.0.0 protocol=1 api=0.1.0 framing=length
-- 1. a byte into the pty, to that byte on the client's own screen (quiet machine)
   latency mode=screen n=200 p50=10.133ms p95=10.625ms p99=10.708ms max=10.759ms lost=0
-- 2. the cost of attaching with a full scrollback
   scrollback filled: quiet pane=3 pty_bytes=156000 after=0.5s mb_per_s=11.8
   attach pane=3 attachment=2 mode=screen seq=12000 snapshot_bytes=1343355 scrollback_rows=10000 attach_ms=38.5
   page of 100 scrollback rows: call pane.scrollback n=20 p50=0.040ms p95=0.089ms p99=0.089ms max=0.089ms
   quiet server: call pane.list n=50 p50=0.024ms p95=0.038ms p99=0.049ms max=0.049ms
-- 3. the flood: 100MB printed in pane 2 with a slow client attached to it
   fixture /var/folders/rf/f34xmczn4659gm98hrsprydc0000gn/T//steg-measure.aAF46X/flood lines=1310720
   slow client pane=2 attachment=3 mode=screen seq=0 snapshot_bytes=3347 scrollback_rows=0 attach_ms=0.1
handshake server=probe-server/0.0.0 protocol=1 api=0.1.0 framing=length
   flood quiet pane=2 pty_bytes=104857604 after=2.3s mb_per_s=79.4
   control calls during the flood: call pane.list n=100 p50=0.032ms p95=0.060ms p99=0.079ms max=0.079ms
   keystrokes in another pane during the flood: latency mode=screen n=100 p50=10.102ms p95=10.465ms p99=10.518ms max=10.518ms lost=0
-- 4. what the server had to do about the slow client
handshake server=probe-server/0.0.0 protocol=1 api=0.1.0 framing=length
   {"panes":[{"attachments":0,"blocked_nanos":0,"overflows":0,"pane":1,"pty_bytes":3760,"resyncs":0,"ticks":457},{"attachments":1,"blocked_nanos":0,"overflows":0,"pane":2,"pty_bytes":104857604,"resyncs":0,"ticks":685},{"attachments":0,"blocked_nanos":0,"overflows":0,"pane":3,"pty_bytes":156000,"resyncs":0,"ticks":684}],"wire_bytes":3131342,"wire_frames":664}
   slow client why="the duration ran out" frames=152 bytes_read=582579 resyncs=0 seq=102727 unknown_stream_frames=0 fetched_scrollback_rows=0
-- 5. and whether the slow client is still correct afterwards
   server capture pane=2 bytes=1818 lines=24 last=0
     the reattached client and the server agree byte for byte (1818 bytes, 24 lines)
   the slow client ended up with the same screen and scrollback as the server
```

Check passed: `spikes/task-1.2-wire-protocol/measure --mode bytes --framing ndjson --mega-bytes 100 --samples 200` exited 0 at 2026-11-14T05:10:41+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.2 on task-1.2-spike-choose-the-wire-protocol-by at e54403d1e1a6 with uncommitted changes:

```text
== measurements
   machine: Darwin 27.0.0 arm64, 28 cpus
   rust:    rustc 1.98.1 (48a229cea 2026-09-01)
   build:   release, opt-level 3, lto off
   crates:  alacritty_terminal 0.26.0, tokio 1.x, tokio-util 0.7, portable-pty 0.9
   run:     framing=ndjson mode=bytes overflow=resync tick=8ms fixture=100MB samples=200
handshake server=probe-server/0.0.0 protocol=1 api=0.1.0 framing=ndjson
handshake server=probe-server/0.0.0 protocol=1 api=0.1.0 framing=ndjson
handshake server=probe-server/0.0.0 protocol=1 api=0.1.0 framing=ndjson
-- 1. a byte into the pty, to that byte on the client's own screen (quiet machine)
   latency mode=bytes n=200 p50=0.058ms p95=0.108ms p99=0.172ms max=0.233ms lost=0
-- 2. the cost of attaching with a full scrollback
   scrollback filled: quiet pane=3 pty_bytes=156000 after=0.5s mb_per_s=12.9
   attach pane=3 attachment=2 mode=bytes seq=12000 snapshot_bytes=1343355 scrollback_rows=10000 attach_ms=94.4
   page of 100 scrollback rows: call pane.scrollback n=20 p50=0.042ms p95=0.071ms p99=0.071ms max=0.071ms
   quiet server: call pane.list n=50 p50=0.025ms p95=0.043ms p99=0.055ms max=0.055ms
-- 3. the flood: 100MB printed in pane 2 with a slow client attached to it
   fixture /var/folders/rf/f34xmczn4659gm98hrsprydc0000gn/T//steg-measure.Diyjdg/flood lines=1310720
   slow client pane=2 attachment=3 mode=bytes seq=0 snapshot_bytes=3347 scrollback_rows=0 attach_ms=0.2
handshake server=probe-server/0.0.0 protocol=1 api=0.1.0 framing=ndjson
   flood quiet pane=2 pty_bytes=104857605 after=2.5s mb_per_s=64.9
   control calls during the flood: call pane.list n=100 p50=0.032ms p95=0.076ms p99=0.088ms max=0.088ms
   keystrokes in another pane during the flood: latency mode=bytes n=100 p50=0.087ms p95=3.611ms p99=4.751ms max=4.751ms lost=0
-- 4. what the server had to do about the slow client
handshake server=probe-server/0.0.0 protocol=1 api=0.1.0 framing=ndjson
   {"panes":[{"attachments":0,"blocked_nanos":0,"overflows":0,"pane":1,"pty_bytes":3760,"resyncs":0,"ticks":451},{"attachments":1,"blocked_nanos":0,"overflows":34,"pane":2,"pty_bytes":104857605,"resyncs":34,"ticks":505},{"attachments":0,"blocked_nanos":0,"overflows":0,"pane":3,"pty_bytes":156000,"resyncs":0,"ticks":527}],"wire_bytes":47210098,"wire_frames":1046}
   slow client why="the duration ran out" frames=237 bytes_read=45695329 resyncs=34 seq=102750 unknown_stream_frames=0 fetched_scrollback_rows=0
-- 5. and whether the slow client is still correct afterwards
   server capture pane=2 bytes=791818 lines=10024 last=0
     the reattached client and the server agree byte for byte (791818 bytes, 10024 lines)
   the slow client ended up with the same screen and scrollback as the server
```

Check passed: `just check` exited 0 at 2026-11-14T05:14:19+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.2 on task-1.2-spike-choose-the-wire-protocol-by at 82a05984617f:

```text
skip    format check: no Rust workspace yet
skip    lint: no Rust workspace yet
skip    licences: no Rust workspace yet
skip    tests: no Rust workspace yet
ok      tool tests (0.1s)
```
<!-- SECTION:NOTES:END -->

## Comments

<!-- COMMENTS:BEGIN -->
author: @bruk
created: 2026-11-14T03:56:00Z

---

Plan approved by @bruk (4dfe21538510, size-3): its worker may build it.

---
<!-- COMMENTS:END -->
