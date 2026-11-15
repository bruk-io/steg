---
id: TASK-1.33
title: 'Architecture model in LikeC4: context, containers, components and deployment'
status: In Progress
assignee:
  - '@bruk.w2'
created_date: '2026-11-15T01:32:00Z'
updated_date: '2026-11-15T02:43:00Z'
labels:
  - size-2
milestone: m-0
dependencies: []
parent_task_id: TASK-1
type: task
project: bruk-io-steg
asdlc_accepted_from: DRAFT-3
asdlc_branch: task-1.33-architecture-model-in-likec4-context
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
doc-16 is the architecture, written at the levels of the C4 model; this task draws it (R108). The model of servers, clients and windows was got wrong three times before decision-5 fixed it, which is why it is drawn. It is not the domain, which is doc-15. Revise the plan to this description; the earlier plan was written before doc-16 existed.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A LikeC4 model in the repository has the system context (with a plugin and a remote host as external systems), the containers (the server, the CLI, Steg.app) and the components (sdk, kernel, server, `steg`, app) as doc-16 gives them, including which package may depend on which
- [ ] #2 A package built into several containers appears as a component of each of them, not as the child of one
- [ ] #3 A deployment view shows one window holding a client to a local server and another to a remote one
- [ ] #4 Two dynamic views: attaching to a pane, and focusing a pane with the path updating upward
- [ ] #5 `just check` validates the model and fails when it does not validate (R108)
- [ ] #6 Every name in the model is a word doc-16, doc-15 or doc-1's vocabulary uses, with the same meaning
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
Revised 2026-11-15 for doc-16 (replaces the earlier plan, which was written before doc-16 existed).

## Where the model lives

Unchanged: a new top-level `architecture/` directory (sibling to `crates/`, `spikes/`, `tools/`), scaffolded once with:

```
python3 <skill-dir>/tools/scaffold.py architecture --system steg
```

`scaffold.py` has real write side effects and no dry-run, and its `-h`/`--help` is swallowed as the target-dir argument (confirmed while planning; two stray scaffolds it produced outside the worktree were already deleted). Run it exactly once, before any element file is written by hand.

## Six structural decisions, two of them flagged for the engineer

1. **Plugin is modeled once, as a container — not also as an external system.** doc-16's Level 1 table lists "A plugin (M2)" as External; its Level 2 table lists "A plugin (M2)" as a Container. A single LikeC4 element cannot be both an `externalSystem` (top-level, black box) and a `container` (nested in `steg`), and AC #1 fixes the container list explicitly (`server, CLI, Steg.app, a plugin`), so the model resolves toward Level 2: `plugin` is a container of `steg`, full stop, and the System Context view simply does not show it — context views never show containers. **Flagged for the engineer:** doc-16 itself disagrees with itself here across its own two tables; R108's "this spec is right" clause can't adjudicate a spec that contradicts itself, so this is a one-line confirmation to get at plan approval, not a silent call.
2. **`remoteHost` stays an external system only** — it appears in Level 1 and in the deployment view (as a deployment node `instanceOf steg.server`, a different element in a different diagram kind), never as a fifth container. Level 2's container table doesn't list it, and doc-16's own Deployment section treats "a server on another machine" as the same `steg` software deployed elsewhere, not a new container.
3. **`sdk`'s single LikeC4 parent is a modeling artifact, not a claim about ownership — flagged for the engineer.** doc-16 says sdk is "Built into: the server, the CLI, the app" (three containers), but a LikeC4 component can nest under exactly one container. Nesting it under `server` would read as "the server owns the sdk," which is the exact inversion R98 and R101 exist to prevent (clients depend on the sdk *instead of* the server). I nest it under `cli` instead — slightly less misleading, since CLI's own package (`steg`) is already doc-16's "entry point" into the whole binary — and the component's own description says explicitly that the nesting is a LikeC4 constraint, not an ownership claim, and that `kernel`, the `server` package and the `app` package all depend on it from outside the box it happens to be drawn in.
4. **"Built into" is prose, never a LikeC4 relationship.** The only arrows the packages view draws are doc-16's "May depend on" table (6 edges: `kernel → sdk`, `server → kernel`, `server → sdk`, `` `steg` → sdk ``, `` `steg` → server ``, `app → sdk`). Which container(s) a package is compiled into is stated in that package's `description`, because it is a build/packaging fact, not a call.
5. **Cross-container dependency edges carry no `technology`, on purpose.** Four of the six edges above cross the container boundary I had to draw for nesting (`kernel → sdk`, `server(pkg) → sdk`, `` `steg` → server(pkg) ``, `app(pkg) → sdk`) — but they are Cargo compile-time dependencies, not runtime calls over a socket or any other protocol, so the hard rule requiring `technology` on a relationship that "crosses a process boundary" doesn't apply in spirit even though it nominally crosses a container box in this drawing. Each such relationship gets a title (`'depends on'`) and the file carries one comment explaining why there's no `technology` field. `c4lint.py` (run by hand during the build, not wired into `just check`) may well flag this mechanically; if it does, that finding goes in the task's build notes, not silenced with an invented `technology 'Rust'`.
6. **This task draws the model; it does not touch Cargo.** doc-16 describes an intended package structure (`sdk`, `kernel`, `server`, `` `steg` ``, `app` — explicitly "no steg-client and no separate CLI package") that does not match the repository's current `crates/` layout (`crates/kernel`, `crates/server`, `crates/cli`, `crates/app`, `crates/steg`). R104's actual enforcement — `boundaries.toml`, the real crate reorg that retires `crates/cli` — is TASK-1.69's job ("Workspace: the server's code kept apart from its clients, and the steg library's package"). This task writes `architecture/` only; nothing under `crates/`, `boundaries.toml`, `Cargo.toml` or `layers.toml` changes. The mismatch between the drawn model and today's `crates/` is expected, not drift, and is worth saying once in the PR description so it isn't mistaken for an oversight.

## File-by-file plan

```
architecture/
  models/
    landscape.c4
      — personAtMac = person 'The person at the Mac' (doc-16 Level 1)
      — programsInPanes = externalSystem 'Programs in panes' { description 'A shell, an editor, an agent. steg starts them, feeds them input and reads their output. Some drive steg back through the CLI.' }
      — macos = externalSystem 'macOS' { description 'Windows, pseudo-terminals, the clipboard, the system's light or dark setting.' }
      — remoteHost = externalSystem 'A remote host' { #planned description 'Another machine running a steg server (M3).' }
      — personAtMac -> steg 'uses, through its window and its CLI'
      — steg -> programsInPanes 'starts, feeds input to and reads output from'
      — programsInPanes -> steg 'drives back, through the CLI'
      — steg -> macos 'opens windows on, allocates pseudo-terminals from, and reads the system theme from'
      — steg -> remoteHost 'connects to, as a client, over SSH' { #planned }
    steg/
      steg.c4                — steg = softwareSystem 'steg' { description 'Runs programs in terminals, keeps them running without a window, and can be driven from scripts.' }
      _relationships.c4      — container-level "talks to" edges, extend steg { }:
                                server -> programsInPanes 'starts, feeds input to and reads output from' { technology 'pseudo-terminal' }
                                cli -> server 'calls, over its socket, through the public API' { technology 'socket' }
                                app -> server 'calls, over its socket, through the public API' { technology 'socket' }
                                plugin -> server 'calls, over its socket, through the public API' { technology 'socket' }   (plugin's relationship carries #planned too, M2)
      server.c4               — extend steg { server = container 'The server' } — technology 'Rust'; description 'One process per person per machine. Owns every session, tab, pane and running program (R1).'
      cli.c4                  — extend steg { cli = container 'The CLI' } — technology 'Rust'; description 'A short-lived process for one command.'
      app.c4                  — extend steg { app = container 'Steg.app' } — technology 'Rust, GPUI'; description 'The window. Draws what the server reports.'
      plugin.c4               — extend steg { plugin = container 'A plugin' } — technology 'Python'; #planned; description 'One Python process per plugin (M2).'
      server/
        kernel.c4             — extend steg.server { kernel = component 'kernel' } — description 'The server's logic: the tree, layout, focus, status, the terminal engine. No I/O (R105).'
        server-pkg.c4         — extend steg.server { serverPkg = component 'server' } — description 'The kernel plus sockets, pseudo-terminals, processes and the clock. Does I/O (per doc-16's package table).'
        _relationships.c4     — serverPkg -> kernel 'depends on'
      cli/
        sdk.c4                — extend steg.cli { sdk = component 'sdk' } — description 'The public interface: the API description, the shapes that cross the API, and the protocol's logic (R101). No I/O of its own (R102); a consumer supplies the transport. No GPUI type, no particular async runtime, in its public interface (R106). Nested here only because a LikeC4 component has one parent — the kernel, the server package and the app package all depend on it from their own containers; it belongs to none of them (R98).'
        steg-pkg.c4           — extend steg.cli { stegPkg = component 'steg' } — description 'The CLI, and the entry point that starts the server (R98). Built into the server and the CLI — they ship as one binary.'
        _relationships.c4     — stegPkg -> sdk 'depends on'
      app/
        app-pkg.c4            — extend steg.app { appPkg = component 'app' } — description 'The GPUI window. Does I/O.'
      _package-dependencies.c4 — the four cross-container edges from doc-16's dependency table, with a file-level comment explaining why none carries `technology` (compile-time crate dependencies, not runtime calls):
                                steg.server.kernel -> steg.cli.sdk 'depends on'
                                steg.server.serverPkg -> steg.cli.sdk 'depends on'
                                steg.cli.stegPkg -> steg.server.serverPkg 'depends on, as the entry point that starts it (R98)'
                                steg.app.appPkg -> steg.cli.sdk 'depends on'
  views/
    landscape.c4 (scaffold default, left as-is — one system, so it adds nothing beyond Context, but isn't pruned)
    steg/
      context.c4              — view stegContext of steg { title 'System Context — steg'; include steg, personAtMac, programsInPanes, macos, remoteHost }
      containers.c4           — view stegContainers of steg { title 'Containers — steg'; include * }
      packages.c4             — view stegPackages of steg { title 'Components — the packages, and what each may depend on'; include steg.server.*, steg.cli.*, steg.app.* } — children-only predicate; LikeC4 should draw the three container boundaries around their packages automatically. Verified with `npx likec4 start` during the build: if the boundaries don't render, the container elements are added to `include` explicitly and that bend (mixing container and component boxes in one view) is noted on the task, not silently done.
      deployment.c4            — views { deployment view stegDeployment { title 'Deployment — steg'; include ** } }
  dynamic_views/
    cross-system/_scenarios.c4 (scaffold default, left empty — one system)
    steg/
      attach-to-a-pane.c4     — container level: personAtMac -> app 'opens'; app -> server 'asks to attach to the pane, to view it' { technology 'socket' }; server -> app 'sends its current screen and scrollback, then streams further output'
      focus-a-pane.c4         — container level: personAtMac -> cli 'runs `pane focus` against'; cli -> server 'asks to focus a pane' { technology 'socket' }; a step description notes the server itself sets the path upward — the pane's tab becoming current and that tab's session becoming current (R85) — without a separate box, since that would mix levels; server -> app 'reports the pane focused and its tab now current'. The view's own title/description states this never changes which session any window shows (R88) — the exact mistake decision-5 records being made three times.
  deployment/
    steg.c4                   — deployment { environment 'Where steg runs' { node 'A host' { instanceOf steg.app; instanceOf steg.cli; instanceOf steg.server } ; node 'A remote host' { #planned; instanceOf steg.server } } } plus:
                                 steg.app -> steg.server 'a window's client attaches to' (on 'A host') { technology 'socket' }
                                 steg.app -> steg.server 'the same window's second client attaches to' (on 'A remote host') { technology 'SSH'; #planned }
                                 a description on 'A host' notes its app, CLI and server instances are the same `steg` binary for the server+CLI half (R61, R98)
```

## `#planned` tag: verify it renders, don't assume it

`shared/_spec.c4` (copied unmodified from the skill's canonical spec) declares `tag planned` with no attached style. The primary signal for "this is M2 or M3, not M1" is the prose already in each element's description (every M2/M3 element's description says so in words); `#planned` is applied everywhere alongside that prose as a bonus (useful later for `where tag is #planned` predicates), but the plan does not rely on it rendering distinguishably. If a visual distinction turns out to matter once `npx likec4 start` is run during the build, adding a `style` block for the `planned` tag in `shared/_spec.c4` is a one-line follow-up, not assumed here. (`#planned` must be the first thing inside an element's body, before `description`/`technology`.)

## Validation command, and whether it joins `just check`

Unchanged from the first plan:

```
npx --yes likec4@1.61.0 validate architecture
```

Version-pinned (1.61.0 confirmed current via `npm view likec4 version` while planning; the build step re-confirms the pin is still intended, not "whatever's latest"). **It joins `just check`** (R108: "The repository's check validates the model," and `just check` is "what a change must pass before it is called done"), via a new recipe:

```
# the architecture model (architecture/) validates (R108)
architecture:
    @{{quiet}} "architecture" -- npx --yes likec4@1.61.0 validate architecture
```

added to `check`'s recipe line (`check: fmt-check lint boundaries deny test test-tools architecture`). Consequences, named up front rather than surfacing at review:

- **Node becomes a hard requirement for `just check`**, which TASK-1.32 runs on every PR. `plugins/bruk-io-steg/commands.toml`'s `[check]` stanza needs a `"Node.js (npx, for \`just architecture\`)"` line added to `needs`, and its `verified = 2026-11-14` date is stale the moment this merges — re-verifying it is part of this task's build, not a follow-up.
- No skip-if-Node-missing guard, matching the justfile's own rule that a step does its real work or fails; none passes by doing nothing.
- No `.gitignore` change is expected (`npx --yes` resolves into npm's own cache, not a local `node_modules/`); the build step confirms `git status` is clean after running validate once before deciding otherwise.
- `layers.toml` is left unchanged — `steg-boundaries` and `steg-affected` (`tools/src/steg_tools/boundaries.py`, `affected.py`) only read Cargo manifests and crate folders; `architecture/**` matching no layer there breaks neither.
- AC #4 says `just check` must fail when the model does not validate — proved during the build by breaking a `.c4` file on purpose, running `just check`, confirming it fails, then fixing it.

## Proof for the acceptance criteria

- AC #1: `npx --yes likec4@1.61.0 validate architecture` passes; `views/steg/context.c4`, `containers.c4`, `packages.c4` exist, each scoped and titled as above, and the container/package names and the dependency edges are checked by hand against doc-16's Level 2 and Level 3 tables, row by row.
- AC #2: `views/steg/deployment.c4` + `deployment/steg.c4` exist and validate; the two app→server relationship titles each name "a window" and "a client."
- AC #3: `dynamic_views/steg/attach-to-a-pane.c4` and `focus-a-pane.c4` exist, both container-level, both validate, and `focus-a-pane.c4`'s description states R88 explicitly.
- AC #4: `just check` is run once with the model intact (passes) and once with a deliberately broken `.c4` file (fails), both outcomes recorded on the task.
- AC #5: every element's title traces to a word in doc-16's three tables, doc-15's "The things" table, or doc-1 §4's vocabulary table — checked by hand, listed against its source in the build notes.

## What's flagged for the engineer at plan approval

1. doc-16 disagrees with itself on whether "a plugin" is External (Level 1) or a Container (Level 2); this plan resolves it toward Level 2 because AC #1 names it as a container. Confirm, or say which table doc-16 means to fix.
2. `sdk` is nested under `cli` rather than `server` for the reason given in decision 3 above; confirm this reading is acceptable, since no single-parent placement is fully faithful to a package three containers are built from.
<!-- SECTION:PLAN:END -->
