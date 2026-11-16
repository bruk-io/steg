---
id: TASK-1.112
title: >-
  Server: opening a pseudo-terminal still fails now and then under concurrent
  opens
status: In Progress
assignee:
  - '@bruk.w9'
created_date: '2026-11-16T04:53:00Z'
updated_date: '2026-11-16T13:47:00Z'
labels:
  - size-2
milestone: m-3
dependencies: []
parent_task_id: TASK-1
project: bruk-io-steg
asdlc_accepted_from: DRAFT-1
asdlc_branch: task-1.112-server-opening-a-pseudo-terminal-still
asdlc_plan_approved_by: bruk
asdlc_plan_approved_hash: '4592947e385d'
asdlc_plan_approved_size: size-2
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Seen on 2026-11-16 (UTC) on main at d06ab92 (the merge of pull request 52, which changed only Python tools): crates/server/tests/real_pty.rs many_threads_opening_a_real_pty_at_once_never_fails failed at round 10, thread 27 of 32 with 'RealLauncher::spawn failed under concurrent opens: the pseudo-terminal failed: failed to openpty: Os { code: 6, Device not configured }', in the step that tests steg-server alone. The same code passed CI on pull request 47's head, which contains that commit. TASK-1.76 added the process-wide OPENPTY_LOCK around openpty on the orchestrator's decision (a lock and no retry) after its review isolated a race between concurrent openpty calls on macOS; this shows the lock is not the whole answer. It is a real defect and not only a flaky test: a user's pane could fail to open. crates/server/src/pty.rs.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The cause of the failure seen in CI is established, not assumed: openpty returned 'Device not configured' (ENXIO) in many_threads_opening_a_real_pty_at_once_never_fails while the process-wide lock around openpty was held, so the race is not only between two openpty calls in one process. The plan finds what else it races with (the spawn that follows and opens the slave, a close of another pty's descriptors on another thread, another process on the machine, a limit) by experiment on macOS, and says which
- [ ] #2 A pane whose pseudo-terminal cannot be opened after that still fails cleanly with the error doc-2 gives, leaving no half-made pane, thread or child
- [ ] #3 Shown by repeated runs: the test passes N times in a row locally under load and on CI (a temporary workflow that repeats it, removed before merge), with N justified against the failure rate seen; and a negative control: with the fix removed, the experiment that reproduces the failure does so
- [ ] #4 The fix addresses that cause: a bounded retry of the open step alone (not of starting the program), on any failure of that step, because the library discards the structured error and the code was seen with both signs; the bound and its evidence are stated, each retry is counted and reported, and when every attempt fails the last error surfaces with the number of attempts (reworded on 2026-11-16 when the engineer approved the plan; it read: on this one error, not a blanket retry)
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
## For the engineer

1. **The retry bound.** Default 5 attempts in total (raised from an earlier draft's 3), no sleep between them, as a field on `RealLauncher`'s own constructor (`RealLauncher::default()` = 5, `RealLauncher::new(n)` for a test - a test builds `RealLauncher::new(1)` to force "no retry"). Reasoning: the shipped test's own false-failure budget at 3 attempts (~1 in 5,000) was too loose for something that runs on every `just check` and every CI run, and rested on treating attempts as independent - optimistic, since an immediate retry right after a collision lands in the same burst of contention that caused it, so the true rate is probably worse than the idealised number. An extra attempt costs a real server nothing. At 5 attempts (same independence caveat, restated below with the measurement that checks it): the shipped test's own false-failure chance drops to roughly 1 in 305 million - see "Proof, layer (b)" for the number and the caveat together, and "Order of work" for the measurement that tests the assumption instead of trusting it.
2. **The proof test's shape**: a deterministic unit test behind a small seam in `pty.rs` (layer (a)), and a statistical test across real, separate processes reusing the already-reviewed `race-fixture`/`race-attempt` pattern in this crate (layer (b)) - confirm you're fine with a second instance of that pattern before I build it.

## Why this is not only a test problem

A real `steg-server` is one process per person per machine (`crates/server/CLAUDE.md`), but a real pane's `openpty()` still shares this platform's pty subsystem with whatever *else* is running on that same machine at that moment - another terminal app, `tmux`, a second person's own `steg-server`, the OS's own activity. None of those would ever honour a lock file steg wrote for itself. So this is not a fix that only quiets a test's own self-inflicted load: it is the same collision a real pane can meet, just far less often, because nothing else on an ordinary machine opens ptys anywhere near as densely as this task's own fixture does on purpose.

## Cause, and the evidence for it

**What's established, not assumed:**

`openpty()` on this platform is not safe against *another OS process* calling it at the same moment, independent of this crate's own code or `portable-pty` - a process-wide lock (`OPENPTY_LOCK`, TASK-1.76) cannot reach this, because a lock is local to the process that holds it.

Reproduced directly, isolated from everything steg-specific: a bare `ctypes` call straight to `libc.openpty()`, run from several **separate OS processes** (not threads - each worker is single-threaded, so there is zero in-process concurrency to blame), each serialising its own calls in a tight loop with no coordination between workers at all:

| concurrent processes | opens attempted | failures | rate |
|---|---|---|---|
| 1, 2, or 4 | 300 each | 0 | 0% |
| 8 | 800 | 1 | ~0.1% |
| 16 | 1,600 (×5 trials) | 5-7 per trial | ~0.3-0.4% |

Closing the fds is not required for the race (a variant that only opens, never closes, reproduces it too, at a comparable rate) - it is concurrent *opens* across processes that race, not a close interacting with an open. `kern.tty.ptmx_max` is 511; at most ~22 ptys were ever alive during any of this - the same ceiling TASK-1.76's own review already ruled out for the in-process case, confirmed again here, not assumed carried over.

The identical failure, through steg's own real dependency (not a bare libc stand-in): calling `portable_pty::native_pty_system().openpty()` directly, from 32 threads of **one process**, with `OPENPTY_LOCK` bypassed (not production code - the point was only to capture the exact rendered text this dependency produces on this exact race, on this machine), reproduced the error on 7 of 8 tries, every time rendering as:

```
failed to openpty: Os { code: -6, kind: Uncategorized, message: "Unknown error: -6" }
```

This is the literal text, from this crate's own dependency, not a guess - and the reason the fix below does not try to match it (see "For the engineer", #2).

**What's ruled out, not assumed:**

- **In-process contention under the current lock, even sustained, even doing the real work (not bare opens).** 32 threads in one process, for 4 continuous seconds (not one synchronised round), each doing a full `RealLauncher::spawn` - lock held only for the `openpty()` call exactly as production does, real `spawn_command`, real child, real `wait()` all outside the lock: 2,405 real spawns, 0 failures. A matching sanity check with no lock at all, same harness: 1 failure in 1,277 - confirming the harness itself can catch the race when it's actually exposed, so the locked run's zero is not "the harness can't see it" (see "What stays of the lock" below for what this one number is for).
- `real_pty.rs` run alone, repeatedly: 8/8 clean (matches TASK-1.76's own review finding).
- The full `real_pty.rs` binary run as several genuinely separate OS processes at once (8, then 16 concurrent copies, 3 rounds each): 0 failures across 64 runs (~20,000+ real opens through the actual `RealLauncher` path). This is an honest negative result, not a disproof - production's per-op cost (a real `fork`+`exec` of a shell) is far higher than a bare syscall, so the same narrow race window is sampled far less often per second of wall clock than the bare-libc probe above, which is consistent with how rare the one recorded CI occurrence was. This is exactly why the statistical proof test (layer (b) below) drives the open step alone, not the full spawn.

**The real mechanism, confirmed in this repository, not hypothesised:** `cargo test -p steg-server` - the exact command `just test`/`just check`'s "test" step and CI both run - launches more than one of this crate's test binaries **concurrently, as separate OS processes**. Sampled directly (`ps` polled every 30ms through a real run): `real_pty`'s own binary and `runtime.rs`'s binary (TASK-1.76's own "18 tests, most real-pty") were both alive at once in several samples; `routing` and `handshake` each overlapped with `runtime` too. Each test binary is its own process, so each has its **own** `OPENPTY_LOCK` - `real_pty`'s lock cannot serialise against an open happening in `runtime`'s process, or vice versa. This is "CI runs test binaries in parallel" as a measured fact about this exact crate's own test suite, not a guess about a shared machine.

**Conclusion for AC #1:** the residual race is across OS process boundaries - demonstrated mechanically (the bare-libc, zero-steg-code reproduction) and demonstrated as reachable in exactly this project's own test run (sibling test binaries of this same crate, concurrent, each with an independent lock). The same mechanism reaches a real server too, against unrelated programs on the same machine (see "Why this is not only a test problem" above) - rarer there, because nothing ordinary opens ptys as densely as this task's own tests do on purpose, but not excludable, which is exactly AC #2's own condition for a bounded retry.

## The fix

Keep `OPENPTY_LOCK` exactly as `pty.rs` has it today (see "What stays of the lock", below). Wrap the one call to `pty_system.openpty(...)` in a bounded retry, still entirely inside the lock, on **any** error - not a specific one. Why not match the error's own text (reversing this plan's first draft): the sign already disagreed between two sightings (`-6` reproduced directly through this exact dependency; `6` in the task's own description of the CI incident) and `portable-pty` discards the structured `io::Error` before it reaches us (`unix.rs:46`: `bail!("failed to openpty: {:?}", io::Error::last_os_error())` - a formatted string inside an `anyhow::Error`, confirmed by reading the source, not assumed). Retrying on any error instead costs a genuine, non-transient failure (the system's own pty limit, a real permission problem) only a few extra syscalls before it is reported exactly as it is today, just with the attempt count added - nothing is masked, and nothing depends on a string that could stop matching on a dependency bump. I don't see a reason to narrow this further.

```rust
impl RealLauncher {
    pub fn new(max_open_attempts: u32) -> Self { ... } // asserts >= 1
}
impl Default for RealLauncher {
    fn default() -> Self { Self::new(5) }
}

// inside RealLauncher::spawn, still under OPENPTY_LOCK:
let (pair, attempts) = open_with_retry(self.max_open_attempts, || {
    pty_system.openpty(portable_pty::PtySize { rows, cols, pixel_width: 0, pixel_height: 0 })
})?;
```

`open_with_retry` (new, private, the seam layer (a)'s unit tests drive directly) takes the bound and a closure for the open call itself - not a mock of `portable_pty`, a plain function parameter:

```rust
fn open_with_retry<T, E>(
    max_attempts: u32,
    mut try_open: impl FnMut() -> Result<T, E>,
) -> Result<(T, u32), (E, u32)> {
    let mut attempt = 0;
    loop {
        attempt += 1;
        match try_open() {
            Ok(value) => return Ok((value, attempt)),
            Err(e) if attempt < max_attempts => continue,
            Err(e) => return Err((e, attempt)),
        }
    }
}
```

On exhaustion, the attempt count goes into `Error::Pty`'s message (e.g. `"failed to openpty after {attempts} attempts: {cause}"`) - checked against every current test that asserts on this error (`grep -rn "failed to openpty" crates/server`): none assert exact text today, only `matches!(_, Err(Error::Pty(_)))` or the `ErrorCode`, so this is additive.

## What's reported when a retry happens

A server that retries silently hides exactly the condition this task found. New `PaneEvent::OpenRetried { pane: PaneId, attempts: u32 }` (same `#[derive(Debug, Clone, Copy, PartialEq, Eq)]` `PaneEvent` already has - `PaneId` is a plain `Copy` newtype, confirmed by reading `crates/sdk/src/ids.rs:44`), the same shape TASK-1.98 is adding `ConnectionEvent::AcceptRetried` for - read that task's own plan before building either, since they land around the same time on sibling enums.

Wiring: `pane::start` already has `events: Sender<PaneEvent>` in scope exactly where it calls `launcher.spawn(&spec)?` (`pane.rs:565`) - if the returned `SpawnedProgram` carries its own `open_attempts: u32` (1 = no retry), `start` sends `OpenRetried` right there when it's `> 1`, before anything else. `PaneHandle::restart` (`pane.rs:441`) calls the same `launcher.spawn` but has no `events` sender on `self` - its existing `PaneCommand::Resume` gains the same `open_attempts: u32` field, and `worker_loop` (which already holds `events`) sends `OpenRetried` when it handles `Resume`, instead of plumbing a second sender through `PaneHandle`. No printing: `crates/server/src` prints nothing today (confirmed true as of TASK-1.87), and nothing here changes that - the count is observable on the channel the moment some task wires a consumer, the same gap TASK-1.98's own plan names for its own new variant.

## What stays of TASK-1.76's lock, and why

The no-lock sanity check above failed 1 time in 1,277 real opens under sustained in-process load - a case the new retry would also absorb, since it's the same kind of transient open failure. Keeping `OPENPTY_LOCK` anyway: it is already reviewed, costs nothing (serialises one call per process, never waits on anything else), and removes that entire in-process cause *deterministically* rather than leaving its (small, but nonzero) probability to compete with the cross-process case for the same finite retry budget. Removing a free, already-accepted fix for no evidentiary gain - the retry existing does not make the lock wrong, it makes the lock's job narrower (in-process only) and gives the retry a cross-process-only job, which is a cleaner division than one mechanism covering both less reliably.

## Proof

### Layer (a): deterministic, behind the seam, no real pty

Unit tests beside `open_with_retry` in `pty.rs`'s own `#[cfg(test)]` module (`.claude/rules/testing-rust.md`: unit tests prove logic, no doubles - this is a plain function parameter, not a mock of `portable_pty`):

- `fails_once_then_succeeds_counts_one_retry`: a closure that errors on its first call, then returns `Ok(())` - asserts `Ok((_, 2))`.
- `fails_every_time_surfaces_after_exactly_the_bound`: a closure that always errors - with `max_attempts = 3` (an arbitrary bound for this generic test, independent of whatever `RealLauncher::default()` ships), asserts `Err((_, 3))` and that the closure was called exactly 3 times (a counter inside the closure).
- `succeeds_first_time_needs_no_retry`: a closure that always succeeds - asserts `Ok((_, 1))`, and that the closure was only called once (so a passing "retries" test isn't secretly retrying every time).
- **Control**: with the retry loop reverted to a single call (`open_with_retry`'s body replaced by `try_open().map(|v| (v, 1))`), `fails_once_then_succeeds_counts_one_retry` fails - recorded via `asdlc record-check` both ways.

### Layer (b): statistical, across real processes

New feature `pty-race-fixture` (off by default), mirroring `race-fixture` exactly: a `[[bin]] openpty-race-attempt` (`tests/fixtures/openpty_race_attempt.rs`) and a `[[test]] openpty_race` (`tests/openpty_race.rs`), both `required-features = ["pty-race-fixture"]`.

The fixture calls a new, crate-visible function that is **only** the locked, retried open step - no `spawn_command`, closing the pair immediately after a successful open so fds don't accumulate across its own loop - so its density matches the bare-libc probe's, which is the one shape that has actually reproduced this. It loops that call a fixed number of times (an argument from the environment, the same convention `race_attempt.rs` already uses - no path or count on argv). The open step already returns `(value, attempts)`/`(error, attempts)` from `open_with_retry` itself, so the fixture tabulates attempts-per-open as it goes (how many needed 1, 2, 3, 4, or 5 attempts, and how many exhausted all 5) with no extra plumbing, and prints `done:<exhausted>:<n>:<attempt histogram>` at the end - this is also the data "Order of work" below uses for the independence measurement, from the same runs, not a separate exercise.

**Synchronised start, no periodic wake-up**: the test spawns all `N` fixture processes first, each blocking on reading one byte from its own inherited stdin before entering its loop (not a sleep-based rendezvous, not polling) - then releases all `N` within the same instant by writing to each. Every spawned process is wrapped in the same `ReapOnDrop` guard `tests/race.rs` already has (kill + wait on drop, including the panicking path), so nothing is left running, bounded, or orphaned regardless of which assertion fails.

**The numbers, from the measured rate (worst local figure, ~0.4% per open)**:

- Total opens needed so a **no-retry control** (`max_open_attempts = 1`, same as today) has a chance of showing *zero* failures below one in ten thousand: solving `(1-0.004)^n < 10⁻⁴` gives `n > 2,297`. Chosen: `N = 16` processes × `200` opens each = 3,200 total - `(0.996)^3200 ≈ 2.8×10⁻⁶`, comfortably under the bar. Unchanged by the attempts bound - this number is about single, independent opens spread across the whole run, not about retries within one open.
- The **same 3,200 opens, with the shipped default (5 attempts)**: chance of at least one open exhausting all 5, *treating attempts as independent*, is `1 - (1 - 0.004⁵)^3200 ≈ 3200 × 1.0×10⁻¹² ≈ 3.3×10⁻⁹` - about one run in 305 million, at this worst-case measured rate. **This idealised number rests on an optimistic assumption**: an immediate retry right after a collision lands inside the same burst of contention that caused it, so attempts are plausibly correlated, not independent, and the true rate could be worse than this. That's exactly what the measurement in "Order of work" below checks, rather than trusting the arithmetic - this number is reported as the starting estimate, not the final word.
- **Permanent control** (mirrors TASK-1.98's own zero-backoff control): the identical fixture and test, `max_open_attempts` forced to 1, must show at least one failure - this is what actually proves the race is real and the fixture reaches it, not merely that the fixed version happens to pass.

**If CI's own runner does not reproduce the control's failure on the first several runs**: that is read as a finding about CI's own hardware/contention density relative to this dev machine, recorded on the task with the numbers from both places side by side - not a reason to delete or weaken the control. The fix ships either way, on the strength of the bare-libc mechanism evidence and AC #2's own allowance for a cause that cannot be excluded; what changes is only how convincing the *local* proof of the control is on that specific runner, which gets written down honestly rather than papered over.

### Both layers together

- `just check`/`just test-all` clean at the final commit, `pty-race-fixture` off by default so an ordinary `cargo build -p steg-server` builds no extra binary (mirrors `race-fixture`'s own stated reason).
- A temporary CI step/job (location: wherever `TASK-1.82`'s own housekeeping notes show the project's convention for a temporary loop, checked before choosing) runs `openpty_race` some number of times with `--features pty-race-fixture`, its numbers recorded on the task, then removed in a follow-up commit of this same pull request before merge (AC #4).
- Every fixture process bounded (its own loop count, no indefinite wait), killed and reaped on every path (`ReapOnDrop`), no leaked process, thread, or socket - matching AC #3's own requirement extended to this task's new test code.

## Files that change

- `crates/server/src/pty.rs` - `RealLauncher` gains `max_open_attempts: u32` and `new`/`Default`; `open_with_retry` and its unit tests; the call site; the error message gains the attempt count; `SpawnedProgram` gains `open_attempts: u32`; a new crate-visible open-only function for the fixture; updated doc comments (what the lock still covers and why, citing this task).
- `crates/server/src/pane.rs` - `PaneEvent::OpenRetried { pane, attempts }`; `start` sends it; `PaneCommand::Resume` gains `open_attempts: u32`; `worker_loop` sends it for a restart.
- `crates/server/Cargo.toml` - `pty-race-fixture` feature, `[[bin]] openpty-race-attempt`, `[[test]] openpty_race`.
- `crates/server/tests/fixtures/openpty_race_attempt.rs` (new).
- `crates/server/tests/openpty_race.rs` (new).
- `.github/workflows/ci.yml` (or a temporary workflow file, per the project's own convention) - temporary loop, removed before merge.
- No change to `crates/server/tests/real_pty.rs`'s existing `many_threads_opening_a_real_pty_at_once_never_fails` - it still proves the in-process half and stays exactly as TASK-1.76 left it; at most a doc-comment pointing at `openpty_race.rs` as where the cross-process half is proved.

## Order of work

1. `pty.rs`: `RealLauncher::new`/`Default`, `open_with_retry` and its three unit tests plus the reverted-loop control, the call site, `SpawnedProgram::open_attempts`, the error message. Run `just test` for `steg-server`.
2. `pane.rs`: `PaneEvent::OpenRetried`, the two send sites, `PaneCommand::Resume`'s new field.
3. `Cargo.toml` feature/bin/test declarations; `tests/fixtures/openpty_race_attempt.rs`; `tests/openpty_race.rs`, including the permanent `max_open_attempts = 1` control.
4. Record both the control and the fixed version via `asdlc record-check`, repeated, with the numbers above.
5. **Test the independence assumption, rather than trust it.** Using the same fixture (shipped `max_open_attempts = 5`), run a larger volume under the same 16-process density (e.g. 16 × 2,000 opens = 32,000, one dedicated run, not part of the routine repeats above) and tabulate, from the attempt counts `open_with_retry` already returns, how many opens needed exactly 1, 2, 3, 4, or 5+ attempts. Put the table in the task's **notes**, not the plan. If a second attempt fails noticeably more often than a first one (the correlated-burst case the independence assumption would miss), say so plainly and say whether 5 is still the right bound or whether a bare `std::thread::yield_now()` between attempts - never a `sleep`, which needs its own evidence this measurement doesn't provide - is better justified by what the table shows. If the table instead looks close to independent, say that too and leave the bound and the no-sleep retry as they are.
6. Add the temporary CI loop, gather its own numbers on the task, remove it in a follow-up commit before merge.
7. `just check`/`just test-all` clean at the final commit.

## Risks

- The statistical test's own false-failure budget (~1 in 305 million at the shipped 5-attempt default, worst measured rate, *if* attempts are independent) is a real, stated number, not zero, and the independence assumption behind it is optimistic by the engineer's own reasoning - "Order of work" step 5 measures this directly rather than leaving it as arithmetic; if the real rate is worse than idealised, that step says so and reassesses, before this is called proven.
- The seam (`open_with_retry`) is a plain generic function, not tied to `portable_pty`'s types, so layer (a)'s tests cost nothing and never touch a real pty - low risk.
- Retrying still happens entirely inside `OPENPTY_LOCK`, still no sleep anywhere in it - TASK-1.76's own reviewed invariant ("nothing under the lock blocks on another thread's progress") is unchanged in kind, only "one call" becomes "one call, retried a bounded number of times".
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
A throwaway probe from this task's investigation (a Python `multiprocessing`-based cross-process `openpty()` hammer, run outside the worktree, never committed) was killed abruptly (`pkill`) while it still held open real ptys and real child processes, rather than being stopped by ending its own loop, closing its descriptors, and waiting on its children. One child (`sh -c true`, pid 4188 on this machine, reparented to pid 1) was left in uninterruptible kernel sleep - `kill -9` cannot reach a process in that state; it was not reaped by this investigation and may still be present on this machine. Nothing in the worktree or git state is affected by this. For the build phase: never end a probe abruptly while it holds ptys - stop the loop, close every descriptor, wait on every child, the same discipline `ReapOnDrop` already gives the real tests (`tests/race.rs`).

Plan (4592947e385d) checked against the four acceptance criteria and the team's testing rules by @bruk.o, 2026-11-16: ok on the cause, the proof and the bound, with one departure the engineer must accept. Criterion 2 as written says the retry is on this one error and is not a blanket retry of any error. The plan retries any failure of the open step (the open alone, not the spawn), up to 5 attempts, counted and reported, because the library discards the structured error and the code was seen as both 6 and -6, so matching would rest on rendered text. The orchestrator asked for that change and recommends rewording criterion 2 to match; it was the orchestrator's own wording in the draft.

Check passed: `just check` exited 0 at 2026-11-16T12:52:38+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.112 on task-1.112-server-opening-a-pseudo-terminal-still at e0ecb19a24c2:

```text
ok      format check (0.3s)
ok      lint (50.5s)
ok      boundaries (0.2s)
ok      foundations (0.2s)
ok      foundations public surface (0.1s)
ok      design pages (0.1s)
ok      licences (0.8s)
skip    tests: this change touches no crate
ok      build (steg-sdk) (3.4s)
ok      test (steg-sdk) (2.0s)
ok      test (steg-sdk, local-socket) (2.2s)
ok      build (steg-kernel) (45.6s)
ok      test (steg-kernel) (1.5s)
ok      build (steg-server) (32.4s)
ok      test (steg-server) (14.1s)
ok      test (steg-server, race-fixture) (13.1s)
ok      build (steg) (2.0s)
ok      test (steg) (0.4s)
ok      test (steg, fixtures) (4.4s)
ok      build (steg-ui) (20.0s)
ok      test (steg-ui) (1.3s)
ok      build (steg-app) (1.7s)
ok      test (steg-app) (0.8s)
ok      tool tests (13.5s)
ok      architecture (1.3s)
ok      recipe names (0.2s)
ok      ui levels (0.2s)
ok      ui colours (0.1s)
```

Check FAILED: `cargo test -p steg-server --lib pty::tests` exited 101 at 2026-11-16T12:58:01+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.112 on task-1.112-server-opening-a-pseudo-terminal-still at e0ecb19a24c2 with uncommitted changes:

```text
   Compiling steg-server v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.112/crates/server)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.72s
     Running unittests src/lib.rs (target/debug/deps/steg_server-6d2f1d51fdd2e018)

running 3 tests
test pty::tests::succeeds_first_time_needs_no_retry ... ok
test pty::tests::fails_once_then_succeeds_counts_one_retry ... FAILED
test pty::tests::fails_every_time_surfaces_after_exactly_the_bound ... FAILED

failures:

---- pty::tests::fails_once_then_succeeds_counts_one_retry stdout ----

thread 'pty::tests::fails_once_then_succeeds_counts_one_retry' (28019556) panicked at crates/server/src/pty.rs:334:9:
assertion `left == right` failed
  left: Err(("transient", 1))
 right: Ok((42, 2))
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace

---- pty::tests::fails_every_time_surfaces_after_exactly_the_bound stdout ----

thread 'pty::tests::fails_every_time_surfaces_after_exactly_the_bound' (28019555) panicked at crates/server/src/pty.rs:345:9:
assertion `left == right` failed
  left: Err(("always fails", 1))
 right: Err(("always fails", 3))


failures:
    pty::tests::fails_every_time_surfaces_after_exactly_the_bound
    pty::tests::fails_once_then_succeeds_counts_one_retry

test result: FAILED. 1 passed; 2 failed; 0 ignored; 0 measured; 145 filtered out; finished in 0.00s

error: test failed, to rerun pass `-p steg-server --lib`
```

Check passed: `cargo test -p steg-server --lib pty::tests` exited 0 at 2026-11-16T12:58:13+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.112 on task-1.112-server-opening-a-pseudo-terminal-still at e0ecb19a24c2 with uncommitted changes:

```text
   Compiling steg-server v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.112/crates/server)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.64s
     Running unittests src/lib.rs (target/debug/deps/steg_server-6d2f1d51fdd2e018)

running 3 tests
test pty::tests::fails_once_then_succeeds_counts_one_retry ... ok
test pty::tests::succeeds_first_time_needs_no_retry ... ok
test pty::tests::fails_every_time_surfaces_after_exactly_the_bound ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 145 filtered out; finished in 0.00s
```

Check passed: `cargo test -p steg-server --features pty-race-fixture --test openpty_race cross_process_opens_at_the_shipped_default_never_fail` exited 0 at 2026-11-16T13:10:03+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.112 on task-1.112-server-opening-a-pseudo-terminal-still at e0ecb19a24c2 with uncommitted changes:

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.08s
     Running tests/openpty_race.rs (target/debug/deps/openpty_race-7feda493ce844436)

running 1 test
test cross_process_opens_at_the_shipped_default_never_fail ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 1 filtered out; finished in 7.08s
```

Check passed: `cargo test -p steg-server --features pty-race-fixture --test openpty_race cross_process_opens_with_no_retry_the_permanent_control_shows_at_least_one_failure` exited 0 at 2026-11-16T13:10:16+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.112 on task-1.112-server-opening-a-pseudo-terminal-still at e0ecb19a24c2 with uncommitted changes:

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.08s
     Running tests/openpty_race.rs (target/debug/deps/openpty_race-7feda493ce844436)

running 1 test
test cross_process_opens_with_no_retry_the_permanent_control_shows_at_least_one_failure ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 1 filtered out; finished in 6.60s
```

Check FAILED: `cargo test -p steg-server --test runtime a_panes_pty_that_cannot_be_opened` exited 101 at 2026-11-16T13:12:45+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.112 on task-1.112-server-opening-a-pseudo-terminal-still at e0ecb19a24c2 with uncommitted changes:

```text
   Compiling steg-server v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.112/crates/server)
warning: function `rollback_mint` is never used
   --> crates/server/src/runtime.rs:805:4
    |
805 | fn rollback_mint(tree: &mut Tree, method: &str, new_pane: PaneId) {
    |    ^^^^^^^^^^^^^
    |
    = note: `#[warn(dead_code)]` (part of `#[warn(unused)]`) on by default

warning: `steg-server` (lib) generated 1 warning
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.69s
     Running tests/runtime.rs (target/debug/deps/runtime-636749399b762beb)

running 1 test
test a_panes_pty_that_cannot_be_opened_after_every_attempt_fails_cleanly_and_leaves_nothing_behind ... FAILED

failures:

---- a_panes_pty_that_cannot_be_opened_after_every_attempt_fails_cleanly_and_leaves_nothing_behind stdout ----

thread 'a_panes_pty_that_cannot_be_opened_after_every_attempt_fails_cleanly_and_leaves_nothing_behind' (28305068) panicked at crates/server/tests/runtime.rs:311:5:
assertion `left == right` failed: nothing should be left behind when the open step itself exhausts every attempt
  left: (1, 1)
 right: (0, 0)
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    a_panes_pty_that_cannot_be_opened_after_every_attempt_fails_cleanly_and_leaves_nothing_behind

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 29 filtered out; finished in 0.00s

error: test failed, to rerun pass `-p steg-server --test runtime`
```

Check passed: `cargo test -p steg-server --test runtime a_panes_pty_that_cannot_be_opened` exited 0 at 2026-11-16T13:12:57+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.112 on task-1.112-server-opening-a-pseudo-terminal-still at e0ecb19a24c2 with uncommitted changes:

```text
   Compiling steg-server v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.112/crates/server)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.72s
     Running tests/runtime.rs (target/debug/deps/runtime-636749399b762beb)

running 1 test
test a_panes_pty_that_cannot_be_opened_after_every_attempt_fails_cleanly_and_leaves_nothing_behind ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 29 filtered out; finished in 0.00s
```

Check FAILED: `just check` exited 101 at 2026-11-16T13:16:18+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.112 on task-1.112-server-opening-a-pseudo-terminal-still at e0ecb19a24c2 with uncommitted changes, last 40 of 80 lines:

```text
running 13 tests
test an_oversized_declared_length_gets_a_malformed_hello_refusal_promptly ... ok
test a_control_frame_arriving_first_gets_a_malformed_hello_refusal ... ok
test a_connection_is_reported_connected_before_it_sends_its_hello ... ok
test undecodable_bytes_as_the_first_frame_get_a_malformed_hello_refusal ... ok
test a_hello_missing_the_protocol_field_gets_a_malformed_hello_refusal ... ok
test a_version_mismatch_names_both_versions_over_a_real_connection ... ok
test a_hello_with_the_wrong_magic_gets_a_malformed_hello_refusal ... ok
test a_connection_that_never_sends_a_hello_is_closed_after_the_timeout ... ok
test a_client_that_sends_half_a_frame_and_then_stalls_is_closed_at_the_timeout_with_no_reply ... ok
test a_hello_sent_one_byte_at_a_time_completing_inside_the_deadline_is_accepted ... ok
test a_completed_handshake_is_not_closed_for_sitting_idle_past_the_timeout ... ok
test a_hello_completing_well_after_the_deadline_is_closed_with_no_reply ... ok
test a_client_trickling_one_byte_at_a_time_is_still_closed_at_the_deadline ... ok

test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.77s

     Running tests/openpty_race.rs (target/debug/deps/openpty_race-7feda493ce844436)

running 2 tests
test cross_process_opens_at_the_shipped_default_never_fail ... ok
test cross_process_opens_with_no_retry_the_permanent_control_shows_at_least_one_failure ... FAILED

failures:

---- cross_process_opens_with_no_retry_the_permanent_control_shows_at_least_one_failure stdout ----

thread 'cross_process_opens_with_no_retry_the_permanent_control_shows_at_least_one_failure' (28421900) panicked at crates/server/tests/openpty_race.rs:191:5:
expected at least one failure across 3200 real cross-process opens with retry forced off (max_open_attempts = 1) - zero means the race was not reproduced on this run; see the task's own notes before weakening or deleting this test
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    cross_process_opens_with_no_retry_the_permanent_control_shows_at_least_one_failure

test result: FAILED. 1 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 13.83s

error: test failed, to rerun pass `-p steg-server --test openpty_race`
full output: .quiet/test-steg-server-pty-race-fixture.log
error: recipe `check-packages` failed with exit code 101
```

Check passed: `cargo test -p steg-server --features pty-race-fixture --test openpty_race cross_process_opens_at_the_shipped_default_never_fail` exited 0 at 2026-11-16T13:45:33+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.112 on task-1.112-server-opening-a-pseudo-terminal-still at e0ecb19a24c2 with uncommitted changes:

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.09s
     Running tests/openpty_race.rs (target/debug/deps/openpty_race-7feda493ce844436)

running 1 test
test cross_process_opens_at_the_shipped_default_never_fail ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 1 filtered out; finished in 6.75s
```

Check passed: `just test-openpty-race-control` exited 0 at 2026-11-16T13:47:30+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.112 on task-1.112-server-opening-a-pseudo-terminal-still at e0ecb19a24c2 with uncommitted changes:

```text
ok      openpty race control (110.2s)
```
<!-- SECTION:NOTES:END -->

## Comments

<!-- COMMENTS:BEGIN -->
author: @bruk
created: 2026-11-16T12:47:00Z

---

Plan approved by @bruk (4592947e385d, size-2): its worker may build it.

---

created: 2026-11-16T13:10:00Z

---

Finding before finishing (pausing per the build instructions - the permanent control not firing on a runner): at the plan's own stated density (16 processes x 200 opens = 3,200 total, through the real subprocess fixture, not the bare-libc probe used while planning), the permanent negative control (max_open_attempts=1) does not reliably show a failure even on this dev machine. Local sample, 20 independent runs of that one test: 17/20 showed at least one failure (passed), 3/20 showed zero (failed) - about a 15% miss rate, far above the <1-in-10,000 the plan's arithmetic assumed. The shipped/fixed test (5 attempts) stayed clean across a separate 15/15 runs - no false failures there.

Likely cause: the plan's density numbers came from a bare ctypes/libc probe (tight loop, no subprocess exec, workers forked from an already-warm parent). This fixture instead execs a fresh Rust binary per worker; process startup/scheduling jitter plausibly spreads out the true instant of overlap, achieving a lower real contention density than the bare-libc measurement predicted (back-solving the observed ~15% miss rate across 3,200 opens suggests something nearer 0.05% per open here, versus the ~0.3-0.4% measured with the bare-libc probe).

Raising density (tried 32 processes x 500 opens = 16,000 as a diagnostic, not committed) ran into the test's own 20s per-result timeout instead of resolving it cleanly - needs either a longer bound or a different way to drive more opens per wall-clock second, not just more processes.

I have not changed CONCURRENT_PROCESSES/OPENS_PER_PROCESS from the plan's stated 16x200 in the committed code. Both tests pass right now (recorded via asdlc record-check), but the permanent control's own reliability is lower than planned. Options I see, for you to pick: (a) accept this miss rate and treat an occasional control failure as "re-run," the same as other cross-process tests this repo already tolerates under load; (b) raise the bound/opens-per-process and the timeout together; (c) a tighter release mechanism than the current stdin-byte handshake, if that's really what's diluting the overlap - not verified which of these it actually is. Holding the temporary CI loop and the independence-assumption measurement (order of work, step 5) until I hear back, since both are sized against the same density assumption.

---

created: 2026-11-16T13:38:00Z

---

Built the sequential design (long-lived warm workers, ready/release handshake, stop-at-first-failure, bounded by a total open count not wall clock) exactly as directed. New finding, before I pick a bound: the achieved rate is far less stable than a single number can capture on this machine.

Numbers, all taken within the last 30 minutes, same machine, same code:
- 9 runs of the new sequential control (16 workers, cap 1,250 each = 20,000 total): 5 reproduced (first failure at opens-until-first-failure of 1, 1, 1, 461, 946 on the worker that saw it - often near-instant), 4 reached the full 20,000 with zero.
- Direct measurement bypassing the test's early-stop (16 workers x 3,000 opens each = 48,000, run twice back to back): 20 failures (0.042%), then 11 failures (0.023%) - roughly 2x apart, minutes apart, same code, same machine.
- The original bare-libc probe (no Rust, no portable-pty, the one that first established the mechanism), re-run right now at the same density (16 x 200 x 3 trials): 3, 2, and 0 failures - a clean miss in one of three trials even there.
- A run at a 5x larger bound (6,250 opens/worker, 100,000 total) did not complete in over two minutes and I stopped it rather than let it run unbounded while I didn't yet know if that was genuine progress or something degrading under this machine's own load.

Reading this together: the rate looks bursty, not a stable per-open constant - several of the fast reproductions (opens-until-first-failure = 1) suggest something *else* on this shared machine was also hammering ptys at that exact moment, and the self-contained rate from this fixture's own 16 workers alone, with nothing else helping, may be considerably lower and is what the zero-failure runs and the newly re-measured ~0.02-0.05% (not the original ~0.3-0.4% from early in this investigation) are showing. I don't yet have a bound I'm confident in without more time on this, and a naive 5x-larger bound just cost over two minutes without finishing - possibly fine on a quieter machine, possibly a sign the approach needs a different shape (e.g. sized and tuned against CI's own numbers rather than this heavily-shared dev machine, since CI's contention profile is presumably more stable run to run).

Not weakening the control to force a pass - holding here instead, same as before. The code for the sequential design (fixture + test) is in the worktree, not committed; CONCURRENT_WORKERS/TOTAL_OPEN_BOUND are still placeholders (16/20,000) pending direction.

---
<!-- COMMENTS:END -->
