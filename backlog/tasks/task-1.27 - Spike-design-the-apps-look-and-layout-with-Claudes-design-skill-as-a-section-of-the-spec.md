---
id: TASK-1.27
title: >-
  Spike: design the app's look and layout with Claude's design skill, as a
  section of the spec
status: In Progress
assignee:
  - '@bruk.w6'
created_date: '2026-11-14T00:16:00Z'
updated_date: '2026-11-14T02:42:00Z'
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
## Three things to settle before anything is built

Plan approval is the cheapest moment for Bruk to answer these. Nothing below is built until he does.

**Q1. The reading of the ask.** Bruk asked for "a spike that will help us generate a part of the spec which will be done with claude design skill". This plan reads "part of the spec" as the app's visual design, serving R20, R21, R23, R24, R25, R26, R27 and R50. That reading is @bruk.o's and Bruk has not confirmed it. If he meant a different part of the spec, stop here: none of the work below applies.

**Q2. bh-01, or a look of steg's own.** bh-01's own skill says its values live in a Design System artifact on claude.ai, that the Artifact tool is the only way to read them, and that if it cannot be read, say so and "Do not reconstruct values from memory". This worker has no Artifact tool, so it cannot read bh-01's tokens; that is a hard dependency, not a preference. bh-01's published direction also pulls against R25: bh-01 is warm neutrals with one mandarin signal colour and a pine secondary, where R25 asks for a near-black base with a violet-through-pink-to-orange gradient. So this plan proposes a look of steg's own, and borrows two of bh-01's habits rather than its values: status is always a word or glyph plus a colour, and hex values are never retyped by hand. If Bruk wants steg to follow bh-01, this task stops and the design is made in a session that can read that artifact.

**Q3. What "Claude's design skill" can be in a worker's hands.** Checked, not assumed: the design guidance skills are reachable here (artifact-design was loaded in this session, alongside artifact-diagramming, dataviz and bh-01), and the Artifact tool that publishes a canvas Design artifact is not in this worker's tool set. It is a tool gap, not a skill gap. So AC#1's words "made with Claude's design skill" cannot be met literally by this worker. Said plainly rather than substituted quietly: the design will be a hand-written standalone HTML page, built to artifact-design's own rules (a token block at the top, both themes defined at token level, no colour defined only inside a theme block), which Bruk opens in a browser. Two things about that beat a canvas artifact for this job: it lives in git, so TASK-1.15 and TASK-1.20 build from a file that can be diffed and reviewed, and the tokens beside it are machine-readable so those tasks generate Rust constants instead of retyping hex. If Bruk also wants the canvas Design artifact, the handover is named in step 8: a session with the Artifact tool publishes the page, after the pass the artifact page contract needs (a published page drops its own doctype, head and body wrapper and takes the two max() safe-area paddings). The mock as written is a desktop-width local file.

## Files that change

All four design files are new; the repository has no `design/` yet. The repository's own rules (`.claude/rules/architecture.md`, `.claude/rules/licence.md`) are scoped to `**/*.rs` and `**/Cargo.toml`, so none of them governs these.

- `design/look/tokens.json` (new): every named value, as data. One object per group, each value tagged with the requirement it serves. Groups: UI surfaces dark and light (base, raised, overlay, border, border-strong); the accent gradient (ordered stops with positions, the angle, and the named places it may appear); UI text (primary, secondary, muted, on-accent); the four status colours; the session accent presets and the rule for an arbitrary one; type (the terminal font stack, size, line height and ligature default for R23, and the UI font with its sizes for tab label, switcher label, palette row and status text); spacing (an 8px step scale, the switcher rail width, tab bar height, tab minimum and maximum width, pane border width, pane gap, palette width and row height, corner radii); the focus ring; and two terminal themes for R27 (16 ANSI colours plus foreground, background, cursor and selection for dark and for light), kept apart from the UI tokens so a theme can be swapped without touching the chrome.
- `design/look/contrast` (new, executable, PEP 723 inline metadata run through uv per `~/.claude/rules/python-scripts.md`): reads `tokens.json` and prints the WCAG contrast ratio of every declared text-on-surface pair in both themes, exiting non-zero when a pair is below its threshold. Thresholds: 4.5:1 for UI body text and for each terminal theme's default foreground on its background, 3:1 for large text, the focus ring, pane borders and status glyphs. The 16 ANSI colours are reported against their terminal background and required to reach 3:1, with the honest note that a program chooses those colours and steg cannot promise what contrast a program asks for.
- `design/look/index.html` (new): the mock. One page, dark first and light beside it, sections in this order - the main window (session switcher with three differently coloured sessions, tab bar, a tab split into three panes with nested splits, the active pane distinguished from the inactive ones); a tab bar row showing each of R50's four states (working with progress, blocked, done, error), each as a glyph plus a word plus a colour; the session switcher showing which sessions hold a blocked or finished program (R50's second sentence); the command palette over the window (R21), with its row anatomy of title, key binding and source, the matched characters marked, and its empty state; the same window under a second and third session colour (R26); the swatch and type tables, each swatch labelled with its token name; and the list of GPUI primitives the design is allowed to use.
- `design/look/README.md` (new): what each file is, how to open the mock, the contrast numbers the script printed, the GPUI primitive list (solid fills, linear gradients, rounded rectangles, borders, text, opacity, and nothing else - no backdrop blur, no macOS materials, no shadow stacks, no blend modes), and the sentence that these values are a proposal until Bruk's acceptance is recorded on this task.
- `justfile` (one new recipe): `design-contrast`, running `design/look/contrast` through the existing `{{quiet}}` helper so it prints one line like every other step. `check` is deliberately not touched: changing the command every other task's worktree and CI runs is TASK-1.23's ground, and these values are accepted by a person, not by a linter. Rejected alternative: putting the checker in `tools/src/steg_tools/` so `just check`'s `test-tools` step covers it. It would be enforced, but it reshapes the build tooling inside a design spike; if Bruk wants it enforced, that is a one-line follow-up task.

Created by command, never by editing `backlog/` by hand:

- A new board document, `asdlc board doc create "Design: the app's look and layout" -t specification`, holding the design section's text: the named-value tables with one row per value tagged with its requirement, the status states, the session-colour composition rule, the accessibility numbers, the GPUI primitive list, and a link to `design/look/index.html`. It follows the precedent of TASK-1.4, which delivered doc-2 rather than editing doc-1. Linked with `asdlc board task edit TASK-1.27 --add-doc <doc id>`.
- This task's notes and comments.

## Order of work

1. Before any file is edited, record where the repository starts: `asdlc record-check TASK-1.27 --in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.27 --as bruk.w6 -- just check`.
2. Write `design/look/tokens.json` with the groups listed above. Every value gets its requirement tag, so the completeness check in step 7 is a table lookup and not a judgement.
3. Write `design/look/contrast` and the `design-contrast` recipe. Run it. Where a pair fails, change the token, never the threshold. Run it again after every later change to `tokens.json`.
4. Write `design/look/index.html` from the tokens, in the section order listed above. Nothing in the page uses a CSS feature outside the GPUI primitive list, which is what keeps the mock honest about what the app can draw: HTML will happily render a backdrop blur or a stack of shadows that GPUI cannot.
5. Write `design/look/README.md`, with the ratios step 3 printed.
6. Create the board document, write its text from `tokens.json` so the two cannot disagree by typo, link it to this task, and state in it which wins if they ever do: the document.
7. Check the named values against the requirements, one row at a time: R20 (switcher, tabs, panes, active pane), R21 (palette), R23 (terminal font, size, line height, ligatures, fallbacks - easy to forget in a colour-led design), R24 (both themes complete), R25 (near-black base, the gradient stops, no materials), R26 (a session's colour and how it composes), R27 (the two terminal themes), R50 (the four states and the switcher's summary). A requirement with no row is a gap to fill before finishing, not a note.
8. Finish: commit with the `Asdlc-Worker: bruk.w6` trailer, fetch, merge `origin/main`, record `just check` and `just design-contrast`, push the branch, open the pull request against `bruk-io/steg`, and `asdlc finish` with notes that name what Bruk has to look at, the answers to Q1 to Q3 as they stood, and the two steps only the orchestrator and Bruk can take: recording what Bruk accepted (AC#4), and folding the document into doc-1 as its design section (AC#3). Then stop.

## Risks

- **The reading of the ask is unconfirmed, and it is the whole task.** Q1 above. Cheapest mitigation: it is answered at plan approval, before any work. If it is wrong, nothing is lost but this plan.
- **AC#1 cannot be met by a worker as written.** Q3 above. The mock is hand-written HTML; a canvas Design artifact needs a session with the Artifact tool. Bruk decides at approval whether the HTML page is what he wants to look at, or whether the design must be made in his own session from this plan's brief.
- **No reference image, and no trustworthy memory of early Arc's palette.** This worker has neither the image Bruk shared nor a reliable recollection of Arc's colours, so the design will propose explicit stops and let Bruk judge them. Nothing written will claim to match the reference.
- **HTML flatters the design.** A browser renders blur, shadow stacks and blend modes that GPUI does not. The primitive list and step 4's rule guard against it on paper only; the real proof is the design drawn in GPUI, which belongs to TASK-1.15 and TASK-1.20. Worth naming for the orchestrator: a `gpui-app` plugin sits in Bruk's plugin cache with a `gpui-dev-loop` skill and a `gpuictl shot` harness that screenshots a running GPUI app. It is not enabled for this worker, and scaffolding a GPUI app here would collide with TASK-1.5 and TASK-1.15, so it is not part of this spike. It is how those tasks should check the look.
- **A per-session accent can quietly break contrast.** R26 lets a running agent set any colour from the CLI, so an arbitrary accent must compose with a near-black base and the gradient without dropping any text pair below AA. The design will carry a stated rule, not just swatches: an accent never carries text, it appears as a fill or bar behind no text and as a derived tint for a surface, and every text colour that sits near it comes from the UI text tokens. Presets are given for the common case.
- **Values land in the repository before Bruk has accepted them.** Each file says so, and the sentence comes out when the acceptance comment goes on the task.
- **Two sources of the same numbers.** `tokens.json` and the board document both carry values. Step 6 generates the document's tables from the file, and the document is named as the one that wins, the way bh-01 handles the same problem.

## Proof

- **Baseline and final checks.** `asdlc record-check TASK-1.27 --in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.27 --as bruk.w6 -- just check`, before the work and after it. Honestly: with no `Cargo.toml` in the tree, every Rust step in the justfile prints `skip` and passes, so a passing `just check` says nothing about whether this design is right. It is recorded because the workflow needs the baseline, not as evidence.
- **AC#1.** `design/look/index.html` shows the main window in dark and in light, each with the session switcher, the tab bar, a tab of nested split panes and the active pane marked, and a tab bar row carrying all four of R50's states. Checked by opening the file; which section shows which criterion is listed in `design/look/README.md`.
- **AC#2.** The same page shows the command palette over the window and the same window under three different session colours.
- **Accessibility, which both themes need.** `asdlc record-check TASK-1.27 --in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.27 --as bruk.w6 -- just design-contrast`. It exits non-zero on any pair below its threshold, so the claim is a number and not prose. Status not resting on colour alone is proved by enumeration: each of the four states has a glyph and a word in `tokens.json` and in the mock, and a state missing either is a failure of step 7.
- **AC#3, the part a worker can do.** The board document exists, is linked to this task, and has one row per named value tagged with the requirement it serves, covering R20, R21, R23, R24, R25, R26, R27 and R50 with no requirement unrowed. AC#3 closes only when the orchestrator folds that text into doc-1.
- **AC#4.** Not provable by this worker. It closes when Bruk has looked at the mock and his acceptance is recorded as a comment on this task, by the orchestrator. This plan stops at step 8 for that reason.

## Corrections to the sections above, which win where they disagree

Three fixes made before approval, after reading the plan back against R25, R27 and the release table in doc-1 section 6.

1. **The 16 ANSI colours are reported, not gated.** The Files section says they are "required to reach 3:1". That is wrong and would do harm: normal-intensity ANSI blue on a near-black base fails 3:1 in every dark terminal theme there is, and step 3's rule of changing the token rather than the threshold would push the theme into colours that no longer read as a terminal's. So `design/look/contrast` gates only the app chrome's text pairs, the focus ring, pane borders, status glyphs and each terminal theme's default foreground on its background. It prints every ANSI colour's ratio and marks the ones under 3:1 without failing on them. The README says which group is gated and which is reported, and why.
2. **Every named value carries the release it belongs to.** The tables in the board document get a release column, M1 or M2, read from doc-1 section 6: R20, R21, R23, R24 and R25 are M1, and R26 and R27 are M2. Without it TASK-1.20, which is M1, cannot tell whether the per-session accent machinery is its scope; with it, TASK-1.20 builds the M1 rows and the M2 rows wait. The mock still shows the M2 states, marked as M2, because AC#1 and AC#2 ask for them.
3. **The gradient is given a geometry, not only stops.** A three-stop gradient squeezed into a 2px underline reads as mud, and GPUI draws a linear gradient into one quad, so "where it may appear" has to name the element and its size: for each place the gradient is allowed, `tokens.json` gives the element, its width and height in the mock, the angle, and the stop positions in that box. A place too small to carry three stops takes a single colour sampled from the gradient instead, and the file says which. This is the one place where the HTML mock can look right while the GPUI build looks wrong.

One thing to verify rather than assume in step 3: the justfile sets `set shell := ["bash", "-euo", "pipefail", "-c"]`, so the `design-contrast` recipe's path to `design/look/contrast` is relative to wherever `just` runs. Run the recipe from the worktree root in step 3 and see it pass before relying on it.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Onboarding skipped by @bruk.o: bruk-io-steg has no repository plugin yet and no code to read layers or verify commands from. TASK-1.24 onboards it once the workspace exists. The repository's own rules are in .claude/rules/ and its commands are in the justfile.
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
<!-- COMMENTS:END -->
