---
id: TASK-1.96
title: 'tools: check the design''s data file against the saved foundations pages'
status: In Progress
assignee:
  - '@bruk.w8'
created_date: '2026-11-16T01:05:00Z'
updated_date: '2026-11-16T02:20:00Z'
labels:
  - size-2
milestone: m-4
dependencies:
  - TASK-1.92
parent_task_id: TASK-1
project: bruk-io-steg
asdlc_accepted_from: DRAFT-1
asdlc_branch: task-1.96-tools-check-the-design-s-data-file
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Split from TASK-1.92 on 2026-11-15. doc-17 (R121, R122). The tool follows the existing ones in tools/src/steg_tools. TASK-1.92's plan has its design.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A tool in tools/ checks that every value in crates/ui/design/foundations.json appears in the saved copies of the design's foundations pages under the same name and with the same value, and that no named value drawn in those pages is missing from the data file; a refusal names the value and both forms
- [ ] #2 It has unit tests for its logic and a test against the repository's real files, with a negative control: one value changed in a scratch copy is refused; it is part of just check and of CI
- [ ] #3 The plan says what the tool cannot see (a value only described in prose, a page that was not saved) so that passing is not read as more than it is
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
One reviewable change. New module `tools/src/steg_tools/foundations.py`, CLI `steg-foundations-check`, recipe `check-foundations`, in `just check`.

## What "appears under the same name and value" can mean, settled from the actual markup

Read all three saved pages at `design/canvas/project/*.dc.html` (not the generator scripts: TASK-1.92's review already showed the scripts don't reproduce today's Tokens.dc.html, and this tool's job is the saved pages, per the task description). Every page has a rendered body followed by one `<script type="text/x-dc">` block holding the page's own internal colour-variable object (`c = {ground, ink, ink2, muted, quiet, hover, off, track, faint, ramp6, ...}`) used to drive the preview, under the *page's* vocabulary (`ink`, `off`, `quiet`), not the design's names (`text`, `surface.overlay`, `fill.quiet`). **The tool reads only the rendered body** (everything before `<script type="text/x-dc"`); the script block is never parsed as page content. Checking it would require a second, separate name-translation table and would make `off` look like a page-named value missing from the file - exactly the value the engineer already decided to drop (TASK-1.92 comment #2).

Within the rendered body, a named value sits in one of two row shapes, always preceded by its own header row:
- 4-column: header cells read `Name` / `Dark` / `Light` / `Used for`; a data row is `<name span (no colour style)> <swatch span containing a nested `background:#hex` swatch + a `#hex` text span> <same for light> <use text>`.
- 2-column: header cells read `Name` / `Value`; a data row is `<name span> <value span, free text>`.

Both shapes are `display:grid` divs, but **the parser keys on the header row's cell text, never on the `grid-template-columns` pixel string** - `design/canvas/README.md` records that Bruk hand-resized boxes on the Tokens page in the canvas after the generator last ran, so a width-keyed parser would silently match zero rows after a future resize and the check would pass by finding nothing. A page section that is expected to hold one of these tables but yields zero rows is itself a refusal ("found a Name/Value table with 0 rows on Tokens.dc.html" ), never a silent pass.

Verified against the real files (not assumed): on Tokens.dc.html there are exactly two 4-column tables (10 rows under "Surfaces and text", 7 under "The six colours" - 17 names total, matching the file's `colours[]` minus `track`) and exactly two 2-column tables ("Type", 7 rows; "Sizes, spacing and corners", 10 rows). Details.dc.html has one ANSI swatch grid per theme (8+8 three-span cards: swatch, name text, hex text), under section headers "Terminal theme: steg dark" / "steg light" matching `themes[].name` exactly. Layout.dc.html's own intro text says "Values such as gaps and radii are on the first Foundations board; this board says how they combine" - confirming by the page's own words that its component cards (Row/Stack/Track/Split/Overlay, generic prop names `gap`/`inset`/`radius`/`fill`/`width`) are not a source of named foundation values and are out of scope entirely, in both directions.

## Three tiers (this is the content of AC#3)

**Tier A - name + exact value, refuses on any mismatch (both directions):**
- The 17 colours with a 4-column row on Tokens.dc.html (`colours[]` minus `track`, which has no named row anywhere in the body - script-only, Tier C): name exact match, `dark`/`light` exact lowercase hex match.
- The 32 ANSI colours (16 names x 2 themes) on Details.dc.html: name match after translating the file's `_` to the page's space (`bright_black` <-> `bright black`), hex exact match, scoped to the correct theme's section so "black" in "steg dark" isn't compared against "steg light"'s row.

**Tier B - name present, every number in the file's entry found in that same name's value cell (not page-wide - scoped to the one row):**
The 2-column rows whose name matches a file key exactly: `type.font.ui`, `type.font.terminal`, `type.terminal`, `type.ui.body`, `type.ui.small`, `type.ui.label`, `type.palette.input`, `sizes.space`, `sizes.nav.side`, `sizes.nav.top`, `sizes.tab.width`, `sizes.pane.gap`, `sizes.pane.caption`, `sizes.palette`, `sizes.status.ring`, `sizes.scrim`. A file entry's numbers are every int/float reachable in its JSON value (ignoring `ref`/`use`/`unit` string fields). `font.ui`/`font.terminal` have no numbers, so this degrades to a name-presence check for those two - documented, not hidden. This is deliberately weaker than Tier A: it cannot see that a number landed in the wrong field, only that it's present somewhere in the row's own sentence.

**Tier C - declared unseen, each with its own one-line reason, printed by `steg-foundations-check` itself (not just the README) so passing is never silently read as "fully checked":**
- `track` (no named row anywhere in the body), `colour_aliases` (`blocked`/`done`/`error`: script-only), `opacities.faint` (script-only).
- `ramps` (`ramp`, `ramp.extended`): the gradients are drawn but their per-stop percentages are not in rendered text, only in the script's `ramp6`. `ramp.caption_opacity` (50%/40% appears in Rules prose, with no name column).
- `tint`: drawn as Layout's Track card row named `fill` ("The session colour at 18%"), not under the name `tint`, and Layout is out of scope (above).
- `motion.status-fade` (200ms is in Rules prose, no name column).
- `focus_ring.corner_radius_addition`: the page states 12px and 8px; 4 is derived (12-8), and never appears as a number on the page at all.
- `themes[].bg`/`fg`/`bg_inactive`/`selection`: a trailing prose line per theme ("fg #ecebee  bg: the pane, #1c1b21 active...  selection #38354d"), not a named row.
- `sizes.radius.surface/track/inner/round`: the page has exactly one named row, `radius`, merged ("14px panes and palette; 10px tab track and session chips; 8px inside them"), plus unlabelled caption-only swatches (no px number for `round`'s "fully round"). No 1:1 name match exists; this is the exact case TASK-1.92's review caught (`radius.round` drawn with no file entry) and is listed, not silently matched.
- Every `row.gap`/`stack.gap`/`track.layout.*`/`split.layout.*`/`overlay.layout.*` size entry: only on Layout.dc.html, out of scope per the page's own text above.
- The literal `"session"` (a runtime sentinel, no fixed page form) and every `{"ref": ...}` value (no page form of its own - the resolved target's own row is what Tier A/B check; TASK-1.92's shape check already proved every ref resolves).

The tool's own stdout, when it passes, names every Tier C entry it skipped (count and a one-line reason each) so a clean exit is never read as "every value checked" - this is what AC#3 asks for in the tool's own output, not only the README.

**Also stated plainly, in the plan and the README, as something structurally out of this tool's reach regardless of tier:** agreement between file and page proves nothing about whether a value is part of the *confirmed* design. TASK-1.92's one real miss (`overlay.layout.max_height_percent_of_window: 60`, now removed) was a value the page and the then-draft file *agreed* on - doc-10 flags it as not yet confirmed, a judgement call this tool cannot make. This check is a consistency check between two documents, not a substitute for reading doc-10's open-questions list.

## Reverse direction (page value with no file entry) and its noise

Enumerate rows only from the Tier A/B table shapes above (Tokens.dc.html's four tables, Details.dc.html's two swatch grids) - never Layout.dc.html, never a Rules `<ul>` bullet, never a Stories caption. This by construction excludes ~20 non-values (Layout's `compact`/`regular`/`full`/`dropped first`, prop names, Rules prose) that would otherwise be false "missing" reports.

Against this restricted list, an explicit, reviewed allow-list of page-named values deliberately absent from the file (currently empty - everything on Tokens/Details maps to a file entry today; the table exists so a future addition becomes a deliberate edit to it, not a silent exemption). A name on the restricted list that is in neither the file nor the allow-list is reported as missing, naming the value, its form on the page, and which page - this is `radius.round`'s exact scenario from TASK-1.92, now automatic.

## Module shape (mirrors boundaries.py: pure comparison, thin shell)

`tools/src/steg_tools/foundations.py`:
- `@dataclass(frozen=True)` row types: `ColourRow` (name, dark, light), `AnsiRow` (theme, name, hex), `SentenceRow` (name, text, numbers). A parsed page is `@dataclass(frozen=True) class Page: colours: tuple[ColourRow,...]; sentences: tuple[SentenceRow,...]; ansi: tuple[AnsiRow,...]`.
- `parse_page(html: str) -> Page` - pure, stdlib `html.parser.HTMLParser` only (no new dependency: `tools/pyproject.toml`'s `dependencies = []` stays empty), keyed on header-row cell text as above.
- `@dataclass(frozen=True) class Mismatch`: name, file_form, page_form, page. `@dataclass(frozen=True) class Missing`: name, page_form, page (reverse direction). `@dataclass(frozen=True) class Skipped`: name, reason (Tier C).
- Pure functions: `tier_a_mismatches`, `tier_b_mismatches`, `reverse_missing`, each `(data: dict, pages: dict[str, Page]) -> tuple[...]`, taking already-parsed data, no I/O.
- Thin `main()`: reads `crates/ui/design/foundations.json` and the three pages from disk (only I/O in the module), calls the pure functions, prints one line per `Mismatch`/`Missing` (naming the value, both forms, the page) and the Tier C summary, exits 1 if any `Mismatch`/`Missing`, 0 otherwise.

`tools/pyproject.toml`: add `steg-foundations-check = "steg_tools.foundations:main"` to `[project.scripts]`.
`justfile`: add `check-foundations` (same shape as `check-boundaries`, line ~160) and add it to `check:`'s dependency list (line 36).

## Proof (every line here is a test in the build, or it's dropped and said so in the notes)

`tools/tests/unit/test_foundations.py` (pure, in-memory HTML fragments and dicts, no doubles):
- A 4-column `Name/Dark/Light/Used for` fragment parses to the right `ColourRow`s.
- A 2-column `Name/Value` fragment parses to the right `SentenceRow`s, numbers extracted correctly.
- An ANSI swatch card fragment parses to `AnsiRow`s, scoped per theme section.
- A fragment with a header row but zero data rows parses to an empty tuple (the "found nothing" case `main` turns into a refusal, tested at that layer).
- Tier A: matching colour -> no mismatch; one dark hex changed -> one `Mismatch` naming the value, the file's hex, the page's hex, the page.
- Tier B: a sentence containing all the entry's numbers -> no mismatch; one number missing -> one `Mismatch`.
- Reverse: a page name on the restricted list absent from both the file and the allow-list -> one `Missing`; a name on the allow-list absent from the file -> not reported; a Layout-only name -> never considered at all (not even constructed from Layout.dc.html's fragment).
- Tier C: each declared-skip name produces exactly one `Skipped` with a non-empty reason, never a `Mismatch` or `Missing`.

`tools/tests/adapter/test_foundations_real_workspace.py` (precedent: `test_affected_real_workspace.py`): `main()` against the real `crates/ui/design/foundations.json` and the real three pages exits 0 today.

Negative controls (via `tmp_path` copies of the real file, per team:testing - the repository's real pages stay untouched, only the data-file copy is mutated):
- One colour's dark hex changed in the scratch copy -> exits 1, naming the value and both forms.
- One colour entry removed from the scratch copy -> exits 1, reported as missing (reverse direction, run against the *real* pages which still draw it).

`just check` and CI: `check-foundations` added to `justfile`'s `check:` list; no separate CI wiring needed since `check-foundations` runs inside `check`, already gated in CI.

## Overlaps with TASK-1.93 and TASK-1.94

Both touch `tools/pyproject.toml`'s `[project.scripts]` table and `justfile`'s `check:` line - three tasks landing on the same two lines is the likely merge conflict, not a design overlap; each adds its own script/recipe name, so a textual merge resolves it. `affected.py`'s `GLOBAL_FOLDERS` already includes `tools/`, so this change (like 1.93/1.94's) selects `test: all` rather than picking crates by diff - expected, not a defect to fix here. No functional overlap: 1.93 writes `generated.rs` from the data file (consumes the file, doesn't check it against pages), 1.94 adds crate-level checks inside `crates/ui` (R117/R119/R120, nothing about the design pages).

## For the engineer to decide

1. Rendered body only, never the script block - confirmed from the actual markup, not assumed; flagging because it's the one choice that determines whether `track`/`off`/`faint`/the aliases are "missing" or "out of scope."
2. The `radius.*` Tier C call: four file keys, one merged page row, no 1:1 map. Alternative is a hand-maintained `{"sizes.radius.surface": "radius", ...}` name-map reviewed and committed alongside - more coverage, more to keep honest by hand. Recommending Tier C (simpler, and the merged row's own numbers (14/10/8) still don't individually tie to a key without the map anyway).
3. The reverse-direction allow-list starts empty. Any future page addition with no file entry is a deliberate choice to extend that table, not silence - worth confirming that's the intended failure mode (refuse first, allow-list second) rather than warn-only.
<!-- SECTION:PLAN:END -->
