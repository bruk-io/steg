---
id: TASK-1.123
title: 'Server tests: bound the remaining waits found by TASK-1.113''s sweep'
status: In Progress
assignee:
  - '@bruk.w5'
created_date: '2026-11-17T01:33:00Z'
updated_date: '2026-11-18T14:03:00Z'
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
- [ ] #1 Every wait on a thread, a channel, a child process or a socket in the tests of crates/server and crates/steg has a bound, through the shared helpers, and a wait that passes its bound fails the test with a message naming what it waited for
- [ ] #2 A check in tools fails just check when a test file in those crates calls join, recv, a child wait or a blocking socket read with no bound, with a message saying which helper to use and why; it has its own tests
- [ ] #3 A line in the repository's Rust testing rule says a test must not rely on a dropped socket end failing its peer at once on macOS, with the reason
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
## Reading done before this plan (named, not restated)

`crates/server/tests/attach.rs` (`FLOOD_TEST_LOCK`, `call_and_read_reply`, the five
named tests' own bound comments), `crates/server/src/signal.rs` (both `#[cfg(test)]`
functions), the five duplicated `wait_bounded`/`unique_socket_path` helpers across
`crates/server/tests/accept_backoff.rs` and `crates/steg/tests/{server_start,
idle_stop,cli_exit_statuses,login_environment}.rs`, `crates/server/src/login_env.rs`'s
grandchild test, `crates/kernel` for the "terminal-scrolling" sighting (not found
there - see class (c) below), `tools/src/steg_tools/{ui_levels,rust_source}.py` and
`tools/src/steg_tools/quiet.py`'s own `fcntl.flock` slot mechanism, `justfile`,
`.claude/rules/testing-rust.md`, `plugins/bruk-io-steg/rules/testing-rust.md`,
TASK-1.113's full plan/notes/checks, TASK-1.82's notes, TASK-1.88 and TASK-1.90 (status
and dependency graph only, to size file overlap).

## 1. Inventory method and totals

Today's tree has moved past TASK-1.113's own "~60 sites" sweep (that count was taken
before `dispatch.rs` split into a directory, before `runtime.rs` split into modules,
and before `attach.rs`'s flood tests and `cli_exit_statuses.rs`/
`cli_connection_arbitration.rs`/`cli_api_call.rs` existed). A manual grep
(`.join()`, `.recv()`/`recv_timeout`/`try_recv`, `child.wait()`/`wait_timeout`,
`set_read_timeout`/`.read(`, `thread::sleep`) across `crates/server/{tests,src}` and
`crates/steg/tests` found on the order of 150-200 call sites touching these four
primitives. Read narrowly, three facts settle how AC #1 and AC #2 must be scoped
before any count is final:

- Most of these already have a bound (`recv_timeout(Duration::from_secs(N))`,
  `set_read_timeout(Some(...))`) - the task description's own words are "about 60
  more waits with **no bound**", not "every wait rewritten through the helper." A
  literal reading of AC #1's "through the shared helpers" would rewrite several
  hundred already-working `recv_timeout(Duration::from_secs(5))` calls for no safety
  gain. **Proposed interpretation, for the engineer to confirm**: split the inventory
  into two classes, counted and fixed separately -
  - **unbounded** (no timeout at all: a bare `.join()`, `.recv()`, `child.wait()`, or
    a socket read with no `set_read_timeout` ever set on that stream) - these get
    wrapped in `run_bounded`/`wait_child_bounded`/a `set_read_timeout` call.
  - **bounded but mute** (already has a timeout, but the failure path is
    `.unwrap()`/`.expect("...")` with no restatement of *what* was being waited for,
    so a timeout's panic message is generic, e.g. "called `Result::unwrap()` on an
    `Err` value: `Timeout`") - these get a message naming the wait, not a rewrite of
    the mechanism. AC #1's second clause ("fails with a message naming what it
    waited for") is the thing actually missing here, not the bound.
- A plain grep cannot tell test code from production code. `listen.rs:258,539,563,
  733`, `connection.rs:337`, `runtime/close.rs:367`, `pane.rs:240,1647,2295,3073`,
  `runtime/mod.rs:350` all matched `.join()` but sit outside any `#[cfg(test)]`
  module - production code, out of AC #1's scope ("the tests of crates/server"), and
  must not be touched by this task. This is the strongest argument for the
  structural (AST) check in part 3, not a weaker regex: a regex has no notion of
  "inside `#[cfg(test)]`" or "inside `tests/`", an AST query does.
- `crates/server/src/runtime/testing.rs` is the shared test helpers for the
  `runtime/*.rs` split (TASK-1.113 predates this split); it is gated the same way the
  rest of the module's unit tests are and is in scope the same as any other unit-test
  file.

**Representative, file-level counts found by the manual sweep** (final, authoritative
count comes from the AST check built in part 3 - a hand grep already produced false
positives above, so this table is sized for splitting the work, not cited as proof of
completeness):

| File | unbounded `.join()`/`.recv()`/`child.wait()` | already a local sleep-poll helper | notes |
|---|---|---|---|
| `tests/shutdown.rs` | ~7 plain `.join()` | - | `stop()` calls, `recv_timeout` elsewhere already bounded |
| `tests/runtime.rs` | ~5 plain `.join()` (`forcing.join().unwrap()`) | - | |
| `tests/handshake.rs` | ~2 plain `.join()` | - | every `recv_timeout`/`read_timeout` already bounded |
| `tests/events.rs` | ~2 plain `.join()` | - | |
| `tests/idle_stop.rs` (server) | ~4 plain `.join()` | - | |
| `tests/routing.rs` | ~4 plain `.join()` | - | |
| `tests/attach.rs` | 1 plain `.join()` (`stop_server`) | - | see class (a), below, for its own bounds |
| `tests/real_pty.rs` | ~1 plain `.join()` (thread handle, separate from its own `wait_bounded` for the `Child`) | - | |
| `tests/accept_backoff.rs` | 1 plain `.join()` | **yes** - `wait_bounded` (`try_wait` + `sleep(10ms)` loop) | duplicate of the pattern below |
| `tests/fixtures/accept_backoff_attempt.rs` | 1 `.join()` | - | a fixture *binary*, not a `#[test]` - flag for the engineer: in scope or not? |
| `src/idle.rs` (`#[cfg(test)]`) | 2 plain `.join()` | - | |
| `src/outbound.rs` (`#[cfg(test)]`) | ~5 plain `writer.join().unwrap()` | - | two run after `close()` (deterministic in the ordinary case, still needs the bound per AC #1's literal "every wait"); the other three already sit beside a `ClosingWriterGuard` whose own `Drop` does a *second*, independent bounded join - the explicit mid-test join is the one still unbounded today and must be wrapped itself, not left to rely on the guard's drop-time join as the only backstop |
| `crates/steg/tests/server_start.rs` | 1 plain `.join()`; **own** `wait_bounded` sleep-poll | **yes** | also the `unique_socket_path`/`unique_output_path` collision, class (e) |
| `crates/steg/tests/idle_stop.rs` | **own** `wait_bounded` sleep-poll | **yes** | |
| `crates/steg/tests/cli_exit_statuses.rs` | **own** `wait_bounded` sleep-poll | **yes** | |
| `crates/steg/tests/login_environment.rs` | **own** `wait_bounded` sleep-poll | **yes** | |
| `crates/steg/tests/cli_connection_arbitration.rs`, `cli_api_call.rs`, `cli_no_server.rs` | - | - | already bounded via `recv_timeout`/explicit reads; own `unique_socket_path(label)` duplicate, class (e) |

Unit-test `#[cfg(test)]` blocks elsewhere in `crates/server/src/*.rs` (pane.rs,
connection.rs, listen.rs, runtime/*.rs) already use `recv_timeout` with a named
constant (`EVENT_TIMEOUT`, `CWD_LOOKUP_TIMEOUT`) almost everywhere - these are the
"bounded but mute" class, not unbounded; a handful of plain `.join()`/`.wait()`
remain and the AST check finds them precisely.

`crates/sdk`: AC #1 names only `crates/server` and `crates/steg`. Proposal: leave
`crates/sdk`'s tests out of this task's scope entirely, with one named exception -
`local_socket.rs`'s `CooperativeHeldOpen` fixture (~line 586, the 100ms sleep) is the
one sdk item the notes explicitly ask about. It is a timing margin, not a bound on a
wait (nothing times out if it's wrong; it would instead occasionally race and the
test would pass for the wrong reason - the opposite failure mode from the other four
classes). Propose fixing it as a one-file, size-1 follow-up task (have the listener
thread hold the stream until the test sends a release signal, replacing the sleep)
rather than folding it into this one, since it needs no shared helper and touches a
crate this task is not scoped to.

## 2. The five sighting classes

**(a) attach.rs flood-test starvation across worktrees.** Read `FLOOD_TEST_LOCK`
(in-process `Mutex`, serialises only within one binary) and the five named tests'
bound comments directly: this file already reasons carefully about three *different*
bound widths for three different proofs - 1-2s for "known-stuck"/"must not be stuck"
proofs (tight on purpose, nothing here should ever wait that long if healthy), and 15s
for "eventually drains" liveness proofs (explicitly margined for `just check`'s own
concurrent CPU contention). `call_and_read_reply` (~line 149) is the one helper still
hard-coded to a flat 5s with no such margin, and every fixture-setup call
(`new_pane_running`, `attach`) goes through it - so under heavy cross-worktree flood
contention, *fixture setup itself*, not just the flood proof, can miss its bound.
Decision: three changes together, because each closes a different failure mode -
  1. Raise `call_and_read_reply`'s bound from a flat 5s to the same 15s "answers at
     all" margin the rest of this file already uses for liveness, since it is used
     for ordinary setup calls that have nothing to do with the flood itself.
  2. A **machine-wide advisory lock**, taken only by the five flood tests, via a real
     OS `flock` (not a sentinel file + manual cleanup) on a fixed, shared path
     (`std::env::temp_dir().join("steg-attach-flood-test.lock")` - deliberately the
     *same* path across every worktree, since the point is to serialise across them,
     unlike every other path in this file which is deliberately unique per test).
     `flock` is the right primitive here specifically because the kernel releases it
     the instant the holding process's last fd reference closes, *including* on a
     `SIGKILL` - exactly the shape `tools/src/steg_tools/quiet.py`'s own
     `held_slot`/`_race_all_slots` already uses for `STEG_CHECK_SLOTS` (TASK-1.132),
     and the reason this needs no stale-lock detection or manual cleanup: a binary
     killed mid-flood by `wait_child_bounded`'s own watchdog, or by CI's own job
     timeout, drops the lock the moment the OS tears down its file table, with no
     code path required to notice. The *acquisition* itself gets a bound too (a
     watchdog thread + `recv_timeout`, the same `run_bounded` shape, not a bare
     blocking `flock`) so a wait for the lock itself fails the test with a message
     naming what it waited for, rather than silently becoming the next unbounded
     wait this task was meant to remove. Lives beside `FLOOD_TEST_LOCK` in
     `attach.rs` (not `tests/support/mod.rs` - nothing else in this crate's tests
     needs a cross-process lock) as `cross_process_flood_guard()`.
  3. Leave `FLOOD_TEST_LOCK` itself as is - it still earns its keep for two flood
     tests racing *within* one binary, which the cross-process lock does not cover.

  **Proof, not "it passed": before the fix, start two copies of
  `an_unresponsive_panes_own_commands_do_not_stall_the_connection_or_another_pane`
  concurrently (`cargo test --features ... -- --exact` run twice in parallel shells,
  or the compiled test binary invoked twice directly) and show at least one fails
  its bound in a tight loop (reproducing the sighting, not assuming it); after the
  fix, run the same two-at-once invocation in a loop (order of tens of runs) and show
  it stays green, plus one run with the lock file pre-held by a third, separate
  process to prove the *acquisition* bound itself fires with a named message.

**(b) `login_env`'s backgrounded-grandchild test.** Read
`capture_login_environment_kills_a_backgrounded_grandchild_that_held_the_pipe_open`
directly: it already widened its *functional* timeout once, deliberately, from 200ms
to 1s (own comment: "a tighter bound flaked under a concurrently busy machine") - that
part is not the gap. The gap is the *outer* wall-clock assertion,
`started.elapsed() < Duration::from_secs(5)`, which proves "`capture_login_environment`
itself didn't hang" but has no contention margin at all beyond the 1s functional
timeout plus 4s of slack for signalling and reaping the grandchild under real load -
the exact same "liveness bound with no margin for this machine's own CPU contention"
shape as class (a), just in a different file. Decision: widen the 5s ceiling to
match this file's own stated reasoning for the 1s functional timeout (generous,
proving "returns at all," not timing precision) - propose 15s, the same number
`attach.rs` already settled on for the analogous proof, rather than inventing a new
constant. The two inner polling loops (pid-file appears, then process actually dies)
are a cross-process liveness check with no in-process event - same exception class as
`race.rs`'s own `wait_until_accepting` (left as is by TASK-1.113's own plan) - named
in criterion 3's list, not changed.

  **Proof**: reproduce first - run this single test alone in a tight loop under an
  artificially throttled machine (`cpulimit`/background `yes >/dev/null &` load, or
  simply running several other full `cargo test` invocations concurrently as the
  sighting itself did) until it is seen to fail the 5s assertion; then the same loop
  with the widened bound, green.

**(c) "a terminal-scrolling timing test."** Searched by keyword (`scroll`) across
`crates/server/{tests,src}` and `crates/steg/tests`: no test name or comment matches
"scroll" combined with a timing/bound assertion in either crate. `crates/kernel`
(ghostty-vt wrapper) has scrollback-counting tests but none wait on a thread, channel,
socket or child - pure logic, no timing at all, and outside this task's named scope
either way. **This sighting's exact location could not be confirmed from the notes
alone.** Rather than guess which test it is (and risk "fixing" the wrong one), this
plan proposes: ask @bruk.o, who gathered the sighting, to name the test before build
starts; if it is in `crates/kernel` or `crates/ui` it is out of this task's stated
scope (server + steg) and gets named as a follow-up instead of folded in here.

**(d) `signal::tests::send_reaches_the_whole_process_group_not_only_the_leader`.**
Read the test directly, together with its own `wait_within` helper (already a
bounded, watchdog-thread join - not the gap), `process_exists` (a `kill -0` probe),
and `KillOnDrop`'s own `killpg` at drop (`signal.rs:160-179`). Two candidate
mechanisms, both consistent with "failed on CI alone, not from cross-worktree load":
(1) `kill -0` on a pid reports success for a *zombie* too (the process table entry
still exists until something reaps it); once the test's own direct child (`sh`) is
killed, the backgrounded `sleep` is reparented to this machine's own init/subreaper,
which reaps it *asynchronously*, on its own schedule - the test's single
`process_exists(background_pid)` check, taken immediately after `wait_within`
confirms `sh` has exited, assumes that reap already happened. (2) `send(pid,
SIGKILL)` (a `killpg` to the whole group, since `process_group(0)` makes `sh`'s own
pgid equal its pid) could race the shell's own `wait` builtin and the kernel's own
delivery to a child still mid-fork/mid-reparent, landing the SIGKILL before the
kernel has finished attaching `sleep` to the group the way the ordinary case assumes.
Both are the same *shape* as the `CLOEXEC` class TASK-1.113 found - an assumption
that something elsewhere (a reap, a reparent, a group attachment) completes
synchronously with this test's own next line, true on a quiet machine and not
guaranteed under CI's own scheduler latency, no cross-worktree load needed to expose
either. Not committing to one over the other without a way to force the exact
window on demand (named as a limitation, below); the fix is correct under either
cause: replace the single `assert!(!process_exists(...))` with a bounded *poll* (not
a bare recheck) - the same cross-process-liveness exception class as (b)'s own inner
loops and `race.rs`'s `wait_until_accepting` - up to a few seconds, asserting the
process is gone *by* the deadline rather than *at* one instant.

  **Proof**: reproduce first by inserting an artificial delay (a short sleep) between
  `wait_within` returning and the `process_exists` check in a scratch copy, confirming
  the assertion can be made to fail this way (demonstrating a window of this general
  shape is real, not theoretical, without claiming it is mechanism (1) rather than
  (2)); then the bounded-poll version, shown to tolerate the same artificial delay.
  This does not reproduce the *exact* CI timing (that would need the real machine's
  own scheduler latency, not reproducible on demand, and does not distinguish which
  of the two candidate mechanisms actually fired) - stated as a limitation, not
  claimed as "the same failure" or "the confirmed cause."

**(e) `crates/steg`'s "unique" socket/output paths collide across processes.**
Confirmed the helper's actual shape in all seven files
(`unique_socket_path`/`unique_socket_path(label)`/`unique_output_path`): every one is
`std::env::temp_dir().join(format!("{pid}-...-{suffix}.sock", std::process::id()))` -
unique *within* one process (a per-process-static `AtomicU64` counter), placed in the
one shared, un-partitioned system temp directory every process on the machine uses.
Two ways this collides, both consistent with the sighting (a plain file write failing
because a *socket* already sat at the computed path): OS pids are reused once a
process exits, and under this machine's own churn of many short-lived `cargo test`
binaries across several worktrees, a freshly-started process can be assigned a pid an
*earlier, already-exited* test binary used, and - since the per-process counter always
starts at 0 - compute the exact same path that earlier run's own, uncleaned-up socket
file is still sitting at (several of these seven files never remove their socket path
at all; `server_start.rs`'s own test is one of the few that does, and only for
itself). A fresh, per-test directory removes the class outright, on both counts: a
freshly created directory cannot already contain a leftover file from an unrelated
run, and it does not depend on pid/counter uniqueness holding across process
lifetimes at all. Decision: add one new `crates/steg/tests/support/mod.rs` (the same
"duplicated, not shared from `src/`, because each `tests/*.rs` file is its own
compilation unit" reasoning `crates/server/tests/support/mod.rs` already states) with
a `fn scoped_socket_path(label: &str) -> (tempfile::TempDir, PathBuf)` using the
`tempfile` crate. Checked `Cargo.lock` directly (the first check, each crate's own
`Cargo.toml` alone, was the wrong check - a dev-dependency that's already resolved
transitively does not show up there): `tempfile 3.27.0` is already resolved
workspace-wide, just not yet a *direct* dependency of `crates/steg` - adding it to
that crate's own `[dev-dependencies]` pulls in no new third-party code and needs no
fresh licence check, only a one-line `Cargo.toml` addition (unlike TASK-1.94's
`ast-grep-py`, which really was tools/'s first-ever runtime dependency - naming that
precedent here would overstate this one). The returned `TempDir` is kept alive by
the caller for the test's own duration - removed on `Drop`, including on a
panicking unwind, which also closes the long-standing "most of these seven files
never clean up their socket path" gap as a side effect, not a separately scoped
fix. All seven files' own `unique_socket_path`/
`unique_output_path` are replaced by calls into this one module. The analogous
`crates/server/tests/attach.rs::unique_socket_path` (pid+counter, same shape) is
**not** touched here - no cross-worktree collision was sighted against it, and
widening this task beyond its two named crates on a hunch, rather than a sighting,
is exactly the "silently widen the scope" TASK-1.113 was asked not to do; named as a
candidate for the same fix in a follow-up if it is ever sighted.

  **Proof**: reproduce by forcing a pid collision deterministically is not practical
  (pid reuse is the OS's own choice) - instead, reproduce the *mechanism*: run the
  affected test with the temp-dir cleanup commented out, confirm a second run shortly
  after leaves a stale socket at a path the next run's own counter can recompute
  (same pid range, same counter value), then show the per-test-`TempDir` version
  cannot recompute any previous run's path by construction (a fresh random directory
  name each time), which is the structural guarantee replacing the probabilistic one.

## 3. The check for criterion #2

Lives in `tools/src/steg_tools/bound_waits.py`, a new module (`just check-bound-waits`
→ `steg-bound-waits` console script, added to `check-recipes`'s own list and to the
`check` recipe in `justfile`), mirroring `ui_levels.py`/`ui_colours.py`'s own shape
exactly: `rust_source.rust_files(root, under)` (shared, already exists) to read every
`.rs` file under `crates/server` and `crates/steg`, then `ast_grep_py.SgRoot(text,
"rust")` - a real tree-sitter parse, not text matching - to find:
- a `call_expression` whose callee is a `field_expression` named `join`, `recv`,
  `wait`, or `read`/`read_to_end`/`read_exact` **only** when the enclosing function is
  itself inside a `mod tests` attributed `#[cfg(test)]` (for `src/*.rs`) or the file's
  own path is under a crate's `tests/` directory (for integration tests) - this is
  exactly the distinction a regex cannot make and the manual sweep in part 1 got
  wrong on `listen.rs`/`pane.rs`/`connection.rs`/`runtime/close.rs`.
- excluding a call already wrapped by (its ancestor call expression's callee resolves
  to) `run_bounded`, `wait_child_bounded`, or a call on a value whose type is known
  to have had `set_read_timeout`/`recv_timeout`/`try_recv` used on it earlier in the
  same block - this last part is the hard case `ui_levels.py`'s own docstring names a
  precedent for (it states outright what it *cannot* see, e.g. a macro's opaque
  token-tree argument): a fully precise "was this stream ever given a timeout"
  requires real dataflow this check will not have. Proposed scope: match only the
  *syntactic* shape (`.read(`/`.join()`/`.recv()`/`.wait()` with no `_timeout` in the
  method name, not preceded in the same block by a sibling `set_read_timeout`/
  `recv_timeout` call on the same identifier) - named explicitly as a heuristic, with
  its own false-positive risk stated in the module's docstring the way `ui_levels.py`
  states its macro-argument gap, rather than claimed as exact.
- a deliberate exception is a `// bound-waits: allow - <reason>` line comment
  immediately above the call (the same shape as `#[allow(..., reason = "...")]`
  clippy already requires in this codebase, `crates/server/tests/support/mod.rs:29`)
  - reviewed the same way any other suppressed lint is, by showing up in the diff.

Its own tests: `tools/tests/unit/test_bound_waits.py` (small Rust-source fixtures as
strings, one per shape: a bare `.join()` inside `#[cfg(test)]` → violation; the same
call wrapped in `run_bounded` → clean; `.join()` in production code outside
`#[cfg(test)]` → clean, proving the false-positive class the manual sweep hit is
actually excluded; a `// bound-waits: allow` line → clean, with the suppression
itself reported in a `--list-exceptions` mode so they stay visible, not silently
accumulating). Cost in `just check`: one more `uv run --quiet --project tools
steg-bound-waits` invocation, parsing at most a few hundred already-small files with
`ast-grep-py` (already proven cheap enough for `ui_levels`/`ui_colours` on the whole
of `crates/ui`) - sub-second, no new slow dependency (no `tempfile` needed here;
that's part 2(e)'s own crate, not tools/).

**Order relative to part (ii)'s mechanical fixes**: write and run the checker (not yet
wired into `just check`) *before* the mechanical sweep, so its own violation list - not
a hand-maintained one in this plan - is the authoritative inventory the build phase
works through; wire it into `check-recipes`'s list only once it is clean, matching the
engineer's own "last, once the tree is clean enough to pass it" ordering for when it
starts *gating*, while still building it first so manual grepping does not have to be
trusted for completeness.

## 4. Criterion #3 - sleep-in-a-loop inventory

**Replaced, with an event (the same fix in five files at once):** `accept_backoff.rs`,
`steg/tests/server_start.rs`, `steg/tests/idle_stop.rs`, `steg/tests/
cli_exit_statuses.rs`, `steg/tests/login_environment.rs` each define their own,
near-identical `wait_bounded(child, timeout)` that polls `child.try_wait()` in a loop
with a 10-20ms `sleep` between tries. This is a sleep-poll for an event that already
has a real one available: a watchdog thread doing the real, blocking `child.wait()`
and the caller doing a bounded `recv_timeout` on a channel - exactly
`crates/server/tests/support/mod.rs::wait_child_bounded`'s own shape, already built
and proven by TASK-1.113. Consolidate: one new `crates/steg/tests/support/mod.rs`
(mirroring the server crate's own module, for the same "can't share across
compilation units" reason) holding `wait_child_bounded`, and all four `steg/tests/*.rs`
sites switch to it; `accept_backoff.rs` switches to the existing
`crates/server/tests/support/mod.rs` one. Five duplicated sleep-polls become zero.

**Left as a poll, named with the reason (no event exists):**
- `crates/server/tests/attach.rs::start_server`'s connect-retry loop (and the
  identical shape in `crates/steg/tests/*.rs` fixtures that wait for a socket to
  start accepting) - polls an external process's own readiness with no in-process
  event to subscribe to; already has an explicit deadline.
- `crates/server/tests/race.rs::wait_until_accepting` - TASK-1.113's own, already-
  recorded exception, same reasoning.
- `login_env.rs`'s two inner loops (pid-file appears; process actually dies) - class
  (b), above; cross-process liveness, no event.
- `signal.rs`'s fix in class (d) - a new poll, for the same reason, replacing a
  single zero-slack check rather than adding a sleep where none existed.
- `accept_backoff.rs:180,262`'s own backoff-observation loops (`Ok(None) =>
  sleep(10ms)`, `None => sleep(20ms)`) - need confirming by reading in the build
  phase whether these observe the *product's* own backoff timer (an external,
  real-time condition, same exception class) or something with an in-process signal
  available; flagged here rather than asserted either way, since this plan did not
  read `accept_backoff.rs` in full.

Every other `thread::sleep` found by the sweep (`handshake.rs`'s per-byte-delay
trickle, `routing.rs`'s fixed small delays between unrelated setup steps,
`runtime.rs`'s `POLL_INTERVAL`) is not a *wait-for-an-event* sleep at all - it is
deliberately pacing a fixture's own behaviour (trickling bytes slowly on purpose,
simulating a slow client) - out of criterion #3's scope, to be confirmed file-by-file
during the build and listed in the task's notes per the acceptance criterion's own
wording ("each remaining poll is listed... with the reason").

## 5. Criterion #4 - the rule line

Verified against both files' own opening lines (not assumed): `.claude/rules/
testing-rust.md`'s own header says it holds "the techniques for the team's testing
policy"; `plugins/bruk-io-steg/rules/testing-rust.md`'s own header says it is "how to
run them" (recipe-level: `just test`/`just check`/`.quiet/` logs). The macOS
dropped-socket fact is a *technique* (what a test must never assume about the
platform), so it belongs in `.claude/rules/testing-rust.md`, as a new bullet beside
the existing "no test sleeps to wait for something; it waits on the event" line.
`.claude/rules/` sits in the repository's own root, not under `plugins/`, so neither
the plugin route (asdlc-plugin) nor team rules' pull-request-only route applies - a
worker may edit this file directly, in the same commit as the rest of this task's
build, with no separate approval step. Proposed sentence: "A test must not rely on a
dropped socket end making its peer's very next read or write fail at once: on macOS,
`UnixStream::pair()`/`UnixListener::accept()` cannot set `CLOEXEC` atomically (the
std library's own `sys/net/connection/socket/unix.rs`), so a `fork` elsewhere in the
same test binary can inherit the dropped end before `CLOEXEC` lands, keeping the peer
open and the expected failure from happening (TASK-1.113)."

## 6. Size and split

Agree with splitting by default, with one correction to the engineer's own framing:
TASK-1.113's own precedent was not "build nothing and split into children" - it
built its own narrowed scope (`outbound.rs`, `race.rs`, `openpty_race.rs`), recorded
checks, shipped PR #69, and spun the *remainder* out as this task, which still
closed with delivered code. Proposing the same shape here: **this task
(TASK-1.123) keeps and delivers part (i) itself** - it is already claimed, sized,
and approved as size-3, and part (i) alone is a reasonable size-3 - and parts (ii)
and (iii) become two **new** draft tasks this task's own notes link, for the
engineer to accept separately, rather than TASK-1.123 dissolving into three children
with nothing shipped under its own number.

One sequencing note that does not change the split itself: TASK-1.88 (`server
show`/`server stop`/client methods, status In Progress, `@bruk.w6`) and TASK-1.90
(pane send/capture/wait, status In Progress, `@bruk.w7`) are *already being built*,
not merely planned, in `crates/server/tests/{shutdown,routing,attach,program}.rs`
and `crates/server/src/{pane.rs,runtime/*.rs}` - exactly the files part (ii) (below)
touches most heavily; part (i), kept here, touches `attach.rs` (class a) and
`login_env.rs`/`signal.rs` (classes b, d) only - `attach.rs`'s own overlap with
TASK-1.90 is real and worth flagging to the orchestrator before this task's own
build starts, but it is one file, not the wide sweep part (ii) would hit.

- **(i) Cross-worktree clashes and the tightest bounds - this task, TASK-1.123,
  narrowed.** Classes (a), (b), (d) and (e) above, plus `call_and_read_reply`'s
  bound. Highest value first: these are costing every worker failed checks *today*,
  independent of the rest of the sweep, and (e)'s fix is a prerequisite for safely
  running `crates/steg`'s test suite concurrently with anything else on this machine
  while the rest of the sweep is still in progress. Criteria: AC #3 only for the
  cross-process polls these classes themselves introduce or replace (the five
  duplicated sleep-polls are part (ii)'s own item, not here); a new, narrower AC #1
  covering classes (a)/(b)/(d)/(e) by name, each with its own reproduce-first-then-
  fix proof (above); no AC #2 claim (the checker is part (iii)). Proposed new AC #4
  text for this task, replacing the current one: "Classes (a), (b), (d) and (e) from
  this task's own notes are each fixed, with a reproduce-first-then-fix control
  recorded for each, and the follow-up tasks for the rest of the sweep (ii, iii) and
  `crates/sdk`'s one named exception are linked."
- **(ii) The mechanical sweep, file by file** (new draft task) - every remaining
  unbounded `.join()`/`.recv()`/`child.wait()`/unbounded socket read gets a helper,
  plus the five duplicated sleep-polls (criterion #3's main item) and `crates/sdk`'s
  one named exception as its own, separate tiny follow-up (not folded into this
  one either). Sequenced to touch `events.rs`, `handshake.rs`, `idle_stop.rs`,
  `runtime.rs`, `src/idle.rs`, `src/outbound.rs` first (TASK-1.88/1.90 do not touch
  these); `shutdown.rs`, `routing.rs`, `attach.rs`, `program.rs`, `pane.rs`,
  `runtime/*.rs` last, re-checked against `origin/main` immediately before each of
  those files' own commit, once TASK-1.88/1.90 have landed or the orchestrator
  confirms the specific lines are clear. Criteria: AC #1 (literal, narrowed to the
  two named crates per part 1's interpretation), the remainder of AC #3's
  listed-poll requirement.
- **(iii) The tools check** (new draft task) - part 3 above, built and run
  standalone first (its own violation list verified against part (ii)'s now-clean
  tree), then wired into `check-recipes`/`justfile`'s `check` list last. Criteria:
  AC #2 in full, including its own tests. Depends on (ii) merging first (its own
  violation list needs a clean tree to report zero against).

Both (ii) and (iii) are drafted with `-m m-3`, the same parent, and linked from
TASK-1.123's notes as the split, for the engineer to accept once this task's own
plan is approved.

## 7. Proof plan and what will not be claimed

Every class above states its own reproduce-first-then-fix control in part 2; part
3's checker gets unit tests proving both the true-positive and the false-positive
cases the manual sweep actually hit. Record checks via `asdlc record-check`: for
part (i), the specific test files touched (`cargo test -p steg-server --test attach`,
`cargo test -p steg-server --lib login_env::`, `cargo test -p steg-server --lib
signal::`, `cargo test -p steg --test server_start` and the other six `crates/steg`
files), each with its own before/after pair; for part (ii), `just test` (affected)
plus a final `just test-all`; for part (iii), `just test-tools` plus `just check`
once wired in. **Not claimed**: that no flake remains in these files (the brief's own
instruction); that `crates/sdk`'s tests are flake-free (out of scope, one exception
named); that sighting (c)'s test has been found and fixed (named as unresolved,
not guessed at); that the AST check's "already wrapped" exclusion is dataflow-precise
(named as a heuristic with a stated false-positive risk, not exact).

## Open questions for the engineer

1. Confirm the "unbounded vs bounded-but-mute" split of AC #1 (part 1) - the literal
   reading would rewrite hundreds of already-working `recv_timeout` calls.
2. Confirm `crates/sdk` stays out of scope, with the one named `local_socket.rs`
   fixture as its own follow-up task.
3. Confirm whether `crates/server/tests/fixtures/accept_backoff_attempt.rs` (a
   fixture *binary*, not a `#[test]`) is in AC #1's scope.
4. Sighting (c)'s test could not be located - need @bruk.o to name it before build
   starts, or confirm it is accepted as unresolved in this task's notes.
5. Confirm the split itself: this task keeps and delivers part (i) under its own
   number (a corrected AC #4, per part 6), with parts (ii) and (iii) as two new
   draft tasks, rather than TASK-1.123 dissolving into three children with nothing
   shipped under this number - and confirm the sequencing against TASK-1.88/1.90's
   own active edits (part 6).
6. `tempfile` as a new *direct* `dev-dependency` for `crates/steg` (part 2(e); already
   resolved transitively in `Cargo.lock`, so this is a one-line `Cargo.toml` addition,
   not a new third-party dependency) - confirm, or name a preferred alternative.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Sightings gathered by @bruk.o on 2026-11-18, while six workers built on one machine (load average 4 to 19). None is in a file the affected tasks touched; main's CI was green throughout. (1) crates/server/tests/attach.rs, the stuck-pane and bound tests: an_unresponsive_panes_own_commands_do_not_stall_the_connection_or_another_pane (most often; fails with 'must answer within a bound', or a read timing out at the helper call_and_read_reply's own five-second socket timeout near line 149), a_second_connections_control_call_is_not_blocked_by_a_stuck_panes_lock_held_attach, pane_close_force_returns_within_its_bound_even_while_the_panes_own_worker_is_stuck, stopping_the_server_returns_within_its_bound_even_while_a_panes_worker_is_stuck, a_pane_that_closes_between_a_queued_attach_and_its_arm_leaves_nothing_to_detach. Seen by the workers of TASK-1.119, 1.130 and 1.135 across about fifteen runs of just check, a different subset each time. Run alone the first test failed six times in a row for the orchestrator (about five seconds each) while another worktree's attach test binary was running at 200 percent CPU, then passed once that had stopped: these tests flood a pane's input, and two such binaries at once starve each other even on a machine with idle cores. The in-process FLOOD_TEST_LOCK serialises them inside one binary only. (2) login_env's backgrounded-grandchild test, under load, many sightings. (3) a terminal-scrolling timing test, once on CI's first run for TASK-1.128. (4) signal::tests::send_reaches_the_whole_process_group_not_only_the_leader, once on CI's first run for TASK-1.132, green on rerun; CI runs alone, so this one is not load from other workers. For the plan: liveness bounds here prove 'answers at all', so they can be generous; the helper's default five-second read timeout is the tightest bound in the file; and a machine-wide guard for the flood tests may be worth more than widening each bound. TASK-1.132 limits concurrent cargo steps to two, which reduces but does not remove two attach binaries overlapping.

Fifth sighting class, 2026-11-18 (TASK-1.135's worker, load about 3): crates/steg/tests/server_start.rs::an_untrusted_existing_file_refuses_with_status_4_and_names_the_reason failed writing a plain file because a socket already existed at the same temporary path - a path collision between two worktrees running crates/steg's tests at once (TASK-1.119's worker was running them). Passed alone and as the whole file. The helper that makes 'unique' socket paths is duplicated across this crate's test files and is evidently not unique across processes; a per-test temporary directory would remove the class. An earlier sdk local_socket path collision is logged on TASK-1.82.

From TASK-1.119's third review (2026-11-18), two test nits left because the task locked on passing: crates/sdk/src/local_socket.rs's cooperative test fixture (Behaviour::CooperativeHeldOpen, near line 586) sleeps 100 ms so the peer does not close before the client clears its timeouts - it passed 300 of 300 runs at load 3, but it is a timing margin, not a synchronised event; have the listener thread hold the stream until the test signals it instead. And a_hello_sent_then_closed_immediately_never_reports_closed_before_hello loosened its attempt-count assertion from exactly 1 to at least 1 without needing to.
<!-- SECTION:NOTES:END -->
