---
id: TASK-1.78
title: 'Server: a forced close ends the program'
status: In Progress
assignee:
  - '@bruk.w4'
created_date: '2026-11-15T19:42:00Z'
updated_date: '2026-11-16T03:07:00Z'
labels:
  - size-3
  - changes-requested
milestone: m-3
dependencies:
  - TASK-1.76
references:
  - task-1.78-server-a-forced-close-ends-the-program
  - https://github.com/bruk-io/steg/pull/47
parent_task_id: TASK-1
project: bruk-io-steg
asdlc_accepted_from: DRAFT-5
asdlc_branch: task-1.78-server-a-forced-close-ends-the-program
asdlc_plan_approved_by: bruk
asdlc_plan_approved_hash: 'd681d6fe44b5'
asdlc_plan_approved_size: size-3
asdlc_review_kind: same-machine
asdlc_review_findings: 'important=1,nit=2'
asdlc_review_policies: ["review", "testing"]
asdlc_plan_matched: no
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Part two of the split of TASK-1.76 (2026-11-15). doc-2 section 9 (--force on the close verbs), doc-8 R97, doc-16. TASK-1.40's reviews established that shutting a pane down does not end a running program, and that a program that prints nothing leaves its relay thread blocked in read with the child alive, because the reader is a separate descriptor and closing the server's end did not hang the program up on this platform. The worker's plan on TASK-1.76, section 5, is the starting design: remove the handles under the lock, drop the lock, hang up, escalate to a kill after a grace bound, suppress the normal exit event for a forced run so the forcing caller alone removes the pane, and reply through the pane's worker. Read it and TASK-1.40's notes before planning.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 pane close, tab close and session close with --force close the pane, tab or session at once, then end every running program in scope and observe each exit, answering the caller only when that is done; for a program that is producing output and for one that prints nothing (the case TASK-1.40 left: a silent program kept the relay blocked in read and the child alive)
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
file changes beyond one doc comment, no new crate dependency (one new *feature*
on an existing one, §8). It also retires `Runtime::stop`'s own unescalated
mechanism in favour of the one this task builds, and fixes one adjacent gap §5
found while reading the code (below) - both named as departures from literal
AC wording, for the engineer to confirm or cut.

**For the engineer, stated at the top rather than buried: AC #1's own stated
order changes.** AC #1 reads "end every running program in scope, observe each
exit, and then close." §5 below closes the pane, tab or session **first**
(under one lock acquisition, the instant the force begins) and ends the
program **after**, replying to the caller only once that is done - the
reverse of the criterion's own words. This is not an oversight; §5 explains
why the order the criterion states cannot also give AC #4's full guarantee
(every other method unreachable, not only a second close), and §6 is the
check of what that reordering costs before committing to it. If the engineer
wants the literal order instead, §6's own "what this would cost" section says
what AC #4 gives up to get it.

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

**Inferred, not tested, and named as a real limitation (Risks, below)**: an *interactive*
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

## 5. The close-first dispatch wrapper (AC #1, #3, #4)

**Close the pane, tab or session in the tree first, under one lock
acquisition, before ending any program - not after.** This is a design change
from this plan's own first draft (which forced the program first and closed
the tree last, behind a new `forcing` set guarding only a second close on the
same pane), made after the engineer's own review of that draft found it did
not actually give AC #4's second clause: a *plain* close arriving during the
grace period would see an already-ended program and succeed, racing the
forcing caller's own later close of the same, by-then-already-gone pane; a
`pane.move` could relocate the pane to another tab while it was being forced.
Closing first removes the entire class of race rather than guarding one
member of it: the instant the lock that performs the close is released, the
pane (or every pane of the tab/session) no longer exists in `state.tree` at
all, so **every** method - not only a second `--force` - gets the ordinary,
already-existing `PaneNotFound`/`NoSuchTarget` any stale target gets, with no
new state and nothing threaded through `dispatch/lookup.rs`'s shared
resolution helpers. §6 is the check of what this costs before committing to
it; read that first if the reordering it requires (above) is in question.

**Shared scope lookup, `dispatch/lookup.rs`** (new): before the close,
`Runtime` needs every pane id the request's target covers, because after a
successful close the tree can no longer be asked:
```rust
pub(crate) fn panes_in_close_scope(tree: &Tree, context: &Context, request: &Request)
    -> Result<Vec<PaneId>, ApiError>
```
`pane::close` → one pane; `tab::close` → a tab's panes; `session::close` → a
session's tabs' panes - the same three lookups `pane::close`/`tab::close`/
`session::close`'s own `any_running` checks already duplicate verbatim; their
own refusal checks are rewritten to call this (`.any(|p| p.program_status() ==
Running)`) instead, no behaviour change, a small deduplication motivated
primarily by `Runtime`'s own new need.

**`Runtime::handle`** gains a fourth intercepted group, alongside the three
starting methods - unconditionally, force or not (needed below for the
idle-handle fix, item 2):
```rust
"pane.close" | "tab.close" | "session.close" => self.close_and_dispatch(description, context, &resolved),
```

`Runtime::close_and_dispatch`:
1. Lock. `scope = dispatch::panes_in_close_scope(&state.tree, context,
   request)?` - propagate a lookup error (`PaneNotFound`/`TabNotFound`/
   `SessionNotFound`) unchanged, before anything else runs.
2. Call the existing, unchanged `dispatch::dispatch(&mut state.tree, &mut
   state.settings, description, context, request)`. Its own close arm's
   refusal check (`ProgramRunning`, untouched) still runs and still wins when
   `!force` and something in scope is running - on that error, return it
   immediately; nothing has changed, so there is nothing to clean up. On
   success (either `force` was true, or nothing was running - the same two
   cases dispatch's own arm already treats as "proceed," unchanged), the real
   `tree.close_pane`/`close_tab`/`close_session` has already run (R94/R95's
   cascade, zero kernel changes) - the entire `scope` is gone from `state.tree`
   from this point on, atomically, under the lock still held.
3. For every pane id in `scope` (all of them, unconditionally - a successful
   close removes the whole scope at once, never partially, so there is no
   "did this one actually close" check to make): pop its entry from
   `state.panes` if present, into `to_force`. **This is the fix for the
   adjacent gap found while reading, included per the engineer's own
   instruction (item 2)**: a pane whose program had already ended before this
   call (an idle, non-running handle) is popped here exactly the same as a
   genuinely running one - nothing before this task ever removed a handle on
   the ordinary, non-force close path, so that handle and its now-pointless
   worker thread lived forever. `force_end` (§2, unchanged) already treats an
   idle handle correctly and cheaply: `signal_hangup`/`signal_kill` read a
   `0` pid and no-op, `await_exit` sees no run and replies (and breaks, §1)
   at once - so folding this fix into the same pass costs nothing extra for
   the already-ended case and is not a second mechanism to maintain.
4. Apply the existing general resize step (`reapply_every_sized_tab`) now,
   under the same lock, exactly as the `_` arm already does - the siblings
   that gained the closed pane's space (R95) resize **immediately**, not
   after the grace period, which §6 names as a genuine improvement over the
   literal AC #1 ordering, not only a cost.
5. Unlock.
6. `force_end(to_force, self.force_grace)` (§2) - lock-free, so no other
   pane's request is ever stalled by this (AC #4's first clause; see §6 for
   why this is now true for *every* method, not only close).
7. Return the response `dispatch` gave in step 2 - only now, once `force_end`
   has finished, so the caller's reply still means "the program has ended (as
   far as this mechanism can make it) and the pane is gone," even though the
   tree update itself happened earlier in this same call.

**Who removes the pane (brief item 2), now trivially**: `apply_event_locked`
needs **no new check at all**. The pane's own eventual, real `PaneEvent::
Exited` (sent unconditionally by the worker, whether the program died from
this task's own signal or on its own during the grace window - the brief's
own named race, directly) reaches the drain thread exactly as it always has.
By then `state.tree` no longer has the pane (closed in step 2, before
`force_end` ever runs) and `state.panes` no longer has its handle (popped in
step 3, into `force_end`'s own local `to_force`, never put back) - so `tree.
record_exit` returns `Err(PaneNotFound)`, and the **existing, already-built**
`Err(PaneNotFound) => { state.panes.remove(&pane); }` arm (TASK-1.76's own
answer to the old `--force` orphan gap) handles it precisely, as a no-op
(removing a key no longer there). What was a named gap in TASK-1.76 - the tree
losing a pane while its handle lingered - is, under this design, exactly what
every forced close now deliberately does, every time, for the whole grace
window; the code that already existed to tolerate it is the right home for
this, not a new check beside it.

## 6. What close-first costs, checked before committing to it

The engineer asked four things to check; each is answered directly, not
asserted.

**Can "a kill that cannot be ignored" fail, and what is left behind if it
does?** `SIGKILL` cannot be trapped or blocked, but it is still only
*delivered*, not instantaneous: a process stuck in an uninterruptible kernel
sleep (classically, certain blocked I/O - a wedged network filesystem is the
textbook case) does not act on any pending signal, including `SIGKILL`, until
it leaves that state, which userspace has no way to force and, in the
pathological case, may never happen. Nothing in this plan, or in
`Runtime::stop`'s own pre-existing mechanism, can do better than this - it is
not a new risk this task introduces. What is left if it happens: the pane is
already gone from the tree (step 2, before this can even be observed) - the
caller still gets `Response::Ok` (there is no error code for "we could not
confirm the program actually died," and inventing one is not asked for by any
of the five criteria); `force_end`'s own bounded `join_worker` times out and
its `JoinHandle` is dropped unjoined - the worker (and the relay thread still
blocked in `child.wait()` underneath it) keeps running, detached, until the
process eventually does exit (possibly on server shutdown, possibly never, in
the fully pathological case) - the same residual risk `Runtime::stop` already
accepted, named rather than newly discovered.

**Is anything a client could want between the force and the death lost?**
Yes, and it matters for a later task, not this one: the pane disappears from
the tree the moment the force begins, not when the program actually dies - up
to one grace period earlier than the literal AC #1 ordering would give. No
method exists today to observe a pane's output once it is gone (`pane.
capture`/attach are both `not_implemented_yet`), so this is not observable by
anything this task's own tests can exercise - but TASK-1.47 (attach) should
read this plainly when it is built: a forced close does not promise a
client watching the pane sees whatever the program printed between the
signal and its actual death. Named here so that task does not have to
rediscover it.

**Does a future tree-change event (TASK-1.46) fire before the program is
dead?** Yes, by construction - the close happens in step 2, before `force_end`
runs. This is not a new problem for TASK-1.46 to solve: such an event would
correctly describe tree state ("this pane no longer exists"), not process
lifecycle ("this program's exit code is N") - the two are already different
facts in this codebase (`PaneRecord`/`ProgramStatus` versus the tree's own
shape), and nothing in today's design claims a close event would mean the
program had finished first. Nothing to build here; named for whoever writes
TASK-1.46.

**Can a `--force` race a pane whose program is still starting?** No - checked
against the existing code, not assumed. `Runtime::start_and_dispatch` (TASK-
1.76 §1) holds `Runtime`'s one lock continuously across minting, the real
spawn, and inserting the `PaneHandle` - its own stated invariant is exactly
"nothing else runs between those steps." `close_and_dispatch` needs the same
lock to do anything at all, so it cannot observe a pane that exists in the
tree without a handle in `state.panes`, or vice versa, at any point - by the
time either call site even acquires the lock, the pane is either fully
started (handle present, status `Running`) or does not exist (rolled back on
a failed start). There is no third state for a force to race.

**What AC #4's "cannot be reached" now covers, restated**: *every* method, not
only a second `--force` - a `pane.show`, `pane.swap`, `pane.move`, `pane.set`/
`unset`, or a plain close naming a pane mid-force all get the tree's own
ordinary "this target does not exist" the instant the force begins, by
construction, with nothing new threaded through `dispatch/lookup.rs`. This is
the full guarantee the first draft's `forcing` set only gave for one method
against itself - resolved by removing the race's precondition (the pane
existing at all) rather than guarding each place that could reach it.

## 7. Error codes (AC #4, #5)

None changed, and none added. The existing `ErrorCode::ProgramRunning` (used
by the three close arms' own, unmoved refusal check) needs no reword this
time - unlike the first draft, there is no second, forcing-specific refusal to
name, because §5's close-first design makes a racing request fail with the
tree's own ordinary `PaneNotFound`/`TabNotFound`/`SessionNotFound`
(`ErrorCode::NoSuchTarget`), the same code any stale target already gets
(doc-2 section 6: "closing something already closed is status 3, like any
target that matches nothing" - this is exactly that case, reached naturally,
not specially coded for).

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

- `crates/server/src/pane.rs` - `Running.pid` replaces `.killer`; `PaneCommand`
  drops `Stop`, gains the one new `AwaitExit` variant (§1 - `Hangup`/`Escalate`
  are plain `PaneHandle` methods over the shared atomic, never commands on the
  queue); `PaneHandle::{signal_hangup,signal_kill,await_exit,join_worker}`
  replace `stop`; `STOP_TIMEOUT` deleted; module doc and `spawn_relay`'s own
  long comment corrected (the gap they describe is this task, landing now, not
  a future one).
- `crates/server/src/signal.rs` - new: the one `killpg` call, `nix`-based.
- `crates/server/src/runtime.rs` - `force_end` (new, shared); `Runtime::stop`
  rewritten to call it; `force_grace: Duration` field and constructor
  parameter, `FORCE_GRACE_DEFAULT`; `close_and_dispatch` (new); `Runtime::
  handle`'s match gains the three close methods. `apply_event_locked` itself
  is **unchanged** - its existing `Err(PaneNotFound)` arm already does what
  §5's design needs (confirm this by reading, not by adding a test that
  exercises a code path that was never touched).
- `crates/server/src/dispatch/lookup.rs` - `panes_in_close_scope` (new, shared).
- `crates/server/src/dispatch/{pane,tab,session}.rs` - `close()`'s own
  `any_running` check calls the new shared helper; behaviour unchanged.
- `crates/server/src/dispatch/mod.rs` - unchanged; `program_running()` keeps
  its current doc comment, verbatim (§7).
- No `crates/sdk` change (§7 - corrected from this plan's first draft, which
  had reworded `ErrorCode::ProgramRunning`'s doc comment for a refusal this
  design no longer needs).
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
8. `runtime.rs`'s `close_and_dispatch` and `Runtime::handle`'s match - the
   full AC #1/#3/#4 proof (§9), including the check that `apply_event_locked`
   needed no change at all (confirmed by reading plus the natural-death-
   during-grace test, not assumed).
9. `nix`'s `"signal"` feature, `boundaries.toml`'s reason string - `just deny`/
   `just check-boundaries` recorded clean.
10. `just check` at each numbered step's own boundary, not only at the end.

## 9. Proof (AC by AC, every mechanism with a negative control)

Every real-process test uses a bounded wait (an event, or a polled condition
with a stated bound, per this crate's own established pattern - never a bare
`sleep`/`recv`) and asserts no child/thread survives it, via the same `kill -0`
probe this crate's tests already use. Every repeated-run claim below is run at
this crate's **default** test parallelism (`cargo test -p steg-server`/`just
test`), not `--test-threads=1` - naming the exact thing the second round of
TASK-1.76's own review found wrong with recording the narrower command instead.

**AC #1** (every running program in scope ends, exit observed, and the pane
closes - **in the order §5/§6 give, the tree first, named there as a
departure from the criterion's own literal wording**; producing output and
silent): `pane.close --force` on one command pane running `yes` (continuous
output) and one running `sleep 1000` (silent), each through `Runtime::
handle`, asserting the pane is gone from the tree **and** the process is gone
(`kill -0`) by the time the call returns, and the response is `Ok`. `tab.close
--force`/`session.close --force` over several such panes at once (brief item
4). Negative control: disable `close_and_dispatch`'s own pop-into-`to_force`
step (step 3), leaving the tree close but skipping `force_end` entirely - the
silent-program case fails (process still alive after the call returns),
proving the test actually exercises the new mechanism, not just the tree
update it would have needed anyway.

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
| A plain close of an idle, already-ended pane | step 3 pops its handle into `to_force`; `force_end` no-ops the signal and ends the worker via one immediate `await_exit` | new - item 2's fix, its own row, its own test (§5) |
| `--force`, program already ended before the call | same as the row above - force-vs-not makes no difference once nothing is running | new, shares the test above |
| `--force`, program dies naturally *during* the grace window | the tree already closed in step 2 (before `force_end` runs); the worker's own real `Exited` reaches the drain thread exactly as always, `tree.record_exit` returns `Err(PaneNotFound)`, the **existing, unmodified** arm removes the (already-nonexistent-elsewhere) handle | new - the brief's own named race, directly, proving the *existing* code handles it with no change, not that a new check does. Asserts the worker thread itself actually ends (via a test-only accessor, or by confirming a second command sent to the same `PaneHandle`'s `commands` sender fails) - §1's own fix (the "replies at once" `AwaitExit` branch must also `break`) is specifically for this row: without it, the test would see the right process-level outcome while leaking the worker thread, which a test that only checks `kill -0` on the *process* would miss entirely |
| `--force` during pane start-up | checked, not tested separately: §6 shows this cannot happen (`Runtime`'s one lock makes "started but no handle yet" unobservable to any other call) | existing invariant, re-confirmed by reading, no new test needed |
| `Runtime::stop`/`Drop` | `force_end`, scope = every pane | rewritten (§4) |

**AC #4** (no stall, no reach meanwhile - now the full guarantee, §6, not only
force-vs-force): one test, using a deliberately long `force_grace` for this
test alone (§3) rather than the suite's usual tiny value - a 30ms window makes
"returned before the window closed" scheduling noise, not evidence. Starts two
sessions, forces one pane with a trapping program (so the window is both real
and guaranteed not to end early) while concurrently issuing `pane.show`
against the *other* session's pane from a second thread, asserting the second
call returns well inside that long window (a small, stated fraction of it, not
"eventually") - proves no stall. A second test, using the same long-grace,
trapping-program setup: a *second* thread issues `pane.show` (then, separately,
a plain `pane.close`, then `pane.swap`) against the **same** pane that is
being forced, each asserting it is refused with the ordinary `PaneNotFound`/
`NoSuchTarget` at once, not after the grace window - proving the pane is
unreachable by every method, not only a second `--force`, which is what §5's
close-first redesign specifically bought over the first draft's narrower
`forcing` set. A third test: two threads both issue `pane.close --force` on
the same pane; asserts exactly one actually signals/ends the program (via a
real process and `kill -0`) and the other's call either returns `PaneNotFound`
at once (it arrived after the first already closed the tree) or itself
performs the one, same close (it arrived first) - never both. Negative
control for all three: temporarily reorder `close_and_dispatch` back to this
plan's first draft's shape (`force_end` before the tree close) - the second
and third tests then flake or double-act (a `pane.show`/`pane.swap` mid-grace
succeeds against a pane that is about to be forced out from under it, or two
concurrent force-closes both proceed), proving close-first, not merely the
lock, is what the tests depend on.

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

- **AC #1's stated order (end, observe, then close) is reversed by design**
  (Scope, §5, §6) - the tree closes first, under one lock acquisition, and the
  program is ended after, with the caller's reply held until that is done.
  This is what gives AC #4 its full guarantee (every method unreachable, not
  only a second `--force`); the literal order was tried first and found not
  to give that guarantee (Scope's own account of why). Confirm this trade, or
  ask for the literal order with a narrower AC #4 instead.
- §5 step 3's idle-handle fix (a plain close of an already-ended pane leaking
  its handle) is a pre-existing TASK-1.76 gap, not named by any of this
  task's five criteria. Included per instruction, folded into the same pass
  `force_end` already runs for a genuinely forced pane, at no extra cost.
- §6 answers four specific costs of close-first (a kill that never actually
  completes because the process is wedged in an uninterruptible kernel sleep;
  a client's view of the pane's last output between the signal and the
  death; a future TASK-1.46 close event firing before the program is
  confirmed dead; a force racing a pane still starting) - each checked
  against the existing code or named as an accepted, pre-existing residual
  risk, none left silent.
- §0's job-control limitation (an interactive shell's own backgrounded job,
  reassigned to a new process group) is named, not fixed - no AC asks for it,
  and fixing it needs either controlling-terminal state or a full descendant
  walk, both out of this task's scope.
- `Runtime::stop`'s rewrite (§4) changes already-reviewed, merged behaviour
  (TASK-1.76 PR #40) - named explicitly rather than folded in silently, per
  that task's own two "unnamed departure" findings.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Rewritten plan checked against doc-2 sections 8 and 9, doc-8 R97, doc-15 and doc-16 by @bruk.o: ok, with one item that is the engineer's. The forced close is done close-first: under one lock acquisition the refusals are checked, the handles taken and the pane, tab or session closed in the tree; the lock is dropped; then every program in scope is ended outside it (the process group signalled, one shared deadline, the stragglers killed, each exit observed, every thread joined) and the caller answered. From the instant the lock is released nothing can name a pane being forced, which meets the fourth criterion for every method and needs no extra state; the program's own exit event finding no pane is what apply_event already tolerates. This reverses the order the first criterion's wording gives (end, observe, then close): approving the plan confirms the change, and the orchestrator then rewords that criterion. Runtime::stop is rebuilt on the same primitive, so its bound no longer grows with the number of panes and a program ignoring a hangup no longer survives it. Folded in, found in TASK-1.76's code: a plain close of a pane whose program had ended left its handle and worker thread behind. Stated residuals: a process in an uninterruptible kernel sleep may outlive the kill for a time, with its thread pair left until it clears; an interactive shell's job control moving a background job to its own process group is not reached by the group signal. No kernel or sdk change, no new dependency (one feature on nix), no unsafe.

Check passed: `just check` exited 0 at 2026-11-16T00:28:57+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.78 on task-1.78-server-a-forced-close-ends-the-program at ab290648a7b0:

```text
ok      format check (0.2s)
ok      lint (92.9s)
ok      boundaries (0.2s)
ok      licences (0.9s)
skip    tests: this change touches no crate
ok      build (steg-sdk) (3.3s)
ok      test (steg-sdk) (1.9s)
ok      test (steg-sdk, local-socket) (2.1s)
ok      build (steg-kernel) (45.2s)
ok      test (steg-kernel) (1.5s)
ok      build (steg-server) (44.8s)
ok      test (steg-server) (6.3s)
ok      test (steg-server, race-fixture) (6.6s)
ok      build (steg) (0.2s)
ok      test (steg) (0.3s)
ok      build (steg-app) (19.3s)
ok      test (steg-app) (0.8s)
ok      tool tests (2.4s)
ok      architecture (2.0s)
ok      recipe names (0.2s)
```

Check passed: `just check` exited 0 at 2026-11-16T00:30:18+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.78 on task-1.78-server-a-forced-close-ends-the-program at 61369d880a6d:

```text
ok      format check (0.2s)
ok      lint (1.0s)
ok      boundaries (0.1s)
ok      licences (0.7s)
skip    tests: this change touches no crate
ok      build (steg-sdk) (0.1s)
ok      test (steg-sdk) (0.2s)
ok      test (steg-sdk, local-socket) (0.3s)
ok      build (steg-kernel) (0.5s)
ok      test (steg-kernel) (1.3s)
ok      build (steg-server) (0.8s)
ok      test (steg-server) (6.2s)
ok      test (steg-server, race-fixture) (7.0s)
ok      build (steg) (0.2s)
ok      test (steg) (0.3s)
ok      build (steg-app) (0.3s)
ok      test (steg-app) (0.2s)
ok      tool tests (0.9s)
ok      architecture (1.0s)
ok      recipe names (0.0s)
```

Check FAILED: `cargo test -p steg-server --lib signal::` exited 101 at 2026-11-16T00:40:49+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.78 on task-1.78-server-a-forced-close-ends-the-program at 61369d880a6d with uncommitted changes, last 40 of 49 lines:

```text
22 | use nix::sys::signal::{Signal, killpg};
   |                                ^^^^^^
   |
   = note: `#[warn(unused_imports)]` (part of `#[warn(unused)]`) on by default

warning: unused import: `nix::unistd::Pid`
  --> crates/server/src/signal.rs:23:5
   |
23 | use nix::unistd::Pid;
   |     ^^^^^^^^^^^^^^^^

warning: `steg-server` (lib test) generated 2 warnings (run `cargo fix --lib -p steg-server --tests` to apply 2 suggestions)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 30.01s
     Running unittests src/lib.rs (target/debug/deps/steg_server-6d2f1d51fdd2e018)

running 2 tests
test signal::tests::send_reaches_a_real_process_with_the_named_signal ... FAILED
test signal::tests::send_reaches_the_whole_process_group_not_only_the_leader ... FAILED

failures:

---- signal::tests::send_reaches_a_real_process_with_the_named_signal stdout ----

thread 'signal::tests::send_reaches_a_real_process_with_the_named_signal' (15974998) panicked at crates/server/src/signal.rs:77:56:
the signalled process must exit
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace

---- signal::tests::send_reaches_the_whole_process_group_not_only_the_leader stdout ----

thread 'signal::tests::send_reaches_the_whole_process_group_not_only_the_leader' (15974999) panicked at crates/server/src/signal.rs:108:56:
the signalled shell must exit


failures:
    signal::tests::send_reaches_a_real_process_with_the_named_signal
    signal::tests::send_reaches_the_whole_process_group_not_only_the_leader

test result: FAILED. 0 passed; 2 failed; 0 ignored; 0 measured; 83 filtered out; finished in 5.01s

error: test failed, to rerun pass `-p steg-server --lib`
```

Check FAILED: `cargo test -p steg-server --lib signal::` exited 101 at 2026-11-16T00:43:27+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.78 on task-1.78-server-a-forced-close-ends-the-program at 61369d880a6d with uncommitted changes:

```text
   Compiling steg-server v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.78/crates/server)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.65s
     Running unittests src/lib.rs (target/debug/deps/steg_server-6d2f1d51fdd2e018)

running 2 tests
test signal::tests::send_reaches_a_real_process_with_the_named_signal ... FAILED
test signal::tests::send_reaches_the_whole_process_group_not_only_the_leader ... FAILED

failures:

---- signal::tests::send_reaches_a_real_process_with_the_named_signal stdout ----

thread 'signal::tests::send_reaches_a_real_process_with_the_named_signal' (16141512) panicked at crates/server/src/signal.rs:109:56:
the signalled process must exit
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace

---- signal::tests::send_reaches_the_whole_process_group_not_only_the_leader stdout ----

thread 'signal::tests::send_reaches_the_whole_process_group_not_only_the_leader' (16141513) panicked at crates/server/src/signal.rs:143:56:
the signalled shell must exit


failures:
    signal::tests::send_reaches_a_real_process_with_the_named_signal
    signal::tests::send_reaches_the_whole_process_group_not_only_the_leader

test result: FAILED. 0 passed; 2 failed; 0 ignored; 0 measured; 83 filtered out; finished in 5.01s

error: test failed, to rerun pass `-p steg-server --lib`
```

Check passed: `cargo test -p steg-server --lib signal::` exited 0 at 2026-11-16T00:44:13+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.78 on task-1.78-server-a-forced-close-ends-the-program at 61369d880a6d with uncommitted changes:

```text
   Compiling steg-server v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.78/crates/server)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.72s
     Running unittests src/lib.rs (target/debug/deps/steg_server-6d2f1d51fdd2e018)

running 2 tests
test signal::tests::send_reaches_a_real_process_with_the_named_signal ... ok
kill: 44839: No such process
test signal::tests::send_reaches_the_whole_process_group_not_only_the_leader ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 83 filtered out; finished in 0.01s
```

Check passed: `cargo test -p steg-server --lib signal::` exited 0 at 2026-11-16T00:45:06+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.78 on task-1.78-server-a-forced-close-ends-the-program at 61369d880a6d with uncommitted changes:

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.08s
     Running unittests src/lib.rs (target/debug/deps/steg_server-6d2f1d51fdd2e018)

running 2 tests
test signal::tests::send_reaches_a_real_process_with_the_named_signal ... ok
kill: 47774: No such process
test signal::tests::send_reaches_the_whole_process_group_not_only_the_leader ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 83 filtered out; finished in 0.01s
```

Check passed: `cargo test -p steg-server --lib` exited 0 at 2026-11-16T01:00:14+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.78 on task-1.78-server-a-forced-close-ends-the-program at 61369d880a6d with uncommitted changes, last 40 of 103 lines:

```text
test runtime::stop_tests::stop_with_no_panes_at_all_returns_at_once ... ok
test runtime::tests::login_shell_from_process_env_never_panics_and_is_never_empty ... ok
test listen::tests::a_finished_connection_is_removed_and_a_still_open_one_is_kept ... ok
test runtime::tests::runtime_new_does_not_panic_and_starts_empty ... ok
test paths::tests::a_real_plain_file_at_the_path_is_not_a_socket ... ok
test bind::tests::a_real_bind_sets_0600_under_the_ambient_umask ... ok
test paths::tests::a_real_private_directory_is_safe ... ok
test arbiter::tests::attempt_finds_a_real_live_listener_already_running ... ok
test signal::tests::send_reaches_a_real_process_with_the_named_signal ... ok
test bind::tests::nothing_is_left_at_the_temporary_name_after_a_successful_bind ... ok
test paths::tests::a_real_world_writable_directory_is_refused ... ok
test paths::tests::a_real_socket_we_own_is_ours_and_a_socket ... ok
test bind::tests::a_second_bind_replaces_the_first_at_the_same_path ... ok
test arbiter::tests::attempt_becomes_the_server_when_nothing_is_there ... ok
test arbiter::tests::attempt_takes_over_a_stale_socket_left_by_a_dead_server ... ok
test pane::ending_tests::signal_hangup_ends_a_real_program_through_the_public_handle ... ok
kill: 1939: No such process
test signal::tests::send_reaches_the_whole_process_group_not_only_the_leader ... ok
test pane::ending_tests::await_exit_after_the_program_already_ended_still_ends_the_worker_thread ... ok
test runtime::tests::apply_event_on_a_pane_the_tree_no_longer_holds_removes_the_stale_handle_without_panicking ... ok
test runtime::tests::a_real_exit_event_drives_the_same_cascade_apply_event_runs ... ok
test runtime::tests::apply_size_changed_lets_a_pane_go_under_the_floor_when_the_kernel_says_so ... ok
test runtime::tests::apply_size_changed_resizes_only_the_panes_the_change_names ... ok
kill: 1966: No such process
test runtime::stop_tests::stop_ends_a_silent_program ... ok
kill: 1971: No such process
test runtime::stop_tests::stop_is_a_no_op_for_a_pane_whose_program_has_already_ended ... ok
kill: 1969: No such process
test runtime::stop_tests::stop_ends_a_program_producing_output ... ok
test runtime::tests::pane_split_inherits_the_live_cwd_of_the_pane_it_split_from_not_its_started_in_cwd ... ok
test runtime::tests::pane_split_falls_back_to_home_when_the_inherited_live_cwd_no_longer_exists ... ok
test runtime::tests::the_general_sizing_step_reaches_every_pane_through_split_close_and_swap ... ok
kill: 1970: No such process
test runtime::stop_tests::stop_ends_a_program_that_traps_the_polite_signal ... ok
test pane::ending_tests::signal_hangup_reaches_a_backgrounded_grandchild_too ... ok
test pane::ending_tests::signal_kill_ends_a_program_that_traps_the_polite_signal ... ok
kill: 1940: No such process
test pane::tests::a_forward_failing_once_the_worker_is_gone_still_reaps_a_program_that_ends_on_its_own ... ok

test result: ok. 90 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.04s
```

Check passed: `just lint` exited 0 at 2026-11-16T01:01:08+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.78 on task-1.78-server-a-forced-close-ends-the-program at 61369d880a6d with uncommitted changes:

```text
ok      lint (0.5s)
```

Check passed: `cargo test -p steg-server --lib --tests` exited 0 at 2026-11-16T01:02:59+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.78 on task-1.78-server-a-forced-close-ends-the-program at 61369d880a6d with uncommitted changes, last 40 of 257 lines:

```text
test a_nonexistent_cwd_through_api_call_leaves_nothing_behind ... ok
test pane_split_with_a_nonexistent_cwd_leaves_no_new_pane_behind ... ok
test session_close_refuses_while_any_pane_in_any_of_its_tabs_is_running ... ok
test pane_close_with_force_on_a_running_program_proceeds_exactly_as_today_the_stated_gap ... ok
test session_new_through_api_call_with_no_program_runs_the_login_shell ... ok
test pane_close_refuses_while_its_program_is_running_and_touches_nothing ... ok
test a_command_that_does_not_exist_fails_and_leaves_no_session_tab_or_pane_behind ... ok
test pane_split_with_no_program_or_cwd_runs_the_login_shell_at_home ... ok
test a_bad_command_through_api_call_leaves_nothing_behind ... ok
test tab_new_with_a_nonexistent_cwd_leaves_no_new_tab_or_pane_behind ... ok
test session_new_through_api_call_starts_a_real_program ... ok
test tab_new_with_a_bad_command_leaves_no_new_tab_or_pane_behind ... ok
test pane_split_with_a_bad_command_leaves_no_new_pane_behind ... ok
test tab_close_refuses_while_any_of_its_panes_is_running ... ok
test session_new_with_a_command_starts_it_and_pane_show_reports_it_running_then_ended ... ok
test tab_new_through_api_call_starts_a_real_program ... ok
test pane_split_through_api_call_starts_a_real_program ... ok

test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.06s

     Running tests/shutdown.rs (target/debug/deps/shutdown-641947b6111d1ffd)

running 8 tests
test stopping_closes_the_listener_removes_the_socket_and_releases_the_lock ... ok
test a_stalled_handshake_is_closed_and_run_returns_promptly_on_stop ... ok
test stop_on_a_genuinely_idle_server_returns_from_run_within_a_bound ... ok
test stop_called_twice_concurrently_on_an_idle_server_is_harmless ... ok
test a_completed_idle_connection_is_closed_and_run_returns_promptly_on_stop ... ok
test many_short_connections_do_not_grow_the_tracked_set ... ok
test stopping_while_a_trickling_handshake_is_in_progress_still_returns_within_a_bound ... ok
test stop_racing_a_client_connecting_still_returns_within_a_bound ... ok

test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.13s

     Running tests/socket_mode.rs (target/debug/deps/socket_mode-e7b594c955bacef5)

running 1 test
test the_socket_is_0600_even_under_a_permissive_umask ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

Check FAILED: `just check` exited 101 at 2026-11-16T01:07:03+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.78 on task-1.78-server-a-forced-close-ends-the-program at 61369d880a6d with uncommitted changes:

```text
ok      format check (0.2s)
FAILED  lint (exit 101, 0.9s)
    Checking steg-server v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.78/crates/server)
    Checking steg v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.78/crates/steg)
error: redundant redefinition of a binding `context`
   --> crates/server/tests/runtime.rs:884:9
    |
884 |         let context = context;
    |         ^^^^^^^^^^^^^^^^^^^^^^
    |
help: `context` is initially defined here
   --> crates/server/tests/runtime.rs:874:9
    |
874 |     let context = Context {
    |         ^^^^^^^
    = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.98.0/index.html#redundant_locals
    = note: `-D clippy::redundant-locals` implied by `-D warnings`
    = help: to override `-D warnings` add `#[allow(clippy::redundant_locals)]`

error: could not compile `steg-server` (test "runtime") due to 1 previous error
warning: build failed, waiting for other jobs to finish...
full output: .quiet/lint.log
error: recipe `lint` failed with exit code 101
```

Check passed: `just check` exited 0 at 2026-11-16T01:09:56+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.78 on task-1.78-server-a-forced-close-ends-the-program at 61369d880a6d with uncommitted changes:

```text
ok      format check (0.2s)
ok      lint (0.4s)
ok      boundaries (0.1s)
ok      licences (0.6s)
ok      tests (every crate) (9.3s)
ok      build (steg-sdk) (0.1s)
ok      test (steg-sdk) (0.2s)
ok      test (steg-sdk, local-socket) (0.4s)
ok      build (steg-kernel) (0.1s)
ok      test (steg-kernel) (0.2s)
ok      build (steg-server) (0.1s)
ok      test (steg-server) (9.0s)
ok      test (steg-server, race-fixture) (9.2s)
ok      build (steg) (0.1s)
ok      test (steg) (0.1s)
ok      build (steg-app) (0.3s)
ok      test (steg-app) (0.2s)
ok      tool tests (1.2s)
ok      architecture (1.0s)
ok      recipe names (0.0s)
```

Check FAILED: `cargo test -p steg-server --test runtime pane_close_with_force_actually_ends_a_silent_program` exited 101 at 2026-11-16T01:10:46+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.78 on task-1.78-server-a-forced-close-ends-the-program at 61369d880a6d with uncommitted changes:

```text
   Compiling ghostty-vt-sys v0.8.0+ghostty.1.4.0-main.83fb956
   Compiling ghostty-vt v0.8.0+ghostty.1.4.0-main.83fb956 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.78/third_party/ghostty-vt-patched)
   Compiling steg-kernel v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.78/crates/kernel)
   Compiling steg-server v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.78/crates/server)
warning: unused variable: `scope`
   --> crates/server/src/runtime.rs:453:13
    |
453 |         let scope = match dispatch::panes_in_close_scope(&state.tree, context, request) {
    |             ^^^^^ help: if this is intentional, prefix it with an underscore: `_scope`
    |
    = note: `#[warn(unused_variables)]` (part of `#[warn(unused)]`) on by default

warning: `steg-server` (lib) generated 1 warning (run `cargo fix --lib -p steg-server` to apply 1 suggestion)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 30.91s
     Running tests/runtime.rs (target/debug/deps/runtime-636749399b762beb)

running 1 test
test pane_close_with_force_actually_ends_a_silent_program ... FAILED

failures:

---- pane_close_with_force_actually_ends_a_silent_program stdout ----

thread 'pane_close_with_force_actually_ends_a_silent_program' (16678026) panicked at crates/server/tests/runtime.rs:556:5:
a silent program must be ended too, not left running because it never produced output for the relay thread to read
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    pane_close_with_force_actually_ends_a_silent_program

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 24 filtered out; finished in 0.03s

error: test failed, to rerun pass `-p steg-server --test runtime`
```

Check passed: `cargo test -p steg-server --test runtime pane_close_with_force_actually_ends_a_silent_program` exited 0 at 2026-11-16T01:11:00+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.78 on task-1.78-server-a-forced-close-ends-the-program at 61369d880a6d with uncommitted changes:

```text
   Compiling steg-server v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.78/crates/server)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.69s
     Running tests/runtime.rs (target/debug/deps/runtime-636749399b762beb)

running 1 test
kill: 52003: No such process
test pane_close_with_force_actually_ends_a_silent_program ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 24 filtered out; finished in 0.03s
```

Check FAILED: `cargo test -p steg-server --test runtime pane_close_with_force_ends_a_program_that_traps_the_polite_signal` exited 101 at 2026-11-16T01:11:31+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.78 on task-1.78-server-a-forced-close-ends-the-program at 61369d880a6d with uncommitted changes:

```text
   Compiling steg-server v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.78/crates/server)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.67s
     Running tests/runtime.rs (target/debug/deps/runtime-636749399b762beb)

running 1 test
test pane_close_with_force_ends_a_program_that_traps_the_polite_signal ... FAILED

failures:

---- pane_close_with_force_ends_a_program_that_traps_the_polite_signal stdout ----

thread 'pane_close_with_force_ends_a_program_that_traps_the_polite_signal' (16688429) panicked at crates/server/tests/runtime.rs:592:5:
a program trapping the polite signal must still be ended, by escalating
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    pane_close_with_force_ends_a_program_that_traps_the_polite_signal

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 24 filtered out; finished in 6.08s

error: test failed, to rerun pass `-p steg-server --test runtime`
```

Check passed: `cargo test -p steg-server --test runtime pane_close_with_force_ends_a_program_that_traps_the_polite_signal` exited 0 at 2026-11-16T01:11:45+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.78 on task-1.78-server-a-forced-close-ends-the-program at 61369d880a6d with uncommitted changes:

```text
   Compiling steg-server v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.78/crates/server)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.66s
     Running tests/runtime.rs (target/debug/deps/runtime-636749399b762beb)

running 1 test
kill: 55008: No such process
test pane_close_with_force_ends_a_program_that_traps_the_polite_signal ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 24 filtered out; finished in 0.06s
```

Check FAILED: `cargo test -p steg-server --lib pane::ending_tests::await_exit_after_the_program_already_ended_still_ends_the_worker_thread` exited 101 at 2026-11-16T01:12:22+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.78 on task-1.78-server-a-forced-close-ends-the-program at 61369d880a6d with uncommitted changes:

```text
   Compiling steg-server v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.78/crates/server)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.59s
     Running unittests src/lib.rs (target/debug/deps/steg_server-6d2f1d51fdd2e018)

running 1 test
test pane::ending_tests::await_exit_after_the_program_already_ended_still_ends_the_worker_thread ... FAILED

failures:

---- pane::ending_tests::await_exit_after_the_program_already_ended_still_ends_the_worker_thread stdout ----

thread 'pane::ending_tests::await_exit_after_the_program_already_ended_still_ends_the_worker_thread' (16702677) panicked at crates/server/src/pane.rs:800:9:
the worker thread must actually end, not just reply - the bug this test is named for left a worker thread running forever behind a reply that looked correct on its own
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    pane::ending_tests::await_exit_after_the_program_already_ended_still_ends_the_worker_thread

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 89 filtered out; finished in 10.01s

error: test failed, to rerun pass `-p steg-server --lib`
```

Check passed: `cargo test -p steg-server --lib pane::ending_tests::await_exit_after_the_program_already_ended_still_ends_the_worker_thread` exited 0 at 2026-11-16T01:12:35+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.78 on task-1.78-server-a-forced-close-ends-the-program at 61369d880a6d with uncommitted changes:

```text
   Compiling steg-server v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.78/crates/server)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.64s
     Running unittests src/lib.rs (target/debug/deps/steg_server-6d2f1d51fdd2e018)

running 1 test
test pane::ending_tests::await_exit_after_the_program_already_ended_still_ends_the_worker_thread ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 89 filtered out; finished in 0.01s
```

Check FAILED: `cargo test -p steg-server --test runtime -- a_pane_being_forced_is_unreachable_by_another_request_meanwhile` exited 101 at 2026-11-16T01:13:43+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.78 on task-1.78-server-a-forced-close-ends-the-program at 61369d880a6d with uncommitted changes:

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.08s
     Running tests/runtime.rs (target/debug/deps/runtime-636749399b762beb)

running 1 test
test a_pane_being_forced_is_unreachable_by_another_request_meanwhile ... FAILED

failures:

---- a_pane_being_forced_is_unreachable_by_another_request_meanwhile stdout ----

thread 'a_pane_being_forced_is_unreachable_by_another_request_meanwhile' (16735843) panicked at crates/server/tests/runtime.rs:841:5:
a request for the pane being forced took 4.955128667s - it must be refused at once (the pane is already gone, close-first), never stalled for the grace window
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    a_pane_being_forced_is_unreachable_by_another_request_meanwhile

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 24 filtered out; finished in 5.04s

error: test failed, to rerun pass `-p steg-server --test runtime`
```

Check passed: `just check` exited 0 at 2026-11-16T01:15:14+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.78 on task-1.78-server-a-forced-close-ends-the-program at 61369d880a6d with uncommitted changes:

```text
ok      format check (0.2s)
ok      lint (0.8s)
ok      boundaries (0.1s)
ok      licences (0.6s)
ok      tests (every crate) (12.0s)
ok      build (steg-sdk) (0.1s)
ok      test (steg-sdk) (0.2s)
ok      test (steg-sdk, local-socket) (0.4s)
ok      build (steg-kernel) (0.1s)
ok      test (steg-kernel) (0.2s)
ok      build (steg-server) (28.8s)
ok      test (steg-server) (11.9s)
ok      test (steg-server, race-fixture) (12.6s)
ok      build (steg) (0.2s)
ok      test (steg) (0.4s)
ok      build (steg-app) (0.3s)
ok      test (steg-app) (0.3s)
ok      tool tests (1.3s)
ok      architecture (1.0s)
ok      recipe names (0.0s)
```

Check passed: `sh -c 'fail=0; for i in $(seq 1 10); do echo "=== run $i ==="; if ! cargo test -p steg-server 2>&1 | tail -3 | grep -q "test result: ok"; then fail=$((fail+1)); echo "RUN $i FAILED"; fi; done; echo "TOTAL FAILURES: $fail / 10"; test "$fail" -eq 0'` exited 0 at 2026-11-16T01:17:29+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.78 on task-1.78-server-a-forced-close-ends-the-program at 61369d880a6d with uncommitted changes:

```text
=== run 1 ===
=== run 2 ===
=== run 3 ===
=== run 4 ===
=== run 5 ===
=== run 6 ===
=== run 7 ===
=== run 8 ===
=== run 9 ===
=== run 10 ===
TOTAL FAILURES: 0 / 10
```

Check passed: `sh -c 'fail=0; for i in $(seq 1 10); do echo "=== run $i ==="; out=$(cargo test -p steg-server 2>&1); if echo "$out" | grep -qE "FAILED|error\["; then fail=$((fail+1)); echo "RUN $i FAILED"; echo "$out" | grep -E "FAILED|error\[" ; fi; done; echo "TOTAL FAILURES: $fail / 10"; test "$fail" -eq 0'` exited 0 at 2026-11-16T01:19:10+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.78 on task-1.78-server-a-forced-close-ends-the-program at 61369d880a6d with uncommitted changes:

```text
=== run 1 ===
=== run 2 ===
=== run 3 ===
=== run 4 ===
=== run 5 ===
=== run 6 ===
=== run 7 ===
=== run 8 ===
=== run 9 ===
=== run 10 ===
TOTAL FAILURES: 0 / 10
```

Check FAILED: `cargo test -p steg-server --lib runtime::tests::a_plain_close_of_an_already_ended_pane_removes_its_idle_handle` exited 101 at 2026-11-16T01:23:23+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.78 on task-1.78-server-a-forced-close-ends-the-program at b7e872af7f6b with uncommitted changes:

```text
   Compiling steg-server v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.78/crates/server)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.75s
     Running unittests src/lib.rs (target/debug/deps/steg_server-6d2f1d51fdd2e018)

running 1 test
test runtime::tests::a_plain_close_of_an_already_ended_pane_removes_its_idle_handle ... FAILED

failures:

---- runtime::tests::a_plain_close_of_an_already_ended_pane_removes_its_idle_handle stdout ----

thread 'runtime::tests::a_plain_close_of_an_already_ended_pane_removes_its_idle_handle' (16997020) panicked at crates/server/src/runtime.rs:1137:9:
the plain close must remove the now-pointless idle handle, not leave it (and its worker thread) running forever
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    runtime::tests::a_plain_close_of_an_already_ended_pane_removes_its_idle_handle

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 90 filtered out; finished in 0.02s

error: test failed, to rerun pass `-p steg-server --lib`
```

Check passed: `cargo test -p steg-server --test runtime pane_close_on_an_already_ended_pane_succeeds_without_force` exited 0 at 2026-11-16T01:23:31+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.78 on task-1.78-server-a-forced-close-ends-the-program at b7e872af7f6b with uncommitted changes:

```text
   Compiling steg-server v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.78/crates/server)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.81s
     Running tests/runtime.rs (target/debug/deps/runtime-636749399b762beb)

running 1 test
test pane_close_on_an_already_ended_pane_succeeds_without_force ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 24 filtered out; finished in 0.03s
```

Check passed: `cargo test -p steg-server --lib runtime::tests::a_plain_close_of_an_already_ended_pane_removes_its_idle_handle` exited 0 at 2026-11-16T01:23:44+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.78 on task-1.78-server-a-forced-close-ends-the-program at b7e872af7f6b with uncommitted changes:

```text
   Compiling steg-server v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.78/crates/server)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.67s
     Running unittests src/lib.rs (target/debug/deps/steg_server-6d2f1d51fdd2e018)

running 1 test
test runtime::tests::a_plain_close_of_an_already_ended_pane_removes_its_idle_handle ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 90 filtered out; finished in 0.02s
```

Check FAILED: `cargo test -p steg-server --test runtime -- tab_close_with_force_ends_every_program_in_scope_within_one_grace_window` exited 101 at 2026-11-16T01:26:03+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.78 on task-1.78-server-a-forced-close-ends-the-program at b7e872af7f6b with uncommitted changes:

```text
   Compiling steg-server v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.78/crates/server)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.91s
     Running tests/runtime.rs (target/debug/deps/runtime-636749399b762beb)

running 1 test
test tab_close_with_force_ends_every_program_in_scope_within_one_grace_window ... FAILED

failures:

---- tab_close_with_force_ends_every_program_in_scope_within_one_grace_window stdout ----

thread 'tab_close_with_force_ends_every_program_in_scope_within_one_grace_window' (17067257) panicked at crates/server/tests/runtime.rs:712:5:
ending four trapping panes at once took 2.020947792s, more than twice one grace window (500ms) - the bound this task's own force_end gives is one shared window regardless of pane count, not `panes * grace`, which four of them would already exceed on its own (2s)
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    tab_close_with_force_ends_every_program_in_scope_within_one_grace_window

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 24 filtered out; finished in 2.06s

error: test failed, to rerun pass `-p steg-server --test runtime`
```

Check passed: `cargo test -p steg-server --test runtime -- tab_close_with_force_ends_every_program_in_scope_within_one_grace_window` exited 0 at 2026-11-16T01:26:20+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.78 on task-1.78-server-a-forced-close-ends-the-program at b7e872af7f6b with uncommitted changes:

```text
    Blocking waiting for file lock on package cache
    Blocking waiting for file lock on package cache
    Blocking waiting for file lock on package cache
   Compiling steg-server v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.78/crates/server)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.91s
     Running tests/runtime.rs (target/debug/deps/runtime-636749399b762beb)

running 1 test
kill: 35796: No such process
kill: 35798: No such process
kill: 35799: No such process
kill: 35800: No such process
test tab_close_with_force_ends_every_program_in_scope_within_one_grace_window ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 24 filtered out; finished in 0.55s
```

Check FAILED: `just check` exited 101 at 2026-11-16T01:27:31+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.78 on task-1.78-server-a-forced-close-ends-the-program at b7e872af7f6b with uncommitted changes, last 40 of 75 lines:

```text
test runtime::tests::a_real_exit_event_drives_the_same_cascade_apply_event_runs ... ok
test runtime::tests::apply_size_changed_resizes_only_the_panes_the_change_names ... ok
test pane::ending_tests::await_exit_after_the_program_already_ended_still_ends_the_worker_thread ... ok
kill: 41772: No such process
test signal::tests::send_reaches_the_whole_process_group_not_only_the_leader ... ok
test runtime::tests::a_plain_close_of_an_already_ended_pane_removes_its_idle_handle ... ok
test runtime::tests::apply_size_changed_lets_a_pane_go_under_the_floor_when_the_kernel_says_so ... ok
kill: 41818: No such process
test runtime::stop_tests::stop_is_a_no_op_for_a_pane_whose_program_has_already_ended ... ok
kill: 41817: No such process
test runtime::stop_tests::stop_ends_a_silent_program ... ok
kill: 41780: No such process
test runtime::stop_tests::stop_ends_a_program_producing_output ... ok
test runtime::tests::pane_split_inherits_the_live_cwd_of_the_pane_it_split_from_not_its_started_in_cwd ... ok
test runtime::tests::pane_split_falls_back_to_home_when_the_inherited_live_cwd_no_longer_exists ... ok
test runtime::tests::the_general_sizing_step_reaches_every_pane_through_split_close_and_swap ... ok
kill: 41821: No such process
test runtime::stop_tests::stop_ends_a_program_that_traps_the_polite_signal ... ok
test pane::ending_tests::signal_hangup_reaches_a_backgrounded_grandchild_too ... ok
test pane::ending_tests::signal_kill_ends_a_program_that_traps_the_polite_signal ... ok
kill: 41716: No such process
test pane::tests::a_forward_failing_once_the_worker_is_gone_still_reaps_a_program_that_ends_on_its_own ... ok

failures:

---- arbiter::tests::attempt_takes_over_a_stale_socket_left_by_a_dead_server stdout ----

thread 'arbiter::tests::attempt_takes_over_a_stale_socket_left_by_a_dead_server' (17098571) panicked at crates/server/src/arbiter.rs:416:17:
expected Became, got something else: AlreadyRunning
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    arbiter::tests::attempt_takes_over_a_stale_socket_left_by_a_dead_server

test result: FAILED. 90 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.03s

error: test failed, to rerun pass `-p steg-server --lib`
full output: .quiet/test-steg-server.log
error: recipe `check-packages` failed with exit code 101
```

Check passed: `just check` exited 0 at 2026-11-16T01:29:56+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.78 on task-1.78-server-a-forced-close-ends-the-program at b7e872af7f6b with uncommitted changes:

```text
ok      format check (0.2s)
ok      lint (0.4s)
ok      boundaries (0.1s)
ok      licences (0.7s)
ok      tests (every crate) (9.5s)
ok      build (steg-sdk) (0.1s)
ok      test (steg-sdk) (0.3s)
ok      test (steg-sdk, local-socket) (0.4s)
ok      build (steg-kernel) (0.2s)
ok      test (steg-kernel) (0.3s)
ok      build (steg-server) (36.4s)
ok      test (steg-server) (13.0s)
ok      test (steg-server, race-fixture) (13.0s)
ok      build (steg) (0.3s)
ok      test (steg) (0.5s)
ok      build (steg-app) (0.3s)
ok      test (steg-app) (0.4s)
ok      tool tests (1.3s)
ok      architecture (1.0s)
ok      recipe names (0.1s)
```

Check passed: `just check` exited 0 at 2026-11-16T01:31:02+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.78 on task-1.78-server-a-forced-close-ends-the-program at 68d10a8032c0:

```text
ok      format check (0.2s)
ok      lint (0.7s)
ok      boundaries (0.1s)
ok      licences (0.9s)
ok      tests (every crate) (9.6s)
ok      build (steg-sdk) (0.1s)
ok      test (steg-sdk) (0.2s)
ok      test (steg-sdk, local-socket) (0.4s)
ok      build (steg-kernel) (0.1s)
ok      test (steg-kernel) (0.2s)
ok      build (steg-server) (0.1s)
ok      test (steg-server) (9.1s)
ok      test (steg-server, race-fixture) (9.2s)
ok      build (steg) (0.1s)
ok      test (steg) (0.1s)
ok      build (steg-app) (0.3s)
ok      test (steg-app) (0.3s)
ok      tool tests (1.4s)
ok      architecture (1.1s)
ok      recipe names (0.1s)
```

Check passed: `just check` exited 0 at 2026-11-16T01:35:32+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.78 on task-1.78-server-a-forced-close-ends-the-program at 68d10a8032c0:

```text
ok      format check (0.2s)
ok      lint (0.4s)
ok      boundaries (0.1s)
ok      licences (0.7s)
ok      tests (every crate) (9.4s)
ok      build (steg-sdk) (0.2s)
ok      test (steg-sdk) (0.4s)
ok      test (steg-sdk, local-socket) (0.4s)
ok      build (steg-kernel) (0.2s)
ok      test (steg-kernel) (0.3s)
ok      build (steg-server) (0.1s)
ok      test (steg-server) (9.0s)
ok      test (steg-server, race-fixture) (9.2s)
ok      build (steg) (0.1s)
ok      test (steg) (0.1s)
ok      build (steg-app) (0.3s)
ok      test (steg-app) (0.3s)
ok      tool tests (1.3s)
ok      architecture (1.1s)
ok      recipe names (0.1s)
```

Built close-first as planned (d681d6fe44b5) and approved by the engineer. Experiments run first, as the first step: four real-process cases through a scratch harness (silent, continuous-output, trapping, backgrounded-grandchild), and the earlier TASK-1.40/1.76 citation of unix.rs for the no-escalation ChildKiller was corrected to lib.rs (the substance they found was right, the file was wrong).

close_and_dispatch closes the pane/tab/session in the tree first, under one lock acquisition, then runs force_end (signal every pane in scope, wait once against a shared grace window, escalate only the stragglers, join every worker) before answering the caller. This is what criterion 1's reworded order says, and what gives criterion 4's full guarantee - every method, not only a second --force, finds the pane gone the instant the force begins.

Every mechanism's own negative control was run and recorded, failing, before the fix was restored and recorded passing again:
- the idle-handle pop in close_and_dispatch's step 3 disabled -> the silent-program force-close test fails (process stays alive)
- force_end's escalation (pass 3) disabled -> the trapping-program test fails
- AwaitExit's "nothing running" branch no longer breaking the worker loop -> the worker-thread-actually-ends test fails (a real bug this fix catches: a reply can arrive correctly while the thread never ends)
- close_and_dispatch reordered back to force-before-close (the first draft's own shape, lock held across force_end) -> the AC #4 stall test fails, taking ~4.95s instead of returning at once
- the idle-handle fix disabled a second time, checked against both a public black-box test and a new unit test with access to state.panes -> the black-box test still passes (it cannot observe the fix at all, so it was renamed to stop claiming it did), the unit test correctly fails
- force_end's shared deadline replaced with a serial per-pane wait -> the four-trapping-panes test fails at ~2.02s against its 1s bound

Found during the proof pass: a test proving the N-pane shared-grace-window bound asserted only elapsed < 5s against a 30ms test grace, which a serial loop would also satisfy - rewritten with four trapping panes, a real 500ms grace, and a check that every pid is actually dead, not only removed from the tree. Also found: the two-threads-forcing-the-same-pane test proves mutual exclusion via the Mutex, not close-first specifically (both designs give it) - renamed its own claim to say so, not overclaimed.

Checks: just check clean at the final head (68d10a8, matching PR #47's green CI); cargo test -p steg-server 10/10 clean at default parallelism, properly checked for FAILED/error[ anywhere in the output, not only the last test binary's tail.

Named, not fixed: an interactive shell's own job-control reassignment of a backgrounded job to a new process group, which the group signal does not reach (killpg targets the pgid portable-pty's setsid() establishes at exec, which an interactive shell's own job control can move a background job out of afterward); a process stuck in an uninterruptible kernel sleep, where SIGKILL is delivered but delivery is not completion, which no userspace mechanism can force - the caller still gets Response::Ok either way (no error code exists for "could not confirm"), and the worker/relay pair is dropped unjoined if this ever actually happens.

Two items were the engineer's at plan time and were decided then: the AC #1 reorder (approved, criterion reworded to match) and the idle-handle fix (told to include it).

Check FAILED: `cargo test -p steg-server --lib pane::tests::a_stale_generations_own_mark_dead_never_clobbers_a_newer_run` exited 101 at 2026-11-16T02:10:24+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.78 on task-1.78-server-a-forced-close-ends-the-program at 68d10a8032c0 with uncommitted changes:

```text
   Compiling steg-server v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.78/crates/server)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.61s
     Running unittests src/lib.rs (target/debug/deps/steg_server-6d2f1d51fdd2e018)

running 1 test
test pane::tests::a_stale_generations_own_mark_dead_never_clobbers_a_newer_run ... FAILED

failures:

---- pane::tests::a_stale_generations_own_mark_dead_never_clobbers_a_newer_run stdout ----

thread 'pane::tests::a_stale_generations_own_mark_dead_never_clobbers_a_newer_run' (18697434) panicked at crates/server/src/pane.rs:935:9:
assertion `left == right` failed: a stale generation's own mark-dead must never clobber a newer run's live pid
  left: None
 right: Some(200)
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    pane::tests::a_stale_generations_own_mark_dead_never_clobbers_a_newer_run

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 92 filtered out; finished in 0.00s

error: test failed, to rerun pass `-p steg-server --lib`
```

Commits since the review: 0.

Check passed: `cargo test -p steg-server --lib pane::tests::a_stale_generations_own_mark_dead_never_clobbers_a_newer_run` exited 0 at 2026-11-16T02:10:36+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.78 on task-1.78-server-a-forced-close-ends-the-program at 68d10a8032c0 with uncommitted changes:

```text
   Compiling steg-server v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.78/crates/server)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.60s
     Running unittests src/lib.rs (target/debug/deps/steg_server-6d2f1d51fdd2e018)

running 1 test
test pane::tests::a_stale_generations_own_mark_dead_never_clobbers_a_newer_run ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 92 filtered out; finished in 0.00s
```

Commits since the review: 0.

Check passed: `cargo test -p steg-server --test runtime -- a_flooding_program_that_exits_by_itself_mid_grace_is_not_misclassified_as_a_straggler` exited 0 at 2026-11-16T02:12:02+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.78 on task-1.78-server-a-forced-close-ends-the-program at 68d10a8032c0 with uncommitted changes:

```text
   Compiling steg-server v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.78/crates/server)
warning: method `await_dead` is never used
   --> crates/server/src/pane.rs:180:8
    |
136 | impl RunGate {
    | ------------ method in this implementation
...
180 |     fn await_dead(&self, timeout: Duration) -> bool {
    |        ^^^^^^^^^^
    |
    = note: `#[warn(dead_code)]` (part of `#[warn(unused)]`) on by default

warning: `steg-server` (lib) generated 1 warning
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.67s
     Running tests/runtime.rs (target/debug/deps/runtime-636749399b762beb)

running 1 test
test a_flooding_program_that_exits_by_itself_mid_grace_is_not_misclassified_as_a_straggler ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 25 filtered out; finished in 0.01s
```

Commits since the review: 0.

Check FAILED: `cargo test -p steg-server --lib pane::ending_tests::await_exit_is_not_delayed_by_a_backlog_on_the_workers_own_queue -- --nocapture` exited 101 at 2026-11-16T02:14:02+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.78 on task-1.78-server-a-forced-close-ends-the-program at 68d10a8032c0 with uncommitted changes:

```text
   Compiling steg-server v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.78/crates/server)
warning: method `await_dead` is never used
   --> crates/server/src/pane.rs:180:8
    |
136 | impl RunGate {
    | ------------ method in this implementation
...
180 |     fn await_dead(&self, timeout: Duration) -> bool {
    |        ^^^^^^^^^^
    |
    = note: `#[warn(dead_code)]` (part of `#[warn(unused)]`) on by default

warning: `steg-server` (lib test) generated 1 warning
    Finished `test` profile [unoptimized + debuginfo] target(s) in 1.73s
     Running unittests src/lib.rs (target/debug/deps/steg_server-6d2f1d51fdd2e018)

running 1 test

thread 'pane::ending_tests::await_exit_is_not_delayed_by_a_backlog_on_the_workers_own_queue' (18746961) panicked at crates/server/src/pane.rs:1256:9:
settle_worker (3.334µs) must actually be slowed by the same backlog await_exit (2.116375ms) was unaffected by - otherwise this test is not proving the two mechanisms differ at all
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace
test pane::ending_tests::await_exit_is_not_delayed_by_a_backlog_on_the_workers_own_queue ... FAILED

failures:

failures:
    pane::ending_tests::await_exit_is_not_delayed_by_a_backlog_on_the_workers_own_queue

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 93 filtered out; finished in 0.03s

error: test failed, to rerun pass `-p steg-server --lib`
```

Commits since the review: 0.

Check FAILED: `cargo test -p steg-server --lib pane::ending_tests::await_exit_is_not_delayed_by_a_backlog_on_the_workers_own_queue -- --nocapture` exited 101 at 2026-11-16T02:14:53+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.78 on task-1.78-server-a-forced-close-ends-the-program at 68d10a8032c0 with uncommitted changes:

```text
   Compiling steg-server v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.78/crates/server)
error[E0425]: cannot find type `PaneHandle` in this scope
    --> crates/server/src/pane.rs:1235:41
     |
1235 |         fn flooded_handle(pane: u64) -> PaneHandle {
     |                                         ^^^^^^^^^^ not found in this scope
     |
help: consider importing this struct through its public re-export
     |
 953 +     use crate::PaneHandle;
     |

For more information about this error, try `rustc --explain E0425`.
error: could not compile `steg-server` (lib test) due to 1 previous error
```

Commits since the review: 0.

Check FAILED: `cargo test -p steg-server --lib pane::ending_tests::await_exit_is_not_delayed_by_a_backlog_on_the_workers_own_queue -- --nocapture` exited 101 at 2026-11-16T02:15:08+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.78 on task-1.78-server-a-forced-close-ends-the-program at 68d10a8032c0 with uncommitted changes:

```text
   Compiling steg-server v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.78/crates/server)
warning: method `await_dead` is never used
   --> crates/server/src/pane.rs:180:8
    |
136 | impl RunGate {
    | ------------ method in this implementation
...
180 |     fn await_dead(&self, timeout: Duration) -> bool {
    |        ^^^^^^^^^^
    |
    = note: `#[warn(dead_code)]` (part of `#[warn(unused)]`) on by default

warning: `steg-server` (lib test) generated 1 warning
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.60s
     Running unittests src/lib.rs (target/debug/deps/steg_server-6d2f1d51fdd2e018)

running 1 test

thread 'pane::ending_tests::await_exit_is_not_delayed_by_a_backlog_on_the_workers_own_queue' (18767573) panicked at crates/server/src/pane.rs:1274:9:
settle_worker (4.621834ms) must actually be slowed by the same backlog await_exit (4.299791ms) was unaffected by - otherwise this test is not proving the two mechanisms differ at all
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace
test pane::ending_tests::await_exit_is_not_delayed_by_a_backlog_on_the_workers_own_queue ... FAILED

failures:

failures:
    pane::ending_tests::await_exit_is_not_delayed_by_a_backlog_on_the_workers_own_queue

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 93 filtered out; finished in 0.18s

error: test failed, to rerun pass `-p steg-server --lib`
```

Commits since the review: 0.

Check passed: `cargo test -p steg-server --lib pane::ending_tests::await_exit_is_not_delayed_by_a_backlog_on_the_workers_own_queue -- --nocapture` exited 0 at 2026-11-16T02:15:26+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.78 on task-1.78-server-a-forced-close-ends-the-program at 68d10a8032c0 with uncommitted changes:

```text
   Compiling steg-server v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.78/crates/server)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.68s
     Running unittests src/lib.rs (target/debug/deps/steg_server-6d2f1d51fdd2e018)

running 1 test
test pane::ending_tests::await_exit_is_not_delayed_by_a_backlog_on_the_workers_own_queue ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 93 filtered out; finished in 0.16s
```

Commits since the review: 0.

Check passed: `cargo test -p steg-server --test runtime -- every_method_that_can_name_it` exited 0 at 2026-11-16T02:19:03+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.78 on task-1.78-server-a-forced-close-ends-the-program at 68d10a8032c0 with uncommitted changes:

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.08s
     Running tests/runtime.rs (target/debug/deps/runtime-636749399b762beb)

running 3 tests
kill: 81580: No such process
kill: 81578: No such process
test a_session_being_forced_is_unreachable_by_every_method_that_can_name_it ... ok
test a_pane_being_forced_is_unreachable_by_every_method_that_can_name_it ... ok
kill: 81579: No such process
test a_tab_being_forced_is_unreachable_by_every_method_that_can_name_it ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 26 filtered out; finished in 5.05s
```

Commits since the review: 0.

Check FAILED: `cargo test -p steg-server --test runtime -- every_method_that_can_name_it` exited 101 at 2026-11-16T02:19:55+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.78 on task-1.78-server-a-forced-close-ends-the-program at 68d10a8032c0 with uncommitted changes:

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.08s
     Running tests/runtime.rs (target/debug/deps/runtime-636749399b762beb)

running 3 tests
test a_pane_being_forced_is_unreachable_by_every_method_that_can_name_it ... FAILED
test a_tab_being_forced_is_unreachable_by_every_method_that_can_name_it ... FAILED
test a_session_being_forced_is_unreachable_by_every_method_that_can_name_it ... FAILED

failures:

---- a_pane_being_forced_is_unreachable_by_every_method_that_can_name_it stdout ----

thread 'a_pane_being_forced_is_unreachable_by_every_method_that_can_name_it' (18812146) panicked at crates/server/tests/runtime.rs:1074:9:
pane.set must find the pane being forced unreachable, not act on it as if nothing were happening
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace

---- a_tab_being_forced_is_unreachable_by_every_method_that_can_name_it stdout ----

thread 'a_tab_being_forced_is_unreachable_by_every_method_that_can_name_it' (18812148) panicked at crates/server/tests/runtime.rs:1145:9:
tab.set must find the tab being forced unreachable, not act on it as if nothing were happening

---- a_session_being_forced_is_unreachable_by_every_method_that_can_name_it stdout ----

thread 'a_session_being_forced_is_unreachable_by_every_method_that_can_name_it' (18812147) panicked at crates/server/tests/runtime.rs:1217:9:
session.show must find the session being forced unreachable, not act on it as if nothing were happening


failures:
    a_pane_being_forced_is_unreachable_by_every_method_that_can_name_it
    a_session_being_forced_is_unreachable_by_every_method_that_can_name_it
    a_tab_being_forced_is_unreachable_by_every_method_that_can_name_it

test result: FAILED. 0 passed; 3 failed; 0 ignored; 0 measured; 26 filtered out; finished in 0.09s

error: test failed, to rerun pass `-p steg-server --test runtime`
```

Commits since the review: 0.

Check passed: `cargo test -p steg-server --test runtime -- every_method_that_can_name_it` exited 0 at 2026-11-16T02:20:12+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.78 on task-1.78-server-a-forced-close-ends-the-program at 68d10a8032c0 with uncommitted changes:

```text
   Compiling steg-server v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.78/crates/server)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.70s
     Running tests/runtime.rs (target/debug/deps/runtime-636749399b762beb)

running 3 tests
kill: 83887: No such process
test a_pane_being_forced_is_unreachable_by_every_method_that_can_name_it ... ok
kill: 83886: No such process
test a_session_being_forced_is_unreachable_by_every_method_that_can_name_it ... ok
kill: 83888: No such process
test a_tab_being_forced_is_unreachable_by_every_method_that_can_name_it ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 26 filtered out; finished in 5.05s
```

Commits since the review: 0.

Check passed: `cargo test -p steg-server` exited 0 at 2026-11-16T02:20:57+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.78 on task-1.78-server-a-forced-close-ends-the-program at 68d10a8032c0 with uncommitted changes, last 40 of 293 lines:

```text
kill: 87842: No such process
kill: 87849: No such process
test tab_close_with_force_ends_every_program_in_scope_within_one_grace_window ... ok
kill: 87788: No such process
test a_pane_being_forced_is_unreachable_by_every_method_that_can_name_it ... ok
kill: 87795: No such process
test a_session_being_forced_is_unreachable_by_every_method_that_can_name_it ... ok
kill: 87792: No such process
kill: 87798: No such process
test a_tab_being_forced_is_unreachable_by_every_method_that_can_name_it ... ok
test a_pane_being_forced_is_unreachable_by_another_request_meanwhile ... ok

test result: ok. 29 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 5.08s

     Running tests/shutdown.rs (target/debug/deps/shutdown-641947b6111d1ffd)

running 8 tests
test a_stalled_handshake_is_closed_and_run_returns_promptly_on_stop ... ok
test stopping_closes_the_listener_removes_the_socket_and_releases_the_lock ... ok
test stop_on_a_genuinely_idle_server_returns_from_run_within_a_bound ... ok
test stop_called_twice_concurrently_on_an_idle_server_is_harmless ... ok
test a_completed_idle_connection_is_closed_and_run_returns_promptly_on_stop ... ok
test many_short_connections_do_not_grow_the_tracked_set ... ok
test stopping_while_a_trickling_handshake_is_in_progress_still_returns_within_a_bound ... ok
test stop_racing_a_client_connecting_still_returns_within_a_bound ... ok

test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.13s

     Running tests/socket_mode.rs (target/debug/deps/socket_mode-e7b594c955bacef5)

running 1 test
test the_socket_is_0600_even_under_a_permissive_umask ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests steg_server

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

Commits since the review: 0.

Check passed: `cargo test -p steg-server -p steg-kernel -p steg` exited 0 at 2026-11-16T02:25:46+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.78 on task-1.78-server-a-forced-close-ends-the-program at c7c751c59813 with uncommitted changes, last 40 of 645 lines:

```text
test a_pane_being_forced_is_unreachable_by_another_request_meanwhile ... ok
kill: 7562: No such process
test a_tab_being_forced_is_unreachable_by_every_method_that_can_name_it ... ok
kill: 7572: No such process
test a_session_being_forced_is_unreachable_by_every_method_that_can_name_it ... ok

test result: ok. 29 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 5.08s

     Running tests/shutdown.rs (target/debug/deps/shutdown-3e485a295dc08c8a)

running 8 tests
test a_stalled_handshake_is_closed_and_run_returns_promptly_on_stop ... ok
test stopping_closes_the_listener_removes_the_socket_and_releases_the_lock ... ok
test stop_on_a_genuinely_idle_server_returns_from_run_within_a_bound ... ok
test stop_called_twice_concurrently_on_an_idle_server_is_harmless ... ok
test a_completed_idle_connection_is_closed_and_run_returns_promptly_on_stop ... ok
test many_short_connections_do_not_grow_the_tracked_set ... ok
test stopping_while_a_trickling_handshake_is_in_progress_still_returns_within_a_bound ... ok
test stop_racing_a_client_connecting_still_returns_within_a_bound ... ok

test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.13s

     Running tests/socket_mode.rs (target/debug/deps/socket_mode-794b69be98f698ad)

running 1 test
test the_socket_is_0600_even_under_a_permissive_umask ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests steg_kernel

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests steg_server

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

Commits since the review: 1, all with the Asdlc-Worker trailer.

Check passed: `just check` exited 0 at 2026-11-16T02:28:51+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.78 on task-1.78-server-a-forced-close-ends-the-program at d1cd52545d59 with uncommitted changes:

```text
ok      format check (0.2s)
ok      lint (0.4s)
ok      boundaries (0.1s)
ok      licences (0.7s)
ok      tests (every crate) (9.8s)
ok      build (steg-sdk) (0.1s)
ok      test (steg-sdk) (0.2s)
ok      test (steg-sdk, local-socket) (0.4s)
ok      build (steg-kernel) (0.1s)
ok      test (steg-kernel) (0.2s)
ok      build (steg-server) (0.1s)
ok      test (steg-server) (8.9s)
ok      test (steg-server, race-fixture) (9.1s)
ok      build (steg) (0.1s)
ok      test (steg) (0.1s)
ok      test (steg, fixtures) (2.4s)
ok      build (steg-ui) (0.3s)
ok      test (steg-ui) (0.4s)
ok      build (steg-app) (0.3s)
ok      test (steg-app) (0.3s)
ok      tool tests (1.2s)
ok      architecture (1.0s)
ok      recipe names (0.0s)
```

Commits since the review: 96, 85 without the Asdlc-Worker trailer.

Check passed: `sh -c 'for i in $(seq 1 20); do cargo test -p steg-server 2>&1 | grep -E "test result|FAILED" || exit 1; done; echo ALL_20_RUNS_CLEAN'` exited 0 at 2026-11-16T02:36:16+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.78 on task-1.78-server-a-forced-close-ends-the-program at a4fb5d0b6a6d, last 40 of 181 lines:

```text
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.13s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 102 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.03s
test result: ok. 61 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.80s
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.97s
test result: ok. 29 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 5.09s
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.13s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 102 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.04s
test result: ok. 61 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.78s
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.77s
test result: ok. 29 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 5.07s
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.13s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 102 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.04s
test result: ok. 61 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.74s
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.77s
test result: ok. 29 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 5.07s
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.13s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 102 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.04s
test result: ok. 61 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.77s
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.86s
test result: ok. 29 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 5.08s
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.13s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
ALL_20_RUNS_CLEAN
```

Commits since the review: 97, 85 without the Asdlc-Worker trailer.

Check FAILED: `just check` exited 143 at 2026-11-16T02:43:31+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.78 on task-1.78-server-a-forced-close-ends-the-program at a4fb5d0b6a6d:

```text
ok      format check (0.2s)
ok      lint (1.3s)
ok      boundaries (0.1s)
ok      licences (1.0s)
ok      tests (every crate) (14.0s)
ok      build (steg-sdk) (0.4s)
ok      test (steg-sdk) (2.2s)
ok      test (steg-sdk, local-socket) (1.7s)
ok      build (steg-kernel) (0.2s)
ok      test (steg-kernel) (0.7s)
ok      build (steg-server) (34.7s)
ok      test (steg-server) (13.8s)
error: recipe `check-packages` failed with exit code 143
```

Commits since the review: 0.

Check passed: `just check` exited 0 at 2026-11-16T02:44:29+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.78 on task-1.78-server-a-forced-close-ends-the-program at a4fb5d0b6a6d:

```text
ok      format check (0.2s)
ok      lint (1.0s)
ok      boundaries (0.1s)
ok      licences (1.1s)
ok      tests (every crate) (13.1s)
ok      build (steg-sdk) (0.1s)
ok      test (steg-sdk) (0.7s)
ok      test (steg-sdk, local-socket) (1.0s)
ok      build (steg-kernel) (0.1s)
ok      test (steg-kernel) (0.4s)
ok      build (steg-server) (0.1s)
ok      test (steg-server) (9.6s)
ok      test (steg-server, race-fixture) (10.1s)
ok      build (steg) (0.3s)
ok      test (steg) (0.2s)
ok      test (steg, fixtures) (3.2s)
ok      build (steg-ui) (0.3s)
ok      test (steg-ui) (0.6s)
ok      build (steg-app) (0.3s)
ok      test (steg-app) (0.6s)
ok      tool tests (1.5s)
ok      architecture (1.2s)
ok      recipe names (0.1s)
```

Commits since the review: 0.

Review findings fixed. Leading with the headline fix: a signal can no longer reach a reaped pid - the pid-clearing that marks a run dead now happens atomically with the reap itself (RunGate: Mutex<GateState> + Condvar, generation-tracked), done only by the relay thread, under the same lock a signaller must hold to send. Previously this happened when the worker dequeued Exited - the same queue a flooding program's PtyBytes could back up arbitrarily, so a signal could in theory reach an already-reaped pid.

Finding 1 (signal-to-reaped-pid race): replaced Arc<AtomicU64> pid with RunGate. The relay thread polls child.try_wait() under RunGate's lock (never a blocking wait() outside it); reap and pid-clear happen in one critical section. nix's waitid/WNOWAIT (observe without reaping) is not available on this project's macOS target - confirmed by reading sys/wait.rs's own cfg-gate directly before committing to try_wait-under-lock as the mechanism, not an alternative narrowing the window. join_worker split into settle_worker (&self, parallel-safe) + join_worker (consumes, sequential) so force_end settles every handle concurrently against one shared bound without reintroducing N-pane serial cost.

Tests added for finding 1, each with a negative control run and reverted:
- pane::tests::a_stale_generations_own_mark_dead_never_clobbers_a_newer_run - a direct unit test of RunGate::clear_if_current (shared by spawn_relay's two reap branches), not only an end-to-end test.
- pane::ending_tests::await_exit_is_not_delayed_by_a_backlog_on_the_workers_own_queue - two separate handles, a real 1,000,000-item backlog, proving await_exit (RunGate-based) is unaffected while settle_worker (still queue-based by design) is measurably slower.
- a_flooding_program_that_exits_by_itself_mid_grace_is_not_misclassified_as_a_straggler (tests/runtime.rs).
- pane::ending_tests::restarting_then_forcing_ends_the_new_program_not_the_old_one - the Exited-arm pid-clear nit (restart a command pane, then force it: the new program is ended).

Note named in the notes, not hidden: an end-to-end attempt to reproduce the misclassification via a real flooding shell program through a real pty did not reliably show a measurable delay under the reverted mechanism, because the kernel's own pty buffer throttles the writer before the backlog this bug needs can build up. The deterministic channel-flooding test above is what actually discriminates the two mechanisms; the end-to-end test is still kept as a realistic regression check.

Finding 2 (AC #4 full-method coverage): added a_pane_being_forced_is_unreachable_by_every_method_that_can_name_it and the tab/session equivalents in tests/runtime.rs, covering every other method that can name a pane/tab/session being forced (pane.close plain/swap/move/set/unset/focus; tab.close plain/move/set/unset/focus; session.close plain/show/move/set/unset). Methods not implemented yet are excluded. Negative control run by hand: reversed close_and_dispatch back to force-before-close (the rejected first draft's order) - all three new tests failed as predicted, reverted, not committed as a toggle.

Nits: (1) the stale 10/10 run re-recorded as 20/20 clean at the final head, no leaked process; (2) any_pane_running's documented "pane id that cannot be found" branch now has a direct test; (3) the duplicated panes_in_close_scope call is left as-is with reasoning posted on the PR and here - dispatch::dispatch must stay correct when called directly (tests/dispatch.rs bypasses Runtime entirely), so the refusal check can't be removed from the three close arms without breaking that invariant, and the cost is a tree walk over a handful of panes, not a stated performance target.

Merged origin/main: brought in TASK-1.45/1.77/1.84's own server changes (RunGate's module doc, pane.rs's own imports, boundaries.toml/Cargo.toml's nix feature list) plus unrelated crates/ui, tools, and backlog changes. Resolved conflicts in boundaries.toml, crates/server/Cargo.toml (merged both nix feature reasons - setsid for TASK-1.77, killpg/Signal for this task), and crates/server/src/pane.rs (merged both import lists). Fixed origin/main's own new runtime.rs tests (TASK-1.84) that used PaneHandle::stop() and the 2-arg Runtime::new, both since replaced by this task.

Checks recorded: cargo test -p steg-server --lib (targeted generation-guard and backlog-comparison tests, each with its negative control); cargo test -p steg-server (full suite, clean after the merge); cargo test -p steg-server -p steg-kernel -p steg; just check (clean, twice - one transient exit 143 from resource contention with other concurrent workers on this shared machine, re-ran clean); the server suite 20 times at the final head (a4fb5d0), 0 failures, no leaked process. CI green at a4fb5d0 (gh pr checks 47, watched in the foreground). Review response posted to the PR as a comment.

Check FAILED: `cargo test -p steg-server --lib pane::tests::the_reap_poll_interval_doubles_then_holds_at_its_cap` exited 101 at 2026-11-16T03:05:52+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.78 on task-1.78-server-a-forced-close-ends-the-program at a4fb5d0b6a6d with uncommitted changes:

```text
   Compiling steg-server v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.78/crates/server)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.62s
     Running unittests src/lib.rs (target/debug/deps/steg_server-6d2f1d51fdd2e018)

running 1 test
test pane::tests::the_reap_poll_interval_doubles_then_holds_at_its_cap ... FAILED

failures:

---- pane::tests::the_reap_poll_interval_doubles_then_holds_at_its_cap stdout ----

thread 'pane::tests::the_reap_poll_interval_doubles_then_holds_at_its_cap' (20108901) panicked at crates/server/src/pane.rs:1031:9:
assertion `left == right` failed: the interval must double each time until it reaches the cap, then hold there - never exceed it, never reset
  left: [5ms, 5ms, 5ms, 5ms, 5ms, 5ms, 5ms, 5ms, 5ms]
 right: [5ms, 10ms, 20ms, 40ms, 80ms, 100ms, 100ms, 100ms, 100ms]
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    pane::tests::the_reap_poll_interval_doubles_then_holds_at_its_cap

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 102 filtered out; finished in 0.00s

error: test failed, to rerun pass `-p steg-server --lib`
```

Commits since the review: 0.

Check passed: `cargo test -p steg-server --lib pane::tests::the_reap_poll_interval_doubles_then_holds_at_its_cap` exited 0 at 2026-11-16T03:06:03+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.78 on task-1.78-server-a-forced-close-ends-the-program at a4fb5d0b6a6d with uncommitted changes:

```text
   Compiling steg-server v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.78/crates/server)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.61s
     Running unittests src/lib.rs (target/debug/deps/steg_server-6d2f1d51fdd2e018)

running 1 test
test pane::tests::the_reap_poll_interval_doubles_then_holds_at_its_cap ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 102 filtered out; finished in 0.00s
```

Commits since the review: 0.

Check FAILED: `cargo test -p steg-server --test runtime -- a_pane_being_forced_is_unreachable_by_every_method_that_can_name_it` exited 101 at 2026-11-16T03:06:47+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.78 on task-1.78-server-a-forced-close-ends-the-program at a4fb5d0b6a6d with uncommitted changes:

```text
   Compiling steg-server v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.78/crates/server)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.84s
     Running tests/runtime.rs (target/debug/deps/runtime-636749399b762beb)

running 1 test
test a_pane_being_forced_is_unreachable_by_every_method_that_can_name_it ... FAILED

failures:

---- a_pane_being_forced_is_unreachable_by_every_method_that_can_name_it stdout ----

thread 'a_pane_being_forced_is_unreachable_by_every_method_that_can_name_it' (20139811) panicked at crates/server/tests/runtime.rs:1091:9:
pane.split must find the pane being forced unreachable, not act on it as if nothing were happening
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    a_pane_being_forced_is_unreachable_by_every_method_that_can_name_it

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 28 filtered out; finished in 0.09s

error: test failed, to rerun pass `-p steg-server --test runtime`
```

Commits since the review: 0.

Check passed: `cargo test -p steg-server --test runtime -- a_pane_being_forced_is_unreachable_by_every_method_that_can_name_it` exited 0 at 2026-11-16T03:07:04+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.78 on task-1.78-server-a-forced-close-ends-the-program at a4fb5d0b6a6d with uncommitted changes:

```text
   Compiling steg-server v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.78/crates/server)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.81s
     Running tests/runtime.rs (target/debug/deps/runtime-636749399b762beb)

running 1 test
kill: 65152: No such process
test a_pane_being_forced_is_unreachable_by_every_method_that_can_name_it ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 28 filtered out; finished in 5.05s
```

Commits since the review: 0.
<!-- SECTION:NOTES:END -->

## Comments

<!-- COMMENTS:BEGIN -->
author: @bruk
created: 2026-11-16T00:24:00Z

---

Plan approved by @bruk (d681d6fe44b5, size-3): its worker may build it.

---

created: 2026-11-16T00:24:00Z

---

Criterion 1 reworded by @bruk.o on 2026-11-15 to the order the approved plan (d681d6fe44b5) builds: close first, then end the programs, answering when they have ended. The engineer approved the plan with this change stated at its top; the earlier wording was 'end every running program in scope, observe each exit, and then close'. The other four criteria are unchanged.

---

author: @bruk.r1
created: 2026-11-16T01:36:00Z

---

@bruk.r1 is reviewing TASK-1.78: a same-machine review: it waited 0m with no other engineer's reviewer, longer than review_fallback_after (0m), so a fresh reviewer on its author's machine takes it.

---

author: @bruk.r1
created: 2026-11-16T01:52:00Z

---

Review by @bruk.r1 (same-machine): changes requested. Back to In Progress for the author's worker.

Findings: 2 Important, 3 nit. Cites: review. The diff does not match the plan.

Reviewed at PR #47 head 68d10a8032c024598469b6c9d3524383a433fe76 (just check clean at this oid, 01:31/01:35 UTC on a clean tree; PR CI green on the same oid).

[Bugs/Compliance, Important] crates/server/src/runtime.rs (force_end, pass 3) + crates/server/src/pane.rs (signal_hangup/signal_kill/Exited). The plan (§1) defends the direct-atomic signal design as carrying only "an existing, unchanged risk class" (a signal reaching a reused pid after reap) and states elsewhere "nothing in this task makes the window wider" - both unsupported, and the first points at the wrong window (a zombie's pid is not yet reusable; reuse needs the reap, which already happened by the time child.wait() returns in spawn_relay). In the pre-task code the signal (killer.kill()) was itself a command on the same single-consumer queue as Exited, so by the time the worker dequeued Stop, any Exited enqueued ahead of it had already cleared `current` - no window. This task's own design deliberately takes the signal off that queue (§1's own stated reason: not to be delayed behind a flooding pane's PtyBytes backlog) via signal_hangup/signal_kill reading `pid: Arc<AtomicU64>` directly, while the atomic itself is still only cleared when the worker *dequeues* the Exited command - i.e. still behind that same backlog. The result: force_end's pass 2 (await_exit, which does go through the queue) can time out and classify a pane as a "straggler" purely because its Exited is still queued behind backlog, even though the program already exited. Pass 3 then fires signal_kill (killpg) at the stale pid - at minimum a wasted SIGKILL plus a pointless 2s wait in pass 4 for an exit that already happened (no test or assertion catches this: the straggler/no-straggler tests use grace windows wide enough that a real backlog-timeout misclassification doesn't come up), and in the worst case (pid already recycled by the OS in that window) a signal to an unrelated process's whole group - remote on this machine's pid space, but not the "unchanged" risk the notes claim. Repeated myself as a negative control in a scratch edit (reverted): with pass 3's signal_kill call disabled, pane_close_with_force_ends_a_program_that_traps_the_polite_signal fails exactly as expected, confirming escalation is the real, load-bearing mechanism the notes say it is - the finding here is the residual-risk characterization and the misclassification path, not that escalation itself is fake.

[Evidence/Compliance, Important] crates/server/tests/runtime.rs - AC #4's "every method, not only a second --force" guarantee (plan §6, §9) is not actually proven. The plan's own §9 text: "A second thread issues pane.show (then, separately, a plain pane.close, then pane.swap) against the same pane that is being forced, each asserting it is refused..." - three methods in one promised test. What is committed: a_pane_being_forced_is_unreachable_by_another_request_meanwhile tests pane.show only; a_force_racing_another_force_on_the_same_pane_never_double_acts tests a second pane.close --force, and its own comment concedes this proves mutual exclusion via the Mutex, not close-first specifically. pane.swap and a plain (non-force) pane.close against a pane mid-force are never exercised; neither are pane.move or pane.set/unset, though dispatch/pane.rs fully implements all four (move_/set/unset/swap, not stubs) and §6 names them explicitly as covered by the restated guarantee. Per policy, a clause the plan/brief names as worth proving needs a test that reaches it, and a test of one method does not cover another; this is an unmet clause, and an unnamed departure from the plan's own proof section (the notes record the renamed double-force test but say nothing about the vanished swap/plain-close coverage).

[Evidence, Nit] Notes' "cargo test -p steg-server 10/10 clean at default parallelism" is recorded at 61369d880a6d with uncommitted changes, not at the final head 68d10a8 the plan's own "Repeated-run honesty" section calls for. Re-ran it myself: 20/20 clean at 68d10a8, default parallelism, no leaked processes before/after (ps count unchanged save for this harness's own unrelated background polling) - the substance holds, only the recorded run was stale.

[Bugs, Nit] pane.rs worker_loop's PaneCommand::Exited arm clears the shared pid atomic unconditionally, with no check that the exiting run is still the current one; PaneHandle::restart writes the new pid before sending Resume but the Resume arm does nothing to re-assert it. An Exited for an old run processed after a Resume for a new one would zero the new run's pid permanently (silent force thereafter, ~6s stall, leaked worker/relay). Not currently reachable: no dispatch path calls restart yet (kernel ExitOutcome has only Closed/Stayed, no restart variant), and the only two existing callers (program.rs, real_pty.rs) call restart only after observing the external PaneEvent::Exited, which structurally cannot happen before the worker has already cleared the atomic. Worth a guard or a comment before restart is wired to dispatch.

[Bugs, Nit] dispatch/{pane,tab,session}.rs close() arms call panes_in_close_scope a second time (once here for the refusal check, once again inside Runtime::close_and_dispatch) - no behaviour difference, a small duplicated tree walk per close.

One more nit not detailed: dispatch/lookup.rs's panes_in_close_scope silently treats an unreachable pane id as "not running" in any_pane_running rather than surfacing it, documented as "never expected" - fine given the doc, flagged only because no test exercises that branch either.

Negative controls repeated by hand, reverted: (1) force_end pass 3's signal_kill call disabled -> pane_close_with_force_ends_a_program_that_traps_the_polite_signal fails by its bound, as recorded; (2) close_and_dispatch's idle-handle pop (step 3) disabled -> pane_close_with_force_actually_ends_a_silent_program fails (process stays alive), as recorded.

Checked and clean: no unsafe anywhere in the diff; nix's "signal" feature is listed in boundaries.toml's [steg-server] allow/reason and Cargo.toml, matching plan §8; no crates/kernel or crates/sdk files touched; files touched match the plan's own "Files that change" list exactly.

Overlap: TASK-1.84 (PR #46, in review) adds two apply_event_locked arms for command marks that "deliberately leave the handle in place" on PaneNotFound - that assumption is about to become moot once this merges, since close_and_dispatch now pops every scope pane's handle synchronously at close time (both forced and plain closes), before any mark or exit event for it can arrive; there will be no handle left in state.panes to leave in place. TASK-1.84 will need to revisit that comment/behaviour against this task's new reality, though it is not this task's own bug. TASK-1.45 (not yet in review) constructs Runtime::new and calls stop() explicitly; it will need to pass a force_grace argument (FORCE_GRACE_DEFAULT for production) and should expect stop() to take up to ~6s worst case now rather than up to 5s per pane serially.

---

author: @bruk.r2
created: 2026-11-16T02:46:00Z

---

@bruk.r2 is reviewing TASK-1.78: a same-machine review: it waited 1m with no other engineer's reviewer, longer than review_fallback_after (0m), so a fresh reviewer on its author's machine takes it.

---

author: @bruk.r2
created: 2026-11-16T02:59:00Z

---

Review by @bruk.r2 (same-machine): changes requested. Back to In Progress for the author's worker.

Findings: 1 Important, 2 nit. Cites: review, testing. The diff does not match the plan.

Second review (same-machine) at PR #47 head a4fb5d0b6a6d45024da43fad01d9413e537de316 (matches gh pr view 47; just check clean at this head per the task's own recorded run at 2026-11-16T02:44:29Z; PR CI green on the same head via gh pr checks 47).

All five of the first review's findings were genuinely fixed: the reaped-pid race is closed by construction (RunGate: Mutex<GateState>+Condvar, generation-tracked; spawn_relay polls child.try_wait() while holding the lock, so the reap and the pid-clear happen in the same critical section a signaller must also hold before it can read/send - traced in pane.rs and confirmed no unsafe is introduced, grep clean); the AC #4 unreachability sweep now covers pane.close(plain)/swap/move/set/unset/focus, tab.close(plain)/move/set/unset/focus and session.close(plain)/show/move/set/unset, closing the swap/plain-close gap the first review found; the stale-check-timestamp nit is moot (checks now recorded at the final head); the duplicated panes_in_close_scope call is accepted with posted reasoning, unchanged.

[Evidence/Compliance, Important] crates/server/src/pane.rs, crates/server/tests/runtime.rs - two of the four tests the notes list under "Tests added for finding 1, each with a negative control run and reverted" do not actually discriminate the mechanism they are cited as proof of, and I confirmed this myself by repeating each control in a scratch edit (reverted):
- pane::ending_tests::restarting_then_forcing_ends_the_new_program_not_the_old_one: cited as proof of the fix for review 1's "Exited-arm pid-clear" nit (GateState::clear_if_current's generation guard). No check for this test is recorded on the task at all (I searched the full history - only bundled runs). I removed the generation check (clear_if_current unconditionally clearing pid) and reran: the test still passes. It cannot reach the race - it restarts only after observing the first run's own real exit, by which point that run's relay thread has already cleared generation 1 under RunGate's lock, leaving nothing stale to clobber generation 2.
- a_flooding_program_that_exits_by_itself_mid_grace_is_not_misclassified_as_a_straggler (tests/runtime.rs): recorded passing, no recorded failing run. I routed PaneHandle::await_exit back through the queue-based settle_worker (the pre-fix, finding-1 mechanism) and reran: the test still passes, because the pty's own kernel buffer throttles the shell's echo loop before the program exits, so the worker's backlog has typically already drained by the time pane.close --force runs - exactly what the notes' own later paragraph concedes ("did not reliably show a measurable delay... because the kernel's own pty buffer throttles the writer"), which sits uneasily with the blanket claim just above it that this test had an effective negative control.
Both underlying mechanisms ARE proven correctly elsewhere, with real recorded negative controls I also reproduced myself: pane::tests::a_stale_generations_own_mark_dead_never_clobbers_a_newer_run (fails when I remove the generation guard) and pane::ending_tests::await_exit_is_not_delayed_by_a_backlog_on_the_workers_own_queue (recorded FAILED then fixed). This is an evidence-claim defect, not a broken fix - but per policy a test that still passes with its mechanism removed, confirmed by the reviewer, is Important, and the notes' claim that every listed test had an effective control is false for two of the four.

Negative controls I repeated myself, all reverted: (1) force_end pass 3 (signal_kill) disabled -> pane_close_with_force_ends_a_program_that_traps_the_polite_signal fails by its bound, as recorded - escalation still holds. (2) GateState::clear_if_current's generation guard removed -> a_stale_generations_own_mark_dead_never_clobbers_a_newer_run fails as recorded (mechanism real); restarting_then_forcing_ends_the_new_program_not_the_old_one still passes (the finding above). (3) PaneHandle::await_exit routed through settle_worker instead of RunGate -> a_flooding_program_...straggler still passes (the finding above) - the queue-routed wait this would reintroduce is not what that test actually catches.

[Compliance, Nit] crates/server/src/runtime.rs (apply_event_locked, CommandOutputStarted/CommandEnded arms) + their own test. TASK-1.84's "handle stays" comment/test, merged into this branch, argues a mark's PaneNotFound must not remove the handle because "the program it owns may still be running" - exactly the scenario the first review's own "Overlap" note flagged as about to become moot once close_and_dispatch (this task) pops every scope pane's handle out of state.panes synchronously, under the same lock as the close, before any such event can arrive. After the merge, this scenario no longer appears reachable via any real production path. The notes for the merge only say TASK-1.84's tests were "fixed... to compile" against the new 3-arg Runtime::new/no PaneHandle::stop - the comment and test were not revisited against the new reality, even though the resulting no-op is harmless. A decision forced by a merge that the notes don't mention is a finding per policy even when the result is not wrong.

[Bugs, Nit] tests/runtime.rs's new "every method that can name it" sweep is otherwise thorough (pane.close/swap/move/set/unset/focus; tab.close/move/set/unset/focus; session.close/show/move/set/unset; pane.show covered separately by the stall test) but omits pane.split, which also takes an existing pane as its target to split from and is fully implemented (dispatch/pane.rs). The same lock-and-tree-lookup mechanism that protects the tested methods likely protects this one too, but it is unverified.

Checked and sound, not repeated as controls beyond the above: shared N-pane deadline (tab_close_with_force_ends_every_program_in_scope_within_one_grace_window, with its own recorded negative control); the idle-handle fix (recorded negative control); no unsafe anywhere in the diff (grep confirmed); a restart gives the new run its own generation (traced in pane::start/restart and RunGate::begin_run); TASK-1.77's api.call entry point and TASK-1.45's constructor signature are both reached correctly from the merge. Process leak check: ran cargo test -p steg-server 5 times myself (ps count for sleep/steg processes before and after: 2 -> 1, no growth) - consistent with, though fewer runs than, the task's own recorded 20/20-clean, no-leaked-process run at this head; this machine runs many concurrent agents, so ps counts alone are noisy corroboration, not independent proof.

plan-matched: no, as review 1 also recorded - RunGate replaces the plan's own Arc<AtomicU64>, force_end gained passes 5/6, and join_worker was split into settle_worker+join_worker, all explained in the notes as the review 1 fix, not findings on their own, but a real departure from the plan's own §1/§2 text.

---
<!-- COMMENTS:END -->
