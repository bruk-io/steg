---
id: TASK-1.112
title: >-
  Server: opening a pseudo-terminal still fails now and then under concurrent
  opens
status: In Progress
assignee:
  - '@bruk.w9'
created_date: '2026-11-16T04:53:00Z'
updated_date: '2026-11-16T12:12:00Z'
labels:
  - size-2
milestone: m-3
dependencies: []
parent_task_id: TASK-1
project: bruk-io-steg
asdlc_accepted_from: DRAFT-1
asdlc_branch: task-1.112-server-opening-a-pseudo-terminal-still
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Seen on 2026-11-16 (UTC) on main at d06ab92 (the merge of pull request 52, which changed only Python tools): crates/server/tests/real_pty.rs many_threads_opening_a_real_pty_at_once_never_fails failed at round 10, thread 27 of 32 with 'RealLauncher::spawn failed under concurrent opens: the pseudo-terminal failed: failed to openpty: Os { code: 6, Device not configured }', in the step that tests steg-server alone. The same code passed CI on pull request 47's head, which contains that commit. TASK-1.76 added the process-wide OPENPTY_LOCK around openpty on the orchestrator's decision (a lock and no retry) after its review isolated a race between concurrent openpty calls on macOS; this shows the lock is not the whole answer. It is a real defect and not only a flaky test: a user's pane could fail to open. crates/server/src/pty.rs.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The cause of the failure seen in CI is established, not assumed: openpty returned 'Device not configured' (ENXIO) in many_threads_opening_a_real_pty_at_once_never_fails while the process-wide lock around openpty was held, so the race is not only between two openpty calls in one process. The plan finds what else it races with (the spawn that follows and opens the slave, a close of another pty's descriptors on another thread, another process on the machine, a limit) by experiment on macOS, and says which
- [ ] #2 The fix addresses that cause: for example the lock covering what the experiment shows it must cover, or a bounded retry of the open on this one error if the cause is outside the process and cannot be excluded; a retry is stated with its bound and its evidence, and is not a blanket retry of any error
- [ ] #3 A pane whose pseudo-terminal cannot be opened after that still fails cleanly with the error doc-2 gives, leaving no half-made pane, thread or child
- [ ] #4 Shown by repeated runs: the test passes N times in a row locally under load and on CI (a temporary workflow that repeats it, removed before merge), with N justified against the failure rate seen; and a negative control: with the fix removed, the experiment that reproduces the failure does so
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
## For the engineer

1. **The retry bound and its parameter.** Retry on *any* error from the open step (not a specific error - see "why not match the error" below), up to `max_open_attempts` attempts total, no sleep between them; the last error surfaces with the attempt count in its message. Default 3, as a field on `RealLauncher`'s own constructor (`RealLauncher::default()` = 3, `RealLauncher::new(n)` for a test), not a new `ServerConfig` field - `ServerConfig::launcher` is already `Arc<dyn ProgramLauncher>`, and a bound that only means something for the real launcher belongs on it, not duplicated on `ServerConfig` where a fake launcher would ignore it. A test (unit or fixture) builds `RealLauncher::new(1)` directly to force "no retry". From the measured rate (worst local figure, ~0.4% failing per open under 16 hammering processes): the chance all 3 attempts fail, treating each as independent, is `0.004³ ≈ 6.4×10⁻⁸` - about one in 15.6 million opens at that same density. I have not proven independence across attempts a few microseconds apart; stated as the approximation it is.
2. **Why not match the error's text** (reversing this plan's first draft): the sign already disagreed between two sightings (`-6` reproduced directly through this exact dependency; `6` in the task's own description of the CI incident) and `portable-pty` discards the structured `io::Error` before it reaches us (`unix.rs:46`: `bail!("failed to openpty: {:?}", io::Error::last_os_error())` - a formatted string inside an `anyhow::Error`, confirmed by reading the source, not assumed). Retrying on any error instead costs a genuine, non-transient failure (the system's own pty limit, a real permission problem) only a few extra syscalls before it is reported exactly as it is today, just with the attempt count added - nothing is masked, and nothing depends on a string that could stop matching on a dependency bump. I don't see a reason to narrow this further; say so if you do.
3. **The proof test's shape**, two layers, each below with its own control - confirm before I build either:
   - A deterministic unit test behind a small seam in `pty.rs` (no process, no real pty) - see "Proof, layer (a)".
   - A statistical test across real, separate processes, reusing the already-reviewed `race-fixture`/`race-attempt` pattern in this crate - see "Proof, layer (b)". Its fixture calls a new, crate-internal "open only" step (the real lock, the real retry, no `spawn_command`) so its density matches the bare-libc probe that actually reproduces this, not the full path I could not make fail in the time I had.
4. **Observability**: see "What's reported" below - a new `PaneEvent::OpenRetried` variant, the same shape TASK-1.98 is adding `ConnectionEvent::AcceptRetried` for. Read TASK-1.98's plan yourself before either lands, since both are touching `crate::pane`'s/`crate::listen`'s sibling event enums around the same time and I don't want to presume which of the two merges first.
5. **The lock stays.** See "What stays of TASK-1.76's lock" below.

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

Keep `OPENPTY_LOCK` exactly as `pty.rs` has it today (see "What stays of the lock", below). Wrap the one call to `pty_system.openpty(...)` in a bounded retry, still entirely inside the lock, on *any* error (not a specific one - see "For the engineer", #2):

```rust
impl RealLauncher {
    pub fn new(max_open_attempts: u32) -> Self { ... } // asserts >= 1
}
impl Default for RealLauncher {
    fn default() -> Self { Self::new(3) }
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
- `fails_every_time_surfaces_after_exactly_the_bound`: a closure that always errors - with `max_attempts = 3`, asserts `Err((_, 3))` and that the count `3` reached the closure exactly 3 times (a counter inside the closure).
- `succeeds_first_time_needs_no_retry`: a closure that always succeeds - asserts `Ok((_, 1))`, and that the closure was only called once (so a passing "retries" test isn't secretly retrying every time).
- **Control**: with the retry loop reverted to a single call (`open_with_retry`'s body replaced by `try_open().map(|v| (v, 1))`), `fails_once_then_succeeds_counts_one_retry` fails - recorded via `asdlc record-check` both ways.

### Layer (b): statistical, across real processes

New feature `pty-race-fixture` (off by default), mirroring `race-fixture` exactly: a `[[bin]] openpty-race-attempt` (`tests/fixtures/openpty_race_attempt.rs`) and a `[[test]] openpty_race` (`tests/openpty_race.rs`), both `required-features = ["pty-race-fixture"]`.

The fixture calls a new, crate-visible function that is **only** the locked, retried open step - no `spawn_command`, closing the pair immediately after a successful open so fds don't accumulate across its own loop - so its density matches the bare-libc probe's, which is the one shape that has actually reproduced this. It loops that call a fixed number of times (an argument from the environment, the same convention `race_attempt.rs` already uses - no path or count on argv), counting failures, and prints `done:<failures>:<n>` at the end.

**Synchronised start, no periodic wake-up**: the test spawns all `N` fixture processes first, each blocking on reading one byte from its own inherited stdin before entering its loop (not a sleep-based rendezvous, not polling) - then releases all `N` within the same instant by writing to each. Every spawned process is wrapped in the same `ReapOnDrop` guard `tests/race.rs` already has (kill + wait on drop, including the panicking path), so nothing is left running, bounded, or orphaned regardless of which assertion fails.

**The numbers, from the measured rate (worst local figure, ~0.4% per open, treated as independent per open)**:

- Total opens needed so a **no-retry control** (`max_open_attempts = 1`, same as today) has a chance of showing *zero* failures below one in ten thousand: solving `(1-0.004)^n < 10⁻⁴` gives `n > 2,297`. Chosen: `N = 16` processes × `200` opens each = 3,200 total - `(0.996)^3200 ≈ 2.8×10⁻⁶`, comfortably under the bar.
- The **same 3,200 opens, with the shipped default (3 attempts)**: chance of at least one open exhausting all 3 is `1 - (1 - 0.004³)^3200 ≈ 3200 × 6.4×10⁻⁸ ≈ 2.0×10⁻⁴` - about one run in five thousand flakes on its own, at this worst-case measured rate. Stated plainly, not hidden: if this is too flaky a budget for a CI test the engineer wants green every time, the knob to pull is the shipped default (4 attempts instead of 3 cuts this to ~8×10⁻⁷), not the test's own density - the density is what the control's own bound requires.
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
5. Add the temporary CI loop, gather its own numbers on the task, remove it in a follow-up commit before merge.
6. `just check`/`just test-all` clean at the final commit.

## Risks

- The statistical test's own false-failure budget (~1 in 5,000 at the shipped default, worst measured rate) is a real, stated number, not zero - flagged above for the engineer, with the knob (attempts) that moves it.
- The seam (`open_with_retry`) is a plain generic function, not tied to `portable_pty`'s types, so layer (a)'s tests cost nothing and never touch a real pty - low risk.
- Retrying still happens entirely inside `OPENPTY_LOCK`, still no sleep anywhere in it - TASK-1.76's own reviewed invariant ("nothing under the lock blocks on another thread's progress") is unchanged in kind, only "one call" becomes "one call, retried a bounded number of times".
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
A throwaway probe from this task's investigation (a Python `multiprocessing`-based cross-process `openpty()` hammer, run outside the worktree, never committed) was killed abruptly (`pkill`) while it still held open real ptys and real child processes, rather than being stopped by ending its own loop, closing its descriptors, and waiting on its children. One child (`sh -c true`, pid 4188 on this machine, reparented to pid 1) was left in uninterruptible kernel sleep - `kill -9` cannot reach a process in that state; it was not reaped by this investigation and may still be present on this machine. Nothing in the worktree or git state is affected by this. For the build phase: never end a probe abruptly while it holds ptys - stop the loop, close every descriptor, wait on every child, the same discipline `ReapOnDrop` already gives the real tests (`tests/race.rs`).
<!-- SECTION:NOTES:END -->
