---
id: TASK-1.77
title: 'Server: start on demand and detach, from the steg entry point'
status: In Progress
assignee:
  - '@bruk.w3'
created_date: '2026-11-15T19:23:00Z'
updated_date: '2026-11-16T00:00:00Z'
labels:
  - size-3
milestone: m-3
dependencies:
  - TASK-1.44
parent_task_id: TASK-1
project: bruk-io-steg
asdlc_accepted_from: DRAFT-4
asdlc_branch: task-1.77-server-start-on-demand-detach-and-stop
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
Rewritten for this piece alone (B1 of the split), against `origin/main` with TASK-1.76
merged. The idle rule (TASK-1.86) and the login environment (TASK-1.87) are separate
drafts; nothing below plans either, and section 8 says plainly what this piece leaves
them.

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
this task adds `"process"` to that list (section 9). `nix::unistd::daemon` is not the
alternative, checked rather than assumed: it is `cfg`-gated to `linux_android,
freebsdlike, solarish, netbsdlike` (`nix-0.28.0/src/unistd.rs:987-992`) - explicitly
**not** `macos`/`apple` targets, so it does not exist on this platform at all.
`nix::unistd::fork` is an **`unsafe fn`** in this version (`nix-0.28.0/src/unistd.
rs:287`) - the reason a double-fork design (an intermediate exits at once so the client
reaps a short-lived child instead of holding a zombie, section 7) is considered and set
aside: it would add the one `unsafe` block AC #6 now forbids outright, to remove a cost
the sdk's own code already accepts and names.

## 2. An explicit `server start` runs in the foreground and never detaches (AC #1)

R3 and R99 give detaching only to an on-demand start; AC #1 now says this in so many
words ("runs in the foreground of the terminal or supervisor that started it"). Two
small functions, not one with a flag, since the two differ in more than a branch
(detaching, which `LoginShell` feeds `Runtime::new`, section 7):

```rust
// crates/server/src/start.rs (new)
pub fn serve_explicit(
    handle: ServerHandle,
    socket_path: &Path,
    config: &ServerConfig,
) -> Result<(), Error> {
    let mut runtime = Runtime::new(Arc::new(RealLauncher), LoginShell::from_process_env());
    // `listen::run` requires a `&Stopper` - one must exist regardless. Bound, not a
    // throwaway temporary, so the point reads as deliberate: nobody else holds this
    // one *today*, so nothing in this task makes this server stop before `run()`
    // ends on its own (no signal handler either, see below) - `server stop`
    // (TASK-1.45) is what will reach it once a request can.
    let stopper = Stopper::new();
    let result = listen::run(handle, socket_path, config, &stopper, None);
    // Every exit path out of `run` - a clean stop or an `Err` from the accept loop -
    // still stops `Runtime`: captured, not `?`-propagated early, the same "cleanup
    // runs on every path, including the error one" shape `listen::run`'s own "no
    // early return inside this loop" comment already uses.
    runtime.stop();
    result
}
```

**What a person at a terminal sees, and how it is stopped.** `run_server_start` (section
5) prints "started a new server" (or the matching line for one already running, or a
refusal) and then this function blocks - the shell prompt does not return - until
`listen::run` itself ends. **No signal handler is added, considered and rejected.** A
graceful `SIGINT`/`SIGTERM` handler (Ctrl-C calling `stopper.stop()` then `runtime.
stop()` in order, rather than the OS's own default disposition for either signal, which
is immediate termination with no Rust code run at all) would need either an `unsafe`
`sigaction` registration (`nix::sys::signal`'s own handler-installing calls are `unsafe`,
since a signal handler's body is constrained to async-signal-safe operations the
compiler cannot check) or a new dependency (`signal-hook`, which does the same thing
behind a safe API, at the cost of a crate none of this task's six criteria asks for).
AC #6 forbids the first outright; nothing in doc-2 or this task's own criteria asks for
the second, and R99's own "stops only when told to" reads more naturally as a request
over the socket (`server stop`, TASK-1.45's and a later task's to build) than a signal a
person's terminal happens to deliver. **So today: `kill -TERM`/`kill -INT`/Ctrl-C all end
an explicit server by the OS's own default action - immediate termination, no exit
status in the normal sense at all (a process ended by an unhandled signal has no `exit()`
code; a shell reports this its own way, e.g. "terminated" or `128+signal`, which is the
shell's convention, not this process's).** This is safe, not merely tolerated: the
arbiter's lock releases on process death by any means, including this one (TASK-1.44's
own proven property, `attempt_takes_over_a_stale_socket_left_by_a_dead_server`), so a
raw kill never corrupts the arbiter's own state for the next `attempt()`.

**An interrupt during start-up leaves no socket file or lock behind - what this claim
actually rests on, since nothing here catches the signal either.** With no handler, the
process can be killed at any instruction boundary, including mid-`attempt()`. `bind_
with_mode` (TASK-1.44, unchanged) only ever creates the *final* socket path by `rename`
onto it after binding under a temporary name and setting its mode - a process killed
before that rename completes leaves, at worst, an orphaned temporary file
(`.steg-tmp-<pid>-<n>`) beside the final path, never a file *at* the final path a later
`attempt()` would have to trust or refuse (pre-existing TASK-1.44 behaviour, not changed
here - named rather than re-proven). The lock itself is never left stale by an
interrupt: released on process death regardless of when, by the kernel, the same
property named above.

## 3. `setsid` fails when the caller is already a process-group leader - both cases, with a test for each

**Why the sdk's own spawn path is never this case.** `std::process::Command::spawn`
performs a plain `fork`+`exec` with no job-control `setpgid` call of its own (confirmed
by reading `local_socket.rs`: no `.process_group(...)` is set) - the spawned process
inherits the *client's* process group, which is not its own pid, so it is not a group
leader and `setsid` succeeds.

**Why typing `steg server start --on-demand` at an interactive shell prompt is exactly
this case.** A job-control shell (`zsh`, `bash`) calls `setpgid(pid, pid)` on *every*
command it launches, foreground or background, making each one the leader of its own,
brand-new process group - this is how job control itself works, not specific to `&`.
A person running the on-demand form directly therefore already *is* a process-group
leader by the time this code's own `setsid()` call runs, and that call returns `EPERM`.

**Decided: carry on attached, with a stated message - not a refusal.** This is an
unusual, debugging-shaped way to invoke `--on-demand` (every real client goes through
the sdk's spawn path, never types this directly), so refusing outright would make the
flag unusable for a curious or debugging person, and R99's guarantee (survives the
client, takes its environment from a login shell) still matters only in the measure
this attached mode can give it - attached, the server is exactly as foreground-bound as
an explicit start, which is an honest, lesser guarantee stated plainly rather than
silently dropped:

```rust
pub fn serve_on_demand(
    handle: ServerHandle,
    socket_path: &Path,
    config: &ServerConfig,
) -> Result<(), Error> {
    if let Err(err) = nix::unistd::setsid() {
        eprintln!(
            "steg: could not detach from this terminal ({err}); continuing attached - \
             closing this terminal will end this server"
        );
    }
    let mut runtime = Runtime::new(Arc::new(RealLauncher), LoginShell::from_process_env());
    let stopper = Stopper::new();
    let result = listen::run(handle, socket_path, config, &stopper, None);
    runtime.stop();
    result
}
```

**A real test for each case, both safe, zero `unsafe`.** The non-leader case is section
7's own SIGHUP test (setsid succeeds silently, no message). The leader case needs a
process that is *itself* already a group leader before it becomes the server - built as
a small fixture binary (the same shape as TASK-1.44's own `race-attempt`,
`tests/fixtures/already_a_group_leader.rs`) whose entire body is:

```rust
fn main() {
    nix::unistd::setsid().expect("the fixture must become its own leader to set up the case");
    let err = std::process::Command::new(env!("CARGO_BIN_EXE_steg"))
        .args(["server", "start", "--on-demand"])
        .exec(); // std::os::unix::process::CommandExt::exec - safe: replaces this
                 // process's image in place via `execve`, no fork, nothing left
                 // half-forked for the compiler's unsafety rules to worry about
    panic!("exec never returns on success: {err}");
}
```

`exec()` keeps the same pid, the same (now session-and-group-leader) process group, and
the same open file descriptors - so the real `steg` binary that replaces this process's
image is running as the exact precondition being tested, with no `unsafe` anywhere in
the fixture or in `start.rs`. `CommandExt::exec()` returns an `io::Error` **only on
failure** - on success the calling process's image is already gone, so the `panic!`
after it is unreachable on the success path, not a missing branch. The test spawns this fixture with piped stdout, asserts
the "could not detach... continuing attached" line appears, and that the server still
starts and answers a connection (not refused) - the chosen behaviour is "carry on," not
"refuse," and this is what proves it rather than only documenting it.

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
  stdin at all** - confirmed by grep across `listen.rs`/`connection.rs`/`pane.rs`/
  `runtime.rs` for `print`/`eprint`/`stdin`: none, outside this task's own one-line
  startup message in `crates/steg`. So even for the group-leader-attached case above,
  where these fds really are the person's live terminal, there is nothing left to write
  to them once that one message is printed - a terminal closing afterward has no later
  write to fail against. Not closed or redirected by this task, because nothing needs
  it: doing so would cost a design decision (closing stdin on an attached, interactive
  invocation is a surprise) for no correctness gained.
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
    let config = steg_server::ServerConfig::default();
    let socket_path = steg_server::resolve_socket_path(&config); // newly `pub`, section 9
    match steg_server::attempt(&config, &socket_path) {
        Ok(steg_server::Arbitration::AlreadyRunning) => { println!("a server is already running"); 0 }
        Ok(steg_server::Arbitration::Refuse(reason)) => {
            eprintln!("steg: could not start a server: {reason}"); // untrusted file/
            4                                                      // directory, or
        }                                                          // arbitration timed out
        Err(err) => { eprintln!("steg: could not start a server: {err}"); 4 }
        Ok(steg_server::Arbitration::Became(handle)) => {
            println!("started a new server");
            let result = if on_demand {
                steg_server::serve_on_demand(handle, &socket_path, &config)
            } else {
                steg_server::serve_explicit(handle, &socket_path, &config)
            };
            match result { Ok(()) => 0, Err(_) => 1 }
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
| Started (became the server) and `run`/`serve_on_demand` later returns cleanly | 0 |
| Already running | 0 |
| Already running, specifically a loser of a simultaneous-start race (section 6's own test asserts this exact case) - same status, named separately since it is the one this piece's own race test checks | 0 |
| Refused - an untrusted existing file or an unsafe directory (`Arbitration::Refuse`) | 4 |
| Refused - arbitration timed out (`Arbitration::Refuse(ArbitrationTimedOut)`) | 4 |
| A genuine I/O error from `attempt` itself | 4 |
| Started, then `run`/`serve_on_demand` returns an `Err` (an accept-loop I/O failure) | 1 |
| Ended by an unhandled signal (section 2) | none - not an `exit()` status at all |

**No test-only hook (AC #5 restated under its new number).** Nothing in `main.rs` or
`start.rs` branches on anything that exists only for a test to control; `STEG_SOCKET_
PATH` (already existing) is the one override every test and a real debugging session
share, unchanged by this task.

## 6. Two (and several) real clients converge on one server (AC #3)

Unchanged in substance from the shared design, widened to four racers since "several"
is now the word AC #3 uses:

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
    wait_for_socket(&socket_path, Duration::from_secs(5)); // bounded poll, a real event
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
  this is the test that actually exercises `setsid`.

## 8. What this piece leaves, by name, stated plainly (AC #4's own scope, and the two drafts)

- **The idle rule (TASK-1.86).** An on-demand server built by this piece **never stops
  by itself** - there is no idle checker here at all, by construction (confirmed by
  `start.rs`'s own code: nothing holds a `Stopper` past this function's own scope, and
  nothing calls `.stop()` from anywhere but this function's own return path). A test
  that needs to end one it started must do one of two things: call `Stopper::stop()`
  directly, for an in-process test that holds its own clone across the thread boundary
  (`listen::run`'s own existing tests already do this); or send a real signal to the
  real process, for an out-of-process test (section 6, 7's own zombie/race tests - a
  `kill()` on the held `Child`).
- **The login environment (TASK-1.87).** `serve_on_demand` builds its `Runtime` from
  `LoginShell::from_process_env()` - this process's own environment, inherited from the
  client that spawned it. **A pane started through this piece's own `Runtime` still
  gets the starting client's environment, not a login shell's** - stated plainly, not
  silently carried, since R99's own text ("not from the client") does not hold yet for
  this one field. TASK-1.87 replaces this one call site; nothing else in `runtime.rs`
  or `start.rs` changes when it does.
- **Routing a real request to `Runtime::handle` at all (TASK-1.45, in planning now).**
  Section 9 names exactly what it changes.

## 9. A real `Runtime`, where it is created, and the seam TASK-1.45 fills

**Today, nothing in production constructs a `Runtime` at all** - every call to `Runtime
::new` on `origin/main` is inside `runtime.rs`'s or `tests/runtime.rs`'s own test code
(confirmed: `git grep Runtime::new origin/main -- crates/` finds no call site outside
`#[cfg(test)]`/`tests/`). This piece is the first: both `serve_explicit` and `serve_on_
demand` (sections 2, 3) construct one, hold it as a plain, un-shared value for the
server's whole life, and call `runtime.stop()` on every exit path out of `listen::run`,
including its `Err` path - matching that function's own "cleanup runs on every path"
convention rather than `?`-propagating early. `Runtime::stop`'s own behaviour (`SIGHUP`
by pid, bounded, no escalation - TASK-1.78's own line to draw) is unchanged; this piece
is only the first thing that calls it for real. `runtime.rs` itself is **not edited at
all** by this piece - no new method, nothing moves.

**The seam TASK-1.45 fills, named rather than built here.** `Runtime::handle` is still
unreachable from a real connection: `listen::run`'s per-connection thread
(`connection::handle`) only ever decodes the hello today (TASK-1.44's own, unchanged
scope). For a real request to reach `Runtime::handle`, TASK-1.45 needs a way to get this
function's own `runtime` value to that thread - which means changing `start.rs`'s local
`let mut runtime = Runtime::new(...)` to `let runtime = Arc::new(Runtime::new(...))` and
threading a clone into whatever `listen::run`/`connection::handle` gains to carry it
(a new parameter, or a small context struct - TASK-1.45's own design call). **`Arc::
get_mut` is already safely available for the `.stop()` call once that happens**:
`listen::run` already joins every spawned connection thread on every path before
returning (TASK-1.44's thread-joining rule), so by the time `run()` returns and this
function calls `.stop()`, every other `Arc<Runtime>` clone a connection thread held has
already been dropped - this function's own clone is the last one standing, which is
exactly `Arc::get_mut`'s own precondition. Named now so TASK-1.45 does not have to
rediscover it.

## 10. Testing discipline this piece's own multi-process tests follow throughout

**Every process a test starts is ended and waited on even when the test fails.** A bare
`child.kill(); child.wait();` at the end of a test function is skipped by a panicking
assertion partway through, leaking the process. Every multi-process test in this piece
wraps each spawned `Child` in:

```rust
struct KillOnDrop(std::process::Child);
impl Drop for KillOnDrop {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait(); // bounded: a killed process's wait() returns promptly
    }
}
```

so a failing assertion still cleans up via the unwind, the same guarantee `?` and an
early return already get from normal control flow, extended to a panic.

**Repeated runs, at this project's actual default test parallelism, with counts.**
Every test in sections 3, 6 and 7 that touches a real process, a real socket or real
signal delivery is run at least 10 times consecutively via `cargo test`'s own default
thread count (not `--test-threads=1`) before being cited as proof, counts recorded with
`record-check` - the same discipline TASK-1.44/1.76's own reviews required, run at full
concurrency specifically because this machine has shown contention under heavy parallel
testing before (TASK-1.76's own `openpty` finding) - though that finding does not apply
here directly: no pseudo-terminal is opened anywhere in this piece (`Runtime::new`'s own
`Arc::new(RealLauncher)` only names the type; nothing calls `RealLauncher::spawn`, since
nothing routes a request that would start a pane, section 8). What this piece's own
width actually stresses is concurrent socket binds and process spawns, and `arbiter::
attempt` already has eight-wide proof of exactly that (TASK-1.44's own race.rs, ten
consecutive runs, no flake) - this piece's four-wide real-process test (section 6) rides
the same already-validated mechanism, with real process-spawn overhead on top rather
than a new source of flakiness.

**A negative control for each mechanism this proof rests on:**

- **`setsid` removed** (a scratch edit to `start.rs`, reverted after, recorded with
  `record-check` the way TASK-1.44's own "prove one bites" steps are): the SIGHUP test
  (section 7) fails - the server dies with the client's process group. The SIGKILL test
  does **not** fail with `setsid` removed, and this is said honestly rather than treated
  as a gap: that property never depended on detaching at all (section 7's own framing),
  so a negative control that "proves" it by failing would be proving the wrong thing;
  its absence of failure here is the correct, stated outcome, not a missing test.
- **The entry point's print moved before `attempt()` resolves** (a scratch edit):
  section 5's own race-the-print test fails intermittently (not every run, since the
  bind is fast) - recorded across several runs to show the intermittency itself, not
  just one failing run, since a single pass would understate what the test is actually
  proving against.

## 11. Dependencies and unsafe

**One Cargo.toml change**: `steg-server`'s `nix` feature list gains `"process"`
(section 1's `setsid`), alongside the existing `"fs"`, `"user"` - no new external crate.
`boundaries.toml`'s `[steg-server]` entry's `reason` and feature list are updated to
name it; `just check-boundaries` proves the declared features match what is actually
used, confirmed in the build phase rather than assumed here.

**Unsafe: none, anywhere in this piece.** `setsid` (section 1) is a safe wrapper already
audited inside `nix`; the group-leader fixture's own `Command::exec()` (section 3) is
also safe (replaces the process image via a direct `execve`, no fork, nothing left
half-forked); `std::env::set_current_dir` (section 4) is a plain safe function, unlike
`set_var`/`remove_var`; no `fork`, no `pre_exec`, no `FromRawFd`, no signal handler.

## Proof, by acceptance criterion

- **AC #1** (autostart through the sdk; explicit stays up with no clients, in the
  foreground): the sdk's own `connect()` already does the client half (TASK-1.70,
  unchanged); section 5's entry point is what makes the spawned process actually become
  a server. `explicit_start_stays_up_with_no_client_ever_connecting`: spawns `steg
  server start` with no flag, waits for the socket to accept, waits a short bound with
  **no client ever connecting**, asserts the process is still alive and the socket still
  accepts - a real, biting assertion (nothing in `serve_explicit` *could* stop it, and
  this proves that remains true rather than reading the code for it). Section 2's own
  "no signal handler" reasoning and its exit-status table cover "runs in the foreground."
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
  unsafe): section 7's zombie test; section 9's `runtime.stop()` on every exit path,
  including `run()`'s own `Err`, proved by forcing that path (a scratch edit to `listen
  ::run` returning an `Err` immediately, reverted after) and confirming `Runtime::stop`'s
  own, already-proven behaviour (TASK-1.76's `stop_tests`) still runs; section 11's own
  accounting for zero `unsafe`.

**No sleeps as synchronisation, anywhere in this piece.** Every wait is on a real event:
a socket accepting, `try_wait()`/`wait()` on a real process, a pipe actually producing
the expected line, or a bounded poll on an externally observable state change. Section
10's negative controls are the one place a scratch edit stands in for "what if this
mechanism were missing," reverted immediately after being recorded.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Carried from TASK-1.70 (2026-11-15): the sdk exports run_fake_server_for_tests as a public, doc-hidden function behind the local-socket feature. TASK-1.44's plan found it cannot simply be removed (the sdk may not depend on the real binary), but the fixture could bind through the public socket_path itself; say in this task's plan whether to move it out of the public interface here.

Carried from TASK-1.44's review (2026-11-15), for this task's plan: (1) the arbiter's liveness probe connects to a running server and is reported on the ConnectionEvent seam as a client arriving and leaving, so the idle rule must not let probes hold an idle server up, or must tell them apart. (2) Stop the server only through the one stop function TASK-1.44 leaves public after its review fix (the first version had a stop() that set the flag without waking accept, which would have hung an idle server: exactly this task's case). (3) On shutdown there is a window while connection threads are joined in which the listener is still bound; see TASK-1.44's notes for what a connecting client sees.

Split on 2026-11-15 under the engineer's standing decision to split by default, into the three pieces the worker's plan proposed. This task is now the first: the entry point, starting on demand, detaching, and two clients converging. The idle stop and the server's login-shell environment are drafts under TASK-1. The plan on this task was written for all three and is to be rewritten for this piece only.
<!-- SECTION:NOTES:END -->
