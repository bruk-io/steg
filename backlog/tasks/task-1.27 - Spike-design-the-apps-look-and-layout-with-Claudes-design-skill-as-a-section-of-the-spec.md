---
id: TASK-1.27
title: >-
  Spike: design the app's look and layout with Claude's design skill, as a
  section of the spec
status: In Progress
assignee:
  - '@bruk.w6'
created_date: '2026-11-14T00:16:00Z'
updated_date: '2026-11-14T03:53:00Z'
labels:
  - size-3
milestone: m-0
dependencies: []
parent_task_id: TASK-1
type: spike
project: bruk-io-steg
asdlc_accepted_from: DRAFT-28
asdlc_branch: task-1.27-spike-design-the-app-s-look-and-layout
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The spec says what the app shows but not what it looks like: R20 (a window with a session switcher, tabs and panes), R21 (the command palette), R24 and R25 (light and dark, and a near-black base with warm gradient accents, violet through pink to orange, in the spirit of the Superlogical site and early Arc, with no Liquid Glass), R26 (a colour per session), R27 (terminal themes) and R50 (how a tab shows a program's status). Words cannot settle those, and the app tasks would each guess. This uses Claude's design skill to produce the visual design, and turns what Bruk accepts into a design section of the spec that the app tasks build from. It delivers a design and spec text, not code.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A design made with Claude's design skill shows the main window in dark and light: session switcher, tabs, split panes, the active pane, and a tab showing each program status
- [ ] #2 The design shows the command palette and a session carrying its own colour
- [ ] #3 The spec (doc-1) has a design section giving the colours, type, spacing and states as named values an app task can build from, with a link to the design
- [ ] #4 Bruk has seen the design and recorded what he accepted
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
## Direction, and what this worker is given

Bruk settled the three open questions (his answers are a comment on this task): the part of the spec is the app's visual design; steg has a look of its own, closer to Rex, not bh-01; and the design is a Design artifact, not a hand-written HTML mock.

This worker has no Artifact tool, so it neither makes nor reads the artifact. @bruk.o makes and publishes it and hands over its values and its URL. Everything below starts from that handover, and nothing in it invents a value: `design/tokens.json` is a transcription, the artifact is the source, and the design document says the artifact wins if the two ever disagree.

**The handover, and where it arrives.** Preferably a comment on this task, because it stays on the board where review can see what the file was transcribed from; otherwise a file at `design/from-artifact.md` in the worktree, which this worker commits as the record. Phase 2 cannot begin before it arrives: no values, no work.

**What the handover has to carry**, by requirement, for `design/tokens.json` to be complete. A value missing here is reported to @bruk.o, never filled in by this worker.

- R24, R25: dark and light UI surfaces (base, raised, overlay, border, border-strong); UI text (primary, secondary, muted, on-accent); the accent gradient's ordered stops, violet through pink to orange.
- R25, the gradient's geometry, not only its stops: for each place the gradient is allowed, the element, its width and height, the angle, and the stop positions within that box. GPUI draws a linear gradient into one quad, and three stops squeezed into a 2px underline read as mud, so a place too small for three stops takes a single colour sampled from the gradient and the handover says which.
- R23: the terminal font stack, size, line height, ligature default and fallbacks, which are the defaults a user changes; and the UI font with its sizes for the tab label, switcher label, palette row and status text.
- Spacing and shape: an 8px step scale, the switcher rail width, tab bar height, tab minimum and maximum width, pane border width, pane gap, palette width, palette row height, the corner radii, and the focus ring.
- R20, R21: how the active pane differs from an inactive one, and the palette's row anatomy (title, key binding, source), its match marking and its empty state.
- R26: the session accent presets, where a session's colour shows (switcher item, window chrome), and the composition rule that holds when a running agent sets an arbitrary colour from the CLI. A rule, not just swatches: an accent carries no text, it appears as a fill or bar behind no text and as a derived surface tint, and text near it comes from the UI text tokens.
- R27: two terminal themes, dark and light, each with 16 ANSI colours plus foreground, background, cursor and selection, kept apart from the chrome tokens so a theme swaps without touching the chrome.
- R50: the four states (working with progress, blocked, done, error), each with a glyph, a word and a colour, and what the switcher shows for a session holding a blocked or finished program.

## Files that change

The repository has no `design/` yet. Its rules (`.claude/rules/architecture.md`, `.claude/rules/licence.md`) are scoped to `**/*.rs` and `**/Cargo.toml`, so none of them governs these.

- `design/tokens.json` (new): every value from the handover, as data, one object per group above. Each value carries the requirement it serves and the release that requirement belongs to (doc-1 section 6: R20, R21, R23, R24 and R25 are M1; R26 and R27 are M2). The release matters because TASK-1.20 is M1 and would otherwise not know whether the per-session accent machinery is its scope.
- `design/contrast` (new, executable, PEP 723 inline metadata run through uv, per `~/.claude/rules/python-scripts.md`): reads `design/tokens.json` and prints the WCAG contrast ratio of every declared text-on-surface pair in both themes. It gates the chrome's text pairs, the focus ring, pane borders, status glyphs and each terminal theme's default foreground on its background, exiting non-zero below threshold: 4.5:1 for body text and the terminal default, 3:1 for large text and for non-text boundaries. It reports the 16 ANSI colours' ratios against their terminal background and marks any under 3:1 without failing: normal-intensity ANSI blue on a near-black base fails 3:1 in every dark terminal theme there is, and gating it would push the theme into colours that no longer read as a terminal's. The README says which group is gated and which is reported, and why.
- `design/README.md` (new): what each file is, the artifact's URL and the date its values were read, the ratios the script printed, the GPUI primitive list (solid fills, linear gradients, rounded rectangles, borders, text, opacity, and nothing else: no backdrop blur, no macOS materials, no shadow stacks, no blend modes), and the sentence that these values are a proposal until Bruk's acceptance is recorded on this task.
- `design/from-artifact.md` (new, only if the handover comes as a file): written by @bruk.o, committed by this worker unchanged as the record of what was transcribed.
- `justfile` (one new recipe): `design-contrast`, running `design/contrast` through the existing `{{quiet}}` helper so it prints one line like every other step. `check` is deliberately not touched: changing the command every other worktree and CI runs is TASK-1.23's ground, and these values are accepted by a person, not by a linter.

Created by command, never by editing `backlog/` by hand:

- A new board document, `asdlc board doc create "Design: the app's look and layout" -t specification`, holding the design section's text: the named-value tables generated from `design/tokens.json`, one row per value with its requirement and its release; the four status states; the session-colour composition rule; the accessibility numbers; the GPUI primitive list; and the artifact's URL. It follows TASK-1.4, which delivered doc-2 rather than editing doc-1. Linked with `asdlc board task edit TASK-1.27 --add-doc <doc id>`.
- This task's notes and comments.

## Order of work

1. Before any file is edited, record where the repository starts: `asdlc record-check TASK-1.27 --in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.27 --as bruk.w6 -- just check`.
2. Read the handover. Check it against the list above, group by group. If anything is missing, write what is missing on this task for @bruk.o and stop: a gap is reported, not invented.
3. Write `design/tokens.json` by transcribing it, each value tagged with its requirement and release.
4. Write `design/contrast` and the `design-contrast` recipe, and run the recipe from the worktree root to see it pass. The justfile sets `set shell := ["bash", "-euo", "pipefail", "-c"]`, so the recipe's path is relative to where `just` runs; prove it rather than assume it.
5. If a gated pair fails, report the pair, both colours and the ratio to @bruk.o so the artifact changes and the corrected values come back. Never re-tint a token locally and never relax a threshold: that is how the file and the artifact drift apart.
6. Write `design/README.md` with the ratios step 4 printed, and the artifact's URL and read date.
7. Create the board document, generate its tables from `design/tokens.json` so the two cannot disagree by typo, link it to this task, and state in it that the artifact wins over both.
8. Check the values against the requirements, one row at a time: R20, R21, R23, R24, R25, R26, R27, R50. A requirement with no row is a gap for step 2's treatment, not a note. Also check that the artifact itself shows what AC#1 and AC#2 ask for (both themes with switcher, tabs, nested splits and the active pane; a tab in each of the four states; the palette; a session carrying its own colour), and record in the document which part of the artifact answers which criterion.
9. Finish: commit with the `Asdlc-Worker: bruk.w6` trailer, fetch, merge `origin/main`, record `just check` and `just design-contrast`, push the branch, open the pull request against `bruk-io/steg`, and `asdlc finish` with notes naming the artifact, the ratios, any ANSI pair marked as low, and the two steps only @bruk.o and Bruk can take. Then stop.

## Risks

- **The handover is a hard dependency.** No values, no work. Step 2 is a stop-point, not a wait.
- **The file and the artifact drifting apart.** The artifact is the source and the file is a transcription, the same way bh-01 keeps its spec above its implementations. Step 5 sends a contrast failure back to the artifact rather than fixing it locally; step 7 writes the precedence down.
- **The artifact may not carry everything `design/tokens.json` needs.** Line heights, the 16 ANSI colours and the gradient's geometry are the ones most easily left out of a visual design. That is why the list above is explicit: it is the brief as much as a checklist.
- **The artifact may show what GPUI cannot draw.** A design canvas will render blur, materials, shadow stacks and blend modes happily. This worker checks the values against the primitive list and reports what falls outside it; the real proof is the design drawn in GPUI, which belongs to TASK-1.15 and TASK-1.20. For the orchestrator: a `gpui-app` plugin sits in Bruk's plugin cache with a `gpui-dev-loop` skill and a `gpuictl shot` harness that screenshots a running GPUI app. It is not enabled for this worker, and scaffolding a GPUI app here would collide with TASK-1.5 and TASK-1.15, so it is not part of this spike. It is how those tasks should check the look.
- **An arbitrary session colour can quietly break contrast.** R26 lets a running agent set any colour from the CLI, so the composition rule in the handover list matters more than the presets do. If the handover gives presets but no rule, that is a step 2 gap.
- **The artifact is not in git.** Its URL can go stale and its canvas can change under the document. `design/tokens.json` and `design/README.md` keep the values and the read date in git, so a later reader can tell what was accepted from what the artifact says today.
- **Values land in the repository before Bruk has accepted them.** Each file says so, and the sentence comes out when the acceptance comment goes on the task.

## Proof

- **Baseline and final checks.** `asdlc record-check TASK-1.27 --in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.27 --as bruk.w6 -- just check`, before the work and after it. Honestly: with no `Cargo.toml` in the tree every Rust step prints `skip` and passes, so a passing `just check` says nothing about whether this design is right. It is recorded because the workflow needs the baseline, not as evidence.
- **Accessibility, which both themes need.** `asdlc record-check TASK-1.27 --in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.27 --as bruk.w6 -- just design-contrast`. It exits non-zero on any gated pair below threshold, so the claim is a number and not prose. Status not resting on colour alone is proved by enumeration: each of the four states has a glyph and a word in `design/tokens.json` and in the document, and a state missing either fails step 8.
- **AC#1 and AC#2** are answered by the artifact @bruk.o publishes, not by this worker. What this worker proves is the mapping: the document names the artifact's URL and which part of it shows each criterion, checked in step 8.
- **AC#3, the part a worker can do.** The board document exists, is linked to this task, and has one row per named value with its requirement and release, covering R20, R21, R23, R24, R25, R26, R27 and R50 with no requirement unrowed. AC#3 closes only when @bruk.o folds that text into doc-1 as its design section.
- **AC#4.** Not provable by this worker. It closes when Bruk has seen the artifact and his acceptance is recorded as a comment on this task. The plan stops at step 9 for that reason.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Onboarding skipped by @bruk.o: bruk-io-steg has no repository plugin yet and no code to read layers or verify commands from. TASK-1.24 onboards it once the workspace exists. The repository's own rules are in .claude/rules/ and its commands are in the justfile.

Plan checked against doc-1 by @bruk.o: consistent with R20, R21, R23 to R27 and R50 as far as it goes, and held, not ok to approve yet. It rests on three questions only Bruk can answer, which the worker put on the task as a comment: whether 'a part of the spec' means the app's visual design, whether steg follows bh-01 or has a look of its own, and whether a hand-written HTML page is the design he wants or the canvas Design artifact, which a worker cannot publish. The plan's own proof is sound: named values in one tokens file, a contrast check that fails on a bad pair, and the design section generated from the tokens. Two criteria cannot be closed by a worker: the design being made with Claude's design skill, and Bruk's recorded acceptance.

Plan revised by @bruk.w6 after Bruk's answers, 2026-11-13, and rewritten as one text (plan 29831ed076eb, not yet approved). What changed from the first version: the three open questions are gone, settled; the hand-written HTML mock (`design/look/index.html`) is dropped, because the design is a Design artifact @bruk.o makes and publishes; this worker's part is now a transcription of the artifact's values into `design/tokens.json` plus the contrast check, the README, the board document and the requirement-by-requirement check. The files moved from `design/look/` to `design/`. Phase 2 cannot start before @bruk.o hands over the artifact's values and URL: the plan lists, requirement by requirement, what that handover must carry, and a missing value is reported back rather than invented. A gated contrast failure also goes back to @bruk.o so the artifact changes, so the file and the artifact cannot drift.
<!-- SECTION:NOTES:END -->

## Comments

<!-- COMMENTS:BEGIN -->
author: @bruk.w6
created: 2026-11-14T02:41:00Z

---

Plan written by @bruk.w6. Three questions for Bruk, which plan approval is the cheapest place to answer, since nothing is built before it:

1. Is "a part of the spec" the app's visual design? The plan assumes it is (R20, R21, R23, R24, R25, R26, R27, R50). That reading is @bruk.o's and is unconfirmed. If it is a different part of the spec, this plan does not apply.
2. Does steg follow bh-01, or have a look of its own? The plan proposes a look of its own, for two reasons: a worker has no Artifact tool, so bh-01's values cannot be read at all (its own skill forbids reconstructing them from memory), and bh-01's warm neutrals with one mandarin signal pull against R25's near-black base with a violet-through-pink-to-orange gradient. If the answer is bh-01, this task stops and the design has to be made in a session that can read that artifact.
3. Is a hand-written HTML page, opened in a browser, the design Bruk wants to look at? A worker cannot publish a canvas Design artifact; the design guidance skills are reachable here but the Artifact tool is not. The plan says so rather than passing HTML off as a design tool's output, and names what has to happen in a session with the Artifact tool if Bruk wants one.

---

created: 2026-11-14T03:51:00Z

---

Bruk's answers, 2026-11-13, relayed by @bruk.o. Q1: yes, 'a part of the spec' is the app's visual design. Q2: not bh-01; a look of steg's own, closer to Rex. Q3: the design is a Design artifact, not a hand-written HTML mock. A worker cannot publish one, so @bruk.o's session makes and publishes the Design artifact and hands its values to the worker; the plan's design/look/index.html is dropped, and the tokens file, contrast check and design document are built from the artifact's values.

---
<!-- COMMENTS:END -->
