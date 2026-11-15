---
id: TASK-1.33
title: 'Architecture model in LikeC4: context, containers, components and deployment'
status: In Progress
assignee:
  - '@bruk.w2'
created_date: '2026-11-15T01:32:00Z'
updated_date: '2026-11-15T02:34:00Z'
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
- [ ] #1 A LikeC4 model in the repository has the system context, the containers (the server, the CLI, Steg.app, a plugin) and the components (sdk, kernel, server, `steg`, app) exactly as doc-16 gives them, including which package may depend on which
- [ ] #2 A deployment view shows one window holding a client to a local server and another to a remote one
- [ ] #3 Two dynamic views: attaching to a pane, and focusing a pane with the path updating upward
- [ ] #4 `just check` validates the model and fails when it does not validate (R108)
- [ ] #5 Every name in the model is a word doc-16, doc-15 or doc-1's vocabulary uses, with the same meaning
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
## Where the model lives

A new top-level directory `architecture/` (sibling to `crates/`, `spikes/`, `tools/`), scaffolded with the `c4-architecture:likec4-diagrams` skill's `scaffold.py`:

```
python3 <skill-dir>/tools/scaffold.py architecture --system steg
```

(Not `docs/`: board docs already live in `backlog/docs/`, and a second `docs/` would be confused with that.) This writes `.likec4rc`, `shared/_spec.c4` (the unmodified canonical spec — no custom kinds are needed), `models/landscape.c4`, `views/landscape.c4`, `dynamic_views/cross-system/_scenarios.c4`, and the `models/steg/`, `views/steg/`, `dynamic_views/steg/` folders. `scaffold.py` has real write side effects and no dry-run and no real `--help` (confirmed while planning: `-h`/`--help` is swallowed as the target-dir argument and a scaffold gets written at that literal path — do not pass either flag to it; two stray `--help`/`-h` directories it produced while planning were already deleted). Run it exactly once, idempotently, as the first build step, before any element file is written by hand.

## The four structural decisions (own these before writing elements, so a reviewer doesn't have to ask)

1. **Kernel and the steg library are components, never containers.** doc-1 §7 calls the kernel "a Rust library" and doc-15 calls it "the library that holds state and has no socket and no window" — not separately deployable. The steg library (R101) is likewise a library "no server logic," built into the CLI and the app. AC #1 names exactly four containers — app, server, CLI, a plugin — and that is the full container list. The kernel is a component of the server; the steg library is a component that appears once inside the CLI's description and once as a real component in the app's component view (the app is the one that gets a formal component view per AC #1).
2. **The one binary (R61/R98) is a deployment fact, not a reason to merge containers.** Server and CLI stay two containers — two process spaces with different lifecycles (R99 vs R33-35) — even though `cargo build` produces one executable (`crates/steg`). The fact is carried in prose (container descriptions, and a note on the deployment node), not in the model's shape.
3. **Window and Client are never boxes at container or deployment level.** decision-5: a Client is one connection to one server; a Window is drawn by the app and belongs to it, not to any server. Both are real **components inside the app's own component view** (that is in-process grouping, which is exactly what a component is for), but neither becomes a container, an external system, or a deployment node. AC #2's "one window holding a client to a local server and another to a remote one" is satisfied by two relationships between deployed **app** and **server** instances, titled to say exactly that (e.g. `'the window's second client attaches to, over SSH'`) — not by drawing Window/Client boxes on the deployment diagram.
4. **AC #4's vocabulary is doc-15's "The things" table (Server, Session, Tab, Pane, Program, Status, Client) + doc-1 §4's vocabulary table (Host, Window, Focus, Action, Event, and the rest) + the exact words doc-1 §7's Parts table and doc-8's own section headings use for the parts AC #1 requires by name (Kernel, Server, the steg library, CLI, Steg.app, Plugins, and "API" from doc-8's "### API" heading, "socket" from R5).** No element invents a synonym for any of these (no "Connection" for Client, no "Viewport" for Window, no "Local socket" — R5 just says "socket," so the component is titled `Socket`, not embellished). This reading is a judgment call, flagged below for the engineer: AC #1 and a literal "only doc-1 §4's table" reading of AC #4 would otherwise conflict, since §4 alone has no word for Server, App, CLI or Plugin.

## File-by-file plan (one element per file, children via `extend <full FQN>`, per the skill's file-layout rule)

```
architecture/
  models/
    landscape.c4                        — bruk = person 'Bruk' (doc-1 §2); bruk -> steg.app and bruk -> steg.cli, titled from doc-1 §1/§3, no technology (person -> container UI edges don't need one)
    steg/
      steg.c4                           — steg = softwareSystem 'Steg' (doc-1 §1's one-paragraph description)
      _relationships.c4                 — steg.app -> steg.server, steg.cli -> steg.server, steg.plugin -> steg.server, each { technology 'socket' } (R5); titles name what each client does (R98)
      server.c4                         — extend steg { server = container 'Server' } — technology 'Rust'; description from R1, R98, R61
      app.c4                            — extend steg { app = container 'Steg.app' } — technology 'Rust, GPUI'; description from R20, R21, doc-1 §7
      cli.c4                            — extend steg { cli = container 'CLI' } — technology 'Rust'; description from R32, R61, notes it is built on the steg library like the app (R101)
      plugin.c4                         — extend steg { plugin = container 'Plugin' } — technology 'Python'; description from doc-1 §7 and R47, one process per plugin; tag #planned (M2, doc-1 release table)
      server/
        kernel.c4                       — extend steg.server { kernel = component 'Kernel' } — technology 'Rust library, steg-kernel'; description from doc-15 R93-R97, R85-R91, R10, R91 (whose client's size a terminal takes), and the architecture rule ("logic only, no I/O")
        socket.c4                       — extend steg.server { socket = component 'Socket' } — description from R5, R39, R99 (on-demand start, detach, handshake, version check, idle stop)
        api.c4                          — extend steg.server { api = component 'API' } — description from R36-R39 (self-describing, versioned, routes to the kernel, publishes events)
        _relationships.c4               — socket -> api 'hands each request to, once the handshake is done'; api -> kernel 'reads and changes sessions, tabs and panes through'; kernel -> api 'tells of changes, for the API to publish as events (R38)'
      app/
        window.c4                       — extend steg.app { window = component 'Window' } — description from R20, R87 (holds one client per server, shows one session at a time)
        client.c4                       — extend steg.app { client = component 'Client' } — description from R86 (one connection to one server, views zero or more sessions)
        steg-library.c4                 — extend steg.app { stegLibrary = component 'Steg library' } — description from R101
        _relationships.c4               — window -> client 'holds one per server it uses, and shows the session one of them views (R87)'; client -> stegLibrary 'uses, to find or start a server, connect, call a method and subscribe to events (R101)'
  views/
    steg/
      context.c4                        — view stegContext of steg { include steg, bruk } — AC #1's system context view
      containers.c4                     — view stegContainers of steg { include * } — pulls in bruk via the connected-neighbors rule; AC #1's container view
      components-server.c4              — view of steg.server { include * } — AC #1's server component view
      components-app.c4                 — view of steg.app { include * } — AC #1's app component view
      deployment.c4                     — views { deployment view stegDeployment { include ** } } — AC #2, reads the nodes from architecture/deployment/steg.c4
  dynamic_views/
    steg/
      attach-to-a-pane.c4               — AC #3, container level: bruk -> app 'opens'; app -> server 'asks to attach to the pane, to view it' { technology 'socket' }; server -> app 'sends its current screen and scrollback, then streams further output'
      focus-a-pane.c4                   — AC #3, container level: bruk -> cli 'runs `pane focus` against'; cli -> server 'asks to focus a pane' { technology 'socket' }; a step description (not a separate box — would mix levels) notes the server itself sets the path upward, the pane's tab becoming current and that tab's session becoming current (R85); server -> app 'reports the pane focused and its tab now current' (R38). The view's own description states explicitly that this never changes which session any window shows (R88) — the exact mistake decision-5 records being made three times.
  deployment/
    steg.c4                             — deployment { environment ... { node 'Bruk's Mac' { instanceOf steg.app; instanceOf steg.cli; instanceOf steg.server } ; node 'A remote Mac' { instanceOf steg.server } } } plus the two relationships AC #2 needs: app -> server (same node, local) and app -> server (remote node) { technology 'SSH' } (R55), each titled to say which of the window's two clients it is. The remote node and its edge are tagged #planned (M3, doc-13); a node description notes the local server and CLI instances are the same `steg` binary (R61, R98).
```

`views/landscape.c4` (scaffold default) and `dynamic_views/cross-system/_scenarios.c4` are left as scaffolded (there is only one software system, so a System Landscape view adds nothing beyond the Context view and is not one of AC #1-3's required views; it is not deleted because `scaffold.py` is idempotent and re-running it should not need re-pruning).

## Validation command, and whether it joins `just check`

```
npx --yes likec4@1.61.0 validate architecture
```

Version-pinned literally in the command (1.61.0 is current as of today, confirmed via `npm view likec4 version`; the build step re-confirms this is still the intended pin, not latest-at-build-time — unpinned `npx likec4` would let a future breaking release silently break CI). This is the command recorded as the architecture model's proof; the skill's own `c4lint.py` (which additionally enforces this skill's authoring rules — kinds, nesting, titles, technology, level-mixing) runs only during the build, by hand, from its path under the plugin cache — it is not persisted into the repo or the justfile, because that path does not exist in CI or on another contributor's machine.

**It joins `just check`.** Add a recipe:

```
# the architecture model (architecture/) validates
architecture:
    @{{quiet}} "architecture" -- npx --yes likec4@1.61.0 validate architecture
```

and add `architecture` to `check`'s recipe line (`check: fmt-check lint boundaries deny test test-tools architecture`). Justification: `just check` is defined as "what a change must pass before it is called done," and every one of its existing steps except `test` already runs unconditionally on every invocation regardless of the diff (fmt-check, lint, boundaries, deny) — there is direct precedent for an always-run structural check, and a model nobody is forced to keep passing is how it rots silently.

Consequences to flag for the engineer at approval, not hidden until review:

- **Node becomes a hard requirement for `just check`**, which `just-check-on-every-pr` (TASK-1.32) runs on every PR. `plugins/bruk-io-steg/commands.toml`'s `[check]` stanza needs `"Node.js (npx, for \`just architecture\`)"` added to `needs`, and its `verified = 2026-11-14` date is stale the moment this merges — re-verifying it (`asdlc-plugin verify-command bruk-io-steg check`) is a step in this task's build, not a follow-up.
- No skip-if-Node-missing guard: the justfile's own header says a step does its real work or fails, none passes by doing nothing.
- No `.gitignore` change is expected: `npx --yes` resolves into npm's own cache outside the repo, not into a local `node_modules/`. The build step confirms `git status` is clean after running validate once, and only then adds a `.gitignore` line if something unexpected was left behind.
- `layers.toml` is left unchanged. `steg-boundaries` reads only Cargo crate manifests and `steg-affected` only picks crate test selection from `Cargo.toml`/`Cargo.lock`/crate folders; `architecture/**` matching no layer there does not break either check (confirmed by reading `tools/src/steg_tools/boundaries.py` and `affected.py`).

## Proof for the acceptance criteria

- AC #1: `npx --yes likec4@1.61.0 validate architecture` passes, and `views/steg/context.c4`, `containers.c4`, `components-server.c4`, `components-app.c4` exist and each render on `npx likec4 start` (checked by hand during build, not persisted as a check).
- AC #2: `views/steg/deployment.c4` + `deployment/steg.c4` exist and validate; the two app→server relationship titles name "the window" and "a client" on each edge.
- AC #3: `dynamic_views/steg/attach-to-a-pane.c4` and `focus-a-pane.c4` exist, both container-level, both validate.
- AC #4: every element name checked by hand against doc-15's table, doc-1 §4's table, and doc-1 §7's Parts table / doc-8's "API" and "socket," as the four structural decisions above state; no element is tagged or named from any other source.

## Risk flagged for the engineer

AC #4's exact scope (doc-1 §4's literal Vocabulary section alone vs. doc-1's words generally, including §7's Parts table) is read generously above because a literal-§4-only reading cannot satisfy AC #1's named containers (app, server, CLI, plugin — none of which doc-1 §4 defines). Worth a one-line confirmation at plan approval rather than a review finding.
<!-- SECTION:PLAN:END -->
