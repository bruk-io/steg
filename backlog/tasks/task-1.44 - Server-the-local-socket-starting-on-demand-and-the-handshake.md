---
id: TASK-1.44
title: 'Server: the local socket, starting on demand, and the handshake'
status: In Progress
assignee:
  - '@bruk.w3'
created_date: '2026-11-15T01:32:00Z'
updated_date: '2026-11-15T19:12:00Z'
labels:
  - size-3
milestone: m-0
dependencies:
  - TASK-1.69
  - TASK-1.70
modified_files:
  - crates/server/src/config.rs
  - crates/server/src/error.rs
  - crates/server/src/sys.rs
  - crates/server/src/paths.rs
  - crates/server/src/bind.rs
  - crates/server/src/arbiter.rs
  - crates/server/src/detach.rs
  - crates/server/src/login_env.rs
  - crates/server/src/idle.rs
  - crates/server/src/accounting.rs
  - crates/server/src/connection.rs
  - crates/server/src/lib.rs
  - crates/server/tests/serve.rs
  - crates/server/Cargo.toml
  - crates/steg/src/main.rs
  - crates/steg/Cargo.toml
  - crates/steg/tests/socket.rs
  - crates/steg/tests/lifecycle.rs
  - crates/steg/tests/race.rs
  - crates/steg/tests/fixtures/fake_client.rs
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
- [ ] #1 The server listens on a socket at an XDG path that only its owner can open, with the mode set rather than assumed
- [ ] #2 The first client that needs a server starts one; a server started explicitly stays up with no clients
- [ ] #3 A client and server whose protocol versions differ get a message naming both versions
- [ ] #4 A connection that never completes its handshake is closed after a timeout
- [ ] #5 A server started on demand keeps running after the client that started it exits or is killed, and its panes' environment comes from a login shell, not from that client (R99)
- [ ] #6 A server started on demand stops after 30 seconds with no sessions and no clients; one with a session, or one started explicitly, does not (R99)
- [ ] #7 Two clients starting a server at the same moment end up connected to the same one (R99)
- [ ] #8 A client's part in starting a server is to run the server's program and wait for the socket; it holds none of the logic above (R98)
- [ ] #9 The handshake and the version check come from the sdk (TASK-1.70, doc-16 R101); this task adds the socket, the lifecycle and a real client starting a real server through the sdk's local-socket provider
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
## Before anything else

This worktree's branch (`task-1.44-server-the-local-socket-starting-on`) is stale: it forked
from the point right after TASK-1.44 was first claimed by `@bruk.w6` (commit `1d256ad`), before
it was released and re-claimed by `@bruk.w3` (commit `10e9448`, current). In that same window
TASK-1.70 (sdk: wire protocol + local-socket provider) merged to `origin/main` (PR #30) and
TASK-1.69 finished, so the worktree's `crates/sdk`, `crates/server`, `crates/steg` are far
behind what is actually on `origin/main` today - everything below is read from `origin/main`
via `git show`, not from the stale worktree. TASK-1.40 (pane programs) is `In Review` with an
open PR, not yet merged - this plan does not depend on it or edit its files, per the brief.
Phase 2's first step is bringing the worktree's branch up to `origin/main` (rebase: nothing of
this branch is pushed yet).

## Size: this is bigger than one reviewable change

Below is the full plan for the task as claimed, but it is large: a process-level arbiter with
an exclusive lock, fd-passing double detach, login-shell environment capture with a timeout,
an idle-with-injected-clock checker, socket mode/ownership checks, and the `steg` entry point,
each with its own real-process or real-socket tests. Proposed split, if the engineer would
rather have two reviews instead of one:

- **A - the socket and the handshake** (ACs #1, #3, #4, #9's socket half): binding with mode
  set and checked, the directory/stale-file checks, the arbiter's lock, the handshake with its
  timeout and version mismatch, reusing the sdk's `wire::Connection` unmodified.
- **B - the lifecycle** (ACs #2, #5, #6, #8, #9's lifecycle half): the detach, the login-shell
  environment, the idle rule, the `steg` entry point, and the real client-starts-real-server
  proof.
- AC #7 (the race) is proved mostly in A (the arbiter) and re-proved end to end in B (N real
  processes). Say in each PR which half it closes.
- **If split, A needs its own foreground path to be testable alone.** Section 2's fd-passing
  detach is B's; A's `serve()` has to bind (section 1's `bind.rs`) and run the accept/handshake
  loop *in the calling process*, with no re-exec at all, so A's own tests (AC #1, #3, #4) have
  something to run against without B existing yet. B then adds the detach as a wrapper around
  that same foreground loop - stage 2 calls the identical `serve()` A already built, just after
  the fd handoff instead of before it - so nothing in A is thrown away, only extended.

If the engineer wants one task, build A then B in that order as two commits; each is
independently `just check`-clean.

### Order of work

1. `config.rs`, `error.rs`, `sys.rs` (section 1) - the shared pieces everything else needs,
   each with its own unit tests (no I/O for `config`/`error`; `sys` is thin wrappers, tested
   where cheap, e.g. `clear_cloexec` round-tripped on a real pipe).
2. `bind.rs`, `paths.rs` (sections 1, 3's step 4 note) - bind-temp-chmod-rename, and the
   directory-safety check; real-socket unit tests in `crates/server`.
3. `arbiter.rs` (section 3) - the pure `decide()` first, exhaustively unit-tested; then
   `attempt()` wired to a real connect-probe and a real `flock`.
4. A foreground `serve()` in `lib.rs` that binds (step 2), arbitrates (step 3), then runs the
   accept loop with per-connection handshake (`connection.rs`, section 5's timeout) and no
   detach yet - this is stage A's own deliverable if split (above), and lets AC #1/#3/#4 be
   proved before anything about detaching exists.
5. `idle.rs` + `accounting.rs` (section 5) - the pure predicate first, then the thread wiring
   client counting from `accept()` (the fix named there) into `serve()`'s accept loop, gated
   on `config.on_demand`.
6. `detach.rs` (section 2) - **first**, a throwaway spawn-and-report check that a cleared
   -`CLOEXEC` fd really does survive `Command::spawn`'s own fd handling across a `pre_exec`
   -bearing exec (std closes fds it created and set `CLOEXEC` on itself, but does not walk and
   close arbitrary inherited descriptors - this is the one assumption the whole detach rests
   on, cheap to confirm before building on it, not left for the end-to-end tests to discover
   if it is wrong; if it does not hold, the fallback is `dup2`-ing onto known low fd numbers
   before exec, which changes this file's shape but nothing above it). Then the fd-passing
   re-exec itself, wired so stage 2 calls the same `serve()` step 4 built, now reached after
   the handoff instead of in the foreground.
7. `login_env.rs` (section 4) - the passwd lookup, the env-cleared-and-reseeded capture with
   its timeout, the pure parser; wired into stage 2's startup, stored on the running server and
   exposed as the resolved `STEG_SERVER` string (see the note at the end of section 4) for a
   later task's `pane_environment` to consume.
8. `crates/steg/src/main.rs`'s narrow `server start [--on-demand]` match, calling `serve`.
9. The real multi-process tests (`crates/steg/tests/`): mode/uid (AC #1), autostart and
   explicit-stays-up (AC #2), survival-after-killed (AC #5), the eight-way race (AC #7), the
   end-to-end hello round trip (AC #9) - each only after the step(s) it depends on above exist.
10. `just check` end to end, recorded with `asdlc record-check`, before handing either stage
    to review.

## 1. Where each part lives, and why

Everything socket/process/clock-touching is `steg-server` (doc-16 R98: the server's code is in
packages no client depends on; the kernel and sdk stay I/O-free). `crates/steg` gains only
enough argument handling to recognise `server start [--on-demand]` and call
`steg_server::serve(config)` - no server logic of its own (R98), matching the `steg-kernel`/
`steg` dependency table in `boundaries.toml` unchanged except for one new external dependency
(section 6).

New files under `crates/server/src/`:

| File | Holds |
|---|---|
| `config.rs` | `ServerConfig`: `socket_path: Option<PathBuf>`, `on_demand: bool`, `handshake_timeout: Duration`, `idle_grace: Duration`, each with a production default and all overridable - the injection point every test uses instead of sleeping real durations. |
| `sys.rs` | The handful of POSIX calls std does not expose, each a safe wrapper with its own `// SAFETY:` comment: `flock_exclusive_nonblocking`, `setsid`, `login_shell_passwd_entry()` (via `getpwuid_r`, giving the shell, home directory, username and login name together - see section 4), `clear_cloexec(fd)`. No umask wrapper (see `bind.rs`, below). One small module per foreign interface (`rust.md`). |
| `arbiter.rs` | The decision logic: a pure `fn decide(connect_probe: Probe, lock: LockOutcome) -> Arbitration` enum (`BecomeServer`, `AlreadyRunning`, `GiveUp(reason)`), unit-tested on all branches with no I/O; `attempt(config) -> Result<Arbitration, Error>` does the real connect-probe-then-lock-then-bind and returns the already-bound pieces. |
| `paths.rs` | `lock_path(&socket_path)` (sibling file, `.lock` suffix - not subject to `sockaddr_un`'s 104-byte limit since it is never bound, only opened); `directory_is_safe(dir) -> Result<(), Reason>` checking the parent directory's owner uid and mode before anything is placed in it. |
| `bind.rs` | `bind_with_mode(path) -> io::Result<UnixListener>`: removes a stale socket file (only ever called after the lock is held), binds under a sibling temporary name in the same directory, sets `0600` on the temp path via std's own `PermissionsExt::set_mode` (no `libc` needed for this step), then `rename()`s it onto `path` - atomic, and touches no process-global state (see "why not umask", below). |
| `detach.rs` | `spawn_detached(listener, lock, config, argv) -> io::Result<()>` (fd-passing re-exec, below) and `inherited_fds() -> Option<(UnixListener, File)>` (the other side, reading the env vars it set). |
| `login_env.rs` | `capture_login_environment(timeout) -> Vec<(OsString, OsString)>` and the pure `parse_env_dump(&[u8]) -> Vec<(OsString, OsString)>` it calls. |
| `idle.rs` | `Counters { clients: u32, sessions: u32 }` and the pure `fn should_stop(idle_since: Option<Instant>, now: Instant, grace: Duration) -> bool`. |
| `accounting.rs` | The one thread that owns `Counters` and the stop decision, fed by a channel (`rust.md`: "one task owns a piece of state, others send messages" - not `Arc<Mutex<_>>`). |
| `connection.rs` | The per-connection thread body: handshake with a read timeout, then (once accepted) an unbounded hold-open read loop that only tracks liveness - it does **not** dispatch requests (see "Left out", below). |
| `error.rs` | `Error`, one variant per failure a caller can tell apart. |
| `lib.rs` | Adds `mod` lines beside the existing `dispatch`; adds `pub use` for `serve`, `ServerConfig`, `Error`. |

`crates/steg/src/main.rs` gains a narrow, hand-matched case for exactly `["server","start"]` and
`["server","start","--on-demand"]`, calling `steg_server::serve`. Nothing else changes; every
other invocation keeps today's placeholder. This is the one necessary exception to "commands
come from the API description" (R32): `server start --on-demand` is what bootstraps a process
that has nothing to connect to yet, which `local_socket.rs`'s own doc comment already says
can't go through the normal client-calls-a-method path. TASK-1.49's generated CLI will need
the same special case; this plan leaves it a `// TASK-1.49:` comment, not a design for it to
inherit blindly.

## 2. How the server detaches, and why it does not accumulate zombies beyond the bound TASK-1.70 already named

**Two stages, not a bare fork.** Stage 1 is the literal process `local_socket::connect`
spawned (or a human's `steg server start`), still attached to its own parent. It runs the
*entire* arbiter (section 3) itself, while still attached - so it can print an error and exit
non-zero if asked explicitly and a server is already up, which a detached process no longer
could. Only once stage 1 holds the lock and has bound and `chmod`ed the listener does it:

1. clear `FD_CLOEXEC` on the listener's and the lock file's descriptors (`sys::clear_cloexec`);
2. spawn itself again (`Command::new(current_exe)`) with `STEG_SERVER_DETACHED=1`,
   `STEG_INHERITED_SOCKET_FD=<n>`, `STEG_INHERITED_LOCK_FD=<n>`, the same argv, stdio set to
   `/dev/null`, and an `unsafe` `pre_exec` closure that calls `sys::setsid()` and nothing
   else - async-signal-safe, one syscall, documented in its own `// SAFETY:` comment (`Command`
   takes the fork+exec path, not the `posix_spawn` fast path, whenever a `pre_exec` closure is
   set, which is exactly why this works);
3. drops the `Child` handle without waiting (see section 8) and exits 0.

Stage 2 (`STEG_SERVER_DETACHED=1` present) is already a session leader with no controlling
terminal by the time its `main` runs (setsid happened between fork and exec, in stage 1's
child, before stage 2's image was even loaded). It reconstructs the listener and the lock from
the inherited fd numbers via `FromRawFd` (one `unsafe` block, documented: the fd numbers are
read from env vars this same binary just set, never untrusted input) and never binds or locks
anything itself - **the lock is held continuously from the moment stage 1 acquires it to the
moment the final process releases it, with no gap where it is closed and reopened.** That is
what answers TASK-1.70's own worry about the local-socket provider never waiting: stage 1's
only job after the handoff is to exit, which it does in a handful of syscalls (no login-shell
capture, no accept loop - those are stage 2's, after detaching), so the zombie entry the
calling client's process table carries is bounded to roughly that many syscalls' worth of
wall-clock time. Tested directly (section 7, "Proof") rather than asserted: spawn stage 1,
`wait()` it from the test (something only a test, not the real sdk provider, gets to do), and
assert it returns promptly.

A SIGKILL to the *original* client (the CLI or app process that called `connect`) never reaches
stage 2: by the time that client's own code resumes after `connect()` returns, stage 2 is
already reparented away from stage 1's process group into its own session, so stage 1's death
(whether by exiting normally or if the client's process group were ever signalled) cannot
propagate a SIGHUP into it either.

## 3. The arbiter: one survivor from several simultaneous starts (AC #7), and a dead server's stale socket

The lock, not `bind`, is the arbiter (binding twice at the same path cannot distinguish "stale
file, nothing listening" from "another process already bound here" without first unlinking,
and unlink-then-bind is itself the unresolved race). Sequence, run by stage 1:

1. **Connect-probe.** Before trusting a successful `UnixStream::connect`, `stat` the path first
   and refuse to trust a file whose owner is not the caller's own euid - ownership is the
   security property R5 actually names ("only its owner can open"); mode is not checked here
   at all, deliberately: the server sets `0600` on its own socket (section 3's step 5), but a
   live, owned server the caller happens to find with some other mode is a bug worth surfacing
   on its own, never a reason to treat the socket as absent and go try to take it over - that
   would send the caller down the lock-and-rebind branch against a server that is actually
   alive, which the lock then correctly blocks, and the caller loops on connect-probe back to
   the exact server it just rejected, converging only once the arbitration timeout gives up. A
   mismatched *owner* is the only case worth distrusting a successful connect over - the
   `/tmp/steg-<user>.sock` fallback is world-writable, so a pre-created file there at the exact
   path owned by someone else is the one case this protects against (the XDG/TMPDIR cases are
   per-user directories, so it never fires there in practice). If the file is absent, owned by
   someone else, or a connect attempt is refused: nothing real of ours is listening - go to 2.
   If it is a real server we own: exit 0 (both modes - the goal "a server is listening" is
   already met).
2. **Directory safety.** `paths::directory_is_safe` on the socket's parent directory (owner
   uid, not world-writable beyond the owner) - refuse with a clear message rather than
   silently trying to bind into an unsafe directory. (Honest gap: testing the cross-uid case
   for real needs a second real user or root, neither available here; this is a defensive
   check with a unit test over fabricated metadata, not an adversarial end-to-end proof - said
   plainly on the task, not hidden.)
3. **Lock attempt**, `flock(LOCK_EX | LOCK_NB)` on the sibling `.lock` file.
   - Acquired: remove any stale socket file (safe now - step 1 already proved nothing answers
     there), bind, chmod, detach (section 2).
   - Not acquired: another stage-1 process is doing exactly this right now. Retry the
     connect-probe on a short bounded loop (same shape as the sdk's own retry in
     `local_socket::connect`, reusing that pattern rather than inventing a second one) up to
     `config.handshake_timeout`-scaled deadline. Connects within it: exit 0, converged on the
     winner (this is what proves AC #7 at the unit level - section 7's real multi-process test
     proves it end to end). Never connects: exit non-zero naming the timeout, since something
     is wrong with whichever process is holding the lock.
4. **A dead server's stale socket self-heals with no special case.** A `flock` is released by
   the kernel when its holder's process ends, by any means including `SIGKILL` - so the next
   `server start` simply finds the lock free, takes it, and removes the stale file in step 3's
   "acquired" branch. No explicit crash-detection code is needed, and I say so rather than
   leaving it looking like an oversight.
5. **Why `bind.rs` does not touch `umask`.** `umask` is process-wide (shared by every thread),
   and `bind_with_mode` is called from the in-process functional tests in
   `crates/server/tests/` too, not only from a freshly-exec'd stage 1 - a umask-set-then-
   restore dance there would transiently corrupt every other test running in parallel in the
   same `cargo test` binary. Bind under a temporary name in the same directory, set `0600` on
   that temp path (std's `PermissionsExt::set_mode`, no `libc`), then `rename()` it onto the
   final path - no process-global state touched at any point. The precise guarantee this gives,
   stated carefully rather than as a blanket "atomic": the socket is never *observable* at its
   final path with any mode other than `0600` - `rename()`'s own atomicity (replacing an
   existing file in one step) is mostly moot here, since step 3 already removed any stale file
   under the lock before this runs, so the common case is a plain create at the final path, not
   a replace. The permissive-umask test (section 7, AC #1) is what proves this claim: it is the
   *rename*, not any umask restore, that makes the mode immune to the ambient umask of whatever
   process or shell invoked `steg server start`.

`decide()` itself (the pure part) takes only `Probe` (`AlreadyServed | Nothing`) and
`LockOutcome` (`Acquired | HeldByOther`) - `on_demand` does not change the arbiter's outcome at
all, only whether stage 2 later runs the idle checker (section 5). I looked at giving the
race's loser a louder "a server is already running" message for an explicit, non-on-demand
`server start`, as a nicety; left it out; it is not required by any of the nine criteria, and
every branch that exists here has a test, with no untested branch invented for the asymmetry.

## 4. The login-shell environment (AC #5's second half)

- **The shell, home directory, username and login name all come from the passwd entry
  (`sys::login_shell_passwd_entry`, `getpwuid_r` on the caller's own uid), never from `$SHELL`/
  `$HOME`/`$USER` inherited from the environment.** Those are inherited, and inherited means
  "from whatever client's environment spawned this chain" - exactly what R99 forbids.
- **The capture command is spawned with `.env_clear()` then re-seeded from that same passwd
  entry before it runs**, not run against a bare-empty environment: `HOME`, `USER`, `LOGNAME`
  from the passwd entry, and a baseline `PATH` (`/usr/bin:/bin:/usr/sbin:/sbin`). Clearing with
  no seed was this plan's first draft, and it does not work: most real login profiles
  (`.zprofile`, `.bash_profile`) reference `$HOME` or append to an existing `$PATH`, and
  against a genuinely empty environment they either error or export nothing, so the capture
  would "succeed" with a near-empty result and no test catches it unless it specifically
  checks for `HOME`/`PATH` being present (section 7 adds that assertion, not just "the marker
  is gone"). `env_clear` is still what makes "not from that client" true - the seed comes from
  the passwd entry this process just looked up itself, never copied from whatever stage 1
  inherited (which, through the unmodified `local_socket::connect`'s plain `Command::spawn`, is
  the original client's own environment).
- **`/usr/bin/env -0` is run by absolute path, confirmed present and NUL-delimited on this
  machine by running it directly rather than assumed** (macOS's BSD `env` is older than
  FreeBSD's, where `-0` could plausibly be missing - it is not, here). `-l` without `-i` is
  what avoids hanging on an interactive prompt; `stdin(Stdio::null())` is the actual anti-hang
  measure for a profile script that still tries to read a tty - it gets EOF, not a block.
- **A timeout, with the child always waited on.** The capture spawns a watcher thread that does
  a blocking `wait_with_output()` and sends the result over a channel; the caller does
  `recv_timeout(timeout)`. On timeout it kills the child by pid (kept before the `Child` moved
  into the watcher thread) and the watcher thread's own `wait_with_output()` still completes
  and is dropped - the child is reaped on every path, never left a zombie because of a timeout.
- **Fallback on any failure** (no passwd entry, spawn error, non-zero exit, timeout): the
  server's own inherited environment, with every `STEG_*` key stripped (the one place a stray
  value from whatever spawned the whole chain could otherwise leak through the fallback path
  specifically, since the primary path already guards against it via `env_clear`).
- `-0` (NUL-delimited) output, parsed by the pure `parse_env_dump`, because plain `env`'s
  newline delimiting breaks on a value containing a newline - tested with exactly that value,
  plus a line with no `=` (skipped) and a value containing its own `=` (split on the first
  one only).
- Captured once, at stage 2 startup, stored on the running server. Nothing in this task spawns
  a pane (TASK-1.40 is unmerged), so there is no `pane_environment` to hand it to yet; this
  plan deliberately does not create or touch `crates/server/src/env.rs`, which is TASK-1.40's
  own file for the per-pane `pane_environment(base, ...)` function - this task's output is
  exactly the `base` that function is written to take, wired at the call site once TASK-1.40
  lands, not before.
- **`serve()` also exposes the resolved socket path as the `STEG_SERVER` string** (doc-2
  section 4: "how to reach the server this terminal belongs to"). This task is where that
  string first becomes known - nothing consumes it yet (pane spawning is TASK-1.40's, unmerged),
  but the plan names the seam now rather than leaving a later task to discover `serve()` has
  nothing to hand it.

## 5. The idle rule and the handshake timeout, both as injected-clock predicates

- `idle::should_stop(idle_since, now, grace)` is pure: `idle_since` is `None` while any client
  or session exists, and gets set to `Some(Instant::now())` by `accounting.rs` the instant the
  count reaches zero (real `Instant`, captured once, at the real moment it happens - never
  read inside the predicate). The predicate itself never calls `Instant::now()`, so tests pass
  `now` as `idle_since + Duration::from_millis(x)` for `x` straddling the grace on both sides,
  with sessions present, with clients present, and in explicit mode (where `accounting.rs`
  never evaluates the predicate at all - "a server that has a session never stops" and "an
  explicit server stops only when told to" are both the absence of a call, not a `false`
  result, which is itself worth a test: explicit mode's accounting thread takes no grace at
  all).
- **A connection counts as a client from the moment `accept()` returns it, not from the moment
  its handshake completes.** The accept loop sends `ClientConnected` and spawns the
  per-connection thread in that order, before any byte of that connection has been read; the
  per-connection thread sends `ClientDisconnected` when it exits, on every exit path (a clean
  EOF, a refused/malformed hello, and the handshake timeout). Counting from handshake
  -acceptance instead was this plan's first draft, and it has a real gap: between `accept()`
  returning and a hello being decoded and accepted, the accounting thread would see zero
  clients, and for an on-demand server that nothing has connected to yet, the idle grace may
  already have elapsed - it would set the stop flag and tear the socket down while a real
  client is mid-handshake, which only then discovers the server gone (its own `connect()`
  succeeded; the subsequent read just hangs until its own timeout). Section 7 adds a test for
  exactly this: connect, stall before sending the hello, and assert the server does not tear
  down.
- `accounting.rs` is the single thread that owns `Counters`: it blocks on
  `recv_timeout(poll_interval)` for `ClientConnected`/`ClientDisconnected` events from
  `connection.rs`'s per-connection threads, updates the counts, and on each wakeup (event or
  timeout) checks `should_stop` with a real `Instant::now()`. On `true` it triggers shutdown by
  connecting a throwaway `UnixStream` to the server's own socket (the standard way to unblock a
  thread blocked in `accept()`), which the accept loop treats as "wake up and check the stop
  flag" rather than a real connection, then it closes the listener, removes the socket file,
  and drops the lock file (releasing the `flock`).
- **The handshake timeout is a real OS timeout, not a sleep-then-check**:
  `UnixStream::set_read_timeout(Some(config.handshake_timeout))` before the first read;
  cleared (`set_read_timeout(None)`) only once a complete hello has decoded and been accepted,
  so a connected client that has finished its handshake and then sits idle is never closed (a
  case I add a test for specifically, since it is the one teams forget and doc-14 does not
  mention it either way beyond "frozen" framing).

## 6. Dependencies, licensed

One new external dependency: **`libc` (`"0.2"`), `steg-server` only.** Covers the POSIX calls
std does not expose: `flock` for the arbiter's lock (R99), `setsid` for detaching (R99),
`getpwuid_r` for the login shell's own passwd entry (R99, not `$SHELL`), and `fcntl(F_SETFD)`
to carry the bound listener and lock fd across the detaching re-exec. Setting the socket's
mode (R5, AC #1) needs no `libc` call at all - `std::fs::set_permissions`/`PermissionsExt`
already covers it (section 3's bind-temp-then-rename). `libc` is dual
`MIT OR Apache-2.0`, both already in `deny.toml`'s allow list - read now, the same way
TASK-1.40 read `portable-pty`'s licence by hand before `just deny` proved it for real in the
build phase, which is how this plan proves it too, not by assertion. `boundaries.toml`'s
`[steg-server]` gains `"libc"` in `allow`, with that same reason. No change to `[steg]`'s own
entry: it already allows `steg-sdk` with no `deny-features`, so `steg`'s own `Cargo.toml`
turning on `steg-sdk`'s `local-socket` feature needs no boundaries edit (the table only denies
features explicitly listed; `[steg]` lists none).

### Risks

- **Size.** Named at the top: this is really two reviewable changes (the split proposal). The
  biggest risk is trying to review it as one large diff rather than as the two ordered stages
  the "Order of work" above lays out.
- **The cross-uid socket hijack check (section 3, step 2) is unverified against a real second
  user or root**, both unavailable in this environment - a defensive check with fabricated
  -metadata unit tests, not an adversarial proof. Said here and in section 10 rather than
  discovered later.
- **`getpwuid_r`/`flock`/`setsid`/`fcntl` are one small `unsafe` FFI module (`sys.rs`)** - the
  risk is scope creep into `libc` calls that don't need to be there; every call in that module
  is named above by exactly what it is for, so a reviewer can check the list is complete and
  not padded.
- **The fd-passing detach (section 2) is the part most likely to behave differently under
  `cargo test`'s own process model than under a real shell invocation** (file descriptor
  inheritance across `Command::spawn` with a `pre_exec` closure) - mitigated by proving it with
  real multi-process tests in `crates/steg/tests/`, not mocked, per section 7.
- **Two drafts of this plan already found and fixed real gaps** (the login-shell seeding, the
  accept-vs-handshake client count, the process-wide umask) before any code was written - a
  signal that this area rewards a second pass over the build, not just over the plan; the
  engineer approving this plan is not the last check it gets.

## 7. Tests, mapped to the acceptance criteria

### Proof

Concrete files, so the modified-files list below
matches what the test descriptions name: `crates/server/tests/serve.rs` (AC #3, #4, #6, the
stalled-client case) and `crates/server/src/*.rs`'s own `#[cfg(test)]` modules (the pure
decision functions, `login_env`'s focused capture test) for everything that needs a real
socket or process but not the real `steg` binary; `crates/steg/tests/socket.rs` (AC #1's
permissive-umask case), `crates/steg/tests/lifecycle.rs` (AC #2, #5, #9's end-to-end round
trip), `crates/steg/tests/race.rs` (AC #7), and `crates/steg/tests/fixtures/fake_client.rs`
(the fixture AC #5's survival test and AC #7's race test both spawn) for everything that needs
the real binary.

**No sleeps as synchronisation**, with one named, deliberate exception: proving a *negative*
("an explicit/sessioned server does not stop") has no event to wait on, since the event never
fires by design. That one test uses a short, injected `idle_grace` and waits a bounded multiple
of it, stated as the exception it is rather than silently breaking the rule.

- **AC #1 (mode and path checked, not assumed)**: `crates/server`'s own functional test binds
  for real in a temp dir and asserts `0600` under a default umask (unit-level, in-process);
  `crates/steg/tests/socket.rs` spawns the real binary under `sh -c 'umask 0; exec "$BIN"
  server start --on-demand'` (umask is process-wide, so this *must* be a separate process, not
  an in-process test, or it would corrupt every test running in parallel in the same `cargo
  test` binary - named explicitly so a reviewer doesn't ask for it to be "simplified" into one)
  and asserts the file is still `0600` regardless. `paths::directory_is_safe` gets its own unit
  tests over fabricated metadata (owned-by-me/safe, owned-by-me/world-writable,
  not-owned-by-me) - the last one is the honest gap named in section 3.
- **AC #2 (first client starts one; explicit stays with no clients)**: `crates/steg/tests/`
  real-binary test drives `steg_sdk::connect` with `autostart: true` against nothing listening,
  asserts it connects; a second real-binary test starts explicit (`server start`, no
  `--on-demand`), connects and disconnects a client, waits a bounded multiple of a short
  injected `idle_grace`, and asserts the socket still accepts (the negative-proof exception,
  named above).
- **AC #3 (version mismatch names both)**: `crates/server` functional test, in-process
  `serve()`, a raw `UnixStream` sending a hand-built `ClientHello` (via the sdk's already-public
  `Frame::hello`/`encode_frame`) at a wrong protocol version, asserting the refusal (decoded
  with the sdk's own `parse_server_hello`) names both versions - reuses `wire::Connection`
  unchanged, proving AC #9's "the handshake... comes from the sdk" by construction.
- **AC #4 (handshake timeout)**: in-process functional test, connect and send nothing; the
  test's own blocking read (with a timeout slightly over the server's own short injected one)
  observes EOF/close - waiting on the close event, not sleeping first. A second test: a
  complete handshake, then idle past where the timeout would have fired, and the connection is
  still open (the one case named in section 5).
- **AC #5 (survives the client; environment not from it)**: split in two, deliberately, rather
  than forced into one test. Survival: `crates/steg/tests/` spawns a small fixture binary
  (`tests/fixtures/fake_client.rs`, a `[[bin]]` on `steg`) whose whole job is `steg_sdk::connect`
  with `autostart: true` against the real binary, then blocks on stdin; the test polls the
  socket (bounded, not a sleep-as-sync - it is genuinely waiting for an async side effect with
  no OS-level event to subscribe to, the same shape `connect`'s own retry already uses) until
  live, sends the fixture `SIGKILL`, `wait()`s it (reaped, not left a zombie in the test's own
  process), then asserts a *fresh* connection still succeeds. Environment: a focused test on
  `login_env::capture_login_environment` alone, with a fake shell script standing in for the
  real one and a marker set in the *test's own* environment before the call, asserting (a) the
  marker is absent from the captured result, proving `env_clear` does what section 4 claims,
  and (b) `HOME` and `PATH` are both present and non-empty - the assertion the seeding fix in
  section 4 needs, since marker-absence alone would also pass against the empty, useless
  environment the first draft of that capture would have produced.
- **AC #6 (30s idle stop; a session or explicit mode does not)**: `idle::should_stop`'s own
  exhaustive unit tests (section 5) for the logic; one `crates/server` functional test with a
  short injected `idle_grace`, a real socket, no clients ever connected, that blocks on a
  channel `serve()` sends to when it returns (bounded `recv_timeout`, not a sleep) and asserts
  it returns inside that bound; a second asserting explicit mode's `serve()` does *not* return
  over the same bound (the negative-proof exception again, named once more rather than hidden
  the second time); a third connecting a client that never sends its hello, waiting past where
  the grace would otherwise have fired with zero *handshake-accepted* clients, and asserting
  `serve()` has *not* returned - the test for the accept-vs-handshake counting fix in section 5.
- **AC #7 (two simultaneous starts converge)**: `arbiter::decide`'s pure branches, unit-tested
  exhaustively, first; then `crates/steg/tests/` spawns eight real `steg server start
  --on-demand` processes at once against the same `STEG_SOCKET_PATH`, `wait()`s every one
  (asserting each exits 0 - the arbiter's losing branch is not an error for the on-demand case),
  and has each report `became`/`joined` through a test-only env-var hook
  (`STEG_TEST_REPORT_OUTCOME_TO=<path>`, the same kind of test seam `run_fake_server_for_tests`
  already establishes) so the test can assert exactly one `became`.
- **AC #8 (a client's part is to run the program and wait)**: proved by construction - every
  test above drives the real, unmodified `steg_sdk::connect`/`ConnectOptions`; nothing in this
  plan adds a line to `crates/sdk/src/local_socket.rs`. Noted explicitly on the task as the
  proof for this criterion rather than left implicit.
- **AC #9 (sdk's handshake; this task's socket, lifecycle, real client+server)**: the sum of
  the above, plus one test using `steg_sdk::connect` end to end against the real detached
  server and asserting a full method call's hello round-trip succeeds (not a deeper API call -
  `session.*`/`pane.*` dispatch over this connection is explicitly not this task's, see below).

## 8. Every spawned child, and whether it is waited on

1. `local_socket::connect`'s own spawn (unmodified sdk code) - never waited on, by R99's own
   design, already justified in that module's doc comment; out of this task's authority to
   change.
2. Stage 1's spawn of stage 2 (`detach::spawn_detached`) - **not** waited on, on purpose: stage
   2 runs forever as the server, so waiting would mean stage 1 never exits, defeating the
   detach. Tested directly (section 7, "Proof"): a test spawns stage 1 and *does* `wait()` it,
   asserting it returns promptly, which only a test - never the real caller - is positioned to
   check.
3. The login-shell capture child (`login_env.rs`) - waited on along **every** path: normally
   via the watcher thread's `wait_with_output()`, and on a timeout via an explicit `kill()` by
   pid followed by that same `wait_with_output()` completing and being reaped.
4. Test-only children (`fake_client`, the eight racing `server start` processes) - every one
   explicitly `wait()`ed (or `kill()`ed then `wait()`ed) by the test that spawned it, so no test
   leaves a zombie in its own process.

## 9. Threading model

Blocking `std::thread`, not `tokio`: one thread per connection (handshake, then hold-open), one
accounting thread owning `Counters` and the idle decision, the main `serve()` call driving the
accept loop. Matches the shape already settled for programs (one thread per pane, TASK-1.40's
plan, unmerged but already reasoned there and consistent with decision-3 section 9); needs no
new async-runtime dependency; and the two places that would otherwise need a runtime's timer
(the handshake timeout, the idle check) fall out as a plain socket read timeout and a plain
channel `recv_timeout` - if both come this cleanly from blocking I/O, `tokio`'s cost is not
earned yet. TASK-1.45/1.47 inherit a blocking per-connection thread and (per decision-2) will
need a bounded per-attachment queue and a dedicated writer when they add streaming - unaffected
by this choice.

## 10. Explicitly left out, named so no reviewer wonders

- **Routing real requests.** Once a connection's handshake is accepted, this task holds it open
  only to track liveness (for the idle rule and `client` counting) - it does not dispatch
  `session.*`/`tab.*`/`pane.*`/`api.*` control frames into `dispatch::dispatch`, even though
  that dispatch logic already exists and already handles those nouns. Wiring a `Tree`, a
  `Context` built from a connection, and the control-thread-owns-the-tree shape to real frames
  is TASK-1.45's named scope ("frames, streams, and requests routed to the API") and the
  `("server", _) => not_implemented_yet(...)` arm in `dispatch/mod.rs` is untouched.
- **Session count is hardcoded to zero.** Nothing in this task creates a session (that needs
  TASK-1.45's wiring), so the idle rule's "no sessions" half is trivially always true here -
  said plainly rather than built out further than this task's own acceptance criteria need.
- **`server.show`/`server.start`/`server.stop` as RPC methods** stay `NotImplementedYet`; an
  already-running explicit server is stopped today only by signalling the process, which the
  arbiter's stale-lock self-healing (section 3's step 4) already makes safe for the next
  `start`.
- **An explicit `server start` that loses a simultaneous race does not get a distinct "already
  running" error** (section 3) - not required by any of the nine criteria, left out to keep
  the arbiter's branches minimal and each one tested.
- **Cross-uid socket hijack** is a defensive check with a unit test over fabricated metadata,
  not an adversarial end-to-end proof - this environment has no second real user or root to
  prove it against for real (section 3, step 2).
- **`run_fake_server_for_tests`, not removed.** `boundaries.toml`'s `[steg-sdk]` has an empty
  allow list, so the sdk's own `local-socket` tests can never link against the real `steg`
  binary regardless of how complete this task's server is - the fixture is required by the
  dependency direction itself, not by the real server's absence. Proposed as a note, not a
  change: `tests/fixtures/fake_server.rs` could in principle bind directly via the already
  -public `socket_path` instead of calling back into the library crate's doc-hidden function,
  but that is TASK-1.70's own test scaffolding and has no bearing on any of this task's nine
  criteria.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Plan checked against doc-8 (R1, R3-R5, R39, R98, R99), doc-13 (R60, R61), doc-14, doc-16 and decision-2 section 7 by @bruk.o: sound, not yet put up for approval. Four things for the engineer or the worker first. (1) Size: the plan itself says this is two reviewable changes (A: socket, arbiter, handshake; B: detach, login environment, idle rule, entry point); the engineer decides whether to split the task. (2) Unsafe: the plan adds libc with a hand-written sys.rs (flock, setsid, getpwuid_r, fcntl), an unsafe pre_exec closure and FromRawFd. nix 0.28/0.29 (MIT) is already in Cargo.lock through portable-pty and offers safe wrappers for all four calls, and stage 2 calling setsid itself at start-up (it is not a process-group leader) would remove the pre_exec closure; ask the worker to reduce unsafe to the one FromRawFd block or say why not. (3) STEG_TEST_REPORT_OUTCOME_TO is a test-only hook in the shipped binary; prefer stage 1 printing its outcome (started / already running) on standard output, which a person also benefits from. (4) State what a client sees when it connects in the instant an idle server is stopping (accept closed, socket file removed, lock released, in which order), and that the session count wired to zero must be replaced by TASK-1.45. Layering matches doc-16 and R98: nothing is added to the sdk, and crates/steg gains only the entry point.
<!-- SECTION:NOTES:END -->
