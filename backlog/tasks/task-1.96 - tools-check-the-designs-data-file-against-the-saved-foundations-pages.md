---
id: TASK-1.96
title: 'tools: check the design''s data file against the saved foundations pages'
status: In Review
assignee:
  - '@bruk.w8'
created_date: '2026-11-16T01:05:00Z'
updated_date: '2026-11-16T05:02:00Z'
labels:
  - size-2
milestone: m-4
dependencies:
  - TASK-1.92
references:
  - task-1.96-tools-check-the-design-s-data-file
  - https://github.com/bruk-io/steg/pull/54
parent_task_id: TASK-1
project: bruk-io-steg
asdlc_accepted_from: DRAFT-1
asdlc_branch: task-1.96-tools-check-the-design-s-data-file
asdlc_plan_approved_by: bruk
asdlc_plan_approved_hash: 'd2f87de12c3c'
asdlc_plan_approved_size: size-2
asdlc_review_kind: same-machine
asdlc_review_findings: 'important=1,nit=1'
asdlc_review_policies: ["review", "testing"]
asdlc_plan_matched: no
asdlc_reviewer: bruk.r5
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

Check passed: `sh -c 'cd tools && uv run --project . pytest tests/unit/test_design_pages.py -q'` exited 0 at 2026-11-16T02:43:30+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.96 on task-1.96-tools-check-the-design-s-data-file at f99466903b08 with uncommitted changes:

```text
warning: `VIRTUAL_ENV=/Users/brukhabtu/.cache/uv/builds-v0/.tmp5eNB2P` does not match the project environment path `.venv` and will be ignored; use `--active` to target the active environment instead
..........................                                               [100%]
26 passed in 0.01s
```

Check passed: `sh -c 'cd tools && uv run --project . pytest tests/adapter/test_design_pages_real_workspace.py -q'` exited 0 at 2026-11-16T02:43:54+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.96 on task-1.96-tools-check-the-design-s-data-file at f99466903b08 with uncommitted changes:

```text
warning: `VIRTUAL_ENV=/Users/brukhabtu/.cache/uv/builds-v0/.tmp6velY9` does not match the project environment path `.venv` and will be ignored; use `--active` to target the active environment instead
....                                                                     [100%]
4 passed in 0.07s
```

Check passed: `just check` exited 0 at 2026-11-16T02:45:47+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.96 on task-1.96-tools-check-the-design-s-data-file at f99466903b08 with uncommitted changes:

```text
ok      format check (0.2s)
ok      lint (0.9s)
ok      boundaries (0.2s)
ok      design pages (0.1s)
ok      licences (0.8s)
ok      tests (every crate) (54.4s)
ok      build (steg-sdk) (0.2s)
ok      test (steg-sdk) (0.6s)
ok      test (steg-sdk, local-socket) (0.9s)
ok      build (steg-kernel) (0.9s)
ok      test (steg-kernel) (0.7s)
ok      build (steg-server) (0.2s)
ok      test (steg-server) (5.2s)
ok      test (steg-server, race-fixture) (5.2s)
ok      build (steg) (0.5s)
ok      test (steg) (0.3s)
ok      test (steg, fixtures) (3.1s)
ok      build (steg-ui) (0.9s)
ok      test (steg-ui) (0.6s)
ok      build (steg-app) (0.4s)
ok      test (steg-app) (0.5s)
ok      tool tests (1.9s)
ok      architecture (0.8s)
ok      recipe names (0.1s)
```

Check passed: `just check` exited 0 at 2026-11-16T02:46:50+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.96 on task-1.96-tools-check-the-design-s-data-file at dcbeaf2d3b83:

```text
ok      format check (0.2s)
ok      lint (0.6s)
ok      boundaries (0.1s)
ok      design pages (0.1s)
ok      licences (0.9s)
ok      tests (every crate) (5.3s)
ok      build (steg-sdk) (0.1s)
ok      test (steg-sdk) (0.4s)
ok      test (steg-sdk, local-socket) (0.5s)
ok      build (steg-kernel) (0.1s)
ok      test (steg-kernel) (0.3s)
ok      build (steg-server) (0.1s)
ok      test (steg-server) (4.4s)
ok      test (steg-server, race-fixture) (4.4s)
ok      build (steg) (0.1s)
ok      test (steg) (0.2s)
ok      test (steg, fixtures) (2.5s)
ok      build (steg-ui) (0.3s)
ok      test (steg-ui) (0.3s)
ok      build (steg-app) (0.3s)
ok      test (steg-app) (0.2s)
ok      tool tests (1.3s)
ok      architecture (1.0s)
ok      recipe names (0.1s)
```

Built per the approved plan (digest d2f87de12c3c): tools/src/steg_tools/design_pages.py, CLI steg-design-check, recipe check-design wired into just check.

Checks (all exit 0, recorded on the task):
- just check, before any change (baseline).
- Unit: tools/tests/unit/test_design_pages.py, 26 tests, first run found two real bugs (fixed before recording green): find_table_rows was passing the colour row's 4-cell tuple straight to ColourRow's 3-arg constructor; ambiguous_matches returned a 1-element tuple for the ordinary single-page case instead of () (fixed to only return non-empty when truly ambiguous); a float-imprecision case in the opacity-to-percent conversion (0.28*100 == 28.000000000000004) that would never equal the page's regex-extracted 28.0 (fixed by rounding to the nearest whole percent, documented in the code as the design's own convention).
- Adapter: tools/tests/adapter/test_design_pages_real_workspace.py - the real data file against the real saved pages passes today; the two required negative controls (a changed dark hex in a scratch copy is refused naming both forms; a removed colour entry is reported missing via the reverse direction, since the real pages still draw it); plus a sanity check that a byte-identical scratch copy still passes.
- just check at the rebased final head (dcbeaf2), after fetching and rebasing onto origin/main (branch had never been pushed) - clean, no conflicts: TASK-1.93/1.94/1.97 hadn't merged tools/pyproject.toml or justfile entries yet.
- CI green on PR 54 at head dcbeaf2: gh pr checks 54 --watch, check passed in 4m20s.

Build-time ordering (plan's "Module shape"): TASK-1.94's steg_tools.foundations.read() had not merged to origin/main at any point during this build (confirmed tools/src/steg_tools/ on origin/main before starting and again before pushing: no foundations.py either time) - so this task's design_pages.py reads the data file itself via a private _read_data_file, not TASK-1.94's shared reader. Swapping to that import once TASK-1.94 lands is a follow-up, named in the module's docstring and the PR description.

Coverage, clause by clause (every proof item from the plan is in the test files above, none dropped):
- AC#1: forward direction (Tier A exact-match for 17/18 colours + 32 ANSI colours, Tier B number-subset for 16 type/size entries) and reverse direction (page value missing from file, restricted to the same rows, allow-list "radius") both built and tested; a refusal names the value and both forms (file_form/page_form on Mismatch/Missing).
- AC#2: unit tests with no I/O, an adapter test against the real files passing today, both required negative controls (value changed -> refused; value removed -> reported missing), part of just check and gated in CI via .github/workflows/ci.yml's existing `just check` call.
- AC#3: TIER_C_SKIPS (24 entries) names everything this tool cannot see and why; printed by the tool itself on every passing run (not only the README), plus the module docstring's explicit statement that page/file agreement doesn't establish a value is part of the confirmed design (TASK-1.92's overlay.layout.max_height_percent_of_window precedent).

One correction to the plan's own text, discovered during build (not a scope change - substance unchanged, just an internal inconsistency in my own planning prose): the plan said the reverse-direction allow-list "starts empty... everything on Tokens/Details maps to a file entry today." That's wrong by one entry: the Sizes table's "radius" row has no exact file key (file has radius.surface/track/inner/round, none spelled "radius"), so REVERSE_ALLOW_LIST contains exactly {"radius"} today, not nothing - the same radius.* exception the plan's Tier C section already discusses, just restated correctly for the reverse-direction registry specifically. No other reverse-direction surprises found; every other page name (17 colours, 16 type/size names, 32 ANSI names after underscore/space translation) matches a file key exactly, verified both by the adapter test and by hand against the real files during planning.

PR: https://github.com/bruk-io/steg/pull/54 (head dcbeaf2, CI green).

Check passed: `sh -c 'cd tools && uv run --project . pytest tests/unit/test_design_pages.py tests/adapter/test_design_pages_real_workspace.py -q'` exited 0 at 2026-11-16T03:17:49+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.96 on task-1.96-tools-check-the-design-s-data-file at 7152cd7a370a with uncommitted changes:

```text
warning: `VIRTUAL_ENV=/Users/brukhabtu/.cache/uv/builds-v0/.tmpYK1CIf` does not match the project environment path `.venv` and will be ignored; use `--active` to target the active environment instead
..................................................                       [100%]
50 passed in 0.38s
```

Commits since the review: 10, 10 without the Asdlc-Worker trailer.

Check passed: `just check` exited 0 at 2026-11-16T03:18:21+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.96 on task-1.96-tools-check-the-design-s-data-file at 7152cd7a370a with uncommitted changes:

```text
ok      format check (0.2s)
ok      lint (0.5s)
ok      boundaries (0.1s)
ok      design pages (0.1s)
ok      licences (0.7s)
ok      tests (every crate) (4.5s)
ok      build (steg-sdk) (0.1s)
ok      test (steg-sdk) (0.2s)
ok      test (steg-sdk, local-socket) (0.4s)
ok      build (steg-kernel) (0.1s)
ok      test (steg-kernel) (0.2s)
ok      build (steg-server) (0.1s)
ok      test (steg-server) (3.9s)
ok      test (steg-server, race-fixture) (4.2s)
ok      build (steg) (0.1s)
ok      test (steg) (0.1s)
ok      test (steg, fixtures) (2.5s)
ok      build (steg-ui) (0.3s)
ok      test (steg-ui) (0.3s)
ok      build (steg-app) (0.3s)
ok      test (steg-app) (0.3s)
ok      tool tests (1.5s)
ok      architecture (1.4s)
ok      recipe names (0.1s)
```

Commits since the review: 10, 10 without the Asdlc-Worker trailer.

Check passed: `just check` exited 0 at 2026-11-16T03:19:37+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.96 on task-1.96-tools-check-the-design-s-data-file at 8626dc6a9474:

```text
ok      format check (0.2s)
ok      lint (0.5s)
ok      boundaries (0.1s)
ok      design pages (0.1s)
ok      licences (0.8s)
ok      tests (every crate) (4.7s)
ok      build (steg-sdk) (0.1s)
ok      test (steg-sdk) (0.3s)
ok      test (steg-sdk, local-socket) (0.4s)
ok      build (steg-kernel) (0.2s)
ok      test (steg-kernel) (0.2s)
ok      build (steg-server) (0.1s)
ok      test (steg-server) (4.4s)
ok      test (steg-server, race-fixture) (4.5s)
ok      build (steg) (0.2s)
ok      test (steg) (0.2s)
ok      test (steg, fixtures) (2.5s)
ok      build (steg-ui) (0.4s)
ok      test (steg-ui) (0.4s)
ok      build (steg-app) (0.3s)
ok      test (steg-app) (0.3s)
ok      tool tests (1.6s)
ok      architecture (1.0s)
ok      recipe names (0.1s)
```

Commits since the review: 80, 79 without the Asdlc-Worker trailer.

Review findings addressed (@bruk.r1, 3 Important, PR 54 at pushed head dcbeaf2):

1. The engineer's decisions (task comments #2/#3) are now in: design/canvas/project/Tokens.dc.html replaced byte-for-byte with the saved .../design-artifact-pages-1794796789-1b13/Tokens.dc.html (checksum 765c9e4e535e2af92d16d403ed9cd61751bc0c3b79e8b36adeacae85712bac4d confirmed both sides). crates/ui/design/foundations.json's diff is exactly the two lines promised (artifact_version; fill.quiet.use). track moved from TIER_C_SKIPS to Tier A (18 of 18 colours, no exception left; EXPECTED_SHAPES' Tokens colours minimum 17->18). The corner map (RADIUS_MAP: radius.surface/track/inner -> the page's one merged "radius" row, 14/10/8) replaces the three sizes.radius.* TIER_C_SKIPS entries; radius.round stays declared unseen. REVERSE_ALLOW_LIST went back to empty: reverse_missing now takes an explicit `allow` parameter and main() unions REVERSE_ALLOW_LIST with RADIUS_MAP.values(), so "radius" is matched through the map, not excused by a separate allow-list entry. Both READMEs updated (design/canvas/README.md's version note; a new section in crates/ui/design/README.md on steg-design-check's coverage and the field-blind caveat).

2. The number check's field-blindness (reviewer's own negative control: swapping tab.width's usual/min left the same two numbers present and still passed) is tightened via FIELD_WORD_MAPS/RADIUS_WORD_MAP: a field's number is tied to the word that follows it in the page's own sentence (`_word_tied_number`/`_tightened_mismatches`), for every field whose clause has exactly one number followed by a distinguishing word - tab.width (all 3 fields), nav.side/nav.top/pane.gap/sizes.palette/status.ring (some fields), the three radius corners (RADIUS_WORD_MAP). Every field not covered this way stays on the weaker "present somewhere" check, now stated in main()'s own passing stdout and in crates/ui/design/README.md's new section, not only the module docstring.

3. focus_ring.width/offset (neither checked nor declared unseen) are now in TIER_C_SKIPS, along with focus_ring.colour, themes[].cursor, and the top-level session_colour - three more occurrences of the literal "session" sentinel the same gap could have hidden, found while building `unaccounted_names()`: enumerates every leaf key of the data file against TIER_B_NAMES/RADIUS_MAP/TIER_C_SKIPS, wired into main() as its own refusal (not just a test), so a key added later can't fall through both silently.

Checks (all exit 0, recorded on the task):
- Unit+adapter: 50 tests (tools/tests/unit/test_design_pages.py, tools/tests/adapter/test_design_pages_real_workspace.py) - new tests for the tab.width swap (both as a synthetic fixture and against the real file), the FIELD_WORD_MAPS fallback case, unaccounted_names and its required negative control (removing one TIER_C_SKIPS entry makes exactly that key unaccounted), and the radius-map's own negative control.
- just check at the merged final head (8626dc6), after fetching and merging origin/main (branch already pushed, so merged not rebased) - clean, no conflicts: TASK-1.93/1.94/1.97 still hadn't merged tools/pyproject.toml, justfile, or crates/ui/src/foundations/generated.rs as of this head, confirmed again before pushing.
- CI green on PR 54 at head 8626dc6: gh pr checks 54 --watch, check passed in 2m43s.

Tool's own output against the real files today: "ok (18 colours, 19 type/size entries, 32 ANSI colours checked)" plus the field-blind caveat (13 fields tightened) and 26 declared-unseen entries - full text in the PR description.

TASK-1.94's steg_tools.foundations.read() still hadn't merged to origin/main as of this head (checked again), so the private _read_data_file shim stays; the swap-over remains a named follow-up in the module docstring and PR body.

PR: https://github.com/bruk-io/steg/pull/54 (head 8626dc6, CI green).

Check FAILED: `python3 -I /private/tmp/claude-501/-Users-brukhabtu-src-github-com-bruk-io-steg/a318fbf0-a659-4ad6-9bfc-86a79ba9cc76/scratchpad/task-1.96-old-unaccounted-names-negative-control.py crates/ui/design/foundations.json` exited 1 at 2026-11-16T03:48:19+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.96 on task-1.96-tools-check-the-design-s-data-file at 8626dc6a9474 with uncommitted changes:

```text
baseline (unmodified real file) unaccounted: ()
themes[0]['underline'] added, OLD implementation reports: ()
BUG REPRODUCED: the OLD implementation silently missed themes[0]['underline'] - review finding (@bruk.r5) confirmed
focus_ring['glow'] added, OLD implementation reports: ()
BUG REPRODUCED: the OLD implementation silently missed focus_ring['glow'] - review finding (@bruk.r5) confirmed
```

Commits since the review: 0.

Check passed: `sh -c 'cd tools && uv run --project . pytest tests/unit/test_design_pages.py tests/adapter/test_design_pages_real_workspace.py -q'` exited 0 at 2026-11-16T03:48:27+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.96 on task-1.96-tools-check-the-design-s-data-file at 8626dc6a9474 with uncommitted changes:

```text
warning: `VIRTUAL_ENV=/Users/brukhabtu/.cache/uv/builds-v0/.tmpxQvYnQ` does not match the project environment path `.venv` and will be ignored; use `--active` to target the active environment instead
............................................................             [100%]
60 passed in 0.26s
```

Commits since the review: 0.

Check passed: `just check` exited 0 at 2026-11-16T03:49:17+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.96 on task-1.96-tools-check-the-design-s-data-file at 8626dc6a9474 with uncommitted changes:

```text
ok      format check (0.2s)
ok      lint (0.7s)
ok      boundaries (0.1s)
ok      design pages (0.0s)
ok      licences (0.7s)
ok      tests (every crate) (5.6s)
ok      build (steg-sdk) (0.1s)
ok      test (steg-sdk) (0.4s)
ok      test (steg-sdk, local-socket) (0.5s)
ok      build (steg-kernel) (0.2s)
ok      test (steg-kernel) (0.4s)
ok      build (steg-server) (0.2s)
ok      test (steg-server) (4.4s)
ok      test (steg-server, race-fixture) (4.6s)
ok      build (steg) (0.1s)
ok      test (steg) (0.2s)
ok      test (steg, fixtures) (2.8s)
ok      build (steg-ui) (0.3s)
ok      test (steg-ui) (0.4s)
ok      build (steg-app) (0.3s)
ok      test (steg-app) (0.3s)
ok      tool tests (1.5s)
ok      architecture (1.0s)
ok      recipe names (0.1s)
```

Commits since the review: 0.

Check passed: `just check` exited 0 at 2026-11-16T03:50:16+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.96 on task-1.96-tools-check-the-design-s-data-file at 320b9d785a96:

```text
ok      format check (0.2s)
ok      lint (0.5s)
ok      boundaries (0.1s)
ok      design pages (0.1s)
ok      licences (0.9s)
ok      tests (every crate) (4.6s)
ok      build (steg-sdk) (0.1s)
ok      test (steg-sdk) (0.2s)
ok      test (steg-sdk, local-socket) (0.4s)
ok      build (steg-kernel) (0.1s)
ok      test (steg-kernel) (0.4s)
ok      build (steg-server) (0.1s)
ok      test (steg-server) (3.9s)
ok      test (steg-server, race-fixture) (4.1s)
ok      build (steg) (0.1s)
ok      test (steg) (0.1s)
ok      test (steg, fixtures) (2.4s)
ok      build (steg-ui) (0.3s)
ok      test (steg-ui) (0.3s)
ok      build (steg-app) (0.3s)
ok      test (steg-app) (0.3s)
ok      tool tests (1.4s)
ok      architecture (0.8s)
ok      recipe names (0.0s)
```

Commits since the review: 39, 38 without the Asdlc-Worker trailer.

Review finding (@bruk.r5, 1 Important) addressed: unaccounted_names enumerated themes[]/focus_ring leaf keys from a fixed field tuple, never from the data file's own keys. The reviewer's own reproduction (focus_ring["glow"]=3, themes[0]["underline"]="#fff" on a scratch copy) printed "ok" and exited 0, silently missing both.

Fixed: unaccounted_names rewritten to walk the parsed JSON generically (_leaf_paths): a dict's keys become path.key (skipping a small universal set - name/use/unit/ansi/artifact_version/artifact_url - never a per-section list); a list of dicts each carrying their own "name" is a list of named entries, recursing into each (_entry_path also handles motion's self-prefixed names generically, not as a special case); anything else reached this way is one leaf. A leaf is accounted for if it equals, or is a dotted descendant of, some registered name (_is_accounted_for) - a descendant is accounted for because the checking function that name stands for already recurses the whole value (_numbers_in sees every number under e.g. sizes.nav.side), so nothing numeric there goes uninspected; a new non-numeric field added the same way is a known, accepted gap, now stated in crates/ui/design/README.md, not a silent one.

Consequences: the five themes[].X wildcard skip entries became ten per-instance entries (themes.steg dark.bg, themes.steg light.bg, etc.) - a generic walk can't collapse two theme instances into one pattern without the per-section special-casing the finding rejects. ramps.ramp.caption_opacity became unreachable (its parent ramps.ramp already covers it via the prefix rule) and was folded into that entry's reason text rather than left dead.

"Inside a size's group" (one of the team lead's listed test levels) is read as: a new numeric field inside an already-checked whole value (e.g. sizes.nav.side) is accounted for by its covering ancestor, per the _numbers_in reasoning above - this is a chosen reading, not something the literal review reproduced (which covered only themes[]/focus_ring), flagged as such rather than assumed. Tested explicitly both ways (the numeric case not refused; the non-numeric case documented as a known gap, also not refused).

Checks (recorded on the task):
- Negative control, recorded not claimed: a scratch script (/private/tmp/.../task-1.96-old-unaccounted-names-negative-control.py) runs a verbatim copy of the pre-fix implementation against the real data file with both of the reviewer's mutations applied - exits 1, printing that it silently missed both, confirming the bug before the fix.
- Unit+adapter: 60 tests (up from 50) - new: the reviewer's own two reproductions re-run directly against the real file; one test per nesting level (top-level key, directly inside sizes, inside a theme, inside focus_ring, a new list entry); the numeric/non-numeric "inside a group" pair; two property-style tests over every real leaf path of the current file (split by checked vs declared-unseen), each confirming that removing a registry entry which alone covers a leaf makes that leaf show up named.
- just check at the merged final head (320b9d7), after fetching and merging origin/main (branch already pushed) - clean, no conflicts; TASK-1.93/1.94/1.97 still hadn't merged tools/pyproject.toml, justfile, or crates/ui/src/foundations/generated.rs.
- CI green on PR 54 at head 320b9d7: gh pr checks 54 --watch, check passed in 3m58s.

Tool's own output unchanged in its counts (ok, 18 colours, 19 type/size entries, 32 ANSI colours checked) - only the printed reasons for ramps.ramp (folds in caption_opacity) and the themes lines (now per-instance) changed; full text in the PR description.

PR: https://github.com/bruk-io/steg/pull/54 (head 320b9d7, CI green).

Check FAILED: `sh -c 'cd tools && uv run --project . python "/private/tmp/claude-501/-Users-brukhabtu-src-github-com-bruk-io-steg/a318fbf0-a659-4ad6-9bfc-86a79ba9cc76/scratchpad/task-1.96-string-prefix-negative-control.py" "/private/tmp/claude-501/-Users-brukhabtu-src-github-com-bruk-io-steg/a318fbf0-a659-4ad6-9bfc-86a79ba9cc76/scratchpad/task-1.96-prev-design-pages.py" ../crates/ui/design/foundations.json'` exited 1 at 2026-11-16T04:18:06+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.96 on task-1.96-tools-check-the-design-s-data-file at 320b9d785a96 with uncommitted changes:

```text
warning: `VIRTUAL_ENV=/Users/brukhabtu/.cache/uv/builds-v0/.tmpvd42Xg` does not match the project environment path `.venv` and will be ignored; use `--active` to target the active environment instead
baseline (unmodified real file) unaccounted: ()
sizes['palette.footer']: OLD (string-prefix) implementation reports: ()
BUG REPRODUCED: the OLD implementation silently missed sizes['palette.footer']
type['terminal.bold']: OLD (string-prefix) implementation reports: ()
BUG REPRODUCED: the OLD implementation silently missed type['terminal.bold']
sizes['nav.side.extra']: OLD (string-prefix) implementation reports: ()
BUG REPRODUCED: the OLD implementation silently missed sizes['nav.side.extra']
ramp named 'ramp.secondary': OLD (string-prefix) implementation reports: ()
BUG REPRODUCED: the OLD implementation silently missed ramp named 'ramp.secondary'
```

Commits since the review: 0.

Check passed: `sh -c 'cd tools && uv run --project . pytest tests/unit/test_design_pages.py tests/adapter/test_design_pages_real_workspace.py -q'` exited 0 at 2026-11-16T04:18:12+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.96 on task-1.96-tools-check-the-design-s-data-file at 320b9d785a96 with uncommitted changes:

```text
warning: `VIRTUAL_ENV=/Users/brukhabtu/.cache/uv/builds-v0/.tmpVyiBXs` does not match the project environment path `.venv` and will be ignored; use `--active` to target the active environment instead
...................................................................      [100%]
67 passed in 0.30s
```

Commits since the review: 0.

Check passed: `just check` exited 0 at 2026-11-16T04:18:56+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.96 on task-1.96-tools-check-the-design-s-data-file at 3e4626128b29 with uncommitted changes:

```text
ok      format check (0.2s)
ok      lint (0.5s)
ok      boundaries (0.1s)
ok      design pages (0.0s)
ok      licences (0.7s)
ok      tests (every crate) (4.6s)
ok      build (steg-sdk) (0.1s)
ok      test (steg-sdk) (0.2s)
ok      test (steg-sdk, local-socket) (0.4s)
ok      build (steg-kernel) (0.1s)
ok      test (steg-kernel) (0.2s)
ok      build (steg-server) (0.1s)
ok      test (steg-server) (4.0s)
ok      test (steg-server, race-fixture) (4.2s)
ok      build (steg) (0.1s)
ok      test (steg) (0.1s)
ok      test (steg, fixtures) (2.5s)
ok      build (steg-ui) (0.3s)
ok      test (steg-ui) (0.3s)
ok      build (steg-app) (0.3s)
ok      test (steg-app) (0.2s)
ok      tool tests (1.3s)
ok      architecture (0.8s)
ok      recipe names (0.1s)
```

Commits since the review: 24, 24 without the Asdlc-Worker trailer.

Check passed: `just check` exited 0 at 2026-11-16T04:20:31+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.96 on task-1.96-tools-check-the-design-s-data-file at 13a0b378db1e:

```text
ok      format check (0.2s)
ok      lint (1.2s)
ok      boundaries (0.1s)
ok      design pages (0.0s)
ok      licences (0.7s)
ok      tests (every crate) (9.6s)
ok      build (steg-sdk) (0.1s)
ok      test (steg-sdk) (0.2s)
ok      test (steg-sdk, local-socket) (0.4s)
ok      build (steg-kernel) (0.1s)
ok      test (steg-kernel) (0.2s)
ok      build (steg-server) (0.7s)
ok      test (steg-server) (8.9s)
ok      test (steg-server, race-fixture) (9.7s)
ok      build (steg) (1.0s)
ok      test (steg) (0.4s)
ok      test (steg, fixtures) (3.5s)
ok      build (steg-ui) (0.3s)
ok      test (steg-ui) (0.3s)
ok      build (steg-app) (0.3s)
ok      test (steg-app) (0.2s)
ok      tool tests (11.7s)
ok      architecture (0.8s)
ok      recipe names (0.1s)
```

Commits since the review: 40, 25 without the Asdlc-Worker trailer.

Check passed: `just check` exited 0 at 2026-11-16T04:24:16+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.96 on task-1.96-tools-check-the-design-s-data-file at a304d37220d9:

```text
ok      format check (0.2s)
ok      lint (0.4s)
ok      boundaries (0.1s)
ok      design pages (0.1s)
ok      licences (0.8s)
ok      tests (every crate) (5.4s)
ok      build (steg-sdk) (0.1s)
ok      test (steg-sdk) (0.2s)
ok      test (steg-sdk, local-socket) (0.4s)
ok      build (steg-kernel) (0.1s)
ok      test (steg-kernel) (0.2s)
ok      build (steg-server) (0.1s)
ok      test (steg-server) (6.0s)
ok      test (steg-server, race-fixture) (4.8s)
ok      build (steg) (0.1s)
ok      test (steg) (0.1s)
ok      test (steg, fixtures) (2.5s)
ok      build (steg-ui) (0.3s)
ok      test (steg-ui) (0.3s)
ok      build (steg-app) (0.3s)
ok      test (steg-app) (0.3s)
ok      tool tests (12.2s)
ok      architecture (0.7s)
ok      recipe names (0.1s)
ok      ui levels (0.1s)
ok      ui colours (0.1s)
```

Commits since the review: 13, 8 without the Asdlc-Worker trailer.

Review findings addressed (@bruk.r1, round 3, 2 Important, PR 54):

1. "Accounted for" compared joined strings, and the data file's own keys contain literal dots (font.ui, nav.side, radius.surface, ramp and ramp.extended) - a new key that merely extends a registered one as a string was swallowed unchecked. The reviewer's four reproductions: sizes["palette.footer"], type["terminal.bold"], sizes["nav.side.extra"], and a ramp literally named "ramp.secondary" all printed "ok" before this fix.

Fixed: a path is now a tuple of segments as the JSON has them, never joined for comparison. Skipped.name is a tuple (display() joins with "." for printing only); _leaf_paths returns tuple[str,...]; _is_accounted_for compares whole segments (path[:len(name)] == name), never a substring. _entry_path's old self-prefix special case for motion's self-prefixed name is gone entirely - every list of named entries now just appends the entry's own name as one segment, uniformly, with no section-specific logic.

2. A new field on an existing colour ("contrast": 1.5) was checked by nothing and declared nowhere, because a blanket whole-entry ancestor ("colours", name) silently covered it - contradicting the README's claim that colours are checked exactly. Fixed: every colour, colour alias, opacity and ramp is now registered field by field, never as a whole-entry ancestor - a colour's dark/light; an alias's ref; an opacity's dark/light; a ramp's stops and, on "ramp" only, caption_opacity. TIER_C_SKIPS's colour_aliases/opacities/ramps/motion entries became 3-segment field-level entries (the same shape themes[] already used after round 2's fix) instead of 2-segment whole-entry blankets.

Checks (recorded on the task):
- Negative control, recorded not claimed: a committed snapshot of the pre-this-round design_pages.py (string-prefix matching), run against the real data file with all four of the reviewer's mutations applied, silently misses every one (exit 1).
- Unit+adapter: 67 tests (up from 60) - new: the four dotted-name-collision reproductions against both a minimal fixture and the real file; the stray-field test covering colours/aliases/opacities/ramps together; two property-style tests whose leaves now come from an enumeration written independently of the tool's own walker (a separate few-line recursion in the test file itself), per the review's explicit requirement that the test and the tool cannot share a blind spot.
- just check at the final head (a304d37), after fetching and merging origin/main three times in this round. The last merge brought in TASK-1.94 (PR 52), which merged to main while this task was being built: resolved a textual conflict on justfile's check: line (kept both check-design and check-ui-levels/check-ui-colours).
- CI green on PR 54 at head a304d37: gh pr checks 54 --watch, check passed in 4m18s.

TASK-1.94's shared steg_tools.foundations.read() (now merged) covers colours/opacity_names only (R120's colour-literal check is its whole AC#2 scope) - not type/sizes/themes/motion/ramps/colour_aliases/tint/focus_ring, which this task's Tier B, mapped checks and unaccounted_names all need as raw JSON. Importing it for colours alone would mean reading the file twice down two different paths for no real gain, so _read_data_file stays the one read of the whole document - documented in the module docstring as a decision made once TASK-1.94's actual shape was known, not an unfinished follow-up. Said plainly per the team lead's ask.

Tool's own output unchanged in its counts (ok, 18 colours, 19 type/size entries, 32 ANSI colours checked) - only the printed field-level breakdown for colour_aliases/opacities/ramps changed; full text in the PR description.

PR: https://github.com/bruk-io/steg/pull/54 (head a304d37, CI green).

Check FAILED: `sh -c 'cd tools && uv run --project . python "/private/tmp/claude-501/-Users-brukhabtu-src-github-com-bruk-io-steg/a318fbf0-a659-4ad6-9bfc-86a79ba9cc76/scratchpad/task-1.96-exclusion-list-negative-control.py" "/private/tmp/claude-501/-Users-brukhabtu-src-github-com-bruk-io-steg/a318fbf0-a659-4ad6-9bfc-86a79ba9cc76/scratchpad/task-1.96-pre-schema-design-pages.py" ../crates/ui/design/foundations.json'` exited 1 at 2026-11-16T04:54:28+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.96 on task-1.96-tools-check-the-design-s-data-file at a304d37220d9 with uncommitted changes:

```text
warning: `VIRTUAL_ENV=/Users/brukhabtu/.cache/uv/builds-v0/.tmpbPWTk7` does not match the project environment path `.venv` and will be ignored; use `--active` to target the active environment instead
baseline (unmodified real file) unaccounted: ()
focus_ring['ansi'] = {...}: OLD (exclusion-list) implementation reports: ()
BUG REPRODUCED: the OLD implementation silently missed focus_ring['ansi'] = {...}
colours[0]['unit'] = 'hex': OLD (exclusion-list) implementation reports: ()
BUG REPRODUCED: the OLD implementation silently missed colours[0]['unit'] = 'hex'
sizes['nav.side']['use'] = '...': OLD (exclusion-list) implementation reports: ()
BUG REPRODUCED: the OLD implementation silently missed sizes['nav.side']['use'] = '...'
colours[0]['artifact_version'] = 'bogus': OLD (exclusion-list) implementation reports: ()
BUG REPRODUCED: the OLD implementation silently missed colours[0]['artifact_version'] = 'bogus'
```

Commits since the review: 0.

Check passed: `sh -c 'cd tools && uv run --project . pytest tests/unit/test_design_pages.py tests/adapter/test_design_pages_real_workspace.py -q'` exited 0 at 2026-11-16T04:54:35+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.96 on task-1.96-tools-check-the-design-s-data-file at a304d37220d9 with uncommitted changes:

```text
warning: `VIRTUAL_ENV=/Users/brukhabtu/.cache/uv/builds-v0/.tmp4yXsql` does not match the project environment path `.venv` and will be ignored; use `--active` to target the active environment instead
........................................................................ [ 87%]
..........                                                               [100%]
82 passed in 0.31s
```

Commits since the review: 0.

Check passed: `just check` exited 0 at 2026-11-16T04:56:09+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.96 on task-1.96-tools-check-the-design-s-data-file at a304d37220d9 with uncommitted changes:

```text
ok      format check (0.2s)
ok      lint (0.6s)
ok      boundaries (0.1s)
ok      design pages (0.1s)
ok      licences (0.7s)
ok      tests (every crate) (7.6s)
ok      build (steg-sdk) (0.1s)
ok      test (steg-sdk) (0.4s)
ok      test (steg-sdk, local-socket) (0.5s)
ok      build (steg-kernel) (0.1s)
ok      test (steg-kernel) (0.4s)
ok      build (steg-server) (0.1s)
ok      test (steg-server) (6.5s)
ok      test (steg-server, race-fixture) (6.7s)
ok      build (steg) (0.1s)
ok      test (steg) (0.2s)
ok      test (steg, fixtures) (2.7s)
ok      build (steg-ui) (0.3s)
ok      test (steg-ui) (0.5s)
ok      build (steg-app) (0.3s)
ok      test (steg-app) (0.4s)
ok      tool tests (11.8s)
ok      architecture (1.4s)
ok      recipe names (0.1s)
ok      ui levels (0.1s)
ok      ui colours (0.1s)
```

Commits since the review: 0.

Check passed: `just check` exited 0 at 2026-11-16T04:57:19+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.96 on task-1.96-tools-check-the-design-s-data-file at c5b48cbcbed3:

```text
ok      format check (0.2s)
ok      lint (0.4s)
ok      boundaries (0.1s)
ok      design pages (0.1s)
ok      licences (0.8s)
ok      tests (every crate) (5.3s)
ok      build (steg-sdk) (0.1s)
ok      test (steg-sdk) (0.2s)
ok      test (steg-sdk, local-socket) (0.3s)
ok      build (steg-kernel) (0.1s)
ok      test (steg-kernel) (0.2s)
ok      build (steg-server) (0.1s)
ok      test (steg-server) (4.7s)
ok      test (steg-server, race-fixture) (6.1s)
ok      build (steg) (0.1s)
ok      test (steg) (0.1s)
ok      test (steg, fixtures) (2.4s)
ok      build (steg-ui) (0.3s)
ok      test (steg-ui) (0.3s)
ok      build (steg-app) (0.3s)
ok      test (steg-app) (0.3s)
ok      tool tests (11.7s)
ok      architecture (0.7s)
ok      recipe names (0.0s)
ok      ui levels (0.0s)
ok      ui colours (0.0s)
```

Commits since the review: 40, 39 without the Asdlc-Worker trailer.

Review finding (@bruk.r1, round 5, 1 Important, 1 nit) addressed: the six excluded field names (name, use, unit, ansi, artifact_version, artifact_url) were excluded at every level of the file, not only where each one's reason held - focus_ring["ansi"], colours[0]["unit"], a bare dict with only "name", and colours[0]["artifact_version"] all passed unseen. Same shape of defect found twice before (a field tuple, then a string-prefix rule); each time a patch to an open-ended walk. Stopped patching the walk.

Fixed: design_pages.py now has one declarative structure, SCHEMA, describing the whole document's shape position by position - Fields (a dict with exactly these keys, closed), NamedEntries (a list open to a new entry, closed in each entry's own fields), and three terminal kinds: Checked (a named checking function inspects it; recurses=True says that function's own walk already covers everything beneath, by design), DeclaredUnseen (out of reach, with its reason), Descriptive (name/use/unit/artifact_version/artifact_url, allowed only at the specific positions that actually have them). validate() walks the real data against SCHEMA and refuses anything at any position the schema doesn't name - no exclusion list, no prefix rule left to be over-broad. TIER_B_NAMES, RADIUS_MAP and the printed "not checked" list are all derived from SCHEMA (_derive_tier_b_names, _derive_radius_map, declared_unseen), never written down beside it, so the printed list and validate()'s refusals cannot disagree.

Nit fixed for free by the same change: a colours[] entry with no "name" (used to raise KeyError) and a colour field replaced by an object (used to raise AttributeError) are both now a named refusal from validate(), which runs before any Tier A/B check in main().

Checks (recorded on the task):
- Negative control, recorded not claimed: a scratch script running a verbatim copy of the pre-this-round (exclusion-list) implementation against the real data file with the reviewer's mutations applied (focus_ring["ansi"], colours[0]["unit"], sizes["nav.side"]["use"], colours[0]["artifact_version"]) silently misses all of them (exit 1) - confirming the bug before the fix. sizes["nav.side"]["use"] is the one case this task's schema also intentionally still passes (nav.side is a documented terminal Tier B position, the same already-accepted gap the review's own "everything else passed cleanly" section declined to re-litigate) - stated as such in a dedicated test and in crates/ui/design/README.md, not silently left unexplained.
- Unit+adapter: 82 tests (up from 67) - new: validate()'s own mechanics against small, hand-built Position trees; the real SCHEMA against a minimal but fully-compliant whole-document fixture, mutated at a dozen positions (a colour/alias/opacity/ramp field, inside caption_opacity, a type/sizes key, inside scrim, a theme field, inside ansi, a focus_ring field, tint) - each either refused or explicitly confirmed terminal-by-design; declared_unseen()/leaf_classification_counts() arithmetic; the same positions and all five rounds' reproductions against the real file.
- just check at the merged final head (c5b48cb), after fetching and merging origin/main - clean, no conflicts; TASK-1.93/1.97 still hadn't merged their own tools/pyproject.toml, justfile, or crates/ui/src/foundations/generated.rs entries.
- CI green on PR 54 at head c5b48cb: gh pr checks 54 --watch, check passed in 3m59s.

Tool's own output unchanged in its counts (ok, 18 colours, 19 type/size entries, 32 ANSI colours checked; 31 declared-unseen entries) - full text in the PR description.

PR: https://github.com/bruk-io/steg/pull/54 (head c5b48cb, CI green).
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

author: @bruk.r1
created: 2026-11-16T03:00:00Z

---

@bruk.r1 is reviewing TASK-1.96: a same-machine review: it waited 7m with no other engineer's reviewer, longer than review_fallback_after (0m), so a fresh reviewer on its author's machine takes it.

---

author: @bruk.r1
created: 2026-11-16T03:06:00Z

---

Review by @bruk.r1 (same-machine): changes requested. Back to In Progress for the author's worker.

Findings: 3 Important, 0 nit. Cites: review. The diff does not match the plan.

Reviewed PR #54 at its pushed head dcbeaf2 (matches the task's recorded `just check` and PR CI green, 4m20s). Verdict: changes requested.

[Compliance, Important] dcbeaf2 does not contain any of the engineer's decisions recorded in task comments #2 and #3 (made after plan approval): (1) the three pixel corners (sizes.radius.surface/track/inner) are still in TIER_C_SKIPS as declared-unseen (tools/src/steg_tools/design_pages.py:388-390), not checked through the reviewed name map against the merged "radius" row's 14/10/8 as decided; (2) colours.track is still declared unseen (design_pages.py:363-368; EXPECTED_SHAPES colours minimum is still 17, not 18) - design/canvas/project/Tokens.dc.html was never re-exported with the track row; (3) crates/ui/design/foundations.json's artifact_version is still 1794791991-5d11 (not 1794796789-1b13) and fill.quiet.use still reads "Key caps, progress track, a button under the pointer" (not the corrected "Key caps, a button under the pointer"); no README was touched. `git diff --stat origin/main...HEAD` confirms the diff touches only justfile, tools/pyproject.toml and tools/*: never design/canvas/project/Tokens.dc.html, crates/ui/design/foundations.json, or any README. The task's own implementation notes and the PR description describe the 17-of-18/old-radius-Tier-C state and never mention comments #2/#3 existing. This confirms the orchestrator's reading: the pushed head predates those decisions. These are unmet requirements the engineer recorded on the task, so AC#1 (track, corners) and the task's carried re-export are not met by this head.

[Bugs/Compliance, Important] Tier B's number check is field-blind (confirmed by a negative control I ran: swapping sizes.tab.width's usual=120 and min=72 in a scratch copy of foundations.json - a real field-meaning corruption, same number set - still exits 0 "ok"). The plan promises this caveat "stated plainly, in the plan and the README" (plan, Tier B section), but the diff touches no README anywhere, and the tool's own passing-run stdout never says this check is field-blind, only the module docstring does. A silent departure from a plan commitment.

[Compliance, Important] focus_ring.width (2) and focus_ring.offset (2) are neither checked (Tier A/B) nor listed in TIER_C_SKIPS, though both numbers are drawn in prose on Details.dc.html's "Focus ring" section ("A 2px ring ... 2px clear of the control"). I enumerated every other file key against TIER_B_NAMES/TIER_C_SKIPS and they match exactly (type: 7=7, sizes: 9+9=18 excluding these two), so this is a real gap, not an artifact of my enumeration. AC#3 requires everything the tool cannot see to be named so passing isn't read as more than it is; these two numbers fall through silently.

What I verified clean, with my own negative controls (via --data-file scratch copies and in-worktree page mutations, all reverted; worktree left clean):
- Tier A: one colour's dark hex changed, one light hex changed, one ANSI colour in one theme changed - each refused, naming the value, both forms, and the page.
- Tier B: one number in a size entry (nav.side.width) changed - refused.
- Reverse direction: one colour removed from a scratch copy - reported missing against the real pages; REVERSE_ALLOW_LIST's one entry ("radius") is explained in the module docstring and TIER_C_SKIPS (the page's merged radius row, four file keys, no 1:1 match).
- Parser: a renamed table header, and a large row-removal, both correctly reduce the row count and trigger the EXPECTED_SHAPES refusal rather than a silent pass; a colour name appearing only in the script block or in unlabelled prose (e.g. "track" used descriptively) is never read as a page row.
- Declared-unseen list (TIER_C_SKIPS) is printed on every passing run, 24 entries; its content otherwise matches the file's remaining keys with the two focus_ring exceptions above.
- Private reader (_read_data_file): marked for removal in its own docstring, swap to TASK-1.94's foundations.read() named as a follow-up in both the docstring and the PR body; TASK-1.94 had genuinely not merged to main at any point during this build (confirmed via the PR body and notes). It handles {"ref": ...} the same way TASK-1.94's hex_index would for this module's narrow usage (contributes no number in _numbers_in), though it does not resolve refs generally - not exercised today since no checked colour uses a ref.
- Wiring: check-design is in just check's dependency list, ungated on rust (correct, since the data file and pages are committed regardless); CI green on PR #54 at head dcbeaf2 (gh pr checks: pass, 4m20s); the recipe routes through {{quiet}} per plugins/bruk-io-steg/rules/tools.md.
- Size/approval: size-2, "Plan approved by @bruk (d2f87de12c3c, size-2)" present and digest matches the notes' "Built per the approved plan (digest d2f87de12c3c)".
- Repeated both of the plan's own required negative controls myself (changed value refused naming both forms; removed value reported missing) - both reproduced.

Unmet: AC#1 (track and the three corners, per the engineer's decisions), AC#3 (focus_ring.width/offset silently uncovered; the README caveat never added). AC#2's tests/negative-controls/CI wiring are all present and I reproduced them.

---

author: @bruk.r5
created: 2026-11-16T03:23:00Z

---

@bruk.r5 is reviewing TASK-1.96: a same-machine review: it waited 0m with no other engineer's reviewer, longer than review_fallback_after (0m), so a fresh reviewer on its author's machine takes it.

---

author: @bruk.r5
created: 2026-11-16T03:31:00Z

---

Review by @bruk.r5 (same-machine): changes requested. Back to In Progress for the author's worker.

Findings: 1 Important, 1 nit. Cites: review. The diff does not match the plan.

Reviewed PR #54 at its pushed head 8626dc6 (matches the task's recorded `just check` and PR CI green, 2m43s - confirmed again via `gh pr checks 54`). Verdict: changes requested.

This is the second review (after @bruk.r1's changes-requested at dcbeaf2). All three of that review's findings are now fixed and verified by my own reproduction:
- The engineer's decisions (comments #2/#3) are in the pushed head: design/canvas/project/Tokens.dc.html is byte-for-byte the saved page at artifact version 1794796789-1b13 (checksum 765c9e4e535e2af92d16d403ed9cd61751bc0c3b79e8b36adeacae85712bac4d confirmed both sides); the data file's diff against main is exactly artifact_version and fill.quiet.use, no colour changed; Layout.dc.html and Details.dc.html are unchanged (still an earlier artifact version, and both READMEs say so plainly); track has a row on Tokens.dc.html and is Tier A (18/18, EXPECTED_SHAPES minimum 17->18); both READMEs updated.
- The corner map: radius.surface/track/inner each check against the merged "radius" row via RADIUS_MAP/RADIUS_WORD_MAP. I changed each corner individually in a scratch copy of foundations.json: each refused, naming the value and the tied word. I swapped radius.surface/track's values: both refused (field-tied mismatch). radius.round stays in TIER_C_SKIPS with a reason; REVERSE_ALLOW_LIST is empty, "radius" is matched through RADIUS_MAP.values(), not a separate exemption.
- Field tightening: read FIELD_WORD_MAPS/RADIUS_WORD_MAP against the real page text for every Tier B sizes row (nav.side, nav.top, tab.width, pane.gap, palette, status.ring, radius) - each tightened field's word correctly follows its number in a single-number clause of the page's own sentence; every field left on the weak check has its word either preceding its number or sharing a clause with another number (e.g. nav.top's tab_height/track_inset, nav.side's row_height/tab_indent, pane.gap's grabbed), matching what the code and its comments claim. I reproduced the swap of tab.width's usual/min (refused), swapped radius.surface/track (refused), and swapped nav.side's two untightened fields row_height/tab_indent (passes, as documented - field-blind by design for fields the sentence doesn't let you tie cleanly). The tool's own passing stdout and crates/ui/design/README.md both state which fields are tightened and which stay weak, matching the plan's instruction. `_word_tied_number` returns None (falls back to the weak check, not a mismatch) when no single-number clause contains the word, so a reordered sentence degrades gracefully rather than producing a false refusal - checked by reasoning through every FIELD_WORD_MAPS/RADIUS_WORD_MAP entry against the real page text, no case where a correct file would be refused.
- Completeness (AC#3): I enumerated every leaf key of the real crates/ui/design/foundations.json by hand (colours: 18, colour_aliases: 3, opacities: 1, ramps: 2 + caption_opacity, tint, motion: 1, type: 7, sizes: 18, themes[0]: ansi/name structural + 5 fields, focus_ring: 4 fields + unit, session_colour) and compared against TIER_B_NAMES/RADIUS_MAP (checked) plus TIER_C_SKIPS (declared unseen): every key is accounted for today, including focus_ring.width/offset (the first review's finding #3), now in TIER_C_SKIPS along with focus_ring.colour, themes[].cursor and session_colour. Adding an unknown sizes key to a scratch copy (sizes.new_thing) is correctly refused by name.

New Important finding, found by my own negative control, not reasoning alone:

[Compliance, Important] tools/src/steg_tools/design_pages.py:518-525 (`unaccounted_names`) enumerates `themes[]` and `focus_ring` leaf keys from a fixed field tuple (`("bg","fg","bg_inactive","selection","cursor")` / `("width","offset","colour","corner_radius_addition")`), never from the data file's own keys the way `type`/`sizes`/`colour_aliases`/`opacities`/`ramps`/`motion` are (those walk `data.get(...)` directly). I added `focus_ring["glow"] = 3` and `themes[0]["underline"] = "#fff"` to a scratch copy of the real foundations.json and ran `steg-design-check --data-file <scratch>`: it printed "ok (18 colours, 19 type/size entries, 32 ANSI colours checked)" and exited 0, silently missing both new keys. This directly contradicts what is committed: crates/ui/design/README.md's new section says the test "enumerates every leaf key of this file and asserts each one is either checked or in that declared list, so a key added here later cannot fall through both silently"; the module's own docstring and the PR description make the same claim. It is also the same category of gap @bruk.r1's finding #3 on this same task already caught (focus_ring.width/offset silently uncovered) - that finding was fixed by naming the two specific keys in TIER_C_SKIPS, not by making the enumeration mechanism genuinely exhaustive for those two sections, so the next field added to either section falls through the same way. AC#3 requires everything the tool cannot see to be named so a pass is never read as more than it is; for themes[] and focus_ring, a future field is not named and not refused.

What I verified clean, with my own negative controls (via --data-file scratch copies and in-worktree page mutations, all reverted; worktree left clean):
- CI green on PR #54 at head 8626dc6 (gh pr checks: pass, 2m43s); `uv run --project tools pytest tests/unit/test_design_pages.py tests/adapter/test_design_pages_real_workspace.py -q`: 50 passed, at this same head.
- Parser regression: renaming "Used for" to "Used For" in a working-copy edit of the real Tokens.dc.html (reverted via git checkout) breaks both colour tables' header match and correctly triggers the EXPECTED_SHAPES refusal ("found 0 colours rows, expected at least 18") rather than a silent pass; the script-block-only colour (`off`) fixture is still excluded by construction (rendered_body never includes the script tag).
- Size/approval: size-2, "Plan approved by @bruk (d2f87de12c3c, size-2)" present and digest matches "Built per the approved plan (digest d2f87de12c3c)".
- Private reader (_read_data_file): TASK-1.94 (PR 52) is still "changes-requested"/In Progress on origin/main, so the shim is still correctly in place; marked for removal in its own docstring and the PR body.

Nit:

[Compliance, Nit] TASK-1.93's branch (PR 55, head d919cf4) generates crates/ui/src/foundations/generated.rs with both `artifact_version` baked into its header comment (generated.rs:2, "1794791991-5d11") and fill.quiet's stale "use" text baked into a per-field doc comment (generated.rs:211, "progress track"), both of which this task's data-file change supersedes (1794796789-1b13; "Key caps, a button under the pointer" - confirmed by reading generated.rs at that head). The task's own notes record only that TASK-1.93 "hadn't merged tools/pyproject.toml, justfile, or crates/ui/src/foundations/generated.rs" - a textual-conflict check - not that a semantic regeneration will be required once either branch lands second. No action is needed from this task: TASK-1.93's own `is_stale` check (tools/src/steg_tools/foundations_generate.py) will refuse and force a regeneration when it rebases onto main after this merges, and this task does not touch that generator or generated.rs. Flagging only so whoever merges either PR knows to expect that check to fire on the other.

Plan-matched: no, by the mechanical rule (the plan's "Module shape" file list names design_pages.py/pyproject.toml/justfile/tests - never Tokens.dc.html, foundations.json, or either README) - but the task's notes and task comments #2/#3 fully explain the departure (the engineer's decisions after plan approval), so per review policy this is not itself a finding.

Unmet: none of AC#1/AC#2's requirements - all reproduced. AC#3 is not fully met: the enumeration mechanism (unaccounted_names) does not live up to its own documented claim for themes[]/focus_ring, per the Important finding above.

---

author: @bruk.r5
created: 2026-11-16T03:55:00Z

---

@bruk.r5 is reviewing TASK-1.96: a same-machine review: it waited 0m with no other engineer's reviewer, longer than review_fallback_after (0m), so a fresh reviewer on its author's machine takes it.

---

author: @bruk.r5
created: 2026-11-16T04:06:00Z

---

Review by @bruk.r5 (same-machine): changes requested. Back to In Progress for the author's worker.

Findings: 2 Important, 0 nit. Cites: review, testing. The diff does not match the plan.

Third review (after @bruk.r1 at dcbeaf2, @bruk.r5 at 8626dc6). Reviewed PR #54 at its pushed head 320b9d7 (matches the task's recorded `just check` and `gh pr view`'s statusCheckRollup SUCCESS at headRefOid 320b9d785a969d04706f5ed66f8a6dc8fbe32b53; I also reran `just check` myself in the review worktree at this head: clean). Verdict: changes requested.

Round 2's finding (unaccounted_names enumerated themes[]/focus_ring from a fixed tuple) is genuinely fixed for that literal reproduction, and I reproduced the fix: a new top-level key, a new sizes key, a new theme field, a new focus_ring field, and a new colour_aliases entry are all correctly refused by unaccounted_names. The data file's diff against main is still exactly the version line and fill.quiet.use; Tokens.dc.html, the tab.width swap, and the radius-corner swap all still refuse correctly (I reran both swaps myself: tab.width.usual/min and radius.surface/radius.track, both refused naming the tied word). 60 unit+adapter tests pass. The tool's passing stdout matches the PR description byte for byte.

New Important finding, found by my own negative controls against the real data file through the actual CLI (`steg-design-check --data-file <scratch>`), not reasoning alone:

[Bugs/Compliance, Important] tools/src/steg_tools/design_pages.py:547-554 (`_is_accounted_for`) matches "dotted descendant" by plain string prefix, not by whole path segments - and the data file's own keys routinely contain a literal "." inside one segment (`font.ui`, `nav.side`, `radius.surface`, `ramp`/`ramp.extended`). A new sizes/type key, or a new list entry, whose name textually extends an already-registered name is swallowed as if it were a field nested inside that name, and reaches no check at all. Confirmed with four scratch `--data-file` runs against the real file, each exiting 0 "ok" with the new key entirely unverified and unmentioned:
- sizes["palette.footer"] = 10 (new sibling sizes key, textually extends registered "sizes.palette")
- type["terminal.bold"] = {...} (extends registered "type.terminal")
- sizes["nav.side.extra"] = 7 (extends registered "sizes.nav.side")
- ramps.append({"name": "ramp.secondary", "use": "...", "stops": [...]}) - a plausible future ramp, following the file's own existing naming (ramp, ramp.extended); extends registered "ramps.ramp" because `_entry_path`'s self-prefix check only strips the plural "ramps." (the list's own path), leaving the entry's own name free to collide with the sibling "ramp"/"ramp.extended" registrations
This directly falsifies the claim stated in the module docstring, crates/ui/design/README.md, and the PR description ("a genuinely new key - at any level that isn't already inside a checked whole value - cannot fall through both silently"): each of the four is a new key, not nested inside an already-covered whole value, yet falls through silently. My prefix-collision probes that lacked an internal dot (sizes["radius.surface2"], sizes["nav.sidekick"], sizes["palettex"]) were correctly refused - which is what made the bug look like whole-segment matching at first; it is specifically names containing their own "." that collide.
The property-style tests added this round (test_property_removing_any_declared_unseen_entry_flags_every_leaf_it_alone_covered / ..._checked_entry...) cannot catch this class of defect by construction: their universe of leaves is generated by `design_pages._leaf_paths(data, "")` itself, the same function that has the bug, so a leaf the walker absorbs under the wrong ancestor never enters the universe either test iterates over - and the adapter's `unaccounted_names(data) == ()` assertion is checking the registry against the walk, never the walk against the file. This is round 2's finding one level removed: a mechanism documented as exhaustive, tested only against its own output.

[Compliance, Important] A new field added to an existing colour entry is checked by nothing, in either direction: `colour_mismatches` (Tier A) reads only entry["dark"]/entry["light"], never the whole entry, and `unaccounted_names` treats any field under a registered colour name as an accounted-for descendant regardless. Confirmed: data["colours"][0]["contrast"] = 1.5 on a scratch copy of the real file -> exits 0 "ok", output otherwise identical to the real file's. `_is_accounted_for`'s own docstring and crates/ui/design/README.md justify absorbing a descendant leaf by "the checking function that name stands for already recurses the whole value (_numbers_in sees every number under it)" - true for Tier B, false for Tier A, and this specific gap is not disclosed anywhere: the README's one stated "accepted gap" is scoped to a non-numeric field inside an already-checked sizes group, and separately claims colours are checked "exactly" with no caveat. I also ran the one case this reasoning predicts is safe, to confirm rather than assume: a brand-new ansi colour added to a theme (themes[0]["ansi"]["bright_pink"] = "#ff00ff") is excluded from the leaf walk (_NOT_A_LEAF_PATH includes "ansi") but is independently caught by `ansi_mismatches`, which iterates theme["ansi"].items() directly from the data rather than a fixed name list - confirmed refused via main(), exit 1, naming "steg dark.bright_pink". So the ansi exclusion is sound; the colour-field gap is not.

What I verified clean, reproduced myself: the fixed themes[]/focus_ring enumeration (5 new-field cases above), the tab.width field-swap and radius-corner-swap negative controls, the real data file's diff against main (version + one description line), 60 tests passing, `just check` green at head 320b9d7, CI SUCCESS at the same head via `gh pr view --json statusCheckRollup`, the tool's passing stdout matching the PR body, and the new-ansi-key case (correctly refused, not a gap).

Plan-matched: no, same basis as round 2 (the plan's file list never named Tokens.dc.html/foundations.json/either README) - explained on the task by comments #2/#3, so not itself a finding, per policy.

Unmet: AC#3 again - the registry/walk pair still does not live up to its own documented exhaustiveness claim, now for a different and broader class of key (any dotted-name collision in sizes/type/ramps, not just themes[]/focus_ring) than round 2 found.

---

author: @bruk.r5
created: 2026-11-16T04:29:00Z

---

@bruk.r5 is reviewing TASK-1.96: a same-machine review: it waited 0m with no other engineer's reviewer, longer than review_fallback_after (0m), so a fresh reviewer on its author's machine takes it.

---

author: @bruk.r5
created: 2026-11-16T04:38:00Z

---

Review by @bruk.r5 (same-machine): changes requested. Back to In Progress for the author's worker.

Findings: 1 Important, 1 nit. Cites: review, testing. The diff does not match the plan.

Fourth review. Reviewed PR #54 at head a304d37 (matches the task's recorded `just check` and `gh pr view 54 --json headRefOid,statusCheckRollup`: SUCCESS at the same oid). Verdict: changes requested.

Rounds 1-3's findings are genuinely fixed, and I reproduced the fixes myself rather than trusting the notes:
- Round 1 (engineer's decisions, field-blindness, focus_ring.width/offset): all in the diff; track is Tier A (18/18); RADIUS_MAP checks the three corners; focus_ring.width/offset are in TIER_C_SKIPS.
- Round 2 (themes[]/focus_ring enumerated from a fixed tuple): unaccounted_names now walks the data generically via _leaf_paths; a new field in a theme or focus_ring is refused.
- Round 3 (string-prefix "accounted for", whole-entry colour ancestor): paths are tuples of segments, matched by whole-segment prefix (_is_accounted_for). I temporarily patched _is_accounted_for back to a joined-string-prefix compare (the pre-round-3 behaviour) and reran the dotted-name tests: the four unit-test and three adapter-test reproductions (sizes["palette.footer"], type["terminal.bold"], sizes["nav.side.extra"], ramp "ramp.secondary") all failed under the old matching and pass under the current code - reverted cleanly (git status clean after). A stray field on an existing colour/alias/opacity/ramp (contrast, weight, blur, intensity) is correctly refused, confirmed against the real file.

[Compliance, Important] tools/src/steg_tools/design_pages.py:530-540 (`_NOT_A_LEAF_PATH`) excludes six field names - name, use, unit, ansi, artifact_version, artifact_url - unconditionally, at every dict, everywhere in the file, not only where each one's stated justification actually holds. This falsifies crates/ui/design/README.md's own claim ("a genuinely new key - at any level, under any name, however it happens to spell - cannot fall through both silently") and the module docstring's identical claim, and it is the same shape of defect rounds 2 and 3 already found twice on this task (a mechanism documented as exhaustive that is not). Confirmed with real negative controls against the real data file, each silently passing `steg-design-check` with no mismatch, no "unaccounted", and no TIER_C_SKIPS entry printed:
- focus_ring["ansi"] = {"black": "#000"} - real hex content, reaching no checker at all. `ansi`'s exclusion comment says it is "checked structurally by ansi_mismatches", but ansi_mismatches only ever reads theme["ansi"]; it never looks at focus_ring, so this specific exclusion's justification is simply false outside themes[].
- colours[0]["unit"] = "hex" and sizes["nav.side"]["use"] = "..." - "unit"/"use" are meant to describe a number or an entry beside them; applied blindly, a same-named field that is not actually doing that is swallowed all the same.
- a bare dict (not a list-of-named-entries) whose only field is "name" - the exclusion's justification ("identifies the entry whose own name the path already carries") does not hold here, since this dict's name was never captured into the path, yet the field still vanishes.
- colours[0]["artifact_version"] = "bogus" - same pattern.

Worse, the property tests added this round to make the enumeration "independent" (test_design_pages.py / test_property_removing_any_...) cannot catch any of this: test_design_pages_real_workspace.py:278 defines `_INDEPENDENT_EXCLUDED_FIELDS` with the identical six names, hardcoded. The independence round 3 asked for (so a bug in the tool's own walker couldn't be masked by the test sharing it) is defeated for this specific class of bug, because the test's "independent" enumeration shares the exact over-broad exclusion set with the tool. This is round 3's own finding ("a mechanism documented as exhaustive, tested only against its own output") recurring one level over, in the one place the test suite was supposed to guard against it.

Everything else I probed from the review's own list passed cleanly, confirmed by running the real CLI (`steg-design-check --data-file <scratch>`) against the real file, not by reasoning:
- A new colour, colour_alias, opacity, ramp, theme, and a new ansi colour inside a theme: each refused (the new colour via Tier A's colour_mismatches, which iterates the file's own colours rather than a fixed list, not via unaccounted_names - correctly, since Tier A already covers every colour in the file).
- A new field (numeric, string, boolean, null, nested object, nested list) on a colour/alias/opacity/ramp entry: all six refused.
- A dotted-name collision at three depths, and a ramp whose name extends another ramp's name: all refused (round 3's fix holds).
- A key that is a registered name's first dot-piece only (sizes["nav"], vs. the real "sizes.nav.side"), and a no-dot suffix (sizes["palette2"]): both correctly refused - segment matching, not string matching.
- A new field inside an already-checked sizes group: numeric is silently absorbed (disclosed, both in the README and by its own pair of tests) and non-numeric is the same documented gap - not re-litigating this, it's accurately stated.
- A duplicate colour entry: a byte-identical duplicate is silently fine (no disagreement to report, consistent with AC#1's literal wording); a duplicate where one copy's value is wrong is correctly refused (both Tier A checks run independently per entry). Not a finding.

Nit:

[Bugs, Nit] Two inputs crash instead of refusing cleanly: a colours[] entry with no "name" key (KeyError in unaccounted_names/main), and an existing colour field's value replaced by an object (colours[0]["dark"] = {"hex": ..., "extra": ...} crashes colour_mismatches with AttributeError: 'dict' object has no attribute 'lower', since Tier A assumes dark/light are flat strings - unlike the Tier B names this reasoning is modelled on, Tier A does not recurse the value, so the registry's own "a whole-segment prefix is accounted for because the checking function recurses the whole value" justification is false for colour fields specifically, though no real export ever produces this shape today). Neither is a silent pass, and neither falsifies AC#1-3's claims about names - both are a hard crash on malformed shape, not a name/value mismatch. Low priority: the real artifact export never produces either shape.

Evidence: `gh pr view 54 --json body` matches the tool's own captured stdout byte-for-byte (the "ok (18 colours, 19 type/size entries, 32 ANSI colours checked)" line, the field-blind caveat, and all 31 TIER_C_SKIPS lines). `just check-design` run fresh in the review worktree at a304d37: ok. 67 unit+adapter tests pass. TASK-1.94's own files (foundations.py/ui_colours.py/ui_levels.py) have an empty diff against main in this branch; justfile's check: line keeps both check-design and check-ui-levels/check-ui-colours after the merge. foundations.py's read() reads only colours + opacity_names (confirmed by reading it) - the worker's claim that the shared reader doesn't cover what this task needs is true; keeping the private _read_data_file is the right call here, and extending foundations.py to the whole document is a real follow-up, not this task's.

Plan-matched: no, same basis as rounds 2-3 (the plan's file list never named Tokens.dc.html/foundations.json/either README) - explained by task comments #2/#3, not itself a finding per policy.

Unmet: AC#3 again, for a new reason each round - this round, the exclusion list itself is unconditional where its own stated reasons are not.

---

author: @bruk.r5
created: 2026-11-16T05:02:00Z

---

@bruk.r5 is reviewing TASK-1.96: a same-machine review: it waited 0m with no other engineer's reviewer, longer than review_fallback_after (0m), so a fresh reviewer on its author's machine takes it.

---
<!-- COMMENTS:END -->
