---
id: TASK-1.151
title: >-
  Tools: a worker can produce a UI change's stills and recording without the
  screen
status: In Progress
assignee:
  - '@bruk.w3'
created_date: '2026-11-22T04:52:00Z'
updated_date: '2026-11-22T19:59:00Z'
labels:
  - size-3
milestone: m-4
dependencies:
  - TASK-1.154
parent_task_id: TASK-1
project: bruk-io-steg
asdlc_accepted_from: DRAFT-1
asdlc_branch: task-1.151-tools-a-worker-can-produce-a-ui-change-s
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Asked for by the engineer on 22 November 2026: UI pull requests need an image, and a recording for motion, but workers must not launch the app, inject input or capture the screen on the engineer's machine, so today every UI change waits for the engineer to record it by hand (TASK-1.127, steg PR 95). Unknowns dominate, so the plan starts with a spike and comes back with the route before building. Routes, in the order to try: (A) draw off screen in process and read the frames back, driven by a controlled clock. Checked today: the pinned gpui 0.2.2 has only the test context that does not really draw; newer GPUI lines carry a visual test context and a headless app context that look like this, so A may mean a GPUI upgrade, which is its own decision for the engineer. (B) run the real app on a macOS CI runner against a scenario and capture only its own window there, uploading the result as a build artefact: nothing of the engineer's is on that machine, at the cost of a slow loop. (C) on the engineer's machine, capture only the app's own window by window id in a throwaway home: still puts a window on their screen and needs screen-recording permission, so it is the fallback. The plan names the route, what it costs, and anything that is the engineer's to decide. Licence: read Zed and GPUI to learn the API, copy nothing from Zed's GPL crates.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 One just recipe takes a named scenario and writes stills, and a recording when the scenario has motion, of the app's own drawing, with no window on the user's screen, no keyboard or mouse input injected, no screen capture and no clipboard use on the machine it runs on
- [ ] #2 A scenario is data in the repository: an explicit pane program with fixed content, a throwaway home, never the login shell, and a scripted timeline (keys, waits, focus changes) on a clock the tool controls, so the same scenario gives the same frames every run and nothing personal can appear
- [ ] #3 The cursor dissolve of TASK-1.127 is the first scenario: its recording and its stills of each step come out of the tool and match what the engineer recorded by hand
- [ ] #4 The team's rules say a UI pull request's image comes from this tool, and the worker brief's ban on launching the app stays
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
Narrowed per the team's direction after the engineer accepted the GPUI move as its own task. The route choice, the gpui-pre provenance finding, and the git-pin-vs-registry decision all now live on TASK-1.154 (git pin on zed-industries/zed, gpui-pre fallback only the engineer can choose) - this task depends on it and does not restate that evidence. This task is now just the harness, its first scenario, and the rule wording.

BUILDING STARTS ONLY AFTER TASK-1.154 MERGES. Nothing here can compile or run before the workspace's `gpui` dependency is the one TASK-1.154 lands - this plan is written against that future state, not spiked further (no build, no run, no app launch in this planning pass, per instruction).

(b) THE HARNESS

- `crates/app` grows `src/lib.rs` (re-exporting what a scenario needs - building the window, wiring steg-sdk, the pane kind) with `main.rs` becoming a thin wrapper over it, plus a second `[[bin]]` in the same package that uses `HeadlessAppContext` (TASK-1.154's own dependency, once merged) instead of `Application::new().run(...)`. This reaches inside the app crate rather than through a public steg API - stated plainly per the architecture rule: justified because the scenario bin still only drives the app through steg-sdk's public API exactly as `main.rs` does today (the same consumer path TASK-1.117 already established); only the entry point changes, and no new capability reaches the CLI or a plugin, nor should it - this is a dev-only capture harness, not a steg feature.
- Scenario data format: scenarios live at `crates/app/scenarios/<name>.toml` (mirrors the gallery's own convention of living beside the code it exercises). Each names: an explicit pane program and its argv (never `$SHELL`/the login shell, so output is identical on every run and nothing from the real environment can appear); a throwaway `HOME` the tool creates per run and deletes after; and a scripted timeline of steps - a key sequence, a wait (as a duration the tool advances `TestDispatcher::advance_clock` by, never a real `sleep`), or a focus change.
- `just capture-scenario <name>` (the verb is already on `check-recipes`'s list via `capture-gallery`'s precedent) builds and runs the scenario bin for `<name>`, writing `target/scenarios/<name>/<step>.png` for every scripted step and, when the scenario's timeline has more than one step or an explicit "motion" marker, `target/scenarios/<name>/recording.gif` (encoded with the `image` crate's own GIF encoder - already a dependency via `HeadlessAppContext`'s `RgbaImage`, so no new dependency, no new licence surface, and no shelling out to ffmpeg/gifski). Never part of `just check`, for the same reason `capture-gallery` isn't - it is a pull request's own evidence, not a build correctness check - but `just check-packages` should still compile the scenario bin alone, so nothing opens a (virtual) window by itself under `cargo build`/`cargo test`.
- Proof this part is done: run `just capture-scenario` against a trivial, throwaway two-step scenario (not the cursor dissolve - that is (c)) and show it writes a still per step plus a recording, with no window, no input injection, no screen capture, no clipboard use (review the new code for any reference to `NSWindow`/`CGWindowID`/`screencapture`/`NSPasteboard`/`CGEvent` - none should exist, the same proof style TASK-1.151's own spike already used on a standalone project).

WHAT THIS TASK ADDS ON TOP OF TASK-1.154's CRITERION 3: TASK-1.154 AC#3 already proves the shipped `steg-app` binary never carries gpui's `test-support` feature, with a check that fails if it does - that mechanism is TASK-1.154's to build, not this task's to reinvent. What this task adds: once the new scenario `[[bin]]` exists in crates/app (a second target that does ask for `test-support`), re-run that same check as this task's own regression proof, showing the new target did not make `test-support` leak into `steg-app`'s own build. One line of proof, not a new criterion or a second check.

(c) THE CURSOR-DISSOLVE SCENARIO

- The first real scenario: TASK-1.127/PR 95's cursor dissolve, as data in `crates/app/scenarios/cursor-dissolve.toml`, wired through (b)'s harness - the real client/server/kernel pipeline, a fixed non-login pane program, waits advanced on the controlled clock so the dissolve's own timer actually plays out deterministically.
- Determinism proof: run `just capture-scenario cursor-dissolve` twice and diff every output PNG (and the recording) byte-for-byte - same bytes both times, proving nothing - wall clock, process ordering, anything in the real environment - leaked into the frames.
- Fidelity judgement: "matches what the engineer recorded by hand" (AC#3) means a reviewer watches both outputs side by side and agrees they show the same dissolve - not a pixel diff. PR 95's hand recording went through macOS `screencapture`'s own colour management, which `tools/src/steg_tools/gallery.py` already documents drifting up to 4/channel; stating this plainly here so a reviewer does not fail this criterion on bytes that were never going to match byte-for-byte.

(d) TEAM RULE WORDING (proposed here; the team lead opens the separate PR, not this worker)

- Proposed line for the team's rules: "a UI pull request's image (and, for motion, its recording) comes from `just capture-scenario <name>`, not a worker launching the app or capturing the screen." The worker brief's existing ban on launching the app, injecting input, capturing the screen, or using the clipboard stays exactly as written - this tool is the sanctioned way to get the evidence without any of that, not an exception to the ban.

SHOULD (b) AND (c) BE TWO TASKS: yes, recommend splitting. They have different proofs and different review surfaces - (b) is infrastructure correctness (the harness runs clean on a throwaway scenario, with no window/input/capture/clipboard use), (c) is content fidelity judged by a reviewer against a hand-made reference, and (c) cannot even start until (b) merges, the same shape of dependency TASK-1.151 now has on TASK-1.154. Splitting lets (b) land and be reusable (for any future scenario, not just the cursor dissolve) before (c)'s specific content is finished, and keeps a reviewer's attention on one kind of question at a time. Recommend: keep this task (TASK-1.151) as (b) the harness, and carve out a new dependent task for (c) the cursor-dissolve scenario, mirroring the TASK-1.151/TASK-1.154 dependency shape. (d) is small enough to ride with whichever of (b)/(c) lands last, or go as its own size-1 - the team lead's call, since it is the team lead's own PR either way.

ENGINEER'S DECISIONS THAT STILL APPLY HERE (route and provenance now live on TASK-1.154; these are this task's own):

2. Keep gpui's `test-support` feature off the shipped `steg-app` binary by giving crates/app a second `[[bin]]` rather than a new crate, relying on the same per-target feature unification TASK-1.154's AC#3 establishes and `capture-gallery`'s separate `gallery` bin already relies on. Recommend: yes, for consistency with both precedents.
3. Exact scenario data format and schema (above is a proposal, not a decision already made). Recommend: approve the shape above; this task's own build-phase plan pins the exact field names.
4. Recording format: the `image` crate's own GIF encoder over shelling out to ffmpeg/gifski. Recommend: yes - no new dependency, no new licence surface.

RULES TO RESPECT: `.claude/rules/architecture.md` (the lib-target-plus-second-bin reaches inside the app crate, justified above); `.claude/rules/licence.md` (nothing new to check here once TASK-1.154 has already cleared gpui's own licence surface - this task adds no dependency beyond what TASK-1.154 brings, since the GIF encoding reuses `image`, already pulled in by `HeadlessAppContext`); `tools/` is a Python package - the scenario runner is Rust and lives in `crates/app`, with the `just` recipe as the only surface `tools/` conventions (recipe naming, `check-recipes`) need to know about.

PROOF PER THIS TASK'S OWN ACCEPTANCE CRITERION:

- AC#1 (no window, no input injection, no screen capture, no clipboard, on the machine it runs on): proved by running `just capture-scenario <name>` and by code review for the OS-surface references listed under (b)'s proof above.
- AC#2 (scenario as data - explicit program, throwaway HOME, never the login shell, scripted timeline on a controlled clock, deterministic, nothing personal): proved by the scenario file itself (reviewable) plus the determinism proof under (c) - run twice, diff byte-for-byte.
- AC#3 (the cursor dissolve matches PR 95's hand recording): proved by a reviewer watching both outputs side by side, as stated under (c) - not a pixel diff, and said so explicitly to set reviewer expectations correctly.
- AC#4 (team rules point at this tool; worker brief's ban stays): proved by the diff itself in (d), landed as the team lead's own separate PR.

UNKNOWNS THAT STILL APPLY (narrowed - the provenance/route unknowns now belong to TASK-1.154):

- Whether `HeadlessAppContext`'s `TestDispatcher` clock actually drives the real cursor-dissolve timer end to end once the real client/server/kernel pipeline is wired in through steg-sdk - reasoned from the newer gpui's own `TestAppContext`/`TestDispatcher` machinery in TASK-1.151's spike, not yet proven against steg's own code. A (b)/(c)-stage risk.
- Whether the scenario's real local-socket server autostart needs anything beyond the throwaway-HOME convention this repo already uses elsewhere for live runs. A (c)-stage risk.
- GitHub-hosted macOS runner GPU/window-server support for Route A: still not established, and still not needed, since AC#1 reads "on the machine it runs on" and the local recipe is what a worker actually runs.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
22 Nov 2026 (@bruk.o): the engineer accepted the GPUI move as TASK-1.154 (git pin on zed-industries/zed; gpui-pre only as a fallback that comes back to the engineer). TASK-1.151 now depends on it and keeps parts (b), (c) and the rule wording (d) of its plan; part (a) is TASK-1.154's.

Plan checked against the task's criteria by @bruk.o (22 Nov 2026, plan a482bf1aa9d2): not yet for approval. (1) Recording: stills per scripted step do not make a recording of motion - a wait step would give one frame; the harness needs frames sampled at a fixed interval of the tool's clock across waits, and the plan must say so. (2) Determinism: the pane program, pty and server run on real threads and real time, outside the tool's clock; the plan must say how the harness waits for pane output (an event with a bound, no polling, no sleep) before advancing, and must read how the dissolve's timer and phase are driven in terminal_element.rs (executor timer or wall clock) since byte-identical frames depend on it. (3) Split: the dissolve scenario is drafted as its own task (DRAFT-3, depends on this one) carrying criterion 3; once the engineer accepts it this plan covers the harness only, proved on a trivial scenario that includes motion.
<!-- SECTION:NOTES:END -->
