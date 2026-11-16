---
id: TASK-1.98
title: 'Server: the accept loop survives an error it can recover from'
status: In Progress
assignee:
  - '@bruk.w6'
created_date: '2026-11-16T02:00:00Z'
updated_date: '2026-11-16T11:33:00Z'
labels:
  - size-2
milestone: m-3
dependencies:
  - TASK-1.45
  - TASK-1.77
parent_task_id: TASK-1
project: bruk-io-steg
asdlc_accepted_from: DRAFT-2
asdlc_branch: task-1.98-server-the-accept-loop-survives-an-error
asdlc_plan_approved_by: bruk
asdlc_plan_approved_hash: '040e19a11fea'
asdlc_plan_approved_size: size-2
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Raised by TASK-1.77's second review on 2026-11-15 as a decision for the engineer: listen.rs (from TASK-1.44) ends the whole server on any error from accept, and TASK-1.77's test now pins that for descriptor exhaustion (exit status 1). Neither decision-2 nor doc-8 says which is right. For a server that owns every session and running program, ending because one more client could not be accepted loses everything the user has open; the orchestrator's recommendation is to carry on. doc-8 R1, R99; doc-2 section 6. If the engineer prefers the server to end, archive this draft.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 An error from accept that does not mean the listener is broken (the process or the system is out of descriptors, a connection aborted before it was accepted, an interrupted call) does not end the server: the connection is not served, the loop waits briefly without spinning and carries on, and every session and program is untouched. The plan lists each error kind and which side it falls on, with its reason
- [ ] #2 An error that does mean the listener is unusable still ends the server with status 1, releasing the lock and removing the socket file as now
- [ ] #3 Shown with a real server through the real binary: with the descriptor limit lowered and connections held open until accept fails, the server is still running, existing connections still get replies, and once descriptors are freed a new client connects; the test TASK-1.77 added for this case (running_out_of_file_descriptors_exits_1) is replaced by it, and its change of expectation is named in the notes. Waits are bounded; no process is left
- [ ] #4 The loop cannot spin at full speed while the condition lasts: shown by a bound on how many accept attempts happen in a fixed interval, with its negative control
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
## For the engineer

- **How a recovered accept error is reported - two options, recommendation reversed
  from this plan's first draft.**
  - **Option A (recommended): a new `ConnectionEvent::AcceptRetried(RecoverableAcceptError)`
    variant**, sent on the `events: Option<Sender<ConnectionEvent>>` `run` already takes,
    the same way `connection.rs` already sends `Connected`/`Disconnected`. No static, no
    global mutable state, no new `test_support` module, no feature-gated reader: AC #4's
    fixture just builds its own `mpsc::channel`, passes the sender into `run`, and counts
    what arrives on its own receiver - the exact shape `race_attempt.rs` and this crate's
    own `shutdown.rs` tests already use for `ConnectionEvent`. `RecoverableAcceptError`
    becomes `pub`, re-exported alongside `ConnectionEvent`. Checked, not assumed, that this
    is a clean addition: no test or production code anywhere in this crate exhaustively
    `match`es on `ConnectionEvent` today (`grep -rn "match.*ConnectionEvent"` finds nothing;
    every consumer compares by `==`/`assert_eq!`/`matches!`), so `wildcard_enum_match_arm`
    has nothing to break - a new variant is additive everywhere, including wherever
    TASK-1.86 is editing this same enum right now.
  - **Option B: an always-compiled, unconditional internal counter** (`static
    ACCEPT_RETRIES: AtomicU64`, incremented unconditionally so the code under test is
    identical between AC #3's binary and AC #4's fixture - never feature-gate the
    increment itself, only a reader), with a feature-gated `test_support::
    accept_retry_count()` reader for the fixture to poll. Rejected in this revision:
    global mutable state purely to serve one test, and a real server still has no way to
    say *why* it refused a client (an operator reading logs learns nothing `ConnectionEvent`
    would not already tell them for free). Named here because it was this plan's first
    draft, not because it is still recommended.
  - Printing stays out of scope either way - `crates/server/src` prints nothing today
    (TASK-1.87's comment at `listen.rs:271` keeps this true), and `crates/steg/src/main.rs`
    calls `run(..., None)`. A printed line is a one-line addition once some task makes
    `crates/steg` consume `ConnectionEvent` for real - not blocking this one.

## The stop path, checked before trusting the classifier near it (AC #1/#2)

**How `run` is told to stop, read directly rather than assumed**: `Stopper::stop()` sets
a flag and calls `wake_accept()`, which does a plain `UnixStream::connect(socket_path)` -
a real client connecting, not a `shutdown()`/`close()` of the listener from another
thread. The listener itself is only ever dropped *after* the loop has already exited
(`listen.rs:347`, well after the loop's own `break`). So the specific risk named in
review - a shut-down listening socket handing a blocked `accept()` an `ECONNABORTED` that
this classifier would misread as "retry forever" - **does not apply to this server's own
stop mechanism**: there is no path where the listener is torn down while a thread is
still blocked inside `accept()`. Confirmed against `shutdown.rs`'s own tests, which cover
exactly this (`stop_on_a_genuinely_idle_server_returns_from_run_within_a_bound`,
`stopping_while_a_trickling_handshake_is_in_progress_still_returns_within_a_bound`): none
of them observe `ECONNABORTED`, or any error at all, from the wakeup - the woken
`accept()` returns `Ok`, and `should_stop()` is checked right after.

**A real, different gap exists, found by reading the loop's own control flow, not by
the scenario named in review**: today, `should_stop()` is checked in exactly one place -
right after a *successful* `accept()`, before `try_clone`. Every arm this task adds is in
the *error* path of that same `match`, which never reaches that check at all. Under
`EMFILE`/`ENFILE` specifically - the one recoverable kind that does not clear on its own -
`accept()` can keep failing indefinitely without ever returning a connection, which means
a `stop()` called mid-storm could go unnoticed for as long as the storm lasts, not for one
backoff interval - worse than anything review named, and this task's own change
introduces it (today, *any* error, including `EMFILE`, ends the loop immediately, so there
is nothing to get stuck in). The fix: in both recoverable-error arms, check
`stop.should_stop()` *before* classifying the error at all, not only before sleeping -
if a stop was asked, take the exact same `break Ok(())` exit the successful-`accept`
path already uses, regardless of what the error was:

```rust
Err(err) => {
    if stop.should_stop() {
        break Ok(());
    }
    match classify_accept_error(&err) {
        Some(_) => { note_retry(&events); reap_finished(&mut connections); thread::sleep(config.accept_error_backoff); continue; }
        None => break Err(err.into()),
    }
}
```

This bounds the worst case to one loop iteration (classify is pure/instant,
`reap_finished` is bounded by however many connections are tracked, the sleep is
`accept_error_backoff` itself) - from "possibly forever, while the storm lasts" to "at
most one backoff interval", every time through the loop.

## The three named cases, and where each falls (AC #1)

Matched by raw errno (`err.raw_os_error()`, mapped through `nix::errno::Errno::from_raw` -
`nix` is already an allowed dependency of this crate per `boundaries.toml`, and it
re-exports `libc` itself at `nix::libc`, so no new dependency and no `boundaries.toml`
edit for the classifier itself), never by `std::io::ErrorKind`: confirmed by reading this
toolchain's own `library/std/src/sys/net/connection/socket/unix.rs` (installed via
`rustup component add rust-src` for 1.98.1, matching `rust-toolchain.toml` - machine-local
only, see the note at the end of this plan) that `EMFILE`/`ENFILE` have no `ErrorKind`
variant that round-trips through `decode_error_kind` on this platform - classifying by
`ErrorKind` alone could never cover the one case AC #4 is about.

- **`EMFILE` (this process is out of descriptors) or `ENFILE` (the whole system is)** -
  recoverable, but only once some descriptor, anywhere, is closed: the condition does not
  clear on its own, which is exactly why the loop backs off instead of retrying at full
  speed while it lasts (AC #4), and exactly why the stop check above has to come first.
- **`ECONNABORTED`** - a connection the kernel already queued was reset or torn down by
  its own peer before this loop's `accept()` picked it up. Not this listener's fault, and
  not a reason to stop: there is no stream here to even report failure to. (Not this
  server's own stop mechanism either - see above.)
- **`EINTR`** - interrupted by a signal before a connection was ready. **Checked, not
  assumed, to be dead in practice on this target**: `UnixListener::accept` ultimately
  calls `Socket::accept`, and on macOS (not in the `accept4`-capable target list in that
  file) that wraps `libc::accept` in `cvt_r`, which already retries `EINTR` internally -
  so this arm is a defensive backstop for a classifier that has to be correct regardless
  of what std happens to do today, not because it is expected to fire. Still required:
  AC #1 names it explicitly.

Everything else - an unlisted errno, or `raw_os_error() == None` (a `std::io::Error` not
actually sourced from a syscall) - is **fatal**, the same conservative default `run`
always had, only narrowed by the three cases above and never widened past them.

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoverableAcceptError {
    DescriptorsExhausted,
    ConnectionAborted,
    Interrupted,
}

fn classify_accept_error(err: &std::io::Error) -> Option<RecoverableAcceptError> {
    match err.raw_os_error().map(nix::errno::Errno::from_raw) {
        Some(nix::errno::Errno::EMFILE | nix::errno::Errno::ENFILE) =>
            Some(RecoverableAcceptError::DescriptorsExhausted),
        Some(nix::errno::Errno::ECONNABORTED) =>
            Some(RecoverableAcceptError::ConnectionAborted),
        Some(nix::errno::Errno::EINTR) =>
            Some(RecoverableAcceptError::Interrupted),
        _ => None,
    }
}
```

**Two call sites in the loop need this, not one - found by reading `try_clone`'s own
implementation, not assumed.** `listen.rs:306-312`'s `stream.try_clone()` is a `dup(2)`
under the hood (`UnixStream::try_clone` -> `Socket::duplicate`), which fails with exactly
`EMFILE`/`ENFILE` under the same descriptor exhaustion `accept()` itself can hit. The real
sequence AC #3's own lowered-`ulimit` scenario will actually produce is: `accept()`
succeeds (one fd was free), `try_clone()` fails (none left for the duplicate) - so a
classifier that only guards `accept()`'s own error would still exit 1 under exactly the
condition this task is about. Both sites get the same disposition, same stop-check, same
backoff - but **no shared helper function**: the two sites sit on opposite sides of
`thread::scope`'s own boundary (`connections: Vec<(UnixStream,
thread::ScopedJoinHandle<'_, ()>)>`'s own lifetime is tied to that closure), and a free
function taking it by `&mut` would need that scope's own lifetime threaded through its
signature just to type-check - not worth it for a handful of lines. Each site inlines the
same handling instead, with the shared reasoning living in `classify_accept_error`'s own
doc comment, not duplicated prose at each call site.

`reap_finished`'s own existing call site (right after a successful `accept()`) is
untouched; each inlined recoverable-error arm adds its own *additional* call, so a
connection that finished on its own during an exhaustion storm gives its fd back as soon
as possible rather than waiting for the next successful `accept()` - which, under
`ENFILE`, might be a while.

## The backoff is data, not a constant (AC #4)

`config.rs`'s own module doc already states the rule this falls under ("Time... come[s]
in as parameters... No test sleeps to wait for something"): a hardcoded
`thread::sleep(FIXED)` inside `run` would make AC #4's own negative control (below)
impossible to run without editing production code. `ServerConfig` gains:

```rust
pub const DEFAULT_ACCEPT_ERROR_BACKOFF: Duration = Duration::from_millis(20);
// ServerConfig:
pub accept_error_backoff: Duration,
```

(`Default` impl, the manual `Debug` impl's field list, and `lib.rs`'s re-export of the new
constant all need the matching one-line addition.) The `thread::sleep` inside `run`'s own
loop is production code reacting to a real, already-happened error - not a test sleeping
to wait for an event - so this does not trip the no-sleep-in-tests rule; only the tests
below must never use it to synchronize.

## Files that change

- `crates/server/src/listen.rs`: `RecoverableAcceptError` (now `pub`),
  `classify_accept_error`, the stop-check-first handling inlined at both call sites in
  `run`'s loop, new `#[cfg(test)] mod tests` cases for the classifier.
- `crates/server/src/connection.rs`: `ConnectionEvent` gains `AcceptRetried
  (RecoverableAcceptError)`.
- `crates/server/src/config.rs`: `DEFAULT_ACCEPT_ERROR_BACKOFF`, `ServerConfig::
  accept_error_backoff`, `Default`/`Debug` impls.
- `crates/server/src/lib.rs`: re-export the new constant and `RecoverableAcceptError`.
- `crates/server/Cargo.toml`: `accept-backoff-fixture` feature (`= ["nix/resource"]` - the
  new fixture's own rlimit lowering needs `nix::sys::resource::setrlimit`, a real, separate
  Cargo feature not bundled into `fs`/`user`/`process` - verified, `nix-0.28.0/src/
  lib.rs:95`/`Cargo.toml:129` - gated so an ordinary build of this crate never turns it
  on), `[[bin]] accept-backoff-attempt`, `[[test]] accept_backoff`, both
  `required-features = ["accept-backoff-fixture"]` (mirrors `race-fixture` exactly, down
  to the shape of the feature/bin/test triple).
- `crates/server/tests/fixtures/accept_backoff_attempt.rs` (new): reads
  `STEG_SOCKET_PATH` (existing convention), `ACCEPT_BACKOFF_MS`, and (for the stop-path
  proof) `STOP_AFTER_RETRIES`; lowers its *own* process's `RLIMIT_NOFILE` (safe here
  specifically because this is its own, dedicated process - never shared with any other
  test); builds its own `mpsc::channel::<ConnectionEvent>`, calls `attempt`+`run` with
  `ServerConfig { accept_error_backoff: Duration::from_millis(parsed), .. }` and
  `Some(tx)`; on a side thread, counts `AcceptRetried` events as they arrive and prints
  `"count {n}"`, flushed, each time - the same "print a line the real test process reads
  via `BufReader`" shape `race_attempt.rs` already uses, just for a count instead of an
  `Arbitration`; if `STOP_AFTER_RETRIES` is set, calls `stopper.stop()` itself once that
  many `AcceptRetried` events have arrived, then exits with `run`'s own `Ok`/`Err` mapped
  to `0`/`1`, matching `crates/steg/src/main.rs`'s own convention.
- `crates/server/tests/accept_backoff.rs` (new): three tests - the backoff-bound test, its
  permanent zero-backoff negative control, and the stop-during-exhaustion test - see
  "Proof", below.
- `boundaries.toml`: no change - `nix` is already allowed, and the new feature's reason is
  self-contained in `Cargo.toml`'s own comment (the existing `race-fixture` entry sets
  this precedent; its own feature carries no `boundaries.toml` mention either).
- `crates/steg/tests/server_start.rs`: `running_out_of_file_descriptors_exits_1` replaced
  by the new survives-and-recovers test (AC #3); old name and its old assertion
  (`status.code() == Some(1)`) named in the notes as the expectation this change reverses.
  Needs a small `connect_and_handshake`-shaped helper over `steg_sdk`'s wire types
  (`Frame`, `encode_frame`, `Connection`, `check_hello`/`Handshake`, `description`,
  `PROTOCOL_VERSION`) - necessarily duplicated from `crates/server/tests/routing.rs`'s own
  private helper of the same shape, since test helpers are not shared across crates and
  this file has never needed one before.

## Order of work

1. `classify_accept_error`/`RecoverableAcceptError` in `listen.rs`, with its own
   `#[cfg(test)]` unit tests (pure, no sockets) - gets the hardest-to-get-right part
   (which errno falls where, and why) reviewable on its own before anything in the real
   loop changes.
2. `ServerConfig::accept_error_backoff` in `config.rs`; `ConnectionEvent::AcceptRetried`
   in `connection.rs`; both re-exported from `lib.rs`.
3. The stop-check-first, classify, note/reap/sleep-or-break handling inlined at both call
   sites in `run`'s loop (`listen.rs`).
4. `Cargo.toml`'s new feature/`[[bin]]`/`[[test]]` entries.
5. `tests/fixtures/accept_backoff_attempt.rs`, then `tests/accept_backoff.rs` (AC #4 and
   the stop-during-exhaustion proof, each with its negative control run and recorded).
6. `crates/steg/tests/server_start.rs`'s replacement test (AC #3, including the real
   `ENOTSOCK`-via-`dup2` fatal-path proof for AC #2) - last, because it is the slowest and
   the one most likely to need its descriptor-limit number tuned empirically against
   whatever the real binary's own startup now costs in fds.

## Proof, by acceptance criterion, each with a negative control

- **AC #1** (each error kind classified correctly, with its reason): pure unit tests in
  `listen.rs` against `std::io::Error::from_raw_os_error(code)` for real errno values on
  both sides - `EMFILE`, `ENFILE`, `ECONNABORTED`, `EINTR` recoverable; `EBADF`, `EINVAL`
  (picked as two plausible-but-wrong neighbours, not exhaustive) and `raw_os_error() ==
  None` fatal. No mock of `accept()` itself - these are real OS error values, just not
  obtained by actually calling a syscall. **Negative control**: the fatal-side assertions
  (`EBADF`/`EINVAL`/`None` all return `None` from the classifier) are each other's control
  for the recoverable-side assertions - a classifier that wrongly folded everything into
  Recoverable would fail these, and one that classified nothing at all would fail the
  first set; both halves are in the same test module and run every time.
- **AC #1's "every session and program is untouched"**, plus **AC #2, proven for real -
  not dropped.** Confirmed by a standalone throwaway probe, not assumed, that a real
  fatal `accept()` error can be produced through the real loop with no unsafe code and no
  trait seam: bind a real `UnixListener`, read its raw fd, `nix::unistd::dup2` an open
  regular file (`/dev/null`) onto that exact fd number, then call `accept()` again on the
  *same* listener object - it returns `Err(os error 38)`, `ENOTSOCK` on Darwin,
  immediately (not blocked, confirmed by making the probe's accept nonblocking and timing
  it) - `classify_accept_error` returns `None` for it (fatal), correctly. `dup2` is a safe
  function in `nix` (no `unsafe` block needed anywhere in this proof - checked:
  `nix-0.28.0/src/unistd.rs:439`, `pub fn dup2`, not `pub unsafe fn`).

  **Why this is safe to do against the *real*, running accept loop, not merely a
  standalone probe**: the earlier plan rejected closing a listener's fd from another
  thread while `accept()` might be *blocked* on it, a genuine double-close race. This is
  different in the one way that matters - the injection only happens while the loop is
  proven to be mid-exhaustion-storm (actively classifying `EMFILE`/`ENFILE` and sleeping
  between attempts, never blocked in a live `accept()` call), because the test keeps
  every exhausting connection open, unchanged, right up to the moment of the `dup2` - the
  storm cannot resolve on its own in that window, so the loop cannot have returned to a
  blocking `accept()` call before the injection lands. The next attempt, whenever it
  comes, sees the dummy fd and gets `ENOTSOCK`.

  Built as a fourth test in `crates/server/tests/accept_backoff.rs`, against the same
  fixture (a new env var, say `DUP2_AFTER_RETRIES`, naming how many `AcceptRetried`
  events to wait for before performing the injection *from a side thread inside the
  fixture process itself* - it already owns the listener's fd at that point, no
  cross-process raw-fd trick needed): asserts the fixture process exits with status 1,
  and that its socket file and lock file are both gone afterward (the same filesystem
  checks `shutdown.rs`'s own tests already use for "cleanup ran"). **Negative control**: a
  scratch edit that makes `classify_accept_error` return `Some(..)` for `ENOTSOCK` too -
  run once, the fixture never exits (still retrying against the dummy fd forever) -
  recorded in the notes, then reverted. If this technique turns out not to behave the
  same across environments once built (recorded either way, not assumed): fall back to
  naming AC #2's end-to-end half as dropped, with the reasoning this plan's first draft
  already gave (the shared cleanup code the fatal branch falls into is unchanged by this
  task, so only the reaching-it part would be unproven).
- **The stop path, mid-exhaustion (part of AC #1/#2's own correctness, not named in the
  acceptance criteria verbatim but required by the control-flow fix above)**: a fifth test
  in the same file - hold descriptors exhausted (same technique), with
  `STOP_AFTER_RETRIES=1` so the fixture calls its own `Stopper::stop()` once it has seen
  one `AcceptRetried` event (confirming it is genuinely mid-storm, not coincidentally
  idle). Assert the fixture process exits within a small bound (a few backoff intervals,
  not "eventually") with status 0, and that its socket/lock files are gone. **Negative
  control**: the `if stop.should_stop() { break Ok(()); }` check removed from the
  recoverable-error arms (a scratch edit) - run once, the fixture process does not exit
  within the bound (it keeps retrying against the exhaustion, oblivious to the flag) -
  recorded in the notes, then restored.
- **AC #3** (real binary, lowered descriptor limit, server survives, existing connections
  still get replies, a new client connects once freed, bounded waits, no process left):
  one test in `crates/steg/tests/server_start.rs`, same `sh -c 'ulimit -n <N>; exec ...'`
  wrapper `running_out_of_file_descriptors_exits_1` already used (kept for the same
  isolation reason: the limit must apply to the *child* process, never this test
  binary's own). Sequence: start the real server with a lowered limit; one real client
  connects and completes a real handshake, held open; hold enough further raw (never
  handshaken) connections open to exhaust descriptors (confirmed empirically which of
  `accept()`/`try_clone()` is actually hit first, and the exact `-n` value needed above
  the server's own baseline fd cost - this number gets picked and recorded in the notes,
  not inherited from the old test's `16`, since this server must now also keep a socket,
  a lock, and one live session's pty open through the exhaustion window, which the old,
  die-immediately test never had to); send a request (`session.list`, needing no new fd)
  over the held-open handshaken connection and confirm a real reply within a bound - this
  is "existing connections still get replies"; `server.child.try_wait()` confirmed `None`
  (still running) at each checkpoint; drop roughly half the held raw connections; confirm
  a fresh client can connect and complete a handshake within a bound. **"Every session and
  program is untouched" gets its own assertion, not just inferred from the reply
  round-trip above**: after descriptors are freed, `session.list` (or `pane.show`) on
  either connection still shows the session created in step one, with its program's
  status still running. **What a client sitting in the kernel's backlog during exhaustion
  does, stated plainly and shown by this same test**: it stays queued - `accept()`
  failing with `EMFILE`/`ENFILE` does not remove a pending connection from the listen
  backlog (standard POSIX accept semantics: nothing was dequeued, since no fd was ever
  created) - so the "fresh client connects once freed" step above is this exact case:
  the client's own `connect()` already returned success before any descriptor was freed,
  and its handshake only completes once `accept()` can finally succeed; a client with its
  own connect-side timeout would instead give up on its own terms, which is that client's
  concern, not the server's. `KillOnDrop::kill_now()` to end the server (it does not die
  on its own - that is the point) so no process is left. **Negative control, run against
  the test as built, not against old `main`** (review found exactly this distinction
  matters this week): a scratch edit disabling both recoverable-error arms (classifier
  forced to return `None` for descriptor exhaustion) run against this *new* test - it
  fails, because the server exits instead of surviving - restored, it passes. Recorded
  with `asdlc record-check`, not merely asserted from the old test's own prior behaviour.
- **AC #4** (bounded attempts, no spin): `tests/accept_backoff.rs`'s two tests, both
  against the real `accept-backoff-attempt` fixture under a real lowered `RLIMIT_NOFILE`
  in its own isolated process - genuine `EMFILE`, not a mock. First test:
  `ACCEPT_BACKOFF_MS=20` (the production default), hold descriptors exhausted for a fixed
  window (500ms), read the fixture's printed counts, assert the count stays under a
  bound with margin stated both ways, not just asserted: expected count is about
  `500ms / 20ms` = 25; the threshold is set generously above that (say, 150) so ordinary
  scheduling jitter under load cannot flake it, while staying far below what a genuine
  spin produces - measured empirically once built and recorded here rather than guessed
  (a tight `accept()`/`try_clone()` retry loop with no backoff at this crate's own
  iteration cost is expected to produce on the order of tens of thousands of attempts in
  500ms, several orders of magnitude past 150). **Negative control, kept as a real,
  permanent second test** (not a one-off manual run, since `accept_error_backoff` being
  config means it costs nothing to keep): `ACCEPT_BACKOFF_MS=0` against the same fixture,
  same window, asserting the count exceeds the same threshold by a wide margin - proving
  this harness can actually tell a spin from a backoff, not merely asserting a number that
  happens to be true either way.

## Risks

- **Pending PRs (47/TASK-1.78, 56/TASK-1.87, 57/TASK-1.89) all touch exactly one line
  near this task's own change**: `Runtime::new`'s call at `listen.rs:268` gains a third
  argument (`FORCE_GRACE_DEFAULT`) in every one of them. This task's own change is in the
  loop body, `listen.rs:296-319`, several lines below that call - a one-line rebase
  conflict at worst, not a design conflict. Confirmed by diffing each PR's head against
  `origin/main` for `listen.rs` directly, not assumed.
- **TASK-1.86 (being planned concurrently) touches the same loop body, `listen.rs:
  296-319`, for a different reason**: it needs to count a connection as a client from the
  instant `accept()` returns it (before a completed handshake), to feed its idle-stop
  predicate. This task adds a stop-check-first, error-classification arm plus a bounded
  `thread::sleep` in the *error* path of that same `match`; it does not touch the
  `Ok(accepted)` arm or anything after the `try_clone` success case, which is where
  TASK-1.86's own counting almost certainly lands. The one real interaction to flag for
  whoever builds second: a backoff sleep in the error path delays TASK-1.86's own idle
  check by up to one backoff interval (`DEFAULT_ACCEPT_ERROR_BACKOFF`, 20ms) - negligible
  against a 30-second idle window, but worth TASK-1.86's own plan saying so rather than
  discovering it in review. `ConnectionEvent` gaining a new variant (`AcceptRetried`) in
  this task is a one-line, additive change for whoever else is mid-edit on that same enum
  - confirmed no exhaustive match exists on it anywhere today (see "For the engineer").
  Recommend this task lands first (a small, local, self-contained change to the error arm
  only) and TASK-1.86 is built against its result, rather than the reverse - lower rebase
  cost either way, but TASK-1.86's own design (a wrapper/observer around the loop) is
  easier to write once this task's control flow inside the loop is settled.
- **The AC #3 test's descriptor-limit number is not yet known** - it has to be picked
  empirically once a real session can be started and held open against a lowered limit
  (see "Order of work", why this is built last). Recorded in the notes once chosen, not
  guessed here.
- **`EINTR`'s own arm is believed dead in practice on this target** (checked against the
  installed toolchain's own `rust-src`, not merely assumed) - kept anyway because AC #1
  names it explicitly and a classifier that is correct only for the errno std happens to
  retry today would be wrong the moment that changes.
- **The `dup2`-based fatal-path proof (AC #2) is new and unreviewed as a technique in this
  repository** - confirmed working in a standalone, disposable probe on this machine
  (`accept()` on a `UnixListener` whose raw fd was `dup2`'d over returns `ENOTSOCK`
  immediately), but not yet exercised against the real, threaded accept loop under real
  descriptor exhaustion. If it does not hold up once built, the fallback named in
  "Proof", above, is to drop AC #2's end-to-end half, as this plan's first draft did.

**For the engineer, on the question asked directly**: `rustup component add rust-src`
only downloaded the standard library's source into this machine's local rustup toolchain
directory (`~/.rustup/toolchains/1.98.1-aarch64-apple-darwin/lib/rustlib/src/`), entirely
outside the repository - confirmed with `git status` on the task's own worktree
immediately after, which showed nothing changed. It did not touch
`rust-toolchain.toml`, any `Cargo.toml`/`Cargo.lock`, or anything else tracked.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Plan (040e19a11fea) checked against the four acceptance criteria, doc-8 R1 and R99 and the team's testing rules by @bruk.o, 2026-11-16: ok. Revised once at the orchestrator's request: the stop flag is checked in both recoverable arms before classifying (a stop during an exhaustion storm ends the server within one backoff), with its own proof and control; criterion 2 is proven with a real fatal error (the listener's descriptor replaced by a non-socket in the fixture's own process, ENOTSOCK, shown feasible by a probe) and is no longer dropped unless that fails once built; a recoverable error is reported as a ConnectionEvent variant, with no static counter; criterion 3's control is a recorded run against the test as built. For the engineer: the reporting choice (event, recommended, or a test-only counter), and that nothing is printed until the binary consumes events.
<!-- SECTION:NOTES:END -->

## Comments

<!-- COMMENTS:BEGIN -->
author: @bruk
created: 2026-11-16T11:33:00Z

---

Plan approved by @bruk (040e19a11fea, size-2): its worker may build it.

---
<!-- COMMENTS:END -->
