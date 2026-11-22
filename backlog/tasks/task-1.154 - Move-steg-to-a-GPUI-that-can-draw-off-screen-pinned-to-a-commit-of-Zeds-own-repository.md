---
id: TASK-1.154
title: >-
  Move steg to a GPUI that can draw off screen, pinned to a commit of Zed's own
  repository
status: In Progress
assignee:
  - '@bruk.w5'
created_date: '2026-11-22T07:18:00Z'
updated_date: '2026-11-22T20:20:00Z'
labels:
  - size-3
milestone: m-4
dependencies: []
parent_task_id: TASK-1
project: bruk-io-steg
asdlc_accepted_from: DRAFT-3
asdlc_branch: task-1.154-move-steg-to-a-gpui-that-can-draw-off
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Split out of TASK-1.151 by its plan (22 November 2026). TASK-1.151's spike showed that drawing off screen and reading the frame back works on a newer GPUI (HeadlessAppContext::capture_screenshot gave a PNG with exact pixels, no window shown) and is closed on the pinned gpui 0.2.2, whose test window does not draw. The official gpui crate on crates.io is still 0.2.2. The spike used gpui-pre 0.3.13, a republish owned by one account that is not Zed's; two of its files were checked byte-identical to zed-industries/zed at the commit it claims, which is a spot check, not an audit. Recommended route, for the engineer to confirm on accepting: a git dependency pinned to that commit of Zed's own repository, so no third party sits in the chain, at the cost of a large first clone (the repository is about 546 MiB) and of tracking an unreleased GPUI. gpui-pre is the fallback only if the git source proves unworkable, and choosing it comes back to the engineer first. Not established: whether steg-boundaries handles a git source; the full size of the API difference (public functions in window, app and canvas looked additive in a diff). Licence rule: GPUI is Apache-2.0; nothing is taken from Zed's GPL crates, and a git dependency on the monorepo must not pull any of them into the build - the plan shows that from the licence check's own output.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The workspace builds against GPUI from zed-industries/zed at one pinned commit, named in the root Cargo.toml with the reason; the app and the gallery build and every existing test passes; just check passes
- [ ] #2 The licence check and the boundaries check both cope with the new source and its split platform crates, with every new boundaries.toml entry given its reason; no exception is added to either check to make it pass
- [ ] #3 The off-screen drawing support is reachable only from a test or tool target: the shipped app binary is shown not to carry the test-support feature, by a check that fails if it does
- [ ] #4 The notes list every source change the move forced in crates/app and crates/ui, and anything in the newer GPUI that changes how the app looks or behaves; the engineer does a live run before merge
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
## Decision 1 (commit): pin to the commit tagged v1.29.3

v1.29.3 (2026-11-20) is the newest stable (non "-pre") tag in zed-industries/zed.
Its commit is `d01b134cddf5e4f71964deb2d6f81cdb38fcf1e7`. `crates/gpui/src/app/
headless_app_context.rs` at that commit is byte-identical to the file at f816e1f
(the commit that added it, 2026-11-01, "gpui: Render macOS window screenshots
offscreen (#66344)") - confirmed with `diff`, exit 0 - and `git fetch --shallow-
since` from this worker's machine confirms f816e1f is a real ancestor of v1.29.3,
not just chronologically later. This satisfies the task's own preference for a
release-tagged commit. Pin with `rev`, not `tag`: Cargo.toml names the commit
("# v1.29.3"), so the dependency is reproducible even if the tag ref ever moved.

## Files that change

- `Cargo.toml` (root): replace `gpui = { version = "=0.2.2" }` with a git
  dependency pinned to `rev = "d01b134cddf5e4f71964deb2d6f81cdb38fcf1e7"`
  (`# v1.29.3`), `default-features = false` (drops `wayland`/`x11`/
  `windows-manifest` - no-ops on macOS: `gpui_linux`/`embed-resource` aren't
  reached without them, since this workspace's `deny.toml` already restricts
  the licence graph to macOS targets and `gpui_linux` is itself target-gated
  out). Add a second new workspace dependency, `gpui_platform`, same `rev`,
  `default-features = false, features = ["font-kit"]` - a real crate in the
  same monorepo (confirmed: `[dev-dependencies] gpui_platform = { workspace =
  true, ... }` in gpui's own `Cargo.toml`), not an invention of the `gpui-pre`
  republish TASK-1.151's spike used; it holds `current_platform`/
  `current_headless_renderer`/`application()`, none of which gpui re-exports
  itself. `font-kit` is explicitly turned on: it is OFF by default at every
  level of the chain (`gpui_platform`'s own default is `["multithreaded"]`
  only; `gpui_macos`'s default forwards to `gpui/default`, not its own
  `font-kit`) and is what actually wires real system font matching on macOS
  (`gpui_macos/font-kit` -> `gpui_apple/font-kit` -> the `zed-font-kit` crate,
  a zed-industries fork, plus `core-text`/`objc2-core-text`/
  `pathfinder_geometry`) - left off, steg's own text shaping would silently
  lose whatever font-kit's matching gives it, which is exactly the kind of
  look-or-behaviour change AC#4 asks to be caught, not shipped by accident.
  Comment explains the pin, the provenance reason (a git dependency on Zed's
  own repository, not the `gpui-pre` registry republish - no third-party
  account in the chain) and the feature choice.
- `Cargo.lock`: regenerated by Cargo once the git source resolves (confirmed
  below it resolves clean, 553 packages, ~210s/~660MiB on a cold cache).
- `boundaries.toml`: `[steg-ui]` and `[steg-app]` each add `gpui_platform` to
  `allow`, with the reason (both crates' own `main.rs` must call
  `gpui_platform::application()` in place of the removed `Application::new()`
  - below). `[steg-ui]`'s existing reason text ("gpui is the one external
  dependency... decision-11") becomes inaccurate with a second one added;
  the plan notes this for the engineer, who owns decision-11, rather than
  silently rewording a decision record's own rationale. No `deny-features`
  entry for `gpui_platform`'s `test-support`: `steg-boundaries` reads
  declared features off every dependency kind alike (its own module doc:
  "Reads each crate's *declared* dependency edges... unaffected by Cargo's
  workspace-wide feature unification" - it does not distinguish `[dependencies]`
  from `[dev-dependencies]` either), so a `deny-features` rule here would also
  refuse the dev-dependency edge this task adds on purpose. Isolation is
  Cargo's own dependency-kind separation instead (below).
- `deny.toml`: no new entries needed - confirmed by running the real
  `cargo deny check licenses` against this exact resolved graph (below); the
  existing allow list already covers everything the new tree needs,
  including `cbindgen`'s `MPL-2.0` (its own comment already names cbindgen
  as a build-only dependency, and that is exactly what pulls it in again
  here, for `gpui_apple`'s Metal shader codegen). One comment goes stale and
  should be corrected: the `CC0-1.0` entry's comment cites "hexf-parse, via
  gpui's renderer" - `hexf-parse` is not in the new tree at all (confirmed:
  absent from this resolve's `Cargo.lock`). Re-check with the real full
  workspace resolve (ghostty-vt and the rest still in the graph) whether
  anything else still needs `CC0-1.0`; reword or drop the comment, never the
  allow entry itself without the same confirmation.
- `crates/app/Cargo.toml`: add `gpui_platform.workspace = true` to
  `[dependencies]` (the real app's own platform, no `test-support`); add a
  second entry for the same crate to a new `[dev-dependencies]` table,
  `gpui_platform = { workspace = true, features = ["test-support"] }` (legal
  in Cargo: the same dependency named twice across the two tables, each with
  its own feature set); add `[[example]] name = "headless_smoke" path =
  "examples/headless_smoke.rs"`.
- `crates/app/src/main.rs`: `Application::new().run(...)` ->
  `gpui_platform::application().run(...)` - `Application::new()` does not
  exist at the pinned commit (only `with_platform`, `new_inaccessible`,
  `run`, `run_embedded`); `gpui_platform::application()` is upstream's own
  one-line wrapper for `Application::with_platform(current_platform(false))`,
  confirmed by its own doc comment showing exactly this call. Add
  `use gpui_platform;` (or call it qualified). Every other `gpui::` symbol
  this file imports (`App`, `AsyncWindowContext`, `Bounds`, `Context`,
  `EntityInputHandler`, `FocusHandle`, `Focusable`, `IntoElement`,
  `KeyDownEvent`, `ParentElement`, `Pixels`, `Render`, `Styled`,
  `Subscription`, `Task`, `TitlebarOptions`, `UTF16Selection`, `Window`,
  `WindowAppearance`, `WindowBounds`, `WindowOptions`, `div`, `prelude::*`,
  `px`, `size`, plus `Modifiers`/`Keystroke` in the test module) is confirmed
  present and public at the pinned commit - no other change forced here.
- `crates/app/examples/headless_smoke.rs` (new): a minimal port of this
  task's own scratch spike into the real workspace - `HeadlessAppContext`
  opens an unshown window (`show: false`, confirmed in the real
  `open_window`), paints a solid-colour square, `capture_screenshot` reads
  real pixels back, asserted against the known colour. Its only job is to
  prove AC#3's mechanism (below); TASK-1.151 owns building the real scenario
  harness on top of it and may restructure this file freely once it starts.
- `crates/app/src/terminal_element.rs`: no source change - every imported
  symbol (`ContentMask`, `ElementInputHandler`, `FontStyle`, `FontWeight`,
  `ShapedLine`, `StrikethroughStyle`, `TextRun`, `UnderlineStyle`, `canvas`,
  `fill`, `point`, `px`, plus the rest) confirmed present and public at the
  pinned commit, including `ShapedGlyph`'s `id: GlyphId` field, which is
  itself `pub` now - TASK-1.136's finding (`GlyphId`'s inner field
  unreachable) was against the pinned 0.2.2 and does not hold here, though
  steg's own code never reads `GlyphId`'s inner value anyway, so this is
  informational, not load-bearing for this task.
- `crates/app/src/keys.rs`, `crates/app/src/metrics.rs`: no source change
  (`Modifiers`; `Bounds`, `Pixels`, `SharedString`, `Window`, `px` all
  confirmed present and public).
- `crates/ui/Cargo.toml`: add `gpui_platform.workspace = true` to
  `[dependencies]` (the gallery's own `main.rs` needs it, same reason as
  `steg-app`; no `test-support`, since the gallery captures a real window via
  OS-level `screencapture`, unrelated to this task's headless path).
- `crates/ui/gallery/main.rs`: `Application::new().run(...)` ->
  `gpui_platform::application().run(...)`, same reason as `crates/app`. No
  other symbol in its `use gpui::{...}` block is affected.
- `crates/ui/src/foundations/mod.rs`, `crates/ui/src/foundations/
  generated.rs`: no source change (`Global`, `Hsla`, `Pixels`, `px` all
  confirmed present and public).

## Order of work

1. In the worktree, change the root `Cargo.toml`'s `gpui` entry and add
   `gpui_platform`, as above. Run `cargo generate-lockfile` (or `cargo
   metadata`) alone first - this is the ~660MiB/~210s one-time clone cost,
   cheaper to hit once here than discover mid-build.
2. Fix the two forced call sites (`crates/app/src/main.rs`,
   `crates/ui/gallery/main.rs`) and the two `Cargo.toml`s' `[dependencies]`
   additions. Run `cargo check --workspace` (not yet `just check`, to get a
   fast compile-error signal before the slower checks).
3. Add `boundaries.toml`'s two new `allow` entries with their reasons. Run
   `just check-boundaries` (`steg-boundaries`) - must pass with no exception
   added to it.
4. Add `crates/app`'s `[dev-dependencies]` entry and `[[example]]` target,
   and `examples/headless_smoke.rs` itself. Run the example directly
   (`cargo run --example headless_smoke -p steg-app`) and confirm the
   captured pixels match the known colour, exactly as this task's own scratch
   spike already proved for the API shape.
5. Run `cargo deny check licenses` for real, against the real workspace (not
   this task's scratch probe). If anything outside the already-confirmed
   `gpui*`/font-kit-chain crates is rejected, resolve it by narrowing
   features first (never by widening `deny.toml`'s allow list without the
   same reasoning this plan gives for every existing entry). Re-check the
   `CC0-1.0` comment's `hexf-parse` attribution against the real resolve and
   correct it if stale.
6. Run `just check` end to end (this change touches `Cargo.toml`,
   `Cargo.lock`, `boundaries.toml`, `deny.toml` - all in `steg-affected`'s
   `GLOBAL_FILES` - so `just test`'s own selection is `all`; say so plainly
   rather than claim a narrower one).
7. Record the AC#3 proof pair (below) and the licence negative control
   (below) at the final head.

## Risks

- **TASK-1.151's own decision 2 (a second `[[bin]]` in `crates/app` for the
  scenario runner) does not hold**, and should not be read as validated by
  this task. `boundaries.toml`'s own comment already states the mechanism
  this plan relies on: "a fixture is a `[[bin]]` target, which only ever sees
  `[dependencies]`, never `[dev-dependencies]`" - the inverse is just as
  true, a `[[bin]]` can *only* see `[dependencies]`, so a second `[[bin]]`
  needing `test-support` would force that feature into `crates/app`'s single
  `[dependencies]` table, reaching the shipped `steg-app` binary too (Cargo
  resolves features per *package*, not per target). The `gallery` precedent
  TASK-1.151 cites doesn't transfer: `gallery` lives in `steg-ui` and turns
  on no feature at all. This task uses `[[example]]` instead, which Cargo
  gives `[dev-dependencies]` access to for free, and that is the actual
  mechanism AC#3 needs. Flagging this for the engineer rather than
  resolving it here: TASK-1.151's plan needs a revised decision 2 (a
  separate workspace member, or an `[[example]]`/optional-dep-plus-
  `required-features` shape) once it reaches its own build phase.
- The full licence audit here used a minimal two-dependency probe
  (`gpui` + `gpui_platform` alone), not steg's full workspace (ghostty-vt,
  portable-pty, etc. are not in it). `cargo deny check licenses` passed
  clean against it, every `gpui_*` crate confirmed `Apache-2.0` directly
  from each one's own `Cargo.toml` at the pinned commit, and every other new
  transitive crate spot-checked (`zed-font-kit`, `core-text`, `dispatch2`,
  `pathfinder_geometry`, `mach2`, `cbindgen`, `resvg`, `usvg`, `accesskit`,
  `etagere`, `derive_more`, `icu_segmenter`, `inventory`, `profiling`,
  `scheduler`, `seahash`) is permissive. The real build task's own run
  against the full workspace (step 5) is still the actual proof, not this
  probe - carried forward as the honest caveat, same as TASK-1.151's spike
  stated for its own `gpui-pre` run.
- `gpui`'s own `Cargo.toml` at this commit still declares `version = "0.2.2"`
  (its internal version number hasn't been bumped past the crates.io
  release). A dependency spec that kept `version = "=0.2.2"` alongside a
  `git` source would silently still resolve - the fix is to drop the version
  requirement entirely and let `rev` alone pin it, which is what this plan's
  `Cargo.toml` entries do; calling this out explicitly so a reviewer checks
  it rather than assumes it.
- Spot check, not an audit (carried forward from TASK-1.151): only two files
  were diffed byte-for-byte against upstream before this task started
  (`scene.rs`, `headless_app_context.rs`); this task adds a third exact
  match (`headless_app_context.rs` again, now against the v1.29.3 tag) but
  does not audit the whole monorepo.
- `windows-manifest`'s `embed-resource` build-dependency is not target-gated
  in `gpui`'s own `Cargo.toml` (only feature-gated), so leaving `gpui`'s
  defaults on would have compiled it uselessly on macOS; `default-features =
  false` avoids this, confirmed by the real resolve/build graph not listing
  it.

## Proof per acceptance criterion

- **AC#1** (workspace builds against the pinned commit; app and gallery
  build; every existing test passes; `just check` passes): recorded `just
  check` at the final head. The pin and its reason are in the root
  `Cargo.toml`'s own comment, reviewable directly.
- **AC#2** (licence and boundaries checks cope, with every new entry's own
  reason; no exception added to either): `just check-boundaries` recorded at
  the final head, plus the two new `boundaries.toml` entries' own reasons as
  the reviewable artefact (item 3 - "does `steg-boundaries` cope with a git
  source" - is answered both from `steg_tools.affected.crates_from_metadata`'s
  own source, which keys purely on `cargo metadata`'s `d["name"]` and never
  reads `d.get("path")` as true for a git source, and from an actual `cargo
  metadata --no-deps` run against a real git-sourced probe dependency in this
  task's own scratch project, confirming `{"name": "gpui", "source": "git+
  https://github.com/zed-industries/zed?tag=v1.29.3", "kind": None}` with no
  `path` key - exactly how a registry dependency already looks to this tool
  today). For the licence half: recorded `cargo deny check licenses` at the
  final head (passing), with its negative control - temporarily drop
  `"Apache-2.0"` from `deny.toml`'s `allow` list, rerun the same command,
  record the failure, restore the list - already rehearsed in this task's
  own scratch probe: the failure names `gpui`, `gpui_apple`, `gpui_macos`,
  `gpui_platform` directly as the rejected packages, tracing to exactly the
  mechanism removed (no other branch or check on this path produces that
  same failure).
- **AC#3** (off-screen drawing reachable only from a test or tool target;
  the shipped binary shown not to carry `test-support`, by a check that
  fails if it does): the mechanism is Cargo's own dependency-kind
  separation - `gpui_platform`'s `test-support` feature is requested only by
  `crates/app`'s new `[dev-dependencies]` edge, never its `[dependencies]`
  edge, and only the new `[[example]]` target (`headless_smoke`) can see a
  dev-dependency at all; `cargo build --bin steg-app` never compiles or
  resolves features for anything outside `[dependencies]`. The check:
  `cargo tree -p steg-app -e normal,build -f "{p} {f}"`, grepped for
  `test-support` - expected empty. Its negative control: temporarily add
  `features = ["test-support"]` to the *normal* `gpui_platform` entry in
  `crates/app/Cargo.toml` (not the dev one), rerun the identical command,
  record the now-nonempty match naming `gpui_platform`/`gpui`/`gpui_macos`,
  then revert. Record both at the final head.
- **AC#4** (every forced source change in `crates/app`/`crates/ui`; anything
  that changes look or behaviour; the engineer's own live run before merge):
  the notes list exactly two forced source changes (`crates/app/src/
  main.rs`, `crates/ui/gallery/main.rs`, both `Application::new()` ->
  `gpui_platform::application()`) and the explicit `font-kit` feature choice
  as the one thing most likely to visibly change behaviour (real system font
  matching, off by default at every level of the new dependency chain) -
  named for the engineer's own live run, which this worker does not perform.
  Every other `gpui::` symbol steg's own code imports is confirmed present
  and public at the pinned commit (listed above, file by file) - a real
  comparison against the pinned commit's actual source, not a diff of two
  published crates' public-function lists the way the earlier spike's
  finding was reached.

## Split

Not split. This is already TASK-1.151's own part (a) (the dependency swap,
`boundaries.toml`/`deny.toml`, `just check` green) plus the minimal addition
this task's own AC#3 asks for (proving the test-support isolation mechanism
with one real example) - one dependency change and its immediate, inseparable
consequences (two call-site fixes the new dependency forces, two boundaries
entries the same dependency forces, one isolation mechanism the same
dependency's own feature shape forces). None of these pieces is separately
mergeable or independently useful without the others.

## Unestablished (flagged, not resolved here)

- Whether the full `gpui-pre` snapshot TASK-1.151's spike checked matches
  upstream beyond the files sampled there, or the one additional file this
  task matched (`headless_app_context.rs`, now against two points in
  upstream's own history) - spot-checked, not an audit, same as TASK-1.151's
  own wording.
- The real, full-workspace `cargo deny check licenses` run (step 5) - this
  task's own probe used a minimal two-crate dependency set, not steg's whole
  tree.
- Whether turning `font-kit` on changes anything visible enough to matter -
  flagged for the engineer's own live run (AC#4), not resolved by this plan.

## Questions for the engineer (each with a recommended default)

1. `boundaries.toml`'s `[steg-ui]` reason currently reads "gpui is the one
   external dependency... (decision-11)" - adding `gpui_platform` makes that
   sentence inaccurate. Recommend: reword the reason in this task (naming
   both dependencies and why) and separately flag decision-11's own wording
   to the engineer, rather than treating a worker's boundaries.toml comment
   edit as authority to amend a decision record.
2. TASK-1.151's decision 2 (second `[[bin]]` in `crates/app`) needs revising
   once TASK-1.151 reaches build, per the risk above. Recommend: a separate
   workspace member for the real scenario runner (clean dependency-kind
   isolation, explicit even to a reader who hasn't seen this task), with an
   `[[example]]`/optional-dep-plus-`required-features` shape as the fallback
   if a new crate is judged too heavy for that task's own size.
3. `font-kit` on by default in the new dependency's feature set (recommended
   above) - confirm during the live run before merge rather than treat this
   worker's reasoning (upstream's own real app almost certainly needs it;
   nothing in the chain turns it on without steg saying so) as settled.
<!-- SECTION:PLAN:END -->
