---
id: TASK-1.137
title: 'Server and CLI tests: every remaining wait with no bound gets one'
status: In Progress
assignee:
  - '@bruk.w6'
created_date: '2026-11-18T14:03:00Z'
updated_date: '2026-11-22T04:12:00Z'
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
## 0. What changed since this plan's last draft - replaces it, does not extend it

The plan previously on this task (sections 0/1/1a/1b there) planned the login_env
grandchild timeout and the attach.rs flood-guard widening as work still to do. Both
are merged: TASK-1.143 ("the login_env grandchild and attach tests stop failing under
sibling builds", PR #88) raised login_env.rs's functional timeout and widened
attach.rs's own flood guard, exactly along the lines that plan proposed. TASK-1.140
(pane wait) is also merged (PR #92), so the AC #4 sequencing question that plan carried
forward is answered: nothing from TASK-1.88/1.90 or TASK-1.140 is still live to sequence
around. The live file-level overlaps today are TASK-1.146 (@bruk.w2: `runtime/start.rs`,
`listen.rs`, `env.rs`, likely `crates/steg/tests/login_environment.rs`) and TASK-1.51
(@bruk.w1: `connection.rs`, `dispatch/mod.rs`, `cli/connect.rs`, new
`cli_terminal_context.rs`) - see section 5.

This plan covers only what is left: the file-by-file bounding sweep (AC #1/#2/#3) plus
the three sightings folded in since (shutdown.rs, cli_connection_arbitration.rs,
pane_wait.rs).

## 1. The three sightings, diagnosed separately - they are not the same kind of problem

### 1a. `shutdown.rs::many_short_connections_do_not_grow_the_tracked_set` - root cause
established; not a missing bound

Every wait in this test already has one: six `recv_timeout(5s)` calls, a `join_within`
with its own 5s bound. The failing assertion (`after < before + 10`) counts this
*whole process's* open file descriptors (`open_fd_count`, `/dev/fd` entries) before and
after 200 connections. `shutdown.rs` has seven `#[test]` fns in one binary
(confirmed: `grep -n '#\[test\]' crates/server/tests/shutdown.rs`), and nothing in the
file serialises them - `cargo test -p steg-server` runs a binary's own tests on
multiple threads by default (confirmed: no `--test-threads=1` anywhere in the
justfile's `cargo test -p steg-server` invocations). Any of the other six tests
opening or closing a socket, pipe or pty concurrently while this one measures `before`
or `after` can move the count past the +10 margin - a false failure, not a leak. This
is why it failed once in CI on an unrelated one-line markdown PR (#89): nothing about
that diff could have changed `run`'s own tracked-set logic.

**Fix, reusing `attach.rs`'s own established pattern** (`FLOOD_TEST_LOCK`, a file-scoped
`static Mutex<()>` every test that needs exclusivity takes before running): add
`static PROCESS_FD_COUNT_LOCK: Mutex<()> = Mutex::new(());` to `shutdown.rs` and have
**all seven** tests in the file take it at the top of their body, not only the
fd-counting one - a guard only one test takes does not stop the other six from running
concurrently with it, which is the whole mechanism this bug depends on. This serialises
the file; the tradeoff (this file's own wall-clock cost in `just check`, currently
parallel, becomes sequential) is the engineer's call (open question #1) against the
alternative of changing what the test measures (e.g., counting only fds this test's
own connections opened, by diffing the socket's own accepted-connection count rather
than the whole process's fd table - a bigger change this plan does not propose without
being asked for it).

**Forceable control** (not load-dependent): in a scratch, uncommitted copy, add a
sibling test in the same file that deliberately opens and holds open ~20 file
descriptors (e.g. a `Vec<File>` from repeatedly opening `/dev/null`) for the duration of
`many_short_connections_do_not_grow_the_tracked_set`'s own measurement window, with no
lock. Without the new lock: fails deterministically (recorded: N/N failures). With the
lock added and the sibling test also taking it: passes (recorded: N/N passes). Revert
the scratch test before building the real fix.

### 1b. `cli_connection_arbitration.rs::a_hostile_first_connection_does_not_stop_a_real_autostart_from_succeeding`
- root cause established by reading the CI log directly; not a missing bound, and not
fixable inside this task's own scope

Fetched PR #91's own first (failed) CI attempt directly (`gh api
repos/bruk-io/steg/actions/jobs/136994285808/logs`, run 44952822506 attempt 1,
`just check` step). The failure is not a timeout: the panic is `ConnectError::
ClosedBeforeHello` - meaning **both** `connect_handshaken` attempts in this run ended
in EOF-before-hello: the hostile fixture's deliberate first closure, *and* the second
attempt, against the real, freshly-autostarted server this test expects to succeed
cleanly. (The log itself is not reproduced here or anywhere else on the board, per this
task's own safety rule; the run/job/attempt ids above are enough for anyone with CI
access to pull it again.)

Every wait on this path already has a bound with a named failure
(`connect_timeout` 10s - `crates/steg/src/cli/connect.rs:32`; `arbitration_timeout` 5s
with a 20ms poll - `crates/server/src/arbiter.rs`; the hostile listener's own
`ready.recv_timeout(2s)`), so this is outside AC #1/#2 as written: nothing here is
unbounded or unnamed. What the log shows instead is `connect_handshaken`'s own
documented one-retry limit (TASK-1.77: "a second such closure... is not retried
again") being hit for a reason that has nothing to do with this test's hostile
fixture - something on the *real* server's own first accepted connection, under CI's
own concurrent-build contention, produced the identical EOF-before-hello symptom the
fixture is built to simulate. **What this reading cannot establish**: which server-side
code path produced that second closure (a losing side of R99's own arbitration
accepting one connection before it notices it lost and shuts down is the most likely
candidate structurally, but that is not confirmed by reading alone, and confirming it
means reading `arbiter.rs`/`listen.rs`'s own accept-loop interaction under genuine
sibling-build contention, which is new investigation, not a bounded-wait fix).

**No code change proposed here.** Recommending either (a) a separate follow-up task to
investigate the real server's own EOF-before-hello path under contention, since fixing
it (if it is real) is a production retry-semantics question, not a test change, or (b)
accepting one CI retry as the cost of this exact rare case, is the engineer's call
(open question #2) - not decided unilaterally in a test-bounding sweep.

### 1c. `pane_wait.rs::wait_for_program_status` - the review nit's own suggestion does
not fit as stated; resolved by reading, not by picking

The nit says wait on "the pane.exited event" instead of polling `pane.show`. Two
candidate events exist and neither is free to use here without extra protocol setup
this file's own tests don't do:
- `ScreenUpdate::Ended` (commit f7887880): delivered only on a pane's own **attach**
  stream - this helper's connection never calls `pane.attach`.
- `TreeEvent::PaneExited` (`tree_event.rs`): delivered only to a client **subscribed**
  via `event.watch` - this helper's connection never subscribes either.

The actual fix that needs neither: this file's own `pane.wait --for exit` (with no
`timeout`) already blocks until the real exit, with no attach or subscription
required - proven by every other test in this same file. `wait_for_program_status`'s
only two call sites (lines 273, 382) both wait for `"ended"` before issuing a
`--timeout 0` or no-timeout `pane.wait` call as the thing under test; replacing the
poll with a blocking `pane.wait` call for the *precondition* is the same mechanism the
tests already exercise, not a new one. Concretely: replace the `loop { pane.show;
sleep(20ms) }` body with one `call_and_read_reply(..., "pane.wait",
wait_params(pane, None))`, asserting `Response::Ok`, and keep the function's name/
signature (callers pass `"ended"` today; the `expected` parameter becomes unused and
is dropped, or kept as a documented assertion that a non-"ended" caller would be a
contract violation this helper no longer supports - open question #3, recommended
default: drop the parameter, since both call sites pass the same literal).

This is not a blocking risk for the already-ended case: a no-timeout `pane.wait`
against a pane that has already exited answers at once rather than blocking, through
`register_wait`'s own step-5 immediate-reply branch - the same branch
`timeout_zero_on_an_already_ended_pane_answers_ok_with_the_real_exit_code` already
proves. The precondition call cannot hang waiting for an exit that already happened.

## 2. Inventory of remaining unbounded waits - tool-based, re-run against this head

Command: `git grep -nE '\.join\(\)'` / `'\.recv\(\)'` (excluding `recv_timeout`) /
`'Barrier'`, each scoped to `crates/server/tests/*.rs`, `crates/steg/tests/*.rs`,
`crates/sdk/src/*.rs` and `crates/sdk/src/**/*.rs`, and separately to
`crates/server/src/*.rs`/`crates/server/src/**/*.rs`/`crates/steg/src/*.rs`/
`crates/steg/src/**/*.rs`, with every `src/` hit read in place to split production
code from its own `#[cfg(test)] mod tests`. `run_bounded`/`wait_child_bounded` call
sites found the same way, to correct the previous draft's "zero usage elsewhere"
claim: `run_bounded` is already called at `crates/server/src/outbound.rs:651` and in
`crates/steg/tests/cli_output_formats.rs` (from TASK-1.120, TASK-1.113's own original
sweep) - not a new finding, just a correction; it does not change which sites below
are still bare.

**Category A - bare `.join()`.** 37 sites, confirmed by file
(`accept_backoff.rs`, `attach.rs`, `events.rs` x2, `accept_backoff_attempt.rs`,
`handshake.rs` x2, `idle_stop.rs` x4, `openpty_race.rs`, `pane_capture.rs`,
`pane_send.rs`, `pane_wait.rs`, `real_pty.rs`, `routing.rs` x4, `runtime.rs` x5,
`server_stop.rs` x4, `shutdown.rs` x7, `server_start.rs`) in `tests/`, plus 14 in
`src/`'s own unit-test modules, re-derived against this head (not copied from the
previous draft's stale line numbers): `idle.rs:371,433` (2), `outbound.rs:
724,767,855,923,1038,1047` (6), `pane.rs:3400` (1), `listen.rs:742` (1), `runtime/
events.rs:519,573,630,649` (4). Production-code `.join()` left untouched, confirmed by
reading each in place: `connection.rs:350`, `listen.rs:258/548/572`, `pane.rs:
258/1776/2622`, `runtime/close.rs:381`, `runtime/mod.rs:371` - none inside a
`#[cfg(test)]` boundary.
Fix: `run_bounded("<what it is joining>", <bound>, move || handle.join().expect(...))`,
the file's own existing liveness bound where it has one (15s/20s), else the shared
default. `shutdown.rs`'s own 7 and `pane_wait.rs`'s own 1 are fixed as part of section
1's own edits to those files (same-file ownership - see section 4), not duplicated
here.
Exceptions left alone, as before: `Drop`-impl backstop reaps right after an explicit
`kill()` (race.rs's/openpty_race.rs's `ReapOnDrop`; the `Drop` impls in `crates/steg/
tests/{cli_exit_statuses,idle_stop,login_environment,server_start}.rs`); `test_support.
rs`'s/`support/mod.rs`'s own helper internals.

**Category B - bare `.recv()`.** 14 sites in `tests/` (`idle_stop.rs:186`, `program.rs`
x13), plus 10 in `src/`'s own unit-test modules:
`idle.rs:314,324,355,362,366,404,421` (7 - two separate `#[test]` fns each make
several `observed_rx.recv()` calls),
`listen.rs:726` (1, inside `a_finished_connection_is_removed_and_a_still_open_one_
is_kept`'s own spawned-thread body - released by the test's own `drop(done_tx)` right
before, but has no bound of its own today), `outbound.rs:784` (1, `StepWriter`'s own
test double), `pane.rs:2483` (1, a spawned-thread test helper). Production-code bare
`.recv()` in `idle.rs:129`, `listen.rs` and `pane.rs`'s own command/reply channels left
untouched (not test code at all). Zero bare `.recv()` in `crates/sdk/src/` (checked,
in scope per the brief, nothing found).
Fix: `.recv_timeout(<bound>).expect("<what it waited for>")`, or `run_bounded` when the
receive is a helper's whole body.

**Category C - `Barrier::wait()`.** Confirmed still 4 sites: `events.rs:907,916`,
`real_pty.rs:463`, `server_start.rs:720`. Recommended default (open question #4,
carried unchanged from the previous draft): leave as a named exception - every
participant is already a running thread at the rendezvous point by construction, so a
stall here surfaces as the test's own outer bound tripping, not a separately unbounded
wait.

**Category D - `Child::wait()` outside the shared helpers.** One real exception,
confirmed unchanged: `race.rs:75`'s `read_outcome_line` does a bare
`reader.read_line(&mut line)` on the fixture's stdout with no reader-thread/
`recv_timeout` wrapper, unlike every other such read in this sweep. Fix the same way
`server_start.rs:236`'s own `read_one_line_bounded` already does it. `real_pty.rs`'s
own `wait_bounded` (line 500) and the `Drop`-impl backstop reaps are left alone - their
own failure, where they have one, already names what they waited for.

**Category E - reader-thread-wrapped reads.** Unchanged from the previous draft:
checked every `read_line`/`read_to_end` call site on a child's stdout; all but
`race.rs:75` already go through a bound (`read_one_line_bounded`,
`assert_stream_stays_silent`, or an inline equivalent).

**AC #3's own listing - every remaining poll/sleep, named with its reason** (not a
fix; the notes record this, as the file-by-file edits land):
- `race.rs::wait_until_accepting` and its file-local equivalents elsewhere - the
  established cross-process-liveness-poll exception (TASK-1.123's own precedent):
  no in-process event exists for "has this other process's socket started accepting
  yet."
- `pane_wait.rs::start_server`'s own 5ms poll waiting for the socket to accept - same
  reason, unaffected by section 1c's fix (that fix only touches
  `wait_for_program_status`).
- `crates/sdk/src/local_socket.rs:193`'s 20ms poll inside `connect`'s own autostart
  wait - production code within the sdk's one permitted I/O exception (R103); not a
  test file, out of scope for AC #3's test-only wording, named here only so the notes
  don't look like they missed it.
- `crates/sdk/src/local_socket.rs:456`'s 30s sleep and `:586`'s 100ms sleep, both in
  the same `fake-server-for-tests`/`CooperativeHeldOpen` fixture: already named by
  TASK-1.123's own notes as its own separate, size-1 follow-up - listed here per AC #3's
  own wording, not fixed in this sweep.
- Every other `loop {}` in both crates' test files (87 by the previous draft's own
  count, not re-verified site-by-site here) already carries an `Instant::now() <
  deadline` assertion - the build phase's own file-by-file pass must still confirm this
  holds for each one as it lands, not claimed complete by this plan.

## 3. Named follow-ups folded into this sweep

- `attach.rs:133,251,342`: extract one named constant (e.g. `const REPLY_DEADLINE:
  Duration = DEFAULT_READ_TIMEOUT.saturating_add(Duration::from_secs(5));`), used by
  all three helpers - confirmed still duplicated three times, unchanged since TASK-1.123.
- `pane_send.rs:54-56`: move its own `AtomicU64` counter/`std::process::id()` socket
  helper onto `ScopedTempDir`/`scoped_socket_path` (`support/mod.rs:325,386`), the way
  the other nine `crates/server/tests/*.rs` files already were.

## 4. Split - by file ownership, not by category

Two tasks, each owning a disjoint file list (no file edited by both):

**Task A - the three sightings** (size-2): `crates/server/tests/shutdown.rs` (the new
`PROCESS_FD_COUNT_LOCK`, taken by all seven tests, plus that file's own 7 Category-A
`.join()` sites), `crates/server/tests/pane_wait.rs` (the `pane.wait`-based rewrite of
`wait_for_program_status`, plus that file's own 1 Category-A `.join()` site); the
`cli_connection_arbitration.rs` finding written up in the task's notes with no code
change, plus a recommendation on whether to open a follow-up task for the real
server's own EOF-before-hello path (open question #2). Proof: section 1a's forced
control (scratch sibling test, N/N failing then N/N passing, reverted before the real
fix); `cargo test -p steg-server --test shutdown` and `--test pane_wait`; a full `just
check` before finishing.

**Task B - the mechanical sweep** (size-3, matching TASK-1.123's own precedent for a
comparable call-site count): every other file in categories A/B/D/E above, the two
named follow-ups (section 3), the AC #3 listing in the notes (which may reference
`shutdown.rs`/`pane_wait.rs`'s own remaining poll sites without editing those files -
a listing is not a competing edit). Order within Task B: Category A and B fixes first,
file by file, smallest first; the two follow-ups next; the `src/` unit-test modules
after (a different compilation unit, does not block on the integration tests);
`listen.rs`'s and `connection.rs`'s own unit-test-module edits and `crates/steg/
tests/login_environment.rs` last of all, rebased onto `main` once TASK-1.146 and
TASK-1.51 merge (section 5) - the same ordering this task's own AC #4 already asked
for against tasks now merged.

Split confirmation is open question #5; the file-ownership rule above is this plan's
own answer to "which entries go where," not left to the split itself to decide later.

## 5. Collisions

- `crates/server/src/listen.rs` and `crates/steg/tests/login_environment.rs`: TASK-1.146
  (@bruk.w2) edits production code in the first and is likely to touch the second;
  Task B only ever touches `listen.rs`'s own `#[cfg(test)]` module, but a same-file
  edit is still a rebase conflict risk - sequenced last in Task B, re-checked against
  `main` after TASK-1.146 merges.
- `crates/server/src/connection.rs`: TASK-1.51 (@bruk.w1) edits its production code.
  `connection.rs:350`'s own `.join()` is production code, already excluded (section
  2) - Task B has no file overlap with TASK-1.51's list.
- TASK-1.51 also adds `crates/steg/tests/cli_terminal_context.rs`, a file this
  inventory cannot have covered (it does not exist yet at this head) - named so Task
  B's own notes don't claim completeness it cannot have.

## 6. Risks

- Wrapping ~51 bare `.join()`/`.recv()` call sites (Task B) touches most of both
  crates' test files - low risk per site, large diff surface; stage file-by-file with
  its own `cargo test -p <crate> --test <file>` (or `--lib`) after each, not only at
  the final `just check`.
- A forceable, recorded failing-then-passing control for every mechanical site (Task
  B) is not practical at this count - not promised. The mechanism itself already has
  its own self-test (`support/mod.rs:416`, `wait_child_bounded` against a sleep that
  outlives its bound); the mechanical sites' own proof is `just check` plus the diff
  showing each wrapped site's failure names what it waited for. Real forced
  failing-then-passing pairs are reserved for Task A's two diagnosed fixes (section 1a;
  1c has no control to force, since it removes a wait rather than re-timing one -
  proof there is the two tests it serves still passing).
- Serialising all seven `shutdown.rs` tests (Task A) may add real wall-clock time to
  `just check`'s own `test (steg-server)` step - the engineer's tradeoff (open
  question #1).
- `Barrier::wait()` (Category C, open question #4) stays a judgement call, not decided
  here.

## 7. Open questions for the engineer, with recommended defaults

1. `shutdown.rs`: serialise all seven tests with a shared lock (recommended default),
   or change what the test measures instead?
2. `cli_connection_arbitration.rs`: open a separate follow-up task to investigate the
   real server's own EOF-before-hello path under contention (recommended default,
   since this task's own scope is bounding waits, not production retry semantics), or
   accept the rare CI retry as-is?
3. `pane_wait.rs`: drop `wait_for_program_status`'s now-unused `expected` parameter
   (recommended default, both call sites pass the same literal), or keep it as a
   documented, enforced precondition?
4. `Barrier::wait()`: leave the four sites as a named exception (recommended default,
   carried from the previous draft), or replace with a bounded channel rendezvous?
5. Confirm the two-task split in section 4, and its file-ownership assignment, before
   either is approved. This task's own AC #4 ("the files TASK-1.88 and TASK-1.90 are
   editing are done last") is satisfied by construction (section 0: both merged,
   nothing left to sequence around) and is now vacuous as written - it needs either
   restating against TASK-1.146/1.51 (section 5) or striking, and neither task drafted
   from this plan yet has an AC covering the `shutdown.rs` lock, the `pane_wait.rs`
   rewrite, or the `cli_connection_arbitration.rs` write-up - the engineer's edit when
   drafting Task A and Task B's own acceptance criteria, not this plan's to write.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Sighting after TASK-1.123 merged, noted by @bruk.o on 2026-11-18: TASK-1.88's worker, on a branch that already contained TASK-1.123's fixes, had two local just check runs fail under three concurrent builds - one in login_env.rs's backgrounded-grandchild test, one in attach.rs's pty-race tests - each green on a single retry; CI was green. So TASK-1.123 narrowed these clashes but did not end them under heavy load. Which attach.rs tests failed was not recorded; whoever takes this task should try to reproduce with three worktrees building at once.

More sightings on 2026-11-18, noted by @bruk.o: with three builds running, login_env's backgrounded-grandchild test failed two consecutive local just check runs for TASK-1.141's worker and two for TASK-1.139's, all on branches that already contain TASK-1.123's widened bounds; CI stayed green each time. Under that load it now fails more often than it passes locally, so start here.

Split by @bruk.o on 2026-11-20, on the planner's recommendation: the flake (login_env grandchild test, attach.rs under sibling builds) moves to its own task (drafted as DRAFT-2, awaiting acceptance). What stays here is the bounding sweep only. Released until TASK-1.140 (pane wait) merges, because the sweep collides with it in the dispatch tests; the next planner trims this plan to the sweep and drops AC 4's stale reference to TASK-1.88/1.90 in favour of TASK-1.140.

Sighting 2026-11-21, by @bruk.o: crates/server/tests/shutdown.rs many_short_connections_do_not_grow_the_tracked_set failed once in CI on PR 89 (a change to one markdown file) at its final assertion, the process-wide open-descriptor count after 200 connections being under before+10. The count is for the whole test process, so tests running beside it in the same binary can move it. Add to the sweep.

Sighting 2026-11-21, by @bruk.o: crates/steg/tests/cli_connection_arbitration.rs a_hostile_first_connection_does_not_stop_a_real_autostart_from_succeeding failed once in CI on PR 91 (TASK-1.136, a change to crates/ui and tools only) at its assertion on line 101, in the steg package's own test step. Passed locally at the same head. Add to the sweep.

From the review of TASK-1.140 (nit, 2026-11-21): crates/server/tests/pane_wait.rs wait_for_program_status polls pane.show with a 20 ms sleep instead of waiting on the pane.exited event. Fold into the sweep.

Sighting, 22 November 2026: crates/steg/tests/cli_connection_arbitration.rs a_hostile_first_connection_does_not_stop_a_real_autostart_from_succeeding failed again in CI, this time on main at 4ccc8344 (the merge of steg PR 91, which touches only crates/ui and tools). Assertion at line 101; the client reported that the server accepted the connection and closed it before a hello arrived, twice. Second sighting of this test; re-run once by @bruk.o.
<!-- SECTION:NOTES:END -->
