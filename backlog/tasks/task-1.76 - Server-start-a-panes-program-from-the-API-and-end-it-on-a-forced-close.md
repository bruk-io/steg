---
id: TASK-1.76
title: 'Server: start a pane''s program from the API, and size it'
status: In Progress
assignee:
  - '@bruk.w4'
created_date: '2026-11-15T18:07:00Z'
updated_date: '2026-11-15T22:19:00Z'
labels:
  - size-5
  - changes-requested
milestone: m-3
dependencies:
  - TASK-1.40
  - TASK-1.75
  - TASK-1.42
references:
  - task-1.76-server-start-a-pane-s-program-from-the
  - https://github.com/bruk-io/steg/pull/40
parent_task_id: TASK-1
project: bruk-io-steg
asdlc_accepted_from: DRAFT-4
asdlc_branch: task-1.76-server-start-a-pane-s-program-from-the
asdlc_plan_approved_by: bruk
asdlc_plan_approved_hash: '308d35ef87d0'
asdlc_plan_approved_size: size-5
asdlc_review_kind: same-machine
asdlc_review_findings: 'important=3,nit=1'
asdlc_review_policies: ["review", "testing-rust"]
asdlc_plan_matched: no
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
## 1. Where the state lives: the owning structure

Today `dispatch` (`crates/server/src/dispatch/mod.rs`) is a plain function
over `&mut Tree` with no I/O in its signature, and nothing holds a
`PaneHandle`. New `crates/server/src/runtime.rs`:

```rust
pub struct Runtime {
    state: Mutex<State>,
    launcher: Arc<dyn ProgramLauncher>,
    login: LoginShell,                  // §2 — the one injected value
    events: Sender<PaneEvent>,          // cloned into every pane::start
    _drain: JoinHandle<()>,             // kept so the thread outlives Runtime's own construction
}
struct State {
    tree: Tree,
    panes: HashMap<PaneId, PaneHandle>,
    // settings: steg_server::Settings   — TASK-1.48's field; see Risks.
}
```

`Runtime::new(launcher, login)` constructs an empty `Tree`, opens the
`PaneEvent` channel, and spawns one drain thread whose entire body is `for
event in events_rx { self.apply_event(event) }` — no logic of its own. The
logic lives in `Runtime::apply_event(&self, event: PaneEvent)` (`pub(crate)`,
not test-only, so a test can call it directly and deterministically, or let
the real thread call it): locks `state`, matches `PaneEvent::Exited{pane,
exit_code}`, calls `tree.record_exit(pane, exit_code)`. On
`Err(Error::PaneNotFound)` — the orphaned-pane case named in §5, not a bug —
it still removes `pane`'s `PaneHandle` from `panes` if present, and stops;
it does not panic and does not retry. On `Ok(ExitOutcome::Closed(closed))`
it removes `closed.pane`'s `PaneHandle` (R94's cascade only empties a
tab/session when the one pane that just exited was its last, so no *other*
pane's `PaneHandle` is ever touched by this cascade — confirmed by reading
`Tree::close_pane`/`close_tab`, tree.rs:489-529,282-306, including the case
where the tab was also the session's last: `close_tab` calls
`close_session` itself, tree.rs:298-300, so one `apply_event` call already
reaches all the way to the session closing with no separate step needed).

Keeping the thread a thin loop over one public, directly-callable method
(rather than a test-only notification hook) means the cascade — including
the shell-closes-itself case, and the session's-last-tab case — is proven
deterministically and synchronously by calling `apply_event` with a
hand-built `PaneEvent::Exited` against a `Tree` already holding the fixture;
that the plumbing *before* `apply_event` is real (a real process's real
exit reaching a real channel) is proven separately, by a test that
constructs its own `(tx, rx)`, starts a real `sh -c 'exit 0'` through
`pane::start(..., tx)` directly (the same shape `crates/server/src/pane.rs`'s
own tests already use to bypass `PaneHandle`), blocks on
`rx.recv_timeout(...)` (bounded, never a bare `recv`) for the real event,
and only then calls `apply_event` against a `Tree` the test set up.

`Runtime::handle(&self, description, context, request) -> Response` is the
one entry point (what TASK-1.44/1.45 will call once a socket exists; today
only this task's own tests call it). It locks `state` once per request. For
every method except `session.new`/`tab.new`/`pane.split` — the only three
that need the launcher — it delegates straight to the **existing,
unchanged** `dispatch(&mut state.tree, description, context, request)`.
This is a smaller intercept list than I first drew: the close-refusal (§5)
needs only `Pane::status()`, which already lives on the kernel's own `Pane`
— it is a pure check and stays *inside* `dispatch`'s own `pane.rs`/`tab.rs`/
`session.rs` arms, not in `Runtime`, the same reasoning that already keeps
`pane.show` pure. `dispatch` stays I/O-free exactly as its own module doc
claims today; `Runtime` is additive, nothing in `mod.rs`'s signature moves.
This three-method list is only about which requests need a new `PaneHandle`
created — `handle` has one further, separate, unconditional step after
*any* successful `dispatch()` call, not limited to these three: reapplying
every currently-sized tab's sizes to its panes (§4). The two are independent
reasons to do something after `dispatch` returns, not one mechanism.

**What runs under the one lock, and why that is acceptable rather than done
outside it** (asked for directly): for the three starting methods, the lock
is held across the existing `dispatch()` arm (mint the pane, call
`tree.start_program` — both pure) *and* the real `pane::start` call
(`openpty` + `spawn_command`, synchronous, no indefinite wait — confirmed by
reading `RealLauncher::spawn`, pty.rs:80-137, which returns once the process
is forked and the pty pair is split apart, never blocking on anything the
*program* does). Nothing that blocks on a running program (a `read()`, a
`wait()`) happens inside the lock; those live on the pane's own worker/relay
threads, started only after `pane::start` returns. Holding the lock here is
what keeps TASK-1.40's own stated invariant exactly as it already reads:
"`Pane::program()`/`status()` is `None` only in the window between minting
and `start_program`, never observed outside that one pairing, because
nothing else runs between the two calls" — true today because the server's
one thread does both calls back to back with nothing interleaved; this
task has no such single thread yet, so the lock is what makes "nothing else
runs between the two calls" still true. **Rejected alternative**: release
the lock between minting and the real I/O (lock → mint, unlock → spawn →
lock → `start_program` + insert the handle). This does let other requests
run between mint and start, which breaks that invariant as TASK-1.40 wrote
it (a concurrent `pane.show` would see a pane that exists with no `program`
at all, for as long as the real spawn takes, and nothing in the kernel
marks that state as "being built" versus "actually programless") and buys
nothing at M1's scale (one local server, no stated concurrent-request
target) in exchange for revisiting a different task's already-reviewed
invariant. Not built.

## 2. Starting (AC #1)

`session.new`, `tab.new`, `pane.split` stop refusing `program`/`cwd`
(`crates/server/src/dispatch/{session,tab,pane}.rs`'s three `if
param_present(...) { return Err(not_implemented_yet(...)) }` blocks are
deleted). New `crates/server/src/program_param.rs`:

```rust
pub fn program_from_request(request: &Request, default_cwd: &str) -> Result<steg_sdk::Program, ApiError>
```

reads `program` (`Argv`) and `cwd` (`Path`); absent `program` → `Program::
Shell { cwd }` (the description's own words: "Defaults to the login shell",
methods.rs:116); present → `Program::Command { argv, cwd }`. `cwd` absent →
the caller-supplied `default_cwd` — this function takes it as plain data, it
never reads the environment itself, which is the point of the injection
below.

**The one injected value, replacing "read `SHELL`/`HOME` from the process
environment in several places"**: `crates/server/src/runtime.rs`:

```rust
pub struct LoginShell {
    pub argv0: String,   // what Program::Shell resolves to, e.g. "/bin/zsh"
    pub home: String,    // the default cwd when nothing else applies
}
```

`Runtime::new` takes one `LoginShell`, read in exactly one place —
`LoginShell::from_process_env()` (new, in `runtime.rs`, called once, by
whichever binary constructs the real `Runtime`; this task's own tests
construct their own `LoginShell` values directly, with no environment
read at all) — `std::env::var("SHELL")` falling back to `"/bin/zsh"` (macOS,
R4), `std::env::var("HOME")` falling back to `"/"`. **This is a named
stand-in, not this task's final answer**: TASK-1.44's draft part B (start on
demand, detach, the login-shell environment taken from the password
database, not the process environment) will own the real base environment;
when it lands, its own resolved shell/home replaces
`LoginShell::from_process_env()`'s call site, and nothing downstream of
`LoginShell` changes, because `program_from_request` and the two cwd rules
below only ever see the already-resolved value.

**Working directory: the plan's default is rule B, described below; rule A
is the described alternative; the engineer chooses at approval.**

**Rule B (built) — inherit the live directory of the pane being split (for
`pane.split`), or of the session's current pane (for `tab.new`); `$HOME`
for `session.new`, which has no pane to inherit from, and whenever the
chain below produces no answer.** This is what `doc-9` R16 already says
the server tracks — confirmed by reading `crates/kernel/src/terminal.
rs:452-459`: `Terminal::cwd()` returns the *live* directory, last reported
by a real OSC 7 from the shell actually running there (not the pane's
stored `Program::cwd()`, which is the directory it *started* in and does
not follow a later `cd` — the same distinction TASK-1.40's own plan drew
for `restart`). Needs one new, small accessor, the same shape as the
existing `snapshot()`: `PaneHandle::cwd(&self) -> Result<Option<String>,
Error>` (new `PaneCommand::Cwd(Sender<Option<String>>)`, the worker replies
`terminal.cwd().map(str::to_string)`), called under `Runtime`'s lock before
building the new `LaunchSpec` — the source pane is already in `state.panes`
at that point, found via `tree.tab_owning_pane`/`session.current_tab()` +
`tab.current()` for `tab.new`'s case (the same lookup `dispatch::pane::
split`'s own rollback already does for "the tab's previous current pane",
mod.rs/pane.rs:88-90). Falls back to `login.home` in every case the chain
does not produce a real answer: no source pane at all (the session `tab.
new` targets has no tab/pane yet — true of nothing today, since `new_
session` always mints a first tab and pane together, but not assumed to
stay true forever), the source pane's `PaneHandle` not yet in `panes`
(should not happen under the one lock, but checked rather than
`.expect()`ed), or the source pane reporting no cwd yet (OSC 7 never
fired). `PaneHandle::cwd` itself uses a bounded `recv_timeout` (not a bare
`recv`) on its reply channel, falling back to `login.home` on timeout too —
keeping §1's claim that nothing under the lock blocks on another thread's
progress literally true even though this read is a channel round-trip to a
pane's worker, which could in principle be mid-`vt_write` on a large flood
(R19's worst case). A timeout here is treated the same as "no answer yet",
not an error. I built this one because it is what a real terminal
multiplexer does, and it is enabled by state TASK-1.39 already tracks for
exactly this reason (R16).

**Rule A (not built, the described alternative) — `$HOME` always.** Every
`session.new`/`tab.new`/`pane.split` with no `cwd` given gets `login.home`,
unconditionally. **If the engineer picks this instead**: delete `PaneHandle
::cwd` and `PaneCommand::Cwd` from `pane.rs`, and `program_from_request`'s
caller passes `login.home` directly rather than resolving a source pane
first — one function's body changes, nothing else in this plan does.

`Program::Shell` resolves to `login.argv0` (never a second, separate
environment read).

`Runtime::handle`'s wrapper for the three starting methods, under the one
lock (§1):

1. Call the existing `dispatch()` arm unchanged. Its own body additionally
   calls `tree.start_program(new_pane, program)` right after minting (new
   code in the three arms) — still pure.
2. On success, resolve the cwd default (rule B: `PaneHandle::cwd()` on the
   inherited-from pane, already in `panes`; rule A: `login.home`), build a
   `LaunchSpec` (argv, the five `STEG_*` vars + `PATH` via the existing
   `pane_environment`, and a size — §4), and call `pane::start(spec,
   new_pane, launcher.clone(), events.clone())`.
3. **On I/O failure** (`Error::Pty` — a bad command, or, pending the
   experiment below, a bad `cwd`): roll back the mint exactly as
   `pane.split`'s existing label-rejection path already does
   (`tree.close_pane`/`close_tab`/`close_session` on whichever was just
   minted — for `tab.new`/`session.new` this also undoes the brand-new tab
   or session, leaving no pane, tab *or* session behind, as AC #1 now says
   explicitly for all three methods) and return the new `ErrorCode::
   ProgramFailed` (§6).
4. On success, insert the `PaneHandle` into `panes`. §4's general
   post-dispatch step (not special-cased here) then reapplies every
   currently-sized tab's sizes, which covers both this new pane and any
   sibling whose share of the tab changed because of the split — nothing
   in this wrapper resizes a second pane itself.
5. Return the original `Outcome`.

**Experiment, first step of phase 2:** does a bad `cwd` fail
`spawn_command` synchronously (same path as `ENOENT`), or does the child
fork, `chdir` fail inside it, and it exit immediately with a nonzero code
(the *normal* exit path — `Ended{exit_code}`, no rollback, pane stays per
R97)? AC #1 groups "a program that cannot be started" as one thing; I do
not assume a bad `cwd` takes the same path as a bad command. Whichever it
is, name it in the notes — no code change either way, since both outcomes
already have a handler.

## 3. Exit (AC #2)

Covered structurally by §1's drain thread for the natural path, including
the session's-last-tab cascade (§1). `pane.show` (today `"list" | "show" =>
not_implemented_yet`, pane.rs:37-39) gets its own arm, pure, inside the
existing `dispatch`: a new `steg_sdk::PaneRecord` (`crates/sdk/src/
program.rs` or a new `record.rs`, same reasoning as `SessionRecord` — this
crosses the API, R105) with `id`, `label`, `cwd` (`Program::cwd()`),
`program` (a short word — "shell" or the command), `exit_code` (`Some` only
once `ProgramStatus::Ended`), `status` ("running"/"ended", `ProgramStatus`'s
own serde words). **`title` is left null**: it needs the terminal's live
OSC title, behind `PaneHandle` (I/O), and nothing in AC #2 asks for it.
`status`'s richer doc-11 states are TASK-1.41's job (in progress — see
Risks); this reports the two-state `ProgramStatus` only, which is what AC #2
itself asks for. `pane.list` stays `not_implemented_yet` — AC #2 only names
`show`.

## 4. Sizes (AC #3)

**This is a general step, not a special case of starting a program.** My
first draft only reapplied sizes right after a pane was started; that
misses every other method that can change a tab's layout or its panes'
share of it — `pane.resize`, `pane.swap`, `pane.move`, `pane.close`,
`pane.set`/`unset --zoomed`, and the three starting methods themselves (a
split can shift a *sibling*'s share, not just mint the new pane). No tab
has a declared size yet (no `client`/`declare_size` dispatch exists), so
none of this is observable today — but the moment TASK-1.45/1.47 wire a
client method that calls `declare_size`, every one of those verbs would
leave stale terminals/ptys behind if this task does not already apply
sizes generically.

Two new, pure kernel methods, `crates/kernel/src/tree.rs`:
```rust
pub fn tab_sizes_now(&self, tab: TabId) -> Option<SizeChanged>   // wraps the existing private size_changed (tree.rs:377)
pub fn sized_tabs(&self) -> Vec<TabId>                           // every tab with tab.size() == Some(_)
```
Both return nothing when no tab anywhere has a declared size (true of
every tab today).

One **public** method on `Runtime`, named for the client-method tasks that
will call it directly once they exist (declaring a size, or the latest
client changing — decision-6):
```rust
pub fn apply_size_changed(&self, change: &SizeChanged)
```
locks `state` itself and, for each `(pane, size)` in `change.panes`, calls
`PaneHandle::resize(size.cols, size.rows)` on whichever of those panes is
in `panes` (`resize` is a fire-and-forget channel send, not a round-trip,
so this is cheap even under a lock). A second, private function,
`apply_size_changed_locked(state: &mut State, change: &SizeChanged)`, is
the same logic taking an *already-locked* `State` — what `Runtime::handle`
itself calls, to avoid the public method re-locking a `Mutex` the request
is already holding (which would deadlock on a non-reentrant lock).

`Runtime::handle`'s own general step, after *every* successful `dispatch()`
call (not only the three starting methods — "simply every tab touched",
cheap because `sized_tabs()` is empty on every server until a client
declares a size, and harmless where it is not empty: reapplying the same
size twice is a no-op send, not a state change):

```rust
for tab in state.tree.sized_tabs() {
    if let Some(change) = state.tree.tab_sizes_now(tab) {
        apply_size_changed_locked(&mut state, &change);
    }
}
```

This single step is what the three starting methods now rely on too —
§2's start-wrapper decides the *new* pane's own initial `LaunchSpec.size`
(from `tab_sizes_now` if the tab already has one, else the stated literal
default `DEFAULT_PANE_SIZE: (u16, u16) = (80, 24)`, `crates/server/src/
pane.rs`, beside `DEFAULT_SCROLLBACK_LINES` — named, not guessed, since no
client has ever reported a size) but does not itself resize any sibling
pane; this general step does that, uniformly, for every method that could
have changed anything, with no per-verb list to keep in sync by hand. This
is also why `pane.move` needs nothing special despite touching two tabs at
once: `sized_tabs()` iterates every sized tab in the tree, not a set this
step has to be told about, so both the source tab (which lost a pane) and
the destination tab (which gained one) are recomputed if either is sized —
one blanket pass, not two call sites to remember.

**`pane.set`/`unset --zoomed` (fill/unfill), named explicitly in the
brief**: the general step runs for these too, with no special case, and —
per R96 ("filling changes nothing in the layout tree... the other panes
keep running") — it is a deliberate no-op: `tab_sizes_now` returns the
*same* `SizeChanged` before and after a fill, since the layout tree itself
did not change, so nothing is actually resized. Whether a *filled* pane's
own terminal/pty should instead be resized to the whole tab while it fills
it (what tmux's zoom does) is a different, real question that neither R96
nor any criterion here decides — left to whichever task builds zoom's
rendering, not missed.

**How AC #3's two halves are actually observed**: "applied to its terminal"
is `PaneHandle::snapshot()` — its `Screen` carries its own dimensions.
"applied to its pty" has no handle to ask (`RealLauncher` moves `pair.
master` into the `resize` closure, pty.rs:118-129, so nothing outside it can
call `MasterPty::get_size()` afterward) — the proof is the pty's own real
effect: a real `sh -c 'stty size'` pane, resized before the shell reads its
controlling terminal's size, with `snapshot()` afterward showing the line
it printed matching what was applied.

**Proving it without client dispatch existing**: `declare_size` is already
public on `Tree` (`crates/kernel/src/tree.rs:1198`, built and tested by
TASK-1.42, just not yet reachable from any method) — a test inside
`runtime.rs`'s own `#[cfg(test)]` module (which has the same access to
`Runtime`'s private `state` field any other code in that file does) calls
`state.tree.connect_client`/`view_pane`/`declare_size` directly against the
real `Tree` a real `Runtime` owns, to give a tab a real size with no
test-only hook added to shipped code — these are exactly the methods the
tasks that build `client.*` dispatch will call from inside `dispatch`
itself, not a shortcut invented for this test. From there the test drives
`pane.split`, `pane.resize`, `pane.close` and a swap through `Runtime::
handle`, and after each asserts every *remaining* pane's terminal
(`snapshot`) and pty (a real `stty size` pane, checked at least once per
distinct code path rather than after every single step, to keep the test
tractable) match the kernel's own `tab_sizes_now` for it exactly.

**The under-floor case, and how it is produced**: R100's own last clause —
"a window growing smaller refuses and closes nothing: a pane may go under
the floor that way, and its terminal is cropped" — means `declare_size`
*can* produce a `SizeChanged` with a pane under 20×4, and the kernel
already does this correctly (`tree.rs`'s own `set_tab_size_smaller_
always_succeeds_and_closes_nothing` test). The test exercises this through
the exact seam future tasks will use: declare a smaller size directly on
`state.tree` (getting the under-floor `SizeChanged` back), then call
`runtime.apply_size_changed(&change)` — the **public** method — directly,
and assert the affected pane's terminal and pty really did shrink under
the floor (cropped, not refused, not clamped back up): this proves `Runtime`
applies whatever the kernel computes faithfully, with no floor logic of
its own to get wrong, and proves the public entry point itself, not only
the private one `handle` uses internally.

**Pixel cell size, its source and what replaces it**: `Terminal::resize`
(terminal.rs:418) already hardcodes `0, 0` into the engine call, and
`RealLauncher`'s `resize`/`openpty` (pty.rs:84-89,119-127) already hardcode
it too — there is no plumbing left for this task to add. Zero is `winsize`'s
own "unknown" convention; no app is attached to a server-only M1 build, so
there is no real pixel size to pass, and fabricating one would tell a
program a geometry that is wrong. terminal.rs:395's own comment currently
names TASK-1.42 as the task that plumbs a real value through — TASK-1.42 is
done and did not, because nothing client-facing calls the pixel path yet;
correcting that comment to name the real source (the app's terminal view,
TASK-1.56, once a window exists to report pixel dimensions) is part of this
task's own diff.

## 5. The refusal, and `--force`'s stated gap (AC #4, #5)

**The missing refusal, found by re-reading `close_pane`/`close_tab`/
`close_session` (tree.rs:489,282,218): none of them check anything today.**
`dispatch::pane::close`/`tab::close`/`session::close` call them straight
through with no status check at all. The description already carries
everything needed (confirmed in `crates/sdk/src/description/methods.rs`):
`pane.close`/`tab.close`/`session.close` already list a `force` switch and
already list `ErrorCode::ProgramRunning` in their `errors`. **No sdk/
description change is needed.** The check is a dispatch-level policy over
kernel state, pure, and stays *inside* the existing `dispatch` function —
no `Runtime` wrapper needed for these three methods at all, since reading
`Pane::status()` needs no launcher and no `PaneHandle`:

```rust
fn running_panes(tree: &Tree, scope: /* one pane, or every pane of a tab, or of every tab of a session */) -> Vec<PaneId>
```

`pane.close` checks one pane; `tab.close` every pane of the target tab
(`Tab::panes()`); `session.close` every pane of every tab of the target
session (`Session::tabs().flat_map(Tab::panes)`). Without `--force` and a
non-empty list: `ApiError::new(ErrorCode::ProgramRunning, ...)`, nothing
touched — proven by a test per noun that the tree is byte-for-byte
unchanged afterward. This holds even for a shell pane whose own exit would
close it on its own (R97): a manual close without `--force` still refuses
while that shell is running — the two are different triggers (the program
ending itself, versus a person asking to end the pane) and only one of
them is "force".

**With `--force`: AC #4 says plainly that it "stays as it is today" —
unconditional, straight through to `tree.close_pane`/`close_tab`/
`close_session`, exactly the no-check behaviour this whole task found and
is otherwise fixing.** I did not make `--force` answer `NotImplementedYet`
instead, because the acceptance criterion already settles this (the
engineer's own rewrite: "`--force` stays as it is today until the
forced-close task") — there is nothing left for me to pick between.

**What that gap means, stated rather than left implicit, since this task
now owns a `Runtime` that makes the gap concrete in a new way it wasn't
before**: forcing a close on a pane with a program still running removes
the pane from the tree but does **not** touch its `PaneHandle` — `Runtime`
never learns the close happened, because the close-refusal and the close
itself are both still inside plain `dispatch`, which has no access to
`panes`. The process keeps running, orphaned: reachable by nothing (its
`PaneId` is gone from the tree, so no API method can address it), but not
leaked in the sense AC #5 forbids — its thread is still alive, still
capable of ending on its own. When it eventually does, the relay's
`PaneEvent::Exited` reaches `apply_event` (§1), which calls `tree.
record_exit` on a `PaneId` the tree no longer has, gets `Err(Error::
PaneNotFound)`, and — per §1's own handling, written for exactly this case —
removes the now-pointless `PaneHandle` from `panes` and stops, without a
panic. This is the one new edge case this task's own structure introduces
(TASK-1.40 could not hit it, since dispatch never called `start_program`
before now), gets its own test (§5's proof, below), and is exactly the gap
part two (the forced-close task) closes, starting from this section.

## 6. Error codes (AC #6)

**No new `ErrorCode` for the refusal** — `ProgramRunning` already exists, is
already on all three methods' `errors` list, and its own doc comment ("A
program is still running and `--force` was not given") is a verbatim match.

### For the engineer: a new public error code, `ErrorCode::ProgramFailed`

Surfaced by §2's rollback, not by the four kernel errors the brief named:
a real OS-level spawn failure (a bad command, and, pending the `cwd`
experiment, possibly a bad working directory) needs a code, and nothing
existing fits. `BadRequest` is "did not match the description"
(validation-shaped) and maps to CLI exit status **2** ("bad usage"),
confirmed from `crates/sdk/src/description/methods.rs:1472` — but the
request here was perfectly well-formed; the OS refused it, which is
doc-2 section 6's status **1** ("the command ran and failed").

**This does not add a new row to doc-2 section 6's table of exit
statuses** — all six already exist, and `ProgramFailed` maps to the
existing status **1**, the same one `ProgramRunning`/`SplitTooSmall`/
`EngineFailed` already use. Nothing in doc-2's own prose names error
codes at all (that table is generated from the sdk's description, not
written by hand), so there is no doc-2 text to edit either way. **What
does change, and is new public surface the generated CLI (TASK-1.49/1.50,
not yet built) will consume mechanically (R32), not an internal
refactor**:
- `crates/sdk/src/description/shapes.rs` — a new `ErrorCode::
  ProgramFailed` variant.
- its `ErrorRecord` entry (wherever the sdk builds `description().errors`
  from each `ErrorCode`) — summary text, `exit_status: 1`.
- `crates/sdk/src/description/methods.rs` — added to `pane.split`/
  `tab.new`/`session.new`'s own `errors: vec![...]` lists, the three
  methods that can now fail to start a real program.
- `crates/server/src/dispatch/server_error.rs` (new) — maps
  `steg_server::Error::Pty` to it; this file, not `kernel_error.rs`,
  since the source is the server's own I/O, never the kernel's.

**The four kernel errors the brief named, checked against doc-2, each with
a reason, none changed**:
- `ProgramAlreadyStarted`/`ProgramStillRunning` → `ProgramRunning` — already
  correct (TASK-1.40's second review fixed this).
- `ProgramNotStarted` — reachable only from this task's own internal `tree.
  record_exit` call in `apply_event` (§1), never from a client request. Per
  `Pane`'s own invariant (program always started immediately after minting,
  before anything else runs — enforced by `Runtime`'s single lock, §1), this
  fires only if that invariant is broken, which is a bug, not a client-
  facing error. Left as `BadRequest` in `kernel_error.rs` (unchanged, dead
  in practice).
- `ShellCannotRestart` — doc-2 names no verb anywhere for "run a command
  pane's program again" (checked: no "restart" string in doc-2 at all, and
  `pane_methods()` has no such method). `Tree::restart_program` stays
  unreachable through dispatch after this task too. Not decided here.
- `ClientNotViewingPane`/`ClientNotViewingTab` — still unreachable through
  anything this task adds. Not decided here, same as today.

## 7. `dispatch/mod.rs`

At 358 lines today, before this task's own additions. This task:
- Moves the `param_*`/`target_*`/`resolve_*`/`tab_mut`/`pane_mut`/
  `session_owning_tab`/`find_tab`/`wire_position_to_index` helpers
  (mod.rs:91-242) into a new `crates/server/src/dispatch/lookup.rs`,
  re-exported `pub(super)` the same as today.
- Keeps `mod.rs` to: `Context`, the `dispatch()` routing function, `not_
  implemented_yet`/`bad_request`, and the completeness test — genuinely
  about *routing*, nothing a shared utility.
- Adds no new behaviour to `mod.rs` itself: the refusal (§5), the program-
  starting logic (§2, split between the pure `dispatch` arm and `Runtime`'s
  thin wrapper), and `pane.show` (§3) all land in `pane.rs`/`tab.rs`/
  `session.rs`/`runtime.rs`, not in `mod.rs`, so this split happens before
  any of this task's own logic would have grown the file further.

## Files that change

- `crates/server/src/runtime.rs` — new: `Runtime`/`State`/`LoginShell`, the
  drain thread, `apply_event`, `handle` (the three-method start-wrapper and
  the general post-dispatch sizing step, §4), `apply_size_changed`
  (public) and `apply_size_changed_locked` (private).
- `crates/server/src/program_param.rs` — new: `program_from_request`, its
  own unit tests (pure, no environment reads).
- `crates/server/src/pane.rs` — new `PaneCommand::Cwd`, `PaneHandle::cwd`
  (rule B, §2); `DEFAULT_PANE_SIZE`.
- `crates/server/src/dispatch/{session,tab,pane}.rs` — delete the program/
  cwd refusal; add `pane.show`'s record (§3); `pane.close`/`tab.close`/
  `session.close` gain the pure refusal check (§5), still calling `tree.
  close_pane`/etc directly (unchanged for the `--force` path).
- `crates/server/src/dispatch/mod.rs` — shrinks (§7).
- `crates/server/src/dispatch/lookup.rs` — new (§7).
- `crates/server/src/dispatch/server_error.rs` — new (§6).
- `crates/kernel/src/tree.rs` — `Tree::tab_sizes_now`, `Tree::sized_tabs`
  (§4), pure, new tests.
- `crates/kernel/src/terminal.rs:395` — comment corrected (§4).
- `crates/sdk/src/program.rs` (or a new `record.rs`) — `PaneRecord` (§3).
- `crates/sdk/src/description/shapes.rs`, `methods.rs` — `ErrorCode::
  ProgramFailed`, its exit status, its presence on `pane.split`/`tab.new`/
  `session.new`'s `errors` (§6); `pane.show`'s `Returns` (§3).
- `crates/server/tests/real_pty.rs` and a new `crates/server/tests/
  start.rs` — the cwd experiment, the proofs below.

## Order of work

1. Bring in `origin/main` first, before anything else, including the
   experiment below — it should answer against current code, not whatever
   this plan was written against. Two sibling tasks may have landed since:
   TASK-1.41 (status, planning) and TASK-1.48 (settings, in review); adapt
   to whichever did (see Risks).
2. The `cwd`-failure experiment (§2) — a scratch program first, then a
   committed test either way.
3. `Tree::tab_sizes_now` and `Tree::sized_tabs` (§4), pure, kernel-only,
   land first and alone.
4. `program_param.rs` and `LoginShell` (§2), pure/data-only, testable
   alone.
5. `PaneRecord`/`pane.show` (§3).
6. The pure close-refusal (§5) — small, and independent of `Runtime`.
7. `Runtime`'s structure, drain thread and `apply_event` (§1), with the
   shell-closes-itself and last-tab-of-session proofs.
8. `Runtime`'s start-wrapper (§2) and the general post-dispatch sizing
   step (§4) together — the second is not an afterthought on the first,
   since the start-wrapper's own new pane relies on the general step to
   resize its siblings — with the ENOENT-rollback, cwd-resolution, and
   the full §4 sizing proofs (including the under-floor case).
9. `dispatch/lookup.rs` split (§7).
10. `just check` at each numbered step's own boundary, not only at the end.

## Risks

- **TASK-1.41 (status) and TASK-1.48 (settings) land mid-build.** TASK-1.41
  is expected to add a moment parameter to `Tree::record_exit` and rename
  `Pane::status` to `Pane::program_status` — if it lands first, `apply_event`
  (§1) and the refusal check's `Pane::status()` calls (§5) are updated to
  the new names/signature as part of step 1's merge, with whatever moment
  source TASK-1.41 settles on (not guessed here). TASK-1.48 is expected to
  add a `Settings` parameter to `dispatch` — if it lands first,
  `Runtime::handle`'s call into `dispatch` threads `state.settings` through
  at the same step; `State` already reserves the field for this. Neither
  changes this plan's shape, both are named rather than assumed away.
- **The cwd rule (A vs. B, §2) is a UX call I made a recommendation on, not
  a closed one.** Both are small, isolated changes; switching later costs
  one function and, for rule A, deleting `PaneHandle::cwd`.
- **The `--force` gap (§5) is a real, user-visible limitation this task
  ships with, not a hidden one**: a forced close on a running program
  orphans its process until it exits on its own. Named plainly, with its
  own test, because the acceptance criteria decided `--force` stays
  unchanged here rather than refusing outright.

## Proof

`just check` (format, lint, boundaries, licences, affected tests, every
package alone, architecture, recipe names), recorded via `asdlc
record-check` at each numbered step above.

Clause by clause:
- **AC #1**: real-pty tests — a command and a shell started through
  `Runtime::handle`, in a resolved `cwd` (both rules exercised, or the
  chosen default plus the alternative named as untested-by-choice if the
  engineer picks rule A instead); for each of the three methods, an
  `ENOENT` argv leaves the tree with no new session/tab/pane (a before/
  after count) and returns `ProgramFailed`; the `cwd`-failure experiment's
  actual result, with its own test.
- **AC #2**: `pane.show`'s record reflects `Running`/`Ended{exit_code}`
  (kernel-level, no doubles, plus one real-pty case); the shell-exit-closes-
  itself cascade proven two ways per §1 (direct `apply_event`, and
  end-to-end via a real process and a bounded `recv_timeout`); a dedicated
  case for "the last pane of the last tab of a session" — a one-pane,
  one-tab session's shell exiting, asserting the session itself is gone
  from `tree.sessions()` afterward, not only the pane.
- **AC #3**: `Tree::tab_sizes_now`/`sized_tabs` unit tests (pure); the §4
  integration test — a tab sized directly at the kernel level (bypassing
  the unbuilt `client` dispatch, the only way to get a tab size to exist
  at all today), then `pane.split`, `pane.resize`, `pane.close` and a swap
  each run through `Runtime::handle`, asserting after each that every
  *remaining* pane's terminal (`PaneHandle::snapshot()`) and pty (a real
  `sh -c 'stty size'` pane) match the kernel's own size for it exactly —
  proving the general step, not only the start path; the under-floor case
  via `apply_size_changed` called directly (the public entry point), with
  the affected pane's terminal and pty asserted cropped, not refused.
- **AC #4**: the refusal with `ProgramRunning` and a byte-for-byte unchanged
  tree, for a pane/tab/session each, including the shell-whose-own-exit-
  would-close-it case; a `--force` close on a pane running a real `sh -c
  'sleep 1000'` (a program that cannot exit on its own within the test, so
  the proof carries no race against its own lifetime) proceeding exactly as
  an un-forced close on an already-ended pane does (the stated gap, §5),
  asserting the tree loses the pane *and* the orphaned `PaneHandle`'s real
  process is still alive right after (`process_exists`, `pane.rs`'s own
  helper, already `kill -0` via `std::process::Command` — no `unsafe`); the
  test then ends that process itself, deliberately, outside the production
  path this task builds (`std::process::Command::new("kill")` by pid, the
  same tool `process_exists` already uses, not `libc::kill` — keeping this
  test's own diff as `unsafe`-free as the rest of it, since ending a
  program on purpose is part two's job, not something this test's own
  plumbing should need a new mechanism for) and asserts `apply_event`
  handles the resulting `Err(PaneNotFound)` without panicking, with the
  stale handle finally removed from `panes` — proving the gap is exactly
  what §5 says, not more and not less.
- **AC #5**: every path *this task* creates that can end a pane — a normal
  exit (command pane, stays; shell pane, cascades), a start that fails
  (rollback, §2) — reaped and thread-ended, via the existing `process_
  exists`/`join_within` helpers; no `unsafe` anywhere in this task's diff
  (trivially true — nothing here signals a process).
- **AC #6**: `map_kernel_error`'s existing tests, unchanged, plus `Program
  Failed`'s own mapping test; a test that `ShellCannotRestart` and
  `ProgramNotStarted` remain absent from every method's `errors` list.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Split on 2026-11-15 under the engineer's standing decision to split by default, along the seam this task's description named and the worker's plan recommended. This task is now part one: programs started from the API, exits, sizes, and the refusal to close a running program without --force. Part two, a draft under TASK-1 depending on this task, takes --force actually ending a program (the worker's plan section 5 from 'With --force' onward, and the experiment it names). The plan on this task was written for both parts and is to be rewritten for part one only.

Check FAILED: `just check` exited 101 at 2026-11-15T20:30:42+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.76 on task-1.76-server-start-a-pane-s-program-from-the at d968e1111070, last 40 of 75 lines:

```text
test result: ok. 52 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s

     Running tests/program.rs (target/debug/deps/program-89613f7a57fade38)

running 5 tests
test a_programs_exit_code_reaches_the_event_channel ... ok
test resize_reaches_the_terminal_even_with_no_program_running ... ok
test a_pane_with_no_program_running_accepts_input_as_a_no_op ... ok
test output_printed_before_exit_is_in_the_snapshot ... ok
test restarting_keeps_the_same_terminal_so_old_output_survives_a_new_run ... ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests/real_pty.rs (target/debug/deps/real_pty-a87d5c4aaa32cd2f)

running 6 tests
test a_command_that_does_not_exist_fails_synchronously_with_no_thread_to_join ... ok
test experiment_a_nonexistent_cwd_scratch ... FAILED
test a_real_program_runs_in_the_given_working_directory ... ok
test a_shell_panes_real_exit_reaches_the_kernels_cascade ... ok
test a_command_panes_real_exit_code_is_known_it_stays_and_can_run_again ... ok
test a_real_programs_environment_carries_the_five_variables_and_path ... ok

failures:

---- experiment_a_nonexistent_cwd_scratch stdout ----

thread 'experiment_a_nonexistent_cwd_scratch' (10456452) panicked at crates/server/tests/real_pty.rs:275:13:
SPAWNED THEN EXITED: Exited { pane: PaneId(1), exit_code: 0 }
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    experiment_a_nonexistent_cwd_scratch

test result: FAILED. 5 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.15s

error: test failed, to rerun pass `-p steg-server --test real_pty`
full output: .quiet/test-steg-server.log
error: recipe `check-packages` failed with exit code 101
```

Check passed: `just check` exited 0 at 2026-11-15T20:37:15+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.76 on task-1.76-server-start-a-pane-s-program-from-the at d968e1111070 with uncommitted changes:

```text
ok      format check (0.2s)
ok      lint (0.8s)
ok      boundaries (0.1s)
ok      licences (0.7s)
ok      tests (-p steg -p steg-kernel -p steg-server) (37.3s)
ok      build (steg-sdk) (0.1s)
ok      test (steg-sdk) (0.2s)
ok      test (steg-sdk, local-socket) (0.4s)
ok      build (steg-kernel) (31.3s)
ok      test (steg-kernel) (0.7s)
ok      build (steg-server) (0.1s)
ok      test (steg-server) (1.3s)
ok      build (steg) (0.2s)
ok      test (steg) (0.1s)
ok      build (steg-app) (18.3s)
ok      test (steg-app) (0.9s)
ok      tool tests (1.9s)
ok      architecture (1.5s)
ok      recipe names (0.1s)
```

Check FAILED: `just check` exited 1 at 2026-11-15T20:55:27+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.76 on task-1.76-server-start-a-pane-s-program-from-the at d968e1111070 with uncommitted changes, last 40 of 64 lines:

```text
 
     fn lock(&self) -> std::sync::MutexGuard<'_, State> {
[31m-        self.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
(B[m[32m+        self.state
(B[m[32m+            .lock()
(B[m[32m+            .unwrap_or_else(std::sync::PoisonError::into_inner)
(B[m     }
 
     /// `session.new`/`tab.new`/`pane.split`: mints through the existing,
Diff in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.76/crates/server/src/runtime.rs:279:
 
         let spec = self.launch_spec(&program, size);
 
[31m-        match pane::start(spec, new_pane, Arc::clone(&self.launcher), self.events.clone()) {
(B[m[32m+        match pane::start(
(B[m[32m+            spec,
(B[m[32m+            new_pane,
(B[m[32m+            Arc::clone(&self.launcher),
(B[m[32m+            self.events.clone(),
(B[m[32m+        ) {
(B[m             Ok(handle) => {
                 state.panes.insert(new_pane, handle);
                 reapply_every_sized_tab(&mut state);
Diff in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.76/crates/server/src/runtime.rs:587:
             settings: steg_kernel::Settings::new(),
         };
 
[31m-        apply_event_locked(&mut state, PaneEvent::Exited { pane: gone, exit_code: 0 });
(B[m[32m+        apply_event_locked(
(B[m[32m+            &mut state,
(B[m[32m+            PaneEvent::Exited {
(B[m[32m+                pane: gone,
(B[m[32m+                exit_code: 0,
(B[m[32m+            },
(B[m[32m+        );
(B[m 
         assert!(
             !state.panes.contains_key(&gone),
full output: .quiet/format-check.log
error: recipe `check-fmt` failed on line 126 with exit code 1
```

Check FAILED: `just check` exited 101 at 2026-11-15T20:55:40+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.76 on task-1.76-server-start-a-pane-s-program-from-the at d968e1111070 with uncommitted changes, last 40 of 65 lines:

```text

error: doc list item without indentation
   --> crates/server/src/dispatch/pane.rs:183:5
    |
183 | /// that forces the program itself to end).
    |     ^
    |
    = help: if this is supposed to be its own paragraph, add a blank line
    = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.98.0/index.html#doc_lazy_continuation
help: indent this line
    |
183 | ///   that forces the program itself to end).
    |     ++

error: docs for function which may panic missing `# Panics` section
   --> crates/server/src/runtime.rs:91:5
    |
 91 |     pub fn new(launcher: Arc<dyn ProgramLauncher>, login: LoginShell) -> Self {
    |     ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
    |
note: first possible panic found here
   --> crates/server/src/runtime.rs:100:21
    |
100 |           let drain = thread::Builder::new()
    |  _____________________^
101 | |             .name("steg-runtime-drain".to_string())
102 | |             .spawn(move || {
103 | |                 for event in events_rx {
...   |
118 | |             })
119 | |             .expect("spawning the one drain thread at construction");
    | |____________________________________________________________________^
    = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.98.0/index.html#missing_panics_doc
    = note: requested on the command line with `-D clippy::missing-panics-doc`

error: could not compile `steg-server` (lib) due to 7 previous errors
warning: build failed, waiting for other jobs to finish...
error: could not compile `steg-server` (lib test) due to 6 previous errors
full output: .quiet/lint.log
error: recipe `lint` failed with exit code 101
```

Check passed: `just check` exited 0 at 2026-11-15T20:59:06+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.76 on task-1.76-server-start-a-pane-s-program-from-the at d968e1111070 with uncommitted changes:

```text
ok      format check (0.2s)
ok      lint (0.4s)
ok      boundaries (0.1s)
ok      licences (0.8s)
ok      tests (-p steg -p steg-app -p steg-kernel -p steg-sdk -p steg-server) (49.8s)
ok      build (steg-sdk) (0.8s)
ok      test (steg-sdk) (0.6s)
ok      test (steg-sdk, local-socket) (2.1s)
ok      build (steg-kernel) (0.6s)
ok      test (steg-kernel) (1.1s)
ok      build (steg-server) (32.1s)
ok      test (steg-server) (2.7s)
ok      build (steg) (0.2s)
ok      test (steg) (0.4s)
ok      build (steg-app) (0.8s)
ok      test (steg-app) (0.8s)
ok      tool tests (1.4s)
ok      architecture (1.1s)
ok      recipe names (0.1s)
```

Check passed: `just check` exited 0 at 2026-11-15T21:15:00+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.76 on task-1.76-server-start-a-pane-s-program-from-the at d968e1111070 with uncommitted changes:

```text
ok      format check (0.1s)
ok      lint (0.5s)
ok      boundaries (0.2s)
ok      licences (1.0s)
ok      tests (-p steg -p steg-app -p steg-kernel -p steg-sdk -p steg-server) (6.3s)
ok      build (steg-sdk) (0.4s)
ok      test (steg-sdk) (0.7s)
ok      test (steg-sdk, local-socket) (1.7s)
ok      build (steg-kernel) (0.4s)
ok      test (steg-kernel) (0.9s)
ok      build (steg-server) (30.6s)
ok      test (steg-server) (3.2s)
ok      build (steg) (0.3s)
ok      test (steg) (0.4s)
ok      build (steg-app) (0.6s)
ok      test (steg-app) (0.8s)
ok      tool tests (1.3s)
ok      architecture (1.1s)
ok      recipe names (0.1s)
```

Check passed: `just check` exited 0 at 2026-11-15T21:17:42+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.76 on task-1.76-server-start-a-pane-s-program-from-the at fdef2fe3f967:

```text
ok      format check (0.1s)
ok      lint (1.0s)
ok      boundaries (0.1s)
ok      licences (0.7s)
ok      tests (-p steg -p steg-app -p steg-kernel -p steg-sdk -p steg-server) (5.3s)
ok      build (steg-sdk) (0.4s)
ok      test (steg-sdk) (1.3s)
ok      test (steg-sdk, local-socket) (1.8s)
ok      build (steg-kernel) (0.6s)
ok      test (steg-kernel) (0.8s)
ok      build (steg-server) (29.3s)
ok      test (steg-server) (3.4s)
ok      build (steg) (0.3s)
ok      test (steg) (0.5s)
ok      build (steg-app) (0.7s)
ok      test (steg-app) (0.8s)
ok      tool tests (1.0s)
ok      architecture (1.0s)
ok      recipe names (0.1s)
```

Built as planned, with one departure noted below (the plan itself is unedited).

What was built, by plan section:

**§1 (the owning structure).** `crates/server/src/runtime.rs`: `Runtime` holds `Arc<Mutex<State>>` (`tree`, `panes`, `settings` - the last added because TASK-1.48 merged into `origin/main` mid-build; adapted per the plan's own Risks, no shape change needed since `State` already reserved the field) plus the launcher, the injected `LoginShell`, the `PaneEvent` sender and the one drain thread's `JoinHandle`. `Runtime::handle` intercepts only `session.new`/`tab.new`/`pane.split`; every other method delegates straight to the existing, unchanged `dispatch()` (now taking `&mut Settings` too, per TASK-1.48).

**§2 (starting).** `program_param.rs`'s `program_from_request` is pure. Rule B (inherit the live directory of the pane being split, or of the session's current pane for `tab.new`; home otherwise) is built, via a new `PaneHandle::cwd` (bounded `recv_timeout`, same shape as `snapshot`). `Runtime::start_and_dispatch` validates, resolves the cwd default, calls the existing `dispatch()` arm to mint, calls `tree.start_program` itself right after (under the same lock - the real implementation detail: it is `Runtime`, not the dispatch arm, that calls `start_program`, since rule B's cwd resolution needs `Runtime`-level access `dispatch` deliberately does not have; the plan's own §1 justification for the lock - "nothing else runs between minting and starting" - holds exactly the same either way), then does the real spawn. A spawn failure rolls back via the same kernel calls the existing label-rejection rollback uses, and returns the new `ErrorCode::ProgramFailed`.

**§3 (exit).** `pane.show` is wired, pure, returning a new `PaneRecord` (title always `None`, undocumented gap named in its own doc comment). The natural-exit cascade is `Runtime`'s drain thread calling `apply_event_locked` (the tested core) in a loop.

**§4 (sizes).** `Tree::tab_sizes_now`/`sized_tabs` (pure, new). `Runtime::apply_size_changed` (public) / `apply_size_changed_locked` (private, used by the general step). `Runtime::handle`'s general step reapplies every currently-sized tab's sizes after *any* successful request, not only the three starting methods.

**§5 (the refusal, and --force's stated gap).** The refusal lives in each noun's own `close` function (`dispatch::{pane,tab,session}`), pure, using `Pane::status()` directly - no `Runtime` involvement needed, smaller than the plan's own worst case. `--force` is untouched: it still calls `tree.close_pane`/`close_tab`/`close_session` unconditionally. `apply_event_locked`'s new `Err(Error::PaneNotFound(_))` arm is the orphan's cleanup: removes the stale `PaneHandle`, no panic.

**§6 (error codes).** `ErrorCode::ProgramFailed` added to `shapes.rs`, its `ErrorRecord` (exit status 1) in `methods.rs`, and to `pane.split`/`tab.new`/`session.new`'s own `errors` lists. No new code for the refusal (`ProgramRunning` already existed on all three methods). `ProgramNotStarted`/`ShellCannotRestart` stay mapped to `BadRequest`, unchanged - confirmed by a new test that no described method has a `restart` verb.

**§7.** `dispatch/mod.rs`'s generic `param_*`/`target_*`/`resolve_*`/`tab_mut`/`pane_mut`/`wire_position_to_index` helpers moved to `dispatch/lookup.rs`, re-exported `pub(crate)` unchanged for every other module's own `use super::{...}`. `session_owning_tab`/`resolve_session_str` dropped to private-to-`lookup.rs`, since nothing outside it actually calls them directly (only `target_session`/`tab_mut` do, internally) - a smaller surface than the plan's own file list implied, caught by the unused-import lint itself.

**Departure from the plan** (noted here, plan unedited): `ErrorCode::ProgramFailed` is mapped from `steg_server::Error::Pty` inline, in `Runtime::start_and_dispatch`'s own match arm (`Response::Error(ApiError::new(ErrorCode::ProgramFailed, error.to_string()))`), not in a new `dispatch/server_error.rs`. There is exactly one error source and one resulting code here; a whole mapping module, parallel to `kernel_error.rs`'s real multi-arm match, would add structure with nothing to hold.

**The cwd experiment's real finding** (plan §2): a bad `cwd` is **not** a failure to start at all. Read from `portable-pty` 0.9.0's own source (`cmdbuilder.rs:498-507,560-576`): `CommandBuilder::as_command` silently substitutes the login shell's home directory for any `cwd` that is not a real, existing directory, and the program spawns there successfully - no error, no non-zero exit tied to the bad `cwd`. Committed as `a_nonexistent_cwd_is_not_a_failure_to_start_portable_pty_silently_uses_home` (`real_pty.rs`), asserting equality with `std::env::var("HOME")` (the same source `get_home_dir()` reads, confirmed by reading `get_base_env()`, not assumed). Left as a question for the engineer, not decided here: should `--cwd` naming a non-existent directory be refused instead of silently redirected? A doc-2 question, not a build-phase one.

Checks recorded via `asdlc record-check` at each build step: the cwd experiment's own result (first `just check`, exit 101 - the scratch experiment's own `panic!` capturing its finding, not a broken baseline, superseded moments later by the committed version of the same test); a clean `just check` once the kernel additions and the committed cwd test compiled; a clean `just check` after `Runtime`'s structure, the sizing mechanism, the close-refusal, and the `lookup.rs` split landed; a clean `just check` at the final, rebased head (`fdef2fe`) before pushing.

The real-process test suite (`steg-server`'s lib tests plus all four integration test files - `program.rs`, `real_pty.rs`, `runtime.rs` internal and external) was run 10 times in a row immediately after the sizing mechanism's signal-based tests were fixed, and 10 more times at the final head after the `lookup.rs` split: 20 consecutive runs, 0 failures. The sizing test's own real flakiness was found and fixed during this: the first `SIGWINCH`-based attempt resent the *same* target size on every retry, which is a genuine no-op once the kernel has already applied it once (no second signal for an unchanged size) - fixed by alternating the probed row count each attempt, so every retry is a real size change and gets its own real signal, confirmed stable afterward.

Coverage, clause by clause:

- **AC #1** ("start that program... with neither, the new pane runs the login shell... A program that cannot be started is reported and leaves no pane behind"): `runtime.rs` (tests/) `session_new_with_a_command_starts_it_and_pane_show_reports_it_running_then_ended`, `pane_split_with_no_program_or_cwd_runs_the_login_shell_at_home`, `a_command_that_does_not_exist_fails_and_leaves_no_session_tab_or_pane_behind`, `tab_new_with_a_bad_command_leaves_no_new_tab_or_pane_behind`, `pane_split_with_a_bad_command_leaves_no_new_pane_behind` (one per method, each asserting the before/after session+pane count is unchanged). `program_param.rs`'s own unit tests cover the pure parsing. `real_pty.rs`'s `a_nonexistent_cwd_is_not_a_failure_to_start_...` covers the cwd experiment's real finding.
- **AC #2** ("pane show reports the running program and... exit code; a shell pane that exits closes through the same cascade as a manual close... including the last pane of the last tab of a session"): `session_new_with_a_command_starts_it_and_pane_show_reports_it_running_then_ended` (running, then ended, with the right exit code and program text). `runtime.rs` (src/) `apply_event_records_an_exit_and_leaves_a_command_pane_in_place`, `apply_event_closes_a_shell_pane_through_the_same_cascade_as_a_manual_close` (asserts `tree.sessions()` is empty - the last-tab-of-session case), `a_real_exit_event_drives_the_same_cascade_apply_event_runs` (the real-process plumbing, separately from the deterministic cascade logic).
- **AC #3** ("a change of a tab's size reaches every terminal in it: applied to its terminal and to its pty, and the pixel cell size... has a stated source"): `tab_sizes_now`/`sized_tabs`'s own kernel unit tests (pure). `runtime.rs` (src/) `a_tabs_declared_size_reaches_every_pane_including_one_split_in_afterward` - a real `SIGWINCH`-reporting shell proves the pty side on an *already-running* pane, a `snapshot()` row count proves the terminal side, and splitting afterward with no size given proves the general step reapplies to both the pre-existing and the new pane, each checked against `tab_sizes_now`'s own live computation, not a hard-coded expectation. `apply_size_changed_lets_a_pane_go_under_the_floor_when_the_kernel_says_so` is the under-floor case (R100's last clause), through the public `apply_size_changed` directly. The pixel cell size's stated source (zero, `winsize`'s own "unknown") is unchanged code (`terminal.rs`/`pty.rs` already hardcoded it); `terminal.rs:395`'s comment is corrected to name TASK-1.56 instead of the already-shipped TASK-1.42.
- **AC #4** ("close... refuse while a program is running, with the ProgramRunning code"; "--force stays as it is today"): `pane_close_refuses_while_its_program_is_running_and_touches_nothing`, `tab_close_refuses_while_any_of_its_panes_is_running` (with a second, already-ended pane in the same tab, so the refusal is proven to come from the *other* pane, not from "every pane is running"), `session_close_refuses_while_any_pane_in_any_of_its_tabs_is_running`, each asserting the before/after tree is byte-for-byte unchanged via `counts()`. `pane_close_with_force_on_a_running_program_proceeds_exactly_as_today_the_stated_gap` proves `--force`'s own unconditional behaviour through the public API; the orphan's own survival and later cleanup is proven separately (next clause).
- **AC #5** ("every path this task creates that can end a pane... leaves no child unreaped and no thread running... no unsafe code is added"): `apply_event_on_a_pane_the_tree_no_longer_holds_removes_the_stale_handle_without_panicking` (a real `sleep 1000`, simulating the orphan `--force` leaves, confirmed still alive, then `apply_event_locked` on its real exit event, confirmed the stale handle is gone - the one new edge case this task's own structure introduces). `real_pty.rs`'s existing `process_exists`/`join_within` helpers, reused unchanged, back every real-process assertion in this task's own new tests. No `unsafe` appears anywhere in this diff (`git diff --stat` plus a direct `grep -n unsafe` over every changed file, both empty) - trivially true, since nothing in this task's own scope signals a process.
- **AC #6** ("the kernel errors... map to an API error code that says what happened; whether each needs its own code is decided in the plan"): `map_kernel_error`'s existing tests, unchanged. `shapes.rs`/`methods.rs`'s own `ProgramFailed` tests (the new `errors_has_twelve_codes_with_their_exit_statuses`, extended; the three methods' own `errors` lists). `no_described_method_can_reach_restart_program_or_record_exit` is the negative assertion: no method names a `restart` verb, so `ShellCannotRestart`/`ProgramNotStarted` stay unreachable and undecided, as the plan said.

`just check` recorded clean at the final head, `fdef2fe` (rebased onto `origin/main` after TASK-1.48 merged; TASK-1.41's own rename/`Moment` change had not merged as of this rebase, so there was nothing further to adapt to - checked directly in `crates/kernel/src/pane.rs` before pushing, not assumed). Pushed, PR opened: https://github.com/bruk-io/steg/pull/40.

Check FAILED: `cargo test -p steg-server --lib runtime::tests::the_general_sizing_step_reaches_every_pane_through_split_close_and_swap -- --test-threads=1` exited 101 at 2026-11-15T22:13:35+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.76 on task-1.76-server-start-a-pane-s-program-from-the at 521b3d7161cd with uncommitted changes:

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.08s
     Running unittests src/lib.rs (target/debug/deps/steg_server-76f729f7c755576f)

running 1 test
test runtime::tests::the_general_sizing_step_reaches_every_pane_through_split_close_and_swap ... FAILED

failures:

---- runtime::tests::the_general_sizing_step_reaches_every_pane_through_split_close_and_swap stdout ----

thread 'runtime::tests::the_general_sizing_step_reaches_every_pane_through_split_close_and_swap' (14462749) panicked at crates/server/src/runtime.rs:1200:17:
pane 00000002 never reported pty size 20 30; last screen: "20 61\n20 61\n20 61\n20 61\n20 61\n20 61\n20 61\n20 61\n20 61\n20 61\n20 61\n20 61\n20 61\n20 61\n20 61\n20 61\n20 61\n20 61\n20 61\n"
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    runtime::tests::the_general_sizing_step_reaches_every_pane_through_split_close_and_swap

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 53 filtered out; finished in 20.03s

error: test failed, to rerun pass `-p steg-server --lib`
```

Commits since the review: 24, 22 without the Asdlc-Worker trailer.

Check passed: `cargo test -p steg-server --lib runtime::tests::the_general_sizing_step_reaches_every_pane_through_split_close_and_swap -- --test-threads=1` exited 0 at 2026-11-15T22:13:48+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.76 on task-1.76-server-start-a-pane-s-program-from-the at 521b3d7161cd with uncommitted changes:

```text
   Compiling steg-server v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.76/crates/server)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.69s
     Running unittests src/lib.rs (target/debug/deps/steg_server-76f729f7c755576f)

running 1 test
test runtime::tests::the_general_sizing_step_reaches_every_pane_through_split_close_and_swap ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 53 filtered out; finished in 0.25s
```

Commits since the review: 24, 22 without the Asdlc-Worker trailer.

Check FAILED: `cargo test -p steg-server --test runtime pane_close_refuses_while_its_program_is_running_and_touches_nothing -- --test-threads=1` exited 101 at 2026-11-15T22:14:01+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.76 on task-1.76-server-start-a-pane-s-program-from-the at 521b3d7161cd with uncommitted changes:

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.08s
     Running tests/runtime.rs (target/debug/deps/runtime-e56fe6b4ab4a7d11)

running 1 test
test pane_close_refuses_while_its_program_is_running_and_touches_nothing ... FAILED

failures:

---- pane_close_refuses_while_its_program_is_running_and_touches_nothing stdout ----

thread 'pane_close_refuses_while_its_program_is_running_and_touches_nothing' (14478076) panicked at crates/server/tests/runtime.rs:78:32:
expected Response::Error, got null
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    pane_close_refuses_while_its_program_is_running_and_touches_nothing

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 17 filtered out; finished in 0.00s

error: test failed, to rerun pass `-p steg-server --test runtime`
```

Commits since the review: 24, 22 without the Asdlc-Worker trailer.

Check passed: `cargo test -p steg-server --test runtime pane_close_refuses_while_its_program_is_running_and_touches_nothing -- --test-threads=1` exited 0 at 2026-11-15T22:14:10+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.76 on task-1.76-server-start-a-pane-s-program-from-the at 521b3d7161cd with uncommitted changes:

```text
   Compiling steg-server v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.76/crates/server)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.57s
     Running tests/runtime.rs (target/debug/deps/runtime-e56fe6b4ab4a7d11)

running 1 test
test pane_close_refuses_while_its_program_is_running_and_touches_nothing ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 17 filtered out; finished in 0.00s
```

Commits since the review: 24, 22 without the Asdlc-Worker trailer.

Check FAILED: `cargo test -p steg-server --test runtime tab_close_refuses_while_any_of_its_panes_is_running -- --test-threads=1` exited 101 at 2026-11-15T22:14:20+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.76 on task-1.76-server-start-a-pane-s-program-from-the at 521b3d7161cd with uncommitted changes:

```text
   Compiling steg-server v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.76/crates/server)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.55s
     Running tests/runtime.rs (target/debug/deps/runtime-e56fe6b4ab4a7d11)

running 1 test
test tab_close_refuses_while_any_of_its_panes_is_running ... FAILED

failures:

---- tab_close_refuses_while_any_of_its_panes_is_running stdout ----

thread 'tab_close_refuses_while_any_of_its_panes_is_running' (14482648) panicked at crates/server/tests/runtime.rs:78:32:
expected Response::Error, got null
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    tab_close_refuses_while_any_of_its_panes_is_running

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 17 filtered out; finished in 0.01s

error: test failed, to rerun pass `-p steg-server --test runtime`
```

Commits since the review: 24, 22 without the Asdlc-Worker trailer.

Check passed: `cargo test -p steg-server --test runtime tab_close_refuses_while_any_of_its_panes_is_running -- --test-threads=1` exited 0 at 2026-11-15T22:14:29+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.76 on task-1.76-server-start-a-pane-s-program-from-the at 521b3d7161cd with uncommitted changes:

```text
   Compiling steg-server v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.76/crates/server)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.56s
     Running tests/runtime.rs (target/debug/deps/runtime-e56fe6b4ab4a7d11)

running 1 test
test tab_close_refuses_while_any_of_its_panes_is_running ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 17 filtered out; finished in 0.01s
```

Commits since the review: 24, 22 without the Asdlc-Worker trailer.

Check FAILED: `cargo test -p steg-server --test runtime session_close_refuses_while_any_pane_in_any_of_its_tabs_is_running -- --test-threads=1` exited 101 at 2026-11-15T22:14:39+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.76 on task-1.76-server-start-a-pane-s-program-from-the at 521b3d7161cd with uncommitted changes:

```text
   Compiling steg-server v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.76/crates/server)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.59s
     Running tests/runtime.rs (target/debug/deps/runtime-e56fe6b4ab4a7d11)

running 1 test
test session_close_refuses_while_any_pane_in_any_of_its_tabs_is_running ... FAILED

failures:

---- session_close_refuses_while_any_pane_in_any_of_its_tabs_is_running stdout ----

thread 'session_close_refuses_while_any_pane_in_any_of_its_tabs_is_running' (14490142) panicked at crates/server/tests/runtime.rs:78:32:
expected Response::Error, got null
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    session_close_refuses_while_any_pane_in_any_of_its_tabs_is_running

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 17 filtered out; finished in 0.01s

error: test failed, to rerun pass `-p steg-server --test runtime`
```

Commits since the review: 24, 22 without the Asdlc-Worker trailer.

Check passed: `cargo test -p steg-server --test runtime session_close_refuses_while_any_pane_in_any_of_its_tabs_is_running -- --test-threads=1` exited 0 at 2026-11-15T22:14:48+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.76 on task-1.76-server-start-a-pane-s-program-from-the at 521b3d7161cd with uncommitted changes:

```text
   Compiling steg-server v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.76/crates/server)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.55s
     Running tests/runtime.rs (target/debug/deps/runtime-e56fe6b4ab4a7d11)

running 1 test
test session_close_refuses_while_any_pane_in_any_of_its_tabs_is_running ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 17 filtered out; finished in 0.01s
```

Commits since the review: 24, 22 without the Asdlc-Worker trailer.

Check FAILED: `just check` exited 101 at 2026-11-15T22:16:10+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.76 on task-1.76-server-start-a-pane-s-program-from-the at 521b3d7161cd with uncommitted changes, last 40 of 75 lines:

```text
test program_param::tests::an_empty_program_array_is_a_command_with_no_arguments_not_a_shell ... ok
test program_param::tests::no_program_and_no_cwd_is_a_shell_at_the_default ... ok
test dispatch::tests::every_described_method_has_exactly_one_handler ... ok
test runtime::stop_tests::stop_with_no_panes_at_all_returns_at_once ... ok
test runtime::tests::apply_event_closes_a_shell_pane_through_the_same_cascade_as_a_manual_close ... ok
test runtime::tests::apply_event_records_an_exit_and_leaves_a_command_pane_in_place ... ok
test runtime::tests::login_shell_from_process_env_never_panics_and_is_never_empty ... ok
test runtime::tests::runtime_new_does_not_panic_and_starts_empty ... ok
test runtime::tests::apply_event_on_a_pane_the_tree_no_longer_holds_removes_the_stale_handle_without_panicking ... ok
test runtime::tests::a_real_exit_event_drives_the_same_cascade_apply_event_runs ... ok
test runtime::tests::apply_size_changed_lets_a_pane_go_under_the_floor_when_the_kernel_says_so ... ok
test runtime::stop_tests::stop_is_a_no_op_for_a_pane_whose_program_has_already_ended ... FAILED
test runtime::tests::pane_split_inherits_the_live_cwd_of_the_pane_it_split_from_not_its_started_in_cwd ... ok
test runtime::tests::pane_split_falls_back_to_home_when_the_inherited_live_cwd_no_longer_exists ... ok
test runtime::tests::apply_size_changed_resizes_only_the_panes_the_change_names ... ok
kill: 25632: No such process
test runtime::stop_tests::stop_ends_a_silent_program ... ok
kill: 25632: No such process
test runtime::stop_tests::stop_ends_a_program_producing_output ... ok
test runtime::tests::the_general_sizing_step_reaches_every_pane_through_split_close_and_swap ... ok
kill: 25600: No such process
test pane::tests::a_forward_failing_once_the_worker_is_gone_still_reaps_a_program_that_ends_on_its_own ... ok

failures:

---- runtime::stop_tests::stop_is_a_no_op_for_a_pane_whose_program_has_already_ended stdout ----

thread 'runtime::stop_tests::stop_is_a_no_op_for_a_pane_whose_program_has_already_ended' (14526986) panicked at crates/server/src/runtime.rs:1624:9:
the program must have fully ended, not merely exited the pty
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    runtime::stop_tests::stop_is_a_no_op_for_a_pane_whose_program_has_already_ended

test result: FAILED. 53 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.03s

error: test failed, to rerun pass `-p steg-server --lib`
full output: .quiet/test-steg-server.log
error: recipe `check-packages` failed with exit code 101
```

Commits since the review: 24, 22 without the Asdlc-Worker trailer.

Check FAILED: `just check` exited 101 at 2026-11-15T22:17:46+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.76 on task-1.76-server-start-a-pane-s-program-from-the at 521b3d7161cd with uncommitted changes, last 40 of 68 lines:

```text
test runtime::stop_tests::stop_with_no_panes_at_all_returns_at_once ... ok
test runtime::tests::login_shell_from_process_env_never_panics_and_is_never_empty ... ok
test runtime::tests::runtime_new_does_not_panic_and_starts_empty ... ok
test runtime::tests::apply_event_on_a_pane_the_tree_no_longer_holds_removes_the_stale_handle_without_panicking ... ok
test runtime::tests::a_real_exit_event_drives_the_same_cascade_apply_event_runs ... ok
test runtime::tests::apply_size_changed_lets_a_pane_go_under_the_floor_when_the_kernel_says_so ... ok
test runtime::tests::apply_size_changed_resizes_only_the_panes_the_change_names ... ok
test runtime::tests::pane_split_inherits_the_live_cwd_of_the_pane_it_split_from_not_its_started_in_cwd ... ok
kill: 32309: No such process
test runtime::stop_tests::stop_is_a_no_op_for_a_pane_whose_program_has_already_ended ... ok
test runtime::tests::pane_split_falls_back_to_home_when_the_inherited_live_cwd_no_longer_exists ... ok
test runtime::stop_tests::stop_ends_a_silent_program ... FAILED
test runtime::tests::the_general_sizing_step_reaches_every_pane_through_split_close_and_swap ... ok
kill: 32305: No such process
test pane::tests::a_forward_failing_once_the_worker_is_gone_still_reaps_a_program_that_ends_on_its_own ... ok
test runtime::stop_tests::stop_ends_a_program_producing_output ... FAILED

failures:

---- runtime::stop_tests::stop_ends_a_silent_program stdout ----

thread 'runtime::stop_tests::stop_ends_a_silent_program' (14561511) panicked at crates/server/src/runtime.rs:1605:9:
stop must end the program too
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace

---- runtime::stop_tests::stop_ends_a_program_producing_output stdout ----

thread 'runtime::stop_tests::stop_ends_a_program_producing_output' (14561510) panicked at crates/server/src/runtime.rs:1565:13:
never saw a pid written to /var/folders/rf/f34xmczn4659gm98hrsprydc0000gn/T/steg-stop-test-00000002.pid


failures:
    runtime::stop_tests::stop_ends_a_program_producing_output
    runtime::stop_tests::stop_ends_a_silent_program

test result: FAILED. 52 passed; 2 failed; 0 ignored; 0 measured; 0 filtered out; finished in 10.08s

error: test failed, to rerun pass `-p steg-server --lib`
full output: .quiet/tests-p-steg-p-steg-app-p-steg-kernel-p-steg-sdk-p-steg-server.log
error: recipe `test` failed with exit code 101
```

Commits since the review: 24, 22 without the Asdlc-Worker trailer.

Check FAILED: `just check` exited 101 at 2026-11-15T22:18:36+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.76 on task-1.76-server-start-a-pane-s-program-from-the at 521b3d7161cd with uncommitted changes, last 40 of 68 lines:

```text

     Running tests/runtime.rs (target/debug/deps/runtime-45e63e48b5c485fa)

running 18 tests
test session_new_with_a_nonexistent_cwd_fails_and_leaves_nothing_behind ... ok
test a_nonexistent_cwd_through_api_call_leaves_nothing_behind ... ok
test a_command_that_does_not_exist_fails_and_leaves_no_session_tab_or_pane_behind ... ok
test pane_split_with_no_program_or_cwd_runs_the_login_shell_at_home ... ok
test session_close_refuses_while_any_pane_in_any_of_its_tabs_is_running ... ok
test tab_new_with_a_bad_command_leaves_no_new_tab_or_pane_behind ... ok
test session_new_with_a_command_starts_it_and_pane_show_reports_it_running_then_ended ... ok
test pane_split_through_api_call_starts_a_real_program ... ok
test tab_new_through_api_call_starts_a_real_program ... ok
test pane_close_refuses_while_its_program_is_running_and_touches_nothing ... ok
test session_new_through_api_call_starts_a_real_program ... ok
test tab_new_with_a_nonexistent_cwd_leaves_no_new_tab_or_pane_behind ... ok
test pane_split_with_a_bad_command_leaves_no_new_pane_behind ... ok
test session_new_through_api_call_with_no_program_runs_the_login_shell ... ok
test pane_close_with_force_on_a_running_program_proceeds_exactly_as_today_the_stated_gap ... ok
test a_bad_command_through_api_call_leaves_nothing_behind ... ok
test tab_close_refuses_while_any_of_its_panes_is_running ... FAILED
test pane_split_with_a_nonexistent_cwd_leaves_no_new_pane_behind ... ok

failures:

---- tab_close_refuses_while_any_of_its_panes_is_running stdout ----

thread 'tab_close_refuses_while_any_of_its_panes_is_running' (14575341) panicked at crates/server/tests/runtime.rs:71:35:
expected Response::Ok, got ApiError { code: ProgramFailed, message: "the pseudo-terminal failed: failed to openpty: Os { code: -6, kind: Uncategorized, message: \"Unknown error: -6\" }", candidates: [] }
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    tab_close_refuses_while_any_of_its_panes_is_running

test result: FAILED. 17 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.43s

error: test failed, to rerun pass `-p steg-server --test runtime`
full output: .quiet/tests-p-steg-p-steg-app-p-steg-kernel-p-steg-sdk-p-steg-server.log
error: recipe `test` failed with exit code 101
```

Commits since the review: 24, 22 without the Asdlc-Worker trailer.

Check FAILED: `just check` exited 101 at 2026-11-15T22:18:50+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.76 on task-1.76-server-start-a-pane-s-program-from-the at 521b3d7161cd with uncommitted changes, last 40 of 68 lines:

```text

     Running tests/runtime.rs (target/debug/deps/runtime-45e63e48b5c485fa)

running 18 tests
test a_nonexistent_cwd_through_api_call_leaves_nothing_behind ... ok
test session_new_with_a_nonexistent_cwd_fails_and_leaves_nothing_behind ... ok
test a_command_that_does_not_exist_fails_and_leaves_no_session_tab_or_pane_behind ... ok
test pane_close_refuses_while_its_program_is_running_and_touches_nothing ... ok
test session_close_refuses_while_any_pane_in_any_of_its_tabs_is_running ... ok
test pane_split_with_no_program_or_cwd_runs_the_login_shell_at_home ... ok
test pane_split_with_a_nonexistent_cwd_leaves_no_new_pane_behind ... ok
test tab_close_refuses_while_any_of_its_panes_is_running ... ok
test tab_new_with_a_bad_command_leaves_no_new_tab_or_pane_behind ... ok
test pane_split_through_api_call_starts_a_real_program ... ok
test tab_new_with_a_nonexistent_cwd_leaves_no_new_tab_or_pane_behind ... ok
test pane_split_with_a_bad_command_leaves_no_new_pane_behind ... ok
test tab_new_through_api_call_starts_a_real_program ... ok
test session_new_through_api_call_starts_a_real_program ... ok
test session_new_with_a_command_starts_it_and_pane_show_reports_it_running_then_ended ... ok
test a_bad_command_through_api_call_leaves_nothing_behind ... ok
test pane_close_with_force_on_a_running_program_proceeds_exactly_as_today_the_stated_gap ... FAILED
test session_new_through_api_call_with_no_program_runs_the_login_shell ... ok

failures:

---- pane_close_with_force_on_a_running_program_proceeds_exactly_as_today_the_stated_gap stdout ----

thread 'pane_close_with_force_on_a_running_program_proceeds_exactly_as_today_the_stated_gap' (14578713) panicked at crates/server/tests/runtime.rs:71:35:
expected Response::Ok, got ApiError { code: ProgramFailed, message: "the pseudo-terminal failed: failed to openpty: Os { code: -6, kind: Uncategorized, message: \"Unknown error: -6\" }", candidates: [] }
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    pane_close_with_force_on_a_running_program_proceeds_exactly_as_today_the_stated_gap

test result: FAILED. 17 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.43s

error: test failed, to rerun pass `-p steg-server --test runtime`
full output: .quiet/tests-p-steg-p-steg-app-p-steg-kernel-p-steg-sdk-p-steg-server.log
error: recipe `test` failed with exit code 101
```

Commits since the review: 24, 22 without the Asdlc-Worker trailer.

Check FAILED: `just check` exited 101 at 2026-11-15T22:19:06+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.76 on task-1.76-server-start-a-pane-s-program-from-the at 521b3d7161cd with uncommitted changes, last 40 of 68 lines:

```text
test a_bad_command_through_api_call_leaves_nothing_behind ... ok
test tab_new_with_a_nonexistent_cwd_leaves_no_new_tab_or_pane_behind ... ok
test session_close_refuses_while_any_pane_in_any_of_its_tabs_is_running ... ok
test pane_close_refuses_while_its_program_is_running_and_touches_nothing ... ok
test pane_split_with_a_nonexistent_cwd_leaves_no_new_pane_behind ... ok
test tab_new_with_a_bad_command_leaves_no_new_tab_or_pane_behind ... ok
test session_new_with_a_command_starts_it_and_pane_show_reports_it_running_then_ended ... ok
test pane_split_through_api_call_starts_a_real_program ... ok
test session_new_through_api_call_starts_a_real_program ... ok
test pane_split_with_a_bad_command_leaves_no_new_pane_behind ... ok
test session_new_through_api_call_with_no_program_runs_the_login_shell ... ok
test tab_new_through_api_call_starts_a_real_program ... ok
test a_command_that_does_not_exist_fails_and_leaves_no_session_tab_or_pane_behind ... ok
test tab_close_refuses_while_any_of_its_panes_is_running ... FAILED
test pane_close_with_force_on_a_running_program_proceeds_exactly_as_today_the_stated_gap ... FAILED
test pane_split_with_no_program_or_cwd_runs_the_login_shell_at_home ... ok

failures:

---- tab_close_refuses_while_any_of_its_panes_is_running stdout ----

thread 'tab_close_refuses_while_any_of_its_panes_is_running' (14584940) panicked at crates/server/tests/runtime.rs:71:35:
expected Response::Ok, got ApiError { code: ProgramFailed, message: "the pseudo-terminal failed: failed to openpty: Os { code: -6, kind: Uncategorized, message: \"Unknown error: -6\" }", candidates: [] }
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace

---- pane_close_with_force_on_a_running_program_proceeds_exactly_as_today_the_stated_gap stdout ----

thread 'pane_close_with_force_on_a_running_program_proceeds_exactly_as_today_the_stated_gap' (14584919) panicked at crates/server/tests/runtime.rs:71:35:
expected Response::Ok, got ApiError { code: ProgramFailed, message: "the pseudo-terminal failed: failed to openpty: Os { code: -6, kind: Uncategorized, message: \"Unknown error: -6\" }", candidates: [] }


failures:
    pane_close_with_force_on_a_running_program_proceeds_exactly_as_today_the_stated_gap
    tab_close_refuses_while_any_of_its_panes_is_running

test result: FAILED. 16 passed; 2 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.42s

error: test failed, to rerun pass `-p steg-server --test runtime`
full output: .quiet/tests-p-steg-p-steg-app-p-steg-kernel-p-steg-sdk-p-steg-server.log
error: recipe `test` failed with exit code 101
```

Commits since the review: 24, 22 without the Asdlc-Worker trailer.

Check FAILED: `just check` exited 101 at 2026-11-15T22:19:52+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.76 on task-1.76-server-start-a-pane-s-program-from-the at 521b3d7161cd with uncommitted changes, last 40 of 68 lines:

```text
test session_new_through_api_call_with_no_program_runs_the_login_shell ... ok
test pane_split_with_a_nonexistent_cwd_leaves_no_new_pane_behind ... ok
test tab_new_with_a_bad_command_leaves_no_new_tab_or_pane_behind ... ok
test session_new_through_api_call_starts_a_real_program ... ok
test tab_new_through_api_call_starts_a_real_program ... ok
test session_new_with_a_command_starts_it_and_pane_show_reports_it_running_then_ended ... ok
test pane_close_refuses_while_its_program_is_running_and_touches_nothing ... ok
test pane_split_with_a_bad_command_leaves_no_new_pane_behind ... ok
test pane_close_with_force_on_a_running_program_proceeds_exactly_as_today_the_stated_gap ... ok
test pane_split_with_no_program_or_cwd_runs_the_login_shell_at_home ... ok
test pane_split_through_api_call_starts_a_real_program ... ok
test a_bad_command_through_api_call_leaves_nothing_behind ... ok
test session_close_refuses_while_any_pane_in_any_of_its_tabs_is_running ... FAILED
test tab_close_refuses_while_any_of_its_panes_is_running ... FAILED
test a_command_that_does_not_exist_fails_and_leaves_no_session_tab_or_pane_behind ... ok
test tab_new_with_a_nonexistent_cwd_leaves_no_new_tab_or_pane_behind ... ok

failures:

---- session_close_refuses_while_any_pane_in_any_of_its_tabs_is_running stdout ----

thread 'session_close_refuses_while_any_pane_in_any_of_its_tabs_is_running' (14590009) panicked at crates/server/tests/runtime.rs:71:35:
expected Response::Ok, got ApiError { code: ProgramFailed, message: "the pseudo-terminal failed: failed to openpty: Os { code: -6, kind: Uncategorized, message: \"Unknown error: -6\" }", candidates: [] }
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace

---- tab_close_refuses_while_any_of_its_panes_is_running stdout ----

thread 'tab_close_refuses_while_any_of_its_panes_is_running' (14590017) panicked at crates/server/tests/runtime.rs:71:35:
expected Response::Ok, got ApiError { code: ProgramFailed, message: "the pseudo-terminal failed: failed to openpty: Os { code: -6, kind: Uncategorized, message: \"Unknown error: -6\" }", candidates: [] }


failures:
    session_close_refuses_while_any_pane_in_any_of_its_tabs_is_running
    tab_close_refuses_while_any_of_its_panes_is_running

test result: FAILED. 16 passed; 2 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.42s

error: test failed, to rerun pass `-p steg-server --test runtime`
full output: .quiet/tests-p-steg-p-steg-app-p-steg-kernel-p-steg-sdk-p-steg-server.log
error: recipe `test` failed with exit code 101
```

Commits since the review: 24, 22 without the Asdlc-Worker trailer.
<!-- SECTION:NOTES:END -->

## Comments

<!-- COMMENTS:BEGIN -->
author: @bruk
created: 2026-11-15T20:25:00Z

---

Plan approved by @bruk (308d35ef87d0, size-5): its worker may build it.

---

author: @bruk.r4
created: 2026-11-15T21:22:00Z

---

@bruk.r4 is reviewing TASK-1.76: a same-machine review: it waited 0m with no other engineer's reviewer, longer than review_fallback_after (0m), so a fresh reviewer on its author's machine takes it.

---

author: @bruk.r4
created: 2026-11-15T21:36:00Z

---

Review by @bruk.r4 (same-machine): changes requested. Back to In Progress for the author's worker.

Findings: 3 Important, 1 nit. Cites: review, testing-rust. The diff does not match the plan.

Head reviewed: fdef2fe3f96799b2895406f66e75887f032f8251 (task's own recorded clean `just check`, and PR #40's CI `check` SUCCESS on the same SHA). Ran the server crate's tests 5+ times myself (`just test`, plus a direct `cargo test -p steg-server`): 0 failures across all runs, all real-pty/runtime integration suites included.

Important findings:

1. [Bugs, Compliance] crates/server/src/runtime.rs:153-163 (Runtime::handle), crates/server/src/dispatch/api.rs:56-78 (api::call) - a request for session.new/tab.new/pane.split made through api.call (decision-1's generic call path) never reaches Runtime::start_and_dispatch's start-wrapper. Runtime::handle matches on the literal request.method, which for such a call is "api.call", not one of the three starting methods, so it falls to the `_` arm and calls the plain, unchanged dispatch::dispatch. api::call rewrites and recurses into that same plain dispatch, which mints the pane/tab/session (pane.rs's split, tab.rs's new, session.rs's new) but never calls tree.start_program or pane::start - both of those only happen in Runtime::start_and_dispatch, which this path never runs. The result: a pane left with program() == None and status() == None permanently, no PaneHandle, no process, and Response::Ok with no error - breaking the very invariant the plan's own S1 lock argument and PaneRecord.status's own doc rest on ("never observed from outside that pairing"), and failing AC #1 for this path (no program is ever started, not even the login shell). crates/server/tests/dispatch.rs's own pane_split_with_a_program_mints_a_pane_but_does_not_start_it documents exactly this gap at the plain-dispatch layer and calls it fine because "that is Runtime's own job, right after this same call" - true when the method is called directly, false when it arrives through api.call. dispatch.rs:1136 (api_call_of_pane_split_gives_the_same_response_a_direct_call_would) tests api.call's equivalence only against plain dispatch(), never against Runtime::handle, so this never surfaced. Nothing in the plan or notes names this as a known gap.

2. [Evidence, Compliance] crates/server/src/runtime.rs:849 (a_tabs_declared_size_reaches_every_pane_including_one_split_in_afterward) - the plan's own S4 proof promised pane.split, pane.resize, pane.close and a swap all driven through Runtime::handle with terminal/pty checks after each. Only split is exercised (pane.resize is still not_implemented_yet, but pane.close and swap are both dispatchable today and are simply absent, with no note explaining why). More importantly, the one case that is built proves nothing about the general step it claims to: I disabled the reapply_every_sized_tab(&mut state) call in start_and_dispatch's success arm (runtime.rs:291) as a negative control and the test still passed unchanged. The split used is direction:"right" (divides columns, not rows), the test only ever asserts screen.rows.len() (never a column count), and the new pane's row count already comes from the initial LaunchSpec size resolved directly from tab_sizes_now at mint time (runtime.rs:264-279), independent of the general step entirely. The implementation notes' claim that this test proves "the general step, not only the start path" is not backed by what the test actually checks.

3. [Bugs] crates/server/src/runtime.rs - Runtime has no Drop impl and no shutdown/stop method anywhere; nothing ever sends PaneCommand::Shutdown to a live pane, joins the drain thread (_drain), or ends a process. Every pane's worker thread holds its own clone of the PaneEvent Sender, so dropping Runtime does not even close the drain thread's channel - the drain thread, every pane's worker/relay threads, and every real child process (e.g. the real `sh -c 'sleep 100'` programs crates/server/tests/runtime.rs's own session_with_a_long_running_pane and the sizing tests start) all outlive a dropped Runtime indefinitely, with no path back to them. This is exactly one of the paths today's own new team rule and AC #5 call out by name ("the runtime itself being dropped"), and unlike the --force orphan (named explicitly in S5 as a stated, deliberate gap with its own test), this path is not named anywhere in the plan or the notes as something left for a later task.

Nit (1):
- [Tests] crates/server/src/runtime.rs:793,949 and crates/server/tests/runtime.rs:130 synchronise with bounded std::thread::sleep polling loops rather than an event, which testing-rust.md's "no test sleeps to wait" rule disallows - each is reasoned about and bounded (20s) in its own comment, but is still sleep-based polling, not a channel wait.

Plan match: mostly followed (owning structure, cwd rule B, pane.show, the pure close-refusal, error codes, the lookup.rs move - all confirmed a straight move/build with no behaviour drift), but the S1 departure the notes already name (Runtime, not the dispatch arm, calls start_program) has the undocumented api.call consequence in finding 1, and the S4 proof section's own promised coverage (close, swap; a test that would actually fail without the general step) was not built, contradicting the notes' own coverage claim for AC #3.

No unsafe code anywhere in the diff (confirmed directly, not just by the worker's own grep). No new dependency. doc-2/doc-8/doc-9/doc-16 boundaries held: kernel diff (tree.rs) is pure with its own unit tests; server owns all I/O; sdk's ProgramFailed/PaneRecord additions are typed, serde-derived, and mirror-tested against the description.

Item 8 (main moved): TASK-1.48 (Settings on dispatch) is correctly adapted (State.settings threaded through, confirmed by reading runtime.rs). TASK-1.41 (Moment on record_exit, Pane::status -> program_status) and TASK-1.81 (api.call's selector) have not merged as of fdef2fe; when TASK-1.41 lands, apply_event_locked's record_exit call and the three dispatch close arms' pane.status() calls will need updating to the new name/signature - not yet a problem, but worth the next worker's attention, and TASK-1.81's api.call rename is adjacent to finding 1 above.

---

created: 2026-11-15T21:38:00Z

---

From @bruk.o, with the review's findings: the worker reported after hand-back that a working directory that does not exist is not a failure, because the pty library starts the program in the home directory instead; so pane.split, tab.new or session.new given such a cwd answers success and runs somewhere else. The review did not rule on it. The worker is asked to check the directory in the server before spawning and refuse with ProgramFailed, leaving nothing behind (criterion 1: a program that cannot be started is reported); rule B's inherited directory falls back to home when it no longer exists.

---
<!-- COMMENTS:END -->
