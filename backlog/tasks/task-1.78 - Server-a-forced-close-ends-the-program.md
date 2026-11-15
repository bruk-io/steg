---
id: TASK-1.78
title: 'Server: a forced close ends the program'
status: In Progress
assignee:
  - '@bruk.w4'
created_date: '2026-11-15T19:42:00Z'
updated_date: '2026-11-15T23:57:00Z'
labels:
  - size-3
milestone: m-3
dependencies:
  - TASK-1.76
parent_task_id: TASK-1
project: bruk-io-steg
asdlc_accepted_from: DRAFT-5
asdlc_branch: task-1.78-server-a-forced-close-ends-the-program
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Part two of the split of TASK-1.76 (2026-11-15). doc-2 section 9 (--force on the close verbs), doc-8 R97, doc-16. TASK-1.40's reviews established that shutting a pane down does not end a running program, and that a program that prints nothing leaves its relay thread blocked in read with the child alive, because the reader is a separate descriptor and closing the server's end did not hang the program up on this platform. The worker's plan on TASK-1.76, section 5, is the starting design: remove the handles under the lock, drop the lock, hang up, escalate to a kill after a grace bound, suppress the normal exit event for a forced run so the forcing caller alone removes the pane, and reply through the pane's worker. Read it and TASK-1.40's notes before planning.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 pane close, tab close and session close with --force end every running program in scope, observe each exit, and then close; for a program that is producing output and for one that prints nothing (the case TASK-1.40 left: a silent program kept the relay blocked in read and the child alive)
- [ ] #2 A program that ignores a polite signal is ended after a bounded wait; the bound and the signals used are stated, and the mechanism is established from portable-pty's own source and a real-process experiment recorded as the first step of the build
- [ ] #3 After a forced close no child is left unreaped and no relay or worker thread is left running, on every path including a program that dies by itself while being forced; shown by tests with real programs and bounded waits
- [ ] #4 A forced close of one pane does not stall requests for other panes for the length of the wait, and a pane being forced cannot be reached by another request meanwhile
- [ ] #5 No unsafe code is added without it being named in the plan with its reason and the alternative rejected
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
## Scope

One reviewable change. It touches `crates/server/src/{pane,pty,runtime}.rs` and
`crates/server/src/dispatch/{mod,pane,tab,session,lookup}.rs`; no kernel or sdk
file changes, no new crate dependency (one new *feature* on an existing one, §8).
It also retires `Runtime::stop`'s own unescalated mechanism in favour of the one
this task builds, and fixes one adjacent gap §5 found while reading the code
(handle below) - both named as departures from literal AC wording, for the
engineer to confirm or cut.

## 0. Experiments (first step of the build, AC #2)

Ran in a scratch directory, not committed (`subprocess.Popen(..., start_new_session=True)`,
which calls the same `setsid()` portable-pty's own `pre_exec` does - confirmed by
reading `portable-pty-0.9.0/src/unix.rs:150-270`: `setsid()` runs before `exec`,
so the child's pgid equals its own pid at that moment, independent of this
crate's own pty code). Four real-process cases, plus one inferred limitation:

1. **Silent** (`exec sleep 1000`): a real `SIGHUP` sent to the pid alone killed it
   (signal 1, confirmed via `poll()`, not just "no longer in `ps`" - an unreaped
   zombie still answers `kill -0`, which an earlier naive version of this probe
   got wrong). Matches TASK-1.76's own committed `stop_ends_a_silent_program`.
2. **Producing output continuously** (`exec yes >/dev/null`): same - `SIGHUP`
   alone kills it.
3. **Traps the polite signal** (`trap '' HUP; exec sleep 1000`): survives a full
   second after `SIGHUP` (still running); a second signal, `SIGKILL`, ends it at
   once. This is the empirical basis for AC #2's escalation requirement -
   `SIGHUP` alone is **not** sufficient in general, confirmed directly rather than
   assumed from TASK-1.76's own scratch note (which this reproduces and extends:
   that review ran the same shape and reported `stop()` giving up after ~10s with
   the process still alive, because the existing mechanism never escalates at
   all).
4. **A grandchild that outlives the signal, two ways**: `sh -c 'sleep 1000 & exec
   sleep 2000'`, no job control (plain non-interactive `sh`). Sending `SIGHUP` to
   the *main* pid alone kills the exec'd `sleep 2000` but leaves the backgrounded
   `sleep 1000` running (different process, same pgid, untouched because nothing
   signalled its pgid). Sending `SIGHUP` to the **process group** (`killpg`,
   pgid = the main pid, per setsid) kills both. This is my own primary evidence
   for signalling the group, not the pid, at every stage (polite and escalated).

**Inferred, not tested, and named as a real limitation (§11)**: an *interactive*
login shell with job control on (the default for an interactive `zsh`/`bash`,
which is what a shell pane actually runs) calls `setpgid` on each backgrounded
job, moving it to a *new* process group distinct from the one `killpg` reaches.
A scripted attempt to measure this directly (`zsh -i`/`bash -i` driven over a
plain pipe, no pty) hung - job control itself needs a controlling terminal to
behave the way an interactive shell expects, which a bare pipe does not give it,
and killing the stuck process confirmed nothing was left running afterward. I
did not re-attempt it through a real pty (out of this step's own time budget,
and the underlying fact - that job control reassigns a job's pgid, which is why
`fg`/`bg`/`jobs` work at all - is bedrock POSIX behaviour, not something this
task discovered). Stated as a named gap, not hidden: reaching a job-control
shell's own backgrounded jobs would need either the pty's controlling-terminal
state or a full descendant-tree walk (platform process enumeration, e.g. macOS's
`libproc`), both out of scope for this task's size and AC's (none of the five
name job-control's own process-group reassignment).

`portable-pty`'s own escalation, re-confirmed and **correcting both prior
tasks' own citation**: TASK-1.40/1.76's notes say "confirmed... from
`unix.rs`" - the actual implementation is in `lib.rs:293-338`, not `unix.rs`
(verified by `grep -rn "ProcessSignaller\|clone_killer\|impl ChildKiller"` over
the crate's whole `src/`; `unix.rs`'s only `SIGHUP` hit is inside
`spawn_command`'s `pre_exec`, resetting the *child's own* signal disposition
before `exec`, nothing to do with the killer). `Child::clone_killer()` on Unix
returns a `ProcessSignaller` whose `kill()` sends `SIGHUP` **by pid, to that one
process, with no escalation and no group** (`lib.rs:323-338`); `std::process::
Child`'s *own* `ChildKiller::kill()` (never reachable here - we only ever hold
the *clone*) does escalate, by pid, after a 250ms grace (`lib.rs:340-373`) - a
different, unused code path, confirmed so no one repeats the PR #40 reviewer's
"no escalation, by design" line without the "and we can't reach the other impl
either" half.

## 1. The kill mechanism (`crates/server/src/pane.rs`, `pty.rs`)

The existing `killer: Box<dyn portable_pty::ChildKiller>` field only ever gives
a pid-only, unescalated `SIGHUP` (§0) - useless for AC #2's own escalation and
for reaching a grandchild (§0.4). `ChildKiller` is opaque (no way to read a pid
back out of it), so the group-aware, escalating signal is built directly, with
`nix`, using the pid already available at spawn time (`spawned.child.
process_id()`, used today only by this crate's own tests).

**The signal itself bypasses the worker's command queue entirely - deliberately,
not just as an optimisation.** `Hangup`/`Escalate` were first drafted as two
more `PaneCommand` variants (fire-and-forget, processed by `worker_loop` like
every other command) - rejected: that queue is the *same* one the run's own
relay thread pushes `PtyBytes` into, and a program flooding output (`yes`, one
of this task's own named test cases) can keep the worker busy draining bytes
indefinitely, so a signal command queued behind that backlog would not be
*sent* when `force_end` calls it, only whenever the worker happens to drain to
it - silently widening the grace window exactly for the flooding case this task
means to prove ends promptly. Fixed by not needing the worker thread at all for
the signal itself: `PaneHandle` and `worker_loop` share one `Arc<AtomicU64>`
(`0` = no program currently running), written by the worker whenever a run
starts (`start`, `restart`/`Resume`) or ends (`Exited`), read directly by
`PaneHandle::signal_hangup`/`signal_kill` on the *calling* thread - no channel,
no queue, no delay regardless of backlog. (An existing, unchanged risk class,
not a new one: a signal sent to a pid between it exiting and the OS reaping it
could in principle reach a reused pid if the kernel recycled it in that window;
the existing `killer.kill()`-by-pid mechanism already carried this, and nothing
in this task makes the window wider.)

`Running` (today: `writer`, `resize`, `killer`, `relay_handle`) drops `killer`
entirely (the pid it would have signalled now lives in the shared atomic, not
inside worker-thread-local state) - `writer`, `resize`, `relay_handle` only.
`pane::start` creates `pid_cell: Arc<AtomicU64>` from `spawned.child.
process_id()` (the exact call this crate's own tests already use) *before*
`child` moves into `spawn_relay`, stores a clone on the returned `PaneHandle`,
and moves the original into `worker_loop`, which updates it on `Resume` (a new
run) and `Exited` (cleared to `0`, so a stale signal after this point can only
ever reach nothing, never a program this pane no longer owns).

New, in `pty.rs` or a new small `crates/server/src/signal.rs` (kept out of
`pane.rs` so `pane.rs` stays about orchestration, not the signal call itself):

```rust
fn send(pid: u32, signal: nix::sys::signal::Signal) {
    // killpg's pgrp is the pid itself: portable-pty's own `pre_exec`
    // (unix.rs) calls setsid() before exec, which makes the child's pgid
    // equal its own pid at that moment (§0) - reaching a pipeline or a
    // non-job-control background job (§0.4), not only the one process a
    // plain `kill` would. ESRCH (already dead) is not an error here: the
    // caller always confirms the real outcome by waiting for `Exited`,
    // never by this call's own result.
    let _ = nix::sys::signal::killpg(nix::unistd::Pid::from_raw(pid as i32), signal);
}
```

Safe code throughout (`nix::sys::signal::killpg`'s own `unsafe { libc::killpg
(...) }` is inside `nix`, confirmed by reading `nix-0.28.0/src/sys/signal.rs:
1064-1083`; nothing here is `unsafe`) - AC #5.

`PaneCommand` drops `Stop` (§4 explains why) and gains exactly one variant -
everything else moved off the queue, above:

```rust
/// Replies once the current run has actually ended (the same `stopping`
/// mechanism `Stop` used - replies immediately, **and ends the worker
/// thread**, if nothing is running right now). The "replies at once"
/// branch must still break the loop, not only the branch that waits for
/// `Exited` - see the note on `force_end`'s own natural-death row (§2):
/// without this, a run that ends on its own *before* `AwaitExit` is even
/// read (plausible - `Exited` and `AwaitExit` both arrive on the same
/// queue, in whichever order the relay thread and the caller happen to
/// race) leaves `current == None` by the time `AwaitExit` is read, and
/// the old `Stop`-derived code only broke on the *other* branch - a
/// leaked worker thread that `join_worker`'s own bounded join would
/// silently time out on, not surface.
AwaitExit(Sender<()>),
```

`PaneHandle` gains, replacing `stop`:
```rust
/// Signals the run's own process group (§0.4) with SIGHUP, if one is
/// running right now - reads the shared `pid` atomic directly, no
/// channel, so a backlog of `PtyBytes` the worker hasn't drained yet
/// (a flooding program) cannot delay this (see the note above).
pub(crate) fn signal_hangup(&self) { if let Some(pid) = self.live_pid() { signal::send(pid, Signal::SIGHUP); } }
/// Same, SIGKILL.
pub(crate) fn signal_kill(&self) { if let Some(pid) = self.live_pid() { signal::send(pid, Signal::SIGKILL); } }
fn live_pid(&self) -> Option<u32> { let p = self.pid.load(Ordering::SeqCst); (p != 0).then(|| p as u32) }
/// `true` if the run ended within `timeout`; a send failure (worker
/// already gone) counts as "ended" too, not a timeout.
pub(crate) fn await_exit(&self, timeout: Duration) -> bool { ... }
/// Consumes the handle: joins its worker thread (bounded). Only
/// meaningful once `await_exit` has returned `true` for this handle -
/// calling it before that can time out on a worker still legitimately
/// serving other commands, which is not a bug in `join_worker` itself.
pub(crate) fn join_worker(self, timeout: Duration) -> bool { join_with_bound(self.worker, timeout) }
```

`STOP_TIMEOUT` is deleted with `Stop`/`stop` (§4). Verified on this repository's
own pinned toolchain (`rust-toolchain.toml`, rustc 1.98.1), not assumed from
memory: `PaneHandle`'s new shape (`Sender<PaneCommand>`, `Arc<dyn
ProgramLauncher>`, `JoinHandle<()>`, `Arc<AtomicU64>`) is `Sync` - `mpsc::
Sender<T: Send>` is `Sync` on this standard library (it was not on older ones;
checked directly with a throwaway `rustc` compile, not inferred), which is what
lets §2's `thread::scope` capture `&PaneHandle` across its spawned threads.

## 2. The shared bounded primitive (`crates/server/src/runtime.rs`)

One function, used by both `Runtime::stop` (§4, scope = every pane) and the
force-close dispatch wrapper (§5, scope = one pane, a tab's panes, or a
session's tabs' panes) - advisor's own point: "signal every pane in scope
first, then collect every exit against one grace window", so N panes cost one
grace window, not N:

```rust
const ESCALATE_CONFIRM_TIMEOUT: Duration = Duration::from_secs(2); // SIGKILL
    // cannot be trapped, so this bounds only OS scheduling/reap latency, not
    // a real wait for cooperation.
const FORCE_JOIN_TIMEOUT: Duration = Duration::from_secs(2); // bounded join,
    // same reasoning as the existing RELAY_JOIN_TIMEOUT: by the time
    // `await_exit` returns true, the worker thread has already broken its
    // own loop (§1's fix) and is on its way out.

/// Ends every program in `handles`, reaps each, joins every worker thread -
/// bounded by `grace` (shared, not per-pane) plus two small, fixed margins,
/// regardless of how many panes are in `handles`. Never panics, never blocks
/// past its own stated bound.
fn force_end(handles: Vec<(PaneId, PaneHandle)>, grace: Duration) {
    // Pass 1: signal every pane at once. `signal_hangup` reads the shared
    // atomic directly (§1) - no channel, so this loop costs microseconds
    // regardless of how backed up any one pane's own worker is.
    for (_, handle) in &handles { handle.signal_hangup(); }

    // Pass 2: wait against one shared deadline, not N deadlines - only
    // `await_exit` needs the worker thread (to learn the real `Exited`),
    // so only this pass and pass 4 need `thread::scope` (std, no new
    // dependency; `PaneHandle: Sync` confirmed, §1).
    let deadline = Instant::now() + grace;
    let stragglers: Vec<PaneId> = thread::scope(|scope| {
        let jobs: Vec<_> = handles.iter().map(|(pane, handle)| {
            scope.spawn(move || {
                let remaining = deadline.saturating_duration_since(Instant::now());
                (*pane, handle.await_exit(remaining))
            })
        }).collect();
        jobs.into_iter()
            .map(|j| j.join().expect("await_exit is channel ops with bounded timeouts only, never panics"))
            .filter(|(_, ended)| !ended)
            .map(|(pane, _)| pane)
            .collect()
    });

    // Pass 3: escalate only the stragglers - same direct-atomic reasoning
    // as pass 1.
    for (pane, handle) in &handles {
        if stragglers.contains(pane) { handle.signal_kill(); }
    }

    // Pass 4: confirm the escalated ones actually died (SIGKILL cannot be
    // trapped, so this bound is a safety margin, not a real wait), and
    // give every other pane's own worker the final `AwaitExit` it needs to
    // break its loop at all (§1) - every handle, not only the stragglers:
    // a pane that ended during pass 2 already got its one `AwaitExit` and
    // its worker already broke, so calling it again here is unnecessary
    // for those - this pass only re-visits the stragglers.
    thread::scope(|scope| {
        for pane in &stragglers {
            if let Some((_, handle)) = handles.iter().find(|(p, _)| p == pane) {
                scope.spawn(|| { handle.await_exit(ESCALATE_CONFIRM_TIMEOUT); });
            }
        }
    });

    // Pass 5: join every worker thread, bounded - sequential, not scoped:
    // `join_worker` consumes `self` (by value), and by this point every
    // handle's own `AwaitExit` has already been answered (pass 2, or pass
    // 4 for a straggler), so each join is expected to return almost at
    // once, not block on another pane's own wait.
    for (_, handle) in handles {
        let _ = handle.join_worker(FORCE_JOIN_TIMEOUT);
    }
}
```

Total bound for *any* scope, any pane count: `grace + ESCALATE_CONFIRM_TIMEOUT +
FORCE_JOIN_TIMEOUT` (default grace, §3: 2s - so ~6s worst case), never
`N * anything` - this is what answers brief item 4 and item 6's N-pane
question, and AC #4's first clause for the *within* one close call case (across
different calls, §5's lock discipline is what answers it).

## 3. Making the grace period injectable (AC #2's own wording)

`Runtime` gains one new constructor field, the same pattern `LoginShell`
already uses (a named, injected value, not read from the environment or
hardcoded): `force_grace: Duration`. `Runtime::new(launcher, login, force_grace)`
- the one real call site (TASK-1.44/45's socket, not built yet) will pass a
stated production default, `FORCE_GRACE_DEFAULT: Duration::from_secs(2)` (public
const in `runtime.rs`) - two seconds: long enough that a program doing real
cleanup on `SIGHUP` (flushing a file, a graceful shutdown handler) has a
realistic chance, short enough that a person who asked to force-close is not
left waiting much longer than they would be for an ordinary close. Every one of
this crate's own tests (8 existing `Runtime::new` call sites, all in
`runtime.rs`/`tests/runtime.rs`) passes a tiny value (e.g. 30ms) through a new
`fn login_and_grace()`-style test helper, so the suite's own total time does not
grow by the production grace on every test that exercises forcing - **except
one**, named here rather than left to contradict this paragraph silently: §9's
AC #4 stall test needs a grace wide enough that "the other session's request
returned before the window closed" is a meaningful assertion, not scheduling
noise against a 30ms window that would pass whether or not the lock is held -
that one test alone constructs its own `Runtime` with a multi-second grace and
a trapping program, specifically so the window is wide enough to mean
something, while the assertion itself still only waits a small, bounded
fraction of it.

## 4. `Runtime::stop` reuses `force_end` (brief item 5)

Today: `for (_, handle) in panes { handle.stop() }` - serial, pid-only, no
escalation (so a trapping program survives `Runtime::stop`/`Drop` too, confirmed
by TASK-1.76's own un-committed scratch test, §0's third case being exactly
that scenario). Rewritten: `Runtime::stop` pops `state.panes` (unchanged) and
calls `force_end(panes, self.force_grace)` once, outside the lock - same
mechanism, same escalation, now reaching a trapping program and a
non-job-control grandchild (§0.4) the old mechanism could not.

**New total bound, stated plainly as a behaviour change**: was serial, up to
`N * STOP_TIMEOUT` (5s each); becomes `force_grace + ESCALATE_CONFIRM_TIMEOUT +
FORCE_JOIN_TIMEOUT` (~6s with the default grace) for *any* number of panes -
faster for more than one pane, and now actually ends a trapping program rather
than giving up on it. `stop_tests` (4 tests) are rewritten against the new
mechanism, not left pointed at deleted code; one new test,
`stop_ends_a_program_that_traps_the_polite_signal` (`trap '' HUP; exec sleep
1000`, same shape as §0's own probe), is added - the one real behaviour change
this rewrite buys, proven rather than only argued.

## 5. The force-close dispatch wrapper (AC #1, #3, #4)

**Shared scope lookup, `dispatch/lookup.rs`** (new, used by both the existing
refusal checks and the new wrapper, replacing three copies of "which panes does
this close touch" with one - `pane::close` has one pane, `tab::close` a tab's
panes, `session::close` a session's tabs' panes, today duplicated verbatim in
each of the three files):
```rust
pub(crate) fn panes_in_close_scope(tree: &Tree, context: &Context, request: &Request)
    -> Result<Vec<PaneId>, ApiError>
```
`pane::close`/`tab::close`/`session::close`'s own `any_running` checks are
rewritten to call this and keep their existing refusal behaviour exactly
(`ErrorCode::ProgramRunning`, nothing touched) - a small, low-risk
deduplication, not a behaviour change, named so a reviewer does not read it as
scope creep.

**`Runtime::handle`** gains a fourth intercepted group, alongside the three
starting methods:
```rust
"pane.close" | "tab.close" | "session.close" => self.close_and_dispatch(description, context, &resolved),
```
(always intercepted, force or not - see below for why force=false costs
nothing extra.)

`Runtime::close_and_dispatch`:
1. Lock. Compute `scope = dispatch::panes_in_close_scope(&state.tree, context,
   request)` - propagate its error unchanged (`PaneNotFound`/`TabNotFound`/
   `SessionNotFound`, exactly what the unintercepted path would have returned).
2. **If any pane in `scope` is already in `state.forcing`**: refuse at once,
   touching nothing - `super::program_running()`, reworded (§7) - this is AC #4's
   second clause, scoped explicitly (§6) to a second force-close racing the
   first, not to every other method.
3. `force = param_bool(request, "force").unwrap_or(false)`. If `!force` **or**
   no pane in `scope` has both `program_status() == Running` and a live entry in
   `state.panes`: nothing to force. Fall straight through to step 6 with an
   empty `to_force` - this is the ordinary refusal-or-plain-close path,
   unchanged in behaviour from today (the refusal check inside `dispatch`'s own
   arms still runs and still wins when `!force` and something is running).
4. Otherwise: pop every `(pane, handle)` in `scope` that has one from
   `state.panes` into `to_force`, and insert each of those pane ids into
   `state.forcing`. Unlock.
5. `force_end(to_force, self.force_grace)` (§2) - lock-free, so other panes'
   requests are never stalled by this (AC #4's first clause).
6. Lock again. Remove every pane just forced from `state.forcing`. Call the
   existing, unchanged `dispatch::dispatch(&mut state.tree, &mut state.settings,
   description, context, request)` - its own close arm's refusal check is a
   no-op when `force` is true (as today) and performs the real
   `tree.close_pane`/`close_tab`/`close_session` (R94/R95's cascade, untouched -
   zero kernel changes anywhere in this task). **New, and the fix for §5's own
   found gap**: for every pane id in the original `scope` that `state.tree`
   no longer resolves after this call, remove its entry from `state.panes` too,
   if present - not only the ones just forced. This also closes an adjacent gap
   this task's own reading found, not named by any of the five criteria: a
   *plain*, non-force close of a pane whose program had already ended on its
   own left that pane's idle `PaneHandle` (and its now-pointless worker thread)
   in `state.panes` forever, because nothing before this task ever removed a
   handle on the ordinary close path (only the three *starting* methods and the
   drain thread's own cascade ever touched `state.panes`). Proposed here because
   it is a one-line consequence of code this task is already adding and
   directly serves AC #3's own "no thread left running" in spirit, even though
   it is not the literal subject of any of the five criteria - **for the
   engineer**: cut if it should be its own task instead.
7. On success, apply the existing general resize step (`reapply_every_sized_tab`),
   exactly as the `_` arm already does. Return the response.

**Who removes the pane (brief item 2), stated precisely**: `apply_event_locked`
(the drain thread, unchanged trigger - every run's own natural `Exited` still
reaches the shared `events` channel regardless of whether it is being forced,
since `Hangup`/`Escalate` are fire-and-forget, not a different exit path) gains
one new first check in its `PaneEvent::Exited` arm: `if state.forcing.contains
(&pane) { return; }` - this is the "suppress the normal exit event for a forced
run" step the task's own description already names. While a pane is in
`forcing`, its handle is not in `state.panes` (popped at step 4) and the drain
thread does nothing with the event at all; the *forcing caller itself* (step 6,
under the same lock, right after removing `pane` from `forcing`) is the only
code that ever calls `tree.close_pane`/`close_tab`/`close_session` for it. The
two paths cannot both act: the drain thread's own check and the forcing
caller's lock-held step 6 are mutually exclusive by construction (one reads
`forcing` and bails, the other removes from `forcing` under the same lock
acquisition it then uses to close) - no interleaving exists where both call
`tree.close_*` for the same pane, which is what the existing `Err
(PaneNotFound) => state.panes.remove(&pane)` arm in `apply_event_locked` was
guarding against for the *old*, now-closed, `--force`-orphan gap. That arm is
now provably unreachable by any path this task's own design permits (every
force-close this task builds drains through `forcing`, properly) - kept,
commented as such, rather than deleted or turned into an `unreachable!()`: a
wrong "impossible" assumption panicking a live drain thread is worse than one
dead, documented, defensive branch.

## 6. What AC #4's "cannot be reached" covers, stated rather than left silent

**In scope**: a second force-close request naming a pane (or a tab/session
containing one) already in `state.forcing` - refused immediately (§5 step 2),
the one genuinely dangerous race (two callers both trying to be "the forcing
caller" for the same pane, §5's own single-remover invariant). Also: no request
for *any other* pane stalls for the length of this one's grace window (§5 step
5 is lock-free).

**Explicitly not guarded, named rather than silently left**: while a pane is
being forced, it is still present in the tree (status `Running`) until step 6,
so `pane.show`, `pane.swap`, `pane.move`, `pane.set`/`unset`, and a plain
(non-force) `pane.close` on it all still resolve and run exactly as if nothing
were happening - none of them touch `state.panes` or the launcher, so none can
race the actual ending of the program or double-signal it; the only visible
effect is a structural change (a swap, a relabel) landing on a pane that is
about to disappear anyway, or a non-force close correctly refusing (status
still `Running`). Guarding these too would mean threading `Runtime`'s private
`forcing` set through `dispatch/lookup.rs`'s shared target-resolution helpers,
used by every method in the module - breaking the deliberate `dispatch`/
`Runtime` boundary TASK-1.76 documented and built (`dispatch` never sees
`Runtime`'s own state) for a race with no correctness consequence, not a
proportionate change for a size-3 task. **For the engineer**: confirm this
scoping, or ask for the broader guard.

## 7. Error code (AC #4, #5)

No new `ErrorCode`. `program_running()`'s existing `ErrorCode::ProgramRunning`
is reused for §5 step 2's refusal; its doc comment (`crates/sdk/src/
description/shapes.rs`) is reworded from "A program is still running and
`--force` was not given" to "A program is still running: either `--force` was
not given, or an earlier `--force` on it is still ending it" - still one error
code, still already listed on all three methods' `errors` (no sdk/description
surface change beyond the doc string), following the exact precedent TASK-1.40/
1.76 already set for reusing an existing bucket rather than growing the set.

## 8. Dependency (AC #5)

`nix` is already allowed for `steg-server` (`fs`, `user`, for TASK-1.44's
arbiter lock and owner check). This task adds the `signal` feature (which pulls
`process` automatically per `nix-0.28.0/Cargo.toml.orig`'s own `signal =
["process"]`) for `nix::sys::signal::{kill, killpg, Signal}` - confirmed gated
behind exactly that feature by reading `sys/signal.rs`'s own `#[cfg(feature =
"signal")]` lines, not assumed from the Cargo.toml feature list alone.
`nix::unistd::Pid` needs no feature (`unistd` is an unconditional module).
**No new `allow` entry** in `boundaries.toml` (`nix` is already listed for
`steg-server`); its `reason` string is extended with this new use. No `deny-
features` entry blocks this (today's table only restricts `steg-kernel`'s use
of `steg-sdk`'s `local-socket` feature - checked, not assumed).

## Files that change

- `crates/server/src/pane.rs` - `Running.pid` replaces `.killer`; `PaneCommand::
  {Hangup,Escalate,AwaitExit}` replace `Stop`; `PaneHandle::{signal_hangup,
  signal_kill,await_exit,join_worker}` replace `stop`; `STOP_TIMEOUT` deleted;
  module doc and `spawn_relay`'s own long comment corrected (the gap they
  describe is this task, landing now, not a future one).
- `crates/server/src/signal.rs` - new: the one `killpg` call, `nix`-based.
- `crates/server/src/runtime.rs` - `force_end` (new, shared); `Runtime::stop`
  rewritten to call it; `force_grace: Duration` field and constructor
  parameter, `FORCE_GRACE_DEFAULT`; `close_and_dispatch` (new); `Runtime::
  handle`'s match gains the three close methods; `State.forcing: HashSet
  <PaneId>` (new); `apply_event_locked`'s new first check.
- `crates/server/src/dispatch/lookup.rs` - `panes_in_close_scope` (new, shared).
- `crates/server/src/dispatch/{pane,tab,session}.rs` - `close()`'s own
  `any_running` check calls the new shared helper; behaviour unchanged.
- `crates/server/src/dispatch/mod.rs` - `program_running()`'s call sites
  unchanged; nothing else moves.
- `crates/sdk/src/description/shapes.rs` - `ErrorCode::ProgramRunning`'s doc
  comment reworded (§7); no variant added, no `errors` list changed.
- `crates/server/Cargo.toml`, `boundaries.toml` - `nix`'s `features` gains
  `"signal"`; `boundaries.toml`'s `[steg-server]` reason string extended.
- `crates/server/tests/runtime.rs` - new tests (§9); every existing test that
  constructs a `Runtime` passes the new `force_grace` argument.

## Order of work

1. Bring in `origin/main` first (the claim's worktree already has TASK-1.76
   merged; re-check at build time for anything newer).
2. §0's experiments, scratch, not committed - `asdlc record-check` records the
   committed real-process tests that follow from them instead.
3. `pty.rs`/`signal.rs`'s `killpg` wrapper, alone, with its own unit test
   (`FakeLauncher`-level: that the right pid and signal reach a recorded call -
   no real process needed for this one check).
4. `pane.rs`: `Running.pid`, the three new `PaneCommand` variants, the new
   `PaneHandle` methods, deleting `Stop`/`stop`/`STOP_TIMEOUT` - with real-
   process tests reproducing §0's four cases directly against `PaneHandle`
   (not yet through `Runtime`).
5. `runtime.rs`: `force_end`, `force_grace`, `FORCE_GRACE_DEFAULT` - unit-level
   (`FakeLauncher` where the signal path itself is already proven in step 4;
   a real-process test or two for the shared-deadline claim specifically, §9).
6. `Runtime::stop` rewritten to call `force_end`; `stop_tests` rewritten plus
   the one new trapping-program test (§4).
7. `dispatch/lookup.rs`'s `panes_in_close_scope`; the three `close()` arms
   rewritten to call it, with the existing refusal tests re-run unchanged to
   prove no behaviour drift.
8. `runtime.rs`'s `close_and_dispatch`, `State.forcing`, `apply_event_locked`'s
   new check, `Runtime::handle`'s match - the full AC #1/#3/#4 proof (§9).
9. `nix`'s `"signal"` feature, `boundaries.toml`'s reason string - `just deny`/
   `just check-boundaries` recorded clean.
10. `just check` at each numbered step's own boundary, not only at the end.

## Proof (AC by AC, every mechanism with a negative control)

Every real-process test uses a bounded wait (an event, or a polled condition
with a stated bound, per this crate's own established pattern - never a bare
`sleep`/`recv`) and asserts no child/thread survives it, via the same `kill -0`
probe this crate's tests already use. Every repeated-run claim below is run at
this crate's **default** test parallelism (`cargo test -p steg-server`/`just
test`), not `--test-threads=1` - naming the exact thing the second round of
TASK-1.76's own review found wrong with recording the narrower command instead.

**AC #1** (every running program in scope ends, exit observed, then closes;
producing output and silent): `pane.close --force` on one command pane running
`yes` (continuous output) and one running `sleep 1000` (silent), each through
`Runtime::handle`, asserting the process is gone (`kill -0`), the pane is gone
from the tree, and the response is `Ok`. `tab.close --force`/`session.close
--force` over several such panes at once (brief item 4). Negative control:
disable `close_and_dispatch`'s own pop-and-force step (route straight to the
old `_`-arm shape) - the silent-program case fails (process still alive after
the call returns), proving the test actually exercises the new mechanism.

**AC #2** (bound and signals stated, mechanism from portable-pty's source and a
real experiment, recorded first): §0's four experiments, written up above and
cited by file/line; a committed test reproducing the trapping case through the
real `PaneHandle`/`Runtime` path (not the scratch harness) -
`pane.close --force` on `trap '' HUP; exec sleep 1000`, asserting it survives
past `force_grace` alone (killed only once `signal_kill` fires) and is dead by
the stated total bound. Negative control: with escalation disabled (`force_end`
without pass 3/4), this test fails - the trapping program is still alive when
the function returns.

**AC #3** (no child unreaped, no thread left running, every path, including a
program dying by itself mid-force): the table below, one real-process test per
row, each asserting via `kill -0` and a joined-or-not check on the relevant
thread handles (via a test-only accessor or by the absence of any further
observable effect, matching this crate's existing style):

| Path | What ends it | Test |
|---|---|---|
| Natural exit (command pane) | the program itself; drain thread's `apply_event_locked` | existing, unchanged |
| Natural exit (shell pane), R97 cascade | same, `ExitOutcome::Closed` | existing, unchanged |
| `--force`, program producing output | `force_end` pass 1 (SIGHUP alone) | new |
| `--force`, silent program | `force_end` pass 1 | new |
| `--force`, program traps the polite signal | `force_end` passes 3-4 (SIGKILL) | new |
| `--force`, program already ended before the call | `panes_in_close_scope` finds nothing to force; plain `dispatch` close | new |
| `--force`, program dies naturally *during* the grace window | `force_end`'s own `await_exit` observes the natural exit (no escalation needed); `apply_event_locked`'s `forcing` check no-ops the race | new - the brief's own named race, directly. Asserts not only that `force_end` returns and the process is gone, but that the worker thread itself actually ends (via a test-only accessor, or by confirming a second command sent to the same `PaneHandle`'s `commands` sender fails) - §1's own fix (the "replies at once" `AwaitExit` branch must also `break`) is specifically for this row: without it, the test would see the right process-level outcome while leaking the worker thread, which a test that only checks `kill -0` on the *process* would miss entirely |
| `--force` during pane start-up | start-and-dispatch's own existing rollback (unaffected by this task - no running program exists yet to force) | existing, unchanged, re-asserted |
| `Runtime::stop`/`Drop` | `force_end`, scope = every pane | rewritten (§4) |

**AC #4** (no stall, no reach meanwhile): one test, using a deliberately long
`force_grace` for this test alone (§3) rather than the suite's usual tiny
value - a 30ms window makes "returned before the window closed" scheduling
noise, not evidence. Starts two sessions, forces one pane with a trapping
program (so the window is both real and guaranteed not to end early) while
concurrently issuing `pane.show` against the *other* session's pane from a
second thread, asserting the second call returns well inside that long window
(a small, stated fraction of it, not "eventually") - proves no stall. A second
test: two threads both issue
`pane.close --force` on the *same* pane; asserts exactly one succeeds in ending
the program and the other is refused with `ProgramRunning` at once (not after
waiting), via `state.forcing`. Negative control: remove the `state.forcing`
check (§5 step 2) - the second test either double-signals or races on which
caller removes the pane (flaky/wrong), proving the check is load-bearing.

**AC #5**: `nix::sys::signal::{kill,killpg}` are safe functions, confirmed by
reading `sys/signal.rs` directly (§0, §8) - no `unsafe` keyword appears
anywhere in this task's diff (checked by `git diff` grep, same as TASK-1.76's
own reviewer did). No new crate; `nix`'s existing `allow` entry gains one
feature, named with its reason (§8).

**Repeated-run honesty**: `cargo test -p steg-server` (default parallelism)
recorded via `asdlc record-check` 10+ times at the final head with 0 failures,
counts stated on the task; `just check` recorded clean at the final head (not a
stale one, the exact thing TASK-1.76's second review was dinged for).

## Risks / for the engineer

- §5 step 6's idle-handle fix (a plain close of an already-ended pane leaking
  its handle) is a pre-existing TASK-1.76 gap, not named by any of this task's
  five criteria. Proposed here because it is nearly free given the
  interception this task already builds; cut to its own task if preferred.
- §6's scoping of "cannot be reached" (force-vs-force only, not force-vs-
  everything-else) is a real, reasoned choice, not a default - confirm or
  widen it.
- §0's job-control limitation (an interactive shell's own backgrounded job,
  reassigned to a new process group) is named, not fixed - no AC asks for it,
  and fixing it needs either controlling-terminal state or a full descendant
  walk, both out of this task's scope.
- `Runtime::stop`'s rewrite (§4) changes already-reviewed, merged behaviour
  (TASK-1.76 PR #40) - named explicitly rather than folded in silently, per
  that task's own two "unnamed departure" findings.
<!-- SECTION:PLAN:END -->
