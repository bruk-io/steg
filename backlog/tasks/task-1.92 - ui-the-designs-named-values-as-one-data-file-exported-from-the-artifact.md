---
id: TASK-1.92
title: 'ui: the design''s named values as one data file, exported from the artifact'
status: Done
assignee:
  - '@bruk.w6'
created_date: '2026-11-16T00:52:00Z'
updated_date: '2026-11-16T02:06:00Z'
labels:
  - size-3
  - review-passed
milestone: m-4
dependencies: []
references:
  - task-1.92-ui-the-design-s-named-values-as-one-data
  - https://github.com/bruk-io/steg/pull/49
parent_task_id: TASK-1
project: bruk-io-steg
asdlc_accepted_from: DRAFT-2
asdlc_branch: task-1.92-ui-the-design-s-named-values-as-one-data
asdlc_plan_approved_by: bruk
asdlc_plan_approved_hash: 'b3ded099a9d8'
asdlc_plan_approved_size: size-3
asdlc_review_kind: same-machine
asdlc_review_findings: 'important=0,nit=0'
asdlc_plan_matched: no
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
Split as of comment #1: this task now keeps the data file and the durable home for the foundations' working files and saved pages only. The tested tool that checks the data file against the saved pages is TASK-1.96 (depends on this task); its design is a short note near the end of this plan, under its own heading, not elaborated here.

No ordering dependency on TASK-1.91 (unaffected by the split): `Cargo.toml`'s `[workspace] members` is an explicit list (`crates/sdk`, `crates/kernel`, `crates/server`, `crates/steg`, `crates/app`), not a glob, and does not yet list `crates/ui`. This task adds `crates/ui/design/` (plain files, no `Cargo.toml`) without touching that list, so cargo never looks at it; TASK-1.91 adds `crates/ui` to the members list itself when it creates the crate. Either task can merge first; they touch disjoint files under `crates/ui/`.

The engineer's decisions in comment #2 are carried into the table and schema below as settled. Per comment #3, the artifact itself was corrected the same session (version `1794791991-5d11`) to match decisions 2 and 4, and the saved pages at that version live at `/Users/brukhabtu/.claude/projects/-Users-brukhabtu-src-github-com-bruk-io-steg/design-artifact-pages-1794791991-5d11`. **This plan now works from those pages, not the original backup**: `artifact_version` in the data file is `"1794791991-5d11"`, and section 3's "saved pages" are this newer set. I diffed the two directories to confirm: across `Tokens.dc.html`, `Layout.dc.html` and `Details.dc.html`, the only content differences are exactly the two decided ones (`.lv-ib:disabled`'s colour `off`→`muted`, and the scrim sentence `Black at 60%`→`#050508 at 60%`), plus layout-only noise from Bruk's own resizing in the canvas (explicit `width`/`height` on a few `<section>` boxes, one `height: 5600px`→`height: 100%`) and one HTML-entity-escaping change in a `<link>` href — no other value changed. So decisions 2 and 4 are no longer differences between the data file and the artifact; section 4 below records this rather than listing them as open discrepancies.

One consequence for section 3: per comment #3, `gen_system.py` no longer reproduces `Tokens.dc.html` exactly (Bruk resized boxes by hand in the canvas, layout only, no value) — the README must say the generator scripts are history, not a working regenerator of the current pages; the saved pages at the new version are the authoritative reference, not a fresh run of `gen_system.py`.

## 1. The exhaustive table (AC#2)

Source for all of this: `foundations.json`, `gen_system.py`, `rebuild_main.py`, and the rendered pages, read from the pre-correction backup (`design-canvas-backup-2026-11-15`) except where a value is one of the two corrected in section 4, which are read from the current, corrected pages (`design-artifact-pages-1794791991-5d11`) instead — the two sets agree on everything but those two points, confirmed by diff. "Today" = structured field already in `foundations.json`; "sentence" = a prose value inside `foundations.json`; "markup/CSS" = only in a generated page's HTML/CSS or in `gen_system.py`'s own constants, never in `foundations.json` at all.

### Colours

`foundations.json`'s `colours` array names 17 (confirmed against the artifact). The data file adds one more that the pages draw and drops one that the pages draw but shouldn't, per the engineer:

| Name | Today | Dark | Light | Decision |
|---|---|---|---|---|
| the 17 named colours (surface.*, fill.*, outline, text*, ramp.1–6, session.blue) | structured, `colours[]` | as given | as given | unchanged shape, plus `artifact_version` at the top of the file |
| `track` (progress-ring/bar track, the terminal theme's `status.ring` reference) | markup/CSS only (`gen_system.py`'s `COL`/mock `c.track`) | `#3d3b45` | `#cfcbc5` | **in**, as a new named colour (engineer, comment #2: "track and off are named as real colours; they are drawn, so they exist"). Confirmed drawn in more than one place: the mock's `COL['track']` (used by every progress-ring icon) and `status.ring`'s "grey track." Its dark value coincides with `outline`'s dark value, but the light value differs (`#cfcbc5` vs `outline`'s `#bfbbb5`) — not an alias, a colour of its own |
| `off` | markup/CSS only, exactly one draw site, now corrected | `#5f5d69` | `#a5a2ab` | **out**. Checked every file in the pre-correction backup for `{{c.off}}`/`var(--off)`: the only place it was ever actually drawn (not just declared as an unused CSS variable, which every board page does in its boilerplate wrapper) was `ibutton()`'s `'off'` state (`gen_system.py:47`, rendered at the old `Atoms.dc.html:86`, the icon button's "cannot be used" state). `button()` and `cbutton()` — the two other controls with a disabled state — both already correctly used `MUTED` there (`gen_system.py:40,43`), matching the Surfaces rule. Per the engineer (comment #2), the rule won; per comment #3, the artifact's `.lv-ib:disabled` rule is now `muted` on every page, so as of the current version nothing draws `off` at all, and it is not in the data file |
| `faint` (the ring's unfilled/background opacity) | sentence, inside `rules` ("Faint is 30% in dark and 45% in light") | 0.3 | 0.45 | **in**, as an opacity value, not a colour: `{name: "faint", dark: 0.30, light: 0.45}` |
| `blocked`, `done`, `error` (status aliases used by icons/CSS vars) | markup/CSS only, but are aliases | = `ramp.5`/`ramp.6`/`ramp.3` | same | export as references to the existing colour names, not new hex, per the Colour rule ("status and session colours... have no values of their own") |

Net count: 17 + `track` = 18 named colours, plus `faint` (an opacity, not a colour) and the three alias names recorded as references.

### The two ramps

Sentence only (`ramp_text`), and derivable today only by a Rust side-effect of string-matching names starting `ramp.`/`session.` (fragile). Verified in `gen_system.py:366-369`:
- `ramp` (short): `[ramp.1, ramp.2, ramp.4]` (violet, pink, orange — skips red, ramp.3; state this, it's non-obvious), stops 0%, 50%, 100%.
- `ramp.extended`: `[ramp.1, ramp.2, ramp.3, ramp.4, ramp.5, ramp.6]`, stops 0/20/40/60/80/100% (even spacing; confirmed by `Layout.dc.html`'s hardcoded `ramp6` gradient).

Structured form: two explicit named lists (ordered colour-name references + stop percentages), stored in the data file rather than reconstructed from naming convention in generated code — `ramps: [{name: "ramp", stops: [["ramp.1",0],["ramp.2",50],["ramp.4",100]]}, {name: "ramp.extended", stops: [["ramp.1",0],["ramp.2",20],["ramp.3",40],["ramp.4",60],["ramp.5",80],["ramp.6",100]]}]`.

Also sentence-only, two more ramp-derived values used in captions/headers:
- `ramp50`: the short ramp at 50% in dark / 40% in light (alpha suffix `80` dark / `66` light on the same three stops) — matches `ramp_text`'s "50% in dark and 40% in light across the active pane's caption." Structured form: an opacity pair on the `ramp` definition, `{dark: 0.50, light: 0.40}`.
- `tint`: a session colour at 18% (`c.acc + '2e'`), used as the open-session track's fill (confirmed in `Layout.dc.html`'s Track card: "fill: the session colour at 18%"). Structured form: `{name: "tint", opacity: 0.18}`, applied through the same session-colour reference mechanism as the terminal cursor and focus ring, below.

### Type (7 entries) — settled

| Name | Sentence | Structured form |
|---|---|---|
| `font.ui` | "The system font (San Francisco)" | `{family: "system", role: "ui"}` or equivalent — the named system font, settled by the engineer (comment #2 item 5). The pages' own `-apple-system, BlinkMacSystemFont, 'Inter', sans-serif` and the Google-Fonts `Inter` import are preview scaffolding, not exported |
| `font.terminal` | "JetBrains Mono, then SF Mono, then Menlo" | a list: `["JetBrains Mono", "SF Mono", "Menlo"]`, settled. The mock's `ui-monospace, monospace` fallback is scaffolding, not exported |
| `terminal` | "13px on a 20px line, weight 400" | `{size: 13, unit: "px", line_height: 20, weight: 400}` |
| `ui.body` | "13px, weight 400; 600 when selected" | `{size: 13, unit: "px", weight: 400, weight_selected: 600}` |
| `ui.small` | "12px: counts, captions, key caps" | `{size: 12, unit: "px"}` (the "counts, captions, key caps" clause is a use-note, not a value) |
| `ui.label` | "11px, weight 600, upper case, 0.06em" | `{size: 11, unit: "px", weight: 600, text_transform: "uppercase", letter_spacing: 0.06, letter_spacing_unit: "em"}` |
| `palette.input` | "15px" | `{size: 15, unit: "px"}` |

### Sizes (10 entries) — settled, every one splits into 2–3 fields

| Name | Sentence | Structured form |
|---|---|---|
| `space` | "Steps of 2, 4, 6, 8, 10, 12, 16" | a list: `[2, 4, 6, 8, 10, 12, 16]` px |
| `nav.side` | "232px wide; rows 30px, tabs indented 28px" | `{width: 232, row_height: 30, tab_indent: 28}` |
| `nav.top` | "40px high; tabs 30px in a track with a 2px inset" | `{height: 40, tab_height: 30, track_inset: 2}` |
| `tab.width` | "120px usual, 240px at most, 72px at least" | `{usual: 120, max: 240, min: 72}` |
| `pane.gap` | "6px between panes and to the window edge; grabbed across 12px" | `{resting: 6, grabbed: 12}` |
| `pane.caption` | "30px" | `{height: 30}` |
| `palette` | "640px wide, rows 40px, 120px from the top" | `{width: 640, row_height: 40, top_offset: 120}` |
| `radius` | "14px panes and palette; 10px tab track and session chips; 8px inside them" | named by role, settled: `radius.surface: 14`, `radius.track: 10`, `radius.inner: 8` |
| `status.ring` | "16px across, 2px thick; grey track" | `{diameter: 16, thickness: 2, track_colour: "track"}` (a reference to the new `track` colour, not a literal grey) |
| `scrim` | "`#050508` at 60% in dark, ink at 28% in light" (corrected, comment #3 — see section 4) | `{dark: {colour: "#050508", opacity: 0.60}, light: {colour: {ref: "text"}, opacity: 0.28}}` ("ink" = the `text` colour's light value, matching the design's own `--ink` naming) |

### Corners

Covered above under `sizes.radius` (now named `radius.surface`/`radius.track`/`radius.inner`), plus the Track layout primitive's own rule ("the inner radius is the outer radius less the inset, so the corners stay parallel" — a *function*, not a value: `inner = outer - inset`, matches the Foundations test "a function from values to a value." Record the rule, not a duplicate literal).

### Durations and easings

The design fixes exactly one: **200ms**, the status fade (`rules` → Accessibility: "A change of status fades in over 200ms on both the ring and the bar"). Structured form: `{name: "motion.status-fade", duration: 200, unit: "ms"}`.

Everything else timing-shaped is one of two things that are **not** this task's foundations data:
- The mock's own preview CSS (`80ms` hover transition, `120ms` tooltip fade, `500ms` tooltip delay, `1.6s` spin, `2.2s` slide) is artifact-preview scaffolding, not a drawn foundations value — do not export these.
- Values doc-10 already lists as **not yet confirmed** (the 250ms roll-out, ~40 flakes over ten seconds each lying 2–6s, the pane header's three breakpoints, the smallest pane at 200×110px, the palette's 40px-clear/60%-height rule, the pressed-state darkening) — carry those as open questions by pointing at doc-10's list, don't re-litigate or re-decide them here.

No easing curve is fixed anywhere beyond `linear` on the two preview keyframe animations (scaffolding, see above); everything else is CSS's default `ease`. **The design fixes no easing** — say so, don't invent one.

### Both terminal themes — settled

`foundations.json`'s `themes[]` already structures `bg`, `fg`, `cursor`, `selection`, and `ansi` (16 hexes) per theme. Three gaps, all now settled:

1. **ANSI naming.** The 16-array has no names. `gen_system.py:385` gives the order explicitly: `['black','red','green','yellow','blue','magenta','cyan','white']`, normal (0–7) then bright (8–15). Export each as a named entry (`{name: "red", bright: false, hex: "..."}`, or an 8-key object with `normal`/`bright` pairs), not a bare array a consumer must index by convention.
2. **The inactive terminal background.** The `base` sentence packs two values the structured fields don't capture: "bg: the pane, #1c1b21 active, #141318 not" — the structured `bg` field only holds the *active* value. The inactive value (`#141318`) equals `surface.pane`'s dark value, so it's exported as a reference (`bg_inactive: {ref: "surface.pane"}`), not a second literal.
3. **`cursor` is "the session colour,"** settled by the engineer (comment #2 item 3): exported as a reference to the active session's colour, not a fixed hex — see the schema mechanism in section 2.

### The focus ring (R123's "measurements") — settled

`focus_text` sentence: "A 2px ring in the session colour, 2px clear of the control... Its 12px corner follows the control's 8px corner at that distance." Two numbers, a colour reference, and a formula:
- `{width: 2, offset: 2}` (unit px).
- Colour = the active session's colour, settled by the engineer (comment #2 item 3), same reference mechanism as the terminal cursor, not a duplicated hex.
- Corner radius is **derived**, not a third literal: `ring_radius = control_radius + offset * 2` (8 + 2×2 = 12, confirmed by the two other call sites in `gen_system.py` — `fring(m, r=12)` default for 8px controls, and `r=14` passed explicitly for the 10px-corner chip/handle: 10 + 2×2 = 14). Record the formula in the README/schema docs, not a table of per-component radii.

### Layout page (Layout.dc.html) — what it holds beyond `foundations.json`

| Value | Where | Structured form / note |
|---|---|---|
| Row gap | markup | 6px in a tab, 8px in a caption or session row (default), 12px in a palette row |
| Row height | markup | 30px, or 40px in the palette (already implied by existing `sizes` entries; confirms, doesn't add) |
| Stack gap | markup | 2px between rows, 16px between groups |
| Stack indent | markup | 28px (already in `nav.side.tab_indent`; confirms) |
| Track inset / gap / radius / fill | markup | inset 2px, gap 2px, radius 10px outer / 8px items (already in `sizes.radius`; confirms), fill = session colour at 18% (= `tint`, above) |
| Split gap / smallest | markup | gap 6px drawn, 12px grabbed (already in `pane.gap`; confirms); smallest pane 200×110px — doc-10 already lists this as not yet confirmed; carry forward |
| Overlay width/top/height/scrim | markup | matches existing `palette` and `scrim` entries; the 720px/40px-clear breakpoint is also already on doc-10's not-yet-confirmed list — carry forward |
| Resizing vocabulary (`compact`/`regular`/`full`/`dropped first`/`when`) | markup | not a value — a vocabulary for how components describe their own breakpoints; out of scope for `foundations.json`, belongs in each atom/molecule's own Rust doc comments when built |

Net: Layout page adds one genuinely new value (`tint`, folded into the ramps section above) plus a handful of gap/indent numbers not yet named as their own entries — the rest confirms existing `sizes` entries or doc-10's existing open questions.

## 2. The data file's schema (AC#1, AC#2, R121, R122, R123)

`crates/ui/design/foundations.json` — new content, not a copy of the working `foundations.json` (different shape: structured, not sentences). Top-level keys:

- `artifact_version`: the string `"1794791991-5d11"` (R122 — the version after the two corrections in section 4), plus the artifact URL in the README, not re-typed into the structured data.
- `colours`: 18 named colours (the 17 plus `track`; `off` excluded, see above), in the existing shape (`name`, `dark`, `light`, `use`), plus `faint` (an opacity pair, not a colour) and `blocked`/`done`/`error` as `{name, ref}` aliases.
- `session_colour`: the one special token, documented once, that a colour field elsewhere in this file can hold instead of a name or hex: the literal string `"session"`. It means "the active session's own colour, assigned at runtime (R26), not a fixed value in this file." Both the terminal theme's `cursor` and `focus_ring.colour` use it. TASK-1.93's generator reads a colour field this way: a known foundations colour name is a reference, a `#`-prefixed string is a literal (none remain after this task, but the shape allows one), and the literal string `"session"` means "read the session's own colour at the call site, not from this file." This is the schema mechanism the plan was asked to give.
- `ramps`: the two named ramps, each an ordered list of `{colour, stop}`, plus `ramp50`'s opacity pair and `tint`'s opacity (applied to `"session"`).
- `type`: the 7 entries above, each replaced by its structured object.
- `sizes`: the existing 10 names, each replaced by its structured object (`radius` split into `radius.surface`/`radius.track`/`radius.inner`), plus the Layout-only row/stack gap and indent values folded in under existing or new names.
- `motion`: the one duration the design fixes (`motion.status-fade`, 200ms) — a new top-level section; not called "durations" plural since there's exactly one.
- `themes`: the existing two, `ansi` reshaped to named entries, `bg_inactive` as a colour reference, `cursor` as `"session"`.
- `focus_ring`: `{width: 2, offset: 2, colour: "session", radius_formula: "control_radius + offset * 2"}`.

R123 ("light and dark form where the design gives both"): every colour already carries both; sizes/type/motion are form-agnostic (one value, not two) except where a sentence itself gives two numbers for dark/light (`scrim`, `faint`, `ramp50`) — those get `{dark, light}` pairs.

## 3. Where the design's working data and scripts live (AC#3) — settled narrow scope

Two homes, not one, because they're different documents answering different questions:

- `crates/ui/design/foundations.json` (above): the new, structured, hand-verified export, `artifact_version: "1794791991-5d11"`. Never edited by hand once merged (R121); regenerated only by re-running the export against a newer artifact version.
- `design/canvas/` (repository root, outside `crates/` — it is not Rust and not consumed by any crate build): the original working materials for the three foundations pages only — settled by the engineer (comment #2 item 7: "only the current foundations files get a home"):
  - the original, sentence-based `foundations.json` (kept at a path clearly distinct from the crate's structured file, e.g. `design/canvas/foundations.json`, so the two are never confused by name alone) — this is the pre-correction version from the backup; its two sentences (the disabled-icon rule's colour and the scrim's base colour) are now stale relative to the corrected artifact, and the README says so rather than silently carrying a working copy that disagrees with its own saved pages
  - `gen_system.py` and `rebuild_main.py` (its one live dependency, confirmed by `gen_system.py`'s own `from rebuild_main import icon as _icon, INNER, F, pill, donut, flat`) — the generator that drew the *original* three foundations pages, kept as history. Per comment #3, it no longer reproduces `Tokens.dc.html` exactly (Bruk resized a few boxes by hand in the canvas afterwards, layout only, no value change) — the README says these scripts are not a working regenerator of the current pages, so nobody runs them expecting today's output
  - `project/Tokens.dc.html`, `project/Layout.dc.html`, `project/Details.dc.html` — the saved pages at the corrected version, `1794791991-5d11` (from `/Users/brukhabtu/.claude/projects/-Users-brukhabtu-src-github-com-bruk-io-steg/design-artifact-pages-1794791991-5d11`, not the earlier backup) — the current, authoritative reference, and TASK-1.96's reference copy
  - a `README.md` saying: these are the design session's own working files, the artifact is the source of truth, `crates/ui/design/foundations.json` is the structured export, this directory does not regenerate anything in `crates/ui` automatically, the generator scripts are history and don't reproduce the current pages exactly, the included `foundations.json` predates the artifact's own correction and disagrees with the saved pages on the two points in section 4, and — per section 4 below — the two places where the artifact itself was corrected during this task's planning, so a reader never mistakes old history for a live discrepancy.

**Left out, with reasons stated in the README, not silently dropped** (settled, item 7):
- `gen_icons.py`, `gen_app.py`, `gen_trial.py`, `apply_main.py`, `apply_refs.py` — draw or patch *other* artifact pages (icons, the App/interactive mock, the texture trial board), not the three foundations pages; `apply_refs.py` specifically encodes a superseded "status ball" design that predates the current ring-based design (its patched text talks about balls; the current `foundations.json`'s `rules` talk about rings) — left for whichever future task under doc-17 covers Atoms/Icons/App, if still needed then.
- `*.before-*.bak` files — backups, explicitly out.
- `fairfax-hd-legacy.woff2` — not committed: not a named value of the foundations, and its licence has not been checked. Stated in the README, not silently omitted.
- `test_app.js`, `app_logic.js` — belong to the App mock, not foundations.

## 4. Where the export differs from the artifact (AC#4)

As of version `1794791991-5d11`, nowhere: the two places the export would otherwise have disagreed with the artifact are already reconciled, because the artifact was corrected in the same session (comment #3), not because the data file is bending the truth:

1. **A disabled icon button's colour.** The working `foundations.json`/`gen_system.py` (the pre-correction backup) drew the icon button's "cannot be used" state in `off` (`#5f5d69` dark / `#a5a2ab` light), conflicting with the Surfaces rule ("a part that cannot be used takes the muted text colour") and with the two other button variants (`button()`, `cbutton()`), which already correctly used `muted`. The engineer decided the rule wins (comment #2); the artifact's `.lv-ib:disabled` rule was then edited in place, across all seven pages that share it, to use `muted` (comment #3, confirmed by diffing the saved pages). `off` is excluded from the data file for the reason in section 1 — not because the export is a deliberate correction, but because nothing in the current, corrected artifact draws it any more.
2. **The overlay's scrim base colour.** The working `foundations.json`'s `scrim` sentence said "Black at 60% in dark," but the drawn overlay used `#050508` at 60%, not pure black. The engineer decided the drawing is right (comment #2); the sentence on both `Tokens.dc.html` and `Layout.dc.html` was then edited to say `#050508 at 60% in dark` (comment #3, confirmed by diffing the saved pages). The data file's `scrim.dark` matches the corrected sentence and the drawing alike.

Diffing the pre-correction backup against the `1794791991-5d11` pages (done for this plan, in the opening note above) found exactly these two content changes, nothing else — confirming no other value moved between the two versions. The not-yet-confirmed items carried forward from doc-10 (smallest pane, pane-header breakpoints, the palette's 40px/60% rule, the roll-out time, the flake counts/timings) are pointed at, not decided, so they are not "changes" either way. The README still names both of these, briefly, as the two places the artifact itself was corrected during this task's planning — useful history, not a live discrepancy.

## 5. What's explicitly not this task

- No `generated.rs`, no Rust code at all (TASK-1.93).
- The tool that checks the data file against the saved pages (TASK-1.96, below) — not built here.

## For TASK-1.96 (moved out of this plan)

Short note only, so the thinking isn't lost: a new entry point under `tools/src/steg_tools/` (e.g. `foundations_check.py`), on the `boundaries.py`/`affected.py` pattern already in the repo — pure comparison logic (data file's parsed colours vs. the saved pages' text) unit-tested with no I/O in `tools/tests/unit/`, a thin `main()` reading the real files in `design/canvas/project/` and `crates/ui/design/foundations.json`, adapter-tested in `tools/tests/adapter/` against the real repository files (mirrors `test_affected_real_workspace.py`'s shape), a `[project.scripts]` entry plus a new `check-foundations` justfile recipe added to `check:`'s dependency list (no change needed to `check-recipes`/`ALLOWED_VERBS`, since `check` is already an allowed verb), and — as the acceptance criteria for that task should require — a named negative control (mutate one colour in a copied data file, assert the tool fails) alongside the positive control that the unmutated pair passes. `team:testing` governs where that task records its check output; load it there, not here.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Rewritten plan checked against doc-17 (R121, R122) and decisions 10 and 11 by @bruk.o: ok. The data file is exported from the saved pages of the artifact's version 1794791991-5d11 and records that version; every sentence-shaped value gets a structured form; the engineer's seven decisions are carried in; off is not exported because nothing draws it after the correction; a colour that means the active session's colour is the one reference the schema allows in place of a value. The working data and scripts for the three foundations pages go under design/canvas with a README that says they are how the pages were first drawn and no longer reproduce the Tokens page exactly. The checking tool is TASK-1.96's. One point for the build, not the plan: the focus ring's radius is written as a formula in a string, which is the kind of thing a program must parse out of prose that criterion 2 rules out; give it a structured form (for example the amount added to the control's radius) and say so in the notes.

Check passed: `just check` exited 0 at 2026-11-16T01:29:43+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.92 on task-1.92-ui-the-design-s-named-values-as-one-data at 1c9f374426a1:

```text
ok      format check (0.2s)
ok      lint (88.0s)
ok      boundaries (0.1s)
ok      licences (0.7s)
skip    tests: this change touches no crate
ok      build (steg-sdk) (3.1s)
ok      test (steg-sdk) (1.7s)
ok      test (steg-sdk, local-socket) (2.6s)
ok      build (steg-kernel) (44.7s)
ok      test (steg-kernel) (1.4s)
ok      build (steg-server) (45.1s)
ok      test (steg-server) (6.1s)
ok      test (steg-server, race-fixture) (6.7s)
ok      build (steg) (0.3s)
ok      test (steg) (0.4s)
ok      build (steg-app) (23.7s)
ok      test (steg-app) (1.0s)
ok      tool tests (2.2s)
ok      architecture (1.2s)
ok      recipe names (0.1s)
```

Check passed: `sh -c '
SRC_BACKUP="/Users/brukhabtu/.claude/projects/-Users-brukhabtu-src-github-com-bruk-io-steg/design-canvas-backup-2026-11-15"
SRC_PAGES="/Users/brukhabtu/.claude/projects/-Users-brukhabtu-src-github-com-bruk-io-steg/design-artifact-pages-1794791991-5d11"
fail=0
check() { a=$(shasum -a 256 "$1" | cut -d" " -f1); b=$(shasum -a 256 "$2" | cut -d" " -f1); if [ "$a" = "$b" ]; then echo "ok $2"; else echo "MISMATCH $2 ($a vs $b)"; fail=1; fi; }
check "$SRC_BACKUP/foundations.json" design/canvas/foundations.json
check "$SRC_BACKUP/gen_system.py" design/canvas/gen_system.py
check "$SRC_BACKUP/rebuild_main.py" design/canvas/rebuild_main.py
check "$SRC_PAGES/Tokens.dc.html" design/canvas/project/Tokens.dc.html
check "$SRC_PAGES/Layout.dc.html" design/canvas/project/Layout.dc.html
check "$SRC_PAGES/Details.dc.html" design/canvas/project/Details.dc.html
exit $fail
'` exited 0 at 2026-11-16T01:31:28+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.92 on task-1.92-ui-the-design-s-named-values-as-one-data at 1c9f374426a1 with uncommitted changes:

```text
ok design/canvas/foundations.json
ok design/canvas/gen_system.py
ok design/canvas/rebuild_main.py
ok design/canvas/project/Tokens.dc.html
ok design/canvas/project/Layout.dc.html
ok design/canvas/project/Details.dc.html
```

Check passed: `python3 -I /private/tmp/claude-501/-Users-brukhabtu-src-github-com-bruk-io-steg/a318fbf0-a659-4ad6-9bfc-86a79ba9cc76/scratchpad/task-1.92-shape-check.py crates/ui/design/foundations.json` exited 0 at 2026-11-16T01:31:35+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.92 on task-1.92-ui-the-design-s-named-values-as-one-data at 1c9f374426a1 with uncommitted changes:

```text
OK: shape check passed (18 colours, 3 aliases, 2 ramps, 7 type entries, 17 size entries, 1 motion entries, 2 themes)
```

Check passed: `python3 -I /private/tmp/claude-501/-Users-brukhabtu-src-github-com-bruk-io-steg/a318fbf0-a659-4ad6-9bfc-86a79ba9cc76/scratchpad/task-1.92-colour-check.py crates/ui/design/foundations.json design/canvas/project/Tokens.dc.html` exited 0 at 2026-11-16T01:31:38+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.92 on task-1.92-ui-the-design-s-named-values-as-one-data at 1c9f374426a1 with uncommitted changes:

```text
OK: all 18 colours' names and both hex values found in design/canvas/project/Tokens.dc.html
```

Check passed: `just check` exited 0 at 2026-11-16T01:32:38+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.92 on task-1.92-ui-the-design-s-named-values-as-one-data at dc9f98533d16:

```text
ok      format check (0.2s)
ok      lint (0.6s)
ok      boundaries (0.1s)
ok      licences (0.8s)
skip    tests: this change touches no crate
ok      build (steg-sdk) (0.1s)
ok      test (steg-sdk) (0.3s)
ok      test (steg-sdk, local-socket) (0.4s)
ok      build (steg-kernel) (0.1s)
ok      test (steg-kernel) (0.2s)
ok      build (steg-server) (0.1s)
ok      test (steg-server) (3.9s)
ok      test (steg-server, race-fixture) (4.1s)
ok      build (steg) (0.1s)
ok      test (steg) (0.1s)
ok      build (steg-app) (0.3s)
ok      test (steg-app) (0.3s)
ok      tool tests (1.2s)
ok      architecture (1.0s)
ok      recipe names (0.1s)
```

Built per the approved plan (digest b3ded099a9d8), with one build-phase departure the team lead directed: focus_ring.radius_formula (a string formula) replaced with focus_ring.corner_radius_addition: 4 (a plain number) — the plan's string-formula sketch was exactly what AC#2 rules out, something a program must parse out of prose.

While restoring the plan to match the approved digest after an inadvertent post-approval wording edit, I found and fixed a real value error in my own draft before writing crates/ui/design/foundations.json: I had truncated the six ramp colours' names from the design's own 'ramp.1 violet' etc. to bare 'ramp.1', which AC#4 ('no value is changed') and AC#1 ('under the design's own names') both rule out. Fixed before committing; the committed file uses the design's exact names throughout.

What was built, matching the plan:
- crates/ui/design/foundations.json: 18 colours (the design's 17 plus track; off excluded per the engineer's decision, since nothing in the corrected artifact draws it), 3 colour aliases, 1 opacity (faint), 2 ramps plus caption_opacity and tint, 7 type entries, 17 size entries (10 plus radius split three ways plus 5 Layout-page-only entries), 1 motion entry, 2 themes with 16 named ANSI colours each and a session-colour reference for cursor, and the focus ring. artifact_version: 1794791991-5d11.
- design/canvas/: the pre-correction working foundations.json, gen_system.py and its one dependency rebuild_main.py (copied unchanged from the 2026-11-15 backup), and the three foundations pages at the corrected artifact version (copied unchanged from design-artifact-pages-1794791991-5d11). README.md says what's current, what's history, and that the artifact is the source, per the plan.

Checks recorded on this task (all exit 0):
- just check, before any change (baseline)
- checksum verification: all 6 copied files byte-identical to their sources in ~/.claude/projects/.../design-canvas-backup-2026-11-15 and .../design-artifact-pages-1794791991-5d11
- ad hoc shape check (not TASK-1.96's tool): every colour has name/dark/light as 6-digit hex, every numeric value is a real number never a string with an embedded unit, both themes have exactly 16 named ANSI colours
- ad hoc colour check (not TASK-1.96's tool): all 18 colours' names and both hex values found in the saved Tokens.dc.html
- just check at the final head (dc9f98533d16), after rebasing onto origin/main (TASK-1.91's crates/ui work had not yet merged to main at rebase time, so there was nothing to reconcile; the rebase was clean)

The tested tool that checks the data file against the saved pages by name and value, with a negative control, is TASK-1.96 (not this task) — noted in the plan and in the PR description, not built here.

CI green on the final head (dc9f98533d16): gh pr checks 49 --watch, 'check' passed in 3m11s.

Check passed: `python3 -I /private/tmp/claude-501/-Users-brukhabtu-src-github-com-bruk-io-steg/a318fbf0-a659-4ad6-9bfc-86a79ba9cc76/scratchpad/task-1.92-shape-check.py crates/ui/design/foundations.json` exited 0 at 2026-11-16T01:54:00+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.92 on task-1.92-ui-the-design-s-named-values-as-one-data at dc9f98533d16 with uncommitted changes:

```text
OK: shape check passed (18 colours, 3 aliases, 2 ramps, 7 type entries, 18 size entries, 1 motion entries, 2 themes)
```

Commits since the review: 0.

Check passed: `python3 -I /private/tmp/claude-501/-Users-brukhabtu-src-github-com-bruk-io-steg/a318fbf0-a659-4ad6-9bfc-86a79ba9cc76/scratchpad/task-1.92-colour-check.py crates/ui/design/foundations.json design/canvas/project/Tokens.dc.html` exited 0 at 2026-11-16T01:54:03+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.92 on task-1.92-ui-the-design-s-named-values-as-one-data at dc9f98533d16 with uncommitted changes:

```text
OK: all 18 colours' names and both hex values found in design/canvas/project/Tokens.dc.html
```

Commits since the review: 0.

Check passed: `just check` exited 0 at 2026-11-16T01:54:56+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.92 on task-1.92-ui-the-design-s-named-values-as-one-data at 91d4e877b240:

```text
ok      format check (0.2s)
ok      lint (0.6s)
ok      boundaries (0.1s)
ok      licences (0.7s)
skip    tests: this change touches no crate
ok      build (steg-sdk) (0.1s)
ok      test (steg-sdk) (0.4s)
ok      test (steg-sdk, local-socket) (0.5s)
ok      build (steg-kernel) (0.1s)
ok      test (steg-kernel) (0.3s)
ok      build (steg-server) (0.1s)
ok      test (steg-server) (4.3s)
ok      test (steg-server, race-fixture) (4.5s)
ok      build (steg) (0.1s)
ok      test (steg) (0.2s)
ok      build (steg-app) (0.4s)
ok      test (steg-app) (0.3s)
ok      tool tests (1.0s)
ok      architecture (1.0s)
ok      recipe names (0.1s)
```

Commits since the review: 30, 29 without the Asdlc-Worker trailer.

Review findings fixed (@bruk.r3, 1 Important, 2 nits). Plan section untouched.

1. [Important, fixed] Removed sizes.overlay.layout.max_height_percent_of_window: 60. doc-10's not-yet-confirmed list bundles the overlay's max-height-as-percent-of-window with its 40px-clear-below-720px rule as one bullet, not two settled values; I'd correctly left out the 40px half but exported the 60% half as settled anyway, breaking AC#2 and departing from the plan's own instruction to point at doc-10's list rather than re-decide it. Re-read doc-10's whole not-yet-confirmed list against the data file line by line as asked: the smallest pane (200x110), the pane header's three breakpoints (360/260/200), the status bar's roll-out (250ms), the failure flakes' count/timing, and the pressed state's darkening are all absent from the data file, as they should be - nothing else from that list had slipped in.

2. [Nit, fixed] Every reference now uses the uniform wrapped form {"ref": "<name>"}: colour_aliases' ref field, both ramps' stops[].colour, status.ring.track_colour, scrim.light.colour, track.layout.fill.colour, track.layout.radius_outer/radius_inner. bg_inactive already used this form. Added crates/ui/design/README.md stating the rule once: a reference is always {"ref": "<name>"} naming any colour, size key, or tint in the file; the two exceptions are a bare hex literal and the literal "session". Extended the ad hoc shape check to assert every {"ref": ...} anywhere resolves to a name that exists, and that every colour-position field (colour/track_colour/cursor) is one of exactly those three forms - sanity-tested by corrupting one ref and confirming the check fails before trusting it to pass.

3. [Nit, fixed] Added sizes.radius.round: {"shape": "round"} for the radii card's fourth swatch (999px "fully round: chips of colour, bars", Tokens.dc.html/gen_system.py:372) that was drawn but had no data-file entry; noted in the file that the name radius.round is the orchestrator's choice, which the engineer may change. Checked every other card on the three foundations pages the same way (Type's specimens - all already in type{}; the ramp/extended-ramp gradient bars; both terminal themes' 16-colour samples and base text; the focus ring's interactive states) for a captioned value with no entry: found none beyond this one.

Checks re-recorded after the fixes: the two ad hoc checks (shape, colours-in-page) against the corrected file, and just check at the final head (91d4e877b240) after merging origin/main (branch was already pushed, so merged rather than rebased; TASK-1.91's crates/ui work still hadn't merged to main, so no overlap - diff stayed disjoint). CI green on that same head (gh pr checks 49 --watch, 2m13s). PR 49's description updated with the corrected section-by-section counts (colours 18, sizes 18 now with radius.round, etc).
<!-- SECTION:NOTES:END -->

## Comments

<!-- COMMENTS:BEGIN -->
created: 2026-11-16T01:05:00Z

---

Split on 2026-11-15 by @bruk.o under the engineer's standing decision to split by default: the worker's plan found two changes. This task keeps the data file and the durable home for the design's working files (reviewed by reading). The tested tool that checks the data file against the saved pages moved to a new draft. Criterion 3 reworded to match; criterion 1 no longer says 17 colours, since the plan found the pages draw two more.

---

created: 2026-11-16T01:17:00Z

---

Decided by the engineer on 2026-11-15, answering the plan's questions (he agreed with each recommendation put to him by @bruk.o): (1) track and off are named as real colours; they are drawn, so they exist. (2) A disabled control takes muted, as the Surfaces rule says: the icon button's use of off is the slip; off stays in the data file only if something else draws it, and the plan says which. This is a change to the design, to be made in the artifact too. (3) The terminal themes' cursor and the focus ring's colour are exported as a reference to the session colour, not as a fixed value. (4) The scrim is what is drawn, #050508 at 60% in dark, and the sentence saying black is corrected. (5) The fonts are the named ones: the system font for the interface; JetBrains Mono, then SF Mono, then Menlo for the terminal. Inter and the generic monospace in the pages are scaffolding and are not exported. (6) The three corner sizes are named by role: radius.surface 14, radius.track 10, radius.inner 8. (7) Only the current foundations files get a home in the repository: the superseded status-ball script, the backups and the font are left out. Where a decision differs from what the artifact's pages draw or say (2 and 4), the data file takes the decision, the README lists each difference, and the artifact is corrected to match as a follow-up the orchestrator raises with the engineer.

---

created: 2026-11-16T01:20:00Z

---

2026-11-15, @bruk.o: at the engineer's word the design artifact was corrected in this session, as version 1794791991-5d11: in the seven pages that carry the shared rule, a disabled icon button now takes muted (the rule .lv-ib:disabled, and the still drawing on the Atoms page), and the scrim's sentence on the Tokens and Layout pages now says #050508 at 60% in dark. Nothing else was changed. The engineer had himself resized a few boxes on the Tokens page in the canvas an hour earlier (layout only, no value), which is why the pages were edited in place and not regenerated: the generator scripts no longer reproduce the Tokens page exactly, and the README must say so. After the correction nothing draws the colour off: it is still defined in the pages' theme data but unused, so it is not exported. The pages as they are in that version are saved at /Users/brukhabtu/.claude/projects/-Users-brukhabtu-src-github-com-bruk-io-steg/design-artifact-pages-1794791991-5d11 : these are the saved pages the data file is exported from and records as its version, and with them decisions 2 and 4 are no longer differences between the data file and the artifact.

---

author: @bruk
created: 2026-11-16T01:23:00Z

---

Plan approved by @bruk (b3ded099a9d8, size-3): its worker may build it.

---

created: 2026-11-16T01:25:00Z

---

Departure from the approved plan's wording, not its substance (restored the plan's exact approved text after accidentally re-saving it with these three clarifications past the approval; the plan itself is unchanged from digest b3ded099a9d8): (1) the off colour section notes it is still defined as an unused colour token in each page's own inline theme data, not just absent from drawing; (2) the generator-scripts README guidance says plainly they are how the pages were first drawn, not a way to regenerate them without losing the hand edit Bruk made to Tokens.dc.html since; (3) same point restated in the design/canvas/README.md bullet list. None of these change what is exported, what is excluded, the schema, or the file layout - they only make the README's wording match what the plan already decided.

---

author: @bruk.r3
created: 2026-11-16T01:37:00Z

---

@bruk.r3 is reviewing TASK-1.92: a same-machine review: it waited 0m with no other engineer's reviewer, longer than review_fallback_after (0m), so a fresh reviewer on its author's machine takes it.

---

author: @bruk.r3
created: 2026-11-16T01:48:00Z

---

Review by @bruk.r3 (same-machine): changes requested. Back to In Progress for the author's worker.

Findings: 1 Important, 2 nit. Cites: review. The diff does not match the plan.

[Compliance, Important] crates/ui/design/foundations.json:217 exports overlay.layout.max_height_percent_of_window: 60. doc-10's "Values... not yet confirmed" lists this in the same bullet as the palette's 40px-clear-below-720px rule ("growing to at most 60% of the window's height"); R79 says neither is settled until Bruk says so. The approved plan itself (section 1, Durations/easings) says to carry this specific item forward by pointing at doc-10's list, "don't re-litigate or re-decide them here" - and the build correctly left out the 40px-clear half but exported the 60%-height half as a settled measurement anyway. That both breaks AC#2 ("anything the design does not actually fix is listed as a question and not guessed") and is an unremarked departure from the plan's own instruction.

[Compliance, Nit] Reference-format inconsistency the plan itself introduces inconsistently and the build follows literally: bg_inactive is wrapped {"ref": "surface.pane"} (plan section 2) while scrim.light.colour ("text"), status.ring.track_colour ("track") and track.layout.fill.colour ("tint") are bare strings - and the plan's own table (section 1) specified {colour: {ref: "text"}, opacity: 0.28} for scrim.light, not the bare string the committed file has. track.layout.fill.colour: "tint" and track.layout.radius_outer: "radius.track" also reference names outside colours[] (a top-level key and a sizes key respectively), which the three-way colour-field rule in section 2 (foundations colour name / #-literal / "session") doesn't literally cover. TASK-1.93's generator will need to special-case more than one representation for what is conceptually the same "reference" concept.

[Compliance, Nit] Tokens.dc.html's radii card (gen_system.py:372) draws a fourth corner value, 999px "fully round: chips of colour, bars", alongside the three (14/10/8) that are in foundations.json's radius.surface/track/inner. It isn't in the working foundations.json's "radius" sentence either (which only lists the three), so it reads as an unnamed shape idiom rather than a design-named value, and I'd treat its omission as acceptable - but it is a value drawn on the page with no entry in the data file, so noting it per the review brief's ask.

Everything else checked independently and found correct, at head dc9f98533d16 (matches both the task's recorded `just check` and PR 49's headRefOid; CI "check" is SUCCESS on this head):
- All 18 colours (the 17 plus track), both hex forms and the design's own names (including the ramp colours' full names, "ramp.1 violet" etc, not truncated), found in design/canvas/project/Tokens.dc.html and in the working design/canvas/foundations.json/gen_system.py, by independently grepping every hex value and cross-checking names - not by relying on the worker's recorded shape/colour-check scripts.
- Both ramps' stops (including the short ramp skipping ramp.3 red) and both ramp-derived opacities (caption_opacity 0.50/0.40, tint 0.18) confirmed against gen_system.py's `ramp`/`ramp6`/`ramp50` gradient literals and the `TINT` alpha-suffix source.
- All 16 ANSI colours of both themes, checked by name as well as by hex presence: gen_system.py:385's `nm` list (black..white, normal 0-7 then bright 8-15) against design/canvas/foundations.json's `ansi` arrays and the committed named entries, position by position - all 16 match in both themes.
- The three corner sizes (14/10/8), the scrim (#050508 at 60% dark; text colour at 28% light), and the one duration (200ms) all confirmed against the corrected pages (both Tokens.dc.html and Layout.dc.html say "#050508 at 60% in dark, ink at 28% in light", not the stale "Black" sentence still present in the working design/canvas/foundations.json, exactly as the plan's section 4 describes).
- JSON is syntactically valid, records artifact_version "1794791991-5d11", and I scripted a scan for any string value embedding a number+unit or a formula: none found outside name/use/url fields (focus_ring.radius_formula was correctly replaced with corner_radius_addition: 4, a plain number).
- off is absent and correctly still only mentioned in the README/history, not the colours[] array; "session" is used only for cursor (both themes), focus_ring.colour and tint.colour, exactly the three places named.
- The six copied files under design/canvas/ are byte-identical (shasum -a 256, done myself, not just re-reading the recorded check) to their sources in design-canvas-backup-2026-11-15 and design-artifact-pages-1794791991-5d11. README.md correctly states current vs. history, that the artifact is the source, that the generator scripts no longer reproduce Tokens.dc.html exactly, that the working foundations.json predates the two corrections, and where off went. No local filesystem path, credential, or font is committed; fairfax-hd-legacy.woff2 is named but correctly left out with its licence unconfirmed.
- Repository checks: diff touches only the 8 files listed in `git diff --stat`, none under justfile/tools/.github, so no check is weakened or newly skipped. Cargo.toml's workspace members list is unchanged and still excludes crates/ui (no Cargo.toml is added under crates/ui/design/), consistent with the recorded `skip tests: this change touches no crate`. TASK-1.91 (PR 48) adds crates/ui's Cargo.toml/src/boundaries.toml entry; this task only touches crates/ui/design/ and design/canvas/ - disjoint paths - so either merge order is clean and neither leaves cargo looking at a non-crate directory.
- No image needed: this task adds data and documents only, nothing the app draws.

plan-matched is "no": the focus_ring.radius_formula -> corner_radius_addition departure is a real, correctly-explained departure, but it is still a departure from the approved digest's literal schema.

---

author: @bruk.r3
created: 2026-11-16T01:58:00Z

---

@bruk.r3 is reviewing TASK-1.92: a same-machine review: it waited 0m with no other engineer's reviewer, longer than review_fallback_after (0m), so a fresh reviewer on its author's machine takes it.

---

author: @bruk.r3
created: 2026-11-16T02:01:00Z

---

Review by @bruk.r3 (same-machine): clean. It stays In Review for a person to merge.

Findings: 0 Important, 0 nit. The diff does not match the plan.

Second review, same-machine, at head 91d4e877b240 (matches the task's recorded `just check` and PR 49's headRefOid; CI "check" is SUCCESS on this head). Reviewed the whole diff against main, with particular attention to the three fixes and the new README.

No findings.

1. doc-10's not-yet-confirmed list, checked item by item against foundations.json: the pressed-state darkening, the button's 72px icon-only threshold, the pane header's three breakpoints (360/260/200), the smallest pane (200x110), the status bar's 250ms roll-out, the failure flakes' count/timing, and the overlay's 40px-clear/60%-height rule are all absent from the data file - none snuck back in, and overlay.layout.max_height_percent_of_window is gone, replaced by a `use` note pointing at doc-10's bullet. (The 72px in tab.width.min and the 200ms in motion.status-fade are different, independently-settled values, not the unconfirmed items re-appearing under another name - confirmed by rereading the plan's own settled tables.)

2. References: wrote and ran my own script against the whole file (not the worker's), independent of the recorded checks. Found exactly 19 `{"ref": ...}` objects, all resolving to an existing name (a colours[] name, a sizes key, or `tint`). Found zero bare-string leftovers anywhere outside the three permitted colour forms (hex literal, the literal "session", or a `{"ref": ...}` object) - scrim.dark.colour is the one hex literal, "session" appears exactly at themes[].cursor (both themes), focus_ring.colour and tint.colour and nowhere else, matching the README's own count. crates/ui/design/README.md's "How a reference is written, everywhere" section states this rule once and it matches the file exactly, including naming the field-name exceptions (`track_colour`, `radius_outer`, the `ref` field on `colour_aliases`, etc) that make the rule non-obvious from the key name alone.

3. radius.round is present as `{"shape": "round", "use": "..."}` - a structured shape, not the literal 999 standing in for a value. Spot-checked two cards in design/canvas/project/Tokens.dc.html myself: the radii card (14/10/8/999px "fully round: chips of colour, bars") matches radius.surface/track/inner/round exactly, and the colour-table captions (name/dark/light/used-for headers, all 17+track colours by name) match colours[]. Found no other captioned value missing beyond radius.round.

4. Diff to foundations.json since the first-round head (dc9f985) is exactly the 38 lines described: the unconfirmed value's removal, the ref-wrapping, and the new radius.round entry - nothing else. All colours (18, both hex forms, full ramp names), both ramps, all 16 ANSI colours by name/position in both themes, sizes, scrim, and the one duration are untouched by this diff, so the first round's independent verification of those still holds.

5. crates/ui/design/README.md and design/canvas/README.md don't contradict each other: the first documents the structured export's schema and reference rule, the second documents the working/history files and the artifact-vs-working-copy corrections; each points at the other and at the artifact as the source, and neither claims to be the source itself.

Evidence: `just check` recorded on the task at 91d4e877b240 matches this head; CI "check" on PR 49 is SUCCESS at headRefOid 91d4e877b240, confirmed directly via `gh pr checks 49`.

---
<!-- COMMENTS:END -->
