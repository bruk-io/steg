# How the team reviews a task

A reviewer agent judges one finished task from the board, from a fresh context: its diff against the repository's default branch, judged against the task and this policy. A repository can add its own policy in its plugin, `plugins/<repo-slug>/review.md`, which adds to this one and never loosens it. Change either file through a pull request to the tasks repository, like any other change; whoever merges approves it.

The review records its findings and a verdict on the task with `asdlc review-done`, and posts the same findings on the pull request as a comment. It never approves, requests changes on, or merges a pull request: a person merges.

## What the work is judged against

- The task: its description, acceptance criteria, plan, and the notes and checks its worker recorded.
- The contract and the spec, when the task has a parent intent: the documents linked on the intent. Only the orchestrator, with the engineer, changes them.
- This policy, and the repository's own when its plugin has one.

## Passes

Tag every finding with its pass.

- **Bugs.** Logic errors, unhandled edge cases, regressions, code that won't build or run, and tests that don't test what they claim. Where the change starts a child process or a thread, ask which code waits on it, and follow every path that can end its life or drop its last handle: the ordinary end, an early return, an error branch, a shutdown. A path that leaves a child unreaped or a thread blocked for good is a finding, unless the notes say why that path is meant to leave it and who takes it over.
- **Security.** Secrets or tokens in code or logs, input reaching a shell, a query or a file path unchecked, permissions widened, and errors swallowed instead of reported.
- **Compliance.** Does the change do what the task's plan says, and meet each acceptance criterion? Compare the files it touches with the plan's files and its order of work. Judge each criterion clause by clause: when the notes say a criterion is covered, find the test or recorded run behind each of its clauses, and treat a clause with nothing behind it as unmet, whatever the summary says. The same holds for a branch the plan, a criterion or the review's brief names as worth proving, such as an error path, a retry, a refusal or one direction of a pair: it needs a test that reaches that branch, and a test of the ordinary path does not cover it. A change that builds against something other than the contract is a finding. A departure from the plan is a finding unless the task's notes say what changed and why.
- **Evidence.** Were the checks the worker recorded run on this branch, after its last change, and did they pass? A check recorded before the last commit, or not at all, is a finding. So is a proof step the plan or an acceptance criterion calls for that the notes describe but no recorded run shows, even when it was a demonstration meant to be reverted. A change to what an app draws whose pull request shows no image of the result, or no recording where the change is motion or a sequence of interactions, is an Important finding: look at the image against the task and its design source, and say what you compared.
- **Size and approval.** A task larger than `size-1` needs "Plan approved by @<engineer>" in its comments. A `size-1` task skipped plan approval, and its notes say "size-1: plan approval skipped; small because ...": its plan must be numbered steps, and the change must really be small and mechanical, a few lines in a few files with no design choice the steps left open. Either missing or untrue is an Important Compliance finding.

## Severity

- **Important**: breaks behaviour, leaves an acceptance criterion unmet, weakens a check or a test, leaks a credential, departs from the plan or the contract without saying so, or skipped plan approval on work that wasn't small.
- **Nit**: anything else. Report at most five nits per review, and summarise the rest as a count.

The verdict is `changes` when there is at least one Important finding, and `clean` otherwise; nits alone don't send work back.

## Skip

- Anything the repository's own checks already enforce: formatting, lint and type errors they catch, tests they run.
- Wording and style preferences, unless the wording would make a reader or a model do the wrong thing.

## Steg

These add to the passes above for work in steg. The rules they cite are in `.claude/rules/`.

- **Boundaries** (Compliance). A change that breaks a line of `.claude/rules/architecture.md` is Important unless the task's plan says why: I/O in the kernel, session state kept in a client, a client reaching the server by anything but the public API, an API method defined outside the API description, or code that assumes a pane is a terminal.
- **Licence** (Compliance). Code copied or closely adapted from a GPL or AGPL source, Zed's terminal and editor crates above all, is Important, as is an exception added to the licence check. A new dependency or borrowed code with no source and licence named on the task is a Nit.
- **Requirements** (Compliance). The task cites spec requirement numbers. A change that does something no cited requirement asks for, or builds a name, flag or exit status the CLI design document does not have, is a finding.
- **Tests** (Bugs). A mocking crate used on code inside the boundary, a test that sleeps to wait, or a terminal test that inspects the emulator's internals instead of the screen, is a finding under `.claude/rules/testing-rust.md`.
- **Rust** (Bugs). Judge the change against `.claude/rules/rust.md` where clippy cannot: a trait used to share code, a wildcard arm or a flag-plus-field where an enum belongs, an error turned into a string, shared locked state with no reason given, or `unsafe` outside its one module. Anything the workspace's lints already catch is skipped, as above.
