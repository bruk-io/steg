---
id: TASK-1.96
title: 'tools: check the design''s data file against the saved foundations pages'
status: In Progress
assignee:
  - '@bruk.w8'
created_date: '2026-11-16T01:05:00Z'
updated_date: '2026-11-16T02:39:00Z'
labels:
  - size-2
milestone: m-4
dependencies:
  - TASK-1.92
parent_task_id: TASK-1
project: bruk-io-steg
asdlc_accepted_from: DRAFT-1
asdlc_branch: task-1.96-tools-check-the-design-s-data-file
asdlc_plan_approved_by: bruk
asdlc_plan_approved_hash: 'd2f87de12c3c'
asdlc_plan_approved_size: size-2
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
One reviewable change. New module `tools/src/steg_tools/design_pages.py`, CLI `steg-design-check`, recipe `check-design`, in `just check`.

## What "appears under the same name and value" can mean, settled from the actual markup

Read all three saved pages at `design/canvas/project/*.dc.html` (not the generator scripts: TASK-1.92's review already showed the scripts don't reproduce today's Tokens.dc.html, and this tool's job is the saved pages, per the task description). Every page has a rendered body followed by one `<script type="text/x-dc">` block holding the page's own internal colour-variable object (`c = {ground, ink, ink2, muted, quiet, hover, off, track, faint, ramp6, ...}`) used to drive the preview, under the *page's* vocabulary (`ink`, `off`, `quiet`), not the design's names (`text`, `surface.overlay`, `fill.quiet`). **The tool reads only the rendered body** (everything before `<script type="text/x-dc"`); the script block is never parsed as page content. Checking it would require a second, separate name-translation table and would make `off` look like a page-named value missing from the file - exactly the value the engineer already decided to drop (TASK-1.92 comment #2).

Within the rendered body, a named value sits in one of two row shapes, always preceded by its own header row:
- 4-column: header cells read `Name` / `Dark` / `Light` / `Used for`; a data row is `<name span (no colour style)> <swatch span containing a nested `background:#hex` swatch + a `#hex` text span> <same for light> <use text>`.
- 2-column: header cells read `Name` / `Value`; a data row is `<name span> <value span, free text>`.

Both shapes are `display:grid` divs, but **the parser keys on the header row's cell text, never on the `grid-template-columns` pixel string** - `design/canvas/README.md` records that Bruk hand-resized boxes on the Tokens page in the canvas after the generator last ran, so a width-keyed parser would silently match zero rows after a future resize and the check would pass by finding nothing. Each table has an asserted minimum row count, checked today: Tokens' two 4-column tables >=10 and >=7 rows ("Surfaces and text", "The six colours"); its two 2-column tables >=7 and >=10 rows ("Type", "Sizes, spacing and corners"); Details' two ANSI grids >=16 rows each ("steg dark", "steg light"). `>=`, not `==`, so adding a colour to the design later doesn't fail the parser instead of the comparison. Finding fewer than the minimum on a table the page is expected to hold is itself a refusal ("found a Name/Value table with 3 rows on Tokens.dc.html's Type section, expected at least 7"), never a silent pass.

Verified against the real files (not assumed): on Tokens.dc.html there are exactly two 4-column tables (10 rows under "Surfaces and text", 7 under "The six colours" - 17 names total, matching the file's `colours[]` minus `track`) and exactly two 2-column tables ("Type", 7 rows; "Sizes, spacing and corners", 10 rows). Details.dc.html has one ANSI swatch grid per theme (8+8 three-span cards: swatch, name text, hex text), under section headers "Terminal theme: steg dark" / "steg light" matching `themes[].name` exactly. Layout.dc.html's own intro text says "Values such as gaps and radii are on the first Foundations board; this board says how they combine" - confirming by the page's own words that its component cards (Row/Stack/Track/Split/Overlay, generic prop names `gap`/`inset`/`radius`/`fill`/`width`) are not a source of named foundation values and are out of scope entirely, in both directions.

## Three tiers (this is the content of AC#3)

**Tier A - name + exact value, refuses on any mismatch (both directions):**
- The 17 colours with a 4-column row on Tokens.dc.html (`colours[]` minus `track`, which has no named row anywhere in the body - script-only, Tier C): name exact match, `dark`/`light` exact lowercase hex match.
- The 32 ANSI colours (16 names x 2 themes) on Details.dc.html: name match after translating the file's `_` to the page's space (`bright_black` <-> `bright black`), hex exact match, scoped to the correct theme's section so "black" in "steg dark" isn't compared against "steg light"'s row.

**Tier B - name present, every number in the file's entry found in that same name's value cell (not page-wide - scoped to the one row):**
The 2-column rows whose name matches a file key exactly: `type.font.ui`, `type.font.terminal`, `type.terminal`, `type.ui.body`, `type.ui.small`, `type.ui.label`, `type.palette.input`, `sizes.space`, `sizes.nav.side`, `sizes.nav.top`, `sizes.tab.width`, `sizes.pane.gap`, `sizes.pane.caption`, `sizes.palette`, `sizes.status.ring`, `sizes.scrim`. A file entry's numbers are every int/float reachable in its JSON value (ignoring `ref`/`use`/`unit` string fields). `font.ui`/`font.terminal` are the only two Tier B entries with zero numbers to check, so for those two specifically this degrades to a name-presence check - documented here so a reader doesn't wonder which others silently degrade; every other Tier B entry has at least one number checked. `sizes.scrim` has a row on both Tokens.dc.html ("Sizes, spacing and corners") and Layout.dc.html's Overlay card; since Layout is out of scope entirely (above), Tokens' row is the one checked - stated once here as the rule, not decided by whichever page the parser happens to read first. A file key that somehow matched a name on more than one *in-scope* page would itself be a refusal (ambiguous match), not a silent first-match-wins; today this never happens; the module's test suite asserts it stays that way. This is deliberately weaker than Tier A: it cannot see that a number landed in the wrong field, only that it's present somewhere in the row's own sentence.

**Tier C - declared unseen, each with its own one-line reason, printed by `steg-design-check` itself (not just the README) so passing is never silently read as "fully checked":**
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

## Module shape (mirrors boundaries.py: pure comparison, thin shell) - reuses TASK-1.94's reader, does not duplicate it

TASK-1.94 (approved, building) is adding `tools/src/steg_tools/foundations.py`: `read(path) -> Foundations` (a frozen dataclass parse of `crates/ui/design/foundations.json`) and `hex_index(foundations) -> dict[str, tuple[NamedColour, ...]]`, resolving one level of `{"ref": ...}`. **This task imports that module for reading the data file; it does not write its own JSON loader.** `tier_a_mismatches`/`tier_b_mismatches`/`reverse_missing` below take a `Foundations` (TASK-1.94's type), not a raw `dict`.

Build-time ordering, stated now so it isn't improvised later: if TASK-1.94's `foundations.py` has merged to `main` by the time this task builds, import it directly. If it has not, build against a minimal private shim inside this module (a few lines: `json.loads` plus the handful of fields this tool actually reads - colours, type, sizes, themes) under a name that cannot collide (`_fallback_read`), delete the shim and switch to the real import on the next rebase onto `main` once TASK-1.94 has landed. This task does not wait on TASK-1.94's merge to start building; it only waits to delete the shim.

`tools/src/steg_tools/design_pages.py`:
- `@dataclass(frozen=True)` row types: `ColourRow` (name, dark, light), `AnsiRow` (theme, name, hex), `SentenceRow` (name, text, numbers). A parsed page is `@dataclass(frozen=True) class Page: colours: tuple[ColourRow,...]; sentences: tuple[SentenceRow,...]; ansi: tuple[AnsiRow,...]`.
- `parse_page(html: str) -> Page` - pure, stdlib `html.parser.HTMLParser` only (no new dependency beyond what TASK-1.94 already adds to `tools/pyproject.toml`), keyed on header-row cell text as above.
- `@dataclass(frozen=True) class Mismatch`: name, file_form, page_form, page. `@dataclass(frozen=True) class Missing`: name, page_form, page (reverse direction). `@dataclass(frozen=True) class Skipped`: name, reason (Tier C).
- Pure functions: `tier_a_mismatches`, `tier_b_mismatches`, `reverse_missing`, each `(foundations: Foundations, pages: dict[str, Page]) -> tuple[...]`, taking an already-parsed `Foundations` and already-parsed pages, no I/O.
- Thin `main()`: calls `steg_tools.foundations.read(...)` for the data file and reads the three pages from disk (only I/O in the module), calls the pure functions, prints one line per `Mismatch`/`Missing` (naming the value, both forms, the page) and the Tier C summary, exits 1 if any `Mismatch`/`Missing`, 0 otherwise.

`tools/pyproject.toml`: add `steg-design-check = "steg_tools.design_pages:main"` to `[project.scripts]`. No new dependency of this task's own.
`justfile`: add `check-design` (same shape as `check-boundaries`, line ~160) and add it to `check:`'s dependency list (line 36).

## Proof (every line here is a test in the build, or it's dropped and said so in the notes)

`tools/tests/unit/test_design_pages.py` (pure, in-memory HTML fragments and dicts, no doubles):
- A 4-column `Name/Dark/Light/Used for` fragment parses to the right `ColourRow`s.
- A 2-column `Name/Value` fragment parses to the right `SentenceRow`s, numbers extracted correctly.
- An ANSI swatch card fragment parses to `AnsiRow`s, scoped per theme section.
- A fragment with a header row but zero data rows parses to an empty tuple (the "found nothing" case `main` turns into a refusal, tested at that layer).
- Tier A: matching colour -> no mismatch; one dark hex changed -> one `Mismatch` naming the value, the file's hex, the page's hex, the page.
- Tier B: a sentence containing all the entry's numbers -> no mismatch; one number missing -> one `Mismatch`.
- Reverse: a page name on the restricted list absent from both the file and the allow-list -> one `Missing`; a name on the allow-list absent from the file -> not reported; a Layout-only name -> never considered at all (not even constructed from Layout.dc.html's fragment).
- Tier C: each declared-skip name produces exactly one `Skipped` with a non-empty reason, never a `Mismatch` or `Missing`.

`tools/tests/adapter/test_design_pages_real_workspace.py` (precedent: `test_affected_real_workspace.py`): `main()` against the real `crates/ui/design/foundations.json` and the real three pages exits 0 today.

Negative controls (via `tmp_path` copies of the real file, per team:testing - the repository's real pages stay untouched, only the data-file copy is mutated):
- One colour's dark hex changed in the scratch copy -> exits 1, naming the value and both forms.
- One colour entry removed from the scratch copy -> exits 1, reported as missing (reverse direction, run against the *real* pages which still draw it).

`just check` and CI: `check-design` added to `justfile`'s `check:` list; no separate CI wiring needed since `check-design` runs inside `check`, already gated in CI.

## Overlaps with TASK-1.93 and TASK-1.94

**TASK-1.94 is a real functional dependency, not just a naming overlap**: this task imports `steg_tools.foundations.read()` (TASK-1.94's shared reader) instead of writing its own, per the above - TASK-1.94's own plan already names this task as the reason it built `foundations.py` as a shared module rather than inlining the reader into its own checks. Beyond that, both TASK-1.93 and TASK-1.94 touch `tools/pyproject.toml`'s `[project.scripts]` table and `justfile`'s `check:` line - three tasks landing on the same two lines is the likely merge conflict on top of the functional dependency; each adds its own script/recipe name, so a textual merge resolves it. `affected.py`'s `GLOBAL_FOLDERS` already includes `tools/`, so this change (like 1.93/1.94's) selects `test: all` rather than picking crates by diff - expected, not a defect to fix here. No other functional overlap: 1.93 writes `generated.rs` from the data file (consumes the file, doesn't check it against pages), 1.94's two checks are about `crates/ui`'s own source (R117/R119/R120), nothing about the design pages this task reads.

## For the engineer to decide

0. **AC#1 is satisfied-with-one-named-exception, not fully satisfied, and that's a feature of the file, not a gap in the tool.** `colours[]` has 18 entries; the saved pages name only 17 of them - `track` has no 4-column row anywhere in either rendered body (confirmed by grep, not assumed; TASK-1.92's review claimed all 18 were "found in Tokens.dc.html" by grepping hex values, which is weaker evidence than a name-row check and turns out to be wrong for `track`). TASK-1.92 comment #2 already decided `track` belongs in the file regardless ("track and off are named as real colours; they are drawn, so they exist" - meaning drawn as a swatch style value, not drawn under its own name). So the tool will always report one Tier C skip for a colour that genuinely is in the file, by design, and AC#1's "every value ... appears ... under the same name" is accurate for 17 of 18 colours, not all 18. Flagging this before build, not after, since it reads as a shortfall against the acceptance criterion's literal wording unless the engineer confirms the exception is intended.

1. Rendered body only, never the script block - confirmed from the actual markup, not assumed; flagging because it's the one choice that determines whether `track`/`off`/`faint`/the aliases are "missing" or "out of scope."
2. The `radius.*` Tier C call: four file keys, one merged page row, no 1:1 map. Alternative is a hand-maintained `{"sizes.radius.surface": "radius", ...}` name-map reviewed and committed alongside - more coverage, more to keep honest by hand. Recommending Tier C (simpler, and the merged row's own numbers (14/10/8) still don't individually tie to a key without the map anyway).
3. The reverse-direction allow-list starts empty. Any future page addition with no file entry is a deliberate choice to extend that table, not silence - worth confirming that's the intended failure mode (refuse first, allow-list second) rather than warn-only.
4. Decision 0's gap is in the design's own page, not in this tool or the data file: `track` is a named colour of the design with no named row on Tokens.dc.html. Correcting the artifact (adding a `track` row to the colours table, the same kind of correction TASK-1.92 already made twice for `off` and the scrim sentence) is the engineer's to ask for, not this task's to do. Until the artifact is corrected, `track` stays a declared Tier C exception rather than something this task works around.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Plan checked against doc-17 (R121, R122) by @bruk.o: ok, with the decisions listed at its end for the engineer. The tool reads the rendered body of the saved Tokens and Details pages (not their script blocks, and not the Layout page, whose own text says it names no values) and sorts every entry of the data file into three tiers that the tool itself prints: exact name and value (17 colours, the 32 terminal colours), name present with its numbers checked within the row (type, most sizes), and declared unseen with a reason. The reverse direction lists the named rows of the same tables and reports any with no entry in the file. It reads the data file through TASK-1.94's shared reader. Named so it does not clash with TASK-1.93 or TASK-1.94: design_pages.py, steg-design-check, check-design. Orchestrator's recommendation on the corners: a small reviewed name map (four entries) so the three pixel corners are checked against the merged row's numbers, since a missed corner is the very error TASK-1.92's review caught by hand; the engineer decides.

Check passed: `just check` exited 0 at 2026-11-16T02:35:52+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.96 on task-1.96-tools-check-the-design-s-data-file at f99466903b08:

```text
ok      format check (0.2s)
ok      lint (89.2s)
ok      boundaries (0.2s)
ok      licences (0.7s)
skip    tests: this change touches no crate
ok      build (steg-sdk) (3.1s)
ok      test (steg-sdk) (1.7s)
ok      test (steg-sdk, local-socket) (3.7s)
ok      build (steg-kernel) (46.6s)
ok      test (steg-kernel) (1.7s)
ok      build (steg-server) (50.9s)
ok      test (steg-server) (7.8s)
ok      test (steg-server, race-fixture) (6.8s)
ok      build (steg) (1.9s)
ok      test (steg) (0.4s)
ok      test (steg, fixtures) (3.4s)
ok      build (steg-ui) (20.0s)
ok      test (steg-ui) (1.4s)
ok      build (steg-app) (1.6s)
ok      test (steg-app) (0.9s)
ok      tool tests (1.6s)
ok      architecture (0.8s)
ok      recipe names (0.1s)
```
<!-- SECTION:NOTES:END -->

## Comments

<!-- COMMENTS:BEGIN -->
author: @bruk
created: 2026-11-16T02:30:00Z

---

Plan approved by @bruk (d2f87de12c3c, size-2): its worker may build it.

---

created: 2026-11-16T02:32:00Z

---

Decided by the engineer on 2026-11-15, after approving the plan ('a name map yes, and add track'): (1) the corners are checked through a small reviewed name map from the file's radius keys to the page's one merged radius row, so 14, 10 and 8 are checked against that row's numbers; the fully round corner has no number on the page and stays declared unseen. (2) The design artifact now names the track colour: @bruk.o added a row to the Tokens page's first colours table, after outline (track, #3d3b45 dark, #cfcbc5 light, 'The grey track behind a progress ring or bar'), and made that board 40px taller; the artifact's version is now 1794796322-6d8e and nothing else changed. The Tokens page as it is in that version is saved at /Users/brukhabtu/.claude/projects/-Users-brukhabtu-src-github-com-bruk-io-steg/design-artifact-pages-1794796322-6d8e/Tokens.dc.html . This task therefore also carries the re-export that follows from it (doc-17 R122: a change arrives as a pull request whose diff shows what changed): replace design/canvas/project/Tokens.dc.html with that file, set artifact_version in crates/ui/design/foundations.json to 1794796322-6d8e, update the READMEs' mention of the version, and move track from the declared exceptions to the exactly checked colours (18 of 18). No value in the data file changes.

---

created: 2026-11-16T02:39:00Z

---

2026-11-15, one more correction to the design, at the engineer's word (he took @bruk.o's recommendation): fill.quiet's description said 'Key caps, progress track, a button under the pointer', but every progress bar and ring in the design's pages is drawn with track and none with fill.quiet (checked across all the pages: 48 bars on track, none on fill.quiet), so 'progress track' was stale. The Tokens page now reads 'Key caps, a button under the pointer'. The artifact's version is now 1794796789-1b13, which supersedes 1794796322-6d8e from the comment above: the page to commit is /Users/brukhabtu/.claude/projects/-Users-brukhabtu-src-github-com-bruk-io-steg/design-artifact-pages-1794796789-1b13/Tokens.dc.html , artifact_version is 1794796789-1b13, and fill.quiet's use text in crates/ui/design/foundations.json changes to match. Colour values are unchanged; the data file's diff is the version line and that one description.

---
<!-- COMMENTS:END -->
