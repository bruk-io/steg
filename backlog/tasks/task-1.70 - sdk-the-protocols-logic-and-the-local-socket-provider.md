---
id: TASK-1.70
title: 'sdk: the protocol''s logic, and the local-socket provider'
status: Done
assignee:
  - '@bruk.w5'
created_date: '2026-11-15T02:21:00Z'
updated_date: '2026-11-15T18:45:00Z'
labels:
  - size-3
  - review-passed
milestone: m-0
dependencies:
  - TASK-1.43
references:
  - task-1.70-sdk-the-protocol-s-logic-and-the-local
  - https://github.com/bruk-io/steg/pull/30
parent_task_id: TASK-1
type: task
project: bruk-io-steg
asdlc_accepted_from: DRAFT-1
asdlc_branch: task-1.70-sdk-the-protocol-s-logic-and-the-local
asdlc_plan_approved_by: bruk
asdlc_plan_approved_hash: 'd2aa568c163d'
asdlc_plan_approved_size: size-3
asdlc_review_kind: same-machine
asdlc_review_findings: 'important=0,nit=1'
asdlc_plan_matched: yes
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
a time. The engineer (per the team lead) will get one pull request with these two commits;
whether to split further is still their call, not squeezed here.

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
  - **Correction after review:** `ClientKind` and `ClientId` are not defined here. TASK-1.42
    (planned in parallel) already defines both once - `ClientId` in `crates/sdk/src/ids.rs`,
    mirroring `SessionId`/`TabId`/`PaneId` (`from_raw`/`into_raw`/`Display`/`FromStr`/serde,
    hex string on the wire); `ClientKind` in a new `crates/sdk/src/client_kind.rs`
    mirroring `pane_kind.rs` (`App`/`Cli`/`Plugin`, `#[serde(rename_all = "snake_case")]`,
    matching the words `client.list`'s `kind` field already declares in `methods.rs`). This
    task's hello types use `crate::ids::ClientId` and `crate::client_kind::ClientKind`
    as-is, adding neither. **Landing order:** if this task lands first, it creates exactly
    those two additions (the file and the `ids.rs` type) precisely as TASK-1.42's own plan
    describes them, says so in its PR/notes, and TASK-1.42 then finds them already there
    instead of redefining them; if TASK-1.42 lands first, this task simply imports what
    already exists and adds nothing to `ids.rs`/`client_kind.rs`.

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
- `ClientHello {steg, protocol, client: String, kind: crate::client_kind::ClientKind, label:
  Option<String>}`, `ServerHello {steg, protocol, server, api, client: crate::ids::ClientId}`
  (doc-14's server-assigned id, typed as an id per doc-16 R105, not a bare string), `Refused`,
  `Refusal {Mismatch, Malformed}`, `Handshake {Accept, Refuse}` - same shape as the spike
  otherwise.
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
    `String` nobody checked): a name the description lists (via the new
    `Description::event` lookup below) comes back as `Incoming::Event`, anything else as
    `Incoming::UnknownEvent` - both carry `body: Value`. The body itself stays untyped-by-
    field on purpose: `methods.rs` already carries `layout`, `status`, and `shown_status` as
    `ValueKind::Text` placeholders pending TASK-1.39/1.41, so there are no settled per-event
    field shapes yet to decode into; typing the body is that task's to do, not this one's to
    fake.
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
    client_id: ClientId) -> (Handshake, Vec<u8>)` - the `ClientId` is minted by the caller
    (the server crate, through the kernel's `Ids::next_client`, TASK-1.42), never by the sdk:
    minting an id is the kernel's behaviour, not a shape (`ids.rs`'s own module doc).
    `reply(&mut self, id: u64, response: description::Response) -> Vec<u8>`;
    `emit_event(&mut self, name: &str, body: Value) -> Vec<u8>`.
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

## A small, explicit doc-2 change this plan proposes, for the engineer to approve separately

**Correction after review:** the provider does not invent a private `serve` entry point.
doc-2 section 3 already has the words for this: `server start`, `server stop`, `server show`
are documented methods/commands, and "a method the CLI does not expose is a failing test"
means a hidden, undocumented verb is exactly what this document forbids adding silently. But
`server.start` as it stands today (`methods.rs`: "Stays up with no clients... unlike a server
started on demand") only ever means the *explicit* half of R99's lifecycle - it has no way to
say "this start is the on-demand kind, which stops itself after 30 seconds with no sessions
and no clients." Read plainly, `server.start`/`.stop`/`.show` are RPCs a connected client
sends to a server that already exists - they cannot be how a client that has found **no**
server brings one into existence at all, since there is nothing to connect to yet and so
nothing to send an RPC to. The gap doc-2 has no words for yet is that second thing: "become a
server, as the on-demand kind, if none exists."

Proposed, smallest change (one new switch on one existing method, not a new verb):
- `crates/sdk/src/description/methods.rs`, `server_methods()`, the `server.start` entry
  gains one parameter: `switch("on_demand", "Start as an on-demand server, which stops \
  itself after 30 seconds with no sessions and no clients (doc-8 R99), rather than staying \
  up until told to stop")`. The CLI gets `--on-demand`/`--no-on-demand` for free, mechanically
  (doc-2 section 5), with no renaming by hand.
- `backlog/docs/doc-2 - Spec-CLI.md`, the `server start` row (currently `| server start |
  none | Stays up with no clients |`) becomes `| server start | --on-demand | Stays up with
  no clients, unless --on-demand is given, in which case it stops itself after 30 seconds
  with no sessions and no clients (doc-8 R99) |`.

**The contract this gives TASK-1.44 (and TASK-1.49, which builds the generated `server start`
command):** the provider's spawn step always runs exactly `<binary> server start
--on-demand`. What that command does, which is not this task's to build, is: try to connect
to `paths::socket_path` first; finding one already listening, connect as an ordinary client
and send the `server.start` RPC (so a person typing plain `steg server start` by hand, with
no flag, still gets today's documented sticky behaviour unchanged against either a fresh or
an already-running server); finding none, become the server directly - bind, detach, serve,
with R99's on-demand lifecycle when `--on-demand` was given - **without** going through the
sdk's own `local_socket::connect`'s autostart path, which would spawn another copy of itself
and recurse. That one sentence is the whole of what this task hands TASK-1.44; everything
about detaching, the 30-second idle stop, and resolving two simultaneous starts to one
survivor (R99) stays its own code, same as before this correction.

This is a change to doc-2 and to the API description, not just to this crate, so it is called
out here as its own item for the engineer to approve, separately from the rest of the plan.

## Half B: the local-socket provider (behind the `local-socket` feature)

**`crates/sdk/src/paths.rs` - no feature gate, no I/O, pure:**
- `pub fn socket_path(explicit: Option<&Path>, xdg_runtime_dir: Option<&Path>, tmp_dir:
  Option<&Path>, user: Option<&str>) -> PathBuf`. `explicit` wins outright when given (an
  override hook, read from `STEG_SOCKET_PATH` by whichever caller wants one - tests, and
  later a person's own debugging, same category as the other `STEG_*` variables doc-2
  section 4 already lists). Otherwise, preference order: `$XDG_RUNTIME_DIR/steg.sock`
  (doc-13 R60's own name), else `$TMPDIR/steg.sock` (macOS's de facto per-user, mode-700,
  runtime-only equivalent - XDG_RUNTIME_DIR is rarely set on macOS), else
  `/tmp/steg-<user>.sock` as a last resort shared-tmp fallback. Flat, no subdirectory:
  `sockaddr_un.sun_path` is 104 bytes on macOS, and a realistic `$TMPDIR` such as
  `/var/folders/zz/zyxvpxvq6csfxvn_n0000000000000/T/` plus `steg.sock` is already close to
  half that budget, so no extra directory component is spent on it. Test asserts the encoded
  path (`to_str().unwrap().len()`, with the function tested against that exact realistic
  example) stays under the limit, with a margin, rather than asserting it by inspection only -
  flagged as a real constraint, not a formality, and as a limitation if some future `$TMPDIR`
  ever runs longer.
- Pure and tested directly with fabricated inputs. Reading the real environment
  (`XDG_RUNTIME_DIR`/`TMPDIR`/`USER`/`STEG_SOCKET_PATH`) is each caller's own job: the
  provider below, and later TASK-1.44's listener, which must call this same function with the
  environment it reads so client and server converge on one path with no message ever naming
  it.
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
  mutate process-wide env vars - when spawning, this exact value is also set as
  `STEG_SOCKET_PATH` on the **child's** environment only, so a test's fake server (or later a
  real one) that calls the same `paths::socket_path` with its own inherited environment
  converges on the identical path with no process-wide env mutation anywhere.
- `ConnectError {NoServerRunning, Spawn(io::Error), Timeout, Connect(io::Error)}`.
  `NoServerRunning` is exactly what doc-2 line 180's `--no-autostart` exits 4 on - TASK-1.49
  maps it there, nothing in the sdk knows about exit statuses.
- `pub fn connect(options: &ConnectOptions) -> Result<UnixStream, ConnectError>`:
  1. Resolve the path: `options.socket_path` directly if given, else `paths::socket_path`
     fed from `env::var("STEG_SOCKET_PATH")`/`"XDG_RUNTIME_DIR"`/`"TMPDIR"`/`"USER"`.
  2. Try `UnixStream::connect(&path)`. On success, return it - a server was already running,
     nothing is spawned.
  3. On failure: if `!options.autostart`, `Err(NoServerRunning)`.
  4. Otherwise spawn `Command::new(&options.binary).args(["server", "start",
     "--on-demand"]).stdin/out/err(Stdio::null())`, setting `STEG_SOCKET_PATH` on the child's
     environment when `options.socket_path` was `Some`, and **do not wait() on the child** -
     R99's detaching, re-parenting or double-fork, and stopping are the server's own code;
     the provider drops the `Child` handle once spawn succeeds and trusts the server to have
     finished detaching by the time its socket answers. See "A small, explicit doc-2 change"
     above for exactly what this command line means and why it is this and not a private
     subcommand.
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
new dependency. The fixture does exactly what a real server is contracted to do for this
purpose: it calls the crate's own `paths::socket_path` fed from its own inherited environment
(so it honours `STEG_SOCKET_PATH` the same way `connect()`'s spawn step sets it, with no path
ever passed on its argv - its argv is ignored, standing in for the fact that a real `steg
server start --on-demand` is not told a path either), binds a `UnixListener` there, and
sleeps.

**AC -> test, half B (all in `crates/sdk/tests/local_socket.rs`, each against a temp dir, no
network, no real `steg` binary):**
- A listener is already bound at `socket_path` -> `connect()` succeeds and nothing is
  spawned, proved by pointing `binary` at a path that cannot execute and asserting no `Spawn`
  error occurs.
- Nothing is listening, `autostart: false` -> `Err(NoServerRunning)`, nothing spawned.
- Nothing is listening, `autostart: true`, `binary` is the fixture, `socket_path: Some(temp
  path)` -> `connect()` sets `STEG_SOCKET_PATH` on the spawned fixture, waits, and succeeds
  once the fixture binds at that same path.
- `connect_timeout` elapses with nothing ever binding -> `Err(Timeout)`.

**Not this task's, confirmed against TASK-1.44's own acceptance criteria:** the socket's 0600
mode, the handshake timeout, the real `server start --on-demand` entry point (including its
"connect first, become the server only if that fails, never recurse into autostart" branch),
and the lock/race resolution for R99 are all named on TASK-1.44 already and stay there.

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
  ClientHello, ServerHello, Refused, Refusal, Handshake}`, `Connection::{new,
  accept_or_refuse, reply, emit_event, open_stream, close_stream, send_data, receive, poll}`
  driven over its own socket read/write loop; `paths::socket_path` fed from its own
  environment reads (including `STEG_SOCKET_PATH` if set, so tests of its own can override
  it the same way this task's tests do); `ClientId` minted via the kernel's
  `Ids::next_client` before calling `accept_or_refuse`. Implements the `server start
  --on-demand` contract above: try to connect first, become the server only on failure,
  never recurse into the sdk's own autostart. Everything about detaching, the 30-second idle
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
  `binary: std::env::current_exe()`. Also builds the generated `server start` command,
  including its new `--on-demand` flag (mechanical, like any other switch) and the one
  hand-written exception doc-2 now needs: `server start`'s own handler does not use the
  ordinary connect-or-autostart flow every other generated command uses, since that would
  recurse into spawning itself; it tries a plain connect first and only becomes the server on
  failure, as the contract above states.
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

Revised plan checked against doc-14, doc-8 (R3, R39, R99), doc-16 (R101 to R106) and decision-2 by @bruk.o: ok. The protocol is logic that takes bytes and gives bytes and typed items, with one Connection type both ends drive and no transport trait; no dependency is added. ClientKind and ClientId come from TASK-1.42's definitions. The local-socket provider is blocking std behind its feature, tested against a plain listener. For the engineer, a change to doc-2 and the API description: server start gains an --on-demand switch, which is what the provider runs to bring a server into being; the server's side of that is TASK-1.44's. One pull request with two commits is recommended.

Check passed: `just check` exited 0 at 2026-11-15T16:31:00+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.70 on task-1.70-sdk-the-protocol-s-logic-and-the-local at b99cf4a2d4d1:

```text
ok      format check (0.1s)
ok      lint (106.2s)
ok      boundaries (0.2s)
ok      licences (0.8s)
skip    tests: this change touches no crate
ok      build (steg-sdk) (3.2s)
ok      test (steg-sdk) (1.2s)
ok      build (steg-kernel) (48.3s)
ok      test (steg-kernel) (1.3s)
ok      build (steg-server) (0.2s)
ok      test (steg-server) (0.4s)
ok      build (steg) (0.2s)
ok      test (steg) (0.3s)
ok      build (steg-app) (19.5s)
ok      test (steg-app) (1.2s)
ok      tool tests (1.6s)
ok      architecture (1.6s)
ok      recipe names (0.2s)
```

Check passed: `just check` exited 0 at 2026-11-15T16:48:01+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.70 on task-1.70-sdk-the-protocol-s-logic-and-the-local at 974e2b165d9d with uncommitted changes:

```text
ok      format check (0.1s)
ok      lint (0.6s)
ok      boundaries (0.1s)
ok      licences (0.7s)
ok      tests (every crate) (0.5s)
ok      build (steg-sdk) (0.1s)
ok      test (steg-sdk) (0.2s)
ok      test (steg-sdk, local-socket) (0.3s)
ok      build (steg-kernel) (0.1s)
ok      test (steg-kernel) (0.2s)
ok      build (steg-server) (0.1s)
ok      test (steg-server) (0.2s)
ok      build (steg) (0.1s)
ok      test (steg) (0.1s)
ok      build (steg-app) (0.3s)
ok      test (steg-app) (0.4s)
ok      tool tests (1.1s)
ok      architecture (1.1s)
ok      recipe names (0.1s)
```

Check passed: `just check` exited 0 at 2026-11-15T16:50:32+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.70 on task-1.70-sdk-the-protocol-s-logic-and-the-local at 0b6aacde5acd:

```text
ok      format check (0.1s)
ok      lint (0.4s)
ok      boundaries (0.1s)
ok      licences (0.7s)
ok      tests (every crate) (0.5s)
ok      build (steg-sdk) (0.1s)
ok      test (steg-sdk) (0.2s)
ok      test (steg-sdk, local-socket) (0.3s)
ok      build (steg-kernel) (0.1s)
ok      test (steg-kernel) (0.2s)
ok      build (steg-server) (0.1s)
ok      test (steg-server) (0.2s)
ok      build (steg) (0.1s)
ok      test (steg) (0.1s)
ok      build (steg-app) (0.3s)
ok      test (steg-app) (0.2s)
ok      tool tests (1.2s)
ok      architecture (1.0s)
ok      recipe names (0.0s)
```

Built as the approved plan's two commits, with the hard boundary kept: half A (crates/sdk/src/wire/) has zero I/O and zero feature-gating; half B (local_socket.rs, paths.rs) is entirely behind the local-socket feature and carries the crate's one #[expect].

AC -> test:
- AC #1 (frames/handshake/stream state, no I/O): wire/frame.rs, wire/handshake.rs, wire/connection.rs unit tests - all in-memory.
- AC #2 (consumer supplies the transport; client+server in memory): crates/sdk/tests/connection.rs - a Role::Client and Role::Server Connection shuttling bytes through a plain Vec<u8>, no socket.
- AC #3 (local-socket provider connects, or starts one and waits, or refuses to): crates/sdk/tests/local_socket.rs against a temp-dir listener and a fake-server-for-tests [[bin]] fixture - connects without spawning when a listener is bound, refuses with autostart:false, spawns and waits successfully, times out when nothing ever binds.
- AC #4 (version mismatch names both versions): wire/handshake.rs's mismatch tests (both directions) and tests/connection.rs's end-to-end refusal case.
- AC #5 (method called by name, validated against the Description, result/error matched by id, events as typed values, nothing hand-written): wire/connection.rs's call()/reply() and event-resolution tests, plus tests/connection.rs's full round trip; call() reuses TASK-1.43's description::validate, and events are resolved against the Description (Description::event(), a small addition mirroring method()) into Event vs UnknownEvent rather than a bare string.
- AC #6 (no GPUI type/async runtime in the public interface): no code proof needed beyond boundaries.toml's [steg-sdk] allow list (serde, serde_json only) plus just check-boundaries - this task adds no dependency.

Checks recorded on this task via asdlc record-check (just check, full green each time) before and after rebasing onto origin/main twice (TASK-1.40/1.42/1.74/1.75 landed board-notes commits in the meantime, no code collisions). CI on PR #30 passed.

Three things worth a reviewer's attention, each already flagged in the plan or discovered while building:
1. ClientId (ids.rs) and ClientKind (client_kind.rs) are TASK-1.42's own planned shapes; this task landed first, so they're created here exactly as that task's plan describes them (from_raw/into_raw/Display/FromStr/serde hex form; App/Cli/Plugin snake_case word). TASK-1.42 should find them already in place.
2. The provider's spawn step runs the already-public `<binary> server start --on-demand`, not a private subcommand - description::methods' server.start method gained the on_demand switch the team lead and engineer approved, matching the already-updated doc-2 row.
3. The local-socket feature's own behavioural tests had to move to an integration test (tests/local_socket.rs), not stay as local_socket.rs's own #[cfg(test)] unit tests: CARGO_BIN_EXE_<name> (needed to find the fixture binary) is only set for tests under tests/, not for a lib crate's own unit test binary - found while building, not assumed. That integration test carries its own second #[expect(clippy::disallowed_types)] (only that one lint fires there, not disallowed_methods) for the same reason the production module does - a test proving socket-finding behaviour has to touch a real socket. Also added a justfile step (check-packages runs steg-sdk a second time with --features local-socket) so these tests actually run under just check, since neither the default build/test nor lint's --all-features (which never executes a test) would otherwise exercise them.

Check FAILED: `cargo test -p steg-sdk --lib wire::frame` exited 101 at 2026-11-15T17:03:35+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.70 on task-1.70-sdk-the-protocol-s-logic-and-the-local at 0b6aacde5acd with uncommitted changes, last 40 of 47 lines:

```text
test wire::frame::tests::a_declared_length_of_zero_is_malformed_fed_a_byte_at_a_time ... ok
test wire::frame::tests::encode_refuses_a_payload_over_the_cap ... ok
test wire::frame::tests::an_unknown_kind_tag_is_malformed ... ok
test wire::frame::tests::a_declared_length_of_zero_is_malformed_fed_whole ... ok
test wire::frame::tests::a_frame_round_trips_through_encode_and_decode ... ok
test wire::frame::tests::a_declared_length_of_five_decodes_fed_a_byte_at_a_time ... ok
test wire::frame::tests::decode_waits_for_more_bytes_a_byte_at_a_time ... ok
test wire::frame::tests::a_declared_length_of_five_the_smallest_accepted_decodes_an_empty_payload_fed_whole ... ok
test wire::frame::tests::two_frames_back_to_back_decode_in_order ... ok
test wire::frame::tests::a_declared_length_of_four_the_largest_refused_length_is_malformed_fed_whole ... FAILED
test wire::frame::tests::a_declared_length_of_four_is_malformed_fed_a_byte_at_a_time ... FAILED
test wire::frame::tests::a_malformed_error_repeats_for_ever_until_the_caller_gives_up_on_the_connection ... FAILED

failures:

---- wire::frame::tests::a_declared_length_of_four_the_largest_refused_length_is_malformed_fed_whole stdout ----

thread 'wire::frame::tests::a_declared_length_of_four_the_largest_refused_length_is_malformed_fed_whole' (7018920) panicked at crates/sdk/src/wire/frame.rs:348:9:
assertion failed: matches!(decoder.next_frame(), Err(FrameError::Malformed(_)))
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace

---- wire::frame::tests::a_declared_length_of_four_is_malformed_fed_a_byte_at_a_time stdout ----

thread 'wire::frame::tests::a_declared_length_of_four_is_malformed_fed_a_byte_at_a_time' (7018919) panicked at crates/sdk/src/wire/frame.rs:364:17:
assertion failed: matches!(result, Err(FrameError::Malformed(_)))

---- wire::frame::tests::a_malformed_error_repeats_for_ever_until_the_caller_gives_up_on_the_connection stdout ----

thread 'wire::frame::tests::a_malformed_error_repeats_for_ever_until_the_caller_gives_up_on_the_connection' (7018924) panicked at crates/sdk/src/wire/frame.rs:406:9:
assertion failed: matches!(first, Err(FrameError::Malformed(_)))


failures:
    wire::frame::tests::a_declared_length_of_four_is_malformed_fed_a_byte_at_a_time
    wire::frame::tests::a_declared_length_of_four_the_largest_refused_length_is_malformed_fed_whole
    wire::frame::tests::a_malformed_error_repeats_for_ever_until_the_caller_gives_up_on_the_connection

test result: FAILED. 11 passed; 3 failed; 0 ignored; 0 measured; 96 filtered out; finished in 0.00s

error: test failed, to rerun pass `-p steg-sdk --lib`
```

Commits since the review: 0.

Check passed: `just check` exited 0 at 2026-11-15T17:06:49+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.70 on task-1.70-sdk-the-protocol-s-logic-and-the-local at a4a7c3556368 with uncommitted changes:

```text
ok      format check (0.2s)
ok      lint (0.4s)
ok      boundaries (0.1s)
ok      licences (0.7s)
ok      tests (every crate) (0.5s)
ok      build (steg-sdk) (0.1s)
ok      test (steg-sdk) (0.2s)
ok      test (steg-sdk, local-socket) (0.3s)
ok      build (steg-kernel) (0.1s)
ok      test (steg-kernel) (0.2s)
ok      build (steg-server) (0.1s)
ok      test (steg-server) (0.2s)
ok      build (steg) (0.1s)
ok      test (steg) (0.1s)
ok      build (steg-app) (0.3s)
ok      test (steg-app) (0.3s)
ok      tool tests (1.2s)
ok      architecture (0.8s)
ok      recipe names (0.1s)
```

Commits since the review: 24, 24 without the Asdlc-Worker trailer.

Review findings addressed (PR 30, commit 6e18352):

1. Important - Decoder::next_frame's declared-length-under-the-header bound had no test. Added wire/frame.rs tests at the bound: 0 and 4 (refused as Malformed) and 5 (the smallest accepted, a zero-payload frame), each fed whole and a byte at a time - six new tests. Bit the proof: temporarily changed `declared_len < HEADER_LEN` to `declared_len < HEADER_LEN - 1`, ran `cargo test -p steg-sdk --lib wire::frame` through asdlc record-check (recorded exit 101, three of the new tests failed exactly as expected), then restored the original bound and confirmed green again.

2. Important - connect()'s ConnectError::Spawn was never reached through connect() itself. Added two tests in tests/local_socket.rs: a binary path that does not exist, and a real file that exists but is not executable (written via std::fs::write with no execute bit) - both with autostart on and nothing listening, both now asserting connect() returns Err(ConnectError::Spawn(_)). The does-not-exist test also times the call and asserts it returns in well under a second against a 5-second connect_timeout: spawn() fails synchronously when exec itself fails, so connect() returns ConnectError::Spawn immediately, before ever reaching the retry loop's deadline - stated now on connect()'s own doc comment, and in an inline comment at the spawn() call site.

3. Nit - stated the convention that a Decoder::next_frame error (TooLarge/Malformed) is fatal: it does not drain the buffer, so calling it again repeats the identical error for ever rather than ever resynchronising; documented on Decoder's own struct doc, next_frame's doc, Incoming::Malformed's doc, and Connection::poll's doc (the caller is expected to drop the connection, not keep polling it). Pinned with a new wire/frame.rs test asserting three successive next_frame() calls after one Malformed input all return the same error.

4. Nit - documented on connect()'s doc comment that it never wait()s on the spawned child, and what that means for a long-lived consumer (the app) calling it with autostart repeatedly: a process-table entry accumulates per call until the consumer itself exits, for as long as the real server takes to detach and exit the process this spawned - attributed to TASK-1.44's own detaching code to keep short, which this function cannot avoid by waiting (R99 forbids that). Also left as a line here for TASK-1.44 to read when it builds the real detaching.

No plan change - all four are within the approved design (tests and doc comments), not new behaviour.

Checks: `just check` recorded via asdlc record-check at the new head (full green), after merging origin/main (board-notes-only commits from TASK-1.40/1.42/1.74/1.75's workers landed in the meantime, no code collisions - confirmed by diffing crates/ and justfile against origin/main before merging). CI on PR 30's new head (6e18352) passed.

AC -> test list from the original build is unchanged in substance; the two Important findings' fixes slot into the existing AC #1 (frame/decoder, no I/O) and AC #3 (local-socket provider) rows - AC #1's proof now also covers the declared-length-under-header bound explicitly, and AC #3's proof now also covers the Spawn error path explicitly, both previously implied but untested.
<!-- SECTION:NOTES:END -->

## Comments

<!-- COMMENTS:BEGIN -->
author: @bruk
created: 2026-11-15T16:26:00Z

---

Plan approved by @bruk (d2aa568c163d, size-3): its worker may build it.

---

author: @bruk.r2
created: 2026-11-15T16:53:00Z

---

@bruk.r2 is reviewing TASK-1.70: a same-machine review: it waited 0m with no other engineer's reviewer, longer than review_fallback_after (0m), so a fresh reviewer on its author's machine takes it.

---

author: @bruk.r2
created: 2026-11-15T16:59:00Z

---

Review by @bruk.r2 (same-machine): changes requested. Back to In Progress for the author's worker.

Findings: 2 Important, 2 nit. Cites: review. The diff matches the plan.

[Bugs/Evidence, Important] crates/sdk/src/wire/frame.rs:190-194 - Decoder::next_frame's check for a declared length smaller than the 5-byte header (returning FrameError::Malformed) has zero test coverage anywhere in the crate. This is exactly the hostile-input case the review brief named ("a length under the header size"); nothing in frame.rs's own tests, connection.rs's unit tests, or tests/connection.rs exercises it, so a regression here (e.g. an off-by-one on the bound, or a panic from indexing before the length check) would go undetected.

[Bugs/Evidence, Important] crates/sdk/src/local_socket.rs:144 and crates/sdk/tests/local_socket.rs - connect()'s ConnectError::Spawn path (binary cannot be executed, with autostart:true and nothing listening) is never exercised through connect() in any test, even though the review brief explicitly named "what happens if the binary does not exist" as a case to prove. The two tests that build binary_that_cannot_execute() both short-circuit before Command::spawn() is ever called (one via an already-bound listener so connect succeeds first, one via autostart:false so the NoServerRunning branch returns first); the only test touching ConnectError::Spawn checks Display formatting of a fabricated error, not that connect() itself returns that variant on a real spawn failure.

[Nit] crates/sdk/src/wire/frame.rs: next_frame does not drain the buffer on a Malformed error (unknown kind tag, undersized length), so a caller that calls poll() again after seeing one Malformed sees the identical error forever rather than the connection ever being able to recover. Likely fine if every caller treats Malformed as fatal and drops the connection, but that convention isn't stated.

[Nit] crates/sdk/src/local_socket.rs: connect() never wait()s on the spawned Child (by design, per the plan). If a long-lived consumer (the app) calls connect with autostart repeatedly over its lifetime and the real server's own detach leaves a short-lived intermediate process, this accumulates zombie process-table entries until the consumer exits. Attributed to TASK-1.44's detaching code, but worth a line noting the consumer-side consequence explicitly.

Everything else checked out: the two-commit boundary is clean (half A is I/O-free and feature-gate-free; half B is entirely behind local-socket, carrying the crate's one #[expect] plus its test-module twin, both scoped correctly per .claude/rules/sdk.md); the ClientId/ClientKind definitions match TASK-1.42's own plan verbatim (confirmed by reading TASK-1.42's plan directly); server.start's on_demand switch matches doc-2's already-approved row; no new dependency in Cargo.toml or boundaries.toml, and boundaries.toml's existing deny-features still blocks steg-kernel from local-socket; the justfile gap is fixed and CI on the PR's head commit (0b6aacde) shows "test (steg-sdk, local-socket)" actually running and passing under just check; the recorded just check is at the branch head. All the hostile-input cases I could find tests for (oversized length incl. the exact 0xFFFF_FFFF case, truncated/byte-at-a-time frames, two frames in one push, unknown kind tag, bad magic, non-JSON hello, missing field, unknown field ignored, client-newer and server-newer version mismatches both naming both versions, data on an unregistered stream, event name not in the Description) are present and look correct by inspection.

A matching open draft already exists (DRAFT-1, "a named error/edge-case branch needs a test exercising it, not just a happy-path test", filed against TASK-1.39/TASK-1.35) that covers exactly this mistake pattern, so no new rule-change draft was filed for it.

---

author: @bruk.r2
created: 2026-11-15T17:09:00Z

---

@bruk.r2 is reviewing TASK-1.70: a same-machine review: it waited 0m with no other engineer's reviewer, longer than review_fallback_after (0m), so a fresh reviewer on its author's machine takes it.

---

author: @bruk.r2
created: 2026-11-15T17:12:00Z

---

Review by @bruk.r2 (same-machine): clean. It stays In Review for a person to merge.

Findings: 0 Important, 1 nit. The diff matches the plan.

Second review, head 6e18352 (PR #30). Both Important findings from the prior review (bruk.r2, 0b6aacde) are genuinely fixed:

1. [Bugs/Evidence] crates/sdk/src/wire/frame.rs - Decoder::next_frame's declared-length-under-header bound (HEADER_LEN=5) is now covered by six new tests (declared length 0 and 4 refused as Malformed, 5 accepted as the smallest zero-payload frame, each fed whole and a byte at a time). The worker recorded a deliberate-fault run (temporarily changed the bound to `< HEADER_LEN - 1`) that failed exactly these new tests (exit 101, recorded on the task at 17:03:35), then restored it - real proof the tests can fail.

2. [Bugs/Evidence] crates/sdk/src/local_socket.rs - connect()'s ConnectError::Spawn path is now reached through connect() itself by two new tests in tests/local_socket.rs (autostart:true, nothing listening, binary path nonexistent or non-executable-but-present). Read connect()'s production code (crates/sdk/src/local_socket.rs:131-170): both paths genuinely call Command::spawn() and surface its io::Error via ConnectError::Spawn - not a fabricated-error Display test. The "elapsed < 1s against a 5s connect_timeout" assertion is not flake-prone: spawn() failure (ENOENT/EACCES) is a synchronous syscall failure, not a scheduling-dependent wait.

Both nits from the prior review are addressed with doc comments (Decoder's struct doc, next_frame's doc, Incoming::Malformed's doc, Connection::poll's doc stating the fatal/non-draining convention; connect()'s doc stating the never-wait()s-on-the-child consequence for a long-lived caller), each pinned by a test where one was feasible (next_frame's "repeats forever" test).

CI on PR #30's head (6e18352) is green ("check" workflow, SUCCESS), and the recorded `just check` at this head shows "test (steg-sdk, local-socket)" passing, so the local-socket feature's tests are proven to run under just check (the justfile gap fix, verified at justfile:98-117, and boundaries.toml's steg-sdk allow-list/deny-features, verified unchanged). No new dependency. ClientId/ClientKind, the doc-2 on_demand switch, and the two-commit half A/half B boundary all match the plan, consistent with the prior review's own findings on those points, which I re-checked by reading the diff directly rather than relying on the earlier review's word.

[Nit] crates/sdk/tests/local_socket.rs, binary_that_exists_but_is_not_executable() - writes a uniquely-named temp file via std::fs::write and never removes it; harmless (OS temp dir, unique per process+counter) but a small accumulating leak across test runs.

No findings repeat a mistake from an earlier review of a different task (the one earlier citation, DRAFT-1, already covers this pattern and was already filed), so no new rule-change draft.

---

created: 2026-11-15T17:16:00Z

---

After the clean second review at 6e18352, the worker removed the leftover temp file in tests/local_socket.rs with a delete-on-drop guard (7dec863) and merged main after TASK-1.74 landed (head f097569). Tests only besides the merge; the merged crates/sdk/src/lib.rs read by @bruk.o. The board locks a task after a clean review, so the check is the pull request's CI: green at f097569.

---
<!-- COMMENTS:END -->
