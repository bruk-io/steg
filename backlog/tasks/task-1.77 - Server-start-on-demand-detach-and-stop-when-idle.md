---
id: TASK-1.77
title: 'Server: start on demand and detach, from the steg entry point'
status: In Progress
assignee:
  - '@bruk.w3'
created_date: '2026-11-15T19:23:00Z'
updated_date: '2026-11-16T00:40:00Z'
labels:
  - size-3
milestone: m-3
dependencies:
  - TASK-1.44
parent_task_id: TASK-1
project: bruk-io-steg
asdlc_accepted_from: DRAFT-4
asdlc_branch: task-1.77-server-start-on-demand-detach-and-stop
asdlc_plan_approved_by: bruk
asdlc_plan_approved_hash: 'd29c62060a8b'
asdlc_plan_approved_size: size-3
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Part B of the split of TASK-1.44, on Bruk's decision of 2026-11-15. doc-8: R1, R3, R4, R39, R98, R99; doc-13 R60, R61; decision-2 section 7. Bruk decided the lifecycle on 2026-11-15: started by the first client, not by launchd, and everything about starting, detaching and stopping is the server's own code (R98).

Builds on TASK-1.44's foreground serve(): the detach wraps that same loop. The worker's plan on TASK-1.44 (its sections 2, 4, 5 and the entry point) is the starting design; read it and its plan check. Carried from that check: prefer the nix crate, already in Cargo.lock through portable-pty, to hand-written libc wrappers, and have the detached stage call setsid itself so no pre-exec closure is needed, leaving unsafe code to the one block that takes over inherited descriptors, or say why not; state what a client sees when it connects in the instant an idle server is stopping. The session count for the idle rule is zero until TASK-1.45 routes requests; say which task replaces it.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The first client that needs a server starts one through the sdk's local-socket provider; a server started explicitly stays up with no clients and runs in the foreground of the terminal or supervisor that started it (R3, R99)
- [ ] #2 A server started on demand keeps running after the client that started it exits or is killed outright, shown with real processes
- [ ] #3 Two real clients starting a server at the same moment end up connected to the same one (R99), shown with real processes
- [ ] #4 A client's part in starting a server is to run the server's program and wait for the socket; it holds none of the logic above (R98), and nothing is added to the sdk
- [ ] #5 steg server start and steg server start --on-demand run the server, and say on standard output whether a server was started or one was already running, with the exit statuses doc-2 gives; no test-only hook is added to the shipped binary
- [ ] #6 Every child this code spawns is waited on along every path or the notes say why not, including what the client that started the server is left holding, proven by test; no unsafe code is added
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
Rewritten again, for this piece alone, now that TASK-1.45's own plan (being finalised)
settles who owns the `Runtime`: not this task. The idle stop (TASK-1.86) and the login
environment (TASK-1.87) are separate, accepted drafts; nothing below plans either -
what this task worked out for them, beyond what belongs in a plan, is in this task's
own notes under a heading for each, for their workers to start from.

## 1. No fork, no fd-passing, one safe `setsid` call - unchanged from the shared design

**The client's spawn already is the separate process; nothing needs to re-exec or hand
anything to a second stage.** `steg_sdk::local_socket::connect` (crates/sdk/src/local_
socket.rs:131-170, unchanged, confirmed by reading it) calls `Command::new(binary).args(
["server","start","--on-demand"])...spawn()` and drops the `Child` without ever
`wait()`-ing on it. That `spawn()` already did a real `fork`+`exec`: the process running
`steg server start --on-demand` is, from the instant it starts, a distinct PID with its
own address space. The two alternatives this task's own carried notes raise - an
`unsafe` `pre_exec` hook passing a bound listener and lock across a *second* exec; a
fork-wait-exit "stage 1" that reports success before a "stage 2" detaches - both solve a
problem this design does not have, because both assume the already-bound socket has to
move between two process images. It does not: this one process calls `arbiter::attempt`
itself, binds and locks itself, and only then detaches.

`nix::unistd::setsid() -> Result<Pid, Errno>` (confirmed by reading `nix-0.28.0/src/
unistd.rs:343-344`) is **safe** - the one `unsafe` block is inside `nix` itself, already
audited the same way this crate's existing `Flock`/`geteuid` use already relies on. It
is gated behind nix's `"process"` feature (confirmed: `nix-0.28.0/src/unistd.rs:167
-168`), **not** `"fs"`/`"user"`, which `steg-server`'s `Cargo.toml` already declares -
this task adds `"process"` to that list (section 11). `nix::unistd::daemon` is not the
alternative, checked rather than assumed: it is `cfg`-gated to `linux_android,
freebsdlike, solarish, netbsdlike` (`nix-0.28.0/src/unistd.rs:987-992`) - explicitly
**not** `macos`/`apple` targets, so it does not exist on this platform at all.
`nix::unistd::fork` is an **`unsafe fn`** in this version (`nix-0.28.0/src/unistd.
rs:287`) - the reason a double-fork design (an intermediate exits at once so the client
reaps a short-lived child instead of holding a zombie, section 7) is considered and set
aside: it would add the one `unsafe` block AC #6 forbids outright, to remove a cost the
sdk's own code already accepts and names.

## 2. An explicit `server start` runs in the foreground and never detaches (AC #1)

R3 and R99 give detaching only to an on-demand start; AC #1 now says this in so many
words ("runs in the foreground of the terminal or supervisor that started it"). No new
server-side function is needed for this path at all - section 5's entry point composes
`attempt`/`run` directly, with a fresh `Stopper` nobody else holds:

```rust
let stopper = steg_server::Stopper::new();
// `listen::run` requires a `&Stopper`; one must exist regardless of whether anything
// outside this call can reach it yet (nothing can, today - see below).
let result = steg_server::run(handle, &socket_path, &config, &stopper, None);
```

**What a person at a terminal sees, and how it is stopped.** `run_server_start` (section
5) prints "started a new server" (or the matching line for one already running, or a
refusal) and then blocks on `run` - the shell prompt does not return - until it ends.
**No signal handler is added, considered and rejected.** A graceful `SIGINT`/`SIGTERM`
handler (Ctrl-C calling `stopper.stop()` in order, rather than the OS's own default
disposition for either signal, which is immediate termination with no Rust code run at
all) would need either an `unsafe` `sigaction` registration (`nix::sys::signal`'s own
handler-installing calls are `unsafe`, since a signal handler's body is constrained to
async-signal-safe operations the compiler cannot check) or a new dependency
(`signal-hook`, which does the same thing behind a safe API, at the cost of a crate none
of this task's six criteria asks for). AC #6 forbids the first outright; nothing in
doc-2 or this task's own criteria asks for the second, and R99's own "stops only when
told to" reads more naturally as a request over the socket (`server stop`, TASK-1.45's
and a later task's to build) than a signal a person's terminal happens to deliver. **So
today: `kill -TERM`/`kill -INT`/Ctrl-C all end an explicit server by the OS's own
default action - immediate termination, no exit status in the normal sense at all** (a
process ended by an unhandled signal has no `exit()` code; a shell reports this its own
way, e.g. "terminated" or `128+signal`, which is the shell's convention, not this
process's). This is safe, not merely tolerated: the arbiter's lock releases on process
death by any means, including this one (TASK-1.44's own proven property, `attempt_
takes_over_a_stale_socket_left_by_a_dead_server`), so a raw kill never corrupts the
arbiter's own state for the next `attempt()`.

**What an interrupt leaves behind, stated exactly, not softened - two cases, since "when"
matters.** With no handler (section 2's own decision), the process can be killed at any
instruction boundary, and what is left differs by when:

- **Mid-startup, before `bind_with_mode`'s own rename onto the final path completes.**
  At worst, an orphaned temporary file (`.steg-tmp-<pid>-<n>`) beside the final path -
  never a file *at* the final path a later `attempt()` would have to trust or refuse
  (pre-existing TASK-1.44 behaviour, unchanged here, named rather than re-proven).
- **Once the server is already running, killed outright.** **The socket file stays at
  its final path** - nothing unlinks it; only `listen::run`'s own graceful shutdown
  sequence does that, and a raw kill never reaches it. **The lock is released** - the
  kernel does this on process death regardless, by any means (TASK-1.44's own proven
  property, `attempt_takes_over_a_stale_socket_left_by_a_dead_server`). **Once a
  `Runtime` exists** (not in this piece's own code, TASK-1.45's - named here because
  the consequence is real even though nothing in this task can trigger it yet): **every
  pane's program gets a hangup.** All of this process's file descriptors, including
  every pane's pty, close unconditionally on process death - a pty's master side closing
  delivers `SIGHUP` to whatever is attached to its slave side. A program with the
  default disposition for `SIGHUP` (most) ends from it; one that traps or ignores it
  (`trap '' HUP`, or anything run under something like `nohup`) survives as an orphan
  with nothing managing it - and **nothing forces it to end**, because there is no
  graceful shutdown code running on a raw kill at all. This is categorically different
  from `Runtime::stop`'s own graceful `SIGHUP`-by-pid mechanism (TASK-1.76), which only
  runs on a *controlled* stop; a raw kill never reaches it either.

**A fresh `steg server start` replaces the stale socket and prints "started" - proved
through the real binary, not only at the library level TASK-1.44 already proved it at.**

```rust
// crates/steg/tests/server_start.rs
fn a_fresh_start_replaces_the_socket_of_a_server_killed_outright() {
    let socket_path = unique_temp_socket_path();
    let mut first = KillOnDrop::new(spawn_steg_server_start(&socket_path, /* on_demand */ false));
    wait_for_socket(&socket_path, Duration::from_secs(5)); // §10's one named exception
    first.kill_now(); // bounded (§10) - kills, reaps, marks itself done so this
                       // guard's own `Drop` does not repeat the kill/wait pointlessly
    // The old listener is gone the instant the kernel tears it down with the
    // process; this is not waiting for anything to happen later, only for
    // `kill_now`'s own bounded `wait()` above to have already returned.
    let second = KillOnDrop::new(spawn_steg_server_start_capturing_stdout(&socket_path, false));
    let first_line = read_one_line_bounded(&second.child, Duration::from_secs(5));
    assert_eq!(first_line, "started a new server"); // not "a server is already
                                                      // running" - proves the stale
                                                      // socket was replaced, not
                                                      // mistaken for a live one
}
```

Reuses `arbiter::decide`'s already-proven branch (`ExistingFile::OursAndASocket`,
`Probe::Refused`, lock free → `Became`) - this test is new in what it exercises (the
real `steg` binary and its own printed line), not in the underlying mechanism, which
TASK-1.44 already proved at the library level.

## 3. `setsid` fails when the caller is already a process-group leader - both cases, with a test for each

**Why the sdk's own spawn path is never this case.** `std::process::Command::spawn`
performs a plain `fork`+`exec` with no job-control `setpgid` call of its own (confirmed
by reading `local_socket.rs`: no `.process_group(...)` is set) - the spawned process
inherits the *client's* process group, which is not its own pid, so it is not a group
leader and `setsid` succeeds.

**Why typing `steg server start --on-demand` at an interactive shell prompt is exactly
this case.** A job-control shell (`zsh`, `bash`) calls `setpgid(pid, pid)` on *every*
command it launches, foreground or background, making each one the leader of its own,
brand-new process group - this is how job control itself works, not specific to `&`. A
person running the on-demand form directly therefore already *is* a process-group
leader by the time this code's own `setsid()` call runs, and that call returns `EPERM`.

**The one new function this task adds to `steg-server`** - `detach()`, in a new, small
module scoped to exactly this (`.claude/rules/rust.md`: behaviour lives in named
modules, not `lib.rs`). Its only callers are outside this crate (`crates/steg`'s entry
point, and the fixture below) - worth naming as deliberate rather than a stray `pub`,
since the last round of this plan moved `session_counter` out for exactly the opposite
reason (no caller anywhere). The difference is R98 itself: "everything about starting,
detaching... is the server's own code" is not a preference this crate could choose to
place differently - detaching has to live in `steg-server` regardless of who calls it,
the same way `attempt`/`run` already have no caller inside this crate either and are
not treated as suspect for it:

```rust
// crates/server/src/detach.rs (new)

//! Leaving the session and process group inherited from whatever started this
//! process (R98: detaching is the server's own code, never the CLI's).

/// Starts a new session, detaching from any controlling terminal and the process
/// group inherited from whatever spawned this process. `Err` only if this process
/// is already a process-group leader (typically: invoked directly at an
/// interactive shell prompt, never through the sdk's own spawn path - section 3's
/// own tests cover both). Not fatal to a caller - `crates/steg`'s own handling
/// (section 5) continues attached and says so.
///
/// # Errors
/// Whatever `setsid(2)` fails with, converted from `nix::errno::Errno` via its own
/// `impl From<Errno> for io::Error` (confirmed by reading `nix-0.28.0`'s own
/// `errno.rs`, the same conversion `error.rs`'s existing `Io` variant already
/// relies on elsewhere in this crate) - kept as a plain `io::Result` rather than
/// wrapped in this crate's own `Error`, since nothing here needs `Error`'s richer
/// shape for a value a caller only ever displays, never pattern-matches.
pub fn detach() -> std::io::Result<()> {
    nix::unistd::setsid().map(|_pid| ()).map_err(std::io::Error::from)
}
```

**Decided: carry on attached, with a stated message - not a refusal.** This is an
unusual, debugging-shaped way to invoke `--on-demand` (every real client goes through
the sdk's spawn path, never types this directly), so refusing outright would make the
flag unusable for a curious or debugging person, and R99's guarantee (survives the
client) still matters only in the measure this attached mode can give it - attached,
the server is exactly as foreground-bound as an explicit start, which is an honest,
lesser guarantee stated plainly rather than silently dropped (section 5's own code).

**A real test for each case, both safe, zero `unsafe`.** The non-leader case is section
7's own SIGHUP test (setsid succeeds silently, no message). The leader case needs a
process that is *itself* already a group leader before it becomes the server - built as
a small fixture binary (the same shape as TASK-1.44's own `race-attempt`, feature
-gated the same way, living in `crates/steg` since it needs `CARGO_BIN_EXE_steg`, only
set for that crate's own tests - `crates/steg/tests/fixtures/already_a_group_leader.rs`):

```rust
use std::os::unix::process::CommandExt; // for `.exec()`, below - a trait method,
                                         // not in scope without this import

fn main() {
    steg_server::detach().expect("the fixture must become its own leader to set up the case");
    let err = std::process::Command::new(env!("CARGO_BIN_EXE_steg"))
        .args(["server", "start", "--on-demand"])
        .exec(); // std::os::unix::process::CommandExt::exec - safe: replaces this
                 // process's image in place via `execve`, no fork, nothing left
                 // half-forked for the compiler's unsafety rules to worry about.
                 // Returns an `io::Error` only on failure - on success this
                 // process's image is already gone, so the panic below is
                 // unreachable-on-success, not a missing branch.
    panic!("exec failed: {err}");
}
```

Reusing `steg_server::detach()` itself (rather than calling `nix::unistd::setsid()` a
second time) is what lets this fixture live in `crates/steg` with **no new dependency**
- `nix` stays confined to `steg-server`; `boundaries.toml`'s `[steg]` entry is untouched.
`exec()` keeps the same pid, the same (now session-and-group-leader) process group, and
the same open file descriptors - so the real `steg` binary that replaces this process's
image is running as the exact precondition being tested. The test spawns this fixture
with piped stdout, asserts the "could not detach... continuing attached" line appears,
and that the server still starts and answers a connection (not refused) - the chosen
behaviour is "carry on," not "refuse," and this is what proves it rather than only
documenting it.

**`detach()` has no unit test of its own calling it directly in-process.** Doing so
would change the *whole test binary's own* session and process-group membership for the
rest of that `cargo test` run - a process-wide side effect that would corrupt any other
test sharing that process under default parallelism, the same hazard TASK-1.44's own
`socket_mode.rs` (the one test that sets the process `umask`) was given its own,
separate integration-test file to avoid. Its success path is proved instead by section
7's SIGHUP test (a real, separate process, its own call); its failure path by this
section's fixture test - both already necessarily out-of-process, so neither needs a
unit test to add the same risk a second time.

## 4. What detaching leaves shared with the client, and what is done about each

Four things a freshly spawned process starts out sharing with whatever spawned it,
addressed one at a time rather than left implicit:

- **The controlling terminal.** `setsid()` removes this entirely - a brand-new session
  has none until something later opens a terminal device without `O_NOCTTY`, which
  nothing in this server's own code ever does. This is the whole mechanism behind "does
  not die of a hangup": `SIGHUP` from a closing terminal is delivered to the terminal's
  own controlling session, and once detached this process is not in it. Proved in
  section 7's own SIGHUP test, and its own negative control (section 10).
- **Standard input, output and error.** `setsid()` touches none of these; for the sdk's
  own spawn path they are already `/dev/null` (`local_socket.rs`'s existing `Stdio::
  null()` on all three, unchanged) before this code ever runs, so "does not write to a
  dead terminal" is already true for that path regardless of what this task does.
  **Nothing in this server's own steady-state code writes to stdout/stderr or reads
  stdin at all** - confirmed by grep across `listen.rs`/`connection.rs` for `print`/
  `eprint`/`stdin`: none, outside this task's own one-line startup messages in `crates/
  steg`. So even for the group-leader-attached case above, where these fds really are
  the person's live terminal, there is nothing left to write to them once that message
  is printed - a terminal closing afterward has no later write to fail against. Not
  closed or redirected by this task, because nothing needs it: doing so would cost a
  design decision (closing stdin on an attached, interactive invocation is a surprise)
  for no correctness gained.
- **The current working directory.** Inherited from the spawning client by default
  (`Command::spawn` does this, unchanged). **A `chdir("/")` once detached was drafted
  for this - the standard daemonising practice `nix::unistd::daemon`'s own `nochdir=
  false` default would have given, had it existed on macOS - and dropped before
  writing it as the real design.** `std::env::set_current_dir` is itself a plain safe
  function (unlike `set_var`/`remove_var`), but it is a *process-wide* change that would
  run after `resolve_socket_path` has already resolved `config.socket_path` (section 5
  calls it before `attempt()`) - fine for the three real-environment sources
  (`XDG_RUNTIME_DIR`/`TMPDIR`/the `/tmp` fallback, all absolute by construction,
  `steg_sdk::socket_path`'s own code), but `STEG_SOCKET_PATH` is a raw override with no
  such guarantee, and this task's own tests set exactly that variable. A relative
  `STEG_SOCKET_PATH` combined with a `chdir` between the bind and `listen::run`'s own
  final `std::fs::remove_file(socket_path)` would make that cleanup silently target the
  wrong path - a real hazard this task's own test paths could trigger, traded for a
  theoretical one (nothing in this piece's own code resolves a relative path *after*
  startup, so staying in whatever directory the client was in costs nothing today).
  Left out on that balance, not because the daemonising idea itself was wrong.

## 5. The entry point, "started" meaning what it says, and exit statuses (AC #1, #5)

Hand-matched, since the generated CLI (TASK-1.49) does not exist - `crates/steg/src/main.rs`:

```rust
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let code = match args.iter().map(String::as_str).collect::<Vec<_>>().as_slice() {
        ["server", "start"] => run_server_start(false),
        ["server", "start", "--on-demand"] => run_server_start(true),
        _ => { println!("steg: not yet implemented"); 0 } // today's placeholder, unchanged
    };
    std::process::exit(code);
}

fn run_server_start(on_demand: bool) -> i32 {
    let config = steg_server::ServerConfig::default(); // section 8: unchanged by
                                                         // this task, before or
                                                         // after TASK-1.45/1.87
    let socket_path = steg_server::resolve_socket_path(&config); // newly `pub`, section 11
    match steg_server::attempt(&config, &socket_path) {
        Ok(steg_server::Arbitration::AlreadyRunning) => { println!("a server is already running"); 0 }
        Ok(steg_server::Arbitration::Refuse(reason)) => {
            eprintln!("steg: could not start a server: {reason}"); // untrusted file/
            4                                                      // directory, or
        }                                                          // arbitration timed out
        Err(err) => { eprintln!("steg: could not start a server: {err}"); 4 }
        Ok(steg_server::Arbitration::Became(handle)) => {
            println!("started a new server");
            if on_demand {
                if let Err(err) = steg_server::detach() {
                    eprintln!(
                        "steg: could not detach from this terminal ({err}); continuing \
                         attached - closing this terminal will end this server"
                    );
                }
            }
            let stopper = steg_server::Stopper::new();
            match steg_server::run(handle, &socket_path, &config, &stopper, None) {
                Ok(()) => 0,
                Err(_) => 1,
            }
        }
    }
}
```

**"Started" means the socket already accepts, provably, not by convention.** `arbiter::
attempt`'s own `Became` variant is returned only *after* `bind_with_mode` has already
succeeded (read directly: `arbiter.rs`'s `Decision::Became` arm binds, then constructs
`Arbitration::Became`; there is no path that returns `Became` before binding). So
`println!("started a new server")` runs strictly after the listener is already live -
a client connecting the instant that line is printed is guaranteed to succeed, not
racing the bind. Proved, not just reasoned about: a test captures the real binary's
stdout on a pipe, and the moment the "started" line arrives on it, immediately attempts
a real `UnixStream::connect` with **no retry and no wait at all** - a single, unretried
attempt that must succeed, which would fail intermittently if the ordering were ever
reversed.

**Exit statuses, each named rather than grouped, per doc-2 section 6:**

| Outcome | Status |
|---|---|
| Started (became the server) and `run` later returns cleanly | 0 |
| Already running | 0 |
| Already running, specifically a loser of a simultaneous-start race (section 6's own test asserts this exact case) - same status, named separately since it is the one this piece's own race test checks | 0 |
| Refused - an untrusted existing file or an unsafe directory (`Arbitration::Refuse`) | 4 |
| Refused - arbitration timed out (`Arbitration::Refuse(ArbitrationTimedOut)`) | 4 |
| A genuine I/O error from `attempt` itself | 4 |
| Started, then `run` returns an `Err` (an accept-loop I/O failure) | 1 |
| Ended by an unhandled signal (section 2) | none - not an `exit()` status at all |

**No test-only hook (AC #5).** Nothing in `main.rs` or `detach.rs` branches on anything
that exists only for a test to control; `STEG_SOCKET_PATH` (already existing) is the
one override every test and a real debugging session share, unchanged by this task.

## 6. Two (and several) real clients converge on one server (AC #3)

**Test-owned `std::process::Command`s, not the sdk's `connect()`, for this specific
test.** The sdk's `connect()` deliberately never returns a `Child`, so a test going
through it cannot deterministically end the real process it starts, and this piece has
no idle-stop to lean on for cleanup (TASK-1.86's). Spawning the same command the sdk
would (`<CARGO_BIN_EXE_steg> server start --on-demand`, same `STEG_SOCKET_PATH`) from
the test directly, kept as held `Child`s, is still real, separate OS processes racing
for the same socket - the thing AC #3 asks to be shown.

```rust
// crates/steg/tests/server_start.rs (new)
fn four_simultaneous_starts_converge_on_one_server() {
    let socket_path = unique_temp_socket_path();
    let barrier = Arc::new(std::sync::Barrier::new(4));
    let children: Vec<KillOnDrop> = (0..4)
        .map(|_| { /* spawn past the barrier, as close together as four threads can
                      manage; see §10 for KillOnDrop */ })
        .collect();
    wait_for_socket(&socket_path, Duration::from_secs(5)); // §10's one named exception
    let (winners, losers) = partition_by_try_wait(&children); // polls `try_wait`
                                                                // repeatedly, bounded;
                                                                // never a bare `wait()`
    assert_eq!(winners.len(), 1, "exactly one of four should still be running");
    assert_eq!(losers.len(), 3);
    for loser in &losers { assert_exit_status(loser, 0); } // "already running"
}
```

`partition_by_try_wait` needs no server-side signal to tell winner from losers: a loser's
`attempt()` resolves to `AlreadyRunning` and the whole process exits almost immediately
with status 0 (section 5's table); the winner's `run()` blocks forever (no idle-stop in
this piece), so repeated, bounded `try_wait()` calls distinguish them cleanly without
ever blocking on the winner.

## 7. What the client is left holding, with SIGKILL and SIGHUP tests that prove different things

**What the client is left holding, by construction, since there is no fork-then-exit
"stage 1."** The sdk's own `Command::spawn()` result is, on every path, a real OS child
it never `wait()`s on:

- **Loses the race**: prints and exits quickly (section 5's table). The client's
  dropped `Child` becomes a zombie the instant it exits, reaped only when the client
  itself exits (reparented to `launchd`, which reaps orphans). The sdk's own existing
  doc comment on `connect` already names this as an accepted cost this task keeps
  *short*, not eliminates - eliminating it would need the client to `wait()`, which R99
  forbids.
- **Wins**: a live, long-running process (not a zombie) until it eventually stops - at
  that point, the same rule.

**Proved by test, not only stated**: `server_start.rs`'s own zombie test spawns through
a test-owned `Command` (so the test can inspect the pid directly), lets it lose a race
against an already-running server, explicitly drops its `Child` *without* waiting
(reproducing the sdk's own behaviour), shells out to `ps -o stat= -p <pid>` and asserts
exactly one line containing `Z` comes back (an empty result - no row at all, which `ps`
gives for a pid already reaped - fails the assertion loudly, not silently read as "not a
zombie"), then reaps it itself (`.wait()`) as test hygiene.

**SIGKILL and SIGHUP, kept separate because they prove different things (section 1, 3,
4):**

- `the_client_being_sigkilled_does_not_end_the_server`: a small test-only binary whose
  only job is calling `steg_sdk::local_socket::connect` with `autostart: true`, SIGKILLed
  the instant it has spawned the server (synchronised on the socket appearing), then the
  socket is confirmed to still accept a connection. Proves something already true "for
  free" from how Unix process spawning works (`SIGKILL` to a PID does not cascade to its
  children) - not the detaching mechanism, named as such.
- `the_clients_process_group_being_sighupd_does_not_end_the_server`: the same shape, but
  `kill(-pgid, SIGHUP)` on the client's *whole process group* once the server is up -
  this is the test that actually exercises `detach()`.

## 8. One `ServerConfig`, three tasks, no `Runtime` here

**This task does not construct or stop a `Runtime` - that changes from the shared
design, settled by TASK-1.45's own plan.** TASK-1.45 (frames and requests, its plan
being finalised now) is what creates the one `Runtime`, inside `listen::run` itself,
from `ServerConfig` (which gains a `launcher` and a `login` field, both with their own
`Default` impl), shares it with the connections it routes to, and stops it explicitly in
`run`'s own shutdown - exactly where in that sequence (the listener closes before the
connection-join loop today, TASK-1.44's own nit fix; the socket file and lock go after
it) is TASK-1.45's own plan to settle, not asserted here - not this task's code, and
not `start.rs`/`serve_on_demand`-style wrapper functions this plan's earlier drafts
proposed (removed). `runtime.rs` is untouched by this piece.

**What this task puts in `ServerConfig`, exactly, for each order the two branches could
land in - read directly against TASK-1.45's own plan, not guessed at.** TASK-1.45's own
plan text, as it stood when read for this plan (its "Who owns `Runtime`, and how it is
stopped" section, read 2026-11-15 - that task's own plan is still with the engineer and
may change before approval, so this quote is accurate as of when it was written, not
asserted to still match whatever TASK-1.45 is approved with), already names this exact
call site: "TASK-1.77's own entry point... passes `login` resolved from the password
database as R99 asks, and can leave `launcher` at its default." That
expectation predates the split - this task no longer resolves a login identity at all
(section 9, TASK-1.87's now) - so it needs correcting here rather than left to collide
quietly at review time:

- **As this task is built today** (`ServerConfig` has neither field yet, since
  TASK-1.45 has not merged): `run_server_start` (section 5) constructs `ServerConfig::
  default()` with nothing extra set, because there is nothing yet to set.
- **If TASK-1.45 merges first**, this task's own worker, rebasing onto it, adds one
  explicit line before calling `attempt`: `config.login = steg_server::LoginShell::
  from_process_env();` - the *same* value `login`'s own default already gives (TASK-1.45:
  "`LoginShell::from_process_env()`"), so this changes nothing about what the server
  does. It is written explicitly anyway, not left to the default, for one reason: it is
  the one line TASK-1.87 needs to find and swap for its own real resolver, and a line
  that is not there to find is a worse hand-off than a redundant one that is.
  `launcher` is left at its own default - TASK-1.45's own text already says this task
  has no reason to touch it.
- **If this task merges first**, TASK-1.45's own worker is the one adapting at that
  point, not this plan - its own call site for `ServerConfig`'s two new fields' defaults
  is its design to make, and this task's `run_server_start` has nothing to change on its
  side regardless of which way the race goes, since `::default()` already compiles
  against either shape.

This is the "touch the struct in a way that merges cleanly" property stated directly:
three tasks (this one, TASK-1.45, TASK-1.87) each add, default, or later override one
field of `ServerConfig`, and whichever lands second only ever *adds* a line at a call
site, never edits one that already exists.

**If this task merges before TASK-1.45, the server process simply has no `Runtime` at
all - said plainly, not implied.** A server this task starts completes real handshakes
(TASK-1.44's own scope, untouched) and does nothing past them: no session can be
created, no pane started, because nothing constructs a `Runtime` and nothing routes a
request to one yet. This is not a regression this task introduces - it is the state the
whole repository is in today (confirmed earlier: no production call to `Runtime::new`
exists anywhere on `origin/main`) - this task's own contribution is making the *socket*
reachable through a real binary for the first time, nothing about panes or sessions.

## 9. What this leaves, by name, stated plainly

- **The idle rule (TASK-1.86).** An on-demand server built by this piece **never stops
  by itself** - there is no idle checker here at all, by construction (nothing holds a
  `Stopper` past `run_server_start`'s own scope). A test that needs to end one it
  started must do one of two things: call `Stopper::stop()` directly, for an in-process
  test that holds its own clone across the thread boundary (`listen::run`'s own existing
  tests already do this); or send a real signal to the real process, for an
  out-of-process test (sections 6, 7's own zombie/race tests - a `kill()` on the held
  `Child`). This task's own worked-out predicate design is in this task's notes, headed
  for TASK-1.86, not in this plan.
- **The login environment (TASK-1.87).** `ServerConfig`'s own `login` field (TASK-1.45's
  addition) keeps whatever stand-in default TASK-1.45 gives it until TASK-1.87 replaces
  it - this task never reads or sets it at all. This task's own worked-out design for
  resolving a login *identity* from the password database is in this task's notes,
  headed for TASK-1.87, with the gap stated there too: identity (which shell, which
  home) is necessary but not what R99 asks for (the environment a login shell gives,
  PATH above all) - TASK-1.87's own acceptance criteria already say so.
- **Routing a real request to `Runtime::handle` at all (TASK-1.45, in planning now).**
  Section 8 names exactly what it adds to `ServerConfig` and where it constructs the
  `Runtime`; nothing further to say here.

## 10. Testing discipline this piece's own multi-process tests follow throughout

**Every process a test starts is ended and waited on even when the test fails.** A bare
`child.kill(); child.wait();` at the end of a test function is skipped by a panicking
assertion partway through, leaking the process. Every multi-process test in this piece
wraps each spawned `Child` in:

```rust
struct KillOnDrop {
    child: std::process::Child,
    done: bool, // set once `kill_now` has already reaped it, so `Drop` is a no-op -
                // without this, a test that deliberately kills mid-test (section 2's
                // own stale-socket test does exactly this) would otherwise `kill`/
                // `wait` an already-reaped child a second time from `Drop`
}

impl KillOnDrop {
    fn new(child: std::process::Child) -> Self {
        Self { child, done: false }
    }

    /// Kills and reaps now, for a test that needs the process gone *before* the end
    /// of the function, not only at teardown - marks itself done so `Drop` finds
    /// nothing left to do.
    fn kill_now(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait(); // bounded: a killed process's wait() returns promptly
        self.done = true;
    }
}

impl Drop for KillOnDrop {
    fn drop(&mut self) {
        if !self.done {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}
```

so a failing assertion still cleans up via the unwind, the same guarantee `?` and an
early return already get from normal control flow, extended to a panic.

**`wait_for_socket` is this piece's one named exception to "no sleeps as
synchronisation" - stated as one, not left to contradict the claim below.** A socket
appearing at a path is a filesystem change this process has no notification for (no
`inotify`-equivalent is used here, and adding one for a handful of tests would be a
disproportionate answer); every one of this section's multi-process tests that waits
for a freshly spawned server's socket to exist does so with a short, bounded poll
(`UnixStream::connect` retried on a short interval, up to a stated deadline), the same
shape TASK-1.44's own sdk-side `connect()` retry loop already uses for the identical
reason. Named here exactly once, the way TASK-1.44's own plan named its own single
sleep exception, rather than claimed away.

**Repeated runs, at this project's actual default test parallelism, with counts.**
Every test in sections 3, 6 and 7 that touches a real process, a real socket or real
signal delivery is run at least 10 times consecutively via `cargo test`'s own default
thread count (not `--test-threads=1`) before being cited as proof, counts recorded with
`record-check` - the same discipline TASK-1.44/1.76's own reviews required, run at full
concurrency specifically because this machine has shown contention under heavy parallel
testing before (TASK-1.76's own `openpty` finding) - though that finding does not apply
here directly: no pseudo-terminal is opened anywhere in this piece (no `Runtime` is
constructed at all, section 8, so nothing calls `RealLauncher::spawn`). What this
piece's own width actually stresses is concurrent socket binds and process spawns, and
`arbiter::attempt` already has eight-wide proof of exactly that (TASK-1.44's own
race.rs, ten consecutive runs, no flake) - this piece's four-wide real-process test
(section 6) rides the same already-validated mechanism, with real process-spawn
overhead on top rather than a new source of flakiness.

**A negative control for each mechanism this proof rests on:**

- **`detach()`'s call to `setsid` removed** (a scratch edit, reverted after, recorded
  with `record-check` the way TASK-1.44's own "prove one bites" steps are): the SIGHUP
  test (section 7) fails - the server dies with the client's process group. The SIGKILL
  test does **not** fail with `setsid` removed, and this is said honestly rather than
  treated as a gap: that property never depended on detaching at all (section 7's own
  framing), so a negative control that "proves" it by failing would be proving the
  wrong thing; its absence of failure here is the correct, stated outcome, not a
  missing test.
- **The entry point's print moved before `attempt()` resolves** (a scratch edit):
  section 5's own race-the-print test fails intermittently (not every run, since the
  bind is fast) - recorded across several runs to show the intermittency itself, not
  just one failing run, since a single pass would understate what the test is actually
  proving against.

## 11. Dependencies and unsafe

**One Cargo.toml change**: `steg-server`'s `nix` feature list gains `"process"`
(section 1's `setsid`), alongside the existing `"fs"`, `"user"` - no new external crate
anywhere, including `crates/steg` (section 3's fixture reuses `steg_server::detach()`
rather than depending on `nix` itself). `boundaries.toml`'s `[steg-server]` entry's
`reason` and feature list are updated to name it; `just check-boundaries` proves the
declared features match what is actually used, confirmed in the build phase rather than
assumed here.

**Unsafe: none, anywhere in this piece.** `setsid` (section 1) is a safe wrapper already
audited inside `nix`; the group-leader fixture's own `Command::exec()` (section 3) is
also safe (replaces the process image via a direct `execve`, no fork, nothing left
half-forked); no `fork`, no `pre_exec`, no `FromRawFd`, no signal handler, no `std::env::
set_var`/`set_current_dir`.

## Proof, by acceptance criterion

- **AC #1** (autostart through the sdk; explicit stays up with no clients, in the
  foreground): the sdk's own `connect()` already does the client half (TASK-1.70,
  unchanged); section 5's entry point is what makes the spawned process actually become
  a server. `explicit_start_stays_up_with_no_client_ever_connecting`: spawns `steg
  server start` with no flag, waits for the socket to accept, waits a short bound with
  **no client ever connecting**, asserts the process is still alive and the socket still
  accepts - a real, biting assertion (nothing in this task's own code *could* stop it,
  and this proves that remains true rather than reading the code for it). Section 2's
  own "no signal handler" reasoning and its exit-status table cover "runs in the
  foreground," and its own `a_fresh_start_replaces_the_socket_of_a_server_killed_
  outright` test covers exactly what an interrupt leaves behind through the real
  binary, not only at the library level.
- **AC #2** (on-demand survives the client, shown with real processes): the SIGKILL test
  (section 7).
- **AC #3** (two/several real clients, one server): section 6's four-wide test, the
  `AlreadyRunning`-status assertion on each loser.
- **AC #4** (client's part stays thin; nothing added to the sdk): proved by construction
  - `crates/sdk` is untouched by this piece (`git diff --stat` against `origin/main`
  shows nothing under `crates/sdk/`, checked in the build phase); `run_fake_server_for_
  tests` is left exactly where it is - the sdk's own tests need a dependency-free
  stand-in and doc-16's table does not let the sdk depend on `steg-server`/`steg`, so
  there is no real binary for its own suite to spawn instead.
- **AC #5** (both forms run the server, print their outcome, doc-2's exit statuses, no
  test hook): section 5's table and its own timing proof; the group-leader message
  (section 3) and the "could not start" messages (section 5) are the only new prints,
  neither gated on anything test-only.
- **AC #6** (every spawned child/thread accounted for; what the client holds; no
  unsafe): section 7's zombie test is the whole of this criterion's "what the client
  holds" clause - there is no idle checker, no `Runtime`, no drain thread in this
  piece's own code to account for beyond it; section 11's own accounting for zero
  `unsafe`.

**No sleeps as synchronisation, with the one named exception section 10 states as
one.** Every other wait is on a real event: `try_wait()`/`wait()` on a real process, a
pipe actually producing the expected line, or `ps`'s own output. `wait_for_socket`
(section 10) is a bounded poll, not a blocking wait on a notification - there is none
to block on - named there rather than folded silently into "a real event." Section 10's
negative controls are the one other place a scratch edit stands in for "what if this
mechanism were missing," reverted immediately after being recorded.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Carried from TASK-1.70 (2026-11-15): the sdk exports run_fake_server_for_tests as a public, doc-hidden function behind the local-socket feature. TASK-1.44's plan found it cannot simply be removed (the sdk may not depend on the real binary), but the fixture could bind through the public socket_path itself; say in this task's plan whether to move it out of the public interface here.

Carried from TASK-1.44's review (2026-11-15), for this task's plan: (1) the arbiter's liveness probe connects to a running server and is reported on the ConnectionEvent seam as a client arriving and leaving, so the idle rule must not let probes hold an idle server up, or must tell them apart. (2) Stop the server only through the one stop function TASK-1.44 leaves public after its review fix (the first version had a stop() that set the flag without waking accept, which would have hung an idle server: exactly this task's case). (3) On shutdown there is a window while connection threads are joined in which the listener is still bound; see TASK-1.44's notes for what a connecting client sees.

Split on 2026-11-15 under the engineer's standing decision to split by default, into the three pieces the worker's plan proposed. This task is now the first: the entry point, starting on demand, detaching, and two clients converging. The idle stop and the server's login-shell environment are drafts under TASK-1. The plan on this task was written for all three and is to be rewritten for this piece only.

For TASK-1.86 (the idle stop), worked out while planning this task, not built here:

A pure predicate, time and the session count both passed in, never read from a clock or
a counter the predicate owns itself (`.claude/rules/testing-rust.md`: "Time... come[s]
in as parameters... No test sleeps to wait for something"):

```rust
struct IdlePredicate {
    live_clients: i64,
    empty_since: Option<Instant>,
}

impl IdlePredicate {
    fn on_event(&mut self, event: ConnectionEvent, now: Instant) {
        match event {
            ConnectionEvent::Connected => {
                self.live_clients += 1;
                self.empty_since = None; // a client just arrived; not idle right now
            }
            ConnectionEvent::Disconnected => {
                self.live_clients -= 1;
                // empty_since is (re)started on the next tick(), not here, so a
                // session that is still open at this instant correctly keeps it None.
            }
        }
    }

    /// `now` and `session_count` are both given, never read by this function.
    fn tick(&mut self, now: Instant, session_count: usize, idle_timeout: Duration) -> bool {
        if self.live_clients > 0 || session_count > 0 {
            self.empty_since = None;
            return false;
        }
        let since = *self.empty_since.get_or_insert(now);
        now.duration_since(since) >= idle_timeout
    }
}
```

Unit tests (literal `Instant`s, no sleeping, same technique as `connection.rs`'s own
`time_remaining` tests): empty from construction and ticking past `idle_timeout` returns
`true`; a `Connected` with no matching `Disconnected` yet never returns `true` no matter
how far `now` moves (AC #2's "a client mid-handshake counts"); a session count that is
non-zero never fires regardless of clients; a `Connected`/`Disconnected` pair arriving
and leaving within the same tick (the arbiter-probe shape) does not reset `empty_since`
to a *later* instant than the one already recorded (`get_or_insert`'s own behaviour -
check this directly, not assumed, it is the one line the probe-caveat guarantee rests
on). The driving loop holds the channel's `Receiver<ConnectionEvent>`, the `Stopper`,
and a `session_count: impl Fn() -> usize + Send + 'static`.

That closure's own shape is open, not settled here, because the `Runtime` it reads from
no longer lives where this was first sketched: TASK-1.45's own plan has `listen::run`
construct and own the one `Runtime`, not a value this task passed around. What this
predicate needs from it is unchanged - a cheap, independently-cloneable way to read the
live session count, callable repeatedly from the checker's own thread long after any
single borrow of `Runtime` could last (the same reason `Runtime`'s own drain thread
clones its internal `Arc<Mutex<State>>` into its closure rather than borrowing `self`).
Whether that is a method on `Runtime` itself (`session_counter(&self) -> impl Fn() ->
usize + Send + 'static`) or something `listen::run` builds and hands to the checker
directly is TASK-1.86's own call, made against whatever `listen::run` actually looks
like once TASK-1.45 lands - not guessed at here.

The arbiter's probe (`connection.rs:27-42`, already documented as a caveat) connects and
disconnects within milliseconds, never sending a hello - it resets `empty_since` and
restarts the clock from very close to where it was, costing at most a few seconds during
active arbitration, never an indefinite hold, since nothing about an idle, already
-decided server calls `probe()` again.

"A client that connects every 29 seconds and sends nothing": decided, not a bug. Any
connection - complete, incomplete, or never completing until `handshake_timeout` force
-closes it - resets `empty_since` to `None` the instant it is accepted (AC #2's own
wording: a client counts from acceptance, not a completed handshake). A connection every
29 seconds is force-closed by the handshake timeout (10s default) well before the next
one arrives, leaving roughly 19 idle seconds between closes - under the 30-second
threshold every time, so such a server never idle-stops for as long as that pattern
continues. Accepted: the predicate cannot distinguish a hostile reconnector from a real,
well-behaved periodic health check without a signal the wire protocol does not carry
(the same limit already named for the arbiter's own probe).

The relationship with `arbitration_timeout`: a client's `connect()` failing against a
stopping server makes the sdk spawn a fresh attempt, which finds the socket file still
present with its lock still held by the stopping server, and retries for up to
`arbitration_timeout` (5s default) before giving up. For the idle-stop case specifically
this window is close to instant - the predicate only fires `stop()` when `live_clients ==
0`, so the listener's own tracked connections are already empty (or very nearly so) at
that moment, meaning there is nothing of substance to join before the lock is released -
5 seconds is comfortably enough. Named here as the relationship, not left for a timing
-dependent test failure to discover later.

A gap this task found and TASK-1.77's own notes are also recording, carried here since
TASK-1.86 is what makes it reachable in normal use (the person splitting this work is
recording it on this task directly too): `listen.rs`'s accept loop can `accept()` a
connection and then see the stop flag true before the handshake code ever runs, dropping
it with zero reply. From the client's side that is `connect()` succeeding followed by
end of file - not `ConnectionRefused`/`NotFound` - a third outcome TASK-1.44's own
shutdown-ordering notes do not cover. Whatever drives a client-side handshake (TASK-1.49
or wherever the sdk's own `Connection::new(Role::Client, ...)` first gets a real caller)
needs to treat this the same as a refusal - try starting a fresh one - not as a protocol
failure to report to the person.

For TASK-1.87 (the login environment), worked out while planning this task, not built
here - and worth stating plainly first: what follows resolves an *identity* (which
shell, which home, which user) from the password database, which TASK-1.87's own
acceptance criteria already say is necessary but not sufficient - R99 asks for the
environment a login shell gives (PATH above all), not only who that shell is. Read
TASK-1.44's own first plan (before its split) for its earlier design capturing the
environment itself with a cleared, seeded child and a bounded wait, which is closer to
what's actually required; what's below is only the identity half of it:

```rust
pub struct LoginIdentity {
    pub shell: PathBuf,
    pub home: PathBuf,
    pub user: String,
}

pub fn resolve_login_identity(timeout: Duration) -> LoginIdentity {
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let result = nix::unistd::User::from_uid(nix::unistd::geteuid());
        let _ = tx.send(result); // dropped silently if the receiver already gave up
    });
    match rx.recv_timeout(timeout) {
        Ok(Ok(Some(user))) => LoginIdentity { shell: user.shell, home: user.dir, user: user.name },
        Ok(Ok(None)) | Ok(Err(_)) | Err(_) => fallback(), // no passwd entry, a real
                                                           // errno, or the timeout
    }
}
```

`nix::unistd::User::from_uid` is already **safe** (the `unsafe` is inside `nix` itself
around `getpwuid_r`, `nix-0.28.0/src/unistd.rs:3393-3402`), already behind the `"user"`
feature `steg-server` already declares (`nix-0.28.0/src/unistd.rs:3131-3132`), and
returns `.shell`/`.dir`/`.name` directly - zero new `unsafe`. On macOS, `getpwuid_r`
resolves through the directory service (`opendirectoryd`), which can in principle be
slow or stall on a machine whose accounts are bound to a network directory - the bounded
wait's own reason, same as R99's "cannot hang the server" language, even though this is
not literally sourcing a shell's `.zprofile`. On the timeout path the worker thread is
*not* joined - joining it would reintroduce the exact unbounded hang the bound exists to
prevent; this is not the same kind of leak a child *process* left unreaped is, since a
dropped `JoinHandle` lets the OS thread finish and reclaim its own resources on its own,
holding nothing open in the meantime.

Rejected, considered first: mutating this process's own environment table (`std::env::
set_var` for `SHELL`/`HOME`/`USER`) so `portable_pty::CommandBuilder`'s own ambient-env
read would pick it up for free. Wrong for two reasons: `set_var` is an `unsafe fn` (the
documented hazard is exactly concurrent access from another thread), and the bounded
-wait thread above does not join on its timeout path, so "nothing else is running yet"
would not actually be true by the time the call happened; and `Runtime`'s own `login`
field (TASK-1.45's addition to `ServerConfig`) already *is* the real seam - data flowing
in through a field, not an ambient table every future call site has to happen to read
correctly.

Rewritten plan for the first piece checked against doc-8 (R3, R98, R99), doc-2 section 6, doc-16 and decision-2 section 7 by @bruk.o: ok. The server process the sdk's provider spawns detaches itself with one safe setsid call (a feature added to the nix dependency the crate already has), so there is no fork, no descriptor passing and no unsafe; the steg entry point composes attempt, the printed line, detach for an on-demand start, and run, with no server logic of its own (R98). An explicit start runs in the foreground and is not detached (R3, R99); an interrupt is left at its default for this piece, with what it leaves behind stated, and the new server-stop task decides whether to handle it. A start that cannot detach because it is already a process-group leader carries on attached with a message, with a test. 'started' is printed only after the socket is bound. This task constructs no Runtime: TASK-1.45 creates and stops it inside run from ServerConfig. Stated plainly as left for TASK-1.86 and TASK-1.87: an on-demand server here never stops by itself, and panes still inherit the starting client's environment.

Plan updated after the check above (interrupt consequences stated in full, a test that a fresh start replaces a killed server's socket through the real binary, the ServerConfig merge order with TASK-1.45): re-read by @bruk.o, still ok; the plan constructs no Runtime. The digest to approve is the one asdlc mine shows after this update.

Check passed: `just check` exited 0 at 2026-11-16T00:29:34+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.77 on task-1.77-server-start-on-demand-detach-and-stop at 221ef18c07f7:

```text
ok      format check (0.2s)
ok      lint (90.7s)
ok      boundaries (0.2s)
ok      licences (0.7s)
skip    tests: this change touches no crate
ok      build (steg-sdk) (3.2s)
ok      test (steg-sdk) (1.9s)
ok      test (steg-sdk, local-socket) (2.0s)
ok      build (steg-kernel) (45.5s)
ok      test (steg-kernel) (1.4s)
ok      build (steg-server) (50.3s)
ok      test (steg-server) (5.0s)
ok      test (steg-server, race-fixture) (5.5s)
ok      build (steg) (0.2s)
ok      test (steg) (0.4s)
ok      build (steg-app) (18.4s)
ok      test (steg-app) (0.8s)
ok      tool tests (1.5s)
ok      architecture (1.0s)
ok      recipe names (0.1s)
```

Check passed: `cargo test -p steg --features fixtures --test server_start -- the_clients_process_group_being_sighupd_does_not_end_the_server the_client_being_sigkilled_does_not_end_the_server` exited 0 at 2026-11-16T00:37:13+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.77 on task-1.77-server-start-on-demand-detach-and-stop at 221ef18c07f7 with uncommitted changes:

```text
   Compiling steg v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.77/crates/steg)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.23s
     Running tests/server_start.rs (target/debug/deps/server_start-6d77ec59f682e657)

running 2 tests
test the_client_being_sigkilled_does_not_end_the_server ... ok
test the_clients_process_group_being_sighupd_does_not_end_the_server ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 6 filtered out; finished in 0.19s
```

Check FAILED: `cargo test -p steg --features fixtures --test server_start -- the_clients_process_group_being_sighupd_does_not_end_the_server the_client_being_sigkilled_does_not_end_the_server` exited 101 at 2026-11-16T00:38:39+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.77 on task-1.77-server-start-on-demand-detach-and-stop at 221ef18c07f7 with uncommitted changes:

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.10s
     Running tests/server_start.rs (target/debug/deps/server_start-6d77ec59f682e657)

running 2 tests
test the_clients_process_group_being_sighupd_does_not_end_the_server ... FAILED
test the_client_being_sigkilled_does_not_end_the_server ... ok

failures:

---- the_clients_process_group_being_sighupd_does_not_end_the_server stdout ----

thread 'the_clients_process_group_being_sighupd_does_not_end_the_server' (16064104) panicked at crates/steg/tests/server_start.rs:72:9:
the server should still be answering after its client's whole process group was SIGHUPed
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    the_clients_process_group_being_sighupd_does_not_end_the_server

test result: FAILED. 1 passed; 1 failed; 0 ignored; 0 measured; 6 filtered out; finished in 2.11s

error: test failed, to rerun pass `-p steg --test server_start`
```

Check passed: `cargo test -p steg --features fixtures --test server_start` exited 0 at 2026-11-16T00:38:52+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.77 on task-1.77-server-start-on-demand-detach-and-stop at 221ef18c07f7 with uncommitted changes:

```text
   Compiling steg v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.77/crates/steg)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.22s
     Running tests/server_start.rs (target/debug/deps/server_start-6d77ec59f682e657)

running 8 tests
test typing_on_demand_at_a_shell_prompt_carries_on_attached_with_a_message ... ok
test started_is_printed_only_after_the_socket_already_accepts ... ok
test a_fresh_start_replaces_the_socket_of_a_server_killed_outright ... ok
test four_simultaneous_starts_converge_on_one_server ... ok
test a_losing_racers_dropped_child_is_a_zombie_until_reaped ... ok
test explicit_start_stays_up_with_no_client_ever_connecting ... ok
test the_client_being_sigkilled_does_not_end_the_server ... ok
test the_clients_process_group_being_sighupd_does_not_end_the_server ... ok

test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.11s
```

Check FAILED: `sh -c 'for i in $(seq 1 15); do cargo test -p steg --features fixtures --test server_start -- started_is_printed_only_after_the_socket_already_accepts || exit 1; done'` exited 1 at 2026-11-16T00:39:25+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.77 on task-1.77-server-start-on-demand-detach-and-stop at 221ef18c07f7 with uncommitted changes:

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.09s
     Running tests/server_start.rs (target/debug/deps/server_start-6d77ec59f682e657)

running 1 test
test started_is_printed_only_after_the_socket_already_accepts ... FAILED

failures:

---- started_is_printed_only_after_the_socket_already_accepts stdout ----

thread 'started_is_printed_only_after_the_socket_already_accepts' (16073789) panicked at crates/steg/tests/server_start.rs:192:5:
the socket should already accept a connection the instant "started" is printed
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    started_is_printed_only_after_the_socket_already_accepts

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 7 filtered out; finished in 0.08s

error: test failed, to rerun pass `-p steg --test server_start`
```

Check passed: `sh -c 'for i in $(seq 1 10); do cargo test -p steg --features fixtures --test server_start || exit 1; done'` exited 0 at 2026-11-16T00:40:05+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.77 on task-1.77-server-start-on-demand-detach-and-stop at 221ef18c07f7 with uncommitted changes, last 40 of 149 lines:

```text
test started_is_printed_only_after_the_socket_already_accepts ... ok
test typing_on_demand_at_a_shell_prompt_carries_on_attached_with_a_message ... ok
test a_fresh_start_replaces_the_socket_of_a_server_killed_outright ... ok
test four_simultaneous_starts_converge_on_one_server ... ok
test a_losing_racers_dropped_child_is_a_zombie_until_reaped ... ok
test explicit_start_stays_up_with_no_client_ever_connecting ... ok
test the_client_being_sigkilled_does_not_end_the_server ... ok
test the_clients_process_group_being_sighupd_does_not_end_the_server ... ok

test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.10s

    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.09s
     Running tests/server_start.rs (target/debug/deps/server_start-6d77ec59f682e657)

running 8 tests
test started_is_printed_only_after_the_socket_already_accepts ... ok
test typing_on_demand_at_a_shell_prompt_carries_on_attached_with_a_message ... ok
test a_fresh_start_replaces_the_socket_of_a_server_killed_outright ... ok
test four_simultaneous_starts_converge_on_one_server ... ok
test a_losing_racers_dropped_child_is_a_zombie_until_reaped ... ok
test explicit_start_stays_up_with_no_client_ever_connecting ... ok
test the_clients_process_group_being_sighupd_does_not_end_the_server ... ok
test the_client_being_sigkilled_does_not_end_the_server ... ok

test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.12s

    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.09s
     Running tests/server_start.rs (target/debug/deps/server_start-6d77ec59f682e657)

running 8 tests
test typing_on_demand_at_a_shell_prompt_carries_on_attached_with_a_message ... ok
test started_is_printed_only_after_the_socket_already_accepts ... ok
test a_fresh_start_replaces_the_socket_of_a_server_killed_outright ... ok
test four_simultaneous_starts_converge_on_one_server ... ok
test a_losing_racers_dropped_child_is_a_zombie_until_reaped ... ok
test explicit_start_stays_up_with_no_client_ever_connecting ... ok
test the_clients_process_group_being_sighupd_does_not_end_the_server ... ok
test the_client_being_sigkilled_does_not_end_the_server ... ok

test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.11s
```
<!-- SECTION:NOTES:END -->

## Comments

<!-- COMMENTS:BEGIN -->
author: @bruk
created: 2026-11-16T00:23:00Z

---

Plan approved by @bruk (d29c62060a8b, size-3): its worker may build it.

---
<!-- COMMENTS:END -->
