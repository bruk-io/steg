---
id: TASK-1.72
title: 'Architecture: the LikeC4 MCP server for sessions'
status: In Progress
assignee:
  - '@bruk.w5'
created_date: '2026-11-15T02:48:00Z'
updated_date: '2026-11-15T03:29:00Z'
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
Facts checked today (2026-11-15), not from memory:

- `likec4@1.61.0` (the version `just`'s `architecture` recipe pins) has a native `likec4 mcp [path]` subcommand (`--stdio`, `--http`, `-p/--port`), confirmed by running `npx --yes likec4@1.61.0 --help` and `likec4 mcp --help` in the worktree.
- Ran `likec4 mcp --stdio <path>/architecture` in the worktree with a piped JSON-RPC `initialize` + `tools/list` + `tools/call list-projects`: the server answers over stdio, reports project name "architecture" (from `.likec4rc`), lists its 19 `.c4` source files, and exposes read-only tools including `list-projects`, `read-project-summary`, `read-view`, `search-element`. This is the real transport `.mcp.json` will use, not a stand-in.
- `git ls-files` on the worktree shows no root `README.md` (only `devs/README.md` and the plugin/spike READMEs). AC#3 ("the README") needs one created, not edited.
- `gh api repos/bruk-io/steg` → private, `has_pages: false`. `gh api orgs/bruk-io` → plan `free`, 3 filled seats.
- docs.github.com (fetched today): "GitHub Pages is available in public repositories with GitHub Free... and in public and private repositories with GitHub Pro, GitHub Team, GitHub Enterprise Cloud, and GitHub Enterprise Server." And: "To publish a GitHub Pages site privately, your organization must use GitHub Enterprise Cloud" — so Team/Pro give Pages on a private repo, but the resulting site is reachable by anyone with its URL (no restriction short of Enterprise Cloud), which is exactly the "private repo, public site" shape Bruk already accepted.
- github.com/pricing (fetched today): GitHub Team is $4 USD/user/month (vs. Enterprise $21). Against 3 filled seats that is about $12/month, recurring, org-wide — not scoped to this one repository.
- code.claude.com/docs/en/mcp (fetched today): a project-scoped server in `.mcp.json` sits at "Pending approval" until approved in an interactive session (`claude mcp reset-project-choices` to redo); `claude -p` / Agent SDK / cloud-session runs load project-scoped servers without that prompt. `${VAR}` expansion in a project-scoped entry's `command`/`args` needs a default, e.g. `${CLAUDE_PROJECT_DIR:-.}`, because that variable lives in the server's environment, not Claude Code's own.

## Part 1 — LikeC4 MCP server for sessions in this repository

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

3. Separately record, once, that an actual Claude Code session sees the server: after committing `.mcp.json`, run `claude -p "List this repository's LikeC4 views and elements using the likec4 MCP tools, then print their names." ` from the worktree and record its output with `asdlc record-check`. Per the docs above, a `claude -p` run loads a project-scoped `.mcp.json` server without the interactive approval prompt, so this is a faithful "a session... can list" demonstration that still runs unattended. State explicitly in the task notes that the *first interactive* `claude` session in this repository will still see the one-time "Pending approval" prompt for this server (not a bug, and `claude mcp reset-project-choices` resets it) — don't let this recorded proof imply an interactive session shows it connected with no prompt at all.

4. `.claude/rules/architecture.md` or doc-16 gets one line pointing at `.mcp.json` as the way agents query the model instead of reading `architecture/` by hand (small wording addition, reviewed with the rest of the diff).

## Part 2 — the model's site on GitHub Pages

### The options (none chosen here; the engineer picks)

a. **Publish from a separate public repository that receives only the built site.** Create a new public repo (e.g. `bruk-io/steg-architecture`); `steg`'s workflow builds the site and pushes the output to that repo's default branch over HTTPS using a fine-grained personal access token scoped to only that repository with `contents: write`, stored as a secret in `steg` (e.g. `ARCHITECTURE_SITE_TOKEN`); that repo's own Settings → Pages is set to "Deploy from a branch" (one-time, no second workflow needed). Cost: $0 recurring. One-time: an org admin creates the repo, mints the token (or a deploy key, which needs an extra ssh-agent step in the workflow — the PAT is simpler), adds it as a `steg` secret, and sets that repo's Pages source. Keeps `steg`'s own private history, issues and backlog out of what's published — only the built diagrams leave the private repo. Ongoing: one more credential to rotate if it's ever revoked, and the site's URL is a second repo's `github.io` address, not `steg`'s.
b. **Make `steg` public.** $0, one admin setting (Settings → General → Change visibility), reversible only by GitHub support, not by re-flipping the toggle. Exposes everything in the repository's history — code, `backlog/`, `plugins/` — not just the architecture model. The intent (TASK-1) and this repo's current visibility suggest that's broader than what was asked for ("it is fine for the architecture site to be public" — doc-16 Implementation Notes — is about the diagrams, not the source).
c. **Upgrade the `bruk-io` org from Free to GitHub Team.** ~$12/month today (3 filled seats × $4), recurring, org-wide (every repo in the org gets private-repo Pages, not just this one). Needs the org's billing owner. Then `steg`'s own Settings → Pages is set directly (source: GitHub Actions), and the workflow in `steg` deploys straight to its own Pages environment with the native `actions/configure-pages` + `actions/upload-pages-artifact` + `actions/deploy-pages` (needs `permissions: pages: write, id-token: write` and a `github-pages` environment) — no second repository, no cross-repo credential. Simplest mechanically, but it's a standing subscription decision, not a one-time setting.
d. **Don't publish a site yet.** $0; defers AC#2/#3 to a later task.

### Recommendation: (a), the separate public repository

It is the only option that is both free and keeps `steg`'s own content (code, backlog, plugins, issues) private while still giving the engineer the public diagrams site he's already said is fine. (b) gives up far more than the diagrams. (c) works but is a recurring org-wide cost for one repository's Pages needs, and is harder to undo cleanly than deleting a secret and a small repo. If the engineer instead wants the simplest workflow and is fine with the subscription, (c) is the one to fall back to — the workflow below would then drop the push-to-another-repo step and deploy natively instead, everything else (validate-then-build gate, triggers, pinning) staying the same.

### The workflow, for option (a)

New file `.github/workflows/architecture-pages.yml` (kept separate from `ci.yml`: different runner class, different trigger scope, and it must never block or be blocked by the Rust build).

- `on: push: branches: [main], paths: ["architecture/**"]` — only pushes to main that touch the model, per AC#2.
- `permissions: contents: read` (no `pages:`/`id-token:` — this workflow pushes to another repository over HTTPS, it never touches `steg`'s own Pages environment because `steg` has none under this option).
- `runs-on: ubuntu-24.04` — a pinned label, not `ubuntu-latest`; this job needs only Node, so it doesn't justify the macOS runner's premium minutes (ci.yml's own comment on `cancel-in-progress` makes the same cost point).
- Steps, each pinned to a commit SHA the same way `ci.yml` does, re-verified against the upstream release pages at implementation time (confirmed live today): `actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1 # v7.0.1`, `actions/setup-node@949feb2413d6458794dcd2491c4babbbce0c15c1 # v7.1.0` with `node-version: "26.13.1"` (current LTS "Lithium" as of today).
  1. Checkout `steg` (shallow is fine; nothing here diffs against `origin/main`).
  2. Setup Node, pinned as above. This is the Node-pinning follow-up noted in `ci.yml`'s header comment — folded into *this* workflow only; whether `ci.yml` itself also gets a pinned `setup-node` step is a separate, one-line decision for the engineer to make (it touches the expensive macOS workflow and widens that review), not assumed here.
  3. `npx --yes likec4@1.61.0 validate architecture` — the exact command the justfile's model-validating recipe runs today, called directly rather than through `just <that recipe>`. That recipe is renamed by TASK-1.73 (in progress in parallel); calling the npx command directly means this workflow needs no coordination with that rename. Leave a comment in both the justfile recipe and this step naming the other as the place to bump the `1.61.0` pin in lockstep.
  4. If validation fails, the job fails here — nothing past this point runs, satisfying AC#4 (the model is validated *before* build, and build is strictly downstream of validate in the same job with no `continue-on-error` anywhere, so a bad model can't reach the deploy step).
  5. `npx --yes likec4@1.61.0 build --base /steg-architecture/ -o site-dist architecture` (the `--base` path must equal `/<destination-repo-name>/` — a project Pages site is served at `https://bruk-io.github.io/<repo>/`, and a wrong or missing `--base` is the classic way a Vite-built site's assets 404 once deployed; whatever name the engineer actually gives the destination repo, this flag must match it exactly).
  6. Checkout the destination repo into a second directory over HTTPS using `https://x-access-token:${{ secrets.ARCHITECTURE_SITE_TOKEN }}@github.com/bruk-io/<destination-repo>.git` (plain `git clone` + `git checkout`, not a third-party action, to keep the write-capable credential's blast radius to code this repo already reviews).
  7. Replace that checkout's tracked contents with `site-dist`'s output (`git rm -rf` the old tree, copy in the new one), commit, and `git push`. If there is no diff, skip the commit instead of pushing an empty one.
- AC#3 (every view, and the address recorded): `likec4 build` with no `--include`/`--only` flag builds every view under `architecture/views/` and `architecture/dynamic_views/` by default, so nothing further is needed for "every view" once the build step runs. The address itself (`https://bruk-io.github.io/<destination-repo>/`) can only be written down once the engineer has created that repo and the Pages source is live — until then there's nothing real to put in the README or doc-16.
- Root `README.md`: none exists (confirmed above). This task adds a minimal one — what steg is, in a sentence or two, and the architecture site's address — rather than leaving AC#3 referring to a file that isn't there. Scope it to that; a fuller project README is not this task's job.
- doc-16 gets the same address added (its Scope or Areas of concern section, next to the existing R108 line about the LikeC4 model). `asdlc board doc update doc-16 --content <full replacement text>` — this is a whole-document replace, so the build step reads the current content first and writes it back with the one line inserted. This runs from the tasks clone and the board commits/pushes it to `main` itself (`asdlc board` convention); it will **not** show up in the `steg` pull request's diff, so say so plainly in the task's finish notes or a reviewer looking only at the branch will think AC#3's doc-16 half is unmet.

### Proof to record for Part 2 (build phase, once the engineer has decided and created what their option needs)

- A run id of the real `architecture-pages.yml` run on a push to main that touches `architecture/`, showing validate → build → push-to-destination all green, and the resulting Pages run on the destination repo.
- A run id of a deliberately-broken model (e.g. a stray element reference) on a branch, showing the workflow fails at the validate step and the destination repo/site is untouched — the mechanical proof for AC#4.
- The published site's address, fetched once to confirm it actually serves every view, then written into the new README and into doc-16 as above.

## Open questions for the engineer, not settled by this plan

- Which Pages option (a/b/c/d above), and if (a), what to name the destination repository.
- Whether `ci.yml` should also gain a pinned `setup-node` step now that one exists elsewhere in the repo (left as the engineer's call, not folded into this task).
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Decided by Bruk on 2026-11-15: it is fine for the architecture site to be public. The repository is private, so the GitHub Pages site will be readable by anyone with its address. The plan still states this, but it does not need confirming again.

Paused by Bruk on 2026-11-15: the GitHub Pages site is not part of this task for now. What was learned stays here for when it resumes: the repository is private and the bruk-io organisation is on GitHub's free plan, where Pages is not available for a private repository; a public site is acceptable to Bruk; the options were a separate public repository that receives only the built site, a paid plan, or making the repository public. The criteria removed were: a pinned workflow that builds and deploys the site on pushes to main that change the model; the site showing every view with its address in the README and doc-16; a model that does not validate failing the workflow and leaving the site as it was.
<!-- SECTION:NOTES:END -->
