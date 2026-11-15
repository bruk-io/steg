---
id: TASK-1.41
title: 'Kernel: status in the first release, and when each one ends'
status: In Progress
assignee:
  - '@bruk.w1'
created_date: '2026-11-15T01:32:00Z'
updated_date: '2026-11-15T19:43:00Z'
labels:
  - size-3
milestone: m-0
dependencies:
  - TASK-1.40
parent_task_id: TASK-1
type: task
project: bruk-io-steg
asdlc_accepted_from: DRAFT-11
asdlc_branch: task-1.41-kernel-status-in-the-first-release-and
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
doc-11: R50, R82, R83. decision-7. The first release shows only what steg can know by itself. Time arrives as data, never from a clock inside the kernel.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A command pane is running while its program runs, and finished or failed from its exit code. A command typed at a shell prompt is left to the task that gets a prompt-marked command's exit code out of the terminal engine
- [ ] #2 Finished ends after 10 minutes or on input to the pane; failed ends only on input or when told the person has dealt with it; opening a tab ends nothing
- [ ] #3 A tab's shown status is the most pressing among its panes, and a session has none
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
## Open question that blocks half of AC #1 — needs the engineer's decision before phase 2

AC #1 asks for "finished or failed from its exit code, including for a command typed
at a shell prompt that reports prompt marks." That needs the terminal engine to report
a command's exit code when the shell marks it with OSC 133 (the "command ends" prompt
mark), for a `Program::Shell` pane where the shell process itself never exits between
commands.

I read `ghostty-vt` 0.8 (the exact vendored dependency, `crates/kernel/Cargo.toml`) to
check this rather than assume it:

- The raw FFI crate, `ghostty-vt-sys` (`~/.cargo/registry/.../ghostty-vt-sys-0.8.0.../src/bindings.rs:3291-3324`),
  *does* carry the data: a `GHOSTTY_TERMINAL_OPT_SEMANTIC_PROMPT` callback whose
  `TerminalSemanticPrompt` struct has `has_exit_code: bool` and `exit_code: i32`, set on
  the `COMMAND_END` step of OSC 133.
- The safe crate we actually depend on, `ghostty-vt` 0.8, does not wire that callback at
  all. `terminal.rs`'s `Event` enum (the only way this crate surfaces anything from the
  terminal) has no semantic-prompt variant and no exit code, anywhere
  (`grep -n "semantic\|exit_code" src/terminal.rs` — zero hits besides
  `is_cursor_at_prompt()`, a bare bool with no code). The standalone `osc::Parser`/
  `CommandType::SemanticPrompt` is a separate, payload-free marker from a parser that
  isn't fed from the `Terminal`'s own byte stream either.
- `Terminal::get`/`set`/`set_ptr`, the only way to reach a raw FFI tag, are `pub(crate)`
  inside `ghostty-vt` (`terminal.rs:483-512`) — unreachable from steg. And `crates/kernel/src/lib.rs`
  has `#![forbid(unsafe_code)]`, while `.claude/rules/rust.md` keeps FFI "in one small
  module per foreign interface" outside logic crates like the kernel — so even if the
  pointer were reachable, wiring this callback from inside the kernel isn't the kernel's
  to do.
- The server can't supply the exit code either: R16's exit code is the pane's own
  foreground process (`waitpid` on the child steg spawned). A command typed at a shell
  prompt is a child of the *shell*, invisible to the server's own process tracking.

So this is a gap in the vendored dependency, not a kernel design choice, and not
something a size-3 kernel task should paper over with its own OSC 133 parser (feeding
the pty bytes to a second parser alongside `Terminal::vt_write` would require tracking
two parsers' states in lockstep and still can't produce an exit code the C struct
doesn't expose through this binding — it would be inventing behaviour the library
doesn't give us, which I was told not to do).

**I'm stopping on this one clause rather than guessing.** The two real options, both the
engineer's call:
1. Treat `Program::Shell` panes as out of scope for Finished/Failed in this task — only
   `Program::Command` panes (and the shell's own process exiting, which closes the pane
   per R97 and so shows nothing afterward) get a status from `record_exit`'s existing
   exit-code plumbing. Raise a follow-up task against the ghostty-vt dependency
   (TASK-1.34's territory) to wire `OPT_SEMANTIC_PROMPT` behind a safe API, and widen
   AC #1 later once that lands.
2. Make wiring that callback a prerequisite of this task (i.e. this task doesn't close
   until it's unblocked).

Whichever is chosen, I did not want to invent a parser and call it done quietly. The
rest of this plan builds everything the gap doesn't touch: AC #1's command-pane half,
and AC #2 and #3 in full, which do not depend on this question's answer — only the
`Program::Shell` half of AC #1 does.

## The shape, and where it lives

A new sdk type, `Status` (`crates/sdk/src/status.rs`, new file; `mod status; pub use
status::Status;` added to `lib.rs` between `screen` and `wire`, alphabetically, matching
every other cross-API shape there):

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Running,
    Finished,
    Failed { exit_code: i32 },
}
```

Doc comments cite doc-11 R82/R83, decision-7, and doc-15's domain table ("Status: what
steg knows about a pane's program at this moment"). This is the M1 subset only —
`Waiting` and the bell are decision-7's M2 additions and are not variants here; adding
them later is additive, not a breaking change to this enum's existing variants.

It carries **no timing data**. `std::time::Instant` has no serde impl and is meaningless
off the process that made it, so it can never appear in a type that crosses the API
(R102/R105, and `crates/sdk/clippy.toml` bans the sdk from reading the clock the same
way the kernel is). All of Finished's 10-minute timer and every status's "since" live
only inside the kernel's own `Pane`, discussed below.

**Relation to `ProgramStatus`: wrapped, not replaced.** `ProgramStatus`'s own doc comment
(`crates/sdk/src/program.rs`) already names this moment: "A richer `Status` ... is built
on top of this in a later task; this is the one fact the kernel itself can always know
with no clock of its own." `ProgramStatus` keeps meaning exactly what it means today
(Running / Ended{exit_code}, no timers, feeds `pane.exit_code` and `closes_on_exit`/
`restart`'s own checks) and does not change shape. `Status` is additive state Pane now
also carries, derived from the same exit event but living through its own, longer
lifecycle (R83's ending rules) after `ProgramStatus` has already settled on `Ended`.

**Naming collision to resolve first.** `Pane` already has a field and accessor both
called `status`, but typed `Option<ProgramStatus>` — that name is now wrong for what it
holds, once doc-15's actual "Status" entity exists as its own type. I'm renaming that
existing field and its public accessor to `program_status`/`Pane::program_status()`
(and the one internal check in `Pane::restart` that matches on it), freeing `status` for
the new, correct thing. Blast radius: confined to `crates/kernel/src/pane.rs` alone — I
checked every call site in the repo (`git grep '\.status()'` against `origin/main`); the
only hits outside this file are `crates/kernel/src/tree.rs`'s own tests (same rename) and
one unrelated `std::process::Command::...status()` in `crates/server/src/pane.rs` that
is `ExitStatus`, not this `Pane` at all. Nothing in `server`, `steg` or `app` calls
`Pane::status()` today (API dispatch isn't built yet), so the rename is free of any
cross-crate ripple.

## Time arrives as data — two different kinds, used for two different things

Two separate questions need "when", and they get two different answers, so neither
forces the other's cost onto call sites that don't need it:

1. **Decision-7's tie-break ("how pressing, then newest, never FIFO")** needs only a
   *relative* order between any two panes' statuses, never a real duration. The tree
   already has exactly this primitive, built for the same reason (R91/decision-6's
   "latest client"): `Tree::tick()`, a `u64` the kernel bumps itself — "not a clock" by
   its own doc comment. I reuse it: every status transition (`start_program`,
   `record_exit`, `restart_program`) stamps the pane's new status with the tree's current
   tick. `Tree`'s own public signatures for `start_program` and `restart_program` don't
   change at all — `Tree` calls `self.tick()` internally and passes the `u64` down to
   `Pane`'s `pub(crate)` methods, the same way it already does for `declare_size`/
   `note_input`'s activity tracking. Only `Pane`'s lower-level, `pub(crate)` signatures
   gain a `tick: u64` parameter.
2. **R83's 10-minute rule for Finished** needs real elapsed wall-clock time, which a tick
   counter cannot give (ticks don't advance at a fixed rate). This is the one place a
   real `std::time::Instant` is unavoidable, and it enters at exactly one call:
   `Tree::record_exit`'s signature gains `now: Instant`, the caller's (the server's) own
   clock reading, passed in as plain data — never read by the kernel (`clippy.toml`'s
   `disallowed-methods` already bans `Instant::now`/`SystemTime::now` in this crate, and
   stays that way; nothing in this plan calls either). `start_program` and
   `restart_program` need no `Instant` at all: Running has no timer.

This keeps the *external* (Tree-level) surface change to exactly one new parameter on
one existing method, not three.

**Why not use `Instant` for the tie-break too (simpler, one mechanism)?** Because std's
`Instant` has no public constructor besides `.now()`, so every one of the ~15 existing
kernel unit tests that call `start_program`/`restart`/`record_exit` directly would need a
`#[expect(clippy::disallowed_methods, reason = "...")]`-scoped test helper just to get a
value to pass in, for a property (ordering) that doesn't need real time at all. Splitting
the two keeps that workaround to only the tests that exercise the one thing that
genuinely needs wall-clock time (`record_exit` and the 10-minute boundary), and every
other existing test call site gains a trivial `u64` literal instead.

## Pane-level mechanics (`crates/kernel/src/pane.rs`)

```rust
const FINISHED_LIFETIME: Duration = Duration::from_secs(10 * 60); // R83

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StatusKind {
    Running,
    Finished { since: Instant }, // R83's 10-minute clock starts here
    Failed { exit_code: i32 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct StatusRecord {
    kind: StatusKind,
    tick: u64, // decision-7's tie-break only; never compared to a duration
}
```

`Pane` gains one new private field, `status: Option<StatusRecord>` (the old field of
that name becomes `program_status`, as above). New methods:

- `pub fn status(&self, now: Instant) -> Option<Status>` — `None` if there is no record,
  or if it's `Finished` and `since + FINISHED_LIFETIME <= now` (expired — see the
  boundary rule below); otherwise the `Status` the record names. This is a pure read: it
  never mutates `self.status`, so an already-timed-out `Finished` simply stops being
  reported the moment `now` says it should, with no sweep needed.
- `pub(crate) fn status_for_ranking(&self, now: Instant) -> Option<(Status, u64)>` — the
  same filtering as `status`, plus the stored `tick`, so `Tab::shown_status` doesn't
  duplicate the expiry check.
- `pub(crate) fn next_status_deadline(&self, now: Instant) -> Option<Instant>` — `Some(since
  + FINISHED_LIFETIME)` only when the current status is `Finished` *and* that instant is
  still ahead of `now`; `None` for Running, Failed (R83: "never on a timer") or no status
  at all, or for a Finished deadline that has already passed (it's already reported as
  gone by `status`, so there's nothing left to schedule for it).
- `pub(crate) fn clear_status(&mut self)` — unconditionally `self.status = None`. The one
  mutating op, called from both "ends on input" and "ends on being told the person has
  dealt with it". Idempotent (a no-op when there's nothing to clear).

`start_program(&mut self, program: Program, tick: u64)`, `restart(&mut self, tick:
u64)` and `record_exit(&mut self, exit_code: i32, now: Instant, tick: u64)` each set
`program_status` exactly as they do today, and additionally set `status`:
`StatusKind::Running` (start/restart) or, on exit, `Finished { since: now }` for code 0
and `Failed { exit_code }` otherwise.

**Boundary rule, stated so the tests can pin it down:** "ends ... after 10 minutes"
means the status has already ended *at* the 10-minute mark, not only strictly after it:
`since + FINISHED_LIFETIME > now` is the "still showing" condition, so at
`now == since + 600s` it is already gone. Three tests pin this exactly: 599s (still
`Some(Finished)`), 600s (`None`), 601s (`None`).

## Ending rules (`crates/kernel/src/tree.rs`)

- **Input** (AC #2, both Finished and Failed): `Tree::note_input` gains one new line —
  after its existing viewing/size-reapplication logic, it calls `clear_status()` on the
  same pane it was already given. No signature or return-type change: it's a pure
  addition to what the method already does, matching exactly what "input to the pane"
  already means in this codebase (TASK-1.42's own `note_input`, which the task brief
  points at directly) — a key press, never a resize alone (`declare_size`) and never a
  bare focus (`Tab::focus`/`Tree::focus_pane` touch nothing here, which is also the proof
  for "opening a tab ends nothing" below). The four existing `note_input` tests assert
  only on its size-reapplication return value, so they're untouched; new tests cover the
  status-clearing effect on top.
- **"Told the person has dealt with it"**: a new `Tree::acknowledge_pane(&mut self, id:
  PaneId) -> Result<(), Error>` (name open to the engineer's preference — nothing in the
  docs fixes a word for it), erroring `PaneNotFound` the same way every other pane
  operation does, otherwise calling `clear_status()`. Per decision-7 point 5 ("Finished:
  on a key press or click ... Failed: on a key press or click"), both read the *same*
  word — "click" — as their second ending surface, so I clear whichever status is
  present, not Failed only. (The acceptance criterion's wording singles out "dealt with"
  for Failed only because, for Finished, it's redundant with the timer it already has —
  not because Finished is excluded from it; I'm flagging this reconciliation explicitly
  rather than picking one silently.) This is the kernel's half of doc-11's own open
  question #3 ("How the app tells the server a person has dealt with a pane by clicking
  in it... a click is not [input the server already sees]") — it answers "what the
  kernel operation is", not "how a click reaches the server", which stays open for
  doc-14/CLI design.
- **Timer** (Finished only): no mutation at all — see `status`/`next_status_deadline`
  above. It is a computed fact of `(stored record, now)`, never swept.
- **"Opening a tab ends nothing"**: true by construction — no code path reachable from
  `Tab::focus`, `Tree::focus_pane`, `Tree::focus_tab` or `Tree::focus_direction` touches
  `status` anywhere, and a test proves it directly (focus every pane and tab in a tab
  holding one Finished and one Failed pane; assert both are unchanged after).

## Tab aggregation (`crates/kernel/src/tab.rs`) and "a session has none"

```rust
pub(crate) fn shown_status(&self, now: Instant) -> Option<PaneId> {
    self.panes
        .iter()
        .filter_map(|p| p.status_for_ranking(now).map(|(s, tick)| (p.id(), s, tick)))
        .max_by_key(|(_, status, tick)| (Reverse(rank(*status)), *tick))
        .map(|(id, _, _)| id)
}

fn rank(status: Status) -> u8 {
    // decision-7: "waiting, failed, bell, finished, running" - M1 only has the last
    // three, in that order; lower number is more pressing.
    match status {
        Status::Failed { .. } => 0,
        Status::Finished => 1,
        Status::Running => 2,
    }
}
```

Exposed from `Tree` as `pub fn tab_shown_status(&self, tab: TabId, now: Instant) ->
Result<Option<PaneId>, Error>` (`TabNotFound` if unknown), matching the API
description's own words for the field this backs (`methods.rs`'s `tab_fields()`:
"shown_status ... the pane whose status this tab's ring stands for, if any") — returning
the pane id, not the `Status` value, on purpose, for exactly that reason.

Order taken from decision-7 point 2/doc-15's own list, not invented. The tie-break
("how pressing, then newest") is decision-7 point 3; I read it as a general rule for
"the most pressing of a tab's panes" (the same words AC #3 and R50 use), not only the M2
"set" bookkeeping R84 adds, so I've built it now rather than picking an undocumented
tie-break (e.g. pane position) that decision-7 never names.

**"A session has none"**: proven by absence, not a runtime assertion — `Session`
(`crates/kernel/src/session.rs`) gains no status field or method, and the API
description's `session_fields()` (`crates/sdk/src/description/methods.rs`) already has
none (checked: id, label, position, colour, current_tab — no status). Nothing in this
plan adds one.

`Tree::next_status_deadline(&self, now: Instant) -> Option<Instant>` — the minimum
`next_status_deadline(now)` across every pane in every tab in every session, or `None`
if nothing has a live timer. This is the "ask when the next thing is due, without
polling" capability the brief asked for: the server computes this once, sleeps until
that instant (or until something else happens first), and re-queries with the real
`now` when it wakes — the kernel never schedules anything or reads a clock itself.

## The sdk's description (`crates/sdk/src/description/methods.rs`)

Leaving `ValueKind::Text` exactly as it is on both `pane_fields()`'s `status` and
`tab_fields()`'s `shown_status`. `ValueKind` (`shapes.rs`) has no variant for a nested or
structured value at all — `layout`, which already has a real sdk type (`Layout`), is the
same `ValueKind::Text` placeholder today, for the same reason. Adding one is a change to
the CLI's self-description and doc-2's table generation, which is a bigger, separate
decision (the comments' own words point at a `PaneRecord`/`TabRecord` type that doesn't
exist yet) and isn't asked for by R49-R52, decision-7, or doc-16. What I will update:
the explanatory *comments* above `tab_fields()`/`pane_fields()`, which currently read as
if `Status` doesn't exist yet ("until TASK-1.41 gives them their own shape") — once this
task lands, I'll reword them to name `Status` directly and say the structured
representation is still owed, so a later reader isn't pointed at a task that already
merged. No behavioural change, comment-only.

## Left out of this task

- Everything decision-7/doc-11 scope to M2 or M3: `Waiting`, the bell, their own timers
  and notifications (R51), the per-tab status *set* and promotion (R84), and R52's
  events/CLI listing.
- Doc-11's own open question on whether `Running` needs a lifetime for a program that
  stops reporting without exiting — unresolved there, unresolved here.
- How a click reaches the server at all (doc-11's open question #3, doc-14) — only the
  kernel operation it will eventually call is built here.
- The `Program::Shell` half of AC #1 — the open question at the top of this plan.

## Proof

- Boundary, exactly as R83 states it: 599s/600s/601s after a Finished exit, as above.
- Each ending rule both ways, per status: input ends Finished; input ends Failed; input
  does *not* end Running (nothing to end); input to pane A does not touch pane B's
  status in the same tab; `acknowledge_pane` ends Finished; `acknowledge_pane` ends
  Failed; the timer ends Finished; the timer *never* ends Failed (a Failed pane still
  shows Failed arbitrarily far past 10 minutes with no input).
- "Opening a tab ends nothing": the test above, capturing both panes' `status(now)` and
  `next_status_deadline` before and after every focus operation, so it would fail if
  anything moved.
- The ordering table exhaustively for M1's three statuses: every one of the six
  orderings of {Running, Finished, Failed} across two panes in one tab, confirming
  `shown_status` always names the more pressing one, plus the tie-break test (two Failed
  panes, newer tick wins; two Finished panes, newer tick wins).
- `next_status_deadline` on an empty tree, and on a tree with only Running/Failed panes,
  returns `None` (nothing to poll for).
- A deliberate failing run: the new tests land before the implementation changes (new
  `status`/`shown_status`/`acknowledge_pane` methods don't exist yet), `just test` on the
  touched crates recorded failing for that reason, then recorded passing after the
  implementation.
- Rename fallout: every existing `pane.status()`/`ProgramStatus` test keeps asserting the
  same thing under `pane.program_status()`, confirming the rename changed no behaviour.

## Files

- `crates/sdk/src/status.rs` (new), `crates/sdk/src/lib.rs` (module + re-export).
- `crates/sdk/src/description/methods.rs` (comments only).
- `crates/kernel/src/pane.rs` (rename + new state + new methods + test updates).
- `crates/kernel/src/tab.rs` (`shown_status`, `rank`).
- `crates/kernel/src/tree.rs` (`record_exit` signature; internal tick-passing to
  `start_program`/`restart_program`; `note_input` addition; `acknowledge_pane`;
  `tab_shown_status`; `next_status_deadline`; test updates).

## Risks

- **Concurrent task on the same method.** `origin/main`'s latest commit is "Claim
  TASK-1.76 for @bruk.w4" — "Server: start a pane's program from the API, and end it on
  a forced close" — which is very likely to call `Tree::record_exit` from the server side
  for the first time. Its signature is changing here (`+ now: Instant`). Phase 2's first
  step (bringing in `origin/main`) needs to check whether that task has landed a caller
  in the meantime and update it, not just resolve a textual conflict.
- **Test-signature churn.** Roughly 15 existing unit tests across `pane.rs` and `tree.rs`
  call `start_program`/`restart`/`record_exit`/`pane.status()` directly and need
  mechanical updates (an added `tick`/`now` argument, or the `program_status` rename).
  None of their assertions change.
- **A new, scoped clippy exception.** Tests that need a concrete `Instant` (only the
  ones exercising `record_exit`/the 10-minute boundary) need a small
  `#[expect(clippy::disallowed_methods, reason = "...")]`-wrapped helper calling
  `Instant::now()` once, inside the test module — the same pattern
  `crates/sdk/src/local_socket.rs` already uses for its one real-I/O exception. Without
  it, `just check`'s `cargo clippy -D warnings` fails on the first test that needs a real
  `Instant`, since std gives no other way to construct one.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Split on 2026-11-15 under the engineer's standing decision to split by default. The worker found, by reading ghostty-vt 0.8 and ghostty-vt-sys, that the engine's safe API does not surface a prompt-marked command's exit code (the data is behind OPT_SEMANTIC_PROMPT in the sys bindings, which the safe crate never wires, and the kernel forbids unsafe). Criterion 1 is narrowed to command panes; finished and failed for a command typed at a shell prompt is a draft under TASK-1 that depends on this task.
<!-- SECTION:NOTES:END -->

## Comments

<!-- COMMENTS:BEGIN -->
created: 2026-11-15T19:34:00Z

---

Plan written. One open question blocks half of AC #1, needs the engineer's decision before build: ghostty-vt 0.8 (the exact vendored dependency) does not surface a prompt-marked command's exit code anywhere in its safe Rust API (verified by reading ghostty-vt-sys's bindings.rs, which has the data behind an OPT_SEMANTIC_PROMPT callback the safe ghostty-vt crate never wires, and ghostty-vt's own terminal.rs Event enum, which has no such variant). That blocks Finished/Failed for a command typed at a shell prompt (the Program::Shell half of AC #1) - a vendored-dependency gap, not a kernel design choice, and not something to work around with a hand-rolled OSC 133 parser. Program::Command panes, and AC #2 and #3 in full, are unaffected and planned in full below. Two options for the engineer, both outside this task's own scope to decide: descope the Shell-prompt clause to a follow-up task against the ghostty-vt dependency, or make wiring that callback a prerequisite of this one. See the plan for the full writeup and reasoning.

---
<!-- COMMENTS:END -->
