---
id: TASK-1.83
title: 'Kernel: running, finished and failed for a command typed at a shell prompt'
status: In Progress
assignee:
  - '@bruk.w1'
created_date: '2026-11-15T21:43:00Z'
updated_date: '2026-11-15T23:52:00Z'
labels:
  - size-3
milestone: m-4
dependencies:
  - TASK-1.79
parent_task_id: TASK-1
project: bruk-io-steg
asdlc_accepted_from: DRAFT-2
asdlc_branch: task-1.83-kernel-running-finished-and-failed-for-a
asdlc_plan_approved_by: bruk
asdlc_plan_approved_hash: 'ca2675da38c4'
asdlc_plan_approved_size: size-3
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
## Revision note (this plan replaces the previous one)

The engineer settled section 4's open decision against my recommendation: a
shell pane is idle, with no status at all, from the moment its shell starts
until a prompt-marked command's own start mark arrives; after that command's
`Finished`/`Failed` status ends, the pane goes back to idle, not back to
`Running`. This changes one line of behaviour TASK-1.41 shipped
(`start_program` set `Running` for every pane, shell or command). Everything
else below is unchanged from before - the new `StatusKind::CommandRunning`
variant, `note_command_output_started`/`note_command_ended`, the return
enums, and the shared `Finished`/`Failed`/10-minute machinery - because none
of it assumed a shell pane's baseline was `Running`; it only assumed
`record_exit`/`restart` (for command panes) keep working, which they do,
untouched.

## Scope

Still one reviewable change, `crates/kernel/src/pane.rs` and
`crates/kernel/src/tree.rs` (plus the `lib.rs` re-export line). No sdk
change, no terminal.rs change.

## 1. The rule, and the one place it changes

**The rule.** A pane's status (doc-11/decision-7's `Status`, not
`ProgramStatus`) answers "what does steg know about this pane's foreground
*activity* right now", and for a shell that is nothing until a typed
command is known to be running. Concretely:

- A command pane (`Program::Command`): unchanged from TASK-1.41 - `Running`
  from `start_program`/`restart` until its program exits, then `Finished`/
  `Failed` from the exit code.
- A shell pane (`Program::Shell`): no status at all from `start_program`
  (idle at its own prompt, R97's "the shell program is running" fact still
  lives in `program_status`, just not in `status` any more). `Running` only
  between a command's start mark (`note_command_output_started`) and its end
  mark (`note_command_ended`). After that command's `Finished`/`Failed`
  status itself ends (input, acknowledgement, or the 10-minute timer), the
  pane is idle again - not back to `Running` - until the next command's own
  start mark.

**The one place it changes: `Pane::start_program`.** Today it
unconditionally sets `self.status = Some(StatusRecord { kind: StatusKind::
Running, tick })` for every pane. It now does that only for a command pane;
for a shell pane it leaves `self.status` as `None`:

```rust
pub(crate) fn start_program(&mut self, program: Program, tick: u64) -> Result<(), Error> {
    if self.program.is_some() {
        return Err(Error::ProgramAlreadyStarted(self.id));
    }
    self.program = Some(program);
    self.program_status = Some(ProgramStatus::Running);
    self.status = if self.closes_on_exit() {
        None // a shell pane is idle at its prompt; no status until a
             // typed command's own start mark (doc-11/decision-7).
    } else {
        Some(StatusRecord { kind: StatusKind::Running, tick })
    };
    Ok(())
}
```

`self.closes_on_exit()` reads `self.program`, which the line above just set,
so the order is load-bearing (program first, then the branch) - the same
"set program, then decide from it" shape `record_exit` already has for
`closes_on_exit()`'s other use.

`program_status` is untouched: it is still set to `Running` unconditionally,
for every pane, shell or command, exactly as before - `ProgramStatus` is the
one clock-free fact about the program itself, which *is* true the instant a
shell starts (the shell process is running), and nothing about this task
changes what that field means or when it is set. Only `status` (doc-11's
richer thing) stops being set for a shell pane.

`Pane::restart` is unaffected and needs no change: it is only ever reachable
for a command pane (`Pane::restart`'s own first guard,
`Error::ShellCannotRestart`, refuses it for a shell before any state is
touched), so its unconditional `self.status = Some(StatusRecord { kind:
StatusKind::Running, tick })` stays correct exactly as written - a restarted
command pane still shows `Running` immediately, same as today.

`Pane::record_exit` is also unaffected: it still unconditionally overwrites
`self.status` from the exit code (`status_kind_for_exit_code`, section 2),
whatever it held before - `None` (an idle shell, the new baseline),
`CommandRunning` (a typed command was producing output when the shell
itself died) or anything else. Row 8 of the state table (section 3) is
exactly this case and needed no change for this revision.

**Existing tests checked, not assumed.** I read every `shell(` / `Program::
Shell` construction site in `crates/kernel/src/pane.rs` and `crates/kernel/
src/tree.rs` on `origin/main` (seven call sites total: `pane.rs` lines 371,
404, 451; `tree.rs` lines 1680, 1709, 1766, plus the `shell()`/`Program::
Shell` helper definitions themselves) and the one in `crates/server/tests/
real_pty.rs` (line ~238). **None of them read `Pane::status()`/`Tree::tab_
shown_status`/anything doc-11-shaped between `start_program(shell(...))` and
the next call.** Each one does exactly one of:
- immediately calls `record_exit` and asserts only on the resulting
  `ExitOutcome`/`closes_on_exit()` (`a_shell_pane_closes_on_exit`,
  `a_shell_panes_exit_closes_it_through_the_same_cascade_as_a_manual_close`,
  `a_shell_panes_exit_cascades_no_further_than_it_should_among_several_panes`,
  `a_shell_pane_is_gone_after_its_exit_so_restarting_it_finds_no_pane_at_all`,
  and `real_pty.rs`'s own shell-exit test), or
- calls `restart` and asserts only `Error::ShellCannotRestart`
  (`restarting_a_shell_pane_errors`), or
- passes `shell()` as a *second*, rejected `start_program` call and asserts
  only `Error::ProgramAlreadyStarted` plus that `pane.program()` still
  reports the *first* program
  (`starting_a_program_twice_errors_and_keeps_the_first`, where the first
  program is `command()`, so `start_program(shell(), 1)` here never even
  succeeds).

So **no existing test in TASK-1.40's or TASK-1.41's own PRs needs an updated
expectation** - there is no test anywhere in the kernel crate, as of
`origin/main`, that starts a shell pane and then reads its `status` without
an intervening real exit. This is a stronger, checked answer than a list of
tests to fix: there is nothing on that list.

**`crates/server`, including TASK-1.76's `Runtime` (merged into `origin/
main` as of this writing, commit range `fdef2fe`/`6065b44`): also nothing to
update, checked by grep, not assumed.** `git grep -n '\.status\(\|steg_sdk::
Status\|Status::Running\|Status::Finished\|Status::Failed' -- crates/
server` on `origin/main` finds exactly two `.status()` hits
(`crates/server/src/pane.rs:606`, `crates/server/src/runtime.rs:1491`), both
`std::process::Command::status()` → `ExitStatus`, unrelated to this crate's
`Pane`, the same false hit TASK-1.41's own notes already named once - and
**zero** hits for `steg_sdk::Status`/`Pane::status()` anywhere in
`crates/server`. `pane.show`'s wire "status" field (`crates/server/src/
dispatch/pane.rs:116-122`, and the `runtime.rs` tests that assert
`record["status"] == json!("running")`/`json!("ended")`) is built from
`ProgramStatus` (`Running` → `"running"`, `Ended{..}` → `"ended"`), a
different, already-existing fact this task's `status`/`Status` doesn't
touch and that this task's change to `start_program` leaves exactly as it
was (`program_status` is unconditional, as stated above). The server has no
wire representation of doc-11's `Status` at all yet (TASK-1.41's own plan
left that to a later `PaneRecord`/structured-field task); until TASK-1.84
wires `note_command_output_started`/`note_command_ended` through, nothing in
`crates/server` reads the thing this task changes, so nothing there needs
updating by this task, now or once TASK-1.76 is accepted.

## 2. The operations (unchanged from the previous plan)

A "shell pane" is exactly `pane.closes_on_exit()`. A command pane's status
comes only from its own exit code; a mark arriving there (a command pane's
own program printing OSC 133 itself) is ignored, not an error - the
program's own status always wins.

**`Pane` (`crates/kernel/src/pane.rs`, beside `record_exit`):**

```rust
pub(crate) fn note_command_output_started(&mut self, tick: u64) {
    if self.closes_on_exit() {
        self.status = Some(StatusRecord { kind: StatusKind::CommandRunning, tick });
    }
}

pub(crate) fn note_command_ended(&mut self, exit_code: Option<i32>, now: Moment, tick: u64) {
    if !self.closes_on_exit() {
        return;
    }
    if !matches!(self.status, Some(StatusRecord { kind: StatusKind::CommandRunning, .. })) {
        return;
    }
    let Some(exit_code) = exit_code else {
        self.status = None; // nothing guessed; back to idle (section 1).
        return;
    };
    self.status = Some(StatusRecord { kind: status_kind_for_exit_code(exit_code, now), tick });
}
```

`status_kind_for_exit_code(exit_code: i32, now: Moment) -> StatusKind` is a
new private free function, the one thing `record_exit` and `note_command_
ended` share (0 → `Finished { since: now }`, else → `Failed { exit_code }`).
Nothing else about `record_exit` changes.

**`Tree` (`crates/kernel/src/tree.rs`, beside `record_exit`):**

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

(`CommandEnded::classify` is sketched, not pinned; `Tree` must read the
outcome from the pane's state *before* `Pane::note_command_ended` mutates
it, whichever of `Tree`/`Pane` ends up owning that read - see Risks.)

Return shapes, exported from `lib.rs` beside `ExitOutcome`:

```rust
pub enum CommandOutputStarted {
    /// The pane's status is now `Status::Running`, for this typed command.
    Started,
    /// Nothing changed: not a shell pane - its own program's exit code
    /// governs its status, never a mark.
    NotAShellPane,
}

pub enum CommandEnded {
    /// The pane's status is now this.
    Ended(Status),
    /// The shell reported no exit code: the status is cleared - back to
    /// idle, nothing guessed (R83).
    EndedWithNoExitCode,
    /// Nothing changed: not a shell pane, or there was no matching
    /// `note_command_output_started` first.
    Ignored,
}
```

Names on all four (two methods, two enums) stay open to the engineer's
preference, as before - `note_command_output_started`/`note_command_ended`
are TASK-1.79's own proposal, kept as given.

## 3. One new `StatusKind` variant, not a new field (unchanged)

```rust
enum StatusKind {
    Running,
    CommandRunning, // new
    Finished { since: Moment },
    Failed { exit_code: i32 },
}
```

`status_for_ranking`'s exhaustive match gains `StatusKind::Running |
StatusKind::CommandRunning => Status::Running` (the forcing function:
missing this fails the build). `clear_status`'s `matches!` (`Finished |
Failed` only) and `next_status_deadline`'s `let ... else` both already treat
anything that isn't `Finished`/their target as "leave it alone"/"no
deadline" - `CommandRunning` falls through correctly, uncaught by the
compiler, so each gets its own test.

## 4. The state table (revised: new rows for the idle baseline)

"Shell pane" = `pane.closes_on_exit()`; "command pane" = the negation.
Rows 1-2 are new in this revision; the rest are unchanged from before, with
descriptions adjusted where the baseline matters.

| # | Starting state | Event(s) | Resulting status | Why |
|---|---|---|---|---|
| 1 | Shell pane just started | (none) | **no status at all** | section 1's rule; `start_program` sets `None` for a shell |
| 2 | Shell pane, a typed command's `Finished`/`Failed` status has just ended (input, acknowledgement, or the 10-minute timer) | (none further) | **no status at all**, not back to `Running` | `clear_status`/expiry already produce `None`, never a reversion to `Running` - this revision's baseline makes that the *only* idle state, where before it would have been masked by `start_program`'s old `Running` default |
| 3 | No status (row 1 or 2) | start mark | `CommandRunning` → reads as `Running` | `note_command_output_started` |
| 4 | `CommandRunning` | end, code 0 | `Finished` | `status_kind_for_exit_code(0, now)` |
| 5 | `CommandRunning` | end, nonzero code | `Failed { exit_code }` | same, else branch |
| 6 | `CommandRunning` | end, no code | `None` - cleared, nothing shown, no timer | R83 never guesses |
| 7 | No status, or `Finished`/`Failed` | end, with or without a code | ignored, unchanged | an end with no matching start proves nothing; `CommandEnded::Ignored` |
| 8 | `CommandRunning` | a second start | stays `CommandRunning`, tick refreshed | TASK-1.79's engine doesn't dedupe repeated `C`s either |
| 9 | `CommandRunning` | the shell's own program exits (`record_exit`, a real exit code, not a mark) | status becomes `Finished`/`Failed` from the shell's own exit, then the pane closes (R97) | `record_exit` unconditionally overwrites `self.status`, unaffected by this revision |
| 10 | command pane (`Program::Command`), any status | a mark arrives | ignored, unchanged | `!pane.closes_on_exit()` guard |
| 11 *(tab-level)* | A tab holding only idle shell panes (row 1/2) | — | `Tab::shown_status` is `None` | every pane's `status_for_ranking` is `None`; `max_by_key` over nothing is `None` - "a tab has none" |
| 12 *(tab-level)* | A tab with one idle shell pane and one `Running` command pane | — | `Tab::shown_status` names the command pane | the idle shell contributes nothing to the ranking; only the command pane has a status to rank |

Rows 1, 2, 10 ("unchanged"/"nothing") must be proven by capturing `program_
status`, `status(now)` *and* `next_status_deadline(now)` before and after,
per the testing skill's rule for a test whose name says nothing changes.

## 5. Ranking and `next_status_deadline` need nothing else; the sdk's field prose stays true

**Ranking (`Tab::shown_status`/`rank`).** No change beyond section 3's one
new match arm. A pane with no status (idle shell, rows 1/2) is already
excluded from `shown_status`'s ranking today - `status_for_ranking` returns
`None` whenever `self.status` is `None`, `filter_map` drops it, and
`max_by_key` over whatever remains is unaffected by how many panes were
dropped. Rows 11/12 above are the direct tests that make this a checked
fact rather than an inference: an idle shell pane ranks exactly as "no
status" already does for any pane (a command pane that has not started yet,
or one whose `Finished` has expired) - nothing new to build.

**`next_status_deadline`.** Same reasoning: `Pane::next_status_deadline`
already starts with `let StatusKind::Finished { since } = self.status?.kind
else { return None }` - `self.status?` returns `None` immediately when
there is no status at all, so an idle shell pane was already excluded, the
same way a `Running` or freshly-created pane already is. No change, and no
new test beyond what rows 11/12 already prove at the tab level (one more
`Tree::next_status_deadline` test, with an idle shell pane and a `Finished`
command pane in the same tree, confirming the idle pane contributes nothing
to the minimum - mechanical, but worth having since this method walks every
pane in the tree).

**The sdk's field prose.** `crates/sdk/src/description/methods.rs`'s `pane_
fields()` already describes `status` as "What steg knows about its program
right now (doc-11), **if anything**" - that "if anything" already allows no
status, so this sentence stays true with no wording change; a shell pane
idling at its prompt is simply one more case the existing "if anything"
covers; it does not need a case naming shells specifically.

Noted as an aside, not a change this task makes: `crates/server/src/
dispatch/pane.rs`'s actual `pane.show` wire implementation currently builds
its JSON `"status"` field from `ProgramStatus` ("running"/"ended"), not from
`steg_sdk::Status` at all (confirmed by grep, section 1) - so today's wire
output doesn't yet reflect doc-11's richer `Status`, this task's change
included, for any pane. That gap belongs to TASK-1.84's wiring (or whatever
task gives `status` its own structured wire shape), not to this one.

## 6. "Nothing is guessed" - a shell with no marks at all (revised)

A shell pane that never receives a single `note_command_output_started`/
`note_command_ended` call now has **no status at all**, from the moment it
starts, for as long as no mark ever arrives - not `Running` (the previous
revision's answer), since `start_program` no longer gives it one. Tested
directly: a shell pane given no marks at all stays with `status(now) ==
None` at `Moment(0)` and at a `Moment` very far in the future (a year),
and `next_status_deadline` stays `None` throughout - proving no implicit
timer, guess, or default status ever appears without a mark.

## 7. The sdk needs nothing new (unchanged)

`steg_sdk::Status` is untouched - no new variant, no wire change.
`CommandRunning` maps onto the existing `Status::Running`; a typed command's
`Finished`/`Failed` are the exact same values TASK-1.41 already defined for
a command pane's exit. No I/O, no clock read, no `unsafe` anywhere in this
task.

## 8. Left to TASK-1.84 and TASK-1.85 (unchanged)

Nothing in this task calls `Terminal::take_command_marks`. The loop TASK-1.84
runs:

```rust
let now: Moment = /* the server's own clock, converted once per drain */;
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

This revision changes nothing about that loop: `start_program` is called
once, at pane creation, well before any terminal write could produce a
mark, so TASK-1.84's loop never has to reason about the idle-baseline rule -
it only ever calls the two methods above, which already encode it.

## 9. Proof

- **Clause by clause**, against the table in section 4 (rows renumbered):
  AC #1's "shows running from when a prompt-marked command starts... while
  the shell's own program keeps running" is rows 1, 3-6, 9 plus a test that
  `program_status` stays `Running` throughout rows 1-9 regardless of what
  `status` does (the shell process itself never looks like it stopped);
  "a command pane's status from its program's exit is unchanged" is row 10.
  AC #2: the 599/600/601-second boundary re-proven through `note_command_
  ended` (as before); `note_input`/`acknowledge_pane` ending a typed
  command's `Finished`/`Failed` (as before); `note_input` not ending a
  running typed command (as before); "opening a tab ends nothing" extended
  with an idle shell pane in the mix too (nothing to clear, so trivially
  unaffected, but tested). AC #3: rows 1, 2, 6, 7, 8 and section 6, each its
  own test, as asked.
- **Rows 11/12, by direct tab-level test**, not just "by construction" -
  section 5's reasoning turned into `tab.rs` tests.
- **Negative control** on the main new mechanism (`note_command_ended`'s
  `CommandRunning` guard): unchanged from before - drop the guard, confirm
  `note_command_ended_with_no_preceding_start_is_ignored` and the
  two-ends-in-a-row test both fail, restore, confirm green again.
- **A second, small negative control for this revision specifically:**
  temporarily revert `start_program`'s branch to always set `Running`
  (today's code); confirm the new row-1 test
  (`a_freshly_started_shell_pane_has_no_status`) fails with `Some(Running)`
  where it expects `None`; restore; confirm green again. This is the direct
  proof that the idle baseline is this task's own code, not an accident of
  some other guard.
- **Existing tests that must pass untouched, named:** every test listed in
  section 1's "existing tests checked" paragraph, by name, plus everything
  already named in the previous plan (the 599/600/601 trio,
  `note_input_has_nothing_to_end_on_a_running_pane`,
  `note_input_ends_a_finished_status_on_the_pane_it_was_sent_to` (+failed),
  `acknowledge_pane_ends_a_finished_status` (+failed),
  `opening_a_tab_or_focusing_a_pane_ends_nothing`,
  `tab_shown_status_delegates_to_the_tabs_own_ranking`, every `shown_status_
  prefers_*`/`shown_status_breaks_a_tie_*` test, and every `next_status_
  deadline_*` test).
- **`just test`, test-first**, then `just check` recorded at an intermediate
  and the final head, as before.

## 10. Files (unchanged)

- `crates/kernel/src/pane.rs`: `start_program`'s branch (section 1);
  `StatusKind::CommandRunning`; `status_for_ranking`'s match; `status_kind_
  for_exit_code`; `note_command_output_started`/`note_command_ended`; new
  tests.
- `crates/kernel/src/tree.rs`: `note_command_output_started`/`note_command_
  ended`; `CommandOutputStarted`/`CommandEnded`; new tests.
- `crates/kernel/src/lib.rs`: two new `pub use tree::{...}` entries.

No other file changes; no sdk change; no `boundaries.toml`/`deny.toml`/
`clippy.toml` edit expected.

## Risks

- **Naming**, as before: four new names stay open.
- **`Tree::note_command_ended`'s outcome read-before-mutate** (section 2),
  as before.
- **This revision's blast radius was verified, not assumed**, by reading
  every shell-pane test site in the kernel crate and grepping the server
  crate for any read of `Pane::status()`/`steg_sdk::Status` - if a task
  lands on `origin/main` between now and build time that adds a new such
  read, phase 2's first step (bringing in `origin/main`) re-runs both
  checks before relying on "nothing to update" again.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Revised plan checked against doc-11 (R50, R82, R83), decision-7, doc-15 and doc-16 by @bruk.o: ok. The engineer decided on 2026-11-15 that a shell waiting at a prompt is idle and shows nothing: a shell pane has no status from start, running only between a typed command's start and end marks, nothing again once its finished or failed status ends; a command pane is unchanged. The plan makes that one branch in start_program, keeps program_status as it is, adds the two operations beside record_exit with a state table of one test per row, and reuses TASK-1.41's status record, ten-minute rule, ending rules and ranking with no second copy. The worker checked rather than listed: no existing test reads a shell pane's status between its start and its next call, and nothing in the server reads the display status yet. Kernel only: no I/O, no clock, no unsafe, no sdk change.

Check passed: `just test` exited 0 at 2026-11-15T23:39:33+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.83 on task-1.83-kernel-running-finished-and-failed-for-a at 7cf881a0404e:

```text
skip    tests: this change touches no crate
```

Check passed: `just test` exited 0 at 2026-11-15T23:50:47+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.83 on task-1.83-kernel-running-finished-and-failed-for-a at 7cf881a0404e with uncommitted changes:

```text
ok      tests (-p steg -p steg-kernel -p steg-server) (4.0s)
```

Check FAILED: `just test` exited 101 at 2026-11-15T23:51:58+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.83 on task-1.83-kernel-running-finished-and-failed-for-a at 7cf881a0404e with uncommitted changes, last 40 of 64 lines:

```text
error[E0599]: no method named `note_command_ended` found for struct `pane::Pane` in the current scope
   --> crates/kernel/src/pane.rs:882:14
    |
 77 | pub struct Pane {
    | --------------- method `note_command_ended` not found for this struct
...
882 |         pane.note_command_ended(Some(0), moment(0), 1);
    |              ^^^^^^^^^^^^^^^^^^ method not found in `pane::Pane`

error[E0599]: no method named `note_command_ended` found for struct `pane::Pane` in the current scope
   --> crates/kernel/src/pane.rs:891:14
    |
 77 | pub struct Pane {
    | --------------- method `note_command_ended` not found for this struct
...
891 |         pane.note_command_ended(Some(0), moment(0), 1);
    |              ^^^^^^^^^^^^^^^^^^ method not found in `pane::Pane`

error[E0599]: no method named `note_command_ended` found for struct `pane::Pane` in the current scope
   --> crates/kernel/src/pane.rs:900:14
    |
 77 | pub struct Pane {
    | --------------- method `note_command_ended` not found for this struct
...
900 |         pane.note_command_ended(Some(3), moment(0), 1);
    |              ^^^^^^^^^^^^^^^^^^ method not found in `pane::Pane`

error[E0599]: no method named `note_command_output_started` found for mutable reference `&mut pane::Pane` in the current scope
   --> crates/kernel/src/tab.rs:747:14
    |
745 | /         tab.pane_mut(pane)
746 | |             .unwrap()
747 | |             .note_command_output_started(tick);
    | |             -^^^^^^^^^^^^^^^^^^^^^^^^^^^ method not found in `&mut pane::Pane`
    | |_____________|
    |

error: could not compile `steg-kernel` (lib test) due to 22 previous errors
full output: .quiet/tests-p-steg-p-steg-kernel-p-steg-server.log
error: recipe `test` failed with exit code 101
```

Check passed: `just test` exited 0 at 2026-11-15T23:52:17+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.83 on task-1.83-kernel-running-finished-and-failed-for-a at 7cf881a0404e with uncommitted changes:

```text
ok      tests (-p steg -p steg-kernel -p steg-server) (6.9s)
```

Check FAILED: `just test` exited 101 at 2026-11-15T23:52:36+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.83 on task-1.83-kernel-running-finished-and-failed-for-a at 7cf881a0404e with uncommitted changes, last 40 of 64 lines:

```text
test tree::tests::view_pane_is_idempotent_and_rejects_an_unknown_pane ... ok
test tree::tests::viewing_alone_never_makes_a_client_latest ... ok
test tree::tests::viewing_follows_from_attachment_not_a_separate_setter ... ok
test terminal::tests::the_line_limit_governs_even_past_the_engines_disabled_byte_limit ... ok

failures:

---- pane::tests::note_command_ended_with_no_preceding_start_is_ignored stdout ----

thread 'pane::tests::note_command_ended_with_no_preceding_start_is_ignored' (15553594) panicked at crates/kernel/src/pane.rs:924:9:
assertion `left == right` failed
  left: Ended(Failed { exit_code: 5 })
 right: Ignored

---- pane::tests::note_command_ended_twice_in_a_row_the_second_is_ignored stdout ----

thread 'pane::tests::note_command_ended_twice_in_a_row_the_second_is_ignored' (15553590) panicked at crates/kernel/src/pane.rs:936:9:
assertion `left == right` failed
  left: Ended(Failed { exit_code: 7 })
 right: Ignored
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace

---- tree::tests::note_command_ended_through_the_tree_with_no_preceding_start_is_ignored stdout ----

thread 'tree::tests::note_command_ended_through_the_tree_with_no_preceding_start_is_ignored' (15553821) panicked at crates/kernel/src/tree.rs:1920:9:
assertion `left == right` failed
  left: Ended(Failed { exit_code: 5 })
 right: Ignored


failures:
    pane::tests::note_command_ended_twice_in_a_row_the_second_is_ignored
    pane::tests::note_command_ended_with_no_preceding_start_is_ignored
    tree::tests::note_command_ended_through_the_tree_with_no_preceding_start_is_ignored

test result: FAILED. 320 passed; 3 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s

error: test failed, to rerun pass `-p steg-kernel --lib`
full output: .quiet/tests-p-steg-p-steg-kernel-p-steg-server.log
error: recipe `test` failed with exit code 101
```
<!-- SECTION:NOTES:END -->

## Comments

<!-- COMMENTS:BEGIN -->
author: @bruk
created: 2026-11-15T23:38:00Z

---

Plan approved by @bruk (ca2675da38c4, size-3): its worker may build it.

---
<!-- COMMENTS:END -->
