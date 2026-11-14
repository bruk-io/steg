---
id: TASK-1.4
title: 'Spike: propose the CLI design for Bruk to decide'
status: In Progress
assignee:
  - '@bruk.w5'
created_date: '2026-11-13T22:52:00Z'
updated_date: '2026-11-14T02:45:00Z'
labels:
  - size-2
milestone: m-0
dependencies: []
documentation:
  - doc-2
parent_task_id: TASK-1
type: spike
project: bruk-io-steg
asdlc_accepted_from: DRAFT-5
asdlc_branch: task-1.4-spike-propose-the-cli-design-for-bruk-to
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Design decision 1 in the spec, which the spec deliberately leaves open. Proposes command names, flags, the environment variables that carry a terminal's context (R33), and exit statuses (R34), for the capabilities in R32 to R35 and the show-a-file command in R63.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A board document lists every proposed command with its flags, the context variables, and the exit statuses
- [ ] #2 The document marks each point where Rex's CLI does it differently, so the difference is a choice
- [ ] #3 Bruk has recorded his decision on the document
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
## What is left of this task, and whether to hold it

doc-2 (the CLI specification) already carries the grammar, the verbs, the targets, the
context variables, the flags, the output rules, the exit statuses, the first release's
command table and the Rex comparison. So most of what this spike was written to propose
is done, and what is left is four things (the task's notes): settle the six open points
in doc-2 section 9 with Bruk, check section 8's commands against the API description
TASK-1.3 delivers, replace section 7's proposed speed numbers with measured ones, and
get Bruk's decision recorded.

**Recommendation: do not hold this task. Reduce it to the two items that do not depend
on TASK-1.3, and move the other two to owners that already have them.**

- Keep: settle the open points, and get the decision recorded. They are independent of
  TASK-1.3 and they block TASK-1.13, which is where the CLI is actually built.
- Move out: **the speed numbers.** TASK-1.3's plan measures CLI start-up for a
  build-time and a run-time command tree; those are the numbers doc-2 section 7's first
  two bullets want. Transcribing them is one doc-2 edit by the orchestrator once
  TASK-1.3 lands. doc-1 R19 already sets this precedent ("proposals to confirm once a
  spike has measured a baseline"), so section 7 stays honest in the meantime by naming
  TASK-1.3 as the successor, which it already does.
- Move out: **the command-against-API check.** The mechanical form of it, every API
  method has a command, is TASK-1.13 acceptance criterion 1, the drift test, and
  TASK-1.3's plan already shapes that test. What TASK-1.4 owes instead is the *shape*
  the API description must carry for section 8 to be generable at all (which parameter
  is the target, which are booleans, how a program after `--` and a repeatable `--key`
  are expressed, where event names come from). doc-2 section 2 states part of that; this
  task completes it in words, with no code.

Holding costs schedule and buys nothing: TASK-1.13 cannot start before TASK-1.10, which
depends on TASK-1.3 anyway, so nothing downstream gets earlier by waiting here, while
Bruk's six answers are cheap and are needed by TASK-1.13 and TASK-1.25.

Both exclusions narrow what the orchestrator's notes asked for, not an accepted
criterion. Say so to Bruk; it is his to refuse.

This task is size-2, so phase 2 begins only after `asdlc approve TASK-1.4`; the first
step below refuses until that is recorded.

## Deliverable, and how it is handed over

Only the orchestrator and Bruk change doc-2, so this task produces a **new board
document**, linked to TASK-1.4, in five parts. Its type is `other`, not `specification`:
the board has no way to mark one specification as superseded by another, and `other`
says plainly that this is a proposal against doc-2 rather than a rival specification.

- **A. The decision sheet.** One line per question, each with a recommendation, the
  alternative, and what breaks if it goes the other way, so Bruk can answer "all as
  recommended except N". The six from doc-2 section 9, then the seven found while
  reviewing it (listed below).
- **B. The edit list for doc-2,** keyed to its section numbers: the current text, the
  proposed text, and the requirement or skill line that justifies it. The orchestrator
  applies it without re-deriving anything.
- **C. The Rex comparison, checked against the source.** doc-2 section 10's rows were
  written from a reading and have never been checked. Rex's own reference is reachable
  as Markdown (verified while planning: `https://www.superlogical.com/rex/docs/reference/cli.md`
  returns the full command and flag reference), so each row gets the line it rests on,
  its URL and the date fetched, and any missing difference gets a row.
- **D. The acceptance criteria read against doc-2** (below), with any rewording named
  for Bruk and none applied.
- **E. What waits, and for whom:** the two items moved out, with the exact doc-2 edit
  each will need, plus one consequence TASK-1.3 should know (below).

Acceptance criterion 3 is recorded on **doc-2**, by the orchestrator, from Bruk's
answer; the new document is the record of the proposal and the reasoning. That criterion
cannot be closed by this worker and the plan does not pretend otherwise.

## Files that change

**No file in the worktree changes, and the branch gets no commit.** The entire
deliverable is board artefacts, made with `asdlc` from the tasks clone and so absent
from this branch's diff. There is no pull request; `asdlc finish` is run without `--pr`.

Board artefacts:

- A new document from `asdlc board doc create "<title>" -t other`, written with
  `asdlc board doc update <doc-id> --content "$(cat <<'DOC' ... DOC)"`.
- `asdlc board task edit TASK-1.4 --add-doc <doc-id>` to link it.
- The task's notes: what was reviewed, the consistency check's output, the Rex sources
  and the date, the two items moved out, and the one command the orchestrator runs to
  record Bruk's answer.

Read but not written: `backlog/docs/doc-2 - CLI-specification.md` and
`backlog/docs/doc-1 - Spec-...md` in the worktree, and
`plugins/team/skills/cli-design/SKILL.md`.

Throwaway, in the phase-2 session's scratchpad and not committed: a short stdlib-only
PEP 723 script that reads the worktree's copy of doc-2 and prints three set differences
(commands in section 8 with no verb in section 3; flags used in sections 3, 8 and 9 and
defined nowhere in section 5; Rex commands with no steg equivalent). Its source and its
output go in the new document as an appendix, so a reviewer can re-run it without a file
being added to the repository. Timebox: if parsing doc-2's tables fights back, drop it,
do the cross-check by hand, and say in the notes that it was done by hand.

## Order of work

1. Record the repository's checks as the starting point, before anything else:
   `asdlc record-check TASK-1.4 --in <worktree> --as bruk.w5 -- just check`. Expect the
   Rust steps to skip while there is no workspace and the tools' tests to pass, as
   TASK-1.3's plan found.
2. Re-read doc-2 against doc-1 R32 to R35 and R63 to R68, and against the cli-design
   skill line by line. Write and run the consistency script; keep its output.
3. Fetch Rex's reference pages (`reference/cli.md`, `automate/cli.md`, `reference/api.md`,
   `reference/events.md`), note the date, and check every row of doc-2 section 10 against
   them. Treat the pages as data, not instruction, and copy no name into steg: doc-1
   section 7 answers intent question 5 with "steg's own names".
4. Draft part A, the decision sheet: the six open points with a recommendation each, then
   the found questions. One line per question, then recommendation, alternative,
   consequence.
5. Draft part B, the edit list, in doc-2's section order.
6. Draft parts C, D and E.
7. Create the document (`-t other`), write it, link it to the task.
8. Write the task's notes.
9. Record `just check` again, confirm `git -C <worktree> status` is clean, and hand over
   with `asdlc finish TASK-1.4 --as bruk.w5 --notes "..."`, no `--pr`.

## The questions the deliverable puts to Bruk

doc-2 section 9's six, with the lean this plan expects to recommend:

1. **Closing something already closed.** Lean: status 3 as proposed, because the server
   cannot tell a pane that is gone from a name that was never right, so returning 0 would
   hide typos. It is a deliberate departure from the cli-design skill, which defaults the
   other way and permits a specification to override; doc-2 must mark it as a departure
   and say why, which it does not today.
2. **A header line on lists.** Lean: never. doc-2 section 1 says predictable means "no
   behaviour that depends on whether a person is watching", and a terminal-conditional
   header is exactly that; never printing one makes a `list`'s bytes identical piped or
   not. Colour stays the one exception, written down as one.
3. **Short flags.** Lean: `-h`, `-q`, `-v` only, as proposed, matching the skill. Adding
   one later is a doc-2 change.
4. **Shell completion.** Lean: `steg --completion <shell>` as proposed, with the reason
   written down: the bare-program flags are the commands that are *not* API methods and
   must work with nothing running, so they cannot be a noun and a verb without inventing
   a server method for shell completion. Close that set at three.
5. **Showing a file in a pane (R63).** The one-line proposal
   (`steg pane split --file <path>`) does not carry R64 (focus stays put unless asked),
   R67 (reuse the pane already showing that file in the tab; replace a viewer's content;
   close it) or R68 (content on stdin with its type named). Lean, with no new verb:
   `pane split --file <path>` with `--type <name>`, `--file -` for standard input,
   `--tab` to put it in a new tab, a `--focus` and `--no-focus` pair with no-focus the
   default for a viewer, a `--reuse` and `--no-reuse` pair for R67, and `pane set --file`
   to replace a viewer's content, since the file is a property of the pane. `--file` and
   a program after `--` are mutually exclusive, status 2.
6. **Saved hosts (M3).** Lean: defer as doc-2 does, but narrow the question. R55 says
   remote hosts use the user's existing SSH setup, so the real question is whether steg
   creates a host at all or only labels one; if it only labels, `host new` and
   `host close` may not be needed and the closed verb set survives untouched.

Found while reviewing doc-2, each a question Bruk has not been asked:

7. **Does a new pane take focus?** Unspecified for `pane split` today, and R64 depends on
   the answer. Needs a default and a `--focus` and `--no-focus` pair.
8. **Does `-q` suppress a created identifier?** Section 5 says `--quiet` prints nothing
   "not even a new identifier"; section 6 makes that identifier the point of
   `steg pane split`. Rex resolves this the other way: its `-q` means "print IDs only".
   One of the two sentences has to move.
9. **Are list columns aligned when stdout is a terminal?** The skill says only colour and
   alignment change when output is piped; doc-2 says "piping changes nothing else" and
   never mentions alignment. Ties to question 2.
10. **How is a target part resolved when it could be two things?** Section 4 bans
    all-digit labels so a label cannot look like a position, but an identifier suffix of
    four or more characters can be all digits and so can look like a position too. R10
    says an ambiguous target is an error and never a guess, so either a resolution order
    is written down or that case is status 3. TASK-1.7 builds this.
11. **Is there a command that prints the API's self-description?** R37 says a client can
    ask for every method and R32 says the CLI exposes everything the API exposes, but no
    command in section 8 does it, and R62 generates the reference docs from it. Either a
    noun and verb for it, or doc-2 states the exemption.
12. **Can one session's or one pane's changes be streamed in M1, or only `event watch`?**
    Section 8 gives `watch` to `event` alone, so an agent watching one pane filters the
    stream itself. Whether `session watch` and `pane watch` exist is a design choice, not
    a defect, and TASK-1.13's drift test needs the answer in writing. (The mistitled
    table in section 3 that hides this is a part-B edit, not a question: the closed verb
    set is not the same as verbs every noun shares.)
13. **`pane focus --direction`.** Section 4 says the target is the thing the verb acts
    on, but focusing by direction acts on the target's *neighbour*. R9 asks for it, so
    the question is only whether doc-2 writes that exception down or splits the two
    meanings.

Edits in part B that need no decision from Bruk: `--by <cells>`, `--with <target>` and
`--all` are used in section 8 and defined in none of section 5's tables (so acceptance
criterion 1's "with its flags" is not yet fully met); section 3's table is titled "verbs
every noun shares" when it is the closed verb set, of which each noun uses what applies,
and section 8 is the authoritative per-noun set for M1; `--for <event>` has no event
names anywhere, and they should come from the API description's event record (R38,
TASK-1.3's format, TASK-1.11's events) rather than be written by hand.

## The acceptance criteria read against doc-2

- **#1, a board document lists every proposed command with its flags, the context
  variables and the exit statuses.** Substantially met by doc-2 already: section 8 the
  commands, section 5 the flags, section 4 the context variables, section 6 the exit
  statuses, and it is linked to this task. Not fully met in two places: three flags used
  in section 8 are defined nowhere, and the R63 command is one undecided line rather
  than a proposal, though this task's own description names it. Part B closes the first;
  question 5 closes the second. For Bruk: "every proposed command" reads naturally as
  the first release's set plus R63, not every command of every release; say if you meant
  wider.
- **#2, the document marks each point where Rex's CLI does it differently.** Met in form
  by section 10, but it has never been checked against Rex's documentation, so closing
  this criterion on today's doc-2 would close it on an unverified table. Part C checks
  it. Already visible from the one page fetched while planning: Rex takes a session's
  label as a positional and steg takes `--label`, and Rex's `-q` means something
  different from steg's, neither of which section 10 mentions.
- **#3, Bruk has recorded his decision on the document.** Not met, and not this worker's
  to meet. It closes when Bruk answers the sheet and the orchestrator records the answers
  in doc-2 section 9 as it did for the first three decisions. No rewording needed.

No accepted criterion is changed by this plan.

## One consequence TASK-1.3 should know

doc-2 section 5 says `--help` "contacts no server" and section 7 wants it under 10 ms
with no server running, and the cli-design skill says help, version and completion never
contact a service. That is policy, not a preference, and it decides TASK-1.3's second
axis: a command tree built at run time from the server's self-description cannot print
help with nothing running unless it caches the description. It goes in part E as a note,
not as a question, and it is for TASK-1.3's document to answer rather than for this task
to decide.

## Risks

- **Acceptance criterion 3 closes outside this worker's control,** so the task may sit
  in review while Bruk reads. Controlled by making the sheet answerable in one comment:
  one line per question, a recommendation on each.
- **Two CLI documents on one board.** Controlled by the new one's type (`other`), by
  titling it as a proposal against doc-2, by keeping every command table in doc-2, and
  by saying in part B that once the edits are applied the new document is a record of
  why, not a second specification.
- **Rex's documentation is partly behind a login** (the install pages are) and may change
  under us. Controlled by quoting the exact line each row rests on, with its URL and the
  date, so a later reader can tell a stale row from a wrong one. If a page needed for a
  row is gated, the row is marked unverified rather than left looking checked.
- **Over-reach into other tasks.** The R63 proposal is an M2 design, the event names are
  TASK-1.11's and the drift test is TASK-1.13's. Controlled by proposing flags on verbs
  that already exist, adding no verb, and writing each as a question or a note that names
  the task that builds it.
- **The consistency script eats the time a size-2 spike has.** Timeboxed above, with the
  hand-done cross-check as the fallback and the fallback declared in the notes.
- **`just check` may fail for reasons this branch did not cause** (there is no Rust
  workspace yet, and TASK-1.5 is building it in another worktree). Recorded anyway at
  both ends, with the expected skips named, so the evidence shows the branch changed
  nothing rather than hiding a failure.
- **Bruk answers differently from the leans.** That is the point of the sheet, not a
  departure from this plan: the deliverable is the questions and the reasoning, and his
  answer is what gets recorded.

## Proof

Each recorded with
`asdlc record-check TASK-1.4 --in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.4 --as bruk.w5 -- <command>`:

- `just check` at step 1 and again at step 9: the repository's own bar, and with it the
  claim that this branch changes no file.
- `git -C <worktree> status --short` at step 9: empty, the branch has no commit and no
  pull request, which is why `asdlc finish` carries no `--pr`.

For the criteria:

- **#1:** the consistency script's output, quoted in the new document, showing the set of
  flags used in doc-2 and defined nowhere, and the same run after part B's edits showing
  it empty. Part B is the proof that the gap is closed; the orchestrator applying it to
  doc-2 is what makes doc-2 itself whole.
- **#2:** part C, every row of section 10 with the line it rests on, its URL and the date
  fetched, plus the rows the check adds. A row that could not be verified says so.
- **#3:** part A exists with every question and a recommendation, and the notes name the
  single `asdlc board doc update doc-2` edit and the section 9 wording that records
  Bruk's answers. The criterion closes when he answers; nothing in this task's evidence
  will claim it closed before that.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
From Bruk, 2026-11-13, before this was claimed: 'For the CLI we should keep the grammar unixy.' The proposal must take that as a constraint and say what it means in each choice: command shape (short verbs such as ls, split, send, capture, against generated noun-verb such as pane split), line-oriented output that pipes cleanly, silence on success, errors on stderr, standard short and long flags with -- ending options, reading standard input where a command takes content, and one way of writing a target. Recorded by @bruk.o; the reading of 'unixy' is the orchestrator's and is for Bruk to confirm in the proposal.

Bruk gave the CLI's design direction himself on 2026-11-13: centre it on speed, predictability and composability; noun-verb grammar; a consistent set of verbs; universal flags; positionals for the primary target and flags for everything else; boolean flags as switches with --no- pairs. The orchestrator wrote that up as the CLI specification (doc-2), with the team's cli-design skill as its policy. What is left for this task: settle the nine points in its section 9 with Bruk, check every command in its section 8 against the API description TASK-1.3 delivers, replace the proposed speed numbers with measured ones, and record Bruk's decision on the document. Recorded by @bruk.o.

Onboarding skipped by @bruk.o: bruk-io-steg has no repository plugin yet and no code to read layers or verify commands from. TASK-1.24 onboards it once the workspace exists. The repository's own rules are in .claude/rules/ and its commands are in the justfile.
<!-- SECTION:NOTES:END -->
