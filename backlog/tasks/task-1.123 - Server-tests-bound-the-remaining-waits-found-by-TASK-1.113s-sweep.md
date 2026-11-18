---
id: TASK-1.123
title: 'Server tests: bound the remaining waits found by TASK-1.113''s sweep'
status: In Progress
assignee:
  - '@bruk.w5'
created_date: '2026-11-17T01:33:00Z'
updated_date: '2026-11-18T14:09:00Z'
labels:
  - size-3
milestone: m-3
dependencies:
  - TASK-1.113
parent_task_id: TASK-1
project: bruk-io-steg
asdlc_accepted_from: DRAFT-1
asdlc_branch: task-1.123-server-tests-bound-the-remaining-waits
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Split from TASK-1.113 by its plan on 2026-11-16, under the engineer's standing decision to split by default. TASK-1.113 fixes the test that hung (outbound.rs), race.rs and openpty_race.rs, adds the shared helpers (run_bounded, wait_child_bounded) and bounds every step of just check in steg-quiet. Its sweep found about 60 more waits with no bound: join, recv, child wait and socket reads in crates/server/tests (shutdown, idle_stop, program, events, routing, handshake, runtime, real_pty), crates/steg/tests (idle_stop, server_start, login_environment) and the unit tests of a dozen server modules. None has been seen to hang. This task bounds them with TASK-1.113's helpers. Also from TASK-1.113's finding: on macOS a socket pair or an accepted socket is marked close-on-exec after it is made, so a test must not rely on dropping one end making the other end fail at once while other tests fork.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A line in the repository's Rust testing rule says a test must not rely on a dropped socket end failing its peer at once on macOS, with the reason
- [ ] #2 Each clash recorded in this task's notes on 2026-11-18 is fixed: the attach flood tests starving when two worktrees run that test binary at once; login_env's backgrounded-grandchild test under load; signal's process-group test; and temporary socket paths colliding across processes in crates/steg's tests. For each, the failure is reproduced first and recorded, and the same reproduction is recorded passing after the fix
- [ ] #3 The helper call_and_read_reply's read bound in crates/server/tests/attach.rs is a liveness bound in line with the rest of that file, and every per-test temporary path in crates/steg's tests comes from one shared helper that cannot collide across processes
- [ ] #4 The notes say what is not fixed here and where it went: the remaining unbounded waits, the tools check, the sdk fixture's sleep, and the terminal-scrolling sighting that could not be located
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
## Scope note

This plan is rewritten for the task's four current criteria (the clash classes,
`call_and_read_reply`'s bound plus the shared temp-path helper, the notes, and the
rule line). The earlier plan's sections 1, 3 and 4 (the wider inventory, the tools
check, and the sleep-in-a-loop sweep) now belong to the two draft tasks this task's
notes link; they are not repeated here. New reading since the first plan, named
because it changed the design below: `crates/server/src/bind.rs`'s own `temp_name`
and its `#[cfg(test)]` module's `unique_path` (both already state, from a real
failure hit while building this crate, that `sockaddr_un`'s 104-byte `sun_path`
budget leaves "too little room for a descriptive name" once `$TMPDIR`'s own length
and a temp-bind suffix are both spent against it); `crates/sdk/src/paths.rs::
socket_path`, whose own doc comment goes further - production socket paths are
"flat, no subdirectory" by deliberate policy, for this exact reason, with a worked
example; and a direct check on this machine, `$TMPDIR` is 49 bytes
(`/var/folders/rf/f34xmczn4659gm98hrsprydc0000gn/T/`), confirming the budget is real
here, not just in the comments. Also read every `crates/server/tests/*.rs` file for
its own `unique_socket_path` - the duplicated helper is not confined to
`crates/steg`.

## 1. The rule line (AC #1)

Unchanged from the first plan, verified against both files' own opening lines:
`.claude/rules/testing-rust.md` holds "the techniques for the team's testing policy";
`plugins/bruk-io-steg/rules/testing-rust.md` is "how to run them" (recipe-level). The
macOS dropped-socket fact is a technique, so it is a new bullet in `.claude/rules/
testing-rust.md`, beside "no test sleeps to wait for something; it waits on the
event." `.claude/rules/` sits in the repository's own root, not under `plugins/`, so
neither the plugin route nor team rules' pull-request-only route applies - edited
directly, in this task's own commit. Sentence: "A test must not rely on a dropped
socket end making its peer's very next read or write fail at once: on macOS,
`UnixStream::pair()`/`UnixListener::accept()` cannot set `CLOEXEC` atomically (the
std library's own `sys/net/connection/socket/unix.rs`), so a `fork` elsewhere in the
same test binary can inherit the dropped end before `CLOEXEC` lands, keeping the
peer open and the expected failure from happening (TASK-1.113)."

## 2. The four clashes (AC #2) - reproduction, fix, and the control each gets

**(a) attach.rs flood-test starvation across worktrees.**

*Fix*: three changes, each closing a different failure mode -
1. Raise `call_and_read_reply`'s flat 5s read bound to 15s, matching the margin
   `an_unresponsive_panes_own_commands_do_not_stall_the_connection_or_another_pane`'s
   own two "eventually drains" checks already use for the same reason (every
   fixture-setup call in this file goes through it, not only the flood proofs).
2. A machine-wide advisory lock, taken only by the five named flood tests, before
   `FLOOD_TEST_LOCK` (the existing in-process `Mutex`, left as is - it still covers
   two flood tests racing within one binary, which a cross-process lock does not).
   Lives beside it in `attach.rs` as `cross_process_flood_guard()`. The lock file's
   path mirrors `tools/src/steg_tools/quiet.py::lock_directory`'s own reasoning
   exactly (read directly): never `$TMPDIR`/`std::env::temp_dir()` (it is
   per-worktree-sandbox and the whole point is to serialise *across* worktrees), a
   fixed path outside every worktree -
   `$XDG_CACHE_HOME/steg/test-locks/attach-flood.lock`, else `$HOME/.cache/steg/
   test-locks/attach-flood.lock`. Taken with a real OS `flock` (`nix::fcntl::Flock`,
   already a dependency, used today by `arbiter.rs` for the same primitive), on a
   dedicated fd opened for the test's own duration. The acquisition itself is
   bounded: a watchdog thread does the real, possibly-blocking `flock`, the test
   thread waits on it through a `recv_timeout`-style channel (the same `run_bounded`
   shape `crates/server/tests/support/mod.rs` already has) with a generous bound
   (proposed 60s - five tests, each already bounded to tens of seconds at most, so
   even a full queue behind this one clears well inside a minute); on timeout, the
   test fails naming "waiting for the cross-process flood-test lock" and the bound,
   not a silent hang - this is itself one of this task's own "every wait has a
   message" waits, not exempted from its own rule. Released by the OS the instant
   this process's last reference to that fd closes, for any reason including
   `SIGKILL` from `wait_child_bounded`'s own watchdog or CI's own job timeout -
   `quiet.py::held_slot`'s own doc comment states the identical guarantee for
   exactly this reason ("the kernel drops the flock the instant this process's last
   reference to that fd closes... There is no lock, pid, or state file recording
   who holds a slot, so a killed run leaves nothing behind to clean up"). No stale-
   lock detection or cleanup code is written, because none is needed.
   **Cannot deadlock with TASK-1.132's two cargo slots**: deadlock needs a cycle -
   two holders each waiting on a resource the other holds. Here, a worktree that
   already holds one of the two cargo slots (TASK-1.132) never needs the *other*
   slot to finish its own run, and never releases its held slot back for someone
   else mid-run; the flood lock is the only resource a flood test ever waits on
   beyond what it already, irrevocably holds. So at most one process is ever
   waiting (on the lock), and the process holding the lock is never itself waiting
   on anything the waiter holds - ordinary serialisation through one extra
   resource, not a cycle. Stated, not just asserted: the two slots and the one lock
   form a strict hierarchy (slot acquired first, lock requested only after, lock
   released before the slot is), and a cycle needs two resources each process waits
   on in opposite order, which this ordering rules out by construction.
3. `call_and_read_reply`'s bound and the lock are independent and additive - the
   lock removes most of the contention the sightings describe; the wider bound
   covers whatever this machine's own baseline CPU contention still costs even with
   the lock held exclusively.

*Reproduction, forced, not waited for*: build once (`cargo test -p steg-server
--test attach --no-run`), locate the compiled binary under `target/debug/deps/
attach-*`, then invoke it directly twice at once, in a loop:
```
for i in $(seq 1 20); do
  "$BIN" an_unresponsive_panes_own_commands_do_not_stall_the_connection_or_another_pane --exact &
  "$BIN" an_unresponsive_panes_own_commands_do_not_stall_the_connection_or_another_pane --exact &
  wait
done
```
recording how many of the 40 invocations fail, before the fix (expect a visible
failure rate - the sighting's own description, "failed six times in a row... while
another worktree's attach test binary was running at 200 percent CPU") and after
(expect 0 of 40, or close to it, across several runs of the loop).

**(b) `login_env`'s backgrounded-grandchild test.**

*Fix*: widen `capture_login_environment_kills_a_backgrounded_grandchild_that_held_
the_pipe_open`'s outer wall-clock assertion from `Duration::from_secs(5)` to 15s,
matching (a)'s own number for the same reason (proving "returns at all" under this
machine's own contention, not timing precision) - the file's own 1s *functional*
timeout is unchanged, it already states it was deliberately widened once before for
the identical reason. The two inner polling loops (pid-file appears; process
actually dies) stay as cross-process-liveness polls with no in-process event, the
same exception class `race.rs::wait_until_accepting` already has - named, not
changed, in this task's own notes (AC #4).

*Reproduction*: run this one test, alone, in a tight loop (order of 50-100 runs)
while this machine is put under real, independent CPU contention - several
concurrent `cargo build`/`cargo test` invocations in other worktrees, or a plain
`yes > /dev/null &` per core for a crude but real floor - and record the failure
rate against the 5s bound; then the same loop, same contention, against the 15s
bound.

**(d) `signal::tests::send_reaches_the_whole_process_group_not_only_the_leader`.**

*Fix*: replace the single `assert!(!process_exists(background_pid))`, taken once
immediately after `wait_within` reaps the main `sh`, with a bounded *poll* (the
same cross-process-liveness exception as (b)'s own inner loops) - up to a few
seconds, asserting the backgrounded process is gone *by* a deadline rather than *at*
one snapshot. This is correct regardless of which of two candidate mechanisms
actually fires (both read directly from the code, neither assumed): (1) `kill -0`
reports success for a *zombie* too, and the backgrounded `sleep`, once orphaned by
`sh`'s own death, is reaped asynchronously by this machine's own init/subreaper, on
no schedule the test controls; (2) `send`'s own `killpg` could in principle race the
kernel's own attachment of the forked `sleep` to its process group, though this
would need to happen between the shell's own `echo $!` (which only runs once the
fork has completed) and the `killpg` call, a narrower window than (1).

*Reproduction - and what happens if it still does not reproduce*: first attempt,
forced rather than waited for: in a scratch, uncommitted copy, insert a short
explicit `std::thread::sleep` between `wait_within(child, ...)` returning and the
`process_exists(background_pid)` check, run the test in a loop, and see whether that
alone is enough to expose a window where `process_exists` is still seen true after
the artificial delay (this does not prove mechanism (1) or (2) - it only proves *a*
window of the right shape exists to assert on, which is what the fix's own poll
needs to tolerate). If this probe itself cannot be made to fail after a reasonable
number of attempts (order of 100+ loop iterations with an increasing artificial
delay), that is itself the answer, not a blocker: the notes will say plainly that no
failing-before-the-fix control was recorded for this one class, name the two
candidate mechanisms as read from the code rather than confirmed by reproduction,
and ship the bounded-poll fix anyway on the grounds that it removes a zero-slack
assumption the rest of this task's own classes establish as a real pattern on this
platform - stated as a weaker claim than the other three classes get, not
substituted with a control that does not exist.

**(e) Temporary socket paths colliding across processes (`crates/steg`, and
confirmed by this reading: `crates/server` too).**

*Found, beyond what the first plan reported*: every one of nine files under
`crates/server/tests/` (`attach.rs`, `accept_backoff.rs`, `events.rs`, `handshake.rs`,
`idle_stop.rs`, `race.rs`, `routing.rs`, `shutdown.rs`, `socket_mode.rs`) defines its
own `unique_socket_path` with the identical shape - `std::process::id()` plus a
per-process-static `AtomicU64` counter, flat in `std::env::temp_dir()` - the same
shape as `crates/steg`'s own seven duplicates, and the same latent collision: two
real processes that happen to be assigned the same pid at different points in this
machine's own churn, each making this its own first call, compute the identical
path. No cross-worktree sighting is recorded against `crates/server`'s own copies
specifically, but it is the same mechanism, confirmed by reading the actual source
of each file rather than inferred - **this plan proposes AC #3 is read as covering
both crates' own duplicated helper, not `crates/steg` alone**, since fixing one and
leaving nine more live copies of the identical bug in the sibling crate would not
match this criterion's own intent ("comes from one shared helper that cannot
collide across processes").

*A real constraint this design must respect, found by reading, not assumed*:
`bind.rs`'s own `temp_name` and its test module's own `unique_path` both state, from
a failure already hit while building this crate, that `sockaddr_un`'s 104-byte
`sun_path` budget leaves very little room once `$TMPDIR`'s own length is spent -
confirmed concretely on this machine, `$TMPDIR` is 49 bytes, leaving 55 for
everything after it. `crates/sdk/src/paths.rs::socket_path`'s own doc comment goes
further: production socket paths are deliberately "flat, no subdirectory" for this
exact reason, with a worked example showing a realistic `$TMPDIR` plus `steg.sock`
already "close to half" the 104-byte budget. A per-test *directory* (what this task
is asked to build) spends some of that remaining 55 bytes that the established
production convention spends none of - a real tension with that convention, named
here rather than silently assumed safe. Mitigation, kept inside the remaining
budget: the directory's own name carries no descriptive label (matching every
existing helper in this codebase, all of which already drop a `label` parameter for
exactly this reason) - just `std::process::id()` (up to 7 digits), a short counter,
and a compact clock token (the low 24 bits of a nanosecond timestamp, 6 hex
characters, not the full 19-digit value), roughly 20 bytes including separators and
the trailing `/`; comfortably inside the 55-byte remainder even before the socket's
own short filename is added. Every call site that builds a socket path from this
helper also asserts the finished path's byte length against a stated safe margin
(proposed 100 of the 104-byte budget, leaving 4 bytes of slack for a trailing NUL/
platform difference) and panics naming the length and the budget if it is ever
exceeded - so a future caller that appends something longer fails loudly at the
point of the mistake, not with a cryptic `bind()` error later, the same way
`bind.rs`'s own comment already treats this as a real, previously-hit failure, not
a theoretical one.

*Design, on the standard library only, per the engineer's own instruction - no new
dependency, `tempfile` named only as the alternative below*: one new type,
`ScopedTempDir`, built the same way in both `crates/server/tests/support/mod.rs`
(already exists; this helper is added to it) and a new `crates/steg/tests/
support/mod.rs` (new file, same "duplicated, not shared from `src/`, because each
`tests/*.rs` file is its own compilation unit" reasoning the server crate's module
already states for `run_bounded`/`wait_child_bounded`). Shape:
```
pub struct ScopedTempDir(PathBuf);

impl ScopedTempDir {
    pub fn new() -> Self {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        for _ in 0..1000 {
            let counter = COUNTER.fetch_add(1, Ordering::Relaxed);
            let clock = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .subsec_nanos()
                & 0xFF_FFFF; // 24 bits, 6 hex chars - compact on purpose, see AC #3's own sun_path note
            let path = std::env::temp_dir()
                .join(format!("{}-{counter:x}-{clock:06x}", std::process::id()));
            match std::fs::create_dir(&path) {
                Ok(()) => return Self(path),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => panic!("could not create a scoped temp dir at {path:?}: {e}"),
            }
        }
        panic!("could not find a free scoped temp dir name after 1000 attempts");
    }

    /// `name`'s own length, plus this directory's, is asserted against
    /// `sockaddr_un`'s 104-byte budget here, not left to fail opaquely at
    /// `bind()` (crates/sdk/src/paths.rs's own documented policy, and
    /// bind.rs's own comment: found exactly this way once already).
    pub fn socket_path(&self, name: &str) -> PathBuf {
        let path = self.0.join(name);
        let len = path.as_os_str().len();
        assert!(
            len <= 100,
            "socket path {path:?} is {len} bytes, over the 100-byte safety margin \
             under sockaddr_un's 104-byte sun_path budget - shorten `name`"
        );
        path
    }
}

impl Drop for ScopedTempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
```
Every `unique_socket_path`/`unique_output_path` call site in the sixteen affected
files (nine in `crates/server/tests/`, seven in `crates/steg/tests/`) is replaced by
a `ScopedTempDir` created at the top of the test and kept alive for its own
duration (a local binding, not dropped early), with `.socket_path("s.sock")` (or
`"o"` for `login_environment.rs`'s separate output path) in place of the old
function call. `crates/server/tests/runtime.rs`'s own `UNIQUE` counter (a pid-file
path, not a socket) is a different, lower-risk case - named here, left alone: it
names a plain file, not bound under `sockaddr_un`'s budget at all, and no
collision has been sighted against it; folding it in would widen this task past
what was asked without a sighting to justify it.

*Alternative named for the engineer, matching `crates/sdk/src/paths.rs`'s own
"flat, no subdirectory" convention more closely*: instead of a directory, a flat
"claim" on the final path itself - `std::fs::File::options().create_new(true).
open(candidate)` (fails if the path already exists, the same create-or-retry
primitive, with no new directory component spent on the `sun_path` budget at all),
immediately removed again right before the caller's own real socket (often bound by
a *different*, spawned process) is created at that same path. This closes the
pid-reuse-across-process-lifetimes window (the one actually sighted) just as
completely as the directory version, at the cost of a narrower, but nonzero,
TOCTOU gap between the claim's removal and the real bind - named, not built, since
the engineer's own instruction specified the directory shape; flagged because it
sits closer to this codebase's own stated production convention.

*Reproduction, forced directly, not by waiting on the kernel to reuse a pid*: pid
reuse itself cannot be forced on demand. What *can* be forced is the mechanism:
extract the current (pre-fix) naming into a pure function taking `pid` and
`counter` as explicit arguments rather than reading `std::process::id()` internally,
then call it twice with the *same* `(pid, counter)` pair - exactly the situation two
real, non-concurrent OS processes are in whenever the kernel happens to reuse a pid
and each makes this its own first call - and show the two results are identical
(trivial for a pure function, but the point is this *is* the sighted bug's whole
mechanism, named precisely, not approximated); then run the full, scripted
end-to-end version once: with the stand-in pid fixed, create a real file at the
first computed path (modelling an earlier run's own leftover socket), then compute
the path again with the same stand-in pid and show a plain `std::fs::write` to it
fails exactly the way the sighting's own description does ("failed writing a plain
file because a socket already existed at the same temporary path"). For the fixed
`ScopedTempDir`, show the equivalent two-call sequence with the same stand-in pid
and counter never produces the same directory twice, because the clock component
differs between the two calls even when both are forced to carry the identical pid
and counter.

## 3. Files touched, and the overlap with TASK-1.88/1.90

Both are In Progress right now: TASK-1.88 in `crates/server/src/runtime/
lifecycle.rs` and the `dispatch/` module; TASK-1.90 in `crates/server/src/
runtime/input.rs`, `crates/server/src/pane.rs`, `crates/server/src/dispatch/pane.rs`,
and - the one place this task's own touch comes close - it may add tests to
`crates/server/tests/attach.rs`. This task's own touch on `attach.rs` is kept to
exactly two things: wrapping the five named flood tests in the new
`cross_process_flood_guard()`, and widening `call_and_read_reply`'s bound - nothing
in the file's own test logic, assertions, or fixtures beyond those two, so a commit
here is a narrow, mechanical diff against whatever TASK-1.90 lands, not a
structural rewrite competing with it. This task's full touch list: `crates/server/
tests/attach.rs` (narrow, as above), `crates/server/src/signal.rs` (class d),
`crates/server/src/login_env.rs` (class b), `crates/server/tests/support/mod.rs`
(the new `ScopedTempDir`), nine `crates/server/tests/*.rs` files' own
`unique_socket_path` call sites (class e; none of these nine are
`runtime.rs`/`pane.rs`/`dispatch/*`/`input.rs`/`lifecycle.rs`, so no file TASK-1.88
or TASK-1.90 touches is in this list beyond `attach.rs` itself), a new
`crates/steg/tests/support/mod.rs`, seven `crates/steg/tests/*.rs` files' own call
sites, and `.claude/rules/testing-rust.md`.

## 4. Proof and notes (AC #4)

Record checks via `asdlc record-check`: for each of the four classes, the specific
test file touched, before (reproduction, recorded failing or - for class (d), should
it not reproduce - recorded as not reproduced, stated plainly) and after (the same
reproduction, recorded passing, over the stated number of runs); `cargo test -p
steg-server --lib` and `--test attach` together once all of part (i)'s own files are
done; `cargo test -p steg --test server_start` and the other six `crates/steg`
files; a final `just check`.

The notes record, per AC #4: the remaining unbounded waits across both crates (the
inventory and its own tooling-verified count) go to the draft task for part (ii);
the AST-based check for criterion #2 of the original task goes to the draft task
for part (iii), sequenced after part (ii) so its own violation list is checked
against an already-clean tree; `crates/sdk`'s one named exception
(`local_socket.rs`'s `CooperativeHeldOpen` fixture, a 100ms timing margin rather
than an unbounded wait) is named as its own, separate, size-1 follow-up, not folded
into either draft task; and the "terminal-scrolling timing test" sighting (class c
of the original five) is recorded as accepted-unlocated, per the engineer's own
instruction - TASK-1.128's worker reported it from one CI run without naming the
test, it was not found by keyword search across `crates/server`, `crates/steg`, or
`crates/kernel`, and it is not pursued further here.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Sightings gathered by @bruk.o on 2026-11-18, while six workers built on one machine (load average 4 to 19). None is in a file the affected tasks touched; main's CI was green throughout. (1) crates/server/tests/attach.rs, the stuck-pane and bound tests: an_unresponsive_panes_own_commands_do_not_stall_the_connection_or_another_pane (most often; fails with 'must answer within a bound', or a read timing out at the helper call_and_read_reply's own five-second socket timeout near line 149), a_second_connections_control_call_is_not_blocked_by_a_stuck_panes_lock_held_attach, pane_close_force_returns_within_its_bound_even_while_the_panes_own_worker_is_stuck, stopping_the_server_returns_within_its_bound_even_while_a_panes_worker_is_stuck, a_pane_that_closes_between_a_queued_attach_and_its_arm_leaves_nothing_to_detach. Seen by the workers of TASK-1.119, 1.130 and 1.135 across about fifteen runs of just check, a different subset each time. Run alone the first test failed six times in a row for the orchestrator (about five seconds each) while another worktree's attach test binary was running at 200 percent CPU, then passed once that had stopped: these tests flood a pane's input, and two such binaries at once starve each other even on a machine with idle cores. The in-process FLOOD_TEST_LOCK serialises them inside one binary only. (2) login_env's backgrounded-grandchild test, under load, many sightings. (3) a terminal-scrolling timing test, once on CI's first run for TASK-1.128. (4) signal::tests::send_reaches_the_whole_process_group_not_only_the_leader, once on CI's first run for TASK-1.132, green on rerun; CI runs alone, so this one is not load from other workers. For the plan: liveness bounds here prove 'answers at all', so they can be generous; the helper's default five-second read timeout is the tightest bound in the file; and a machine-wide guard for the flood tests may be worth more than widening each bound. TASK-1.132 limits concurrent cargo steps to two, which reduces but does not remove two attach binaries overlapping.

Fifth sighting class, 2026-11-18 (TASK-1.135's worker, load about 3): crates/steg/tests/server_start.rs::an_untrusted_existing_file_refuses_with_status_4_and_names_the_reason failed writing a plain file because a socket already existed at the same temporary path - a path collision between two worktrees running crates/steg's tests at once (TASK-1.119's worker was running them). Passed alone and as the whole file. The helper that makes 'unique' socket paths is duplicated across this crate's test files and is evidently not unique across processes; a per-test temporary directory would remove the class. An earlier sdk local_socket path collision is logged on TASK-1.82.

From TASK-1.119's third review (2026-11-18), two test nits left because the task locked on passing: crates/sdk/src/local_socket.rs's cooperative test fixture (Behaviour::CooperativeHeldOpen, near line 586) sleeps 100 ms so the peer does not close before the client clears its timeouts - it passed 300 of 300 runs at load 3, but it is a timing margin, not a synchronised event; have the listener thread hold the stream until the test signals it instead. And a_hello_sent_then_closed_immediately_never_reports_closed_before_hello loosened its attempt-count assertion from exactly 1 to at least 1 without needing to.

Narrowed and split by @bruk.o on 2026-11-18, as the plan recommended: this task keeps the cross-worktree clashes and the tightest bounds, which cost three tasks their local checks on 2026-11-18, plus the rule line (the criterion kept from before). The mechanical bounding of the remaining waits and the tools check that enforces it moved to two draft tasks. Orchestrator's answers to the plan's questions: the terminal-scrolling sighting (class c) is accepted as unlocated - TASK-1.128's worker reported it from one CI run without naming the test; the temporary-directory helper is to be built on the standard library (create_dir fails if the name exists, so a name from process id, a counter and the clock can be retried) so no dependency or package boundary changes - tempfile stays the named alternative if the engineer prefers it.
<!-- SECTION:NOTES:END -->
