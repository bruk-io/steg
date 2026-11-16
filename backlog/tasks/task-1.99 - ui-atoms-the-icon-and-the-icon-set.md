---
id: TASK-1.99
title: 'ui atoms: the icon, and the icon set'
status: In Progress
assignee:
  - '@bruk.w7'
created_date: '2026-11-16T02:45:00Z'
updated_date: '2026-11-16T13:51:00Z'
labels:
  - size-3
milestone: m-4
dependencies:
  - TASK-1.93
  - TASK-1.118
parent_task_id: TASK-1
project: bruk-io-steg
asdlc_accepted_from: DRAFT-2
asdlc_branch: task-1.99-ui-atoms-the-icon-and-the-icon-set
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Design pages: Atoms (Icon), Atoms - icons. doc-17 (the four levels, R117 to R125), decision-10 (the design is the artifact; a worker cannot open it, so the orchestrator supplies the saved page named below), decision-11 (GPUI directly). Read GPUI's README first, then its examples (https://github.com/zed-industries/zed/tree/main/crates/gpui/examples), checked against the pinned gpui 0.2.2 source in the cargo registry. Pure interface: no steg domain type, no I/O (R118). Depends on the generated foundations (TASK-1.93).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 An Icon atom draws each of the 19 icons of the design's Icons board at 16px by name, in the colour it is given, with the faint second tone where the design uses one; an unknown name cannot be asked for (the names are a type)
- [ ] #2 Every state and variant the design's card draws is shown in the gallery, in light and in dark, each labelled as the card labels it; the pull request carries the gallery images (and a recording where the component moves or reacts), and the notes list the card's states against what the gallery shows, one by one
- [ ] #3 It is given plain values and reports what the user did (R118); every colour, size, corner and duration comes from foundations through the theme (R120, R123); the level and colour checks pass with no exemption
- [ ] #4 The plan says how the icons' shapes reach the code from the design's saved page without being redrawn by hand or changed, and how a reviewer checks one against the design
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
## For the engineer

Recommend splitting this task before building (default-to-splitting-tasks; the seam is clean and matches the TASK-1.91 -> TASK-1.92 -> TASK-1.93 precedent already on this board):

- **A — "ui: the icon set's shapes, generated from the design"** (size-2). `design/canvas/project/{IconSet,Atoms}.dc.html` committed verbatim, `design/canvas/README.md` updated, the extraction tool, `crates/ui/src/atoms/icon/generated.rs` (the 19-name enum + raw shape markup, no drawable atom yet - same shape as TASK-1.93 landing `foundations/generated.rs` before any atom consumed it), `generate-icons`/`check-icons` recipes, and the new `sizes.icon` foundations value. No UI change, no PR image needed. AC: the enum compiles and is collision-checked; `check-icons` fails on any drift from the saved page; `just check` (levels, colours, foundations, design) passes unexempted.
- **B — this task's own remaining scope, "ui atoms: the icon"** (size-2, depends on A). The `Icon` element, the custom `AssetSource`, the gallery's Icons section, the tests that need the real atom, the PR images.

If you'd rather keep one PR, say so and I build both halves in this task as planned below in that order (first commit = A's scope, second = B's) - nothing below changes either way, only whether it lands as one task or two.

Three decisions that are yours, each with my recommendation and reasoning below (section 4, 5 and 6): (1) `size`'s three values are genuinely all load-bearing, not just a "16px, default" story - recommend an `IconSize` enum with `Sixteen` as `Default`, matching AC#1's "16px" to the default rather than the only value. (2) the faint second tone stays inert shape data, never a new foundations value - `foundations.json` and `check-design`'s schema are untouched by this task. (3) `gpui`'s `test-support` feature (real headless rendering via `TestAppContext`) is deferred, not taken on now - it needs a `boundaries.toml` feature entry and a `check-packages`-isolation proof neither exists today; proof here is plain `#[cfg(test)]` units plus the generator's own staleness gate plus the gallery capture. Revisit the first time an atom's own interactive behaviour (not just its shape) needs proving.

## Criterion 4: how the icons' shapes reach the code

Verified directly against the saved pages, not assumed:

- Each of the 19 icons appears on `IconSet.dc.html` exactly three times per card (once at 48px, twice at 16px in context), but only the 48px copy carries `role="img" aria-label="<Name>"` - `grep -c 'role="img"'` is exactly 19, and every `aria-label` matches the bold title in the same tile, byte for byte (checked, not assumed; the two extra bold titles, "steg" and "zsh", belong to the "In place" usage mock and carry no `role="img"`, so this one predicate - `svg[role="img"][aria-label]` - is the whole extraction key, with no need to also exclude the "In place" card by number or position). "Status is not an icon" (the page's own words): the status-ring SVGs in that card use `{{k1}}`-style template placeholders from the page's animation script and have no `role="img"`, so they fall outside the key on their own.
- GPUI 0.2.2's `svg()` element (`src/elements/svg.rs`) takes only `.path(impl Into<SharedString>)`, resolved through `AssetSource::load(path) -> Result<Option<Cow<'static, [u8]>>>` (`src/assets.rs`) - there is no bytes-literal constructor on `Svg` itself. `SvgRenderer::render` (`src/svg_renderer.rs`) parses the loaded bytes as a real document with `usvg::Tree::from_data`, rasterises with `resvg::render`, and converts the result to a **pixel alpha mask only** (`p.alpha()`); `paint_svg` then tints that one mask with a single `color` argument. Two consequences, both load-bearing: (a) a path drawn with `fill-opacity`/`opacity` in the source SVG survives as a partial-alpha region in the mask, so the design's own two-tone icons ("the part that matters is solid and the rest is the same colour at 35%") reproduce correctly with the atom supplying exactly **one** colour - no second colour parameter needed or possible; (b) the element needs real bytes behind a path string, which this crate supplies itself, in memory, via a small `AssetSource` impl over `include_str!`-style constants - no file I/O, no new dependency, R118's "no I/O" holds.
- I checked whether GPUI 0.2.2 can instead paint these shapes by hand (`PathBuilder`, `src/path_builder.rs`) rather than through an asset: it can - `PathBuilder::stroke`/`fill`, `line_to`, `curve_to` (quadratic), `cubic_bezier_to`, `arc_to` are all real, backed by `lyon`'s tessellator, and do support the stroke widths these icons use. I'm not recommending it anyway: hand-painting - even generated, never typed by a person - needs this crate to parse and reinterpret every icon's SVG path syntax (M/L/C/A commands, arc flags, implicit relative coordinates) into `PathBuilder` calls, which is new, untested parsing machinery with its own chance of silently drawing the wrong shape. The `svg()` + `AssetSource` route needs none of that: the extracted markup is the exact bytes from the page, interpreted by `usvg` (already one of GPUI's own transitive dependencies - no new licence surface), so there is nothing of ours between the design's own path data and what's drawn. This is what AC#4's "without being redrawn by hand or changed" asks for literally, not just in spirit.
- The extraction tool (`tools/src/steg_tools/icons_generate.py`, the same pure-core/thin-shell/`--check` shape as `foundations_generate.py`) reads `design/canvas/project/IconSet.dc.html`, finds the 19 `role="img"` elements, and for each keeps only `viewBox` (constant `"0 0 16 16"` across all 19, confirmed) and its child `<path>`/`<rect>`/`<circle>` elements verbatim - dropping `width`, `height`, `role`, `aria-label` and the outer tag's own `style="...color: #ecebee"` (page-chrome for the reference illustration, not part of the icon's own shape: an icon has no colour of its own, it's coloured by whoever draws it, which is exactly the point of taking colour through the theme). This is a correctness rule I'm stating explicitly as a rule of the tool (not an accident of the implementation) with its own test: generated markup contains no `style=`/`color:` attribute, for every one of the 19. (I checked whether `check-ui-colours` would catch a leftover `color:#ecebee` on its own: its hex-string regex is a full-match anchor against the *whole* string literal's content, not a substring search, so a long SVG-markup string with a hex colour embedded partway through it would not trip that check - this is a gap this task's own test closes, not something `just check` already covers for free.)
- The extracted markup's own `fill`/`stroke="currentColor"` is rewritten to a fixed opaque literal (`#000`) rather than kept as `currentColor`: GPUI discards every channel but alpha (confirmed above - `p.alpha()` only), so the actual colour value painted into the standalone per-icon SVG is never seen and doesn't matter, but `currentColor` depends on CSS inheritance from a `color` property this standalone document no longer has (the design page's own wrapper `<div style="...color:...">` is dropped along with everything else page-chrome) - using a fixed literal sidesteps needing to confirm how `usvg` resolves an unset `currentColor`, rather than relying on undocumented fallback behaviour.
- Output: `crates/ui/src/atoms/icon/generated.rs` (a `mod generated;`/`pub(crate) use generated::*;` pair inside `atoms/icon/mod.rs`, exactly mirroring `foundations/mod.rs`+`generated.rs`): the `IconName` enum (19 variants, Rust identifiers derived from the design's own labels with the same `identifier()`/collision-check machinery `foundations_generate.py` already has, reused or duplicated - see "risks" below) and a `pub(crate) fn markup(name: IconName) -> &'static str` match. `check-icons` (part of `just check`) re-extracts from the committed page and fails byte-for-byte on drift, exactly like `check-foundations`.
- How a reviewer checks one icon without rendering anything: the generated file carries, right beside each icon's markup, a `/// Design name: "X"` marker in the same convention `foundations_generate.py`'s own `_marker()` uses, and a test enumerates `design_entries`-style over the 19 names so a reviewer can diff one name's Rust constant against that same name's tile on the (committed, plain-text, unrendered) saved page directly. Beyond that: the gallery (criterion 2) draws every icon at real size, labelled by name, in both themes, for the PR image a reviewer compares by eye against the page's own drawn tiles.

## Names as a type

19 variants (verified against the page, listed in file order): `New`, `Close`, `Closed`, `Open`, `BarOnTheSide` ("Bar on the side"), `BarOnTop` ("Bar on top"), `CommandPalette`, `Search`, `SplitRight`, `SplitDown`, `FillTheTab` ("Fill the tab"), `Restore`, `Session`, `Tab`, `Pane`, `Plugin`, `Settings`, `ThisMachine` ("This machine"), `AnotherMachine` ("Another machine") - no two labels collide under the identifier transform (checked by hand; the generator still runs a real collision check, not just trusts this). `IconName` is a plain `#[derive(Debug, Clone, Copy, PartialEq, Eq)] pub enum`, matched without a `_` arm wherever it's consumed (rust.md), so a 20th icon added to the design fails the build everywhere it isn't handled yet, rather than silently rendering nothing.

## The atom's interface

From `Atoms.dc.html`'s Icon card (Prop table, quoted exactly):

| Prop | Values | Default |
|---|---|---|
| `name` | Any icon on the Icons board | required |
| `size` | 12, 14, 16 | 16 |
| `tone` | ink, secondary, muted | secondary |

Rule on the same card: "12px is only for the chevrons in the side list; 14px only for close" - a usage guideline for callers, not something this atom can enforce without knowing which caller it is (out of scope for a pure-interface atom; stated in the doc comment, not mechanically gated).

- `name: IconName` - required, no default; this is what makes AC#1's "an unknown name cannot be asked for" literal.
- `size: IconSize` (`Twelve | Fourteen | Sixteen`, `Default` = `Sixteen`, matching AC#1's "16px" as the default rather than the only size - the Stories row on the card draws all three, labelled `16px`/`14px`/`12px`, so AC#2 needs all three in the gallery regardless). Resolves to a *new* named foundations value, `sizes.icon` (`{"steps": [12, 14, 16], "unit": "px"}`, same shape as the existing `sizes.space.steps` - `foundations_generate.py`'s existing generic list-of-numbers path renders it with no new generator code), because `size` is a declared prop on the atom's own public interface, the same category R120 already requires named for `tone`. I'm proposing the key `icon.size` (dotted, matching this file's own convention for compound names - `radius.surface`, `pane.gap`) rather than bare `icon`, so the generated constant is `ICON_SIZE`, not `ICON` (which would read oddly next to `atoms::icon`, a different thing). `impl Default for IconSize` picks `Sixteen`.
- `tone: IconTone` (`Ink | Secondary | Muted`, `Default` = `Secondary`). The card's `ink`/`secondary`/`muted` are, verified against the page's own inline theme variables, exactly `{{c.ink}}`/`{{c.ink2}}`/`{{c.muted}}` - already-generated `Theme` methods `text()`/`text_secondary()`/`text_muted()` (`crates/ui/src/foundations/mod.rs`, confirmed against its own `colour_entries`). No new foundations value needed. The mapping from `IconTone` to one of those three methods is a **private** function inside `atoms/icon/mod.rs` (not `pub`), e.g. `fn resolve_tone(theme: &Theme, tone: IconTone) -> Hsla`: `check-foundations-public-surface` only matches `pub fn ... -> Hsla`, by its own stated design (R123 is about the crate's *public* surface) - a private helper is invisible to it and needs no exemption, and keeping it private (rust.md: start with no `pub`) also avoids growing `Theme`'s own public surface with an icon-specific concept that foundations has no reason to know about; `tone` stays entirely the Icon atom's own business, resolved through `Theme`'s *existing* methods, never introducing or returning a bare colour from this atom's own public interface.
- No "faint"/"second tone" prop at all - the Icon card's own Prop table lists only `name`/`size`/`tone`; the two-tone effect belongs to *which icon* was asked for (10 of the 19: `BarOnTheSide`, `BarOnTop`, `SplitRight`, `SplitDown`, `Session`, `Tab`, `Pane`, `Settings`, `ThisMachine`, `AnotherMachine`, each verified by hand against the page's own `opacity="0.35"` attributes), not a choice the caller makes. The 35% value itself stays inert extracted shape data inside the generated markup, not a new foundations value: it's not a prop, it carries no light/dark pair (the page states one flat number for both), and there is no existing mechanism (`check-design`'s `SCHEMA`) that reaches `IconSet.dc.html` at all - adding a foundations entry for it would create a second, unchecked source of truth for the same number, when the icon extraction's own byte-for-byte fidelity test against the saved page already proves it's faithful to the design. Stating this plainly so a reviewer doesn't wonder why `foundations.json`/`design_pages.py` are untouched by this task.
- Nothing reported out (R118, AC#3): the Icon atom takes `IconName`/`IconSize`/`IconTone` (plain values, none of them a steg domain type) and returns `impl IntoElement`; it has no event handler, no `FocusHandle`, nothing a caller observes beyond the drawn pixels. Doc-17's own level table lists Icon purely as a drawing atom with "no say in where it is placed," consistent with the design: nothing on the Icon card shows a hover/pressed/disabled state (compare the Button card, not this one, once TASK-1.101 gets to it).
- Accessibility (R125/AC#3's "what role and name... if 0.2.2 can express either"): checked directly, not assumed - **GPUI 0.2.2 has no accessibility API of any kind.** The examples list (`ls examples/`) has no `a11y.rs`, `focus_visible.rs`, or `testing.rs` (doc-17's own references to them are from Zed's current main branch, which this pin predates - doc-17 already warns of exactly this). A grep of `src/` for `accessib`/`Accessib`/`aria`/`AccessKit` found only prose false positives (`variant`, `invariant`, "no longer accessible"); `Cargo.toml` has no `accesskit` dependency at any version. So: there is no role, no accessible name, and no "hidden from assistive technology" flag to set either way at this pin - not a choice this task is declining, a capability that doesn't exist yet. Recording this for doc-17's own "Areas of concern" practice ("what cannot be done yet is recorded then, not assumed now") and for TASK-1.95: *the first interactive atom* (not this one - Icon has nothing to act on) is still where R125 gets properly established; what this task adds is the confirmation that 0.2.2 brings nothing for it regardless of which atom goes first, so TASK-1.95's rule for atoms shouldn't promise a role/name mechanism that doesn't exist, and should instead say plainly that a decorative icon is just chrome owned by its interactive parent (the eventual icon-button/tab/etc.), which is also the Icon card's own framing (it's never drawn alone in the "In place" story - always inside a button, a tab, a row).

## Where it lives, and the pattern for the next dozen atoms

- `crates/ui/src/atoms/icon/mod.rs` (the atom: `IconName`, `IconSize`, `IconTone`, the public `icon(name) -> Icon` builder with `.size()`/`.tone()`, the `AssetSource` impl, the private tone resolver) + `crates/ui/src/atoms/icon/generated.rs` (generated-only, never hand-edited, same split as `foundations/{mod,generated}.rs`). `atoms/mod.rs` gains `pub mod icon;`.
- A concrete, checked rule for cross-level references, since this is the first atom file and the next dozen copy it: **reach foundations with an absolute path, `crate::foundations::...`, never `super::`.** Verified against `ui_levels.py`'s own source: `resolve_path` fully supports `crate::<level>::...` at any nesting depth, but a `super::` path only resolves reliably one hop below a level's own `mod.rs` (which is all `foundations/generated.rs` ever needs, reaching its *own* level's sibling items, not a different level) - a second `super::` hop is named in the check's own docstring as unresolved (returns `None`, i.e. silently unchecked, not refused) until "a level first grows its own submodule (TASK-1.95 or later)." **This task is that moment**, the moment one level (`atoms`) first gets a submodule (`atoms/icon/`) that needs something from an earlier level (`foundations`) - so `crate::foundations::...` throughout, confirmed to resolve correctly today, with no change to `ui_levels.py` needed. This is the concrete thing I'd feed to TASK-1.95: state this as the level rule for atoms/molecules/organisms outright, rather than leaving every later nested module to rediscover it.
- A component with no generated data (most future atoms) stays a flat `atoms/<name>.rs`; one that needs extracted/generated shape data (the next icon-shaped situation, if any) gets the same `atoms/<name>/{mod.rs, generated.rs}` split this task establishes.
- `AssetSource`: a small `pub(crate) struct IconAssets;` whose `load`/`list` match against the generated `markup()` function, returning `Ok(Some(Cow::Borrowed(bytes)))` - no filesystem, no I/O, just static data already compiled into the binary, so R117/R118 hold. `with_assets(...)` is set once on `Application`, not per element - `crates/ui/gallery/main.rs`'s `fn main()` gains `.with_assets(steg_ui::atoms::icon::IconAssets)` (or similar; exact visibility TBD at build time), and the eventual `steg-app` crate will need the same call once it exists (noted for later, not built here). What happens if a consumer forgets: `paint_svg(...).log_err()` swallows the error and draws nothing, silently - worth a one-line note in the atom's own doc comment (`panic`s are for bugs, not this; a missing `AssetSource` is a wiring mistake a person makes once setting up a binary, not a recoverable runtime state a caller's values can trigger) rather than a louder failure this crate has no way to raise anyway (no I/O, nothing to return an error through - the atom's own build function returns `impl IntoElement`, unconditionally).

## Gallery

New "Icons" section, in its own module (`crates/ui/gallery/icons.rs`, a sibling of `main.rs`, returning one `impl IntoElement` the existing `Render` impl adds as a `.child(...)` - the same "a dozen more components, not one long file" split the orchestrator asked for, applied to the gallery itself this time, since `main.rs` is already ~550 lines before this task touches it).

Every state/variant the Icon card draws (quoted from the Stories row, checked against the page directly), each labelled exactly as the card labels it:

| Card's label | What's shown |
|---|---|
| "16px, ink" | one icon, 16px, `tone: Ink` |
| "16px, secondary" | same icon, 16px, `tone: Secondary` |
| "16px, muted" | a different icon (the card's own choice - Search), 16px, `tone: Muted` |
| "14px" | Close, 14px |
| "12px" | the chevron (Closed), 12px |

Plus, beyond the card's own five stories (needed for AC#1's full icon-set coverage and the "Icons board" it points to): all 19 icons, in a grid labelled by name, drawn at 16px/secondary as the common case - this is the gallery's own addition (same pattern as the existing "Colours"/"Terminal themes" sections showing *everything* a reader could want to check, not just the card's five examples), captured in both `STEG_GALLERY_THEME` runs for light and dark, same as every other section already in `main.rs`.

`just capture-gallery`'s pixel probe (`steg-gallery-check ... 8 60 r g b`) checks the window's own background colour near the top padding, independent of how much content is below it - read the recipe directly to confirm (`justfile`): adding a new section doesn't move or risk that probe.

PR notes list each of the five card states against what the gallery shows, one by one (AC#2's own discipline), plus the 19-icon grid, with the light and dark captures attached as images (ui-prs-need-an-image).

## Tests, each with its stated negative control

All of these are plain `#[cfg(test)]` units - no window, no `test-support` feature (see "For the engineer" #3) - plus the two generator `--check` gates, which is what "nothing in `just check` or CI opens a window" requires and what the crate's own `CLAUDE.md`/testing-rust.md already establish as the pattern here.

| Test | Negative control |
|---|---|
| Extraction finds exactly 19 `role="img"` entries, each name matching its tile's bold title | Delete one tile from a copy of the saved page -> count becomes 18, test fails |
| `IconName`'s 19 identifiers have no collision | Edit two labels on a copy of the page to collide -> the collision check fires |
| Generated markup for each name equals a hand-checked expected string, byte for byte | Flip one byte in the committed `generated.rs` (or re-run the generator against a hand-edited copy of the page) -> `check-icons`/the equality test fails |
| Generated markup carries no `style=`/`color:` attribute, for all 19 | Deliberately leave one in during development -> the test catches what `check-ui-colours`'s anchored-regex would not |
| `IconSize`/`IconTone` each resolve through `ICON_SIZE`/`text()`-family `Theme` methods correctly, including their `Default` | Point `IconTone::Muted`'s resolver arm at `text_secondary()` instead of `text_muted()` -> the test (which constructs a `Theme` directly, same pattern as `foundations/mod.rs`'s own tests, no window needed) fails |
| `check-icons` fails when `generated.rs` doesn't match the saved page | Same mechanism as `check-foundations`'s own `is_stale`; reuse that test shape directly |
| `IconAssets::load` returns the right bytes for a known path, `None` for an unknown one | Ask for a path with a typo -> `None`, not a panic |

Each test above is run and recorded against the code as it stands at the end of the build phase, not assumed from this plan.

## What the level and colour checks need

- `check-ui-levels`: passes with no exemption once every cross-level reference in `atoms/icon/**` is written `crate::foundations::...` (see "Where it lives," above) - confirmed by reading `resolve_path` directly, not assumed.
- `check-ui-colours`: passes with no exemption. The one real risk - a stray hex colour surviving inside extracted SVG markup - isn't actually caught by this check's own anchored hex-string regex (a substring inside a longer string literal doesn't match a full-string anchor), which is exactly why this task adds its own dedicated test for it rather than relying on `just check` to catch it for free. Stating this precisely so a reviewer doesn't assume `check-ui-colours` is a backstop here that it isn't.
- `check-foundations`/`check-design`: `check-foundations` re-runs once `sizes.icon` lands in `foundations.json` (mechanical, no new generator code - the existing list-of-numbers path in `foundations_generate.py` already handles it). `check-design` is untouched - it only ever reaches `Tokens.dc.html`/`Details.dc.html`, and this task adds no field to the three sections (`colours`/`type`/`sizes`-minus-icon) that tool already schemas; `sizes.icon` is a *new* key under `sizes`, so `design_pages.py`'s `SCHEMA` needs a position added for it, named "declared unseen" like most of `sizes` already is (the tool's own three-way classification, not a new category) - a one-line addition to `SCHEMA`, confirmed against `design/README.md`'s own description of that classification, not an unexamined gap.
- A brand-new `check-icons` recipe and tool, parallel in every way to `check-foundations`/`steg-generate-foundations` (`generate-icons`/`check-icons` as verbs - both already in `ALLOWED_VERBS`, so `check-recipes` needs no change, confirmed by reading it directly).

## Files that change

- `design/canvas/project/IconSet.dc.html`, `design/canvas/project/Atoms.dc.html` - new, copied byte for byte from the saved files, version `1794796789-1b13` recorded in `design/canvas/README.md` the way it already records `Tokens.dc.html`'s.
- `design/canvas/README.md` - "What is here" gains these two pages' provenance.
- `crates/ui/design/foundations.json` - one new entry, `sizes.icon` (`{"steps": [12, 14, 16], "unit": "px"}`).
- `crates/ui/src/foundations/generated.rs` - regenerated (`just generate-foundations`), gains `ICON_SIZE`/`ICON_SIZE_DESCRIPTION`.
- `crates/ui/src/atoms/mod.rs` - gains `pub mod icon;`.
- `crates/ui/src/atoms/icon/mod.rs`, `crates/ui/src/atoms/icon/generated.rs` - new.
- `crates/ui/gallery/main.rs` - gains the Icons section's wiring and `.with_assets(...)`.
- `crates/ui/gallery/icons.rs` - new.
- `tools/src/steg_tools/icons_generate.py` - new, parallel to `foundations_generate.py`.
- `tools/tests/unit/test_icons_generate.py`, `tools/tests/adapter/test_icons_generate_real_workspace.py` - new, parallel to the `design_pages`/`foundations_generate` test pairs.
- `tools/src/steg_tools/design_pages.py` - one `SCHEMA` position added for `sizes.icon` ("declared unseen").
- `justfile` - `generate-icons`, `check-icons` recipes; `check:` gains `check-icons`.

## Order of work

1. `foundations.json`'s `sizes.icon` entry; `just generate-foundations`; confirm `check-foundations` passes and the new `ICON_SIZE` constant looks right.
2. `design/canvas/project/{IconSet,Atoms}.dc.html` committed verbatim; `design/canvas/README.md` updated; `design_pages.py`'s `SCHEMA` gains the one new position; confirm `check-design` still passes.
3. `tools/src/steg_tools/icons_generate.py` + its unit/adapter tests; `crates/ui/src/atoms/icon/generated.rs` generated and committed; `check-icons` wired into the justfile and `just check`.
4. `crates/ui/src/atoms/icon/mod.rs`: `IconName`/`IconSize`/`IconTone`, the private tone resolver, `IconAssets`, the public builder; its own unit tests.
5. `crates/ui/gallery/icons.rs` + `main.rs` wiring (`.with_assets`, the new section, the 19-icon grid); `just run-gallery` by hand to look at it; `just capture-gallery` for both themes.
6. `just check` (full) and `just test`; PR with both images and the AC#2 state-by-state notes.

## Risks

- The `identifier()`/collision-check logic either gets duplicated into `icons_generate.py` or factored out of `foundations_generate.py` into something both import - duplicating a ~15-line pure function is probably fine for this task's size; flagging it rather than silently picking one, since `steg_tools.foundations`'s own docstring (read while touring `foundations_generate.py`) already shows this codebase cares about *not* duplicating readers once two modules need the same one.
- `design_pages.py`'s `SCHEMA` addition for `sizes.icon` is a small, mechanical, well-precedented change, but it's still a change to a tool this task didn't otherwise need to touch - worth a reviewer's extra look given that file's own history of three failed narrower mechanisms before `SCHEMA` replaced them (`design/README.md`'s own account).
- `IconAssets`/`with_assets` wiring in the gallery is this crate's first contact with GPUI's asset system at all - needs confirming empirically in the build phase by actually running the gallery once (`just run-gallery`), not just reading source, in case `usvg`/`resvg` reject something about the generated markup's shape (an unexpected attribute, a missing root `xmlns`) that a read of the source alone can't rule out.
- The split proposal (section "For the engineer") is mine to raise, not mine to decide; if the engineer keeps one task, the order of work above is unchanged, only the task boundary.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Plan (7845702ae4ea) checked against doc-17 (R117 to R125), decision-10, decision-11 and the four acceptance criteria by @bruk.o, 2026-11-16: sound in design; not yet ok for approval. (1) The split is taken: the shapes and names generated from the page are drafted as their own task; this task keeps the Icon atom and the gallery. (2) Not ok as written: the plan adds sizes.icon to foundations.json by hand. R121 says every named value comes from the design artifact and the data file is exported from it, never edited by hand. Either the design's Tokens page gains an icon size row (the orchestrator edits the artifact and exports, as on 2026-11-15), or the sizes stay on the Icon card of the Atoms page and are generated from there. For the engineer. (3) Prefer the page's markup unchanged: the plan rewrites currentColor to a literal black; keep currentColor if a run shows the pinned renderer draws it, so the tool changes nothing and no colour literal appears. (4) For the engineer to know: GPUI 0.2.2 has no accessibility API at all, so R125 (role, name and state for assistive technology) cannot be met at this pin by any component.
<!-- SECTION:NOTES:END -->
