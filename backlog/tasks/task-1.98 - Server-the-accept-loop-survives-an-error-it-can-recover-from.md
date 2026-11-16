---
id: TASK-1.98
title: 'Server: the accept loop survives an error it can recover from'
status: In Progress
assignee:
  - '@bruk.w6'
created_date: '2026-11-16T02:00:00Z'
updated_date: '2026-11-16T08:42:00Z'
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

- **No new `ConnectionEvent` variant, and no human-visible report of a recovered accept
  error, in this task.** `crates/server/src` prints nothing today (TASK-1.87's comment at
  `listen.rs:271` states this as a line it keeps true), and `crates/steg/src/main.rs`
  calls `run(..., None)` - there is no consumer of `ConnectionEvent` in the real binary to
  print through yet. Adding a variant would also force a match-arm edit in whatever
  TASK-1.86 is writing against this same enum right now. Recommendation: report nothing
  outward; count internally only, behind a feature flag the default build never turns on
  (detailed below), purely so AC #4 has something real to measure. If a printed line is
  wanted later, it is a one-line addition once some task makes `crates/steg` consume
  `ConnectionEvent` for real - not blocking this one.
- **AC #2's end-to-end half (a genuinely fatal `accept()` error still exits 1, with
  cleanup) is not re-proven by a new test - named as dropped, not silently left.** The
  only existing test that ever reached `run`'s `break Err(...)` branch was
  `running_out_of_file_descriptors_exits_1`, and this task reclassifies exactly the error
  it used (`EMFILE`) as recoverable. Producing a different, genuinely fatal `accept()`
  error for real would mean either closing the listener's own fd out from under a blocked
  `accept()` call from another thread (a real double-close hazard - unsound, not merely
  delicate, and a guaranteed fight with `undocumented_unsafe_blocks`) or introducing a
  trait boundary over `UnixListener` purely to fake this one path (real scope creep for a
  size-2 task). Recommendation: drop it. `classify_accept_error` is a pure function,
  unit-tested exhaustively on both sides (recoverable codes and otherwise); the shared
  cleanup code the fatal branch falls into (`listen.rs:347-366`: close the listener, join
  every connection, stop the runtime, remove the socket file, release the lock) is
  unchanged by this task, so nothing about *that* code's own correctness is unproven, only
  that no automated test today exercises reaching it from a real `accept()` failure. Worth
  a follow-up task if this ever needs re-proving; not this one.

## The three named cases, and where each falls (AC #1)

Matched by raw errno (`err.raw_os_error()`, mapped through `nix::errno::Errno::from_raw` -
`nix` is already an allowed dependency of this crate per `boundaries.toml`, and it
re-exports `libc` itself at `nix::libc`, so no new dependency and no `boundaries.toml`
edit for the classifier itself), never by `std::io::ErrorKind`: confirmed by reading this
toolchain's own `library/std/src/sys/net/connection/socket/unix.rs` (installed via
`rustup component add rust-src` for 1.98.1, matching `rust-toolchain.toml`) that `EMFILE`/
`ENFILE` have no `ErrorKind` variant that round-trips through `decode_error_kind` on this
platform - classifying by `ErrorKind` alone could never cover the one case AC #4 is about.

- **`EMFILE` (this process is out of descriptors) or `ENFILE` (the whole system is)** -
  recoverable, but only once some descriptor, anywhere, is closed: the condition does not
  clear on its own, which is exactly why the loop backs off instead of retrying at full
  speed while it lasts (AC #4).
- **`ECONNABORTED`** - a connection the kernel already queued was reset or torn down by
  its own peer before this loop's `accept()` picked it up. Not this listener's fault, and
  not a reason to stop: there is no stream here to even report failure to.
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
enum RecoverableAcceptError {
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
condition this task is about. Both sites get the same disposition, same backoff - but
**no shared helper function**: the two sites sit on opposite sides of `thread::scope`'s
own boundary (`connections: Vec<(UnixStream, thread::ScopedJoinHandle<'_, ()>)>`'s own
lifetime is tied to that closure), and a free function taking it by `&mut` would need
that scope's own lifetime threaded through its signature just to type-check - not worth
it for three lines. Each site inlines the same three lines instead (classify, note +
reap + sleep on `Some`, `break Err(...)` on `None`), with the shared reasoning living in
`classify_accept_error`'s own doc comment, not duplicated prose at each call site.

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

## What is reported, and where

Nothing outward in production (see "For the engineer"). Internally: a crate-private
`static ACCEPT_RETRIES: AtomicU64`, incremented by `note_accept_retry()` - a plain,
*unconditional* `fetch_add(1, Relaxed)` on an already-rare error path, compiled in every
build, no `#[cfg]` on the counter or the increment itself. This matters beyond tidiness:
feature-gating the counter would make the code under test in AC #3's real-binary test
(built without any fixture feature) different from the code AC #4's fixture measures
(built with it) - two variants of the same function, with only one of them ever
measured. `applied_marks` (`runtime.rs`) is not the precedent to copy here: that gates a
*field on `State`*, not a branch inside the function the acceptance criteria are about.

What *is* feature-gated, the same way this crate already gates test-only surface
(`sdk`'s own `local-socket`/`fake-server-for-tests` precedent, `crates/sdk/src/
lib.rs:56-59`): a new, default-off Cargo feature `accept-backoff-fixture` turning on (a)
a `#[doc(hidden)] pub` reader `test_support::accept_retry_count()` re-exported from
`lib.rs` (the counter it reads is always there; only the *reader* needs gating, so
nothing public leaks into an ordinary build), (b) a new `[[bin]]` fixture, and (c) the
test that drives it - mirroring `race-fixture`'s own pattern in this exact crate
(`Cargo.toml`'s existing `race-fixture` feature, `[[bin]] race-attempt`, `[[test]]
race`). `accept-backoff-fixture = ["nix/resource"]` in `[features]` - the new fixture's
own rlimit lowering needs `nix::sys::resource::setrlimit`, gated so the `resource`
feature is never on for an ordinary build of this crate (verified: `nix`'s `resource` is
a real, separate Cargo feature, `nix-0.28.0/src/lib.rs:95`/`Cargo.toml:129`, not bundled
into `fs`/`user`/`process`). `boundaries.toml`'s `steg-server` reason gets one added
clause naming this, for the same honesty its existing sentence already keeps about
`fs`/`user`/`process` - `just check-boundaries` does not require it (there is no
feature-allow-list, only `deny-features`, and `resource` is not denied), but the prose
should stay true.

## Files that change

- `crates/server/src/listen.rs`: `RecoverableAcceptError`, `classify_accept_error`,
  `note_accept_retry`/`ACCEPT_RETRIES` (unconditional, not feature-gated - see "What is
  reported"), the two inlined call sites in `run`'s loop, new `#[cfg(test)] mod tests`
  cases for the classifier.
- `crates/server/src/config.rs`: `DEFAULT_ACCEPT_ERROR_BACKOFF`, `ServerConfig::
  accept_error_backoff`, `Default`/`Debug` impls.
- `crates/server/src/lib.rs`: re-export the new constant; `#[cfg(feature =
  "accept-backoff-fixture")] pub mod test_support` re-exporting `accept_retry_count`.
- `crates/server/Cargo.toml`: `accept-backoff-fixture` feature, `[[bin]]
  accept-backoff-attempt`, `[[test]] accept_backoff`, both `required-features =
  ["accept-backoff-fixture"]` (mirrors `race-fixture` exactly).
- `crates/server/tests/fixtures/accept_backoff_attempt.rs` (new): reads
  `STEG_SOCKET_PATH` (existing convention) and `ACCEPT_BACKOFF_MS`; lowers its *own*
  process's `RLIMIT_NOFILE` (safe here specifically because this is its own, dedicated
  process - never shared with any other test); calls `attempt`+`run` with `ServerConfig {
  accept_error_backoff: Duration::from_millis(parsed), .. }`; on a side thread, polls
  `test_support::accept_retry_count()` every few ms and prints `"count {n}"`, flushed -
  the same "print a line the real test process reads via `BufReader`" shape
  `race_attempt.rs` already uses, just for a count instead of an `Arbitration`.
- `crates/server/tests/accept_backoff.rs` (new): two tests, `accept_errors_back_off_
  instead_of_spinning` and its negative control, below.
- `boundaries.toml`: one clause added to `steg-server`'s `reason` (see above). No `allow`
  list change - `nix` is already on it.
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
2. `ServerConfig::accept_error_backoff` in `config.rs`, threaded through `lib.rs`'s
   re-export.
3. Inline the classify/note/reap/sleep-or-break handling at both call sites in `run`'s
   loop (`listen.rs`).
4. `test_support`'s feature-gated reader over the (always-compiled) counter,
   `Cargo.toml`'s new feature/`[[bin]]`/`[[test]]` entries, `boundaries.toml`'s clause.
5. `tests/fixtures/accept_backoff_attempt.rs`, then `tests/accept_backoff.rs` (AC #4,
   with its negative control run and recorded).
6. `crates/steg/tests/server_start.rs`'s replacement test (AC #3) - last, because it is
   the slowest and the one most likely to need its descriptor-limit number tuned
   empirically against whatever the real binary's own startup now costs in fds.

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
- **AC #2**: proven by construction, not by a new test - see "For the engineer" for why,
  and what is dropped.
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
  program is untouched" (AC #1's own words, re-checked here since AC #3 is where a real
  session exists to check it on) gets its own assertion, not just inferred from the reply
  round-trip above**: after descriptors are freed, `session.list` (or `pane.show`) on
  either connection still shows the session created in step one, with its program's
  status still running - a request round-tripping proves the *connection*/dispatch path
  survived, not that the session or its pty did. `KillOnDrop::kill_now()` to end the
  server (it does not die on its own - that is the point) so no process is left.
  **Negative control**: before writing the fix, this exact sequence was
  run once against unmodified `main` (the server dies instead of surviving, as the old
  test already proved) - recorded here rather than kept as code, since there is no way to
  keep "the old behaviour" around to re-run it against.
- **AC #4** (bounded attempts, no spin): `tests/accept_backoff.rs`'s two tests, both
  against the real `accept-backoff-attempt` fixture under a real lowered `RLIMIT_NOFILE`
  in its own isolated process - genuine `EMFILE`, not a mock. First test:
  `ACCEPT_BACKOFF_MS=20` (or whatever the production default is), hold descriptors
  exhausted for a fixed window (500ms), read the fixture's printed counts, assert the
  count stays under a generous bound (loosely ~`window / backoff` plus slack - wide
  enough that scheduling jitter never flakes it, narrow enough that a real spin, which
  would produce orders of magnitude more, cannot pass). **Negative control, kept as a
  real, permanent second test** (not a one-off manual run, since `accept_error_backoff`
  being config means it costs nothing to keep): `ACCEPT_BACKOFF_MS=0` against the same
  fixture, same window, asserting the count blows well past the bound - proving this
  harness can actually tell a spin from a backoff, not merely asserting a number that
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
  predicate. This task adds an error-classification arm plus a bounded `thread::sleep` in
  the *error* path of that same `match`; it does not touch the `Ok(accepted)` arm or
  anything after the `try_clone` success case, which is where TASK-1.86's own counting
  almost certainly lands. The one real interaction to flag for whoever builds second: a
  backoff sleep in the error path delays TASK-1.86's own idle check by up to one backoff
  interval (`DEFAULT_ACCEPT_ERROR_BACKOFF`, 20ms) - negligible against a 30-second idle
  window, but worth TASK-1.86's own plan saying so rather than discovering it in review.
  Recommend this task lands first (a small, local, self-contained change to the error
  arm only) and TASK-1.86 is built against its result, rather than the reverse - lower
  rebase cost either way, but TASK-1.86's own design (a wrapper/observer around the loop)
  is easier to write once this task's control flow inside the loop is settled.
- **The AC #3 test's descriptor-limit number is not yet known** - it has to be picked
  empirically once a real session can be started and held open against a lowered limit
  (see "Order of work", why this is built last). Recorded in the notes once chosen, not
  guessed here.
- **`EINTR`'s own arm is believed dead in practice on this target** (checked against the
  installed toolchain's own `rust-src`, not merely assumed) - kept anyway because AC #1
  names it explicitly and a classifier that is correct only for the errno std happens to
  retry today would be wrong the moment that changes.
<!-- SECTION:PLAN:END -->
