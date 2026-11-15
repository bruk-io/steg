---
id: TASK-1.77
title: 'Server: start on demand, detach, and stop when idle'
status: In Progress
assignee:
  - '@bruk.w3'
created_date: '2026-11-15T19:23:00Z'
updated_date: '2026-11-15T23:39:00Z'
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
- [ ] #1 The first client that needs a server starts one through the sdk's local-socket provider; a server started explicitly stays up with no clients
- [ ] #2 A server started on demand keeps running after the client that started it exits or is killed, and its panes' environment comes from a login shell, not from that client (R99); capturing that environment cannot hang the server on a slow or interactive profile
- [ ] #3 A server started on demand stops after 30 seconds with no sessions and no clients; one with a session, or one started explicitly, does not; a client still in its handshake counts as a client (R99)
- [ ] #4 Two real clients starting a server at the same moment end up connected to the same one (R99), shown with real processes
- [ ] #5 A client's part in starting a server is to run the server's program and wait for the socket; it holds none of the logic above (R98), and nothing is added to the sdk
- [ ] #6 steg server start and steg server start --on-demand run the server, and say on standard output whether a server was started or one was already running; no test-only hook is added to the shipped binary
- [ ] #7 Every child this code spawns is waited on along every path or the notes say why not, including what the client that started the server is left holding
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
## This is three reviewable changes, not one

Seven acceptance criteria spanning process lifecycle (detach + survive), an environment
capture with its own failure/timeout mode, a timing rule with a predicate and a seam
that does not exist yet, and a CLI entry point wiring all of it together, is the same
shape TASK-1.44 was before the engineer split it, and that task still took four review
rounds split in half. Proposing the split along the seams the design below falls into
on its own, for the engineer to decide, the same way TASK-1.44's worker proposed its
own split:

- **B1 - the entry point, detaching, and the two-clients race** (AC #1, #2's survival
  clause, #4, #5, #6, #7). `steg server start [--on-demand]` exists and works; an
  on-demand server survives the client that started it; two real clients converge on
  one; every spawned child is accounted for. No new `ServerConfig` fields, no new nix
  feature beyond what section 6 names, no login-shell code at all.
- **B2 - the idle rule** (AC #3, and the "what does a client see while stopping"
  question doc-8/TASK-1.44 carried here). Depends on B1 for the one function its own
  checker thread is wired into (`ondemand::serve_on_demand`, section 4). Adds the two
  new `ServerConfig` fields.
- **B3 - the login-shell identity** (AC #2's environment clause). No dependency on B1
  or B2 at all - it is one pure-ish function with its own bounded wait and its own
  tests, consumed by nobody today (section 2 says why), so it can be built and reviewed
  in parallel with either.

Everything below is the full design for all three, so the engineer has the whole shape
before deciding; if the split is approved this plan is rewritten per child the way
TASK-1.44's was. **This plan's own digest, once approved, covers all three pieces
together - if the split is approved instead, that approval does not carry over to a
child's own, narrower diff; each child needs its own plan written and approved before
its worker builds, the same two-step TASK-1.44 went through.**

## 1. Detaching (B1): no fork, no fd-passing, one `setsid` call

**The client's spawn already is the separate process; nothing needs to re-exec or hand
anything to a second stage.** `steg_sdk::local_socket::connect` (crates/sdk/src/local_
socket.rs:131-170, unchanged, confirmed by reading it) calls `Command::new(binary).args(
["server","start","--on-demand"])...spawn()` and drops the `Child` without ever
`wait()`-ing on it. That `spawn()` already did a real `fork`+`exec`: the process running
`steg server start --on-demand` is, from the instant it starts, a distinct PID with its
own address space - there is no "stage 1 still attached, hand off to stage 2" to design,
because there is only ever one process for this whole task. The two alternatives the
task's own carried notes and doc-8 R98's history point at (an `unsafe` `pre_exec` hook
passing a bound listener and lock across a *second* exec; a fork-wait-exit "stage 1"
that reports success before a "stage 2" detaches) both solve a problem this design does
not have, because both assume the already-bound socket has to move between two process
images. It does not: this one process calls `arbiter::attempt` itself, binds and locks
itself, and only then detaches.

**What detaching has to do, concretely: leave the session and process group the client's
shell or terminal controls, so a `SIGHUP` delivered to that group (the terminal closing,
the shell's job control cleaning up on exit) does not reach this process.** A plain
`SIGKILL` sent to the *client's own PID* already does not reach this process either way
- Unix signals do not cascade from a killed process to its children, the on-demand
process is reparented to `launchd` the moment its immediate parent (the client) exits,
regardless of any detaching code here. The proof section below tests both: SIGKILL-ing
the client (shows the "no signal cascade" property that is already true for free) and
SIGHUP-ing the client's whole process group (shows what `setsid` actually buys).

`nix::unistd::setsid() -> Result<Pid, Errno>` (confirmed by reading
`nix-0.28.0/src/unistd.rs:343-344`: `pub fn setsid() -> Result<Pid> { Errno::result(unsafe
{ libc::setsid() }).map(Pid) }`) is a **safe** function - the one `unsafe` block is
inside `nix` itself, already audited the same way this task's own `Flock`/`geteuid` use
already relies on. It is gated behind nix's `"process"` feature (confirmed:
`nix-0.28.0/src/unistd.rs:167-168`, `feature! { #![feature = "process"] ...` encloses
`setsid` at line 343), **not** `"fs"`/`"user"`, which `steg-server`'s `Cargo.toml` already
declares - this task adds `"process"` to that features list (section 6). It fails with
`EPERM` only if the calling process is already a process-group leader, which this
process is not (`Command::spawn` does not call `setpgid`/`process_group` anywhere in the
sdk's provider, confirmed by reading it, so the spawned child stays in the client's own
group) - `Err` here is logged and otherwise ignored, not fatal, since the only way to
reach it is a scenario (this process already detached by something else before this
line runs) where the detaching this line asks for is already true.

**`nix::unistd::daemon` is not the alternative, checked rather than assumed**: it is
`cfg`-gated to `linux_android, freebsdlike, solarish, netbsdlike`
(`nix-0.28.0/src/unistd.rs:987-992`) - explicitly **not** `macos`/`apple` targets, so it
does not exist on this platform at all. Naming this so nobody reaches for it from
memory later.

**The one new function, `ondemand::serve_on_demand`, re-exported at the crate root as
`steg_server::serve_on_demand` (`lib.rs` gains `mod ondemand; pub use ondemand::
serve_on_demand;`, the same `mod`/`pub use`-only shape every other module in this crate
already follows, per `.claude/rules/rust.md`'s "`lib.rs`... hold[s] `mod` and `pub use`
lines only")**, is where `setsid`
lives, and where B2 and B3 each add one line later without changing its signature:

```rust
// crates/server/src/ondemand.rs (new)
pub fn serve_on_demand(
    handle: arbiter::ServerHandle,
    socket_path: &Path,
    config: &ServerConfig,
) -> Result<(), Error> {
    if let Err(err) = nix::unistd::setsid() {
        // EPERM: already a process-group leader (not reached in the sdk's own spawn
        // path, named here for the case this is run some other way) - not fatal.
        log_detach_failure(err); // a single eprintln! today; crates/server has no
                                 // logger dependency and this task does not add one
    }
    listen::run(handle, socket_path, config, &Stopper::new(), None) // B2 replaces the
                                                                     // Stopper::new()/None
                                                                     // pair with a real
                                                                     // one and the idle
                                                                     // checker's sender
}
```

**Where printing lives, and why `crates/steg` stays print-only.** doc-16's table and
`crates/steg/CLAUDE.md` ("it holds no server logic of its own") say the CLI package
holds no server logic; doc-8 R98 says detaching is "the server's own code." Printing
"started"/"already running" to stdout (AC #6) is CLI output, doc-2 section 6's own
domain ("stdout is the result and nothing else"), so it belongs in `crates/steg`, not
`crates/server`. The two cannot be satisfied by one function call, because `attempt()`'s
outcome has to be known and printed **before** `run()` blocks for the rest of the
server's life - so `crates/steg`'s entry point calls `steg_server::attempt` itself (adding
nothing new; already `pub` from `lib.rs`), prints, and only then calls either
`steg_server::run` directly (explicit start - no detaching at all, matching R99's own
wording that only assigns detaching to "started on demand") or the new
`steg_server::serve_on_demand` (section 4 has the whole function). `steg_server`
exposes outcomes as plain data (`Arbitration`, already public) and prints nothing itself,
consistent with everything else in this crate today.

**What the client is left holding, proven by test, and the alternative rejected.**
Since there is no fork-then-exit "stage 1," the spawning client's `Command::spawn()`
result is, on every path, a real OS child it never `wait()`s on (sdk's own existing
doc comment on `connect`, unchanged by this task):

- If this process **loses** the race (`AlreadyRunning`/`Refuse`): it prints and exits
  quickly (section 4's exit statuses). The client's dropped `Child` becomes a zombie the
  instant it exits, reaped only when the client itself exits (any remaining children of
  an exited parent reparent to `launchd`, which reaps them). For a short-lived CLI
  invocation this is milliseconds; for a long-lived client (the app) that races and
  loses more than once over its life, this is the sdk's own already-documented,
  accepted cost ("can accumulate a process-table entry per call until it itself exits"),
  which this task's job is to keep *short* (the loser exits almost immediately after
  `attempt()` resolves), not to eliminate (eliminating it would need the client to
  `wait()`, which R99 forbids since that reintroduces exactly the block R99 exists to
  avoid).
- If it **wins**: the client's dropped `Child` is a *live*, long-running process (not a
  zombie) until the server eventually stops (idle timeout, explicit stop, or the test
  killing it) - at that point it becomes a zombie under the same rule, reaped once the
  client exits or the process is explicitly reaped.

**The alternative rejected**: a double-fork, where an intermediate process forks the
real detached server and exits immediately, so the *client* reaps a near-instantly
-exiting intermediate and the real server is orphaned to `launchd` at once, never a
child of the client at all. This removes the "zombie until the client exits" cost
entirely, which is attractive, but needs a `fork()` - and `nix::unistd::fork` is an
**`unsafe fn`** in this version (`nix-0.28.0/src/unistd.rs:287`, documented as unsafe
because forking a multithreaded process is not generally safe - async-signal-safety,
allocator and mutex state). Introducing the one `unsafe` block this task's own carried
note asks to minimise, to remove a cost the sdk's own code already accepts and names,
is the wrong trade here - named explicitly rather than silently chosen.

## 2. The login-shell identity (B3): data, not an ambient side-channel

**What this produces, and what it feeds.** TASK-1.76's (in-review, unmerged) plan
already defines the consumer shape exactly: `runtime.rs`'s `LoginShell { argv0: String,
home: String }`, built today by a named stand-in, `LoginShell::from_process_env()`,
reading `SHELL`/`HOME` from *this process's own environment* - which, for an on-demand
server, is inherited from whatever client spawned it (`Command::spawn` inherits the
parent's environment by default; the sdk's provider never clears it), making that
stand-in read exactly what R99 forbids ("not from the client"). This task's job is to
build the real replacement for that one call site, as data, with no change to anything
downstream of it (TASK-1.76's own plan says the same: "nothing downstream of `LoginShell`
changes" once this lands).

```rust
// crates/server/src/login.rs (new)
pub struct LoginIdentity {
    pub shell: PathBuf,
    pub home: PathBuf,
    pub user: String,
}

pub fn resolve_login_identity(timeout: Duration) -> LoginIdentity
```

**From the password database, confirmed against the real crate.** `nix::unistd::User::
from_uid(nix::unistd::geteuid())` is already **safe** (the `unsafe` is inside `nix`
itself around `getpwuid_r`, `nix-0.28.0/src/unistd.rs:3393-3402`), already behind the
`"user"` feature this crate already declares (`nix-0.28.0/src/unistd.rs:3131-3132`), and
returns `Result<Option<User>, Errno>` where `User` carries `.shell: PathBuf`, `.dir:
PathBuf` (home) and `.name: String` (confirmed by reading the struct,
`nix-0.28.0/src/unistd.rs:3142-3178`) - exactly the three fields `LoginIdentity` needs,
with **zero new `unsafe` code** of this task's own.

**Why a bounded wait at all, if this is "just" a `getpwuid_r` call.** On macOS, `getpwuid_
r` resolves through the directory service (`opendirectoryd`), which can in principle be
slow or stall if a machine's accounts are bound to a network directory - the risk R99's
own "cannot hang the server" language is naming, even though this is not literally
sourcing a shell's `.zprofile` (the earlier, broader reading considered and rejected:
`LoginShell`'s own two fields, and R99's wording, point at "which shell and which home,"
not "run the shell and capture everything its profile exports" - the latter would need a
real child *process* per call, a far bigger scope than TASK-1.76's own stand-in implies
replacing).

```rust
pub fn resolve_login_identity(timeout: Duration) -> LoginIdentity {
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let result = User::from_uid(geteuid());
        let _ = tx.send(result); // dropped silently if the receiver already gave up
    });
    match rx.recv_timeout(timeout) {
        Ok(Ok(Some(user))) => LoginIdentity { shell: user.shell, home: user.dir, user: user.name },
        Ok(Ok(None)) | Ok(Err(_)) | Err(_) => fallback(), // no passwd entry, a real
                                                           // errno, or the timeout
    }
}

fn fallback() -> LoginIdentity {
    // The same literals TASK-1.76's own stand-in already falls back to, so the two
    // agree exactly whenever both are in a fallback state.
    LoginIdentity { shell: "/bin/zsh".into(), home: "/".into(), user: "unknown".into() }
}
```

**The one named exception to "every child/thread is joined," stated the way AC #4's
sleep exception already is in TASK-1.44's own plan, not hidden.** On the timeout path
the worker thread is not joined - joining it would reintroduce the exact unbounded
hang this bound exists to prevent, since the only way to guarantee a join returns is to
wait for the thing that might be hanging. This is not the same kind of leak a child
*process* left unreaped is: a `std::thread::JoinHandle` simply dropped lets the OS
thread keep running to completion and reclaim its own resources when it exits, on its
own, with nothing held open in the meantime (no file descriptor, no process-table
entry) - there is nothing here for AC #7's "every child... waited on... or the notes
say why not" to find wanting, but it is named because the project's own culture
(TASK-1.44's thread-joining rule, applied retroactively to `listen::run`) treats an
un-joined thread as worth a sentence, not silence.

**Why `std::env::set_var` is the wrong mechanism, considered and rejected.** An earlier
draft of this design had the detaching step mutate this process's own environment table
(`SHELL`/`HOME`/`USER`) once at startup, reasoning that `portable_pty::CommandBuilder::
new`'s own `get_base_env()` reads `std::env::vars_os()` at construction time
(`portable-pty-0.9.0/src/cmdbuilder.rs:74-98`), so every future pane's `CommandBuilder`
would pick this up for free with no change needed in TASK-1.76's own, unmerged code.
This is wrong for two reasons, caught before writing it as the real design rather than
after: `std::env::set_var` is an `unsafe fn` (the documented hazard is exactly concurrent
access from another thread), and this task's own `resolve_login_identity` spawns a
worker thread whose timeout path does **not** join it - so the "nothing else is running
yet" argument that would justify the `unsafe` is not actually true by the time the call
would happen. Second, and more basic: TASK-1.76's own carried note already says what
the real seam is - "`Runtime` takes one injected login value" - which is data flowing in
through a constructor argument, not an ambient table every future call site has to
happen to read correctly. **Nothing in this task's own code calls `resolve_login_identity`
except its own tests.** It is exported (`pub use login::{LoginIdentity,
resolve_login_identity}`) and sits unconsumed until TASK-1.76 merges and its own
`Runtime::new` call site is changed from `LoginShell::from_process_env()` to this
function's result, which is a one-line change in a task that has not merged, outside
this task's own scope to make. **Net: zero `unsafe` anywhere in this task.**

## 3. The idle rule (B2)

**A pure predicate, time and the session count both passed in, never read from a clock
or a counter the predicate owns itself** (`.claude/rules/testing-rust.md`: "Time...
come[s] in as parameters... No test sleeps to wait for something").

```rust
// crates/server/src/idle.rs (new)
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
    /// Returns whether the idle timeout has elapsed with no clients and no sessions.
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
how far `now` is moved (proves AC #3's "a client still in its handshake counts as a
client" at the predicate level, not just by construction); a `Disconnected` followed
immediately by a fresh tick before `idle_timeout` has elapsed does not fire; a session
count that is non-zero never fires regardless of clients; a `Connected`/`Disconnected`
pair arriving and leaving within the same tick (the arbiter-probe shape) does not reset
`empty_since` to a *later* instant than the one already recorded (`get_or_insert`'s own
behaviour - checked directly, not assumed, since it is the one line this whole
guarantee rests on).

**The driving loop** (`idle::run_checker`, spawned from `ondemand::serve_on_demand`,
section 4): holds the channel's `Receiver<ConnectionEvent>`, the `Stopper`, a
`session_count: impl Fn() -> usize + Send + 'static` (the seam, below), and
`config.idle_timeout`/`config.idle_poll_interval` (two new `ServerConfig` fields, this
task's own addition, same pattern as `handshake_timeout`/`arbitration_timeout`:
production defaults, every test overrides). Loop: `receiver.recv_timeout(poll_interval)`
- `Ok(event)` feeds `predicate.on_event`; `Err(Timeout)` is the periodic recheck (needed
because `session_count()` can change for reasons that are not a `ConnectionEvent` at
all, once TASK-1.45 wires real sessions); `Err(Disconnected)` means `run()` has already
returned and dropped every sender clone (someone else called `stop()`, or an error
ended the accept loop) - breaks the loop, nothing left to watch. After handling the
`recv_timeout` result, `predicate.tick(Instant::now(), session_count(), idle_timeout)`;
`true` calls `stopper.stop()` and breaks (one-shot: this thread's only job past that
point is to end). `ondemand::serve_on_demand` joins this thread's `JoinHandle` after
`run()` returns - bounded, because the channel closing (which `run()`'s own return
already causes) wakes the next `recv_timeout` promptly, satisfying AC #7 for this
thread specifically.

**The session-count seam, stated as a seam, not solved here.** `session_count` is
`|| 0` in this task - TASK-1.76's `Runtime` is the real count (`tree.sessions().len()`
or equivalent, read under its own lock) once it merges and some later task (routing,
TASK-1.45, or whichever task first gives `ondemand::serve_on_demand` a live `Runtime` to
ask) replaces the closure. Nothing about `idle::run_checker`'s own signature changes
when that happens - the same seam-building TASK-1.44 already did once for
`ConnectionEvent` itself.

**The arbiter's probe, already documented on `ConnectionEvent` by TASK-1.44's own review
(connection.rs:27-42), decays on its own and is not fixed here.** A probe connects and
disconnects within milliseconds, never sending a hello - `predicate.on_event(Connected,
t)` clears `empty_since`, and the matching `Disconnected` a moment later lets the *next*
tick restart the idle clock from very close to `t`. The only cost is a few-second reset
during active arbitration (bounded by `arbitration_timeout`, 5s by default, and only
while two or more processes are actively contending to become the server - not a
steady-state occurrence once one has won), never an indefinite hold, because nothing
about an idle, already-decided server calls `probe()` again.

**"A client that connects every 29 seconds and sends nothing": decided, stated plainly,
not treated as a bug.** By this predicate's own definition (and AC #3's own wording,
"a client still in its handshake counts as a client"), any connection - complete,
incomplete, or never completing at all until TASK-1.44's own `handshake_timeout` force
-closes it - resets `empty_since` to `None` the instant it is accepted. A connection
every 29 seconds is force-closed by the handshake timeout (10s default) well before the
next one arrives, leaving roughly 19 idle seconds between closes - under the 30-second
threshold every time, so this server never idle-stops for as long as that pattern
continues. This is accepted, not fixed: AC #3's own definition of "client" is "accepted,"
not "completed a handshake," so a program that keeps a server alive by reconnecting,
honestly or not, is doing exactly what a real, well-behaved periodic health-check client
would also do, and the predicate cannot distinguish the two without a signal this wire
protocol does not carry (the same limit already named for the arbiter's own probe).

**What a client sees in the instant an idle server is stopping, and the timing
relationship this creates with `arbitration_timeout` - TASK-1.44's notes asked this
task to state.** TASK-1.44's own shutdown order (listen.rs: stop accepting, close the
listener, join open connections, remove the socket file, release the lock *last*) means
a client connecting in that window sees `ConnectionRefused` or `NotFound`, exactly as
if nothing had ever been there (TASK-1.44's own notes already cover this half). What
neither task's notes connect explicitly: the sdk's `connect()` reacts to *that* by
spawning a fresh `steg server start --on-demand`, which calls `arbiter::attempt` and
finds the stale-looking socket file still present (not yet removed) with its lock still
held by the *stopping* server - `decide()` returns `Retry`, looping on `POLL_INTERVAL`
(20ms) until either the stopping server finishes releasing the lock (within
`arbitration_timeout`, 5s default) and this fresh attempt wins it, or the timeout
expires first and it refuses outright. **`arbitration_timeout` must comfortably exceed
how long the stopping server's own join-and-cleanup phase takes.** For the idle-stop
case specifically this is close to instant: the predicate only fires `stop()` when
`live_clients == 0`, so `run()`'s own `connections` list is already empty (or very
nearly so) at the moment `stop()` is called, meaning there is nothing of substance to
join - 5 seconds is comfortably enough, and this is named here as the relationship
rather than left for a future task to discover by a timing-dependent test failure.

## 4. The entry point (B1)

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
    let socket_path = steg_server::resolve_socket_path(&config); // newly `pub`, section 6
    match steg_server::attempt(&config, &socket_path) {
        Ok(steg_server::Arbitration::AlreadyRunning) => {
            println!("a server is already running");
            0
        }
        Ok(steg_server::Arbitration::Refuse(reason)) => {
            eprintln!("steg: could not start a server: {reason}");
            4
        }
        Err(err) => {
            eprintln!("steg: could not start a server: {err}");
            4
        }
        Ok(steg_server::Arbitration::Became(handle)) => {
            println!("started a new server");
            let result = if on_demand {
                steg_server::serve_on_demand(handle, &socket_path, &config)
            } else {
                steg_server::run(handle, &socket_path, &config, &steg_server::Stopper::new(), None)
            };
            match result {
                Ok(()) => 0,
                Err(_) => 1,
            }
        }
    }
}
```

**Exit statuses, taken from doc-2 section 6 rather than invented:** 0 success (started,
or found one already running - both are the caller's happy path); 4 "the server could
not be reached or started" for a refusal or an `Error`; 1 for the rare case `run()`
itself returns an I/O error after actually serving for a while (doc-2's "ran and
failed"). Bad usage (an unrecognised third argument that is not `--on-demand`) is not
handled specially by this task - it falls through to today's placeholder, since parsing
every malformed invocation is TASK-1.49's job once the real CLI exists; named as a
narrow, deliberate gap rather than a silent one.

**Why `steg_server::serve_on_demand` (section 1) rather than reusing `listen::
serve`.** `serve()` (listen.rs, unmodified, still used by TASK-1.44's own tests) returns
`ServeOutcome::Became` only *after* `run()` has already finished - it cannot be used by
`crates/steg` to print "started" before the server's whole life is over. This task does
not change `serve()` at all; it becomes effectively test-only for production purposes
(still fully public, still exercised by TASK-1.44's own existing test suite, so nothing
here marks it unused) - worth a sentence for a reviewer, not a change.

**No test-only hook added to the shipped binary (AC #6).** Nothing in `main.rs` or
`ondemand.rs` branches on an environment variable or a flag that exists only for a test
to control; `STEG_SOCKET_PATH` (already existing, sdk's own) is the one override every
test and every real debugging session shares, unchanged by this task.

## 5. Two real clients converge on one server (B1)

**Two directly-spawned, test-owned `std::process::Command`s, not the sdk's `connect()`,
for this specific test - and why.** The sdk's `connect()` deliberately never returns a
`Child` (section 1, same reason production code does not wait on it), so a test that
goes through it cannot deterministically end the real process it started, and B1 has no
idle-stop yet (that is B2) to lean on for cleanup the way the end-to-end test below does.
Spawning the *same* command the sdk would (`<CARGO_BIN_EXE_steg> server start --on
-demand` with `STEG_SOCKET_PATH` set to a shared temp path) from the test directly, kept
as two `Child` handles, is still two real, separate OS processes racing for the same
socket at the same instant - the thing AC #4 asks to be shown - while giving the test a
deterministic way to end whichever one wins (`child.kill()` then a bounded `child.wait()`,
both expected to return promptly since `SIGKILL` is not catchable).

```
// crates/steg/tests/server_start.rs (new)
fn two_simultaneous_starts_converge_on_one_server() {
    // same temp socket_path for both; a barrier so both `spawn()` calls happen as
    // close together as two threads in one process can manage
    let (mut a, mut b) = (spawn_steg_server_start(&socket_path, true), spawn_steg_server_start(&socket_path, true));
    // bounded poll: the socket accepts a connection within a short deadline
    wait_for_socket(&socket_path, Duration::from_secs(5));
    // exactly one of {a, b} is still running past a short grace; the other has
    // already exited (printed "already running", status 0)
    let (winner, loser) = identify_winner(&mut a, &mut b); // polls try_wait() on both,
                                                            // bounded, never a bare wait()
    // AC #7, "what the client is left holding": the loser, left unwaited the way the
    // sdk's own connect() leaves every losing spawn, is observably a zombie right now
    assert_zombie(loser.id()); // runs `ps -o stat= -p <pid>`, test-only, not a new
                                // production dependency. Asserts the command produced
                                // exactly one line of output (a bare, empty result -
                                // no row at all, which `ps` gives for a pid it cannot
                                // find - fails the test loudly rather than being read
                                // as "not a zombie"; the plan's own worker confirms
                                // this empty-output case cannot be reached here before
                                // the explicit `loser.wait()` below runs, since the
                                // test's own process is the loser's parent and nothing
                                // else can have reaped it yet) and that line contains
                                // 'Z'. Whichever of the two conditions fails, the
                                // failure message names which one.
    loser.wait().unwrap(); // reap it, test hygiene, not something production code does
    winner.kill().unwrap();
    winner.wait().unwrap(); // bounded: SIGKILL is not caught, this returns promptly
}
```

`identify_winner` does not need a server-side signal to tell the two apart: the loser's
`attempt()` resolves to `AlreadyRunning` and the whole process exits almost immediately
(status 0); the winner's `run()` blocks forever (no idle-stop in B1), so `try_wait()`
polled a few times over a short bound distinguishes them cleanly.

**A second, corroborating signal, once `wire::parse_server_hello` is confirmed to expose
it** (checked directly, not assumed: `crates/sdk/src/wire/handshake.rs`'s `ServerHello`
has a public `client: ClientId` field, and `parse_server_hello`'s own test
`parse_server_hello_reads_an_accepted_hello` already asserts a caller can read it) - a
real client-side handshake against each surviving connection (built from the sdk's own
public `ClientHello`/`Frame::hello`/`encode_frame`, nothing added to the sdk) over the
winner's socket, repeated a few times, should mint distinct, server-local `ClientId`
values from the one process's own `AtomicU64` counter (`connection.rs`'s `next_local_
client_id`) - not required for AC #4 on its own (the `AlreadyRunning`/zombie evidence
above already proves it), but cheap to add and it is the kind of corroborating check
this project's own review culture already asks for elsewhere (TASK-1.44's reviews
independently re-verified claims rather than taking them on trust).

**The SIGKILL and SIGHUP proofs (AC #2's survival clause), kept separate because they
prove different things (section 1):**

- `the_client_being_sigkilled_does_not_end_the_server`: spawn the client's own
  equivalent of a CLI invocation (a real, separate process whose only job is to call
  `steg_sdk::local_socket::connect` with `autostart: true` against a fresh socket path,
  built as a small test-only binary the same shape `race-attempt` already is in
  TASK-1.44), `SIGKILL` it the instant it has spawned the server (synchronised on the
  socket appearing), then confirm the socket still accepts a connection afterward. Named
  as proving what is already true "for free" from how Unix process spawning works
  (section 1), not the detaching mechanism itself.
- `the_clients_process_group_being_sighupd_does_not_end_the_server`: the same shape, but
  send `SIGHUP` to the *client's whole process group* (`kill(-pgid, SIGHUP)`) once the
  server has been spawned - this is the test that actually exercises `setsid`. "Prove one
  bites": with `setsid` commented out (a scratch edit, reverted after, recorded on the
  task the way TASK-1.44's own "prove one bites" steps are), this same test is expected
  to show the server receiving the `SIGHUP` and ending - confirming the mechanism this
  section's design rests on, not merely asserting the happy path.

**Width and parallelism**: no pseudo-terminal is opened anywhere in this task (confirmed
by reading `pty.rs`/`pane.rs` are untouched - this task adds no caller of either), so the
`openpty` contention this machine has shown under heavy parallel real-pty tests
(TASK-1.76's own review finding) does not apply here. What does matter at `cargo test`'s
default parallelism is concurrent socket binds and process spawns, and `arbiter::attempt`
already has eight-wide proof of exactly that, run ten times consecutively with no flake
(TASK-1.44's own recorded checks) - this task's own two-and-four-wide tests ride the
same already-validated mechanism, just through real spawned `steg` binaries instead of
the in-process `race-attempt` fixture, so the process-spawn overhead (heavier than an
in-process call) is the only new cost, not a new source of flakiness.

## 6. Dependencies, unsafe, and what's left out

**One Cargo.toml change**: `steg-server`'s `nix` feature list gains `"process"` (section
1's `setsid`), alongside the existing `"fs"`, `"user"` - no new external crate.
`boundaries.toml`'s `[steg-server]` entry's `reason` and feature list are updated to name
it, the same way the existing entry already names `fs`/`user`; `just check-boundaries`
is what proves the declared features match what is actually used, confirmed in the
build phase rather than assumed here (the same caveat TASK-1.44's own plan stated for
`fs`/`user`).

**Unsafe: none, anywhere in this task.** `setsid` (section 1) and `User::from_uid`
(section 2) are both safe wrappers already audited inside `nix`; `std::env::set_var` was
considered for section 2 and rejected (section 2's own reasoning); no `fork`, no
`pre_exec`, no `FromRawFd`.

**Left out, naming what inherits each:**

- **Routing a real request past the hello**, and the real session count the idle
  predicate's seam needs - TASK-1.45/TASK-1.76, unchanged from TASK-1.44's own "left
  out" list.
- **`steg_sdk::run_fake_server_for_tests`, left where it is, not moved.** TASK-1.70's own
  carried note asked this task to say whether to move it out of the sdk's public
  interface. Not moved: the sdk's own tests (`local_socket.rs`'s AC #3 coverage) need a
  dependency-free stand-in server, and doc-16's dependency table (`[steg-sdk]`'s `allow`
  list) does not let the sdk depend on `steg-server` or `steg` - there is no real binary
  for the sdk's *own* test suite to spawn instead, so this one `#[doc(hidden)]` function
  stays exactly as TASK-1.70 built it.
- **An explicit start's own distinct "already running" message** - doc-2 does not
  distinguish one from on-demand's, and this task does not invent one; both print the
  same two messages (section 4).
- **`server stop` and anything that lets a client ask a running server to end** - no
  dispatch exists yet (TASK-1.45) for a request to reach `Stopper::stop`; today the only
  callers of `stop()` are this task's own idle checker (B2) and tests.
- **Bad-usage parsing for `server start`'s own arguments** beyond the two forms matched
  (section 4) - TASK-1.49's generated CLI.

## Proof, by acceptance criterion

- **AC #1** (first client autostarts through the sdk's provider; explicit stays up with
  no clients): the sdk's own existing `connect()` already does the client half
  (TASK-1.70, unchanged); this task's new entry point (section 4) is what makes the
  spawned process actually become a server, proved by `server_start.rs`'s
  `on_demand_start_through_the_real_sdk_connects_and_handshakes` (one real client via
  the sdk, autostart, a real handshake over the result) and
  `explicit_start_stays_up_with_no_client_ever_connecting` (B1: spawns `steg server
  start` with no flag, waits for the socket to accept, then waits a short bound with
  **no client ever connecting** and asserts the process is still alive and the socket
  still accepts - a real, biting assertion, not an appeal to construction, since B1's
  own `run()` call for the explicit path passes `events: None` and a fresh `Stopper`
  nobody holds, so nothing in this path *could* stop it; the test proves that remains
  true rather than asserting it by reading the code. Kills and reaps the process as
  teardown. B2 adds its own explicit-vs-on-demand idle test once the checker exists,
  proving the same thing again under B2's own `ServerConfig` fields).
- **AC #2** (survives the client; login-shell environment; bounded capture): the
  SIGKILL and SIGHUP tests (section 5) for the first clause; `login.rs`'s own tests
  (section 2) for the second and third - a real call against this test process's own,
  real passwd entry (asserting the result matches `whoami`/the test's own known
  `$HOME`, a ground truth this environment can produce for real, unlike the cross-uid
  gaps TASK-1.44 already named) plus an injected-delay version (a fake "lookup" that
  sleeps past a short injected timeout, proving the bound fires and the fallback is used
  - this is the one place a sleep stands in for a real slow NSS backend, named as such
  since there is no way to make a real `getpwuid_r` call slow on demand in this
  environment).
- **AC #3** (idle stop; sessions/explicit exempt; mid-handshake counts): `idle.rs`'s own
  predicate unit tests (section 3) prove the logic with literal `Instant`s; a functional
  test in `crates/server/tests/idle.rs` drives a real `run()` with a short injected
  `idle_timeout`/`idle_poll_interval` and real connections (one that completes a
  handshake and leaves, one that is left mid-handshake and never completes it) and
  asserts the server stops only once both are gone and the injected grace has passed,
  and does not stop while the mid-handshake one is still open; **one** slow, named
  end-to-end test (`crates/steg/tests/idle_end_to_end.rs`) uses the sdk's real
  `connect()` against the real binary with the *production* 30-second default, drops
  its stream, and polls (bounded, ~45s) for the socket to stop accepting - the one test
  in this whole task that proves the shipped default end to end rather than an injected
  stand-in for it, and the test that gives B2's own cleanup problem (section 1 avoided
  needing this for B1) a legitimate, already-necessary mechanism instead of a bespoke
  one.
- **AC #4** (two real clients, one server): `server_start.rs`'s
  `two_simultaneous_starts_converge_on_one_server` (section 5), plus the corroborating
  `ClientId` check if `parse_server_hello`'s exposure is confirmed as expected.
- **AC #5** (client's part stays thin; nothing added to the sdk): proved by construction
  - `crates/sdk` is untouched by this task (`git diff --stat` against `origin/main`
  shows nothing under `crates/sdk/`, checked in the build phase); `run_fake_server_for_
  tests` is addressed above, not moved, not extended.
- **AC #6** (`server start`/`--on-demand` print their outcome; no test hook): the exit
  -status/stdout assertions already folded into `server_start.rs`'s tests above (each
  asserts the captured stdout line, via `std::process::Command::output()` on the real
  binary, matches the expected message, and the exit code matches section 4's table).
- **AC #7** (every spawned child/thread accounted for, including what the client holds):
  the zombie/reap test in section 5; `login.rs`'s named, deliberate non-join on timeout
  (section 2); the idle-checker thread's own join after `run()` returns (section 3);
  nothing else is spawned by this task's own code.

**No sleeps as synchronisation, with the one named exception already called out in
section "AC #2"** (the injected-delay fallback test) - every other wait is on a real
event: a socket accepting, a channel receiving, `try_wait()`/`wait()` on a real process,
or a bounded poll on an externally observable state change (the socket refusing once a
server has stopped).
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Carried from TASK-1.70 (2026-11-15): the sdk exports run_fake_server_for_tests as a public, doc-hidden function behind the local-socket feature. TASK-1.44's plan found it cannot simply be removed (the sdk may not depend on the real binary), but the fixture could bind through the public socket_path itself; say in this task's plan whether to move it out of the public interface here.

Carried from TASK-1.44's review (2026-11-15), for this task's plan: (1) the arbiter's liveness probe connects to a running server and is reported on the ConnectionEvent seam as a client arriving and leaving, so the idle rule must not let probes hold an idle server up, or must tell them apart. (2) Stop the server only through the one stop function TASK-1.44 leaves public after its review fix (the first version had a stop() that set the flag without waking accept, which would have hung an idle server: exactly this task's case). (3) On shutdown there is a window while connection threads are joined in which the listener is still bound; see TASK-1.44's notes for what a connecting client sees.
<!-- SECTION:NOTES:END -->
