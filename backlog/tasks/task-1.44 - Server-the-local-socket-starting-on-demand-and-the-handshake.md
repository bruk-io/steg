---
id: TASK-1.44
title: 'Server: the local socket, one server per socket, and the handshake'
status: In Progress
assignee:
  - '@bruk.w3'
created_date: '2026-11-15T01:32:00Z'
updated_date: '2026-11-15T22:03:00Z'
labels:
  - size-3
  - changes-requested
milestone: m-3
dependencies:
  - TASK-1.69
  - TASK-1.70
references:
  - task-1.44-server-the-local-socket-starting-on
  - https://github.com/bruk-io/steg/pull/37
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
asdlc_plan_approved_by: bruk
asdlc_plan_approved_hash: 'c074345d1296'
asdlc_plan_approved_size: size-3
asdlc_review_kind: same-machine
asdlc_review_findings: 'important=1,nit=0'
asdlc_review_policies: ["review"]
asdlc_plan_matched: no
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

Check FAILED: `just check` exited 101 at 2026-11-15T20:25:51+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.44 on task-1.44-server-the-local-socket-starting-on at 30f6ac77b152 with uncommitted changes, last 40 of 68 lines:

```text
test dispatch::tests::every_described_method_has_exactly_one_handler ... ok
test env::tests::exactly_six_variables_are_returned ... ok
test env::tests::path_prepends_bin_dir_to_the_existing_path ... ok
test env::tests::path_is_just_bin_dir_when_there_is_nothing_to_prepend_to ... ok
test env::tests::carries_the_five_variables ... ok
test paths::tests::lock_path_is_a_sibling_with_a_lock_suffix ... ok
test paths::tests::nothing_at_the_path_is_absent ... ok
test paths::tests::owned_by_us_is_false_for_a_fabricated_different_uid ... ok
test paths::tests::owned_by_us_is_true_for_this_processs_own_euid ... ok
test paths::tests::a_real_socket_we_own_is_ours_and_a_socket ... ok
test paths::tests::a_real_private_directory_is_safe ... ok
test arbiter::tests::attempt_finds_a_real_live_listener_already_running ... ok
test arbiter::tests::attempt_becomes_the_server_when_nothing_is_there ... ok
test arbiter::tests::attempt_refuses_a_world_writable_directory ... ok
test bind::tests::nothing_is_left_at_the_temporary_name_after_a_successful_bind ... ok
test paths::tests::a_real_plain_file_at_the_path_is_not_a_socket ... ok
test bind::tests::a_real_bind_sets_0600_under_the_ambient_umask ... ok
test paths::tests::a_real_world_writable_directory_is_refused ... ok
test arbiter::tests::attempt_takes_over_a_stale_socket_left_by_a_dead_server ... FAILED
test bind::tests::a_second_bind_replaces_the_first_at_the_same_path ... ok
kill: 63178: No such process
test pane::tests::a_forward_failing_once_the_worker_is_gone_still_reaps_a_program_that_ends_on_its_own ... ok

failures:

---- arbiter::tests::attempt_takes_over_a_stale_socket_left_by_a_dead_server stdout ----

thread 'arbiter::tests::attempt_takes_over_a_stale_socket_left_by_a_dead_server' (10143278) panicked at crates/server/src/arbiter.rs:416:17:
expected Became, got something else: AlreadyRunning
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    arbiter::tests::attempt_takes_over_a_stale_socket_left_by_a_dead_server

test result: FAILED. 48 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.06s

error: test failed, to rerun pass `-p steg-server --lib`
full output: .quiet/tests-every-crate.log
error: recipe `test` failed with exit code 101
```

Check FAILED: `just test` exited 101 at 2026-11-15T20:32:49+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.44 on task-1.44-server-the-local-socket-starting-on at 30f6ac77b152 with uncommitted changes, last 40 of 64 lines:

```text
test bind::tests::a_second_bind_replaces_the_first_at_the_same_path ... FAILED
test arbiter::tests::attempt_takes_over_a_stale_socket_left_by_a_dead_server ... ok
kill: 50325: No such process
test pane::tests::a_forward_failing_once_the_worker_is_gone_still_reaps_a_program_that_ends_on_its_own ... ok

failures:

---- arbiter::tests::attempt_becomes_the_server_when_nothing_is_there stdout ----

thread 'arbiter::tests::attempt_becomes_the_server_when_nothing_is_there' (10647160) panicked at crates/server/src/arbiter.rs:377:17:
assertion `left == right` failed
  left: 493
 right: 384
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace

---- bind::tests::a_real_bind_sets_0600_under_the_ambient_umask stdout ----

thread 'bind::tests::a_real_bind_sets_0600_under_the_ambient_umask' (10647167) panicked at crates/server/src/bind.rs:95:9:
assertion `left == right` failed
  left: 493
 right: 384

---- bind::tests::a_second_bind_replaces_the_first_at_the_same_path stdout ----

thread 'bind::tests::a_second_bind_replaces_the_first_at_the_same_path' (10647168) panicked at crates/server/src/bind.rs:142:9:
assertion `left == right` failed
  left: 493
 right: 384


failures:
    arbiter::tests::attempt_becomes_the_server_when_nothing_is_there
    bind::tests::a_real_bind_sets_0600_under_the_ambient_umask
    bind::tests::a_second_bind_replaces_the_first_at_the_same_path

test result: FAILED. 46 passed; 3 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.02s

error: test failed, to rerun pass `-p steg-server --lib`
full output: .quiet/tests-every-crate.log
error: recipe `test` failed with exit code 101
```

Check passed: `just test` exited 0 at 2026-11-15T20:33:41+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.44 on task-1.44-server-the-local-socket-starting-on at 30f6ac77b152 with uncommitted changes:

```text
ok      tests (every crate) (5.5s)
```

Check FAILED: `just test` exited 101 at 2026-11-15T20:34:26+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.44 on task-1.44-server-the-local-socket-starting-on at 30f6ac77b152 with uncommitted changes, last 40 of 64 lines:

```text
test resize_reaches_the_terminal_even_with_no_program_running ... ok
test a_pane_with_no_program_running_accepts_input_as_a_no_op ... ok
test a_programs_exit_code_reaches_the_event_channel ... ok
test output_printed_before_exit_is_in_the_snapshot ... ok
test restarting_keeps_the_same_terminal_so_old_output_survives_a_new_run ... ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests/race.rs (target/debug/deps/race-9ccfa22c1fcb949d)

running 3 tests
test attempt_times_out_when_the_lock_stays_contended_and_nothing_answers ... FAILED
test a_socket_file_left_by_a_real_dead_process_is_replaced ... ok
test eight_simultaneous_starts_leave_exactly_one_server ... FAILED

failures:

---- attempt_times_out_when_the_lock_stays_contended_and_nothing_answers stdout ----

thread 'attempt_times_out_when_the_lock_stays_contended_and_nothing_answers' (10813150) panicked at crates/server/tests/race.rs:198:5:
expected Refuse(ArbitrationTimedOut), got Became(ServerHandle { listener: UnixListener { fd: FileDesc(OwnedFd { fd: 13 }), local: "/var/folders/rf/f34xmczn4659gm98hrsprydc0000gn/T/.steg-tmp-76728-0" (pathname) }, lock: Flock(File { fd: 12, path: "/private/var/folders/rf/f34xmczn4659gm98hrsprydc0000gn/T/76728-1.sock.fake-76728-1794774866638135000", read: false, write: true }) })
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace

---- eight_simultaneous_starts_leave_exactly_one_server stdout ----

thread 'eight_simultaneous_starts_leave_exactly_one_server' (10813151) panicked at crates/server/tests/race.rs:94:5:
assertion `left == right` failed: expected exactly one winner, got outcomes: ["became", "became", "became", "became", "became", "became", "became", "became"]
  left: 8
 right: 1


failures:
    attempt_times_out_when_the_lock_stays_contended_and_nothing_answers
    eight_simultaneous_starts_leave_exactly_one_server

test result: FAILED. 1 passed; 2 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.09s

error: test failed, to rerun pass `-p steg-server --test race`
full output: .quiet/tests-every-crate.log
error: recipe `test` failed with exit code 101
```

Check passed: `just check` exited 0 at 2026-11-15T20:35:27+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.44 on task-1.44-server-the-local-socket-starting-on at 30f6ac77b152 with uncommitted changes:

```text
ok      format check (0.2s)
ok      lint (1.0s)
ok      boundaries (0.1s)
ok      licences (0.8s)
ok      tests (every crate) (5.2s)
ok      build (steg-sdk) (0.1s)
ok      test (steg-sdk) (0.2s)
ok      test (steg-sdk, local-socket) (0.4s)
ok      build (steg-kernel) (0.1s)
ok      test (steg-kernel) (0.2s)
ok      build (steg-server) (31.6s)
ok      test (steg-server) (4.6s)
ok      build (steg) (0.2s)
ok      test (steg) (0.4s)
ok      build (steg-app) (0.3s)
ok      test (steg-app) (0.3s)
ok      tool tests (1.2s)
ok      architecture (1.1s)
ok      recipe names (0.1s)
```

Check passed: `just check` exited 0 at 2026-11-15T20:39:46+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.44 on task-1.44-server-the-local-socket-starting-on at 30f6ac77b152 with uncommitted changes:

```text
ok      format check (0.2s)
ok      lint (0.8s)
ok      boundaries (0.1s)
ok      licences (0.8s)
ok      tests (every crate) (5.4s)
ok      build (steg-sdk) (0.1s)
ok      test (steg-sdk) (0.2s)
ok      test (steg-sdk, local-socket) (0.4s)
ok      build (steg-kernel) (0.1s)
ok      test (steg-kernel) (0.2s)
ok      build (steg-server) (32.2s)
ok      test (steg-server) (4.5s)
ok      build (steg) (0.2s)
ok      test (steg) (0.4s)
ok      build (steg-app) (0.3s)
ok      test (steg-app) (0.3s)
ok      tool tests (1.3s)
ok      architecture (1.0s)
ok      recipe names (0.1s)
```

Check passed: `just check` exited 0 at 2026-11-15T20:41:05+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.44 on task-1.44-server-the-local-socket-starting-on at d7a4dea56426:

```text
ok      format check (0.1s)
ok      lint (3.4s)
ok      boundaries (0.1s)
ok      licences (0.7s)
ok      tests (every crate) (8.9s)
ok      build (steg-sdk) (0.8s)
ok      test (steg-sdk) (1.9s)
ok      test (steg-sdk, local-socket) (2.2s)
ok      build (steg-kernel) (1.2s)
ok      test (steg-kernel) (1.3s)
ok      build (steg-server) (1.0s)
ok      test (steg-server) (5.0s)
ok      build (steg) (0.3s)
ok      test (steg) (0.5s)
ok      build (steg-app) (0.9s)
ok      test (steg-app) (0.8s)
ok      tool tests (1.4s)
ok      architecture (0.8s)
ok      recipe names (0.1s)
```

Check FAILED: `sh -c 'cargo test -p steg-server --test shutdown'` exited 101 at 2026-11-15T20:59:45+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.44 on task-1.44-server-the-local-socket-starting-on at 55592e06308a with uncommitted changes:

```text
warning: unused import: `std::net::Shutdown`
 --> crates/server/src/listen.rs:8:5
  |
8 | use std::net::Shutdown;
  |     ^^^^^^^^^^^^^^^^^^
  |
  = note: `#[warn(unused_imports)]` (part of `#[warn(unused)]`) on by default

warning: `steg-server` (lib) generated 1 warning (run `cargo fix --lib -p steg-server` to apply 1 suggestion)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.08s
     Running tests/shutdown.rs (target/debug/deps/shutdown-4f329e528730f187)

running 4 tests
test a_stalled_handshake_is_closed_and_run_returns_promptly_on_stop ... ok
test stopping_closes_the_listener_removes_the_socket_and_releases_the_lock ... ok
test many_short_connections_do_not_grow_the_tracked_set ... ok
test a_completed_idle_connection_is_closed_and_run_returns_promptly_on_stop ... FAILED

failures:

---- a_completed_idle_connection_is_closed_and_run_returns_promptly_on_stop stdout ----

thread 'a_completed_idle_connection_is_closed_and_run_returns_promptly_on_stop' (12828267) panicked at crates/server/tests/shutdown.rs:220:21:
assertion `left != right` failed: the client should see its connection closed well within the 5s bound
  left: WouldBlock
 right: WouldBlock
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    a_completed_idle_connection_is_closed_and_run_returns_promptly_on_stop

test result: FAILED. 3 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 5.01s

error: test failed, to rerun pass `-p steg-server --test shutdown`
```

Check passed: `sh -c 'for i in 1 2 3 4 5 6 7 8 9 10; do cargo test -p steg-server --test race || exit 1; done'` exited 0 at 2026-11-15T21:02:00+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.44 on task-1.44-server-the-local-socket-starting-on at 55592e06308a with uncommitted changes, last 40 of 99 lines:

```text

    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.09s
     Running tests/race.rs (target/debug/deps/race-da9d5307ee8a0d67)

running 3 tests
test a_socket_file_left_by_a_real_dead_process_is_replaced ... ok
test eight_simultaneous_starts_leave_exactly_one_server ... ok
test attempt_times_out_when_the_lock_stays_contended_and_nothing_answers ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.10s

    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.09s
     Running tests/race.rs (target/debug/deps/race-da9d5307ee8a0d67)

running 3 tests
test a_socket_file_left_by_a_real_dead_process_is_replaced ... ok
test eight_simultaneous_starts_leave_exactly_one_server ... ok
test attempt_times_out_when_the_lock_stays_contended_and_nothing_answers ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.12s

    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.08s
     Running tests/race.rs (target/debug/deps/race-da9d5307ee8a0d67)

running 3 tests
test a_socket_file_left_by_a_real_dead_process_is_replaced ... ok
test attempt_times_out_when_the_lock_stays_contended_and_nothing_answers ... ok
test eight_simultaneous_starts_leave_exactly_one_server ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.11s

    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.09s
     Running tests/race.rs (target/debug/deps/race-da9d5307ee8a0d67)

running 3 tests
test a_socket_file_left_by_a_real_dead_process_is_replaced ... ok
test attempt_times_out_when_the_lock_stays_contended_and_nothing_answers ... ok
test eight_simultaneous_starts_leave_exactly_one_server ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.10s
```

Check passed: `sh -c 'for i in 1 2 3 4 5; do cargo test -p steg-server --test shutdown || exit 1; done'` exited 0 at 2026-11-15T21:02:08+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.44 on task-1.44-server-the-local-socket-starting-on at 55592e06308a with uncommitted changes, last 40 of 55 lines:

```text
running 4 tests
test a_stalled_handshake_is_closed_and_run_returns_promptly_on_stop ... ok
test stopping_closes_the_listener_removes_the_socket_and_releases_the_lock ... ok
test a_completed_idle_connection_is_closed_and_run_returns_promptly_on_stop ... ok
test many_short_connections_do_not_grow_the_tracked_set ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s

    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.07s
     Running tests/shutdown.rs (target/debug/deps/shutdown-4f329e528730f187)

running 4 tests
test a_stalled_handshake_is_closed_and_run_returns_promptly_on_stop ... ok
test stopping_closes_the_listener_removes_the_socket_and_releases_the_lock ... ok
test a_completed_idle_connection_is_closed_and_run_returns_promptly_on_stop ... ok
test many_short_connections_do_not_grow_the_tracked_set ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s

    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.08s
     Running tests/shutdown.rs (target/debug/deps/shutdown-4f329e528730f187)

running 4 tests
test a_stalled_handshake_is_closed_and_run_returns_promptly_on_stop ... ok
test stopping_closes_the_listener_removes_the_socket_and_releases_the_lock ... ok
test a_completed_idle_connection_is_closed_and_run_returns_promptly_on_stop ... ok
test many_short_connections_do_not_grow_the_tracked_set ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s

    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.09s
     Running tests/shutdown.rs (target/debug/deps/shutdown-4f329e528730f187)

running 4 tests
test stopping_closes_the_listener_removes_the_socket_and_releases_the_lock ... ok
test a_stalled_handshake_is_closed_and_run_returns_promptly_on_stop ... ok
test a_completed_idle_connection_is_closed_and_run_returns_promptly_on_stop ... ok
test many_short_connections_do_not_grow_the_tracked_set ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
```

Check passed: `just check` exited 0 at 2026-11-15T21:04:32+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.44 on task-1.44-server-the-local-socket-starting-on at 12d6802de7c0:

```text
ok      format check (0.1s)
ok      lint (0.4s)
ok      boundaries (0.1s)
ok      licences (0.8s)
ok      tests (every crate) (2.7s)
ok      build (steg-sdk) (0.1s)
ok      test (steg-sdk) (0.2s)
ok      test (steg-sdk, local-socket) (0.4s)
ok      build (steg-kernel) (0.1s)
ok      test (steg-kernel) (0.2s)
ok      build (steg-server) (32.6s)
ok      test (steg-server) (5.1s)
ok      build (steg) (0.3s)
ok      test (steg) (0.5s)
ok      build (steg-app) (0.3s)
ok      test (steg-app) (0.3s)
ok      tool tests (1.1s)
ok      architecture (1.1s)
ok      recipe names (0.1s)
```

Finished taking over from a worker stopped mid-build (orchestrator's mistake, not its work). Inherited: head d7a4dea pushed/green, a merge of origin/main (55592e0), and one uncommitted edit in listen.rs applying today's thread-joining rule to run()'s accept loop (track a try_clone'd stream beside each JoinHandle; shut down and join every open connection on stop, before closing the listener/removing the socket/releasing the lock).

Found the inherited edit incomplete and fixed it, both written up in full on the task's notes:

1. connections only ever grew - nothing reaped an already-finished entry, leaking memory and fds without bound on a long-lived server. Added reap_finished, called once per accept, removing/joining entries whose JoinHandle::is_finished(). Proved by listen::tests (the predicate, in-process) and shutdown.rs::many_short_connections_do_not_grow_the_tracked_set (200 connections come and go, fd count before/after a trigger connection).

2. Proving the shutdown-and-join step mattered exposed a real regression: holding a try_clone'd duplicate fd per connection means a plain close() (returning from connection::handle) no longer fully closes the socket while the duplicate is held, so a timed-out handshake stopped closing for the peer - caught by handshake.rs's own pre-existing a_connection_that_never_sends_a_hello_is_closed_after_the_timeout, which started failing deterministically. Fixed in connection.rs: shutdown(Both) on every exit path, which acts on the socket regardless of outstanding duplicates.

Also joined shutdown.rs's own pre-existing test's previously-detached server thread, and added tests for stop with an idle completed connection (returns within a 5s bound, client sees the close) and stop during a stalled handshake (returns within a 5s bound, not the connection's own 30s timeout).

"Prove one bites": removed the shutdown-and-join step (bare double-drop), ran `cargo test -p steg-server --test shutdown`, a_completed_idle_connection_is_closed_and_run_returns_promptly_on_stop failed by its 5s bound exactly as expected (not a hang) - recorded on the task, then restored.

Checks recorded, in order: the failing shutdown test with the join step removed; 10 consecutive `cargo test -p steg-server --test race` runs (no flake); 5 consecutive `cargo test -p steg-server --test shutdown` runs (no flake); `just check` at the final head (12d6802, after merging origin/main) - green, the state this is handed off on. PR #37's CI (`check`) passed on the same head in 3m15s.

Departure from the plan (plan digest c074345d1296, not edited): the plan's own shutdown section named only the accept/close/remove-file/release-lock ordering, with no thread-joining, because the team rule merged after the plan was approved. The ordering itself is unchanged; what's added is making sure every thread this task's loop spawns is actually brought down first. Full writeup, including what this means for part B (TASK-1.77, whose idle stop calls the same Stopper), and coverage against all five acceptance criteria clause by clause, is in the task's own notes (two comments posted today).

Check FAILED: `sh -c 'cargo test -p steg-server --test handshake'` exited 101 at 2026-11-15T21:27:16+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.44 on task-1.44-server-the-local-socket-starting-on at 12d6802de7c0 with uncommitted changes, last 40 of 51 lines:

```text

running 10 tests
test undecodable_bytes_as_the_first_frame_get_a_malformed_hello_refusal ... FAILED
test a_control_frame_arriving_first_gets_a_malformed_hello_refusal ... FAILED
test an_oversized_declared_length_gets_a_malformed_hello_refusal_promptly ... FAILED
test a_connection_is_reported_connected_before_it_sends_its_hello ... ok
test a_hello_with_the_wrong_magic_gets_a_malformed_hello_refusal ... ok
test a_version_mismatch_names_both_versions_over_a_real_connection ... ok
test a_hello_missing_the_protocol_field_gets_a_malformed_hello_refusal ... ok
test a_client_that_sends_half_a_frame_and_then_stalls_is_closed_at_the_timeout_with_no_reply ... ok
test a_connection_that_never_sends_a_hello_is_closed_after_the_timeout ... ok
test a_completed_handshake_is_not_closed_for_sitting_idle_past_the_timeout ... ok

failures:

---- undecodable_bytes_as_the_first_frame_get_a_malformed_hello_refusal stdout ----

thread 'undecodable_bytes_as_the_first_frame_get_a_malformed_hello_refusal' (13804324) panicked at crates/server/tests/handshake.rs:41:9:
the server closed before a complete frame arrived
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace

---- a_control_frame_arriving_first_gets_a_malformed_hello_refusal stdout ----

thread 'a_control_frame_arriving_first_gets_a_malformed_hello_refusal' (13804311) panicked at crates/server/tests/handshake.rs:41:9:
the server closed before a complete frame arrived

---- an_oversized_declared_length_gets_a_malformed_hello_refusal_promptly stdout ----

thread 'an_oversized_declared_length_gets_a_malformed_hello_refusal_promptly' (13804320) panicked at crates/server/tests/handshake.rs:41:9:
the server closed before a complete frame arrived


failures:
    a_control_frame_arriving_first_gets_a_malformed_hello_refusal
    an_oversized_declared_length_gets_a_malformed_hello_refusal_promptly
    undecodable_bytes_as_the_first_frame_get_a_malformed_hello_refusal

test result: FAILED. 7 passed; 3 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.71s

error: test failed, to rerun pass `-p steg-server --test handshake`
```

Commits since the review: 0.

Check FAILED: `sh -c 'cargo test -p steg-server --test shutdown'` exited 101 at 2026-11-15T21:35:13+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.44 on task-1.44-server-the-local-socket-starting-on at 12d6802de7c0 with uncommitted changes, last 40 of 45 lines:

```text
test stopping_closes_the_listener_removes_the_socket_and_releases_the_lock ... ok
test stop_racing_a_client_connecting_still_returns_within_a_bound ... ok
test stop_called_twice_concurrently_on_an_idle_server_is_harmless ... FAILED
test stop_on_a_genuinely_idle_server_returns_from_run_within_a_bound ... FAILED
test a_completed_idle_connection_is_closed_and_run_returns_promptly_on_stop ... FAILED
test many_short_connections_do_not_grow_the_tracked_set ... FAILED

failures:

---- stop_called_twice_concurrently_on_an_idle_server_is_harmless stdout ----

thread 'stop_called_twice_concurrently_on_an_idle_server_is_harmless' (13926365) panicked at crates/server/tests/shutdown.rs:129:29:
run should have returned within 5s of being stopped
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace

---- stop_on_a_genuinely_idle_server_returns_from_run_within_a_bound stdout ----

thread 'stop_on_a_genuinely_idle_server_returns_from_run_within_a_bound' (13926366) panicked at crates/server/tests/shutdown.rs:129:29:
run should have returned within 5s of being stopped

---- a_completed_idle_connection_is_closed_and_run_returns_promptly_on_stop stdout ----

thread 'a_completed_idle_connection_is_closed_and_run_returns_promptly_on_stop' (13926362) panicked at crates/server/tests/shutdown.rs:129:29:
run should have returned within 5s of being stopped

---- many_short_connections_do_not_grow_the_tracked_set stdout ----

thread 'many_short_connections_do_not_grow_the_tracked_set' (13926364) panicked at crates/server/tests/shutdown.rs:129:29:
run should have returned within 5s of being stopped


failures:
    a_completed_idle_connection_is_closed_and_run_returns_promptly_on_stop
    many_short_connections_do_not_grow_the_tracked_set
    stop_called_twice_concurrently_on_an_idle_server_is_harmless
    stop_on_a_genuinely_idle_server_returns_from_run_within_a_bound

test result: FAILED. 3 passed; 4 failed; 0 ignored; 0 measured; 0 filtered out; finished in 5.04s

error: test failed, to rerun pass `-p steg-server --test shutdown`
```

Commits since the review: 0.

Check passed: `sh -c 'for i in 1 2 3 4 5 6 7 8 9 10; do cargo test -p steg-server --features race-fixture --test race || exit 1; done'` exited 0 at 2026-11-15T21:41:59+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.44 on task-1.44-server-the-local-socket-starting-on at 12d6802de7c0 with uncommitted changes, last 40 of 99 lines:

```text

    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.11s
     Running tests/race.rs (target/debug/deps/race-e0bba94806d2a1ef)

running 3 tests
test a_socket_file_left_by_a_real_dead_process_is_replaced ... ok
test eight_simultaneous_starts_leave_exactly_one_server ... ok
test attempt_times_out_when_the_lock_stays_contended_and_nothing_answers ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.12s

    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.10s
     Running tests/race.rs (target/debug/deps/race-e0bba94806d2a1ef)

running 3 tests
test a_socket_file_left_by_a_real_dead_process_is_replaced ... ok
test attempt_times_out_when_the_lock_stays_contended_and_nothing_answers ... ok
test eight_simultaneous_starts_leave_exactly_one_server ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.11s

    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.11s
     Running tests/race.rs (target/debug/deps/race-e0bba94806d2a1ef)

running 3 tests
test eight_simultaneous_starts_leave_exactly_one_server ... ok
test a_socket_file_left_by_a_real_dead_process_is_replaced ... ok
test attempt_times_out_when_the_lock_stays_contended_and_nothing_answers ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.10s

    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.11s
     Running tests/race.rs (target/debug/deps/race-e0bba94806d2a1ef)

running 3 tests
test a_socket_file_left_by_a_real_dead_process_is_replaced ... ok
test eight_simultaneous_starts_leave_exactly_one_server ... ok
test attempt_times_out_when_the_lock_stays_contended_and_nothing_answers ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.12s
```

Commits since the review: 0.

Check passed: `sh -c 'for i in 1 2 3 4 5 6 7 8 9 10; do cargo test -p steg-server --test shutdown || exit 1; done'` exited 0 at 2026-11-15T21:42:06+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.44 on task-1.44-server-the-local-socket-starting-on at 12d6802de7c0 with uncommitted changes, last 40 of 139 lines:

```text
     Running tests/shutdown.rs (target/debug/deps/shutdown-be635be15e6581f1)

running 7 tests
test a_stalled_handshake_is_closed_and_run_returns_promptly_on_stop ... ok
test stop_on_a_genuinely_idle_server_returns_from_run_within_a_bound ... ok
test stop_called_twice_concurrently_on_an_idle_server_is_harmless ... ok
test stopping_closes_the_listener_removes_the_socket_and_releases_the_lock ... ok
test a_completed_idle_connection_is_closed_and_run_returns_promptly_on_stop ... ok
test many_short_connections_do_not_grow_the_tracked_set ... ok
test stop_racing_a_client_connecting_still_returns_within_a_bound ... ok

test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.13s

    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.08s
     Running tests/shutdown.rs (target/debug/deps/shutdown-be635be15e6581f1)

running 7 tests
test a_stalled_handshake_is_closed_and_run_returns_promptly_on_stop ... ok
test stop_on_a_genuinely_idle_server_returns_from_run_within_a_bound ... ok
test stop_called_twice_concurrently_on_an_idle_server_is_harmless ... ok
test stopping_closes_the_listener_removes_the_socket_and_releases_the_lock ... ok
test a_completed_idle_connection_is_closed_and_run_returns_promptly_on_stop ... ok
test many_short_connections_do_not_grow_the_tracked_set ... ok
test stop_racing_a_client_connecting_still_returns_within_a_bound ... ok

test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.13s

    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.08s
     Running tests/shutdown.rs (target/debug/deps/shutdown-be635be15e6581f1)

running 7 tests
test a_stalled_handshake_is_closed_and_run_returns_promptly_on_stop ... ok
test stop_on_a_genuinely_idle_server_returns_from_run_within_a_bound ... ok
test stop_called_twice_concurrently_on_an_idle_server_is_harmless ... ok
test a_completed_idle_connection_is_closed_and_run_returns_promptly_on_stop ... ok
test stopping_closes_the_listener_removes_the_socket_and_releases_the_lock ... ok
test many_short_connections_do_not_grow_the_tracked_set ... ok
test stop_racing_a_client_connecting_still_returns_within_a_bound ... ok

test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.13s
```

Commits since the review: 0.

Check passed: `just check` exited 0 at 2026-11-15T21:44:33+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.44 on task-1.44-server-the-local-socket-starting-on at 100bc9969ab6:

```text
ok      format check (0.2s)
ok      lint (5.3s)
ok      boundaries (0.1s)
ok      licences (0.7s)
ok      tests (every crate) (10.0s)
ok      build (steg-sdk) (0.9s)
ok      test (steg-sdk) (2.4s)
ok      test (steg-sdk, local-socket) (2.7s)
ok      build (steg-kernel) (1.3s)
ok      test (steg-kernel) (1.5s)
ok      build (steg-server) (31.5s)
ok      test (steg-server) (5.1s)
ok      test (steg-server, race-fixture) (6.2s)
ok      build (steg) (0.2s)
ok      test (steg) (0.4s)
ok      build (steg-app) (0.8s)
ok      test (steg-app) (0.8s)
ok      tool tests (0.9s)
ok      architecture (1.0s)
ok      recipe names (0.0s)
```

Commits since the review: 39, 36 without the Asdlc-Worker trailer.

Review findings fixed at head 100bc99 (PR #37, CI green: check passed in 4m7s). Each finding, and what was done about it:

[Important #1 - malformed-hello for every non-hello first frame] Fixed: connection.rs now routes every decoded first-frame outcome other than Hello (undecodable bytes at the frame level, or a well-formed control/event/data frame arriving first) through the same malformed-hello refusal check_hello already produces for the two cases it could already reach (wrong magic, missing protocol) - built from the sdk's own public Refusal::Malformed/Frame::hello/encode_frame/PROTOCOL_VERSION, nothing added to crates/sdk. Six new tests in handshake.rs cover all four of decision-2 section 7's cases plus the oversized-length and stalled-half-frame cases the review also named; "prove one bites" reverted the fix and got exactly the three previously-unreached cases failing by their own bound, recorded on the task, then restored.

[Important #2 - Stopper::stop not reliably stopping an idle run] Fixed: Stopper now binds the socket path itself (set by run() before its first accept()), so its one public stop() always both sets the flag and wakes the loop; the separate stop_and_wake function and the bare, easy-to-misuse flag-set are gone (the flag-set is now private, called internally). Three new tests cover a genuinely idle server, concurrent double-stop, and a stop racing a real connect(). Writing the first of these surfaced a test-design bug of my own - the shared start_server helper's own startup probe leaves a connection queued that masks this exact regression - caught by actually running "prove one bites" and seeing a suspiciously fast pass, then fixed with a new start_truly_idle_server helper that drains the probe first. Redid "prove one bites" with the corrected tests: all four tests calling stop() failed by their 5s bound exactly as expected with the wakeup disabled, recorded, then restored.

[Nits] race-attempt is now feature-gated (confirmed an ordinary build produces no extra binary, and just check still runs the race test via a new check-packages line); the arbiter's probe showing up on the ConnectionEvent seam is documented as a TASK-1.77 caveat; the listener now closes immediately after the accept loop stops rather than after the connection-join phase, closing the window a connecting client could be queued-then-reset in (fixed, not only documented); error.rs's stale plan metadata is confirmed byte-identical to origin/main and noted here, plan left untouched.

Checks recorded, in order: the failing handshake test with finding #1 reverted; the failing shutdown test with finding #2 reverted (using the corrected idle-server tests, after catching my own test-design bug); 10 consecutive race-fixture race-test runs; 10 consecutive shutdown-test runs; just check at the merged final head (100bc99, after merging origin/main - brought in TASK-1.41's kernel/sdk changes, no conflicts). Full writeup with code-level detail is in the task's own notes (two comments posted this round).
<!-- SECTION:NOTES:END -->

## Comments

<!-- COMMENTS:BEGIN -->
author: @bruk
created: 2026-11-15T20:02:00Z

---

Plan approved by @bruk (c074345d1296, size-3): its worker may build it.

---

created: 2026-11-15T21:02:00Z

---

Finishing this task after taking it over mid-build (orchestrator's note: the previous worker was stopped by mistake, not for anything wrong in its work). Inherited state: head d7a4dea pushed and green; a merge of origin/main (55592e0) and one uncommitted change in listen.rs, not yet committed - the previous worker's response to today's merged team rule ("every spawned child and thread waited on or joined along every path that can end it"), applied to run()'s per-connection threads: track a try_clone'd stream beside each JoinHandle, and on stop shut each one down and join it before closing the listener, removing the socket file and releasing the lock.

This is a departure from the approved plan (c074345d1296): the plan's own shutdown section (part 4) only named "stop accepting -> close the listener -> remove the socket file -> release the lock", with no mention of joining per-connection threads, because at plan time the team's thread-joining rule had not yet merged. The departure is in reaction to that rule, not a reconsideration of the plan's own design - the shutdown *ordering* itself (accept loop stops, listener closes, file removed, lock released last) is unchanged; what's added is making sure every thread this task's own accept loop ever spawns is actually brought down before that ordering runs, rather than left to finish (or never finish) on its own.

Why it matters for part A specifically: AC #4's second clause ("one that has completed [the handshake] is not closed for being idle") means a completed connection's thread blocks forever in a read with no timeout at all. Before this change, nothing made that thread return when stop() was called, so a single still-open, idle client would have made run() hang indefinitely on stop - a real regression against AC #5's "testable alone" requirement (a test driving this directly would itself hang), not merely a tidiness concern.

Finishing this took two things beyond what I inherited:

1. The inherited edit tracked `connections: Vec<(UnixStream, JoinHandle<()>)>` but only ever grew it - nothing removed an entry once its thread had already finished on its own (timeout, refusal, or a client closing normally). Over a long-lived server this leaks both memory (the Vec) and file descriptors (the try_clone'd shutdown handle, held open until the whole vec is drained at stop) without bound. Added `reap_finished`, called once per accept-loop iteration right after a new connection is accepted and before it is pushed, which removes and joins every already-finished entry (`JoinHandle::is_finished`). The tracked set is now bounded by how many connections are concurrently live plus whatever finished since the last accept - not by the server's whole history. Proved by `listen::tests` (the removal predicate, in-process) and `shutdown.rs::many_short_connections_do_not_grow_the_tracked_set` (an end-to-end fd-count proof: 200 short-lived connections come and go, one more connection triggers the reap pass, and the process's own fd count afterward is asserted close to where it started - no accessor added to shipped code for this, which the plan's own review comment #1 already ruled out as a pattern for this task).

2. Reaping only on the next accept exposed a second, more serious bug once I tried to prove the shutdown-and-join step actually works: holding a try_clone'd duplicate fd of every connection, for as long as it's tracked, means a plain close() of the connection thread's own copy (what simply returning from connection::handle does) does *not* fully close the socket while the duplicate is still open - a close() only drops that one fd's reference; it does not shutdown the socket or notify the peer. Before this change there was only ever one fd per connection, so the thread returning *did* fully close it. With the clone now held, a client whose handshake times out (AC #4's first clause) saw its connection silently stay open - proved by a real regression in `handshake.rs`'s existing `a_connection_that_never_sends_a_hello_is_closed_after_the_timeout`, which started failing deterministically once the inherited edit's clone was in place, and by a from-scratch repro (`UnixStream::pair()`, drop one end, `set_read_timeout` on the other - EINVAL on this platform once the peer side is gone, a separate finding noted below). Fixed in connection.rs: every exit path out of `connection::handle` now calls `stream.shutdown(Shutdown::Both)` before returning, which acts on the socket itself regardless of how many local duplicates exist, so the peer sees its connection end promptly whether or not `run`'s own tracking entry has been reaped yet. This decouples correctness (the peer seeing a close) from reaping (purely a resource-bookkeeping concern now).

Separate, smaller finding folded in while writing the new tests: on this platform, UnixStream::set_read_timeout returns EINVAL once the peer has already shut down that socket - confirmed with a standalone two-line repro, not assumed. The new test that depends on this (`a_completed_idle_connection_is_closed_and_run_returns_promptly_on_stop`) sets the client's read timeout *before* calling stop, not after, to avoid the race between "server already shut the socket down" and "client tries to set a timeout on it."

Also fixed while applying today's rule: `shutdown.rs`'s own pre-existing test (`stopping_closes_the_listener_removes_the_socket_and_releases_the_lock`) spawned its server on a background thread and dropped the JoinHandle (detached) rather than joining it - the channel recv proved the work finished, but nothing joined the thread itself. Now bound and joined after the recv.

What this means for part B (TASK-1.77): its idle checker holds the same Stopper and calls .stop() the same way this task's own tests do, so it inherits this behaviour for free - no change needed on B's side. One thing worth B's own attention when it's planned: B's idle-stop path will be the first *real*, non-test caller of this shutdown step, and it's worth B explicitly re-confirming (not just inheriting on faith) that stopping an on-demand server with live, idle, long-running client connections (an app window open for hours, say) still unblocks and joins promptly the same way this task's short-lived test connections do - nothing here suggests it wouldn't (shutdown() isn't sensitive to how long a connection has been open), but B is where that actually gets exercised by a real idle timer rather than a test calling stop() directly.

Decision this task made that the plan didn't: whether "finished handles are reaped" needs an accessor in shipped code to prove end-to-end. Decided no - proved instead through this test process's own fd count (serve runs on an in-process thread in every test here), consistent with this task's own review comment #1's rejection of STEG_TEST_REPORT_OUTCOME_TO as a test hook in shipped code.

Proof added, mapped to the acceptance criteria already in the plan: AC #4 (the fix to connection.rs is what makes its first clause - "closed after a timeout" - actually hold once a shutdown handle is kept around; `shutdown.rs`'s two new tests cover its second clause under a stop, not just under simple idling as `handshake.rs` already proved) and AC #5 (run/serve remain directly, foreground-testable - these are still all in-process or fixture-binary tests, nothing added that needs a `steg` binary). AC #1, #2, #3 are unaffected by this change and keep the proof already recorded.

"No sleeps as synchronisation, with one named exception" (AC #4's second clause, as the plan's own Proof section states): unaffected - the new tests wait on real events (a channel, is_finished(), a bounded read observing a close) except where the plan's own named exception already applies, which none of these new tests are.

---

created: 2026-11-15T21:03:00Z

---

Coverage against the five acceptance criteria, clause by clause, including today's additions:

#1 (mode and path checked and trusted, not assumed): unchanged by today's work. socket_mode.rs (real umask 0, real bind, asserts 0600) and paths.rs's existing_file_trust/directory_is_safe unit tests (absent, ours-and-dead, not-a-socket, not-ours fabricated, world-writable directory) - all still green, recorded.

#2 (one survivor; a dead server's socket replaced): unchanged. arbiter::decide's exhaustive pure unit tests; race.rs's eight-simultaneous-starts and killed-holder tests, plus the in-process arbitration-timeout test - run 10 times consecutively today with no failure, recorded as one check.

#3 (version mismatch names both): unchanged. handshake.rs's a_version_mismatch_names_both_versions_over_a_real_connection, green.

#4 (handshake timeout closes; completed-and-idle does not): directly touched by today's fix. handshake.rs's two existing tests (closed after timeout with no stop involved; not closed for idling past the timeout) are green, and the first of those is what caught the real regression (connection.rs's missing shutdown()) before any new test was even run - confirmed by reverting my connection.rs fix and re-running just that test, which failed the same way, then restoring it. Two new tests in shutdown.rs extend this clause under a *stop*, which the plan's original proof section didn't separately name: a_completed_idle_connection_is_closed_and_run_returns_promptly_on_stop (stop with an idle, handshake-completed client connected: run returns within a 5s bound, and the client's own read then sees the connection closed) and a_stalled_handshake_is_closed_and_run_returns_promptly_on_stop (stop while a handshake is stalled: run returns within a 5s bound, not left waiting on the connection's own, deliberately long 30s handshake timeout).

#5 (sdk's handshake; foreground, no detach, testable alone): unchanged in kind, strengthened by today's regression: before today's fix, a test driving run()/serve() directly with one idle completed connection would have hung on stop() with no bound at all - the exact failure mode "testable alone" is meant to rule out. Every test proving this (today's included) still drives serve/attempt/run directly, in-process or via the existing fixture binary, with no steg binary anywhere in the suite.

Additional proof beyond the five criteria, for the thread-joining rule itself: listen::tests::a_finished_connection_is_removed_and_a_still_open_one_is_kept and ::an_empty_set_is_left_empty (reap_finished's own predicate, in-process, real UnixStream::pair and real threads); shutdown.rs::many_short_connections_do_not_grow_the_tracked_set (end-to-end: 200 connect/handshake/close cycles plus one trigger connection, fd count before vs. after). "Prove one bites": removed the shutdown-and-join step in run() (replaced with a bare double-drop), ran `cargo test -p steg-server --test shutdown`, and a_completed_idle_connection_is_closed_and_run_returns_promptly_on_stop failed by its 5s bound exactly as expected (not a hang) - recorded on this task, then the step was restored and the full suite re-confirmed green.

Checks recorded on this task today, in order: the failing `cargo test -p steg-server --test shutdown` with the join step removed (proving the test bites); 10 consecutive `cargo test -p steg-server --test race` runs (proving AC #2's race and timeout tests are not flaky); 5 consecutive `cargo test -p steg-server --test shutdown` runs (proving the new tests are not flaky); and `just check` at the final head, all green - that last one is the final state this task is handed off on.

---

author: @bruk.r2
created: 2026-11-15T21:08:00Z

---

@bruk.r2 is reviewing TASK-1.44: a same-machine review: it waited 0m with no other engineer's reviewer, longer than review_fallback_after (0m), so a fresh reviewer on its author's machine takes it.

---

author: @bruk.r2
created: 2026-11-15T21:21:00Z

---

Review by @bruk.r2 (same-machine): changes requested. Back to In Progress for the author's worker.

Findings: 2 Important, 3 nit. Cites: review, rust. The diff does not match the plan.

Reviewed at head 12d6802de7c0 (origin/main...HEAD diff in a detached worktree); PR #37 CI ("check") passed on this same head in 3m15s. Re-ran the race test 15x and the shutdown test 3x myself at this head (all green), consistent with the task's own recorded 10x/5x runs (recorded at 55592e06308a, carried forward to the green final check at 12d6802). cargo tree -p steg-server -i nix shows one nix 0.28.0 line; features (fs, user) match the Cargo.toml/boundaries.toml comments; MIT is in deny.toml's allow list; just check-boundaries is green; no unsafe anywhere in the diff; no file under crates/sdk changed.

[Compliance, Important] crates/server/src/connection.rs:81-96 only routes a decoded Incoming::Hello(payload) to the sdk's check_hello/accept_or_refuse; every other Incoming variant - including Malformed (an undecodable first frame) and a well-formed Control-kind frame arriving first - is mapped to None and closed with no reply at all (connection.rs:139's shutdown, no write_all). decision-2 section 7, cited in this task's own description as binding, records the spike's proven contract: "a first frame that is not JSON, that has the wrong magic, that is a control frame instead of a hello, or that is missing protocol, gets malformed-hello naming the server's version and saying the client's is unknown." The sdk's check_hello already computes exactly that (Refusal::Malformed, handshake.rs:111-113) for two of those four cases (wrong magic, missing protocol - both still arrive as Incoming::Hello with a bad payload), but the other two (undecodable bytes, a control frame first) never reach check_hello in this server and get silent closure instead. This is a real departure from the cited contract, not just a missing test, and it is undisclosed in the plan or notes. handshake.rs also has no test sending any hostile/non-hello first frame at all (only the version-mismatch case, which is a well-formed Hello) - the review brief named "hostile bytes in place of a hello" explicitly as a branch worth proving, and nothing reaches it.

[Bugs, Important] listen.rs:32-36 and :50-54: Stopper's own doc and Stopper::stop's doc both describe calling bare .stop() as the way "Part B's idle checker... will hold one of these and call Stopper::stop" to end run. In fact run's accept loop only re-checks the stop flag once accept() returns (listen.rs:152-160), so a bare .stop() with no accompanying wakeup connection leaves run blocked forever in accept() on a genuinely idle server - exactly the case TASK-1.77's 30-second idle-with-no-clients stop exists for. Only stop_and_wake (which also connects a throwaway client to unblock accept()) reliably stops run; every caller and test in this task already uses stop_and_wake, but the doc comment that is this seam's whole contract for TASK-1.77 names the wrong function. Followed as written, TASK-1.77's idle checker would reproduce, in production, the exact "testable alone"/hang regression this task's own notes say today's thread-joining fix was needed to prevent.

Nits (3):
1. crates/server/Cargo.toml:41-43: the race-attempt fixture binary always builds (steg-server has no feature to gate it behind, unlike the sdk's own feature-gated fake-server-for-tests) - acknowledged in the manifest's own comment, but still ships as an extra binary in every build of this crate; worth gating behind something before this becomes the convention other tasks copy.
2. arbiter::probe (arbiter.rs:177-182) connects to a live server purely to test liveness during arbitration; run's accept loop treats that exactly like a real client, emitting ConnectionEvent::Connected then Disconnected for it. Every arbitration probe against an already-running server will look like a client arriving and leaving on the ConnectionEvent seam - not a bug in this task, but a caveat for TASK-1.77's idle counter that the notes do not mention.
3. error.rs is listed in the task's modified_files but is byte-for-byte identical to origin/main - harmless stale plan metadata (the plan's own section 6 already explains no new Error variant was needed), but worth cleaning up.

Verified directly, confirming the brief's other named concerns: reaping only runs on the next accept() (listen.rs:92-102/156), so on a quiet server a finished connection's dup fd + JoinHandle sit until the next accept or a stop - bounded by connections-since-last-accept, not by server history (proved by many_short_connections_do_not_grow_the_tracked_set), and connection.rs:139's unconditional shutdown(Both) means a peer's own view of the connection closing no longer depends on reaping timing at all. Shutdown order (listen.rs:174-192) destructures ServerHandle rather than dropping it whole, so the lock is released strictly last, after the listener is closed and the socket file removed; shutdown.rs's own test asserts a connecting client sees NotFound or ConnectionRefused. A narrower point the ordering doesn't fully address: between "stop accepting" and "close the listener" there's now a join phase (the departure below) during which the listener is still bound - a client connecting in that exact window could have its connect() succeed into the kernel backlog and only be reset once the listener is later dropped, rather than seeing an immediate refusal; not tested or discussed in the notes, worth a mention but not blocking on its own.

Departure from the plan (digest c074345d1296, not edited): the plan's section 4 named only stop-accepting/close-listener/remove-file/release-lock, with no thread-joining. The notes correctly disclose the departure (today's merged team rule on joining spawned threads) and its reason, and the regression it caught (connection.rs needing shutdown(Both) once a duplicate fd is held) is itself a legitimate, well-proven fix. Per policy this still marks plan-matched as "no" even though the notes explain it well.

---

created: 2026-11-15T21:42:00Z

---

Review findings addressed (review #5, @bruk.r2, at 12d6802). Each finding below with what changed and why; nothing here edits the approved plan (c074345d1296) - departures are recorded here instead, per the brief.

[Important #1 - malformed-hello for every non-hello first frame] connection.rs's first-frame handling is restructured: read_one's result is now classified into a FirstFrame enum (NoCompleteFrame, Hello(payload), NotAHello(reason)), with a new describe_non_hello() naming each non-Hello Incoming variant and a new malformed_hello_reply() building the refusal directly from the sdk's own public Refusal::Malformed, Frame::hello and encode_frame (PROTOCOL_VERSION also public) - nothing added to crates/sdk, confirmed by cargo tree and the diff touching no file under crates/sdk. The two cases the review found unreached (undecodable bytes at the frame level, and a well-formed-but-wrong-kind frame such as a control/event frame arriving first) now get the same malformed-hello refusal naming the server's version that check_hello already produces for the other two cases (wrong magic, missing protocol) - decision-2 section 7's contract now holds for all four, uniformly, through one path.

Six new tests in handshake.rs, one per case named in the review plus two more it asked for: a_hello_with_the_wrong_magic_gets_a_malformed_hello_refusal, a_hello_missing_the_protocol_field_gets_a_malformed_hello_refusal (both previously undertested - the logic was already correct but nothing proved it over a real connection), undecodable_bytes_as_the_first_frame_get_a_malformed_hello_refusal, a_control_frame_arriving_first_gets_a_malformed_hello_refusal (the two real gaps), an_oversized_declared_length_gets_a_malformed_hello_refusal_promptly (asserts the refusal arrives well under the handshake timeout, not by waiting one out), and a_client_that_sends_half_a_frame_and_then_stalls_is_closed_at_the_timeout_with_no_reply (the negative case: a stalled handshake has nothing decoded to refuse, so it correctly gets AC #4's original no-reply timeout behaviour, not a malformed-hello - named explicitly so the distinction is not left for a reader to wonder about). "Prove one bites": reverted the NotAHello branch to close with no reply (the old behaviour), ran cargo test -p steg-server --test handshake, and exactly the three new cases this finding named failed by their own bound (not a hang) while the two already-correct cases (wrong magic, missing protocol) stayed green - recorded on the task, then restored.

[Important #2 - Stopper::stop not reliably stopping an idle run] Stopper now holds the socket path internally (Arc<Mutex<Option<PathBuf>>>), set once by run() itself (bind_socket_path, private) before it ever calls accept() for the first time. stop() is now the one public method, and it always both sets the flag and wakes the accept loop - the separate stop_and_wake free function is removed (not deprecated: every caller already used it, so there was no reason to keep two names for what is now one correct function), and the bare flag-set is private (set_stop_flag), called by stop() and by run() itself (to catch a stop() that raced in before the path was bound, checked once more immediately after binding, before the loop is ever entered).

Three new tests in shutdown.rs: stop_on_a_genuinely_idle_server_returns_from_run_within_a_bound, stop_called_twice_concurrently_on_an_idle_server_is_harmless, and stop_racing_a_client_connecting_still_returns_within_a_bound (20 rounds, both orderings). Writing the first of these surfaced a test-design bug of my own, worth recording for whoever reads this later: using the existing start_server helper (which itself makes a probe connection to confirm the socket is accepting) left a connection queued in the kernel's own backlog by the time the test called stop() - run()'s first accept() call consumes that leftover probe regardless of whether stop()'s wakeup works at all, which masks exactly this regression (a test built this way would have passed even against the old, broken stop()). Caught this by actually running the "prove one bites" step before trusting the test: reverting stop() to the old bare-flag behaviour and rerunning showed the test still passing in 0.01s - too fast to be real, which is what sent me looking for why. Fixed by adding start_truly_idle_server, which drains the probe's own Connected/Disconnected pair over an events channel before returning, so run()'s loop has genuinely cycled back to a fresh, truly-empty, blocking accept() call before the test ever calls stop(). The third test (racing a real connect()) has the same structural limitation for a different reason (documented in its own doc comment): as long as the listener is still open, which it is until run() actually returns, a connecting client's own connect() succeeding also masks the same bug by the same mechanism - so that test is honest in its own doc about proving a different, real property (no deadlock/panic/leak when stop() races a real connection) rather than re-proving this specific regression, which the other two tests are what actually cover. "Prove one bites", corrected: reran with stop()'s wakeup disabled using the fixed start_truly_idle_server-based tests - all four tests that now call stop() (the three new ones plus two from the reap/shutdown work) failed by their 5s bound exactly as expected; recorded, then restored.

[Nit #1 - race-attempt always builds] Added a race-fixture feature (off by default) gating both the [[bin]] and an explicit [[test]] entry for race.rs (required-features on both) - confirmed cargo build -p steg-server with no features produces no race-attempt binary at all, and cargo test -p steg-server --no-run with the feature off skips race.rs while every other test file still auto-discovers (checked the actual target list both ways, not assumed - autotests stays on for the rest even with one explicit [[test]] entry). justfile's check-packages gained a second cargo test -p steg-server --features race-fixture line, the same pattern already used for steg-sdk's local-socket feature, so just check still runs the race test (verified: it shows as its own "test (steg-server, race-fixture)" line).

[Nit #2 - arbiter's probe looks like a client on the ConnectionEvent seam] Documented directly on ConnectionEvent's own doc comment in connection.rs, stated as a caveat for part B (TASK-1.77) rather than fixed here: arbiter::probe's plain UnixStream::connect against a running server is indistinguishable, on this seam, from a real client that connected and left before completing a handshake - the server has no signal to filter on (a probe carries nothing that marks it as one), so this is a real limit TASK-1.77's idle checker will need to account for, not something this task can resolve from its own side.

[Nit #3 - the listener-still-open window during the join phase] Fixed rather than only documented: drop(listener) now happens immediately after the accept loop breaks, before the connections-shutdown-and-join loop runs (previously the other way round). Closing the listener only affects new connection attempts (refused immediately, nothing queues after that point) - it has no effect on already-accepted connections, which live on their own independent file descriptors the listener never touches, so moving it earlier costs nothing and closes the window entirely. Both run()'s own top-level doc comment and the shutdown-order comment at the point of the change now describe this ordering and why. No new timing-based test added for this specifically - the window this closes was already sub-millisecond in practice even before the fix (closing the listener is itself one of the fastest possible operations), so a test asserting its absence would be testing scheduler timing, not this code; the existing stopping_closes_the_listener_removes_the_socket_and_releases_the_lock test already pins the correct end state (NotFound/ConnectionRefused once everything has settled).

[Nit #4 - error.rs listed as modified but unchanged] Confirmed with git diff origin/main -- crates/server/src/error.rs: byte-identical. Stale plan metadata from the plan's own modified-files list (section 6 of the plan already explains why no new Error variant was needed) - noted here per the review's own ask, plan left untouched.

Checks recorded on this task for this round, in order: the failing cargo test -p steg-server --test handshake with the malformed-hello fix reverted (finding #1's prove-one-bites); the failing cargo test -p steg-server --test shutdown with stop()'s wakeup disabled, using the corrected idle-server tests (finding #2's prove-one-bites, redone after catching my own test-design bug); 10 consecutive cargo test -p steg-server --features race-fixture --test race runs; 10 consecutive cargo test -p steg-server --test shutdown runs; just check at the merged final head.

---

author: @bruk.r2
created: 2026-11-15T21:49:00Z

---

@bruk.r2 is reviewing TASK-1.44: a same-machine review: it waited 0m with no other engineer's reviewer, longer than review_fallback_after (0m), so a fresh reviewer on its author's machine takes it.

---

author: @bruk.r2
created: 2026-11-15T22:03:00Z

---

Review by @bruk.r2 (same-machine): changes requested. Back to In Progress for the author's worker.

Findings: 1 Important, 0 nit. Cites: review. The diff does not match the plan.

Reviewed at head 100bc9969ab67750a59c842777a74cea4947cd02 (origin/main...HEAD diff in a detached worktree). PR #37 CI ("check") passed at this same headRefOid in 4m7s (confirmed via `gh pr view 37 --json headRefOid` matching, and `gh pr checks 37`). I also ran `just check` myself at this head: green (format, lint, boundaries, licences, tests every crate, each package build+test including `test (steg-server, race-fixture)`, tool tests, architecture, recipe names). Ran `cargo test -p steg-server --test shutdown` 8x (7/7 green each time), `cargo test -p steg-server --features race-fixture --test race` 5x (3/3 green each time), `cargo test -p steg-server --test handshake` once (10/10 green). `cargo tree -p steg-server -i nix` shows one nix 0.28.0 line. Confirmed with `rm -f target/debug/race-attempt && cargo build -p steg-server` that an ordinary build produces no race-attempt binary (feature-gating works). Confirmed via `git diff --stat origin/main...HEAD` that no file under crates/sdk changed.

Both of review #5's Important findings are genuinely fixed, verified independently rather than taken on trust:

- Finding #1 (malformed-hello only for 2 of 4 decision-2 §7 cases): read connection.rs's new FirstFrame/describe_non_hello/malformed_hello_reply, and handshake.rs's six tests covering wrong-magic, missing-protocol, undecodable-bytes, control-frame-first, oversized-length, and half-a-frame-then-stall. All ten handshake tests pass. Built only from the sdk's own public Refusal/Frame/encode_frame/PROTOCOL_VERSION; no file under crates/sdk changed.
- Finding #2 (Stopper::stop not reliably stopping an idle run): read the new Stopper::bind_socket_path/stop()/set_stop_flag design. Verified myself, independent of the worker's own "prove one bites": made a scratch edit to listen.rs disabling stop()'s wake_accept call (wrapped in `if false`), ran `cargo test -p steg-server --test shutdown stop_on_a_genuinely_idle_server_returns_from_run_within_a_bound`, and it failed exactly as claimed ("run should have returned within 5s of being stopped"). Reverted the edit (`git checkout -- crates/server/src/listen.rs`, confirmed `git status --short` clean), rebuilt, and all 7 shutdown tests pass. This also independently confirms `start_truly_idle_server` genuinely drains the startup probe as claimed.

New finding this round:

[Bugs, Important] crates/server/src/connection.rs:75-87 (`read_one`) and :145-155 (`first_frame`): `set_read_timeout` arms a *per-read* timeout, and `read_one`'s loop re-arms it on every successful `stream.read()` call that returns any bytes at all - the timeout is "no byte received within handshake_timeout of the last one", not "the handshake must complete within handshake_timeout of connecting". A client that sends one byte every interval just under handshake_timeout, forever, without ever completing a frame, keeps `read_one`'s loop alive indefinitely: no complete frame ever arrives, so `first_frame` never returns, the connection's thread (spawned per-connection by `run`) never exits, and `ConnectionEvent::Disconnected` is never sent.

Reproduced empirically (not just reasoned about): a scratch harness (outside this worktree, linking steg-server as a library) started a server with `handshake_timeout: Duration::from_secs(2)`, connected, and wrote one byte every 1.5s. The connection survived 9.0s - 4.5x the configured timeout - after 6 trickled bytes, with no complete frame ever sent, before I stopped the server myself; nothing in the server ever closed it.

This is exactly the failure decision-2 section 7 (cited in this task's own description as binding, "The server runs... decision-2, section 7") names as the reason a handshake timeout is needed at all: "A client that opens the connection and sends half a hello holds a connection for ever. TASK-1.9 owns it." The implemented per-read timeout closes this for a client that stalls completely after a partial frame (handshake.rs's `a_client_that_sends_half_a_frame_and_then_stalls_is_closed_at_the_timeout_with_no_reply` - one read, then silence, then the timeout fires correctly), but does not close it for a client that keeps the connection alive by trickling - a strictly weaker, still-hostile input the suite never tries. AC #4's first clause ("A connection that never completes its handshake is closed after a timeout") does not hold for that input class, and the notes do not disclose this as a known gap or accepted limitation anywhere in the plan or the round-by-round write-ups.

Downstream cost, since it ties to the seam this task built for TASK-1.77: a trickling connection never emits `ConnectionEvent::Disconnected`, so TASK-1.77's idle-with-no-clients counter would be held open indefinitely by a single hostile or malfunctioning client, defeating the idle stop entirely for as long as that one connection trickles.

On allocation (the brief's other named concern): bounded per-connection - the decoder's declared-length check (`crates/sdk/src/wire/frame.rs:197`, unmodified by this task) refuses anything over `MAX_FRAME` (8 MiB) before allocating for the body, confirmed by reading it and by the oversized-length test passing. This is not the headline risk here; the unbounded-held-thread is.

Not a repeat of a cited policy from an earlier review (first occurrence of this shape), so no rule-change draft filed for it per the review policy's step 5.

Plan-matched: no, same as review #5's call - the departures from the approved plan (c074345d1296) disclosed in the notes (thread-joining added mid-build, the listener now closing before the join phase) are real, well-disclosed fixes, but remain departures under the policy's own wording even when well explained.

---
<!-- COMMENTS:END -->
