# `foundations.json`

Exported from the design artifact (decision-10), under the design's own names.
Never edited by hand: a value changes by changing the design and exporting again
(R121). `artifact_version` records which version of the artifact this was exported
from; `design/canvas/README.md` (repository root) is the durable home for the
working data and generator scripts the three foundations pages were drawn from, and
the saved copies of those pages this file is checked against.

## How a reference is written, everywhere

A field whose value points at another named entry in this file — a colour, a size,
the ramp-derived `tint` — is always the wrapped object `{"ref": "<name>"}`, never a
bare string. This holds regardless of what the field itself is called: `track_colour`,
`radius_outer`, `fill.colour`, `bg_inactive`, the `ref` field on each of
`colour_aliases`, and `colour` inside `ramps[].stops[]` and `scrim.light` are all
written this way. A generator needs exactly one case to recognise a reference: a
JSON object with a single key, `ref`, whose value is a name found somewhere else in
this file (a `colours[].name`, a `sizes` key, `tint.name`, …).

Two things are deliberately not wrapped:

- **A literal hex colour**, such as `scrim.dark.colour: "#050508"`. It names no entry
  in this file — it's its own value — so it stays a bare `#`-prefixed string.
- **The word `"session"`**, the one case this file cannot resolve on its own: it
  means "the active session's own colour, assigned at runtime (R26), not a fixed
  value here." It appears as `themes[].cursor`, `focus_ring.colour`, and
  `tint.colour`, and nowhere else.

So a colour-position value in this file is always exactly one of: `{"ref": "<name>"}`,
a bare `#`-prefixed hex literal, or the bare string `"session"`. Nothing else.

## Top-level shape

- `artifact_version`, `artifact_url` — where this was exported from.
- `colours` — every named colour (18: the design's 17, plus `track`), each with
  `name`, `dark`, `light`, `use`.
- `colour_aliases` — names that are themselves a reference to one of `colours`
  (`blocked`, `done`, `error`), never a second hex for the same value.
- `opacities` — named values that are an opacity, not a colour (`faint`).
- `ramps` — the two named ramps (`ramp`, `ramp.extended`), each an ordered list of
  colour references with a stop percentage.
- `tint` — the session-colour-at-18% fill, referencing `"session"`.
- `type`, `sizes`, `motion` — every other named value, each a number with its unit,
  a list, or a small structured object — never a sentence a program must parse.
- `themes` — the two terminal themes, each with 16 named ANSI colours.
- `focus_ring` — width, offset, a session-colour reference, and
  `corner_radius_addition` (a plain number added to whatever control radius it
  rings — not a formula string, which is what AC#2 rules out).

## What is deliberately not here

A value the design does not actually fix is not guessed into this file. Carried
forward instead, as open questions for Bruk (doc-10's "Values in the design that are
not yet confirmed", and decision-10): the smallest pane, the pane header's
breakpoints, the overlay's 40px-clear-below-720px rule and its own max-height-as-
percent-of-window (one bullet in doc-10, not two settled values), the status bar's
roll-out time, the failure flakes' count and timing, and the pressed state's exact
darkening.

## What `steg-design-check` checks against the saved pages, and what it does not see

`tools/src/steg_tools/design_pages.py` (`just check-design`) compares this file against
`design/canvas/project/{Tokens,Details}.dc.html`'s rendered markup, by name and value,
in both directions.

**This file's own shape is closed.** `design_pages.py`'s `SCHEMA` describes the document
position by position - at the top level, these keys and no others; under `colours`, a
list open to a new entry but each entry closed to exactly `name`/`dark`/`light`/`use`;
under `sizes`, these named groups; and so on, for every section. `validate()` walks the
real file against `SCHEMA` and refuses any key at any position the schema doesn't name,
naming its path - there is no exclusion list and no prefix rule involved, so a stray key
cannot pass because it happens to share a name, or a string prefix, with something the
schema does describe elsewhere. `just check-design` runs this before anything else, so a
malformed shape (a colour missing `name`, a `dark` that isn't a string) is a named
refusal, never a crash.

This task tried three narrower mechanisms first, and a review found each one's blind
spot in turn: a fixed tuple of field names to look at (invisible to a field not already
in it); a string-prefix "is this covered by that" test (defeated by this file's own keys
containing literal dots - `font.ui`, `nav.side`, `radius.surface`, `ramp.extended` - so
`sizes["palette.footer"]` passed as "covered by" the unrelated `palette`); and a fixed
exclusion list (`name`/`use`/`unit`/`ansi`/`artifact_version`/`artifact_url`) applied at
every position regardless of whether that position's own reason for excluding it held -
so `focus_ring["ansi"]`, `colours[0]["unit"]` and `colours[0]["artifact_version"]` all
passed, because the exclusion never checked *where* it was being applied. `SCHEMA`
replaces all three: `name`/`use`/`unit`/`ansi` are allowed only at the specific positions
that actually list them (a theme's `ansi`, not `focus_ring`'s; a colour's `use`, not a
`unit` field it doesn't have at all).

What the schema classifies each position as:

- **Checked.** Every colour (18 of 18) and every ANSI colour (32): name and both hex
  forms must match, or the tool refuses, naming the value and both forms. 16 `type`/
  `sizes` entries, plus the three corner sizes (`radius.surface`/`track`/`inner`, via a
  small reviewed map to the page's one merged `radius` row) are checked by name, but **by
  default only as a set of numbers**: a sentence such as `tab.width`'s "120px usual, 240px
  at most, 72px at least" is checked for "every number in the file's entry is present
  somewhere in this sentence", not for which number belongs to which field. Two of an
  entry's own fields swapped (its numbers unchanged, just attached to the wrong field) is
  **not** caught by this weaker check alone. `FIELD_WORD_MAPS`/`RADIUS_WORD_MAP` tighten
  specific fields where the page's own sentence ties a number to a distinguishing word
  right after it (`tab.width`'s three fields all tighten this way; most other entries
  have at least one field that does not, because its word comes before the number
  instead of after, or shares a clause with another number) - those fields stay on the
  weaker set check, which the tool's own passing output says plainly every time it runs.

  A Tier B/mapped position, and a theme's `ansi`, is terminal *by design*: the schema
  doesn't describe what's inside `sizes.nav.side` field by field, because the checking
  function that position names already recurses the whole value (`_numbers_in` sees
  every number under it; `ansi_mismatches` every ANSI colour). A new *numeric* field
  added inside one is still inspected; a new *non-numeric* field the same way is a known,
  accepted gap of the same shape as the field-blindness above, not a silent one.

- **Declared unseen.** Everything else named in this file - `colour_aliases`,
  `opacities`, `ramps` (`stops` and, on `"ramp"` only, `caption_opacity`), `tint`,
  `motion.status-fade`, `focus_ring.width`/`offset`/`colour`/`corner_radius_addition`,
  `session_colour`, each theme's `bg`/`fg`/`bg_inactive`/`selection`/`cursor`, and every
  `sizes` key that lives only on Layout.dc.html - is declared out of this tool's reach,
  each with its own reason, printed on every passing run, not silently passed over.
  Every colour, colour alias, opacity and ramp is registered field by field (a colour's
  `dark`/`light`; an alias's `ref`; an opacity's `dark`/`light`; a ramp's `stops` and,
  on `"ramp"` only, `caption_opacity`) - a stray field beyond those on an otherwise-known
  entry (`contrast` on a colour, say) is refused, not swallowed by the entry's name
  being known.

- **Descriptive.** `name`, `use` and `unit` only at the specific positions that have
  them (a colour's `use`, not `unit`, which it has no such field for at all), plus the
  two top-level export-provenance fields, `artifact_version`/`artifact_url`.

A test enumerates every leaf path of the real file from an enumeration written
independently of `SCHEMA` (so a bug in one cannot be masked by the other sharing it) and
asserts each one is classified checked, declared unseen or descriptive by the tool - the
three counts must add up to the leaf count the test found on its own, so a key that
falls through all three silently breaks the arithmetic, not just a missed assertion.

Agreement between this file and a page proves nothing about whether a value is part of
the *confirmed* design (see "What is deliberately not here", above) - this tool is a
consistency check between two documents, not a substitute for reading doc-10.
