---
id: TASK-1.137
title: 'Server and CLI tests: every remaining wait with no bound gets one'
status: In Review
assignee:
  - '@bruk.w6'
created_date: '2026-11-18T14:03:00Z'
updated_date: '2026-11-22T06:15:00Z'
labels:
  - size-3
milestone: m-3
dependencies:
  - TASK-1.123
references:
  - task-1.137-server-and-cli-tests-every-remaining
  - https://github.com/bruk-io/steg/pull/98
parent_task_id: TASK-1
project: bruk-io-steg
asdlc_accepted_from: DRAFT-1
asdlc_branch: task-1.137-server-and-cli-tests-every-remaining
asdlc_plan_approved_by: bruk
asdlc_plan_approved_hash: '33d5d39c8d80'
asdlc_plan_approved_size: size-3
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
## 0. Scope - this task is Task B only; DRAFT-1 and DRAFT-2 hold the rest

Per the engineer's split: this task keeps only the mechanical join/recv bounding
sweep, the two named follow-ups (`attach.rs`'s `REPLY_DEADLINE` constant,
`pane_send.rs`'s `ScopedTempDir` move), and AC #3's poll/sleep listing.

- **DRAFT-1** owns `shutdown.rs`'s fd-count serialisation (the new lock, taken by all
  seven tests) and `pane_wait.rs`'s `wait_for_program_status` rewrite (poll ->
  `pane.wait`). This task does **not** touch either of those two specific mechanisms.
- **DRAFT-2** owns the `cli_connection_arbitration.rs` `ClosedBeforeHello`
  investigation (H1/H2) and any fix that comes out of it. This task makes no change to
  that file and draws no conclusion about it.

**The boundary inside `shutdown.rs` and `pane_wait.rs`, by line, so DRAFT-1's worker
can see exactly where this task stops:** this task still wraps the two files' own
*ordinary* bare `.join()` sites - the ones that are not the sighted fix itself:
- `shutdown.rs:208,438,439,492,553` (5 sites - see section 1 for why this is 5, not
  the 7 originally counted) stay in this task. `shutdown.rs:124,131` (the file's own
  existing `join_within` helper) are not touched by anyone - already bounded, see
  section 1.
- `pane_wait.rs:82` (`stop_server`'s own bare `handle.join()`) stays in this task -
  it is unrelated to `wait_for_program_status`, which DRAFT-1 owns.

## 1. Inventory - tool-derived, re-verified per site against its own context, not just
   counted by pattern match

Command: `git grep -nE '\.join\(\)'` / `'\.recv\(\)'` (excluding `recv_timeout`) /
`'Barrier'`, scoped to `crates/server/tests/*.rs`, `crates/steg/tests/*.rs`,
`crates/sdk/src/*.rs`/`crates/sdk/src/**/*.rs`, and separately to
`crates/server/src/*.rs`/`crates/server/src/**/*.rs`/`crates/steg/src/*.rs`/
`crates/steg/src/**/*.rs`. Every hit was then read in place - not just counted - to
settle two things a pattern match cannot: whether it sits inside a `#[cfg(test)]`
module at all (several files have more than one separate `mod tests`/`mod
some_other_tests` block, so a line number alone does not say), and whether it is
genuinely bare or already wrapped by a file-local bounded-wait helper that duplicates
`run_bounded`'s own shape without calling it by name. This reading corrected the
previous draft's counts in both directions - fewer real fixes in several files, and
two real fixes previously miscounted as production code - detailed below so the
correction itself is checkable, not asserted.

**Category A - bare `.join()`, real fixes needed.**

*An already-bounded idiom this reading found duplicated across four files - not a new
mechanism, the same shape `run_bounded` and `support/mod.rs`'s own helpers already
use (a watcher thread does the real join; the caller only ever blocks on a channel
`recv_timeout`), just hand-rolled per file rather than calling the shared helper by
name. The exemption rests on one property common to all four, not on the four being
identical in shape: the bound is the surrounding `recv_timeout`, whatever its own
timeout path does next. Two of the four (`idle_stop.rs`, `shutdown.rs`) share the
exact same two-join shape as `server_stop.rs`'s own `join_within` - the outer
`waiter.join()` is unreachable on the timeout path at all, since the `recv_timeout`'s
own `unwrap_or_else` panics first. `accept_backoff.rs`'s own `join_with_bound` is a
different shape - it returns `None` on timeout rather than panicking, and its own doc
says the watcher thread "is never joined" - but the exempting property still holds:
the real join is still bounded by the same `recv_timeout`, just handled differently
once that bound is crossed. Each already names what it waited for
(`"run should have returned within {bound:?} of being stopped"`,
`"the waiter thread should not panic"`). These are not Category A fixes - wrapping an
already-bounded helper's own internals in a second layer of `run_bounded` would be
circular, the same reasoning that already exempts `support/mod.rs`'s own helpers:*
- `idle_stop.rs`'s own `join_within` (two separate call sites of the idiom in this
  file: lines 73-82 and 341-380) - **all 4 of this file's own `.join()` sites
  (75,82,342,380) are this helper's internals, not fixes.** (The previous draft
  counted this file as 4 real sites; it is 0.)
- `server_stop.rs`'s own `join_within` (lines 164-173) - 2 of this file's own 4
  `.join()` sites (166,173) are the helper's internals; **212 and 280 are genuinely
  bare** (`second_handle.join()`, `handle.join()` with no bound) - 2 real fixes, not 4.
- `shutdown.rs`'s own `join_within` (lines 122-131) - 2 of this file's own 7
  `.join()` sites (124,131) are the helper's internals; **208, 438, 439, 492, 553 are
  genuinely bare** - 5 real fixes, not 7 (these 5 stay in this task per section 0).
- `accept_backoff.rs`'s own `join_with_bound` (lines 120-141, explicitly documented as
  the same shape) - its one `.join()` site (133) is the helper's own internal, never
  the fix. (The previous draft counted this file as 1 real site; it is 0.)

*One more correction in the other direction - already fixed, miscounted as needing
one:*
- `openpty_race.rs:347`'s `.join()` is already inside a `run_bounded(...)` call
  (confirmed by reading the surrounding closure) - this file was one of the two
  TASK-1.113 already finished directly; this line should never have been in the
  count. (The previous draft counted it as 1 real site; it is 0.)

*Two sites the previous draft placed in the production-code exclusion list that are
actually test code, genuinely bare, and real fixes - found by checking each file's
own `#[cfg(test)]` module boundaries precisely rather than inheriting the line numbers:*
- `pane.rs:2622` - inside `mod attach_tests` (starts line 2515), a test double's own
  `finish(self)` method: `let _ = self.writer.join();`, no bound.
- `runtime/mod.rs:371` - inside `mod tests` (starts line 342),
  `a_panic_while_holding_the_lock_does_not_poison_a_later_call`'s own
  `std::thread::spawn(...).join()`, no bound. One shape difference from every other
  site in this list: this `.join()`'s own `Result` is bound to a name and asserted on
  next (the test's whole point is checking that the spawned thread *did* panic), so
  the mechanical `run_bounded("...", bound, move || handle.join().expect(...))`
  rewrite does not apply as-is - `run_bounded` returns the body's value, so this site
  needs `run_bounded("...", bound, move || handle.join())` with the existing assertion
  then run against what `run_bounded` returns, not a plain `.expect()` inside the
  closure.

**Real Category A fixes, by file, corrected:**
`tests/`: `attach.rs` (1), `events.rs` (2), `handshake.rs` (2), `pane_capture.rs` (1),
`pane_send.rs` (1), `pane_wait.rs` (1 - stays here, section 0), `real_pty.rs` (1),
`routing.rs` (4), `runtime.rs` (5), `server_stop.rs` (2), `server_start.rs` (1),
`shutdown.rs` (5 - stays here, section 0) = **26** (not the previous draft's 37).
`src/` unit-test modules, each re-verified against its own file's `#[cfg(test)]`
boundaries: `idle.rs:371,433` (2), `outbound.rs:724,767,855,923,1038,1047` (6),
`pane.rs:3400,2622` (2, not 1 - the correction above), `listen.rs:742` (1),
`runtime/events.rs:519,573,630,649` (4), `runtime/mod.rs:371` (1, not 0 - the
correction above) = **16** (not the previous draft's 14).
**Total: 42 real Category A fixes** (26 + 16), down from the previous draft's
51-site estimate once the already-bounded helpers are excluded and the two
miscounted production sites are added back in.

Fix: `run_bounded("<what it is joining>", <bound>, move || handle.join().expect(...))`,
the file's own existing liveness bound where it has one (15s/20s), else the shared
default.

Production `.join()` confirmed excluded by checking each file's own (possibly
multiple) `#[cfg(test)]` module locations against the target line, not inherited:
`connection.rs:350` (before its own only `#[cfg(test)]` at line 992),
`listen.rs:258/548/572` (before its own only `#[cfg(test)]` at line 633),
`pane.rs:258` (before its first `#[cfg(test)]` at line 1742),
`runtime/close.rs:381` (before its own only `#[cfg(test)]` at line 441).
Also exempt, confirmed test code but already bounded via a local helper the same way
as above: `pane.rs:1776` (`join_within`'s own internal, inside `mod tests` at 1742).

Exceptions left alone, as before: `Drop`-impl backstop reaps right after an explicit
`kill()` (`race.rs`'s/`openpty_race.rs`'s `ReapOnDrop`; the `Drop` impls in
`crates/steg/tests/{cli_exit_statuses,idle_stop,login_environment,server_start}.rs`);
`test_support.rs`'s/`support/mod.rs`'s own helper internals.

One inventory note, not a fix: `crates/server/tests/fixtures/
accept_backoff_attempt.rs` is a fixture *binary*, a separate glob this task's own
`crates/server/tests/*.rs` scope does not reach. It has one bare `.join()` (line 197),
already documented safe in its own comment ("`reporter`'s own loop ends on its own
once `run` drops every clone of `events_tx`") - out of this task's own file list
either way.

**Category B - bare `.recv()`.** Re-verified the same way (every `src/` hit checked
against its own file's `#[cfg(test)]` boundary; no local already-bounded-recv helper
found anywhere, unlike Category A). Counts unchanged from the previous draft: 14 in
`tests/` (`idle_stop.rs:186`, `program.rs` x13), 10 in `src/` (`idle.rs:
314,324,355,362,366,404,421` - 7, two separate `#[test]` fns; `listen.rs:726` - 1,
inside a spawned-thread test body; `outbound.rs:784` - 1, a test double's own
`StepWriter`; `pane.rs:2483` - 1, inside `mod capture_tests`, started at 2315).
Production bare `.recv()` confirmed excluded the same way: `idle.rs:129` (before its
own only `#[cfg(test)]` at 156), `listen.rs`'s and `pane.rs`'s own command/reply
channels (all well before their own first `#[cfg(test)]` marker). Zero bare `.recv()`
in `crates/sdk/src/`.
Fix: `.recv_timeout(<bound>).expect("<what it waited for>")`, or `run_bounded` when
the receive is a helper's whole body.

**Category C - `Barrier::wait()`.** Unchanged: 4 sites, `events.rs:907,916`,
`real_pty.rs:463`, `server_start.rs:720`. Recommended default (open question #1,
carried forward): leave as a named exception - every participant is already a
running thread at the rendezvous point by construction, so a stall here surfaces as
the test's own outer bound tripping, not a separately unbounded wait.

**Category D - `Child::wait()` outside the shared helpers.** Unchanged: one real
exception, `race.rs:75`'s `read_outcome_line`, a bare `reader.read_line(&mut line)`
on the fixture's stdout with no reader-thread/`recv_timeout` wrapper. Fix the same way
`server_start.rs:236`'s own `read_one_line_bounded` already does it. `real_pty.rs`'s
own `wait_bounded` (line 500) and the `Drop`-impl backstop reaps are left alone.

**Category E - reader-thread-wrapped reads.** Unchanged: every other `read_line`/
`read_to_end` call site on a child's stdout already goes through a bound
(`read_one_line_bounded`, `assert_stream_stays_silent`, or an inline equivalent).

**AC #3's own listing - every remaining poll/sleep, named with its reason** (not a
fix; recorded in the notes as the file-by-file edits land):
- `race.rs::wait_until_accepting` and its file-local equivalents elsewhere - the
  established cross-process-liveness-poll exception (TASK-1.123's own precedent): no
  in-process event exists for "has this other process's socket started accepting yet."
- `crates/sdk/src/local_socket.rs:193`'s 20ms poll inside `connect`'s own autostart
  wait - production code within the sdk's one permitted I/O exception (R103), named
  here only so the notes don't look like they missed it.
- `crates/sdk/src/local_socket.rs:456`'s 30s sleep and `:586`'s 100ms sleep, in the
  `fake-server-for-tests`/`CooperativeHeldOpen` fixture - already named by TASK-1.123's
  own notes as its own separate, size-1 follow-up; listed here, not fixed in this sweep.
- `pane_wait.rs::start_server`'s own 5ms poll waiting for the socket to accept - same
  cross-process-liveness reason; unaffected by DRAFT-1's own fix, which only touches
  `wait_for_program_status`.
- Every other `loop {}` in both crates' test files already carries an
  `Instant::now() < deadline` assertion - the build phase's own file-by-file pass must
  still confirm this holds for each one as it lands, not claimed complete by this plan.

## 2. Named follow-ups folded into this sweep

- `attach.rs:133,251,342`: extract one named constant (e.g. `const REPLY_DEADLINE:
  Duration = DEFAULT_READ_TIMEOUT.saturating_add(Duration::from_secs(5));`), used by
  all three helpers - confirmed still duplicated three times. (Checked: each one's own
  failure message already names what it waited for - `"timed out waiting for a reply
  to {method}"` - so this is a dedup, not an AC #2 fix.)
- `pane_send.rs:54-56`: move its own `AtomicU64` counter/`std::process::id()` socket
  helper onto `ScopedTempDir`/`scoped_socket_path` (`support/mod.rs:325,386`), the way
  the other nine `crates/server/tests/*.rs` files already were.

## 3. Criterion 4, restated - every file named in it is merged; the files to sequence
   around now are different ones

TASK-1.88, TASK-1.90 and TASK-1.140 (all three originally named or implied by this
criterion) are merged - nothing from any of them is still live to sequence around.
Read criterion 4 as: **the files other in-flight tasks are editing are done last and
re-checked against `main` first.** Today that is:
- `crates/server/src/listen.rs` and `crates/steg/tests/login_environment.rs`
  (TASK-1.146, @bruk.w2) - this task's own touch on `listen.rs` is its `#[cfg(test)]`
  module only (one `.join()` fix, line 742), but a same-file edit is still a rebase
  conflict risk.
- Whatever TASK-1.51 (@bruk.w1) touches (`connection.rs`, `dispatch/mod.rs`,
  `cli/connect.rs`, new `cli_terminal_context.rs`) - checked and found **no actual
  file overlap**: `connection.rs`'s only `.join()` (line 350) is production code,
  already excluded above, so this task makes no edit to any file TASK-1.51 is
  editing. Named here so the notes don't claim more completeness than they have -
  TASK-1.51's own new `cli_terminal_context.rs` is a file this inventory cannot have
  covered, since it does not exist yet at this head.

Sequencing: this task's own `listen.rs` and `src/` unit-test-module edits go last,
rebased onto `main` once TASK-1.146 merges, re-checked with `just check` before this
task finishes.

## 4. Criterion 2 (already-bounded waits), answered with counts

**The answer, in three lines: zero already-bounded waits need changing; roughly 25
already-bounded call sites stay exactly as they are, each already naming what it
waited for; recommended default is no criterion-2 changes in this task.** The
counts and the sites behind that answer, for the reviewer to check against the same
file list section 1 built, follow below.

**Already-bounded waits this plan would leave alone, with counts** (all of them,
confirmed): the four file-local bounded-join helpers found in section 1
(`idle_stop.rs`'s own `join_within` - 4 internal call sites; `server_stop.rs`'s own
`join_within` - 2; `shutdown.rs`'s own `join_within` - 2; `accept_backoff.rs`'s own
`join_with_bound` - 1; `pane.rs`'s own test-module `join_within` - 1 =
**10 call sites**), each with its own named failure (`"run should have returned
within {bound:?} of being stopped"` or equivalent); `openpty_race.rs`'s own already-
`run_bounded`-wrapped join (1); `real_pty.rs`'s own `wait_bounded` (1) and the
`Drop`-impl backstop reaps (6, named in section 1); `attach.rs`'s own `REPLY_DEADLINE`
helpers (3 call sites, already named `"timed out waiting for a reply to {method}"` -
section 2's fix is a dedup of the constant, not a message fix); every Category E
reader-thread-wrapped read (already named, per file).

**Already-bounded waits this plan would change, with counts: zero.** Every
already-bounded wait this task's own file list reaches already names what it waited
for in its own failure message - none triggers criterion 2's own "unless" clause.

**Recommended default: make no changes under criterion 2** - leave every
already-bounded wait exactly as it is. This is the engineer's call to confirm before
the build starts, since it is a negative finding (nothing to change) rather than a
positive one, and a reviewer should be able to check it against the same file list
above rather than take it on trust.

## 5. Risks

- Wrapping 42 bare `.join()`/24 bare `.recv()` call sites touches most of both
  crates' test files - low risk per site, large diff surface; stage file-by-file with
  its own `cargo test -p <crate> --test <file>` (or `--lib`) after each, not only at
  the final `just check`.
- A forceable, recorded failing-then-passing control for every mechanical site is not
  practical at this count - not promised. The mechanism itself already has its own
  self-test (`support/mod.rs:416`, `wait_child_bounded` against a sleep that outlives
  its bound); the mechanical sites' own proof is `just check` plus the diff showing
  each wrapped site's failure names what it waited for.
- `shutdown.rs` and `pane_wait.rs`: this task's own edits (the ordinary bare-join
  wraps named in section 0) and DRAFT-1's own edits (the fd-lock, the
  `wait_for_program_status` rewrite) land in the same two files - real merge-conflict
  risk between the two tasks even with the line-level boundary in section 0 drawn as
  precisely as it is; whoever finishes second rebases onto the other's merge.
- `Barrier::wait()` (Category C, open question #1) stays a judgement call, not
  decided here.
- `runtime/mod.rs:371` (section 1) needs the slightly different `run_bounded` shape
  noted there, not the mechanical one-liner every other site gets - worth flagging so
  a reviewer checks that site specifically, not just the diff's shape in general.

## 6. Proof per acceptance criterion

- AC #1 (every unbounded wait gets a bound, named failure message): per file,
  `cargo test -p steg-server --test <file>` (or `-p steg --test <file>`, or `--lib`
  for the `src/` unit-test modules) recorded via `asdlc record-check` before and after
  that file's own edit; a full `just check` at the end.
- AC #2 (already-bounded waits left alone unless unnamed): section 4's own count (zero
  changes) is the proof - the reviewer checks it against the same file list, not a
  separate step.
- AC #3 (no sleep-in-a-loop where an event exists; every remaining poll/sleep named):
  the notes list every site section 1's own listing names, with its reason.
- AC #4 (other in-flight tasks' files done last, re-checked against `main`): section 3's
  own sequencing - `listen.rs` and the `src/` unit-test modules last, rebased onto
  `main` after TASK-1.146 merges, `just check` run once more before this task finishes.

## 7. Open questions for the engineer, with recommended defaults

1. `Barrier::wait()` (Category C): leave the four sites as a named exception
   (recommended default), or replace with a bounded channel rendezvous?
2. Criterion 2 (section 4): confirm making no changes to any already-bounded wait
   (recommended default - nothing found that needs one), before the build starts.
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

Plan checked by @bruk.o, 22 November 2026: ok for the narrowed scope (the mechanical sweep; DRAFT-1 and DRAFT-2 hold the sighted problems). Covers each criterion; criterion 4 read as the files other in-flight tasks edit (listen.rs, login_environment.rs for TASK-1.146). Three things for the build, which the brief will carry: bring main in by merging, never by rebasing a pushed branch; narrow test runs go through the pinned Zig, and the final proof is just check; and since a control per site is not practical, record one representative control (one wrapped join made to outlive its bound, failing with its named message, then passing).

Check passed: `just check` exited 0 at 2026-11-22T04:57:14+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.137 on task-1.137-server-and-cli-tests-every-remaining at 98ca618e9202:

```text
ok      format check (0.4s)
ok      lint (87.9s)
ok      boundaries (0.2s)
ok      foundations (0.2s)
ok      foundations public surface (0.1s)
ok      design pages (0.1s)
ok      licences (0.8s)
skip    tests: this change touches no crate
ok      build (steg-sdk) (3.7s)
ok      test (steg-sdk) (2.6s)
ok      test (steg-sdk, local-socket) (2.9s)
ok      build (steg-kernel) (43.0s)
ok      test (steg-kernel) (1.5s)
ok      build (steg-server) (44.0s)
ok      test (steg-server) (61.2s)
ok      test (steg-server, race-fixture) (63.5s)
ok      test (steg-server, pty-race-fixture) (68.8s)
ok      test (steg-server, accept-backoff-fixture) (67.0s)
ok      build (steg) (2.9s)
ok      test (steg) (4.5s)
ok      test (steg, fixtures) (9.3s)
ok      build (steg-ui) (21.0s)
ok      test (steg-ui) (1.3s)
ok      build (steg-app) (2.8s)
ok      test (steg-app) (2.2s)
ok      tool tests (41.2s)
ok      architecture (0.8s)
ok      recipe names (0.1s)
ok      ui levels (0.1s)
ok      ui colours (0.1s)
```

Check passed: `just check` exited 0 at 2026-11-22T05:41:31+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.137 on task-1.137-server-and-cli-tests-every-remaining at a83377cddb75:

```text
ok      format check (0.3s)
ok      lint (0.4s)
ok      boundaries (0.1s)
ok      foundations (0.1s)
ok      foundations public surface (0.0s)
ok      design pages (0.1s)
ok      licences (0.8s)
ok      tests (-p steg -p steg-server) (94.7s)
ok      build (steg-sdk) (0.1s)
ok      test (steg-sdk) (0.3s)
ok      test (steg-sdk, local-socket) (0.5s)
ok      build (steg-kernel) (0.1s)
ok      test (steg-kernel) (0.2s)
ok      build (steg-server) (0.1s)
ok      test (steg-server) (96.7s)
ok      test (steg-server, race-fixture) (88.1s)
ok      test (steg-server, pty-race-fixture) (65.0s)
ok      test (steg-server, accept-backoff-fixture) (56.9s)
ok      build (steg) (0.1s)
ok      test (steg) (2.6s)
ok      test (steg, fixtures) (5.7s)
ok      build (steg-ui) (0.3s)
ok      test (steg-ui) (0.4s)
ok      build (steg-app) (0.3s)
ok      test (steg-app) (0.8s)
ok      tool tests (40.1s)
ok      architecture (0.7s)
ok      recipe names (0.1s)
ok      ui levels (0.1s)
ok      ui colours (0.1s)
```

Check FAILED: `cargo test -p steg-server --test server_stop -- server_stop_without_force_while_a_session_exists_refuses_and_the_server_keeps_running --exact` exited 101 at 2026-11-22T05:43:09+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.137 on task-1.137-server-and-cli-tests-every-remaining at a83377cddb75 with uncommitted changes:

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.09s
     Running tests/server_stop.rs (target/debug/deps/server_stop-e2ff52c6cb96857f)

running 1 test
test server_stop_without_force_while_a_session_exists_refuses_and_the_server_keeps_running ... FAILED

failures:

---- server_stop_without_force_while_a_session_exists_refuses_and_the_server_keeps_running stdout ----

thread 'server_stop_without_force_while_a_session_exists_refuses_and_the_server_keeps_running' (77529526) panicked at crates/server/tests/support/mod.rs:44:29:
timed out after 5s waiting for: the server thread to end after stop()
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    server_stop_without_force_while_a_session_exists_refuses_and_the_server_keeps_running

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 10 filtered out; finished in 5.02s

error: test failed, to rerun pass `-p steg-server --test server_stop`
```

Check passed: `cargo test -p steg-server --test server_stop -- server_stop_without_force_while_a_session_exists_refuses_and_the_server_keeps_running --exact` exited 0 at 2026-11-22T05:43:25+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.137 on task-1.137-server-and-cli-tests-every-remaining at a83377cddb75:

```text
   Compiling steg-server v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.137/crates/server)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.37s
     Running tests/server_stop.rs (target/debug/deps/server_stop-e2ff52c6cb96857f)

running 1 test
test server_stop_without_force_while_a_session_exists_refuses_and_the_server_keeps_running ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 10 filtered out; finished in 0.03s
```

Check FAILED: `cargo test -p steg-server --test server_stop -- server_stop_without_force_while_a_session_exists_refuses_and_the_server_keeps_running --exact` exited 101 at 2026-11-22T05:52:11+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.137 on task-1.137-server-and-cli-tests-every-remaining at 52529634000d with uncommitted changes:

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.09s
     Running tests/server_stop.rs (target/debug/deps/server_stop-e2ff52c6cb96857f)

running 1 test
test server_stop_without_force_while_a_session_exists_refuses_and_the_server_keeps_running ... FAILED

failures:

---- server_stop_without_force_while_a_session_exists_refuses_and_the_server_keeps_running stdout ----

thread 'server_stop_without_force_while_a_session_exists_refuses_and_the_server_keeps_running' (77604760) panicked at crates/server/tests/support/mod.rs:44:29:
timed out after 5s waiting for: the server thread to end after stop()
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    server_stop_without_force_while_a_session_exists_refuses_and_the_server_keeps_running

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 10 filtered out; finished in 5.02s

error: test failed, to rerun pass `-p steg-server --test server_stop`
```

Check passed: `cargo test -p steg-server --test server_stop -- server_stop_without_force_while_a_session_exists_refuses_and_the_server_keeps_running --exact` exited 0 at 2026-11-22T05:52:26+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.137 on task-1.137-server-and-cli-tests-every-remaining at 52529634000d:

```text
   Compiling steg-server v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.137/crates/server)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.36s
     Running tests/server_stop.rs (target/debug/deps/server_stop-e2ff52c6cb96857f)

running 1 test
test server_stop_without_force_while_a_session_exists_refuses_and_the_server_keeps_running ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 10 filtered out; finished in 0.03s
```

Check passed: `just check` exited 0 at 2026-11-22T06:00:04+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.137 on task-1.137-server-and-cli-tests-every-remaining at e0be1986efe7:

```text
ok      format check (0.3s)
ok      lint (2.4s)
ok      boundaries (0.1s)
ok      foundations (0.1s)
ok      foundations public surface (0.0s)
ok      design pages (0.1s)
ok      licences (0.8s)
ok      tests (-p steg -p steg-server) (98.7s)
ok      build (steg-sdk) (0.1s)
ok      test (steg-sdk) (0.7s)
ok      test (steg-sdk, local-socket) (1.0s)
ok      build (steg-kernel) (0.1s)
ok      test (steg-kernel) (0.4s)
ok      build (steg-server) (0.8s)
ok      test (steg-server) (63.4s)
ok      test (steg-server, race-fixture) (64.2s)
ok      test (steg-server, pty-race-fixture) (71.3s)
ok      test (steg-server, accept-backoff-fixture) (64.7s)
ok      build (steg) (0.1s)
ok      test (steg) (2.4s)
ok      test (steg, fixtures) (13.2s)
ok      build (steg-ui) (0.4s)
ok      test (steg-ui) (0.4s)
ok      build (steg-app) (0.3s)
ok      test (steg-app) (0.8s)
ok      tool tests (40.1s)
ok      architecture (0.7s)
ok      recipe names (0.1s)
ok      ui levels (0.1s)
ok      ui colours (0.1s)
```

Mechanical sweep built: 42 bare .join() sites bounded (26 tests/, 16 src/), 24 bare .recv() sites bounded (14 tests/, 10 src/), one Child::wait() exception (race.rs::read_outcome_line), two named follow-ups (attach.rs's REPLY_DEADLINE constant, pane_send.rs's move onto ScopedTempDir/scoped_socket_path). listen.rs's two test-module sites built last, confirmed TASK-1.146 has not merged production code there yet; the engineer confirmed it was safe to proceed without waiting.

Corrections found and fixed during planning/build, not just carried from the plan: four files (idle_stop.rs, server_stop.rs, shutdown.rs, accept_backoff.rs) each had a file-local bounded-join helper duplicating run_bounded's own shape - those internals are already bounded and already named, left alone, not double-wrapped. Two sites (pane.rs:2622, runtime/mod.rs:371) were miscounted as production code in the earlier plan; both are test code, genuinely bare, now fixed. attach.rs's stop_server bound was re-derived from force_end's own documented worst-case formula and this file's own established ~5x contention multiplier (confirmed force_grace really is the production 2s default), not left at a value picked from one local measurement that a reviewer would otherwise have to re-derive themselves.

Checks recorded on the task: just check clean at an early head (before listen.rs), a representative failing-then-passing control at the final head (server_stop.rs's own run_bounded site, forced to outlive its bound with a reverted scratch edit - failed with exactly the named message, confirmed not another nearby timeout, then passed once reverted), and a final just check recorded clean at the final head (format, lint, boundaries, every crate's tests across all four steg-server feature combinations, tool tests, architecture, UI checks). CI on PR 98 passed (confirmed via gh pr checks, not a watched notification).

AC #3's own listing (every remaining poll/sleep, named with its reason) is on the task as a comment, including the sdk's own two already-tracked sleep sites and the park-forever fixture processes that are not polls for an event at all.

Deferred, not this task's own scope: TASK-1.149 (shutdown.rs's fd-count serialisation + pane_wait.rs's helper poll - was DRAFT-1) and TASK-1.150 (the cli_connection_arbitration.rs ClosedBeforeHello investigation - was DRAFT-2). Barrier::wait() (4 sites) left as a named exception per the engineer's accepted default; criterion 2 confirmed with counts - no already-bounded wait needed a message change.

No test output, environment dump, or env output was put in any commit, comment, or the PR - confirmed unrecorded before every recorded failing run.
<!-- SECTION:NOTES:END -->

## Comments

<!-- COMMENTS:BEGIN -->
author: @bruk
created: 2026-11-22T04:47:00Z

---

Plan approved by @bruk (33d5d39c8d80, size-3): its worker may build it.

---

created: 2026-11-22T05:43:00Z

---

Representative control recorded (failing-then-passing pair at the final head, recorded above): `crates/server/tests/server_stop.rs::server_stop_without_force_while_a_session_exists_refuses_and_the_server_keeps_running`'s own `run_bounded("the server thread to end after stop()", Duration::from_secs(5), ...)`.

Scratch edit: inserted `std::thread::sleep(Duration::from_secs(6))` as the first line inside the run_bounded closure (before the real `handle.join()`), so the watcher thread's own body takes longer than the 5s bound - the same effect, from run_bounded's own point of view, as the real joined thread itself taking too long. Ran unrecorded first (confirmed only the named panic message, nothing from the environment), then recorded: fails at `support/mod.rs:44:29` with exactly "timed out after 5s waiting for: the server thread to end after stop()" - the bound this fix added, not some other nearby timeout (the test's own prior assertions, on the refusal and the still-answering server, already passed before this point). Reverted (confirmed clean diff), re-ran, recorded passing.

Chosen as representative: this is the `run_bounded`-around-a-plain-`.join()` shape every other mechanical site in this sweep uses verbatim (the dominant pattern across ~40 of the 42 real Category A fixes), on one of the shorter bounds (5s) so the control itself stays fast, and it is a site with no other timing-sensitive assertion nearby that could produce a false confirmation.

---

created: 2026-11-22T05:44:00Z

---

AC #3 - every remaining poll/sleep, named with its reason (criterion 3; this is a listing, not a fix, per the plan's own section 1):

Confirmed while making the join/recv fixes above, file by file: every `loop {}` that polls with a `std::thread::sleep` in every file this task touched already carries its own `Instant::now() < deadline` (or equivalent) assertion with a named failure message - the established cross-process-liveness-poll exception TASK-1.123's own `race.rs::wait_until_accepting` set as precedent (no in-process event exists for "has this other process's socket started accepting yet" / "has the pid file appeared" / etc.). Spot-checked by comparing each touched file's own `loop {}` count against its own deadline-reference count (every file: deadline references well exceed loop counts) in addition to reading each site directly while fixing its own nearby join/recv. No new unguarded poll loop was found during this build.

Named exceptions, not fixed in this sweep (per the plan, section 1):
- `crates/sdk/src/local_socket.rs:193`'s 20ms poll inside `connect`'s own autostart wait - production code within the sdk's one permitted I/O exception (R103).
- `crates/sdk/src/local_socket.rs:456`'s 30s sleep and `:586`'s 100ms sleep, in the `fake-server-for-tests`/`CooperativeHeldOpen` fixture - already TASK-1.123's own named, separate, size-1 follow-up.
- `crates/server/tests/pane_wait.rs::start_server`'s own 5ms poll waiting for the socket to accept - unaffected by DRAFT-1's own `wait_for_program_status` fix, which is a different function in the same file.
- `crates/steg/tests/fixtures/autostart_client.rs`'s `loop { sleep(3600) }` and the similarly-shaped park-forever fixtures in `crates/server/tests/support/mod.rs:419`/`crates/steg/tests/support/mod.rs:419`/`cli_exit_statuses.rs:536` - not a poll for an event at all: each is a deliberately parked process killed externally by a real signal from the test, already documented as such in its own comment. Listed here so this notes entry doesn't look like it missed them.
- `crates/server/tests/fixtures/accept_backoff_attempt.rs`/`crates/server/tests/fixtures/race_attempt.rs`/`openpty_race_attempt.rs` - fixture binaries, outside the `tests/*.rs` glob this task scopes to (the open scope question the task's own description carried, not settled here).

Every other `std::thread::sleep` site found (one-shot waits like `handshake.rs:181`'s deliberate `handshake_timeout * 5`, or a fixed pacing delay inside an already-bounded trickle, e.g. `handshake.rs:505`'s 40ms-per-byte write cadence) is not a retry-poll for an event at all - a single wait for a known fixed duration, or a deliberate write-cadence, not something AC #3's own wording targets.

---

created: 2026-11-22T05:48:00Z

---

Status correction, and a bound revised after a second look:

Counts delivered so far vs. the plan's own totals (42 Category A, 24 Category B) - stating this explicitly rather than letting the plan's totals stand as if complete: Category A delivered is 41 of 42 (26 of 26 in tests/, 15 of 16 in src/) - crates/server/src/listen.rs's own one join is the outstanding site, held per the engineer's own instruction pending TASK-1.146. Category B delivered is 23 of 24 (14 of 14 in tests/, 9 of 10 in src/) - listen.rs's own one recv is the other outstanding site, same reason. Nothing else is deferred.

attach.rs's stop_server bound, revised: the first version (60s) was picked from one local measurement (32.5s observed once). Re-derived instead from force_end's own documented worst case (crates/server/src/runtime/close.rs: force_grace + ESCALATE_CONFIRM_TIMEOUT + FORCE_JOIN_TIMEOUT, ~6s nominal for one pane; this file's own flood tests stop a server with two panes, so ~10s nominal for them specifically, pass 6 being sequential per pane) times the same ~5x contention multiplier this file's own pane_close_force_returns_within_its_bound... test already uses and justifies in its own comment (30s against a 6s single-pane nominal, "a generous bound... not a tight race against this machine's own CPU contention"). Landed on 90s - real margin above both that derivation and the one measured run, still well under cross_process_flood_guard's own 180s and steg-quiet's 900s per-step ceiling. Re-ran the full attach.rs suite (31 passed) after the change.

The `just check` recorded earlier is now stale (a commit landed after it, and it did not cover listen.rs either way) - will record a fresh one at the final head once listen.rs is resolved, not before, to avoid recording it twice for the same reason.

Also: found and removed a stray /tmp/attach.rs.bak this worktree's own earlier diagnostic step left behind (should have gone in the scratchpad) - confirmed gone, confirmed nothing else landed outside the worktree or the scratchpad.

---

created: 2026-11-22T05:50:00Z

---

Confirmed, not assumed: attach.rs::start_server's own ServerConfig only sets socket_path/handshake_timeout/login explicitly, with `..ServerConfig::default()` for the rest - and ServerConfig::default()'s own force_grace field is literally `crate::runtime::FORCE_GRACE_DEFAULT` (2s, crates/server/src/config.rs:157), the real production default, not a shortened test-only one. The 90s bound's own derivation holds.

---
<!-- COMMENTS:END -->
