---
id: TASK-1.151
title: >-
  Tools: a worker can produce a UI change's stills and recording without the
  screen
status: In Progress
assignee:
  - '@bruk.w3'
created_date: '2026-11-22T04:52:00Z'
updated_date: '2026-11-22T20:02:00Z'
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
Narrowed again per the team's three corrections, with origin/main read (not run) for the dissolve's own timer mechanism. The dissolve scenario itself is now DRAFT-3 (depends on this task, carries its own fidelity criterion) - this task covers the harness only. Route/provenance still live on TASK-1.154, unchanged from before.

BUILDING STARTS ONLY AFTER TASK-1.154 MERGES. No build, no run, no app launch in this planning pass - the two points below came from reading `crates/app/src/terminal_element.rs` on `origin/main` (PR 95, already merged) directly, not from running anything.

1. RECORDING OF MOTION - FRAME SAMPLING

A "wait" step does not itself make a frame: the harness samples frames at a fixed interval of its own clock (`TestDispatcher`'s virtual time, advanced by `advance_clock`) across every wait, not just once at the end. Default interval: 100ms (10 samples/second - the cadence the team lead named for the dissolve), overridable per scenario (`sample_interval_ms` in the scenario's own `.toml`). For each 100ms (or configured) tick of virtual time the harness advances, it calls `cx.run_until_parked()` (pure virtual-dispatcher draining, no real sleep) to let any pending repaint settle, then captures a frame.

How stills relate to the sampled frames: every scripted step boundary (a key sequence finishing, a wait ending, a focus change landing) is one of the fixed-interval sample points by construction - the harness rounds a step's own wait duration up to the sampling interval, so a step boundary always lands on a sample. The still PNGs the recipe writes (`target/scenarios/<name>/<step>.png`) are named subsets of the sampled frames - one per scripted step, at the step's own boundary sample. The recording (`recording.gif`) is the full ordered sequence of every sampled frame across the whole scenario, not just the step-boundary ones, with each GIF frame's own delay set to the sampling interval that produced it.

2. DETERMINISM AGAINST REAL TIME

The pane program, the pty, the local server and the kernel run on real OS threads and real wall-clock time, outside `TestDispatcher` entirely - advancing the dispatcher's virtual clock does nothing to make their output arrive sooner, and nothing stops the harness from capturing before it has. The harness must never poll (checking in a loop with a sleep) and never sleep a fixed guess - it waits on a bounded event tied to the real arrival: the same shape gpui's own `TestAppContext::condition`/`Entity::next_notification` already use in the version this spike read (a predicate race against a bounded timer, woken by the real notification when the model updates, not by re-checking on a cadence) - concretely, waiting for the client's model to hold the scenario's own expected screen content, or steg-sdk's own bounded pane-wait primitive if one already exists for this (to be confirmed in the build task, not assumed here), each with its own timeout so a stuck scenario fails loudly instead of hanging. Only once that bounded wait resolves does the harness advance the virtual clock or capture a sample.

Read `crates/app/src/terminal_element.rs` and `main.rs` on `origin/main` (PR 95, merged) to answer the team lead's direct question - how the dissolve's own timer and phase are driven: `CursorFadeState.cycle_started_at` is `std::time::Instant` (`terminal_element.rs`, `Default::default()` and `show_whole()` both call `Instant::now()`); `paint_cursor_fade` computes `fraction_through_cycle` from `cycle_started_at.elapsed().as_secs_f32()` - `Instant::elapsed()`, the real OS monotonic clock, not a gpui-owned or virtualisable value. The *wake itself* is scheduled through gpui's own `BackgroundExecutor::timer` (`schedule_fade_wake`), which the harness's `TestDispatcher` can fire at a chosen virtual instant - but the *phase painted once that wake fires* is still computed from real elapsed wall-clock time since `cycle_started_at`, which the harness's virtual clock advance does not move. So advancing the dispatcher's clock to make a wake fire "early" in virtual time does not make the dissolve paint the phase that virtual duration would imply - it paints whatever phase the real wall-clock elapsed time (however little, since no real sleep happened) implies instead. The dissolve would not reach a chosen phase deterministically under this harness as it stands today - frames would depend on incidental real-time scheduling, not on the scenario's own scripted waits. This is a product change to TASK-1.127's own shipped code, not something this harness task can work around: `CursorFadeState`/`paint_cursor_fade` would need to source "now" from something the scenario harness can set deterministically (for example, a `now` the window or app already carries for this purpose, if the newer GPUI line exposes one, or a parameter threaded through painting that defaults to `Instant::now()` in production and is fixed under the harness) instead of calling `Instant::now()`/`.elapsed()` directly. Stating this plainly rather than working around it silently: it is its own line in the dissolve scenario's own plan - DRAFT-3, which carries that scenario's own fidelity criterion - or, if the change turns out bigger than one function, its own task depending on this one the same way DRAFT-3 already does. This harness task does not need to fix it: it only needs to expose whatever clock-control primitive DRAFT-3's own fix ends up needing (see the short paragraph below).

(b) THE HARNESS - this task's own scope

- `crates/app` grows `src/lib.rs` (re-exporting what a scenario needs - building the window, wiring steg-sdk, the pane kind) with `main.rs` becoming a thin wrapper over it, plus a second `[[bin]]` in the same package using `HeadlessAppContext` (TASK-1.154's dependency, once merged) instead of `Application::new().run(...)`. Reaches inside the app crate rather than through a public steg API - justified per the architecture rule because the scenario bin still only drives the app through steg-sdk's public API exactly as `main.rs` does today; only the entry point changes, and no new capability reaches the CLI or a plugin.
- Scenario data format: `crates/app/scenarios/<name>.toml`. Each names an explicit pane program and its argv (never `$SHELL`/the login shell), a throwaway `HOME` the tool creates per run and deletes after, the sampling interval (default 100ms, overridable), and a scripted timeline of steps (key sequence, wait, or focus change).
- `just capture-scenario <name>` builds and runs the scenario bin, writing the step stills and, when the scenario has more than one sampled frame, `recording.gif` (via the `image` crate's own GIF encoder - already pulled in by `HeadlessAppContext`'s `RgbaImage`, no new dependency). Never part of `just check`, same reason `capture-gallery` isn't; `just check-packages` still compiles the scenario bin alone.
- THIS TASK'S OWN PROOF SCENARIO: a trivial scenario, but not a static one - it must have motion (so point 1's fixed-interval sampling is actually exercised and proven here, not deferred to the dissolve) and a real pane program behind a real pty (so point 2's bounded-wait-on-real-output is actually exercised and proven here). Proposal: a tiny fixed program that prints one line, waits, prints a second line, and exits - never `$SHELL` - with the scenario's own scripted timeline waiting on the model holding each line's expected content (the bounded event from point 2) before advancing, and a short extra wait with nothing happening on either side of it, purely to prove the fixed-interval sampler produces more than one frame across a quiet period. Proof itself: run `just capture-scenario` against this proof scenario twice and diff every output PNG and the recording byte-for-byte - same bytes both times, proving the sampling, the bounded wait, and the throwaway-HOME/explicit-program handling are all free of anything that varies run to run. Also: review the new code for any reference to `NSWindow`/`CGWindowID`/`screencapture`/`NSPasteboard`/`CGEvent` - none should exist.

WHAT THIS TASK ADDS ON TOP OF TASK-1.154's CRITERION 3: unchanged from before - TASK-1.154 AC#3 proves the shipped `steg-app` binary never carries `test-support`; this task re-runs that same check once its own new scenario `[[bin]]` exists, as its own regression proof, not a new check.

WHAT THE DISSOLVE SCENARIO (DRAFT-3) WILL NEED FROM THIS HARNESS: the fixed-interval sampler and GIF assembly from point 1 (unchanged - the dissolve's own motion is still frames over time, however its phase ends up being computed); the bounded-wait-on-real-pane-output primitive from point 2, for the real attached pane the dissolve scenario needs; and, because of the `Instant::now()` finding above, very likely a way for the harness to set or hold fixed whatever "now" TASK-1.127's own fix ends up reading - this task should keep that door open (not build it) by not hard-coding `Instant::now()` anywhere in the harness's own frame-sampling loop, so DRAFT-3's own plan can decide the exact shape once it knows what TASK-1.127's fix looks like.

(d) TEAM RULE WORDING (unchanged - proposed here, the team lead opens the separate PR): "a UI pull request's image (and, for motion, its recording) comes from `just capture-scenario <name>`, not a worker launching the app or capturing the screen." The worker brief's existing ban stays exactly as written.

ENGINEER'S DECISIONS THAT STILL APPLY HERE:

2. Keep `test-support` off the shipped `steg-app` binary via a second `[[bin]]`, relying on TASK-1.154's own per-target feature unification and `capture-gallery`'s precedent. Recommend: yes.
3. Scenario schema (above, including the new `sample_interval_ms` field) is a proposal; this task's own build-phase plan pins exact field names. Recommend: approve the shape.
4. Recording via the `image` crate's own GIF encoder. Recommend: yes - no new dependency.

RULES TO RESPECT: unchanged from the previous pass - `.claude/rules/architecture.md` (the lib+bin reach inside the app crate, justified above), `.claude/rules/licence.md` (nothing new beyond what TASK-1.154 already clears), `tools/` is Python and the scenario runner is not it.

PROOF PER THIS TASK'S OWN ACCEPTANCE CRITERION (AC#3, the cursor-dissolve fidelity criterion, now belongs to DRAFT-3, not this task - noted so a reviewer doesn't look for it here):

- AC#1 (no window, no input injection, no screen capture, no clipboard, on the machine it runs on): proved by running `just capture-scenario` on the proof scenario and by code review for the OS-surface references listed above.
- AC#2 (scenario as data - explicit program, throwaway HOME, never the login shell, scripted timeline on a controlled clock, deterministic, nothing personal): proved by the proof scenario's own file (reviewable) plus the two-run byte-identical check above.
- AC#4 (team rules point at this tool; worker brief's ban stays): proved by the diff itself in (d), landed as the team lead's own separate PR.

UNKNOWNS THAT STILL APPLY: whether steg-sdk already exposes a bounded content-wait primitive the harness can reuse directly, or whether the harness has to build its own against the model's own notification stream (a build-task-stage detail, not a route question); whether the real local-socket server autostart needs anything beyond the throwaway-HOME convention this repo already uses elsewhere for live runs; GitHub-hosted macOS runner GPU/window-server support - still not established and still not needed, since AC#1 reads "on the machine it runs on" and the local recipe is what a worker actually runs.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
22 Nov 2026 (@bruk.o): the engineer accepted the GPUI move as TASK-1.154 (git pin on zed-industries/zed; gpui-pre only as a fallback that comes back to the engineer). TASK-1.151 now depends on it and keeps parts (b), (c) and the rule wording (d) of its plan; part (a) is TASK-1.154's.

Plan checked against the task's criteria by @bruk.o (22 Nov 2026, plan a482bf1aa9d2): not yet for approval. (1) Recording: stills per scripted step do not make a recording of motion - a wait step would give one frame; the harness needs frames sampled at a fixed interval of the tool's clock across waits, and the plan must say so. (2) Determinism: the pane program, pty and server run on real threads and real time, outside the tool's clock; the plan must say how the harness waits for pane output (an event with a bound, no polling, no sleep) before advancing, and must read how the dissolve's timer and phase are driven in terminal_element.rs (executor timer or wall clock) since byte-identical frames depend on it. (3) Split: the dissolve scenario is drafted as its own task (DRAFT-3, depends on this one) carrying criterion 3; once the engineer accepts it this plan covers the harness only, proved on a trivial scenario that includes motion.
<!-- SECTION:NOTES:END -->
