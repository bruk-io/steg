---
id: TASK-1.147
title: 'Rename STEG_SOCKET_PATH to STEG_SERVER: one variable for reaching a server'
status: In Progress
assignee:
  - '@bruk.w6'
created_date: '2026-11-21T20:23:00Z'
updated_date: '2026-11-21T21:43:00Z'
labels:
  - size-2
milestone: m-3
dependencies: []
parent_task_id: TASK-1
project: bruk-io-steg
asdlc_accepted_from: DRAFT-4
asdlc_branch: task-1.147-rename-steg-socket-path-to-steg-server
asdlc_plan_approved_by: bruk
asdlc_plan_approved_hash: '042346b24b0f'
asdlc_plan_approved_size: size-2
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Decided by the engineer on 2026-11-21: one variable, not two. doc-2 section 4 defines STEG_SERVER as how to reach the server a terminal belongs to; STEG_SOCKET_PATH is in no document and grew up in the code (crates/sdk/src/local_socket.rs and paths.rs, crates/server/src/config.rs and listen.rs, and about fourteen test and fixture files in crates/server and crates/steg) as the way to point a client or a server at one socket. Rename it to STEG_SERVER everywhere, with a deterministic tool rather than by hand, including doc comments, and change no behaviour: the value is still a socket path, read in the same places, and a server reads it to decide where to listen. Nothing reads both names and there is no fallback to the old one (nothing released depends on it). TASK-1.146 puts the same variable in a pane's environment and TASK-1.51 has the CLI follow it; with one name there is no precedence between two. Touches many test files by one line each, so merge origin/main just before finishing.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 No occurrence of STEG_SOCKET_PATH remains in the repository's code, tests, fixtures, justfile or tools; every reader and writer uses STEG_SERVER
- [ ] #2 A client and a server given STEG_SERVER use that socket, proven by the existing tests that set it, which pass unchanged apart from the name
- [ ] #3 just check passes; the notes list the files changed and say the rename was done with a deterministic tool
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
## 1. Inventory

A repo-wide grep (`grep -rn STEG_SOCKET_PATH .`, excluding `target/` and `.git/`) finds 79 occurrences. 34 are under `backlog/` (this task's own file, task-1.51's plan, and six review/task files for 1.49, 1.70, 1.77, 1.87, 1.98, 1.121) — those are historical board records and are never hand-edited (`CLAUDE.md`); they stay as `STEG_SOCKET_PATH` forever, describing the name that was true when written. `justfile`, `tools/`, `.github/`, `.claude/rules/`, `README*` and `crates/ui`/`crates/app` have zero occurrences — nothing to do there.

The remaining 45 occurrences, in 18 files, all under `crates/`:

**Readers of the explicit override** (3, all `env::var_os("STEG_SOCKET_PATH")`, each feeding `steg_sdk::paths::socket_path`):
- `crates/sdk/src/local_socket.rs:126` (`resolve_path`, used by every `connect()`)
- `crates/sdk/src/local_socket.rs:445` (`run_fake_server_for_tests`, test support)
- `crates/server/src/listen.rs:598` (`resolve_socket_path`)

**The one writer** (sets it on a spawned child's environment):
- `crates/sdk/src/local_socket.rs:174` (`connect()`'s autostart spawn, from `options.socket_path`)

**Doc comments naming the variable** (mechanical swap, reads fine either way):
- `crates/server/src/config.rs:80`, `crates/server/src/listen.rs:582`
- `crates/sdk/src/local_socket.rs:54,57`, `crates/sdk/src/paths.rs:10`
- `crates/steg/tests/cli_exit_statuses.rs:119`, `crates/steg/tests/cli_no_server.rs:5,10,29`
- `crates/steg/tests/fixtures/autostart_client.rs:20`
- `crates/server/tests/fixtures/race_attempt.rs:2`

**Test/fixture code setting it explicitly on a spawned process** (mechanical swap — each points a specific test at its own throwaway socket): `crates/server/tests/race.rs:42`, `accept_backoff.rs:187`, `fixtures/race_attempt.rs:18`, `fixtures/accept_backoff_attempt.rs:58-59`, `crates/steg/tests/server_start.rs` (8 occurrences, 2 shared helpers used by many tests), `server_show_stop.rs:97,113`, `cli_exit_statuses.rs:127,137,558`, `idle_stop.rs:91,134`, `cli_no_server.rs:35`, `cli_output_formats.rs:90,700`, `cli_api_call.rs:27,37`, `cli_connection_arbitration.rs:35`.

**One file needs more than a mechanical swap** — `crates/steg/tests/login_environment.rs` (3 occurrences: doc comment at line 78, `.env("STEG_SOCKET_PATH", &socket_path)` at line 269, and a bare string in a list at line 315). See point 3.

**What does NOT change:** `socket_path`/`bind_socket_path`/`resolve_socket_path`/`unique_test_socket_path` and the `socket_path`/`explicit` field and parameter names throughout `crates/sdk/src/paths.rs`, `crates/sdk/src/local_socket.rs`, `crates/server/src/config.rs`, `crates/server/src/listen.rs`. These name a path value, never the environment variable — the mechanical command only ever matches the literal uppercase token `STEG_SOCKET_PATH`, so these are untouched by construction (confirmed: `grep -rn 'STEG_SOCKET_PATH[A-Za-z_]' crates` finds nothing, so there is no suffix collision either).

## 2. The deterministic tool

One command, scoped to `crates/` (which is where every non-backlog occurrence lives, so this scoping alone keeps `backlog/` untouched with no separate exclusion needed):

```
grep -rl --null 'STEG_SOCKET_PATH' crates | xargs -0 sed -i '' 's/STEG_SOCKET_PATH/STEG_SERVER/g'
```

(`grep` here is `ugrep`, whose null-terminated-filename flag is `--null`/`-0`, not GNU's `-Z` — checked with `grep --help` before writing this, not assumed. `sed -i ''` is BSD sed's in-place syntax, confirmed by `sed --version` erroring, the BSD signature, on this machine.)

The build is: run that command, then one hand edit (point 3), then review the whole diff — not a file-by-file hand edit of the mechanical occurrences.

## 3. The one hand edit: `crates/steg/tests/login_environment.rs`

This file's `plumbing_var` list (around line 309-316) asserts a set of variable names are **absent** from a pane's whole environment dump, because the launching client (`env_leak_client`) sets them on *its own* process only, as plumbing for the test fixture chain, and the pane could never legitimately inherit them. `STEG_SOCKET_PATH` is in that list today for exactly that reason: it is the override `env_leak_client` sets on itself so the sdk's `connect()` autostarts a server at a specific throwaway socket, never something `pane_environment` copies into a pane.

After the rename, `STEG_SOCKET_PATH` and `STEG_SERVER` are the same name. A mechanical swap of this one list entry would make it assert `STEG_SERVER` is absent from the pane — but `pane_environment` (`crates/server/src/env.rs:51`) always sets `STEG_SERVER` on every pane (currently to `""`, the placeholder `crates/server/src/runtime/start.rs:217-218` passes it, pending TASK-1.146). `assert_var_absent`'s own check (`dump.lines().any(|line| line.starts_with("STEG_SERVER="))`) matches the empty-valued line `STEG_SERVER=` that `env` genuinely prints. **A naive rename of just this list entry makes this test fail on every run, deterministically, not flakily — checked by reading `assert_var_absent`'s implementation, not assumed.**

The hand edit: remove `"STEG_SOCKET_PATH"` from the list entirely (not rename it in place) with a one-line comment explaining why — once the two names are unified, "a pane must never carry this name" is no longer true for it, the opposite of what the surrounding list means; nothing distinct is being tested once the names are one, so dropping it is a deliberate removal of a now-contradictory check, not a loss of coverage. Reword the one clause in the line-78 doc comment that lists it among the plumbing vars, to match. The `.env("STEG_SOCKET_PATH", &socket_path)` at line 269 (setting the override on `env_leak_client` itself) is unaffected by this and renames mechanically along with everything else.

## 4. The behaviour point (flagged for the engineer): empty must be treated as absent

**Forward direction (point 3 of the brief).** `env::var_os` returns `Some(OsString::from(""))` for a variable that is *set but empty* — it does not collapse to `None`. Today, inside a pane, `STEG_SOCKET_PATH` is never set at all (`pane_environment` doesn't emit it), so `resolve_path`/`resolve_socket_path` fall through to `XDG_RUNTIME_DIR`/`TMPDIR`/the `USER` fallback and find the real socket. After this rename, both functions read `STEG_SERVER` instead — which `pane_environment` *does* always set, today to the empty-string placeholder. `paths::socket_path` (`crates/sdk/src/paths.rs:30`) treats any `Some(path)` as an override that "wins outright," with no check for emptiness, so it returns the empty path. `UnixStream::connect("")` fails; a client falls back to autostarting a second server, and that new server's own `resolve_socket_path` resolves the same empty path and fails to bind. **A `steg` run inside a pane, between this task merging and TASK-1.146 merging, would be worse off than today** — this is a real regression this rename would introduce if done as a pure mechanical swap.

**Reverse direction (point 4 of the brief).** Checked `crates/server/src/arbiter.rs`'s `attempt` and `crates/steg/src/main.rs:106-130`: a second `steg server start` resolving to the *same* socket path that's already bound gets `Arbitration::AlreadyRunning`, prints "a server is already running", and exits 0 — the correct "one server per socket, the second finds the first" outcome, not a conflict (`Arbitration::Refuse`/exit 4 is reserved for a genuinely untrusted or unsafe existing file at that path, a different case). Today, a program in a pane that runs `steg server start` inherits no `STEG_SOCKET_PATH`, so it resolves the same default path as the first server and correctly hits `AlreadyRunning`. After the rename, with the point-3 fix in place, it inherits `STEG_SERVER=""`, treated as absent, so it resolves the *same* default path too — identical, correct outcome. Without the fix, it would resolve an empty path, fail to bind, and get `Refuse`/exit 4 instead of the harmless `AlreadyRunning` — a second, related regression. **The one fix below answers both points.**

**The fix:** in `paths::socket_path`, treat an empty explicit path the same as `None` — it is already the pure function every reader of the explicit override funnels into (`local_socket.rs:126`, `local_socket.rs:445`, `listen.rs:598` all call it; `options.socket_path`/`config.socket_path`'s own early returns are real `Option<PathBuf>`s a caller built directly, never read from a bare env-string, so they're never empty-by-accident and are out of scope here), so one change in one already-pure, already-tested function covers every reader with no duplication — in keeping with this repo's functional-core rule (the three `env::var_os` reads stay the only I/O, at the edges; this is a decision about data already received, not a new environment read).

```rust
pub fn socket_path(
    explicit: Option<&Path>,
    xdg_runtime_dir: Option<&Path>,
    tmp_dir: Option<&Path>,
    user: Option<&str>,
) -> PathBuf {
    if let Some(path) = explicit.filter(|p| !p.as_os_str().is_empty()) {
        return path.to_path_buf();
    }
    ...
```

Alternative the engineer may prefer instead: filter at each of the three `env::var_os` call sites (`.filter(|v| !v.is_empty())` before `.map(PathBuf::from)`), keeping `socket_path` itself unaware of the "set but empty" env quirk. Either reaches the same behaviour; the single-function fix is recommended because it is one change instead of three and the function already owns this exact kind of precedence decision.

Add one test next to `an_explicit_override_wins_over_everything_else` in `crates/sdk/src/paths.rs`:
```rust
#[test]
fn an_empty_explicit_override_is_treated_as_absent() {
    let path = socket_path(
        Some(Path::new("")),
        Some(Path::new("/run/user/501")),
        Some(Path::new("/tmp/t")),
        Some("bruk"),
    );
    assert_eq!(path, Path::new("/run/user/501/steg.sock"));
}
```

This is the one behaviour change in this otherwise-pure rename, flagged here for the engineer to confirm before it is built.

## 5. Proof

- `just check` at the final head (format, lint, boundaries, licences, the affected tests).
- `grep -rl --null 'STEG_SOCKET_PATH' crates` recorded showing no output (exit 1) — the zero-occurrence proof, scoped to code/tests/fixtures as criterion #1 asks, deliberately not repo-wide (`backlog/`'s 34 occurrences are historical record, excluded on purpose, explained above).
- Criterion #2 (a client and a server given `STEG_SERVER` use that socket): the existing tests that set it and pass unchanged apart from the name — `crates/steg/tests/server_show_stop.rs`, `cli_connection_arbitration.rs`, `cli_exit_statuses.rs`, `cli_output_formats.rs`, `cli_api_call.rs`, `idle_stop.rs`, `cli_no_server.rs` (client side, through the real binary) and `crates/server/tests/race.rs`, `accept_backoff.rs` (server side, through the real fixture binaries).
- Negative control for the main mechanism, run by hand during the build and recorded, then reverted (not committed): in `crates/steg/tests/cli_connection_arbitration.rs`, the test at line ~33 does `.env_clear()` then sets only `PATH` and the renamed `STEG_SERVER` — remove that one `.env("STEG_SERVER", socket)` line and re-run; with no `XDG_RUNTIME_DIR`/`TMPDIR`/`USER` in the cleared environment either, resolution falls all the way to `/tmp/steg-unknown.sock`, nothing is listening there, and the test must fail. `env_clear()` makes this deterministic rather than dependent on this machine's real `TMPDIR` happening to differ from the test's chosen socket.
- The point-3/4 fix's own proof: the new `paths.rs` unit test above (point 4), run clean.
- Notes list every file the mechanical command touched (from a diff, not by hand), name the command, and name the one hand edit (point 3) and the one behaviour fix (point 4) separately so a reviewer can tell "mechanical" from "judgement" at a glance.

## 6. Overlaps

- **TASK-1.145** (PR 90): already merged into the branch this worktree is based on (confirmed — `git log` on this worktree shows TASK-1.145's finish/review commits already in its history before this task's claim commit). No further action.
- **TASK-1.140** (committed locally, unpushed: `connection.rs`, `runtime/`, `dispatch/`, a new `crates/server/tests/pane_wait.rs`): confirmed this worktree does not have `crates/server/tests/pane_wait.rs` yet. The rename command is idempotent and files-based, not a fixed line list, so either order is handled the same way: merge `origin/main` (step already required by this task's own description, "just before finishing"), then re-run the identical `grep`/`sed` command and the zero-occurrence `grep` check again. Whichever of us merges second picks up any new occurrence the other's task introduced, for free, with no manual reconciliation.
- **TASK-1.136** (`crates/ui`, `tools/`): checked both — zero occurrences in either, confirmed by grep. No overlap.
- **TASK-1.51** (depends on this task, builds after it): no action needed here beyond what this task already does; 1.51's own plan already accounts for the rename landing first.

## Build-phase steps, in order

1. Run the sed/grep command from point 2.
2. Make the one hand edit in `login_environment.rs` (point 3): remove the list entry and its matching doc-comment clause.
3. Make the one behaviour fix in `crates/sdk/src/paths.rs` (point 4), plus its new unit test.
4. Run the negative control by hand (point 5), confirm it fails, then confirm reverting the removed line makes it pass again (don't commit the removal).
5. Run the zero-occurrence `grep` scoped to `crates/`.
6. Merge `origin/main`; if any new occurrence appeared (e.g. from TASK-1.140 landing first), re-run steps 1 and 5.
7. `just check`; record it.
8. Commit, with notes listing the files touched, the command, the hand edit, and the behaviour fix, each labelled separately.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Plan (042346b24b0f) checked by @bruk.o on 2026-11-21 against the task and doc-2 section 4: ok. The rename is one command over crates/, with one hand edit: crates/steg/tests/login_environment.rs lists the old name among variables a pane must not carry, and STEG_SERVER is one a pane does carry by design, so that entry is removed rather than renamed; a reviewer should confirm no other assertion is loosened. One behaviour change, for the engineer: an explicit path that is set but empty is treated as absent, in paths::socket_path, because a pane's STEG_SERVER is an empty placeholder until TASK-1.146 and would otherwise be read as a path. Orchestrator's recommendation: the single-function fix. The zero-occurrence proof is a recorded grep whose exit status 1 means nothing found, and the notes must say so. backlog/ keeps the old name in historical records, untouched.

Check FAILED: `cargo test -p steg --test cli_connection_arbitration` exited 101 at 2026-11-21T21:43:11+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.147 on task-1.147-rename-steg-socket-path-to-steg-server at a747ba875cc5 with uncommitted changes, last 40 of 45 lines:

```text
   |
   = note: `#[warn(unused_variables)]` (part of `#[warn(unused)]`) on by default

warning: `steg` (test "cli_connection_arbitration") generated 1 warning (run `cargo fix --test "cli_connection_arbitration" -p steg` to apply 1 suggestion)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.09s
     Running tests/cli_connection_arbitration.rs (target/debug/deps/cli_connection_arbitration-c87ff0de211c214f)

running 14 tests
test support::tests::candidate_name_does_not_collide_under_the_same_forced_pid_and_counter ... ok
test support::tests::the_old_pid_and_counter_only_scheme_collides_when_two_processes_share_a_pid ... ok
test support::tests::run_bounded_returns_the_bodys_value_when_it_finishes_in_time ... ok
test support::tests::a_fresh_scoped_temp_dir_is_0700 ... ok
test support::tests::path_panics_rather_than_hand_back_something_over_the_sun_path_budget - should panic ... ok
test support::tests::wait_for_socket_returns_once_something_is_listening ... ok
test support::tests::one_scoped_temp_dir_can_hand_out_two_different_paths ... ok
test support::tests::scoped_temp_dir_exists_after_new_and_is_gone_after_drop ... ok
test support::tests::create_unique_dir_skips_a_name_that_already_exists ... ok
test the_hostile_listener_itself_accepts_and_closes_with_nothing_written ... ok
test support::tests::two_scoped_temp_dirs_made_back_to_back_are_different ... ok
test support::tests::run_bounded_panics_when_the_body_does_not_finish_in_time - should panic ... ok
test support::tests::wait_for_socket_panics_when_nothing_ever_answers - should panic ... ok
test a_hostile_first_connection_does_not_stop_a_real_autostart_from_succeeding ... FAILED

failures:

---- a_hostile_first_connection_does_not_stop_a_real_autostart_from_succeeding stdout ----

thread 'a_hostile_first_connection_does_not_stop_a_real_autostart_from_succeeding' (73686263) panicked at crates/steg/tests/cli_connection_arbitration.rs:100:5:
assertion `left == right` failed: stderr: steg: timed out waiting for the steg server's socket

  left: Some(4)
 right: Some(0)


failures:
    a_hostile_first_connection_does_not_stop_a_real_autostart_from_succeeding

test result: FAILED. 13 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 10.25s

error: test failed, to rerun pass `-p steg --test cli_connection_arbitration`
```

Check passed: `cargo test -p steg --test cli_connection_arbitration` exited 0 at 2026-11-21T21:43:22+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.147 on task-1.147-rename-steg-socket-path-to-steg-server at a747ba875cc5 with uncommitted changes:

```text
   Compiling steg v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.147/crates/steg)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.35s
     Running tests/cli_connection_arbitration.rs (target/debug/deps/cli_connection_arbitration-c87ff0de211c214f)

running 14 tests
test support::tests::the_old_pid_and_counter_only_scheme_collides_when_two_processes_share_a_pid ... ok
test support::tests::candidate_name_does_not_collide_under_the_same_forced_pid_and_counter ... ok
test support::tests::run_bounded_returns_the_bodys_value_when_it_finishes_in_time ... ok
test support::tests::a_fresh_scoped_temp_dir_is_0700 ... ok
test support::tests::scoped_temp_dir_exists_after_new_and_is_gone_after_drop ... ok
test support::tests::one_scoped_temp_dir_can_hand_out_two_different_paths ... ok
test support::tests::path_panics_rather_than_hand_back_something_over_the_sun_path_budget - should panic ... ok
test support::tests::wait_for_socket_returns_once_something_is_listening ... ok
test the_hostile_listener_itself_accepts_and_closes_with_nothing_written ... ok
test support::tests::two_scoped_temp_dirs_made_back_to_back_are_different ... ok
test support::tests::create_unique_dir_skips_a_name_that_already_exists ... ok
test support::tests::wait_for_socket_panics_when_nothing_ever_answers - should panic ... ok
test support::tests::run_bounded_panics_when_the_body_does_not_finish_in_time - should panic ... ok
test a_hostile_first_connection_does_not_stop_a_real_autostart_from_succeeding ... ok

test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.37s
```
<!-- SECTION:NOTES:END -->

## Comments

<!-- COMMENTS:BEGIN -->
author: @bruk
created: 2026-11-21T21:37:00Z

---

Plan approved by @bruk (042346b24b0f, size-2): its worker may build it.

---
<!-- COMMENTS:END -->
