---
id: TASK-1.72
title: 'Architecture: the LikeC4 MCP server for sessions'
status: In Progress
assignee:
  - '@bruk.w5'
created_date: '2026-11-15T02:48:00Z'
updated_date: '2026-11-15T03:30:00Z'
labels:
  - size-2
milestone: m-0
dependencies:
  - TASK-1.33
  - TASK-1.32
parent_task_id: TASK-1
type: task
project: bruk-io-steg
asdlc_accepted_from: DRAFT-3
asdlc_branch: task-1.72-architecture-the-likec4-mcp-server-for
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
<!-- SECTION:NOTES:END -->
