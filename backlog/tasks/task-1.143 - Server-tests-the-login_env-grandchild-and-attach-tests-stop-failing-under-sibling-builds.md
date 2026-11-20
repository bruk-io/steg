---
id: TASK-1.143
title: >-
  Server tests: the login_env grandchild and attach tests stop failing under
  sibling builds
status: In Progress
assignee:
  - '@bruk.w3'
created_date: '2026-11-20T17:21:00Z'
updated_date: '2026-11-20T17:38:00Z'
labels:
  - size-2
milestone: m-3
dependencies: []
parent_task_id: TASK-1
project: bruk-io-steg
asdlc_accepted_from: DRAFT-2
asdlc_branch: task-1.143-server-tests-the-login-env-grandchild
asdlc_plan_approved_by: bruk
asdlc_plan_approved_hash: '880746049d3b'
asdlc_plan_approved_size: size-2
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Split out of TASK-1.137 on its planner's recommendation (2026-11-20): the flake is small and urgent and touches only crates/server/src/login_env.rs's test and crates/server/tests/attach.rs, while the rest of TASK-1.137 is a sweep across most test files. Diagnosis already on TASK-1.137's plan, read it first: (1) login_env - the fixture forks before writing its pid file, and capture_login_environment's one-second functional timeout kills the process group when it expires; under heavy load the shell has not reached the write yet, so the later poll waits for a file that never appears. Proposed: raise that test's own timeout from 1 s to 5 s. (2) attach.rs - which tests fail was never recorded; just check runs four copies of the attach binary and the cross-process flood guard covers only five named tests. Sightings: TASK-1.137's notes, plus two more just check runs on 2026-11-20 (TASK-1.142's worker). CI stays green throughout, so this is a local-load fault, not a product fault.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The login_env backgrounded-grandchild test no longer depends on the pane's shell reaching its pid-file write within one second: a recorded control with a fixture that delays the write fails at the old bound and passes at the new one
- [ ] #2 The attach.rs tests that fail under concurrent builds are named from a recorded experiment (the four feature-variant binaries run together, with and without sibling build load), and the fix follows what the experiment shows
- [ ] #3 If the experiment cannot reproduce an attach.rs failure, the task says so plainly and applies only the stated default (the flood guard widened to the whole file), with that limit recorded
- [ ] #4 No test is weakened or skipped, no liveness bound is removed, and just check passes at the final head
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
## 0. Where this plan agrees and disagrees with TASK-1.137's diagnosis

Verified by reading the code directly, not copied on trust.

**login_env (1a): agree, fully.** `crates/server/src/login_env.rs:646` passes
`Duration::from_secs(1)` as this one test's own inline argument, nowhere else.
`capture_login_environment`'s only non-test caller (`runtime/login.rs:103`) passes a
configurable `environment_timeout`, whose product default is
`DEFAULT_LOGIN_ENVIRONMENT_TIMEOUT = Duration::from_secs(3)` (`config.rs:57`) - lower
even than the proposed 5s, and never 1s anywhere. Raising this test's own 1s is
confirmed to touch no product default; it was always a test-only, tighter-than-shipped
number. The kill-then-30s-poll mechanism TASK-1.137 traced (fork/exec delay under load
-> kill lands before the pid-file write -> the poll waits out a file that will never
appear) matches the code as read, line for line.

**attach.rs (1b): structural facts confirmed, two corrections.**
- Test count: actual is 23 `#[test]` functions in `attach.rs`, 5 already taking
  `cross_process_flood_guard()` (lines 1449, 1591, 1689, 1735, 1804:
  `an_unresponsive_panes_own_commands_do_not_stall_the_connection_or_another_pane`,
  `a_second_connections_control_calls_is_not_blocked_by_a_stuck_panes_lock_held_attach`,
  `pane_close_force_returns_within_its_bound_even_while_the_panes_own_worker_is_stuck`,
  `stopping_the_server_returns_within_its_bound_even_while_a_panes_worker_is_stuck`,
  `a_pane_that_closes_between_a_queued_attach_and_its_arm_leaves_nothing_to_detach`), so
  18 unguarded, not TASK-1.137's "31 total / 26 unguarded". The structural gap it
  identified still holds at the corrected count: those 18 have zero cross-process
  protection.
- Concurrency claim: TASK-1.137 says one `just check` run "runs four copies of the
  attach binary... with no serialisation between them". `attach.rs` carries no
  `#[cfg(feature = ...)]` at all (confirmed by grep) - it is the identical 23 tests
  compiled four times, under four different `steg-server` feature sets. But
  `check-packages` (justfile:129-144) runs those four `cargo test -p steg-server
  --features ...` steps in a plain sequential bash `for` loop - **within one worktree
  they never overlap**. The real concurrency is cross-worktree: several workers' own
  `just check` runs, each at a different point in its own sequential four-step loop, so
  at any instant one worktree might be mid-"plain" while another is mid-"race-fixture".
  Running this task's four variants together, from one worktree, by hand (as this
  plan's experiment does below) is a deliberate stand-in for that cross-worktree mix,
  not a reproduction of what `just check` does by itself - worth being explicit about,
  since it changes what a null result would mean.
- `cross_process_flood_guard` locks a path under `$XDG_CACHE_HOME` (or
  `$HOME/.cache`)/steg/test-locks - outside every worktree, so it already serialises
  its 5 tests machine-wide, across worktrees too, not only within one process. That
  narrows what "the fix follows what the experiment shows" can mean for those 5: a
  reproduced failure in one of them would point at the lock's own 60s acquire bound, or
  at CPU contention degrading the *in-lock* throughput assumption `flood_stdin`'s own
  doc states (sibling cargo builds competing for CPU while the lock is held, not a
  serialisation gap) - not at a missing lock.
- `tools/src/steg_tools/quiet.py` throttles every `cargo build/test/clippy` step
  machine-wide to `STEG_CHECK_SLOTS` (default 2) concurrent slots (TASK-1.132) when run
  through `just`. This is where this task's own "no more than two concurrent builds as
  load" bound comes from - it is not an arbitrary cap, it is this machine's own default
  ceiling when every participant goes through `just`. If any sighting ran `just check`
  on a worktree whose branch predates TASK-1.132 (slots not yet present there), that
  worktree's own heavy steps bypassed the throttle entirely - a possible reason
  sightings described heavier contention than two slots would allow; not resolved
  here, named as a limit on how far a null result in this task's own bounded experiment
  can be read.

## 1. login_env fix (AC #1)

### 1a. The fix
In `crates/server/src/login_env.rs`, raise
`capture_login_environment_kills_a_backgrounded_grandchild_that_held_the_pipe_open`'s
own inline timeout argument (line 646) from `Duration::from_secs(1)` to
`Duration::from_secs(5)`: an order of magnitude below the same test's own 15s/30s outer
bounds (untouched), comfortably past plausible fork/exec scheduling delay under
contention, still negligible against the file's liveness-bound convention. Rewrite the
test's own comment at lines 629-642, which currently argues "15s, not 5s... close to
its own 1s functional timeout" - that sentence becomes false the moment the inline
timeout changes; replace it with the real reasoning (the outer 15s/30s bounds are
liveness bounds unrelated to the tightened inner one, and 5s is chosen for fork/exec
margin, not proximity to the outer bounds).

### 1b. Deterministic control (AC #1's own wording: "a recorded control with a fixture
that delays the write fails at the old bound and passes at the new one")

A scratch, never-committed fixture edit cannot satisfy this project's own rule that
"controls count only as recorded failing+passing pairs at the final head" - a change
reverted before commit does not exist at the head the pair is supposed to be recorded
against. Instead:

Add a second, permanent, committed test in the same module,
`capture_login_environment_tolerates_a_delayed_pid_file_write_from_a_backgrounded_grandchild`,
built on the same `FakeShell` fixture used by its sibling, with one deliberate
difference: `"sleep 100 & sleep 2; echo $! > {pidfile}; exit 0"` - a 2-second sleep
between the fork and the pid-file write, standing in for the fork/exec delay a loaded
machine introduces. This fixture's own sleep is the stated exception this task's rules
carve out ("a fixture's deliberate delay for a control"): it is not a poll standing in
for an event, it is the independent variable the control is built to force. The test
body mirrors its sibling's assertions (returns within a liveness bound with
`Capture::Fallback(FallbackReason::TimedOut, _)`; the grandchild's pid file appears
well inside the 30s poll bound; the grandchild is confirmed killed after) and calls
`capture_login_environment` with the same final timeout, `Duration::from_secs(5)`, so
at the final head it passes: the 2s delay finishes comfortably before the 5s kill
fires, so the pid file exists by the time the poll looks for it.

To record the failing half at the same final head (not a different commit): temporarily
edit only this new test's own timeout argument from `Duration::from_secs(5)` down to
`Duration::from_secs(1)` (the old bound), nothing else, and run it. With the write
delayed to 2s and the kill now firing at 1s, the kill lands before the write
deterministically, every time - this is forced, not load-dependent, the same "force the
mechanism, don't wait for the kernel" standard TASK-1.123 used for its own class (e).
Expect the run to take close to 30s (the full pid-file poll exhausting) and fail with
exactly "the backgrounded grandchild's own pid file never appeared" - name both the
expected message and the expected ~30s duration before running it, so the watched
failure is unambiguous and not mistaken for a hang or an unrelated error. Record this
failing run via `asdlc record-check`, running the single test directly (not through
`just`, since this is a forced, temporary state, not a project check):
`ZIG="$(mise which zig 2>/dev/null || true)" cargo test -p steg-server --lib
login_env::tests::capture_login_environment_tolerates_a_delayed_pid_file_write_from_a_backgrounded_grandchild
-- --exact`. Then revert the timeout edit back to `Duration::from_secs(5)` (the
committed value) and record the same command passing. Both runs are against the
identical final-head source except for that one reverted line, satisfying "recorded
failing+passing pairs at the final head, each actually watched failing."

**Safety**: run both recorded commands unrecorded once first and read the output
directly, confirming nothing from the environment appears in it (this test's own
fixture never execs `/usr/bin/env`, so it should print nothing beyond the assertion
message and timing, but confirm before recording either run, per this task's standing
rule).

### 1c. Proof for AC #1
- The forced failing-then-passing pair above, recorded at the final head.
- `cargo test -p steg-server --lib login_env::` (both tests in the module) passing
  once the real timeout (5s) is in place and the scratch edit is reverted.
- Final `just check` (section 6) exercises both tests again as part of `check-packages`'s
  `test (steg-server)` step.

## 2. attach.rs experiment and fix (AC #2, #3)

### 2a. What the experiment tests, and the two conditions

Per this task's own brief, the experiment is bounded: at most two concurrent sibling
`cargo build`/`cargo test` processes as load, no load window longer than about ten
minutes. Two conditions, both using default test-harness threading (not
`--test-threads=1` - this machine runs with 28 logical CPUs and the real `just check`
invocation never serialises test threads either; serialising here would remove the most
likely mechanism, 18 unguarded tests already running concurrently inside one binary,
each starting its own server):

- **Condition A - self-contention only.** The four feature-variant `attach` test
  binaries (default, `race-fixture`, `pty-race-fixture`, `accept-backoff-fixture`) run
  concurrently with each other, no added sibling load. Reproduces, at today's machine
  and today's code, the scenario TASK-1.123 measured as 0/20 failures - a sanity
  baseline, not expected to find anything new by itself.
- **Condition B - self-contention plus bounded sibling build load.** The same four
  binaries, run concurrently with each other, while up to two separate `cargo
  build`/`cargo test` processes run as sibling load, each in its own throwaway worktree
  with its own `target/` (not this task's own worktree, and not another task's real
  worktree - a scratch clone made and discarded for this experiment only, so no other
  in-flight task is touched), for up to ten minutes, repeated.

Pre-build each of the four variants first, with `cargo test -p steg-server --features
<...> --test attach --no-run` (ZIG exported the same way as any narrow cargo run), and
locate the resulting binary under `target/debug/deps/` rather than re-invoking `cargo
test` for the timed runs. Reason: re-invoking `cargo test` for all four at once would
have them contend for the same `target/` directory's own Cargo-level build lock,
serialising their *build* phases regardless of real OS contention and confounding the
measurement; running the already-built binaries directly isolates execution-time
contention, which is what this experiment is testing.

### 2b. Stopping rule (write into the plan now, so the builder does not loop)
- Never more than two sibling load processes at once.
- Never run the sibling load for more than about ten minutes total, wall-clock, across
  every repetition.
- Within that window, repeat Condition B as many times as fit; stop at the time bound
  regardless of how many repetitions that allows.
- Capture each repetition's full output to a local file, not the board; only extract
  test names, pass/fail, and elapsed time for recording.

### 2c. Safety
Before recording anything from a failing run: read the captured output directly and
confirm it carries nothing from the environment (attach.rs's tests exchange protocol
frames over a local socket, not shell/profile output, so this is expected to be clean,
but confirm per this task's standing rule before any failing run goes into a note,
comment, or `record-check` invocation).

### 2d. Decision rule
- **A specific test name fails, reproducibly, in Condition A or B**: name it and its
  failure mode (which assertion, which bound, which message) in the task notes.
  - If it is one of the 5 already under `cross_process_flood_guard`: the existing
    machine-wide lock is not the gap (it already serialises these across worktrees) -
    the fix targets what actually failed inside it (the lock's own 60s acquire bound,
    or a throughput assumption `flood_stdin`'s doc states, degraded by sibling CPU
    load) - described concretely once named, not decided in advance here.
  - If it is one of the 18 unguarded: this directly justifies the stated default (widen
    `cross_process_flood_guard`'s coverage to the whole file) and names which test made
    that concrete, rather than leaving it a mechanical guess.
  - If it points somewhere this diagnosis does not reach (a mechanism unrelated to
    cross-process contention): say so plainly, treat it as a separate finding, and do
    not fold a fix for it into this task.
- **Nothing fails within the bounded budget (Condition A and B both)**: say so plainly,
  including that this is below the sighted severity ("three worktrees building") and
  that TASK-1.132's slot throttle may not have been active on every sighted worktree
  (section 0) - a null result here does not rule out the mechanism at higher
  concurrency. Apply only the stated default: widen `cross_process_flood_guard`'s
  coverage from the 5 named tests to all 23 tests in `attach.rs` (remove the
  five-tests-only carve-out at each of the 18 unguarded tests' own call sites), and
  record this limit explicitly in the task notes per AC #3.

### 2e. Proof for AC #2 and #3
- The experiment's own recorded repetitions (count, duration, any failing test names)
  in the task notes - not raw captured output.
- Whichever fix path 2d selects, the specific diff and the reasoning tying it to what
  the experiment showed (or the stated default, if nothing reproduced).
- `cargo test -p steg-server --test attach` (and the three feature variants) passing
  after the fix, each recorded via `asdlc record-check` through `just check-packages`
  (not a bare cargo invocation, since by then this is validating a real change, not a
  forced single-test control).

## 3. Files touched

- `crates/server/src/login_env.rs` - the timeout constant (1a), the new control test
  and its assertions (1b), the rewritten comment (1a).
- `crates/server/tests/attach.rs` - whichever of `cross_process_flood_guard`'s coverage
  or a more targeted fix 2d selects. No other file. Confirmed no overlap with
  TASK-1.140 (`dispatch/pane.rs`, `tests/dispatch/cross_cutting.rs`) - neither file this
  task touches is in that list.

## 4. Order of work
1. login_env: write the new control test with the 2s-delay fixture and the final 5s
   timeout; confirm it passes once added (sanity, unrecorded).
2. Record the forced failing run (temporary 1s edit), revert, record the passing run
   (section 1b) - both via direct `cargo test`, both at the final head.
3. Rewrite the sibling test's stale comment (1a).
4. `cargo test -p steg-server --lib login_env::` once more, recorded, as the
   module-level check.
5. attach.rs: pre-build the four variants, run Condition A, then Condition B within its
   bounded window, record results in the notes.
6. Apply whichever fix 2d selects; recheck the specific test(s) it names, or the whole
   file if the default applies.
7. Final `just check` at the end (section 6).

## 5. Risks
- The attach.rs experiment may not reproduce within its stated budget even though the
  real sighting did at higher concurrency - the fallback (section 2d) ships regardless,
  stated honestly as unconfirmed at this task's own bound, the same honesty standard
  TASK-1.123 used for its own unreproduced class.
- Pre-building the four `attach` binaries and locating them under `target/debug/deps/`
  by name is a bit more fragile than re-invoking `cargo test` directly (binary names
  carry a build hash) - mitigate by reading each `--no-run`'s own stderr/stdout for the
  exact path it printed, rather than globbing by name.
- The new control test in `login_env.rs` adds a second ~15-30s-bounded test to a module
  that already has slow tests; this is accepted cost for a genuinely forced,
  deterministic control rather than a load-dependent one.

## 6. Proof per acceptance criterion
- AC #1: section 1c.
- AC #2: section 2e, naming which test(s) the experiment found, or stating plainly that
  none did.
- AC #3: section 2d's fallback path, applied and recorded, only if 2d's null-result
  branch is reached; the stated limit (capped below the sighted severity, slot-throttle
  uncertainty) written into the notes either way.
- AC #4: no test weakened or skipped, no liveness bound removed (the 15s/30s bounds in
  `login_env.rs` are untouched; the only inner bound raised is the test's own 1s
  functional timeout, raised rather than removed) - `just check` passes at the final
  head, recorded via `asdlc record-check`.

## 7. Open questions for the engineer, with recommended defaults
1. The new control test's name
   (`capture_login_environment_tolerates_a_delayed_pid_file_write_from_a_backgrounded_grandchild`)
   - recommended default: use it as proposed, or shorten if the engineer prefers; no
     functional stake either way.
2. Whether a null result in the attach.rs experiment (section 2d) should still trigger
   a separate, higher-concurrency follow-up task given the slot-throttle uncertainty
   (section 0) - recommended default: file one only if the bounded experiment here
   finds nothing, named explicitly rather than silently dropped.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Plan checked against the task's criteria and .claude/rules/testing-rust.md by @bruk.o on 2026-11-20: ok. It touches only login_env.rs's tests and attach.rs, stays out of TASK-1.140's files, raises a test-only timeout (the product default is separate and unchanged), keeps every liveness bound, and its login_env control is a committed delayed-write test recorded failing at the old bound and passing at the new, at the final head. The attach.rs part is an experiment with a stated decision rule and a stated fallback; if nothing reproduces the task says so. Two corrections to TASK-1.137's diagnosis are accepted: 23 attach tests with 5 guarded, and the four feature-variant runs are sequential within one worktree. The planner's two questions take its recommended defaults unless the engineer says otherwise.

Check FAILED: `just check` exited 101 at 2026-11-20T17:38:51+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.143 on task-1.143-server-tests-the-login-env-grandchild at e4b4c98ad1c0, last 40 of 78 lines:

```text
test pane::attach_tests::a_program_that_exits_on_its_own_sends_ended_with_its_real_exit_code_after_its_last_content ... ok
test pane::attach_tests::mark_closing_sent_before_a_programs_own_exit_is_the_reason_ended_reports ... ok
test runtime::events::tests::an_unfiltered_subscriber_receives_nothing ... ok
test runtime::input::tests::the_general_sizing_step_reaches_every_pane_through_split_close_and_swap ... ok
test pane::ending_tests::await_exit_is_not_delayed_by_a_backlog_on_the_workers_own_queue ... ok
test pane::ending_tests::signal_hangup_reaches_a_backgrounded_grandchild_too ... ok
kill: 40809: No such process
test runtime::lifecycle::stop_tests::stop_ends_a_program_that_traps_the_polite_signal ... ok
test idle::tests::the_checker_blocks_with_no_timeout_while_ineligible_and_waits_exactly_once_while_eligible ... ok
test outbound::tests::enqueue_control_and_wait_until_written_does_not_return_until_write_all_itself_returns ... ok
test runtime::events::tests::a_mark_then_the_shells_own_real_exit_cascades_with_no_panic ... ok
test test_support::tests::a_body_that_never_returns_fails_fast_naming_what_it_waited_for - should panic ... ok
test runtime::events::tests::a_shell_with_no_marks_leaves_every_status_field_unchanged_until_one_arrives ... ok
test pane::ending_tests::signal_kill_ends_a_program_that_traps_the_polite_signal ... ok
test runtime::events::tests::a_typed_command_that_succeeds_is_running_then_finished_and_the_tab_shows_it ... ok
test runtime::events::tests::a_typed_command_that_fails_is_failed_with_its_exit_code ... ok
test runtime::events::tests::a_typed_commands_status_reaches_pane_show_and_tab_show_through_runtime_handle ... ok
test runtime::events::tests::a_command_panes_own_program_printing_marks_does_not_change_its_status ... ok
kill: 40750: No such process
test pane::tests::a_forward_failing_once_the_worker_is_gone_still_reaps_a_program_that_ends_on_its_own ... ok
test login_env::tests::capture_login_environment_falls_back_on_a_shell_that_exits_non_zero ... ok
test login_env::tests::capture_login_environment_prints_noise_before_the_marker_without_corrupting_the_dump ... ok
test login_env::tests::capture_login_environment_kills_a_backgrounded_grandchild_that_held_the_pipe_open ... FAILED

failures:

---- login_env::tests::capture_login_environment_kills_a_backgrounded_grandchild_that_held_the_pipe_open stdout ----

thread 'login_env::tests::capture_login_environment_kills_a_backgrounded_grandchild_that_held_the_pipe_open' (67485561) panicked at crates/server/src/login_env.rs:675:13:
the backgrounded grandchild's own pid file never appeared


failures:
    login_env::tests::capture_login_environment_kills_a_backgrounded_grandchild_that_held_the_pipe_open

test result: FAILED. 220 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 30.02s

error: test failed, to rerun pass `-p steg-server --lib`
full output: .quiet/test-steg-server.log
error: recipe `check-packages` failed with exit code 101
```
<!-- SECTION:NOTES:END -->

## Comments

<!-- COMMENTS:BEGIN -->
author: @bruk
created: 2026-11-20T17:34:00Z

---

Plan approved by @bruk (880746049d3b, size-2): its worker may build it.

---
<!-- COMMENTS:END -->
