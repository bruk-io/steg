---
id: TASK-1.91
title: 'ui: the steg-ui package, its boundary, and the gallery'
status: In Review
assignee:
  - '@bruk.w5'
created_date: '2026-11-16T00:52:00Z'
updated_date: '2026-11-16T01:37:00Z'
labels:
  - size-3
milestone: m-4
dependencies: []
references:
  - task-1.91-ui-the-steg-ui-package-its-boundary-and
  - https://github.com/bruk-io/steg/pull/48
parent_task_id: TASK-1
project: bruk-io-steg
asdlc_accepted_from: DRAFT-1
asdlc_branch: task-1.91-ui-the-steg-ui-package-its-boundary-and
asdlc_plan_approved_by: bruk
asdlc_plan_approved_hash: 'f7b4047abc0d'
asdlc_plan_approved_size: size-3
asdlc_reviewer: bruk.r4
asdlc_review_kind: same-machine
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
doc-17 (R117, R124), decision-11, doc-16's package tables as changed on 2026-11-15. The first piece of the UI work: it needs nothing from the server and overlaps none of the server tasks. GPUI's own examples are the reference (https://github.com/zed-industries/zed/tree/main/crates/gpui/examples: window.rs, hello_world.rs, testing.rs); they follow GPUI's main branch and steg pins gpui 0.2.2, so check an example against the pinned version. Carried from TASK-1.52's notes: how a UI pull request's screenshot is captured is decided here.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 crates/ui exists as package steg-ui with the folders doc-17 gives (foundations, atoms, molecules, organisms, gallery); boundaries.toml allows it gpui and nothing of steg's, and allows the app steg-ui, each with its reason; the app depends on it (R117)
- [ ] #2 The gallery is a program in the package that opens a window and shows what the package holds, in light and in dark, switchable; with nothing built yet it shows one placeholder drawn with GPUI so that the window, the theme switch and the capture are proved (R124)
- [ ] #3 A just recipe captures the gallery to an image file without a person at the screen, or the plan says why that cannot be done on this GPUI version and what a pull request shows instead; the pull request for this task carries that image (team rule: a UI pull request shows an image)
- [ ] #4 The architecture model, the affected-crates tool and just check know the new package: the LikeC4 model has it with its one allowed dependency, and a change under crates/ui selects its tests and the app's
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
One reviewable change, not several. AC#1-#4 mutually prove each other: the boundary
rule (AC1) is only meaningful once something that uses gpui exists to test it against;
the gallery (AC2/AC3) is the thing that proves the window and capture mechanisms work
at all; the tooling/model (AC4) only has a real crate to recognise once the others
land. A skeleton crate with no program, or a program with no boundary, would each be
unreviewable alone.

## Experiment run (not assumed) before committing to the capture approach

Built a throwaway gpui=0.2.2 binary (scratchpad, not in the repo) reusing this
workspace's cached target dir. Findings, each checked, not read off the README:

- `cx.open_window` succeeds from a plain Bash-invoked process (no terminal focus) on
  this machine, given a live WindowServer session: `open_window: Ok`, a real NSWindow.
- GPUI 0.2.2's public API has no way to read rendered pixels back: `TestWindow::draw`
  (src/platform/test/window.rs) discards the `Scene` outright (`fn draw(&self, _scene:
  &Scene) {}`); `MetalRenderer`/`CAMetalLayer` (src/platform/mac/metal_renderer.rs) are
  `pub(crate)`, unreachable from a consumer crate. No `screenshot`/`save_png` anywhere
  in `src/`. The only road to pixels is OS-level screen capture of a real on-screen
  window.
- Composed proof, with a positive and a negative control, that OS-level capture
  actually catches the gallery's own content (not a lucky blank capture):
  - Opened the real window at a fixed origin/size (100,100)-(400,400), red
    (`0xff0000`), `cx.activate(true)`.
  - `screencapture -x` (full screen) while the window was open: decoded the PNG
    (stdlib-only decoder, no Pillow available) and read pixel (250,250) →
    `(255,0,0,255)`. Pixel (10,10), outside the window, was desktop background, not
    red.
  - Negative control: same capture 9s later, after the 12s self-quit timer fired and
    the process was confirmed gone (`pgrep` found nothing) → pixel (250,250) was
    `(36,39,45,255)`, not red. The capture mechanism is catching the window's actual
    render, not passing by accident.
  - Second mechanism, also proven: looked up the window's `CGWindowID` by the
    process's pid (`CGWindowListCopyWindowInfo`, via a 15-line Swift script - no
    Accessibility permission needed, only Screen Recording) and captured with
    `screencapture -x -o -l <id>`. Produced a 300x332 PNG (300 wide = the content
    area; the extra height is the titlebar). Pixel (150,150) inside the content area →
    `(255,0,0,255)`; pixel (10,10), in the titlebar chrome → not red.
  - `screencapture` only worked here because this session's terminal already holds a
    one-time, interactive Screen Recording TCC grant. That grant is per-app, cannot be
    scripted or pre-seeded (SIP-protected TCC database), and does not exist at all on a
    GitHub-hosted macOS runner, which is also re-imaged per job, so even a successful
    grant wouldn't survive to the next run.

Decision: capture by `CGWindowID`, not by a fixed screen region. It survives occlusion
(another window, a notification, Mission Control) that a region capture would silently
record instead of failing on; a fixed region is a trap waiting for a flaky capture the
first time something overlaps that part of the screen.

## What this means for `just check` / CI

`.github/workflows/ci.yml` runs `just check` on every PR on `macos-26`, no TCC
accommodation anywhere in it (no pre-grant step, no `tccutil`, nothing). Given the
finding above, that's correct and must stay true: capture is its own recipe, outside
`check`'s dependency chain, full stop. `cargo build`/`cargo clippy`/`cargo test`
(everything `just check` runs) only ever *compile* the gallery binary - compiling a
`fn main()` never calls it - so CI is unaffected by any of this; it proves nothing
about whether a window can open there and isn't asked to.

## Layout (doc-17's tree, only what this task's AC#1 names)

```
crates/ui/                      package steg-ui
  Cargo.toml                    lib + one [[bin]] (gallery)
  CLAUDE.md                     doc-16 R111: one per package
  src/
    lib.rs                      pub mod foundations; atoms; molecules; organisms;
    foundations/mod.rs          doc comment only; TASK-1.93 adds generated.rs here
    atoms/mod.rs                doc comment only
    molecules/mod.rs            doc comment only
    organisms/mod.rs            doc comment only
  gallery/
    main.rs                     opens the window; the only thing in the crate that
                                 calls gpui::Application::run
```
`design/foundations.json` is deliberately not created here - AC#1 names five folders
(foundations, atoms, molecules, organisms, gallery) and omits `design/` on purpose; the
board's own dependency graph does not list TASK-1.92 as a dependent of this task,
confirming the export is that task's job, not this one's.

Each of the four level modules holds nothing but a module doc comment naming what the
level is for and that TASK-1.93/1.95 fill it in - compiles cleanly, nothing to lint.

## The gallery: a `[[bin]]` of `steg-ui`, not an example, not a separate crate

```toml
[[bin]]
name = "gallery"
path = "gallery/main.rs"
```
- Not `examples/`: doc-17's own tree puts `gallery/` as a sibling of `src/`, not
  `examples/`; R124 calls it "a program in the package", something looked at while
  building every later component, not an incidental usage demo.
- Not a separate crate: doc-16's package table has exactly one UI package, `ui`. A
  separate `steg-ui-gallery` crate would need its own boundaries.toml row depending on
  `steg-ui`, and doc-17 lists no second package.
- Effect on the boundary table: none. The bin target is governed by the same
  `[steg-ui]` entry as the lib; Cargo has no per-target dependency declaration.
- Effect on other crates' build time: none. A path dependency only ever pulls in the
  dependency's **lib** target - Cargo has no mechanism to depend on a sibling crate's
  binary - so `cargo build -p steg-app` never compiles `gallery/main.rs`. The cost is
  real but contained: every direct build of `steg-ui` itself (`-p steg-ui`, or
  `--workspace`) now also compiles the gallery binary, every time, not only when
  `gallery/` changes - proof step below confirms this is contained correctly.

## Theme switch placeholder (named as what TASK-1.93 replaces)

`Window::appearance()` in gpui 0.2.2 is read-only (follows the OS; no public setter) -
confirmed by reading `src/window.rs`. The placeholder is therefore the gallery's own
state, nothing from GPUI's theming: an enum on the root view,
`enum GalleryTheme { Light, Dark }`, read once at startup from a `STEG_GALLERY_THEME`
env var (default `light`) and toggled live by a key (`t`) for a person looking at it.
Env var, not only a key, because AC#3's capture has no person to press a key: the
`capture-gallery` recipe launches the binary twice, once per value, and gets both
images without any synthetic input (which would need its own, separate Accessibility
permission grant - deliberately not taken on). A short comment in `gallery/main.rs`
says this whole enum is the placeholder TASK-1.93 (`steg-ui`'s real foundations, with
light/dark named values) replaces.

Literal colours in `gallery/main.rs` (red/whatever marks each theme) are the gallery's
own chrome, not a component of the four levels R120 binds - noted inline so a later
reader doesn't mistake it for a violation once real atoms exist.

## Files

- `Cargo.toml` (root): add `"crates/ui"` to `members`.
- `crates/ui/Cargo.toml`, `crates/ui/CLAUDE.md` (new, modelled on `crates/app/CLAUDE.md`).
- `crates/ui/src/lib.rs`, `src/{foundations,atoms,molecules,organisms}/mod.rs` (new, doc-comment-only).
- `crates/ui/gallery/main.rs` (new): window, theme enum, key toggle, fixed titlebar title (e.g. "steg-ui gallery") so the pid-based CGWindowID lookup is also human-identifiable in Activity Monitor during local debugging.
- `crates/ui/gallery/window_id.swift` (new, small, mac-only): given a pid, print its one on-screen `CGWindowID` via `CGWindowListCopyWindowInfo`. No new Cargo dependency, no Accessibility permission - only the Screen Recording grant the capture already needs.
- `crates/app/Cargo.toml`: add `steg-ui = { path = "../ui" }`. `crates/app/src/main.rs` stays as-is (not wired to call into steg-ui yet - no real component exists to use; this is a deliberate, scoped no-op, called out here so it doesn't read as forgotten).
- `crates/app/CLAUDE.md`: "May depend on" gains `steg-ui`.
- `boundaries.toml`: new `[steg-ui]` (`allow = ["gpui"]`, reason citing R101/R117 - nothing of steg's, gpui only); `[steg-app]`'s `allow` gains `"steg-ui"`, reason extended to mention the app now depends on it for its components (doc-17).
- `justfile`: `check-packages`'s hardcoded loop (`for pkg in steg-sdk steg-kernel steg-server steg steg-app`) gains `steg-ui`, inserted before `steg-app` (its dependent); two new recipes, `run-gallery` (`cargo run --package steg-ui --bin gallery`, mirrors the existing `run`) and `capture-gallery` (launches the binary twice, STEG_GALLERY_THEME=light then =dark, looks up each CGWindowID via window_id.swift, `screencapture -x -o -l <id>` each to `target/gallery/{light,dark}.png`, quits each). Neither needs `zig-deps-seeded` for correctness (steg-ui/steg-app don't reach ghostty-vt), but both include it anyway to match every other real-compile recipe's convention.
- `tools/src/steg_tools/recipes.py`: add `"capture"` to `ALLOWED_VERBS`, with a comment - a genuinely new action (screenshot-to-file), per the file's own instruction to extend rather than route around the check.
- `architecture/models/steg/app/ui-pkg.c4` (new): `extend steg.app { ui = component 'ui' { technology 'Rust library' description '...same wording as doc-17's package-table row...' } }`. No `#shared` tag - unlike `sdk`, this package is built into exactly one container. No id clash (checked: no existing element is named `ui` anywhere under `architecture/`).
- `architecture/models/steg/app/_relationships.c4`: add `appPkg -> ui 'depends on'` alongside the existing `appPkg -> sdk`.
- `architecture/views/steg/packages.c4`: **not edited** - `include steg.app.*` already pulls in every child of `steg.app`, confirmed by reading the file; editing it would be a no-op at best, redundant at worst.

## Proof, per acceptance criterion, each with its negative control

1. **AC#1, boundary.** `just check-boundaries` passes on the real diff. Negative
   control: scratch-edit `crates/ui/Cargo.toml` to add `steg-sdk`, run
   `just check-boundaries`, confirm it refuses naming the `[steg-ui]` reason; revert
   the scratch edit before finishing.
2. **AC#2, gallery window + switch.** `just run-gallery` opens a real window;
   `STEG_GALLERY_THEME=dark just run-gallery` opens it in the other state; pressing `t`
   toggles it live. (Manual/local - this AC is about the program existing and working
   when run, not about anything CI executes.)
3. **AC#3, capture.** `just capture-gallery` produces `target/gallery/light.png` and
   `dark.png` with no person at the screen beyond having once granted the invoking
   terminal Screen Recording permission (documented in `crates/ui/CLAUDE.md` as a
   one-time local precondition, same category as installing Xcode's command-line
   tools). Proof re-runs the same positive/negative pixel check already done in the
   throwaway experiment, this time against the real recipe's own output: light.png's
   and dark.png's pixel at the same fixed coordinate differ (the switch actually
   changed what got captured, not just the env var being accepted). The pull request
   carries both images (team rule: a UI PR shows an image). `just check` never calls
   this recipe or `run-gallery`; CI's own run of `just check` is the proof that nothing
   here reached it.
4. **AC#4, affected tool + model.**
   - `architecture`: `just check-architecture` passes; open the rendered `stegPackages`
     view and confirm `ui` appears as a fourth box under Steg.app next to `app`/`sdk`.
   - `affected`, real probe, not inferred from this task's own diff: this task's commit
     touches `Cargo.toml`/`boundaries.toml`/`justfile`, all in `affected.py`'s
     `GLOBAL_FILES`, so `steg-affected` on the task's own diff will correctly print
     `all` - that is not evidence AC#4 holds. After the crate is committed, as a
     separate step: touch one file under `crates/ui/src/` only, run
     `just show-affected`, confirm it prints exactly `-p steg-app -p steg-ui` (not
     `all`, not just `-p steg-ui`: `with_dependents` must walk forward to `steg-app`).
     Negative control: touch a file under `crates/app/src/` only instead, confirm the
     selection does **not** include `steg-ui` (the dependency is one-directional).
     Revert both probe touches.
   - `check-packages`: confirm the updated loop actually builds+tests `steg-ui` alone
     (`cargo build -p steg-ui`, `cargo test -p steg-ui`) as part of `just check`.

## Licences

No new Cargo dependency enters the workspace. `gpui` is already graphed in via
`steg-app`, so `cargo deny check licenses` already covers it; `steg-ui` adds no new
edge to anything deny.toml doesn't already allow. The capture mechanism is the OS
`screencapture` binary via `std::process::Command`-or-equivalent, not a Rust
image-writing crate - deliberately, so this task adds zero new licence surface at all.

## For the engineer to decide

- Extending `recipes.py`'s `ALLOWED_VERBS` with `"capture"` is a vocabulary call, not
  just a mechanical one - flagging rather than deciding it silently.
- `just capture-gallery`/`run-gallery` depend on the invoking machine's terminal having
  a one-time, interactive Screen Recording grant already in place. That's a real,
  undocumented-until-now precondition for anyone's first run on a fresh Mac. Proposing
  it live in `crates/ui/CLAUDE.md`; say if it belongs somewhere more prominent (e.g.
  the repo root `CLAUDE.md` or a `just setup` note) instead or as well.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Read first: GPUI's README, https://github.com/zed-industries/zed/blob/main/crates/gpui/README.md (the engineer: 'a really good intro to it'), then the examples; accessibility examples are a11y.rs, focus_visible.rs and tab_stop.rs.

Plan checked against doc-17 (R117, R124), decision-11 and doc-16 (R104, R108, R111) by @bruk.o: ok. steg-ui is a package allowed gpui and nothing of steg's, and the app is allowed steg-ui; the gallery is a binary of the package; the theme switch is a placeholder named as what TASK-1.93 replaces. The capture was established by experiment: gpui 0.2.2 offers no way to read back what it drew, so the image comes from the system's screen capture of the gallery's own window, which needs the invoking terminal to hold the Screen Recording permission, granted once by a person; CI's runner cannot hold it, so just check and CI only compile the gallery and the capture is a recipe a worker or a person runs. Orchestrator's answers to the two points raised: 'capture' is added to the recipe verbs; the permission is documented in the package's CLAUDE.md, and, for the build, the capture recipe itself checks that the image shows the gallery (a known colour at a known place) and fails with a message naming the Screen Recording permission when it does not, so that a blank or wrong capture can never be attached to a pull request as its image.

Check FAILED: `just check` exited 1 at 2026-11-16T01:19:04+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.91 on task-1.91-ui-the-steg-ui-package-its-boundary-and at d6e18b7c21ba:

```text
ok      format check (0.2s)
ok      lint (83.2s)
ok      boundaries (0.2s)
ok      licences (0.7s)
skip    tests: this change touches no crate
ok      build (steg-sdk) (3.1s)
ok      test (steg-sdk) (1.7s)
ok      test (steg-sdk, local-socket) (2.0s)
ok      build (steg-kernel) (43.1s)
ok      test (steg-kernel) (1.4s)
ok      build (steg-server) (41.5s)
ok      test (steg-server) (6.0s)
ok      test (steg-server, race-fixture) (6.7s)
ok      build (steg) (0.3s)
ok      test (steg) (0.3s)
ok      build (steg-app) (19.3s)
ok      test (steg-app) (0.8s)
FAILED  tool tests (exit 1, 1.6s)
F...................................................                     [100%]
=================================== FAILURES ===================================
__________ test_the_five_real_crates_and_their_real_dependency_edges ___________

    def test_the_five_real_crates_and_their_real_dependency_edges() -> None:
        by_name = {crate.name: crate for crate in real_crates()}
>       assert by_name.keys() == {"steg-sdk", "steg-kernel", "steg-server", "steg", "steg-app"}
E       AssertionError: assert dict_keys(['s..., 'steg-app']) == {'steg', 'ste...'steg-server'}
E         
E         Extra items in the left set:
E         'steg-ui'
E         Use -v to get more diff

tools/tests/adapter/test_affected_real_workspace.py:21: AssertionError
=========================== short test summary info ============================
FAILED tools/tests/adapter/test_affected_real_workspace.py::test_the_five_real_crates_and_their_real_dependency_edges
1 failed, 51 passed in 1.18s
full output: .quiet/tool-tests.log
error: recipe `test-tools` failed on line 92 with exit code 1
```

Check passed: `just check` exited 0 at 2026-11-16T01:30:29+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.91 on task-1.91-ui-the-steg-ui-package-its-boundary-and at d6e18b7c21ba with uncommitted changes:

```text
ok      format check (0.2s)
ok      lint (0.4s)
ok      boundaries (0.1s)
ok      licences (0.7s)
ok      tests (every crate) (4.6s)
ok      build (steg-sdk) (0.1s)
ok      test (steg-sdk) (0.2s)
ok      test (steg-sdk, local-socket) (0.5s)
ok      build (steg-kernel) (0.2s)
ok      test (steg-kernel) (0.3s)
ok      build (steg-server) (0.2s)
ok      test (steg-server) (4.0s)
ok      test (steg-server, race-fixture) (4.3s)
ok      build (steg) (0.2s)
ok      test (steg) (0.1s)
ok      build (steg-ui) (0.3s)
ok      test (steg-ui) (0.4s)
ok      build (steg-app) (0.4s)
ok      test (steg-app) (0.3s)
ok      tool tests (1.4s)
ok      architecture (0.9s)
ok      recipe names (0.1s)
```

Check passed: `just check` exited 0 at 2026-11-16T01:32:02+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.91 on task-1.91-ui-the-steg-ui-package-its-boundary-and at 90c7a992686c:

```text
ok      format check (0.2s)
ok      lint (1.6s)
ok      boundaries (0.1s)
ok      licences (0.7s)
ok      tests (every crate) (5.5s)
ok      build (steg-sdk) (0.1s)
ok      test (steg-sdk) (0.2s)
ok      test (steg-sdk, local-socket) (0.3s)
ok      build (steg-kernel) (0.1s)
ok      test (steg-kernel) (0.2s)
ok      build (steg-server) (0.1s)
ok      test (steg-server) (4.0s)
ok      test (steg-server, race-fixture) (4.2s)
ok      build (steg) (0.1s)
ok      test (steg) (0.1s)
ok      build (steg-ui) (0.7s)
ok      test (steg-ui) (0.9s)
ok      build (steg-app) (0.6s)
ok      test (steg-app) (0.8s)
ok      tool tests (1.0s)
ok      architecture (0.8s)
ok      recipe names (0.1s)
```

Built as planned (digest f7b4047abc0d), no departures.

AC#1 (boundary): crates/ui (steg-ui) exists with the five folders doc-17 gives,
each level a doc-comment-only placeholder. boundaries.toml: new [steg-ui]
(allow = ["gpui"]) and [steg-app] gains "steg-ui", each with its reason;
crates/app/Cargo.toml depends on it. Negative control run live: a scratch
steg-sdk dependency on steg-ui was refused by `steg-boundaries` quoting the
[steg-ui] reason, then reverted before committing.

AC#2 (gallery window + switch): gallery/main.rs is a [[bin]] of steg-ui (not
an example or a separate crate - reasoning on the task's plan). Opens a real
GPUI window with one placeholder, theme set by STEG_GALLERY_THEME (light/dark,
case-insensitive, unrecognised/unset falls back to light) and toggled live by
`t`. 5 unit tests on the pure parsing/toggling logic (cargo test -p steg-ui).
Ran it directly for both themes and screenshotted each (CGWindowID capture) to
confirm the placeholder and the switch actually render distinctly before
building the recipe around the same mechanism.

AC#3 (capture): `just capture-gallery` launches the binary twice
(STEG_GALLERY_THEME=light, then dark), finds each window's CGWindowID
(crates/ui/gallery/window_id.swift - pid lookup, no Accessibility permission),
captures with `screencapture -l<id>`, and checks each PNG with
`steg-gallery-check` (tools/src/steg_tools/gallery.py, stdlib-only PNG
decoder) before calling it done - a wrong or blank capture fails the recipe
with a message naming Screen Recording permission, rather than reaching a
pull request. Ran the real recipe end to end: both images produced, both
checks passed, and the negative control (checked light.png against dark's
expected colour) failed with that exact message. 6 unit tests + 2 functional
tests (tmp_path, real file I/O) on the checker itself, including its own
negative control (a desktop-coloured pixel must not pass as the gallery's
placeholder). Confirmed no leftover `gallery` process after every run
(`pgrep`/`ps aux`, clean each time - the recipe's `trap ... EXIT` on every
tracked pid). Neither this recipe nor `run-gallery` is reachable from `just
check`; ci.yml has no TCC accommodation, matching the plan's finding that
GitHub's hosted macOS runner cannot grant Screen Recording unattended. Both
images attached to the PR via `gh pr comment --attach` (real
user-attachments URLs, not placeholders) and linked from the PR body.

AC#4 (architecture + affected tool): new
architecture/models/steg/app/ui-pkg.c4 (component `ui`, no #shared - built
into one container) plus one edge (appPkg -> ui) in that container's
_relationships.c4; `views/steg/packages.c4` needed no edit (`include
steg.app.*` already pulls in every child - checked by reading it, not
assumed). `likec4 validate architecture` passes. The affected-crates tool
(tools/src/steg_tools/affected.py) needed no code change - it's generic over
`cargo metadata` - but tools/tests/adapter/test_affected_real_workspace.py's
hardcoded five-crate expectation did, and now asserts, against the real
workspace: a change under crates/ui alone selects `-p steg-app -p steg-ui`
(positive), and a change under crates/app alone selects only `-p steg-app`,
not steg-ui (negative control for the one-directional dependency) - stronger
and more durable than a manual touch-and-revert probe, so that's what stands
as the proof rather than a throwaway manual run. `check-packages`'s hardcoded
crate loop gained steg-ui.

Checks recorded on this task: `just check` at the pre-rebase head (exit 0,
all 21 steps ok) and again at the post-rebase head 90c7a99 (exit 0, same).
CI (https://github.com/bruk-io/steg/actions/runs/43786853386) passed in
3m58s against that same head - watched to completion in the foreground, not
backgrounded. pytest tools/tests: 60 passed (51 pre-existing + 9 new: 6 unit
+ 2 functional for the gallery checker, 1 unit-suite addition folded into the
existing file's count via the edited adapter test).

Rebase onto origin/main (never pushed before, so rebase not merge) brought in
two unrelated board-only commits (a check record on TASK-1.77, then after a
second fetch also one on TASK-1.78 and TASK-1.92) - no code, no conflicts.
TASK-1.92's own commits add crates/ui/design/ alongside this task's crates/ui/
tree; nothing in this diff touches that path, so no interaction expected, but
flagging since the task description named it as the known adjacent landing.

One thing not in the original plan, caught by the first `just check` run
racing with an early edit (both editing Cargo.toml's members list at
overlapping times - noted here since it's a plan-time gap, not a build
mistake): tools/tests/adapter/test_affected_real_workspace.py's hardcoded
five-crate assertion wasn't on the plan's file list. Fixed as part of AC#4
(see above); no behaviour change beyond what the plan already called for.
<!-- SECTION:NOTES:END -->

## Comments

<!-- COMMENTS:BEGIN -->
author: @bruk
created: 2026-11-16T01:15:00Z

---

Plan approved by @bruk (f7b4047abc0d, size-3): its worker may build it.

---

author: @bruk.r4
created: 2026-11-16T01:37:00Z

---

@bruk.r4 is reviewing TASK-1.91: a same-machine review: it waited 0m with no other engineer's reviewer, longer than review_fallback_after (0m), so a fresh reviewer on its author's machine takes it.

---
<!-- COMMENTS:END -->
