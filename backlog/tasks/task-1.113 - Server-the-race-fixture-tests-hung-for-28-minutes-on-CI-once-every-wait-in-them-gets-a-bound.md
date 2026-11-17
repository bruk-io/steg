---
id: TASK-1.113
title: >-
  Server: the race-fixture tests hung for 28 minutes on CI once; every wait in
  them gets a bound
status: In Progress
assignee:
  - '@bruk.w9'
created_date: '2026-11-16T05:42:00Z'
updated_date: '2026-11-17T03:37:00Z'
labels:
  - size-2
milestone: m-3
dependencies: []
parent_task_id: TASK-1
project: bruk-io-steg
asdlc_accepted_from: DRAFT-2
asdlc_branch: task-1.113-server-the-race-fixture-tests-hung-for
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
On 2026-11-16 pull request 55's check run 43802834909 (attempt 1, commit 51c1659, a change to crates/ui and tools only) printed 'ok test (steg-server) (12.3s)' at 04:16:46 UTC and nothing more until the job's 30-minute limit cancelled it at 04:44:21. The next step is the steg-server tests with the race-fixture feature (crates/server/tests/race.rs: several real processes racing for the arbiter's socket). Attempt 2 on the same commit passed that step in 10.6s. No earlier occurrence in the last 100 runs. A test that can wait for ever breaks the team's rule that a wait is bounded, costs 30 CI minutes when it happens, and left a worker idle for 80 minutes behind it. It may share a cause with the flakes recorded on TASK-1.82 (the arbiter's stale-socket test; the sdk's local socket reporting the address in use). Rests on doc-8 (arbitration, R1 to R3) and the team's testing rules.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The step that hung is identified from the evidence, or the notes say plainly that it could not be reproduced and what was tried
- [ ] #2 A test proves a bound works: a fixture process made to hang fails its test within the bound and leaves no process behind, with a recorded negative control
- [ ] #3 just check cannot spend its whole CI time limit on one hung test: the suite fails within minutes when a test hangs
- [ ] #4 Every wait in the test that hung (outbound.rs), in race.rs and in openpty_race.rs has a bound, and a test that passes its bound fails with a message naming what it waited for and kills and reaps the processes it started; the remaining waits found by the sweep are TASK-1.123's
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
## Findings (settle AC #1 and the orchestrator's question #1 - revised after team-lead review)

Read `crates/server/src/outbound.rs:267-298` (`run_writer`'s wait loop) and `:253-256`
(`close`): every mutation of `Queues.closed`/the queues happens under the same `Mutex`
the waiter re-checks before calling `Condvar::wait`, and `wait` is only ever called
while already holding that lock - a `close()`/`enqueue_*` landing between the check and
the wait cannot be missed. Read `crates/server/src/connection.rs:274-322`
(`ConnectionCleanup::drop`): `outbound.close()` runs on every path out of
`run_post_handshake` - normal return, early return, or a panic unwind - because it is a
local's `Drop`. So no production path ever abandons a connection without calling
`close()`: there is no "writer thread that never ends" in the product.

My first pass at the test-side cause (a panicking assertion elsewhere in this file
leaking a background writer thread) was wrong, as team-lead's review caught: a parked
non-main thread does not keep the test binary alive. The harness collects each
dispatched test's own result on its own thread and the process exits once that
collection is done; a thread some other, already-failed test spawned and never joined is
abandoned at process exit, with no effect on the next test or the run's own completion.
The sampled stack was the hanging test's *own* thread, inside its own direct,
synchronous `run_writer` call - consistent with only
`a_broken_write_closes_the_queue_so_the_readers_next_enqueue_fails`, the one test in this
file that calls `run_writer` without a `thread::spawn` in between.

Confirmed instead, per team-lead's direction: on macOS, `UnixStream::pair()` cannot set
`CLOEXEC` atomically. Read the toolchain's own std source
(`sys/net/connection/socket/unix.rs:111-144`, `Socket::new_pair`): the atomic
`SOCK_CLOEXEC`-on-`socketpair()` path is gated to Linux-like targets only
(`target_os = "linux"/"freebsd"/"netbsd"/...`, not `apple`); on macOS it calls
`socketpair()` plain, then sets `CLOEXEC` on each fd with two separate, later `fcntl`
calls (`:134-140`). A `fork`+`exec` landing in that gap inherits both fds regardless of
what `CLOEXEC` is set to a moment later in the parent - inheritance happens at `fork`,
`CLOEXEC` only matters at `exec`. `UnixListener::accept()` has the identical shape
(`:243-273`): atomic `accept4(..., SOCK_CLOEXEC)` on Linux-like targets, plain `accept()`
plus a later `fcntl` on macOS.

`outbound.rs`'s own unit tests run in the same compiled test binary, and by default the
same process, as every other `#[cfg(test)]` module in this crate - several of which fork
real processes from their own test threads, concurrently, under libtest's default
parallelism: `signal.rs:111-144` spawns `sh` children directly, `login_env.rs:209-228`
and `:632-667` spawn a real shell and a background `sleep ... &`, `pty.rs`'s
`RealLauncher` opens a pty and spawns a program through `portable_pty`, `pane.rs:952`
spawns `kill`. Any of these landing in the `socketpair()`-to-`set_cloexec()` gap of
`a_broken_write_closes_the_queue_...`'s own `UnixStream::pair()` call would have the
forked child inherit the `client` end; `drop(client)` in the test then closes only the
parent's own reference, the kernel still sees an open reference held by the forked
child, and the next `write_all` on `server` succeeds instead of failing.

Reproduced directly (scratch probe,
`/private/tmp/.../scratchpad/task-1.113-sockprobe/probe3.rs`, not committed, run in the
foreground, every spawned child explicitly `wait()`ed): with no concurrent forking, 0
successes in 800,000+ trials (serial and 16-way concurrent, both already run before
team-lead's review and recorded in the earlier plan revision). With 8 threads
continuously forking+exec'ing short-lived `sh -c true` children (the same shape
`signal.rs`'s own tests use) while 8 other threads repeated
`pair(); drop(client); write_all(server)`: **406,116 of 1,600,000 writes succeeded**
(roughly 1 in 4, once real concurrent forking is happening) - confirming the mechanism
directly, not just by reading the std source. `run_writer` then loops back to an empty,
unclosed queue and calls `Condvar::wait`, which nothing in that test will ever notify
again. This is the real cause: a test-side assumption (dropping one half of a freshly
made pair makes the other half's very next write fail at once) that macOS's own
non-atomic `CLOEXEC` makes unreliable whenever another thread in the same process forks
at the wrong moment - which this crate's own test suite does, routinely, by design.

Checked the rest of `outbound.rs`'s own tests for the same assumption: none of the
others depend on it - `closing_drains_what_is_already_queued_then_the_writer_returns`
calls `outbound.close()` explicitly before running the writer, never drops a peer to end
it, and the four spawn-then-join tests all end their writer via `outbound.close()` too,
not a dropped socket. Checked the other `drop(client)`/`drop(stream)` sites across
`crates/server/tests/{shutdown,idle_stop,routing}.rs`: all of them wait on an event with
seconds of slack (`recv_timeout(Duration::from_secs(5))` or similar) for the *server's*
own read loop to see the disconnect, not a bare, zero-slack "the very next local call
must fail" - a stray inherited fd from this same race would not survive seconds, so
these are not at the same risk and need no change.

**Product-adjacent finding, named but not fixed here** (team-lead's point 4): the same
non-atomic `accept()` applies to every connection `listen.rs` accepts on macOS. Grepped
`crates/server/src/*.rs` for `close_fds`/`CLOEXEC`/`set_cloexec`/`FD_CLOEXEC`: none of
this crate's own code does anything to scrub inherited descriptors before a pane's
program execs. `pty.rs`'s `RealLauncher` hands the command to `portable_pty`
(`CommandBuilder`/`PtyPair::slave.spawn_command`, not `std::process::Command` directly
for real panes), so whatever protection exists against a freshly accepted connection fd
being inherited by a concurrently-spawned pane's program lives entirely inside
`portable_pty`'s own spawn path, which I have not read. Whether that crate's spawn
already closes everything but its own wired-up pty fds, or shares the same gap, is
unverified - naming it for a follow-up task to check, not building anything for it here.

AC #2's literal scope is still larger than size-2 for the reasons already found (the
sweep across `tests/*.rs` and `#[cfg(test)]` `src/*.rs` blocks, ~60 sites) - unchanged
by this revision, see the "Files that change"/split note below.

## Files that change

- `crates/server/src/outbound.rs`: `a_broken_write_closes_the_queue_so_the_readers_next_enqueue_fails`
  no longer uses a real `UnixStream` at all. A tiny test-only `Write` type
  (`AlwaysFailsToWrite` or similar), whose `write`/`write_all` unconditionally returns
  `Err(io::Error::new(ErrorKind::BrokenPipe, ...))`, replaces the dropped-peer
  `UnixStream::pair()` - `run_writer` is already generic over `W: Write`, so this needs
  no change to the production code. This removes the dependence on `CLOEXEC`/fork timing
  entirely, rather than merely bounding around it; a bound alone would only have turned
  the hang into an intermittent ~1-in-4-when-contended failure, per the 406,116/1,600,000
  reproduction above. The call is still wrapped in `run_bounded` (below) as a second,
  independent guard for AC #2's literal "every wait has a bound", not as the fix for this
  test's own flakiness. `enqueue_data`/`enqueue_control` cap tests and the four
  spawn-then-assert-then-`close()` tests are unaffected (they don't depend on a dropped
  peer); those four gain a `ClosingWriterGuard` (below) purely as cleanup hygiene - stated
  as hygiene, not as a fix for any hang, per team-lead's review.
- `crates/server/src/lib.rs`: add `#[cfg(test)] mod test_support;`.
- `crates/server/src/test_support.rs` (new): `run_bounded(what, bound, body)` - runs
  `body` on a thread, `mpsc::Receiver::recv_timeout(bound)` on the result, panics naming
  `what` and `bound` on timeout; `ClosingWriterGuard` - owns the `Arc<Outbound>` and the
  writer's `JoinHandle`, and on `Drop` calls `outbound.close()` then joins with a bound,
  logging (never panicking - this can run mid-unwind) if the join itself ever times out.
  Mirrors `connection.rs:274-322`'s own "a local's `Drop` cleans up every exit path"
  idiom, moved into test code as the same hygiene, for the same reason - not presented as
  explaining the sighted hang.
- `crates/server/tests/support/mod.rs` (new): the same `run_bounded` shape (duplicated,
  not shared from `src/` - integration tests are separate compilation units and cannot
  import a crate-private module; stated plainly rather than hidden), plus
  `wait_child_bounded(label, child, bound) -> ExitStatus`: `child.wait()` inside
  `run_bounded`; on timeout, SIGKILLs the child and does one more short bounded wait
  before giving up with a message naming the child and the bound.
- `crates/server/tests/race.rs`: `mod support;`. `ReapOnDrop::drop` (:54-59) and the
  explicit `kill()`/`wait()` pairs (:135-146, 176-180) use `wait_child_bounded` instead
  of a bare `child.wait()`. `wait_until_accepting`'s poll (:75-86) is left as is, with a
  comment: it retries an external, cross-process readiness condition with no in-process
  event to subscribe to, and already has an explicit deadline.
- `crates/server/tests/openpty_race.rs`: `mod support;`. `wait_timeout_or_kill` (:367+)
  is replaced by a call to the shared `wait_child_bounded`, removing its 5ms sleep-poll
  (`.claude/rules/testing-rust.md`: "no test sleeps to wait for something; it waits on
  the event") - the exact replacement TASK-1.82's notes already named this task for.
- `tools/src/steg_tools/quiet.py`: bounds the whole step, independent of Rust-level
  coverage. `main` switches from `subprocess.run` to `Popen(..., start_new_session=True)`
  plus `communicate(timeout=bound)`; on `TimeoutExpired`,
  `os.killpg(os.getpgid(proc.pid), signal.SIGKILL)` (the direct child alone would leave a
  hung test binary's own children behind), drains the already-captured output, and
  reports through a new `timed_out_report(label, bound, seconds, output, log)` (same
  one-line-plus-tail shape as `failed_report`, "TIMED OUT" in place of "FAILED", naming
  the bound). The bound is `STEG_QUIET_TIMEOUT_S` (env, default 600 - checked against
  `.quiet/*.log` timings from a recent `just check` run so the default clears every
  legitimate step, including the Zig/ghostty-vt build, with real headroom, far under
  CI's 30-minute job limit).
- `tools/tests/unit/test_quiet.py`: a unit test for `timed_out_report`'s formatting,
  beside the existing `failed_report` test.
- `tools/tests/functional/test_quiet_timeout.py` (new): runs real `steg-quiet` as a
  subprocess against a command that outlives a very short `STEG_QUIET_TIMEOUT_S` (a
  Python one-liner that sleeps and then writes a sentinel file), asserts `steg-quiet`
  itself returns within a small bound, reports "TIMED OUT", and that the sentinel file is
  never written - proving the process group was actually killed, not just the immediate
  child abandoned.

## Order of work

1. `tools/src/steg_tools/quiet.py` + its two test files (the AC #4 backstop) - smallest,
   most independent piece, and the one that makes every later step safe to run even if
   something in it still hangs.
2. `crates/server/src/test_support.rs` + `lib.rs`'s one-line addition.
3. `crates/server/src/outbound.rs`: the `AlwaysFailsToWrite` sink and the rewritten
   `a_broken_write_closes_the_queue_...` test (deterministic fix), `run_bounded` wrapped
   around its call, `ClosingWriterGuard` wired into the four spawn-then-assert tests as
   hygiene, then `run_bounded_fails_fast_on_a_body_that_never_returns` as the generic
   negative control for the bound mechanism itself.
4. `crates/server/tests/support/mod.rs`, then `race.rs` and `openpty_race.rs` wired to
   it.
5. Record checks (Proof, below); write the findings above into this task's notes
   (including the product-adjacent `accept()`/`portable_pty` question, named for a
   follow-up); draft the follow-up task for the rest of the sweep and link it.

## Risks

- `ClosingWriterGuard`'s `Drop` runs during an active panic if the guard itself is being
  unwound past; it must never panic itself (a second panic during unwind aborts the
  process) - its timeout path only logs.
- `os.killpg` needs the child's own process group to actually contain what hung
  (`start_new_session=True` at spawn, not added after) - get this wrong and the backstop
  silently keeps the old behaviour. Mitigation: the functional test's sentinel-file check
  proves the kill really reached the process, not just the immediate `cargo`.
- The chosen `STEG_QUIET_TIMEOUT_S` default could be too tight for a slow CI runner and
  turn a slow-but-fine step into a false failure. Mitigation: set it from this task's own
  recorded `just check`/`just test-all` timings with real headroom, not a guess, and say
  in the notes what the slowest observed step was.
- Converting `wait_timeout_or_kill` changes `openpty_race.rs`'s own already-passing
  control test's timing characteristics. Mitigation: re-run
  `test-openpty-race-control` after the change and record it.
- The same `CLOEXEC` race could, in principle, affect any other test that drops one half
  of a freshly made pair and expects the other half's very next call to fail with no
  slack - checked and found none in this crate today (see Findings), but a future test
  written the same way would reintroduce it; worth a line in `testing-rust.md` or the
  follow-up task, not built here.

## Proof

- `asdlc record-check -- cargo test -p steg-server --features race-fixture outbound::tests`
  (AC #1, #2 for the sighted file): the rewritten test passes deterministically (no
  socket, no fork timing involved); `run_bounded_fails_fast_on_a_body_that_never_returns`
  passes as the negative control for the bound mechanism (AC #3).
- `asdlc record-check -- cargo test -p steg-server --features race-fixture` and
  `--features pty-race-fixture` (AC #2 for `race.rs`/`openpty_race.rs`), plus
  `just test-openpty-race-control` recorded again.
- `asdlc record-check -- just test-tools` (the new `quiet.py` unit + functional tests,
  AC #4).
- `asdlc record-check -- just check` at the end, full run.
- Note on the task naming the follow-up task created for the rest of the sweep (AC #2's
  literal, whole-crate reading) and the unverified `portable_pty`/`accept()` question,
  asking the engineer to confirm the split rather than silently narrowing AC #2.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Sighting with a stack, 2026-11-16 by @bruk.o: on this machine, in TASK-1.121's worktree, 'just check' hung for 29 minutes in the step 'test (steg-server, race-fixture)'. A sample of the test binary showed one test thread still alive: outbound::tests::a_broken_write_closes_the_queue_so_the_readers_next_enqueue_fails. Its stack: the test calls outbound::run_writer on a UnixStream, which sits in Condvar::wait on the Queues lock (pthread_cond_wait), never woken. So the test calls run_writer on its own thread and relies on a write failing to end it; when the queue is empty at that moment the writer waits for work with nothing left to wake it. This is likely the same hang as the 28 minutes on CI, and it is in the unit tests built with the race-fixture feature, not in the race-fixture integration tests. Fix belongs here: the test must not be able to block forever (enqueue before running the writer, or run the writer on a thread joined with a bound), and check whether run_writer itself can miss a wake-up when the queue is closed while it waits.

Plan checked against the task's criteria and the testing rules by @bruk.o (plan 2eb24f415089): ok. The cause is confirmed, not guessed: on macOS a socket pair is marked close-on-exec after it is made, other unit tests in the same binary fork, and a child that inherits the dropped end keeps the peer open, so the write succeeds and run_writer waits forever (probe: about one write in four succeeded with concurrent forks, none without). The test gets a sink that always fails, so it no longer depends on a closed peer. Kept: a bound on every step in steg-quiet with a process-group kill, the helpers, race.rs and openpty_race.rs. Split, for the engineer to accept: the wider sweep and the product question of a pane's program inheriting a connection are drafted as separate tasks; criterion 2 here is to be narrowed to the files this task names once the engineer agrees.
<!-- SECTION:NOTES:END -->

## Comments

<!-- COMMENTS:BEGIN -->
created: 2026-11-16T13:14:00Z

---

2026-11-16, @bruk.o: a second kind of hung test, for the same lesson: TASK-1.98's new accept-backoff tests joined a reader thread with no bound and hung for 30 minutes on CI (run 43866214322) and three times on the worker's machine. Fixed in that task; when this task bounds the race tests, consider a suite-level guard so that no single test binary can run past a few minutes in just check (the fourth criterion here).

---

created: 2026-11-17T01:27:00Z

---

2026-11-16, @bruk.o, plan check (fbc7088082a8): not yet for approval; the finding needs one more step. (1) The second mechanism cannot be the hang: a leaked parked thread does not keep a Rust test binary alive, the process exits when the harness returns; and the sampled stack was the test's own thread under the harness, inside the direct run_writer call. So the write to the stream whose peer was dropped did succeed. (2) A mechanism that fits and that the 800,000-trial probe could not show: UnixStream::pair on macOS makes the pair and sets close-on-exec afterwards, not atomically. Other tests in the same binary fork and exec programs (pty and pane tests). A fork between the pair being made and drop(client) leaves a child holding the client end, so the peer is not gone, the write succeeds, and run_writer waits forever on an empty queue. Test this: repeat the probe while other threads spawn short-lived processes. (3) If confirmed, bounding that test only turns a hang into an intermittent failure. The test should not depend on a closed peer at all: run_writer is generic over Write, so give it a sink that always fails. (4) If confirmed it is also a product question, to note as a follow-up and not fix here: sockets the server accepts are likewise marked close-on-exec after the fact on macOS, so a pane's program started in that window could inherit a client's connection. Keep the rest: the step bound in steg-quiet, the helper, race.rs and openpty_race.rs, and the split of the wider sweep.

---

created: 2026-11-17T03:37:00Z

---

2026-11-16, @bruk.o: criterion 2 narrowed to the files this task names, after the engineer accepted the split: the wider sweep is TASK-1.123, the question of a pane's program inheriting a connection is TASK-1.124.

---
<!-- COMMENTS:END -->
