---
id: TASK-1.143
title: >-
  Server tests: the login_env grandchild and attach tests stop failing under
  sibling builds
status: Done
assignee:
  - '@bruk.w3'
created_date: '2026-11-20T17:21:00Z'
updated_date: '2026-11-21T19:50:00Z'
labels:
  - size-2
  - review-passed
milestone: m-3
dependencies: []
references:
  - task-1.143-server-tests-the-login-env-grandchild
  - https://github.com/bruk-io/steg/pull/88
parent_task_id: TASK-1
project: bruk-io-steg
asdlc_accepted_from: DRAFT-2
asdlc_branch: task-1.143-server-tests-the-login-env-grandchild
asdlc_plan_approved_by: bruk
asdlc_plan_approved_hash: '880746049d3b'
asdlc_plan_approved_size: size-2
asdlc_review_kind: same-machine
asdlc_review_findings: 'important=0,nit=1'
asdlc_review_policies: ["review"]
asdlc_plan_matched: yes
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
## login_env (AC #1)

Fix: raised `capture_login_environment_kills_a_backgrounded_grandchild_that_held_the_pipe_open`'s own inline timeout from 1s to 5s (`crates/server/src/login_env.rs`). The product's own default, `DEFAULT_LOGIN_ENVIRONMENT_TIMEOUT` (3s), is untouched and was never 1s - only this test's own tighter number changed. Rewrote the test's own stale comment.

Organic confirmation: this task's own baseline `just check` (recorded on this task before any fix) failed this exact way twice in a row, both times "the backgrounded grandchild's own pid file never appeared" at ~30s, with no forcing at all.

Deterministic control (AC #1): added a permanent, committed test, `capture_login_environment_tolerates_a_delayed_pid_file_write_from_a_backgrounded_grandchild`, using a fixture with a deliberate 2s write delay (the one sleep in this module's tests that is not a poll for an event - the independent variable the control forces). At the final head (committed timeout 5s), it passes. Recorded failing+passing pair, both at the final head: temporarily lowered only this test's own timeout to the old 1s bound, ran and recorded it failing (~30.0s, "the backgrounded grandchild's own pid file never appeared" - identical mechanism, forced rather than waited for); reverted (confirmed via `git diff` - byte-identical to the committed state) and recorded it passing. Both runs read back clean of anything from the environment before being recorded, per this task's safety rule.

## attach.rs (AC #2, #3)

Experiment (bounded: at most two sibling load processes, load window kept to a few minutes total, well inside the stated ~10-minute cap): pre-built the four `steg-server` feature-variant `attach` test binaries (default, race-fixture, pty-race-fixture, accept-backoff-fixture) once each with `--no-run`, then ran the four already-built binaries directly and concurrently (default thread count, not serialised), to isolate execution-time contention from Cargo's own build-lock serialisation.

- Condition A (self-contention only, no sibling load): 4/4 binaries passed, 0 failures - matches TASK-1.123's own prior measurement at this level.
- Condition B (the same four binaries, plus two sibling `cargo build --workspace` processes in throwaway clones as load, genuinely overlapping): 2 repetitions with real overlapping load. 3 of those 8 runs (2 reps x 4 binaries) failed - every one of them the identical test, `a_second_connections_control_call_is_not_blocked_by_a_stuck_panes_lock_held_attach` (one of the 5 tests already under `cross_process_flood_guard`), every one of them with the identical message: "timed out after 60s waiting for the cross-process flood-test lock... another worktree's own flood test held it for longer than this bound allows". No other test in the file failed, in either condition. (A third attempted repetition used too weak a sibling load, by mistake - a partial `cargo clean` that rebuilt in ~2s - and is not counted as a genuine Condition B trial.)

Decision (per the plan's decision rule, section 2d): this names a specific, already-guarded test and its exact failure mode - the lock's own 60s acquire bound, not a missing serialisation (the lock already serialises machine-wide, across worktrees, confirmed by reading `flood_lock_path`). The five guarded tests run four-to-a-worktree (the four feature variants), so one worktree's own run can already queue up to 20 acquisitions of one lock; a second worktree building concurrently slows whichever process holds it, adding more queue depth. Fix: raised the bound from 60s to 180s (`cross_process_flood_guard`, `crates/server/tests/attach.rs`) - comfortably past the observed failures (all at 60s) and past the observed total suite time under this same load (~150s worst case), while remaining a liveness bound, not removed. This is a targeted fix, not the task's stated fallback (widening the guard to all 23 tests) - the evidence does not support the fallback, since none of the 18 unguarded tests failed under either condition.

Verified: re-ran the identical Condition B load once more with the fix in place - all four variants passed; three of them showed the same lock wait cross 60s (logged via the test harness's own "has been running for over 60 seconds" notice) and still succeeded at the new 180s bound.

Limit stated plainly (AC #3's own spirit, even though the fallback itself was not needed): this experiment is capped at two sibling load processes, below the sighted severity ("three worktrees building"), and `STEG_CHECK_SLOTS`'s own default (2) may not have been active on every sighted worktree if any predated TASK-1.132 - a null result would not have ruled out the mechanism at higher concurrency, though this experiment was not a null result.

## Checks recorded (via `asdlc record-check`)
- Baseline `just check` (twice, before any fix): both FAILED, both the login_env flake, organically.
- `cargo test -p steg-server --lib login_env::tests::capture_login_environment_tolerates_a_delayed_pid_file_write_from_a_backgrounded_grandchild -- --exact`: forced failing (old 1s bound), then passing (final 5s bound), both at the final head.
- `cargo test -p steg-server --lib login_env::`: passing (module-level, all 13 tests).
- `just check-packages`: passing (all packages, all four `steg-server` feature variants).
- `just check` (full, before and after the rebase onto `origin/main`): both passing.

PR #88: https://github.com/bruk-io/steg/pull/88

Check FAILED: `env ZIG=/Users/brukhabtu/.local/share/mise/installs/zig/0.17.1/bin/zig cargo test -p steg-server --lib login_env::tests::capture_login_environment_tolerates_a_delayed_pid_file_write_from_a_backgrounded_grandchild -- --exact` exited 101 at 2026-11-20T18:41:00+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.143 on task-1.143-server-tests-the-login-env-grandchild at 97005f3c4faf with uncommitted changes:

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.08s
     Running unittests src/lib.rs (target/debug/deps/steg_server-6d2f1d51fdd2e018)

running 1 test
test login_env::tests::capture_login_environment_tolerates_a_delayed_pid_file_write_from_a_backgrounded_grandchild ... FAILED

failures:

---- login_env::tests::capture_login_environment_tolerates_a_delayed_pid_file_write_from_a_backgrounded_grandchild stdout ----

thread 'login_env::tests::capture_login_environment_tolerates_a_delayed_pid_file_write_from_a_backgrounded_grandchild' (68415581) panicked at crates/server/src/login_env.rs:766:13:
the backgrounded grandchild's own pid file never appeared
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    login_env::tests::capture_login_environment_tolerates_a_delayed_pid_file_write_from_a_backgrounded_grandchild

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 221 filtered out; finished in 30.01s

error: test failed, to rerun pass `-p steg-server --lib`
```

Check passed: `env ZIG=/Users/brukhabtu/.local/share/mise/installs/zig/0.17.1/bin/zig cargo test -p steg-server --lib login_env::tests::capture_login_environment_tolerates_a_delayed_pid_file_write_from_a_backgrounded_grandchild -- --exact` exited 0 at 2026-11-20T18:41:29+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.143 on task-1.143-server-tests-the-login-env-grandchild at 97005f3c4faf:

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.08s
     Running unittests src/lib.rs (target/debug/deps/steg_server-6d2f1d51fdd2e018)

running 1 test
kill: 25474: No such process
test login_env::tests::capture_login_environment_tolerates_a_delayed_pid_file_write_from_a_backgrounded_grandchild ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 221 filtered out; finished in 5.02s
```

Check passed: `env ZIG=/Users/brukhabtu/.local/share/mise/installs/zig/0.17.1/bin/zig cargo test -p steg-server --lib login_env::` exited 0 at 2026-11-20T18:41:41+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.143 on task-1.143-server-tests-the-login-env-grandchild at 97005f3c4faf:

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.08s
     Running unittests src/lib.rs (target/debug/deps/steg_server-6d2f1d51fdd2e018)

running 13 tests
test login_env::tests::fallback_env_omits_user_and_logname_when_the_identity_itself_has_none ... ok
test login_env::tests::parse_env_dump_splits_a_value_containing_its_own_equals_sign_on_the_first_one_only ... ok
test login_env::tests::fallback_env_is_exactly_the_named_constant_set ... ok
test login_env::tests::parse_env_dump_skips_a_malformed_entry_with_no_equals_sign ... ok
test login_env::tests::capture_login_environment_falls_back_on_a_shell_that_does_not_exist ... ok
test login_env::tests::resolve_login_identity_never_panics_and_returns_a_home_a_shell_and_a_user_that_exist ... ok
test login_env::tests::capture_login_environment_sources_the_login_profile_not_the_interactive_one ... ok
test login_env::tests::capture_login_environment_never_leaks_a_variable_set_only_in_the_callers_own_environment ... ok
test login_env::tests::capture_login_environment_falls_back_on_a_shell_that_never_finishes_in_time ... ok
test login_env::tests::capture_login_environment_falls_back_on_a_shell_that_exits_non_zero ... ok
test login_env::tests::capture_login_environment_prints_noise_before_the_marker_without_corrupting_the_dump ... ok
kill: 25972: No such process
kill: 25973: No such process
test login_env::tests::capture_login_environment_kills_a_backgrounded_grandchild_that_held_the_pipe_open ... ok
test login_env::tests::capture_login_environment_tolerates_a_delayed_pid_file_write_from_a_backgrounded_grandchild ... ok

test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 209 filtered out; finished in 5.02s
```

Check passed: `just check-packages` exited 0 at 2026-11-20T18:48:11+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.143 on task-1.143-server-tests-the-login-env-grandchild at 97005f3c4faf:

```text
ok      build (steg-sdk) (0.1s)
ok      test (steg-sdk) (0.3s)
ok      test (steg-sdk, local-socket) (0.5s)
ok      build (steg-kernel) (0.1s)
ok      test (steg-kernel) (0.2s)
ok      build (steg-server) (0.5s)
ok      test (steg-server) (64.8s)
ok      test (steg-server, race-fixture) (69.2s)
ok      test (steg-server, pty-race-fixture) (101.1s)
ok      test (steg-server, accept-backoff-fixture) (122.3s)
ok      build (steg) (0.9s)
ok      test (steg) (5.1s)
ok      test (steg, fixtures) (11.6s)
ok      build (steg-ui) (0.4s)
ok      test (steg-ui) (0.4s)
ok      build (steg-app) (0.3s)
ok      test (steg-app) (0.8s)
```

Check passed: `just check` exited 0 at 2026-11-20T18:55:14+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.143 on task-1.143-server-tests-the-login-env-grandchild at 97005f3c4faf:

```text
ok      format check (0.3s)
ok      lint (2.6s)
ok      boundaries (0.1s)
ok      foundations (0.1s)
ok      foundations public surface (0.1s)
ok      design pages (0.0s)
ok      licences (0.7s)
ok      tests (-p steg -p steg-server) (94.2s)
ok      build (steg-sdk) (0.1s)
ok      test (steg-sdk) (0.3s)
ok      test (steg-sdk, local-socket) (0.5s)
ok      build (steg-kernel) (0.1s)
ok      test (steg-kernel) (0.2s)
ok      build (steg-server) (0.1s)
ok      test (steg-server) (87.8s)
ok      test (steg-server, race-fixture) (56.5s)
ok      test (steg-server, pty-race-fixture) (63.6s)
ok      test (steg-server, accept-backoff-fixture) (58.3s)
ok      build (steg) (0.1s)
ok      test (steg) (1.9s)
ok      test (steg, fixtures) (5.0s)
ok      build (steg-ui) (0.3s)
ok      test (steg-ui) (0.4s)
ok      build (steg-app) (0.3s)
ok      test (steg-app) (0.8s)
ok      tool tests (40.1s)
ok      architecture (0.7s)
ok      recipe names (0.0s)
ok      ui levels (0.0s)
ok      ui colours (0.0s)
```

Final head: 97005f3c4faf8cc3dd5ca530a3d7ec89ce7ae29e. PR #88, CI green.

login_env (AC #1): raised the grandchild test's own inline timeout 1s -> 5s; product default (3s) untouched. Baseline just check (recorded before any fix) failed this exact way twice organically. Deterministic control (capture_login_environment_tolerates_a_delayed_pid_file_write_from_a_backgrounded_grandchild, a permanent committed test with a fixture's own deliberate 2s write delay) recorded failing at the old 1s bound and passing at the new 5s bound, both at this final head.

attach.rs (AC #2, #3): bounded experiment (<=2 sibling cargo build --workspace processes as load, well inside the ~10-minute cap; four pre-built steg-server feature-variant attach binaries run directly together). Condition A (no sibling load): 4/4 passed. Condition B (2 repetitions with genuine sibling load): 3 of 8 runs failed, always the same already-guarded test, a_second_connections_control_call_is_not_blocked_by_a_stuck_panes_lock_held_attach, always at the identical "timed out after 60s waiting for the cross-process flood-test lock" message - no other test in the file failed, so none of the 18 unguarded tests are implicated. Fix: raised cross_process_flood_guard's own lock-acquire bound 60s -> 180s (a targeted fix, not the stated fallback, since the evidence named a specific already-guarded test's own bound, not a missing serialisation). Verified by re-running the identical load condition once more post-fix: all four variants passed.

Checks recorded via asdlc record-check, each a direct command (no sh -c/pipe/tail), all re-recorded fresh at this final head after an earlier --notes edit of mine had replaced (not appended to) the Notes section and wiped the record-check blocks that had accumulated at stale, pre-rebase commit hashes - corrected by re-running every check directly against the clean final head and using --comment (append) rather than --notes (replace) for my own summary:
- just check: FAILED x2 at the pre-fix baseline (organic login_env flake), then passing after the fix, both before and after the rebase onto origin/main, and once more at this final head.
- login_env control pair: forced failing (old 1s bound) then passing (final 5s bound), both at this final head, each read back clean of anything from the environment before recording.
- cargo test -p steg-server --lib login_env:: (module-level): passing at this final head.
- just check-packages: passing at this final head (all four steg-server feature variants, all other packages).

No test weakened or skipped; no liveness bound removed (the 15s/30s login_env bounds and the attach.rs lock bound were only raised, with their comments explaining why).

No experiment-generated process or throwaway clone left running or on disk (ps checked, clones removed) - only raw captured output scratch logs remain, kept under the session scratchpad, not the board.

Files touched: crates/server/src/login_env.rs, crates/server/tests/attach.rs - no overlap with TASK-1.140.

Restored by @bruk.o on 2026-11-20 after the worker's --notes edit replaced this section: the plan (hash 880746049d3b) was checked against the task's criteria and .claude/rules/testing-rust.md by @bruk.o on 2026-11-20 before the engineer approved it: ok; the planner's two open questions took its recommended defaults. The earlier recorded checks that the edit removed are still in the board's git history.
<!-- SECTION:NOTES:END -->

## Comments

<!-- COMMENTS:BEGIN -->
author: @bruk
created: 2026-11-20T17:34:00Z

---

Plan approved by @bruk (880746049d3b, size-2): its worker may build it.

---

created: 2026-11-20T18:55:00Z

---

Build summary (worker's own account; see the record-check blocks above for the raw evidence).

**login_env (AC #1).** Raised the grandchild test's own inline timeout 1s -> 5s (crates/server/src/login_env.rs); the product's own default (DEFAULT_LOGIN_ENVIRONMENT_TIMEOUT, 3s) is untouched. This task's own baseline `just check`, recorded before any fix, failed this exact way twice in a row with no forcing at all - organic confirmation. Added a permanent, committed control test (capture_login_environment_tolerates_a_delayed_pid_file_write_from_a_backgrounded_grandchild) using a fixture's own deliberate 2s write delay; recorded it failing at the old 1s bound and passing at the new 5s bound, both at this exact final head (97005f3c), each confirmed clean of anything from the environment before recording.

**attach.rs (AC #2, #3).** Bounded experiment (at most two sibling cargo build --workspace processes in throwaway clones as load, well inside the ~10-minute cap; pre-built the four steg-server feature-variant attach binaries with --no-run and ran them directly to avoid Cargo's own build-lock confound): Condition A (self-contention only) - 4/4 passed, 0 failures. Condition B (plus genuine sibling load, 2 repetitions) - 3 of 8 runs failed, every one of them the identical already-guarded test (a_second_connections_control_call_is_not_blocked_by_a_stuck_panes_lock_held_attach), every one at the identical message ("timed out after 60s waiting for the cross-process flood-test lock... another worktree's own flood test held it for longer than this bound allows"). No other test in the file failed in either condition, so none of the 18 unguarded tests are implicated. This named a targeted fix rather than the stated fallback: raised cross_process_flood_guard's own lock-acquire bound 60s -> 180s, reasoned from the measured queue depth (5 guarded tests x 4 feature-variant processes = up to 20 acquisitions of one machine-wide lock, worse under sibling CPU load) and the observed worst-case total suite time (~150s) under the same load. Verified: re-ran the identical load condition once more with the fix in place - all four variants passed, three of them crossing 60s on the same lock wait and still succeeding at 180s.

PR #88: https://github.com/bruk-io/steg/pull/88, head 97005f3c4faf8cc3dd5ca530a3d7ec89ce7ae29e, CI green.

---

author: @bruk.r2
created: 2026-11-20T18:56:00Z

---

@bruk.r2 is reviewing TASK-1.143: a same-machine review: it waited 0m with no other engineer's reviewer, longer than review_fallback_after (0m), so a fresh reviewer on its author's machine takes it.

---

author: @bruk.r2
created: 2026-11-20T19:01:00Z

---

Review by @bruk.r2 (same-machine): clean. It stays In Review for a person to merge.

Findings: 0 Important, 1 nit. Cites: review. The diff matches the plan.

No Important findings.

[Compliance, Nit] PR #88 history: the worker used `git rebase` where the brief said merge, never rebase a pushed branch. Checked via `gh api .../pulls/88/commits` and the issue timeline: all three commits (429c767d, 629b11e, 97005f3c) share committer date 2026-11-20T18:14:46Z, before the PR was opened (18:22:05Z); the timeline shows only "committed" events, no head_ref_force_pushed. The rebase happened before the branch's first push, not a rewrite of already-pushed/reviewed history, so the concern behind "never rebase a pushed branch" doesn't apply here. It is disclosed in the notes ("before and after the rebase onto origin/main"). Recorded as a Nit for process hygiene only; no pushed history was rewritten.

Verified independently (brief point 5): in a fresh review worktree (removed after), temporarily lowered capture_login_environment_tolerates_a_delayed_pid_file_write_from_a_backgrounded_grandchild's own timeout argument from 5s to 1s (the only line touched) - failed at 30.00s with exactly "the backgrounded grandchild's own pid file never appeared", matching the recorded forced-failure run. Reverted (confirmed byte-identical via git diff) - passed in 5.06s, matching the recorded passing run. The control is genuine, not narrated.

Evidence: every recorded check on the task names the final head (97005f3c), was run directly (no sh -c/pipe/tail), and together cover the login_env control pair, the login_env module, `just check-packages`, and `just check`; only the deliberate forced-failure run carries "with uncommitted changes", as expected. The record-check blocks an earlier --notes edit wiped, and the orchestrator's restored plan-approval note, are already accounted for on the task and not re-raised here.

login_env (AC #1): only the test's own inline timeout changed (1s -> 5s); the product default (DEFAULT_LOGIN_ENVIRONMENT_TIMEOUT, 3s) and the 15s/30s liveness bounds are untouched. The new control's 2s fixture delay is commented as the deliberate, stated exception.

attach.rs (AC #2-4): the fix (cross_process_flood_guard's acquire bound 60s -> 180s) is tied to a disclosed, bounded experiment (2 reps x 4 binaries under genuine sibling load; 3/8 failed, all the same already-guarded test, all at the old 60s bound) and derived from observed numbers (worst-case total suite time ~150s), not guessed. The one post-fix repetition is reported as exactly that ("re-ran ... once more"), not overclaimed as proof. Raising this bound lengthens a wait for a contended lock; it does not weaken an assertion or remove a liveness bound (AC #4).

No findings under Boundaries, Licence, Rust or Security. Files touched match the plan (crates/server/src/login_env.rs, crates/server/tests/attach.rs), no overlap with TASK-1.140. Plan approved by @bruk (880746049d3b, size-2), comment #1.

---
<!-- COMMENTS:END -->
