---
id: TASK-1.112
title: >-
  Server: opening a pseudo-terminal still fails now and then under concurrent
  opens
status: In Progress
assignee:
  - '@bruk.w9'
created_date: '2026-11-16T04:53:00Z'
updated_date: '2026-11-16T12:06:00Z'
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

Three things need your call before this is built, all flagged inline below too:

1. **The retry bound.** I recommend 1 retry (2 attempts total), no sleep. Every recovery in ~68,000 synthetic cross-process opens needed exactly 1 retry; 0 were still failing after 1 even with up to 3 allowed. A 2nd retry (3 attempts) costs nothing measurable and is a free margin if you'd rather have it - your call, stated with the same "no sleep" reasoning either way.
2. **How the retry recognises "this one error".** `portable-pty` throws away the structured `io::Error` before it reaches us (`unix.rs:46`, read directly: `bail!("failed to openpty: {:?}", io::Error::last_os_error())` - a formatted string inside an `anyhow::Error`, not something `.downcast_ref::<io::Error>()` can reach). So the predicate has to match the error's own *rendered text* for a `code` of magnitude 6 together with `kind: Uncategorized` - fragile against a `portable-pty`/OS wording change, but its failure mode is safe (stops retrying, reverts to today's behaviour, never retries the wrong thing). The bigger fix - vendoring a patched `portable-pty` that preserves the real `io::Error`, the same way `third_party/ghostty-vt-patched` is already vendored (decision-3) - is sized out of this task; flag it as a follow-up if you want it done now instead.
3. **The new cross-process proof test's shape.** I'm proposing a `pty-race-fixture` Cargo feature plus a `[[bin]]` fixture and a `[[test]]`, mirroring the already-reviewed `race-fixture`/`race-attempt` pattern in this same crate (`Cargo.toml`, `tests/fixtures/race_attempt.rs`, `tests/race.rs`) exactly, because a cross-process race cannot be proved any other way (the same reasoning `race.rs`'s own module comment already gives). Confirm you're fine with a second instance of that pattern rather than something lighter.

Also: this task's own temporary CI loop (AC #4) needs a real pull request number to attach to and gets removed before merge - I've written where it goes below, but the exact removal commit happens once the numbers are in, which is this task's own last step, not something to pre-judge here.

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

The identical failure, through steg's own real dependency (not a bare libc stand-in): calling `portable_pty::native_pty_system().openpty()` directly, from 32 threads of **one process**, with `OPENPTY_LOCK` bypassed (not production code - the point was to capture the exact rendered text this dependency produces on this exact race, on this machine), reproduced the error on 7 of 8 tries, every time rendering as:

```
failed to openpty: Os { code: -6, kind: Uncategorized, message: "Unknown error: -6" }
```

This is the literal text, from this crate's own dependency, not a guess. The task's own description quotes the original CI incident as `Os { code: 6, Device not configured }` - a different sign, and missing the `kind`/`message` fields my own reproduction always shows. I could not resolve that discrepancy (the raw CI output is not preserved, correctly, per policy) - treat both signs as the same signature until proven otherwise; the predicate below checks the magnitude, not the sign.

**What's ruled out, not assumed:**

- **In-process contention under the current lock, even sustained, even doing the real work (not bare opens).** 32 threads in one process, for 4 continuous seconds (not one synchronised round), each doing a full `RealLauncher::spawn` - lock held only for the `openpty()` call exactly as production does, real `spawn_command`, real child, real `wait()` all outside the lock: 2,405 real spawns, 0 failures. A matching sanity check with no lock at all reproduced a failure in the same harness (1 in 1,277), confirming the harness itself can catch the race when it's actually exposed - the locked run's zero is not "the harness can't see it".
- `real_pty.rs` run alone, repeatedly: 8/8 clean (matches TASK-1.76's own review finding).
- The full `real_pty.rs` binary run as several genuinely separate OS processes at once (8, then 16 concurrent copies, 3 rounds each): 0 failures across 64 runs (~20,000+ real opens through the actual `RealLauncher` path). This is an honest negative result, not a disproof - production's per-op cost (a real `fork`+`exec` of a shell) is far higher than a bare syscall, so the same narrow race window is sampled far less often per second of wall clock than the bare-libc probe above, which is consistent with how rare the one recorded CI occurrence was.

**The real mechanism, confirmed in this repository, not hypothesised:** `cargo test -p steg-server` - the exact command `just test`/`just check`'s "test" step and CI both run - launches more than one of this crate's test binaries **concurrently, as separate OS processes**. Sampled directly (`ps` polled every 30ms through a real run): `real_pty`'s own binary and `runtime.rs`'s binary (TASK-1.76's own "18 tests, most real-pty") were both alive at once in several samples; `routing` and `handshake` each overlapped with `runtime` too. Each test binary is its own process, so each has its **own** `OPENPTY_LOCK` - `real_pty`'s lock cannot serialise against an open happening in `runtime`'s process, or vice versa. This is "CI runs test binaries in parallel" as a measured fact about this exact crate's own test suite, not a guess about a shared machine.

**Conclusion for AC #1:** the residual race is across OS process boundaries - demonstrated mechanically (the bare-libc, zero-steg-code reproduction) and demonstrated as reachable in exactly this project's own test run (sibling test binaries of this same crate, concurrent, each with an independent lock). It is not reproduced at will through the slower, real `RealLauncher` path in the volume I could generate here, which is a rate-of-sampling gap, not a reason to doubt the mechanism. AC #2's own wording - "a bounded retry of the open on this one error if the cause is outside the process and cannot be excluded" - is exactly this case: a real server is one process (`crates/server/CLAUDE.md`), so it cannot coordinate with whatever else, if anything, opens a pty on the same machine at the same instant, and the evidence says that collision is rare but real.

## The fix

Keep `OPENPTY_LOCK` exactly as `pty.rs` has it today - it still removes this process's own contribution, and TASK-1.76's reviewed invariant (nothing under it waits on anything) does not change. Add a bounded retry **around the one call to `pty_system.openpty(...)` only**, still inside the lock, matching a specific rendered-error signature:

```rust
/// Recognises the one platform race this retries (TASK-1.112): portable_pty's
/// own openpty() wraps a failed libc::openpty() as a formatted string inside
/// an anyhow::Error (unix.rs:46 - `bail!("failed to openpty: {:?}",
/// io::Error::last_os_error())`), discarding the structured io::Error before
/// it reaches us, so this is the only way left to recognise *this* failure
/// rather than any other reason the open could fail. Matches a `code` of
/// magnitude 6 (ENXIO; both `6` and `-6` have been seen for the identical
/// race on this platform - see the task's own notes) together with
/// `kind: Uncategorized`. Anything else is a real, non-transient failure and
/// is not retried.
fn is_the_cross_process_openpty_race(cause: &(dyn std::error::Error + Send + Sync)) -> bool {
    let text = format!("{cause:?}");
    text.contains("kind: Uncategorized")
        && extract_os_code(&text).is_some_and(|code| code.abs() == 6)
}
```

(`extract_os_code` is a small, private parse of `Os { code: <n>,` - not a crate, this is one field of one fixed format.)

The call site becomes a bounded loop, still entirely inside the existing `OPENPTY_LOCK` guard, replacing the single `pty_system.openpty(...).map_err(...)?`:

```rust
let _guard = OPENPTY_LOCK.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
let mut attempt = 0;
loop {
    attempt += 1;
    match pty_system.openpty(portable_pty::PtySize { rows, cols, pixel_width: 0, pixel_height: 0 }) {
        Ok(pair) => break pair,
        Err(cause) if attempt <= OPENPTY_MAX_RETRIES && is_the_cross_process_openpty_race(cause.as_ref()) => continue,
        Err(cause) => return Err(Error::Pty(cause.into())),
    }
}
```

No sleep between attempts - the evidence (every one of ~68,000 synthetic opens that failed once succeeded on the very next bare call, 0 needing a second retry even when up to 3 were allowed) says none is needed, and the lock's own stated cost ("held only across that one call") stays true in spirit - now "across that one call, retried a bounded number of times," still no indefinite wait, nothing that can block on another thread.

## What a real server needs, apart from what only the test's load needs

A real `steg-server` is **one process per person per machine** (`crates/server/CLAUDE.md`'s own words). In ordinary operation its only exposure to this race is against some *other*, unrelated program on the same machine also opening a pty at that exact instant (another terminal app, a second person's own `steg-server`, the OS's own activity) - never against another copy of itself, and nothing it could coordinate with via a lock file, since an unrelated program would never know to honour one. A cross-process lock file was considered and is not the fix for that reason: it adds real cost (contention between any of this machine's own `steg-server` processes, which do not otherwise need to know about each other) for a case it cannot even cover (a program that isn't steg). The bounded retry is the only candidate that helps both: a real server's rare, genuine collision with something else, and this task's own test, which manufactures the same collision far more densely, on purpose, to prove it.

## Files that change

- `crates/server/src/pty.rs` - the retry loop, the predicate, updated doc comments citing this investigation (replacing/extending the existing `OPENPTY_LOCK` doc comment, not rewriting TASK-1.76's own reasoning).
- `crates/server/Cargo.toml` - new feature `pty-race-fixture` (off by default, mirroring `race-fixture` exactly), a new `[[bin]] openpty-race-attempt`, a new `[[test]] openpty_race`, both `required-features = ["pty-race-fixture"]`.
- `crates/server/tests/fixtures/openpty_race_attempt.rs` (new) - loops real `RealLauncher::spawn` calls some fixed number of times, reaping each child (`child.wait()`) before the next iteration, exits non-zero the first time an open fails (prints `fail:<message>` first) so the parent test can tell from the exit status alone which copies raced.
- `crates/server/tests/openpty_race.rs` (new) - spawns several real copies of the fixture at once (`ReapOnDrop`-style guard copied from `tests/race.rs`, so a panicking assertion still kills and waits every child), waits for all, asserts every exit status is success.
- `.github/workflows/ci.yml` - a temporary step (or temporary workflow file, whichever this project's own convention prefers - check `TASK-1.82`'s own housekeeping notes for precedent before choosing) that runs the new test some N times, deleted in a later commit of this same pull request once the CI-side numbers are recorded on the task.
- No change to `crates/server/tests/real_pty.rs`'s existing `many_threads_opening_a_real_pty_at_once_never_fails` - it still proves the in-process half of this and stays exactly as TASK-1.76 left it; at most a doc-comment note pointing at the new cross-process test as where the other half is proved, so a future reader does not wonder why this one test's own scope stops at one process.

## Order of work

1. Add the retry and predicate to `pty.rs`, with the doc comment. Run `just test` for `steg-server` to confirm nothing existing regresses.
2. Add the `Cargo.toml` feature/bin/test declarations, write the fixture binary and `openpty_race.rs`, following `race_attempt.rs`/`race.rs` line for line where the shape matches (bounded waits, `ReapOnDrop`, real exit-status checks).
3. **Before** the fix is applied to `pty.rs` (or with it reverted locally for this one step): record, via `asdlc record-check`, repeated runs of the new `openpty_race` test and/or a denser bare-syscall-shaped variant if the fixture-level test proves too rare to fail in reasonable time - whichever actually reproduces at a measurable rate becomes the number cited as the negative control. Keep only the one error line, never full output, per policy.
4. Apply the fix. Record the same test repeated (N chosen against the rate found in step 3) via `asdlc record-check`: 0 failures.
5. Add the temporary CI loop, gather its own numbers on the task, then remove it in a follow-up commit of the same pull request before merge - recording that removal commit's own `just check` as clean.
6. `just check` and `just test-all` recorded clean once more at the final commit.

## Risks

- The error-matching predicate is a string match against a third-party crate's own formatted text, not a structured error - it can silently stop matching if `portable-pty` or the OS wording changes. Its failure mode is safe (falls back to today's un-retried behaviour), not silent wrong behaviour, which is why this is an accepted risk rather than a blocker - flagged above for the engineer to confirm.
- The new cross-process test is inherently probabilistic: my own attempts to reproduce the failure through the real `RealLauncher` path, even at 16 concurrent processes across 64 runs, did not succeed in the time available. The proof in step 3 needs to find a shape and a density that actually reproduces locally before the negative control can be recorded honestly; if none is found within reasonable effort, that itself is recorded on the task (what was tried, what rate if any), not glossed over, and the fix still ships on the strength of the bare-libc mechanism evidence plus AC #2's own allowance for a cause that "cannot be excluded."
- Retrying still happens under `Runtime`'s single `state` lock (`runtime.rs`) - confirmed bounded and non-blocking (no sleep anywhere in the retry), so TASK-1.76's own reviewed invariant ("nothing under the lock blocks on another thread's progress") still holds.

## Proof

- **Negative control**: the test/shape found in step 3 above, run repeated times with the fix absent (or the retry bound set to 0), failing at the measured rate - cited as a count out of a count (e.g., "N failures in M attempted opens"), never as pasted test output.
- **With the fix**: the same test, same shape, run repeatedly via `asdlc record-check` (count chosen so the expected number of "chances" at the measured per-open rate is comfortably in the hundreds, e.g. if the rate is ~0.3% per open and one run drives a few thousand opens, 20-30 runs is already tens of thousands of chances) - 0 failures across all of them.
- The temporary CI loop's own numbers (AC #4's "and on CI"), recorded on the task before its removal.
- Every wait bounded: the new fixture reaps (`child.wait()`) every process it spawns before its own next iteration and before exit; the new test's guard kills and waits every copy it started on every path, including a panicking assertion (copied from `tests/race.rs`'s own `ReapOnDrop`) - no leaked process, thread, or socket on any path, matching AC #3's own requirement extended to this task's new test code, not only production code.
- `just check` and `just test-all` both recorded clean at the final commit, after the temporary CI loop is removed.
<!-- SECTION:PLAN:END -->
