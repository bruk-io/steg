---
id: TASK-1.87
title: >-
  Server: a pane's environment comes from a login shell, not from the client
  that started the server
status: In Review
assignee:
  - '@bruk.w1'
created_date: '2026-11-15T23:40:00Z'
updated_date: '2026-11-16T06:30:00Z'
labels:
  - size-3
milestone: m-3
dependencies:
  - TASK-1.77
references:
  - task-1.87-server-a-pane-s-environment-comes-from-a
  - https://github.com/bruk-io/steg/pull/56
parent_task_id: TASK-1
project: bruk-io-steg
asdlc_accepted_from: DRAFT-2
asdlc_branch: task-1.87-server-a-pane-s-environment-comes-from-a
asdlc_plan_approved_by: bruk
asdlc_plan_approved_hash: '03e926f72daf'
asdlc_plan_approved_size: size-3
asdlc_review_kind: same-machine
asdlc_review_findings: 'important=1,nit=4'
asdlc_review_policies: ["review", "testing"]
asdlc_plan_matched: no
asdlc_reviewer: bruk.r2
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

Built exactly as planned, with the changes below noted since they departed from
the plan's own text (plan section left unedited, per instruction).

## Coverage, clause by clause

AC #1 (environment from a login shell, not the client; shell and command pane;
real on-demand server, real client): proven two ways. End to end, through the
real binary, a real on-demand server (autostart) and a real client fixture
(crates/steg/tests/login_environment.rs + tests/fixtures/env_leak_client.rs):
a unique marker set only in the launching client's own environment is absent
from the pane, and the pane's own PATH carries a path_helper-specific entry
(/System/Cryptexes/App/usr/bin) that only a real capture produces, never the
fallback - proving a real capture happened, not merely "didn't crash". The
shell-pane half is proven separately and directly, not only by construction,
at the Runtime level (runtime.rs's a_shell_pane_not_only_a_command_pane_gets_
the_same_composed_environment): a session.new with no program at all, real
input sent to the real login shell, same TERM override confirmed.

One stated departure, documented in login_environment.rs's own module doc:
identity resolution reads the real password database by design (R99 forbids
$SHELL/$HOME from the environment), so this end-to-end test cannot redirect it
to a fabricated shell/home the way login_env.rs's own unit tests do. Mutating
a real account's own ~/.zprofile (even temporarily) was rejected outright - a
crashed test would leave a developer's shell configuration corrupted. Seeding
ZDOTDIR was explored and confirmed empirically to work (zsh honours it for
dotfile lookup while leaving $HOME itself untouched) but rejected too: the
only way to reach it from the real binary is either an ambient-env passthrough
in production code (itself a new leak vector - a client could point ZDOTDIR
at an attacker-controlled directory) or a test-only hook in the shipped
binary (this team's own convention disfavours one). The throwaway-home-
specific proof - that a login shell's own start-up *file* is sourced, not an
interactive one - stays at the unit level instead
(capture_login_environment_sources_the_login_profile_not_the_interactive_one),
with a fabricated shell/home pair no production code path can reach ambiently.

AC #2 (bounded; stated fallback; never the client's environment unscrubbed;
every child waited on): two separate bounds (login_identity_timeout, 2s;
login_environment_timeout, 3s), proven in login_env.rs's own tests with a real
fake shell that hangs/exits non-zero/exits missing - each falls back to a
named, fixed environment built from the identity alone (fallback_env), never
derived from the server's own inherited environment (a deliberate, stated
departure from TASK-1.44's stale draft, which fell back to "inherited minus
STEG_*" - still a leak). Every child is waited on: the reader thread owns the
whole Child and does read-then-wait as one unit sent over one channel, so one
recv_timeout bounds both; on timeout the caller kills by pid (via a plain
`kill` subprocess, the same technique crates/steg/tests/server_start.rs
already uses, not a new nix feature) and the abandoned thread still reaps it.

AC #3 (STEG_* and TERM set by server regardless of login shell; cwd rule
unchanged): proven with a fabricated base_env exporting conflicting TERM/
STEG_SERVER values - the server's own override always wins
(steg_vars_and_term_always_win_over_a_conflicting_value_from_the_login_shells_
own_base_env), with its own negative control (ordering reversed, recorded
failing, reverted). TERM=xterm-256color, matching this repository's own
existing precedent (spikes/task-1.2-wire-protocol/probe-server/src/pty.rs),
not invented - ghostty-vt's own xterm-ghostty terminfo entry is flagged in
runtime.rs's own doc comment as a better-but-larger follow-up, out of this
task's scope (would need writing a terminfo cache directory and setting
TERMINFO). resolve_default_cwd (rule B) is untouched.

AC #4 (the plan weighs the three ways with reasons; what each means for a
command pane): weighed in the plan itself (capture once, chosen; routing every
pane through a login shell, rejected - repeated profile-sourcing cost and a
new shell-quoting surface neither needed before); the "command pane needs
nothing different" half is proven by construction (launch_spec's own env
composition never branches on Program::Shell vs Program::Command) and now
also directly, per the shell-pane test above.

## Departures from the plan's own text

- capture_login_environment's timeout handling changed from "kill the Child
  directly on the caller's stack" (what the plan described) to "the reader
  thread owns the whole Child, doing read-then-wait as one unit; on timeout
  the caller kills by pid via a plain `kill` subprocess." The plan's own
  version had a real bug: wait() on the caller's side, after a successful
  read, was unbounded (a child that backgrounds a process of its own could
  let the read finish before the shell itself exits). Caught before building
  against it, not after.
- Capture::Fallback now carries the built environment (Fallback(reason, env))
  instead of callers reconstructing a LoginIdentity by hand to call
  fallback_env a second time - avoids a round trip where a future identity
  field could silently be dropped.
- The two new timeout constants (DEFAULT_LOGIN_IDENTITY_TIMEOUT,
  DEFAULT_LOGIN_ENVIRONMENT_TIMEOUT) live in config.rs, matching
  DEFAULT_HANDSHAKE_TIMEOUT/DEFAULT_ARBITRATION_TIMEOUT's own existing
  placement, not in login_env.rs as an earlier draft had them.
- crates/server/tests/program.rs needed no PATH fix: it only ever drives a
  FakeLauncher, never RealLauncher, so env_clear() does not touch it -
  verified by running it, not assumed; removed from the plan's own file list
  in substance (the plan listed it; it did not need touching).
- crates/steg/Cargo.toml's serde_json ended up in [dependencies], not
  [dev-dependencies] as first drafted: a fixture is a [[bin]] target, which
  only ever sees [dependencies]. boundaries.toml's [steg] entry updated to
  allow it, with the reason stated there.
- One test added beyond the plan's own proof list: the shell-pane half of
  AC #1/#4, found worth proving directly rather than leaving it to
  construction alone (above).

## Checks recorded

Every check below is on the task, each against the commit/branch state at the
time: `cargo test -p steg-server --lib login_env::` (new module, standalone);
the pre-fix failing runs this task's own env_clear() change caused
(`cargo test -p steg-server --lib` exit 101, `--test real_pty` exit 101,
naming "Unable to resolve the PATH") and the fixed, passing re-runs; the three
negative controls (launcher env_clear() removed; capture's own env_clear()
removed; override ordering reversed), each applied as a scratch edit, run
failing, reverted, re-run passing; `cargo test -p steg --features fixtures
--test login_environment` (the end-to-end proof); `just check`, final run at
the pushed, CI-green head (cc3e1f0).

PR #56, CI green at the final head. `crates/steg/tests/server_start.rs`'s own
suite (the leftover-process discipline) still passes unchanged - no new
leftover server or output file across repeated runs.

Check passed: `cargo test -p steg-server --lib login_env::` exited 0 at 2026-11-16T05:44:36+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.87 on task-1.87-server-a-pane-s-environment-comes-from-a at cc3e1f0503a2 with uncommitted changes:

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.08s
     Running unittests src/lib.rs (target/debug/deps/steg_server-8563ad9cc9013408)

running 12 tests
test login_env::tests::fallback_env_is_exactly_the_named_constant_set ... ok
test login_env::tests::fallback_env_omits_user_and_logname_when_the_identity_itself_has_none ... ok
test login_env::tests::parse_env_dump_skips_a_malformed_entry_with_no_equals_sign ... ok
test login_env::tests::parse_env_dump_splits_a_value_containing_its_own_equals_sign_on_the_first_one_only ... ok
test login_env::tests::capture_login_environment_falls_back_on_a_shell_that_does_not_exist ... ok
test login_env::tests::resolve_login_identity_never_panics_and_always_names_a_real_shell_and_home ... ok
test login_env::tests::capture_login_environment_sources_the_login_profile_not_the_interactive_one ... ok
test login_env::tests::capture_login_environment_never_leaks_a_variable_set_only_in_the_callers_own_environment ... ok
test login_env::tests::capture_login_environment_falls_back_on_a_shell_that_never_finishes_in_time ... ok
test login_env::tests::capture_login_environment_falls_back_on_a_shell_that_exits_non_zero ... ok
test login_env::tests::capture_login_environment_prints_noise_before_the_marker_without_corrupting_the_dump ... ok
kill: 72252: No such process
test login_env::tests::capture_login_environment_kills_a_backgrounded_grandchild_that_held_the_pipe_open ... ok

test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 115 filtered out; finished in 0.31s
```

Commits since the review: 0.

Check passed: `sh -c 'rm -f target/debug/fixture_server target/debug/env_leak_client; cargo build -p steg && test ! -f target/debug/fixture_server && test ! -f target/debug/env_leak_client && echo "fixture_server/env_leak_client absent from a default build, as required"'` exited 0 at 2026-11-16T05:46:56+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.87 on task-1.87-server-a-pane-s-environment-comes-from-a at cc3e1f0503a2 with uncommitted changes:

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.09s
fixture_server/env_leak_client absent from a default build, as required
```

Commits since the review: 0.

Check FAILED: `just check` exited 101 at 2026-11-16T05:50:43+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.87 on task-1.87-server-a-pane-s-environment-comes-from-a at cc3e1f0503a2 with uncommitted changes, last 40 of 68 lines:

```text
test runtime::tests::apply_event_on_a_pane_the_tree_no_longer_holds_removes_the_stale_handle_without_panicking ... ok
test runtime::tests::apply_size_changed_lets_a_pane_go_under_the_floor_when_the_kernel_says_so ... ok
test runtime::tests::apply_size_changed_resizes_only_the_panes_the_change_names ... ok
test runtime::tests::a_shell_pane_not_only_a_command_pane_gets_the_same_composed_environment ... ok
test runtime::tests::disconnect_client_reapplies_the_next_latest_declared_size ... ok
test runtime::tests::steg_vars_and_term_always_win_over_a_conflicting_value_from_the_login_shells_own_base_env ... ok
test runtime::tests::pane_split_falls_back_to_home_when_the_inherited_live_cwd_no_longer_exists ... ok
test runtime::tests::pane_split_inherits_the_live_cwd_of_the_pane_it_split_from_not_its_started_in_cwd ... ok
test runtime::tests::a_typed_command_still_running_stays_running_until_told_otherwise ... ok
kill: 7106: No such process
test runtime::stop_tests::stop_ends_a_program_producing_output ... ok
test login_env::tests::capture_login_environment_falls_back_on_a_shell_that_never_finishes_in_time ... ok
test runtime::tests::the_general_sizing_step_reaches_every_pane_through_split_close_and_swap ... ok
test login_env::tests::capture_login_environment_falls_back_on_a_shell_that_exits_non_zero ... ok
test login_env::tests::capture_login_environment_prints_noise_before_the_marker_without_corrupting_the_dump ... ok
test runtime::tests::a_mark_then_the_shells_own_real_exit_cascades_with_no_panic ... ok
test runtime::tests::a_shell_with_no_marks_leaves_every_status_field_unchanged_until_one_arrives ... ok
test runtime::tests::a_typed_command_that_succeeds_is_running_then_finished_and_the_tab_shows_it ... ok
test runtime::tests::a_typed_command_that_fails_is_failed_with_its_exit_code ... ok
test runtime::tests::a_command_panes_own_program_printing_marks_does_not_change_its_status ... ok
kill: 7103: No such process
test pane::tests::a_forward_failing_once_the_worker_is_gone_still_reaps_a_program_that_ends_on_its_own ... ok
test login_env::tests::capture_login_environment_kills_a_backgrounded_grandchild_that_held_the_pipe_open ... FAILED

failures:

---- login_env::tests::capture_login_environment_kills_a_backgrounded_grandchild_that_held_the_pipe_open stdout ----

thread 'login_env::tests::capture_login_environment_kills_a_backgrounded_grandchild_that_held_the_pipe_open' (25078114) panicked at crates/server/src/login_env.rs:635:13:
the backgrounded grandchild's own pid file never appeared


failures:
    login_env::tests::capture_login_environment_kills_a_backgrounded_grandchild_that_held_the_pipe_open

test result: FAILED. 126 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 10.01s

error: test failed, to rerun pass `-p steg-server --lib`
full output: .quiet/tests-every-crate.log
error: recipe `test` failed with exit code 101
```

Commits since the review: 0.

Check passed: `just check` exited 0 at 2026-11-16T05:54:13+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.87 on task-1.87-server-a-pane-s-environment-comes-from-a at cc3e1f0503a2 with uncommitted changes:

```text
ok      format check (0.2s)
ok      lint (0.4s)
ok      boundaries (0.1s)
ok      licences (0.7s)
ok      tests (every crate) (6.6s)
ok      build (steg-sdk) (0.1s)
ok      test (steg-sdk) (0.2s)
ok      test (steg-sdk, local-socket) (0.4s)
ok      build (steg-kernel) (0.1s)
ok      test (steg-kernel) (0.2s)
ok      build (steg-server) (0.1s)
ok      test (steg-server) (4.7s)
ok      test (steg-server, race-fixture) (4.9s)
ok      build (steg) (0.1s)
ok      test (steg) (0.1s)
ok      test (steg, fixtures) (2.9s)
ok      build (steg-ui) (0.3s)
ok      test (steg-ui) (0.3s)
ok      build (steg-app) (0.3s)
ok      test (steg-app) (0.2s)
ok      tool tests (11.4s)
ok      architecture (0.7s)
ok      recipe names (0.0s)
ok      ui levels (0.0s)
ok      ui colours (0.0s)
```

Commits since the review: 0.

Check passed: `sh -c '
echo "process count before:"; ps aux | grep -iE "target/(debug|release)/(steg|fixture_server|env_leak_client|autostart_client|already_a_group_leader)\b" | grep -v grep | wc -l
fail=0
for i in $(seq 1 10); do cargo test -p steg-server --quiet 2>&1 | grep -q "FAILED" && { echo "server run $i FAILED"; fail=1; } || echo "server run $i ok"; done
for i in $(seq 1 10); do cargo test -p steg --features fixtures --quiet 2>&1 | grep -q "FAILED" && { echo "steg run $i FAILED"; fail=1; } || echo "steg run $i ok"; done
echo "process count after:"; ps aux | grep -iE "target/(debug|release)/(steg|fixture_server|env_leak_client|autostart_client|already_a_group_leader)\b" | grep -v grep | wc -l
exit $fail
'` exited 0 at 2026-11-16T06:00:00+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.87 on task-1.87-server-a-pane-s-environment-comes-from-a at cc3e1f0503a2 with uncommitted changes:

```text
process count before:
       0
server run 1 ok
server run 2 ok
server run 3 ok
server run 4 ok
server run 5 ok
server run 6 ok
server run 7 ok
server run 8 ok
server run 9 ok
server run 10 ok
steg run 1 ok
steg run 2 ok
steg run 3 ok
steg run 4 ok
steg run 5 ok
steg run 6 ok
steg run 7 ok
steg run 8 ok
steg run 9 ok
steg run 10 ok
process count after:
       0
```

Commits since the review: 0.

Check passed: `just check` exited 0 at 2026-11-16T06:07:43+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.87 on task-1.87-server-a-pane-s-environment-comes-from-a at 6e1082ef7c7f:

```text
ok      format check (0.2s)
ok      lint (0.7s)
ok      boundaries (0.1s)
ok      licences (0.8s)
ok      tests (every crate) (8.5s)
ok      build (steg-sdk) (0.1s)
ok      test (steg-sdk) (0.9s)
ok      test (steg-sdk, local-socket) (0.9s)
ok      build (steg-kernel) (0.1s)
ok      test (steg-kernel) (0.6s)
ok      build (steg-server) (0.2s)
ok      test (steg-server) (6.0s)
ok      test (steg-server, race-fixture) (6.4s)
ok      build (steg) (0.1s)
ok      test (steg) (0.2s)
ok      test (steg, fixtures) (3.8s)
ok      build (steg-ui) (0.3s)
ok      test (steg-ui) (0.6s)
ok      build (steg-app) (0.3s)
ok      test (steg-app) (0.4s)
ok      tool tests (11.7s)
ok      architecture (0.7s)
ok      recipe names (0.0s)
ok      ui levels (0.0s)
ok      ui colours (0.1s)
```

Commits since the review: 0.

Every finding addressed. PR #56 updated (cc3e1f0 -> 6e1082e), CI green at the
final head.

## Important: the end-to-end test no longer reads this machine's real identity

Added `crates/steg/tests/fixtures/fixture_server.rs`, a fourth `fixtures`
-gated [[bin]] following the exact convention `already_a_group_leader`/
`autostart_client`/`env_leak_client` already established: the real server
code (attempt/detach/build_login_shell/run), given a LoginIdentity from its
own environment (STEG_TEST_LOGIN_SHELL/_HOME/_USER) instead of
resolve_login_identity's real password-database lookup. Factored the shared
"capture, then report a fallback" step into steg_server::build_login_shell
(new, exported) so main.rs and the fixture call the same code - this also
closed a small round-trip risk the earlier version had (reconstructing a
LoginIdentity by hand from LoginShell's own fields just to call
capture_login_environment).

login_environment.rs now builds a throwaway home with its own .zprofile
(exporting a marker and a PATH addition), points the fixture client at
fixture_server instead of the real steg binary, and asserts on: the
throwaway profile's own marker and PATH addition being present (proving a
real capture happened, not the fallback - specific to this test's own
profile, nothing about this machine), a variable set only in the launching
client's own environment being absent, and TERM=xterm-256color (AC #3).
Nothing in the test depends on this machine's real account, real shell, or
real .zprofile.

Proved the fixture is actually absent from the shipped binary, not just
inert: recorded `rm -f target/debug/{fixture_server,env_leak_client}; cargo
build -p steg && test ! -f target/debug/fixture_server && test ! -f
target/debug/env_leak_client` - exits 0, confirming a default build produces
neither.

Kept one small test of real identity resolution alone, strengthened per the
request: resolve_login_identity_never_panics_and_returns_a_home_a_shell_and_
a_user_that_exist now checks the returned shell is a real file, the home a
real directory, and the user non-empty - not just non-empty strings.

## Nits

(a) capture_login_environment now spawns the shell in its own process group
(.process_group(0)) and kills the whole group on timeout (kill -9 -<pgid>),
not just the shell's own pid. Reproduced the exact finding first (a fake
shell backgrounding a grandchild that holds stdout open, then exits) and
confirmed the bug: without the group kill, the grandchild (confirmed alive
via a pid file + repeated kill -0) is never reaped and the reader thread
stays blocked on it indefinitely. New test:
capture_login_environment_kills_a_backgrounded_grandchild_that_held_the_pipe_
open, asserting both halves (the function still returns within its own
bound, and the grandchild is actually gone shortly after). Documented in
capture_login_environment's own doc comment, including the one remaining,
accepted gap stated plainly: a profile that deliberately re-parents its own
background job into a different process group would still outlive the
bound.

(b) The capture marker is now generated fresh per call (capture_marker():
pid + a monotonic counter + the current time) instead of a fixed constant,
so a profile that happens to echo the same literal text can no longer
corrupt parse_env_dump's own search for it - closed by construction rather
than by searching for the last match instead. parse_env_dump now takes the
marker as a parameter. The "noise before the marker" test now has its fake
shell actually eval the real -c argument capture_login_environment passes
(echo banner; eval "$3") instead of hardcoding what used to be a fixed
global marker, so it exercises the real, now-dynamic mechanism rather than a
stand-in for it.

(c) The .zshrc-not-read limit is now documented in login_env.rs's own module
doc (a new paragraph, with the engineer's own confirmation referenced) and
in crates/server/CLAUDE.md, not only in board comments.

(d) Traced the flaky run: capture_login_environment_prints_noise_before_the_
marker_without_corrupting_the_dump (and its neighbours) used a 5s/10s bound
with no particular reason to be tight - these test parsing correctness, not
timing. Widened to 20s, matching this crate's own EVENT_TIMEOUT convention
elsewhere. Left capture_login_environment_falls_back_on_a_shell_that_never_
finishes_in_time's own 200ms bound alone - that one is testing the timeout
path itself, so a short bound is the point.

## Found and fixed one more race while stress-testing this round's own fixes

env_leak_client's shell wrote its four-line output (TERM/STEG_SERVER/PATH/
MARK_ZPROFILE) with a plain `> output_path` redirect. Stress-testing (10x
loops) caught the test's own poll occasionally reading the file mid-write
under load - two lines landed, two hadn't yet. Fixed by writing to a
temporary path and renaming onto output_path, which is atomic on the same
filesystem - the poll now only ever sees "absent" or "complete," never
"partial." Confirmed with 15 additional isolated runs and the 10x sweep
below, all clean.

## Checks recorded

login_env.rs's own module in isolation (new/changed tests); the negative
control for the grandchild fix was the bug reproduction itself, recorded
failing before the process-group fix landed (not a separate scratch-edit-
and-revert, since this was a real fix to a real finding, not a mechanism
already built); `just check`, twice (once mid-fix, once at the final
pushed head); the "fixture absent from a default build" check above; the
10x sweep: `cargo test -p steg-server` x10 and `cargo test -p steg
--features fixtures` x10, with the real-process count (grep for steg/
fixture binaries in `ps aux`) recorded at 0 both before and after - no
leftover process from any of the 20 runs.

## Overlap with PR #47 (TASK-1.78)

Still open (checked `gh pr view 47` again just before this finish) - no
merge to resolve yet. If it lands before this is reviewed again, the one
shared line is unchanged from the last review's own note: listen.rs's
Runtime::new(...) call (47 adds a third force_grace argument; this task
only changes what config.login.clone() carries) - orthogonal, resolved by
keeping both.
<!-- SECTION:NOTES:END -->

## Comments

<!-- COMMENTS:BEGIN -->
author: @bruk
created: 2026-11-16T04:22:00Z

---

Plan approved by @bruk (03e926f72daf, size-3): its worker may build it.

---

author: @bruk.r2
created: 2026-11-16T05:19:00Z

---

@bruk.r2 is reviewing TASK-1.87: a same-machine review: it waited 0m with no other engineer's reviewer, longer than review_fallback_after (0m), so a fresh reviewer on its author's machine takes it.

---

author: @bruk.r2
created: 2026-11-16T05:35:00Z

---

Review by @bruk.r2 (same-machine): changes requested. Back to In Progress for the author's worker.

Findings: 1 Important, 4 nit. Cites: review, testing. The diff does not match the plan.

Head reviewed: cc3e1f0503a26cd0fbb1ee300278afb1aa940fb8 (PR #56). Task's own final `just check` passed at this exact head; PR #56's CI ("check") is SUCCESS at the same head.

[Bugs, Important] crates/steg/tests/login_environment.rs + tests/fixtures/env_leak_client.rs + login_env::resolve_login_identity: the only end-to-end proof of AC #1 resolves the real password-database identity, so the real server it autostarts runs this developer's/CI runner's actual real login shell against their actual real ~/.zprofile (and /etc/zprofile) - directly reading this machine's own start-up files, which the orchestrator's brief said tests must never do. The test's own outcome therefore depends on the content and speed of a real person's shell configuration: a slow personal .zprofile step (a network check, a prompt, a direnv/asdf/nvm hook) can push the capture past its 3s bound and silently flip to the fixed fallback, failing the test's own "/System/Cryptexes/App/usr/bin" assertion for reasons unrelated to any regression; a customised PATH that strips that macOS path_helper entry, or any non-zsh login shell (which the plan itself names as an accepted *production* gap), would do the same. The module doc (login_environment.rs) weighs and rejects two alternatives (mutating the real ~/.zprofile; ZDOTDIR via ambient env) but never considers the one the crate's own convention already supports: crates/steg/Cargo.toml already ships three `fixtures`-feature-gated [[bin]] fixtures (already_a_group_leader, autostart_client, and this task's own env_leak_client), compiled out of the shipped binary entirely (fixtures = [], off by default) - confirmed by reading that file. A fixture-only entry point letting ServerConfig/run_server_start accept a pre-resolved LoginIdentity pointing at a throwaway home (with its own controlled .zprofile) follows that exact, already-accepted convention, never reaches the real shipped binary a client uses, and removes this test's dependence on whoever happens to run it. The worker's stated reason for rejecting "a test-only hook in the shipped binary" does not apply to this narrower, already-established pattern, which was never evaluated.

[Bugs, Nit] crates/server/src/login_env.rs - capture_login_environment: tried myself with a fake shell that backgrounds a grandchild holding stdout open then exits (`(sleep 20)&; echo marker; exec env -0`). Confirmed empirically: the function still returns within its own bound (~312ms against a 300ms timeout), but the grandchild is left running - reparented to pid 1, still alive 2s later per `ps` - and the capture's own reader thread stays blocked in read_to_end (with the direct child left as a zombie) until the grandchild exits on its own, unbounded by this function's own timeout. Not tested, and not stated where the mechanism is documented (the module doc explains only why the thread owns the whole Child, never this consequence).

[Bugs, Nit] crates/server/src/login_env.rs:227 parse_env_dump: finds the capture's own marker with `.position()` (first match), not the last. A start-up script whose own noise happens to echo a line equal to ENV_CAPTURE_MARKER before the capture's real marker would make the dump start parsing from the wrong, earlier point, silently corrupting the result rather than failing loudly. Low probability, easily made robust by searching from the end instead.

[Compliance, Nit] The known, engineer-accepted limit (capturing via `-l` without `-i` means .zshrc is never read, so a command pane gets nothing a person set only there - many people set PATH/tool managers in .zshrc) is recorded only in the task's own board comments (bruk.o's plan-check note), never in the code's own documentation where the mechanism and a pane's environment are actually described - login_env.rs's module doc states only the "no tty" reason for -l without -i, never this consequence. Checked: doc-2 section 4 and doc-8 R99 (both read from the board) don't carry this detail either, which is fine for those (spec-level, engineer/orchestrator-only) - but nowhere in the repository states it.

[Bugs/Evidence, Nit] Ran the server+steg suites 10 times at default parallelism (cargo test -p steg-server; cargo test -p steg --features fixtures), checking `ps` for leftover processes before/after: one run (6 of 10) failed login_env::tests::capture_login_environment_prints_noise_before_the_marker_without_corrupting_the_dump with Fallback(TimedOut, ...) - confirmed via `ps` that this coincided with heavy, unrelated concurrent compilation from a different review worktree on the same machine, not a leftover from this suite. Re-run 50/50 times in isolation (--test-threads=1, nothing else building): always passed. No leftover zsh/sh/env/sleep processes found after any run; the only post-run processes matching the filter belonged to an unrelated worktree's build. Slowest combined run: 10s. Not a logic bug, but the fixed 5s/10s bounds several login_env.rs tests use leave real margin risk under genuine CI/dev-machine contention.

Repeated myself, as scratch edits immediately reverted: (1) removed RealLauncher::spawn's env_clear() in pty.rs - the leak test (a_real_programs_environment_never_leaks_this_processs_own_ambient_variables) failed as claimed, reverted, passes again. (2) reversed the base_env/overrides ordering in Runtime::launch_spec - steg_vars_and_term_always_win_over_a_conflicting_value_from_the_login_shells_own_base_env failed as claimed (the fake shell's TERM/STEG_SERVER won), reverted, passes again.

Everything else checked out: the ~20 existing Runtime::new/LaunchSpec call sites fixed via test_base_env()/login()/login_for_marks() helpers, no expectation weakened; no `unsafe` anywhere in the diff; no new dependency/feature (serde_json moved to [dependencies] only for the fixture [[bin]], boundaries.toml updated with the reason stated); ServerConfig's two new timeouts have defaults; TERM=xterm-256color; capture happens once after bind, before accept (crates/steg/src/main.rs run_server_start), matching the plan and now reachable end-to-end since TASK-1.45 merged (listen::run builds the real Runtime from config.login). Overlap with PR #47 (TASK-1.78): the one shared line is listen.rs's Runtime::new(...) call - #47 adds a third `force_grace` argument, this task only changes what config.login.clone() now carries (richer LoginShell with user/base_env) - orthogonal in meaning, resolved by keeping both (three-arg call with the TASK-1.87-shaped LoginShell). #47's runtime.rs diff only changes Runtime::new's own signature/body (the grace field) and adds force_end/stop-test helpers; it never touches LoginShell's fields or launch_spec's environment composition, so there's no deeper semantic conflict beyond the many colliding call sites both tasks happen to touch in the shared test module - mechanical, not a design clash.

plan-matched: no - the notes themselves record six stated departures from the plan's own text (the read-then-wait combining into one unit on timeout, Capture::Fallback carrying the built env, the two timeout constants living in config.rs not login_env.rs, program.rs needing no fix, serde_json in [dependencies] not [dev-dependencies], and the extra shell-pane test beyond the plan's own proof list) - each is explained, none is itself a finding, but the flag reflects the departure regardless.

---

created: 2026-11-16T06:29:00Z

---

Merged PR #47's reviewed head (3e4594e) into this branch, as asked, before
the second review starts. New head: ed80ab8 (pushed, PR #56, CI green).

## The three conflicting hunks, and how each was resolved

1. `crates/server/src/lib.rs` - both sides added a `pub use runtime::{...}`
   line naming different new symbols (mine: `build_login_shell`; #47's:
   `FORCE_GRACE_DEFAULT`). Kept both in one line.

2. `crates/server/src/listen.rs` - both sides touched the same
   `Runtime::new(...)` call in `run()`. Mine added a doc comment above it
   explaining why `config.login.base_env` is assumed already resolved by
   that point; #47's added the third `force_grace` argument
   (`crate::runtime::FORCE_GRACE_DEFAULT`). Kept my comment, used #47's
   3-argument call underneath it - the comment's own claim (this crate
   prints nothing, a fallback is reported by the caller) is still true
   after the merge.

3. `crates/server/src/runtime.rs`, two hunks in `mod tests`/`mod
   stop_tests` - both sides added a helper/constant right after the same
   line (`EVENT_TIMEOUT`/`POLL_TIMEOUT`). Mine: `test_base_env()` (a
   working `PATH`, needed once `RealLauncher::spawn`'s `.env_clear()`
   landed). #47's: `TEST_GRACE` (a short `force_grace` for `Runtime::new`'s
   new third argument). Kept both, in the order each module already used
   them.

## Hand edits outside the conflicts, and why

Found by running the suites, not by the compiler - these are the kind
`git` does not flag, since each is a *semantically* broken literal that
stayed *syntactically* valid after either branch's own change:

- `crates/server/tests/runtime.rs`: seven of `#47`'s own new force-close
  tests each built their own `LoginShell { argv0: "/bin/sh".to_string(),
  home: "/tmp".to_string() }` literal - the two-field shape from before
  this task added `user`/`base_env`. Compiled as a type error (missing
  fields), so the compiler *did* catch this half. Fixed by factoring one
  shared `login_for_force_tests()` helper (with a working `PATH`, same
  reasoning `runtime()`'s own helper in this file already has) and
  pointing all seven at it, rather than fixing each literal by hand -
  smaller diff, one place to find next time this shape changes again.

- `crates/server/src/pane.rs`: a second `real_spec` helper, in `#47`'s own
  new `mod ending_tests`, still had `env: Vec::new()` - the pre-TASK-1.87
  meaning (RealLauncher ambiently inherited the test process's own
  environment, which does not happen any more). This is exactly what the
  compiler could *not* catch: a `Vec<(OsString, OsString)>` is a valid
  value whether or not it carries a working `PATH`, so nothing failed to
  build - it would have failed to *run*, the first time a test using it
  actually needed to resolve a relative command name against an empty
  environment. Fixed the same way `mod tests`'s own `real_spec` in this
  file was already fixed for TASK-1.87. One test in `mod ending_tests`
  also built a `LaunchSpec` literal directly (for a `.restart()` call)
  with the same `env: Vec::new()` - fixed the same way, inline.

Confirmed the fix was real, not incidental: before fixing
`ending_tests::real_spec` and the inline literal, both of this module's
own tests that actually run the restarted/relayed program through
`RealLauncher` would have failed the instant that program needed to
resolve `"sh"` with no `PATH` at all (the identical failure mode this
task's own earlier rounds already found and fixed in `crates/server/
tests/real_pty.rs`/`routing.rs`) - caught here by running the suite
*after* the merge, which is why this needed finding rather than being
flagged.

## Checked for the two interaction risks named, found none

- **A shared process-group-kill path with #47's own forced close.**
  `crate::signal::send` (#47, `killpg` on a *pane's* pid, used by
  `PaneHandle::signal_hangup`/`signal_kill`, driven by `Runtime::
  force_end`) and this task's `login_env::capture_login_environment`
  (`.process_group(0)` + `kill -9 -<pgid>` on the *login-shell capture's
  own, unrelated* child, spawned once at server start, before any pane
  exists) are structurally independent: different process trees,
  different trigger points, no shared code, no shared constant, no
  possible pid/pgid collision (the OS never reuses a live pgid). `pty.rs`
  itself (where a *pane's* process actually starts) was untouched by #47
  at all - confirmed by diffing it between the two branches' common
  ancestor and #47's own head, finding no change.

- **Reads of a renamed thing.** `PaneHandle::stop` was removed by #47
  (replaced by `signal_hangup`/`signal_kill`/`await_exit` through
  `force_end`); nothing in this task's own code or comments called it -
  `Runtime::stop` (a different method, unchanged in name, now calling
  `force_end` internally per #47's own rewrite) is the only "stop" this
  task's own code or comments ever named. `STOP_TIMEOUT` (removed by #47)
  is referenced once, in a doc comment inside `#47`'s own rewritten
  `Runtime::stop` doc, as history ("this used to be serial... 5s per
  pane") - not a symbol this task's code reads, and the file still
  compiles.

## What was run

`cargo build --workspace --all-targets --all-features` (clean);
`cargo test -p steg-server --all-features` and `--lib` separately (one
flake, `arbiter::tests::attempt_takes_over_a_stale_socket_left_by_a_dead_
server`, failed once under the full lib suite's own parallelism, then
passed in isolation and 5/5 more full-suite runs after - a pre-existing,
socket-path-collision-shaped flake in a file neither this task nor #47
touches, not a regression from this merge, named here rather than
silently reasoned away); `cargo test -p steg --features fixtures`;
`just check` (clean, twice - once before, once after the committed
merge); the 10x sweep (`cargo test -p steg-server` x10, `cargo test -p
steg --features fixtures` x10) with the real-process count at 0 both
before and after - all 20 runs green, repeated once more at the exact
committed, pushed head (ed80ab8) for the record below.

Did not run `asdlc record-check`/`asdlc finish` - the task is not held by
this worker for that purpose any more, per the brief for this step.

---

author: @bruk.r2
created: 2026-11-16T06:30:00Z

---

@bruk.r2 is reviewing TASK-1.87: a same-machine review: it waited 21m with no other engineer's reviewer, longer than review_fallback_after (0m), so a fresh reviewer on its author's machine takes it.

---
<!-- COMMENTS:END -->
