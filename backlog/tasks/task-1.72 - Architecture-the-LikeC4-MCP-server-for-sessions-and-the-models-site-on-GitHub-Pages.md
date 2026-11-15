---
id: TASK-1.72
title: 'Architecture: the LikeC4 MCP server for sessions'
status: In Review
assignee:
  - '@bruk.w5'
created_date: '2026-11-15T02:48:00Z'
updated_date: '2026-11-15T03:44:00Z'
labels:
  - size-2
  - review-passed
milestone: m-0
dependencies:
  - TASK-1.33
  - TASK-1.32
references:
  - task-1.72-architecture-the-likec4-mcp-server-for
  - https://github.com/bruk-io/steg/pull/19
parent_task_id: TASK-1
type: task
project: bruk-io-steg
asdlc_accepted_from: DRAFT-3
asdlc_branch: task-1.72-architecture-the-likec4-mcp-server-for
asdlc_plan_approved_by: bruk
asdlc_plan_approved_hash: '367ae3e2b7cc'
asdlc_plan_approved_size: size-2
asdlc_review_kind: same-machine
asdlc_review_findings: 'important=0,nit=5'
asdlc_plan_matched: no
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
A follow-up to the LikeC4 model (TASK-1.33, doc-16 R108), asked for by Bruk on 2026-11-15. Two things: agents working in this repository can ask the model questions through LikeC4's MCP server instead of reading its source, and the built model is published as a site so it can be browsed without a checkout.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The repository configures LikeC4's MCP server for Claude Code sessions, pinned to the same LikeC4 version the check uses, and a session started in the repository can list the model's views and elements through it, shown by a recorded run
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
Scoped to AC#1 only: the GitHub Pages site was paused by Bruk while this plan was being written (see Implementation Notes) and its acceptance criteria were removed from this task. The research already done for it is kept there for whenever it resumes. Everything below is Part 1 only.

Facts checked today (2026-11-15), not from memory:

- `likec4@1.61.0` (the version `just`'s `architecture` recipe pins) has a native `likec4 mcp [path]` subcommand (`--stdio`, `--http`, `-p/--port`), confirmed by running `npx --yes likec4@1.61.0 --help` and `likec4 mcp --help` in the worktree.
- Ran `likec4 mcp --stdio <path>/architecture` in the worktree with a piped JSON-RPC `initialize` + `tools/list` + `tools/call list-projects`: the server answers over stdio, reports project name "architecture" (from `.likec4rc`), lists its 19 `.c4` source files, and exposes read-only tools including `list-projects`, `read-project-summary`, `read-view`, `search-element`. This is the real transport `.mcp.json` will use, not a stand-in.
- code.claude.com/docs/en/mcp (fetched today): a project-scoped server in `.mcp.json` sits at "Pending approval" until approved in an interactive session (`claude mcp reset-project-choices` to redo); `claude -p` / Agent SDK / cloud-session runs load project-scoped servers without that prompt. `${VAR}` expansion in a project-scoped entry's `command`/`args` needs a default, e.g. `${CLAUDE_PROJECT_DIR:-.}`, because that variable lives in the server's environment, not Claude Code's own.

## The LikeC4 MCP server for sessions in this repository

1. Add `.mcp.json` at the repository root (none exists today):

   {
     "mcpServers": {
       "likec4": {
         "command": "npx",
         "args": ["--yes", "likec4@1.61.0", "mcp", "--stdio", "${CLAUDE_PROJECT_DIR:-.}/architecture"]
       }
     }
   }

   Pinned to 1.61.0, the same version `just`'s `architecture` recipe (justfile) passes to `npx --yes likec4@...`. Leave a one-line comment-equivalent next to both pins (the plugin's testing/architecture rule, or a line in `.claude/rules/architecture.md`) saying the other place to bump when this version changes — there is no single source of truth for it today and this task doesn't add one; `args` uses `${CLAUDE_PROJECT_DIR:-.}` rather than a bare relative path because sessions run from a task's worktree (`steg.worktrees/<task>`), not always the main checkout, and the docs above are explicit that the default is required for project-scoped entries.

2. Proof, as a repeatable check rather than a one-off: add `tools/tests/test_likec4_mcp_smoke.py` (pytest, under the existing `tools/` uv package, run by `just test-tools` and therefore by `just check`). It is an **adapter test** (team:testing: "one gateway... against the real thing", no doubles) — it spawns the exact command `.mcp.json` declares (`npx --yes likec4@1.61.0 mcp --stdio <repo_root>/architecture`) as a subprocess, speaks the stdio JSON-RPC protocol directly (write `initialize`, the `notifications/initialized` notification, then `tools/call` for `list-projects` and `read-project-summary`, reading newline-delimited JSON responses off stdout), and asserts: the project name is `architecture` (matching `.likec4rc`), the view ids `read-project-summary` reports are non-empty and include the ones named in doc-16 R108 (context, containers, components/packages, deployment, and the two dynamic views for attaching to and focusing a pane — read their actual ids out of `architecture/views/` and `architecture/dynamic_views/` rather than guessing), and that at least one element from each of the five packages (sdk, kernel, server, `steg`, app) is present. This makes AC#1's "a session... can list the model's views and elements" a mechanical, CI-checked fact every time the model changes, not a single recorded transcript that can go stale.
   Record it with `asdlc record-check TASK-1.72 --in <worktree> --as bruk.w5 -- uv run --project tools pytest -q tools/tests/test_likec4_mcp_smoke.py`.

3. Separately record, once, that an actual Claude Code session sees the server: after committing `.mcp.json`, run `claude -p "List this repository's LikeC4 views and elements using the likec4 MCP tools, then print their names."` from the worktree and record its output with `asdlc record-check`. Per the docs above, a `claude -p` run loads a project-scoped `.mcp.json` server without the interactive approval prompt, so this is a faithful "a session... can list" demonstration that still runs unattended. State explicitly in the task notes that the *first interactive* `claude` session in this repository will still see the one-time "Pending approval" prompt for this server (not a bug, and `claude mcp reset-project-choices` resets it) — don't let this recorded proof imply an interactive session shows it connected with no prompt at all.

4. `.claude/rules/architecture.md` or doc-16 gets one line pointing at `.mcp.json` as the way agents query the model instead of reading `architecture/` by hand (small wording addition, reviewed with the rest of the diff).

No GitHub-side changes and no workflow are part of this plan: the Pages half is paused, and nothing above touches repository settings.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Decided by Bruk on 2026-11-15: it is fine for the architecture site to be public. The repository is private, so the GitHub Pages site will be readable by anyone with its address. The plan still states this, but it does not need confirming again.

Paused by Bruk on 2026-11-15: the GitHub Pages site is not part of this task for now. What was learned stays here for when it resumes: the repository is private and the bruk-io organisation is on GitHub's free plan, where Pages is not available for a private repository; a public site is acceptable to Bruk; the options were a separate public repository that receives only the built site, a paid plan, or making the repository public. The criteria removed were: a pinned workflow that builds and deploys the site on pushes to main that change the model; the site showing every view with its address in the README and doc-16; a model that does not validate failing the workflow and leaving the site as it was.

Plan checked by @bruk.o against the task as narrowed (the MCP server only): ok. A project .mcp.json runs LikeC4's own MCP server over stdio, pinned to the version the check uses; the proof is a recorded session run plus an adapter test that speaks the protocol to the real pinned command, so it keeps being proved by just check. The test makes Node a requirement of the tools' tests as it already is of just check. TASK-1.73 renames the architecture recipe; whichever lands second uses the name on main.

Check passed: `just check` exited 0 at 2026-11-15T03:36:37+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.72 on task-1.72-architecture-the-likec4-mcp-server-for at d41ef04bd837:

```text
ok      format check (0.2s)
ok      lint (18.5s)
ok      boundaries (0.2s)
ok      licences (0.7s)
ok      tests (every crate) (19.2s)
ok      tool tests (1.1s)
ok      architecture (1.1s)
```

Check passed: `uv run --project tools pytest -v tools/tests/adapter/test_likec4_mcp_smoke.py tools/tests/unit/test_likec4_mcp.py` exited 0 at 2026-11-15T03:36:47+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.72 on task-1.72-architecture-the-likec4-mcp-server-for at d41ef04bd837:

```text
warning: `VIRTUAL_ENV=/Users/brukhabtu/.cache/uv/builds-v0/.tmp6dEB2W` does not match the project environment path `tools/.venv` and will be ignored; use `--active` to target the active environment instead
============================= test session starts ==============================
platform darwin -- Python 3.15.0, pytest-9.2.0, pluggy-1.7.0 -- /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.72/tools/.venv/bin/python
cachedir: .pytest_cache
rootdir: /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.72/tools
configfile: pyproject.toml
collecting ... collected 4 items

tools/tests/adapter/test_likec4_mcp_smoke.py::test_a_session_can_list_this_repositorys_views_and_elements_through_the_mcp_server PASSED [ 25%]
tools/tests/unit/test_likec4_mcp.py::test_pulls_title_view_ids_and_element_ids_regardless_of_response_order PASSED [ 50%]
tools/tests/unit/test_likec4_mcp.py::test_missing_summary_response_raises_lookup_error PASSED [ 75%]
tools/tests/unit/test_likec4_mcp.py::test_tool_error_raises_lookup_error PASSED [100%]

============================== 4 passed in 0.99s ===============================
```

Check passed: `timeout 180 claude -p 'Using only the likec4 MCP server'"'"'s tools (do not read any file under architecture/ by hand), list every view id and every element id this repository'"'"'s LikeC4 model reports. Print the view ids as one line and the element ids as another line, nothing else.' --allowedTools mcp__likec4` exited 0 at 2026-11-15T03:37:39+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.72 on task-1.72-architecture-the-likec4-mcp-server-for at d41ef04bd837, last 40 of 62 lines:

```text
Permission allow rule (../../steg/.claude/settings.local.json): Bash(git -C /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/* checkout -b *) has a wildcard before the rest of the command, so it also matches any options inserted at that position and approves them without a prompt. For git, options such as -c and --exec-path can run arbitrary commands. Replace that * with the exact value you mean, or only use * after the subcommand (for example Bash(git status *)).
Permission allow rule (../../steg/.claude/settings.local.json): Bash(git -C /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/* checkout -b) has a wildcard before the rest of the command, so it also matches any options inserted at that position and approves them without a prompt. For git, options such as -c and --exec-path can run arbitrary commands. Replace that * with the exact value you mean, or only use * after the subcommand (for example Bash(git status *)).
Permission allow rule (../../steg/.claude/settings.local.json): Bash(git -C /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/* rev-parse *) has a wildcard before the rest of the command, so it also matches any options inserted at that position and approves them without a prompt. For git, options such as -c and --exec-path can run arbitrary commands. Replace that * with the exact value you mean, or only use * after the subcommand (for example Bash(git status *)).
Permission allow rule (../../steg/.claude/settings.local.json): Bash(git -C /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/* rev-parse) has a wildcard before the rest of the command, so it also matches any options inserted at that position and approves them without a prompt. For git, options such as -c and --exec-path can run arbitrary commands. Replace that * with the exact value you mean, or only use * after the subcommand (for example Bash(git status *)).
Permission allow rule (../../steg/.claude/settings.local.json): Bash(git -C /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/* branch *) has a wildcard before the rest of the command, so it also matches any options inserted at that position and approves them without a prompt. For git, options such as -c and --exec-path can run arbitrary commands. Replace that * with the exact value you mean, or only use * after the subcommand (for example Bash(git status *)).
Permission allow rule (../../steg/.claude/settings.local.json): Bash(git -C /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/* branch) has a wildcard before the rest of the command, so it also matches any options inserted at that position and approves them without a prompt. For git, options such as -c and --exec-path can run arbitrary commands. Replace that * with the exact value you mean, or only use * after the subcommand (for example Bash(git status *)).
Permission allow rule (../../steg/.claude/settings.local.json): Bash(git -C /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/* symbolic-ref *) has a wildcard before the rest of the command, so it also matches any options inserted at that position and approves them without a prompt. For git, options such as -c and --exec-path can run arbitrary commands. Replace that * with the exact value you mean, or only use * after the subcommand (for example Bash(git status *)).
Permission allow rule (../../steg/.claude/settings.local.json): Bash(git -C /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/* symbolic-ref) has a wildcard before the rest of the command, so it also matches any options inserted at that position and approves them without a prompt. For git, options such as -c and --exec-path can run arbitrary commands. Replace that * with the exact value you mean, or only use * after the subcommand (for example Bash(git status *)).
Permission allow rule (../../steg/.claude/settings.local.json): Bash(git -C /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/* mv *) has a wildcard before the rest of the command, so it also matches any options inserted at that position and approves them without a prompt. For git, options such as -c and --exec-path can run arbitrary commands. Replace that * with the exact value you mean, or only use * after the subcommand (for example Bash(git status *)).
Permission allow rule (../../steg/.claude/settings.local.json): Bash(git -C /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/* mv) has a wildcard before the rest of the command, so it also matches any options inserted at that position and approves them without a prompt. For git, options such as -c and --exec-path can run arbitrary commands. Replace that * with the exact value you mean, or only use * after the subcommand (for example Bash(git status *)).
Permission allow rule (../../steg/.claude/settings.local.json): Bash(git -C /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/* rm *) has a wildcard before the rest of the command, so it also matches any options inserted at that position and approves them without a prompt. For git, options such as -c and --exec-path can run arbitrary commands. Replace that * with the exact value you mean, or only use * after the subcommand (for example Bash(git status *)).
Permission allow rule (../../steg/.claude/settings.local.json): Bash(git -C /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/* rm) has a wildcard before the rest of the command, so it also matches any options inserted at that position and approves them without a prompt. For git, options such as -c and --exec-path can run arbitrary commands. Replace that * with the exact value you mean, or only use * after the subcommand (for example Bash(git status *)).
Permission allow rule (../../steg/.claude/settings.local.json): Bash(git -C /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/* restore *) has a wildcard before the rest of the command, so it also matches any options inserted at that position and approves them without a prompt. For git, options such as -c and --exec-path can run arbitrary commands. Replace that * with the exact value you mean, or only use * after the subcommand (for example Bash(git status *)).
Permission allow rule (../../steg/.claude/settings.local.json): Bash(git -C /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/* restore) has a wildcard before the rest of the command, so it also matches any options inserted at that position and approves them without a prompt. For git, options such as -c and --exec-path can run arbitrary commands. Replace that * with the exact value you mean, or only use * after the subcommand (for example Bash(git status *)).
Permission allow rule (../../steg/.claude/settings.local.json): Bash(git -C /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/* stash *) has a wildcard before the rest of the command, so it also matches any options inserted at that position and approves them without a prompt. For git, options such as -c and --exec-path can run arbitrary commands. Replace that * with the exact value you mean, or only use * after the subcommand (for example Bash(git status *)).
Permission allow rule (../../steg/.claude/settings.local.json): Bash(git -C /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/* stash) has a wildcard before the rest of the command, so it also matches any options inserted at that position and approves them without a prompt. For git, options such as -c and --exec-path can run arbitrary commands. Replace that * with the exact value you mean, or only use * after the subcommand (for example Bash(git status *)).
Permission allow rule (../../steg/.claude/settings.local.json): Bash(git -C /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/* reset *) has a wildcard before the rest of the command, so it also matches any options inserted at that position and approves them without a prompt. For git, options such as -c and --exec-path can run arbitrary commands. Replace that * with the exact value you mean, or only use * after the subcommand (for example Bash(git status *)).
Permission allow rule (../../steg/.claude/settings.local.json): Bash(git -C /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/* reset) has a wildcard before the rest of the command, so it also matches any options inserted at that position and approves them without a prompt. For git, options such as -c and --exec-path can run arbitrary commands. Replace that * with the exact value you mean, or only use * after the subcommand (for example Bash(git status *)).
Permission allow rule (../../steg/.claude/settings.local.json): Bash(git -C /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/* ls-files *) has a wildcard before the rest of the command, so it also matches any options inserted at that position and approves them without a prompt. For git, options such as -c and --exec-path can run arbitrary commands. Replace that * with the exact value you mean, or only use * after the subcommand (for example Bash(git status *)).
Permission allow rule (../../steg/.claude/settings.local.json): Bash(git -C /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/* ls-files) has a wildcard before the rest of the command, so it also matches any options inserted at that position and approves them without a prompt. For git, options such as -c and --exec-path can run arbitrary commands. Replace that * with the exact value you mean, or only use * after the subcommand (for example Bash(git status *)).
Permission allow rule (../../steg/.claude/settings.local.json): Bash(git -C /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/* grep *) has a wildcard before the rest of the command, so it also matches any options inserted at that position and approves them without a prompt. For git, options such as -c and --exec-path can run arbitrary commands. Replace that * with the exact value you mean, or only use * after the subcommand (for example Bash(git status *)).
Permission allow rule (../../steg/.claude/settings.local.json): Bash(git -C /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/* grep) has a wildcard before the rest of the command, so it also matches any options inserted at that position and approves them without a prompt. For git, options such as -c and --exec-path can run arbitrary commands. Replace that * with the exact value you mean, or only use * after the subcommand (for example Bash(git status *)).
Permission allow rule (../../steg/.claude/settings.local.json): Bash(git -C /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/* blame *) has a wildcard before the rest of the command, so it also matches any options inserted at that position and approves them without a prompt. For git, options such as -c and --exec-path can run arbitrary commands. Replace that * with the exact value you mean, or only use * after the subcommand (for example Bash(git status *)).
Permission allow rule (../../steg/.claude/settings.local.json): Bash(git -C /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/* blame) has a wildcard before the rest of the command, so it also matches any options inserted at that position and approves them without a prompt. For git, options such as -c and --exec-path can run arbitrary commands. Replace that * with the exact value you mean, or only use * after the subcommand (for example Bash(git status *)).
Permission allow rule (../../steg/.claude/settings.local.json): Bash(git -C /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/* cat-file *) has a wildcard before the rest of the command, so it also matches any options inserted at that position and approves them without a prompt. For git, options such as -c and --exec-path can run arbitrary commands. Replace that * with the exact value you mean, or only use * after the subcommand (for example Bash(git status *)).
Permission allow rule (../../steg/.claude/settings.local.json): Bash(git -C /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/* cat-file) has a wildcard before the rest of the command, so it also matches any options inserted at that position and approves them without a prompt. For git, options such as -c and --exec-path can run arbitrary commands. Replace that * with the exact value you mean, or only use * after the subcommand (for example Bash(git status *)).
Permission allow rule (../../steg/.claude/settings.local.json): Bash(git -C /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/* ls-tree *) has a wildcard before the rest of the command, so it also matches any options inserted at that position and approves them without a prompt. For git, options such as -c and --exec-path can run arbitrary commands. Replace that * with the exact value you mean, or only use * after the subcommand (for example Bash(git status *)).
Permission allow rule (../../steg/.claude/settings.local.json): Bash(git -C /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/* ls-tree) has a wildcard before the rest of the command, so it also matches any options inserted at that position and approves them without a prompt. For git, options such as -c and --exec-path can run arbitrary commands. Replace that * with the exact value you mean, or only use * after the subcommand (for example Bash(git status *)).
Permission allow rule (../../steg/.claude/settings.local.json): Bash(git -C /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/* merge-base *) has a wildcard before the rest of the command, so it also matches any options inserted at that position and approves them without a prompt. For git, options such as -c and --exec-path can run arbitrary commands. Replace that * with the exact value you mean, or only use * after the subcommand (for example Bash(git status *)).
Permission allow rule (../../steg/.claude/settings.local.json): Bash(git -C /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/* merge-base) has a wildcard before the rest of the command, so it also matches any options inserted at that position and approves them without a prompt. For git, options such as -c and --exec-path can run arbitrary commands. Replace that * with the exact value you mean, or only use * after the subcommand (for example Bash(git status *)).
Permission allow rule (../../steg/.claude/settings.local.json): Bash(git -C /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/* rev-list *) has a wildcard before the rest of the command, so it also matches any options inserted at that position and approves them without a prompt. For git, options such as -c and --exec-path can run arbitrary commands. Replace that * with the exact value you mean, or only use * after the subcommand (for example Bash(git status *)).
Permission allow rule (../../steg/.claude/settings.local.json): Bash(git -C /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/* rev-list) has a wildcard before the rest of the command, so it also matches any options inserted at that position and approves them without a prompt. For git, options such as -c and --exec-path can run arbitrary commands. Replace that * with the exact value you mean, or only use * after the subcommand (for example Bash(git status *)).
Permission allow rule (../../steg/.claude/settings.local.json): Bash(git -C /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/* describe *) has a wildcard before the rest of the command, so it also matches any options inserted at that position and approves them without a prompt. For git, options such as -c and --exec-path can run arbitrary commands. Replace that * with the exact value you mean, or only use * after the subcommand (for example Bash(git status *)).
Permission allow rule (../../steg/.claude/settings.local.json): Bash(git -C /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/* describe) has a wildcard before the rest of the command, so it also matches any options inserted at that position and approves them without a prompt. For git, options such as -c and --exec-path can run arbitrary commands. Replace that * with the exact value you mean, or only use * after the subcommand (for example Bash(git status *)).
Permission allow rule (../../steg/.claude/settings.local.json): Bash(git -C /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/* show-ref *) has a wildcard before the rest of the command, so it also matches any options inserted at that position and approves them without a prompt. For git, options such as -c and --exec-path can run arbitrary commands. Replace that * with the exact value you mean, or only use * after the subcommand (for example Bash(git status *)).
Permission allow rule (../../steg/.claude/settings.local.json): Bash(git -C /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/* show-ref) has a wildcard before the rest of the command, so it also matches any options inserted at that position and approves them without a prompt. For git, options such as -c and --exec-path can run arbitrary commands. Replace that * with the exact value you mean, or only use * after the subcommand (for example Bash(git status *)).
Permission allow rule (../../steg/.claude/settings.local.json): Bash(git -C /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/* remote get-url *) has a wildcard before the rest of the command, so it also matches any options inserted at that position and approves them without a prompt. For git, options such as -c and --exec-path can run arbitrary commands. Replace that * with the exact value you mean, or only use * after the subcommand (for example Bash(git status *)).
Permission allow rule (../../steg/.claude/settings.local.json): Bash(git -C /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/* remote get-url) has a wildcard before the rest of the command, so it also matches any options inserted at that position and approves them without a prompt. For git, options such as -c and --exec-path can run arbitrary commands. Replace that * with the exact value you mean, or only use * after the subcommand (for example Bash(git status *)).
attachToAPane focusAPane index stegContainers stegContext stegDeployment stegPackages
personAtMac programsInPanes macos plugin remoteHost steg steg.app steg.app.appPkg steg.app.sdk steg.cli steg.cli.sdk steg.cli.stegPkg steg.server steg.server.kernel steg.server.sdk steg.server.serverPkg steg.server.stegPkg
```

Check passed: `just check` exited 0 at 2026-11-15T03:38:06+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.72 on task-1.72-architecture-the-likec4-mcp-server-for at 5f440ca665b2:

```text
ok      format check (0.1s)
ok      lint (0.4s)
ok      boundaries (0.1s)
ok      licences (0.6s)
ok      tests (every crate) (0.3s)
ok      tool tests (1.2s)
ok      architecture (1.0s)
```

Built as planned, scoped to AC#1 only (the Pages half was paused by the engineer before the build phase and its acceptance criteria were removed).

What was built:
- .mcp.json at the repository root: a project-scoped stdio server (likec4 mcp --stdio), pinned to likec4@1.61.0, the same version the model-validating `just` recipe checks against. The path arg uses ${CLAUDE_PROJECT_DIR:-.}/architecture so it resolves correctly from a task's worktree, not just the main checkout.
- tools/src/steg_tools/likec4_mcp.py: a pure JSON-RPC response parser (parse_project_summary) plus one function (read_project_summary) that owns the subprocess and pipe - the only I/O in the module.
- tools/tests/unit/test_likec4_mcp.py: unit tests for the parser against hand-built responses, no doubles.
- tools/tests/adapter/test_likec4_mcp_smoke.py: an adapter test that runs the exact pinned command .mcp.json declares against this repository's real architecture/ model and asserts the seven real view ids (index, stegContext, stegContainers, stegPackages, stegDeployment, attachToAPane, focusAPane) and one element id per package (sdk, kernel, server, steg, app) come back. This now runs on every `just test-tools` / `just check`.
- .claude/rules/architecture.md: one bullet pointing agents at the MCP server instead of reading architecture/ by hand (placed there only, not doc-16, per the engineer's direction).

Which recorded check proves which clause of AC#1:
- "configures LikeC4's MCP server... pinned to the same LikeC4 version the check uses": .mcp.json's args pin likec4@1.61.0, matching the justfile's model-validating recipe's own npx pin - visible in the diff, confirmed passing by the `just check` record-check (its "architecture" step runs the same pinned version).
- "a session started in the repository can list the model's views and elements through it, shown by a recorded run": two recorded proofs -
  1. `uv run --project tools pytest -v tools/tests/adapter/test_likec4_mcp_smoke.py tools/tests/unit/test_likec4_mcp.py` - the mechanical, repeatable proof: the pinned stdio server answers for the real model and every expected view/element id is present.
  2. `timeout 180 claude -p "..." --allowedTools mcp__likec4` - a real Claude Code session, using only the likec4 MCP tools (no file reads under architecture/), printing all 7 view ids and all 17 element ids correctly. This is the literal "a session... can list" demonstration. Per code.claude.com/docs/en/mcp, a `claude -p` run loads a project-scoped .mcp.json server without the interactive approval prompt; the first *interactive* `claude` session in this repository will still see a one-time "Pending approval" for this server (not a bug - `claude mcp reset-project-choices` resets it).
- `just check` recorded clean both before and after rebasing onto origin/main (no divergence either time - this branch was never pushed before this task).

Departure from the plan: none. The plan's Part 1 was built as written; Part 2 (Pages) was dropped from scope before the build phase per the engineer, not by me.
<!-- SECTION:NOTES:END -->

## Comments

<!-- COMMENTS:BEGIN -->
author: @bruk
created: 2026-11-15T03:31:00Z

---

Plan approved by @bruk (367ae3e2b7cc, size-2): its worker may build it.

---

author: @bruk.r1
created: 2026-11-15T03:38:00Z

---

@bruk.r1 is reviewing TASK-1.72: a same-machine review: it waited 0m with no other engineer's reviewer, longer than review_fallback_after (0m), so a fresh reviewer on its author's machine takes it.

---

author: @bruk.r1
created: 2026-11-15T03:44:00Z

---

Review by @bruk.r1 (same-machine): clean. It stays In Review for a person to merge.

Findings: 0 Important, 5 nit. The diff does not match the plan.

No Important findings; verdict clean. Pages work correctly absent from the diff (5 files: .claude/rules/architecture.md, .mcp.json, tools/src/steg_tools/likec4_mcp.py, tools/tests/adapter/test_likec4_mcp_smoke.py, tools/tests/unit/test_likec4_mcp.py - no workflow, no repository settings).

AC#1: .mcp.json pins likec4@1.61.0, matching justfile's `architecture` recipe (npx --yes likec4@1.61.0 validate architecture) - the same version the check uses. The "session can list" clause has a real recorded-check entry (not prose): `timeout 180 claude -p '...' --allowedTools mcp__likec4` exited 0, recorded with asdlc record-check, run inside steg.worktrees/task-1.72 and printing all 7 view ids and 17 element ids - direct evidence the worktree case (not just the main checkout) resolves correctly via ${CLAUDE_PROJECT_DIR:-.}. The adapter test backs this mechanically: `test-tools` runs `uv run ... pytest -q tools/tests` unconditionally (justfile:43), so it cannot have been skipped when the "tool tests" step passed on PR19's head commit 5f440ca in CI run 43673271987 (SUCCESS).

Nits (5):
[Compliance, Nit] Implementation notes' "Departure from the plan: none" mislabels a disclosed delta: "What was built" correctly names tools/src/steg_tools/likec4_mcp.py and tools/tests/unit/test_likec4_mcp.py, neither in the approved plan (which put the protocol-speaking logic inline in one test file, tools/tests/test_likec4_mcp_smoke.py). The module/unit-test split itself is sound - required even, by plugins/team/rules/testing-python.md's four-layer tests/unit|functional|adapter|e2e layout and the pure-core/thin-shell style - but the summary line should say so rather than deny any departure.
[Bugs, Nit] tools/src/steg_tools/likec4_mcp.py:124-131 (read_project_summary) reads exactly two lines and assumes they are the replies to ids 1 and 2. A notification from the server has no "id", would land under key None in parse_project_summary's by_id map, and would displace one of the two expected reads - parse_project_summary would then raise LookupError spuriously instead of the loop waiting for id 2 specifically. False-negative only, bounded by the existing timeout.
[Bugs, Nit] Same function: stderr=subprocess.PIPE is opened but never drained or surfaced. On a TimeoutError or LookupError the server's own diagnostic output is discarded, leaving a CI failure with no clue why.
[Nit] likec4_mcp.py:8 `from __future__ import annotations` is unneeded - tools/pyproject.toml pins requires-python >=3.15 - and isn't caught by ruff's default rule set (no pyupgrade/UP rules enabled here).
[Compliance, Nit] .claude/rules/architecture.md: PR19's new bullet is appended directly after the "dependency from one crate on another" bullet, the exact line PR17/TASK-1.71 rewrites. Purely textual: the content composes cleanly (no semantic contradiction), but whoever merges second will hit a merge conflict there to resolve by hand.

plan-matched=no: the extra production module and unit-test file are a real departure from the plan's named files, even though it's disclosed in full in "What was built" and backed by the testing-python layout rule.

---
<!-- COMMENTS:END -->
