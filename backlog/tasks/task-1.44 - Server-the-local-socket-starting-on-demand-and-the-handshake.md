---
id: TASK-1.44
title: 'Server: the local socket, one server per socket, and the handshake'
status: In Progress
assignee:
  - '@bruk.w3'
created_date: '2026-11-15T01:32:00Z'
updated_date: '2026-11-15T19:43:00Z'
labels:
  - size-3
milestone: m-0
dependencies:
  - TASK-1.69
  - TASK-1.70
modified_files:
  - crates/server/src/config.rs
  - crates/server/src/paths.rs
  - crates/server/src/bind.rs
  - crates/server/src/arbiter.rs
  - crates/server/src/connection.rs
  - crates/server/src/listen.rs
  - crates/server/src/error.rs
  - crates/server/src/lib.rs
  - crates/server/tests/handshake.rs
  - crates/server/tests/shutdown.rs
  - crates/server/tests/race.rs
  - crates/server/tests/socket_mode.rs
  - crates/server/tests/fixtures/race_attempt.rs
  - crates/server/Cargo.toml
  - boundaries.toml
parent_task_id: TASK-1
type: task
project: bruk-io-steg
asdlc_accepted_from: DRAFT-14
asdlc_branch: task-1.44-server-the-local-socket-starting-on
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
doc-8: R1, R3, R4, R5, R39, R99; doc-13 R60, R61. decision-2, section 7. Bruk decided the lifecycle on 2026-11-15: started by the first client, not by launchd, and everything about starting, detaching and stopping is the server's own code (R98).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The server listens on a socket at an XDG path that only its owner can open, with the mode set rather than assumed, and refuses a directory or an existing file it cannot trust
- [ ] #2 Several servers started at the same moment for one socket leave exactly one listening; the others find it and exit without error. A socket file left by a dead server is replaced
- [ ] #3 A client and server whose protocol versions differ get a message naming both versions
- [ ] #4 A connection that never completes its handshake is closed after a timeout; one that has completed it is not closed for being idle
- [ ] #5 The handshake and the version check come from the sdk (TASK-1.70, doc-16 R101); the server runs in the foreground of the process that called it, with no detaching, so this part is testable alone
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
## Before anything else

This task is now part A only, after the engineer's split along the seam this task's own
earlier plan proposed: the socket, the one-server-per-socket arbiter, and the handshake. Part
B (starting on demand through the sdk's provider, detaching, the login-shell environment, the
30-second idle stop, and the `steg` entry point) is a separate draft under TASK-1 that depends
on this one - not planned here.

TASK-1.40 (pane programs) merged to `origin/main` since this task's first plan was written
(PR #32, now `Done`) - `crates/server` already has `dispatch/`, `env.rs`, `error.rs`, `pane.rs`,
`pty.rs` on top of what the first plan saw. Everything below accounts for that: this plan adds
new modules beside those, touches the existing `error.rs` rather than adding a second error
type, and does not create `env.rs` (TASK-1.40's own file for `pane_environment`, unrelated to
this task). Phase 2 still starts by rebasing the worktree's branch onto `origin/main`.

Part A's five acceptance criteria, as the task now states them:

1. The server listens on a socket at an XDG path that only its owner can open, with the mode
   set rather than assumed, and refuses a directory or an existing file it cannot trust.
2. Several servers started at the same moment for one socket leave exactly one listening; the
   others find it and exit without error. A socket file left by a dead server is replaced.
3. A client and server whose protocol versions differ get a message naming both versions.
4. A connection that never completes its handshake is closed after a timeout; one that has
   completed it is not closed for being idle.
5. The handshake and the version check come from the sdk (TASK-1.70, doc-16 R101); the server
   runs in the foreground of the process that called it, with no detaching, so this part is
   testable alone.

### Order of work

1. `paths.rs` - `lock_path`, `directory_is_safe`, and the existing-file trust check (type and
   owner). Unit tests first: a real, writable-by-anyone temp directory (no second user needed)
   and fabricated metadata for the owner-mismatch case (the honest gap, named below).
2. `bind.rs` - bind under a temporary name, set the mode, `rename` onto the final path. Real
   -socket tests (no fabrication needed: a real bind, a real `fs::metadata` read-back).
3. `arbiter.rs` - the pure `decide()` first, exhaustive unit tests, no I/O; then `attempt()`
   wired to a real stat, a real connect, and `nix`'s `Flock`.
4. `connection.rs` - the per-connection handshake: a real read timeout, the sdk's
   `wire::Connection` for decoding and replying, clearing the timeout once accepted, and the
   `ConnectionEvent` seam (section on "What part B inherits", below).
5. `listen.rs` - the accept loop (`run`), the `Stopper` that tells it to stop, and `serve`
   (`attempt` then, on `Became`, `run`) for this task's own foreground, no-detach case.
6. `config.rs` - `ServerConfig`: the one place every timing and path this task's tests
   override lives, each with a production default.
7. `error.rs` - extend the existing `steg_server::Error` (section on error handling, below);
   `lib.rs` - add `mod`/`pub use` lines beside the existing ones, nothing removed.
8. `Cargo.toml`, `boundaries.toml` - add `nix` (section on dependencies, below).
9. Tests mapped to each criterion (the "Proof" section) - written against the real modules as
   each becomes available, not held until the end.
10. `just check`, recorded with `asdlc record-check`.

## 1. Where each part lives, and the API part B wraps

Everything here is `steg-server`, added beside TASK-1.40's `dispatch`, `env`, `error`, `pane`,
`pty` - none of those are touched except `error.rs` (extended, not replaced) and `lib.rs` (new
`mod`/`pub use` lines only). Nothing in `crates/steg` changes: part A has no binary entry
point of its own (the task's criterion 5 says so explicitly - "the server runs in the
foreground of the process that called it" means the *caller*, which for this task is a test,
never a `steg server start` invocation; that invocation, and the `steg` package's own
narrow argument handling for it, is part B's).

| File | Holds |
|---|---|
| `config.rs` | `ServerConfig { socket_path: Option<PathBuf>, handshake_timeout: Duration, arbitration_timeout: Duration }`, each overridable, with a production default - the injection point every test uses instead of sleeping real durations. `arbitration_timeout` bounds the lock-contended retry loop (section 2, step 3) - without its own field the only way to test that branch would be to wait out a production-length duration, which is how a test quietly becomes a sleep. |
| `paths.rs` | `lock_path(&socket_path)` (sibling `.lock` file - not subject to `sockaddr_un`'s 104-byte limit, since it is only ever opened, never bound); `directory_is_safe(dir) -> Result<(), Reason>`; `existing_file_trust(path) -> ExistingFile` (an enum: `Absent`, `OursAndASocket`, `NotASocket`, `NotOurs` - see section 3). |
| `bind.rs` | `bind_with_mode(path) -> io::Result<UnixListener>`: binds under a sibling temporary name in the same directory, sets `0600` via `std::fs::set_permissions`/`PermissionsExt`, `rename()`s onto `path`. |
| `arbiter.rs` | The pure, total `fn decide(existing: ExistingFile, probe: Probe, lock: LockOutcome) -> Arbitration` (`Probe`: `Answered | Refused` - the connect attempt's own result, kept out of `ExistingFile` so `decide` holds the whole decision rather than `attempt` quietly making the interesting call itself; `Arbitration::Became(ServerHandle) | AlreadyRunning | Refuse(Reason)`, every variant `Ok` - see section 6), exhaustively unit-tested with no I/O; `attempt(config: &ServerConfig) -> Result<Arbitration, Error>`, the real version, which does the stat, the connect and the lock attempt, feeds their results into `decide`, binds only on `Became`, and reserves `Err` for the I/O failures section 6 names. |
| `connection.rs` | The per-connection thread body: set a read timeout, decode and answer the hello (`steg_sdk::wire::Connection`), clear the timeout once accepted, emit `ConnectionEvent`s, then hold the connection open until it closes on its own (this task reads nothing past the hello; TASK-1.45 is what routes further frames). |
| `listen.rs` | `ServerHandle` (the bound listener + the held `Flock<File>` lock, dropped together); `Stopper` (the handle that tells `run` to stop, and how it wakes a blocked `accept()`); `run(handle, config, stop, events) -> io::Result<()>` (the accept loop, until stopped); `serve(config, stop, events) -> Result<ServeOutcome, Error>` (`attempt` then, on `Became`, `run`; returns immediately on `AlreadyRunning` or a refusal). |
| `lib.rs` | Adds `mod config; mod paths; mod bind; mod arbiter; mod connection; mod listen;` beside the existing five, and `pub use` for `serve`, `ServeOutcome`, `ServerConfig`, `Stopper`, `ConnectionEvent`, `arbiter::attempt` (for part B and this task's own race test, below), `listen::run` (for part B's post-detach call). |

**The contract part B wraps, stated once:** `serve()` is exactly `attempt()` then, if it became
the server, `run()`; it is what this task's own tests call for a self-contained, no-detach
server. Part B's stage 1 (still attached, pre-detach) calls `attempt()` directly, carries the
resulting `ServerHandle` across its own fd-passing re-exec, and stage 2 (post-detach) calls
`run()` directly - the same two functions this task builds and tests, composed around a detach
step this task adds nothing of its own to. Nothing in this task needs to anticipate exactly how
B calls them beyond their existing signatures.

## 2. The arbiter: one survivor (AC #2's first half), and a dead server's stale socket (AC #2's second half)

The lock, not `bind`, is the arbiter (binding twice at the same path cannot distinguish "stale
file, nothing listening" from "another process already bound here" without first unlinking,
and unlink-then-bind is itself the unresolved race it would be trying to settle).

1. **Existing-file trust (`paths::existing_file_trust`, AC #1's "refuses... an existing file it
   cannot trust").** `stat` the path before touching it at all:
   - **Absent.** Nothing there - proceed to the lock.
   - **A socket, owned by the caller's own euid.** Try connecting. A successful connect means a
     real server of ours is up: `Arbitration::Became` is not possible here, report
     `AlreadyRunning` and stop (both AC #2's first clause and AC #5 - this is the same path a
     losing racer in a simultaneous start takes). A refused connect means the owning process is
     dead and the file is stale - safe to remove once the lock is held (step 3).
   - **A socket, owned by someone else.** Never connected to (connecting to a socket we do not
     own and trusting a reply from it is the exact thing R5 forbids) and never removed (we did
     not create it, and in the one place this matters in practice - the world-writable
     `/tmp/steg-<user>.sock` fallback - the directory's own sticky bit would refuse our unlink
     anyway). `decide()` returns `Arbitration::Refuse` naming why.
   - **Not a socket at all** (a plain file, a directory, anything else left at that exact
     path). Refused the same way, for the same reason: we did not put it there and do not know
     what it is for. This branch is fully testable for real with no second user - pre-create an
     ordinary file at the path and assert the refusal.
2. **Directory safety (`paths::directory_is_safe`).** The socket's parent directory must be
   owned by the caller and not writable by anyone else. Also fully testable for real with no
   second user: `chmod 0777` a real temporary directory and assert the refusal; the only
   untestable half of this and of step 1's "owned by someone else" branch is the cross-uid
   case itself, which this environment has no second real user or root to produce - said
   plainly here and in "left out", not hidden.
3. **Lock attempt**, `nix::fcntl::Flock::lock(lock_file, FlockArg::LockExclusiveNonblock)` on
   the sibling `.lock` file, confirmed against the real crate (not assumed): contention surfaces
   as `Err((file, Errno::EAGAIN))`, verified by compiling and running exactly this call with two
   file descriptors open on one file in one process, which is a stronger proof of the
   contention path than two real processes would be forced to show differently.
   - **Acquired.** Remove any stale socket file found in step 1 (safe now: step 1 already
     proved either nothing was there or the owner is dead), bind (section "bind", above), and
     report `Arbitration::Became` with the `ServerHandle`.
   - **Not acquired** (`EAGAIN`). Another process holds it right now, mid-becoming-the-server.
     Retry the connect probe from step 1 on a short bounded loop up to `config
     .arbitration_timeout` (this plan's own deadline, not sdk's `local_socket::connect`'s
     retry, which this task's code never touches). Connects within it: `AlreadyRunning`,
     converged on the winner - this is what proves AC #2's first clause at the unit level
     (`decide()`'s own tests, fed `Probe::Answered` after one or more `LockOutcome::HeldByOther`
     results) and end to end (the real multi-process test below). Never connects before the
     deadline: `Refuse(Reason::ArbitrationTimedOut)` - its own test, section "Proof", feeds a
     short injected `arbitration_timeout` against a lock a fixture holds and never releases
     within it, so the branch is proved inside a bound, never by waiting out a production
     -length timeout.
4. **A dead server's stale socket self-heals because the lock does, not because anything here
   detects death.** `Flock` releases on drop, which runs when its holder's process ends by any
   means including `SIGKILL` - the kernel does this, not this crate. The real test for this
   (not merely a stale file with no listener, which only proves the unlink): a small fixture
   process binds a socket and takes the lock, gets `SIGKILL`ed, and a fresh `attempt()` is
   asserted to acquire the lock and replace the file. A pre-created file with nothing holding
   the lock proves less - it never exercises the lock actually being free because its holder
   died, only that a stale file is removed once the lock (never contended) is taken.

`decide()` itself takes only `ExistingFile` and `LockOutcome` - nothing in this task's
`ServerConfig` distinguishes an explicit start from an on-demand one (that distinction, and
what it changes, is entirely part B's).

## 3. The handshake: version mismatch (AC #3), the timeout, and not closing an idle completed one (AC #4)

Reuses `steg_sdk::wire::Connection` exactly as TASK-1.70 built it - nothing in this task adds a
line to the sdk. `connection.rs`'s per-connection thread:

1. `UnixStream::set_read_timeout(Some(config.handshake_timeout))` before the first read.
2. Decode frames with the sdk's `Decoder`/`Connection::poll` until a `Hello` arrives or the read
   times out.
3. On a timed-out read: close the connection with no reply (AC #4's first clause). On a `Hello`:
   `Connection::accept_or_refuse` (the sdk's own, unmodified) decides accept or refuse by
   protocol version (AC #3 - the refusal already names both versions, proved by the sdk's own
   tests; this task's test proves the *server* delivers that refusal over a real socket, not
   that the refusal's wording is right, which is TASK-1.70's proof already).
4. On accept: `set_read_timeout(None)` - this is what AC #4's second clause rests on. A
   connection that has finished its handshake is never touched by this timeout again, proved by
   a test that completes a handshake, waits past where the timeout would have fired, and asserts
   the connection is still open.
5. On refuse: write the refusal frame, then close.

**`ConnectionEvent`, the seam part B inherits.** `run`'s accept loop sends
`ConnectionEvent::Connected` the moment `accept()` returns a connection - before any byte is
read - and the per-connection thread sends `ConnectionEvent::Disconnected` on every exit path
(timed-out, refused, or a clean close after acceptance), over an optional channel `run` takes.
This task has no accounting to feed it to - there is no idle rule here, and "not closed for
being idle" in AC #4 is proved directly by step 4 above, not by anything watching this channel.
Part B's idle checker is the real consumer: it holds the same channel, counts
`Connected`/`Disconnected` into its own clock-injected predicate, and calls `Stopper::stop()`
when its rule says to - nothing about `run`'s own signature changes to make that possible,
which is the point of building the seam now rather than letting part B add it later.

## 4. Shutdown, in the foreground (part of AC #5, and the seam's other half)

`run` takes a `Stopper` (a small `Arc`-backed handle with `.stop(&self)`); this task's own
tests build one, pass a clone into `run` (spawned on a background thread or called directly,
depending on the test), and call `.stop()` from elsewhere once they are done asserting. The
accept loop blocks in `accept()`; `.stop()` sets a flag and then connects a throwaway
`UnixStream` to the server's own socket to unblock it (the standard way to wake a thread parked
in `accept()` with no other listener-level cancel in std) - the accept loop treats that
connection as nothing (it never reaches the handshake step) and, seeing the flag, breaks out.

**Shutdown order, stated so a client connecting in that instant has a definite, safe
experience:** stop accepting (break out of the loop) → close the listener (drop it) → remove
the socket file at its final path → release the lock (drop the `Flock` guard) last, never
before the path is fully vacated. A client whose `connect()` lands in this window sees exactly
what it would see against a server that was never there (`ConnectionRefused`, or - once the
file is removed - `NotFound`) - indistinguishable from "nothing is listening," which is the
correct, safe reading for any client (the sdk's own `local_socket::connect`, part B's future
caller of this, treats that as "try to start one," never as an error of its own).

Part B's idle checker will hold the same `Stopper` this task defines and call `.stop()` once
its clock-injected predicate says to; this task's own tests call it directly, standing in for
that future caller with no change needed to `run` or `Stopper` themselves.

## 5. Dependencies, verified against the real crate rather than assumed

One new external dependency: **`nix` (`"0.28"`), `steg-server` only**, with features `fs` (for
`fcntl::Flock`/`FlockArg`) and `user` (for `unistd::geteuid`) - both confirmed by compiling and
running exactly the calls this task needs, not inferred from memory:

- `nix::fcntl::{Flock, FlockArg}`: `Flock::lock(file, FlockArg::LockExclusiveNonblock)` returns
  `Ok(Flock<File>)` (derefs to the file, unlocks on drop) or `Err((file, Errno))`; a contended
  lock surfaces as `Errno::EAGAIN`, confirmed by actually contending one in a real run, not
  assumed from the type signature.
- `nix::unistd::geteuid() -> Uid`, with `.as_raw()` to compare against
  `std::os::unix::fs::MetadataExt::uid()` (a plain `u32`) for the owner checks in section 2 -
  confirmed to need the `user` feature specifically (it is not in nix's base, unguarded
  surface; a first attempt without it failed to compile with exactly that message).
- Socket type (`FileTypeExt::is_socket()`) and mode (`PermissionsExt`) need no `nix` or `libc`
  call at all - `std::os::unix::fs` already covers both, confirmed the same way.

**No `setsid`, no `getpwuid_r`, no `fcntl(F_SETFD)`, no `FromRawFd` reconstruction of an
inherited descriptor, and no `unsafe` block of this task's own anywhere** - detaching, the
login shell, and fd-passing across an exec are all part B's; this task never forks, execs, or
crosses a process boundary with anything but a plain, independently-bound socket and lock.
`nix`'s own API for `Flock`/`geteuid` is safe Rust; this task adds none of the
`sys.rs`-with-hand-written-FFI the original, pre-split plan carried.

`nix` 0.28.0 is already resolved in the workspace's `Cargo.lock`, pulled in transitively through
`portable-pty` (TASK-1.40's dependency, also in `steg-server`) - declaring `nix = "0.28"`
directly resolves to that same line rather than adding a second version, checked with
`cargo tree -p steg-server -i nix` showing exactly one entry once this is wired in (not assumed
now: `portable-pty`'s *own* choice of `nix` version is not this task's to control, and a future
bump of it could move that line, which is why this is a check to run in the build phase, not a
fact to assert here). `nix` is `MIT`, read directly from its own `Cargo.toml`'s `license` field
rather than taken on trust, and `MIT` is already in `deny.toml`'s allow list - `just deny` is
still what proves it for real, the same way TASK-1.40 proved `portable-pty`'s licence.
`boundaries.toml`'s `[steg-server]` gains `"nix"` in `allow`, with that reason, and
`just check-boundaries` is the check that the features actually declared (`fs`, `user`) match
what a reviewer sees named here.

## 6. Error handling

**This task adds no new variant to `steg_server::Error` at all.** The arbiter's refusals (an
unsafe directory, an untrusted existing file, arbitration timing out) are not errors in the
sense `rust.md` means by the word - each is a normal, decided outcome a caller must act on, not
a failure the caller could not have avoided by deciding differently. They are carried as
`Arbitration::Refuse(Reason)`, a plain `Ok` value `decide()`'s own total, pure signature
already returns (section 1's table), with `Reason` a small enum local to `arbiter.rs`
(`NotASocket`, `NotOurs`, `UnsafeDirectory`, `ArbitrationTimedOut`), never wrapped in
`steg_server::Error`. Adding a second, parallel error type instead - the alternative this plan
considered and rejected - would make TASK-1.45, which has to handle both a socket failure and
a dispatch failure on one call path, carry conversion boilerplate between `Error` and a new
`ListenError` for no reason this task's own criteria need; keeping refusals as plain data
avoids the question entirely rather than resolving it by extending `Error` for values that are
never actually `Err`.

`Error::Io` (already there, `error.rs`, TASK-1.40's own file) is the one thing a genuine I/O
failure in this task's own code becomes: `nix::errno::Errno` converts to `std::io::Error`
directly (`impl From<Errno> for io::Error`, confirmed by reading `nix-0.28.0`'s own
`errno.rs`, not assumed), and `error.rs`'s existing `impl From<std::io::Error> for Error`
already covers the rest - so a `Flock::lock` failure other than the one errno `attempt`
matches on explicitly (`EAGAIN`, which is never an `Error`, only ever feeds `decide()`'s
`LockOutcome::HeldByOther`) reaches a caller exactly the way a failed `bind` or `rename`
already does, no new variant needed for that either. `attempt(config: &ServerConfig) ->
Result<Arbitration, Error>` - `Arbitration::Became(ServerHandle)`/`AlreadyRunning`/
`Refuse(Reason)` are all `Ok`; `Err` is reserved for the I/O this paragraph names.

`Io(std::io::Error)` already exists and covers bind/stat/rename failures with no new variant.

## 7. Threading model

Blocking `std::thread`: one thread per connection (handshake, then held open), the accept loop
on whichever thread calls `run` (this task's own tests call it on a spawned thread so the test
can keep driving connections and the `Stopper` from its own main thread). No new async runtime;
consistent with TASK-1.40's own choice (one thread per pane) already on `origin/main`.

### Risks

- **The cross-uid cases (section 2, steps 1 and 2) are unverified against a real second user
  or root** - both unavailable in this environment. Fabricated-metadata unit tests cover the
  logic; the world-writable-directory and not-a-socket cases are proved for real instead,
  narrowing the honest gap to ownership specifically.
- **`nix`'s version resolving to the single, already-present `0.28.0` line is a fact to check
  in the build phase (`cargo tree -p steg-server -i nix`), not to assume from the version string
  alone** - `portable-pty`'s own choice of `nix` is outside this task's control and could move.
- **The fixture-process test for AC #2's dead-server clause (section 2, step 4) depends on
  `SIGKILL` reliably releasing a real `flock` on this machine** - expected POSIX behaviour, but
  proved by running it, not assumed; if it does not hold the way expected here, that is itself
  a finding worth surfacing rather than working around silently.
- **The race test's winning fixture must flush its outcome line before entering `run`'s
  blocking accept loop**, or the test reads nothing from a process now parked in `accept()` -
  named here so it is not rediscovered as a confusing hang in the build phase.

## 8. Explicitly left out, naming part B for each

- **Starting on demand through the sdk's `local_socket::connect`, and everything about
  detaching** (fd-passing re-exec, `setsid`, surviving the client that triggered the autostart)
  - part B.
- **The login-shell environment capture** - part B.
- **The 30-second idle-with-no-sessions-and-no-clients stop, and its clock-injected
  predicate** - part B, consuming the `ConnectionEvent` seam (section 3) and holding the
  `Stopper` this task defines (section 4); this task adds no session count, no clock, and no
  `on_demand` distinction anywhere in `ServerConfig`.
- **The `steg` entry point's `server start [--on-demand]` handling** - part B; nothing in
  `crates/steg` changes here.
- **Routing real requests past the hello** (`session.*`/`tab.*`/`pane.*`/`api.*` control
  frames) - TASK-1.45's named scope, unchanged by this task; `connection.rs` holds an accepted
  connection open and reports its liveness, nothing more.
- **An explicit start's distinct "already running" message** - there is no explicit/on-demand
  distinction in this task at all; `ServeOutcome::AlreadyRunning` and `Arbitration::AlreadyRunning`
  (section 1) are values a future caller can act on, but nothing here decides what that caller
  prints.

### Proof

Tests, mapped to the five acceptance criteria. Files:
`crates/server/src/paths.rs`/`bind.rs`/`arbiter.rs`'s own `#[cfg(test)]` modules for everything
pure or needing only a real socket/file, no second process and no umask mutation;
`crates/server/tests/socket_mode.rs` (the one test that sets the process umask - its own
integration-test file, so `cargo test` gives it its own process rather than sharing one with
every other unit test in the crate's lib test binary, which a umask mutation inside `bind.rs`'s
own `#[cfg(test)]` module would otherwise corrupt under `cargo test`'s default parallelism);
`crates/server/tests/handshake.rs` (AC #3, #4) and `crates/server/tests/shutdown.rs` (section
4's ordering) for functional, in-process `serve`/`run` tests; `crates/server/tests/race.rs`
plus a small fixture binary declared as its own `[[bin]]` in `crates/server/Cargo.toml`
(`crates/server/tests/fixtures/race_attempt.rs`, the same shape the sdk's own
`fake-server-for-tests` already establishes, so `CARGO_BIN_EXE_race-attempt` is set for
`race.rs` - a path under `tests/fixtures/` is not auto-discovered as a binary on its own).
`steg-server` has no Cargo features of its own today, so this `[[bin]]` always builds; stated
here rather than left for a reviewer to ask why `just check-packages` now builds one more
binary for this crate.

**No sleeps as synchronisation, with one named exception.** Every wait but one is on a real
event: a socket closing or refusing, a channel receiving, a process exiting. The exception is
AC #4's second clause - "one that has completed it is not closed for being idle" is a negative
proof (nothing happens), which has no event to wait on since the event never fires by design;
that one test waits a short, injected multiple of `handshake_timeout` past where the timeout
would have fired and asserts the connection is still open, named as the exception it is rather
than silently waiting like the rest.

- **AC #1 (mode and path checked and trusted, not assumed)**: `socket_mode.rs` sets the
  process umask to `0`, binds for real through `bind_with_mode`, and asserts the final file is
  still `0600` regardless. `paths::existing_file_trust`'s tests (in `paths.rs`, no umask
  involved, safe beside the crate's other unit tests): absent, ours-and-a-socket-but-dead
  (refused connect, proceeds), not-a-socket (a real plain file, real refusal), not-ours
  (fabricated metadata, the honest gap). `paths::directory_is_safe`: a real world-writable temp
  directory, real refusal.
- **AC #2 (one survivor; a dead server's socket replaced)**: `arbiter::decide`'s pure branches,
  exhaustive. Then two real, multi-process tests in `race.rs`: (a) eight copies of
  `fixtures/race_attempt.rs` spawned at once against the same socket path, each calling
  `arbiter::attempt` directly and printing (flushed before any possible block in `run`) which
  `Arbitration` it got, then - only the one that got `Became` - calling `listen::run` and
  blocking until the test `kill()`s and `wait()`s it; the test asserts exactly one `Became`,
  the rest `AlreadyRunning`, and every process exited 0 or was cleanly reaped after the kill.
  (b) a fixture binds and locks, is `SIGKILL`ed, and a fresh `attempt()` in the test process
  itself is asserted to win the lock and find the stale file replaced - the test named in
  section 2's step 4 as proving lock-release-on-death specifically, not merely a stale file.
  A third, in-process case for the branch section 2's step 3 names but the above two never
  reach: a second `File` handle on the same lock path holds the lock for real (the same
  technique this plan's own nix verification used to produce a real `EAGAIN`) and is never
  released within a short, injected `arbitration_timeout`; `attempt()` is asserted to return
  `Refuse(Reason::ArbitrationTimedOut)` inside that bound, not by waiting out the production
  default.
- **AC #3 (version mismatch names both)**: `handshake.rs`, a real socket, `serve`/`run` in the
  background, a raw `UnixStream` sending a hand-built `ClientHello` at a wrong protocol version
  (via the sdk's already-public `Frame::hello`/`encode_frame`), asserting the refusal (read
  with the sdk's own `parse_server_hello`) names both versions - proving this task delivers the
  sdk's own, unmodified decision over a real connection, not re-proving the decision itself.
- **AC #4 (handshake timeout; not idle-closed after completing)**: `handshake.rs`, connect and
  send nothing, the test's own blocking read (timeout slightly over the server's short injected
  one) observes the close - waiting on the event, not sleeping first. A second case: a complete
  handshake, then an idle wait past where the timeout would have fired, asserting the connection
  is still open.
- **AC #5 (sdk's handshake; foreground, no detach, testable alone)**: proved by construction -
  every test above drives `serve`/`attempt`/`run` directly, in-process or via the small fixture
  under `tests/fixtures/`, with no `steg` binary anywhere in this task's own test suite. Said
  explicitly here, the way the previous plan named its own analogous criterion, rather than
  left for a reader to infer from the absence of `crates/steg` in the modified-files list.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Plan checked against doc-8 (R1, R3-R5, R39, R98, R99), doc-13 (R60, R61), doc-14, doc-16 and decision-2 section 7 by @bruk.o: sound, not yet put up for approval. Four things for the engineer or the worker first. (1) Size: the plan itself says this is two reviewable changes (A: socket, arbiter, handshake; B: detach, login environment, idle rule, entry point); the engineer decides whether to split the task. (2) Unsafe: the plan adds libc with a hand-written sys.rs (flock, setsid, getpwuid_r, fcntl), an unsafe pre_exec closure and FromRawFd. nix 0.28/0.29 (MIT) is already in Cargo.lock through portable-pty and offers safe wrappers for all four calls, and stage 2 calling setsid itself at start-up (it is not a process-group leader) would remove the pre_exec closure; ask the worker to reduce unsafe to the one FromRawFd block or say why not. (3) STEG_TEST_REPORT_OUTCOME_TO is a test-only hook in the shipped binary; prefer stage 1 printing its outcome (started / already running) on standard output, which a person also benefits from. (4) State what a client sees when it connects in the instant an idle server is stopping (accept closed, socket file removed, lock released, in which order), and that the session count wired to zero must be replaced by TASK-1.45. Layering matches doc-16 and R98: nothing is added to the sdk, and crates/steg gains only the entry point.

Split on the engineer's decision of 2026-11-15 ("default to splitting going forward"), along the seam the worker's own plan proposed. This task is now part A: the socket, the arbiter that leaves one server per socket, and the handshake. Part B, a draft under TASK-1 that depends on this task, takes the lifecycle: starting on demand through the sdk's provider, detaching, the login-shell environment, the 30-second idle stop and the steg entry point (the former criteria 2, 5, 6 and 8, and the end-to-end halves of 7 and 9). The plan on this task was written for both parts and is to be rewritten for part A only.

Rewritten plan for part A checked against doc-8 (R1, R3-R5, R39, R98), doc-13 (R60, R61), doc-14, doc-16 and decision-2 section 7 by @bruk.o: ok. The four points of the first check are met: part A only, with part B named for each thing left out; nix 0.28 (MIT, already in Cargo.lock) for the lock and the owner check, and no unsafe code at all; no test hook in shipped code (serve returns its outcome; the race is proved by a test fixture binary); shutdown order stated (stop accepting, close the listener, remove the socket file, release the lock last) with what a connecting client sees. Proof covers the five criteria clause by clause with real sockets and processes, including a server killed outright to prove the lock is released on death. Known and stated gap: the other-user cases are tested over fabricated metadata, there being no second user here.
<!-- SECTION:NOTES:END -->
