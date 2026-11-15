---
id: TASK-1.33
title: 'Architecture model in LikeC4: context, containers, components and deployment'
status: In Progress
assignee:
  - '@bruk.w2'
created_date: '2026-11-15T01:32:00Z'
updated_date: '2026-11-15T02:52:00Z'
labels:
  - size-2
milestone: m-0
dependencies: []
parent_task_id: TASK-1
type: task
project: bruk-io-steg
asdlc_accepted_from: DRAFT-3
asdlc_branch: task-1.33-architecture-model-in-likec4-context
asdlc_plan_approved_by: bruk
asdlc_plan_approved_hash: '7ade241dd954'
asdlc_plan_approved_size: size-2
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
Revised 2026-11-15, second pass: doc-16's two errors are now fixed (a plugin is External only; a shared package is a component of each container it's built into, not nested under one). This replaces the previous plan in full.

## Where the model lives (unchanged)

A new top-level `architecture/` directory (sibling to `crates/`, `spikes/`, `tools/`), scaffolded once with:

```
python3 <skill-dir>/tools/scaffold.py architecture --system steg
```

`scaffold.py` has real write side effects, no dry-run, and no real `--help` (confirmed while planning; two stray scaffolds it produced outside the worktree were deleted). Run it exactly once, before any element file is written by hand.

## Structural decisions

1. **Three containers, not four.** doc-16 now says plainly: "A plugin is not a container of steg's: anyone can write one, so it is an external system in the context above, and it reaches the server the same way the CLI and the app do." So `steg`'s containers are exactly `server`, `cli`, `app`; `plugin` is an `externalSystem`, declared once, in `models/landscape.c4`, alongside `remoteHost`.
2. **A plugin's edge to the server stays at context level, not container level.** The black-box rule's one carve-out for reaching past a system's boundary is "a person may use a container" — it does not extend to another external system, so `plugin`'s relationship is `plugin -> steg` (black box to black box), matching Level 1's own table; there is no `plugin -> steg.server` edge reaching into a container from outside. (The CLI and the app, by contrast, *are* containers of `steg`, so their own `-> server` edges are ordinary intra-system container relationships, declared where the other container-level edges are.)
3. **`sdk` and `` `steg` `` are each modeled as separate components, once per container doc-16's "Built into" column names — never nested under one container with the others reaching in.** This is AC #2, and it is also the structurally cleaner outcome: because a dependency edge only ever needs to exist between two packages that share a container (that's what "built into" was chosen to guarantee), duplicating `sdk` into `server`, `cli` and `app`, and `` `steg` `` into `server` and `cli`, means **every single dependency edge in the model is intra-container** — none crosses a container boundary, so the "does a compile-time Cargo dependency need a `technology` field" question from the earlier draft never comes up. Each of the 5 duplicated component files (3 for `sdk`, 2 for `` `steg` ``) carries the **same title and the same description**, per doc-16's own wording, and is tagged `#shared`.
4. **The c4-components skill's general heuristic — draw a shared library once with fan-in, not duplicated — is not used here**, even though it's the more common orthodox pattern for a library several components depend on. AC #2 states the opposite explicitly for this model ("appears as a component of each of them, not as the child of one"), and unlike the general case the heuristic is written for, duplicating here has a concrete, stated benefit (point 3: it keeps every edge intra-container). Noted so the asymmetry with ordinary C4 practice is a decision on record, not an oversight.
5. **A new tag, `#shared`, is added to `shared/_spec.c4`.** The scaffolded canonical spec only declares `#external`, `#deprecated` and `#planned`; none of them means "this is the same package drawn more than once." `tag shared` is added next to them, with no style initially (plain, like `planned`) — see the note below on verifying tag styling renders before relying on it visually.
6. **Scope boundary, stated once more:** this task draws `architecture/` only. R104 (the dependency table is the only one enforced), R110 (an I/O lint on the sdk and the kernel) and R111 (one `CLAUDE.md` per package) are all requirements about the real `crates/` workspace and its checks — none of them is a LikeC4 modeling concern, and none of them is touched here. Today's `crates/` (`crates/kernel`, `crates/server`, `crates/cli`, `crates/app`, `crates/steg`) doesn't yet match doc-16's five packages (no separate CLI crate, a new `sdk` crate); reconciling that, and R104/R110/R111's enforcement, is TASK-1.69's job, not this one.

## File-by-file plan

```
architecture/
  models/
    landscape.c4
      — personAtMac = person 'The person at the Mac'
      — programsInPanes = externalSystem 'Programs in panes' { description 'A shell, an editor, an agent. steg starts them, feeds them input and reads their output. Some drive steg back through the CLI.' }
      — macos = externalSystem 'macOS' { description 'Windows, pseudo-terminals, the clipboard, the system's light or dark setting.' }
      — plugin = externalSystem 'A plugin' { #planned description 'A Python program beside the server, using the public API (M2).' }
      — remoteHost = externalSystem 'A remote host' { #planned description 'Another machine running a steg server (M3).' }
      — personAtMac -> steg 'uses, through its window and its CLI'
      — steg -> programsInPanes 'starts, feeds input to and reads output from'
      — programsInPanes -> steg 'drives back, through the CLI'
      — steg -> macos 'opens windows on, allocates pseudo-terminals from, and reads the system theme from'
      — plugin -> steg 'calls the public API of, as a Python program beside the server' { #planned }
      — steg -> remoteHost 'connects to, as a client, over SSH' { #planned }
    steg/
      steg.c4                — steg = softwareSystem 'steg' { description 'Runs programs in terminals, keeps them running without a window, and can be driven from scripts.' }
      _relationships.c4      — the three container-level "talks to" edges, extend steg { }:
                                server -> programsInPanes 'starts, feeds input to and reads output from' { technology 'pseudo-terminal' }
                                cli -> server 'calls, over its socket, through the public API' { technology 'socket' }
                                app -> server 'calls, over its socket, through the public API' { technology 'socket' }
      server.c4               — extend steg { server = container 'The server' } — technology 'Rust'; description 'One process per person per machine. Owns every session, tab, pane and running program (R1).'
      cli.c4                  — extend steg { cli = container 'The CLI' } — technology 'Rust'; description 'A short-lived process for one command.'
      app.c4                  — extend steg { app = container 'Steg.app' } — technology 'Rust, GPUI'; description 'The window. Draws what the server reports.'
      server/
        kernel.c4             — extend steg.server { kernel = component 'kernel' } — description 'The server's logic: the tree, layout, focus, status, the terminal engine. No I/O (R105).'
        server-pkg.c4         — extend steg.server { serverPkg = component 'server' } — description 'The kernel plus sockets, pseudo-terminals, processes and the clock. Does I/O.'
        sdk.c4                — extend steg.server { sdk = component 'sdk' } — #shared; description 'The public interface: the API description, the shapes that cross the API, and the protocol's logic (R101). No I/O of its own (R102); a consumer supplies the transport. Built into the server, the CLI and the app alike — it belongs to none of them more than another (doc-16).'
        steg-pkg.c4           — extend steg.server { stegPkg = component 'steg' } — #shared; description 'The CLI, and the entry point that starts the server (R98). Built into the server and the CLI, which ship as one binary.'
        _relationships.c4     — kernel -> sdk 'depends on'; serverPkg -> kernel 'depends on'; serverPkg -> sdk 'depends on'; stegPkg -> sdk 'depends on'; stegPkg -> serverPkg 'depends on, as the entry point that starts it (R98)'
      cli/
        sdk.c4                — extend steg.cli { sdk = component 'sdk' } — #shared; same description as steg.server.sdk, word for word
        steg-pkg.c4           — extend steg.cli { stegPkg = component 'steg' } — #shared; same description as steg.server.stegPkg, word for word
        _relationships.c4     — stegPkg -> sdk 'depends on'
      app/
        sdk.c4                — extend steg.app { sdk = component 'sdk' } — #shared; same description as the other two, word for word
        app-pkg.c4            — extend steg.app { appPkg = component 'app' } — description 'The GPUI window. Does I/O.'
        _relationships.c4     — appPkg -> sdk 'depends on'
  views/
    landscape.c4 (scaffold default, left as-is — one system, so a Landscape view adds nothing beyond Context, but isn't pruned)
    steg/
      context.c4              — view stegContext of steg { title 'System Context — steg'; include steg, personAtMac, programsInPanes, macos, plugin, remoteHost }
      containers.c4            — view stegContainers of steg { title 'Containers — steg'; include * } — three containers plus the connected person/externals
      packages.c4              — view stegPackages of steg { title 'Components — the packages, and what each may depend on'; include steg.server.*, steg.cli.*, steg.app.* } — pulls in all eight component instances (kernel, server, sdk×3, steg×2, app) with their three container boundaries drawn automatically. Verified with `npx likec4 start` during the build; if the boundaries don't render on their own, the three containers are added to `include` explicitly and that's noted as a bend, not done silently.
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

## `#planned`/`#shared`: verify, don't assume

Neither tag has a `style` block in the scaffolded canonical spec, so neither is guaranteed to render distinguishably by default (`#shared` is new; `#planned` was already unstyled in the first draft). The primary signal for both stays in prose — every M2/M3 element's description says so in words, and every shared component's description says "Built into..., it belongs to none of them more than another," matching doc-16 verbatim. The tags are applied everywhere alongside that prose as a bonus (useful later for `where tag is #planned` predicates), not relied on visually. If `npx likec4 start` during the build shows no visual distinction and one would help, adding `style` blocks for `planned` and `shared` in `shared/_spec.c4` is a one-line follow-up, not assumed here. Both tags must be the first thing inside an element's body, before `description`/`technology`; an element needing both (none currently do) would write `#planned #shared` together, first.

## Validation command, and whether it joins `just check` (unchanged)

```
npx --yes likec4@1.61.0 validate architecture
```

Version-pinned (1.61.0 confirmed current via `npm view likec4 version` while planning; the build step re-confirms the pin is still intended). **It joins `just check`** (R108: "The repository's check validates the model," and `just check` is "what a change must pass before it is called done"), via a new recipe:

```
# the architecture model (architecture/) validates (R108)
architecture:
    @{{quiet}} "architecture" -- npx --yes likec4@1.61.0 validate architecture
```

added to `check`'s recipe line (`check: fmt-check lint boundaries deny test test-tools architecture`). Consequences, named up front:

- **Node becomes a hard requirement for `just check`**, which TASK-1.32 runs on every PR. `plugins/bruk-io-steg/commands.toml`'s `[check]` stanza needs `"Node.js (npx, for \`just architecture\`)"` added to `needs`, and its `verified = 2026-11-14` date needs re-stamping as part of this task's build.
- No skip-if-Node-missing guard, matching the justfile's rule that a step does its real work or fails.
- No `.gitignore` change is expected (`npx --yes` resolves into npm's own cache, not a local `node_modules/`); the build step confirms `git status` is clean after running validate once before deciding otherwise.
- `layers.toml` is left unchanged — `steg-boundaries` and `steg-affected` only read Cargo manifests and crate folders; `architecture/**` matching no layer breaks neither.

## Proof for the acceptance criteria

- AC #1: `npx --yes likec4@1.61.0 validate architecture` passes; `context.c4` includes `plugin` and `remoteHost` as external systems and no plugin container exists; `containers.c4` shows exactly three containers; `packages.c4` shows the five packages and the seven dependency edges, checked row by row against doc-16's two Level 3 tables.
- AC #2: `sdk` exists as three separate component files (`server/sdk.c4`, `cli/sdk.c4`, `app/sdk.c4`) and `` `steg` `` as two (`server/steg-pkg.c4`, `cli/steg-pkg.c4`) — none nested under a single container with the others pointed at it.
- AC #3: `views/steg/deployment.c4` + `deployment/steg.c4` exist and validate; the two app→server relationship titles each name "a window" and "a client."
- AC #4: `dynamic_views/steg/attach-to-a-pane.c4` and `focus-a-pane.c4` exist, both container-level, both validate, and `focus-a-pane.c4`'s description states R88 explicitly.
- AC #5: `just check` is run once with the model intact (passes) and once with a deliberately broken `.c4` file (fails), both outcomes recorded on the task.
- AC #6: every element's title traces to a word in doc-16's tables, doc-15's "The things" table, or doc-1 §4's vocabulary table — checked by hand, listed against its source in the build notes.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Revised plan checked against doc-16 by @bruk.o: ok. Three containers, a plugin and a remote host external, the five packages as components with a shared package drawn in each container that builds it, doc-16's dependency table as the only edges, and the model validated in just check. For the engineer: validating in just check makes Node a requirement of every check run, CI included; and the c4 skill's usual advice is to draw a shared library once, which the plan sets aside because doc-16 says a shared package belongs to each container.
<!-- SECTION:NOTES:END -->

## Comments

<!-- COMMENTS:BEGIN -->
author: @bruk
created: 2026-11-15T02:52:00Z

---

Plan approved by @bruk (7ade241dd954, size-2): its worker may build it.

---
<!-- COMMENTS:END -->
