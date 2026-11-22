---
id: TASK-1.150
title: >-
  Find why a freshly autostarted server closes a client's first connection
  before the hello
status: In Progress
assignee:
  - '@bruk.w5'
created_date: '2026-11-22T04:13:00Z'
updated_date: '2026-11-22T05:06:00Z'
labels:
  - size-3
milestone: m-3
dependencies: []
parent_task_id: TASK-1
project: bruk-io-steg
asdlc_accepted_from: DRAFT-2
asdlc_branch: task-1.150-find-why-a-freshly-autostarted-server
asdlc_plan_approved_by: bruk
asdlc_plan_approved_hash: '39ca1e575b3d'
asdlc_plan_approved_size: size-3
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found by TASK-1.137's planner (22 November 2026). The test has failed twice in CI in one day (steg PR 91's CI, and main at 4ccc8344). Both times the client reported that the server accepted the connection and closed it before a hello arrived, twice: both handshake attempts met end-of-file, not a timeout. Every wait on the path already has a bound, so this is not a test-bounds problem. It reads as a product question: either the client's retry met the test's hostile listener both times, or the real, just-started server closed its first connection early. Unknowns dominate, so start by reproducing (repeat the one test) and reading the autostart and arbitration paths in crates/sdk/src/local_socket.rs and crates/server/src/arbiter.rs and listen.rs. Never paste a failing run's output or any environment text onto the board.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The cause of ClosedBeforeHello on both handshake attempts against a real autostarted server is named, from the code and a reproduction or from evidence that rules candidates out
- [ ] #2 Either the fix is made with a test that fails without it, or a follow-up task states the fix and why it was not made here
- [ ] #3 crates/steg/tests/cli_connection_arbitration.rs a_hostile_first_connection_does_not_stop_a_real_autostart_from_succeeding passes in a recorded run of many repetitions
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
Cause (AC #1): H1 confirmed, H2 unsupported. This is a test-fixture race, not a product fault.

Structural proof (not timing-dependent): `local_socket.rs::connect` (lines 159-183) only reaches `command.spawn()` on the branch where the raw `UnixStream::connect(&path)` returns `Err`. In every failing run observed (the 1/150 local baseline repeat and 20/20 reproductions below), both of `connect_handshaken`'s two `one_handshake_attempt` calls had their inner `connect()` return `Ok(stream)` - so by construction, autostart's spawn was never reached on either attempt. The retry's second raw connect landed back on the hostile fixture's own listener, not on a real server.

Mechanism: `spawn_hostile_listener` (cli_connection_arbitration.rs:50-61) does `accept()` once, `drop(stream)`, then `drop(listener)` - two drops on the fixture's thread with no synchronisation against the client. A Unix-domain `connect()` completes at the kernel level into the listen backlog without needing a matching `accept()`, so the client's own second `connect()` (made the instant its first attempt sees EOF) can land in that backlog in the gap between the two drops, before the listener is actually closed. That queued connection is then torn down when `drop(listener)` runs, giving the client the identical EOF-before-hello signature as a second hostile accept, with no real server ever involved.

Evidence gathered this phase (worktree reverted clean after, `git status` confirmed):
- Baseline: the unmodified fixture, repeated 150 times locally (`cargo test -p steg --test cli_connection_arbitration`, scratch loop, reverted), reproduced the CI failure once (1/150, ~0.7%) - consistent with "twice in one CI day" for an intermittent race, and both `connect()`s in that failure returned `Ok` per the structural argument above.
- Widened-gap control: a scratch copy of `spawn_hostile_listener` with an 80ms sleep inserted between `drop(stream)` and `drop(listener)` (widening exactly the gap the mechanism needs) reproduced the same failure 20/20 runs. An `lsof -t <lock path>` check after each run found no server process in every failure - corroborating, not the primary proof, since it only confirms "no server was left running," not "autostart was never reached."
- Code read, no mechanism found for H2: traced the real autostart path (`arbiter::attempt`, `listen::run`, `idle.rs`) for anything that could close a freshly accepted first connection before its hello. The only timed teardown is the on-demand idle-stop checker, default 30s (`DEFAULT_IDLE_TIMEOUT`), against a test completing in well under a second - not a plausible collision. No other close-before-hello path exists in a fresh accept loop.

Fix (files: crates/steg/tests/cli_connection_arbitration.rs only - test fixture, not product code):
1. In `spawn_hostile_listener`, swap the two drops: `drop(listener)` immediately after `accept()` returns, then `drop(stream)`. This closes the listening socket before the hostile stream is dropped - the drop that produces the client's EOF - so the client's EOF-triggered retry's `connect()` happens causally after the listener is already closed (closing precedes the EOF by construction, not by timing). No sleep, no new flakiness risk: this is the same ordering discipline `listen.rs` lines 536-544 already applies in the real server's own shutdown path, whose comment states the identical reasoning (closing the listener before any slower work avoids leaving exactly this backlog window open) - production already does this; the fixture didn't.
2. Rewrite `spawn_hostile_listener`'s doc comment (lines 40-49): it currently describes and justifies the old order ("accepts... drops it immediately... then drops the listener itself"). Update it to describe the new order and why (cite the mechanism and `listen.rs`'s own precedent).
3. Check `the_hostile_listener_itself_accepts_and_closes_with_nothing_written` (line 118), which shares this fixture: under the reorder, its own `connect()` can still land in the backlog before `accept()` picks it up, and the accepted stream still sees a clean EOF once the listener later closes (the same property `listen.rs` 536-543 states for the product) - expected to keep passing unchanged. Confirm this in the build phase rather than assuming it; note the result either way.

Proof (AC #2 and #3):
- AC #2: the fix above *is* the fix, made in this task (not deferred) - it is a fixture change, not a product change, so say that plainly rather than leaving AC #2 looking unaddressed. Add one new permanent test, e.g. `closing_the_hostile_listener_before_the_stream_leaves_no_window_for_a_late_connect`: build a variant fixture that inserts an explicit, generous delay (e.g. 200ms) at the exact point the race lived - between `drop(stream)` and `drop(listener)` for the pre-fix order, between `drop(listener)` and `drop(stream)` for the fixed order - and assert the real CLI (`run_steg`) still exits 0 and prints `[]`. This is deterministic in both directions (not a sleep-based flake): run against today's ordering it must fail near-certainly (matches the 20/20 control above); run against the fixed ordering it must pass, because by the time the delay starts the listener is already gone. This one test is both "a test that fails without the fix" (AC #2) and a permanent regression guard - built in the implementation step, not carried over from this phase's throwaway scratch tests (which used a raw-connect probe during the delay instead of the real CLI; the committed test should assert on product behaviour, i.e. the real CLI's exit status, not on fixture internals).
- AC #3: after the fix, run the real test `a_hostile_first_connection_does_not_stop_a_real_autostart_from_succeeding` many times as one recorded command, e.g.:
  `sh -c 'for i in $(seq 1 300); do cargo test -p steg --test cli_connection_arbitration -- a_hostile_first_connection_does_not_stop_a_real_autostart_from_succeeding --exact || exit 1; done'`
  run directly under `asdlc record-check`, with `ZIG` set from `mise which zig` first. Be honest about what this shows: at the measured 1/150 base rate, 300 reps only has ~87% power to catch a bug at that same rate if it somehow survived - this run is satisfying AC #3's literal wording and giving real additional confidence, not the primary proof that the fix works. The primary proof is the causal argument plus the deterministic new test above.

Question for the engineer, with a recommended default: could a real server's own narrow shutdown window produce the same double-`ClosedBeforeHello` against a genuine (not test-fixture) server, meaning `connect_handshaken`'s exactly-one-retry (local_socket.rs) deserves widening? Recommended default: no change. Reason: `listen.rs` lines 536-544 already closes its listener immediately after the accept loop stops, specifically to avoid leaving this backlog window open at all in production - the discipline the fixture lacked is already applied where it matters; there's no evidence this has ever happened against a real server, and widening the retry count would only shrink an already-rare race's probability further, not eliminate it.

Not splitting: this is one fixture reorder, one new deterministic test, and one recorded repetition run - a single size-3 change.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
From TASK-1.137's planner, 22 November 2026, after reading both failed CI logs (same panic both times, ConnectError::ClosedBeforeHello). Two candidates, neither proven. H1 (better supported, needs no new mechanism): the test's own fixture. spawn_hostile_listener accepts one connection, drops it, then drops the listener; while the listening socket is still open the kernel queues further connections in its backlog, so the client's retry can land there between the two drops and is discarded when the listener closes - the same end-of-file as the first attempt, with autostart never run and no real server involved. That would make this a test-fixture race, not a product fault. H2: a real autostarted server closed its first connection early, which needs a second, unconfirmed race with no other symptom in the suite. Neither log records whether the autostart spawn ran in the failing case. Start by instrumenting the fixture to tell the two apart (did the spawn run), before writing any fix.

Plan checked by @bruk.o, 22 November 2026: ok. It names the cause with a reproduction (1 failure in 150 unmodified runs; 20 of 20 with the gap widened; the autostart spawn is unreachable when both connects succeed), fixes only the test fixture by closing the listener before the stream so the close precedes the client's retry by cause and not by timing, and proves it with a test that fails without the reorder plus a 300-repetition run. One thing for review: the new test places a deliberate delay where the race lived; that is an adversary, not a wait on an event, and the notes must say so.

Check passed: `env ZIG=/Users/brukhabtu/.local/share/mise/installs/zig/0.17.1/bin/zig cargo test -p steg --test cli_connection_arbitration` exited 0 at 2026-11-22T05:05:08+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.150 on task-1.150-find-why-a-freshly-autostarted-server at 48b88c74cb52 with uncommitted changes:

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.10s
     Running tests/cli_connection_arbitration.rs (target/debug/deps/cli_connection_arbitration-dfa156c3f3528d90)

running 15 tests
test support::tests::candidate_name_does_not_collide_under_the_same_forced_pid_and_counter ... ok
test support::tests::the_old_pid_and_counter_only_scheme_collides_when_two_processes_share_a_pid ... ok
test support::tests::run_bounded_returns_the_bodys_value_when_it_finishes_in_time ... ok
test support::tests::a_fresh_scoped_temp_dir_is_0700 ... ok
test support::tests::scoped_temp_dir_exists_after_new_and_is_gone_after_drop ... ok
test support::tests::one_scoped_temp_dir_can_hand_out_two_different_paths ... ok
test support::tests::path_panics_rather_than_hand_back_something_over_the_sun_path_budget - should panic ... ok
test the_hostile_listener_itself_accepts_and_closes_with_nothing_written ... ok
test support::tests::two_scoped_temp_dirs_made_back_to_back_are_different ... ok
test support::tests::create_unique_dir_skips_a_name_that_already_exists ... ok
test support::tests::wait_for_socket_returns_once_something_is_listening ... ok
test support::tests::wait_for_socket_panics_when_nothing_ever_answers - should panic ... ok
test support::tests::run_bounded_panics_when_the_body_does_not_finish_in_time - should panic ... ok
test a_hostile_first_connection_does_not_stop_a_real_autostart_from_succeeding ... ok
test closing_the_hostile_listener_before_the_stream_leaves_no_window_for_a_late_connect ... ok

test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.63s
```

Check FAILED: `env ZIG=/Users/brukhabtu/.local/share/mise/installs/zig/0.17.1/bin/zig cargo test -p steg --test cli_connection_arbitration -- closing_the_hostile_listener_before_the_stream_leaves_no_window_for_a_late_connect --exact` exited 101 at 2026-11-22T05:05:32+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.150 on task-1.150-find-why-a-freshly-autostarted-server at 48b88c74cb52 with uncommitted changes:

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.10s
     Running tests/cli_connection_arbitration.rs (target/debug/deps/cli_connection_arbitration-dfa156c3f3528d90)

running 1 test
test closing_the_hostile_listener_before_the_stream_leaves_no_window_for_a_late_connect ... FAILED

failures:

---- closing_the_hostile_listener_before_the_stream_leaves_no_window_for_a_late_connect stdout ----

thread 'closing_the_hostile_listener_before_the_stream_leaves_no_window_for_a_late_connect' (75655228) panicked at crates/steg/tests/cli_connection_arbitration.rs:209:5:
assertion `left == right` failed: stderr: steg: the steg server accepted the connection and closed it before a hello arrived, twice

  left: Some(4)
 right: Some(0)
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    closing_the_hostile_listener_before_the_stream_leaves_no_window_for_a_late_connect

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 14 filtered out; finished in 0.48s

error: test failed, to rerun pass `-p steg --test cli_connection_arbitration`
```

Check passed: `env ZIG=/Users/brukhabtu/.local/share/mise/installs/zig/0.17.1/bin/zig cargo test -p steg --test cli_connection_arbitration` exited 0 at 2026-11-22T05:05:57+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.150 on task-1.150-find-why-a-freshly-autostarted-server at 48b88c74cb52 with uncommitted changes:

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.10s
     Running tests/cli_connection_arbitration.rs (target/debug/deps/cli_connection_arbitration-dfa156c3f3528d90)

running 15 tests
test support::tests::candidate_name_does_not_collide_under_the_same_forced_pid_and_counter ... ok
test support::tests::the_old_pid_and_counter_only_scheme_collides_when_two_processes_share_a_pid ... ok
test support::tests::run_bounded_returns_the_bodys_value_when_it_finishes_in_time ... ok
test support::tests::a_fresh_scoped_temp_dir_is_0700 ... ok
test support::tests::two_scoped_temp_dirs_made_back_to_back_are_different ... ok
test support::tests::path_panics_rather_than_hand_back_something_over_the_sun_path_budget - should panic ... ok
test support::tests::one_scoped_temp_dir_can_hand_out_two_different_paths ... ok
test support::tests::scoped_temp_dir_exists_after_new_and_is_gone_after_drop ... ok
test the_hostile_listener_itself_accepts_and_closes_with_nothing_written ... ok
test support::tests::create_unique_dir_skips_a_name_that_already_exists ... ok
test support::tests::wait_for_socket_returns_once_something_is_listening ... ok
test support::tests::wait_for_socket_panics_when_nothing_ever_answers - should panic ... ok
test support::tests::run_bounded_panics_when_the_body_does_not_finish_in_time - should panic ... ok
test a_hostile_first_connection_does_not_stop_a_real_autostart_from_succeeding ... ok
test closing_the_hostile_listener_before_the_stream_leaves_no_window_for_a_late_connect ... ok

test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.63s
```

Check FAILED: `/private/tmp/claude-501/-Users-brukhabtu-src-github-com-bruk-io-steg/a318fbf0-a659-4ad6-9bfc-86a79ba9cc76/scratchpad/TASK-1.150-repeat-run.sh /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.150` exited 1 at 2026-11-22T05:06:52+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.150 on task-1.150-find-why-a-freshly-autostarted-server at 48b88c74cb52 with uncommitted changes:

```text
TASK-1.150: failed at run 1 of 300
```
<!-- SECTION:NOTES:END -->

## Comments

<!-- COMMENTS:BEGIN -->
author: @bruk
created: 2026-11-22T05:02:00Z

---

Plan approved by @bruk (39ca1e575b3d, size-3): its worker may build it.

---
<!-- COMMENTS:END -->
