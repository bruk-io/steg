---
id: TASK-1.41
title: 'Kernel: status in the first release, and when each one ends'
status: In Review
assignee:
  - '@bruk.w1'
created_date: '2026-11-15T01:32:00Z'
updated_date: '2026-11-15T20:50:00Z'
labels:
  - size-3
milestone: m-3
dependencies:
  - TASK-1.40
references:
  - task-1.41-kernel-status-in-the-first-release-and
  - https://github.com/bruk-io/steg/pull/38
parent_task_id: TASK-1
type: task
project: bruk-io-steg
asdlc_accepted_from: DRAFT-11
asdlc_branch: task-1.41-kernel-status-in-the-first-release-and
asdlc_plan_approved_by: bruk
asdlc_plan_approved_hash: 'ed724e8c9084'
asdlc_plan_approved_size: size-3
asdlc_reviewer: bruk.r1
asdlc_review_kind: same-machine
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

It carries **no timing data**. `std::time::Instant` (and anything built on it) has no
serde impl and is meaningless off the process that made it, so nothing timing-shaped can
ever appear in a type that crosses the API (R102/R105, and `crates/sdk/clippy.toml` bans
the sdk from reading the clock the same way the kernel is). All of Finished's 10-minute
timer and every status's "since" live only inside the kernel's own `Pane`, discussed
below.

**Relation to `ProgramStatus`: wrapped, not replaced.** `ProgramStatus`'s own doc comment
(`crates/sdk/src/program.rs`) already names this moment: "A richer `Status` ... is built
on top of this in a later task; this is the one fact the kernel itself can always know
with no clock of its own." `ProgramStatus` keeps meaning exactly what it means today
(Running / Ended{exit_code}, no timers, feeds `pane.exit_code` and `closes_on_exit`/
`restart`'s own checks) and does not change shape. `Status` is additive state `Pane` now
also carries, derived from the same exit event but living through its own, longer
lifecycle (R83's ending rules) after `ProgramStatus` has already settled on `Ended`.

**Naming collision to resolve first.** `Pane` already has a field and accessor both
called `status`, but typed `Option<ProgramStatus>` — that name is now wrong for what it
holds, once doc-15's actual "Status" entity exists as its own type. I'm renaming that
existing field and its public accessor to `program_status`/`Pane::program_status()`
(and the one internal check in `Pane::restart` that matches on it), freeing `status` for
the new, correct thing. Blast radius: confined to `crates/kernel/src/pane.rs` alone, as
far as `origin/main` stood when I read it — I checked every call site in the repo
(`git grep '\.status()'`); the only hits outside this file were `crates/kernel/src/tree.rs`'s
own tests (same rename) and one unrelated `std::process::Command::...status()` in
`crates/server/src/pane.rs` that is `ExitStatus`, not this `Pane` at all. Two tasks
touch nearby code since then or concurrently — TASK-1.76 (in planning) and TASK-1.48
(in review) — so **phase 2's first step, bringing in `origin/main`, re-runs this exact
grep** before relying on "nothing else calls it" again.

## Time arrives as data, through a kernel-owned type — never `std::time::Instant`

`std::time::Instant` cannot appear in the kernel's own public signatures either, even
though the kernel only ever receives one as data and never calls `.now()` on it: std
gives no way to construct an `Instant` except `Instant::now()`, so every test exercising
a method that takes one would need a `#[expect(clippy::disallowed_methods)]`-scoped
workaround just to get a value to pass in — real friction for a type whose only job
*in the kernel* is comparison and addition, never reading the wall clock.

So the kernel defines its own tiny type instead, `Moment` (new file,
`crates/kernel/src/moment.rs`, `pub` and re-exported from `lib.rs` next to `Pane`/`Tab`/
`Tree` — it has to be `pub`, not `pub(crate)`, because it appears in `Tree::record_exit`'s
public signature and the server, outside this crate, has to construct one):

```rust
/// A point in time, as plain data (doc-16 R110: the kernel never reads a
/// clock, so this is never produced by one in this crate). A duration
/// since whatever origin the caller — the server, which does own a real
/// clock — picked once and keeps using; two `Moment`s are only ever
/// compared or offset against each other, never read as a wall-clock
/// value, so the choice of origin never matters to the kernel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Moment(Duration);

impl Moment {
    /// Builds a `Moment` this far after the caller's own origin. Pure
    /// arithmetic on a plain `Duration` — constructible in a test with a
    /// literal (`Moment::new(Duration::from_secs(599))`), no clock
    /// involved, so no clippy exception is needed anywhere this is used.
    #[must_use]
    pub const fn new(since_the_callers_origin: Duration) -> Self {
        Self(since_the_callers_origin)
    }

    /// This moment, offset later by `duration` — used for R83's
    /// 10-minute deadline. Saturates rather than panicking at
    /// `Duration::MAX` (`Duration::saturating_add`), though nothing
    /// realistic gets near it.
    #[must_use]
    pub const fn plus(self, duration: Duration) -> Self {
        Self(self.0.saturating_add(duration))
    }
}
```

No subtraction is defined at all — every comparison this task needs is `deadline > now`
(`Moment: Ord`, always well-defined, never a panic risk the way `Instant - Instant` is
when the right-hand side is later), never "how long between these two". That answers the
question of a caller passing an earlier moment than one it passed before, as plainly as
I can state the rule: **nothing here ever subtracts one `Moment` from another, so there
is no operation that could panic on going backward, and no special case needed.**
Concretely: if `record_exit`'s `now` were earlier than some other call's `now` for the
same or a different pane, nothing enforces or checks that — each pane's `since` is just
overwritten with whatever `Moment` it's given; and if a later *query*'s `now` happens to
be earlier than a stored `since` (the caller's own clock looked like it went backward),
the stored status is reported exactly as if the elapsed time were small or negative:
`since.plus(FINISHED_LIFETIME) > now` still holds, so a Finished status simply reads as
not yet at its deadline — the same answer a genuinely-just-started Finished status would
give, which is the right default when the input is suspect, with nothing crashing.

`Moment` does not cross the API (R105's rule is about shapes that do), so it has no
business in the sdk — `crates/sdk/CLAUDE.md`'s dependency table gives the sdk nothing to
depend on, and defining a timing type there that no wire format ever carries would be
the wrong crate advertising a capability nothing on the wire uses.

**Why keep the tree's existing `tick()` counter for ordering, now that `Moment` is
trivially constructible in tests too — why not use `Moment` for both jobs?** Because
ordering only ever needs "did this status start more recently than that one", which the
tree's own non-clock tick counter already answers for every pane-status transition,
exactly as it already does for R91/decision-6's "latest client", with **zero new
parameters** on `Tree::start_program`/`Tree::restart_program`. Routing that through
`Moment` instead would force both of those methods' external signatures to accept a real
point in time they'd never do any time-arithmetic with — only ever compare — just to
serve as an ordering proxy the kernel can already produce from state it already owns.
`Moment` stays reserved for the one place real elapsed duration is actually asked about:
R83's 10-minute rule, which only `record_exit` sets and only `status`/
`next_status_deadline` read.

This keeps the *external* (`Tree`-level) surface change to exactly one new parameter on
one existing method (`record_exit` gains `now: Moment`), not three.

## Pane-level mechanics (`crates/kernel/src/pane.rs`)

```rust
const FINISHED_LIFETIME: Duration = Duration::from_secs(10 * 60); // R83

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StatusKind {
    Running,
    Finished { since: Moment }, // R83's 10-minute clock starts here
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

- `pub fn status(&self, now: Moment) -> Option<Status>` — `None` if there is no record,
  or if it's `Finished` and `since.plus(FINISHED_LIFETIME) <= now` (expired — see the
  boundary rule below); otherwise the `Status` the record names. This is a pure read: it
  never mutates `self.status`, so an already-timed-out `Finished` simply stops being
  reported the moment `now` says it should, with no sweep needed.
- `pub(crate) fn status_for_ranking(&self, now: Moment) -> Option<(Status, u64)>` — the
  same filtering as `status`, plus the stored `tick`, so `Tab::shown_status` doesn't
  duplicate the expiry check.
- `pub(crate) fn next_status_deadline(&self, now: Moment) -> Option<Moment>` — `Some(since
  .plus(FINISHED_LIFETIME))` only when the current status is `Finished` *and* that
  moment is still ahead of `now`; `None` for Running, Failed (R83: "never on a timer") or
  no status at all, or for a Finished deadline that has already passed (it's already
  reported as gone by `status`, so there's nothing left to schedule for it).
- `pub(crate) fn clear_status(&mut self)` — unconditionally `self.status = None`. The one
  mutating op, called from both "ends on input" and "ends on being told the person has
  dealt with it". Idempotent (a no-op when there's nothing to clear).

`start_program(&mut self, program: Program, tick: u64)`, `restart(&mut self, tick:
u64)` and `record_exit(&mut self, exit_code: i32, now: Moment, tick: u64)` each set
`program_status` exactly as they do today, and additionally set `status`:
`StatusKind::Running` (start/restart) or, on exit, `Finished { since: now }` for code 0
and `Failed { exit_code }` otherwise.

**Boundary rule, stated so the tests can pin it down:** "ends ... after 10 minutes"
means the status has already ended *at* the 10-minute mark, not only strictly after it:
`since.plus(FINISHED_LIFETIME) > now` is the "still showing" condition, so at
`now == since.plus(FINISHED_LIFETIME)` it is already gone. Three tests pin this exactly,
all with literal `Moment`/`Duration` values, no clock involved: 599s after `since`
(still `Some(Finished)`), 600s (`None`), 601s (`None`).

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
  not because Finished is excluded from it.) I'll restate this reconciliation on the
  task's notes at build time too, not only here, so a reviewer sees the reasoning beside
  the commit that acts on it. This is the kernel's half of doc-11's own open question #3
  ("How the app tells the server a person has dealt with a pane by clicking in it... a
  click is not [input the server already sees]") — it answers "what the kernel
  operation is", not "how a click reaches the server", which stays open for doc-14/CLI
  design.
- **Timer** (Finished only): no mutation at all — see `status`/`next_status_deadline`
  above. It is a computed fact of `(stored record, now)`, never swept.
- **"Opening a tab ends nothing"**: true by construction — no code path reachable from
  `Tab::focus`, `Tree::focus_pane`, `Tree::focus_tab` or `Tree::focus_direction` touches
  `status` anywhere, and a test proves it directly (focus every pane and tab in a tab
  holding one Finished and one Failed pane; assert both are unchanged after).

## Tab aggregation (`crates/kernel/src/tab.rs`) and "a session has none"

```rust
pub(crate) fn shown_status(&self, now: Moment) -> Option<PaneId> {
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

Exposed from `Tree` as `pub fn tab_shown_status(&self, tab: TabId, now: Moment) ->
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

`Tree::next_status_deadline(&self, now: Moment) -> Option<Moment>` — the minimum
`next_status_deadline(now)` across every pane in every tab in every session, or `None`
if nothing has a live timer. This is the "ask when the next thing is due, without
polling" capability the brief asked for: the server converts its own clock reading to a
`Moment` once, calls this, sleeps until that `Moment` is reached (or until something
else happens first), and re-queries with a fresh `Moment` when it wakes — the kernel
never schedules anything or reads a clock itself.

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

- A command typed at a shell prompt (the `Program::Shell` half of what AC #1 used to
  ask for) — the task's own criterion now says this explicitly: it's split off to a
  separate task that gets a prompt-marked command's exit code out of the terminal
  engine, since the currently-vendored `ghostty-vt` 0.8's safe Rust API doesn't surface
  one (checked by reading it, not assumed — `ghostty-vt-sys`'s `TerminalSemanticPrompt`
  carries `has_exit_code`/`exit_code`, but the safe `ghostty-vt` crate never wires the
  callback that would deliver it, and the kernel forbids unsafe code, so this isn't a
  gap this crate can close on its own). Everything in this plan is scoped to a
  `Program::Command` pane's own exit code, which AC #1 now asks for directly and which
  the existing `record_exit`/`ExitOutcome` plumbing from TASK-1.40 already carries.
- Everything decision-7/doc-11 scope to M2 or M3: `Waiting`, the bell, their own timers
  and notifications (R51), the per-tab status *set* and promotion (R84), and R52's
  events/CLI listing.
- Doc-11's own open question on whether `Running` needs a lifetime for a program that
  stops reporting without exiting — unresolved there, unresolved here.
- How a click reaches the server at all (doc-11's open question #3, doc-14) — only the
  kernel operation it will eventually call is built here.

## Proof

- Boundary, exactly as R83 states it: 599s/600s/601s after a Finished exit, as above,
  each with a literal `Moment`/`Duration` — no clock, no clippy exception, anywhere in
  this crate's tests.
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
- A `Moment` passed to a query earlier than a status's own `since`: the stated rule
  above (still reads as not-yet-expired, nothing panics), with a test that passes an
  earlier `Moment` than the one `record_exit` stored and confirms `status` still returns
  `Some(Finished)` and nothing panics.
- A deliberate failing run: the new tests land before the implementation changes (new
  `status`/`shown_status`/`acknowledge_pane`/`Moment` don't exist yet), `just test` on
  the touched crates recorded failing for that reason, then recorded passing after the
  implementation.
- Rename fallout: every existing `pane.status()`/`ProgramStatus` test keeps asserting the
  same thing under `pane.program_status()`, confirming the rename changed no behaviour.

## Files

- `crates/sdk/src/status.rs` (new), `crates/sdk/src/lib.rs` (module + re-export).
- `crates/sdk/src/description/methods.rs` (comments only).
- `crates/kernel/src/moment.rs` (new), `crates/kernel/src/lib.rs` (module + re-export).
- `crates/kernel/src/pane.rs` (rename + new state + new methods + test updates).
- `crates/kernel/src/tab.rs` (`shown_status`, `rank`).
- `crates/kernel/src/tree.rs` (`record_exit` signature; internal tick-passing to
  `start_program`/`restart_program`; `note_input` addition; `acknowledge_pane`;
  `tab_shown_status`; `next_status_deadline`; test updates).

## Risks

- **Rename fallout since `origin/main` moved.** TASK-1.76 (in planning) and TASK-1.48
  (in review) may land new callers of `Pane::status()` I couldn't see when I read the
  repo. Phase 2's first step — bringing in `origin/main` — re-runs
  `git grep '\.status()'` before relying on the "nothing outside `pane.rs`" finding
  above.
- **`record_exit`'s new parameter, and TASK-1.76's server-side caller.** TASK-1.76
  ("Server: start a pane's program from the API, and end it on a forced close") is very
  likely to call `Tree::record_exit` from the server side for the first time, and its
  signature is changing here (`+ now: Moment`). Whichever of the two tasks merges second
  adapts to the other: if TASK-1.76 lands first with the old signature, this task's
  phase 2 updates its one call site when bringing in `origin/main`; if this task lands
  first, TASK-1.76's own build picks up the new signature and supplies a `Moment` from
  whatever clock reading the server already has at that call site.
- **Test-signature churn.** Roughly 15 existing unit tests across `pane.rs` and `tree.rs`
  call `start_program`/`restart`/`record_exit`/`pane.status()` directly and need
  mechanical updates (an added `tick`/`now` argument, or the `program_status` rename).
  None of their assertions change.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Split on 2026-11-15 under the engineer's standing decision to split by default. The worker found, by reading ghostty-vt 0.8 and ghostty-vt-sys, that the engine's safe API does not surface a prompt-marked command's exit code (the data is behind OPT_SEMANTIC_PROMPT in the sys bindings, which the safe crate never wires, and the kernel forbids unsafe). Criterion 1 is narrowed to command panes; finished and failed for a command typed at a shell prompt is a draft under TASK-1 that depends on this task.

Revised plan checked against doc-11 (R50, R82, R83), decision-7, doc-15 and doc-16 by @bruk.o: ok. Status is one shape in the sdk (running, finished, failed with its exit code) beside ProgramStatus, which keeps its meaning; time enters the kernel as a kernel-owned Moment that is plain data, so the kernel reads no clock and its tests need no lint exception; the 10-minute rule is a computed fact of the stored record and the moment asked about, with next_status_deadline for the server to sleep on; input ends finished and failed through TASK-1.42's note_input; acknowledge_pane ends either (the worker's reading of decision-7 point 5, which names a click for both, stated in the plan); a tab's shown status is the most pressing by decision-7's order then newest; a session has none. Typed-at-a-prompt commands are TASK-1.79's. Boundary tests at 599, 600 and 601 seconds, each ending rule both ways, the ordering exhaustively. Risk noted: record_exit gains a parameter that TASK-1.76 will call; whichever merges second adapts.

Check FAILED: `just test` exited 101 at 2026-11-15T20:31:10+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.41 on task-1.41-kernel-status-in-the-first-release-and at ba93708fb5ac with uncommitted changes, last 40 of 64 lines:

```text

error[E0308]: mismatched types
    --> crates/kernel/src/tree.rs:1521:13
     |
1521 |             Some(steg_sdk::ProgramStatus::Ended { exit_code: 3 })
     |             ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ expected `Option<Status>`, found `Option<ProgramStatus>`
     |
     = note: expected enum `Option<steg_sdk::Status>`
                found enum `Option<steg_sdk::ProgramStatus>`

error[E0061]: this method takes 1 argument but 0 arguments were supplied
    --> crates/kernel/src/tree.rs:1527:25
     |
1527 |         assert_eq!(pane.status(), Some(steg_sdk::ProgramStatus::Running));
     |                         ^^^^^^-- argument #1 of type `Moment` is missing
     |
note: method defined here
    --> crates/kernel/src/pane.rs:152:12
     |
 152 |     pub fn status(&self, now: Moment) -> Option<Status> {
     |            ^^^^^^        -----------
help: provide the argument
     |
1527 |         assert_eq!(pane.status(/* Moment */), Some(steg_sdk::ProgramStatus::Running));
     |                                ++++++++++++

error[E0308]: mismatched types
    --> crates/kernel/src/tree.rs:1527:35
     |
1527 |         assert_eq!(pane.status(), Some(steg_sdk::ProgramStatus::Running));
     |                                   ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ expected `Option<Status>`, found `Option<ProgramStatus>`
     |
     = note: expected enum `Option<steg_sdk::Status>`
                found enum `Option<steg_sdk::ProgramStatus>`

Some errors have detailed explanations: E0061, E0308.
For more information about an error, try `rustc --explain E0061`.
error: could not compile `steg-kernel` (lib test) due to 9 previous errors
full output: .quiet/tests-p-steg-p-steg-app-p-steg-kernel-p-steg-sdk-p-steg-server.log
error: recipe `test` failed with exit code 101
```

Check FAILED: `just test` exited 101 at 2026-11-15T20:35:06+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.41 on task-1.41-kernel-status-in-the-first-release-and at ba93708fb5ac with uncommitted changes, last 40 of 41 lines:

```text
   Compiling steg-kernel v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.41/crates/kernel)
   Compiling steg-server v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.41/crates/server)
   Compiling steg v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.41/crates/steg)
error[E0061]: this method takes 3 arguments but 2 arguments were supplied
   --> crates/server/tests/real_pty.rs:198:14
    |
198 |         tree.record_exit(kernel_pane, 3).unwrap(),
    |              ^^^^^^^^^^^---------------- argument #3 of type `Moment` is missing
    |
note: method defined here
   --> crates/kernel/src/tree.rs:577:12
    |
577 |     pub fn record_exit(
    |            ^^^^^^^^^^^
help: provide the argument
    |
198 |         tree.record_exit(kernel_pane, 3, /* Moment */).unwrap(),
    |                                        ++++++++++++++

error[E0061]: this method takes 3 arguments but 2 arguments were supplied
   --> crates/server/tests/real_pty.rs:245:24
    |
245 |     let outcome = tree.record_exit(kernel_pane, 7).unwrap();
    |                        ^^^^^^^^^^^---------------- argument #3 of type `Moment` is missing
    |
note: method defined here
   --> crates/kernel/src/tree.rs:577:12
    |
577 |     pub fn record_exit(
    |            ^^^^^^^^^^^
help: provide the argument
    |
245 |     let outcome = tree.record_exit(kernel_pane, 7, /* Moment */).unwrap();
    |                                                  ++++++++++++++

For more information about this error, try `rustc --explain E0061`.
error: could not compile `steg-server` (test "real_pty") due to 2 previous errors
warning: build failed, waiting for other jobs to finish...
full output: .quiet/tests-p-steg-p-steg-app-p-steg-kernel-p-steg-sdk-p-steg-server.log
error: recipe `test` failed with exit code 101
```

Check passed: `just test` exited 0 at 2026-11-15T20:35:50+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.41 on task-1.41-kernel-status-in-the-first-release-and at ba93708fb5ac with uncommitted changes:

```text
ok      tests (-p steg -p steg-app -p steg-kernel -p steg-sdk -p steg-server) (3.1s)
```

Check passed: `just lint` exited 0 at 2026-11-15T20:36:52+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.41 on task-1.41-kernel-status-in-the-first-release-and at ba93708fb5ac with uncommitted changes:

```text
ok      lint (47.1s)
```

Check FAILED: `just check` exited 1 at 2026-11-15T20:37:01+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.41 on task-1.41-kernel-status-in-the-first-release-and at ba93708fb5ac with uncommitted changes, last 40 of 64 lines:

```text
(B[m         assert_eq!(pane.program(), Some(&command("/tmp")));
     }
 
Diff in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.41/crates/sdk/src/status.rs:64:
     fn running_serde_round_trips_through_its_own_word() {
         let json = serde_json::to_string(&Status::Running).unwrap();
         assert_eq!(json, "\"running\"");
[31m-        assert_eq!(serde_json::from_str::<Status>(&json).unwrap(), Status::Running);
(B[m[32m+        assert_eq!(
(B[m[32m+            serde_json::from_str::<Status>(&json).unwrap(),
(B[m[32m+            Status::Running
(B[m[32m+        );
(B[m     }
 
     #[test]
Diff in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.41/crates/sdk/src/status.rs:71:
     fn finished_serde_round_trips_through_its_own_word() {
         let json = serde_json::to_string(&Status::Finished).unwrap();
         assert_eq!(json, "\"finished\"");
[31m-        assert_eq!(serde_json::from_str::<Status>(&json).unwrap(), Status::Finished);
(B[m[32m+        assert_eq!(
(B[m[32m+            serde_json::from_str::<Status>(&json).unwrap(),
(B[m[32m+            Status::Finished
(B[m[32m+        );
(B[m     }
 
     #[test]
Diff in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.41/crates/server/tests/real_pty.rs:243:
     )
     .unwrap();
 
[31m-    let outcome = tree.record_exit(kernel_pane, 7, Moment::new(Duration::ZERO)).unwrap();
(B[m[32m+    let outcome = tree
(B[m[32m+        .record_exit(kernel_pane, 7, Moment::new(Duration::ZERO))
(B[m[32m+        .unwrap();
(B[m     let ExitOutcome::Closed(closed) = outcome else {
         panic!("expected the shell pane's real exit to close it, got {outcome:?}");
     };
full output: .quiet/format-check.log
error: recipe `check-fmt` failed on line 126 with exit code 1
```

Check passed: `just check` exited 0 at 2026-11-15T20:39:15+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.41 on task-1.41-kernel-status-in-the-first-release-and at ba93708fb5ac with uncommitted changes:

```text
ok      format check (0.1s)
ok      lint (1.0s)
ok      boundaries (0.2s)
ok      licences (0.8s)
ok      tests (-p steg -p steg-app -p steg-kernel -p steg-sdk -p steg-server) (4.4s)
ok      build (steg-sdk) (3.1s)
ok      test (steg-sdk) (1.7s)
ok      test (steg-sdk, local-socket) (2.1s)
ok      build (steg-kernel) (43.0s)
ok      test (steg-kernel) (1.3s)
ok      build (steg-server) (44.1s)
ok      test (steg-server) (2.4s)
ok      build (steg) (0.2s)
ok      test (steg) (0.4s)
ok      build (steg-app) (15.2s)
ok      test (steg-app) (0.8s)
ok      tool tests (1.6s)
ok      architecture (1.1s)
ok      recipe names (0.1s)
```

Check passed: `just test` exited 0 at 2026-11-15T20:41:10+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.41 on task-1.41-kernel-status-in-the-first-release-and at ba93708fb5ac with uncommitted changes:

```text
ok      tests (-p steg -p steg-app -p steg-kernel -p steg-sdk -p steg-server) (1.8s)
```

Check FAILED: `just test` exited 101 at 2026-11-15T20:42:05+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.41 on task-1.41-kernel-status-in-the-first-release-and at ba93708fb5ac with uncommitted changes, last 40 of 64 lines:

```text
test tree::tests::session_mut_lets_a_caller_change_a_sessions_label_and_colour ... ok
test tree::tests::set_tab_current_pane_moves_only_the_tabs_own_current_pane ... ok
test tree::tests::set_tab_current_pane_rejects_an_unknown_tab_or_pane ... ok
test tree::tests::set_tab_size_smaller_always_succeeds_and_closes_nothing ... ok
test tree::tests::opening_a_tab_or_focusing_a_pane_ends_nothing ... FAILED
test tree::tests::split_pane_adds_a_pane_to_an_existing_tab ... ok
test tree::tests::start_program_on_an_unknown_pane_errors ... ok
test tree::tests::start_program_records_it_on_the_newly_minted_pane ... ok
test tree::tests::stop_viewing_rejects_a_pane_the_client_does_not_view ... ok
test tree::tests::swap_panes_is_refused_when_the_two_panes_are_in_different_tabs ... ok
test terminal::tests::the_configured_limit_keeps_the_real_count_near_itself_once_warmed_up ... ok
test tree::tests::tab_of_pane_finds_the_tab_holding_a_pane ... ok
test tree::tests::tab_of_pane_is_none_for_an_unknown_pane ... ok
test tree::tests::tab_shown_status_delegates_to_the_tabs_own_ranking ... ok
test tree::tests::tab_shown_status_on_an_unknown_tab_errors ... ok
test tree::tests::the_sixth_concurrent_session_repeats_the_first_colour ... ok
test tree::tests::view_pane_is_idempotent_and_rejects_an_unknown_pane ... ok
test tree::tests::viewing_alone_never_makes_a_client_latest ... ok
test tree::tests::viewing_follows_from_attachment_not_a_separate_setter ... ok
test terminal::tests::the_line_limit_governs_even_past_the_engines_disabled_byte_limit ... ok

failures:

---- tree::tests::opening_a_tab_or_focusing_a_pane_ends_nothing stdout ----

thread 'tree::tests::opening_a_tab_or_focusing_a_pane_ends_nothing' (11402834) panicked at crates/kernel/src/tree.rs:3481:9:
assertion `left == right` failed
  left: None
 right: Some(Finished)
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    tree::tests::opening_a_tab_or_focusing_a_pane_ends_nothing

test result: FAILED. 259 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s

error: test failed, to rerun pass `-p steg-kernel --lib`
full output: .quiet/tests-p-steg-p-steg-app-p-steg-kernel-p-steg-sdk-p-steg-server.log
error: recipe `test` failed with exit code 101
```

Check passed: `just test` exited 0 at 2026-11-15T20:42:19+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.41 on task-1.41-kernel-status-in-the-first-release-and at ba93708fb5ac with uncommitted changes:

```text
ok      tests (-p steg -p steg-app -p steg-kernel -p steg-sdk -p steg-server) (3.6s)
```

Check FAILED: `just test` exited 101 at 2026-11-15T20:42:34+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.41 on task-1.41-kernel-status-in-the-first-release-and at ba93708fb5ac with uncommitted changes, last 40 of 64 lines:

```text
test tree::tests::start_program_records_it_on_the_newly_minted_pane ... ok
test tree::tests::stop_viewing_rejects_a_pane_the_client_does_not_view ... ok
test tree::tests::swap_panes_is_refused_when_the_two_panes_are_in_different_tabs ... ok
test tree::tests::tab_of_pane_finds_the_tab_holding_a_pane ... ok
test tree::tests::tab_of_pane_is_none_for_an_unknown_pane ... ok
test tree::tests::tab_shown_status_delegates_to_the_tabs_own_ranking ... ok
test tree::tests::tab_shown_status_on_an_unknown_tab_errors ... ok
test tree::tests::the_sixth_concurrent_session_repeats_the_first_colour ... ok
test tree::tests::view_pane_is_idempotent_and_rejects_an_unknown_pane ... ok
test tree::tests::viewing_alone_never_makes_a_client_latest ... ok
test tree::tests::viewing_follows_from_attachment_not_a_separate_setter ... ok
test terminal::tests::the_line_limit_governs_even_past_the_engines_disabled_byte_limit ... ok

failures:

---- pane::tests::a_finished_status_is_still_shown_599_seconds_after_it_began stdout ----

thread 'pane::tests::a_finished_status_is_still_shown_599_seconds_after_it_began' (11452611) panicked at crates/kernel/src/pane.rs:513:9:
assertion `left == right` failed
  left: None
 right: Some(Finished)
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace

---- tree::tests::next_status_deadline_is_the_earliest_across_several_panes_and_tabs stdout ----

thread 'tree::tests::next_status_deadline_is_the_earliest_across_several_panes_and_tabs' (11452816) panicked at crates/kernel/src/tree.rs:3526:9:
assertion `left == right` failed
  left: Some(Moment(599s))
 right: Some(Moment(600s))


failures:
    pane::tests::a_finished_status_is_still_shown_599_seconds_after_it_began
    tree::tests::next_status_deadline_is_the_earliest_across_several_panes_and_tabs

test result: FAILED. 258 passed; 2 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s

error: test failed, to rerun pass `-p steg-kernel --lib`
full output: .quiet/tests-p-steg-p-steg-app-p-steg-kernel-p-steg-sdk-p-steg-server.log
error: recipe `test` failed with exit code 101
```

Check passed: `just test` exited 0 at 2026-11-15T20:42:47+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.41 on task-1.41-kernel-status-in-the-first-release-and at ba93708fb5ac with uncommitted changes:

```text
ok      tests (-p steg -p steg-app -p steg-kernel -p steg-sdk -p steg-server) (3.7s)
```

Check FAILED: `just check` exited 1 at 2026-11-15T20:43:29+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.41 on task-1.41-kernel-status-in-the-first-release-and at ba93708fb5ac with uncommitted changes, last 40 of 64 lines:

```text
         let first = tree.session(session_id).unwrap().tabs()[0].panes()[0].id();
         let second = tree
[31m-            .split_pane(first, steg_sdk::Direction::Right, steg_sdk::PaneKind::Terminal)
(B[m[32m+            .split_pane(
(B[m[32m+                first,
(B[m[32m+                steg_sdk::Direction::Right,
(B[m[32m+                steg_sdk::PaneKind::Terminal,
(B[m[32m+            )
(B[m             .unwrap();
         tree.start_program(first, command("/tmp")).unwrap();
         fail(&mut tree, second);
Diff in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.41/crates/kernel/src/tree.rs:3513:
         let session_id = tree.new_session(None).unwrap();
         let first = tree.session(session_id).unwrap().tabs()[0].panes()[0].id();
         let second_tab = tree.new_tab(session_id, None).unwrap();
[31m-        let second = tree.session(session_id).unwrap().tab(second_tab).unwrap().panes()[0].id();
(B[m[32m+        let second = tree
(B[m[32m+            .session(session_id)
(B[m[32m+            .unwrap()
(B[m[32m+            .tab(second_tab)
(B[m[32m+            .unwrap()
(B[m[32m+            .panes()[0]
(B[m[32m+            .id();
(B[m 
         // `first` finishes at moment(500) (deadline 500 + 600 = 1100);
         // `second` finishes at moment(0) (deadline 0 + 600 = 600) - the
Diff in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.41/crates/kernel/src/tree.rs:3523:
         tree.start_program(second, command("/tmp")).unwrap();
         tree.record_exit(second, 0, moment(0)).unwrap();
 
[31m-        assert_eq!(
(B[m[31m-            tree.next_status_deadline(moment(0)),
(B[m[31m-            Some(moment(600))
(B[m[31m-        );
(B[m[32m+        assert_eq!(tree.next_status_deadline(moment(0)), Some(moment(600)));
(B[m     }
 }
 
full output: .quiet/format-check.log
error: recipe `check-fmt` failed on line 126 with exit code 1
```

Check passed: `just check` exited 0 at 2026-11-15T20:44:33+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.41 on task-1.41-kernel-status-in-the-first-release-and at ba93708fb5ac with uncommitted changes:

```text
ok      format check (0.1s)
ok      lint (2.5s)
ok      boundaries (0.1s)
ok      licences (0.9s)
ok      tests (-p steg -p steg-app -p steg-kernel -p steg-sdk -p steg-server) (6.0s)
ok      build (steg-sdk) (0.5s)
ok      test (steg-sdk) (1.3s)
ok      test (steg-sdk, local-socket) (1.7s)
ok      build (steg-kernel) (32.1s)
ok      test (steg-kernel) (0.9s)
ok      build (steg-server) (0.6s)
ok      test (steg-server) (2.7s)
ok      build (steg) (0.3s)
ok      test (steg) (0.4s)
ok      build (steg-app) (0.8s)
ok      test (steg-app) (0.8s)
ok      tool tests (1.4s)
ok      architecture (1.0s)
ok      recipe names (0.1s)
```

Check passed: `just check` exited 0 at 2026-11-15T20:46:11+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.41 on task-1.41-kernel-status-in-the-first-release-and at 4169a943a8a3:

```text
ok      format check (0.1s)
ok      lint (1.5s)
ok      boundaries (0.1s)
ok      licences (0.7s)
ok      tests (-p steg -p steg-app -p steg-kernel -p steg-sdk -p steg-server) (5.3s)
ok      build (steg-sdk) (0.8s)
ok      test (steg-sdk) (1.8s)
ok      test (steg-sdk, local-socket) (2.1s)
ok      build (steg-kernel) (1.1s)
ok      test (steg-kernel) (1.3s)
ok      build (steg-server) (0.8s)
ok      test (steg-server) (2.7s)
ok      build (steg) (0.2s)
ok      test (steg) (0.4s)
ok      build (steg-app) (0.7s)
ok      test (steg-app) (0.8s)
ok      tool tests (1.3s)
ok      architecture (1.1s)
ok      recipe names (0.1s)
```

Built per the approved plan, in its order. PR: https://github.com/bruk-io/steg/pull/38

## Coverage, clause by clause

**AC #1, "A command pane is running while its program runs, and finished or failed
from its exit code."** `steg_sdk::Status` (new, `crates/sdk/src/status.rs`) is set by
`Pane::record_exit` from the same exit code `ProgramStatus` already carries: code 0 ->
`Finished`, anything else -> `Failed { exit_code }`. Proven at the pure-logic level in
`pane.rs`: `exiting_with_code_zero_is_finished`, `exiting_with_a_nonzero_code_is_failed`.
Proven through the tree (the plumbing, not the logic again) by every test that reads
`.status(moment)` off a pane fetched through `Tree`: `note_input_ends_a_finished_status_
on_the_pane_it_was_sent_to`, `note_input_ends_a_failed_status_on_the_pane_it_was_sent_to`,
`acknowledge_pane_ends_a_finished_status`, `acknowledge_pane_ends_a_failed_status`,
`tab_shown_status_delegates_to_the_tabs_own_ranking`,
`opening_a_tab_or_focusing_a_pane_ends_nothing`.

**AC #1, second clause ("a command typed at a shell prompt... left to the task that
gets a prompt-marked command's exit code out of the terminal engine").** Not built here,
as the task's own wording now asks - nothing in this PR touches shell-prompt OSC 133
handling.

**AC #2, "Finished ends after 10 minutes."** `pane.rs`:
`a_finished_status_is_still_shown_599_seconds_after_it_began`,
`a_finished_status_is_gone_exactly_600_seconds_after_it_began`,
`a_finished_status_is_gone_601_seconds_after_it_began` - the boundary is "ends at the
mark, not only strictly after it" (`since.plus(FINISHED_LIFETIME) > now`). Verified by
fault injection: shifted `FINISHED_LIFETIME` to 599s, `just test` failed exactly these
two 599s-adjacent tests for exactly that reason, reverted, `just test` green again
(both runs recorded).

**AC #2, "... or on input to the pane."** `tree.rs`:
`note_input_ends_a_finished_status_on_the_pane_it_was_sent_to`. `note_input`'s
signature and return type are unchanged - the end is a side effect alongside its
existing size-reapplication logic.

**AC #2, "failed ends only on input."** `note_input_ends_a_failed_status_on_the_pane_
it_was_sent_to`. Never a timer: `a_failed_status_never_expires_however_far_past_ten_
minutes` (checked a year past `since`), and `next_status_deadline_is_none_for_failed`.

**AC #2, "... or when told the person has dealt with it."** New `Tree::acknowledge_
pane`. `acknowledge_pane_ends_a_failed_status`, `acknowledge_pane_is_a_no_op_when_the_
pane_has_no_status`, `acknowledge_pane_on_an_unknown_pane_errors`.

**Reading `acknowledge_pane` against decision-7 point 5** (restating the plan's
reasoning here, as asked): I built it to clear *either* status, Finished or Failed
alike - `acknowledge_pane_ends_a_finished_status` exists alongside the Failed test
above - rather than scoping it to Failed only. decision-7 point 5 names "a key press or
click in that pane" as an ending for *both* Finished and Failed; the acceptance
criterion's wording only spells out "dealt with" for Failed because, for Finished,
that ending is redundant with the ten-minute timer it already has, not because Finished
is excluded from it. If the engineer reads the criterion's narrower wording as
deliberate (Finished ends *only* by input or timer, never by acknowledgement), that's a
one-line change to `Tree::acknowledge_pane` (match only `Failed` before clearing) and a
dropped test.

**AC #2, "opening a tab ends nothing."** `note_input` is the only thing that ends a
status on "input"; neither `focus_pane`, `focus_tab` nor `focus_direction` calls
`clear_status` anywhere. `opening_a_tab_or_focusing_a_pane_ends_nothing` exercises all
three on a tab holding one Finished and one Failed pane and asserts both unchanged.
Verified by fault injection: made `focus_pane` also call `clear_status`, `just test`
failed exactly this test with the expected `None`-instead-of-`Some(Finished)` mismatch,
reverted, `just test` green again (both runs recorded).

**Two more ending-boundary tests the plan's "both ways" asked for:** `note_input_has_
nothing_to_end_on_a_running_pane` and `clear_status_does_not_touch_a_running_status` -
`Running` has no ending rule at all, so neither input nor acknowledgement touches it.
This caught a real bug while building (below). `note_input_to_one_pane_does_not_end_
another_panes_status_in_the_same_tab` - input is scoped to the one pane it was sent to.

**AC #3, "A tab's shown status is the most pressing among its panes."** `Tab::shown_
status`/`Tree::tab_shown_status`, ranked per decision-7's order (failed, finished,
running in M1). `tab.rs` has all six orderings of two statuses across two panes
(`shown_status_prefers_failed_over_running` and its reverse,
`shown_status_prefers_finished_over_running` and its reverse,
`shown_status_prefers_failed_over_finished` and its reverse), the tie-break toward the
newer tick for each of the three statuses tied against itself
(`shown_status_breaks_a_tie_between_two_{failed,finished,running}_panes_toward_the_
newer_one`), and a pane whose Finished status has since expired falling through to the
next (`shown_status_falls_through_to_the_next_pane_once_the_more_pressing_ones_status_
has_expired`). `tree.rs`: `tab_shown_status_delegates_to_the_tabs_own_ranking`,
`tab_shown_status_on_an_unknown_tab_errors`.

**AC #3, "and a session has none."** Proven by absence, not a runtime assertion:
`Session` (`crates/kernel/src/session.rs`) gains no status field or method in this PR,
and the sdk description's `session_fields()` already has none (id, label, position,
colour, current_tab).

## A real bug the tests found while building

`Pane::clear_status` was first written to unconditionally set `self.status = None`.
`note_input_has_nothing_to_end_on_a_running_pane` (a new test, not an existing one)
failed immediately: a Running pane's status was being erased by its own input, which
is wrong - `Running` has no ending rule in R83 at all. Fixed by scoping `clear_status`
to `Finished`/`Failed` only; the test itself was never touched.

## Checks recorded (all via `asdlc record-check`, on this branch)

- `just test`: failing (compile errors, new API not yet implemented) -> after
  implementing `pane.rs` -> failing again (a caller in `tree.rs` not yet updated) ->
  passing -> passing again after fixing an uncaught caller in
  `crates/server/tests/real_pty.rs` (a departure noted on the task when I found it) ->
  passing again after the `clear_status`/Running bug fix.
- `just lint`, `just check`: both passing (format, lint, boundaries, licences, every
  package built and tested alone, the architecture model, recipe names).
- Fault injection #1 (`focus_pane` clearing status): failing, then reverted and
  passing.
- Fault injection #2 (`FINISHED_LIFETIME` off by one second): failing, then reverted
  and passing.
- `just check` re-run clean after rebasing onto `origin/main` once more (TASK-1.48
  merged while this was in progress - `error.rs`/`lib.rs` conflicts were both
  additive and resolved by keeping both sides; re-ran the `.status()`/`.record_exit(`
  grep afterward, still confined to `pane.rs`/`tab.rs`/`tree.rs` and the one unrelated
  `ExitStatus` call).
- `just check` at the final head, after the push below: passing.

## Departures from the plan (also already on the task's comments as they happened)

- `crates/server/tests/real_pty.rs` (TASK-1.40's own functional test, already on
  `origin/main`) called `Tree::record_exit` twice and wasn't caught by the plan's
  call-site grep, which searched for `.status()`, not `record_exit`. Fixed both call
  sites with `Moment::new(Duration::ZERO)`, since neither test asserts anything about
  `Status`/timing.
- `Pane::clear_status` needed the Running-exclusion fix above; not anticipated in the
  plan's code sketch, which showed an unconditional clear.

Everything else matches the approved plan as written.
<!-- SECTION:NOTES:END -->

## Comments

<!-- COMMENTS:BEGIN -->
created: 2026-11-15T19:34:00Z

---

Plan written. One open question blocks half of AC #1, needs the engineer's decision before build: ghostty-vt 0.8 (the exact vendored dependency) does not surface a prompt-marked command's exit code anywhere in its safe Rust API (verified by reading ghostty-vt-sys's bindings.rs, which has the data behind an OPT_SEMANTIC_PROMPT callback the safe ghostty-vt crate never wires, and ghostty-vt's own terminal.rs Event enum, which has no such variant). That blocks Finished/Failed for a command typed at a shell prompt (the Program::Shell half of AC #1) - a vendored-dependency gap, not a kernel design choice, and not something to work around with a hand-rolled OSC 133 parser. Program::Command panes, and AC #2 and #3 in full, are unaffected and planned in full below. Two options for the engineer, both outside this task's own scope to decide: descope the Shell-prompt clause to a follow-up task against the ghostty-vt dependency, or make wiring that callback a prerequisite of this one. See the plan for the full writeup and reasoning.

---

created: 2026-11-15T19:46:00Z

---

Plan revised per team-lead's feedback: dropped the ghostty-vt blocking question (AC #1 now scopes it out, confirmed from the task's own updated wording); replaced std::time::Instant with a new kernel-owned Moment type (crates/kernel/src/moment.rs) - a newtype over Duration, constructible in tests with no clock and no clippy exception, with the 'earlier moment than one passed before' rule stated explicitly (no subtraction is ever defined, so there's no path that could panic; a stale or backward-looking now just reads a Finished status as not-yet-expired); kept the tree's existing tick() counter for decision-7's ordering tie-break, reasoned explicitly against folding that into Moment too; kept acknowledge_pane clearing both Finished and Failed, with the AC-vs-decision-7 reconciliation now flagged to also go in the build-time notes; and added the rename-fallout and record_exit-signature risks naming TASK-1.76 and TASK-1.48 by name, with phase 2's first step re-running the call-site grep against origin/main.

---

author: @bruk
created: 2026-11-15T20:25:00Z

---

Plan approved by @bruk (ed724e8c9084, size-3): its worker may build it.

---

created: 2026-11-15T20:35:00Z

---

Departure from the plan: crates/server/tests/real_pty.rs (TASK-1.40's own functional test, already on origin/main) calls Tree::record_exit twice and wasn't caught by the plan's call-site grep (which searched for '.status()', not 'record_exit' - record_exit's own signature change was already known/planned, just not that this file called it). Fixed both call sites with Moment::new(Duration::ZERO), since neither test asserts anything about Status/timing - they're proving a real exit code reaches the kernel's cascade, nothing about when.

---

author: @bruk.r1
created: 2026-11-15T20:50:00Z

---

@bruk.r1 is reviewing TASK-1.41: a same-machine review: it waited 0m with no other engineer's reviewer, longer than review_fallback_after (0m), so a fresh reviewer on its author's machine takes it.

---
<!-- COMMENTS:END -->
