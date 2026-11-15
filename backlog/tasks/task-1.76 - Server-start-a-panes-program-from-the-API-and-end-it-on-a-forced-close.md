---
id: TASK-1.76
title: 'Server: start a pane''s program from the API, and size it'
status: In Progress
assignee:
  - '@bruk.w4'
created_date: '2026-11-15T18:07:00Z'
updated_date: '2026-11-15T19:42:00Z'
labels:
  - size-5
milestone: m-0
dependencies:
  - TASK-1.40
  - TASK-1.75
  - TASK-1.42
parent_task_id: TASK-1
project: bruk-io-steg
asdlc_accepted_from: DRAFT-4
asdlc_branch: task-1.76-server-start-a-pane-s-program-from-the
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Joins three finished parts that do not meet yet: the kernel's rule for a pane's program (TASK-1.40), the server's pane runtime that runs a real program on a pty (TASK-1.40), and dispatch (TASK-1.75), which today refuses a program or working directory on session.new, tab.new and pane.split and treats --force as a no-op. After this a request through the API creates a pane with a real program in it, sized by the kernel, and a forced close ends it.

Carried from reviews: TASK-1.40's second review found that shutting a pane down while a program that prints nothing is running leaves the relay thread blocked and the child alive, because the reader is a separate descriptor and closing the server's end does not hang the program up on this platform; the worker judged a fix to need unsafe or a change of ownership. portable-pty's Child::clone_killer() may give a way to end the child without unsafe; confirm from the crate's source. TASK-1.39 left the pixel cell size for Terminal::resize unassigned. TASK-1.75 left dispatch/mod.rs holding behaviour against .claude/rules/rust.md; split it here if this task grows that file.

Cites doc-2 sections 8 and 9, doc-8 (R97, R99), doc-15, doc-16 (R101-R108: the kernel does no I/O; the server is where I/O meets the kernel). If the plan finds this is more than one reviewable change, the split is: programs started from the API and sized; then forced close.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 pane split, tab new and session new given a program or a working directory start that program in the new pane through the server's pane runtime, where today dispatch refuses them; with neither, the new pane runs the login shell (doc-2 section 8). A program that cannot be started is reported and leaves no pane behind
- [ ] #2 pane show reports the running program and, once it has ended, its exit code; a shell pane that exits closes through the same cascade as a manual close (R97), including the last pane of the last tab of a session
- [ ] #3 A change of a tab's size reaches every terminal in it: the kernel's size for each pane is applied to its terminal and to its pty, and the pixel cell size Terminal::resize needs has a stated source
- [ ] #4 pane close, tab close and session close without --force refuse while a program is running, with the ProgramRunning code, and change nothing; --force stays as it is today until the forced-close task
- [ ] #5 Every path this task creates that can end a pane (a normal exit, a shell's exit cascade, a start that fails) leaves no child unreaped and no thread running, shown by tests with real programs and bounded waits; no unsafe code is added
- [ ] #6 The kernel errors ProgramNotStarted and ShellCannotRestart, and ClientNotViewingPane and ClientNotViewingTab if a method here can return them, map to an API error code that says what happened; whether each needs its own code is decided in the plan against doc-2
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
## This is two reviewable changes, not one

The description's own fallback split is right, and nothing found below changes
that: **programs started from the API, and sized** (AC #1, #2, #3, #6, plus
AC #4's *refusal* clause: "without `--force` refuses... with the
`ProgramRunning` code" — a dispatch-level policy over kernel state, no
process work, so it belongs with part one, leaving part one's own `--force`
a no-op exactly as it is today) is reviewable on its own; **forced close**
(AC #4's remaining clause — actually ending the program on `--force` — and
AC #5 for the kill path) depends on it and is the harder, riskier half. AC #5
straddles: part one owns "no child unreaped, no thread running" for every
path *it* creates (a normal exit, a shell's exit cascade, a start that
fails); part two owns it only for the kill path. I recommend the engineer
split this task along that seam before approving. Everything below is
written so that is possible: the order of work builds and fully checks part
one before part two starts, and each part's own files/proof are named
separately. If it stays one task, I build both in this order within it.

## 1. Where the state lives: the owning structure

Today `dispatch` (`crates/server/src/dispatch/mod.rs`) is a plain function
over `&mut Tree` with no I/O in its signature, and nothing holds a
`PaneHandle`. New `crates/server/src/runtime.rs`:

```rust
pub struct Runtime {
    state: Mutex<State>,
    launcher: Arc<dyn ProgramLauncher>,
    events: Sender<PaneEvent>,          // cloned into every pane::start/restart
    _drain: JoinHandle<()>,             // joined by tests; a real process just exits
}
struct State {
    tree: Tree,
    panes: HashMap<PaneId, PaneHandle>,
    // settings: steg_server::Settings   — TASK-1.48's field, added when it merges;
    // see Risks.
}
```

`Runtime::new(launcher)` constructs an empty `Tree`, opens the `PaneEvent`
channel, and spawns one drain thread whose entire body is `for event in
events_rx { self.apply_event(event) }` — no logic of its own. The logic lives
in `Runtime::apply_event(&self, event: PaneEvent)` (`pub(crate)`, not test-
only): locks `state`, matches `PaneEvent::Exited{pane, exit_code}`, calls
`tree.record_exit(pane, exit_code)`, and on `ExitOutcome::Closed(closed)`
removes `closed.pane`'s `PaneHandle` from `panes` (R94's cascade only empties
a tab/session when the one pane that just exited was its last, so no *other*
pane's `PaneHandle` is ever affected by this specific cascade — confirmed by
reading `Tree::close_pane`/`close_tab`, tree.rs:489-529,282-306). Keeping the
thread a thin loop over one public, directly-callable method (rather than a
test-only notification hook — a board that took an Important finding for an
unnamed `anyhow` would take one for a hidden test hook too) means:
- the cascade itself, including the shell-closes-itself case, is proven
  deterministically and synchronously by calling `apply_event` directly with
  a hand-built `PaneEvent::Exited` against a `Tree` already holding a
  started shell pane — no thread, no timing, no sleep;
- that the plumbing *before* `apply_event` is real is proven separately, by
  one test that (the same shape `crates/server/src/pane.rs`'s own tests
  already use to bypass `PaneHandle` for a focused check) constructs its own
  `(tx, rx)`, starts a real `sh -c 'exit 0'` through `pane::start(..., tx)`
  directly, blocks on `rx.recv_timeout(...)` (bounded, never a bare `recv`)
  for the real `PaneEvent::Exited` a real process produced, and only then
  calls `apply_event` on it against a `Tree` the test set up — proving the
  channel really carries a real exit, with the cascade logic itself already
  covered by the deterministic test above. AC #2 only requires the cascade
  to happen, not that no further request is ever issued in this codebase's
  tests; that framing was this design's own motivation, not the criterion's
  text, and the proof above does not lean on it.

`Runtime::handle(&self, description, context, request) -> Response` is the
one entry point (what TASK-1.44/1.45 will call once a socket exists; today
only this task's own tests call it). It locks `state` once per request —
simple, and correct for M1's single local server with no long-held I/O inside
the lock (the real pty open + spawn is fast and synchronous, matching
TASK-1.40's own design; nothing inside the lock ever blocks on a running
program). For every method except the six named in §2 and §5 below, it
delegates straight to the **existing, unchanged** `dispatch(&mut state.tree,
description, context, request)` — `pane.show`/`list`/`set`/`unset`/`focus`/
`resize`/`swap`/`move` need no I/O at all (`Program`/`ProgramStatus` already
live on the kernel's own `Pane`, not on `PaneHandle` — confirmed by reading
pane.rs:60-74), so `dispatch` stays I/O-free exactly as its own module doc
claims today. The six methods that need the launcher/panes map are
intercepted in `Runtime::handle` *before* falling through to `dispatch`.

**Pass-through to `State`:** `Context`'s existing shape works unchanged;
`Runtime` is additive, nothing in `mod.rs`'s signature moves.

## 2. Starting (AC #1)

`session.new`, `tab.new`, `pane.split` stop refusing `program`/`cwd`
(`crates/server/src/dispatch/{session,tab,pane}.rs`'s three `if
param_present(...) { return Err(not_implemented_yet(...)) }` blocks are
deleted). New `crates/server/src/program_param.rs`:

```rust
pub fn program_from_request(request: &Request, default_cwd: &str) -> Result<steg_sdk::Program, ApiError>
```

reads `program` (`Argv`, already validated as "all strings" by `validate`)
and `cwd` (`Path`); absent `program` → `Program::Shell { cwd }` (the
description's own words: "Defaults to the login shell", methods.rs:116);
present → `Program::Command { argv, cwd }`. `cwd` absent → `default_cwd`:
**`$HOME`** (`std::env::var("HOME")`, falling back to `"/"` only if unset —
essentially never on this platform), the same default a real login shell
uses with no directory asked for. Flagged for the engineer: `pane.split`
could instead inherit the *splitting* pane's own `Program::cwd()` (what
tmux/iTerm do); nothing in doc-9/doc-2 decides this, so I defaulted to the
simpler, uniform rule across all three methods rather than guess a UX
decision. `Program::Shell` resolves to a concrete `argv` here too (TASK-1.40
left this to the caller): `std::env::var("SHELL")`, falling back to
`"/bin/zsh"` (macOS, R4). Flagged for the engineer: this may overlap with
TASK-1.44's own "login shell" base-environment work (R99) — if TASK-1.44
lands first, its resolution should win and this helper should read whatever
it exposes instead of re-deriving `$SHELL` itself.

`Runtime::handle`'s wrapper for these three methods, under the single lock
held the whole time (this *is* the "single-threaded, not reentrant between
mint and start" control thread TASK-1.40's plan asked for — there is no real
control thread yet, so this lock is what provides it):

1. Call the existing `dispatch()` arm unchanged. Its own body additionally
   calls `tree.start_program(new_pane, program)` right after minting (new
   code in the three arms) — still pure, still inside the one `dispatch`
   call, so the "`program`/`status` is `None`" window TASK-1.40 named stays
   exactly as unobservable as its own plan argued, just enforced by this
   lock instead of a control thread.
2. On success, build a `LaunchSpec` (argv resolved above, the five `STEG_*`
   vars + `PATH` from the already-built `pane_environment`, and a size —
   §4) and call `pane::start(spec, new_pane, launcher.clone(), events.clone())`.
3. **On I/O failure** (`Error::Pty` — a bad command, or, pending the
   experiment below, a bad `cwd`): roll back the mint exactly as
   `pane.split`'s existing label-rejection path already does
   (`tree.close_pane`/`close_tab`/`close_session` on whichever was just
   minted) and return the new `ErrorCode::ProgramFailed` (§6) — AC #1's
   "leaves nothing behind" is this rollback, proven by a real `ENOENT`
   argv and asserting the tree afterward has exactly as many sessions/
   tabs/panes as before the call.
4. On success, insert the `PaneHandle` into `panes`, return the original
   `Outcome`.

**Experiment, first step of phase 2:** does a bad `cwd` fail
`spawn_command` synchronously (same path as `ENOENT`), or does the child
fork, `chdir` fail inside it, and it exit immediately with a nonzero code
(the *normal* exit path — `Ended{exit_code}`, no rollback, pane stays per
R97)? AC #1 groups "command not found, bad directory" together; I do not
assume they take the same path. Whichever it is, name it in the notes and
adjust `program_from_request`'s doc and this flow's tests accordingly —
no code change either way, since both outcomes already have a handler.

## 3. Exit (AC #2)

Covered structurally by §1's drain thread for the *natural* path. `pane.show`
(today `"list" | "show" => not_implemented_yet`, pane.rs:37-39) gets its own
arm, pure, inside the existing `dispatch`: a new `steg_sdk::PaneRecord`
(`crates/sdk/src/program.rs` or a new `record.rs`, same reasoning as
`SessionRecord` — this crosses the API, R105) with `id`, `label`, `cwd`
(`Program::cwd()`), `program` (its `Display`/a short word — "shell" or the
command), `exit_code` (`Some` only once `ProgramStatus::Ended`), `status`
("running"/"ended", `ProgramStatus`'s own serde words). **`title` is left
null**: it needs the terminal's live OSC-0/2 title, which lives behind
`PaneHandle` (I/O), and nothing in AC #2 asks for it — naming it as a gap for
whichever task wires `PaneHandle` into `pane.show`, not invented here.
`status`'s richer doc-11 states (waiting/failed/bell/finished) are TASK-1.41's
job, in progress; this reports the two-state `ProgramStatus` only, which is
what AC #2 itself asks for ("the running program and... its exit code").
`pane.list` stays `not_implemented_yet` — AC #2 only names `show`, and
building `list`'s `Records` shape is the same work repeated, not a new
decision; flagged as a one-line follow-up, not done here to keep this task's
diff matched to its own acceptance criteria.

## 4. Sizes (AC #3)

New, pure kernel method, `crates/kernel/src/tree.rs`:
```rust
pub fn tab_sizes_now(&self, tab: TabId) -> Option<SizeChanged>
```
a public wrapper around the existing private `size_changed` (tree.rs:377),
returning `None` when `tab.size()` is `None` (nothing declared yet — true of
every tab today, since no client/`declare_size` dispatch exists: `("client",
_)` is still `not_implemented_yet`, mod.rs:292). `Runtime`'s start-wrapper
(§2) and its close-wrapper (§5, after a non-forced partial effect — see
below) both call this after any tree mutation that can change a tab's
panes' sizes (split, close, swap, move, fill/unfill, resize_pane) and, if
`Some`, apply `SizeChanged.panes` via `PaneHandle::resize(cols, rows)` for
each — the one mechanism serving both "a brand-new pane needs this tab's
already-declared size" and "an existing pane's size shifted because the
split changed ratios". When `tab_sizes_now` is `None` (today, always, until
TASK-1.42's client/size dispatch lands), the new pane's `LaunchSpec.size`
falls back to a stated literal default, `DEFAULT_PANE_SIZE: (u16, u16) =
(80, 24)` (`crates/server/src/pane.rs`, beside `DEFAULT_SCROLLBACK_LINES`) —
named, not guessed: no client has ever reported a size, so there is nothing
real to use.

**How AC #3's two halves are actually observed** (named now, not discovered
at test-writing time): "applied to its terminal" is `PaneHandle::snapshot()`
— its `Screen` carries its own dimensions, so a resize followed by a
snapshot proves the kernel `Terminal` was resized, no new surface needed.
"applied to its pty" has no handle to ask today: `RealLauncher` moves
`pair.master` into the `resize` closure (pty.rs:118-129), so nothing outside
it can call `MasterPty::get_size()` afterward. Rather than add a getter only
a test would call, the proof is the pty's own real effect: a real `sh -c
'stty size'` pane, resized via `PaneHandle::resize` before the shell reads
its controlling terminal's size, with `snapshot()` afterward showing the
line `stty size` itself printed — "24 80" or whatever was applied — which
only matches if the kernel's `winsize` ioctl, not just the `Terminal`
object, actually changed. One real program, one assertion, both halves.

**Pixel cell size**: `Terminal::resize` (terminal.rs:418) already hardcodes
`0, 0` into the engine call, and `RealLauncher`'s `resize`/`openpty`
(pty.rs:84-89,119-127) already hardcode it too — there is no plumbing left
for this task to add. Zero is `winsize`'s own "unknown" convention; no app is
attached to a server-only M1 build, so there is no real pixel size to pass,
and fabricating one would tell a program a geometry that is wrong. State
this as the answer (not a new default) and correct terminal.rs:395's comment,
which names TASK-1.42 as the task that plumbs a real value through —
TASK-1.42 is done and did not, because nothing client-facing calls
`Terminal::resize`'s pixel path yet; the real source is the app's terminal
view (TASK-1.56) reporting pixel dimensions once a window exists, so the
comment should name that instead.

## 5. Forced close, the hard part (AC #4, #5)

**The missing refusal, found by re-reading `close_pane`/`close_tab`/
`close_session` (tree.rs:489,282,218): none of them check anything today.**
`dispatch::pane::close`/`tab::close`/`session::close` call them straight
through with no status check at all — AC #4's first clause ("refuse while a
program is running") does not exist, in either direction, today. The
description already carries everything needed for it (confirmed by reading
`crates/sdk/src/description/methods.rs`): `pane.close`/`tab.close`/
`session.close` already list a `force` switch and already list
`ErrorCode::ProgramRunning` in their `errors`. **No sdk/description change is
needed for this clause at all** — it is a dispatch-level policy, not a
kernel one, because the kernel's own close has no concept of "refuse" (it
always just closes); adding a kernel `Error` variant for this would be the
"force flag threaded through `close_pane`'s own signature" shape TASK-1.40's
plan already rejected (~123 call sites). The check lives in `Runtime`'s
close-wrapper, using the already-public `Pane::status()`:

```rust
fn running_panes(tree: &Tree, scope: /* one pane, or every pane of a tab, or of every tab of a session */) -> Vec<PaneId>
```

`pane.close` checks one pane; `tab.close` checks every pane of the target
tab (`Tab::panes()`); `session.close` checks every pane of every tab of the
target session (`Session::tabs().flat_map(Tab::panes)`). Without `--force`
and a non-empty list: `ApiError::new(ErrorCode::ProgramRunning, ...)`,
nothing touched. This holds even for a shell pane whose *own* exit would
close it on its own (R97) — a *manual* close without `--force` still refuses
while that shell is running, which is a deliberate reading of AC #4, not an
oversight: the two are different triggers (the program ending itself, versus
a person asking to end the pane) and only one of them is "force".

**With `--force`, under the lock:** compute `running_panes`, and
`panes.remove(id)` each one's `PaneHandle` out of the shared map right away
— before any kill is sent. **Then drop the lock** before doing anything that
waits: the alternative (holding `state`'s lock across every pane's bounded
kill-and-confirm) stalls every other request and every unrelated pane's own
`apply_event` behind up to the full escalation bound *per running pane*
(seconds, for `tab.close --force` over several stuck programs) — removing
the handles first is also what makes a pane mid-kill unreachable by any
other request, so nothing else needs to know the lock was ever held only
briefly. Re-acquire the lock afterward, only to call the structural close.

For each removed handle (sequentially; cheap at M1's scale and keeps the
bounded-wait reasoning per-pane rather than needing to fan out and join),
outside the lock:

1. `PaneHandle::force_end(bound) -> Result<i32, Error>` — new, in
   `crates/server/src/pane.rs`. New `PaneCommand::ForceEnd(Sender<i32>)`.
2. The pane's worker: on `ForceEnd(reply)`, if nothing is `current`, reply
   immediately with nothing to wait for (defensive — `running_panes` already
   filtered to `Running` panes, so this should be unreachable, but a true
   no-op here costs nothing). Otherwise: records `reply` on the run (a new
   `force_end_reply: Option<Sender<i32>>` field on `Running`); calls
   `current.killer.kill()` (§ below — `SIGHUP`); spawns a one-shot timer
   thread that sleeps the grace bound then sends `PaneCommand::EscalateKill`
   on the same `commands` channel (a deliberate, intentional `thread::sleep`
   inside a disposable timer thread, not a polling wait and not a test —
   distinct from the no-sleep-in-tests rule, named here so a reviewer
   doesn't conflate the two).
3. `PaneCommand::Exited(exit_code)` (unchanged relay path): if `current`
   carries `Some(force_end_reply)`, this run **does not** forward
   `PaneEvent::Exited` to the shared `events` channel — it replies on
   `force_end_reply` instead, sets `current = None`, and stops there. This
   is the fix for the one real race in this design: without it, the drain
   thread (§1) could win the lock first and run `record_exit`'s R97 cascade
   (closing a shell pane itself) *concurrently* with the force-close
   caller's own, unconditional `tree.close_pane` — the second call then
   fails with `PaneNotFound` on an already-gone pane. Suppressing the
   normal event for a run that was force-ended makes the force-close caller
   the sole owner of that pane's tree removal (and is moot structurally
   too, now: the handle is already out of `panes`, so the drain thread's own
   `apply_event` can't reach it even if a stray event arrived), proven by a
   test asserting no `PaneEvent::Exited` is ever observed on the shared
   channel for a force-ended run, alongside the normal-path test that *does*
   observe one for an un-forced exit.
4. `PaneCommand::EscalateKill`: **the guard is simply
   `current.as_ref().is_some_and(|r| r.force_end_reply.is_some())`** — no
   `Sender` identity check is needed or possible (`mpsc::Sender` has no
   `PartialEq`). This is already correct for every case: the program died
   first → step 3 already replied and set `current = None` → this is a
   no-op; a `restart` landed in between (`PaneCommand::Resume` always
   replaces `current` wholesale, with a fresh `Running` whose
   `force_end_reply` is `None`) → also a no-op; still running → `Some` →
   send **`SIGKILL`**, below, without `unsafe` of this crate's own.
5. The caller (no longer holding `state`'s lock): `reply_rx.
   recv_timeout(bound + margin)` — bounded, never a bare `recv`. On success,
   call `handle.shutdown()` (ends the worker thread cleanly — unchanged from
   TASK-1.40). On timeout (should not happen once `SIGKILL` is unblockable,
   but named rather than assumed — see the experiment below): still call
   `handle.shutdown()` best-effort (so the `Shutdown` command is at least
   sent, rather than dropping the handle on the floor and abandoning the
   worker thread outright), and return a server-error-mapped failure instead
   of proceeding — AC #5 forbids treating an unconfirmed pane as closed.
6. Once every removed handle is confirmed ended and shut down, re-acquire
   `state`'s lock and call the **existing** `tree.close_pane`/`close_tab`/
   `close_session` — unconditionally, regardless of R97 (a forced close
   removes a command pane too, not just a shell), exactly matching what an
   un-forced close on an already-ended pane already does today.

**The signal mechanism**, confirmed from `portable-pty` 0.9.0's own source
(not assumed): `Child: ChildKiller` is a supertrait (lib.rs:130), so
`clone_killer(&self)` (lib.rs:157) is callable on the `Box<dyn Child + Send +
Sync>` **before** it moves into `spawn_relay` — this is the "way to end the
child without `unsafe`" the task's own description names, and exactly what
`spawn_relay`'s own comment already points at (pane.rs:360-362: "Taking that
clone before handing `child` to this function is DRAFT-4's to add"). The
clone is `pid`-only, `Send + Sync`, independent of every pty descriptor
(lib.rs:293-299,323-338) — it signals the process **by pid**, which is why it
sidesteps the whole fd-ownership problem the module doc and TASK-1.40's
reviews explored: that problem was specifically about the *hang-up* signal,
which needs every master-side descriptor closed; signalling by pid needs
none of that. `ProcessSignaller::kill` (the clone's own) sends only `SIGHUP`
(`libc::kill` inside `portable-pty`, not this crate) and does **not**
escalate (lib.rs:324-333) — only `std::process::Child`'s own `ChildKiller`
impl does the 5×50ms-then-`SIGKILL` dance (lib.rs:340-373), and that impl is
not reachable through the clone. So this crate does its own, small,
escalation: `killer.kill()` for `SIGHUP`, a bound (proposed **500ms**, double
`portable-pty`'s own grace window, tuned by the experiment below), then
`SIGKILL` via **`nix::sys::signal::kill`** — a safe function (confirmed from
`nix` 0.28.0's own source, `src/sys/signal.rs:1054`), not `unsafe
libc::kill`. `nix` 0.28.0 is already in `Cargo.lock` as `portable-pty`'s own
dependency, already licence-checked (MIT) in TASK-1.40's own review. **New
direct dependency, named here rather than discovered by a reviewer**:
`steg-server`'s `Cargo.toml` gains `nix = { version = "0.28", features =
["signal"] }`, and `boundaries.toml`'s `[steg-server]` gains `"nix"` with a
reason citing this section — the exact shape TASK-1.40 took an Important
finding for omitting with `anyhow`. **Rejected alternative**: `unsafe {
libc::kill(pid, libc::SIGKILL) }` directly — functionally identical (same
call `nix` wraps), but adds this crate's first `unsafe` for no gain over a
safe, already-present dependency. **A program that ignores `SIGHUP`
entirely gets `SIGKILL` after the 500ms bound — it cannot ignore that.**

**Four cases, verified by a real-process experiment, first step of phase 2
— not assumed:**
1. `sh -c 'sleep 1000'` (silent, no output): does `killer.kill()`'s `SIGHUP`
   alone end it and unblock the relay's `read()`? (Expected yes, per above,
   but TASK-1.40's own reviews probed a *different* mechanism and found a
   negative result there — this one is genuinely new and gets its own
   check.)
2. The same, but producing output throughout (`yes`, or a tight loop
   writing): same question, to rule out the two cases needing different
   handling.
3. `sh -c 'trap "" HUP; sleep 1000'`: confirms escalation is actually
   reached and `SIGKILL` ends it within the bound.
4. `sh -c 'sleep 1000 & wait'`: the direct child's pid is the shell running
   `wait`; killing *it* may leave the backgrounded `sleep`, a grandchild,
   holding the pty's slave side open — relay stays blocked, AC #5 fails.
   `MasterPty::process_group_leader()` exists on unix (unix.rs:374-379, via
   `tcgetpgrp`); if case 4 shows the direct pid is not enough, the fix is
   signalling the **process group** instead (`nix::sys::signal::killpg`,
   also safe, `src/sys/signal.rs:1075` — same dependency, no new one), which
   needs the pgid captured at spawn time (`SpawnedProgram` gains a
   `pub pgid: Option<libc::pid_t>` field, read via `pair.master
   .process_group_leader()` right after `spawn_command`, since
   `CommandBuilder` makes the child a session/group leader by default on
   unix). Named as a real risk, not a footnote: if case 4 fails, the
   mechanism becomes "signal the group, both stages", not "signal the pid" —
   a one-file change (`pty.rs` + the two `kill`/`killpg` call sites in
   `pane.rs`), not a redesign, but the plan's own correctness claim for AC #5
   rests on this case's actual result, so it is checked before anything
   else is built.

## 6. Error codes (AC #6)

**No new `ErrorCode` for the close-refusal** — `ProgramRunning` already
exists, is already on all three methods' `errors` list, and its own doc
comment ("A program is still running and `--force` was not given") is a
verbatim match.

**One new code is needed**, surfaced by §2's rollback, not by the four
kernel errors the brief named: `ErrorCode::ProgramFailed` — "the program
could not be started" (a real OS-level spawn failure: `ENOENT`, and,
pending the cwd experiment, possibly a bad directory). Nothing existing
fits: `BadRequest` is "did not match the description" (validation-shaped,
not I/O-failure-shaped) and maps to CLI exit status **2** ("bad usage"),
confirmed from `crates/sdk/src/description/methods.rs:1472` — a status-2
command named *no unknown flag, no missing value*; the request was fine,
the OS refused it, which is doc-2 section 6's status **1** ("the command
ran and failed"). New code added to `ErrorCode` (shapes.rs), to the
`exit_status` table (status 1), to `pane.split`/`tab.new`/`session.new`'s
`errors` lists (methods.rs), and mapped from `steg_server::Error::Pty` to
this code in a new `crates/server/src/dispatch/server_error.rs` (parallel
to `kernel_error.rs`, but for the server's own I/O errors — a distinct
source `map_kernel_error` was never meant to cover, since its own doc
states the direction it checks is `steg_kernel::Error` only).

**Flagged for the engineer as its own item, per the brief's instruction to
name a doc-2/description change separately**: `ErrorCode::ProgramFailed` is
new public surface in the sdk's description — the generated CLI
(TASK-1.49/1.50, not yet built) will consume it mechanically (R32) — not an
internal refactor, even though the reasoning above rests on CLI exit
statuses doc-2 already specifies rather than on inventing a new one.

**The four kernel errors the brief named, checked against doc-2, each with
a reason, none changed**:
- `ProgramAlreadyStarted`/`ProgramStillRunning` → `ProgramRunning` — already
  correct (TASK-1.40's second review fixed this).
- `ProgramNotStarted` — reachable only from this task's own internal
  `tree.record_exit` call in the drain thread (§1), never from a client
  request (no API method calls `record_exit` directly). Per `Pane`'s own
  invariant (program always started immediately after minting, before
  anything else runs — now enforced by `Runtime`'s single lock, §2), this
  can only fire if that invariant is broken, which is a bug, not a client-
  facing error. Left as `BadRequest` in `kernel_error.rs` (unchanged,
  dead in practice) rather than invented a client-facing code for state that
  should be unreachable — the drain thread's own call documents this with
  `.expect("…")`-style reasoning in the same style `tree.rs` already uses
  throughout, not a silent `Result` swallow.
- `ShellCannotRestart` — **doc-2 names no verb anywhere for "run a command
  pane's program again"** (checked: no "restart" string appears in doc-2 at
  all, and `crates/sdk/src/description/methods.rs`'s `pane_methods()` has no
  such method). `Tree::restart_program` stays unreachable through dispatch
  after this task too, exactly as before it. Not decided here — the mapping
  stays `BadRequest`, and the real gap (doc-2 needs a verb before this error
  can ever reach a client) is named for the engineer, not invented as a side
  effect of this task.
- `ClientNotViewingPane`/`ClientNotViewingTab` — still unreachable through
  anything this task adds (no `client` dispatch exists). Not decided here,
  same as today.

## 7. `dispatch/mod.rs`

At 358 lines today, before this task's own additions (`ProgramFailed`'s
mapping, the `program_from_request` helper's call sites, the force-aware
close logic moving *out* to `Runtime` rather than in). This task:
- Moves the `param_*`/`target_*`/`resolve_*`/`tab_mut`/`pane_mut`/
  `session_owning_tab`/`find_tab`/`wire_position_to_index` helpers
  (mod.rs:91-242) into a new `crates/server/src/dispatch/lookup.rs`,
  re-exported `pub(super)` the same as today.
- Keeps `mod.rs` to: `Context`, the `dispatch()` routing function itself,
  `not_implemented_yet`/`bad_request`, and the completeness test —
  everything that is genuinely about *routing*, nothing that is a shared
  utility other modules both need.
- Adds no new behaviour to `mod.rs` itself: the close-refusal and the
  start/force wiring live in `Runtime` (`runtime.rs`), not here, so this
  split happens *before* any of this task's own new logic would have grown
  the file further, per the brief's own instruction.

## Files that change

- `crates/server/src/runtime.rs` — new: `Runtime`/`State`, the drain thread,
  `handle`, the start-wrapper (§2), the close-wrapper (§5).
- `crates/server/src/program_param.rs` — new: `program_from_request`,
  `$HOME`/`$SHELL` resolution, its own unit tests (pure).
- `crates/server/src/pane.rs` — `Running` gains `killer: Box<dyn
  portable_pty::ChildKiller + Send + Sync>` and `force_end_reply:
  Option<Sender<i32>>`; `PaneCommand` gains `ForceEnd`/`EscalateKill`;
  `PaneHandle::force_end`; `DEFAULT_PANE_SIZE`; the module doc's "known gap"
  section rewritten to say what is now proven instead of what was missing.
- `crates/server/src/pty.rs` — `SpawnedProgram` gains `pgid: Option<...>`
  if case 4 needs it (named conditional on the experiment, not committed to
  blindly).
- `crates/server/src/dispatch/{session,tab,pane}.rs` — delete the
  program/cwd refusal; add program-starting (§2) via `Runtime`, not inline;
  add `pane.show`'s record (§3); `pane.close`/`tab.close`/`session.close`
  stay calling `tree.close_pane`/etc unchanged — the refusal and force
  handling move *up* to `Runtime`, not into these files.
- `crates/server/src/dispatch/mod.rs` — shrinks (§7); unaffected by §2/§5's
  new logic, which lives in `Runtime`.
- `crates/server/src/dispatch/lookup.rs` — new (§7).
- `crates/server/src/dispatch/server_error.rs` — new (§6).
- `crates/server/Cargo.toml`, `boundaries.toml` — `nix` added to
  `steg-server` (§5).
- `crates/kernel/src/tree.rs` — `Tree::tab_sizes_now` (§4), pure, new tests.
- `crates/kernel/src/terminal.rs:395` — comment corrected (§4).
- `crates/sdk/src/program.rs` (or a new `record.rs`) — `PaneRecord` (§3).
- `crates/sdk/src/description/shapes.rs`, `methods.rs` — `ErrorCode::
  ProgramFailed`, its exit status, its presence on `pane.split`/`tab.new`/
  `session.new`'s `errors` (§6); `pane.show`'s `Returns` (§3).
- `crates/server/tests/real_pty.rs` and a new
  `crates/server/tests/force_close.rs` — the experiment's four (or five)
  cases as committed tests, plus the close/start/race proofs below.

## Order of work

1. **The four (or five) real-process experiments (§5), as throwaway/scratch
   programs first, then as committed tests either way.** Nothing in §5 is
   built before this returns real answers — if case 4 changes the
   mechanism, every later step in §5 reflects that from the start, not as a
   rework.
2. The `cwd`-failure experiment (§2), same reasoning, same step.
3. Bring in `origin/main` (per the brief: read the code from there first,
   and this is also where TASK-1.48's `Settings` risk below gets resolved
   one way or the other before anything is built on top of it).
4. **Part one**, fully built and checked, in this order: `tab_sizes_now`
   (§4, pure, kernel-only, lands first and alone, same reasoning TASK-1.40
   used for its own kernel/server split); `program_param.rs` (pure,
   testable alone); `PaneRecord`/`pane.show` (§3); `Runtime`'s structure and
   drain thread (§1) with the shell-exit-closes-itself proof; `Runtime`'s
   start-wrapper (§2) with the ENOENT-rollback and cwd-default proofs;
   `running_panes` and the close-refusal (§5's first half — a dispatch-level
   policy over kernel state the refused-without-`--force` ACs need, with
   `--force` itself still a no-op here); `ErrorCode::ProgramFailed` and
   `server_error.rs` (§6); `dispatch/lookup.rs` split (§7). This is the
   point at which, if the engineer agrees with the split above, part one
   could be committed and reviewed as its own change.
5. **Part two**: `nix` (§5), the kill mechanism, the escalation timer, the
   force-end reply path and the race-avoidance (§5's suppressed-event fix);
   the close-wrapper's full sequence built on top of part one's
   `running_panes`. Nothing in part two needs a new `ErrorCode`.
6. `just check` at each numbered step's own boundary, not only at the end.

## Risks

- **TASK-1.48 (`Settings`, in progress) lands mid-build.** `State` reserves
  the field; if it merges before part one is built, add it in the same pass
  as the `origin/main` merge (step 3); if after, a small follow-up commit.
  Not a blocker either way — nothing in this plan reads a setting yet.
- **Case 4 (§5) changes the mechanism from pid to process group.** Named
  above as a real possibility, with the exact fallback, not a surprise if
  it happens.
- **The `cwd` default (`$HOME` vs. the splitting pane's own cwd) is a UX
  call, not a technical one.** Flagged for the engineer in §2; easy to
  change later since it is one function.
- **Holding `Runtime`'s single lock across the real `pty::start` call (§2)**
  blocks every other request for the duration of one `openpty` + `spawn`
  (fast, synchronous, matching TASK-1.40's own design) — acceptable at M1's
  scale (one server, no concurrent load target stated anywhere), named so
  a reviewer checks the claim rather than assumes it. **§5's force-close does
  not carry this same risk**: the lock is held only to remove the running
  handles from the map, then dropped before any kill-and-confirm wait, and
  re-acquired only for the final structural close — stated in §5 itself, not
  repeated here as a second risk.
- **Sequential force-close across several running panes (`tab.close
  --force` on a tab with three running programs)** pays the escalation
  bound once per pane in the worst case (every one ignores `SIGHUP`) rather
  than in parallel. Acceptable for M1 (closing several stuck programs at
  once is not a stated latency target); named as a one-line "worth
  parallelising later" rather than built that way now, to keep the
  sequencing easy to reason about and test.

## Proof

`just check` (format, lint, boundaries, licences — `nix`'s MIT licence
checked by `just deny`, not assumed — affected tests, every package alone,
architecture, recipe names), recorded via `asdlc record-check` at each
numbered step in "Order of work", including the experiments' own runs
(passing and, where a deliberate failing run demonstrates a fix like
TASK-1.40's own `shutdown_mid_run` did, that failing run recorded too).

Clause by clause:
- **AC #1**: real-pty tests — a command and a shell started through
  `Runtime::handle`, in a given `cwd`; an `ENOENT` argv leaves the tree with
  no new session/tab/pane (a before/after count); the `cwd`-failure
  experiment's actual result, whichever it is, with its own test.
- **AC #2**: `pane.show`'s record reflects `Running`/`Ended{exit_code}`
  (kernel-level, no doubles, plus one real-pty case); the shell-exit-closes-
  itself cascade proven two ways per §1 — deterministically via a direct
  `apply_event` call, and end-to-end via one real-pty test that blocks on a
  bounded `recv_timeout` for the real `PaneEvent::Exited` a real `sh -c
  'exit 0'` produces.
- **AC #3**: `Tree::tab_sizes_now` unit tests (pure); a real-pty test that
  declares a tab size directly at the kernel level (bypassing the
  unbuilt `client` dispatch, which is honest since that is the only way to
  get a tab size to exist at all today), splits a pane, and — per §4's
  "how this is observed" — asserts the kernel `Terminal`'s own reported
  size via `PaneHandle::snapshot()` and the pty's own real size via a real
  `sh -c 'stty size'` pane reporting what the OS actually applied, for
  every pane in the tab.
- **AC #4/#5**: the close-refusal with `ProgramRunning` and no tree change,
  for a pane/tab/session each; the four (or five) experiment cases as
  committed tests; a forced close of a program producing output and one
  producing nothing, in each case asserting via `ps`/`kill -0` (the same
  `process_exists` helper pane.rs's own tests already use) that the child
  is gone and (via `join_within`, already present) that the relay thread
  and the worker thread have both ended; the suppressed-event race test
  (no `PaneEvent::Exited` observed for a force-ended run, while a normal
  exit still produces one); a `tab.close --force` on a tab with two running
  panes, both reaped.
- **AC #6**: `map_kernel_error`'s existing tests, unchanged, plus
  `ProgramFailed`'s own mapping test; a test that `ShellCannotRestart` and
  `ProgramNotStarted` remain absent from every method's `errors` list
  (nothing this task adds reaches them) — a negative assertion, naming the
  claim rather than leaving it implicit.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Split on 2026-11-15 under the engineer's standing decision to split by default, along the seam this task's description named and the worker's plan recommended. This task is now part one: programs started from the API, exits, sizes, and the refusal to close a running program without --force. Part two, a draft under TASK-1 depending on this task, takes --force actually ending a program (the worker's plan section 5 from 'With --force' onward, and the experiment it names). The plan on this task was written for both parts and is to be rewritten for part one only.
<!-- SECTION:NOTES:END -->
