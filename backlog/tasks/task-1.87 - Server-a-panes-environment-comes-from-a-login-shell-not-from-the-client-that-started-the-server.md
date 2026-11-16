---
id: TASK-1.87
title: >-
  Server: a pane's environment comes from a login shell, not from the client
  that started the server
status: In Progress
assignee:
  - '@bruk.w1'
created_date: '2026-11-15T23:40:00Z'
updated_date: '2026-11-16T05:14:00Z'
labels:
  - size-3
milestone: m-3
dependencies:
  - TASK-1.77
parent_task_id: TASK-1
project: bruk-io-steg
asdlc_accepted_from: DRAFT-2
asdlc_branch: task-1.87-server-a-pane-s-environment-comes-from-a
asdlc_plan_approved_by: bruk
asdlc_plan_approved_hash: '03e926f72daf'
asdlc_plan_approved_size: size-3
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Third piece of the split of TASK-1.77 (2026-11-15). doc-8 R99: a server started on demand 'takes its environment from a login shell and not from that client'. The worker's plan on TASK-1.77 proposed resolving only the login identity (shell, home, user) from the password database; that is needed but is not the requirement, since the server would still pass the starting client's environment on to every pane. TASK-1.76's Runtime takes one injected LoginShell value read today from the process environment as a named stand-in; this task replaces that and supplies the base environment. TASK-1.44's first plan (before its split) has an earlier design for capturing the environment with a cleared, seeded child and a bounded wait: read it. No unsafe: std::env::set_var is not to be used; the environment is data handed to the launcher.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The environment a pane's program starts with comes from a login shell of the person's account, found from the password database, and not from the environment of the client that happened to start the server (R99): a variable set only in that client's environment is absent from a new pane, and what a login shell exports (PATH above all) is present, for a shell pane and for a command pane, shown with a real server started on demand by a real client
- [ ] #2 Getting that environment cannot hang the server or delay its start beyond a stated bound on a slow, broken or interactive shell profile; what is used when it fails or times out is stated, never the client's environment unscrubbed, and every child started to get it is waited on along every path
- [ ] #3 The STEG_ variables a pane is given (doc-2 section 4) and TERM are set by the server whatever the login shell says; the working-directory rule of TASK-1.76 is unchanged
- [ ] #4 The plan weighs the ways to do it and picks one with reasons: capturing a login shell's environment once at server start; starting every pane's program through a login shell with a scrubbed environment; or both; and says what each means for a command pane, which runs no shell of its own
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
One reviewable change. The natural-looking seam - "launcher gets `env_clear()`" as one
change, "where the base environment comes from" as another - does not survive: shipping
`env_clear()` without a captured base gives every pane an *empty* environment, strictly
worse than today's leak. The two halves are only safe to ship together.

**Blocking fact, not a risk to work around: AC #1 cannot be proven as worded until
TASK-1.45 (PR #50, reviewed clean) merges.** There is no production call to
`Runtime::handle` from a real connection today (confirmed: `grep -rn "Runtime::new"
crates/` outside tests finds nothing, and `listen::run` takes no `Runtime` on `main`) -
"shown with a real server started on demand by a real client" has no path to exist until
TASK-1.45's dispatch wiring lands. This is the engineer's call, not mine to route around:
flagged in my message, not resolved by substituting a weaker Runtime-level proof for AC #1.

## 1. What "a login shell's environment" means here, and how it is obtained

Two bounded, sequential steps, each its own function, each its own timeout - kept separate
because they have different costs and different failure shapes (TASK-1.77's own
`resolve_login_identity` sketch already treats identity as cheap/no-child-process; this
task's own environment capture is the one that spawns a real shell).

- **Identity** (`login_env::resolve_login_identity(timeout) -> LoginIdentity { shell:
  PathBuf, home: PathBuf, user: Option<String> }`): `nix::unistd::User::from_uid(geteuid())`
  on a worker thread, `recv_timeout` on the caller - verbatim TASK-1.77's own design,
  already reviewed sound. `user: None` on no passwd entry/error/timeout (not joined on
  timeout - an abandoned thread is harmless, TASK-1.77's own reasoning); `shell`/`home`
  fall back to the literal defaults `LoginShell::from_process_env` uses today
  (`/bin/zsh`, `/`) - moved here as named constants, not reinvented.
- **The environment** (`login_env::capture_login_environment(shell: &Path, home: &Path,
  user: Option<&str>, timeout: Duration) -> Vec<(OsString, OsString)>`, called with the
  identity just resolved): spawns `shell -l -c '<marker-echo>; exec /usr/bin/env -0'`
  with `.env_clear()`, four seeded variables, `.stdin(Stdio::null())`,
  `.stderr(Stdio::null())`, an absolute path for `/usr/bin/env` (PATH was just cleared).
  Captured, then reaped, on every path (next section).

**Seeded: `HOME`, `USER`, `LOGNAME` (from the identity just resolved - zero client
provenance), and `PATH` (a fixed, minimal default, see §5).** Verified empirically, not
assumed (`env -i HOME=<real identity> USER=... LOGNAME=... PATH=/usr/bin:/bin /bin/zsh -l
-c 'echo HOME=$HOME'` on this machine): leaving `HOME` unset makes zsh resolve the *real*
machine's own home directory via passwd on its own - correct in production (it's the same
answer), but it means a test cannot point a throwaway home at a temp directory without
seeding it explicitly, and the brief asks the plan to say which few variables are seeded
and why. `PATH` is seeded because a profile script might assume one exists even though
(verified) macOS's own `/etc/zprofile`'s `path_helper -s` works fine with none at all -
seeded anyway, defensively, for a non-`path_helper` profile.

**Which startup file actually runs - checked, not assumed.** `/bin/zsh -l -c 'exec
/usr/bin/env -0'` with a throwaway `HOME` whose `.zprofile` and `.zshrc` each export a
distinct marker: only `.zprofile`'s marker appears (`-l` without `-i` sources the login
profile, not the interactive `.zshrc` - confirmed by running it, output attached to this
task's checks). `PATH` above all` (the acceptance criterion's own phrase) means
`.zprofile`, not `.zshrc`, is the file this task's own leak/inheritance proof has to
write its marker into, or it proves nothing - most macOS users who edit `PATH` do it in
`.zshrc`, so this is named explicitly rather than left for a reviewer to catch.

**Noise from startup files is handled by a marker, not assumed absent.** Login-profile
sourcing happens during shell start-up, strictly before the `-c` string's own commands
run, so a fixed sentinel printed as the *first* thing `-c` does (`echo
__steg_env_capture__; exec /usr/bin/env -0`) always lands after every line of profile
noise - `parse_env_dump` discards every byte up to and including that line's newline,
so a profile that echoes a banner never corrupts the NUL-delimited dump that follows.

## 2. The bounded wait, and what happens when the shell is slow, hangs, or fails

No child is ever left unreaped, on any path - verified, not merely designed: a
throwaway Rust project confirmed `CommandBuilder::env_clear()` with no `PATH` fails
`spawn_command` outright (`Unable to resolve the PATH`) for a relative program name,
which is why every `LaunchSpec.env` below now carries a real `PATH`, and confirmed that
a later `.env(key, value)` call for a key already set wins over an earlier one -
`BTreeMap`'s own insert semantics, which is what makes the override ordering in §4 and
§6 correct without a separate merge function.

**`capture_login_environment`'s own bounded wait**, same shape as TASK-1.44's original
(now-stale) draft and TASK-1.77's own `resolve_login_identity`: spawn the child, keep its
pid (`child.id()`, read *before* the move below), move only `child.stdout.take()`'s
`ChildStdout` into a reader thread that reads it to completion and sends the bytes over a
channel; the caller does `recv_timeout(timeout)`. On timeout: call `child.kill()` then
`child.wait()` *directly on the caller's own `Child`* (never moved, since only its stdout
was) - no new `nix` feature needed for this (an earlier draft of this plan proposed
`nix`'s `signal` feature for a kill-by-pid; dropped once the design kept the `Child`
itself on the caller's side instead of moving it into the watcher thread - named here so a
reviewer does not wonder why `nix`'s `Cargo.toml` entry is unchanged by this task).

**On any failure - spawn error, non-zero exit, timeout, no passwd entry at all - the
fallback is a small, fixed, named environment, never the server's own inherited
environment in any form.** This departs from TASK-1.44 §4's own stale draft, which fell
back to "the inherited environment, with every `STEG_*` key stripped" - rejected here
because that still carries every other variable the client's environment held, which is
exactly what R99 forbids; AC #2's own wording ("never the client's environment
unscrubbed") is read here as "never derived from it at all," the stronger and simpler
reading. The fallback is a named constant, so the fallback test can assert on it exactly:

```rust
const FALLBACK_PATH: &str = "/usr/bin:/bin:/usr/sbin:/sbin";

fn fallback_env(identity: &LoginIdentity) -> Vec<(OsString, OsString)> {
    let mut env = vec![
        (OsString::from("PATH"), OsString::from(FALLBACK_PATH)),
        (OsString::from("HOME"), identity.home.clone().into_os_string()),
    ];
    if let Some(user) = &identity.user {
        env.push((OsString::from("USER"), OsString::from(user)));
        env.push((OsString::from("LOGNAME"), OsString::from(user)));
    }
    env
}
```

**The server still starts, with this fallback environment, and the fallback is
reported.** `login_env`'s own functions return data, never print (functional
core/imperative shell, matching this crate's own existing boundary): the resolving
call returns an enum alongside the `LoginShell` - `Resolution::Captured` or
`Resolution::Fallback(Reason)` (`Reason` names spawn/exit/timeout/no-identity) - and
whichever binary crate owns the actual call (today, nowhere real yet; see the blocking
fact above) prints a stderr line on `Fallback`, the same way `crates/steg/src/main.rs`
already prints its own diagnostics (doc-2 §6: diagnostics to stderr) - this task wires
the *data*, not the print, since `steg-server` does not print today and this task does
not start.

## 3. When it is captured, and the bound on a client's first request

**Identity resolution stays at the exact call site TASK-1.77 names (before `attempt`),
with its own short bound (`DEFAULT_LOGIN_IDENTITY_TIMEOUT`, proposed 2s) - a modest,
accepted risk to the socket's own bind time**, matching the bound TASK-1.77's own design
already accepted for this same call (a directory-service-bound account's `getpwuid_r`
*can* stall; this is not new risk this task introduces, only a bound placed on
pre-existing risk).

**Environment capture happens once, after the socket is already bound, inside whichever
code constructs the one `Runtime` (TASK-1.45's own `listen::run`, once it merges) -
*before* that function's own accept loop starts, with its own bound
(`DEFAULT_LOGIN_ENVIRONMENT_TIMEOUT`, proposed 3s).** Checked against the real code,
not assumed: `attempt()`'s own bind already happened earlier in `main.rs`'s
`run_server_start`, so a client's `UnixStream::connect()` already succeeds (kernel-level
backlog) regardless of this step; nothing reads from that connection - no hello reply -
until `accept()` is actually called, which only happens once this capture step returns.
**Correction to an assumption worth stating plainly: there is no client-side read
deadline anywhere in this codebase today** (`crates/sdk/src/wire/connection.rs`'s
`Connection` is a pure, no-I/O state machine with no timeout field at all, confirmed by
reading it; `local_socket::connect`'s own `connect_timeout` only bounds the TCP-level
connect retry, not any hello exchange) - so this capture's own bound cannot violate a
deadline that does not yet exist. The two bounds (2s + 3s = 5s worst case) are still
chosen with comfortable margin against the server's *own* `DEFAULT_HANDSHAKE_TIMEOUT`
(10s, the bound a connection has to complete its hello once `accept()` finally runs),
so a future client-side deadline has room, and so an operator watching a cold on-demand
start sees it within a few seconds either way.

**No special-casing between an explicit and an on-demand start.** Both end up at the
same post-bind point before the one `Runtime`'s accept loop starts; `on_demand` never
appears in this task's own code.

## 4. What a pane's program actually receives

Per pane, in order (later entries win, by key, via `CommandBuilder`'s own
insert-overwrites-by-key semantics, verified in §2's throwaway project - no merge
function needed): the captured base environment (§1/§2), then `pane_environment`'s own
five `STEG_*` variables and `PATH` (bin dir prepended to the captured base's own `PATH`,
not `std::env::var_os("PATH")` - today's actual bug), then `TERM`.

**`TERM=xterm-256color`** - matching the existing precedent already in this repository
(`spikes/task-1.2-wire-protocol/probe-server/src/pty.rs:49`, decision-3's own probe),
checked rather than invented. **Explicitly not `xterm-ghostty`**, even though
`third_party/ghostty-vt-patched` vendors a compiled `xterm-ghostty` terminfo database
(`ghostty_vt::terminfo::{TERM, dir()}`) that is arguably the *more correct* answer for
this exact VT engine (decision-3): that needs writing the terminfo cache directory
somewhere persistent and setting `TERMINFO` to point at it, a second piece of
integration this task's own acceptance criteria do not ask for and no code anywhere
uses yet (`grep -rn "terminfo" crates/` finds nothing). Named here as a real, better
option explicitly left out, for the engineer to schedule separately, not silently
dropped.

**AC #3's "whatever the login shell says"**: if a login shell's own profile happens to
export `TERM` or any `STEG_*` name (unlikely, but not impossible), the override always
wins - proven in §6's proof list, not merely asserted by ordering.

**A command pane needs nothing different from a shell pane.** Both get exactly the same
composed environment; the only difference between them is `argv[0]` (the login shell's
own path for `Program::Shell`, the caller's own program for `Program::Command`) - neither
is routed *through* a login shell (see §5's weighing).

## 5. The three ways weighed, and why "capture once" wins (AC #4)

- **Capture once, reuse as the base for every pane (chosen).** One bounded risk, taken
  once at server start, not once per pane open. A command pane gets the same captured
  base as a shell pane - no shell is in its process tree at all, so it costs nothing it
  would not already pay.
- **Start every pane's program through a login shell with a scrubbed environment
  (rejected).** For a command pane this means wrapping the caller's own argv in `<shell>
  -l -c 'exec <argv...>'`, which: (a) runs the login shell's own startup files on *every*
  pane open, not once - the same slow/hanging-profile risk §2 bounds, now repeated per
  pane instead of amortised; (b) needs correct shell-quoting of arbitrary argv, a new
  correctness surface `portable_pty::CommandBuilder::from_argv` does not have today; (c)
  a profile that prints a banner now pollutes every pane's own screen, not just a
  discarded capture buffer. `exec` does replace the shell so no extra process lingers and
  the exit code still passes through cleanly - but the repeated-startup-cost and
  quoting problems alone are enough to reject it.
- **Both (rejected, for the same reasons as the second option, with none of the first
  option's savings).**

## 6. Proof, by acceptance criterion, with a negative control per mechanism

Every control below is a scratch edit, recorded and reverted immediately
(TASK-1.77's own precedent for this).

- **AC #1** (login shell's environment reaches a pane, not the client's; shell and
  command pane; real on-demand server, real client): blocked on TASK-1.45 (see the top
  of this plan) - built and proven once it merges, through the real socket, the real
  `steg` binary, and a throwaway `HOME` (own `.zprofile`, see §1) so the test never reads
  this machine's own startup files. Negative control: with `env_clear()` removed from
  `pty.rs`, the leak half of this test fails (a variable set only in the *test harness's*
  own environment, never the fixture client's, appears in the pane anyway).
- **AC #2** (cannot hang/delay the server beyond a stated bound; stated fallback, never
  the client's environment unscrubbed; every spawned child waited on): `login_env.rs`'s
  own `#[cfg(test)]` module (real I/O, same convention as `bind.rs`/`paths.rs`), with a
  fake shell script (a real, throwaway executable) standing in for the real one -
  success path, non-zero exit, a script that sleeps past an injected short timeout
  (asserts `fallback_env`'s exact constant value, asserts the child is gone via
  `kill -0`/`wait` after the timeout fires), a script with no `env` binary reachable
  (spawn error). Negative control: the capture child's own `.env_clear()` removed - a
  marker set only in the *test's own* calling environment (never seeded) leaks into the
  captured result.
- **AC #3** (`STEG_*` and `TERM` always win): a fake login shell script that itself
  exports a conflicting `TERM` and `STEG_SERVER` - both come back as the server's own
  values in a real pty's screen (same technique as the existing
  `a_real_programs_environment_carries_the_five_variables_and_path` test). Negative
  control: the override/base ordering reversed in `launch_spec` - the fake shell's own
  values win instead, proving the ordering in §4 is load-bearing, not incidental.
- **AC #4**: satisfied by this plan's own §5, read by the engineer as part of review -
  no test proves a design weighing; the "command pane needs nothing different" half is
  proven by AC #1/#3's own tests using both `Program::Shell` and `Program::Command`.

## Files that change

- `crates/server/src/login_env.rs` (new): `LoginIdentity`, `resolve_login_identity`,
  `capture_login_environment`, `parse_env_dump`, `fallback_env`, `Resolution`/`Reason`,
  the two `DEFAULT_LOGIN_*_TIMEOUT` constants, `FALLBACK_PATH`. Own `#[cfg(test)]` module,
  real I/O, no mocking crate.
- `crates/server/src/runtime.rs`: `LoginShell` gains `base_env: Vec<(OsString,
  OsString)>`; `LoginShell::from_process_env` removed (its one call site is TASK-1.77's
  hand-off, found and replaced, or - if unmerged - this task's own equivalent call site);
  `launch_spec` reads `current_path` from `self.login.base_env`, not
  `std::env::var_os("PATH")`, and appends `TERM`. Test helpers `login()`/
  `login_for_marks()` gain a working `base_env` (ambient `PATH` only - a test, allowed to
  read it, unlike production code) so the 14 existing `Runtime::new(...)` call sites in
  this file's own tests stay unchanged; `from_process_env`'s own smoke test removed with
  it.
- `crates/server/src/pty.rs`: `RealLauncher::spawn` calls `command.env_clear()` before
  applying `spec.env`; `LaunchSpec.env`'s own doc comment rewritten - no longer "the
  overrides on top of whatever this process inherits," now "the pane's whole
  environment."
- `crates/server/src/env.rs`: module doc's "this only ever has to return the *overrides*"
  line corrected to say what's now true once `env_clear()` lands - `pane_environment`
  itself is unchanged code, only its doc's premise.
- `crates/server/src/config.rs`: two new fields, `login_identity_timeout` and
  `login_environment_timeout`, with the two new `DEFAULT_*` constants, following the
  existing `handshake_timeout`/`arbitration_timeout` pattern exactly.
- `crates/server/src/lib.rs`: `mod login_env;` (no new `pub use` - its functions are
  `pub(crate)`, consumed by `runtime.rs`/`listen.rs`, not public API).
- `crates/server/Cargo.toml`/`boundaries.toml`: unchanged - no new dependency, no new
  feature of an existing one (the `nix` `signal` feature this plan first considered is
  dropped, §2).
- Test-only `env: Vec::new()` fixes, every one needed because `env_clear()` now means
  "inherits nothing," verified empirically (§2) rather than assumed:
  `crates/server/src/pane.rs` (`real_spec`, one helper), `crates/server/tests/
  program.rs` (`spec`, one helper), `crates/server/tests/real_pty.rs` (its own `spec`/
  `sized_spec` callers, 6 call sites, fixed via one small local helper added to the
  file), `crates/server/src/runtime.rs`'s own test module (14 `Runtime::new` call sites,
  fixed at the two `login()`/`login_for_marks()` helpers rather than individually, 6
  remaining inline `LaunchSpec { env: Vec::new(), .. }` literals fixed via one local
  helper added to the same `#[cfg(test)]` module).
- `crates/steg/src/main.rs`: `run_server_start` gains the identity-resolution call at
  the existing pre-`attempt` call site (§3); the environment-capture call itself lands
  wherever `Runtime::new` is actually constructed once TASK-1.45 merges - if it has not,
  this task stops short of that wiring and says so (top of this plan), rather than
  inventing a second, throwaway call site.

## Risks

- **TASK-1.45 merge order, named concretely**: if it merges first, rebase, and the one
  line TASK-1.77 flagged becomes two (identity at the existing site, capture inside
  `listen::run` before its accept loop) - both additive to `ServerConfig`, no edit to an
  existing line beyond that. If it has not merged when the build phase starts, stop and
  say so rather than building a weaker substitute for AC #1 (top of this plan).
- **`ServerConfig`'s `Default` impl is a second, smaller point of contention with
  TASK-1.45**: both tasks add fields to the same struct literal. A textual merge
  conflict there is expected, not a sign anything is wrong - resolved by combining both
  tasks' added lines, same as TASK-1.77's own notes already expect for this struct.
- **A login shell that does not understand `-l`/`-c`** (csh/tcsh's own login convention
  differs) is not specially handled - it fails to spawn or produces unparseable output,
  which §2's own failure paths already catch, falling back cleanly rather than crashing.
  Named as an accepted gap, not silently absent.
- **The cross-uid/no-passwd-entry case is unverified against a real second account** -
  this machine has none, same honest gap TASK-1.44 (part A) already named for its own
  owner checks; the fallback path is proven instead with a fabricated/short-circuited
  identity.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Plan checked against doc-8 (R3, R98, R99), doc-2 section 4 and doc-16 by @bruk.o: ok, with one decision that is the engineer's and one ordering constraint. The environment is captured once, after the socket is bound and before requests are served, by running the account's login shell from a cleared environment seeded with HOME, USER, LOGNAME and a minimal PATH, with a marker before the dump so start-up noise cannot corrupt it and a bound of about three seconds; on any failure the server starts with a small fixed environment, never the starting client's. A pane's program gets the captured environment plus TERM and the STEG_ variables set by the server. No unsafe, no new dependency. The decision for the engineer: the capture runs the shell as a login shell and not as an interactive one, so on zsh it reads .zprofile and not .zshrc; a shell pane still reads .zshrc itself when it starts, but a command pane started through the API or the CLI gets only what .zprofile gives, and many people set PATH and tool managers in .zshrc. The alternative is to capture from an interactive login shell, as some editors do, at the cost of slower and noisier start-up files running in the capture; the bound and the fallback are the same either way. Ordering: criterion 1 is proved through a real client and a real server, which needs TASK-1.45 merged; the build starts after it merges.

Check passed: `cargo test -p steg-server --lib login_env::` exited 0 at 2026-11-16T04:33:18+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.87 on task-1.87-server-a-pane-s-environment-comes-from-a at ae10f11951f8 with uncommitted changes:

```text
warning: constant `DEFAULT_LOGIN_IDENTITY_TIMEOUT` is never used
  --> crates/server/src/login_env.rs:54:11
   |
54 | pub const DEFAULT_LOGIN_IDENTITY_TIMEOUT: Duration = Duration::from_secs(2);
   |           ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
   |
   = note: `#[warn(dead_code)]` (part of `#[warn(unused)]`) on by default

warning: constant `DEFAULT_LOGIN_ENVIRONMENT_TIMEOUT` is never used
  --> crates/server/src/login_env.rs:62:11
   |
62 | pub const DEFAULT_LOGIN_ENVIRONMENT_TIMEOUT: Duration = Duration::from_secs(3);
   |           ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^

warning: `steg-server` (lib test) generated 2 warnings
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.09s
     Running unittests src/lib.rs (target/debug/deps/steg_server-8563ad9cc9013408)

running 11 tests
test login_env::tests::capture_login_environment_falls_back_on_a_shell_that_does_not_exist ... ok
test login_env::tests::fallback_env_is_exactly_the_named_constant_set ... ok
test login_env::tests::fallback_env_omits_user_and_logname_when_the_identity_itself_has_none ... ok
test login_env::tests::parse_env_dump_splits_a_value_containing_its_own_equals_sign_on_the_first_one_only ... ok
test login_env::tests::parse_env_dump_skips_a_malformed_entry_with_no_equals_sign ... ok
test login_env::tests::resolve_login_identity_never_panics_and_always_names_a_real_shell_and_home ... ok
test login_env::tests::capture_login_environment_sources_the_login_profile_not_the_interactive_one ... ok
test login_env::tests::capture_login_environment_never_leaks_a_variable_set_only_in_the_callers_own_environment ... ok
test login_env::tests::capture_login_environment_falls_back_on_a_shell_that_exits_non_zero ... ok
test login_env::tests::capture_login_environment_prints_noise_before_the_marker_without_corrupting_the_dump ... ok
test login_env::tests::capture_login_environment_falls_back_on_a_shell_that_never_finishes_in_time ... ok

test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 112 filtered out; finished in 0.21s
```

Check FAILED: `cargo test -p steg-server --lib` exited 101 at 2026-11-16T04:39:24+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.87 on task-1.87-server-a-pane-s-environment-comes-from-a at ae10f11951f8 with uncommitted changes, last 40 of 147 lines:

```text
test login_env::tests::capture_login_environment_sources_the_login_profile_not_the_interactive_one ... ok
test runtime::tests::a_real_exit_event_drives_the_same_cascade_apply_event_runs ... ok
test runtime::tests::apply_event_on_a_pane_the_tree_no_longer_holds_removes_the_stale_handle_without_panicking ... ok
kill: 63478: No such process
test runtime::stop_tests::stop_is_a_no_op_for_a_pane_whose_program_has_already_ended ... ok
test runtime::tests::apply_size_changed_lets_a_pane_go_under_the_floor_when_the_kernel_says_so ... ok
kill: 63476: No such process
test runtime::stop_tests::stop_ends_a_silent_program ... ok
test runtime::tests::apply_size_changed_resizes_only_the_panes_the_change_names ... ok
test runtime::tests::disconnect_client_reapplies_the_next_latest_declared_size ... ok
test runtime::tests::pane_split_falls_back_to_home_when_the_inherited_live_cwd_no_longer_exists ... ok
test runtime::tests::pane_split_inherits_the_live_cwd_of_the_pane_it_split_from_not_its_started_in_cwd ... ok
kill: 63474: No such process
test runtime::stop_tests::stop_ends_a_program_producing_output ... ok
test runtime::tests::a_typed_command_still_running_stays_running_until_told_otherwise ... ok
test login_env::tests::capture_login_environment_falls_back_on_a_shell_that_exits_non_zero ... ok
test login_env::tests::capture_login_environment_prints_noise_before_the_marker_without_corrupting_the_dump ... ok
test login_env::tests::capture_login_environment_falls_back_on_a_shell_that_never_finishes_in_time ... ok
test runtime::tests::the_general_sizing_step_reaches_every_pane_through_split_close_and_swap ... ok
test runtime::tests::a_mark_then_the_shells_own_real_exit_cascades_with_no_panic ... ok
test runtime::tests::a_shell_with_no_marks_leaves_every_status_field_unchanged_until_one_arrives ... ok
test runtime::tests::a_typed_command_that_fails_is_failed_with_its_exit_code ... ok
test runtime::tests::a_typed_command_that_succeeds_is_running_then_finished_and_the_tab_shows_it ... ok
test runtime::tests::a_command_panes_own_program_printing_marks_does_not_change_its_status ... ok

failures:

---- pane::tests::a_forward_failing_once_the_worker_is_gone_still_reaps_a_program_that_ends_on_its_own stdout ----

thread 'pane::tests::a_forward_failing_once_the_worker_is_gone_still_reaps_a_program_that_ends_on_its_own' (22633151) panicked at crates/server/src/pane.rs:673:14:
called `Result::unwrap()` on an `Err` value: Pty(Unable to spawn sh because:
Unable to resolve the PATH)


failures:
    pane::tests::a_forward_failing_once_the_worker_is_gone_still_reaps_a_program_that_ends_on_its_own

test result: FAILED. 123 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.64s

error: test failed, to rerun pass `-p steg-server --lib`
```

Check FAILED: `cargo test -p steg-server --test real_pty` exited 101 at 2026-11-16T04:40:01+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.87 on task-1.87-server-a-pane-s-environment-comes-from-a at ae10f11951f8 with uncommitted changes, last 40 of 56 lines:

```text
thread 'a_command_panes_real_exit_code_is_known_it_stays_and_can_run_again' (22653532) panicked at crates/server/tests/real_pty.rs:166:6:
called `Result::unwrap()` on an `Err` value: Pty(Unable to spawn sh because:
Unable to resolve the PATH)
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace

---- a_nonexistent_cwd_is_not_a_failure_to_start_portable_pty_silently_uses_home stdout ----

thread 'a_nonexistent_cwd_is_not_a_failure_to_start_portable_pty_silently_uses_home' (22653534) panicked at crates/server/tests/real_pty.rs:303:6:
a bad cwd alone does not fail start() - portable-pty substitutes home and spawns: Pty(Unable to spawn sh because:
Unable to resolve the PATH)

---- a_shell_panes_real_exit_reaches_the_kernels_cascade stdout ----

thread 'a_shell_panes_real_exit_reaches_the_kernels_cascade' (22653537) panicked at crates/server/tests/real_pty.rs:224:6:
called `Result::unwrap()` on an `Err` value: Pty(Unable to spawn sh because:
Unable to resolve the PATH)

---- a_real_program_runs_in_the_given_working_directory stdout ----

thread 'a_real_program_runs_in_the_given_working_directory' (22653535) panicked at crates/server/tests/real_pty.rs:79:6:
called `Result::unwrap()` on an `Err` value: Pty(Unable to spawn sh because:
Unable to resolve the PATH)

---- many_threads_opening_a_real_pty_at_once_never_fails stdout ----

thread 'many_threads_opening_a_real_pty_at_once_never_fails' (22653538) panicked at crates/server/tests/real_pty.rs:407:31:
round 0/10, thread 0/32: RealLauncher::spawn failed under concurrent opens: the pseudo-terminal failed: Unable to spawn sh because:
Unable to resolve the PATH


failures:
    a_command_panes_real_exit_code_is_known_it_stays_and_can_run_again
    a_nonexistent_cwd_is_not_a_failure_to_start_portable_pty_silently_uses_home
    a_real_program_runs_in_the_given_working_directory
    a_shell_panes_real_exit_reaches_the_kernels_cascade
    many_threads_opening_a_real_pty_at_once_never_fails

test result: FAILED. 2 passed; 5 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.14s

error: test failed, to rerun pass `-p steg-server --test real_pty`
```

Check passed: `cargo test -p steg-server` exited 0 at 2026-11-16T04:42:26+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.87 on task-1.87-server-a-pane-s-environment-comes-from-a at ae10f11951f8 with uncommitted changes, last 40 of 323 lines:

```text
test a_command_that_does_not_exist_fails_and_leaves_no_session_tab_or_pane_behind ... ok
test pane_close_refuses_while_its_program_is_running_and_touches_nothing ... ok
test pane_split_with_a_nonexistent_cwd_leaves_no_new_pane_behind ... ok
test tab_new_with_a_nonexistent_cwd_leaves_no_new_tab_or_pane_behind ... ok
test pane_split_with_a_bad_command_leaves_no_new_pane_behind ... ok
test tab_new_with_a_bad_command_leaves_no_new_tab_or_pane_behind ... ok
test tab_close_refuses_while_any_of_its_panes_is_running ... ok
test session_new_with_a_command_starts_it_and_pane_show_reports_it_running_then_ended ... ok
test session_new_through_api_call_starts_a_real_program ... ok
test tab_new_through_api_call_starts_a_real_program ... ok
test pane_split_through_api_call_starts_a_real_program ... ok

test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.07s

     Running tests/shutdown.rs (target/debug/deps/shutdown-28715a1c11dd3b93)

running 8 tests
test a_stalled_handshake_is_closed_and_run_returns_promptly_on_stop ... ok
test stopping_closes_the_listener_removes_the_socket_and_releases_the_lock ... ok
test stop_on_a_genuinely_idle_server_returns_from_run_within_a_bound ... ok
test stop_called_twice_concurrently_on_an_idle_server_is_harmless ... ok
test a_completed_idle_connection_is_closed_and_run_returns_promptly_on_stop ... ok
test many_short_connections_do_not_grow_the_tracked_set ... ok
test stopping_while_a_trickling_handshake_is_in_progress_still_returns_within_a_bound ... ok
test stop_racing_a_client_connecting_still_returns_within_a_bound ... ok

test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.13s

     Running tests/socket_mode.rs (target/debug/deps/socket_mode-7da8aa152f6f03c1)

running 1 test
test the_socket_is_0600_even_under_a_permissive_umask ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests steg_server

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

Check passed: `cargo test -p steg --features fixtures` exited 0 at 2026-11-16T04:42:34+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.87 on task-1.87-server-a-pane-s-environment-comes-from-a at ae10f11951f8 with uncommitted changes:

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.08s
     Running unittests tests/fixtures/already_a_group_leader.rs (target/debug/deps/already_a_group_leader-e262f96fc889a606)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running unittests tests/fixtures/autostart_client.rs (target/debug/deps/autostart_client-6b0dd6f6b50bd8a0)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running unittests src/main.rs (target/debug/deps/steg-7b4495d74eacd8b0)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests/server_start.rs (target/debug/deps/server_start-6d77ec59f682e657)

running 11 tests
test an_unrecognised_server_start_variant_exits_2 ... ok
test an_untrusted_existing_file_refuses_with_status_4_and_names_the_reason ... ok
test typing_on_demand_at_a_shell_prompt_carries_on_attached_with_a_message ... ok
test started_is_printed_only_after_the_socket_already_accepts ... ok
test a_fresh_start_replaces_the_socket_of_a_server_killed_outright ... ok
test four_simultaneous_starts_converge_on_one_server ... ok
test running_out_of_file_descriptors_exits_1 ... ok
test a_losing_racers_dropped_child_is_a_zombie_until_reaped ... ok
test explicit_start_stays_up_with_no_client_ever_connecting ... ok
test the_clients_process_group_being_sighupd_does_not_end_the_server ... ok
test the_client_being_sigkilled_does_not_end_the_server ... ok

test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.34s
```

Check passed: `cargo test -p steg-server` exited 0 at 2026-11-16T04:46:20+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.87 on task-1.87-server-a-pane-s-environment-comes-from-a at ae10f11951f8 with uncommitted changes, last 40 of 323 lines:

```text
test pane_split_with_no_program_or_cwd_runs_the_login_shell_at_home ... ok
test session_close_refuses_while_any_pane_in_any_of_its_tabs_is_running ... ok
test session_new_with_a_command_starts_it_and_pane_show_reports_it_running_then_ended ... ok
test a_command_that_does_not_exist_fails_and_leaves_no_session_tab_or_pane_behind ... ok
test pane_split_with_a_nonexistent_cwd_leaves_no_new_pane_behind ... ok
test pane_split_with_a_bad_command_leaves_no_new_pane_behind ... ok
test tab_close_refuses_while_any_of_its_panes_is_running ... ok
test tab_new_with_a_bad_command_leaves_no_new_tab_or_pane_behind ... ok
test session_new_through_api_call_starts_a_real_program ... ok
test tab_new_through_api_call_starts_a_real_program ... ok
test pane_split_through_api_call_starts_a_real_program ... ok

test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.07s

     Running tests/shutdown.rs (target/debug/deps/shutdown-28715a1c11dd3b93)

running 8 tests
test stopping_closes_the_listener_removes_the_socket_and_releases_the_lock ... ok
test a_stalled_handshake_is_closed_and_run_returns_promptly_on_stop ... ok
test stop_called_twice_concurrently_on_an_idle_server_is_harmless ... ok
test stop_on_a_genuinely_idle_server_returns_from_run_within_a_bound ... ok
test a_completed_idle_connection_is_closed_and_run_returns_promptly_on_stop ... ok
test many_short_connections_do_not_grow_the_tracked_set ... ok
test stopping_while_a_trickling_handshake_is_in_progress_still_returns_within_a_bound ... ok
test stop_racing_a_client_connecting_still_returns_within_a_bound ... ok

test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.13s

     Running tests/socket_mode.rs (target/debug/deps/socket_mode-7da8aa152f6f03c1)

running 1 test
test the_socket_is_0600_even_under_a_permissive_umask ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests steg_server

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

Check FAILED: `cargo test -p steg-server --test real_pty a_real_programs_environment_never_leaks_this_processs_own_ambient_variables` exited 101 at 2026-11-16T04:46:57+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.87 on task-1.87-server-a-pane-s-environment-comes-from-a at ae10f11951f8 with uncommitted changes, last 40 of 221 lines:

```text































note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    a_real_programs_environment_never_leaks_this_processs_own_ambient_variables

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 7 filtered out; finished in 0.02s

error: test failed, to rerun pass `-p steg-server --test real_pty`
```

Check passed: `cargo test -p steg-server --test real_pty` exited 0 at 2026-11-16T04:47:38+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.87 on task-1.87-server-a-pane-s-environment-comes-from-a at ae10f11951f8 with uncommitted changes:

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.09s
     Running tests/real_pty.rs (target/debug/deps/real_pty-d5887de6b4314ba9)

running 8 tests
test a_command_that_does_not_exist_fails_synchronously_with_no_thread_to_join ... ok
test a_shell_panes_real_exit_reaches_the_kernels_cascade ... ok
test a_nonexistent_cwd_is_not_a_failure_to_start_portable_pty_silently_uses_home ... ok
test a_real_program_runs_in_the_given_working_directory ... ok
test a_real_programs_environment_never_leaks_this_processs_own_ambient_variables ... ok
test a_command_panes_real_exit_code_is_known_it_stays_and_can_run_again ... ok
test a_real_programs_environment_carries_the_five_variables_and_path ... ok
test many_threads_opening_a_real_pty_at_once_never_fails ... ok

test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.77s
```

Check FAILED: `cargo test -p steg-server --lib login_env::tests::capture_login_environment_never_leaks_a_variable_set_only_in_the_callers_own_environment` exited 101 at 2026-11-16T04:47:52+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.87 on task-1.87-server-a-pane-s-environment-comes-from-a at ae10f11951f8 with uncommitted changes:

```text
   Compiling steg-server v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.87/crates/server)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.69s
     Running unittests src/lib.rs (target/debug/deps/steg_server-8563ad9cc9013408)

running 1 test
test login_env::tests::capture_login_environment_never_leaks_a_variable_set_only_in_the_callers_own_environment ... FAILED

failures:

---- login_env::tests::capture_login_environment_never_leaks_a_variable_set_only_in_the_callers_own_environment stdout ----

thread 'login_env::tests::capture_login_environment_never_leaks_a_variable_set_only_in_the_callers_own_environment' (22988126) panicked at crates/server/src/login_env.rs:417:9:
assertion `left == right` failed: a variable set only in the calling process's own environment must not reach the captured login shell's environment
  left: Some("steg-server")
 right: None
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    login_env::tests::capture_login_environment_never_leaks_a_variable_set_only_in_the_callers_own_environment

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 123 filtered out; finished in 0.01s

error: test failed, to rerun pass `-p steg-server --lib`
```

Check passed: `cargo test -p steg-server --lib login_env::` exited 0 at 2026-11-16T04:48:03+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.87 on task-1.87-server-a-pane-s-environment-comes-from-a at ae10f11951f8 with uncommitted changes:

```text
   Compiling steg-server v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.87/crates/server)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.72s
     Running unittests src/lib.rs (target/debug/deps/steg_server-8563ad9cc9013408)

running 11 tests
test login_env::tests::fallback_env_omits_user_and_logname_when_the_identity_itself_has_none ... ok
test login_env::tests::fallback_env_is_exactly_the_named_constant_set ... ok
test login_env::tests::parse_env_dump_skips_a_malformed_entry_with_no_equals_sign ... ok
test login_env::tests::parse_env_dump_splits_a_value_containing_its_own_equals_sign_on_the_first_one_only ... ok
test login_env::tests::capture_login_environment_falls_back_on_a_shell_that_does_not_exist ... ok
test login_env::tests::resolve_login_identity_never_panics_and_always_names_a_real_shell_and_home ... ok
test login_env::tests::capture_login_environment_never_leaks_a_variable_set_only_in_the_callers_own_environment ... ok
test login_env::tests::capture_login_environment_sources_the_login_profile_not_the_interactive_one ... ok
test login_env::tests::capture_login_environment_falls_back_on_a_shell_that_exits_non_zero ... ok
test login_env::tests::capture_login_environment_prints_noise_before_the_marker_without_corrupting_the_dump ... ok
test login_env::tests::capture_login_environment_falls_back_on_a_shell_that_never_finishes_in_time ... ok

test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 113 filtered out; finished in 0.20s
```

Check FAILED: `cargo test -p steg-server --lib runtime::tests::steg_vars_and_term_always_win` exited 101 at 2026-11-16T04:50:32+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.87 on task-1.87-server-a-pane-s-environment-comes-from-a at ae10f11951f8 with uncommitted changes, last 40 of 45 lines:

```text
test runtime::tests::steg_vars_and_term_always_win_over_a_conflicting_value_from_the_login_shells_own_base_env ... FAILED

failures:

---- runtime::tests::steg_vars_and_term_always_win_over_a_conflicting_value_from_the_login_shells_own_base_env stdout ----

thread 'runtime::tests::steg_vars_and_term_always_win_over_a_conflicting_value_from_the_login_shells_own_base_env' (23151613) panicked at crates/server/src/runtime.rs:1217:9:
the server's own TERM must win over the login shell's own base_env: TERM=bogus-term-from-login-shell
STEG_SERVER=bogus-server-from-login-shell






















note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    runtime::tests::steg_vars_and_term_always_win_over_a_conflicting_value_from_the_login_shells_own_base_env

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 124 filtered out; finished in 0.02s

error: test failed, to rerun pass `-p steg-server --lib`
```

Check passed: `cargo test -p steg-server --lib` exited 0 at 2026-11-16T04:50:44+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.87 on task-1.87-server-a-pane-s-environment-comes-from-a at ae10f11951f8 with uncommitted changes, last 40 of 136 lines:

```text
test runtime::tests::login_shell_default_is_a_fixed_zero_io_value ... ok
test arbiter::tests::attempt_finds_a_real_live_listener_already_running ... ok
test paths::tests::a_real_plain_file_at_the_path_is_not_a_socket ... ok
test arbiter::tests::attempt_takes_over_a_stale_socket_left_by_a_dead_server ... ok
test runtime::tests::runtime_new_does_not_panic_and_starts_empty ... ok
test paths::tests::a_real_socket_we_own_is_ours_and_a_socket ... ok
test arbiter::tests::attempt_becomes_the_server_when_nothing_is_there ... ok
test login_env::tests::resolve_login_identity_never_panics_and_always_names_a_real_shell_and_home ... ok
test runtime::tests::login_shell_resolve_identity_never_panics_and_is_never_empty ... ok
test login_env::tests::capture_login_environment_never_leaks_a_variable_set_only_in_the_callers_own_environment ... ok
test login_env::tests::capture_login_environment_sources_the_login_profile_not_the_interactive_one ... ok
test runtime::tests::a_mark_for_a_pane_the_tree_no_longer_holds_does_nothing_and_does_not_panic ... ok
test runtime::tests::a_real_exit_event_drives_the_same_cascade_apply_event_runs ... ok
kill: 56751: No such process
test runtime::stop_tests::stop_is_a_no_op_for_a_pane_whose_program_has_already_ended ... ok
kill: 56747: No such process
test runtime::stop_tests::stop_ends_a_silent_program ... ok
test runtime::tests::apply_event_on_a_pane_the_tree_no_longer_holds_removes_the_stale_handle_without_panicking ... ok
test runtime::tests::apply_size_changed_lets_a_pane_go_under_the_floor_when_the_kernel_says_so ... ok
test runtime::tests::apply_size_changed_resizes_only_the_panes_the_change_names ... ok
test runtime::tests::disconnect_client_reapplies_the_next_latest_declared_size ... ok
test runtime::tests::pane_split_falls_back_to_home_when_the_inherited_live_cwd_no_longer_exists ... ok
test runtime::tests::steg_vars_and_term_always_win_over_a_conflicting_value_from_the_login_shells_own_base_env ... ok
test runtime::tests::pane_split_inherits_the_live_cwd_of_the_pane_it_split_from_not_its_started_in_cwd ... ok
test runtime::tests::a_typed_command_still_running_stays_running_until_told_otherwise ... ok
kill: 56750: No such process
test runtime::stop_tests::stop_ends_a_program_producing_output ... ok
test login_env::tests::capture_login_environment_falls_back_on_a_shell_that_exits_non_zero ... ok
test login_env::tests::capture_login_environment_prints_noise_before_the_marker_without_corrupting_the_dump ... ok
test login_env::tests::capture_login_environment_falls_back_on_a_shell_that_never_finishes_in_time ... ok
test runtime::tests::the_general_sizing_step_reaches_every_pane_through_split_close_and_swap ... ok
test runtime::tests::a_mark_then_the_shells_own_real_exit_cascades_with_no_panic ... ok
test runtime::tests::a_shell_with_no_marks_leaves_every_status_field_unchanged_until_one_arrives ... ok
test runtime::tests::a_typed_command_that_fails_is_failed_with_its_exit_code ... ok
test runtime::tests::a_typed_command_that_succeeds_is_running_then_finished_and_the_tab_shows_it ... ok
test runtime::tests::a_command_panes_own_program_printing_marks_does_not_change_its_status ... ok
kill: 56746: No such process
test pane::tests::a_forward_failing_once_the_worker_is_gone_still_reaps_a_program_that_ends_on_its_own ... ok

test result: ok. 125 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.02s
```

Check passed: `cargo test -p steg --features fixtures --test login_environment` exited 0 at 2026-11-16T04:55:43+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.87 on task-1.87-server-a-pane-s-environment-comes-from-a at ae10f11951f8 with uncommitted changes:

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.09s
     Running tests/login_environment.rs (target/debug/deps/login_environment-5d80ac678d8aeeb7)

running 1 test
test a_pane_started_by_a_real_on_demand_server_never_inherits_the_launching_clients_own_environment ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.39s
```

Check passed: `just check` exited 0 at 2026-11-16T05:03:15+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.87 on task-1.87-server-a-pane-s-environment-comes-from-a at ae10f11951f8 with uncommitted changes:

```text
ok      format check (0.2s)
ok      lint (0.6s)
ok      boundaries (0.1s)
ok      licences (0.8s)
ok      tests (every crate) (6.6s)
ok      build (steg-sdk) (0.1s)
ok      test (steg-sdk) (0.2s)
ok      test (steg-sdk, local-socket) (0.4s)
ok      build (steg-kernel) (0.1s)
ok      test (steg-kernel) (0.2s)
ok      build (steg-server) (0.1s)
ok      test (steg-server) (6.0s)
ok      test (steg-server, race-fixture) (4.8s)
ok      build (steg) (0.1s)
ok      test (steg) (0.1s)
ok      test (steg, fixtures) (2.7s)
ok      build (steg-ui) (0.3s)
ok      test (steg-ui) (0.3s)
ok      build (steg-app) (0.3s)
ok      test (steg-app) (0.3s)
ok      tool tests (11.4s)
ok      architecture (0.8s)
ok      recipe names (0.0s)
ok      ui levels (0.0s)
ok      ui colours (0.1s)
```

Check passed: `just check` exited 0 at 2026-11-16T05:10:12+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.87 on task-1.87-server-a-pane-s-environment-comes-from-a at be2feaf9dca4:

```text
ok      format check (0.3s)
ok      lint (0.5s)
ok      boundaries (0.1s)
ok      licences (0.8s)
ok      tests (every crate) (7.3s)
ok      build (steg-sdk) (0.1s)
ok      test (steg-sdk) (0.2s)
ok      test (steg-sdk, local-socket) (0.4s)
ok      build (steg-kernel) (0.1s)
ok      test (steg-kernel) (0.3s)
ok      build (steg-server) (0.1s)
ok      test (steg-server) (4.9s)
ok      test (steg-server, race-fixture) (5.0s)
ok      build (steg) (0.1s)
ok      test (steg) (0.1s)
ok      test (steg, fixtures) (2.9s)
ok      build (steg-ui) (0.3s)
ok      test (steg-ui) (0.4s)
ok      build (steg-app) (0.3s)
ok      test (steg-app) (0.3s)
ok      tool tests (11.5s)
ok      architecture (0.8s)
ok      recipe names (0.1s)
ok      ui levels (0.1s)
ok      ui colours (0.1s)
```

Check passed: `just check` exited 0 at 2026-11-16T05:14:05+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.87 on task-1.87-server-a-pane-s-environment-comes-from-a at cc3e1f0503a2:

```text
ok      format check (0.2s)
ok      lint (0.4s)
ok      boundaries (0.1s)
ok      licences (0.7s)
ok      tests (every crate) (5.2s)
ok      build (steg-sdk) (0.1s)
ok      test (steg-sdk) (0.2s)
ok      test (steg-sdk, local-socket) (0.4s)
ok      build (steg-kernel) (0.1s)
ok      test (steg-kernel) (0.2s)
ok      build (steg-server) (0.1s)
ok      test (steg-server) (6.0s)
ok      test (steg-server, race-fixture) (6.1s)
ok      build (steg) (0.1s)
ok      test (steg) (0.1s)
ok      test (steg, fixtures) (2.8s)
ok      build (steg-ui) (0.3s)
ok      test (steg-ui) (0.3s)
ok      build (steg-app) (0.3s)
ok      test (steg-app) (0.3s)
ok      tool tests (11.4s)
ok      architecture (0.7s)
ok      recipe names (0.0s)
ok      ui levels (0.0s)
ok      ui colours (0.0s)
```
<!-- SECTION:NOTES:END -->

## Comments

<!-- COMMENTS:BEGIN -->
author: @bruk
created: 2026-11-16T04:22:00Z

---

Plan approved by @bruk (03e926f72daf, size-3): its worker may build it.

---
<!-- COMMENTS:END -->
