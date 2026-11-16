---
id: TASK-1.92
title: 'ui: the design''s named values as one data file, exported from the artifact'
status: In Progress
assignee:
  - '@bruk.w6'
created_date: '2026-11-16T00:52:00Z'
updated_date: '2026-11-16T01:05:00Z'
labels:
  - size-3
milestone: m-4
dependencies: []
parent_task_id: TASK-1
project: bruk-io-steg
asdlc_accepted_from: DRAFT-2
asdlc_branch: task-1.92-ui-the-design-s-named-values-as-one-data
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
doc-17 (R121, R122, the section 'The design, and where its values are today'), decision-10, decision-11. The design artifact holds its foundations as drawn pages only; the data and generator scripts behind them were working files of the session that built it. The orchestrator checked on 2026-11-15 that the copies at /Users/brukhabtu/.claude/projects/-Users-brukhabtu-src-github-com-bruk-io-steg/design-canvas-backup-2026-11-15 (foundations.json, gen_system.py and the rest, and project/Tokens.dc.html and Details.dc.html) are identical to the artifact's version 1794702644-d50e for the two foundations pages and that all 17 colours appear in the artifact's page. A worker cannot read the artifact itself: work from that copy, and the orchestrator re-reads the artifact before the pull request merges. Whether the data is also published into the artifact is an open question in doc-17 for the engineer and not this task's.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 crates/ui/design/foundations.json holds every named value of the design's foundations under the design's own names: the colours with their light and dark forms, the two ramps, type, sizes, corners, durations, both terminal themes with their 16 ANSI colours and base colours, and the focus ring's measurements; and it records the artifact version it was exported from (R121, R122)
- [ ] #2 Values the design's working data holds as sentences (type, sizes, the ramps, the themes' base colours) are given a structured form: each is a number with its unit, a colour, or a list of them, and nothing a program must parse out of prose. The plan lists each such value with the form it takes, for the engineer, and anything the design does not actually fix is listed as a question and not guessed
- [ ] #3 The working data and the scripts that drew the design's foundations pages, and the saved copies of those pages, have a durable home in the repository, with a README saying what they are, which are current and which are history, and that the artifact is the source
- [ ] #4 No value is changed in the export: the pull request says so, and a reviewer can compare the data file with the artifact's pages by name
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
Likely two reviewable changes, not one (flagging per the team's default): (a) a hand-written data export plus a new durable home for the design session's working files — no executable behaviour, reviewed by comparing text to the saved pages; (b) a new Python tool with tests, wired into `just check`. AC#3 bundles them in one criterion, which argues against splitting, but their review shape differs enough that I'd split unless the engineer says otherwise. Proceeding with one plan; the engineer can split at approval.

No ordering dependency on TASK-1.91: `Cargo.toml`'s `[workspace] members` is an explicit list (`crates/sdk`, `crates/kernel`, `crates/server`, `crates/steg`, `crates/app`), not a glob, and does not yet list `crates/ui`. This task adds `crates/ui/design/` (plain files, no `Cargo.toml`) without touching that list, so cargo never looks at it; TASK-1.91 adds `crates/ui` to the members list itself when it creates the crate. Either task can merge first; they touch disjoint files under `crates/ui/`.

## 1. The exhaustive table (AC#2)

Source for all of this: `foundations.json`, `gen_system.py`, `rebuild_main.py`, and the rendered `project/Tokens.dc.html` / `Layout.dc.html` / `Details.dc.html`, all in the verified backup. "Today" = structured field already in `foundations.json`; "sentence" = a prose value inside `foundations.json`; "markup/CSS" = only in a generated page's HTML/CSS or in `gen_system.py`'s own constants, never in `foundations.json` at all.

### Colours

`foundations.json`'s `colours` array names 17 (confirmed against the artifact). But the pages use at least 20 named colour slots, so AC#1's "17 colours" undercounts what the pages actually draw — flagging, not guessing a fix:

| Name | Today | Dark | Light | Proposed structured form |
|---|---|---|---|---|
| the 17 named colours (surface.*, fill.*, outline, text*, ramp.1–6, session.blue) | structured, `colours[]` | as given | as given | unchanged shape, plus `artifact_version` at the top of the file |
| `track` (progress ring/bar background, terminal selection-adjacent) | markup/CSS only (`gen_system.py` `COL`/mock `c.track`) | `#3d3b45` | `#cfcbc5` | new colour entry. Dark value equals `outline`'s dark value but light differs (`#cfcbc5` vs `outline`'s `#bfbbb5`) — **not** an alias; question for the engineer whether that's intended or a slip |
| `off` ("cannot be used" in at least one control, per `.lv-ib:disabled`) | markup/CSS only | `#5f5d69` | `#a5a2ab` | new colour entry. **Question**: the Surfaces rule says disabled takes the muted text colour, never faded, but `.lv-ib:disabled` uses `off` while `.lv-btn:disabled` uses `muted` — the rule and two controls disagree with each other. Don't resolve; ask Bruk which is right |
| `faint` (the ring's unfilled/background opacity) | sentence, inside `rules` ("Faint is 30% in dark and 45% in light") | 0.3 | 0.45 | an opacity value, not a colour: `{name: "faint", dark: 0.30, light: 0.45}` |
| `blocked`, `done`, `error` (status aliases used by icons/CSS vars) | markup/CSS only, but **are aliases** | = `ramp.5`/`ramp.6`/`ramp.3` | same | export as references to the existing colour names, not new hex, per the Colour rule ("status and session colours... have no values of their own") |

### The two ramps

Sentence only (`ramp_text`), and derivable today only by a Rust side-effect of string-matching names starting `ramp.`/`session.` (fragile). Verified in `gen_system.py:366-369`:
- `ramp` (short): `[ramp.1, ramp.2, ramp.4]` (violet, pink, orange — **skips red, ramp.3**; state this, it's non-obvious), stops 0%, 50%, 100%.
- `ramp.extended`: `[ramp.1, ramp.2, ramp.3, ramp.4, ramp.5, ramp.6]`, stops 0/20/40/60/80/100% (even spacing; confirmed by `Layout.dc.html`'s hardcoded `ramp6` gradient).

Proposed form: two explicit named lists (ordered colour-name references + stop percentages), stored in the data file rather than reconstructed from naming convention in generated code — e.g. `ramps: [{name: "ramp", stops: [["ramp.1",0],["ramp.2",50],["ramp.4",100]]}, {name: "ramp.extended", stops: [["ramp.1",0],["ramp.2",20],["ramp.3",40],["ramp.4",60],["ramp.5",80],["ramp.6",100]]}]`.

Also sentence-only, two more ramp-derived values used in captions/headers:
- `ramp50`: the short ramp at 50% in dark / 40% in light (alpha suffix `80` dark / `66` light on the same three stops) — matches `ramp_text`'s "50% in dark and 40% in light across the active pane's caption." Structured form: an opacity pair on the `ramp` definition, `{dark: 0.50, light: 0.40}`.
- `tint`: a session colour at 18% (`c.acc + '2e'`), used as the open-session track's fill (confirmed in `Layout.dc.html`'s Track card: "fill: the session colour at 18%"). Structured form: `{name: "tint", opacity: 0.18}`, applied to whichever colour is the active session's.

### Type (7 entries, all sentences today)

| Name | Sentence | Structured form |
|---|---|---|
| `font.ui` | "The system font (San Francisco)" | **Question**: the pages themselves load Inter from Google Fonts and use the stack `-apple-system, BlinkMacSystemFont, 'Inter', sans-serif`. That stack is the artifact-preview's own scaffolding (so people without the real San Francisco/Inter installed still see something), not necessarily the shipped value. Ask which is the actual design value: the named system font, or the fallback stack as drawn |
| `font.terminal` | "JetBrains Mono, then SF Mono, then Menlo" | a list: `["JetBrains Mono", "SF Mono", "Menlo"]`. **Question**: the mock's own CSS instead falls back to `ui-monospace, monospace` — again likely preview scaffolding; confirm the three-font list is the real value |
| `terminal` | "13px on a 20px line, weight 400" | `{size: 13, unit: "px", line_height: 20, weight: 400}` |
| `ui.body` | "13px, weight 400; 600 when selected" | `{size: 13, unit: "px", weight: 400, weight_selected: 600}` |
| `ui.small` | "12px: counts, captions, key caps" | `{size: 12, unit: "px"}` (the "counts, captions, key caps" clause is a use-note, not a value) |
| `ui.label` | "11px, weight 600, upper case, 0.06em" | `{size: 11, unit: "px", weight: 600, text_transform: "uppercase", letter_spacing: 0.06, letter_spacing_unit: "em"}` |
| `palette.input` | "15px" | `{size: 15, unit: "px"}` |

### Sizes (10 entries, all sentences today) — every one splits into 2–3 fields

| Name | Sentence | Structured form |
|---|---|---|
| `space` | "Steps of 2, 4, 6, 8, 10, 12, 16" | a list: `[2, 4, 6, 8, 10, 12, 16]` px |
| `nav.side` | "232px wide; rows 30px, tabs indented 28px" | `{width: 232, row_height: 30, tab_indent: 28}` |
| `nav.top` | "40px high; tabs 30px in a track with a 2px inset" | `{height: 40, tab_height: 30, track_inset: 2}` |
| `tab.width` | "120px usual, 240px at most, 72px at least" | `{usual: 120, max: 240, min: 72}` |
| `pane.gap` | "6px between panes and to the window edge; grabbed across 12px" | `{resting: 6, grabbed: 12}` |
| `pane.caption` | "30px" | `{height: 30}` |
| `palette` | "640px wide, rows 40px, 120px from the top" | `{width: 640, row_height: 40, top_offset: 120}` |
| `radius` | "14px panes and palette; 10px tab track and session chips; 8px inside them" | three named corners, since the design gives three distinct sizes with no name of their own — **question for the engineer**: name them (e.g. `radius.surface`/`radius.track`/`radius.inner`) or keep them keyed by use (`{panes_and_palette: 14, track_and_chips: 10, inside: 8}`) |
| `status.ring` | "16px across, 2px thick; grey track" | `{diameter: 16, thickness: 2, track_colour: "track"}` (a reference to the new `track` colour, not a literal grey) |
| `scrim` | "Black at 60% in dark, ink at 28% in light" | **Question**: the Overlay story in `Layout.dc.html` draws `#05050899` — 60% alpha of `#050508`, not pure black. Sentence vs. drawing disagree on the base colour; ask which is right before structuring. Alpha values (`0.60` dark, `0.28` light) aren't in question |

### Corners

Covered above under `sizes.radius` plus the Track layout primitive's own rule ("the inner radius is the outer radius less the inset, so the corners stay parallel" — a *function*, not a value: `inner = outer - inset`, matches the Foundations test "a function from values to a value." Record the rule, not a duplicate literal).

### Durations and easings

The design fixes exactly one: **200ms**, the status fade (`rules` → Accessibility: "A change of status fades in over 200ms on both the ring and the bar"). Structured form: `{name: "motion.status-fade", duration: 200, unit: "ms"}`.

Everything else timing-shaped belongs to one of two places that are **not** this task's foundations data:
- The mock's own preview CSS (`80ms` hover transition, `120ms` tooltip fade, `500ms` tooltip delay, `1.6s` spin, `2.2s` slide) is artifact-preview scaffolding for the interactive mock, not a drawn foundations value — do not export these.
- Values doc-10 already lists as **not yet confirmed** (the 250ms roll-out, ~40 flakes over ten seconds each lying 2–6s, the pane header's three breakpoints, the smallest pane at 200×110px, the palette's 40px-clear/60%-height rule, the pressed-state darkening) — carry those as open questions by pointing at doc-10's list, don't re-litigate or re-decide them here.

No easing curve is fixed anywhere beyond `linear` on the two preview keyframe animations (which are scaffolding, see above); everything else is CSS's default `ease`. **The design fixes no easing** — say so, don't invent one.

### Both terminal themes

`foundations.json`'s `themes[]` already structures `bg`, `fg`, `cursor`, `selection`, and `ansi` (16 hexes) per theme — good shape, three gaps:

1. **ANSI naming.** The 16-array has no names. `gen_system.py:385` gives the order explicitly: `['black','red','green','yellow','blue','magenta','cyan','white']`, normal (0–7) then bright (8–15). Export each as `{name: "red", bright: false, hex: "..."}` (or an 8-key object with `normal`/`bright` pairs), not a bare array a consumer must index by convention.
2. **The inactive terminal background.** The `base` sentence packs two values the structured fields don't capture: "bg: the pane, #1c1b21 active, #141318 not" — the structured `bg` field only holds the *active* value (`#1c1b21`). The inactive value (`#141318`) equals `surface.pane`'s dark value, so propose it as a reference (`bg_inactive: "surface.pane"`) rather than a second literal, and ask the engineer to confirm that's intended (vs. an independent terminal-specific value that happens to match today).
3. **`cursor` is really "session colour," not a fixed hex.** The `base` sentence says "cursor: session colour," but the structured `cursor` field holds a fixed hex that equals `ramp.1`'s value (the default session colour). **Question, not to guess**: should the data file record `cursor` as a reference to "the active session's colour" (a function of runtime state, out of scope for a static foundations value) with `ramp.1` kept only as the *default session* fallback, or is freezing the default as if it were the theme's own value acceptable for now? Flag for the engineer; don't silently export the default as if it were the rule.

### The focus ring (R123's "measurements")

`focus_text` sentence: "A 2px ring in the session colour, 2px clear of the control... Its 12px corner follows the control's 8px corner at that distance." Two numbers plus a formula:
- `{width: 2, offset: 2}` (unit px), colour = the active session's colour (same open question as the terminal cursor, above — don't duplicate a hex, reference it).
- Corner radius is **derived**, not a third literal: `ring_radius = control_radius + offset * 2` (8 + 2×2 = 12, confirmed by the two other call sites in `gen_system.py` — `fring(m, r=12)` default for 8px controls, and `r=14` passed explicitly for the 10px-corner chip/handle: 10 + 2×2 = 14). Record the formula in `generated.rs`-facing docs, not a table of per-component radii.

### Layout page (Layout.dc.html) — what it holds beyond `foundations.json`

Per the brief: check what Layout holds that the data file doesn't. Findings, all sentence/markup-only today:

| Value | Where | Structured form / note |
|---|---|---|
| Row gap | markup | 6px in a tab, 8px in a caption or session row (default), 12px in a palette row |
| Row height | markup | 30px, or 40px in the palette (already implied by existing `sizes` entries; confirms, doesn't add) |
| Stack gap | markup | 2px between rows, 16px between groups |
| Stack indent | markup | 28px (already in `nav.side.tab_indent`; confirms) |
| Track inset / gap / radius / fill | markup | inset 2px, gap 2px, radius 10px outer / 8px items (already in `sizes.radius`; confirms), fill = session colour at 18% (= the new `tint` value above) |
| Split gap / smallest | markup | gap 6px drawn, 12px grabbed (already in `pane.gap`; confirms); smallest pane 200×110px — **doc-10 already lists this as not yet confirmed**; carry forward, don't freeze it here |
| Overlay width/top/height/scrim | markup | matches existing `palette` and `scrim` entries; the 720px/40px-clear breakpoint is **also already on doc-10's not-yet-confirmed list** — carry forward |
| Resizing vocabulary (`compact`/`regular`/`full`/`dropped first`/`when`) | markup | not a value at all — a vocabulary for how components describe their own breakpoints; out of scope for foundations.json, belongs in each atom/molecule's own Rust doc comments when built |

Net: Layout page adds one genuinely new value (tint/session-at-18%, already folded into the ramps section above) plus a handful of gap/indent numbers not yet named as their own entries (row gap variants, stack gap/indent) — the rest is confirmation of existing `sizes` entries or doc-10's existing open questions.

## 2. The data file's schema (AC#1, AC#2, R121, R122, R123)

`crates/ui/design/foundations.json` — **new content**, not a copy of the working `foundations.json` (different shape: structured, not sentences). Top-level keys:

- `artifact_version`: the string `"1794702644-d50e"` (R122), plus the artifact URL as a comment-equivalent field or in the README, not re-typed into the structured data.
- `colours`: the existing 17, in the existing shape (`name`, `dark`, `light`, `use`), plus `track`, `off`, `faint` (opacity, not colour), `blocked`/`done`/`error` as `{name, ref}` aliases.
- `ramps`: the two named ramps, each an ordered list of `{colour, stop}`, plus `ramp50`'s opacity pair and `tint`'s opacity.
- `type`: the 7 entries above, each replaced by its structured object.
- `sizes`: the existing 10 names, each replaced by its structured object (radius kept as a question pending naming), plus the Layout-only row/stack gap and indent values folded in under existing or new names.
- `motion`: the one duration the design fixes (`motion.status-fade`, 200ms) — a new top-level section; not called "durations" plural since there's exactly one.
- `themes`: the existing two, `ansi` reshaped to named entries, plus `bg_inactive` (as a colour reference) and a documented open question on `cursor`.
- `focus_ring`: `{width: 2, offset: 2, radius_formula: "control_radius + offset * 2"}`.

Each value that genuinely has no settled answer is *recorded as an open question in the README*, not guessed into the JSON (R122's "deliberate step" — a question written down is also deliberate, a guess is not).

R123 ("light and dark form where the design gives both"): every colour already carries both; sizes/type/motion are form-agnostic (one value, not two) except where a sentence itself gives two numbers for dark/light (the `scrim`, `faint`, `ramp50` opacities) — those get `{dark, light}` pairs.

## 3. Where the design's working data and scripts live (AC#3, narrow scope)

Two homes, not one, because they're different documents answering different questions:

- `crates/ui/design/foundations.json` (above): the **new**, structured, hand-verified export. Never edited by hand once merged (R121); regenerated only by re-running the export against a newer artifact version.
- `design/canvas/` (repository root, outside `crates/` — it is not Rust and not consumed by any crate build): the **original** working materials for the three foundations pages only, narrowly scoped — not the whole artifact:
  - `foundations.json` (the original, sentence-based file, renamed or clearly distinguished from the crate's structured file of the same base name — e.g. kept as `design/canvas/foundations.json` with a path difference making the distinction obvious, or renamed `source.json`; naming is a plan detail, not a question)
  - `gen_system.py` and `rebuild_main.py` (its one live dependency, confirmed by `gen_system.py`'s own `from rebuild_main import icon as _icon, INNER, F, pill, donut, flat`) — together the actual, current generator for all three foundations pages, confirmed against the page output (`board('Tokens.dc.html', ...)`, `board('Layout.dc.html', ...)`, `board('Details.dc.html', ...)` all three found in `gen_system.py`)
  - `project/Tokens.dc.html`, `project/Layout.dc.html`, `project/Details.dc.html` (the saved, orchestrator-verified pages — the check tool's reference copy)
  - a `README.md` saying: these are the design session's own working files, the artifact is the source of truth, `crates/ui/design/foundations.json` is the structured export checked against the saved pages below, and this directory does not regenerate anything in `crates/ui` automatically — a future export re-reads the artifact, not these files

**Left out, with reasons stated in the README, not silently dropped**:
- `gen_icons.py`, `gen_app.py`, `gen_trial.py`, `apply_main.py`, `apply_refs.py` — these draw or patch *other* artifact pages (icons, the App/interactive mock, the texture trial board), not the three foundations pages; `apply_refs.py` specifically encodes a superseded "status ball" design that predates the current ring-based design (its patched text talks about balls; the current `foundations.json`'s `rules` talk about rings) — committing it under a "durable home" would misrepresent stale work as current. Leave for whichever future task under doc-17 covers Atoms/Icons/App, if still needed then.
- `*.before-*.bak` files — backups, explicitly out per the brief.
- `fairfax-hd-legacy.woff2` — not committed: it is not a named value of the foundations, and its licence has not been checked. State this in the README rather than silently omitting the file.
- `test_app.js`, `app_logic.js` — belong to the App mock, not foundations.

**Question for the engineer** (doesn't block this task, but the narrow-vs-broad choice above is a judgement call): should "durable home" mean just the foundations pages' own files now, with later tasks (Atoms, App, IconSet) bringing in their own working files as they're built — or should one `design/canvas/` land everything from the backup in this task, clearly marked live vs. historical in the README? Proceeding on the narrow reading; flag for approval.

## 4. The tool and its negative control (AC#3's "tested tool")

New entry point under `tools/src/steg_tools/`, named `foundations_check.py` (or similar — final name at build time), on the `boundaries.py`/`affected.py` pattern already in the repo:

- Pure comparison logic (no I/O): given the structured data file's parsed colours and the saved pages' text, return the list of mismatches (colour present in the data file but absent from a page under the same name and value, or vice versa for the 17+ colours this check covers). Unit-tested in `tools/tests/unit/` against literal strings/fixtures, no file reads — mirrors `test_boundaries.py`'s shape.
- A thin `main()` that reads `crates/ui/design/foundations.json` and the three saved pages from `design/canvas/project/`, calls the pure function, and prints/exit-codes — adapter-tested in `tools/tests/adapter/` against the real files in the repo, mirroring `test_affected_real_workspace.py`'s "the real thing, not a fixture" shape.
- `[project.scripts]` entry in `tools/pyproject.toml` (e.g. `steg-check-foundations = "steg_tools.foundations_check:main"`), and a new `check-foundations` recipe in `justfile`, added to the `check:` recipe's dependency list. `check-recipes` needs no change: `check` is already on `recipes.py`'s `ALLOWED_VERBS`, so `check-foundations` passes without extending the list.
- **Negative control, as a named test**: copy the data file to a temp path, flip one colour's hex (or its name) so it no longer matches the saved page, assert the tool exits non-zero and names the mismatch; alongside a positive-control test that the unmutated pair exits zero. Per `team:testing`, load that skill before writing the test file — it governs where this kind of check-output evidence gets recorded on the task, and this plan doesn't substitute for it.

This check is deliberately narrower than TASK-1.94's ("checks that keep steg-ui's levels and named values honest" — presumably the generated-code-matches-data-file check and the no-literal-colours check): this task's tool only proves the *export* didn't drift from the *artifact's saved pages*, nothing about `generated.rs`, which doesn't exist until TASK-1.93.

## 5. What's explicitly not this task

- No value is guessed where the design doesn't fix one — every "Question" row above goes to the engineer, not into the JSON.
- No `generated.rs`, no Rust code at all (TASK-1.93).
- No change to any value (AC#4) — the PR's diff should be readable name-by-name against this table for review.
<!-- SECTION:PLAN:END -->

## Comments

<!-- COMMENTS:BEGIN -->
created: 2026-11-16T01:05:00Z

---

Split on 2026-11-15 by @bruk.o under the engineer's standing decision to split by default: the worker's plan found two changes. This task keeps the data file and the durable home for the design's working files (reviewed by reading). The tested tool that checks the data file against the saved pages moved to a new draft. Criterion 3 reworded to match; criterion 1 no longer says 17 colours, since the plan found the pages draw two more.

---
<!-- COMMENTS:END -->
