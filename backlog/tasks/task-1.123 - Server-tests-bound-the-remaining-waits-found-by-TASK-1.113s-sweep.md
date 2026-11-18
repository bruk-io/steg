---
id: TASK-1.123
title: 'Server tests: bound the remaining waits found by TASK-1.113''s sweep'
status: In Review
assignee:
  - '@bruk.w5'
created_date: '2026-11-17T01:33:00Z'
updated_date: '2026-11-18T20:49:00Z'
labels:
  - size-3
milestone: m-3
dependencies:
  - TASK-1.113
references:
  - task-1.123-server-tests-bound-the-remaining-waits
  - https://github.com/bruk-io/steg/pull/81
parent_task_id: TASK-1
project: bruk-io-steg
asdlc_accepted_from: DRAFT-1
asdlc_branch: task-1.123-server-tests-bound-the-remaining-waits
asdlc_plan_approved_by: bruk
asdlc_plan_approved_hash: 'a21307bb73f5'
asdlc_plan_approved_size: size-3
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

Plan checked against the task's four criteria by @bruk.o: ok. Each of the four clashes gets a forced reproduction recorded failing first (two copies of the attach test binary at once; the login_env test under induced load; a naming function called twice with the same process id and counter), then the fix recorded passing; for signal's process-group test the plan states it will say plainly if the failure cannot be forced, and record no control it does not have. Fixes: a machine-wide advisory lock for the five flood tests, outside any worktree, acquired with a bound and released by the operating system if the test is killed, with the argument for why it cannot deadlock against TASK-1.132's two cargo slots; liveness bounds of 15 seconds; a bounded poll in the signal test; one standard-library temporary-directory helper replacing the duplicated unique-path helpers in sixteen test files across crates/server and crates/steg, with a compact name and a length assertion because a socket path has about 104 bytes. Its touch on attach.rs is the lock and one bound.

Check passed: `cargo test -p steg-server --features race-fixture --test race` exited 0 at 2026-11-18T19:21:13+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.123 on task-1.123-server-tests-bound-the-remaining-waits at c9289ac47d97 with uncommitted changes:

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.20s
     Running tests/race.rs (target/debug/deps/race-d9220f423a0e57f7)

running 10 tests
test support::tests::the_old_pid_and_counter_only_scheme_collides_when_two_processes_share_a_pid ... ok
test support::tests::candidate_name_does_not_collide_under_the_same_forced_pid_and_counter ... ok
test support::tests::scoped_temp_dir_exists_after_new_and_is_gone_after_drop ... ok
test support::tests::path_panics_rather_than_hand_back_something_over_the_sun_path_budget - should panic ... ok
test support::tests::two_scoped_temp_dirs_made_back_to_back_are_different ... ok
test support::tests::create_unique_dir_skips_a_name_that_already_exists ... ok
test attempt_times_out_when_the_lock_stays_contended_and_nothing_answers ... ok
test a_socket_file_left_by_a_real_dead_process_is_replaced ... ok
test eight_simultaneous_starts_leave_exactly_one_server ... ok
test support::tests::wait_child_bounded_kills_and_reaps_a_child_that_outlives_the_bound ... ok

test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.31s
```

Check passed: `cargo test -p steg --features fixtures` exited 0 at 2026-11-18T19:27:10+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.123 on task-1.123-server-tests-bound-the-remaining-waits at c9289ac47d97 with uncommitted changes, last 40 of 190 lines:

```text
test support::tests::scoped_temp_dir_exists_after_new_and_is_gone_after_drop ... ok
test support::tests::two_scoped_temp_dirs_made_back_to_back_are_different ... ok
test support::tests::create_unique_dir_skips_a_name_that_already_exists ... ok
test a_pane_started_by_a_real_on_demand_server_never_inherits_the_launching_clients_own_environment ... ok

test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.30s

     Running tests/server_start.rs (target/debug/deps/server_start-ab0ae1f7b0736962)

running 22 tests
test support::tests::the_old_pid_and_counter_only_scheme_collides_when_two_processes_share_a_pid ... ok
test support::tests::candidate_name_does_not_collide_under_the_same_forced_pid_and_counter ... ok
test support::tests::scoped_temp_dir_exists_after_new_and_is_gone_after_drop ... ok
test support::tests::one_scoped_temp_dir_can_hand_out_two_different_paths ... ok
test support::tests::create_unique_dir_skips_a_name_that_already_exists ... ok
test support::tests::path_panics_rather_than_hand_back_something_over_the_sun_path_budget - should panic ... ok
test support::tests::two_scoped_temp_dirs_made_back_to_back_are_different ... ok
test an_unrecognised_server_start_variant_exits_2 ... ok
test server_start_help_prints_the_generated_help_and_starts_nothing ... ok
test an_untrusted_existing_file_refuses_with_status_4_and_names_the_reason ... ok
test started_is_printed_only_after_the_socket_already_accepts ... ok
test a_fresh_start_replaces_the_socket_of_a_server_killed_outright ... ok
test four_simultaneous_starts_converge_on_one_server ... ok
test on_demand_given_after_no_on_demand_wins ... ok
test typing_on_demand_at_a_shell_prompt_carries_on_attached_with_a_message ... ok
test surviving_descriptor_exhaustion_still_serves_existing_connections_and_recovers ... ok
test a_losing_racers_dropped_child_is_a_zombie_until_reaped ... ok
test server_start_no_on_demand_resolves_to_the_explicit_kind_and_never_attempts_to_detach ... ok
test no_on_demand_given_after_on_demand_wins ... ok
test explicit_start_stays_up_with_no_client_ever_connecting ... ok
test the_client_being_sigkilled_does_not_end_the_server ... ok
test the_clients_process_group_being_sighupd_does_not_end_the_server ... ok

test result: ok. 22 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.31s

   Doc-tests steg

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

Check passed: `cargo test -p steg-server --test attach` exited 0 at 2026-11-18T19:30:28+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.123 on task-1.123-server-tests-bound-the-remaining-waits at c9289ac47d97 with uncommitted changes:

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.20s
     Running tests/attach.rs (target/debug/deps/attach-1c3f77caae0e73d3)

running 30 tests
test support::tests::the_old_pid_and_counter_only_scheme_collides_when_two_processes_share_a_pid ... ok
test support::tests::candidate_name_does_not_collide_under_the_same_forced_pid_and_counter ... ok
test support::tests::scoped_temp_dir_exists_after_new_and_is_gone_after_drop ... ok
test support::tests::path_panics_rather_than_hand_back_something_over_the_sun_path_budget - should panic ... ok
test support::tests::two_scoped_temp_dirs_made_back_to_back_are_different ... ok
test support::tests::create_unique_dir_skips_a_name_that_already_exists ... ok
test detaching_a_pane_this_connection_never_attached_to_is_a_bad_request ... ok
test detaching_a_pane_that_already_closed_still_releases_the_stream ... ok
test disconnecting_without_detaching_does_not_leak_or_block_a_later_attach ... ok
test attach_returns_the_real_screen_and_updates_follow_it ... ok
test declaring_a_size_of_zero_is_refused ... ok
test a_frame_on_an_already_detached_stream_closes_the_connection ... ok
test typed_bytes_reach_a_real_program_in_order ... ok
test input_clears_a_finished_status ... ok
test detaching_releases_the_claim_and_the_survivor_returns_to_its_own_declared_size ... ok
test a_dropped_connection_releases_its_claim_the_same_way ... ok
test a_declared_size_reaches_the_pty_and_is_reported_on_every_attached_stream ... ok
test two_connections_attached_to_one_pane_each_get_their_own_updates_and_a_detach_does_not_touch_the_other ... ok
test repeated_attach_and_detach_against_a_flooding_pane_never_sees_an_unknown_stream ... ok
test support::tests::wait_child_bounded_kills_and_reaps_a_child_that_outlives_the_bound ... ok
test pane_close_force_reports_ended_closed_not_the_signals_own_exit_code ... ok
test a_program_that_exits_on_its_own_sends_ended_with_its_real_exit_code ... ok
test attaching_to_a_pane_whose_program_already_ended_gets_a_final_full_then_ended ... ok
test detach_ends_the_stream_even_though_the_program_keeps_producing_output ... ok
test two_clients_attached_to_one_pane_both_get_ended_for_the_same_exit ... ok
test a_pane_that_closes_between_a_queued_attach_and_its_arm_leaves_nothing_to_detach ... ok
test an_unresponsive_panes_own_commands_do_not_stall_the_connection_or_another_pane ... ok
test a_second_connections_control_call_is_not_blocked_by_a_stuck_panes_lock_held_attach ... ok
test pane_close_force_returns_within_its_bound_even_while_the_panes_own_worker_is_stuck ... ok
test stopping_the_server_returns_within_its_bound_even_while_a_panes_worker_is_stuck ... ok

test result: ok. 30 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 30.40s
```

Check passed: `cargo test -p steg-server --lib` exited 0 at 2026-11-18T19:52:45+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.123 on task-1.123-server-tests-bound-the-remaining-waits at c9289ac47d97 with uncommitted changes, last 40 of 216 lines:

```text
test pane::attach_tests::an_attachment_gets_nothing_before_it_is_armed_even_if_output_happens_first ... ok
test runtime::tests::a_panic_while_holding_the_lock_does_not_poison_a_later_call ... ok
test runtime::tests::runtime_new_does_not_panic_and_starts_empty ... ok
test signal::tests::send_reaches_a_real_process_with_the_named_signal ... ok
test runtime::start::tests::a_shell_pane_not_only_a_command_pane_gets_the_same_composed_environment ... ok
test pane::attach_tests::attaching_to_a_pane_that_already_ended_gets_a_final_full_then_ended ... ok
test test_support::tests::a_body_that_returns_promptly_is_not_delayed ... ok
test runtime::start::tests::steg_vars_and_term_always_win_over_a_conflicting_value_from_the_login_shells_own_base_env ... ok
kill: 16364: No such process
test signal::tests::send_reaches_the_whole_process_group_not_only_the_leader ... ok
test runtime::start::tests::pane_split_falls_back_to_home_when_the_inherited_live_cwd_no_longer_exists ... ok
test runtime::start::tests::pane_split_inherits_the_live_cwd_of_the_pane_it_split_from_not_its_started_in_cwd ... ok
test runtime::attach::tests::attach_pane_on_a_busy_panes_own_worker_does_not_stall_an_unrelated_requests_lock ... ok
test login_env::tests::capture_login_environment_falls_back_on_a_shell_that_never_finishes_in_time ... ok
test pane::attach_tests::attaching_returns_the_panes_current_screen_and_updates_follow_it ... ok
test pane::attach_tests::a_program_that_exits_on_its_own_sends_ended_with_its_real_exit_code_after_its_last_content ... ok
test pane::attach_tests::mark_closing_sent_before_a_programs_own_exit_is_the_reason_ended_reports ... ok
test runtime::events::tests::an_unfiltered_subscriber_receives_nothing ... ok
test pane::ending_tests::await_exit_is_not_delayed_by_a_backlog_on_the_workers_own_queue ... ok
test runtime::input::tests::the_general_sizing_step_reaches_every_pane_through_split_close_and_swap ... ok
kill: 16187: No such process
test runtime::lifecycle::stop_tests::stop_ends_a_program_that_traps_the_polite_signal ... ok
test test_support::tests::a_body_that_never_returns_fails_fast_naming_what_it_waited_for - should panic ... ok
test pane::ending_tests::signal_hangup_reaches_a_backgrounded_grandchild_too ... ok
test idle::tests::the_checker_blocks_with_no_timeout_while_ineligible_and_waits_exactly_once_while_eligible ... ok
test runtime::events::tests::a_mark_then_the_shells_own_real_exit_cascades_with_no_panic ... ok
test runtime::events::tests::a_shell_with_no_marks_leaves_every_status_field_unchanged_until_one_arrives ... ok
test pane::ending_tests::signal_kill_ends_a_program_that_traps_the_polite_signal ... ok
test login_env::tests::capture_login_environment_falls_back_on_a_shell_that_exits_non_zero ... ok
test runtime::events::tests::a_typed_commands_status_reaches_pane_show_and_tab_show_through_runtime_handle ... ok
test runtime::events::tests::a_typed_command_that_succeeds_is_running_then_finished_and_the_tab_shows_it ... ok
test runtime::events::tests::a_typed_command_that_fails_is_failed_with_its_exit_code ... ok
test login_env::tests::capture_login_environment_prints_noise_before_the_marker_without_corrupting_the_dump ... ok
test runtime::events::tests::a_command_panes_own_program_printing_marks_does_not_change_its_status ... ok
kill: 16447: No such process
test login_env::tests::capture_login_environment_kills_a_backgrounded_grandchild_that_held_the_pipe_open ... ok
kill: 16155: No such process
test pane::tests::a_forward_failing_once_the_worker_is_gone_still_reaps_a_program_that_ends_on_its_own ... ok

test result: ok. 203 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.05s
```

Check passed: `cargo test -p steg-server --lib` exited 0 at 2026-11-18T19:56:08+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.123 on task-1.123-server-tests-bound-the-remaining-waits at c9289ac47d97 with uncommitted changes, last 40 of 216 lines:

```text
test pane::attach_tests::an_attachment_gets_nothing_before_it_is_armed_even_if_output_happens_first ... ok
test runtime::tests::a_panic_while_holding_the_lock_does_not_poison_a_later_call ... ok
test runtime::tests::runtime_new_does_not_panic_and_starts_empty ... ok
test signal::tests::send_reaches_a_real_process_with_the_named_signal ... ok
test runtime::start::tests::a_shell_pane_not_only_a_command_pane_gets_the_same_composed_environment ... ok
test pane::attach_tests::attaching_to_a_pane_that_already_ended_gets_a_final_full_then_ended ... ok
test test_support::tests::a_body_that_returns_promptly_is_not_delayed ... ok
kill: 21913: No such process
test signal::tests::send_reaches_the_whole_process_group_not_only_the_leader ... ok
test runtime::start::tests::steg_vars_and_term_always_win_over_a_conflicting_value_from_the_login_shells_own_base_env ... ok
test runtime::start::tests::pane_split_falls_back_to_home_when_the_inherited_live_cwd_no_longer_exists ... ok
test runtime::start::tests::pane_split_inherits_the_live_cwd_of_the_pane_it_split_from_not_its_started_in_cwd ... ok
test runtime::attach::tests::attach_pane_on_a_busy_panes_own_worker_does_not_stall_an_unrelated_requests_lock ... ok
test login_env::tests::capture_login_environment_falls_back_on_a_shell_that_never_finishes_in_time ... ok
test pane::attach_tests::a_program_that_exits_on_its_own_sends_ended_with_its_real_exit_code_after_its_last_content ... ok
test pane::attach_tests::attaching_returns_the_panes_current_screen_and_updates_follow_it ... ok
test pane::attach_tests::mark_closing_sent_before_a_programs_own_exit_is_the_reason_ended_reports ... ok
test runtime::events::tests::an_unfiltered_subscriber_receives_nothing ... ok
test pane::ending_tests::await_exit_is_not_delayed_by_a_backlog_on_the_workers_own_queue ... ok
test runtime::input::tests::the_general_sizing_step_reaches_every_pane_through_split_close_and_swap ... ok
kill: 21734: No such process
test runtime::lifecycle::stop_tests::stop_ends_a_program_that_traps_the_polite_signal ... ok
test pane::ending_tests::signal_hangup_reaches_a_backgrounded_grandchild_too ... ok
test test_support::tests::a_body_that_never_returns_fails_fast_naming_what_it_waited_for - should panic ... ok
test idle::tests::the_checker_blocks_with_no_timeout_while_ineligible_and_waits_exactly_once_while_eligible ... ok
test runtime::events::tests::a_mark_then_the_shells_own_real_exit_cascades_with_no_panic ... ok
test runtime::events::tests::a_shell_with_no_marks_leaves_every_status_field_unchanged_until_one_arrives ... ok
test pane::ending_tests::signal_kill_ends_a_program_that_traps_the_polite_signal ... ok
test runtime::events::tests::a_typed_command_that_fails_is_failed_with_its_exit_code ... ok
test runtime::events::tests::a_typed_command_that_succeeds_is_running_then_finished_and_the_tab_shows_it ... ok
test runtime::events::tests::a_typed_commands_status_reaches_pane_show_and_tab_show_through_runtime_handle ... ok
test login_env::tests::capture_login_environment_falls_back_on_a_shell_that_exits_non_zero ... ok
test login_env::tests::capture_login_environment_prints_noise_before_the_marker_without_corrupting_the_dump ... ok
test runtime::events::tests::a_command_panes_own_program_printing_marks_does_not_change_its_status ... ok
kill: 21998: No such process
test login_env::tests::capture_login_environment_kills_a_backgrounded_grandchild_that_held_the_pipe_open ... ok
kill: 21702: No such process
test pane::tests::a_forward_failing_once_the_worker_is_gone_still_reaps_a_program_that_ends_on_its_own ... ok

test result: ok. 203 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.05s
```

Check passed: `cargo test -p steg-server --test attach` exited 0 at 2026-11-18T20:11:52+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.123 on task-1.123-server-tests-bound-the-remaining-waits at c9289ac47d97 with uncommitted changes:

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.08s
     Running tests/attach.rs (target/debug/deps/attach-1c3f77caae0e73d3)

running 31 tests
test support::tests::candidate_name_does_not_collide_under_the_same_forced_pid_and_counter ... ok
test support::tests::the_old_pid_and_counter_only_scheme_collides_when_two_processes_share_a_pid ... ok
test support::tests::path_panics_rather_than_hand_back_something_over_the_sun_path_budget - should panic ... ok
test support::tests::a_fresh_scoped_temp_dir_is_0700 ... ok
test support::tests::scoped_temp_dir_exists_after_new_and_is_gone_after_drop ... ok
test support::tests::two_scoped_temp_dirs_made_back_to_back_are_different ... ok
test support::tests::create_unique_dir_skips_a_name_that_already_exists ... ok
test detaching_a_pane_that_already_closed_still_releases_the_stream ... ok
test detaching_a_pane_this_connection_never_attached_to_is_a_bad_request ... ok
test a_frame_on_an_already_detached_stream_closes_the_connection ... ok
test declaring_a_size_of_zero_is_refused ... ok
test disconnecting_without_detaching_does_not_leak_or_block_a_later_attach ... ok
test a_dropped_connection_releases_its_claim_the_same_way ... ok
test attach_returns_the_real_screen_and_updates_follow_it ... ok
test a_declared_size_reaches_the_pty_and_is_reported_on_every_attached_stream ... ok
test input_clears_a_finished_status ... ok
test detaching_releases_the_claim_and_the_survivor_returns_to_its_own_declared_size ... ok
test typed_bytes_reach_a_real_program_in_order ... ok
test two_connections_attached_to_one_pane_each_get_their_own_updates_and_a_detach_does_not_touch_the_other ... ok
test repeated_attach_and_detach_against_a_flooding_pane_never_sees_an_unknown_stream ... ok
test support::tests::wait_child_bounded_kills_and_reaps_a_child_that_outlives_the_bound ... ok
test pane_close_force_reports_ended_closed_not_the_signals_own_exit_code ... ok
test a_program_that_exits_on_its_own_sends_ended_with_its_real_exit_code ... ok
test attaching_to_a_pane_whose_program_already_ended_gets_a_final_full_then_ended ... ok
test detach_ends_the_stream_even_though_the_program_keeps_producing_output ... ok
test two_clients_attached_to_one_pane_both_get_ended_for_the_same_exit ... ok
test a_pane_that_closes_between_a_queued_attach_and_its_arm_leaves_nothing_to_detach ... ok
test a_second_connections_control_call_is_not_blocked_by_a_stuck_panes_lock_held_attach ... ok
test an_unresponsive_panes_own_commands_do_not_stall_the_connection_or_another_pane ... ok
test pane_close_force_returns_within_its_bound_even_while_the_panes_own_worker_is_stuck ... ok
test stopping_the_server_returns_within_its_bound_even_while_a_panes_worker_is_stuck ... ok

test result: ok. 31 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 31.26s
```

Check passed: `just check` exited 0 at 2026-11-18T20:25:42+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.123 on task-1.123-server-tests-bound-the-remaining-waits at c9289ac47d97 with uncommitted changes:

```text
ok      format check (0.3s)
ok      lint (0.4s)
ok      boundaries (0.1s)
ok      foundations (0.1s)
ok      foundations public surface (0.0s)
ok      design pages (0.1s)
ok      licences (0.6s)
ok      tests (-p steg -p steg-server) (52.6s)
ok      build (steg-sdk) (0.1s)
ok      test (steg-sdk) (0.3s)
ok      test (steg-sdk, local-socket) (0.5s)
ok      build (steg-kernel) (0.1s)
ok      test (steg-kernel) (0.2s)
ok      build (steg-server) (0.1s)
ok      test (steg-server) (52.7s)
ok      test (steg-server, race-fixture) (50.4s)
ok      test (steg-server, pty-race-fixture) (54.2s)
ok      test (steg-server, accept-backoff-fixture) (52.8s)
ok      build (steg) (0.1s)
ok      test (steg) (1.6s)
ok      test (steg, fixtures) (4.8s)
ok      build (steg-ui) (0.3s)
ok      test (steg-ui) (0.4s)
ok      build (steg-app) (0.3s)
ok      test (steg-app) (0.8s)
ok      tool tests (39.9s)
ok      architecture (0.7s)
ok      recipe names (0.0s)
ok      ui levels (0.0s)
ok      ui colours (0.0s)
```

Built per the approved, narrowed plan (plan a21307bb73f5): the four clash classes,
call_and_read_reply's bound, one shared temporary-path helper across both crates, and
the rule line. PR #81, branch task-1.123-server-tests-bound-the-remaining-waits.

AC #1 (rule line): added to .claude/rules/testing-rust.md, beside "no test sleeps to
wait for something; it waits on the event" - the macOS dropped-socket sentence from
TASK-1.113's own CLOEXEC finding.

AC #2 (four classes, each reproduced then fixed):

- Class (a), attach.rs flood-test starvation: DEFAULT_READ_TIMEOUT (connect_and_
  handshake's own connection default) widened 5s to 15s, three dependent logical
  deadlines widened 10s to 20s, and a new cross-process flock lock
  (cross_process_flood_guard, nix::fcntl::Flock - the same primitive arbiter.rs
  already uses) taken only by the five named flood tests, at a fixed path outside
  every worktree ($XDG_CACHE_HOME/steg/test-locks, else $HOME/.cache/steg/test-locks,
  mirroring tools/src/steg_tools/quiet.py::lock_directory's own reasoning). Bounded
  acquisition (60s, named-what-it-waited-for panic on timeout); released by the OS on
  SIGKILL, no stale-lock cleanup written because none is needed.
  Reproduced: the unmodified original code failed 20/20 under four concurrent copies
  of the compiled attach test binary (the exact sighted symptom, "pane.show must
  answer within a bound"); the fix passes 0/20 under the identical reproduction.
  Also checked in isolation: disabling only the lock (keeping the widened bounds)
  still passed 0/20 under 2-way concurrency on this machine at the time - the widened
  bounds alone already carry real weight at that level; the lock's own necessity is
  clearest at the sighting's own reported severity (a worktree's attach binary at
  200% CPU) or the 4-way level this task's own control used, not claimed to be
  independently isolated beyond that.

- Class (b), login_env's backgrounded-grandchild test. Departure from the plan,
  found while verifying this task's own build, not synthesised: the plan predicted
  the tight bound was the first assert's 5s outer check ("capture_login_environment
  must still return within a bound"); the real failure, hit organically during this
  task's own full-suite verification run, was the second, inner pid-file-appearance
  loop's own 10s deadline (login_env.rs, "the backgrounded grandchild's own pid file
  never appeared"). Both widened once the real failure point was identified: the
  outer check to 15s (matching attach.rs's own reasoning), the inner loop to 30s.
  Reproduced: the organic failure (10s bound) was followed minutes later by a clean
  pass (30s bound) against the identical command (cargo test -p steg-server
  --features race-fixture,pty-race-fixture,accept-backoff-fixture); forced reliably
  with the old 10s bound (8-9 of 9 failures across three rounds of three concurrent
  `cargo test -p steg-server --lib` invocations, no added synthetic load); the 30s
  bound passes the same single-invocation reproduction cleanly. Named plainly, not
  hidden: a self-stacked worst case (two or three copies of this crate's own
  heaviest, fork-centric lib test file running at once) still misses even the
  widened bound - this is not a case `just check` itself ever produces (TASK-1.132's
  own two-slot limiter caps heavy cargo steps machine-wide at two, normally
  different steps across different worktrees, not two copies of the identical
  heaviest file in one crate) and no further widening was pursued once that became
  clear - see the task's own notes for how this was determined.

- Class (d), signal.rs's process-group test: the single zero-slack
  `assert!(!process_exists(background_pid))` snapshot, taken once immediately after
  the leading shell is reaped, replaced with a bounded poll (wait_until_gone, 5s) -
  the same cross-process-liveness exception class as race.rs's own
  wait_until_accepting. Correct regardless of which of two candidate mechanisms
  actually fired (read from the code, not guessed): a zombie not yet reaped by its
  own init/subreaper after reparenting, or killpg racing the kernel's own group
  attachment. Not reproduced: 60 runs total (30 alone, 30 under 14 `yes` processes of
  synthetic CPU load) against the original single-check code found zero failures.
  Named as unresolved-by-reproduction in these notes, per the plan's own allowance -
  not claimed as a verified control.

- Class (e), temporary path collisions: ScopedTempDir (std library only - create_dir
  with its own AlreadyExists retry, a name built from pid, a per-process counter and
  a low-24-bit clock slice so two processes sharing a reused pid can no longer
  compute the same path, removed on Drop including through a panic). Lives in
  crates/server/tests/support/mod.rs (added alongside run_bounded/wait_child_bounded)
  and a new crates/steg/tests/support/mod.rs (the same module, duplicated for the
  same "each tests/*.rs file is its own compilation unit" reason already stated
  there). Replaces unique_socket_path/unique_output_path in all sixteen files named
  in the plan - nine in crates/server/tests/ (attach, accept_backoff, events,
  handshake, idle_stop, race, routing, shutdown, socket_mode), seven in
  crates/steg/tests/ (server_start, idle_stop, cli_exit_statuses, login_environment,
  cli_connection_arbitration, cli_api_call, cli_no_server) - confirmed by reading
  every file in both crates, not assumed from the task's own notes alone (which
  named only the crates/steg side).
  Found and fixed while building this: a plain create_dir inherits the ambient
  umask, and socket_mode.rs's own test (which deliberately sets a permissive umask
  to prove the socket's own mode does not depend on it) made the resulting
  world-writable directory get correctly refused by arbiter::attempt
  (UnsafeDirectory(WorldWritable)) - fixed with an explicit 0700 set_permissions
  call (the same reason bind.rs's own bind_with_mode never trusts the umask for the
  socket itself), and given its own unit test in both support/mod.rs copies
  (a_fresh_scoped_temp_dir_is_0700) that does not itself touch the process-wide
  umask, for the same reason socket_mode.rs's own doc comment already states that is
  unsafe from a module compiled into every other integration test binary in the
  crate.
  Reproduced the mechanism directly, not by waiting on the kernel to reuse a pid:
  the old scheme's naming (pid + counter only) was shown to collide when fed the
  same (pid, counter) pair twice (exactly what two real processes sharing a reused
  pid would each compute on their own first call); the new scheme does not collide
  under the identical forced input, because the clock component differs between the
  two calls - both as permanent unit tests in each support/mod.rs copy.

AC #3 (call_and_read_reply's bound, the shared helper): covered above (class a for
the bound, class e for the helper) - restated here because the criterion names them
separately from the four classes. The helper now covers crates/server's own nine
files too, not only crates/steg's seven, per the finding under class (e).

AC #4 (what is not fixed here, and where it went):
- The remaining unbounded waits across both crates (the file-by-file sweep) -
  draft-1, "Server and CLI tests: every remaining wait with no bound gets one".
- The tools check for criterion #2 of the original task - draft-2, "Tools: a check
  that a test's wait has a bound" - sequenced after draft-1 so its own violation
  list is checked against an already-clean tree.
- crates/sdk's one named exception (local_socket.rs's CooperativeHeldOpen fixture,
  a 100ms timing margin, not an unbounded wait) - named in draft-1's own notes as a
  tiny follow-up, not folded into either draft or this task.
- The "terminal-scrolling timing test" sighting (the original task's class (c)):
  accepted as unlocated, per the engineer's own instruction. Searched by keyword
  across crates/server, crates/steg and crates/kernel; not found. TASK-1.128's
  worker reported it from one CI run without naming the test. Not pursued further.

Checks recorded via asdlc record-check: cargo test -p steg-server --features
race-fixture --test race (the new ScopedTempDir helper's own unit tests);
cargo test -p steg --features fixtures (all seven crates/steg files); cargo test -p
steg-server --test attach (class a, 31 tests); cargo test -p steg-server --lib
(twice - classes b and d); just check, full workspace, clean. CI on PR #81: [see
final message].

ps checked clean of leftover processes after every reproduction loop (the attach
flood binaries, the concurrent --lib runs, the synthetic `yes` load) - each started
with tracked pids and killed explicitly, never by name.
<!-- SECTION:NOTES:END -->

## Comments

<!-- COMMENTS:BEGIN -->
author: @bruk
created: 2026-11-18T19:12:00Z

---

Plan approved by @bruk (a21307bb73f5, size-3): its worker may build it.

---
<!-- COMMENTS:END -->
