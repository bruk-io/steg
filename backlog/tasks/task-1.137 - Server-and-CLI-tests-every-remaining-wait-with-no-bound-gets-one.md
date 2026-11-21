---
id: TASK-1.137
title: 'Server and CLI tests: every remaining wait with no bound gets one'
status: To Do
assignee: []
created_date: '2026-11-18T14:03:00Z'
updated_date: '2026-11-21T20:19:00Z'
labels:
  - size-3
milestone: m-3
dependencies:
  - TASK-1.123
parent_task_id: TASK-1
project: bruk-io-steg
asdlc_accepted_from: DRAFT-1
asdlc_branch: task-1.137-server-and-cli-tests-every-remaining
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Split from TASK-1.123 by its plan on 2026-11-18. TASK-1.113's sweep counted about 60 waits with no bound; TASK-1.123's plan re-counted against today's tree and found 150 to 200 call sites of join, recv, child wait and socket reads, most of which already have a timeout. The plan's proposed reading, for the engineer to confirm: unbounded waits get the helper; already-bounded waits are touched only where their failure message does not name the wait. Whether the fixture binary crates/server/tests/fixtures/accept_backoff_attempt.rs is in scope, and whether crates/sdk's tests are, are open questions carried from that plan. TASK-1.123's plan, sections 1 and 4, holds the inventory and the sleep list as a starting point.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Every wait on a thread, a channel, a child process or a socket in the tests of crates/server and crates/steg that has no bound today gets one through the shared helpers, and fails with a message naming what it waited for; the plan gives the inventory by file with counts and how it was found
- [ ] #2 A wait that already has a bound is left as it is unless its failure does not say what it waited for; the plan lists which those are and the engineer confirms how far to go before the build
- [ ] #3 No test waits by sleeping in a loop where an event can be waited on; each remaining poll and each fixed sleep is listed in the notes with the reason no event exists, including the 100 ms sleep in crates/sdk/src/local_socket.rs's test fixture
- [ ] #4 The files TASK-1.88 and TASK-1.90 are editing are done last and re-checked against main first; just check passes
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
## 0. Scope correction found while reading (AC #4's own premise is stale)

AC #4 names TASK-1.88 and TASK-1.90 as the files to sequence around. Both are now Done
and merged (TASK-1.88: PR #82, TASK-1.90: PR #80, both merged 2026-11-18/20). The live
overlap today is TASK-1.140 ("Server: wait for a pane's program to exit, with a
timeout", In Progress, @bruk.w2), per the brief: it adds crates/server/tests for pane
wait plus edits to crates/server/src/dispatch/pane.rs and
crates/server/tests/dispatch/cross_cutting.rs. Read this plan's own AC #4 as "the files
TASK-1.140 is editing", not TASK-1.88/1.90 - open question #1 below asks the engineer
to confirm that reading in the approval.

## 1. The live flake - diagnosis, split in two because the two classes are at different
   stages of being understood

### 1a. login_env's backgrounded-grandchild test - root cause found by reading, not a guess

File: crates/server/src/login_env.rs, test
`capture_login_environment_kills_a_backgrounded_grandchild_that_held_the_pipe_open`
(line 617), and the function it drives, `capture_login_environment` (line 199).

The race is not in either bound TASK-1.123 widened (the outer 15s liveness assert, the
inner 30s pid-file-appearance poll). It is in the one bound TASK-1.123 did not touch:
the 1-second functional `timeout` the test itself passes to
`capture_login_environment`. Reading the fixture script and the function together:

- The fixture is `sleep 100 & echo $! > <pidfile>; exit 0` - the fork happens before
  the pid is written to the file.
- The backgrounded `sleep` inherits the stdout pipe, so the reader thread's
  `read_to_end` never sees EOF; `rx.recv_timeout(timeout)` in
  `capture_login_environment` is therefore guaranteed to fire for any finite `timeout`
  - this is not a maybe, it is the function's whole documented purpose (its own doc
    comment: "a backgrounded grandchild... is the reason this kills the group").
- On that timeout, `capture_login_environment` runs `kill -9 -<pgid>` against the
  shell's whole process group - immediately, with no further wait for the shell script
  itself to have progressed.
- Under ordinary (idle-machine) scheduling, 1 second is enough wall-clock time for the
  shell to fork `sleep 100 &` and execute `echo $! > <pidfile>` before that timeout
  fires. Under heavy CPU oversubscription (several worktrees compiling and testing at
  once, load average 4 to 19 per the task's own notes), fork/exec and the shell's own
  script-line scheduling can be delayed past 1 second, so the kill can land *before*
  the shell has written the pid file at all.
- When that happens, the test's own 30-second poll for the pid file is waiting for a
  file that will now never be written - it was never a timing margin to begin with, it
  was asserting on a `sh` process the parent just killed mid-script. The eventual
  failure message, "the backgrounded grandchild's own pid file never appeared", is
  exactly this outcome, and matches TASK-1.123's own recorded sightings precisely.

This also explains why CI stays green throughout: CI runs this test alone, so even
under its own ordinary scheduling noise, 1 second of wall-clock time is essentially
always enough; only concurrent local load pushes fork/exec latency past it.

Why this reading, not "the bound is still too tight for a different reason": the
sibling test `capture_login_environment_falls_back_on_a_shell_that_never_finishes_in_
time` (line 526) passes the *same* function a 200ms timeout against a shell that only
ever sleeps, and only asserts `Fallback(TimedOut)` - it does not touch the pid-file
mechanism at all, so widening *this* test's 1s timeout does not loosen what that
sibling test proves; the two tests are independent.

**Fix**: raise `capture_login_environment_kills_a_backgrounded_grandchild_that_held_
the_pipe_open`'s own functional timeout argument from `Duration::from_secs(1)` to a
value with real margin against fork/exec scheduling delay under contention - proposed
5s (open question #2: the brief's own file still calls this "close to its own 1s
functional timeout" in a comment that would need rewriting either way). The outer 15s
liveness assert and the 30s pid-file poll are untouched - they were never the problem,
and this task's own acceptance criteria do not ask to re-tune bounds that are not
broken.

**Control, forced and deterministic, not load-dependent** (the same "force the
mechanism, don't wait for the kernel" standard TASK-1.123 used for its own class (e)):
in a scratch, uncommitted copy of the fixture script, insert a deterministic delay
between the fork and the pid-file write - `sleep 100 & sleep 2; echo $! > <pidfile>;
exit 0`. Against the current 1s timeout, the kill now provably lands before the write
- this fails every single time, not flakily (recorded: N/N failures, all with "the
backgrounded grandchild's own pid file never appeared"). Against the raised bound
(5s, comfortably past the 2s artificial delay), the identical script passes cleanly
(recorded: N/N passes). This is a real, forced, recorded failing-then-passing pair at
the final head, stronger than a load-dependent reproduction - revert the scratch
script change before building the real fix.

### 1b. attach.rs's stuck-pane/flood tests - cannot be fully diagnosed by reading alone; says so plainly

Reading establishes two structural facts, but not yet which specific tests fail or
why the margins TASK-1.123 measured are still being exceeded:

- `just check` runs four separate `steg-server` test steps that each build and run the
  *entire* `attach` integration binary: plain, `race-fixture`, `pty-race-fixture`,
  `accept-backoff-fixture` (confirmed from TASK-1.123's own recorded `just check`
  output and the justfile's own step names). So **one worktree's own `just check`
  already runs four copies of the attach binary**, each ~50s, with no serialisation
  between them beyond `cross_process_flood_guard`.
- `cross_process_flood_guard` (attach.rs:1408) is taken only by the five tests TASK-
  1.123 named (the flood/stuck-pane tests). The other 26 tests in the same file run
  unserialised against however many copies of the binary are active at once, across
  however many worktrees are building - including ordinary cargo/rustc compilation
  load from *other* tasks' workers, not only other copies of this same test binary.
- TASK-1.123's own reproduction and margin-sizing (0/20 failures at 4-way attach-binary
  concurrency; the login_env class forced reliably at 3-way self-stacked `--lib`
  invocations) did not include concurrent *compilation* load from several worktrees'
  own `cargo build` steps running at the same time as the test steps - a different,
  and per the brief's own sighting ("three concurrent just check runs"), evidently
  heavier contention than what was measured. TASK-1.123's own notes already state
  plainly that its margins were not claimed to hold at a higher level than its own
  test: "the lock's own necessity is clearest at the sighting's own reported
  severity... not claimed to be independently isolated beyond that."

What reading cannot establish: which of the four `just check` steps and which test(s)
within them are the ones actually failing under today's heavier load (the task's own
notes record this was never captured - "which attach.rs tests failed was not
recorded"), and whether the cause is (i) the four-feature-variant self-contention
TASK-1.123's own class (a) reproduction already measured but at a higher concurrency
level than 4-way, (ii) one of the 26 unguarded tests in the same file hitting the same
starvation the five guarded ones used to, or (iii) something TASK-1.123's fix does not
touch at all.

**Experiment to run before fixing**, stated honestly as not yet run: build the attach
test binary for each of the four feature combinations
(`cargo test -p steg-server --test attach [--features ...] --no-run`), run all four
compiled binaries concurrently with `--test-threads=1` and per-test timing (`-- -Z
unstable-options --report-time` is nightly-only; plain wall-clock bracketing around
each test name from a `--list`-driven harness, or `cargo nextest` if already available,
is enough), repeated in a loop, and record which test names actually fail and at what
elapsed time. Repeat with one or two extra `cargo build`/`cargo test` processes running
in sibling worktrees to approximate "several worktrees building" rather than relying on
synthetic `yes` load (TASK-1.123's own class (b) notes found synthetic load and real
contention are not interchangeable). Only once a specific failing test and its failure
mode are captured does this plan's fix for 1b stop being speculative; until then the
proposed response is: widen `cross_process_flood_guard` to cover every test in the
file (removing the five-tests-only carve-out, since the structural fact above shows the
other 26 are exposed to the identical contention), which is a safe, mechanical move
regardless of which specific test the experiment names - named as the default fix,
open question #3, pending the experiment's result in case it points somewhere more
specific (e.g. widening `DEFAULT_READ_TIMEOUT`/the 20s call deadlines further, which
this plan does not propose without evidence it is the actual bottleneck).

## 2. Inventory of remaining unbounded waits - methodology and counts by category

Grepped directly against the worktree (`git grep`), both `crates/server/tests/`,
`crates/steg/tests/`, and the `#[cfg(test)]` unit-test modules under `crates/server/
src/` and `crates/steg/src/` (TASK-1.123's own description already named "the unit
tests of a dozen server modules" as part of the original sweep's count; AC #1 does not
limit itself to the `tests/` directory). Cross-checked against where `run_bounded`/
`wait_child_bounded` (crates/server/tests/support/mod.rs,
crates/steg/tests/support/mod.rs) and this crate's own `test_support::run_bounded`
(crates/server/src/test_support.rs, `#[cfg(test)]` only) are actually called, to find
what is already wrapped versus still bare.

Finding: outside `race.rs` and `openpty_race.rs` (the two files TASK-1.113 fixed
directly) and the handful of helpers TASK-1.123 added, none of the shared bounded-wait
helpers are used anywhere else in either crate's tests, in the integration directory or
in `src/`'s own unit-test modules - zero call sites of `run_bounded`/`wait_child_
bounded`/`test_support::run_bounded` across every other test file checked.

**Category A - bare thread `.join()` with no timeout.**
37 call sites across `crates/server/tests/*.rs` and `crates/steg/tests/*.rs`
(accept_backoff, attach, events, handshake, idle_stop x4, openpty_race, pane_capture,
pane_send, real_pty, routing x4, runtime x5, server_stop x4, shutdown x6,
server_start.rs x1), plus roughly 15-20 more in `crates/server/src/`'s own unit-test
modules (idle.rs, outbound.rs x6, pane.rs, runtime/events.rs x4, listen.rs's test
module, connection.rs's test module - each needs confirming its surrounding `#[cfg
(test)]` boundary before counting, since some `.join()` hits in these files are
production shutdown-path code, out of scope: connection.rs:350, listen.rs:258/548/572,
pane.rs:258/1776/2622, runtime/close.rs:367, runtime/mod.rs:362 are all non-test code,
left untouched).
Fix: wrap each in `run_bounded("<what it is joining, e.g. the server thread>", <bound>,
move || handle.join().expect(...))`, using this file's own existing liveness-bound
convention (15s where the file already has one, else the shared default).
Exceptions, left alone (already effectively bounded or not worth the churn): `.join()`
calls in a `Drop` impl immediately after an explicit `kill()` of the same child/thread
(e.g. race.rs's `ReapOnDrop`, openpty_race.rs's `ReapOnDrop`) - a backstop reap of an
already-killed process, not a wait for anything to happen; `test_support.rs`'s own
`ClosingWriterGuard::drop` and the `tests/support/mod.rs` helpers themselves, which
already implement the bound.

**Category B - bare channel `.recv()` (not `.recv_timeout()`).**
14 call sites in the integration directory (idle_stop.rs:186, program.rs x9 and a few
more), plus at least 3 in `crates/server/src/idle.rs`'s own unit test (`observed_rx.
recv()`, lines 355/361/365). Fix: either `.recv_timeout(<bound>).expect("<what it
waited for>")` inline, or route through `run_bounded` when the receive is the whole
body of a helper.

**Category C - `std::sync::Barrier::wait()`.**
4 call sites: events.rs:907/916, real_pty.rs:463, server_start.rs:720. `std::sync::
Barrier` has no bounded variant in the standard library. Recommended default (open
question #4): keep these as a named exception in the inventory's own notes, not
wrapped - every participant is a thread already running and already at the rendezvous
point by construction (the whole point of a barrier), so a stall here would be a
logic bug surfacing as the *test's own outer bound* tripping (each of these tests
already has one), not a genuinely separate unbounded wait. Replacing `Barrier` with a
channel-based rendezvous that can itself time out is possible but adds real complexity
for a wait that, structurally, cannot outlive the test's own outer bound - the
engineer's call.

**Category D - `Child::wait()` outside the shared helpers.**
Mostly already bounded by an equivalent, file-local pattern and left alone:
`real_pty.rs`'s own `wait_bounded` (line 500) already does exactly what `wait_child_
bounded` does, just not through the shared helper - its own failure message already
names what it waited for, so AC #2's "left as it is unless its failure does not say
what it waited for" keeps it as is. The `self.child.wait()` calls inside several
`Drop` impls across `crates/steg/tests/*.rs` (cli_exit_statuses.rs:40, idle_stop.rs:46,
login_environment.rs:154, server_start.rs:44/53) follow an explicit `kill()` the same
way category A's `ReapOnDrop` does - backstop reaps, left alone.
One real exception found: `crates/server/tests/race.rs:75`'s `read_outcome_line`
reads the fixture's one stdout line directly with a bare `reader.read_line(&mut line)`
- no reader-thread-plus-`recv_timeout` wrapper, unlike every other stdout-line read
checked in this sweep (see category E). The function's own doc comment argues the
line is always there to read by the time this runs, but that is not itself a bound on
the primitive: fix the same way `crates/steg/tests/server_start.rs`'s own
`read_one_line_bounded` (line 236) already does it.

**Category E - `BufReader::read_line`/`read_to_end` on a child's stdout pipe (no
`set_read_timeout` on a pipe).**
Checked every call site found by the earlier `loop {`/`read_line` grep
(crates/steg/tests/idle_stop.rs:150, server_start.rs:248/1001,
cli_exit_statuses.rs's `assert_stream_stays_silent`): all of these already go through
a reader-thread-plus-`recv_timeout` wrapper (`read_one_line_bounded`,
`assert_stream_stays_silent`, or an inline equivalent) - genuinely bounded already,
despite `read_line` itself having no timeout, because the bound comes from the
channel receive around it, not the read call. Socket reads (e.g. attach.rs's
`stream.set_read_timeout(DEFAULT_READ_TIMEOUT)`) are bounded directly and were not
re-checked here. The one exception is category D's `race.rs:75` above - counted there,
not duplicated here.

**Remaining sleep-in-a-loop poll sites (AC #3, not AC #1/#2)**: 87 `loop {` blocks and
56 `std::thread::sleep` call sites across both crates' test files. The large majority
already carry an `Instant::now() < deadline` assertion inside the loop body (checked
file-by-file in this plan's own reading: every file's `loop {` count is matched or
exceeded by its own count of `Instant::now`/`deadline`/`elapsed()` references) - these
are the accepted cross-process-liveness-poll exception class TASK-1.123's own notes
already established (`race.rs::wait_until_accepting` is the named precedent) and do
not need a different mechanism, only AC #3's own listing-with-reason in the notes. The
build phase's own sweep must still confirm, file by file, that every `loop {` without
a visible deadline check in this plan's own grep is not a silent exception - this plan
does not claim that confirmation is complete, only that the pattern holds everywhere
spot-checked.

## 3. Named follow-ups folded into this sweep (from TASK-1.82, both confirmed still live)

- `crates/server/tests/attach.rs:133,251,342` (line numbers shifted slightly since the
  review nit was written, but all three still read `Instant::now() + Duration::from_
  secs(20); // TASK-1.123: above DEFAULT_READ_TIMEOUT (15s)`): extract one named
  constant, e.g. `const REPLY_DEADLINE: Duration = DEFAULT_READ_TIMEOUT.saturating_add
  (Duration::from_secs(5));`, used by all three helpers (`call_and_read_reply`,
  `attach`, `declare_size`).
- `crates/server/tests/pane_send.rs:54-56`: still has its own `AtomicU64` counter plus
  `std::process::id()` socket-directory helper, confirmed by reading - not yet moved
  onto `ScopedTempDir`/`scoped_socket_path` the way the other nine `crates/server/
  tests/*.rs` files were in TASK-1.123. Replace it the same way.

Not folded in (per TASK-1.123's own notes, already tracked elsewhere, left alone here):
`crates/sdk/src/local_socket.rs`'s `CooperativeHeldOpen` fixture's 100ms sleep - its
own named, size-1 follow-up.

## 4. Files touched, order of work, and the split recommendation

Recommend splitting into two tasks, per the board's own default and because the two
halves are at genuinely different stages of readiness (one has a diagnosed root cause
and a deterministic control ready to build against; the other still needs an
experiment before any fix can be justified) and touch almost entirely disjoint files:

**Task A - the live flake** (small, urgent, build first): `crates/server/src/
login_env.rs` (the timeout constant, 1a); the attach.rs experiment (2a) plus, pending
its result, either widening `cross_process_flood_guard`'s own coverage in
`crates/server/tests/attach.rs` or a more targeted fix the experiment points to. Touch
list: `login_env.rs`, `attach.rs`. No overlap with TASK-1.140 (which touches
`dispatch/pane.rs` and `tests/dispatch/cross_cutting.rs`, not `attach.rs` or
`login_env.rs`).

**Task B - the file-by-file bounding sweep** (the bulk of this task's original scope):
every file named in categories A, B, D (race.rs only) above, across `crates/server/
tests/*.rs` (all of them except `race.rs`/`openpty_race.rs`, already done),
`crates/server/src/{idle,outbound,pane,listen,connection}.rs` and `runtime/events.rs`'s
own unit-test modules, and `crates/steg/tests/*.rs`, plus the two named follow-ups
(section 3) and the `pane_send.rs` helper move. This is the task most likely to collide
with TASK-1.140, which is adding its own tests to `crates/server/tests/dispatch/
cross_cutting.rs` and `dispatch/pane.rs` right now - recommend sequencing Task B's own
touch on `crates/server/tests/dispatch/*.rs` last, re-checked against `main` once
TASK-1.140 merges, the same ordering AC #4 already asked for against the tasks it
named (open question #1: confirm this reading of AC #4 replaces the stale TASK-1.88/
1.90 reference).

Order within Task B: Category A (joins) and B (recv) first, file by file, smallest
files first to build confidence in the wrapping pattern before the larger files
(routing.rs, runtime.rs, shutdown.rs); the two named follow-ups (section 3) next, since
they are small and independent; the `src/` unit-test modules last, since they are a
different compilation unit from the integration tests and do not block on them; the
`dispatch/*.rs` touch last of all, re-checked against `main` after TASK-1.140 merges.

## 5. Risks

- The attach.rs experiment (1b) may not reproduce cleanly even with a sibling
  worktree's own build running - if so, this plan's fallback (widen the flood guard to
  every test in the file) ships anyway as a safe, mechanical improvement, but the
  notes must say plainly that it was not confirmed to be the actual mechanism, the same
  honesty standard TASK-1.123 used for its own unreproduced class (d).
- Wrapping ~37+ bare `.join()` and ~17 bare `.recv()` calls in `run_bounded` touches
  nearly every test file in both crates - a change with low individual risk per site
  but a large diff surface; staging it file-by-file with its own `cargo test -p <crate>
  --test <file>` check after each file keeps any one mistake small and locatable,
  rather than discovering a break only at the final `just check`.
- `crates/server/src/test_support.rs`'s `run_bounded` is `pub(crate)`, so unit tests in
  every module can call it directly without a new `#[path]` include, unlike the
  integration-test files, which already duplicate `run_bounded` per the existing
  "each `tests/*.rs` file is its own compilation unit" reasoning - no new duplication
  needed there.
- Category C's `Barrier::wait()` exception (open question #4) is a judgement call, not
  a mechanical one - flagged for the engineer rather than decided unilaterally here.

## 6. Proof per acceptance criterion

- AC #1 (every unbounded wait gets a bound, named failure message): per file in Task
  B's touch list, `cargo test -p steg-server --test <file>` (or `-p steg --test
  <file>`, or `--lib` for the `src/` unit-test modules) recorded via `asdlc record-
  check` before and after that file's own edit; a final `cargo test -p steg-server -p
  steg` (both crates, every feature combination `just check` itself uses) and a full
  `just check` at the end of Task B.
- AC #2 (already-bounded waits left alone unless unnamed): the inventory in section 2
  states, per category, which call sites are left untouched and why (category D's
  `real_pty.rs::wait_bounded`, the `Drop`-impl backstop reaps); no separate proof step
  beyond the inventory itself and the reviewer checking it against the diff.
- AC #3 (no sleep-in-a-loop where an event exists; every remaining poll/sleep named
  with a reason): the notes list every `loop {`/`thread::sleep` site this sweep leaves
  as a poll, with its reason (cross-process liveness, no in-process event), continuing
  the precedent `race.rs::wait_until_accepting`'s own comment already set; the 100ms
  `crates/sdk/src/local_socket.rs` sleep is named explicitly per AC #3's own wording,
  pointing to its own separate follow-up rather than fixed here.
- AC #4 (TASK-1.140's files done last, re-checked against `main`): Task B's own
  `dispatch/*.rs` touch is sequenced after TASK-1.140 merges, rebased onto `main`
  first, then `just check` run once more before Task B's own finish.
- Task A's own proof, separately: 1a's forced control (section 1a, N/N failing then
  N/N passing against the deterministic scratch-script reproduction); 1b's experiment
  result and, once a fix is chosen, whatever reproduction the experiment itself
  establishes as the right one to re-run after the fix (not decided yet - see section
  1b).

## 7. Open questions for the engineer, with recommended defaults

1. AC #4 names TASK-1.88 and TASK-1.90, both now merged; the live overlap is TASK-
   1.140. Recommended default: read AC #4 as naming TASK-1.140's files instead (section
   0) and confirm in the approval rather than editing the criterion's own text.
2. The proposed new functional timeout for login_env's grandchild test (section 1a):
   recommended default 5s (comfortably past a plausible fork/exec delay under
   contention, still an order of magnitude below the file's own 15s/30s outer bounds).
3. The default fix for attach.rs pending the experiment (section 1b): recommended
   default is widening `cross_process_flood_guard` to cover every test in the file,
   not only the five named ones, unless the experiment itself points somewhere more
   specific (e.g. a particular test's own tighter inner bound).
4. `std::sync::Barrier::wait()` (category C, section 2): recommended default is to
   leave these four call sites as a named exception rather than replace `Barrier` with
   a bounded channel rendezvous, since a stall there cannot outlive the test's own
   outer bound by construction.
5. Split confirmation (section 4): recommended default is Task A (the flake) first and
   separately from Task B (the sweep), per the board's own default to split and
   because the two are at different stages of readiness.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Sighting after TASK-1.123 merged, noted by @bruk.o on 2026-11-18: TASK-1.88's worker, on a branch that already contained TASK-1.123's fixes, had two local just check runs fail under three concurrent builds - one in login_env.rs's backgrounded-grandchild test, one in attach.rs's pty-race tests - each green on a single retry; CI was green. So TASK-1.123 narrowed these clashes but did not end them under heavy load. Which attach.rs tests failed was not recorded; whoever takes this task should try to reproduce with three worktrees building at once.

More sightings on 2026-11-18, noted by @bruk.o: with three builds running, login_env's backgrounded-grandchild test failed two consecutive local just check runs for TASK-1.141's worker and two for TASK-1.139's, all on branches that already contain TASK-1.123's widened bounds; CI stayed green each time. Under that load it now fails more often than it passes locally, so start here.

Split by @bruk.o on 2026-11-20, on the planner's recommendation: the flake (login_env grandchild test, attach.rs under sibling builds) moves to its own task (drafted as DRAFT-2, awaiting acceptance). What stays here is the bounding sweep only. Released until TASK-1.140 (pane wait) merges, because the sweep collides with it in the dispatch tests; the next planner trims this plan to the sweep and drops AC 4's stale reference to TASK-1.88/1.90 in favour of TASK-1.140.

Sighting 2026-11-21, by @bruk.o: crates/server/tests/shutdown.rs many_short_connections_do_not_grow_the_tracked_set failed once in CI on PR 89 (a change to one markdown file) at its final assertion, the process-wide open-descriptor count after 200 connections being under before+10. The count is for the whole test process, so tests running beside it in the same binary can move it. Add to the sweep.
<!-- SECTION:NOTES:END -->
