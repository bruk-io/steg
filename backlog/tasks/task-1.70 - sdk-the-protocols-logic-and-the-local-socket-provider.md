---
id: TASK-1.70
title: 'sdk: the protocol''s logic, and the local-socket provider'
status: In Progress
assignee:
  - '@bruk.w5'
created_date: '2026-11-15T02:21:00Z'
updated_date: '2026-11-15T16:05:00Z'
labels:
  - size-3
milestone: m-0
dependencies:
  - TASK-1.43
parent_task_id: TASK-1
type: task
project: bruk-io-steg
asdlc_accepted_from: DRAFT-1
asdlc_branch: task-1.70-sdk-the-protocol-s-logic-and-the-local
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
doc-16: R101, R102, R103, R106; doc-8 R3, R39, R99. The protocol written once, with no I/O, for the server, the CLI and the app to consume, and the one provider that finds or starts a local server. Attaching to a pane is added to the sdk by TASK-1.47.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The sdk encodes and decodes frames, does the handshake and tracks each stream's state as logic that takes bytes and returns bytes and events, with no socket, clock or runtime, and its tests need none (R102)
- [ ] #2 A consumer supplies the transport through an interface the sdk defines; a test drives a client and a server end against each other in memory
- [ ] #3 The optional local-socket provider connects to a running server, and with none running starts one by running the steg binary and waits for its socket; it can be told not to start one (R103)
- [ ] #4 A version mismatch is reported naming both versions (R39)
- [ ] #5 A method is called by name with its parameters and returns its result or its error, and events arrive as typed values; the methods come from the API description, with none written by hand
- [ ] #6 No GPUI type and no particular async runtime appears in the sdk's public interface (R106)
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
## One task, two reviewable changes

Build as two commits on one branch, with a hard boundary: half A touches no feature-gated
code, does no I/O anywhere, and every test runs in memory; half B is entirely behind
`local-socket`, is the only place with the crate's one `#[expect]`, and is the only part
whose test needs a filesystem and a real listener. They prove disjoint acceptance criteria
(A: #1, #2, #4, #6; B: #3) and fail for different reasons, so a reviewer can take them one at
a time. Whether that becomes two PRs or stays one PR with two commits is the engineer's call
at plan approval - I can't split this task myself - but the branch must not interleave them.

## Where decision-2 and doc-14 diverge from what's being built

- decision-2 section 11 lists the protocol's dependencies as `serde`, `serde_json`, `bytes`,
  `tokio`, `tokio-util` (plus `base64` for the superseded NDJSON stand-in). None of that
  holds here: R102/R106 forbid an async runtime or an I/O crate in the sdk's public
  interface, and `boundaries.toml`'s `[steg-sdk]` already allows only `serde`/`serde_json`.
  This task adds no dependency, in either half. `bytes`/`BytesMut` from the spike's
  `frame.rs` become plain `Vec<u8>`/`&[u8]`: a copy at the boundary once per `receive()`
  call, not zero-copy - simplicity over throughput headroom, acceptable for the control
  plane and for a tick-bound terminal path decision-2 section 4 already measured; revisit
  only if profiling says otherwise.
- The NDJSON `Framing::Ndjson` candidate is dropped entirely - decision-2 chose length
  framing and the comparison is over.
- decision-2 section 7 freezes the hello at `{steg, protocol, client}` / `{steg, protocol,
  server, api}`. doc-14 "Clients" adds `kind` (`app`/`cli`/`plugin`) and optional `label` to
  the client's hello, and `client` (the server-assigned id) to the server's reply - allowed
  because unknown keys are ignored, and TASK-1.46 needs `kind` for `client.list`. This task
  adds those fields now rather than leaving a later task to widen a frozen type.

## Half A: the protocol's logic (no I/O, no feature)

New module `crates/sdk/src/wire/` (`mod.rs` holds only `mod`/`pub use`, per `.claude/rules/rust.md`):

**`wire/frame.rs`** - ported from `spikes/task-1.2-wire-protocol/probe-wire/src/frame.rs`,
length framing only, `tokio`/`bytes` stripped:
- `MAX_FRAME` (8 MiB), `HEADER_LEN`, `MAX_PAYLOAD`, `CONTROL_STREAM = 0`, `Kind {Hello,
  Control, Data}`, `Frame {stream: u32, kind: Kind, payload: Vec<u8>}`.
- `encode_frame(&Frame) -> Vec<u8>` (len-prefix + header + payload; this is the one thing a
  consumer writes to its transport verbatim).
- `Decoder` owns one growable `Vec<u8>` (who owns buffers: the decoder owns its own
  accumulation buffer; the caller owns whatever it read bytes into and only ever hands over a
  borrowed `&[u8]` via `push`). `Decoder::push(&mut self, bytes: &[u8])` appends.
  `Decoder::next_frame(&mut self) -> Result<Option<Frame>, FrameError>` returns one decoded
  frame or `None` (need more bytes). The 4-byte length is read and checked against
  `MAX_FRAME` before anything beyond those 4 bytes is touched, so `FrameError::TooLarge` is
  returned with nothing allocated for the declared body - test: feed exactly 4 bytes
  declaring `0xFFFF_FFFF` and assert `TooLarge` comes back without ever pushing a 5th byte.
- `FrameError {TooLarge{size,limit}, Malformed(String)}` (no `Io` variant here - half A never
  touches a transport, so it has no I/O error to carry).

**`wire/handshake.rs`** - ported from `probe-wire/src/hello.rs`, `shake_hands`'s async
round-trip dropped (half A has no transport to round-trip over; driving the handshake is
`wire/connection.rs`'s job, below):
- `PROTOCOL_VERSION: u32 = 1`, `STEG_MAGIC: &str = "steg"`.
- `ClientHello {steg, protocol, client, kind: ClientKind, label: Option<String>}`,
  `ClientKind {App, Cli, Plugin}` (doc-14), `ServerHello {steg, protocol, server, api,
  client: String}` (doc-14's server-assigned id), `Refused`, `Refusal {Mismatch, Malformed}`,
  `Handshake {Accept, Refuse}` - same shape as the spike otherwise.
- `check_hello(payload: &[u8], server_protocol: u32) -> Handshake` - ported as-is, already
  sans-I/O in the spike.
- Tests: client newer, server newer (both directions named, R39), bad magic, missing field,
  non-JSON first frame, unknown field ignored.

**`wire/control.rs`** - the envelope, built on TASK-1.43's `description` types rather than
beside them:
- `Control {Request{id: u64, request: description::Request}, Reply{id: u64, response:
  description::Response}, Event{name: String, body: serde_json::Value}}`. `Response` is
  already `Ok(Value) | Error(ApiError)`, so "its result or its error, matched by id" (AC #5)
  needs no new type.
- `to_payload`/`from_payload` via `serde_json`.

**`wire/connection.rs`** - the multiplexed state machine, the one type both ends drive
(R102: "it serves both ends of the connection, since the server is a consumer too"):
- `Role {Client, Server}`. `Connection::new(role: Role, description: Description) ->
  Connection` - construction takes the `Description` once, so `call()` below validates
  against it with nothing hand-written (AC #5), and a server surfacing `Incoming::Request`
  can validate the same way before dispatching.
- Stream bookkeeping: an open set of non-zero stream ids. `open_stream(&mut self, stream:
  u32)`, `close_stream(&mut self, stream: u32)`. Deliberately thin - the attach ordering rule
  decision-2 section 3 found the hard way ("queue the attach result first, register the
  attachment's queue second") is a calling-convention TASK-1.47 must follow by sequencing its
  own `reply()` before its own `open_stream()`; this task's job is only to make `open_stream`
  a separate, explicit call so getting that order right is possible, not to enforce it.
- Input: `receive(&mut self, bytes: &[u8])` feeds the internal `Decoder`.
  `poll(&mut self) -> Option<Incoming>` drains one decoded item per call, `None` meaning
  "need more bytes." `Incoming {Request{id, request}, Reply{id, response}, Event{name,
  body}, UnknownEvent{name, body}, Data{stream, bytes}, UnknownStream{stream, kind},
  Malformed(FrameError)}`.
  - Events are resolved against the stored `Description` at decode time (not left as a bare
    `String` nobody checked): a name the description lists comes back as `Incoming::Event`,
    anything else as `Incoming::UnknownEvent` - both carry `body: Value`. The body itself
    stays untyped-by-field on purpose: `methods.rs` already carries `layout`, `status`, and
    `shown_status` as `ValueKind::Text` placeholders pending TASK-1.39/1.41, so there are no
    settled per-event field shapes yet to decode into; typing the body is that task's to do,
    not this one's to fake.
  - `Data` on a registered stream, `UnknownStream` on one that is not - test named for
    decision-2 section 3's own invariant (`unknown_stream_frames=0`), proving the inverse: a
    frame on an unregistered stream is surfaced, not silently dropped or silently accepted.
- Output, each returning the exact bytes to write, nothing more:
  - Client: `send_hello(&mut self, client: &str, kind: ClientKind, label: Option<&str>) ->
    Vec<u8>`; `call(&mut self, method: &str, params: Map<String, Value>) ->
    Result<(u64, Vec<u8>), ApiError>` - validates with `description::validate` first (the
    `Description` given at construction), assigns the next id from an internal counter, and
    is the one path every method call takes, generated CLI commands and the app alike (AC #5
    "with none written by hand").
  - Server: `accept_or_refuse(&mut self, payload: &[u8], server_name: &str, api: &str,
    client_id: &str) -> (Handshake, Vec<u8>)`; `reply(&mut self, id: u64, response:
    description::Response) -> Vec<u8>`; `emit_event(&mut self, name: &str, body: Value) ->
    Vec<u8>`.
  - Both: `send_data(&mut self, stream: u32, bytes: &[u8]) -> Result<Vec<u8>, FrameError>` -
    refuses an unregistered stream or an over-cap payload before encoding.
- No trait is defined for "the transport." The brief itself names why: a blocking CLI,
  GPUI's executor, and a future Python asyncio transport each have a different I/O model,
  and Python cannot implement a Rust trait at all, so a transport trait would either leak one
  runtime's shape into the public interface (what R106 forbids) or constrain nothing useful.
  The interface R102 says "the sdk defines" is this method contract itself: bytes handed to
  `receive`, bytes taken back from every `send_*`/`call`/`reply`, who owns which buffer
  stated above. If the engineer wants something literally `impl`-able for the blocking case,
  that belongs beside `local_socket` in half B, where blocking is already assumed - not here.

**Small amendment to an existing file:** `description::Description` gains `pub fn
event(&self, name: &str) -> Option<&Event>`, mirroring the existing `method()` lookup one
line above it - needed by `connection.rs`'s event resolution, nothing else changes in
`description/`.

**Settles, for TASK-1.47 without it changing anything here:** terminal data on the wire needs
no new `Kind`. decision-2 section 3 already settled it - a screen-mode attachment's snapshot
travels as the `call()`/`reply()` result on stream 0, its updates as `Control` frames (so,
`Incoming::Event`-shaped, not `Data`) on the attachment's own stream, and only pane input
travels as `Data` on that stream. TASK-1.47 picks a stream id and uses `open_stream`,
`emit_event`-the-equivalent-on-a-non-zero-stream (or a thin wrapper this task leaves it to
add), and `send_data`; it adds no framing.

**AC -> test, half A:**
- AC #1: `wire/frame.rs`, `wire/handshake.rs`, `wire/connection.rs` unit tests, all in-memory,
  no socket/clock/runtime anywhere in the module tree.
- AC #2: `crates/sdk/tests/connection.rs` (integration test, still no I/O - an in-process
  `Vec<u8>` shuttle moves bytes between a `Role::Client` and a `Role::Server` `Connection`):
  hello both ways to `Accept`, then `call()` -> shuttle -> `Incoming::Request` -> `reply()` ->
  shuttle -> `Incoming::Reply` with the matching id.
- AC #4: `wire/handshake.rs`'s mismatch tests, both directions named in the message text.
- AC #5: the `call()`/`reply()`/event tests above, plus one asserting `call()` refuses a
  method the given `Description` doesn't list (reusing TASK-1.43's `validate`, not a new
  check).
- AC #6: no code proof needed beyond what exists - `boundaries.toml`'s `[steg-sdk]` allow
  list (`serde`, `serde_json` only) plus `just check-boundaries` is the existing mechanical
  proof that no `gpui`/`tokio` dependency can be added; this task adds none.

## Half B: the local-socket provider (behind the `local-socket` feature)

**`crates/sdk/src/paths.rs` - no feature gate, no I/O, pure:**
- `pub fn socket_path(xdg_runtime_dir: Option<&Path>, tmp_dir: Option<&Path>, user:
  Option<&str>) -> PathBuf`. Preference order: `$XDG_RUNTIME_DIR/steg.sock` (doc-13 R60's own
  name), else `$TMPDIR/steg.sock` (macOS's de facto per-user, mode-700, runtime-only
  equivalent - XDG_RUNTIME_DIR is rarely set on macOS), else `/tmp/steg-<user>.sock` as a last
  resort shared-tmp fallback. Flat, no subdirectory: `sockaddr_un.sun_path` is 104 bytes on
  macOS, and a realistic `$TMPDIR` such as
  `/var/folders/zz/zyxvpxvq6csfxvn_n0000000000000/T/` plus `steg.sock` is already close to
  half that budget, so no extra directory component is spent on it. Test asserts the encoded
  path (`to_str().unwrap().len()`, with the function tested against that exact realistic
  example) stays under the limit, with a margin, rather than asserting it by inspection only -
  flagged as a real constraint, not a formality, and as a limitation if some future `$TMPDIR`
  ever runs longer.
- This function takes resolved inputs, not environment access, so it is unit-tested directly
  with fabricated values and needs nothing from `local-socket`. Reading the real environment
  is each caller's own job: the provider below, and later TASK-1.44's listener, which must
  call this same function with the environment it reads so client and server converge on one
  path with no message ever naming it.
- **The lock half of "the socket's path and lock" is explicitly not this task's.** The
  provider never creates, checks, or holds a lock; it only spawns and waits for the socket to
  accept a connection. Mutual exclusion when two clients start a server at the same moment
  (R99) is entirely TASK-1.44's - whatever it does (an exclusive bind, a lock file beside the
  socket, a losing second server exiting once it sees the winner's socket) is invisible to
  the provider, which only ever sees "the socket now accepts, or it still doesn't."

**`crates/sdk/src/local_socket.rs`, `#[cfg(feature = "local-socket")]`, std only, blocking:**
- `ConnectOptions {binary: PathBuf, socket_path: Option<PathBuf>, autostart: bool,
  connect_timeout: Duration}`. `binary` has **no default anywhere in the sdk** - the CLI
  (`steg` package, TASK-1.49) sets it to `std::env::current_exe()`, which is correct only
  because the server and the CLI ship as one binary (R61/R98); the app (TASK-1.52) is "a
  separate program" (doc-16 level 2) and must supply wherever it finds its own copy of
  `steg`, which is an open question left to that task's plan, not answered here.
  `socket_path: None` means "compute it from the real environment via `paths::socket_path`";
  `Some(path)` is the override a test (and later `--host`/debugging) uses so no test has to
  mutate process-wide env vars.
- `ConnectError {NoServerRunning, Spawn(io::Error), Timeout, Connect(io::Error)}`.
  `NoServerRunning` is exactly what doc-2 line 180's `--no-autostart` exits 4 on - TASK-1.49
  maps it there, nothing in the sdk knows about exit statuses.
- `pub fn connect(options: &ConnectOptions) -> Result<UnixStream, ConnectError>`:
  1. Resolve the path (`options.socket_path` or `paths::socket_path` fed from
     `env::var("XDG_RUNTIME_DIR")`/`"TMPDIR"`/`"USER"`).
  2. Try `UnixStream::connect(&path)`. On success, return it - a server was already running,
     nothing is spawned.
  3. On failure: if `!options.autostart`, `Err(NoServerRunning)`.
  4. Otherwise spawn `Command::new(&options.binary).arg("serve").stdin/out/err(Stdio::null())`
     and **do not wait() on the child** - R99's detaching, re-parenting or double-fork, and
     stopping are the server's own code; the provider drops the `Child` handle once spawn
     succeeds and trusts the server to have finished detaching by the time its socket
     answers. `serve` is a private entry point, not part of doc-2's documented grammar;
     TASK-1.44 implements a `steg` binary that, run this way, computes its own socket path
     with `paths::socket_path` over its own inherited environment and binds there - that's
     the whole of the contract this task hands it.
  5. Retry `UnixStream::connect` on a short interval until it succeeds or
     `connect_timeout` elapses (`Timeout`).
- This is the module `.claude/rules/sdk.md` describes: one `#[expect(clippy::disallowed_types,
  clippy::disallowed_methods, reason = "R103: ...")]` scoped to it, and both lints it names
  must actually fire there or the `#[expect]` itself fails the build (an unfired `expect` is
  an error) - `UnixStream`/`Command` trip `disallowed-types`, `thread::sleep`/`Instant::now`
  (the retry loop and its deadline) trip `disallowed-methods`; if the final shape only hits
  one of the two, split the attribute per-lint rather than leaving one half unfired.

**Test fixture, so AC #3 is proved without running the real (not-yet-built) server:** add a
`[[bin]]` target `name = "fake-server-for-tests"`, `path =
"tests/fixtures/fake_server.rs"`, `required-features = ["local-socket"]` to
`crates/sdk/Cargo.toml` - it only builds under that feature, and its compiled path reaches
`crates/sdk/tests/local_socket.rs` (`#![cfg(feature = "local-socket")]`) via
`env!("CARGO_BIN_EXE_fake-server-for-tests")`, a standard Cargo integration-test mechanism, no
new dependency. The fixture binds a `UnixListener` at the path given on its own argv and
sleeps, standing in for "a server now accepts there."

**AC -> test, half B (all in `crates/sdk/tests/local_socket.rs`, each against a temp dir, no
network, no real `steg` binary):**
- A listener is already bound at `socket_path` -> `connect()` succeeds and nothing is
  spawned, proved by pointing `binary` at a path that cannot execute and asserting no `Spawn`
  error occurs.
- Nothing is listening, `autostart: false` -> `Err(NoServerRunning)`, nothing spawned.
- Nothing is listening, `autostart: true`, `binary` is the fixture -> `connect()` waits and
  succeeds once the fixture binds.
- `connect_timeout` elapses with nothing ever binding -> `Err(Timeout)`.

**Not this task's, confirmed against TASK-1.44's own acceptance criteria:** the socket's 0600
mode, the handshake timeout, the real `steg serve` entry point, and the lock/race resolution
for R99 are all named on TASK-1.44 already and stay there.

## A gap this task has to close in `justfile`, or half B's tests never run in `just check`

Neither `test` nor `check-packages` passes any `--features` flag; only `lint`
(`--all-features`) does, and clippy never executes a test. As the repository's recipes stand
today, `local-socket`'s own tests (half B, and the fixture `[[bin]]`) would compile and run
under `cargo test -p steg-sdk --all-features` by hand, but never under `just check` or `just
test` - a silent gap this task would otherwise leave behind it. Fix: add one line to
`check-packages`'s loop, specific to `steg-sdk`, running `cargo test -p steg-sdk --features
local-socket` alongside its existing no-features build/test, so the package is proved both
with the feature off (the default every other consumer gets) and on (what half B's own tests
need), each alone - consistent with the recipe's existing comment about proving a feature
isn't accidentally on everywhere else.

## What each dependent needs from this task, as public types and functions

- **TASK-1.44** (server, socket, lifecycle): `wire::{PROTOCOL_VERSION, STEG_MAGIC,
  ClientHello, ServerHello, Refused, Refusal, Handshake, ClientKind}`,
  `Connection::{new, accept_or_refuse, reply, emit_event, open_stream, close_stream,
  send_data, receive, poll}` driven over its own socket read/write loop; `paths::socket_path`
  fed from its own environment reads, so it binds at the identical path the provider computes
  with no path ever sent over the wire. Everything about detaching, the 30-second idle
  stop, the handshake timeout, and resolving two simultaneous starts to one survivor (R99) is
  its own code, built on top of these, not supplied here.
- **TASK-1.45** (frames, streams, requests routed to the API): `Connection::poll`'s
  `Incoming::Request{id, request}`/`Data{stream, bytes}`/`UnknownStream`, and `description`'s
  existing `validate`/dispatch types, to turn a decoded request into a kernel call and a
  decoded `reply()`/`emit_event()` back out.
- **TASK-1.47** (attach): `open_stream`/`close_stream`/`send_data`, and `emit_event`
  (or a thin per-stream wrapper it is free to add) for updates on the attachment's own
  stream - no new `Kind`, per the framing settled above.
- **TASK-1.49** (CLI): `description::{description, validate, Request, Response, ApiError}`
  (unchanged, from TASK-1.43), `Connection::{new(Role::Client, ...), send_hello, call, poll}`,
  and `local_socket::{ConnectOptions, ConnectError, connect}` wired to `--autostart`/
  `--no-autostart` (`ConnectError::NoServerRunning` -> exit status 4, doc-2 line 180);
  `binary: std::env::current_exe()`.
- **TASK-1.52** (app): the same `Connection` surface as the CLI, driven from GPUI's own
  executor rather than a blocking read loop (its own concern, not this task's - R106 bars no
  particular async runtime from the sdk, and nothing here assumes one either way);
  `local_socket::connect` is blocking and must be run off whatever thread the app can afford
  to block, and the app supplies its own `binary` path rather than `current_exe()`, which
  TASK-1.52's plan has to settle, not this one.

## Commands

`just check` for the full proof (format, lint with `--all-features`, boundaries, licences,
affected tests, every package alone including the `justfile` fix above, tools' own tests,
architecture, recipe names); `just test` for the affected-crate subset while iterating. No
`cargo` invoked directly.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Order changed by @bruk.o on 2026-11-15: the handshake and framing are this task's and the server's socket task (TASK-1.44) uses them, so this comes first. The local-socket provider is tested here against a listener in a temporary directory; starting a real server end to end is proved in TASK-1.44.
<!-- SECTION:NOTES:END -->
