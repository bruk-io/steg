---
id: TASK-1.83
title: 'Kernel: running, finished and failed for a command typed at a shell prompt'
status: In Progress
assignee:
  - '@bruk.w1'
created_date: '2026-11-15T21:43:00Z'
updated_date: '2026-11-15T23:25:00Z'
labels:
  - size-3
milestone: m-4
dependencies:
  - TASK-1.79
parent_task_id: TASK-1
project: bruk-io-steg
asdlc_accepted_from: DRAFT-2
asdlc_branch: task-1.83-kernel-running-finished-and-failed-for-a
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Piece two of the split of TASK-1.79 (2026-11-15). doc-11: R50, R82, R83; decision-7. The worker's plan on TASK-1.79 proposes new kernel operations (note_command_output_started, note_command_ended) factored beside record_exit, since a shell's own program never exits between commands. Read that plan and TASK-1.41's notes first. No I/O and no unsafe in the kernel.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A shell pane shows running from when a prompt-marked command starts producing output until it ends, then finished or failed from its exit code (R50, R82), while the shell's own program keeps running; a command pane's status from its program's exit is unchanged
- [ ] #2 The ending rules of TASK-1.41 apply unchanged to a status that arrives this way: finished ends after 10 minutes or on input, failed on input or when the person has dealt with it, opening a tab ends nothing; time arrives as a Moment
- [ ] #3 A shell that reports no prompt marks shows no finished or failed status for typed commands and nothing is guessed; a command end with no exit code, an end with no start, and a start followed by another start each have a stated result and a test
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
## Scope

One reviewable change, touching only `crates/kernel/src/pane.rs` and
`crates/kernel/src/tree.rs`. No sdk change (section 5 below says why), no
terminal.rs change (TASK-1.79 already built `CommandMark`/`take_command_marks`;
this task only takes the two facts those marks carry - "started" and "ended,
with or without a code" - as plain arguments to the new operations, never the
`CommandMark` type itself, so neither file gains a new dependency on
`crate::terminal`).

## 1. The operations, and what each returns

A "shell pane" is exactly `pane.closes_on_exit()` - true only for
`Program::Shell` (R97) - the distinction TASK-1.40 already built for "does
this pane close when its program ends", reused here unchanged rather than
inventing a second name for the same fact. A command pane's status comes only
from its own exit code (`Pane::record_exit`, R82); a mark arriving there (the
brief's case of a program that prints OSC 133 itself, e.g. an editor) is
answered once, in one place: **the program's own status always wins, the
mark is ignored.** This is not an error - a command pane's own program
printing OSC 133 is not a caller mistake - so both new `Tree` methods return
`Ok` with an outcome that says nothing happened, never an `Err`, for that
case.

**`Pane` (`crates/kernel/src/pane.rs`, beside `record_exit`):**

```rust
/// A typed command started producing output in this pane (OSC 133 `C`;
/// TASK-1.79's `CommandMark::OutputStarted`), stamped with `tick` the
/// same way `start_program`/`record_exit` are. A no-op on a command
/// pane (`!self.closes_on_exit()`): its status comes only from its own
/// exit code, never from a mark - a mark here is ignored, not an error.
pub(crate) fn note_command_output_started(&mut self, tick: u64) {
    if self.closes_on_exit() {
        self.status = Some(StatusRecord { kind: StatusKind::CommandRunning, tick });
    }
}

/// A typed command ended in this pane (OSC 133 `D`;
/// `CommandMark::Ended`), with `exit_code` exactly as the mark carried
/// it (`None` for a bare `D` or one the engine's own parser could not
/// read - TASK-1.79 already made those two report the same way).
/// A no-op on a command pane, same reason as above, and a no-op if
/// this pane's status is not currently `CommandRunning` - an end with
/// no matching start, or a second end in a row, is ignored rather than
/// guessed (R83).
pub(crate) fn note_command_ended(&mut self, exit_code: Option<i32>, now: Moment, tick: u64) {
    if !self.closes_on_exit() {
        return;
    }
    if !matches!(self.status, Some(StatusRecord { kind: StatusKind::CommandRunning, .. })) {
        return;
    }
    let Some(exit_code) = exit_code else {
        self.status = None; // nothing guessed; see section 3.
        return;
    };
    self.status = Some(StatusRecord { kind: status_kind_for_exit_code(exit_code, now), tick });
}
```

`status_kind_for_exit_code(exit_code: i32, now: Moment) -> StatusKind` is a
new private free function - the one piece of logic `record_exit` and
`note_command_ended` actually share (0 -> `Finished { since: now }`, else ->
`Failed { exit_code }`). Nothing else about `record_exit` changes: it still
sets `program_status` and still runs through `Tree::record_exit`'s R97
cascade exactly as today, bit for bit, with its own existing tests untouched.
Both the 10-minute rule and the tie-break stay single-sourced too: `status_
for_ranking`/`next_status_deadline` (`FINISHED_LIFETIME`) and `Tab::shown_
status` (`rank`) are not touched at all, because `StatusKind::CommandRunning`
maps to `Status::Running` in exactly the same arm `StatusKind::Running`
already does (section 1's new variant, below) - a typed command's `Finished`/
`Failed` is stored as the identical `StatusKind::Finished`/`Failed` a command
pane's exit produces, so every rule that already governs those two keeps
governing them with no new branch.

**`Tree` (`crates/kernel/src/tree.rs`, beside `record_exit`), mirroring its
signature shape (`tick()` taken internally, `PaneNotFound` on an unknown
pane):**

```rust
pub fn note_command_output_started(&mut self, id: PaneId) -> Result<CommandOutputStarted, Error> {
    let tick = self.tick();
    let pane = self.tab_owning_pane_mut(id).and_then(|t| t.pane_mut(id)).ok_or(Error::PaneNotFound(id))?;
    let outcome = if pane.closes_on_exit() {
        CommandOutputStarted::Started
    } else {
        CommandOutputStarted::NotAShellPane
    };
    pane.note_command_output_started(tick);
    Ok(outcome)
}

pub fn note_command_ended(&mut self, id: PaneId, exit_code: Option<i32>, now: Moment) -> Result<CommandEnded, Error> {
    let tick = self.tick();
    let pane = self.tab_owning_pane_mut(id).and_then(|t| t.pane_mut(id)).ok_or(Error::PaneNotFound(id))?;
    let outcome = CommandEnded::classify(pane, exit_code); // reads pane's state before mutating
    pane.note_command_ended(exit_code, now, tick);
    Ok(outcome)
}
```

(`CommandEnded::classify` is sketched, not pinned - the point is that the
outcome is computed from the pane's state *before* `Pane::note_command_ended`
runs, so `Tree` never duplicates `Pane`'s own guard logic; the engineer or I
may instead have `Pane::note_command_ended` return the outcome itself and
have `Tree` just pass it through - whichever reads cleaner once written, not
a design choice that changes behaviour.)

Return shapes, new to `crates/kernel/src/tree.rs`, exported from `lib.rs`
beside `ExitOutcome` the same way:

```rust
/// What `Tree::note_command_output_started` did (TASK-1.83; doc-11
/// R82/R83) - named so the server can decide whether to raise an event
/// with no further lookup.
pub enum CommandOutputStarted {
    /// The pane's status is now `Status::Running`, for this typed command.
    Started,
    /// Nothing changed: not a shell pane (R97) - its own program's exit
    /// code governs its status, never a mark.
    NotAShellPane,
}

/// What `Tree::note_command_ended` did.
pub enum CommandEnded {
    /// The pane's status is now this.
    Ended(Status),
    /// The shell reported no exit code: the status is cleared, not
    /// replaced with anything (R83: nothing is guessed).
    EndedWithNoExitCode,
    /// Nothing changed: not a shell pane, or there was no matching
    /// `note_command_output_started` first - an end with no start is
    /// ignored, never guessed.
    Ignored,
}
```

Names on all four (methods and enums) are open to the engineer's preference
at approval, the same way TASK-1.41 left `acknowledge_pane`'s name open -
nothing in the docs fixes a word for any of them; `note_command_output_
started`/`note_command_ended` are TASK-1.79's own proposal, named on this
task's description, and I'm keeping them as given.

## 2. One new `StatusKind` variant, not a new field

```rust
enum StatusKind {
    Running,
    CommandRunning, // new
    Finished { since: Moment },
    Failed { exit_code: i32 },
}
```

Not a separate `typed_command: Option<Moment>` field beside the existing
`status: Option<StatusRecord>` - that shape would make `(typed_command:
Some(_), status: None)` representable, exactly the "flag beside a field
that's only sometimes valid" `.claude/rules/rust.md` rules out. A variant
keeps "is a status currently CommandRunning" and "is there a status at all"
the same question, as it already is for `Running`.

Two places this forces a decision, both already correct by construction, but
worth stating since the compiler won't catch either:

- `status_for_ranking` (`pane.rs`'s one *exhaustive* match over `StatusKind`,
  no `_` arm) must add `StatusKind::CommandRunning => Status::Running` (or
  fold it into the existing `Running` arm, `StatusKind::Running |
  StatusKind::CommandRunning => Status::Running`) - this one **does** fail
  the build if missed, so it's the forcing function proving the mapping
  exists.
- `clear_status`'s `matches!` (`Finished | Failed` only) and `next_status_
  deadline`'s `let StatusKind::Finished { since } = ... else { None }` are
  both *non*-exhaustive - `CommandRunning` silently falls through either as
  "not cleared" or "no deadline", which is the behaviour wanted (section 3),
  but nothing forces it, so each gets its own test (section 3's table) since
  a silent fallthrough proves nothing on its own.

## 3. The state table

"Shell pane" = `pane.closes_on_exit()`; "command pane" = the negation. Every
row is a dedicated test; existing tests the row must not disturb are named
where relevant.

| # | Starting state | Event(s) | Resulting status | Why |
|---|---|---|---|---|
| 1 | Shell pane just started, no mark yet | (none) | `Running` - the shell program itself (R82/R97), unchanged from today | `start_program` already sets this; see section 4's open decision for whether this stays |
| 2 | `CommandRunning` | end, code 0 | `Finished` | `status_kind_for_exit_code(0, now)` |
| 3 | `CommandRunning` | end, nonzero code | `Failed { exit_code }` | same, else branch |
| 4 | `CommandRunning` | end, no code (bare `D` or a malformed one - TASK-1.79 made both report `None`) | `None` - cleared, nothing shown, no timer | R83 never guesses; we know the command ended but not how, so nothing stands in for "finished" or "failed" |
| 5 | `Running` or no status at all | end, with or without a code | ignored, unchanged | an end with no matching start proves nothing; `CommandEnded::Ignored` |
| 6 | `CommandRunning` | a second start | stays `CommandRunning`, tick refreshed | TASK-1.79's own engine doesn't dedupe repeated `C`s either ("harmless"); a fresh tick is correct since decision-7's tie-break is "newest", and this *is* newer activity |
| 7 | `Finished` or `Failed` (from a prior end) | another end | ignored, unchanged | the guard is "status is currently `CommandRunning`", which a completed end already cleared - `CommandEnded::Ignored` |
| 8 | `CommandRunning` | the shell's own program exits (`Tree::record_exit`, a real exit code, not a mark) | pane's status becomes `Finished`/`Failed` from the shell's own exit, then the pane closes (R97) | `record_exit` is untouched and still unconditionally overwrites `self.status` from `self.program_status`'s exit code, exactly as before this task; whatever `CommandRunning` state existed is moot once the pane is gone |
| 9 | command pane (`Program::Command`), any status | a mark arrives (`note_command_output_started`/`note_command_ended`) | ignored, unchanged | `!pane.closes_on_exit()` guard; `CommandOutputStarted::NotAShellPane`/`CommandEnded::Ignored`; the program's own `record_exit` is the only thing that ever sets a command pane's status |

Row 1 and row 9's "unchanged" must be proven by capturing `program_status`,
`status(now)` *and* `next_status_deadline(now)` before and after, not just
`status(now)` - the testing skill's rule for a test whose name says nothing
changes.

## 4. Open decision for the engineer

Row 1 assumes TASK-1.41's behaviour for a shell pane's baseline is kept:
`start_program` already sets `StatusKind::Running` for every pane, including
a plain shell, so a shell pane shows `Running` from launch, for as long as
the shell runs, whether or not any command is currently producing output.
AC #1's wording ("a shell pane shows running from when a prompt-marked
command starts producing output") could instead be read as redefining this -
showing nothing until the first mark arrives.

I'm recommending **no change to `start_program`**: the shell's own `Running`
is R82/R97's existing fact about the *shell program itself* being alive, not
an inference about a typed command, and nothing in AC #2 ("the ending rules
of TASK-1.41 apply unchanged") or decision-7 asks to take that away. The
practical difference is only in the gap before a shell's first prompt mark
ever arrives (at startup, and right after a command exits before the next
prompt reprints) - today's `Running` is shown then either way, under either
reading, since there is no third option between "no status" and the only
status a shell pane can have been given. If the engineer reads AC #1 as
requiring a shell pane to show *no* status until the first `C`, that is a
one-line change (`start_program` only sets a status for a command pane,
`None` for a shell) and row 1's test changes with it - flagging it here
rather than deciding it unilaterally.

## 5. "Nothing is guessed" - a shell with no marks at all

A shell pane that never receives a single `note_command_output_started`/
`note_command_ended` call (its shell never emits OSC 133, or TASK-1.84/1.85
haven't wired it yet) can only ever be `Running` (row 1) - `Finished` and
`Failed` are reachable on a shell pane *exclusively* through `note_command_
ended`'s two code-bearing arms, nothing else ever writes them for a shell
pane (the shell's own `record_exit`, row 8, is a different, real exit, and
closes the pane rather than leaving a typed-command status to read). Tested
directly: a shell pane given no marks at all stays `Running` at `Moment(0)`
and stays `Running` at a `Moment` very far in the future (a year, say) - not
`Finished`, not `Failed`, and `next_status_deadline` stays `None` throughout,
proving no implicit timer or guess ever starts without a mark.

## 6. The sdk needs nothing new

`steg_sdk::Status` is untouched. `CommandRunning` maps onto the existing
`Status::Running`; a typed command's `Finished`/`Failed` are the exact same
`Status::Finished`/`Status::Failed { exit_code }` TASK-1.41 already defined
for a command pane's exit - this task adds a second *path* to those same two
values, not new values. No wire shape changes, so `crates/sdk/src/status.rs`
and `crates/sdk/src/description/methods.rs` are not touched. No I/O, no
clock read, no `unsafe` anywhere in this task: both new `Pane`/`Tree` methods
are pure functions of their arguments and `self`, the same shape as
`start_program`/`record_exit`/`restart`, and this task touches no code
outside `crates/kernel/src/pane.rs` and `crates/kernel/src/tree.rs`.

## 7. Left to TASK-1.84 and TASK-1.85, with the exact calls written down

Nothing in this task calls `Terminal::take_command_marks` - that drain, and
the clock reading, belong to TASK-1.84 alone. The loop it runs, so that task
has nothing left to invent:

```rust
// Once per pane, per server tick (or whenever that pane's terminal was
// just written to): one clock reading, converted to one Moment, reused
// for every mark this drain produced - never read per mark.
let now: Moment = /* the server's own clock, converted once */;
for mark in terminal.take_command_marks() {
    match mark {
        CommandMark::OutputStarted => {
            tree.note_command_output_started(pane_id)?;
        }
        CommandMark::Ended { exit_code } => {
            tree.note_command_ended(pane_id, exit_code, now)?;
        }
    }
}
```

The mapping from `CommandMark`'s two variants to these two calls is exact and
total - nothing left to decide. Order matters and is preserved: `take_
command_marks` returns a `Vec` specifically because more than one mark can
arrive from a single terminal write (TASK-1.79's "two `C`s in a row" case),
and they must be applied in that order (a `C` then a `D` in one drain is a
start immediately followed by its own end, not the other way round) - the
`for` loop over the `Vec` in its existing order is what gives that, with no
sorting or buffering of its own. Whether the server raises an event from
`CommandOutputStarted`/`CommandEnded`'s returned variant, and what that event
looks like on the wire, is TASK-1.84's own design, not fixed here - only that
it has everything it needs from the return value to decide, with no further
read of the pane.

TASK-1.85 (the shell integration emitting the marks) needs nothing from this
task beyond the two `CommandMark` variants it already targets (TASK-1.79) -
nothing here changes that shape.

## 8. Proof

- **Clause by clause.**
  - AC #1: rows 1-4, 8 of section 3 (running until a mark starts output,
    then finished/failed from the exit code, shell kept running throughout -
    `closes_on_exit` is never touched by any of this); row 9 ("a command
    pane's status from its program's exit is unchanged") - a test that
    starts a command pane, sends it both kinds of mark, and asserts
    `program_status`/`status`/`next_status_deadline` are bit-for-bit what
    they were before, i.e. exactly what `record_exit` alone would have
    produced.
  - AC #2: the existing 599/600/601-second boundary tests
    (`a_finished_status_is_still_shown_599_seconds_after_it_began` and its
    600/601 siblings) are not touched; three parallel tests reach the same
    `FINISHED_LIFETIME` constant through `note_command_ended` instead of
    `record_exit`, at the same three literal moments, proving the single
    mechanism, not a second copy of the rule. `note_input`/`acknowledge_
    pane` ending a typed command's `Finished`/`Failed`: two new tests each,
    mirroring `note_input_ends_a_finished_status_on_the_pane_it_was_sent_to`
    /`_failed_...`/`acknowledge_pane_ends_a_finished_status`/`_failed_...`
    exactly, reached via `note_command_ended` first. `note_input` *not*
    ending a running typed command: a new test mirroring `note_input_has_
    nothing_to_end_on_a_running_pane`, with `CommandRunning` in place of
    `Running`. "Opening a tab ends nothing": true by the same construction
    as before (no code path from `focus_pane`/`focus_tab`/`focus_direction`
    touches `status`, and this task adds no such path) - the existing test
    `opening_a_tab_or_focusing_a_pane_ends_nothing` is extended with one more
    pane in `CommandRunning`, asserting it too is unchanged by every focus
    operation.
  - AC #3: row 4 (no code -> cleared, nothing guessed) and section 5 (no
    marks at all -> never finished or failed, ever) are each their own test;
    rows 5 and 6 (end with no start; two starts in a row) are each their own
    test, exactly as asked.
- **Ranking, by direct test, not just "by construction".** One new `tab.rs`-
  level test: a tab with one pane `CommandRunning` and another `Failed`
  ranks the `Failed` one, the same way `shown_status_prefers_failed_over_
  running` already proves for plain `Running` - because `rank()` operates on
  `Status`, never `StatusKind`, this is guaranteed to pass once section 1's
  mapping is in, and the test is what makes that a checked fact instead of
  an assertion in a doc comment.
- **Negative control, on the main new mechanism (the `CommandRunning` guard
  in `note_command_ended`).** Temporarily drop the
  `matches!(self.status, Some(StatusRecord { kind: StatusKind::CommandRunning, .. }))`
  guard (always proceed to set a status from `exit_code`); `just test` then
  fails exactly `note_command_ended_with_no_preceding_start_is_ignored` and
  `note_command_ended_twice_in_a_row_the_second_is_ignored` (both now wrongly
  set a status), recorded with `asdlc record-check`; restore the guard;
  `just test` green again, recorded again.
- **Existing tests that must pass untouched, named, as the proof they were
  not disturbed:** `starting_a_program_records_it_as_running`,
  `a_command_pane_stays_ended_and_does_not_close_on_exit`,
  `a_shell_pane_closes_on_exit`,
  `restarting_a_command_pane_resets_its_status_and_keeps_its_spec`,
  `a_finished_status_is_still_shown_599_seconds_after_it_began` (+600s/601s),
  `note_input_has_nothing_to_end_on_a_running_pane`,
  `note_input_ends_a_finished_status_on_the_pane_it_was_sent_to` (+failed),
  `acknowledge_pane_ends_a_finished_status` (+failed),
  `opening_a_tab_or_focusing_a_pane_ends_nothing`,
  `tab_shown_status_delegates_to_the_tabs_own_ranking`, and every
  `shown_status_prefers_*`/`shown_status_breaks_a_tie_*` test in `tab.rs`.
- **`just test`, test-first.** The new tests land before `CommandRunning`/
  `note_command_output_started`/`note_command_ended`/`CommandOutputStarted`/
  `CommandEnded` exist, recorded failing to compile; then recorded passing
  once built. `just check` recorded passing once at an intermediate head and
  once more at the final pushed head, the same shape TASK-1.41/1.79 both
  used.

## 9. Files

- `crates/kernel/src/pane.rs`: `StatusKind::CommandRunning`; `status_for_
  ranking`'s match gains it; `status_kind_for_exit_code` (new private
  function, factored out of `record_exit`); `note_command_output_started`,
  `note_command_ended` (new `pub(crate)` methods); new tests.
- `crates/kernel/src/tree.rs`: `note_command_output_started`,
  `note_command_ended` (new `pub` methods, mirroring `record_exit`'s shape);
  `CommandOutputStarted`, `CommandEnded` (new enums, beside `ExitOutcome`);
  new tests.
- `crates/kernel/src/lib.rs`: two new `pub use tree::{..., CommandOutputStarted, CommandEnded}` entries, alphabetised into the existing line.

No other file changes. No new dependency, no sdk change, no `boundaries.toml`/
`deny.toml`/`clippy.toml` edit expected - nothing here adds I/O, a new crate,
or anything those files would need to cover.

## Risks

- **The open decision in section 4.** If the engineer reads AC #1 the other
  way (a shell pane shows nothing until the first mark), `start_program`
  changes and row 1's test changes with it - a small, contained edit, but
  one that should be settled before building, not discovered mid-build.
- **Naming.** Four new names (two methods, two enums) are stated but open,
  as section 1 says; if the engineer prefers different ones this is a
  mechanical rename, not a design change.
- **`Tree::note_command_ended`'s outcome computed before mutation
  (section 1).** Whichever of `Tree` or `Pane` ends up owning that
  classification, it must read the *old* status, not the one just written -
  worth a specific test (`note_command_ended_with_no_preceding_start_is_
  ignored` already proves this for the "ignored" case; add one proving
  `Ended(Status::Finished)` is reported, not `Ignored`, for the ordinary
  case) so a refactor later can't silently flip the read order.
<!-- SECTION:PLAN:END -->
